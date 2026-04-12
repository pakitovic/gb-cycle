impl Ppu {
    fn is_lcd_enabled(&self) -> bool {
        self.lcd_state.is_enabled()
    }

    fn sync_visible_registers(&mut self) {
        self.visible_registers = PpuVisibleRegisters {
            lcdc: self.lcdc,
            scy: self.scy,
            scx: self.scx,
            bgp: self.bgp,
            obp0: self.obp0,
            obp1: self.obp1,
            wy: self.wy,
            wx: self.wx,
        };
    }

    fn sync_pipeline_registers(&mut self) {
        self.pipeline_registers = self.visible_registers;
    }

    fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    fn write_lcdc(&mut self, value: u8, source: PpuRegisterWriteSource) {
        let was_lcd_enabled = self.is_lcd_enabled() || self.lcd_enable_pending_delay_tcycles != 0;
        self.lcdc = value;
        self.startup_mode_latch = None;

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

    fn read_stat(&self, source: PpuRegisterReadSource) -> u8 {
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
                    PpuRegisterReadSource::CpuBusOperation => self.current_published_stat_access_mode(),
                }
                .stat_bits()
            } else {
                PpuAccessMode::HBlank.stat_bits()
            }
    }

    fn read_stat_lyc_coincidence(&self, source: PpuRegisterReadSource) -> bool {
        if source == PpuRegisterReadSource::CpuBusOperation
            && self.is_lcd_enabled()
            && self.line_dot == 0
        {
            false
        } else {
            self.effective_lyc_coincidence()
        }
    }

    fn write_stat(&mut self, value: u8) {
        self.stat_interrupt_enable = value & STAT_WRITABLE_ENABLE_MASK;
        self.refresh_stat_irq_line(self.stat_write_quirk_active());
    }

    fn read_ly(&self) -> u8 {
        if self.is_lcd_enabled()
            && !self.blank_frame_active
            && self.line_dot >= self.current_ly_read_advance_start_dot()
            && self.ly + 1 < TOTAL_SCANLINES
        {
            self.ly + 1
        } else {
            self.ly
        }
    }

    fn current_access_mode(&self) -> PpuAccessMode {
        self.current_raster_state().access_mode()
    }

    fn access_mode_for_line_dot(&self, line_dot: u16) -> PpuAccessMode {
        if !self.is_lcd_enabled() {
            return PpuAccessMode::HBlank;
        }

        if let Some(raster_state) = self.lcd_restart_phase.raster_state(self.ly, line_dot) {
            return raster_state.access_mode();
        }

        access_mode_from_raster(self.ly, line_dot, self.current_mode0_start_dot())
    }

    fn bus_access_mode_for_line_dot(&self, line_dot: u16) -> PpuAccessMode {
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

    fn current_published_bus_access_mode(&self) -> PpuAccessMode {
        let published_line_dot = self.line_dot.saturating_sub(1);
        self.bus_access_mode_for_line_dot(published_line_dot)
    }

    fn current_published_video_write_access_mode(&self) -> PpuAccessMode {
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

    fn current_published_stat_access_mode(&self) -> PpuAccessMode {
        if self.line_dot != 0 {
            let published_mode = self.access_mode_for_line_dot(self.line_dot - 1);
            let sprite_extended_mode3 =
                self.current_mode0_start_dot() > self.baseline_mode0_start_dot();

            if published_mode == PpuAccessMode::OamScan
                && self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::Drawing
                && !self.blank_frame_active
                && self.ly < VISIBLE_SCANLINES
                && self.line_dot == MODE2_DOTS
            {
                return PpuAccessMode::Drawing;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.terminal_visible_tail_should_publish_hblank_early()
                && !self.saturated_placeholder_backed_terminal_bg_tail_still_owned_by_mode3()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::HBlank
                && !self.blank_frame_active
                && self.ly < VISIBLE_SCANLINES
                && self.line_dot == self.current_mode0_start_dot()
                && !sprite_extended_mode3
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::HBlank
                && !self.blank_frame_active
                && self.ly < VISIBLE_SCANLINES
                && sprite_extended_mode3
                && self.line_dot == self.current_mode0_start_dot().saturating_add(2)
            {
                return PpuAccessMode::Drawing;
            }

            return published_mode;
        }

        if self.ly == 0 {
            return self.current_access_mode();
        }

        if self.ly > VISIBLE_SCANLINES {
            PpuAccessMode::VBlank
        } else {
            PpuAccessMode::HBlank
        }
    }

    fn terminal_visible_tail_should_publish_hblank_early(&self) -> bool {
        let mode0_interrupt_enabled =
            self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0;
        let saturated_sprite_line =
            usize::from(self.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE;
        let saturated_sprite_line_uses_earlier_terminal_hblank =
            saturated_sprite_line && self.bg_pipeline_state.current_transfer_x == 163;
        let saturated_sprite_line_placeholder_backed_visible_tail_can_publish_hblank =
            saturated_sprite_line
                && self.bg_pipeline_state.startup_fifo_placeholders > 0
                && if self.blank_frame_active {
                    self.bg_pipeline_state.current_transfer_x >= 162
                } else {
                    matches!(self.bg_pipeline_state.current_transfer_x, 162 | 163)
                };
        let saturated_sprite_line_exact_x151_ready_tail_can_publish_hblank =
            saturated_sprite_line
                && self.bg_pipeline_state.current_transfer_x == 151
                && self.bg_pipeline_state.fifo.len() == 1
                && self.bg_pipeline_state.startup_fifo_placeholders == 0;
        let saturated_sprite_line_exact_x159_ready_tail_can_publish_hblank =
            saturated_sprite_line
                && self.bg_pipeline_state.current_transfer_x == 159
                && self.bg_pipeline_state.fifo.len() == 1
                && self.bg_pipeline_state.startup_fifo_placeholders == 0
                && self.current_mode0_start_dot() >= MODE0_START_DOT + 65;
        let saturated_sprite_line_placeholder_tail_can_publish_hblank =
            mode0_interrupt_enabled
                && saturated_sprite_line
                && self.bg_pipeline_state.current_transfer_x >= 164;
        let saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank =
            saturated_sprite_line
                && self.bg_pipeline_state.current_transfer_x >= 152
                && self.bg_pipeline_state.fifo.is_empty()
                && self.bg_pipeline_state.startup_fifo_placeholders == 0;

        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 1 == self.current_mode0_start_dot()
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && (((self.blank_frame_active && self.bg_pipeline_state.current_transfer_x >= 165)
                || self.bg_pipeline_state.current_transfer_x >= 167)
                || saturated_sprite_line_uses_earlier_terminal_hblank
                || saturated_sprite_line_placeholder_backed_visible_tail_can_publish_hblank
                || saturated_sprite_line_exact_x151_ready_tail_can_publish_hblank
                || saturated_sprite_line_exact_x159_ready_tail_can_publish_hblank
                || saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank)
            && (self.bg_pipeline_state.fifo_contains_real_pixels()
                || saturated_sprite_line_placeholder_backed_visible_tail_can_publish_hblank
                || saturated_sprite_line_placeholder_tail_can_publish_hblank
                || saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank)
            && self.current_transfer().is_some_and(|transfer| {
                matches!(transfer.context.lane, Mode3TransferLane::Visible)
                    && transfer.context.source_window == Mode3TransferSourceWindow::FifoBacked
                    && (matches!(transfer.readiness, Mode3TransferReadiness::Ready(_))
                        || (saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank
                            && matches!(
                                transfer.readiness,
                                Mode3TransferReadiness::WaitingForFifo(_)
                            )))
            })
    }

    fn current_published_oam_write_access_mode(&self) -> PpuAccessMode {
        let published_mode = self.current_published_video_write_access_mode();

        if published_mode == PpuAccessMode::OamScan
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot == MODE2_DOTS
        {
            PpuAccessMode::HBlank
        } else {
            published_mode
        }
    }

    fn current_published_oam_read_access_mode(&self) -> PpuAccessMode {
        let published_mode = self.current_published_bus_access_mode();

        if published_mode == PpuAccessMode::Drawing
            && !self.blank_frame_active
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

    fn current_bus_access_mode(&self) -> PpuAccessMode {
        let current_mode = self.current_access_mode();

        if !self.is_lcd_enabled() || self.ly >= VISIBLE_SCANLINES {
            return current_mode;
        }

        if current_mode == PpuAccessMode::HBlank
            && self.ly + 1 < VISIBLE_SCANLINES
            && self.line_dot + 4 >= self.current_scanline_length()
        {
            return PpuAccessMode::OamScan;
        }

        if current_mode == PpuAccessMode::OamScan && self.line_dot + 4 >= MODE2_DOTS {
            return PpuAccessMode::Drawing;
        }

        current_mode
    }

    fn current_cpu_visible_access_mode(&self) -> PpuAccessMode {
        if self.blank_frame_active && self.is_lcd_enabled() && self.line_dot != 0 {
            return self
                .lcd_restart_phase
                .raster_state(self.ly, self.line_dot - 1)
                .map(PpuRasterState::access_mode)
                .unwrap_or_else(|| self.current_access_mode());
        }

        self.current_access_mode()
    }

    fn current_raster_state(&self) -> PpuRasterState {
        if !self.is_lcd_enabled() {
            return PpuRasterState::Disabled;
        }

        if let Some(raster_state) = self.lcd_restart_phase.raster_state(self.ly, self.line_dot) {
            return raster_state;
        }

        let mode0_start_dot = self.current_mode0_start_dot();
        let mode = self
            .startup_mode_latch
            .unwrap_or_else(|| access_mode_from_raster(self.ly, self.line_dot, mode0_start_dot));

        PpuRasterState::Active {
            mode,
            mode_dot: mode_dot_from_raster_mode(mode, self.line_dot, mode0_start_dot),
            mode2_scan_active: self.ly < VISIBLE_SCANLINES
                && self.line_dot != 0
                && self.line_dot <= MODE2_DOTS,
        }
    }

    fn current_mode0_start_dot(&self) -> u16 {
        if self.ly >= VISIBLE_SCANLINES {
            return MODE0_START_DOT;
        }

        let mut mode0_start_dot = if self.bg_pipeline_state.mode3_started {
            let shortens_for_all_offscreen_right_sprites =
                self.bg_pipeline_state.mode0_start_dot == self.baseline_mode0_start_dot()
                    && self.visible_registers.obj_enabled()
                    && self.mode2_scan_state.selected_sprite_count() > 0
                    && (0..self.mode2_scan_state.selected_sprite_count()).all(|slot| {
                        self.mode2_scan_state
                            .selected_sprite(slot)
                            .is_some_and(|sprite| sprite.x >= 168)
                    });
            self.bg_pipeline_state
                .mode0_start_dot
                .saturating_sub(u16::from(shortens_for_all_offscreen_right_sprites))
        } else {
            MODE0_START_DOT + u16::from(self.visible_registers.scx & 0x07)
        };

        let pending_obj_hit_owns_current_transfer_x = self.obj_pipeline_state.pending_match_x
            == Some(self.bg_pipeline_state.current_transfer_x)
            && !self.obj_pipeline_state.pending_sprite_slots.is_empty();
        let live_transfer_still_owned_by_mode3 = self.current_transfer().is_some();
        if self.bg_pipeline_state.mode3_started
            && (self.obj_pipeline_state.fetch.stage != PpuObjFetcherStage::Idle
                || pending_obj_hit_owns_current_transfer_x
                || live_transfer_still_owned_by_mode3
                || self.saturated_placeholder_backed_terminal_bg_tail_still_owned_by_mode3())
        {
            mode0_start_dot = mode0_start_dot.max(self.line_dot.saturating_add(1));
        }

        mode0_start_dot
    }

    fn baseline_mode0_start_dot(&self) -> u16 {
        MODE0_START_DOT + u16::from(self.visible_registers.scx & 0x07)
    }

    fn saturated_placeholder_backed_terminal_bg_tail_still_owned_by_mode3(&self) -> bool {
        let terminal_bg_tail_has_unfinished_fetch_work = matches!(
            self.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
        ) || (self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining > 0);
        self.bg_pipeline_state.mode3_started
            && self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.bg_pipeline_state.current_transfer_x >= 168
            && usize::from(self.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.bg_pipeline_state.startup_fifo_placeholders > 0
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && terminal_bg_tail_has_unfinished_fetch_work
    }

    pub(crate) fn current_mode2_oam_row(&self) -> Option<u8> {
        if !self.is_lcd_enabled()
            || self.ly >= VISIBLE_SCANLINES
            || self.current_access_mode() != PpuAccessMode::OamScan
        {
            return None;
        }

        Some((self.line_dot / OAM_CORRUPTION_DOTS_PER_ROW) as u8)
    }

    pub(crate) fn apply_oam_corruption_event(
        &mut self,
        event: OamCorruptionEventKind,
        oam_bytes: &mut [u8],
    ) -> bool {
        let Some(row) = self.current_mode2_oam_row() else {
            return false;
        };

        self.oam_corruption_controller
            .apply(self.console_model, row, event, oam_bytes)
    }

    fn advance_mode2_scan(&mut self, oam: &OamBusView<'_>, dma_oam_active: bool) {
        let raster_state = self.current_raster_state();

        if self.ly >= VISIBLE_SCANLINES
            || !raster_state.is_mode2_scan()
            || self.line_dot == 0
            || !self.line_dot.is_multiple_of(MODE2_T_CYCLES_PER_OAM_ENTRY)
            || self.mode2_scan_state.scanned_entries() >= OAM_SPRITE_COUNT
        {
            return;
        }

        let oam_index = self.mode2_scan_state.scanned_entries();
        self.mode2_scan_state.increment_scanned_entries();

        if self.mode2_scan_state.is_full() {
            return;
        }

        let nominal_sprite = read_oam_sprite(oam, oam_index);
        let sprite = if dma_oam_active && self.console_model.is_dmg_family() {
            let Some((y, x)) = self.mode2_scan_state.latched_mode2_yx_word() else {
                return;
            };
            let (tile_index, attributes) = nominal_sprite
                .map(|sprite| (sprite.tile_index, sprite.attributes))
                .unwrap_or((0xFF, 0xFF));
            PpuSelectedSprite {
                oam_index,
                y,
                x,
                tile_index,
                attributes,
            }
        } else {
            let sprite = match nominal_sprite {
                Some(sprite) => sprite,
                None => return,
            };
            self.mode2_scan_state
                .latch_mode2_yx_word(sprite.y, sprite.x);
            sprite
        };

        if sprite_matches_line(sprite, self.ly, self.current_obj_height()) {
            self.mode2_scan_state.push(sprite);
        }
    }

    fn current_obj_height(&self) -> u8 {
        self.visible_registers.obj_height()
    }

    fn window_activation_registers(&self) -> PpuVisibleRegisters {
        if self.console_model.is_dmg_family() {
            self.pipeline_registers
        } else {
            self.visible_registers
        }
    }

    fn pixel_pipeline_lcdc(&self) -> u8 {
        if !self.console_model.is_dmg_family() {
            return self.visible_registers.lcdc;
        }

        if self.bg_pipeline_state.current_transfer_x == 8 {
            self.visible_registers.lcdc
        } else {
            self.pipeline_registers.lcdc
        }
    }

    fn pixel_pipeline_bgp(&self) -> u8 {
        if self.console_model.is_dmg_family() {
            self.visible_registers.bgp | self.pipeline_registers.bgp
        } else {
            self.visible_registers.bgp
        }
    }

    fn pixel_transfer_bg_enabled(&self) -> bool {
        self.pixel_pipeline_lcdc() & LCDC_BG_ENABLE_BIT != 0
    }

    fn pixel_transfer_obj_enabled(&self) -> bool {
        self.pixel_pipeline_lcdc() & LCDC_OBJ_ENABLE_BIT != 0
    }

    fn prepare_visible_scanline_state(&mut self) {
        if self.line_dot != 1 || self.ly >= VISIBLE_SCANLINES {
            return;
        }

        if self.visible_registers.wy < VISIBLE_SCANLINES && self.ly == self.visible_registers.wy {
            self.window_state.wy_triggered = true;
        }

        let wy_latch =
            self.window_state.wy_triggered && self.visible_registers.wy < VISIBLE_SCANLINES;
        let force_x0_this_line = wy_latch && self.window_state.pending_wx166_next_line;
        self.window_state.pending_wx166_next_line = false;
        self.bg_pipeline_state
            .prepare_window_line(wy_latch, force_x0_this_line);
    }

    fn live_lyc_coincidence(&self) -> bool {
        self.ly == self.lyc
    }

    fn effective_lyc_coincidence(&self) -> bool {
        if self.is_lcd_enabled() {
            self.live_lyc_coincidence()
        } else {
            self.stat_state.lcd_disabled_lyc_coincidence
        }
    }

    fn lcd_enable_pending_lyc_rise_source(&self) -> bool {
        self.lcd_enable_pending_delay_tcycles == 2
            && self.stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT != 0
            && !self.stat_state.lcd_disabled_lyc_coincidence
            && self.live_lyc_coincidence()
    }

    fn ordinary_stat_irq_line(&self) -> bool {
        let coincidence_source = self.stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT != 0
            && self.effective_lyc_coincidence();

        if !self.is_lcd_enabled() {
            return coincidence_source || self.lcd_enable_pending_lyc_rise_source();
        }

        let mode0_start_dot = self.current_mode0_start_dot();
        let mode0_pretrigger_source = self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT
            != 0
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot < mode0_start_dot
            && self.line_dot + 4 >= mode0_start_dot;
        let mode2_pretrigger_source = self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT
            != 0
            && self.ly + 1 < VISIBLE_SCANLINES
            && self.line_dot + 4 >= self.current_scanline_length();
        let dmg_mode2_vblank_entry_source = self.console_model.is_dmg_family()
            && self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.current_access_mode() == PpuAccessMode::VBlank
            && self.ly == VISIBLE_SCANLINES
            && self.line_dot == 0;
        let mode_source = match self.current_access_mode() {
            PpuAccessMode::HBlank => {
                self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0
            }
            PpuAccessMode::VBlank => {
                self.stat_interrupt_enable & STAT_MODE1_INTERRUPT_ENABLE_BIT != 0
            }
            PpuAccessMode::OamScan => {
                self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            }
            PpuAccessMode::Drawing => false,
        };

        coincidence_source
            || mode_source
            || mode0_pretrigger_source
            || mode2_pretrigger_source
            || dmg_mode2_vblank_entry_source
    }

    fn compute_stat_irq_line(&self, quirk_active: bool) -> bool {
        self.ordinary_stat_irq_line() || quirk_active
    }

    fn refresh_stat_irq_line(&mut self, quirk_active: bool) {
        let new_line = self.compute_stat_irq_line(quirk_active);
        if !self.stat_state.irq_line && new_line {
            self.queue_interrupt_request(InterruptSource::LcdStat);
        }
        self.stat_state.irq_line = new_line;
    }

    fn queue_interrupt_request(&mut self, source: InterruptSource) {
        let bit = match source {
            InterruptSource::VBlank => PPU_PENDING_VBLANK_INTERRUPT_BIT,
            InterruptSource::LcdStat => PPU_PENDING_LCD_STAT_INTERRUPT_BIT,
            InterruptSource::Timer | InterruptSource::Serial | InterruptSource::Joypad => {
                return;
            }
        };
        self.pending_interrupts |= bit;
    }

    fn stat_write_quirk_active(&self) -> bool {
        self.console_model.is_dmg_family()
            && self.is_lcd_enabled()
            && (self.current_access_mode() != PpuAccessMode::Drawing
                || self.live_lyc_coincidence())
    }

    fn refresh_visible_output(&mut self) {
        self.visible_output =
            if self.is_lcd_enabled() && !self.blank_frame_active && !self.system_stop_active {
                PpuVisibleOutputState::Driving
            } else {
                PpuVisibleOutputState::ForcedBlank
            };
    }

    fn advance_lcd_restart_phase(&mut self) {
        self.lcd_restart_phase = self.lcd_restart_phase.advance(self.ly, self.line_dot);
    }

    fn reset_runtime_pipeline_state(&mut self) {
        self.startup_mode_latch = None;
        self.mode2_scan_state.reset();
        self.window_state.reset();
        self.bg_pipeline_state.reset();
        self.obj_pipeline_state.reset();
        self.current_scanline_pixels.fill(0);
        self.current_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
    }

    fn clear_visible_buffers(&mut self) {
        self.current_scanline_pixels.fill(0);
        self.framebuffer.fill(0);
    }

    fn enter_lcd_disabled_state(&mut self) {
        self.lcd_state = PpuLcdState::Disabled;
        self.lcd_enable_pending_delay_tcycles = 0;
        self.blank_frame_active = false;
        self.stat_state.lcd_disabled_lyc_coincidence = self.live_lyc_coincidence();
        self.ly = 0;
        self.line_dot = 0;
        self.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        self.reset_runtime_pipeline_state();
        self.sync_visible_registers();
        self.sync_pipeline_registers();
        self.clear_visible_buffers();
        self.refresh_visible_output();
    }

    fn enter_lcd_enabled_restart_state(&mut self) {
        self.lcd_state = PpuLcdState::Enabled;
        self.lcd_enable_pending_delay_tcycles = 0;
        self.blank_frame_active = true;
        self.ly = 0;
        self.line_dot = LCD_REENABLE_INITIAL_LINE_DOT;
        self.lcd_restart_phase = PpuLcdRestartPhase::first_line_after_enable();
        self.stat_state.lcd_disabled_lyc_coincidence = false;
        self.reset_runtime_pipeline_state();
        self.sync_visible_registers();
        self.sync_pipeline_registers();
        self.clear_visible_buffers();
        self.refresh_visible_output();
    }

    fn enter_lcd_enable_pending_state(&mut self, delay_tcycles: u8) {
        self.lcd_state = PpuLcdState::Disabled;
        self.lcd_enable_pending_delay_tcycles = delay_tcycles;
        self.startup_mode_latch = None;
        self.refresh_visible_output();
    }
}
