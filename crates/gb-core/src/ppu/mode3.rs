impl Ppu {
    fn advance_mode3_pipeline<O>(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
        observer: &mut O,
    ) where
        O: PpuStepObserver,
    {
        if self.ly >= VISIBLE_SCANLINES
            || self.line_dot < MODE2_DOTS
            || self.line_dot >= self.current_mode0_start_dot()
        {
            return;
        }

        if !self.bg_pipeline_state.mode3_started {
            observe_ppu_step_region(observer, PpuStepRegion::Mode3Startup, || {
                self.bg_pipeline_state
                    .start_line(self.visible_registers.scx);
            });
        }

        let bg_pipeline_region = self.current_mode3_bg_pipeline_region();
        observe_ppu_step_region(observer, bg_pipeline_region, || {
            self.maybe_recompute_pending_background_fill(vram);
            self.flush_pending_bg_fifo_fill();
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

    fn advance_mode3_object_phase(
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

    fn advance_mode3_output_phase_with_vram(&mut self, vram: &VramBusView<'_>) -> Mode3TransferDot {
        if self
            .bg_pipeline_state
            .consume_startup_transfer_entry_delay_dot()
        {
            return Mode3TransferDot::not_served();
        }

        let transfer_dot = if !self.current_dot_arbitration().can_serve_bg_transfer() {
            self.bg_pipeline_state.extend_mode3_by_one_dot();
            Mode3TransferDot::not_served()
        } else {
            match self.current_transfer() {
                None => return Mode3TransferDot::not_served(),
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
        transfer_dot
    }

    #[cfg(test)]
    fn advance_mode3_output_phase(&mut self) -> Mode3TransferDot {
        let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
        vram.set_acquired(BusMaster::Ppu, true);
        self.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram))
    }

    fn current_dot_has_pending_obj_hit(&self) -> bool {
        self.obj_enabled()
            && self
                .obj_pipeline_state
                .pending_hits_own_current_dot(self.current_obj_hit_ownership())
    }

    fn current_dot_arbitration(&self) -> Mode3DotArbitration {
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

    fn previsible_same_x_chain_can_start_obj_fetch(&self, transfer: Mode3CurrentTransfer) -> bool {
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

    fn previsible_fifo_backed_same_x_chain_can_start_obj_fetch(&self) -> bool {
        if !self.current_transfer_x_supports_early_same_x_obj_start() {
            return false;
        }

        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_pending_match_x();
        matches!(fetched_same_x_count, 1 | 3)
            || (fetched_same_x_count >= 2 && fetched_same_x_count.is_multiple_of(2))
            || self.terminal_previsible_same_x_chain_can_start_obj_fetch()
    }

    fn current_transfer_context(&self) -> Option<Mode3TransferContext> {
        let mode3_dot = self.line_dot.saturating_sub(MODE2_DOTS);
        if !self
            .bg_pipeline_state
            .startup_transfer_window_open(mode3_dot)
        {
            return None;
        }
        if self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH {
            return None;
        }

        let lane = if self.bg_pipeline_state.scx_discard_remaining > 0
            || self.bg_pipeline_state.current_transfer_x < 8
        {
            self.bg_pipeline_state.current_startup_transfer_lane()
        } else {
            Mode3TransferLane::Visible
        };

        let source_window = self
            .bg_pipeline_state
            .current_startup_source_window(mode3_dot);

        Some(Mode3TransferContext {
            lane,
            source_window,
        })
    }

    fn transfer_service_plan_from_context(
        &self,
        context: Mode3TransferContext,
    ) -> Option<Mode3TransferServicePlan> {
        let execution = if self.bg_pipeline_state.scx_discard_remaining > 0 {
            Mode3TransferServiceExecution::ConsumeScxDiscard
        } else if self.bg_pipeline_state.current_transfer_x < 8 {
            match context.lane {
                Mode3TransferLane::PreVisible => {
                    Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop
                }
                Mode3TransferLane::Hidden => {
                    Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop
                }
                Mode3TransferLane::Visible => unreachable!("x < 8 cannot be a visible transfer"),
            }
        } else if context.lane == Mode3TransferLane::Visible {
            Mode3TransferServiceExecution::EmitVisiblePixel
        } else {
            return None;
        };

        let result_kind = if matches!(execution, Mode3TransferServiceExecution::EmitVisiblePixel) {
            Mode3TransferDotKind::ServedVisiblePixel
        } else {
            context.lane.dot_kind()
        };

        let backing = match context.source_window {
            Mode3TransferSourceWindow::AbstractStartup => Mode3TransferBacking::Abstract,
            Mode3TransferSourceWindow::FifoBacked => Mode3TransferBacking::FifoBacked,
        };

        Some(Mode3TransferServicePlan {
            result_kind,
            execution,
            backing,
        })
    }

    #[cfg(test)]
    fn current_transfer_service_plan(&self) -> Option<Mode3TransferServicePlan> {
        self.current_transfer()
            .map(|transfer| transfer.service_plan())
    }

    fn current_transfer(&self) -> Option<Mode3CurrentTransfer> {
        let context = self.current_transfer_context()?;
        let plan = self.transfer_service_plan_from_context(context)?;
        let readiness = if plan.requires_real_bg_fifo_pixel() {
            if self.bg_pipeline_state.fifo.is_empty() {
                Mode3TransferReadiness::WaitingForFifo(plan)
            } else {
                Mode3TransferReadiness::Ready(plan)
            }
        } else if plan.requires_effective_bg_fifo_pixel()
            && self.bg_pipeline_state.effective_fifo_is_empty()
        {
            Mode3TransferReadiness::WaitingForFifo(plan)
        } else {
            Mode3TransferReadiness::Ready(plan)
        };

        Some(Mode3CurrentTransfer { context, readiness })
    }

    fn advance_bg_fetcher(&mut self, vram: &VramBusView<'_>) -> bool {
        self.maybe_abort_window_fetcher_to_background();
        self.maybe_recompute_pending_background_push(vram);

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
                if fetcher.source == PpuBgFetcherSource::Background {
                    self.bg_pipeline_state.fetcher.cached_origin = self
                        .bg_pipeline_state
                        .peek_startup_background_fetch_origin();
                    self.bg_pipeline_state
                        .fetcher
                        .needs_live_tilemap_refetch_on_push = false;
                }
                let tile_map_address =
                    self.compute_fetch_tile_index_address(fetcher.source, fetcher.fetch_x);
                self.bg_pipeline_state.fetcher.tile_map_address = tile_map_address;
                let delay_tileindex_read = fetcher.source == PpuBgFetcherSource::Background
                    && self
                        .bg_pipeline_state
                        .startup_background_tileindex_reads_on_stage_one();
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
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileIndex, 1) => {
                if fetcher.source == PpuBgFetcherSource::Background
                    && self
                        .bg_pipeline_state
                        .startup_background_tileindex_reads_on_stage_one()
                {
                    self.bg_pipeline_state.fetcher.tile_index = vram
                        .read(self.bg_pipeline_state.fetcher.tile_map_address as usize)
                        .unwrap_or(0);
                }
                self.maybe_apply_bgwin_tilemap_selector_glitch(vram, fetcher.source);
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
            }
            (PpuBgFetcherStage::TileDataLow, 0) => {
                let tile_data_address = self.compute_fetch_tile_data_address(
                    fetcher.source,
                    fetcher.fetch_x,
                    fetcher.tile_index,
                    0,
                );
                self.bg_pipeline_state.fetcher.tile_data_address = tile_data_address;
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
                    .push
                    .queue_from_fetcher(self.bg_pipeline_state.fetcher);
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

    fn maybe_abort_window_fetcher_to_background(&mut self) {
        if self.bg_pipeline_state.fetcher.source != PpuBgFetcherSource::Window {
            return;
        }

        if self.visible_registers.window_enabled() {
            return;
        }

        self.bg_pipeline_state.fetcher.abort_window_to_background();
    }

    fn advance_bg_push_stage(&mut self) -> BgPushDotResult {
        let ownership = self.current_bg_push_dot_ownership();
        self.execute_bg_push_dot_ownership(ownership)
    }

    fn current_step_region_after_line_advance(&self) -> PpuStepRegion {
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

    fn current_mode3_bg_pipeline_region(&self) -> PpuStepRegion {
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
    fn advance_bg_push(&mut self) -> BgPushDotResult {
        self.execute_bg_push_dot_ownership(self.current_bg_push_dot_ownership())
    }

    fn current_bg_push_dot_ownership(&self) -> BgPushDotOwnership {
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

    fn execute_bg_push_dot_ownership(&mut self, ownership: BgPushDotOwnership) -> BgPushDotResult {
        match ownership {
            BgPushDotOwnership::NotReady => BgPushDotResult::NotReady,
            BgPushDotOwnership::EntryDelay => {
                debug_assert!(self.bg_pipeline_state.push.entry_delay_remaining > 0);
                self.bg_pipeline_state.push.entry_delay_remaining -= 1;
                if self.bg_pipeline_state.push.entry_delay_remaining == 0
                    && self.saturated_placeholder_backed_terminal_bg_tail_can_hold_one_post_push_dot()
                {
                    self.bg_pipeline_state.push.terminal_placeholder_tail_extra_hold_remaining = 2;
                }
                self.bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open = true;
                BgPushDotResult::EntryDelay
            }
            BgPushDotOwnership::WaitingForEmptyFifo => {
                if self.bg_pipeline_state.push.terminal_placeholder_tail_extra_hold_remaining > 0 {
                    self.bg_pipeline_state.push.terminal_placeholder_tail_extra_hold_remaining -= 1;
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

    fn queue_bg_fill_from_push(&mut self) {
        let push = self.bg_pipeline_state.push;
        if push.cached.is_startup_alignment_seed() {
            self.bg_pipeline_state.begin_post_alignment_followup();
            self.bg_pipeline_state
                .fill
                .queue_startup_alignment_from_push(
                    push,
                    self.bg_pipeline_state.startup_fifo_placeholders,
                );
        } else {
            self.bg_pipeline_state.fill.queue_from_push(push);
        }
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

    fn flush_pending_bg_fifo_fill(&mut self) {
        if !self.bg_pipeline_state.fill.pending {
            return;
        }

        let fill = self.bg_pipeline_state.fill;
        if fill.startup_dummy_pixels > 0 {
            self.bg_pipeline_state
                .push_dummy_fifo_pixels(fill.startup_dummy_pixels);
        }
        if fill.includes_real_tile_pixels {
            self.bg_pipeline_state
                .push_cached_slice_fifo_pixels(fill.cached);
        }
        self.bg_pipeline_state.fill.reset();
    }

    fn maybe_recompute_pending_background_fill(&mut self, vram: &VramBusView<'_>) {
        if !self.bg_pipeline_state.fill.pending
            || self.bg_pipeline_state.fill.cached.source != PpuBgFetcherSource::Background
            || !self.bg_pipeline_state.fill.includes_real_tile_pixels
        {
            return;
        }

        let Some(recomputed) = recompute_live_background_cached_slice(
            self.bg_pipeline_state.fill.cached,
            vram,
            self.lcdc,
            self.scy,
            self.ly,
            self.last_unsigned_tile_data_low_fetch,
            self.last_unsigned_tile_data_high_fetch,
        ) else {
            return;
        };

        self.bg_pipeline_state.fill.cached = recomputed;
    }

    fn maybe_recompute_pending_background_push(&mut self, vram: &VramBusView<'_>) {
        if !self.bg_pipeline_state.push.pending
            || self.bg_pipeline_state.push.cached.source != PpuBgFetcherSource::Background
        {
            return;
        }

        let Some(recomputed) = recompute_live_background_cached_slice(
            self.bg_pipeline_state.push.cached,
            vram,
            self.lcdc,
            self.scy,
            self.ly,
            self.last_unsigned_tile_data_low_fetch,
            self.last_unsigned_tile_data_high_fetch,
        ) else {
            return;
        };

        self.bg_pipeline_state.push.cached = recomputed;
        self.bg_pipeline_state.fetcher.tile_map_address = recomputed.tile_map_address;
        self.bg_pipeline_state.fetcher.tile_index = recomputed.tile_index;
        self.bg_pipeline_state.fetcher.tile_data_address = recomputed.tile_data_address;
        self.bg_pipeline_state.fetcher.tile_low = recomputed.tile_low;
        self.bg_pipeline_state.fetcher.tile_high = recomputed.tile_high;
    }

    fn execute_transfer_service_plan(
        &mut self,
        plan: Mode3TransferServicePlan,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
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
                let bg_pixel = if self.pixel_transfer_bg_enabled() {
                    self.pop_visible_bg_fifo_pixel(vram)
                        .expect("visible transfer plans must carry a BG pixel")
                } else {
                    0
                };
                let obj_pixel = self.pop_obj_fifo_pixel();
                let output_pixel = self.mix_bg_and_obj(bg_pixel, obj_pixel);
                let panel_pixel = if self.visible_output == PpuVisibleOutputState::Driving {
                    self.map_mixed_pixel_to_panel_shade(output_pixel)
                } else {
                    0
                };
                let scanline_pixel = if self.visible_output == PpuVisibleOutputState::Driving {
                    output_pixel.color
                } else {
                    0
                };
                let visible_x = self.bg_pipeline_state.visible_pixels_output as usize;
                self.current_scanline_mixed_pixels[visible_x] = output_pixel;
                self.current_scanline_pixels[visible_x] = scanline_pixel;
                self.framebuffer[self.ly as usize * SCREEN_WIDTH + visible_x] = panel_pixel;
                self.bg_pipeline_state.current_transfer_x =
                    self.bg_pipeline_state.current_transfer_x.saturating_add(1);
                self.bg_pipeline_state.visible_pixels_output += 1;
                if self.dmg_bgp_cpu_commit_output_delay_pixels_remaining > 0 {
                    self.dmg_bgp_cpu_commit_output_delay_pixels_remaining -= 1;
                    if self.dmg_bgp_cpu_commit_output_delay_pixels_remaining == 0 {
                        self.dmg_bgp_cpu_commit_output_palette_override = None;
                    }
                }
                Mode3TransferDot::served(plan.result_kind, false)
            }
        }
    }

    fn pop_visible_bg_fifo_pixel(&mut self, vram: &VramBusView<'_>) -> Option<u8> {
        let mut pixel = self.bg_pipeline_state.pop_visible_fifo_pixel()?;
        let Some(cached) = pixel.cached.as_mut() else {
            return Some(pixel.color);
        };
        let Some(recomputed) = recompute_live_background_cached_slice(
            cached.cached,
            vram,
            self.lcdc,
            self.scy,
            self.ly,
            self.last_unsigned_tile_data_low_fetch,
            self.last_unsigned_tile_data_high_fetch,
        ) else {
            return Some(pixel.color);
        };

        cached.cached = recomputed;
        pixel.color = bg_tile_pixel_value(
            recomputed.tile_low,
            recomputed.tile_high,
            cached.pixel_index,
        );
        Some(pixel.color)
    }

    fn obj_enabled(&self) -> bool {
        self.visible_registers.obj_enabled()
    }

    fn maybe_apply_wx0_shortening_after_transfer_dot(&mut self, transfer_dot: Mode3TransferDot) {
        if !transfer_dot.consumed_scx_discard
            || self.bg_pipeline_state.window_started_this_line
            || !self.bg_pipeline_state.window_wy_latch
            || !self.window_runtime_enabled()
            || self.window_activation_registers().wx != 0
            || self.bg_pipeline_state.window_force_x0_this_line
            || self.bg_pipeline_state.visible_pixels_output != 0
            || self.bg_pipeline_state.current_transfer_x >= 8
            || self.bg_pipeline_state.initial_scx_discard == 0
            || self.bg_pipeline_state.scx_discard_remaining != 0
        {
            return;
        }

        self.bg_pipeline_state.apply_wx0_scx_shortening();
    }

    fn maybe_start_window_after_transfer_dot(&mut self, transfer_dot: Mode3TransferDot) -> bool {
        if !transfer_dot.is_served()
            || self.bg_pipeline_state.window_started_this_line
            || !self.bg_pipeline_state.window_wy_latch
            || !self.window_runtime_enabled()
        {
            return false;
        }

        if self.window_activation_registers().wx == 166
            && !self.bg_pipeline_state.window_force_x0_this_line
        {
            if self.bg_pipeline_state.visible_pixels_output as usize == SCREEN_WIDTH
                && self.bg_pipeline_state.scx_discard_remaining == 0
                && !self.bg_pipeline_state.wx166_armed_this_line
            {
                self.window_state.pending_wx166_next_line = true;
                self.bg_pipeline_state.wx166_armed_this_line = true;
            }
            return false;
        }

        let Some(trigger_x) = self.window_trigger_x_for_current_line() else {
            return false;
        };

        if !self.should_start_window_after_transfer_dot_now(trigger_x, transfer_dot) {
            return false;
        }

        self.start_window_fetcher_restart();
        true
    }

    fn window_runtime_enabled(&self) -> bool {
        let registers = self.window_activation_registers();
        registers.window_enabled() && registers.bg_enabled()
    }

    fn latch_object_fetch_hits(&mut self) {
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
                self.obj_pipeline_state
                    .queue_fetch_hit(sprite_slot, current_owner);
            }
        }
    }

    fn sync_pending_obj_hit_ownership(&mut self) {
        if !self.obj_enabled() {
            self.obj_pipeline_state.clear_pending_fetch_hits();
            return;
        }

        let current_owner = self.current_obj_hit_ownership();
        self.obj_pipeline_state
            .clear_pending_fetch_hits_if_stale(current_owner);
    }

    fn try_start_object_fetch_from_current_dot(
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

        let Some(sprite_slot) = self.obj_pipeline_state.pop_pending_fetch_hit() else {
            return false;
        };
        let Some(sprite) = self.mode2_scan_state.selected_sprite(sprite_slot) else {
            return false;
        };

        self.obj_pipeline_state.start_fetch(sprite_slot, sprite);
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

    fn current_obj_hit_ownership(&self) -> ObjHitOwnership {
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

    fn bg_fetcher_ready_for_fifo_backed_obj_start(&self) -> bool {
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

    fn advance_object_fetch(
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
            && !hidden_left_edge_same_x_chain_pays_push_dot)
            ;
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
                let resolved_sprite = fetch
                    .resolved_sprite
                    .expect("active OBJ fetch must resolve tile metadata before reading tile data");
                self.obj_pipeline_state.fetch.tile_low =
                    self.read_obj_tile_data_byte(vram, resolved_sprite, 0);
                self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
                self.obj_pipeline_state.fetch.stage_dot = 0;
            }
            (PpuObjFetcherStage::TileDataHigh, 0) => {
                self.obj_pipeline_state.fetch.stage_dot = 1;
            }
            (PpuObjFetcherStage::TileDataHigh, 1) => {
                let resolved_sprite = fetch
                    .resolved_sprite
                    .expect("active OBJ fetch must resolve tile metadata before reading tile data");
                self.obj_pipeline_state.fetch.tile_high =
                    self.read_obj_tile_data_byte(vram, resolved_sprite, 1);
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
                    self.push_obj_pixels(
                        resolved_sprite,
                        fetch.tile_low,
                        fetch.tile_high,
                        self.bg_pipeline_state.visible_pixels_output,
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

    fn obj_fetch_startup_ready(&self) -> bool {
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

    fn resolve_obj_fetch_sprite(
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

    fn chained_same_x_obj_fetch_skips_first_tile_data_low_half_step(&self) -> bool {
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

    fn chained_same_x_obj_fetch_uses_long_tail_restart(&self) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        fetched_same_x_count >= 5
            && fetched_same_x_count % 2 == 1
            && !self.current_transfer_x_supports_early_same_x_obj_start()
    }

    fn first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte(&self) -> bool {
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

    fn initial_nonterminal_same_x_cluster_skips_first_low_half_step(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 167
            && (self.bg_pipeline_state.current_transfer_x & 0x07) < 7
            && self.current_transfer_x_supports_early_same_x_obj_start()
    }

    fn nonterminal_same_x_cluster_restart_skips_first_low_half_step(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 167
            && (self.bg_pipeline_state.current_transfer_x & 0x07) < 7
            && self.current_transfer_x_supports_early_same_x_obj_start()
    }

    fn hidden_same_x_cluster_restart_skips_first_low_half_step(&self) -> bool {
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

    fn visible_periodic_same_x_cluster_restart_skips_first_low_half_step(&self) -> bool {
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

    fn hidden_left_edge_same_x_chain_pays_push_dot(&self) -> bool {
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

    fn visible_left_edge_same_x_chain_shares_push_dot(&self) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        let visible_output = self.bg_pipeline_state.visible_pixels_output;
        let current_transfer_x = self.bg_pipeline_state.current_transfer_x;
        let repeated_visible_boundary_fetched_count_supports_push_dot_share =
            (1..=4).contains(&fetched_same_x_count);
        let late_visible_same_x_cluster_shares_push_dot =
            (visible_output & 0x1f) >= 24
                && self.current_transfer_x_supports_early_same_x_obj_start()
                && fetched_same_x_count >= 5;
        let same_x_screen_edge_supports_push_dot_share =
            (visible_output < 8 && self.current_transfer_x_supports_early_same_x_obj_start())
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

    fn first_late_visible_push_backed_same_x_cluster_chains_after_push(&self) -> bool {
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

    fn right_edge_visible_same_x_cluster_continues_after_push(&self) -> bool {
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

    fn continue_same_x_obj_chain_after_push(
        &mut self,
        oam: &OamBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        let pending_nonterminal_same_x_cluster_pays_startup_dot =
            self.pending_nonterminal_same_x_cluster_pays_startup_dot();
        let right_edge_visible_same_x_cluster_pays_startup_dot =
            self.right_edge_visible_same_x_cluster_pays_startup_dot();
        let right_edge_visible_same_x_cluster =
            right_edge_visible_same_x_cluster_pays_startup_dot
                || self.right_edge_visible_same_x_cluster_continues_after_push();
        let started =
            self.try_start_object_fetch_from_current_dot(ObjFetchStartSource::FifoBackedTransfer, true);
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
                self.obj_pipeline_state.fetch.stage_dot = u8::from(
                    self.chained_same_x_obj_fetch_skips_first_tile_data_low_half_step(),
                );
            }
        }

        true
    }

    fn right_edge_visible_same_x_cluster_pays_startup_dot(&self) -> bool {
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

    fn saturated_placeholder_backed_terminal_bg_tail_can_hold_one_post_push_dot(&self) -> bool {
        self.bg_pipeline_state.mode3_started
            && self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.bg_pipeline_state.current_transfer_x >= 168
            && (160..=161).any(|sprite_x| {
                (0..self.mode2_scan_state.selected_sprite_count()).filter(|&slot| {
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

    fn terminal_previsible_same_x_chain_can_start_obj_fetch(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 8
            && self.obj_pipeline_state.pending_match_x
                == Some(self.bg_pipeline_state.current_transfer_x)
            && self.obj_pipeline_state.pending_sprite_slots.len() == 1
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() > 0
    }

    fn terminal_previsible_same_x_chain_skips_first_low_half_step(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 8
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() > 0
    }

    fn terminal_previsible_same_x_chain_skips_obj_tile_data_low_byte(&self) -> bool {
        self.terminal_previsible_same_x_chain_skips_first_low_half_step()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() >= 9
    }

    fn terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step(&self) -> bool {
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

    fn terminal_right_edge_same_x_chain_shares_push_dot(&self) -> bool {
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

    fn current_transfer_x_supports_early_same_x_obj_start(&self) -> bool {
        matches!(self.bg_pipeline_state.current_transfer_x & 0x07, 2..=7)
    }

    fn terminal_mode3_dot_started_shared_obj_fetch(&self) -> bool {
        matches!(
            (
                self.obj_pipeline_state.fetch.stage,
                self.obj_pipeline_state.fetch.stage_dot,
            ),
            (PpuObjFetcherStage::Startup, 1)
        ) && self.line_dot.saturating_add(1) == self.current_mode0_start_dot()
    }

    fn pending_nonterminal_same_x_cluster_pays_startup_dot(&self) -> bool {
        self.bg_pipeline_state.current_transfer_x < 167
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && self.obj_pipeline_state.pending_match_x
                == Some(self.bg_pipeline_state.current_transfer_x)
            && self.obj_pipeline_state.pending_sprite_slots.len() >= 2
            && !self.first_late_visible_push_backed_same_x_cluster_chains_after_push()
    }

    fn fetched_same_x_obj_sprite_count_for_active_fetch(&self) -> usize {
        let Some(sprite) = self.obj_pipeline_state.fetch.sprite else {
            return 0;
        };
        let Some(trigger_x) = sprite_trigger_x(sprite) else {
            return 0;
        };

        self.fetched_same_x_obj_sprite_count_for_trigger_x(trigger_x)
    }

    fn fetched_same_x_obj_sprite_count_for_pending_match_x(&self) -> usize {
        let Some(trigger_x) = self.obj_pipeline_state.pending_match_x else {
            return 0;
        };

        self.fetched_same_x_obj_sprite_count_for_trigger_x(trigger_x)
    }

    fn fetched_same_x_obj_sprite_count_for_trigger_x(&self, trigger_x: u8) -> usize {
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

    fn window_trigger_x_for_current_line(&self) -> Option<u8> {
        if self.bg_pipeline_state.window_force_x0_this_line {
            return Some(0);
        }

        let registers = self.window_activation_registers();
        match registers.wx {
            0..=166 => Some(registers.wx.saturating_sub(7)),
            _ => None,
        }
    }

    fn should_start_window_after_transfer_dot_now(
        &self,
        trigger_x: u8,
        transfer_dot: Mode3TransferDot,
    ) -> bool {
        if self.bg_pipeline_state.visible_pixels_output != trigger_x {
            return false;
        }

        if trigger_x == 0 {
            return self.bg_pipeline_state.scx_discard_remaining == 0
                && self.bg_pipeline_state.current_transfer_x >= 8
                && transfer_dot.can_start_window_after_x0_service();
        }

        self.bg_pipeline_state.scx_discard_remaining == 0
            && transfer_dot.kind == Mode3TransferDotKind::ServedVisiblePixel
    }

    fn start_window_fetcher_restart(&mut self) {
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
