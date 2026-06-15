use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3TransferPolicy {
    mode3_started: bool,
    startup_source_state: Mode3StartupSourceState,
    startup_pre_visible_transfer_dots_remaining: u8,
    initial_scx_discard: u8,
    current_transfer_x: u8,
    visible_pixels_output: u8,
    scx_discard_remaining: u8,
    line_dot: u16,
}

impl PpuMode3TransferPolicy {
    pub(in crate::ppu) const fn from_pipeline_state(
        bg_pipeline_state: &BgPipelineState,
        line_dot: u16,
    ) -> Self {
        Self {
            mode3_started: bg_pipeline_state.mode3_started,
            startup_source_state: bg_pipeline_state.startup_source_state,
            startup_pre_visible_transfer_dots_remaining: bg_pipeline_state
                .startup_pre_visible_transfer_dots_remaining,
            initial_scx_discard: bg_pipeline_state.initial_scx_discard,
            current_transfer_x: bg_pipeline_state.current_transfer_x,
            visible_pixels_output: bg_pipeline_state.visible_pixels_output,
            scx_discard_remaining: bg_pipeline_state.scx_discard_remaining,
            line_dot,
        }
    }

    pub(in crate::ppu) const fn mode3_dot(self) -> u16 {
        self.line_dot.saturating_sub(MODE2_DOTS)
    }

    pub(in crate::ppu) const fn startup_transfer_window_open(self) -> bool {
        if !self.mode3_started {
            return self.mode3_dot() >= MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
        }

        !matches!(
            self.startup_source_state,
            Mode3StartupSourceState::EntryDelay { .. }
        )
    }

    pub(in crate::ppu) const fn current_startup_source_window(self) -> Mode3TransferSourceWindow {
        if !self.mode3_started {
            if self.mode3_dot() < MODE3_BG_FETCH_PRIMING_DOTS {
                return Mode3TransferSourceWindow::AbstractStartup;
            }

            return Mode3TransferSourceWindow::FifoBacked;
        }

        match self.startup_source_state {
            Mode3StartupSourceState::EntryDelay { .. }
            | Mode3StartupSourceState::Abstract { .. } => {
                Mode3TransferSourceWindow::AbstractStartup
            }
            Mode3StartupSourceState::FifoBacked => Mode3TransferSourceWindow::FifoBacked,
        }
    }

    pub(in crate::ppu) const fn current_startup_transfer_lane(self) -> Mode3TransferLane {
        if self.startup_pre_visible_transfer_dots_remaining > 0 {
            Mode3TransferLane::PreVisible
        } else {
            Mode3TransferLane::Hidden
        }
    }

    pub(in crate::ppu) fn current_transfer_context(self) -> Option<Mode3TransferContext> {
        if !self.startup_transfer_window_open()
            || self.visible_pixels_output as usize >= SCREEN_WIDTH
        {
            return None;
        }

        let lane = if self.scx_discard_remaining > 0 || self.current_transfer_x < 8 {
            self.current_startup_transfer_lane()
        } else {
            Mode3TransferLane::Visible
        };

        Some(Mode3TransferContext {
            lane,
            source_window: self.current_startup_source_window(),
        })
    }

    pub(in crate::ppu) fn transfer_service_plan(
        self,
        context: Mode3TransferContext,
    ) -> Option<Mode3TransferServicePlan> {
        let execution = if self.scx_discard_remaining > 0 {
            Mode3TransferServiceExecution::ConsumeScxDiscard
        } else if self.current_transfer_x < 8 {
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

        Some(Mode3TransferServicePlan {
            result_kind,
            execution,
        })
    }

    pub(in crate::ppu) fn current_transfer(
        self,
        bg_fifo_empty: bool,
    ) -> Option<Mode3CurrentTransfer> {
        let context = self.current_transfer_context()?;
        let plan = self.transfer_service_plan(context)?;
        // Canonical: every transfer dot pops one real BG FIFO entry (the startup junk pixels are
        // real FIFO entries now), so readiness is simply whether the FIFO has a pixel to pop —
        // the abstract/real backing distinction is gone. See docs/roadmap/12 §24.
        let readiness = if bg_fifo_empty {
            Mode3TransferReadiness::WaitingForFifo(plan)
        } else {
            Mode3TransferReadiness::Ready(plan)
        };

        Some(Mode3CurrentTransfer { context, readiness })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3LineTimingPolicy {
    visible_registers: PpuVisibleRegisters,
    mode3_started: bool,
    mode0_start_dot: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3LineTimingContext {
    pub(in crate::ppu) line_dot: u16,
    pub(in crate::ppu) obj_fetch_active: bool,
    pub(in crate::ppu) pending_obj_hit_owns_current_transfer_x: bool,
    pub(in crate::ppu) live_transfer_still_owned_by_mode3: bool,
    pub(in crate::ppu) saturated_placeholder_tail_still_owned_by_mode3: bool,
}

impl PpuMode3LineTimingPolicy {
    pub(in crate::ppu) const fn new(
        visible_registers: PpuVisibleRegisters,
        mode3_started: bool,
        mode0_start_dot: u16,
    ) -> Self {
        Self {
            visible_registers,
            mode3_started,
            mode0_start_dot,
        }
    }

    pub(in crate::ppu) const fn baseline_mode0_start_dot(self) -> u16 {
        MODE0_START_DOT + (self.visible_registers.scx & 0x07) as u16
    }

    pub(in crate::ppu) fn current_mode0_start_dot(self, context: PpuMode3LineTimingContext) -> u16 {
        let mut mode0_start_dot = if self.mode3_started {
            self.mode0_start_dot
        } else {
            self.baseline_mode0_start_dot()
        };

        if self.mode3_started
            && (context.obj_fetch_active
                || context.pending_obj_hit_owns_current_transfer_x
                || context.live_transfer_still_owned_by_mode3
                || context.saturated_placeholder_tail_still_owned_by_mode3)
        {
            mode0_start_dot = mode0_start_dot.max(context.line_dot.saturating_add(1));
        }
        mode0_start_dot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3WindowPolicy {
    visible_registers: PpuVisibleRegisters,
    activation: PpuMode3WindowActivationState,
    fetcher_window_enabled: bool,
    wy_latch: bool,
    started_this_line: bool,
}

impl PpuMode3WindowPolicy {
    pub(in crate::ppu) const fn new(
        visible_registers: PpuVisibleRegisters,
        activation: PpuMode3WindowActivationState,
        fetcher_window_enabled: bool,
        wy_latch: bool,
        started_this_line: bool,
    ) -> Self {
        Self {
            visible_registers,
            activation,
            fetcher_window_enabled,
            wy_latch,
            started_this_line,
        }
    }

    pub(in crate::ppu) fn prepare_line(
        self,
        ly: u8,
        wy_triggered: bool,
        pending_wx166_next_line: bool,
    ) -> PpuMode3PreparedWindowLine {
        let wy_triggered = wy_triggered
            || (self.visible_registers.wy < VISIBLE_SCANLINES && ly == self.visible_registers.wy);
        let wy_latch = wy_triggered && self.visible_registers.wy < VISIBLE_SCANLINES;
        let force_x0_this_line = wy_latch && pending_wx166_next_line;
        PpuMode3PreparedWindowLine::new(wy_triggered, wy_latch, force_x0_this_line)
    }

    pub(in crate::ppu) const fn fetcher_should_stay_windowed(self) -> bool {
        self.fetcher_window_enabled
    }

    pub(in crate::ppu) fn can_apply_wx0_shortening(
        self,
        transfer_dot: Mode3TransferDot,
        visible_pixels_output: u8,
        current_transfer_x: u8,
        initial_scx_discard: u8,
        scx_discard_remaining: u8,
    ) -> bool {
        (transfer_dot.consumed_scx_discard || scx_discard_remaining == 0)
            && !self.started_this_line
            && self.wy_latch
            && self.activation.runtime_enabled()
            && self.activation.is_wx_zero()
            && visible_pixels_output == 0
            && current_transfer_x < 8
            && initial_scx_discard != 0
            && scx_discard_remaining == 0
    }

    pub(in crate::ppu) fn start_decision_after_transfer_dot(
        self,
        transfer_dot: Mode3TransferDot,
        visible_pixels_output: u8,
        current_transfer_x: u8,
        initial_scx_discard: u8,
        scx_discard_remaining: u8,
        wx166_armed_this_line: bool,
    ) -> PpuMode3WindowStartDecision {
        if !transfer_dot.is_served() || !self.wy_latch || !self.activation.runtime_enabled() {
            return PpuMode3WindowStartDecision::NotReady;
        }

        if self.activation.is_wx_166() {
            if self.started_this_line {
                return PpuMode3WindowStartDecision::NotReady;
            }
            if visible_pixels_output as usize == SCREEN_WIDTH
                && scx_discard_remaining == 0
                && !wx166_armed_this_line
            {
                return PpuMode3WindowStartDecision::ArmWx166NextLine;
            }
            return PpuMode3WindowStartDecision::NotReady;
        }

        if let Some(hidden_trigger_transfer_x) = self.activation.low_wx_hidden_trigger_transfer_x()
        {
            let can_start_now = !self.started_this_line
                && scx_discard_remaining == 0
                && visible_pixels_output == 0
                && current_transfer_x == hidden_trigger_transfer_x
                && matches!(
                    transfer_dot.kind,
                    Mode3TransferDotKind::ServedPreVisibleTransfer
                        | Mode3TransferDotKind::ServedHiddenTransfer
                );

            return if can_start_now {
                PpuMode3WindowStartDecision::StartNow
            } else {
                PpuMode3WindowStartDecision::NotReady
            };
        }

        let Some(trigger_x) = self.activation.trigger_x() else {
            return PpuMode3WindowStartDecision::NotReady;
        };

        let can_start_now = if trigger_x == 0 {
            let first_visible_pixel_from_scx_discard = initial_scx_discard != 0
                && visible_pixels_output == 1
                && current_transfer_x <= 8
                && transfer_dot.kind == Mode3TransferDotKind::ServedVisiblePixel;

            scx_discard_remaining == 0
                && transfer_dot.can_start_window_after_x0_service()
                && ((visible_pixels_output == 0 && current_transfer_x >= 8)
                    || first_visible_pixel_from_scx_discard)
        } else {
            scx_discard_remaining == 0
                && visible_pixels_output == trigger_x
                && transfer_dot.kind == Mode3TransferDotKind::ServedVisiblePixel
        };

        if can_start_now {
            PpuMode3WindowStartDecision::StartNow
        } else {
            PpuMode3WindowStartDecision::NotReady
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3BgWinFetchPolicy {
    register_latches: PpuMode3RegisterLatches,
    console_model: ConsoleModel,
    dmg_software_contract: bool,
    background_tilemap_uses_pipeline_snapshot: bool,
    background_tiledata_uses_pipeline_snapshot: bool,
    background_tileindex_reads_on_stage_one: bool,
    window_tilemap_uses_pipeline_snapshot: bool,
}

impl PpuMode3BgWinFetchPolicy {
    pub(in crate::ppu) const fn new(
        register_latches: PpuMode3RegisterLatches,
        console_model: ConsoleModel,
        dmg_software_contract: bool,
        background_tilemap_uses_pipeline_snapshot: bool,
        background_tiledata_uses_pipeline_snapshot: bool,
        background_tileindex_reads_on_stage_one: bool,
        window_tilemap_uses_pipeline_snapshot: bool,
    ) -> Self {
        Self {
            register_latches,
            console_model,
            dmg_software_contract,
            background_tilemap_uses_pipeline_snapshot,
            background_tiledata_uses_pipeline_snapshot,
            background_tileindex_reads_on_stage_one,
            window_tilemap_uses_pipeline_snapshot,
        }
    }

    pub(in crate::ppu) fn background_fetch_context(
        self,
        next_fetch_pixel: u16,
        ly: u8,
    ) -> PpuMode3BackgroundFetchContext {
        PpuMode3BackgroundFetchContext::new(
            self.register_latches
                .bg_fetch_registers(self.background_tilemap_uses_pipeline_snapshot),
            self.register_latches
                .bg_fetch_registers(self.background_tiledata_uses_pipeline_snapshot),
            next_fetch_pixel,
            ly,
        )
    }

    pub(in crate::ppu) fn window_fetch_context(
        self,
        window_line_counter: u8,
        window_tilemap_x: u8,
    ) -> PpuMode3WindowFetchContext {
        PpuMode3WindowFetchContext::new(
            self.register_latches
                .window_fetch_registers(self.window_tilemap_uses_pipeline_snapshot),
            window_line_counter,
            window_tilemap_x,
        )
    }

    pub(in crate::ppu) fn tile_data_selector_changed(self) -> bool {
        self.console_model.is_dmg_family()
            && self
                .register_latches
                .lcdc_bit_changed(LCDC_BG_WINDOW_TILE_DATA_BIT)
    }

    pub(in crate::ppu) fn tilemap_selector_changed(self, source: PpuBgFetcherSource) -> bool {
        let map_bit = match source {
            PpuBgFetcherSource::Background => LCDC_BG_TILE_MAP_BIT,
            PpuBgFetcherSource::Window => LCDC_WINDOW_TILE_MAP_BIT,
        };
        self.dmg_software_contract && self.register_latches.lcdc_bit_changed(map_bit)
    }

    pub(in crate::ppu) fn should_delay_background_tileindex_read(
        self,
        source: PpuBgFetcherSource,
    ) -> bool {
        source == PpuBgFetcherSource::Background && self.background_tileindex_reads_on_stage_one
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(in crate::ppu) struct PpuMode3LiveScyWriteRouting {
    pub(in crate::ppu) pending_high_plane_only: bool,
    pub(in crate::ppu) pending_tilemap_row_refetch: bool,
    pub(in crate::ppu) startup_visible_tile2_tilemap_row_refetch: bool,
    pub(in crate::ppu) startup_visible_tile2_phase6_tilemap_row_refetch: bool,
    pub(in crate::ppu) defers_current_tile_data_fetch_to_next: bool,
    pub(in crate::ppu) defers_current_tile_tilemap_row_to_next: bool,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(in crate::ppu) struct PpuMode3CgbDmgLiveScyWriteRoute {
    pending_cached_slices: bool,
    startup_alignment_fifo: bool,
    current_fetch: bool,
    scy_routing: PpuMode3LiveScyWriteRouting,
}

impl PpuMode3CgbDmgLiveScyWriteRoute {
    pub(in crate::ppu) const fn new(
        pending_cached_slices: bool,
        startup_alignment_fifo: bool,
        current_fetch: bool,
        scy_routing: PpuMode3LiveScyWriteRouting,
    ) -> Self {
        Self {
            pending_cached_slices,
            startup_alignment_fifo,
            current_fetch,
            scy_routing,
        }
    }

    pub(in crate::ppu) const fn routes_anything(self) -> bool {
        self.pending_cached_slices || self.startup_alignment_fifo || self.current_fetch
    }

    pub(in crate::ppu) const fn pending_cached_slices(self) -> bool {
        self.pending_cached_slices
    }

    pub(in crate::ppu) const fn startup_alignment_fifo(self) -> bool {
        self.startup_alignment_fifo
    }

    pub(in crate::ppu) const fn current_fetch(self) -> bool {
        self.current_fetch
    }

    pub(in crate::ppu) const fn scy_routing(self) -> PpuMode3LiveScyWriteRouting {
        self.scy_routing
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(in crate::ppu) struct PpuMode3LiveBackgroundWriteEffects {
    tilemap_refetch: bool,
    tilemap_full_refetch: bool,
    tile_data_refetch: bool,
    tile_data_current_row_refetch: bool,
    tile_low_current_row_refetch: bool,
    tile_high_current_row_refetch: bool,
    fetcher_tilemap_refetch_on_push: bool,
    fetcher_tilemap_full_refetch_on_push: bool,
    fetcher_tile_data_refetch_on_push: bool,
    fetcher_tile_data_current_row_refetch_on_push: bool,
    fetcher_tile_low_current_row_refetch_on_push: bool,
    fetcher_tile_high_current_row_refetch_on_push: bool,
}

impl PpuMode3LiveBackgroundWriteEffects {
    pub(in crate::ppu) fn for_push_pending_slice(
        cached: BgCachedSlice,
        register: PpuMode3LiveBackgroundRegister,
        write_context: PpuMode3LiveRegisterWriteContext,
        entry_delay_active: bool,
        ly: u8,
        scy_routing: PpuMode3LiveScyWriteRouting,
    ) -> Self {
        if cached.is_startup_alignment_seed() {
            return Self::default();
        }

        let background_cached = cached.is_background();
        let scy_pending_refetch_window = background_cached
            && (entry_delay_active
                || cached.same_cycle_live_tilemap_refetch_window_open
                || cached.is_second_or_third_visible_post_startup_push());
        let scy_tile_data_row_changed = background_cached
            && matches!(register, PpuMode3LiveBackgroundRegister::Scy)
            && scy_pending_refetch_window
            && write_context.bg_scy_tile_data_row_changed(ly);
        let scy_uses_startup_visible_tile2_tilemap_row = scy_routing
            .startup_visible_tile2_tilemap_row_refetch
            && matches!(
                cached.origin,
                BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
            );
        let scy_startup_visible_tile2_tilemap_phase_matches = matches!(ly & 0x07, 7)
            || scy_routing.startup_visible_tile2_phase6_tilemap_row_refetch
                && matches!(ly & 0x07, 6);
        let scy_tilemap_row_changed = scy_routing.pending_tilemap_row_refetch
            || scy_uses_startup_visible_tile2_tilemap_row
                && scy_startup_visible_tile2_tilemap_phase_matches;
        let scy_tilemap_row_changed = scy_tilemap_row_changed
            && background_cached
            && matches!(register, PpuMode3LiveBackgroundRegister::Scy)
            && scy_pending_refetch_window
            && write_context.bg_scy_tilemap_row_changed(ly);
        let lcdc_tilemap_refetch = background_cached
            && matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bgwin_tilemap_select_changed(cached.source)
            && (entry_delay_active
                || cached.same_cycle_live_tilemap_refetch_window_open
                || cached.is_second_or_third_visible_post_startup_push());
        let lcdc_tiledata_refetch = matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bg_window_tile_data_select_changed()
            && (background_cached || cached.fetch_x != 0);

        Self {
            tilemap_refetch: lcdc_tilemap_refetch || scy_tilemap_row_changed,
            tilemap_full_refetch: scy_tilemap_row_changed,
            tile_data_refetch: lcdc_tiledata_refetch || scy_tile_data_row_changed,
            tile_data_current_row_refetch: scy_tile_data_row_changed
                && !scy_routing.pending_high_plane_only,
            tile_low_current_row_refetch: false,
            tile_high_current_row_refetch: scy_tile_data_row_changed
                && scy_routing.pending_high_plane_only,
            fetcher_tilemap_refetch_on_push: false,
            fetcher_tilemap_full_refetch_on_push: false,
            fetcher_tile_data_refetch_on_push: false,
            fetcher_tile_data_current_row_refetch_on_push: false,
            fetcher_tile_low_current_row_refetch_on_push: false,
            fetcher_tile_high_current_row_refetch_on_push: false,
        }
    }

    pub(in crate::ppu) fn for_fill_pending_slice(
        cached: BgCachedSlice,
        register: PpuMode3LiveBackgroundRegister,
        write_context: PpuMode3LiveRegisterWriteContext,
        includes_real_tile_pixels: bool,
        startup_dummy_pixels: u8,
        ly: u8,
        scy_routing: PpuMode3LiveScyWriteRouting,
    ) -> Self {
        if !includes_real_tile_pixels {
            return Self::default();
        }

        let background_cached = cached.is_background();
        let scy_pending_refetch_window = background_cached
            && startup_dummy_pixels == 0
            && (cached.same_cycle_live_tilemap_refetch_window_open
                || cached.is_second_or_third_visible_post_startup_push());
        let scy_tile_data_row_changed = background_cached
            && matches!(register, PpuMode3LiveBackgroundRegister::Scy)
            && scy_pending_refetch_window
            && write_context.bg_scy_tile_data_row_changed(ly);
        let scy_uses_startup_visible_tile2_tilemap_row = scy_routing
            .startup_visible_tile2_tilemap_row_refetch
            && matches!(
                cached.origin,
                BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
            );
        let scy_startup_visible_tile2_tilemap_phase_matches = matches!(ly & 0x07, 7)
            || scy_routing.startup_visible_tile2_phase6_tilemap_row_refetch
                && matches!(ly & 0x07, 6);
        let scy_tilemap_row_changed = scy_routing.pending_tilemap_row_refetch
            || scy_uses_startup_visible_tile2_tilemap_row
                && scy_startup_visible_tile2_tilemap_phase_matches;
        let scy_tilemap_row_changed = scy_tilemap_row_changed
            && background_cached
            && matches!(register, PpuMode3LiveBackgroundRegister::Scy)
            && scy_pending_refetch_window
            && write_context.bg_scy_tilemap_row_changed(ly);
        let lcdc_tilemap_refetch = background_cached
            && matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bgwin_tilemap_select_changed(cached.source)
            && startup_dummy_pixels == 0
            && (cached.same_cycle_live_tilemap_refetch_window_open
                || cached.is_second_or_third_visible_post_startup_push());
        let lcdc_tiledata_refetch = matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bg_window_tile_data_select_changed()
            && (background_cached || cached.fetch_x != 0);

        Self {
            tilemap_refetch: lcdc_tilemap_refetch
                || background_cached
                    && matches!(register, PpuMode3LiveBackgroundRegister::Scx)
                    && write_context.bg_scx_tilemap_column_changed()
                    && startup_dummy_pixels == 0
                    && (cached.same_cycle_live_tilemap_refetch_window_open
                        || cached.is_second_or_third_visible_post_startup_push())
                || scy_tilemap_row_changed,
            tilemap_full_refetch: background_cached
                && matches!(register, PpuMode3LiveBackgroundRegister::Scx)
                && write_context.bg_scx_tilemap_column_changed()
                && startup_dummy_pixels == 0
                && (cached.same_cycle_live_tilemap_refetch_window_open
                    || cached.is_second_or_third_visible_post_startup_push())
                || scy_tilemap_row_changed,
            tile_data_refetch: lcdc_tiledata_refetch || scy_tile_data_row_changed,
            tile_data_current_row_refetch: scy_tile_data_row_changed
                && !scy_routing.pending_high_plane_only,
            tile_low_current_row_refetch: false,
            tile_high_current_row_refetch: scy_tile_data_row_changed
                && scy_routing.pending_high_plane_only,
            fetcher_tilemap_refetch_on_push: false,
            fetcher_tilemap_full_refetch_on_push: false,
            fetcher_tile_data_refetch_on_push: false,
            fetcher_tile_data_current_row_refetch_on_push: false,
            fetcher_tile_low_current_row_refetch_on_push: false,
            fetcher_tile_high_current_row_refetch_on_push: false,
        }
    }

    pub(in crate::ppu) fn for_visible_fifo_slice(
        cached: BgCachedSlice,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) -> Self {
        Self {
            tilemap_refetch: cached.is_background()
                && write_context.bg_tilemap_select_changed()
                && cached.is_second_or_third_visible_post_startup_push(),
            tilemap_full_refetch: false,
            tile_data_refetch: false,
            tile_data_current_row_refetch: false,
            tile_low_current_row_refetch: false,
            tile_high_current_row_refetch: false,
            fetcher_tilemap_refetch_on_push: false,
            fetcher_tilemap_full_refetch_on_push: false,
            fetcher_tile_data_refetch_on_push: false,
            fetcher_tile_data_current_row_refetch_on_push: false,
            fetcher_tile_low_current_row_refetch_on_push: false,
            fetcher_tile_high_current_row_refetch_on_push: false,
        }
    }

    pub(in crate::ppu) fn for_current_background_fetch(
        fetcher: BgFetcherState,
        register: PpuMode3LiveBackgroundRegister,
        write_context: PpuMode3LiveRegisterWriteContext,
        ly: u8,
        window_tile_row: u8,
        scy_routing: PpuMode3LiveScyWriteRouting,
    ) -> Self {
        let scy_uses_startup_visible_tile2_tilemap_row = scy_routing
            .startup_visible_tile2_tilemap_row_refetch
            && matches!(
                fetcher.cached_origin,
                BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
            );
        let scy_startup_visible_tile2_tilemap_phase_matches = matches!(ly & 0x07, 7)
            || scy_routing.startup_visible_tile2_phase6_tilemap_row_refetch
                && matches!(ly & 0x07, 6);
        let background_coordinate_fetch = fetcher.source == PpuBgFetcherSource::Background
            && (fetcher.stage == PpuBgFetcherStage::TileIndex
                || (scy_routing.pending_tilemap_row_refetch
                    || scy_uses_startup_visible_tile2_tilemap_row
                        && scy_startup_visible_tile2_tilemap_phase_matches)
                    && matches!(
                        fetcher.stage,
                        PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
                    ));
        let background_tile_data_fetch = fetcher.source == PpuBgFetcherSource::Background
            && matches!(
                fetcher.stage,
                PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
            );
        let background_scx_uncommitted_tile_index_fetch = fetcher.source
            == PpuBgFetcherSource::Background
            && (fetcher.stage == PpuBgFetcherStage::TileDataLow
                || fetcher.stage == PpuBgFetcherStage::TileDataHigh && fetcher.stage_dot == 0);
        let background_tile_low_plane_committed = fetcher.source == PpuBgFetcherSource::Background
            && (fetcher.stage == PpuBgFetcherStage::TileDataLow && fetcher.stage_dot >= 1
                || fetcher.stage == PpuBgFetcherStage::TileDataHigh);
        let scy_current_tile_defers = background_tile_low_plane_committed
            && matches!(
                fetcher.cached_origin,
                BgCachedSliceOrigin::Ordinary
                    | BgCachedSliceOrigin::StartupContinuation(
                        BgStartupContinuationSlice::VisibleTile3
                    )
            );
        let scy_defers_current_tile_data =
            scy_current_tile_defers && scy_routing.defers_current_tile_data_fetch_to_next;
        let scy_defers_current_tile_tilemap_row =
            scy_current_tile_defers && scy_routing.defers_current_tile_tilemap_row_to_next;
        let scy_tilemap_row_changed = matches!(register, PpuMode3LiveBackgroundRegister::Scy)
            && background_coordinate_fetch
            && !scy_defers_current_tile_tilemap_row
            && write_context.bg_scy_tilemap_row_changed(ly);
        let scy_tile_data_row_changed = matches!(register, PpuMode3LiveBackgroundRegister::Scy)
            && background_tile_data_fetch
            && !scy_defers_current_tile_data
            && write_context.bg_scy_tile_data_row_changed(ly);
        let window_tile_data_fetch = fetcher.source == PpuBgFetcherSource::Window
            && matches!(
                fetcher.stage,
                PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
            );
        let window_tilemap_changed = matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bgwin_tilemap_select_changed(PpuBgFetcherSource::Window)
            && window_tile_data_fetch;
        let window_tiledata_changed = matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bg_window_tile_data_select_changed()
            && window_tile_data_fetch;
        let window_unsigned_to_signed_tiledata_change = window_tiledata_changed
            && window_tile_row >= 24
            && write_context.previous_lcdc() & LCDC_BG_WINDOW_TILE_DATA_BIT != 0
            && write_context.current_lcdc() & LCDC_BG_WINDOW_TILE_DATA_BIT == 0;

        Self {
            tilemap_refetch: matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
                && write_context.bg_tilemap_select_changed()
                && fetcher.source == PpuBgFetcherSource::Background
                && matches!(
                    fetcher.cached_origin,
                    BgCachedSliceOrigin::StartupContinuation(
                        BgStartupContinuationSlice::VisibleTile3
                    )
                )
                && matches!(
                    fetcher.stage,
                    PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
                )
                || window_tilemap_changed
                || scy_tilemap_row_changed,
            tilemap_full_refetch: false,
            tile_data_refetch: window_tiledata_changed || scy_tile_data_row_changed,
            tile_data_current_row_refetch: scy_tile_data_row_changed,
            tile_low_current_row_refetch: false,
            tile_high_current_row_refetch: false,
            fetcher_tilemap_refetch_on_push: matches!(
                register,
                PpuMode3LiveBackgroundRegister::Lcdc
            ) && write_context.bg_tilemap_select_changed()
                && fetcher.source == PpuBgFetcherSource::Background
                && matches!(
                    fetcher.cached_origin,
                    BgCachedSliceOrigin::StartupContinuation(
                        BgStartupContinuationSlice::VisibleTile3
                    )
                )
                && matches!(
                    fetcher.stage,
                    PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
                )
                || matches!(register, PpuMode3LiveBackgroundRegister::Scx)
                    && write_context.bg_scx_tilemap_column_changed()
                    && background_scx_uncommitted_tile_index_fetch
                || window_tilemap_changed
                || scy_tilemap_row_changed,
            fetcher_tilemap_full_refetch_on_push: matches!(
                register,
                PpuMode3LiveBackgroundRegister::Scx
            ) && write_context
                .bg_scx_tilemap_column_changed()
                && background_scx_uncommitted_tile_index_fetch
                || scy_tilemap_row_changed,
            fetcher_tile_data_refetch_on_push: window_tiledata_changed
                && !window_unsigned_to_signed_tiledata_change
                || scy_tile_data_row_changed,
            fetcher_tile_data_current_row_refetch_on_push: scy_tile_data_row_changed,
            fetcher_tile_low_current_row_refetch_on_push: false,
            fetcher_tile_high_current_row_refetch_on_push: false,
        }
    }

    pub(in crate::ppu) fn apply_to_cached_slice(self, cached: &mut BgCachedSlice) {
        cached.needs_live_tilemap_refetch |= self.tilemap_refetch;
        cached.needs_live_tilemap_full_refetch |= self.tilemap_full_refetch;
        cached.needs_live_tile_data_refetch |= self.tile_data_refetch;
        cached.needs_live_tile_data_current_row_refetch |= self.tile_data_current_row_refetch;
        cached.needs_live_tile_low_current_row_refetch |= self.tile_low_current_row_refetch;
        cached.needs_live_tile_high_current_row_refetch |= self.tile_high_current_row_refetch;
    }

    pub(in crate::ppu) fn apply_to_fetcher(self, fetcher: &mut BgFetcherState) {
        fetcher.needs_live_tilemap_refetch_on_push |= self.fetcher_tilemap_refetch_on_push;
        fetcher.needs_live_tilemap_full_refetch_on_push |=
            self.fetcher_tilemap_full_refetch_on_push;
        fetcher.needs_live_tile_data_refetch_on_push |= self.fetcher_tile_data_refetch_on_push;
        fetcher.needs_live_tile_data_current_row_refetch_on_push |=
            self.fetcher_tile_data_current_row_refetch_on_push;
        fetcher.needs_live_tile_low_current_row_refetch_on_push |=
            self.fetcher_tile_low_current_row_refetch_on_push;
        fetcher.needs_live_tile_high_current_row_refetch_on_push |=
            self.fetcher_tile_high_current_row_refetch_on_push;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) enum PpuMode3ScyObjPhaseOwner {
    PendingHit { match_x: u8 },
    ActiveFetch { sprite_x: u8 },
    CurrentTransferSprite { sprite_x: u8 },
    StartupLineLead { sprite_x: u8 },
}

impl PpuMode3ScyObjPhaseOwner {
    pub(in crate::ppu) const fn x(self) -> u8 {
        match self {
            Self::PendingHit { match_x } => match_x,
            Self::ActiveFetch { sprite_x }
            | Self::CurrentTransferSprite { sprite_x }
            | Self::StartupLineLead { sprite_x } => sprite_x,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3ScyObjPhaseContext {
    pub(in crate::ppu) phase_owner: PpuMode3ScyObjPhaseOwner,
    pub(in crate::ppu) current_transfer_x: u8,
    pub(in crate::ppu) current_transfer: Option<Mode3CurrentTransfer>,
    pub(in crate::ppu) bg_fetcher_stage: PpuBgFetcherStage,
    pub(in crate::ppu) bg_fetcher_stage_dot: u8,
    pub(in crate::ppu) bg_fifo_len: usize,
    pub(in crate::ppu) startup_fifo_placeholders: u8,
    pub(in crate::ppu) obj_fetcher_stage: PpuObjFetcherStage,
    pub(in crate::ppu) obj_fetcher_stage_dot: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3ScyTilemapRetarget {
    pub(in crate::ppu) tilemap_row_delta: i8,
    pub(in crate::ppu) tiledata_row_delta: i8,
}

/// Observed DMG SCY/OBJ startup phase table that can perturb BG refetch routing.
/// These ranges are an explicit hardware hypothesis until an oracle lets the
/// pipeline derive them directly from shared BG/OBJ arbitration state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3ObservedScyObjPhaseTable {
    obj_match_x: u8,
}

impl PpuMode3ObservedScyObjPhaseTable {
    pub(in crate::ppu) const fn new(obj_match_x: u8) -> Self {
        Self { obj_match_x }
    }

    pub(in crate::ppu) const fn obj_match_x(self) -> u8 {
        self.obj_match_x
    }

    pub(in crate::ppu) const fn obj_match_tile_phase(self) -> u8 {
        self.obj_match_x() & (BG_TILE_WIDTH - 1)
    }

    pub(in crate::ppu) const fn pending_refetch_prefers_high_plane_only(self) -> bool {
        matches!(self.obj_match_tile_phase(), 5..=7)
    }

    pub(in crate::ppu) const fn pending_refetch_prefers_tilemap_row(self) -> bool {
        matches!(self.obj_match_tile_phase(), 0..=2)
    }

    pub(in crate::ppu) const fn startup_visible_tile2_refetch_prefers_tilemap_row(self) -> bool {
        matches!(self.obj_match_tile_phase(), 4..=7)
    }

    pub(in crate::ppu) const fn startup_visible_tile2_phase6_refetch_prefers_tilemap_row(
        self,
    ) -> bool {
        matches!(self.obj_match_x(), 4..=7)
    }

    pub(in crate::ppu) const fn startup_visible_tile2_placeholder_uses_previous_tilemap_row(
        self,
        ly: u8,
        visible_x: u8,
    ) -> bool {
        matches!(
            (self.obj_match_x(), ly & (BG_TILE_WIDTH - 1), visible_x),
            (16, 5, 16) | (17, 6, 8)
        )
    }

    pub(in crate::ppu) const fn startup_visible_tile2_tilemap_retarget(
        self,
        ly: u8,
        pixel_index: u8,
    ) -> Option<PpuMode3ScyTilemapRetarget> {
        let (tilemap_row_delta, tiledata_row_delta) =
            match (self.obj_match_x(), ly & (BG_TILE_WIDTH - 1), pixel_index) {
                (9, 6, 4 | 5 | 7) => (1, 0),
                (9, 6, 6) => (1, -1),
                (10, 6, 5..=7) => (-1, 0),
                (11, 7, 5) => (0, 0),
                (16, 6, 0) => (1, 0),
                (16, 6, 7) => (0, -1),
                _ => return None,
            };

        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta,
            tiledata_row_delta,
        })
    }

    pub(in crate::ppu) const fn startup_visible_tile2_uses_previous_tiledata_row(self) -> bool {
        matches!(self.obj_match_x(), 8..=15)
    }

    pub(in crate::ppu) const fn startup_visible_tile3_uses_previous_tiledata_row(self) -> bool {
        matches!(self.obj_match_x(), 16..=17)
    }

    pub(in crate::ppu) const fn startup_alignment_seed_pending_tracks_live_tiledata_row(
        self,
    ) -> bool {
        self.obj_match_x() == 0
    }

    /// Observed CGB-family DMG-software SCY/OBJ startup phase table.
    /// This intentionally does not reuse the DMG startup-alignment FIFO route
    /// wholesale: CGB evidence keeps the write route and the row-retarget seams
    /// as a separate hardware hypothesis.
    pub(in crate::ppu) const fn cgb_dmg_software_live_scy_write_route(
        self,
    ) -> PpuMode3CgbDmgLiveScyWriteRoute {
        let tilemap_row_routing = PpuMode3LiveScyWriteRouting {
            pending_high_plane_only: false,
            pending_tilemap_row_refetch: true,
            startup_visible_tile2_tilemap_row_refetch: false,
            startup_visible_tile2_phase6_tilemap_row_refetch: false,
            defers_current_tile_data_fetch_to_next: true,
            defers_current_tile_tilemap_row_to_next: true,
        };
        let no_special_routing = PpuMode3LiveScyWriteRouting {
            pending_high_plane_only: false,
            pending_tilemap_row_refetch: false,
            startup_visible_tile2_tilemap_row_refetch: false,
            startup_visible_tile2_phase6_tilemap_row_refetch: false,
            defers_current_tile_data_fetch_to_next: false,
            defers_current_tile_tilemap_row_to_next: false,
        };

        match self.obj_match_x() {
            0 | 8 => PpuMode3CgbDmgLiveScyWriteRoute::new(true, false, true, tilemap_row_routing),
            1 | 4..=7 | 10..=16 => {
                PpuMode3CgbDmgLiveScyWriteRoute::new(false, false, true, tilemap_row_routing)
            }
            2 => PpuMode3CgbDmgLiveScyWriteRoute::new(true, false, false, no_special_routing),
            _ => PpuMode3CgbDmgLiveScyWriteRoute::new(false, false, false, no_special_routing),
        }
    }

    pub(in crate::ppu) const fn cgb_dmg_software_startup_visible_tile2_tilemap_retarget(
        self,
        ly: u8,
        pixel_index: u8,
    ) -> Option<PpuMode3ScyTilemapRetarget> {
        let ly_phase = ly & (BG_TILE_WIDTH - 1);
        let (tilemap_row_delta, tiledata_row_delta) =
            match (self.obj_match_x(), ly_phase, pixel_index) {
                (2, 6, 7) | (2, 7, 6) => (0, 0),
                (2, _, _) => (0, -1),
                (3, _, _) => (0, 1),
                (8, 5, 6 | 7) => (0, -1),
                (8, _, _) => (0, -2),
                (9, 7, 4..=7) => (0, 0),
                (9, _, _) => (0, 1),
                (17, 7, 0) => (0, 2),
                (17, _, _) => (0, 1),
                _ => return None,
            };

        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta,
            tiledata_row_delta,
        })
    }

    pub(in crate::ppu) const fn cgb_dmg_software_startup_visible_tile3_tilemap_retarget(
        self,
        _ly: u8,
        _pixel_index: u8,
    ) -> Option<PpuMode3ScyTilemapRetarget> {
        if !matches!(self.obj_match_x(), 16) {
            return None;
        }

        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta: 0,
            tiledata_row_delta: 0,
        })
    }
}

/// Resolves the current SCY/OBJ phase owner and exposes the observed table through
/// an explicit context object, keeping the hypothesis away from mutation code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3ScyObjPhasePolicy {
    context: PpuMode3ScyObjPhaseContext,
}

impl PpuMode3ScyObjPhasePolicy {
    pub(in crate::ppu) const fn new(context: PpuMode3ScyObjPhaseContext) -> Self {
        Self { context }
    }

    pub(in crate::ppu) const fn phase_owner(self) -> PpuMode3ScyObjPhaseOwner {
        self.context.phase_owner
    }

    pub(in crate::ppu) const fn observed_phase_table(self) -> PpuMode3ObservedScyObjPhaseTable {
        PpuMode3ObservedScyObjPhaseTable::new(self.phase_owner().x())
    }

    pub(in crate::ppu) const fn pending_refetch_prefers_high_plane_only(self) -> bool {
        self.observed_phase_table()
            .pending_refetch_prefers_high_plane_only()
    }

    pub(in crate::ppu) const fn pending_refetch_prefers_tilemap_row(self) -> bool {
        self.observed_phase_table()
            .pending_refetch_prefers_tilemap_row()
    }

    pub(in crate::ppu) const fn startup_visible_tile2_refetch_prefers_tilemap_row(self) -> bool {
        self.observed_phase_table()
            .startup_visible_tile2_refetch_prefers_tilemap_row()
    }

    pub(in crate::ppu) const fn startup_visible_tile2_phase6_refetch_prefers_tilemap_row(
        self,
    ) -> bool {
        self.observed_phase_table()
            .startup_visible_tile2_phase6_refetch_prefers_tilemap_row()
    }

    pub(in crate::ppu) const fn startup_visible_tile2_placeholder_uses_previous_tilemap_row(
        self,
        ly: u8,
        visible_x: u8,
    ) -> bool {
        self.observed_phase_table()
            .startup_visible_tile2_placeholder_uses_previous_tilemap_row(ly, visible_x)
    }

    pub(in crate::ppu) const fn startup_visible_tile2_tilemap_retarget(
        self,
        ly: u8,
        pixel_index: u8,
    ) -> Option<PpuMode3ScyTilemapRetarget> {
        self.observed_phase_table()
            .startup_visible_tile2_tilemap_retarget(ly, pixel_index)
    }

    pub(in crate::ppu) const fn startup_visible_tile2_uses_previous_tiledata_row(self) -> bool {
        self.observed_phase_table()
            .startup_visible_tile2_uses_previous_tiledata_row()
    }

    pub(in crate::ppu) const fn startup_visible_tile3_uses_previous_tiledata_row(self) -> bool {
        self.observed_phase_table()
            .startup_visible_tile3_uses_previous_tiledata_row()
    }

    pub(in crate::ppu) const fn startup_alignment_seed_pending_tracks_live_tiledata_row(
        self,
    ) -> bool {
        self.observed_phase_table()
            .startup_alignment_seed_pending_tracks_live_tiledata_row()
    }

    pub(in crate::ppu) const fn cgb_dmg_software_live_scy_write_route(
        self,
    ) -> PpuMode3CgbDmgLiveScyWriteRoute {
        self.observed_phase_table()
            .cgb_dmg_software_live_scy_write_route()
    }

    pub(in crate::ppu) const fn cgb_dmg_software_startup_visible_tile2_tilemap_retarget(
        self,
        ly: u8,
        pixel_index: u8,
    ) -> Option<PpuMode3ScyTilemapRetarget> {
        self.observed_phase_table()
            .cgb_dmg_software_startup_visible_tile2_tilemap_retarget(ly, pixel_index)
    }

    pub(in crate::ppu) const fn cgb_dmg_software_startup_visible_tile3_tilemap_retarget(
        self,
        ly: u8,
        pixel_index: u8,
    ) -> Option<PpuMode3ScyTilemapRetarget> {
        self.observed_phase_table()
            .cgb_dmg_software_startup_visible_tile3_tilemap_retarget(ly, pixel_index)
    }
}

/// Resolves sprite-phased DMG Mode 3 live-write quirks through explicit
/// observed tables, keeping imperative control code focused on applying
/// the already-decided hardware hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3SingleSpritePhasePolicy {
    sprite_x: u8,
}

impl PpuMode3SingleSpritePhasePolicy {
    pub(in crate::ppu) const fn new(sprite_x: u8) -> Self {
        Self { sprite_x }
    }

    pub(in crate::ppu) const fn sprite_x(self) -> u8 {
        self.sprite_x
    }

    pub(in crate::ppu) const fn observed_lcdc0_onset_table(
        self,
    ) -> PpuMode3ObservedLcdc0OnsetTable {
        PpuMode3ObservedLcdc0OnsetTable::new(self.sprite_x())
    }

    pub(in crate::ppu) const fn observed_lcdc1_disable_onset_visible_x(self) -> Option<u8> {
        const ONSETS: [u8; 16] = [0, 0, 0, 2, 3, 4, 4, 4, 3, 4, 5, 6, 7, 8, 8, 8];
        let sprite_x = self.sprite_x() as usize;
        if sprite_x < ONSETS.len() {
            Some(ONSETS[sprite_x])
        } else {
            None
        }
    }

    pub(in crate::ppu) const fn cgb_dmg_software_lcdc1_disable_onset_visible_x(self) -> Option<u8> {
        const ONSETS: [u8; 16] = [0, 1, 2, 3, 4, 5, 5, 5, 4, 5, 6, 7, 8, 9, 9, 9];
        let sprite_x = self.sprite_x() as usize;
        if sprite_x < ONSETS.len() {
            Some(ONSETS[sprite_x])
        } else {
            None
        }
    }

    pub(in crate::ppu) const fn observed_lcdc3_phase_table(
        self,
    ) -> PpuMode3ObservedLcdc3PhaseTable {
        PpuMode3ObservedLcdc3PhaseTable::new(self.sprite_x())
    }

    pub(in crate::ppu) const fn observed_lcdc4_phase_table(
        self,
    ) -> PpuMode3ObservedLcdc4PhaseTable {
        PpuMode3ObservedLcdc4PhaseTable::new(self.sprite_x())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3ObservedLcdc0OnsetTable {
    sprite_x: u8,
}

impl PpuMode3ObservedLcdc0OnsetTable {
    pub(in crate::ppu) const fn new(sprite_x: u8) -> Self {
        Self { sprite_x }
    }

    pub(in crate::ppu) const fn onset_visible_x(self, write_index: usize) -> Option<u8> {
        const WRITE0_ONSETS: [u8; 18] = [0, 0, 0, 2, 3, 4, 4, 4, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        const WRITE1_ONSETS: [u8; 18] = [
            11, 12, 13, 14, 15, 16, 16, 16, 11, 12, 13, 14, 15, 16, 16, 16, 11, 12,
        ];

        let sprite_x = self.sprite_x as usize;
        match write_index {
            0 if sprite_x < WRITE0_ONSETS.len() => Some(WRITE0_ONSETS[sprite_x]),
            1 if sprite_x < WRITE1_ONSETS.len() => Some(WRITE1_ONSETS[sprite_x]),
            2 if sprite_x < WRITE1_ONSETS.len() => Some(WRITE1_ONSETS[sprite_x] + 8),
            3 if sprite_x < WRITE1_ONSETS.len() => Some(WRITE1_ONSETS[sprite_x] + 16),
            _ => None,
        }
    }

    pub(in crate::ppu) const fn cgb_dmg_software_onset_visible_x(
        self,
        write_index: usize,
    ) -> Option<u8> {
        const WRITE0_ONSETS: [u8; 18] = [0, 1, 2, 3, 4, 5, 5, 5, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        const WRITE1_ONSETS: [u8; 18] = [
            12, 13, 14, 15, 16, 17, 17, 17, 12, 13, 14, 15, 16, 17, 17, 17, 12, 13,
        ];

        let sprite_x = self.sprite_x as usize;
        match write_index {
            0 if sprite_x < WRITE0_ONSETS.len() => Some(WRITE0_ONSETS[sprite_x]),
            1 if sprite_x < WRITE1_ONSETS.len() => Some(WRITE1_ONSETS[sprite_x]),
            2 if sprite_x < WRITE1_ONSETS.len() => Some(WRITE1_ONSETS[sprite_x] + 8),
            3 if sprite_x < WRITE1_ONSETS.len() => Some(WRITE1_ONSETS[sprite_x] + 16),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3Lcdc3StartupTilemapOverride {
    pub(in crate::ppu) tilemap_select: bool,
    pub(in crate::ppu) applies_to_visible_tile2: bool,
    pub(in crate::ppu) applies_to_visible_tile3: bool,
}

impl PpuMode3Lcdc3StartupTilemapOverride {
    pub(in crate::ppu) const fn has_effect(self) -> bool {
        self.applies_to_visible_tile2 || self.applies_to_visible_tile3
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3Lcdc3LiveWriteDecision {
    pub(in crate::ppu) clear_visible_tile2_live_refetch: bool,
    pub(in crate::ppu) tilemap_override: Option<PpuMode3Lcdc3StartupTilemapOverride>,
}

impl PpuMode3Lcdc3LiveWriteDecision {
    pub(in crate::ppu) const fn has_effect(self) -> bool {
        self.clear_visible_tile2_live_refetch || self.tilemap_override.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3ObservedLcdc3PhaseTable {
    sprite_x: u8,
}

impl PpuMode3ObservedLcdc3PhaseTable {
    pub(in crate::ppu) const fn new(sprite_x: u8) -> Self {
        Self { sprite_x }
    }

    pub(in crate::ppu) const fn live_write_decision(
        self,
        write_index: usize,
        current_bg_tilemap_select: bool,
    ) -> Option<PpuMode3Lcdc3LiveWriteDecision> {
        let clear_visible_tile2_live_refetch = matches!(
            (write_index, self.sprite_x),
            (0, 3..=17) | (1, 16..=u8::MAX)
        );

        let tilemap_override = match write_index {
            0 if current_bg_tilemap_select => {
                let override_decision = PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: self.sprite_x <= 2,
                    applies_to_visible_tile3: matches!(self.sprite_x & 0x07, 3..=7),
                };
                if override_decision.has_effect() {
                    Some(override_decision)
                } else {
                    None
                }
            }
            1 if self.sprite_x <= 2 => Some(PpuMode3Lcdc3StartupTilemapOverride {
                tilemap_select: true,
                applies_to_visible_tile2: true,
                applies_to_visible_tile3: false,
            }),
            _ => None,
        };

        let decision = PpuMode3Lcdc3LiveWriteDecision {
            clear_visible_tile2_live_refetch,
            tilemap_override,
        };
        if decision.has_effect() {
            Some(decision)
        } else {
            None
        }
    }

    pub(in crate::ppu) const fn cgb_dmg_software_live_write_decision(
        self,
        write_index: usize,
        current_bg_tilemap_select: bool,
    ) -> Option<PpuMode3Lcdc3LiveWriteDecision> {
        let clear_visible_tile2_live_refetch = matches!(
            (write_index, self.sprite_x),
            (0, 1..=17) | (1, 1..=2 | 16..=u8::MAX)
        );

        let tilemap_override = match write_index {
            0 if current_bg_tilemap_select => {
                let override_decision = PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: self.sprite_x == 0,
                    applies_to_visible_tile3: matches!(self.sprite_x, 1..=7 | 9..=15),
                };
                if override_decision.has_effect() {
                    Some(override_decision)
                } else {
                    None
                }
            }
            1 if self.sprite_x == 0 => Some(PpuMode3Lcdc3StartupTilemapOverride {
                tilemap_select: true,
                applies_to_visible_tile2: true,
                applies_to_visible_tile3: false,
            }),
            1 if matches!(self.sprite_x, 1..=2) => Some(PpuMode3Lcdc3StartupTilemapOverride {
                tilemap_select: true,
                applies_to_visible_tile2: false,
                applies_to_visible_tile3: true,
            }),
            _ => None,
        };

        let decision = PpuMode3Lcdc3LiveWriteDecision {
            clear_visible_tile2_live_refetch,
            tilemap_override,
        };
        if decision.has_effect() {
            Some(decision)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3Lcdc4StartupOverride {
    pub(in crate::ppu) slice: BgVisibleStartupSlice,
    pub(in crate::ppu) override_select: BgTileDataSelectOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3ObservedLcdc4PhaseTable {
    sprite_x: u8,
}

impl PpuMode3ObservedLcdc4PhaseTable {
    pub(in crate::ppu) const fn new(sprite_x: u8) -> Self {
        Self { sprite_x }
    }

    pub(in crate::ppu) const fn startup_override_for_target_select(
        self,
        target_select: BgTileDataSelect,
    ) -> Option<PpuMode3Lcdc4StartupOverride> {
        let (slice, override_select) = match (target_select, self.sprite_x) {
            (BgTileDataSelect::Unsigned8000, 3 | 4) => (
                BgVisibleStartupSlice::VisibleTile2,
                PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            ),
            (BgTileDataSelect::Unsigned8000, 5..=7) => (
                BgVisibleStartupSlice::VisibleTile2,
                PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            ),
            (BgTileDataSelect::Unsigned8000, 8..=17) => (
                BgVisibleStartupSlice::VisibleTile2,
                PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Signed8800),
                ),
            ),
            (BgTileDataSelect::Signed8800, 2..=4 | 8..=12) => (
                BgVisibleStartupSlice::VisibleTile3,
                PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Signed8800),
                ),
            ),
            (BgTileDataSelect::Signed8800, 5..=7 | 13..=15) => (
                BgVisibleStartupSlice::VisibleTile3,
                PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Signed8800),
                ),
            ),
            (BgTileDataSelect::Signed8800, 16..=17) => (
                BgVisibleStartupSlice::VisibleTile3,
                PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            ),
            _ => return None,
        };

        Some(PpuMode3Lcdc4StartupOverride {
            slice,
            override_select,
        })
    }

    /// Observed CGB-family DMG-software LCDC.4 startup phase table.
    ///
    /// Compatibility-mode CGB keeps the DMG software-visible contract for the
    /// startup BG slices, but its CGB pixel pipeline does not line up with the
    /// monochrome DMG LCDC.4 phase table one-to-one. Keep the table separate
    /// from the DMG one so later revision-specific evidence can refine it
    /// without weakening either model.
    pub(in crate::ppu) const fn cgb_dmg_software_startup_override_for_target_select(
        self,
        target_select: BgTileDataSelect,
        ly: u8,
    ) -> Option<PpuMode3Lcdc4StartupOverride> {
        let (slice, override_select) = match (target_select, self.sprite_x) {
            (BgTileDataSelect::Unsigned8000, 3 | 4) => (
                BgVisibleStartupSlice::VisibleTile2,
                PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            ),
            (BgTileDataSelect::Unsigned8000, 5..=17) => (
                BgVisibleStartupSlice::VisibleTile2,
                PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Signed8800),
                ),
            ),
            (BgTileDataSelect::Signed8800, 1..=3 | 8..=11) => (
                BgVisibleStartupSlice::VisibleTile3,
                PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Signed8800),
                ),
            ),
            (BgTileDataSelect::Signed8800, 4..=7 | 12..=15) => (
                BgVisibleStartupSlice::VisibleTile3,
                PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Signed8800),
                ),
            ),
            (BgTileDataSelect::Signed8800, 16) if ly & 0x07 == 0 => (
                BgVisibleStartupSlice::VisibleTile3,
                PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            ),
            (BgTileDataSelect::Signed8800, 16 | 17) => (
                BgVisibleStartupSlice::VisibleTile3,
                PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            ),
            _ => return None,
        };

        Some(PpuMode3Lcdc4StartupOverride {
            slice,
            override_select,
        })
    }
}

/// Observed DMG LCDC.2 16->8 Mode 3 seam for the curated Mealybug OBJ-size
/// pulses. The remaining hardware-visible difference is confined to which
/// sprite bitplanes keep the line-start 8x16 interpretation across the active
/// shrink window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) enum PpuMode3Lcdc2ObjSizePlaneSelection {
    Live8,
    Live8LowLineStart16High,
    LineStart16LowLive8High,
    LineStart16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) enum PpuMode3Lcdc2ObjSizeObservedEffect {
    RetroactiveRepaint { background_only: bool },
    FifoRewrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3Lcdc2ObjSizeObservedDecision {
    pub(in crate::ppu) plane_selection: PpuMode3Lcdc2ObjSizePlaneSelection,
    pub(in crate::ppu) pending_effect: Option<PpuMode3Lcdc2ObjSizeObservedEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum PpuMode3Lcdc2ObjSizeObservedProfile {
    DmgFamily,
    CgbDmgSoftware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuMode3ObservedLcdc2ObjSizePhaseTable {
    sprite_x: u8,
    scx: u8,
    raw_row: u8,
    profile: PpuMode3Lcdc2ObjSizeObservedProfile,
}

impl PpuMode3ObservedLcdc2ObjSizePhaseTable {
    pub(in crate::ppu) const fn new(sprite_x: u8, scx: u8, raw_row: u8) -> Self {
        Self {
            sprite_x,
            scx: scx & 0x07,
            raw_row,
            profile: PpuMode3Lcdc2ObjSizeObservedProfile::DmgFamily,
        }
    }

    pub(in crate::ppu) const fn cgb_dmg_software(sprite_x: u8, scx: u8, raw_row: u8) -> Self {
        Self {
            sprite_x,
            scx: scx & 0x07,
            raw_row,
            profile: PpuMode3Lcdc2ObjSizeObservedProfile::CgbDmgSoftware,
        }
    }

    pub(in crate::ppu) fn decision(
        self,
        write_index: usize,
        active_write_visible_x: Option<u8>,
    ) -> Option<PpuMode3Lcdc2ObjSizeObservedDecision> {
        let plane_selection = self.plane_selection(write_index, active_write_visible_x)?;
        let pending_effect = match (write_index, self.sprite_x, self.scx) {
            (0, 12, 4..=7) if self.raw_row >= 8 => {
                Some(PpuMode3Lcdc2ObjSizeObservedEffect::RetroactiveRepaint {
                    background_only: false,
                })
            }
            (2, 32, 0)
                if matches!(self.raw_row, 4..=7)
                    && active_write_visible_x
                        .is_some_and(|visible_x| i16::from(visible_x) > self.sprite_screen_x()) =>
            {
                Some(PpuMode3Lcdc2ObjSizeObservedEffect::FifoRewrite)
            }
            _ => None,
        };

        Some(PpuMode3Lcdc2ObjSizeObservedDecision {
            plane_selection,
            pending_effect,
        })
    }

    pub(in crate::ppu) fn plane_selection(
        self,
        write_index: usize,
        active_write_visible_x: Option<u8>,
    ) -> Option<PpuMode3Lcdc2ObjSizePlaneSelection> {
        if let PpuMode3Lcdc2ObjSizeObservedProfile::CgbDmgSoftware = self.profile
            && let Some(selection) =
                self.cgb_dmg_software_plane_selection(write_index, active_write_visible_x)
        {
            return Some(selection);
        }

        self.dmg_family_plane_selection(write_index, active_write_visible_x)
    }

    fn dmg_family_plane_selection(
        self,
        write_index: usize,
        active_write_visible_x: Option<u8>,
    ) -> Option<PpuMode3Lcdc2ObjSizePlaneSelection> {
        match (write_index, self.sprite_x, self.scx) {
            (0, 12, 4..=7) if self.raw_row >= 8 => Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
            (0, 32, 0 | 4..=7) if self.raw_row < 8 => {
                Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8)
            }
            (0, 32, 1..=2) => Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
            (2, 32, 0)
                if matches!(self.raw_row, 4..=7)
                    && active_write_visible_x
                        .is_some_and(|visible_x| i16::from(visible_x) > self.sprite_screen_x()) =>
            {
                Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16LowLive8High)
            }
            (2, 32, 0)
                if active_write_visible_x
                    .is_some_and(|visible_x| i16::from(visible_x) <= self.sprite_screen_x()) =>
            {
                Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16)
            }
            (2, 34..=39, 0) => Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
            _ => self.base_plane_selection(write_index),
        }
    }

    const fn base_plane_selection(
        self,
        write_index: usize,
    ) -> Option<PpuMode3Lcdc2ObjSizePlaneSelection> {
        match (write_index, self.sprite_x) {
            (0, 8) => Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
            (0, 16) => Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High),
            (0, 24) => Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
            (2, 33) => Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High),
            (2, 40) => Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
            _ => None,
        }
    }

    fn cgb_dmg_software_plane_selection(
        self,
        write_index: usize,
        active_write_visible_x: Option<u8>,
    ) -> Option<PpuMode3Lcdc2ObjSizePlaneSelection> {
        match (write_index, self.sprite_x, self.scx) {
            (0, 16, _) => Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
            (2, 33, _) => Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
            (0, 32, 0) if matches!(self.raw_row, 2..=7) && active_write_visible_x == Some(10) => {
                Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16)
            }
            (0, 32, 5..=7) if matches!(self.raw_row, 4..=7) => {
                Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16LowLive8High)
            }
            (2, 32, 0) if matches!(self.raw_row, 4..=7) => {
                Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High)
            }
            _ => None,
        }
    }

    const fn sprite_screen_x(self) -> i16 {
        self.sprite_x as i16 - 8
    }
}
