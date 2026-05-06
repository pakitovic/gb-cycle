use super::*;

impl Ppu {
    pub(in crate::ppu) fn prepare_visible_scanline_state(&mut self) {
        if self.line_dot != 1 || self.ly >= VISIBLE_SCANLINES {
            return;
        }

        let prepared_line = self.mode3_window_policy().prepare_line(
            self.ly,
            self.runtime.window_state.wy_triggered,
            self.runtime.window_state.pending_wx166_next_line,
        );
        self.runtime.window_state.wy_triggered = prepared_line.wy_triggered();
        self.runtime.window_state.pending_wx166_next_line = false;
        self.runtime
            .bg_pipeline_state
            .prepare_window_line(prepared_line.wy_latch(), prepared_line.force_x0_this_line());
    }

    pub(in crate::ppu) fn live_lyc_coincidence(&self) -> bool {
        self.live_ly_for_lyc_compare() == self.lyc
    }

    pub(in crate::ppu) fn live_ly_for_lyc_compare(&self) -> u8 {
        if self.ly == TOTAL_SCANLINES - 1 && self.line_dot >= LINE_153_LY0_DOT {
            0
        } else {
            self.ly
        }
    }

    pub(in crate::ppu) fn effective_lyc_coincidence(&self) -> bool {
        if self.is_lcd_enabled() {
            self.live_lyc_coincidence()
        } else {
            self.runtime.stat_state.lcd_disabled_lyc_coincidence
        }
    }

    pub(in crate::ppu) fn lcd_enable_pending_lyc_rise_source(&self) -> bool {
        self.lcd_enable_pending_delay_tcycles == 2
            && self.stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT != 0
            && !self.runtime.stat_state.lcd_disabled_lyc_coincidence
            && self.live_lyc_coincidence()
    }

    pub(in crate::ppu) fn ordinary_stat_irq_line(&self) -> bool {
        let stat_interrupt_enable = self.stat_interrupt_enable;
        if stat_interrupt_enable == 0 {
            return false;
        }

        let coincidence_source = stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT != 0
            && self.effective_lyc_coincidence();

        if !self.is_lcd_enabled() {
            return coincidence_source || self.lcd_enable_pending_lyc_rise_source();
        }

        let mode_interrupt_enable = stat_interrupt_enable
            & (STAT_MODE0_INTERRUPT_ENABLE_BIT
                | STAT_MODE1_INTERRUPT_ENABLE_BIT
                | STAT_MODE2_INTERRUPT_ENABLE_BIT)
            != 0;
        if !mode_interrupt_enable {
            return coincidence_source;
        }

        let mode0_start_dot = self.current_mode0_start_dot();
        let mode0_pretrigger_source = stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot < mode0_start_dot
            && self.line_dot + 4 >= mode0_start_dot;
        let mode2_pretrigger_source = stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.ly + 1 < VISIBLE_SCANLINES
            && self.line_dot + 4 >= self.current_scanline_length();
        let dmg_mode2_vblank_entry_source = self.console_model.is_dmg_family()
            && stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.current_access_mode() == PpuAccessMode::VBlank
            && self.ly == VISIBLE_SCANLINES
            && self.line_dot == 0;
        let mode_source = match self.current_access_mode() {
            PpuAccessMode::HBlank => stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0,
            PpuAccessMode::VBlank => stat_interrupt_enable & STAT_MODE1_INTERRUPT_ENABLE_BIT != 0,
            PpuAccessMode::OamScan => stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0,
            PpuAccessMode::Drawing => false,
        };

        coincidence_source
            || mode_source
            || mode0_pretrigger_source
            || mode2_pretrigger_source
            || dmg_mode2_vblank_entry_source
    }

    pub(in crate::ppu) fn compute_stat_irq_line(&self, quirk_active: bool) -> bool {
        self.ordinary_stat_irq_line() || quirk_active
    }

    pub(in crate::ppu) fn refresh_stat_irq_line(&mut self, quirk_active: bool) {
        let new_line = self.compute_stat_irq_line(quirk_active);
        if !self.runtime.stat_state.irq_line && new_line {
            self.queue_interrupt_request_with_cpu_if_visibility(
                InterruptSource::LcdStat,
                !self.stat_request_hidden_from_same_cycle_cpu_if(),
            );
        }
        self.runtime.stat_state.irq_line = new_line;
    }

    pub(in crate::ppu) fn queue_interrupt_request(&mut self, source: InterruptSource) {
        self.queue_interrupt_request_with_cpu_if_visibility(source, true);
    }

    pub(in crate::ppu) fn queue_interrupt_request_with_cpu_if_visibility(
        &mut self,
        source: InterruptSource,
        cpu_if_visible: bool,
    ) {
        let bit = match source {
            InterruptSource::VBlank => PPU_PENDING_VBLANK_INTERRUPT_BIT,
            InterruptSource::LcdStat => PPU_PENDING_LCD_STAT_INTERRUPT_BIT,
            InterruptSource::Timer | InterruptSource::Serial | InterruptSource::Joypad => {
                return;
            }
        };
        self.runtime.pending_interrupts |= bit;
        if cpu_if_visible {
            self.runtime.pending_interrupts_hidden_from_cpu_if &= !bit;
        } else {
            self.runtime.pending_interrupts_hidden_from_cpu_if |= bit;
        }
    }

    pub(in crate::ppu) fn stat_request_hidden_from_same_cycle_cpu_if(&self) -> bool {
        self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self
                .lcd_restart_phase
                .is_first_line_after_enable_active(self.ly)
            && self.ly + 1 < VISIBLE_SCANLINES
            && self.line_dot + 4 >= self.current_scanline_length()
    }

    pub(in crate::ppu) fn stat_write_quirk_active(&self) -> bool {
        self.console_model.is_dmg_family()
            && self.is_lcd_enabled()
            && (matches!(
                self.current_access_mode(),
                PpuAccessMode::HBlank | PpuAccessMode::VBlank | PpuAccessMode::OamScan
            ) || self.live_lyc_coincidence())
    }

    pub(in crate::ppu) fn refresh_visible_output(&mut self) {
        let system_stop_forces_blank =
            self.runtime.system_stop_active && !self.cgb_stop_preserves_mode3_output();
        self.runtime.panel.visible_output = if self.is_lcd_enabled()
            && !self.runtime.blank_frame_active
            && !system_stop_forces_blank
        {
            PpuVisibleOutputState::Driving
        } else {
            PpuVisibleOutputState::ForcedBlank
        };
    }

    pub(in crate::ppu) fn advance_lcd_restart_phase(&mut self) {
        self.lcd_restart_phase = self.lcd_restart_phase.advance(self.ly, self.line_dot);
    }

    pub(in crate::ppu) fn enter_lcd_disabled_state(&mut self) {
        self.lcd_state = PpuLcdState::Disabled;
        self.lcd_enable_pending_delay_tcycles = 0;
        self.runtime.blank_frame_active = false;
        self.runtime.stat_state.lcd_disabled_lyc_coincidence = self.live_lyc_coincidence();
        self.ly = 0;
        self.line_dot = 0;
        self.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        self.runtime.reset_runtime_pipeline_state();
        self.reload_mode3_register_latches_from_mmio();
        self.runtime.panel.clear_visible_buffers();
        self.refresh_visible_output();
    }

    pub(in crate::ppu) fn enter_lcd_enabled_restart_state(&mut self) {
        self.lcd_state = PpuLcdState::Enabled;
        self.lcd_enable_pending_delay_tcycles = 0;
        self.runtime.blank_frame_active = true;
        self.ly = 0;
        self.line_dot = LCD_REENABLE_INITIAL_LINE_DOT;
        self.lcd_restart_phase = PpuLcdRestartPhase::first_line_after_enable();
        self.runtime.stat_state.lcd_disabled_lyc_coincidence = false;
        self.runtime.reset_runtime_pipeline_state();
        self.reload_mode3_register_latches_from_mmio();
        self.runtime.panel.clear_visible_buffers();
        self.refresh_visible_output();
    }

    pub(in crate::ppu) fn enter_lcd_enable_pending_state(&mut self, delay_tcycles: u8) {
        self.lcd_state = PpuLcdState::Disabled;
        self.lcd_enable_pending_delay_tcycles = delay_tcycles;
        self.runtime.startup_mode_latch = None;
        self.refresh_visible_output();
    }
}
