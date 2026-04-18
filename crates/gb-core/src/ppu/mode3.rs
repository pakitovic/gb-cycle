use super::*;

impl Ppu {
    pub(super) fn advance_mode3_pipeline<O>(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
        observer: &mut O,
    ) where
        O: PpuStepObserver,
    {
        if self.ly >= VISIBLE_SCANLINES || self.line_dot < MODE2_DOTS {
            return;
        }

        if !self.bg_pipeline_state.mode3_started {
            observe_ppu_step_region(observer, PpuStepRegion::Mode3Startup, || {
                self.bg_pipeline_state
                    .start_line(self.mode3_register_latches().mode3_start_scx());
                self.obj_pipeline_state.mode3_line_start_obj_height =
                    self.mode3_register_latches().current_obj_height();
            });
        }
        if self.line_dot == MODE2_DOTS + MODE3_INITIAL_SCX_CAPTURE_DOT {
            observe_ppu_step_region(observer, PpuStepRegion::Mode3Startup, || {
                self.bg_pipeline_state
                    .capture_initial_scx(self.mode3_register_latches().mode3_start_scx());
            });
        }
        observe_ppu_step_region(observer, PpuStepRegion::Mode3Startup, || {
            self.maybe_retune_previsible_live_scx_discard();
        });
        if self.line_dot >= self.current_mode0_start_dot() {
            return;
        }

        let bg_pipeline_region = self.current_mode3_bg_pipeline_region();
        observe_ppu_step_region(observer, bg_pipeline_region, || {
            self.maybe_recompute_pending_background_fill(vram);
            self.flush_pending_bg_fifo_fill();
            self.apply_pending_dmg_lcdc2_observed_write_effects(vram);
        });

        if observe_ppu_step_region(observer, PpuStepRegion::Mode3ObjFetch, || {
            self.advance_mode3_object_phase(oam, vram, dma_oam_conflict)
        }) {
            return;
        }

        let output_dot =
            observe_ppu_step_region(observer, PpuStepRegion::Mode3PixelTransfer, || {
                self.advance_mode3_output_phase_with_vram(vram)
            });
        observe_ppu_step_region(observer, PpuStepRegion::Mode3WindowFetch, || {
            self.maybe_apply_wx0_shortening_after_transfer_dot(output_dot);
            let _ = self.maybe_start_window_after_transfer_dot(output_dot);
        });
        let bg_pipeline_region = self.current_mode3_bg_pipeline_region();
        let _ = observe_ppu_step_region(observer, bg_pipeline_region, || {
            self.advance_bg_fetcher(vram)
        });
    }

    pub(super) fn advance_mode3_object_phase(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        self.sync_pending_obj_hit_ownership();
        self.latch_object_fetch_hits();
        let started = self
            .try_start_object_fetch_from_current_dot(ObjFetchStartSource::FifoBackedTransfer, true);
        if started && self.terminal_mode3_dot_started_shared_obj_fetch() {
            self.bg_pipeline_state.extend_mode3_by_one_dot();
        }
        self.advance_object_fetch(oam, vram, dma_oam_conflict)
    }

    pub(super) fn advance_mode3_output_phase_with_vram(
        &mut self,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
        if self
            .bg_pipeline_state
            .consume_startup_transfer_entry_delay_dot()
        {
            self.consume_dmg_bgp_cpu_commit_output_delay();
            return Mode3TransferDot::not_served();
        }

        let transfer_dot = if !self.current_dot_arbitration().can_serve_bg_transfer() {
            self.bg_pipeline_state.extend_mode3_by_one_dot();
            Mode3TransferDot::not_served()
        } else {
            match self.current_transfer() {
                None => Mode3TransferDot::not_served(),
                Some(Mode3CurrentTransfer {
                    readiness: Mode3TransferReadiness::WaitingForFifo(_),
                    ..
                }) => {
                    self.bg_pipeline_state.extend_mode3_by_one_dot();
                    Mode3TransferDot::not_served()
                }
                Some(Mode3CurrentTransfer {
                    readiness: Mode3TransferReadiness::Ready(plan),
                    ..
                }) => self.execute_transfer_service_plan(plan, vram),
            }
        };

        self.bg_pipeline_state.consume_startup_source_window_dot();
        if transfer_dot.kind != Mode3TransferDotKind::ServedVisiblePixel {
            self.repeat_last_dmg_recent_panel_dot();
        }
        self.consume_dmg_bgp_cpu_commit_output_delay();
        transfer_dot
    }

    #[cfg(test)]
    pub(super) fn advance_mode3_output_phase(&mut self) -> Mode3TransferDot {
        let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
        vram.set_acquired(BusMaster::Ppu, true);
        self.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram))
    }

    fn maybe_retune_previsible_live_scx_discard(&mut self) {
        if !self.console_model.is_dmg_family()
            || self.bg_pipeline_state.window_started_this_line
            || !self.bg_pipeline_state.startup_alignment_seed_pending()
        {
            return;
        }

        self.bg_pipeline_state
            .retune_previsible_scx_discard(self.mode3_register_latches().visible().scx);
    }

    pub(super) fn current_dot_has_pending_obj_hit(&self) -> bool {
        self.obj_enabled()
            && self
                .obj_pipeline_state
                .pending_hits_own_current_dot(self.current_obj_hit_ownership())
    }

    pub(super) fn current_dot_arbitration(&self) -> Mode3DotArbitration {
        let has_pending_obj_hit = self.current_dot_has_pending_obj_hit();
        let obj_fetch_can_start = self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_enabled()
            && has_pending_obj_hit;
        let current_transfer_is_fifo_backed = self.current_transfer().is_some_and(|transfer| {
            (transfer.can_start_obj_fetch_from_fifo_backed_transfer(
                self.bg_pipeline_state.fifo_contains_real_pixels(),
            ) || self.previsible_same_x_chain_can_start_obj_fetch(transfer))
                && self.bg_fetcher_ready_for_fifo_backed_obj_start()
        });

        Mode3DotArbitration {
            bg_transfer_can_advance: !has_pending_obj_hit,
            obj_fetch_can_start_from_fifo_backed_transfer: obj_fetch_can_start
                && current_transfer_is_fifo_backed,
            obj_fetch_can_start_from_queued_bg_fill: obj_fetch_can_start,
        }
    }

    pub(super) fn previsible_same_x_chain_can_start_obj_fetch(
        &self,
        transfer: Mode3CurrentTransfer,
    ) -> bool {
        matches!(
            (transfer.context.lane, transfer.readiness),
            (
                Mode3TransferLane::PreVisible,
                Mode3TransferReadiness::Ready(Mode3TransferServicePlan {
                    execution: Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop,
                    ..
                }),
            )
        ) && !self.bg_pipeline_state.effective_fifo_is_empty()
            && self.obj_pipeline_state.pending_match_x
                == Some(self.bg_pipeline_state.current_transfer_x)
            && !self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && match transfer.context.source_window {
                Mode3TransferSourceWindow::AbstractStartup => {
                    self.fetched_same_x_obj_sprite_count_for_pending_match_x() > 0
                }
                Mode3TransferSourceWindow::FifoBacked => {
                    self.previsible_fifo_backed_same_x_chain_can_start_obj_fetch()
                }
            }
    }

    pub(super) fn previsible_fifo_backed_same_x_chain_can_start_obj_fetch(&self) -> bool {
        if !self.current_transfer_x_supports_early_same_x_obj_start() {
            return false;
        }

        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_pending_match_x();
        matches!(fetched_same_x_count, 1 | 3)
            || (fetched_same_x_count >= 2 && fetched_same_x_count.is_multiple_of(2))
            || self.terminal_previsible_same_x_chain_can_start_obj_fetch()
    }

    #[cfg(test)]
    pub(super) fn current_transfer_service_plan(&self) -> Option<Mode3TransferServicePlan> {
        self.current_transfer()
            .map(|transfer| transfer.service_plan())
    }

    pub(super) fn current_transfer(&self) -> Option<Mode3CurrentTransfer> {
        self.mode3_transfer_policy().current_transfer(
            self.bg_pipeline_state.fifo.is_empty(),
            self.bg_pipeline_state.effective_fifo_is_empty(),
        )
    }

    pub(super) fn advance_bg_fetcher(&mut self, vram: &VramBusView<'_>) -> bool {
        self.maybe_abort_window_fetcher_to_background();
        self.maybe_recompute_pending_background_push(vram);
        let fetch_policy = self.mode3_bgwin_fetch_policy();

        match (
            self.bg_pipeline_state.fetcher.stage,
            self.bg_pipeline_state.fetcher.stage_dot,
        ) {
            (PpuBgFetcherStage::Idle, _) => {
                self.bg_pipeline_state.fetcher.start_background();
                return false;
            }
            (PpuBgFetcherStage::WindowActivating, _) => {
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
                return false;
            }
            (PpuBgFetcherStage::Push, _) => {
                return matches!(
                    self.advance_bg_push_stage(),
                    BgPushDotResult::HandedOffToObjectFetch
                        | BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
                );
            }
            _ => {}
        }

        if self
            .bg_pipeline_state
            .fetcher
            .post_alignment_fetch_restart_delay_dots
            > 0
        {
            self.bg_pipeline_state
                .fetcher
                .post_alignment_fetch_restart_delay_dots -= 1;
            return false;
        }

        let fetcher = self.bg_pipeline_state.fetcher;
        match (fetcher.stage, fetcher.stage_dot) {
            (PpuBgFetcherStage::TileIndex, 0) => {
                self.bg_pipeline_state
                    .fetcher
                    .needs_live_tilemap_refetch_on_push = false;
                self.bg_pipeline_state
                    .fetcher
                    .needs_live_tilemap_full_refetch_on_push = false;
                self.bg_pipeline_state
                    .fetcher
                    .needs_live_tile_data_refetch_on_push = false;
                self.bg_pipeline_state
                    .fetcher
                    .needs_live_tile_data_current_row_refetch_on_push = false;
                self.bg_pipeline_state
                    .fetcher
                    .needs_live_tile_low_current_row_refetch_on_push = false;
                self.bg_pipeline_state
                    .fetcher
                    .needs_live_tile_high_current_row_refetch_on_push = false;
                if fetcher.source == PpuBgFetcherSource::Background {
                    self.bg_pipeline_state.fetcher.cached_origin = self
                        .bg_pipeline_state
                        .peek_startup_background_fetch_origin();
                }
                let tile_map_address =
                    self.compute_fetch_tile_index_address(fetcher.source, fetcher.fetch_x);
                self.bg_pipeline_state.fetcher.tile_map_address = tile_map_address;
                let delay_tileindex_read =
                    fetch_policy.should_delay_background_tileindex_read(fetcher.source);
                if !delay_tileindex_read {
                    self.bg_pipeline_state.fetcher.tile_index =
                        vram.read(tile_map_address as usize).unwrap_or(0);
                }
                if fetcher.source == PpuBgFetcherSource::Window {
                    self.bg_pipeline_state.fetcher.window_tilemap_x = self
                        .bg_pipeline_state
                        .fetcher
                        .window_tilemap_x
                        .wrapping_add(1);
                }
                if self
                    .bg_pipeline_state
                    .fetcher
                    .rewind_bg_resume_after_first_tile_index_dot
                {
                    self.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = self
                        .bg_pipeline_state
                        .fetcher
                        .bg_resume_fetch_pixel
                        .wrapping_sub(BG_TILE_WIDTH as u16);
                    self.bg_pipeline_state
                        .fetcher
                        .rewind_bg_resume_after_first_tile_index_dot = false;
                }
                self.bg_pipeline_state
                    .fetcher
                    .same_cycle_window_tilemap_lcdc_hold = false;
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileIndex, 1) => {
                if fetch_policy.should_delay_background_tileindex_read(fetcher.source) {
                    self.bg_pipeline_state.fetcher.tile_index = vram
                        .read(self.bg_pipeline_state.fetcher.tile_map_address as usize)
                        .unwrap_or(0);
                }
                self.maybe_apply_bgwin_tilemap_selector_glitch(vram, fetcher.source);
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
            }
            (PpuBgFetcherStage::TileDataLow, 0) => {
                let tile_data_address = if fetcher.source == PpuBgFetcherSource::Window {
                    self.bg_pipeline_state
                        .fetcher
                        .dmg_lcdc4_previous_tiledata_select_on_next_low
                        .take()
                        .map_or_else(
                            || {
                                self.compute_fetch_tile_data_address(
                                    fetcher.source,
                                    fetcher.fetch_x,
                                    fetcher.tile_index,
                                    0,
                                )
                            },
                            |selector| {
                                self.compute_window_fetch_tile_data_address_with_selector(
                                    fetcher.tile_index,
                                    0,
                                    selector,
                                )
                            },
                        )
                } else {
                    self.compute_fetch_tile_data_address(
                        fetcher.source,
                        fetcher.fetch_x,
                        fetcher.tile_index,
                        0,
                    )
                };
                self.bg_pipeline_state.fetcher.tile_data_address = tile_data_address;
                self.bg_pipeline_state.fetcher.tile_low_address = tile_data_address;
                let tile_data = vram.read(tile_data_address as usize).unwrap_or(0);
                self.bg_pipeline_state.fetcher.tile_low = tile_data;
                self.maybe_cache_unsigned_bgwin_tile_data_fetch(
                    fetcher.source,
                    fetcher.fetch_x,
                    0,
                    tile_data,
                );
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileDataLow, 1) => {
                self.maybe_apply_bgwin_tile_data_selector_glitch(vram, fetcher.source, 0);
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
            }
            (PpuBgFetcherStage::TileDataHigh, 0) => {
                let tile_data_address = self.compute_fetch_tile_data_address(
                    fetcher.source,
                    fetcher.fetch_x,
                    fetcher.tile_index,
                    1,
                );
                self.bg_pipeline_state.fetcher.tile_data_address = tile_data_address;
                self.bg_pipeline_state.fetcher.tile_high_address = tile_data_address;
                let tile_data = vram.read(tile_data_address as usize).unwrap_or(0);
                self.bg_pipeline_state.fetcher.tile_high = tile_data;
                self.maybe_cache_unsigned_bgwin_tile_data_fetch(
                    fetcher.source,
                    fetcher.fetch_x,
                    1,
                    tile_data,
                );
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileDataHigh, 1) => {
                self.maybe_apply_bgwin_tile_data_selector_glitch(vram, fetcher.source, 1);
                if self.bg_pipeline_state.startup_alignment_seed_pending() {
                    self.bg_pipeline_state
                        .push
                        .queue_startup_alignment_seed_from_fetcher(self.bg_pipeline_state.fetcher);
                    self.bg_pipeline_state
                        .fetcher
                        .startup_visible_tile3_scx_boundary_full_refetch_next_tile = false;
                    self.bg_pipeline_state
                        .fetcher
                        .first_window_tile_after_activation = false;
                    self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
                    self.bg_pipeline_state.fetcher.stage_dot = 0;
                    let push_result = self.advance_bg_push_stage();
                    if matches!(
                        push_result,
                        BgPushDotResult::HandedOffToObjectFetch
                            | BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
                    ) {
                        return true;
                    }
                    return false;
                }
                self.bg_pipeline_state
                    .maybe_attach_startup_visible_tile3_scx_boundary_next_slice_to_fetcher();
                self.bg_pipeline_state
                    .push
                    .queue_from_fetcher(self.bg_pipeline_state.fetcher);
                self.bg_pipeline_state
                    .maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_push();
                self.bg_pipeline_state
                    .maybe_apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_push();
                self.bg_pipeline_state
                    .fetcher
                    .startup_visible_tile3_scx_boundary_full_refetch_next_tile = false;
                self.bg_pipeline_state
                    .fetcher
                    .clear_startup_visible_tile3_scx_boundary_old_pixel_window();
                if fetcher.source == PpuBgFetcherSource::Background {
                    self.bg_pipeline_state
                        .advance_startup_background_fetch_tile();
                }
                let mut advance_push_immediately = false;
                if self
                    .bg_pipeline_state
                    .take_startup_first_real_push_skip_entry_delay()
                {
                    self.bg_pipeline_state.push.entry_delay_remaining = 0;
                    advance_push_immediately = true;
                }
                self.bg_pipeline_state
                    .fetcher
                    .first_window_tile_after_activation = false;
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
                if advance_push_immediately {
                    return matches!(
                        self.advance_bg_push_stage(),
                        BgPushDotResult::HandedOffToObjectFetch
                            | BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
                    );
                }
            }
            (PpuBgFetcherStage::Idle, _)
            | (PpuBgFetcherStage::WindowActivating, _)
            | (PpuBgFetcherStage::Push, _) => unreachable!(
                "special BG fetcher stages are handled before the explicit dot-stage automaton"
            ),
            (_, other_dot) => unreachable!(
                "invalid BG fetcher stage_dot {other_dot} for non-push stage {:?}",
                fetcher.stage
            ),
        }

        false
    }

    pub(super) fn maybe_abort_window_fetcher_to_background(&mut self) {
        if self.bg_pipeline_state.fetcher.source != PpuBgFetcherSource::Window {
            return;
        }

        if self.mode3_window_policy().fetcher_should_stay_windowed() {
            return;
        }

        self.bg_pipeline_state.fetcher.abort_window_to_background();
    }

    pub(super) fn advance_bg_push_stage(&mut self) -> BgPushDotResult {
        let ownership = self.current_bg_push_dot_ownership();
        self.execute_bg_push_dot_ownership(ownership)
    }

    pub(super) fn current_step_region_after_line_advance(&self) -> PpuStepRegion {
        let next_line_dot = self.line_dot + 1;
        let next_lcd_restart_phase = self.lcd_restart_phase.advance(self.ly, next_line_dot);
        if let Some(raster_state) = next_lcd_restart_phase.raster_state(self.ly, next_line_dot) {
            return match raster_state.access_mode() {
                PpuAccessMode::Drawing => PpuStepRegion::Mode3Startup,
                PpuAccessMode::HBlank | PpuAccessMode::VBlank => PpuStepRegion::Mode0Or1,
                PpuAccessMode::OamScan => PpuStepRegion::Mode2Scan,
            };
        }

        if self.ly >= VISIBLE_SCANLINES || next_line_dot >= self.current_mode0_start_dot() {
            return PpuStepRegion::Mode0Or1;
        }

        if next_line_dot < MODE2_DOTS {
            return PpuStepRegion::Mode2Scan;
        }

        if !self.bg_pipeline_state.mode3_started {
            return PpuStepRegion::Mode3Startup;
        }

        PpuStepRegion::Other
    }

    pub(super) fn current_mode3_bg_pipeline_region(&self) -> PpuStepRegion {
        if self.bg_pipeline_state.fill.pending
            || self.bg_pipeline_state.push.pending
            || matches!(
                self.bg_pipeline_state.fetcher.stage,
                PpuBgFetcherStage::Push
            )
        {
            return PpuStepRegion::Mode3Push;
        }

        if matches!(
            self.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::WindowActivating
        ) || self.bg_pipeline_state.fetcher.source == PpuBgFetcherSource::Window
        {
            PpuStepRegion::Mode3WindowFetch
        } else {
            PpuStepRegion::Mode3BgFetch
        }
    }

    #[cfg(test)]
    pub(super) fn advance_bg_push(&mut self) -> BgPushDotResult {
        self.execute_bg_push_dot_ownership(self.current_bg_push_dot_ownership())
    }

    pub(super) fn current_bg_push_dot_ownership(&self) -> BgPushDotOwnership {
        let push = self.bg_pipeline_state.push;
        if !push.pending || push.disposition != BgPushDisposition::Ready {
            return BgPushDotOwnership::NotReady;
        }

        if push.entry_delay_remaining > 0 {
            return BgPushDotOwnership::EntryDelay;
        }

        let push_can_start_object_fetch = self.obj_pipeline_state.fetch.stage
            == PpuObjFetcherStage::Idle
            && !push.just_activated_window_tile
            && self.obj_enabled()
            && self.current_dot_has_pending_obj_hit()
            && (!push.cached.is_startup_alignment_seed()
                || self.bg_pipeline_state.current_transfer_x < 8);
        if self.bg_pipeline_state.fifo_contains_real_pixels() {
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

    pub(super) fn execute_bg_push_dot_ownership(
        &mut self,
        ownership: BgPushDotOwnership,
    ) -> BgPushDotResult {
        match ownership {
            BgPushDotOwnership::NotReady => BgPushDotResult::NotReady,
            BgPushDotOwnership::EntryDelay => {
                debug_assert!(self.bg_pipeline_state.push.entry_delay_remaining > 0);
                self.bg_pipeline_state.push.entry_delay_remaining -= 1;
                if self.bg_pipeline_state.push.entry_delay_remaining == 0
                    && self
                        .saturated_placeholder_backed_terminal_bg_tail_can_hold_one_post_push_dot()
                {
                    self.bg_pipeline_state
                        .push
                        .terminal_placeholder_tail_extra_hold_remaining = 2;
                }
                self.bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open = true;
                BgPushDotResult::EntryDelay
            }
            BgPushDotOwnership::WaitingForEmptyFifo => {
                if self
                    .bg_pipeline_state
                    .push
                    .terminal_placeholder_tail_extra_hold_remaining
                    > 0
                {
                    self.bg_pipeline_state
                        .push
                        .terminal_placeholder_tail_extra_hold_remaining -= 1;
                }
                self.bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open =
                    self.bg_pipeline_state.push.cached.source == PpuBgFetcherSource::Background
                        && self.bg_pipeline_state.push.cached.fetch_x == BG_TILE_WIDTH as u16
                        && self.bg_pipeline_state.fifo.len()
                            == self.bg_pipeline_state.startup_fifo_placeholders as usize + 2;
                BgPushDotResult::WaitingForEmptyFifo
            }
            BgPushDotOwnership::FifoBackedTransferObjectFetch => {
                self.bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open = false;
                let started = self.try_start_object_fetch_from_current_dot(
                    ObjFetchStartSource::PushCachedBgFetch,
                    true,
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
                    let started = self.try_start_object_fetch_from_current_dot(
                        ObjFetchStartSource::QueuedBgFill,
                        true,
                    );
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

    pub(super) fn queue_bg_fill_from_push(&mut self) {
        let push = self.bg_pipeline_state.push;
        if push.cached.is_startup_alignment_seed() {
            let startup_leading_pixel_skip = self.bg_pipeline_state.initial_scx_discard;
            self.bg_pipeline_state.begin_post_alignment_followup();
            self.bg_pipeline_state.scx_discard_remaining = 0;
            self.bg_pipeline_state
                .fill
                .queue_startup_alignment_from_push(
                    push,
                    self.bg_pipeline_state.startup_fifo_placeholders,
                    startup_leading_pixel_skip,
                );
        } else {
            self.bg_pipeline_state.fill.queue_from_push(push);
        }
        self.bg_pipeline_state
            .maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_fill();
        self.bg_pipeline_state
            .maybe_apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_fill();
        self.bg_pipeline_state
            .apply_startup_scy_tiledata_latch_to_fill();
        self.bg_pipeline_state.fetcher.fetch_x = push.next_fetch_pixel;
        self.bg_pipeline_state.fetcher.next_fetch_pixel = push.next_fetch_pixel;
        self.bg_pipeline_state
            .fetcher
            .post_alignment_fetch_restart_delay_dots = if push.cached.is_startup_alignment_seed() {
            1
        } else {
            0
        };
        self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
        self.bg_pipeline_state.push.reset();
    }

    pub(super) fn flush_pending_bg_fifo_fill(&mut self) {
        if !self.bg_pipeline_state.fill.pending {
            return;
        }

        self.bg_pipeline_state
            .maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_fill();
        self.bg_pipeline_state
            .maybe_apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_fill();
        let fill = self.bg_pipeline_state.fill;
        if fill.startup_dummy_pixels > 0 {
            self.bg_pipeline_state
                .push_dummy_fifo_pixels(fill.startup_dummy_pixels);
        }
        if fill.includes_real_tile_pixels {
            self.bg_pipeline_state
                .push_cached_slice_fifo_pixels_with_skip(
                    fill.cached,
                    fill.startup_leading_pixel_skip,
                );
        }
        self.bg_pipeline_state.fill.reset();
    }

    pub(super) fn maybe_recompute_pending_background_fill(&mut self, vram: &VramBusView<'_>) {
        if !self.bg_pipeline_state.fill.pending
            || !self.bg_pipeline_state.fill.includes_real_tile_pixels
        {
            return;
        }

        let Some(recomputed) = recompute_live_background_cached_slice(
            self.bg_pipeline_state.fill.cached,
            vram,
            self.current_mode3_live_background_refetch_context(),
        ) else {
            return;
        };

        self.bg_pipeline_state.fill.cached = recomputed;
    }

    pub(super) fn maybe_recompute_pending_background_push(&mut self, vram: &VramBusView<'_>) {
        if !self.bg_pipeline_state.push.pending {
            return;
        }

        let Some(recomputed) = recompute_live_background_cached_slice(
            self.bg_pipeline_state.push.cached,
            vram,
            self.current_mode3_live_background_refetch_context(),
        ) else {
            return;
        };

        self.bg_pipeline_state.push.cached = recomputed;
        self.bg_pipeline_state.fetcher.tile_map_address = recomputed.tile_map_address;
        self.bg_pipeline_state.fetcher.tile_index = recomputed.tile_index;
        self.bg_pipeline_state.fetcher.tile_data_address = recomputed.tile_data_address;
        self.bg_pipeline_state.fetcher.tile_low_address = recomputed.tile_low_address;
        self.bg_pipeline_state.fetcher.tile_high_address = recomputed.tile_high_address;
        self.bg_pipeline_state.fetcher.tile_low = recomputed.tile_low;
        self.bg_pipeline_state.fetcher.tile_high = recomputed.tile_high;
    }

    pub(super) fn execute_transfer_service_plan(
        &mut self,
        plan: Mode3TransferServicePlan,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
        self.apply_pending_dmg_window_lcdc4_output_repaint(vram);
        let pixel = if matches!(
            plan.execution,
            Mode3TransferServiceExecution::EmitVisiblePixel
        ) {
            None
        } else if plan.requires_real_bg_fifo_pixel() {
            self.bg_pipeline_state.pop_real_fifo_pixel()
        } else if plan.requires_effective_bg_fifo_pixel() {
            self.bg_pipeline_state.consume_effective_fifo_pixel()
        } else {
            None
        };

        self.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        if !matches!(
            plan.execution,
            Mode3TransferServiceExecution::ConsumeScxDiscard
                | Mode3TransferServiceExecution::EmitVisiblePixel
        ) {
            self.bg_pipeline_state
                .consume_startup_pre_visible_transfer_dot();
        }

        match plan.execution {
            Mode3TransferServiceExecution::ConsumeScxDiscard => {
                let _ = pixel.expect(
                    "startup scx discard must consume one effective BG FIFO slot before output",
                );
                self.bg_pipeline_state.scx_discard_remaining -= 1;
                Mode3TransferDot::served(plan.result_kind, true)
            }
            Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop => {
                let _ = pixel
                    .expect("pre-visible startup transfer must consume one effective BG FIFO slot");
                self.bg_pipeline_state.current_transfer_x += 1;
                Mode3TransferDot::served(plan.result_kind, false)
            }
            Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop => {
                let _ = pixel.expect("hidden transfer must consume one effective BG FIFO slot");
                self.bg_pipeline_state.current_transfer_x += 1;
                let _ = self.pop_obj_fifo_pixel();
                Mode3TransferDot::served(plan.result_kind, false)
            }
            Mode3TransferServiceExecution::EmitVisiblePixel => {
                let bg_pixel = self
                    .pop_visible_bg_fifo_pixel(vram)
                    .expect("visible transfer plans must carry a BG pixel");
                let bg_enabled = self.pixel_transfer_bg_enabled();
                let visible_x = self.bg_pipeline_state.visible_pixels_output;
                let bg_pixel = self
                    .compute_startup_visible_tile2_scy_placeholder_pixel(visible_x, vram)
                    .unwrap_or(bg_pixel);
                let effective_bg_priority_pixel = if bg_enabled { bg_pixel } else { 0 };
                let obj_pixel = self.pop_obj_fifo_pixel();
                let obj_pixel =
                    self.apply_dmg_lcdc2_live_obj_size_output_override(obj_pixel, visible_x, vram);
                let output_pixel =
                    self.mix_bg_and_obj(bg_pixel, effective_bg_priority_pixel, obj_pixel);
                let dmg_bg_forced_white =
                    self.dmg_bg_panel_dot_is_forced_white(bg_enabled, output_pixel);
                let panel_pixel = if self.visible_output == PpuVisibleOutputState::Driving {
                    if dmg_bg_forced_white {
                        0
                    } else {
                        self.map_mixed_pixel_to_panel_shade(output_pixel)
                    }
                } else {
                    0
                };
                let scanline_pixel = if self.visible_output == PpuVisibleOutputState::Driving
                    && !dmg_bg_forced_white
                {
                    output_pixel.color
                } else {
                    0
                };
                let visible_x = visible_x as usize;
                self.current_scanline_bg_pixels[visible_x] = bg_pixel;
                self.current_scanline_mixed_pixels[visible_x] = output_pixel;
                self.current_scanline_dmg_bg_forced_white[visible_x] = dmg_bg_forced_white;
                self.current_scanline_pixels[visible_x] = scanline_pixel;
                self.framebuffer[self.ly as usize * SCREEN_WIDTH + visible_x] = panel_pixel;
                self.record_dmg_recent_panel_dot(
                    visible_x as u8,
                    output_pixel,
                    dmg_bg_forced_white,
                );
                self.consume_dmg_lcdc0_bg_enable_visible_hold();
                self.consume_dmg_lcdc1_obj_enable_visible_hold();
                self.consume_dmg_bgp_cpu_commit_bg_visible_hold(output_pixel);
                self.bg_pipeline_state.current_transfer_x =
                    self.bg_pipeline_state.current_transfer_x.saturating_add(1);
                self.bg_pipeline_state.visible_pixels_output += 1;
                Mode3TransferDot::served(plan.result_kind, false)
            }
        }
    }

    pub(super) fn pop_visible_bg_fifo_pixel(&mut self, vram: &VramBusView<'_>) -> Option<u8> {
        let visible_x = self.bg_pipeline_state.visible_pixels_output as usize;
        let mut pixel = self.bg_pipeline_state.pop_visible_fifo_pixel()?;
        let Some(cached) = pixel.cached.as_mut() else {
            self.current_scanline_bg_dot_contexts[visible_x] = None;
            if let Some(override_pixel) = self.compute_startup_visible_tile2_scy_placeholder_pixel(
                self.bg_pipeline_state.visible_pixels_output,
                vram,
            ) {
                return Some(override_pixel);
            }
            return Some(pixel.color);
        };
        let visible_tile2_scy_tilemap_override = self
            .compute_startup_visible_tile2_scy_tilemap_retarget_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let window_activation_tilemap_override = self.compute_window_activation_tilemap_override(
            cached.cached,
            cached.pixel_index,
            vram,
        );
        let window_tiledata_selector_override = self
            .compute_window_lcdc4_tiledata_selector_override(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let next_tile_output_retarget = self
            .compute_startup_visible_tile3_scx_boundary_next_tile_output_retarget_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let old_pixel_override = self.compute_startup_visible_tile3_scx_boundary_old_pixel(
            cached.cached,
            cached.pixel_index,
            vram,
        );
        let low_band_shifted_override = self
            .compute_startup_visible_tile3_scx_low_band_shifted_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let visible_tile2_previous_row_override = self
            .compute_startup_visible_tile2_previous_row_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let visible_tile3_previous_row_override = self
            .compute_startup_visible_tile3_previous_row_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let Some(recomputed) = recompute_live_background_cached_slice(
            cached.cached,
            vram,
            self.current_mode3_live_background_refetch_context(),
        ) else {
            self.current_scanline_bg_dot_contexts[visible_x] = Some(PpuRecentBgDotContext {
                source: cached.cached.source,
                fetch_x: cached.cached.fetch_x,
                pixel_index: cached.pixel_index,
                tile_index: cached.cached.tile_index,
            });
            return Some(
                old_pixel_override
                    .or(window_activation_tilemap_override)
                    .or(window_tiledata_selector_override)
                    .or(low_band_shifted_override)
                    .or(visible_tile2_scy_tilemap_override)
                    .or(visible_tile2_previous_row_override)
                    .or(visible_tile3_previous_row_override)
                    .or(next_tile_output_retarget)
                    .unwrap_or(pixel.color),
            );
        };

        cached.cached = recomputed;
        self.current_scanline_bg_dot_contexts[visible_x] = Some(PpuRecentBgDotContext {
            source: cached.cached.source,
            fetch_x: cached.cached.fetch_x,
            pixel_index: cached.pixel_index,
            tile_index: cached.cached.tile_index,
        });
        pixel.color = old_pixel_override
            .or(window_activation_tilemap_override)
            .or(window_tiledata_selector_override)
            .or(low_band_shifted_override)
            .or(visible_tile2_scy_tilemap_override)
            .or(visible_tile2_previous_row_override)
            .or(visible_tile3_previous_row_override)
            .or(next_tile_output_retarget)
            .unwrap_or_else(|| {
                bg_tile_pixel_value(
                    recomputed.tile_low,
                    recomputed.tile_high,
                    cached.pixel_index,
                )
            });
        Some(pixel.color)
    }

    pub(super) fn compute_startup_visible_tile2_scy_placeholder_pixel(
        &self,
        visible_x: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.bg_pipeline_state.startup_scy_tiledata_latch?;

        let sprite_phase = self.scy_obj_phase_policy()?;
        if !sprite_phase
            .startup_visible_tile2_placeholder_uses_previous_tilemap_row(self.ly, visible_x)
        {
            return None;
        }

        let registers = self.mode3_register_latches().visible();
        let tile_map_base = if registers.lcdc & LCDC_BG_TILE_MAP_BIT != 0 {
            0x1C00
        } else {
            0x1800
        };
        let tile_map_row =
            ((self.ly.wrapping_add(self.scy) / BG_TILE_WIDTH) as u16).wrapping_sub(1) & 0x1F;
        let tile_map_column = u16::from(visible_x / BG_TILE_WIDTH);
        let tile_map_address = tile_map_base + tile_map_row * 32 + tile_map_column;
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let tile_data_base = bg_tile_data_base(registers.lcdc, tile_index);
        let tile_low_address = tile_data_base + 7 * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(
            tile_low,
            tile_high,
            visible_x & (BG_TILE_WIDTH - 1),
        ))
    }

    fn compute_window_activation_tilemap_override(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        let previous_tilemap_select =
            cached.window_activation_first_pixel_previous_tilemap_select?;
        if cached.source != PpuBgFetcherSource::Window {
            return None;
        }

        if let Some(current_tilemap_mask) = window_activation_tile_current_tilemap_mask(
            cached.fetch_x,
            self.window_state.window_line_counter,
        ) {
            let use_first_write_current_tilemap = current_tilemap_mask & (0x80 >> pixel_index) != 0;
            let tilemap_select = if use_first_write_current_tilemap {
                !previous_tilemap_select
            } else {
                previous_tilemap_select
            };
            return Some(self.read_window_activation_tilemap_pixel(
                cached,
                pixel_index,
                tilemap_select,
                vram,
            ));
        }

        if cached.fetch_x != 0
            || pixel_index != 0
            || !window_activation_first_pixel_uses_previous_tilemap(
                self.window_state.window_line_counter,
            )
        {
            return None;
        }

        Some(self.read_window_activation_tilemap_pixel(
            cached,
            pixel_index,
            previous_tilemap_select,
            vram,
        ))
    }

    fn read_window_activation_tilemap_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        tilemap_select: bool,
        vram: &VramBusView<'_>,
    ) -> u8 {
        let tile_map_offset = cached.tile_map_address & 0x03FF;
        let tile_map_base = if tilemap_select { 0x1C00 } else { 0x1800 };
        let tile_map_address = tile_map_base | tile_map_offset;
        let tile_index = vram
            .read(tile_map_address as usize)
            .unwrap_or(cached.tile_index);
        let registers = self.mode3_register_latches().visible();
        let tile_row = self
            .current_mode3_live_background_refetch_context()
            .current_window_tile_row();
        let tile_low_address =
            bg_tile_data_base(registers.lcdc, tile_index) + tile_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        bg_tile_pixel_value(tile_low, tile_high, pixel_index)
    }

    fn compute_window_lcdc4_tiledata_selector_override(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        let previous_select = cached.dmg_lcdc4_previous_tiledata_select_for_output_override?;
        if cached.source != PpuBgFetcherSource::Window {
            return None;
        }

        let previous_plane_masks = window_lcdc4_unsigned_to_signed_previous_plane_masks(
            cached.fetch_x,
            self.window_state.window_line_counter,
        )?;
        Some(self.read_window_lcdc4_tiledata_selector_pixel(
            cached,
            pixel_index,
            previous_select,
            previous_plane_masks,
            vram,
        ))
    }

    #[cfg(test)]
    pub(super) fn test_compute_window_lcdc4_tiledata_selector_override(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.compute_window_lcdc4_tiledata_selector_override(cached, pixel_index, vram)
    }

    fn read_window_lcdc4_tiledata_selector_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        previous_select: BgTileDataSelect,
        previous_plane_masks: PerPlane<u8>,
        vram: &VramBusView<'_>,
    ) -> u8 {
        let bit = 0x80 >> pixel_index;
        let current_lcdc = self.mode3_register_latches().visible().lcdc;
        let previous_lcdc = previous_select.apply_to_lcdc(current_lcdc);
        let current_tile_row = (self.window_state.window_line_counter & (BG_TILE_WIDTH - 1)) as u16;
        let previous_tile_low_address =
            bg_tile_data_base(previous_lcdc, cached.tile_index) + current_tile_row * TILE_ROW_BYTES;
        let previous_tile_high_address = previous_tile_low_address + 1;
        let current_tile_low_address =
            bg_tile_data_base(current_lcdc, cached.tile_index) + current_tile_row * TILE_ROW_BYTES;
        let current_tile_high_address = current_tile_low_address + 1;
        let previous_tile_low = vram.read(previous_tile_low_address as usize).unwrap_or(0);
        let previous_tile_high = vram.read(previous_tile_high_address as usize).unwrap_or(0);
        let current_tile_low = vram.read(current_tile_low_address as usize).unwrap_or(0);
        let current_tile_high = vram.read(current_tile_high_address as usize).unwrap_or(0);
        let tile_low = if previous_plane_masks.low & bit != 0 {
            previous_tile_low
        } else {
            current_tile_low
        };
        let tile_high = if previous_plane_masks.high & bit != 0 {
            previous_tile_high
        } else {
            current_tile_high
        };
        bg_tile_pixel_value(tile_low, tile_high, pixel_index)
    }

    fn compute_window_lcdc4_tiledata_selector_override_from_context(
        &self,
        context: PpuRecentBgDotContext,
        previous_select: BgTileDataSelect,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        if context.source != PpuBgFetcherSource::Window {
            return None;
        }

        let previous_plane_masks = window_lcdc4_unsigned_to_signed_previous_plane_masks(
            context.fetch_x,
            self.window_state.window_line_counter,
        )?;
        let bit = 0x80 >> context.pixel_index;
        let current_lcdc = self.mode3_register_latches().visible().lcdc;
        let previous_lcdc = previous_select.apply_to_lcdc(current_lcdc);
        let current_tile_row = (self.window_state.window_line_counter & (BG_TILE_WIDTH - 1)) as u16;
        let previous_tile_low_address = bg_tile_data_base(previous_lcdc, context.tile_index)
            + current_tile_row * TILE_ROW_BYTES;
        let previous_tile_high_address = previous_tile_low_address + 1;
        let current_tile_low_address =
            bg_tile_data_base(current_lcdc, context.tile_index) + current_tile_row * TILE_ROW_BYTES;
        let current_tile_high_address = current_tile_low_address + 1;
        let previous_tile_low = vram.read(previous_tile_low_address as usize).unwrap_or(0);
        let previous_tile_high = vram.read(previous_tile_high_address as usize).unwrap_or(0);
        let current_tile_low = vram.read(current_tile_low_address as usize).unwrap_or(0);
        let current_tile_high = vram.read(current_tile_high_address as usize).unwrap_or(0);
        let tile_low = if previous_plane_masks.low & bit != 0 {
            previous_tile_low
        } else {
            current_tile_low
        };
        let tile_high = if previous_plane_masks.high & bit != 0 {
            previous_tile_high
        } else {
            current_tile_high
        };
        Some(bg_tile_pixel_value(
            tile_low,
            tile_high,
            context.pixel_index,
        ))
    }

    #[cfg(test)]
    pub(super) fn test_compute_window_lcdc4_tiledata_selector_override_from_context(
        &self,
        context: PpuRecentBgDotContext,
        previous_select: BgTileDataSelect,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.compute_window_lcdc4_tiledata_selector_override_from_context(
            context,
            previous_select,
            vram,
        )
    }

    fn apply_pending_dmg_window_lcdc4_output_repaint(&mut self, vram: &VramBusView<'_>) {
        let Some(previous_select) = self.pending_dmg_window_lcdc4_output_repaint.take() else {
            return;
        };

        let bg_enabled = self.pixel_transfer_bg_enabled();
        let visible_output_driving = self.visible_output == PpuVisibleOutputState::Driving;
        let row_start = self.ly as usize * SCREEN_WIDTH;
        let visible_limit = usize::from(self.bg_pipeline_state.visible_pixels_output);

        for visible_x in 0..visible_limit {
            let Some(context) = self.current_scanline_bg_dot_contexts[visible_x] else {
                continue;
            };
            let Some(bg_pixel) = self.compute_window_lcdc4_tiledata_selector_override_from_context(
                context,
                previous_select,
                vram,
            ) else {
                continue;
            };

            self.current_scanline_bg_pixels[visible_x] = bg_pixel;
            if self.current_scanline_mixed_pixels[visible_x].source != MixedPixelSource::Background
            {
                continue;
            }

            let output_pixel = MixedPixel::background(bg_pixel);
            let dmg_bg_forced_white =
                self.dmg_bg_panel_dot_is_forced_white(bg_enabled, output_pixel);
            let scanline_pixel = if visible_output_driving && !dmg_bg_forced_white {
                output_pixel.color
            } else {
                0
            };
            let panel_pixel = if visible_output_driving {
                if dmg_bg_forced_white {
                    0
                } else {
                    self.map_mixed_pixel_to_panel_shade(output_pixel)
                }
            } else {
                0
            };

            self.current_scanline_mixed_pixels[visible_x] = output_pixel;
            self.current_scanline_dmg_bg_forced_white[visible_x] = dmg_bg_forced_white;
            self.current_scanline_pixels[visible_x] = scanline_pixel;
            self.framebuffer[row_start + visible_x] = panel_pixel;

            for dot in &mut self.dmg_panel_live_write_state.recent_panel_dots {
                if usize::from(dot.visible_x) == visible_x {
                    dot.pixel = output_pixel;
                    dot.dmg_bg_forced_white = dmg_bg_forced_white;
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn test_apply_pending_dmg_window_lcdc4_output_repaint(
        &mut self,
        vram: &VramBusView<'_>,
    ) {
        self.apply_pending_dmg_window_lcdc4_output_repaint(vram);
    }

    pub(super) fn compute_startup_visible_tile2_scy_tilemap_retarget_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.bg_pipeline_state.startup_scy_tiledata_latch?;

        if !matches!(
            cached.origin,
            BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
        ) || cached.needs_live_tilemap_refetch
            || cached.needs_live_tile_data_refetch
        {
            return None;
        }

        let retarget = self
            .scy_obj_phase_policy()?
            .startup_visible_tile2_tilemap_retarget(self.ly, pixel_index)?;

        Some(self.read_startup_visible_tile2_scy_retargeted_pixel(
            cached,
            pixel_index,
            retarget.tilemap_row_delta,
            retarget.tiledata_row_delta,
            vram,
        ))
    }

    fn read_startup_visible_tile2_scy_retargeted_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        tilemap_row_delta: i8,
        tiledata_row_delta: i8,
        vram: &VramBusView<'_>,
    ) -> u8 {
        let tile_map_offset = cached.tile_map_address & 0x03FF;
        let tile_map_base = cached.tile_map_address & !0x03FF;
        let tile_map_row =
            ((tile_map_offset / 32) as i16 + i16::from(tilemap_row_delta)).rem_euclid(32) as u16;
        let tile_map_column = tile_map_offset & 0x1F;
        let tile_map_address = tile_map_base + tile_map_row * 32 + tile_map_column;
        let tile_index = vram
            .read(tile_map_address as usize)
            .unwrap_or(cached.tile_index);

        let registers = self.mode3_register_latches().visible();
        let cached_tile_data_base = bg_tile_data_base(registers.lcdc, cached.tile_index);
        let tile_data_row = (((cached.tile_high_address - cached_tile_data_base) / TILE_ROW_BYTES)
            as i16
            + i16::from(tiledata_row_delta))
        .rem_euclid(8) as u16;
        let tile_data_base = bg_tile_data_base(registers.lcdc, tile_index);
        let tile_low_address = tile_data_base + tile_data_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        bg_tile_pixel_value(tile_low, tile_high, pixel_index)
    }

    pub(super) fn current_transfer_selected_sprite_x(&self) -> Option<u8> {
        let current_transfer_x = self.bg_pipeline_state.current_transfer_x;
        (0..self.mode2_scan_state.selected_sprite_count())
            .filter(|&slot| !self.obj_pipeline_state.has_fetched(slot))
            .filter_map(|slot| self.mode2_scan_state.selected_sprite(slot))
            .find(|sprite| sprite_trigger_x(*sprite) == Some(current_transfer_x))
            .map(|sprite| sprite.x)
    }

    pub(super) fn startup_line_lead_sprite_x(&self) -> Option<u8> {
        (0..self.mode2_scan_state.selected_sprite_count())
            .filter_map(|slot| self.mode2_scan_state.selected_sprite(slot))
            .min_by_key(|sprite| sprite.x)
            .map(|sprite| sprite.x)
    }

    pub(super) fn scy_startup_line_lead_owner_window_open(&self) -> bool {
        self.current_transfer().is_some()
            || self.bg_pipeline_state.mode3_started
                && !matches!(
                    self.bg_pipeline_state.startup_fetch_seam,
                    BgStartupFetchSeamState::Inactive
                )
    }

    pub(super) fn scy_obj_phase_owner(&self) -> Option<PpuMode3ScyObjPhaseOwner> {
        if self.current_dot_has_pending_obj_hit() {
            return Some(PpuMode3ScyObjPhaseOwner::PendingHit {
                match_x: self.bg_pipeline_state.current_transfer_x,
            });
        }

        if self.obj_enabled() && self.obj_pipeline_state.fetch.stage != PpuObjFetcherStage::Idle {
            let sprite = self.obj_pipeline_state.fetch.sprite?;
            return Some(PpuMode3ScyObjPhaseOwner::ActiveFetch { sprite_x: sprite.x });
        }

        self.current_transfer_selected_sprite_x()
            .map(|sprite_x| PpuMode3ScyObjPhaseOwner::CurrentTransferSprite { sprite_x })
            .or_else(|| {
                if !self.scy_startup_line_lead_owner_window_open() {
                    return None;
                }
                self.startup_line_lead_sprite_x()
                    .map(|sprite_x| PpuMode3ScyObjPhaseOwner::StartupLineLead { sprite_x })
            })
    }

    pub(super) fn scy_obj_phase_policy(&self) -> Option<PpuMode3ScyObjPhasePolicy> {
        let phase_owner = self.scy_obj_phase_owner()?;
        let context = PpuMode3ScyObjPhaseContext {
            phase_owner,
            current_transfer_x: self.bg_pipeline_state.current_transfer_x,
            current_transfer: self.current_transfer(),
            bg_fetcher_stage: self.bg_pipeline_state.fetcher.stage,
            bg_fetcher_stage_dot: self.bg_pipeline_state.fetcher.stage_dot,
            bg_fifo_len: self.bg_pipeline_state.fifo.len(),
            startup_fifo_placeholders: self.bg_pipeline_state.startup_fifo_placeholders,
            obj_fetcher_stage: self.obj_pipeline_state.fetch.stage,
            obj_fetcher_stage_dot: self.obj_pipeline_state.fetch.stage_dot,
        };

        Some(PpuMode3ScyObjPhasePolicy::new(context))
    }

    pub(super) fn compute_startup_visible_tile3_previous_row_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.bg_pipeline_state.startup_scy_tiledata_latch?;

        if !matches!(
            cached.origin,
            BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
        ) || cached.needs_live_tilemap_refetch
            || cached.needs_live_tile_data_refetch
            || self
                .scy_obj_phase_policy()
                .is_none_or(|phase| !phase.startup_visible_tile3_uses_previous_tiledata_row())
        {
            return None;
        }

        let tile_data_base = bg_tile_data_base(
            self.mode3_register_latches().visible().lcdc,
            cached.tile_index,
        );
        let current_row = ((cached.tile_high_address - tile_data_base) / TILE_ROW_BYTES) & 0x07;
        let previous_row = current_row.wrapping_sub(1) & 0x07;
        let tile_low_address = tile_data_base + previous_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(tile_low, tile_high, pixel_index))
    }

    pub(super) fn compute_startup_visible_tile2_previous_row_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.bg_pipeline_state.startup_scy_tiledata_latch?;

        if !matches!(
            cached.origin,
            BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
        ) || cached.needs_live_tilemap_refetch
            || cached.needs_live_tile_data_refetch
            || self
                .scy_obj_phase_policy()
                .is_none_or(|phase| !phase.startup_visible_tile2_uses_previous_tiledata_row())
        {
            return None;
        }

        let tile_data_base = bg_tile_data_base(
            self.mode3_register_latches().visible().lcdc,
            cached.tile_index,
        );
        let current_row = ((cached.tile_high_address - tile_data_base) / TILE_ROW_BYTES) & 0x07;
        let previous_row = current_row.wrapping_sub(1) & 0x07;
        let tile_low_address = tile_data_base + previous_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(tile_low, tile_high, pixel_index))
    }

    pub(super) fn compute_startup_visible_tile3_scx_boundary_next_tile_output_retarget_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        let scx = cached.startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx?;
        if (0x08..=0x0E).contains(&self.scx) {
            let retarget_low_band_pixel = matches!(self.scx & 0x07, 0x03) && pixel_index == 6;
            if !retarget_low_band_pixel {
                return None;
            }
        }
        let mut registers = self
            .current_mode3_live_background_refetch_context()
            .registers();
        registers.scx = scx;
        let fetch_x = cached.fetch_x + BG_TILE_WIDTH as u16;
        let tile_map_address =
            PpuMode3BackgroundFetchContext::new(registers, registers, fetch_x, self.ly)
                .tile_index_address();
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let tile_row = self
            .current_mode3_live_background_refetch_context()
            .current_scanline_tile_row();
        let tile_low_address =
            bg_tile_data_base(registers.lcdc, tile_index) + tile_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(tile_low, tile_high, pixel_index))
    }

    pub(super) fn compute_startup_visible_tile3_scx_low_band_shifted_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        if !matches!(
            cached.origin,
            BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
        ) || cached.fetch_x != BG_TILE_WIDTH as u16 * 2
            || !(0x08..=0x0E).contains(&self.scx)
        {
            return None;
        }

        match self.scx & 0x07 {
            0x00 | 0x06 if pixel_index == 0 => {
                Some(bg_tile_pixel_value(cached.tile_low, cached.tile_high, 1))
            }
            0x01 | 0x05 if pixel_index == 1 => {
                Some(bg_tile_pixel_value(cached.tile_low, cached.tile_high, 2))
            }
            0x03 if pixel_index == 5 => self
                .compute_startup_visible_tile3_scx_boundary_next_tile_output_retarget_pixel(
                    cached, 6, vram,
                ),
            _ => None,
        }
    }

    fn compute_startup_visible_tile3_scx_boundary_old_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        let previous_scx =
            cached.preserve_old_startup_visible_tile3_scx_boundary_pixel(pixel_index)?;
        let mut registers = self
            .current_mode3_live_background_refetch_context()
            .registers();
        registers.scx = previous_scx;
        let tile_map_address =
            PpuMode3BackgroundFetchContext::new(registers, registers, cached.fetch_x, self.ly)
                .tile_index_address();
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let tile_row = self
            .current_mode3_live_background_refetch_context()
            .current_scanline_tile_row();
        let tile_low_address =
            bg_tile_data_base(registers.lcdc, tile_index) + tile_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(tile_low, tile_high, pixel_index))
    }

    pub(super) fn obj_enabled(&self) -> bool {
        self.mode3_register_latches().visible().obj_enabled()
    }

    pub(super) fn maybe_apply_wx0_shortening_after_transfer_dot(
        &mut self,
        transfer_dot: Mode3TransferDot,
    ) {
        if !self.mode3_window_policy().can_apply_wx0_shortening(
            transfer_dot,
            self.bg_pipeline_state.visible_pixels_output,
            self.bg_pipeline_state.current_transfer_x,
            self.bg_pipeline_state.initial_scx_discard,
            self.bg_pipeline_state.scx_discard_remaining,
        ) {
            return;
        }

        self.bg_pipeline_state.apply_wx0_scx_shortening();
    }

    pub(super) fn maybe_start_window_after_transfer_dot(
        &mut self,
        transfer_dot: Mode3TransferDot,
    ) -> bool {
        match self
            .mode3_window_policy()
            .start_decision_after_transfer_dot(
                transfer_dot,
                self.bg_pipeline_state.visible_pixels_output,
                self.bg_pipeline_state.current_transfer_x,
                self.bg_pipeline_state.initial_scx_discard,
                self.bg_pipeline_state.scx_discard_remaining,
                self.bg_pipeline_state.wx166_armed_this_line,
            ) {
            PpuMode3WindowStartDecision::NotReady => false,
            PpuMode3WindowStartDecision::ArmWx166NextLine => {
                self.window_state.pending_wx166_next_line = true;
                self.bg_pipeline_state.wx166_armed_this_line = true;
                false
            }
            PpuMode3WindowStartDecision::StartNow => {
                self.start_window_fetcher_restart();
                true
            }
        }
    }

    pub(super) fn latch_object_fetch_hits(&mut self) {
        if !self.obj_enabled() {
            return;
        }

        let current_owner = self.current_obj_hit_ownership();
        for sprite_slot in 0..self.mode2_scan_state.selected_sprite_count() {
            if self.obj_pipeline_state.has_fetched(sprite_slot) {
                continue;
            }

            let Some(sprite) = self.mode2_scan_state.selected_sprite(sprite_slot) else {
                continue;
            };
            let Some(trigger_x) = sprite_trigger_x(sprite) else {
                continue;
            };

            if trigger_x == current_owner.match_x {
                self.obj_pipeline_state.queue_fetch_hit(
                    sprite_slot,
                    current_owner,
                    self.obj_pipeline_state.mode3_line_start_obj_height,
                );
            }
        }
    }

    pub(super) fn sync_pending_obj_hit_ownership(&mut self) {
        if !self.obj_enabled() {
            self.obj_pipeline_state.clear_pending_fetch_hits();
            return;
        }

        let current_owner = self.current_obj_hit_ownership();
        self.obj_pipeline_state
            .clear_pending_fetch_hits_if_stale(current_owner);
    }

    pub(super) fn try_start_object_fetch_from_current_dot(
        &mut self,
        start_source: ObjFetchStartSource,
        overlap_current_dot: bool,
    ) -> bool {
        if !self
            .current_dot_arbitration()
            .can_start_obj_fetch(start_source)
        {
            return false;
        }

        let Some((sprite_slot, selected_obj_height)) =
            self.obj_pipeline_state.pop_pending_fetch_hit()
        else {
            return false;
        };
        let Some(sprite) = self.mode2_scan_state.selected_sprite(sprite_slot) else {
            return false;
        };

        self.obj_pipeline_state.start_fetch(
            sprite_slot,
            sprite,
            selected_obj_height,
            self.current_obj_height(),
        );
        let pending_nonterminal_same_x_cluster_pays_startup_dot =
            self.pending_nonterminal_same_x_cluster_pays_startup_dot();
        let overlap_current_dot =
            overlap_current_dot && !pending_nonterminal_same_x_cluster_pays_startup_dot;
        if overlap_current_dot {
            if matches!(
                start_source,
                ObjFetchStartSource::FifoBackedTransfer | ObjFetchStartSource::PushCachedBgFetch
            ) {
                self.bg_pipeline_state.push.interrupt_for_object_fetch();
            }
            self.obj_pipeline_state.fetch.stage_dot = 1;
        }
        true
    }

    pub(super) fn current_obj_hit_ownership(&self) -> ObjHitOwnership {
        let phase = self
            .current_transfer()
            .map_or(ObjHitPhase::PreVisible, |transfer| {
                match transfer.context.lane {
                    Mode3TransferLane::PreVisible => ObjHitPhase::PreVisible,
                    Mode3TransferLane::Hidden => ObjHitPhase::Hidden,
                    Mode3TransferLane::Visible => ObjHitPhase::Visible,
                }
            });

        ObjHitOwnership {
            match_x: self.bg_pipeline_state.current_transfer_x,
            phase,
        }
    }

    pub(super) fn bg_fetcher_ready_for_fifo_backed_obj_start(&self) -> bool {
        let allow_same_x_cluster_tileindex_overlap = self
            .current_transfer_x_supports_early_same_x_obj_start()
            && self.obj_pipeline_state.pending_sprite_slots.len() >= 2
            && self.obj_pipeline_state.pending_match_x
                == Some(self.bg_pipeline_state.current_transfer_x);
        if self.bg_pipeline_state.current_transfer_x < 8 {
            allow_same_x_cluster_tileindex_overlap
                || !matches!(
                    self.bg_pipeline_state.fetcher.stage,
                    PpuBgFetcherStage::TileIndex
                )
        } else {
            !matches!(
                self.bg_pipeline_state.fetcher.stage,
                PpuBgFetcherStage::TileIndex | PpuBgFetcherStage::TileDataLow
            )
        }
    }

    pub(super) fn advance_object_fetch(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        if self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle {
            return false;
        }

        if self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Startup
            && !self.obj_fetch_startup_ready()
        {
            return false;
        }

        if !self.obj_enabled() {
            self.obj_pipeline_state.fetch.cancelled = true;
        }

        let fetch = self.obj_pipeline_state.fetch;
        let startup_dot_is_shared = matches!(
            (fetch.stage, fetch.stage_dot),
            (PpuObjFetcherStage::Startup, 1)
        ) && !fetch.count_terminal_push_dot;
        let hidden_left_edge_same_x_chain_pays_push_dot =
            self.hidden_left_edge_same_x_chain_pays_push_dot();
        let visible_left_edge_same_x_chain_shares_push_dot =
            dma_oam_conflict.is_none() && self.visible_left_edge_same_x_chain_shares_push_dot();
        let terminal_right_edge_same_x_chain_shares_push_dot =
            self.terminal_right_edge_same_x_chain_shares_push_dot();
        let push_dot_is_shared = (matches!(
            (fetch.stage, fetch.stage_dot),
            (PpuObjFetcherStage::Push, 1)
        ) && !fetch.count_terminal_push_dot
            && (self.bg_pipeline_state.current_transfer_x < 8
                || visible_left_edge_same_x_chain_shares_push_dot
                || terminal_right_edge_same_x_chain_shares_push_dot)
            && !hidden_left_edge_same_x_chain_pays_push_dot);
        if !startup_dot_is_shared && !push_dot_is_shared {
            self.bg_pipeline_state.extend_mode3_by_one_dot();
        }
        match (fetch.stage, fetch.stage_dot) {
            (PpuObjFetcherStage::Startup, 0) => {
                self.obj_pipeline_state.fetch.stage_dot = 1;
            }
            (PpuObjFetcherStage::Startup, 1) => {
                let resolved_sprite = fetch
                    .sprite
                    .map(|sprite| self.resolve_obj_fetch_sprite(oam, sprite, dma_oam_conflict));
                let resolved_tile = resolved_sprite.and_then(|sprite| {
                    self.obj_tile_index_and_row_for_mode3_fetch(
                        sprite,
                        fetch.selected_obj_height,
                        fetch.latched_obj_height,
                    )
                });
                let first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte =
                    self.first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte();
                let terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step =
                    self.terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step();
                let first_fast_same_x_cluster_fetch_skips_first_tile_data_low_half_step =
                    !first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte
                        && !terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step
                        && self.initial_nonterminal_same_x_cluster_skips_first_low_half_step()
                        && !self.obj_pipeline_state.pending_sprite_slots.is_empty()
                        && self.fetched_same_x_obj_sprite_count_for_active_fetch() == 0;
                self.obj_pipeline_state.fetch.resolved_sprite = resolved_sprite;
                self.obj_pipeline_state.fetch.resolved_tile_index =
                    resolved_tile.map(|(tile_index, _)| tile_index);
                self.obj_pipeline_state.fetch.resolved_tile_row =
                    resolved_tile.map(|(_, tile_row)| tile_row);
                if first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte {
                    self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
                    self.obj_pipeline_state.fetch.stage_dot = 0;
                } else if terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step {
                    self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
                    self.obj_pipeline_state.fetch.stage_dot = 1;
                } else {
                    self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataLow;
                    self.obj_pipeline_state.fetch.stage_dot = u8::from(
                        first_fast_same_x_cluster_fetch_skips_first_tile_data_low_half_step,
                    );
                }
            }
            (PpuObjFetcherStage::TileDataLow, 0) => {
                self.obj_pipeline_state.fetch.stage_dot = 1;
            }
            (PpuObjFetcherStage::TileDataLow, 1) => {
                self.obj_pipeline_state.fetch.tile_low = fetch
                    .resolved_tile_index
                    .zip(fetch.resolved_tile_row)
                    .map(|(tile_index, tile_row)| {
                        self.read_obj_tile_data_byte_for_resolved_tile(
                            vram, tile_index, tile_row, 0,
                        )
                    })
                    .unwrap_or(0);
                self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
                self.obj_pipeline_state.fetch.stage_dot = 0;
            }
            (PpuObjFetcherStage::TileDataHigh, 0) => {
                self.obj_pipeline_state.fetch.stage_dot = 1;
            }
            (PpuObjFetcherStage::TileDataHigh, 1) => {
                self.obj_pipeline_state.fetch.tile_high = fetch
                    .resolved_tile_index
                    .zip(fetch.resolved_tile_row)
                    .map(|(tile_index, tile_row)| {
                        self.read_obj_tile_data_byte_for_resolved_tile(
                            vram, tile_index, tile_row, 1,
                        )
                    })
                    .unwrap_or(0);
                self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
                self.obj_pipeline_state.fetch.stage_dot = 0;
            }
            (PpuObjFetcherStage::Push, 0) => {
                self.obj_pipeline_state.fetch.stage_dot = 1;
            }
            (PpuObjFetcherStage::Push, 1) => {
                let resolved_sprite = fetch
                    .resolved_sprite
                    .expect("active OBJ fetch must keep resolved metadata until FIFO push");
                if !fetch.cancelled && self.obj_enabled() {
                    let (tile_low, tile_high) = self.dmg_lcdc2_live_obj_size_push_bytes(
                        resolved_sprite,
                        fetch.tile_low,
                        fetch.tile_high,
                        vram,
                    );
                    self.push_obj_pixels(
                        resolved_sprite,
                        tile_low,
                        tile_high,
                        self.bg_pipeline_state.visible_pixels_output,
                    );
                    self.repaint_observed_startup_obj_prefix_overlap(
                        resolved_sprite,
                        tile_low,
                        tile_high,
                    );
                }
                self.obj_pipeline_state.mark_fetched(fetch.sprite_slot);
                self.obj_pipeline_state.fetch = ObjFetchState::default();
                self.bg_pipeline_state.push.resume_after_object_fetch();
                if self.bg_pipeline_state.current_transfer_x < 8
                    || self.right_edge_visible_same_x_cluster_continues_after_push()
                {
                    let _ = self.continue_same_x_obj_chain_after_push(oam, dma_oam_conflict);
                } else if self.first_late_visible_push_backed_same_x_cluster_chains_after_push() {
                    let _ = self.try_start_object_fetch_from_current_dot(
                        ObjFetchStartSource::FifoBackedTransfer,
                        true,
                    );
                }
            }
            (PpuObjFetcherStage::Idle, _) => unreachable!(
                "idle OBJ fetch must have returned before entering the explicit dot automaton"
            ),
            (_, other_dot) => unreachable!(
                "invalid OBJ fetcher stage_dot {other_dot} for stage {:?}",
                fetch.stage
            ),
        }

        true
    }

    pub(super) fn obj_fetch_startup_ready(&self) -> bool {
        let fifo_ready = !self.bg_pipeline_state.fifo.is_empty();
        let Some(sprite) = self.obj_pipeline_state.fetch.sprite else {
            return fifo_ready;
        };

        if sprite.x >= 8 {
            return fifo_ready;
        }

        fifo_ready
            && !matches!(
                self.bg_pipeline_state.fetcher.stage,
                PpuBgFetcherStage::TileIndex
            )
    }

    pub(super) fn resolve_obj_fetch_sprite(
        &mut self,
        oam: &OamBusView<'_>,
        sprite: PpuSelectedSprite,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> PpuSelectedSprite {
        let (tile_index, attributes) =
            read_obj_fetch_sprite_metadata(oam, sprite, dma_oam_conflict);
        self.obj_pipeline_state.late_metadata_word = Some((tile_index, attributes));

        PpuSelectedSprite {
            tile_index,
            attributes,
            ..sprite
        }
    }

    fn repaint_observed_startup_obj_prefix_overlap(
        &mut self,
        sprite: PpuSelectedSprite,
        tile_low: u8,
        tile_high: u8,
    ) {
        self.repaint_observed_obj_scanline_overlap(
            sprite,
            tile_low,
            tile_high,
            self.bg_pipeline_state.visible_pixels_output,
            true,
        );
    }

    pub(in crate::ppu) fn repaint_observed_obj_scanline_overlap(
        &mut self,
        sprite: PpuSelectedSprite,
        tile_low: u8,
        tile_high: u8,
        overlap_end_visible_x: u8,
        background_only: bool,
    ) {
        if !self.console_model.is_dmg_family()
            || self.visible_output != PpuVisibleOutputState::Driving
            || overlap_end_visible_x == 0
        {
            return;
        }

        let sprite_screen_x = sprite_screen_x(sprite);
        let overlap_start = sprite_screen_x.max(0) as u8;
        let overlap_end = (sprite_screen_x + BG_TILE_WIDTH as i16)
            .min(i16::from(overlap_end_visible_x))
            .min(SCREEN_WIDTH as i16) as u8;
        if overlap_start >= overlap_end {
            return;
        }

        let bg_enabled = self.pixel_transfer_bg_enabled();
        for visible_x in overlap_start..overlap_end {
            let tile_pixel = i16::from(visible_x) - sprite_screen_x;
            if !(0..BG_TILE_WIDTH as i16).contains(&tile_pixel) {
                continue;
            }

            let bit = if sprite.attributes & 0x20 != 0 {
                tile_pixel as u8
            } else {
                7 - tile_pixel as u8
            };
            let low_bit = (tile_low >> bit) & 0x01;
            let high_bit = (tile_high >> bit) & 0x01;
            let candidate = ObjPixel {
                color: (high_bit << 1) | low_bit,
                palette_obp1: sprite.attributes & 0x10 != 0,
                bg_over_obj: sprite.attributes & 0x80 != 0,
                sprite_x: sprite.x,
                oam_index: sprite.oam_index,
            };
            if background_only && candidate.is_transparent() {
                continue;
            }

            let visible_x = visible_x as usize;
            if background_only
                && self.current_scanline_mixed_pixels[visible_x].source
                    != MixedPixelSource::Background
            {
                continue;
            }

            let bg_pixel = self.current_scanline_bg_pixels[visible_x];
            let effective_bg_priority_pixel = if bg_enabled { bg_pixel } else { 0 };
            let output_pixel = if candidate.is_transparent() {
                MixedPixel::background(bg_pixel)
            } else {
                self.mix_bg_and_obj(bg_pixel, effective_bg_priority_pixel, candidate)
            };
            let dmg_bg_forced_white =
                self.dmg_bg_panel_dot_is_forced_white(bg_enabled, output_pixel);
            let scanline_pixel =
                if self.visible_output == PpuVisibleOutputState::Driving && !dmg_bg_forced_white {
                    output_pixel.color
                } else {
                    0
                };
            let panel_pixel = if dmg_bg_forced_white {
                0
            } else {
                self.map_mixed_pixel_to_panel_shade(output_pixel)
            };

            self.current_scanline_mixed_pixels[visible_x] = output_pixel;
            self.current_scanline_dmg_bg_forced_white[visible_x] = dmg_bg_forced_white;
            self.current_scanline_pixels[visible_x] = scanline_pixel;
            self.framebuffer[self.ly as usize * SCREEN_WIDTH + visible_x] = panel_pixel;

            for dot in &mut self.dmg_panel_live_write_state.recent_panel_dots {
                if usize::from(dot.visible_x) == visible_x {
                    dot.pixel = output_pixel;
                    dot.dmg_bg_forced_white = dmg_bg_forced_white;
                }
            }
        }
    }

    pub(super) fn chained_same_x_obj_fetch_skips_first_tile_data_low_half_step(&self) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        let first_hidden_x4_same_x_restart_pays_low_half_step = fetched_same_x_count == 1
            && self.bg_pipeline_state.current_transfer_x == 4
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Hidden,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    ..
                })
            );
        (fetched_same_x_count >= 2 && fetched_same_x_count.is_multiple_of(2))
            || (matches!(fetched_same_x_count, 1 | 3)
                && !first_hidden_x4_same_x_restart_pays_low_half_step
                && self.nonterminal_same_x_cluster_restart_skips_first_low_half_step())
            || (fetched_same_x_count >= 5
                && fetched_same_x_count % 2 == 1
                && (self.hidden_same_x_cluster_restart_skips_first_low_half_step()
                    || self.visible_periodic_same_x_cluster_restart_skips_first_low_half_step()))
            || self.terminal_previsible_same_x_chain_skips_first_low_half_step()
    }

    pub(super) fn chained_same_x_obj_fetch_uses_long_tail_restart(&self) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        fetched_same_x_count >= 5
            && fetched_same_x_count % 2 == 1
            && !self.current_transfer_x_supports_early_same_x_obj_start()
    }

    pub(super) fn first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 167
            && (self.bg_pipeline_state.current_transfer_x & 0x07) < 6
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && !self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() == 0
            && matches!(
                (
                    self.bg_pipeline_state.fetcher.stage,
                    self.bg_pipeline_state.fetcher.stage_dot,
                ),
                (PpuBgFetcherStage::TileDataHigh, 1)
            )
    }

    pub(super) fn initial_nonterminal_same_x_cluster_skips_first_low_half_step(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 167
            && (self.bg_pipeline_state.current_transfer_x & 0x07) < 7
            && self.current_transfer_x_supports_early_same_x_obj_start()
    }

    pub(super) fn nonterminal_same_x_cluster_restart_skips_first_low_half_step(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 167
            && (self.bg_pipeline_state.current_transfer_x & 0x07) < 7
            && self.current_transfer_x_supports_early_same_x_obj_start()
    }

    pub(super) fn hidden_same_x_cluster_restart_skips_first_low_half_step(&self) -> bool {
        matches!(
            self.current_transfer(),
            Some(Mode3CurrentTransfer {
                context: Mode3TransferContext {
                    lane: Mode3TransferLane::Hidden,
                    source_window: Mode3TransferSourceWindow::FifoBacked,
                },
                ..
            })
        ) && matches!(
            (
                self.bg_pipeline_state.fetcher.stage,
                self.bg_pipeline_state.fetcher.stage_dot,
            ),
            (PpuBgFetcherStage::TileDataHigh, 1)
        )
    }

    pub(super) fn visible_periodic_same_x_cluster_restart_skips_first_low_half_step(&self) -> bool {
        self.bg_pipeline_state.visible_pixels_output >= 24
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    ..
                })
            )
            && matches!(
                (
                    self.bg_pipeline_state.fetcher.stage,
                    self.bg_pipeline_state.fetcher.stage_dot,
                ),
                (PpuBgFetcherStage::TileDataHigh, 1)
            )
    }

    pub(super) fn hidden_left_edge_same_x_chain_pays_push_dot(&self) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        (4..=7).contains(&self.bg_pipeline_state.current_transfer_x)
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Hidden,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    ..
                })
            )
            && (1..=4).contains(&fetched_same_x_count)
    }

    pub(super) fn visible_left_edge_same_x_chain_shares_push_dot(&self) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        let visible_output = self.bg_pipeline_state.visible_pixels_output;
        let current_transfer_x = self.bg_pipeline_state.current_transfer_x;
        let repeated_visible_boundary_fetched_count_supports_push_dot_share =
            (1..=4).contains(&fetched_same_x_count);
        let late_visible_same_x_cluster_shares_push_dot = (visible_output & 0x1f) >= 24
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && fetched_same_x_count >= 5;
        let same_x_screen_edge_supports_push_dot_share = (visible_output < 8
            && self.current_transfer_x_supports_early_same_x_obj_start())
            || late_visible_same_x_cluster_shares_push_dot
            || ((visible_output & 0x0f) == 8
                && (current_transfer_x & 0x07) == 0
                && repeated_visible_boundary_fetched_count_supports_push_dot_share);
        same_x_screen_edge_supports_push_dot_share
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    ..
                })
            )
            && fetched_same_x_count > 0
    }

    pub(super) fn first_late_visible_push_backed_same_x_cluster_chains_after_push(&self) -> bool {
        self.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.bg_pipeline_state.fifo.len() == 2
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && (self.bg_pipeline_state.current_transfer_x & 0x07) == 2
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && matches!(
                (
                    self.bg_pipeline_state.fetcher.stage,
                    self.bg_pipeline_state.fetcher.stage_dot,
                ),
                (PpuBgFetcherStage::Push, 0)
            )
            && self.obj_pipeline_state.pending_match_x
                == Some(self.bg_pipeline_state.current_transfer_x)
            && !self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() > 0
    }

    pub(super) fn right_edge_visible_same_x_cluster_continues_after_push(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x >= 160
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && self.obj_pipeline_state.pending_match_x
                == Some(self.bg_pipeline_state.current_transfer_x)
            && self.obj_pipeline_state.pending_sprite_slots.len() >= 2
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() > 0
    }

    pub(super) fn continue_same_x_obj_chain_after_push(
        &mut self,
        oam: &OamBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        let pending_nonterminal_same_x_cluster_pays_startup_dot =
            self.pending_nonterminal_same_x_cluster_pays_startup_dot();
        let right_edge_visible_same_x_cluster_pays_startup_dot =
            self.right_edge_visible_same_x_cluster_pays_startup_dot();
        let right_edge_visible_same_x_cluster = right_edge_visible_same_x_cluster_pays_startup_dot
            || self.right_edge_visible_same_x_cluster_continues_after_push();
        let started = self
            .try_start_object_fetch_from_current_dot(ObjFetchStartSource::FifoBackedTransfer, true);
        if !started || !self.obj_fetch_startup_ready() {
            return started;
        }
        if right_edge_visible_same_x_cluster {
            self.bg_pipeline_state
                .saw_right_edge_visible_same_x_cluster_this_line = true;
        }

        let long_same_x_tail_restart = self.chained_same_x_obj_fetch_uses_long_tail_restart();
        if long_same_x_tail_restart {
            self.bg_pipeline_state.extend_mode3_by_one_dot();
        }
        let sprite = self
            .obj_pipeline_state
            .fetch
            .sprite
            .expect("chained OBJ fetch must keep sprite metadata");
        let resolved_sprite = self.resolve_obj_fetch_sprite(oam, sprite, dma_oam_conflict);
        self.obj_pipeline_state.fetch.resolved_sprite = Some(resolved_sprite);
        if long_same_x_tail_restart {
            self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Startup;
            self.obj_pipeline_state.fetch.stage_dot = 0;
            self.obj_pipeline_state.fetch.count_terminal_push_dot = true;
        } else {
            if pending_nonterminal_same_x_cluster_pays_startup_dot
                || right_edge_visible_same_x_cluster_pays_startup_dot
            {
                self.bg_pipeline_state.extend_mode3_by_one_dot();
            }
            if self.terminal_previsible_same_x_chain_skips_obj_tile_data_low_byte() {
                self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
                self.obj_pipeline_state.fetch.stage_dot = 0;
            } else {
                self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataLow;
                self.obj_pipeline_state.fetch.stage_dot =
                    u8::from(self.chained_same_x_obj_fetch_skips_first_tile_data_low_half_step());
            }
        }

        true
    }

    pub(super) fn right_edge_visible_same_x_cluster_pays_startup_dot(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x >= 160
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && self.obj_pipeline_state.pending_match_x
                == Some(self.bg_pipeline_state.current_transfer_x)
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() >= 5
    }

    pub(super) fn saturated_placeholder_backed_terminal_bg_tail_can_hold_one_post_push_dot(
        &self,
    ) -> bool {
        self.bg_pipeline_state.mode3_started
            && self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.bg_pipeline_state.current_transfer_x >= 168
            && (160..=161).any(|sprite_x| {
                (0..self.mode2_scan_state.selected_sprite_count())
                    .filter(|&slot| {
                        self.mode2_scan_state
                            .selected_sprite(slot)
                            .is_some_and(|sprite| sprite.x == sprite_x)
                    })
                    .count()
                    >= 5
            })
            && usize::from(self.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.bg_pipeline_state.startup_fifo_placeholders == 4
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.bg_pipeline_state.fetcher.stage == PpuBgFetcherStage::Push
    }

    pub(super) fn terminal_previsible_same_x_chain_can_start_obj_fetch(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 8
            && self.obj_pipeline_state.pending_match_x
                == Some(self.bg_pipeline_state.current_transfer_x)
            && self.obj_pipeline_state.pending_sprite_slots.len() == 1
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() > 0
    }

    pub(super) fn terminal_previsible_same_x_chain_skips_first_low_half_step(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 8
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() > 0
    }

    pub(super) fn terminal_previsible_same_x_chain_skips_obj_tile_data_low_byte(&self) -> bool {
        self.terminal_previsible_same_x_chain_skips_first_low_half_step()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() >= 9
    }

    pub(super) fn terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step(
        &self,
    ) -> bool {
        self.bg_pipeline_state.current_transfer_x >= 160
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() > 0
    }

    pub(super) fn terminal_right_edge_same_x_chain_shares_push_dot(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x >= 160
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() > 0
    }

    pub(super) fn current_transfer_x_supports_early_same_x_obj_start(&self) -> bool {
        matches!(self.bg_pipeline_state.current_transfer_x & 0x07, 2..=7)
    }

    pub(super) fn terminal_mode3_dot_started_shared_obj_fetch(&self) -> bool {
        matches!(
            (
                self.obj_pipeline_state.fetch.stage,
                self.obj_pipeline_state.fetch.stage_dot,
            ),
            (PpuObjFetcherStage::Startup, 1)
        ) && self.line_dot.saturating_add(1) == self.current_mode0_start_dot()
    }

    pub(super) fn pending_nonterminal_same_x_cluster_pays_startup_dot(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 167
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && self.obj_pipeline_state.pending_match_x
                == Some(self.bg_pipeline_state.current_transfer_x)
            && self.obj_pipeline_state.pending_sprite_slots.len() >= 2
            && !self.first_late_visible_push_backed_same_x_cluster_chains_after_push()
    }

    pub(super) fn fetched_same_x_obj_sprite_count_for_active_fetch(&self) -> usize {
        let Some(sprite) = self.obj_pipeline_state.fetch.sprite else {
            return 0;
        };
        let Some(trigger_x) = sprite_trigger_x(sprite) else {
            return 0;
        };

        self.fetched_same_x_obj_sprite_count_for_trigger_x(trigger_x)
    }

    pub(super) fn fetched_same_x_obj_sprite_count_for_pending_match_x(&self) -> usize {
        let Some(trigger_x) = self.obj_pipeline_state.pending_match_x else {
            return 0;
        };

        self.fetched_same_x_obj_sprite_count_for_trigger_x(trigger_x)
    }

    pub(super) fn fetched_same_x_obj_sprite_count_for_trigger_x(&self, trigger_x: u8) -> usize {
        let mut fetched_same_x_count = 0_usize;
        for sprite_slot in 0..self.mode2_scan_state.selected_sprite_count() {
            if !self.obj_pipeline_state.has_fetched(sprite_slot) {
                continue;
            }
            let Some(selected_sprite) = self.mode2_scan_state.selected_sprite(sprite_slot) else {
                continue;
            };
            if sprite_trigger_x(selected_sprite) == Some(trigger_x) {
                fetched_same_x_count += 1;
            }
        }
        fetched_same_x_count
    }

    pub(super) fn start_window_fetcher_restart(&mut self) {
        let bg_resume_fetch_pixel = self.bg_pipeline_state.fetcher.next_fetch_pixel;
        self.bg_pipeline_state.fifo.clear();
        self.bg_pipeline_state.fifo_cached_pixels.clear();
        self.bg_pipeline_state.startup_fifo_placeholders = 0;
        self.bg_pipeline_state.push.reset();
        self.bg_pipeline_state.fill.reset();
        self.bg_pipeline_state
            .fetcher
            .start_window(bg_resume_fetch_pixel);
        self.bg_pipeline_state.scx_discard_remaining = 0;
        self.bg_pipeline_state.window_started_this_line = true;
        self.bg_pipeline_state.window_force_x0_this_line = false;
    }
}

const fn window_activation_first_pixel_uses_previous_tilemap(window_tile_row: u8) -> bool {
    window_tile_row >= 24 && matches!(window_tile_row & 0x07, 0x01 | 0x02 | 0x04 | 0x06)
}

const WINDOW_ACTIVATION_FIRST_TILE_CURRENT_TILEMAP_MASKS: [[u8; 8]; 15] = [
    [0x80, 0x40, 0x20, 0xA0, 0x20, 0xA0, 0x40, 0x80],
    [0xC0, 0x20, 0x90, 0x50, 0x90, 0x50, 0x20, 0xC0],
    [0xE0, 0x10, 0xC8, 0x28, 0xC8, 0x28, 0x10, 0xE0],
    [0xF0, 0x08, 0xE4, 0x94, 0xE4, 0x94, 0x08, 0xF0],
    [0x78, 0x84, 0x72, 0x4A, 0x72, 0x4A, 0x84, 0x78],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
    [0x0F, 0x10, 0x2E, 0x29, 0x2E, 0x29, 0x10, 0x0F],
    [0x07, 0x08, 0x17, 0x14, 0x17, 0x14, 0x08, 0x07],
    [0x03, 0x04, 0x0B, 0x0A, 0x0B, 0x0A, 0x04, 0x03],
    [0x01, 0x02, 0x05, 0x05, 0x05, 0x05, 0x02, 0x01],
    [0x00, 0x01, 0x02, 0x02, 0x02, 0x02, 0x01, 0x00],
    [0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
];

const WINDOW_ACTIVATION_SECOND_TILE_CURRENT_TILEMAP_MASKS: [[u8; 8]; 15] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00],
    [0x00, 0x80, 0x40, 0x40, 0x40, 0x40, 0x80, 0x00],
    [0x80, 0x40, 0x20, 0xA0, 0x20, 0xA0, 0x40, 0x80],
    [0xC0, 0x20, 0x90, 0x50, 0x90, 0x50, 0x20, 0xC0],
    [0xE0, 0x10, 0xC8, 0x28, 0xC8, 0x28, 0x10, 0xE0],
    [0xF0, 0x08, 0xE4, 0x94, 0xE4, 0x94, 0x08, 0xF0],
    [0x78, 0x84, 0x72, 0x4A, 0x72, 0x4A, 0x84, 0x78],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_CURRENT_TILE_PREVIOUS_LOW_MASKS: [[u8; 8]; 15] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xE0, 0x10, 0xC8, 0x28, 0xC8, 0x28, 0x10, 0xE0],
    [0xF0, 0x08, 0xE4, 0x94, 0xE4, 0x94, 0x08, 0xF0],
    [0x78, 0x84, 0x72, 0x4A, 0x72, 0x4A, 0x84, 0x78],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
    [0x0F, 0x10, 0x2E, 0x29, 0x2E, 0x29, 0x10, 0x0F],
    [0x07, 0x08, 0x17, 0x14, 0x17, 0x14, 0x08, 0x07],
    [0x03, 0x04, 0x0B, 0x0A, 0x0B, 0x0A, 0x04, 0x03],
    [0x01, 0x02, 0x05, 0x05, 0x05, 0x05, 0x02, 0x01],
    [0x00, 0x01, 0x02, 0x02, 0x02, 0x02, 0x01, 0x00],
    [0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_CURRENT_TILE_PREVIOUS_HIGH_MASKS: [[u8; 8]; 15] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
    [0x0F, 0x10, 0x2E, 0x29, 0x2E, 0x29, 0x10, 0x0F],
    [0x07, 0x08, 0x17, 0x14, 0x17, 0x14, 0x08, 0x07],
    [0x03, 0x04, 0x0B, 0x0A, 0x0B, 0x0A, 0x04, 0x03],
    [0x01, 0x02, 0x05, 0x05, 0x05, 0x05, 0x02, 0x01],
    [0x00, 0x01, 0x02, 0x02, 0x02, 0x02, 0x01, 0x00],
    [0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_NEXT_TILE_PREVIOUS_LOW_MASKS: [[u8; 8]; 15] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00],
    [0x00, 0x80, 0x40, 0x40, 0x40, 0x40, 0x80, 0x00],
    [0x80, 0x40, 0x20, 0xA0, 0x20, 0xA0, 0x40, 0x80],
    [0xC0, 0x20, 0x90, 0x50, 0x90, 0x50, 0x20, 0xC0],
    [0xE0, 0x10, 0xC8, 0x28, 0xC8, 0x28, 0x10, 0xE0],
    [0xF0, 0x08, 0xE4, 0x94, 0xE4, 0x94, 0x08, 0xF0],
    [0x78, 0x84, 0x72, 0x4A, 0x72, 0x4A, 0x84, 0x78],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_NEXT_TILE_PREVIOUS_HIGH_MASKS: [[u8; 8]; 15] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_THIRD_TILE_PREVIOUS_LOW_MASKS: [[u8; 8]; 15] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_THIRD_TILE_PREVIOUS_HIGH_MASKS: [[u8; 8]; 15] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00],
];

const fn window_activation_tile_current_tilemap_mask(
    fetch_x: u16,
    window_tile_row: u8,
) -> Option<u8> {
    if window_tile_row < 24 || window_tile_row >= 144 {
        return None;
    }

    let block = ((window_tile_row - 24) / 8) as usize;
    let row = (window_tile_row & 0x07) as usize;
    match fetch_x {
        0 => Some(WINDOW_ACTIVATION_FIRST_TILE_CURRENT_TILEMAP_MASKS[block][row]),
        x if x == BG_TILE_WIDTH as u16 => {
            Some(WINDOW_ACTIVATION_SECOND_TILE_CURRENT_TILEMAP_MASKS[block][row])
        }
        x if x == BG_TILE_WIDTH as u16 * 2 => match window_tile_row {
            112..=127 => Some(0x00),
            128..=143 => Some(0xFF),
            _ => None,
        },
        _ => None,
    }
}

pub(super) const fn window_lcdc4_unsigned_to_signed_previous_plane_masks(
    fetch_x: u16,
    window_tile_row: u8,
) -> Option<PerPlane<u8>> {
    if window_tile_row < 24 || window_tile_row >= 144 {
        return None;
    }

    let block = ((window_tile_row - 24) / 8) as usize;
    let row = (window_tile_row & 0x07) as usize;
    match fetch_x {
        0 => Some(PerPlane::new(
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_CURRENT_TILE_PREVIOUS_LOW_MASKS[block][row],
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_CURRENT_TILE_PREVIOUS_HIGH_MASKS[block][row],
        )),
        x if x == BG_TILE_WIDTH as u16 => Some(PerPlane::new(
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_NEXT_TILE_PREVIOUS_LOW_MASKS[block][row],
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_NEXT_TILE_PREVIOUS_HIGH_MASKS[block][row],
        )),
        x if x == BG_TILE_WIDTH as u16 * 2 => Some(PerPlane::new(
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_THIRD_TILE_PREVIOUS_LOW_MASKS[block][row],
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_THIRD_TILE_PREVIOUS_HIGH_MASKS[block][row],
        )),
        _ => None,
    }
}
