use super::*;

impl Ppu {
    pub(in crate::ppu) fn current_published_stat_access_mode(&self) -> PpuAccessMode {
        let Some(context) = self.current_published_stat_mode_context() else {
            return self.published_stat_mode_at_line_start();
        };

        self.resolve_published_stat_access_mode(context)
    }

    fn current_published_stat_mode_context(&self) -> Option<PpuPublishedStatModeContext> {
        let published_line_dot = self.current_published_stat_line_dot()?;

        Some(PpuPublishedStatModeContext {
            published_mode: self.access_mode_for_line_dot(published_line_dot),
            current_mode: self.access_mode_for_line_dot(self.line_dot),
            sprite_extended_mode3: self.current_mode0_start_dot() > self.baseline_mode0_start_dot(),
        })
    }

    fn current_published_stat_line_dot(&self) -> Option<u16> {
        if self.vblank_wrap_line0_stat_readback_delay_active() {
            return self
                .line_dot
                .checked_sub(LINE0_VBLANK_WRAP_STAT_READBACK_DELAY_DOTS);
        }

        self.line_dot.checked_sub(1)
    }

    fn vblank_wrap_line0_stat_readback_delay_active(&self) -> bool {
        self.runtime.stat_state.vblank_wrap_line0_stat_delay_active && self.ly == 0
    }

    fn published_stat_mode_at_line_start(&self) -> PpuAccessMode {
        if self.ly > VISIBLE_SCANLINES
            || (self.console_model.is_cgb_family()
                && self.vblank_wrap_line0_stat_readback_delay_active())
        {
            PpuAccessMode::VBlank
        } else {
            PpuAccessMode::HBlank
        }
    }

    fn resolve_published_stat_access_mode(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> PpuAccessMode {
        if self.published_stat_mode2_to_mode3_override_applies(context) {
            return PpuAccessMode::Drawing;
        }

        if self.published_stat_early_hblank_override_applies(context) {
            return PpuAccessMode::HBlank;
        }

        if let Some(mode) = self.published_stat_terminal_boundary_override(context) {
            return mode;
        }

        context.published_mode
    }

    fn published_stat_mode2_to_mode3_override_applies(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> bool {
        context.published_mode == PpuAccessMode::OamScan
            && context.current_mode == PpuAccessMode::Drawing
            && !self.vblank_wrap_line0_stat_readback_delay_active()
            && !self.runtime.blank_frame_active
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot == MODE2_DOTS
    }

    fn published_stat_early_hblank_override_applies(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> bool {
        if context.published_mode != PpuAccessMode::Drawing {
            return false;
        }

        self.terminal_visible_tail_should_publish_hblank_early()
    }

    fn no_unfetched_sprite_can_still_match(&self) -> bool {
        if !self.obj_enabled() {
            return true;
        }

        let current_transfer_x = self.runtime.bg_pipeline_state.current_transfer_x;
        (0..self.runtime.mode2_scan_state.selected_sprite_count()).all(|slot| {
            if self.runtime.obj_pipeline_state.has_fetched(slot) {
                return true;
            }
            let Some(sprite) = self.runtime.mode2_scan_state.selected_sprite(slot) else {
                return true;
            };
            match sprite_trigger_x(sprite) {
                Some(trigger_x) => trigger_x < current_transfer_x,
                None => true,
            }
        })
    }

    pub(in crate::ppu) fn terminal_visible_tail_should_publish_hblank_early(&self) -> bool {
        if self.dmg_wx0_scx3_window_tail_should_keep_published_drawing() {
            return false;
        }

        let early_dots: u16 = if self.scx & 0x07 == 0 { 3 } else { 1 };
        self.ly < VISIBLE_SCANLINES
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.no_unfetched_sprite_can_still_match()
            && self.line_dot + early_dots >= self.current_mode0_start_dot()
    }

    fn published_stat_terminal_boundary_override(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> Option<PpuAccessMode> {
        if context.published_mode == PpuAccessMode::Drawing
            && context.current_mode == PpuAccessMode::HBlank
            && !self.vblank_wrap_line0_stat_readback_delay_active()
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot == self.current_mode0_start_dot()
        {
            return Some(PpuAccessMode::HBlank);
        }

        None
    }

    fn dmg_wx0_scx3_window_tail_should_keep_published_drawing(&self) -> bool {
        let visible_registers = self.mode3_register_latches().visible();

        self.console_model.is_dmg_family()
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot + 1 == self.current_mode0_start_dot()
            && self.runtime.mode2_scan_state.selected_sprite_count() == 0
            && self.runtime.bg_pipeline_state.window_started_this_line
            && self.runtime.bg_pipeline_state.fetcher.source == PpuBgFetcherSource::Window
            && visible_registers.window_enabled()
            && visible_registers.wx == 0
            && visible_registers.scx & 0x07 == 3
    }

    pub(in crate::ppu) fn current_published_oam_write_access_mode(&self) -> PpuAccessMode {
        if let Some(mode) = self.dmg_boot_power_on_bus_access_mode() {
            return mode;
        }

        let published_mode = self.current_published_video_write_access_mode();
        self.current_published_oam_write_access_mode_from(published_mode)
    }

    pub(in crate::ppu) fn current_published_oam_write_access_mode_from(
        &self,
        published_mode: PpuAccessMode,
    ) -> PpuAccessMode {
        if published_mode == PpuAccessMode::OamScan
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot == MODE2_DOTS
        {
            PpuAccessMode::HBlank
        } else {
            published_mode
        }
    }

    pub(in crate::ppu) fn current_published_oam_read_access_mode(&self) -> PpuAccessMode {
        if let Some(mode) = self.dmg_boot_power_on_bus_access_mode() {
            return mode;
        }

        let published_mode = self.current_published_bus_access_mode();
        self.current_published_oam_read_access_mode_from(published_mode)
    }

    pub(in crate::ppu) fn current_published_oam_read_access_mode_from(
        &self,
        published_mode: PpuAccessMode,
    ) -> PpuAccessMode {
        if published_mode == PpuAccessMode::Drawing
            && !self.runtime.blank_frame_active
            && self.ly < VISIBLE_SCANLINES
            && ((self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::HBlank
                && self.line_dot == self.current_mode0_start_dot())
                || (self.line_dot != 0
                    && self.access_mode_for_line_dot(self.line_dot - 1) == PpuAccessMode::OamScan
                    && self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::Drawing
                    && self.line_dot == MODE2_DOTS))
        {
            PpuAccessMode::HBlank
        } else {
            published_mode
        }
    }
}
