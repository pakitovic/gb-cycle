impl Ppu {
    const DMG_WX0_WINDOW_DISABLE_PREFIX_PIXELS: [u8; 8] = [9, 10, 3, 4, 5, 6, 7, 8];
    const DMG_LATE_WINDOW_ENABLE_SEGMENT_PIXELS: u8 = 24;
    const DMG_VISIBLE_WINDOW_ORIGIN_WX: u8 = 7;
    const DMG_LOW_WX_CANCEL_ONLY_PREVIOUS_WX_MIN: u8 = 6;
    const DMG_ONE_HIDDEN_PREFIX_SKIP: u8 = 1;
    const DMG_RETAINED_FIFO_PREFIX_RESUME_MIN_HIDDEN_SKIP: u8 = 2;
    const DMG_RETAINED_FIFO_PREFIX_NEXT_TILEMAP_MIN_HIDDEN_SKIP: u8 = 3;
    const DMG_LATE_WRITE_ONE_HIDDEN_PREFIX_KEEP_DISTANCE: u8 = 3;

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

        if !self.runtime.bg_pipeline_state.mode3_started {
            observe_ppu_step_region(observer, PpuStepRegion::Mode3Startup, || {
                let mode3_start_scx = self.mode3_register_latches().mode3_start_scx();
                let current_obj_height = self.mode3_register_latches().current_obj_height();
                self.runtime.bg_pipeline_state.start_line(mode3_start_scx);
                self.runtime.obj_pipeline_state.mode3_line_start_obj_height = current_obj_height;
            });
        }
        if self.line_dot == MODE2_DOTS + MODE3_INITIAL_SCX_CAPTURE_DOT {
            observe_ppu_step_region(observer, PpuStepRegion::Mode3Startup, || {
                let mode3_start_scx = self.mode3_register_latches().mode3_start_scx();
                self.runtime
                    .bg_pipeline_state
                    .capture_initial_scx(mode3_start_scx);
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
            self.maybe_apply_pending_dmg_live_wx_trigger_glitch(output_dot);
            self.maybe_apply_pending_dmg_previsible_wx_carry(output_dot, vram);
            self.maybe_apply_wx0_shortening_after_transfer_dot(output_dot);
            let _ = self.maybe_start_window_after_transfer_dot(output_dot);
            self.maybe_apply_pending_dmg_previsible_wx_onset_glitch_repaint(vram);
            self.apply_dmg_late_window_enable_override_repaint_up_to(
                usize::from(self.runtime.bg_pipeline_state.visible_pixels_output),
                vram,
            );
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
            self.runtime.bg_pipeline_state.extend_mode3_by_one_dot();
        }
        self.advance_object_fetch(oam, vram, dma_oam_conflict)
    }

    pub(super) fn advance_mode3_output_phase_with_vram(
        &mut self,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
        if self
            .runtime
            .bg_pipeline_state
            .consume_startup_transfer_entry_delay_dot()
        {
            self.consume_dmg_bgp_cpu_commit_output_delay();
            return Mode3TransferDot::not_served();
        }

        let transfer_dot = if !self.current_dot_arbitration().can_serve_bg_transfer() {
            self.runtime.bg_pipeline_state.extend_mode3_by_one_dot();
            Mode3TransferDot::not_served()
        } else {
            match self.current_transfer() {
                None => Mode3TransferDot::not_served(),
                Some(Mode3CurrentTransfer {
                    readiness: Mode3TransferReadiness::WaitingForFifo(_),
                    ..
                }) => {
                    self.runtime.bg_pipeline_state.extend_mode3_by_one_dot();
                    Mode3TransferDot::not_served()
                }
                Some(Mode3CurrentTransfer {
                    readiness: Mode3TransferReadiness::Ready(plan),
                    ..
                }) => self.execute_transfer_service_plan(plan, vram),
            }
        };

        self.runtime
            .bg_pipeline_state
            .consume_startup_source_window_dot();
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
            || self.runtime.bg_pipeline_state.window_started_this_line
            || !self
                .runtime
                .bg_pipeline_state
                .startup_alignment_seed_pending()
        {
            return;
        }

        let visible_scx = self.mode3_register_latches().visible().scx;
        self.runtime
            .bg_pipeline_state
            .retune_previsible_scx_discard(visible_scx);
    }

    pub(super) fn current_dot_has_pending_obj_hit(&self) -> bool {
        self.obj_enabled()
            && self
                .runtime
                .obj_pipeline_state
                .pending_hits_own_current_dot(self.current_obj_hit_ownership())
    }

    pub(super) fn current_dot_arbitration(&self) -> Mode3DotArbitration {
        let has_pending_obj_hit = self.current_dot_has_pending_obj_hit();
        let obj_fetch_can_start = self.runtime.obj_pipeline_state.fetch.stage
            == PpuObjFetcherStage::Idle
            && self.obj_enabled()
            && has_pending_obj_hit;
        let current_transfer_is_fifo_backed = self.current_transfer().is_some_and(|transfer| {
            (transfer.can_start_obj_fetch_from_fifo_backed_transfer(
                self.runtime.bg_pipeline_state.fifo_contains_real_pixels(),
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
        ) && !self.runtime.bg_pipeline_state.effective_fifo_is_empty()
            && self.runtime.obj_pipeline_state.pending_match_x
                == Some(self.runtime.bg_pipeline_state.current_transfer_x)
            && !self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
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
            self.runtime.bg_pipeline_state.fifo.is_empty(),
            self.runtime.bg_pipeline_state.effective_fifo_is_empty(),
        )
    }
}
