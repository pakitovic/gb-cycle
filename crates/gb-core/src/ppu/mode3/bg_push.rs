use super::*;

impl Ppu {
    pub(in crate::ppu) fn advance_bg_push_stage(&mut self) -> BgPushDotResult {
        let ownership = self.current_bg_push_dot_ownership();
        self.execute_bg_push_dot_ownership(ownership)
    }

    pub(in crate::ppu) fn current_mode3_bg_pipeline_region(&self) -> PpuStepRegion {
        if self.runtime.bg_pipeline_state.fill.pending
            || self.runtime.bg_pipeline_state.push.pending
            || matches!(
                self.runtime.bg_pipeline_state.fetcher.stage,
                PpuBgFetcherStage::Push
            )
        {
            return PpuStepRegion::Mode3Push;
        }

        if matches!(
            self.runtime.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::WindowActivating
        ) || self.runtime.bg_pipeline_state.fetcher.source == PpuBgFetcherSource::Window
        {
            PpuStepRegion::Mode3WindowFetch
        } else {
            PpuStepRegion::Mode3BgFetch
        }
    }

    #[cfg(test)]
    pub(in crate::ppu) fn advance_bg_push(&mut self) -> BgPushDotResult {
        self.execute_bg_push_dot_ownership(self.current_bg_push_dot_ownership())
    }

    pub(in crate::ppu) fn current_bg_push_dot_ownership(&self) -> BgPushDotOwnership {
        let push = self.runtime.bg_pipeline_state.push;
        if !push.pending || push.disposition != BgPushDisposition::Ready {
            return BgPushDotOwnership::NotReady;
        }

        if push.entry_delay_remaining > 0 {
            return BgPushDotOwnership::EntryDelay;
        }

        let push_can_start_object_fetch = self.runtime.obj_pipeline_state.fetch.stage
            == PpuObjFetcherStage::Idle
            && !push.just_activated_window_tile
            && self.obj_enabled()
            && self.current_dot_has_pending_obj_hit()
            && (!push.cached.is_startup_alignment_seed()
                || self.runtime.bg_pipeline_state.current_transfer_x < 8);
        if self.runtime.bg_pipeline_state.fifo_contains_real_pixels() {
            if push_can_start_object_fetch {
                BgPushDotOwnership::FifoBackedTransferObjectFetch
            } else {
                BgPushDotOwnership::WaitingForEmptyFifo
            }
        } else if push_can_start_object_fetch {
            BgPushDotOwnership::QueueFillThenObjectFetch
        } else {
            BgPushDotOwnership::QueueFill
        }
    }

    pub(in crate::ppu) fn execute_bg_push_dot_ownership(
        &mut self,
        ownership: BgPushDotOwnership,
    ) -> BgPushDotResult {
        match ownership {
            BgPushDotOwnership::NotReady => BgPushDotResult::NotReady,
            BgPushDotOwnership::EntryDelay => {
                debug_assert!(self.runtime.bg_pipeline_state.push.entry_delay_remaining > 0);
                self.runtime.bg_pipeline_state.push.entry_delay_remaining -= 1;
                self.runtime
                    .bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open = true;
                BgPushDotResult::EntryDelay
            }
            BgPushDotOwnership::WaitingForEmptyFifo => {
                if self
                    .runtime
                    .bg_pipeline_state
                    .push
                    .terminal_placeholder_tail_extra_hold_remaining
                    > 0
                {
                    self.runtime
                        .bg_pipeline_state
                        .push
                        .terminal_placeholder_tail_extra_hold_remaining -= 1;
                }
                self.runtime
                    .bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open =
                    self.runtime.bg_pipeline_state.push.cached.source
                        == PpuBgFetcherSource::Background
                        && self.runtime.bg_pipeline_state.push.cached.fetch_x
                            == BG_TILE_WIDTH as u16
                        && self.runtime.bg_pipeline_state.fifo.len()
                            == self.runtime.bg_pipeline_state.startup_fifo_placeholders as usize
                                + 2;
                BgPushDotResult::WaitingForEmptyFifo
            }
            BgPushDotOwnership::FifoBackedTransferObjectFetch => {
                self.runtime
                    .bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open = false;
                let started = self.try_start_object_fetch_from_current_dot(
                    ObjFetchStartSource::PushCachedBgFetch,
                );
                debug_assert!(
                    started,
                    "fifo-backed push ownership must only be selected when OBJ fetch can start"
                );
                BgPushDotResult::HandedOffToObjectFetch
            }
            BgPushDotOwnership::QueueFill | BgPushDotOwnership::QueueFillThenObjectFetch => {
                self.queue_bg_fill_from_push();
                if matches!(ownership, BgPushDotOwnership::QueueFillThenObjectFetch) {
                    let started = self
                        .try_start_object_fetch_from_current_dot(ObjFetchStartSource::QueuedBgFill);
                    debug_assert!(
                        started,
                        "queued-fill push ownership must only be selected when OBJ fetch can start"
                    );
                    BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
                } else {
                    BgPushDotResult::QueuedFill
                }
            }
        }
    }

    pub(in crate::ppu) fn queue_bg_fill_from_push(&mut self) {
        let push = self.runtime.bg_pipeline_state.push;
        if push.cached.is_startup_alignment_seed() {
            self.runtime
                .bg_pipeline_state
                .begin_post_alignment_followup();
            // The leading junk pixels are already real FIFO entries (pre-filled at start_line), so
            // the seed fill only appends its real tile pixels behind whatever junk remains — it no
            // longer re-materializes the placeholder count as dummies. See docs/roadmap/12 §24.
            self.runtime
                .bg_pipeline_state
                .fill
                .queue_startup_alignment_from_push(push, 0);
        } else {
            self.runtime.bg_pipeline_state.fill.queue_from_push(push);
        }
        self.runtime
            .bg_pipeline_state
            .maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_fill();
        self.runtime
            .bg_pipeline_state
            .maybe_apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_fill();
        self.runtime
            .bg_pipeline_state
            .apply_startup_scy_tiledata_latch_to_fill();
        self.runtime.bg_pipeline_state.fetcher.fetch_x = push.next_fetch_pixel;
        self.runtime.bg_pipeline_state.fetcher.next_fetch_pixel = push.next_fetch_pixel;
        self.runtime
            .bg_pipeline_state
            .fetcher
            .post_alignment_fetch_restart_delay_dots = if push.cached.is_startup_alignment_seed() {
            1
        } else {
            0
        };
        self.runtime.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
        self.runtime.bg_pipeline_state.push.reset();
    }

    pub(in crate::ppu) fn flush_pending_bg_fifo_fill(&mut self) {
        if !self.runtime.bg_pipeline_state.fill.pending {
            return;
        }

        self.runtime
            .bg_pipeline_state
            .maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_fill();
        self.runtime
            .bg_pipeline_state
            .maybe_apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_fill();
        let fill = self.runtime.bg_pipeline_state.fill;
        if fill.startup_dummy_pixels > 0 {
            self.runtime
                .bg_pipeline_state
                .push_dummy_fifo_pixels(fill.startup_dummy_pixels);
        }
        if fill.includes_real_tile_pixels {
            self.runtime
                .bg_pipeline_state
                .push_cached_slice_fifo_pixels_with_skip(fill.cached, fill.leading_pixel_skip);
        }
        self.runtime.bg_pipeline_state.fill.reset();
    }

    pub(in crate::ppu) fn maybe_recompute_pending_background_fill(
        &mut self,
        vram: &VramBusView<'_>,
    ) {
        if !self.runtime.bg_pipeline_state.fill.pending
            || !self
                .runtime
                .bg_pipeline_state
                .fill
                .includes_real_tile_pixels
        {
            return;
        }

        let Some(recomputed) = recompute_live_background_cached_slice(
            self.runtime.bg_pipeline_state.fill.cached,
            vram,
            self.current_mode3_live_background_refetch_context(),
        ) else {
            return;
        };

        self.runtime.bg_pipeline_state.fill.cached = recomputed;
    }

    pub(in crate::ppu) fn maybe_recompute_pending_background_push(
        &mut self,
        vram: &VramBusView<'_>,
    ) {
        if !self.runtime.bg_pipeline_state.push.pending {
            return;
        }

        let Some(recomputed) = recompute_live_background_cached_slice(
            self.runtime.bg_pipeline_state.push.cached,
            vram,
            self.current_mode3_live_background_refetch_context(),
        ) else {
            return;
        };

        self.runtime.bg_pipeline_state.push.cached = recomputed;
        self.runtime.bg_pipeline_state.fetcher.tile_map_address = recomputed.tile_map_address;
        self.runtime.bg_pipeline_state.fetcher.tile_index = recomputed.tile_index;
        self.runtime.bg_pipeline_state.fetcher.tile_data_address = recomputed.tile_data_address;
        self.runtime.bg_pipeline_state.fetcher.tile_low_address = recomputed.tile_low_address;
        self.runtime.bg_pipeline_state.fetcher.tile_high_address = recomputed.tile_high_address;
        self.runtime.bg_pipeline_state.fetcher.tile_low = recomputed.tile_low;
        self.runtime.bg_pipeline_state.fetcher.tile_high = recomputed.tile_high;
        self.runtime.bg_pipeline_state.fetcher.cgb_bg_attrs = recomputed.cgb_bg_attrs;
    }
}
