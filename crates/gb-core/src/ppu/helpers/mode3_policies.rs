use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ppu) struct PpuMode3TransferPolicy {
    mode3_started: bool,
    startup_source_state: Mode3StartupSourceState,
    startup_pre_visible_transfer_dots_remaining: u8,
    current_transfer_x: u8,
    visible_pixels_output: u8,
    scx_discard_remaining: u8,
    line_dot: u16,
}

impl PpuMode3TransferPolicy {
    pub(in crate::ppu) const fn new(
        mode3_started: bool,
        startup_source_state: Mode3StartupSourceState,
        startup_pre_visible_transfer_dots_remaining: u8,
        current_transfer_x: u8,
        visible_pixels_output: u8,
        scx_discard_remaining: u8,
        line_dot: u16,
    ) -> Self {
        Self {
            mode3_started,
            startup_source_state,
            startup_pre_visible_transfer_dots_remaining,
            current_transfer_x,
            visible_pixels_output,
            scx_discard_remaining,
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

    pub(in crate::ppu) fn current_transfer(
        self,
        real_bg_fifo_empty: bool,
        effective_bg_fifo_empty: bool,
    ) -> Option<Mode3CurrentTransfer> {
        let context = self.current_transfer_context()?;
        let plan = self.transfer_service_plan(context)?;
        let readiness = if plan.requires_real_bg_fifo_pixel() {
            if real_bg_fifo_empty {
                Mode3TransferReadiness::WaitingForFifo(plan)
            } else {
                Mode3TransferReadiness::Ready(plan)
            }
        } else if plan.requires_effective_bg_fifo_pixel() && effective_bg_fifo_empty {
            Mode3TransferReadiness::WaitingForFifo(plan)
        } else {
            Mode3TransferReadiness::Ready(plan)
        };

        Some(Mode3CurrentTransfer { context, readiness })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3LineTimingPolicy {
    visible_registers: PpuVisibleRegisters,
    mode3_started: bool,
    mode0_start_dot: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3LineTimingContext {
    pub(in crate::ppu) line_dot: u16,
    pub(in crate::ppu) selected_sprite_count: u8,
    pub(in crate::ppu) all_selected_sprites_offscreen_right: bool,
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
            let shortens_for_all_offscreen_right_sprites = self.mode0_start_dot
                == self.baseline_mode0_start_dot()
                && self.visible_registers.obj_enabled()
                && context.selected_sprite_count > 0
                && context.all_selected_sprites_offscreen_right;
            self.mode0_start_dot
                .saturating_sub(u16::from(shortens_for_all_offscreen_right_sprites))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3WindowPolicy {
    visible_registers: PpuVisibleRegisters,
    activation: PpuMode3WindowActivationState,
    wy_latch: bool,
    started_this_line: bool,
}

impl PpuMode3WindowPolicy {
    pub(in crate::ppu) const fn new(
        visible_registers: PpuVisibleRegisters,
        activation: PpuMode3WindowActivationState,
        wy_latch: bool,
        started_this_line: bool,
    ) -> Self {
        Self {
            visible_registers,
            activation,
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
        self.visible_registers.window_enabled()
    }

    pub(in crate::ppu) fn can_apply_wx0_shortening(
        self,
        transfer_dot: Mode3TransferDot,
        visible_pixels_output: u8,
        current_transfer_x: u8,
        initial_scx_discard: u8,
        scx_discard_remaining: u8,
    ) -> bool {
        transfer_dot.consumed_scx_discard
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
        scx_discard_remaining: u8,
        wx166_armed_this_line: bool,
    ) -> PpuMode3WindowStartDecision {
        if !transfer_dot.is_served()
            || self.started_this_line
            || !self.wy_latch
            || !self.activation.runtime_enabled()
        {
            return PpuMode3WindowStartDecision::NotReady;
        }

        if self.activation.is_wx_166() {
            if visible_pixels_output as usize == SCREEN_WIDTH
                && scx_discard_remaining == 0
                && !wx166_armed_this_line
            {
                return PpuMode3WindowStartDecision::ArmWx166NextLine;
            }
            return PpuMode3WindowStartDecision::NotReady;
        }

        let Some(trigger_x) = self.activation.trigger_x() else {
            return PpuMode3WindowStartDecision::NotReady;
        };

        if visible_pixels_output != trigger_x {
            return PpuMode3WindowStartDecision::NotReady;
        }

        let can_start_now = if trigger_x == 0 {
            scx_discard_remaining == 0
                && current_transfer_x >= 8
                && transfer_dot.can_start_window_after_x0_service()
        } else {
            scx_discard_remaining == 0
                && transfer_dot.kind == Mode3TransferDotKind::ServedVisiblePixel
        };

        if can_start_now {
            PpuMode3WindowStartDecision::StartNow
        } else {
            PpuMode3WindowStartDecision::NotReady
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3BgWinFetchPolicy {
    register_latches: PpuMode3RegisterLatches,
    console_model: ConsoleModel,
    background_tilemap_uses_pipeline_snapshot: bool,
    background_tiledata_uses_pipeline_snapshot: bool,
    background_tileindex_reads_on_stage_one: bool,
}

impl PpuMode3BgWinFetchPolicy {
    pub(in crate::ppu) const fn new(
        register_latches: PpuMode3RegisterLatches,
        console_model: ConsoleModel,
        background_tilemap_uses_pipeline_snapshot: bool,
        background_tiledata_uses_pipeline_snapshot: bool,
        background_tileindex_reads_on_stage_one: bool,
    ) -> Self {
        Self {
            register_latches,
            console_model,
            background_tilemap_uses_pipeline_snapshot,
            background_tiledata_uses_pipeline_snapshot,
            background_tileindex_reads_on_stage_one,
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
            self.register_latches.window_fetch_registers(),
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
        self.console_model.is_dmg_family() && self.register_latches.lcdc_bit_changed(map_bit)
    }

    pub(in crate::ppu) fn should_delay_background_tileindex_read(
        self,
        source: PpuBgFetcherSource,
    ) -> bool {
        source == PpuBgFetcherSource::Background && self.background_tileindex_reads_on_stage_one
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(in crate::ppu) struct PpuMode3LiveBackgroundWriteEffects {
    tilemap_refetch: bool,
    tile_data_refetch: bool,
    tile_data_current_row_refetch: bool,
    fetcher_tilemap_refetch_on_push: bool,
}

impl PpuMode3LiveBackgroundWriteEffects {
    pub(in crate::ppu) fn for_push_pending_slice(
        cached: BgCachedSlice,
        register: PpuMode3LiveBackgroundRegister,
        write_context: PpuMode3LiveRegisterWriteContext,
        entry_delay_active: bool,
    ) -> Self {
        if !cached.is_background() || cached.is_startup_alignment_seed() {
            return Self::default();
        }

        Self {
            tilemap_refetch: matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
                && write_context.bg_tilemap_select_changed()
                && (entry_delay_active
                    || cached.same_cycle_live_tilemap_refetch_window_open
                    || cached.is_second_or_third_visible_post_startup_push()),
            tile_data_refetch: matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
                && write_context.bg_window_tile_data_select_changed(),
            tile_data_current_row_refetch: false,
            fetcher_tilemap_refetch_on_push: false,
        }
    }

    pub(in crate::ppu) fn for_fill_pending_slice(
        cached: BgCachedSlice,
        register: PpuMode3LiveBackgroundRegister,
        write_context: PpuMode3LiveRegisterWriteContext,
        includes_real_tile_pixels: bool,
        startup_dummy_pixels: u8,
    ) -> Self {
        if !cached.is_background() || !includes_real_tile_pixels {
            return Self::default();
        }

        Self {
            tilemap_refetch: matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
                && write_context.bg_tilemap_select_changed()
                && startup_dummy_pixels == 0
                && (cached.same_cycle_live_tilemap_refetch_window_open
                    || cached.is_second_or_third_visible_post_startup_push()),
            tile_data_refetch: matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
                && write_context.bg_window_tile_data_select_changed(),
            tile_data_current_row_refetch: false,
            fetcher_tilemap_refetch_on_push: false,
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
            tile_data_refetch: false,
            tile_data_current_row_refetch: false,
            fetcher_tilemap_refetch_on_push: false,
        }
    }

    pub(in crate::ppu) fn for_current_background_fetch(
        fetcher: BgFetcherState,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) -> Self {
        Self {
            tilemap_refetch: false,
            tile_data_refetch: false,
            tile_data_current_row_refetch: false,
            fetcher_tilemap_refetch_on_push: fetcher.source == PpuBgFetcherSource::Background
                && write_context.bg_tilemap_select_changed()
                && matches!(
                    fetcher.cached_origin,
                    BgCachedSliceOrigin::StartupContinuation(
                        BgStartupContinuationSlice::VisibleTile3
                    )
                )
                && matches!(
                    fetcher.stage,
                    PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
                ),
        }
    }

    pub(in crate::ppu) fn apply_to_cached_slice(self, cached: &mut BgCachedSlice) {
        cached.needs_live_tilemap_refetch |= self.tilemap_refetch;
        cached.needs_live_tile_data_refetch |= self.tile_data_refetch;
        cached.needs_live_tile_data_current_row_refetch |= self.tile_data_current_row_refetch;
    }

    pub(in crate::ppu) fn apply_to_fetcher(self, fetcher: &mut BgFetcherState) {
        fetcher.needs_live_tilemap_refetch_on_push |= self.fetcher_tilemap_refetch_on_push;
    }
}
