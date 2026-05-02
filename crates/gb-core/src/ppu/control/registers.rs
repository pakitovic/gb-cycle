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
                    PpuRegisterReadSource::CpuBusOperation => {
                        self.current_published_stat_access_mode()
                    }
                }
                .stat_bits()
            } else {
                PpuAccessMode::HBlank.stat_bits()
            }
    }

    pub(in crate::ppu) fn read_stat_lyc_coincidence(&self, source: PpuRegisterReadSource) -> bool {
        if source == PpuRegisterReadSource::CpuBusOperation
            && self.is_lcd_enabled()
            && self.line_dot == 0
        {
            false
        } else {
            self.effective_lyc_coincidence()
        }
    }

    pub(in crate::ppu) fn write_stat(&mut self, value: u8) {
        self.stat_interrupt_enable = value & STAT_WRITABLE_ENABLE_MASK;
        self.refresh_stat_irq_line(self.stat_write_quirk_active());
    }

    pub(in crate::ppu) fn read_ly(&self) -> u8 {
        if self.is_lcd_enabled()
            && !self.runtime.blank_frame_active
            && self.console_model.is_cgb_family()
            && self.ly == TOTAL_SCANLINES - 1
            && self.line_dot >= LINE_153_LY0_DOT
        {
            return 0;
        }

        if self.is_lcd_enabled()
            && !self.runtime.blank_frame_active
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot >= self.current_ly_read_advance_start_dot()
            && self.ly + 1 < TOTAL_SCANLINES
        {
            self.ly + 1
        } else {
            self.ly
        }
    }

    pub(in crate::ppu) fn current_access_mode(&self) -> PpuAccessMode {
        self.current_raster_state().access_mode()
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
        let published_line_dot = self.line_dot.saturating_sub(1);
        self.bus_access_mode_for_line_dot(published_line_dot)
    }

    pub(in crate::ppu) fn current_published_video_write_access_mode(&self) -> PpuAccessMode {
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
}
