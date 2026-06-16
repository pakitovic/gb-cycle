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

        if self.published_stat_steady_frame_mode0_boundary_override_applies(context) {
            return PpuAccessMode::HBlank;
        }

        context.published_mode
    }

    fn published_stat_steady_frame_mode0_boundary_override_applies(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> bool {
        if self.vblank_wrap_line0_stat_readback_delay_active()
            || self.runtime.blank_frame_active
            || self.ly >= VISIBLE_SCANLINES
            || !(self.scx == 0 || self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0)
        {
            return false;
        }

        let mode0_start_dot = self.current_mode0_start_dot();

        if context.published_mode == PpuAccessMode::Drawing
            && context.current_mode == PpuAccessMode::HBlank
            && self.line_dot == mode0_start_dot
        {
            return true;
        }

        // The CPU micro-op observes the pre-tick `line_dot`, so the Drawing→HBlank
        // boundary must publish one dot earlier to match the same-cycle CPU read, exactly
        // like the OamScan→Drawing override above (wilbertpol/mooneye intr_2_mode0_timing).
        self.line_dot + 1 == mode0_start_dot
            && self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::Drawing
            && self.access_mode_for_line_dot(self.line_dot + 1) == PpuAccessMode::HBlank
    }

    fn published_stat_mode2_to_mode3_override_applies(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> bool {
        if context.published_mode != PpuAccessMode::OamScan
            || self.vblank_wrap_line0_stat_readback_delay_active()
            || self.runtime.blank_frame_active
            || self.ly >= VISIBLE_SCANLINES
        {
            return false;
        }

        if context.current_mode == PpuAccessMode::Drawing && self.line_dot == MODE2_DOTS {
            return true;
        }

        // The CPU micro-op observes the pre-tick `line_dot`, so the OamScan→Drawing
        // boundary must publish one dot earlier to match the same-cycle CPU read.
        self.line_dot == MODE2_DOTS - 1
            && self.access_mode_for_line_dot(self.line_dot + 1) == PpuAccessMode::Drawing
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
