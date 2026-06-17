use super::*;

impl Ppu {
    pub(in crate::ppu) fn current_window_line_counter(&self) -> u8 {
        if self.runtime.bg_pipeline_state.window_started_this_line {
            self.runtime.bg_pipeline_state.window_active_line_counter
        } else {
            self.runtime.window_state.window_line_counter
        }
    }

    pub(in crate::ppu) const fn current_mmio_visible_registers(&self) -> PpuVisibleRegisters {
        PpuVisibleRegisters {
            lcdc: self.lcdc,
            scy: self.scy,
            scx: self.scx,
            bgp: self.bgp,
            obp0: self.obp0,
            obp1: self.obp1,
            wy: self.wy,
            wx: self.wx,
        }
    }

    pub(in crate::ppu) fn mode3_register_latches(&self) -> PpuMode3RegisterLatches {
        PpuMode3RegisterLatches::new(
            self.runtime.visible_registers,
            self.runtime.pipeline_registers,
        )
    }

    pub(in crate::ppu) const fn current_mode3_live_register_write_context(
        &self,
        previous_mmio_registers: PpuVisibleRegisters,
    ) -> PpuMode3LiveRegisterWriteContext {
        PpuMode3LiveRegisterWriteContext::new(
            previous_mmio_registers,
            self.current_mmio_visible_registers(),
        )
    }

    pub(in crate::ppu) fn current_mode3_live_background_refetch_context(
        &self,
    ) -> PpuMode3LiveBackgroundRefetchContext {
        PpuMode3LiveBackgroundRefetchContext::new(
            self.current_mmio_visible_registers(),
            self.ly,
            self.current_window_line_counter(),
            self.runtime.last_unsigned_tile_data_low_fetch,
            self.runtime.last_unsigned_tile_data_high_fetch,
        )
    }

    pub(in crate::ppu) fn set_mode3_register_latches(&mut self, latches: PpuMode3RegisterLatches) {
        self.runtime.visible_registers = latches.visible();
        self.runtime.pipeline_registers = latches.pipeline();
    }

    pub(in crate::ppu) fn reload_mode3_register_latches_from_mmio(&mut self) {
        self.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(
            self.current_mmio_visible_registers(),
        ));
    }

    pub(in crate::ppu) fn advance_mode3_register_latches_from_mmio(&mut self) {
        self.set_mode3_register_latches(
            self.mode3_register_latches()
                .advance(self.current_mmio_visible_registers()),
        );
    }

    pub(in crate::ppu) fn is_lcd_enabled(&self) -> bool {
        self.lcd_state.is_enabled()
    }

    #[cfg(test)]
    pub(in crate::ppu) fn sync_visible_registers(&mut self) {
        self.runtime.visible_registers = self.current_mmio_visible_registers();
    }

    #[cfg(test)]
    pub(in crate::ppu) fn sync_pipeline_registers(&mut self) {
        self.runtime.pipeline_registers = self.runtime.visible_registers;
    }

    pub(in crate::ppu) fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    pub(in crate::ppu) fn write_lcdc(&mut self, value: u8, source: PpuRegisterWriteSource) {
        let was_lcd_enabled = self.is_lcd_enabled() || self.lcd_enable_pending_delay_tcycles != 0;
        self.lcdc = value;
        self.runtime.startup_mode_latch = None;

        match (was_lcd_enabled, value & LCDC_ENABLE_BIT != 0) {
            (true, false) => self.enter_lcd_disabled_state(),
            (false, true) => {
                if source == PpuRegisterWriteSource::CpuMmioCommit {
                    self.enter_lcd_enable_pending_state(CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES);
                } else {
                    self.enter_lcd_enabled_restart_state();
                }
            }
            _ => {
                self.lcd_state = lcd_state_from_lcdc(value);
                self.refresh_visible_output();
            }
        }

        self.refresh_stat_irq_line(false);
    }

    pub(in crate::ppu) fn read_stat(&self, source: PpuRegisterReadSource) -> u8 {
        STAT_FORCED_HIGH_BIT
            | self.stat_interrupt_enable
            | if self.read_stat_lyc_coincidence(source) {
                0x04
            } else {
                0x00
            }
            | if self.is_lcd_enabled() {
                match source {
                    PpuRegisterReadSource::Immediate => self.current_cpu_visible_access_mode(),
                    PpuRegisterReadSource::CpuBusOperation => self
                        .dmg_boot_power_on_stat_access_mode()
                        .unwrap_or_else(|| self.current_published_stat_access_mode()),
                }
                .stat_bits()
            } else {
                PpuAccessMode::HBlank.stat_bits()
            }
    }

    pub(in crate::ppu) fn read_stat_lyc_coincidence(&self, source: PpuRegisterReadSource) -> bool {
        if source == PpuRegisterReadSource::CpuBusOperation
            && let Some(ly) = self.dmg_boot_power_on_visible_ly()
        {
            return ly == self.lyc;
        }

        if source == PpuRegisterReadSource::CpuBusOperation
            && self.dmg_lcd_restart_line1_lyc_readback_delay_active()
        {
            return false;
        }

        self.lyc_coincidence_for_readback()
    }

    fn dmg_lcd_restart_line1_lyc_readback_delay_active(&self) -> bool {
        self.console_model.is_dmg_family()
            && self.is_lcd_enabled()
            && self.runtime.blank_frame_active
            && self.ly == 1
            && self.line_dot < LINE0_VBLANK_WRAP_STAT_READBACK_DELAY_DOTS
    }

    pub(in crate::ppu) fn write_stat(&mut self, value: u8) {
        self.stat_interrupt_enable = value & STAT_WRITABLE_ENABLE_MASK;
        if self.cancel_obsolete_line_153_lyc0_stat_irq_pretrigger() {
            self.runtime.stat_state.irq_line = false;
        }
        let quirk_active = self.stat_write_quirk_active_for_write();
        self.runtime
            .stat_state
            .dmg_stat_write_quirk_blocks_line153_lyc0 = false;
        if quirk_active
            && self.console_model.is_dmg_family()
            && self.current_access_mode() == PpuAccessMode::VBlank
        {
            self.runtime
                .stat_state
                .dmg_stat_write_quirk_blocks_line153_lyc0 = true;
        }
        let ordinary_line = self.ordinary_stat_irq_line();
        let new_line = ordinary_line || quirk_active;
        let write_requests_ordinary_edge = ordinary_line
            && (self.mode0_stat_write_irq_source()
                || self.mode1_stat_write_irq_source()
                || self.mode2_stat_write_irq_source()
                || self.lyc_stat_write_irq_source());
        if !self.runtime.stat_state.irq_line && (quirk_active || write_requests_ordinary_edge) {
            self.queue_interrupt_request_with_cpu_if_visibility(
                InterruptSource::LcdStat,
                !self.stat_request_hidden_from_same_cycle_cpu_if(),
            );
        }
        self.runtime.stat_state.irq_line = new_line;
    }

    pub(in crate::ppu) fn read_ly(&self, source: PpuRegisterReadSource) -> u8 {
        if source == PpuRegisterReadSource::CpuBusOperation
            && let Some(ly) = self.dmg_boot_power_on_visible_ly()
        {
            return ly;
        }

        let visible_ly = self.read_ly_without_skip_boot_lag();

        if self.skip_boot_ly_read_lag_active() {
            visible_ly.checked_sub(1).unwrap_or(TOTAL_SCANLINES - 1)
        } else {
            visible_ly
        }
    }

    // The CPU-first reorder makes every CPU register read observe the PPU at the PRE-tick
    // `line_dot` (one dot behind the post-tick raster that `main` publishes). The canonical
    // readback model therefore evaluates `main`'s exact register bodies at the post-tick
    // reference `line_dot + 1` (with the natural end-of-line / line-153 wrap), undoing the
    // reorder in ONE place instead of the scattered per-path `+1` compensations.
    pub(in crate::ppu) fn readback_reference(&self) -> (u8, u16) {
        let next_dot = self.line_dot + 1;
        if next_dot >= self.current_scanline_length() {
            let next_ly = if self.ly + 1 == TOTAL_SCANLINES {
                0
            } else {
                self.ly + 1
            };
            (next_ly, 0)
        } else {
            (self.ly, next_dot)
        }
    }

    fn read_ly_without_skip_boot_lag(&self) -> u8 {
        let (ref_ly, ref_dot) = self.readback_reference();

        if self.line_153_reads_as_ly0_at(ref_ly, ref_dot) {
            return 0;
        }

        if self.is_lcd_enabled()
            && !self.runtime.blank_frame_active
            && ref_ly < VISIBLE_SCANLINES
            && !self.vblank_wrap_line0_ly_read_delay_active()
            && ref_dot >= self.current_ly_read_advance_start_dot()
            && ref_ly + 1 < TOTAL_SCANLINES
        {
            return ref_ly + 1;
        }

        ref_ly
    }

    fn skip_boot_ly_read_lag_active(&self) -> bool {
        self.runtime.stat_state.skip_boot_ly_read_lag_active && self.is_lcd_enabled()
    }

    fn line_153_reads_as_ly0_at(&self, ref_ly: u8, ref_dot: u16) -> bool {
        let ly0_dot = if self.console_model.is_cgb_family() {
            CGB_LINE_153_LY_READ_ZERO_DOT
        } else {
            LINE_153_LY_READ_ZERO_DOT
        };

        self.is_lcd_enabled() && ref_ly == TOTAL_SCANLINES - 1 && ref_dot >= ly0_dot
    }

    fn vblank_wrap_line0_ly_read_delay_active(&self) -> bool {
        self.runtime.stat_state.vblank_wrap_line0_stat_delay_active && self.ly == 0
    }

    pub(in crate::ppu) fn current_access_mode(&self) -> PpuAccessMode {
        let restart_raster_state = self.lcd_restart_phase.raster_state(self.ly, self.line_dot);
        if !self.is_lcd_enabled() {
            PpuAccessMode::HBlank
        } else if let Some(raster_state) = restart_raster_state {
            raster_state.access_mode()
        } else if self.runtime.startup_mode_latch.is_none() && self.ly >= VISIBLE_SCANLINES {
            PpuAccessMode::VBlank
        } else if self.runtime.startup_mode_latch.is_none() && self.line_dot < MODE2_DOTS {
            PpuAccessMode::OamScan
        } else {
            let mode0_start_dot = self.current_mode0_start_dot();
            self.runtime
                .startup_mode_latch
                .unwrap_or_else(|| access_mode_from_raster(self.ly, self.line_dot, mode0_start_dot))
        }
    }

    pub(in crate::ppu) fn access_mode_for_line_dot(&self, line_dot: u16) -> PpuAccessMode {
        if !self.is_lcd_enabled() {
            return PpuAccessMode::HBlank;
        }

        if let Some(raster_state) = self.lcd_restart_phase.raster_state(self.ly, line_dot) {
            return raster_state.access_mode();
        }

        if self.ly >= VISIBLE_SCANLINES {
            return PpuAccessMode::VBlank;
        }

        if line_dot < MODE2_DOTS {
            return PpuAccessMode::OamScan;
        }

        access_mode_from_raster(self.ly, line_dot, self.current_mode0_start_dot())
    }

    pub(in crate::ppu) fn bus_access_mode_for_line_dot(&self, line_dot: u16) -> PpuAccessMode {
        let current_mode = self.access_mode_for_line_dot(line_dot);

        if !self.is_lcd_enabled() || self.ly >= VISIBLE_SCANLINES {
            return current_mode;
        }

        if current_mode == PpuAccessMode::HBlank
            && self.ly + 1 < VISIBLE_SCANLINES
            && line_dot + 4 >= self.current_scanline_length()
        {
            return PpuAccessMode::OamScan;
        }

        if current_mode == PpuAccessMode::OamScan && line_dot + 4 >= MODE2_DOTS {
            return PpuAccessMode::Drawing;
        }

        current_mode
    }

    pub(in crate::ppu) fn current_published_bus_access_mode(&self) -> PpuAccessMode {
        if let Some(mode) = self.dmg_boot_power_on_bus_access_mode() {
            return mode;
        }

        let published_line_dot = self.line_dot.saturating_sub(1);
        self.bus_access_mode_for_line_dot(published_line_dot)
    }

    pub(in crate::ppu) fn current_published_video_write_access_mode(&self) -> PpuAccessMode {
        if let Some(mode) = self.dmg_boot_power_on_bus_access_mode() {
            return mode;
        }

        if self.line_dot != 0 {
            self.access_mode_for_line_dot(self.line_dot - 1)
        } else if self.ly == 0 {
            self.current_access_mode()
        } else if self.ly > VISIBLE_SCANLINES {
            PpuAccessMode::VBlank
        } else {
            PpuAccessMode::HBlank
        }
    }

    pub(in crate::ppu) fn dmg_boot_power_on_visible_ly(&self) -> Option<u8> {
        let elapsed_mcycles = self.dmg_boot_power_on_elapsed_mcycles()?;
        Some(match elapsed_mcycles {
            0..=119 => 0,
            120..=233 => 1,
            _ => 2,
        })
    }

    pub(in crate::ppu) fn dmg_boot_power_on_stat_access_mode(&self) -> Option<PpuAccessMode> {
        let elapsed_mcycles = self.dmg_boot_power_on_elapsed_mcycles()?;
        Some(match elapsed_mcycles {
            0..=5 => PpuAccessMode::VBlank,
            6 => PpuAccessMode::HBlank,
            7..=26 => PpuAccessMode::OamScan,
            27..=69 => PpuAccessMode::Drawing,
            70..=120 => PpuAccessMode::HBlank,
            121..=140 => PpuAccessMode::OamScan,
            141..=183 => PpuAccessMode::Drawing,
            184..=234 => PpuAccessMode::HBlank,
            235 => PpuAccessMode::OamScan,
            _ => return None,
        })
    }

    pub(in crate::ppu) fn dmg_boot_power_on_bus_access_mode(&self) -> Option<PpuAccessMode> {
        let elapsed_mcycles = self.dmg_boot_power_on_elapsed_mcycles()?;
        Some(match elapsed_mcycles {
            0..=5 => PpuAccessMode::HBlank,
            6..=25 => PpuAccessMode::OamScan,
            26..=69 => PpuAccessMode::Drawing,
            70..=119 => PpuAccessMode::HBlank,
            120..=139 => PpuAccessMode::OamScan,
            140..=183 => PpuAccessMode::Drawing,
            184..=233 => PpuAccessMode::HBlank,
            234..=235 => PpuAccessMode::OamScan,
            _ => return None,
        })
    }

    pub(in crate::ppu) fn dmg_boot_power_on_elapsed_mcycles(&self) -> Option<u16> {
        if !self.runtime.stat_state.boot_power_on_ppu_phase_active
            || !self.console_model.is_dmg_family()
            || !self.is_lcd_enabled()
        {
            return None;
        }

        let frame_dots = self.dmg_boot_power_on_frame_dots();
        let base_dot = self.runtime.stat_state.boot_power_on_ppu_phase_base_dot % frame_dots;
        let current_dot = self.dmg_boot_power_on_current_frame_dot();
        let elapsed_dots = if current_dot >= base_dot {
            current_dot - base_dot
        } else {
            frame_dots - base_dot + current_dot
        };
        let elapsed_mcycles = (elapsed_dots / 4) as u16;
        (elapsed_mcycles <= DMG_BOOT_POWER_ON_MAX_DELAY_M_CYCLES).then_some(elapsed_mcycles)
    }

    pub(in crate::ppu) fn dmg_boot_power_on_current_frame_dot(&self) -> u32 {
        (u32::from(self.ly) * u32::from(DOTS_PER_SCANLINE) + u32::from(self.line_dot))
            % self.dmg_boot_power_on_frame_dots()
    }

    pub(in crate::ppu) fn dmg_boot_power_on_frame_dots(&self) -> u32 {
        u32::from(DOTS_PER_SCANLINE) * u32::from(TOTAL_SCANLINES)
    }
}
