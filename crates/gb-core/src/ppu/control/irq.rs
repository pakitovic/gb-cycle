use super::*;

impl Ppu {
    pub(in crate::ppu) fn prepare_visible_scanline_state(&mut self) {
        if self.line_dot != 1 || self.ly >= VISIBLE_SCANLINES {
            return;
        }

        let window_lcdc5_latch = self.window_activation_registers().window_enabled();
        let prepared_line = self.mode3_window_policy().prepare_line(
            self.ly,
            self.runtime.window_state.wy_triggered,
            self.runtime.window_state.pending_wx166_next_line,
        );
        self.runtime.window_state.wy_triggered = prepared_line.wy_triggered();
        self.runtime.window_state.pending_wx166_next_line = false;
        self.runtime.bg_pipeline_state.prepare_window_line(
            prepared_line.wy_latch(),
            window_lcdc5_latch,
            prepared_line.force_x0_this_line(),
        );
    }

    pub(in crate::ppu) fn live_lyc_coincidence(&self) -> bool {
        self.live_ly_for_lyc_compare()
            .is_some_and(|compare_ly| compare_ly == self.lyc)
    }

    pub(in crate::ppu) fn live_ly_for_lyc_compare(&self) -> Option<u8> {
        if self.console_model.is_cgb_family() && self.ly == TOTAL_SCANLINES - 1 {
            return match self.line_dot {
                CGB_LINE_153_LYC153_COMPARE_START_DOT..CGB_LINE_153_LYC153_COMPARE_END_DOT => {
                    Some(TOTAL_SCANLINES - 1)
                }
                CGB_LINE_153_LYC0_COMPARE_START_DOT.. => {
                    if self.lyc_compare_blanked_at_line_end() {
                        None
                    } else {
                        Some(0)
                    }
                }
                _ => None,
            };
        }

        if self.ly == TOTAL_SCANLINES - 1 {
            return match self.line_dot {
                LINE_153_LYC153_COMPARE_START_DOT..LINE_153_LYC153_COMPARE_END_DOT => {
                    Some(TOTAL_SCANLINES - 1)
                }
                LINE_153_LYC0_COMPARE_START_DOT.. => Some(0),
                _ => None,
            };
        }

        if self.lyc_compare_blanked_at_line_end() {
            return None;
        }

        if self.ly == 0 {
            return Some(0);
        }

        if self.line_dot == 0 {
            return None;
        }

        Some(self.ly)
    }

    fn lyc_compare_blanked_at_line_end(&self) -> bool {
        self.console_model.is_cgb_family()
            && self.line_dot + CGB_LINE_END_LYC_COMPARE_BLANK_DOTS >= self.current_scanline_length()
    }

    pub(in crate::ppu) fn update_lyc_compare_latch(&mut self) {
        if let Some(compare_ly) = self.live_ly_for_lyc_compare() {
            self.runtime.stat_state.lyc_compare_latch = compare_ly == self.lyc;
        }
    }

    pub(in crate::ppu) fn lyc_coincidence_for_irq_line(&self) -> bool {
        if !self.is_lcd_enabled() {
            return self.runtime.stat_state.lcd_disabled_lyc_coincidence;
        }

        match self.live_ly_for_lyc_compare() {
            Some(compare_ly) => compare_ly == self.lyc,
            None => {
                // L2-a.1: under the CPU-first reorder the CPU observes the raster one
                // dot ahead, so the dot0 LYC coincidence edge is seen one read-position
                // too early. Defer the regular-line dot0 edge to line_dot 1 (latch only)
                // so the CPU-observable LYC STAT IRQ matches hardware (wilbertpol ly_lyc).
                self.runtime.stat_state.lyc_compare_latch
            }
        }
    }

    fn regular_line_dot0_compare_window(&self) -> bool {
        self.line_dot == 0 && self.ly != TOTAL_SCANLINES - 1
    }

    pub(in crate::ppu) fn lyc_coincidence_for_readback(&self) -> bool {
        if !self.is_lcd_enabled() {
            return self.runtime.stat_state.lcd_disabled_lyc_coincidence;
        }

        match self.live_ly_for_lyc_compare() {
            Some(compare_ly) => compare_ly == self.lyc,
            None => self.console_model.is_cgb_family() && self.runtime.stat_state.lyc_compare_latch,
        }
    }

    // Capture the line-153 LYC0 pretrigger window membership at the END of the tick, as
    // a 1-T-cycle delayed register (DocBoy last_* family, ppu.cpp:554-560). Under the
    // CPU-first reorder the CPU observes the raster one dot ahead, so the line-153 LYC0
    // STAT IRQ edge is otherwise seen one read-position too early; reading the delayed
    // membership defers that edge by one dot, exactly as E1 defers the regular-line dot0
    // edge and item (3) defers the VBlank IF edge (wilbertpol ly_lyc_0 / ly_lyc_0_write).
    // INTERIM: the full observation-tables-seam deletion is coupled to the item-2 raster
    // rephasing + the write-vs-tick re-eval model; see §24.17.
    pub(in crate::ppu) fn capture_delayed_lyc_state(&mut self) {
        self.runtime.stat_state.last_line_153_lyc0_pretrigger_window =
            self.line_153_lyc0_pretrigger_window_live();
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
            && self.lyc_coincidence_for_irq_line()
            && !self.dmg_stat_write_quirk_blocks_line153_lyc0_stat_source();
        let line_153_lyc0_pretrigger_source = self.line_153_lyc0_stat_irq_pretrigger_source();

        if !self.is_lcd_enabled() {
            return coincidence_source || self.lcd_enable_pending_lyc_rise_source();
        }

        let mode_interrupt_enable = stat_interrupt_enable
            & (STAT_MODE0_INTERRUPT_ENABLE_BIT
                | STAT_MODE1_INTERRUPT_ENABLE_BIT
                | STAT_MODE2_INTERRUPT_ENABLE_BIT)
            != 0;
        if !mode_interrupt_enable {
            return coincidence_source || line_153_lyc0_pretrigger_source;
        }

        let mode0_start_dot = self.current_mode0_start_dot();
        let real_boot_scx_seam_suppresses_pretrigger = self
            .runtime
            .stat_state
            .real_boot_handoff_mode0_scx_seam_phase_active
            && matches!(self.scx & 0x07, 3 | 7);
        let mode0_pretrigger_source = stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0
            && !self
                .runtime
                .stat_state
                .suppress_mode0_pretrigger_until_vblank
            && !self.runtime.stat_state.startup_mode0_irq_phase_active
            && !real_boot_scx_seam_suppresses_pretrigger
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot < mode0_start_dot
            && self.line_dot + 4 >= mode0_start_dot;
        let mode2_pretrigger_source = self.ordinary_mode2_stat_pretrigger_source();
        let mode2_vblank_entry_source = self.mode2_vblank_entry_stat_source();
        let mode_source = match self.current_stat_irq_access_mode() {
            PpuAccessMode::HBlank => stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0,
            PpuAccessMode::VBlank => stat_interrupt_enable & STAT_MODE1_INTERRUPT_ENABLE_BIT != 0,
            PpuAccessMode::OamScan => stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0,
            PpuAccessMode::Drawing => false,
        };

        coincidence_source
            || line_153_lyc0_pretrigger_source
            || mode_source
            || mode0_pretrigger_source
            || mode2_pretrigger_source
            || mode2_vblank_entry_source
    }

    fn current_stat_irq_access_mode(&self) -> PpuAccessMode {
        let lcd_restart_first_line = self
            .lcd_restart_phase
            .is_first_line_after_enable_active(self.ly);
        if lcd_restart_first_line
            && (self.line_dot < LCD_REENABLE_LINE0_MODE3_START_DOT
                || self.line_dot < self.lcd_reenable_line0_mode0_irq_dot())
        {
            return PpuAccessMode::Drawing;
        }

        if !lcd_restart_first_line
            && self.runtime.stat_state.startup_mode0_irq_phase_active
            && self.current_access_mode() == PpuAccessMode::HBlank
            && self.line_dot < self.current_mode0_stat_irq_start_dot()
        {
            return PpuAccessMode::Drawing;
        }

        if !lcd_restart_first_line
            && self
                .runtime
                .stat_state
                .suppress_mode0_pretrigger_until_vblank
            && self.current_access_mode() == PpuAccessMode::HBlank
            && self.line_dot < self.current_mode0_stat_irq_start_dot()
        {
            return PpuAccessMode::Drawing;
        }

        self.current_access_mode()
    }

    fn current_mode0_stat_irq_start_dot(&self) -> u16 {
        let mode0_start_dot = self.current_mode0_start_dot();
        if self.runtime.stat_state.startup_mode0_irq_phase_active {
            let startup_scx_seam_delay = if matches!(self.scx & 0x07, 3 | 7) {
                64
            } else {
                60
            };
            return mode0_start_dot.saturating_add(startup_scx_seam_delay);
        }

        if !self
            .runtime
            .stat_state
            .suppress_mode0_pretrigger_until_vblank
        {
            return mode0_start_dot;
        }

        let scx_seam = match self.scx & 0x07 {
            3 => 4,
            7 => 1,
            _ => 0,
        };
        mode0_start_dot.saturating_add(scx_seam + 1)
    }

    fn lcd_reenable_line0_mode0_irq_dot(&self) -> u16 {
        let scx_group_delay = u16::from((self.scx & 0x07).saturating_add(3) / 4) * 4;
        LCD_REENABLE_LINE0_MODE0_RESTORE_DOT + scx_group_delay + 1
    }

    fn lcd_reenable_line0_mode0_halt_wake_dot(&self) -> u16 {
        (self.current_mode0_start_dot().saturating_sub(3) & !0x0003) + 1
    }

    pub(crate) fn dmg_lcd_reenable_mode0_halt_wake_deferred(&self) -> bool {
        if !self.console_model.is_dmg_family()
            || self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT == 0
            || !self.is_lcd_enabled()
            || !self
                .lcd_restart_phase
                .is_first_line_after_enable_active(self.ly)
            || self.current_stat_irq_access_mode() != PpuAccessMode::HBlank
        {
            return false;
        }

        let irq_dot = self.lcd_reenable_line0_mode0_irq_dot();
        let halt_wake_dot = self.lcd_reenable_line0_mode0_halt_wake_dot().max(irq_dot);

        self.line_dot >= irq_dot && self.line_dot < halt_wake_dot
    }

    fn ordinary_mode2_stat_pretrigger_lead_dots(&self) -> u16 {
        if self.runtime.blank_frame_active {
            3
        } else {
            4
        }
    }

    fn ordinary_mode2_stat_pretrigger_source(&self) -> bool {
        self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.is_lcd_enabled()
            && self.ly + 1 < VISIBLE_SCANLINES
            && self.line_dot + self.ordinary_mode2_stat_pretrigger_lead_dots()
                >= self.current_scanline_length()
    }

    fn mode2_vblank_entry_stat_pretrigger_dots(&self) -> Option<u16> {
        if self.console_model.is_dmg_family() {
            return Some(DMG_MODE2_VBLANK_ENTRY_STAT_PRETRIGGER_DOTS);
        }

        if self.console_model.is_cgb_family() && self.operating_mode.uses_dmg_software_contract() {
            return Some(CGB_COMPAT_MODE2_VBLANK_ENTRY_STAT_PRETRIGGER_DOTS);
        }

        None
    }

    fn mode2_vblank_entry_stat_source(&self) -> bool {
        let Some(mut pretrigger_dots) = self.mode2_vblank_entry_stat_pretrigger_dots() else {
            return false;
        };

        // The blank frame after an LCD re-enable shifts the mode2 STAT pretrigger one dot
        // later, exactly like `ordinary_mode2_stat_pretrigger_lead_dots` (4->3); the
        // vblank-entry source needs the same adjustment (wilbertpol intr_2_timing round5).
        if self.runtime.blank_frame_active {
            pretrigger_dots -= 1;
        }

        self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.is_lcd_enabled()
            && self.ly + 1 == VISIBLE_SCANLINES
            && self.current_access_mode() == PpuAccessMode::HBlank
            && self.line_dot + pretrigger_dots == self.current_scanline_length()
    }

    fn line_153_lyc0_pretrigger_window_live(&self) -> bool {
        if self.ly != TOTAL_SCANLINES - 1 {
            return false;
        }

        if self.console_model.is_cgb_family() {
            (CGB_LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT..CGB_LINE_153_LYC0_COMPARE_START_DOT)
                .contains(&self.line_dot)
        } else {
            (LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT..LINE_153_LYC0_COMPARE_START_DOT)
                .contains(&self.line_dot)
        }
    }

    fn line_153_lyc0_stat_irq_pretrigger_source(&self) -> bool {
        if self.stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT == 0
            || !self.is_lcd_enabled()
            || self.ly != TOTAL_SCANLINES - 1
            || self.lyc != 0
        {
            return false;
        }

        if !self.console_model.is_cgb_family() && self.dmg_stat_write_quirk_blocks_line153_lyc0() {
            return false;
        }

        // The line-153 LYC0 STAT IRQ edge is observed one read-position too early under
        // the CPU-first reorder; defer it one dot via the same 1-T-cycle register delay
        // as E1's dot0 latch (see capture_delayed_lyc_state). Subsumes the ad-hoc
        // pretrigger-window edge skew without re-deriving the window constants.
        self.runtime.stat_state.last_line_153_lyc0_pretrigger_window
    }

    fn dmg_stat_write_quirk_blocks_line153_lyc0_stat_source(&self) -> bool {
        self.dmg_stat_write_quirk_blocks_line153_lyc0()
            && self.ly == TOTAL_SCANLINES - 1
            && self.lyc == 0
            && self.line_dot >= LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT
    }

    fn dmg_stat_write_quirk_blocks_line153_lyc0(&self) -> bool {
        self.console_model.is_dmg_family()
            && self
                .runtime
                .stat_state
                .dmg_stat_write_quirk_blocks_line153_lyc0
    }

    pub(in crate::ppu) fn mode2_stat_write_irq_source(&self) -> bool {
        self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.is_lcd_enabled()
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot == 0
            && self.current_access_mode() == PpuAccessMode::OamScan
    }

    pub(in crate::ppu) fn mode1_stat_write_irq_source(&self) -> bool {
        self.stat_interrupt_enable & STAT_MODE1_INTERRUPT_ENABLE_BIT != 0
            && self.is_lcd_enabled()
            && self.current_access_mode() == PpuAccessMode::VBlank
    }

    pub(in crate::ppu) fn mode0_stat_write_irq_source(&self) -> bool {
        self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0
            && self.is_lcd_enabled()
            && self.ly < VISIBLE_SCANLINES
            && self.current_stat_irq_access_mode() == PpuAccessMode::HBlank
    }

    pub(in crate::ppu) fn lyc_stat_write_irq_source(&self) -> bool {
        self.stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT != 0
            && self.is_lcd_enabled()
            && !self.regular_line_dot0_compare_window()
            && self.lyc_coincidence_for_irq_line()
            && !self.dmg_stat_write_quirk_blocks_line153_lyc0_stat_source()
    }

    pub(crate) fn dmg_mode2_oam_halt_wake_deferred(&self) -> bool {
        self.console_model.is_dmg_family()
            && self.runtime.blank_frame_active
            && self.ordinary_mode2_stat_pretrigger_source()
    }

    pub(crate) fn dmg_mode2_vblank_entry_halt_wake_deferred(&self) -> bool {
        self.console_model.is_dmg_family()
            && self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.is_lcd_enabled()
            && self.ly + 1 == VISIBLE_SCANLINES
            && self.current_access_mode() == PpuAccessMode::HBlank
            && self.line_dot + 4 >= self.current_scanline_length()
    }

    pub(crate) fn mode0_hblank_halt_wake_deferred(&self) -> bool {
        (self.console_model.is_cgb_family() || matches!(self.scx & 0x07, 1 | 2 | 5 | 6))
            && self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0
            && self.is_lcd_enabled()
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot < self.current_mode0_start_dot()
            && self.line_dot + 4 >= self.current_mode0_start_dot()
    }

    pub(crate) fn dmg_mode2_vblank_entry_interrupt_service_deferred(&self) -> bool {
        self.console_model.is_dmg_family()
            && self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.is_lcd_enabled()
            && self.ly + 1 == VISIBLE_SCANLINES
            && self.current_access_mode() == PpuAccessMode::HBlank
            && self.line_dot + 4 >= self.current_scanline_length()
    }

    pub(in crate::ppu) fn compute_stat_irq_line(&self, quirk_active: bool) -> bool {
        self.ordinary_stat_irq_line() || quirk_active
    }

    pub(in crate::ppu) fn refresh_stat_irq_line(&mut self, quirk_active: bool) {
        if !quirk_active && self.stat_interrupt_enable == 0 && !self.runtime.stat_state.irq_line {
            return;
        }

        let new_line = self.compute_stat_irq_line(quirk_active);
        if self.line_dot != 0 {
            self.runtime.stat_state.dot0_lyc_stat_irq_edge_pending = false;
        }
        let line_153_lyc0_pretrigger_request = !self.runtime.stat_state.irq_line
            && new_line
            && self.line_153_lyc0_stat_irq_pretrigger_source();
        if !self.runtime.stat_state.irq_line && new_line {
            self.queue_interrupt_request_with_cpu_if_visibility(InterruptSource::LcdStat, true);
            if self.dot0_lyc_stat_irq_edge_is_cancellable() {
                self.runtime.stat_state.dot0_lyc_stat_irq_edge_pending = true;
            }
        }
        if line_153_lyc0_pretrigger_request {
            self.runtime
                .stat_state
                .line_153_lyc0_stat_irq_pretrigger_pending = true;
        }
        self.runtime.stat_state.irq_line = new_line;
    }

    fn dot0_lyc_stat_irq_edge_is_cancellable(&self) -> bool {
        self.regular_line_dot0_compare_window()
            && self.ly == self.lyc
            && self.stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT != 0
    }

    pub(in crate::ppu) fn cancel_obsolete_dot0_lyc_stat_irq_edge(&mut self) -> bool {
        if !self.runtime.stat_state.dot0_lyc_stat_irq_edge_pending
            || self.lyc_coincidence_for_irq_line()
        {
            return false;
        }

        self.runtime.pending_interrupts &= !PPU_PENDING_LCD_STAT_INTERRUPT_BIT;
        self.runtime.pending_interrupts_hidden_from_cpu_if &= !PPU_PENDING_LCD_STAT_INTERRUPT_BIT;
        self.runtime.stat_state.dot0_lyc_stat_irq_edge_pending = false;
        true
    }

    pub(in crate::ppu) fn cancel_obsolete_line_153_lyc0_stat_irq_pretrigger(&mut self) -> bool {
        if !self
            .runtime
            .stat_state
            .line_153_lyc0_stat_irq_pretrigger_pending
            || self.line_153_lyc0_stat_irq_pretrigger_source()
        {
            return false;
        }

        self.runtime.pending_interrupts &= !PPU_PENDING_LCD_STAT_INTERRUPT_BIT;
        self.runtime.pending_interrupts_hidden_from_cpu_if &= !PPU_PENDING_LCD_STAT_INTERRUPT_BIT;
        self.runtime
            .stat_state
            .line_153_lyc0_stat_irq_pretrigger_pending = false;
        true
    }

    #[cfg(test)]
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

    pub(in crate::ppu) fn stat_write_quirk_active_for_write(&self) -> bool {
        if !self.console_model.is_dmg_family() || !self.is_lcd_enabled() {
            return false;
        }

        if self.stat_write_quirk_vblank_window_active() || self.lyc_coincidence_for_irq_line() {
            return true;
        }

        let lcd_restart_first_line = self
            .lcd_restart_phase
            .is_first_line_after_enable_active(self.ly);

        (!lcd_restart_first_line
            && (self.stat_write_quirk_line0_oam_window_active()
                || self.stat_write_quirk_oam_start_window_active()))
            || self.stat_write_quirk_hblank_window_active()
    }

    fn stat_write_quirk_vblank_window_active(&self) -> bool {
        self.ly >= VISIBLE_SCANLINES
    }

    fn stat_write_quirk_line0_oam_window_active(&self) -> bool {
        self.ly == 0 && self.line_dot < MODE2_DOTS
    }

    fn stat_write_quirk_oam_start_window_active(&self) -> bool {
        self.ly < VISIBLE_SCANLINES && self.line_dot == 0
    }

    fn stat_write_quirk_hblank_window_active(&self) -> bool {
        let mode0_quirk_start_dot = self.current_mode0_start_dot().saturating_add(4);

        self.ly < VISIBLE_SCANLINES
            && self.line_dot >= mode0_quirk_start_dot
            && self.line_dot < self.current_scanline_length()
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
        let retained_lyc_coincidence = self.lyc_coincidence_for_irq_line();
        self.lcd_state = PpuLcdState::Disabled;
        self.lcd_enable_pending_delay_tcycles = 0;
        self.runtime.blank_frame_active = false;
        self.runtime.stat_state.lcd_disabled_lyc_coincidence = retained_lyc_coincidence;
        self.ly = 0;
        self.line_dot = 0;
        self.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        self.runtime
            .stat_state
            .suppress_mode0_pretrigger_until_vblank = false;
        self.runtime.stat_state.startup_mode0_irq_phase_active = false;
        self.runtime
            .stat_state
            .real_boot_handoff_mode0_scx_seam_phase_active = false;
        self.runtime.stat_state.vblank_wrap_line0_stat_delay_active = false;
        self.runtime.stat_state.skip_boot_ly_read_lag_active = false;
        self.runtime.stat_state.boot_power_on_ppu_phase_active = false;
        self.runtime.stat_state.boot_power_on_ppu_phase_base_dot = 0;
        self.runtime
            .stat_state
            .boot_power_on_ppu_phase_extends_until_vblank = false;
        self.runtime
            .stat_state
            .line_153_lyc0_stat_irq_pretrigger_pending = false;
        self.runtime
            .stat_state
            .dmg_stat_write_quirk_blocks_line153_lyc0 = false;
        self.dmg_real_boot_power_on_lcd_enable_phase_active = false;
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
        self.line_dot = if self.dmg_real_boot_power_on_lcd_enable_phase_active {
            DMG_REAL_BOOT_POWER_ON_LCD_ENABLE_INITIAL_LINE_DOT
        } else {
            LCD_REENABLE_INITIAL_LINE_DOT
        };
        self.runtime
            .stat_state
            .real_boot_handoff_mode0_scx_seam_phase_active =
            self.dmg_real_boot_power_on_lcd_enable_phase_active;
        self.dmg_real_boot_power_on_lcd_enable_phase_active = false;
        self.lcd_restart_phase = PpuLcdRestartPhase::first_line_after_enable();
        self.runtime.stat_state.lcd_disabled_lyc_coincidence = false;
        self.runtime
            .stat_state
            .suppress_mode0_pretrigger_until_vblank = true;
        self.runtime.stat_state.startup_mode0_irq_phase_active = false;
        self.runtime.stat_state.vblank_wrap_line0_stat_delay_active = false;
        self.runtime.stat_state.skip_boot_ly_read_lag_active = false;
        self.runtime.stat_state.boot_power_on_ppu_phase_active = false;
        self.runtime.stat_state.boot_power_on_ppu_phase_base_dot = 0;
        self.runtime
            .stat_state
            .boot_power_on_ppu_phase_extends_until_vblank = false;
        self.runtime
            .stat_state
            .line_153_lyc0_stat_irq_pretrigger_pending = false;
        self.runtime
            .stat_state
            .dmg_stat_write_quirk_blocks_line153_lyc0 = false;
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
