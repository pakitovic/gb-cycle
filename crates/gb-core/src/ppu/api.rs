use super::snapshot::*;
use super::*;

impl Ppu {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: PpuStatus::RegistersReady,
            lcdc: 0,
            stat_interrupt_enable: 0,
            lcd_state: PpuLcdState::Disabled,
            lcd_enable_pending_delay_tcycles: 0,
            visible_output: PpuVisibleOutputState::ForcedBlank,
            scy: 0,
            scx: 0,
            ly: 0,
            line_dot: 0,
            lcd_restart_phase: PpuLcdRestartPhase::Inactive,
            lyc: 0,
            bgp: 0,
            obp0: None,
            obp1: None,
            wy: 0,
            wx: 0,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
            visible_registers: PpuVisibleRegisters::default(),
            pipeline_registers: PpuVisibleRegisters::default(),
            dmg_bgp_cpu_commit_output_palette_override: None,
            dmg_bgp_cpu_commit_output_delay_pixels_remaining: 0,
            dmg_bgp_cpu_commit_output_followup_palette_override: None,
            dmg_bgp_cpu_commit_output_followup_pixels_remaining: 0,
            dmg_bgp_cpu_commit_bg_visible_hold_palette_override: None,
            dmg_bgp_cpu_commit_bg_visible_hold_bg_pixels_remaining: 0,
            dmg_bgp_cpu_commit_bg_visible_hold_fallback_palette: None,
            dmg_bgp_cpu_commit_current_line_start_palette: 0,
            dmg_bgp_cpu_commit_previous_line_start_palette: 0,
            dmg_bgp_cpu_commit_current_line_writes: Vec::new(),
            dmg_bgp_cpu_commit_previous_line_writes: Vec::new(),
            dmg_recent_panel_dots: VecDeque::with_capacity(DMG_PALETTE_RETROACTIVE_DOT_HISTORY),
            last_unsigned_tile_data_fetch: 0,
            last_unsigned_tile_data_low_fetch: 0,
            last_unsigned_tile_data_high_fetch: 0,
            startup_mode_latch: None,
            stat_state: StatState::default(),
            pending_interrupts: 0,
            blank_frame_active: false,
            system_stop_active: false,
            oam_corruption_controller: OamCorruptionController,
            mode2_scan_state: Mode2ScanState::default(),
            window_state: WindowState::default(),
            bg_pipeline_state: BgPipelineState::default(),
            obj_pipeline_state: ObjPipelineState::default(),
            current_scanline_pixels: [0; SCREEN_WIDTH],
            current_scanline_mixed_pixels: [MixedPixel::background(0); SCREEN_WIDTH],
            current_scanline_dmg_bg_forced_white: [false; SCREEN_WIDTH],
            previous_scanline_mixed_pixels: [MixedPixel::background(0); SCREEN_WIDTH],
            previous_scanline_dmg_bg_forced_white: [false; SCREEN_WIDTH],
            previous_scanline_ly: None,
            framebuffer: vec![0; FRAMEBUFFER_PIXELS],
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> PpuStatus {
        self.status
    }

    pub fn bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_bus_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn cpu_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_published_bus_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn cpu_write_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_published_video_write_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn cpu_oam_read_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_published_oam_read_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn cpu_oam_write_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_published_oam_write_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn owner_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_bus_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub fn read_register(&self, address: u16) -> u8 {
        self.read_register_with_source(address, PpuRegisterReadSource::Immediate)
    }

    pub(crate) fn read_register_with_source(
        &self,
        address: u16,
        source: PpuRegisterReadSource,
    ) -> u8 {
        let Some(register) = PpuRegister::from_address(address) else {
            return 0xFF;
        };
        self.read_ppu_register(register, source)
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        self.write_register_with_source(address, value, PpuRegisterWriteSource::Immediate);
    }

    pub(crate) fn write_register_with_source(
        &mut self,
        address: u16,
        value: u8,
        source: PpuRegisterWriteSource,
    ) {
        let Some(register) = PpuRegister::from_address(address) else {
            return;
        };
        let previous_mmio_registers = self.current_mmio_visible_registers();
        self.write_ppu_register(register, value, source);

        if let Some(live_background_register) =
            PpuMode3LiveBackgroundRegister::from_register(register)
            && self.current_access_mode() == PpuAccessMode::Drawing
        {
            let write_context =
                self.current_mode3_live_register_write_context(previous_mmio_registers);
            if self.bg_pipeline_state.push.pending {
                self.bg_pipeline_state
                    .push
                    .cached
                    .mark_live_register_write_while_push_pending(
                        live_background_register,
                        write_context,
                        self.bg_pipeline_state.push.entry_delay_remaining > 0,
                    );
            }

            if self.bg_pipeline_state.fill.pending {
                self.bg_pipeline_state
                    .fill
                    .cached
                    .mark_live_register_write_while_fill_pending(
                        live_background_register,
                        write_context,
                        self.bg_pipeline_state.fill.includes_real_tile_pixels,
                        self.bg_pipeline_state.fill.startup_dummy_pixels,
                    );
            }

            if matches!(
                live_background_register,
                PpuMode3LiveBackgroundRegister::Lcdc
            ) {
                self.bg_pipeline_state
                    .mark_live_lcdc3_write_while_fifo_visible(write_context);
            }

            self.bg_pipeline_state
                .fetcher
                .mark_live_register_write_for_current_background_fetch(
                    live_background_register,
                    write_context,
                );

            if matches!(
                live_background_register,
                PpuMode3LiveBackgroundRegister::Scx
            ) && write_context.bg_scx_tilemap_column_changed()
                && self.startup_visible_tile3_scx_boundary_full_refetch_needs_next_tile()
            {
                self.bg_pipeline_state
                    .fetcher
                    .needs_live_tilemap_refetch_on_push = false;
                self.bg_pipeline_state
                    .fetcher
                    .needs_live_tilemap_full_refetch_on_push = false;
                self.bg_pipeline_state
                    .fetcher
                    .startup_visible_tile3_scx_boundary_full_refetch_next_tile = false;
                self.bg_pipeline_state
                    .fetcher
                    .clear_startup_visible_tile3_scx_boundary_old_pixel_window();
                self.bg_pipeline_state
                    .startup_visible_tile3_scx_boundary_next_slice_previous_scx = None;
                self.bg_pipeline_state
                    .startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels = 0;
            }

            if matches!(
                live_background_register,
                PpuMode3LiveBackgroundRegister::Scx
            ) && write_context.bg_scx_tilemap_column_changed()
                && self.inactive_visible_tile3_scx_push_boundary_needs_old_pixel_window()
            {
                self.bg_pipeline_state
                    .push
                    .cached
                    .needs_live_tilemap_refetch = false;
                self.bg_pipeline_state
                    .push
                    .cached
                    .needs_live_tilemap_full_refetch = false;
                self.bg_pipeline_state
                    .push
                    .cached
                    .startup_visible_tile3_scx_boundary_previous_scx = None;
                self.bg_pipeline_state
                    .push
                    .cached
                    .startup_visible_tile3_scx_boundary_old_tail_start_pixel = BG_TILE_WIDTH;
                self.bg_pipeline_state
                    .push
                    .cached
                    .startup_visible_tile3_scx_boundary_old_prefix_pixels = 0;
                self.bg_pipeline_state
                    .startup_visible_tile3_scx_boundary_next_slice_previous_scx = None;
                self.bg_pipeline_state
                    .startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels = 0;

                if (0x08..=0x0E).contains(&self.scx) && self.scx & 0x07 == 0x03 {
                    self.bg_pipeline_state
                        .push
                        .cached
                        .arm_startup_visible_tile3_scx_boundary_next_tile_output_retarget(
                            self.visible_registers.scx,
                        );
                }
            }

            if matches!(
                live_background_register,
                PpuMode3LiveBackgroundRegister::Scx
            ) && write_context.bg_scx_tilemap_column_changed()
                && self.inactive_visible_tile3_scx_push_boundary_needs_next_tile_output_retarget()
            {
                let scx_low_bits = self.scx & 0x07;
                self.bg_pipeline_state
                    .push
                    .cached
                    .arm_startup_visible_tile3_scx_boundary_next_tile_output_retarget(self.scx);
                if scx_low_bits >= 0x03 {
                    self.bg_pipeline_state
                        .push
                        .cached
                        .arm_startup_visible_tile3_scx_boundary_old_tail(
                            self.visible_registers.scx,
                            self.scx,
                        );
                    if scx_low_bits == 0x03 {
                        self.bg_pipeline_state
                            .push
                            .cached
                            .startup_visible_tile3_scx_boundary_old_tail_start_pixel =
                            BG_TILE_WIDTH.saturating_sub(4);
                    }
                }
                if matches!(scx_low_bits, 0x00 | 0x06) {
                    self.bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_previous_scx =
                        Some(self.visible_registers.scx);
                    self.bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_old_prefix_pixels = 1;
                }
                if self.scx >= 0x60 && scx_low_bits == 0x01 {
                    self.bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_previous_scx =
                        Some(self.visible_registers.scx);
                    self.bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_old_prefix_pixels = 2;
                }
                if self.scx >= 0x78 && matches!(scx_low_bits, 0x00..=0x02) {
                    self.bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_previous_scx =
                        Some(self.visible_registers.scx);
                    self.bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_old_tail_start_pixel = self
                        .bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_old_tail_start_pixel
                        .min(BG_TILE_WIDTH.saturating_sub(scx_low_bits.saturating_add(1)));
                }
                if self.scx >= 0x60 && matches!(scx_low_bits, 0x03..=0x05) {
                    self.bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_previous_scx =
                        Some(self.visible_registers.scx);
                    self.bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_old_prefix_pixels = self
                        .bg_pipeline_state
                        .push
                        .cached
                        .startup_visible_tile3_scx_boundary_old_prefix_pixels
                        .max(5);
                }
            }
        }

        if !self.is_lcd_enabled() {
            self.reload_mode3_register_latches_from_mmio();
        }
    }

    fn read_ppu_register(&self, register: PpuRegister, source: PpuRegisterReadSource) -> u8 {
        match register {
            PpuRegister::Lcdc => self.read_lcdc(),
            PpuRegister::Stat => self.read_stat(source),
            PpuRegister::Scy => self.scy,
            PpuRegister::Scx => self.scx,
            PpuRegister::Ly => self.read_ly(),
            PpuRegister::Lyc => self.lyc,
            PpuRegister::Bgp => self.bgp,
            PpuRegister::Obp0 => self
                .obp0
                .unwrap_or(self.obj_palette_read_policy.default_read_value()),
            PpuRegister::Obp1 => self
                .obp1
                .unwrap_or(self.obj_palette_read_policy.default_read_value()),
            PpuRegister::Wy => self.wy,
            PpuRegister::Wx => self.wx,
        }
    }

    fn write_ppu_register(
        &mut self,
        register: PpuRegister,
        value: u8,
        source: PpuRegisterWriteSource,
    ) {
        if let Some(palette_register) = register.palette_register() {
            self.write_dmg_palette_register(palette_register, value, source);
            return;
        }

        match register {
            PpuRegister::Lcdc => self.write_lcdc(value, source),
            PpuRegister::Stat => self.write_stat(value),
            PpuRegister::Scy => self.scy = value,
            PpuRegister::Scx => self.scx = value,
            PpuRegister::Ly => {}
            PpuRegister::Lyc => {
                self.lyc = value;
                if self.is_lcd_enabled() {
                    self.refresh_stat_irq_line(false);
                }
            }
            PpuRegister::Wy => self.wy = value,
            PpuRegister::Wx => self.wx = value,
            PpuRegister::Bgp | PpuRegister::Obp0 | PpuRegister::Obp1 => {
                unreachable!("palette writes return early")
            }
        }
    }

    pub fn apply_startup_state(&mut self, startup_state: PpuStartupState) {
        self.lcdc = startup_state.lcdc;
        self.stat_interrupt_enable = startup_state.stat & STAT_WRITABLE_ENABLE_MASK;
        self.lcd_state = lcd_state_from_lcdc(self.lcdc);
        self.lcd_enable_pending_delay_tcycles = 0;
        self.visible_output = visible_output_for_lcd_state(self.lcd_state);
        self.scy = startup_state.scy;
        self.scx = startup_state.scx;
        self.ly = startup_state.ly;
        self.line_dot = 0;
        self.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        self.lyc = startup_state.lyc;
        self.bgp = startup_state.bgp;
        self.wy = startup_state.wy;
        self.wx = startup_state.wx;
        self.obp0 = None;
        self.obp1 = None;
        self.obj_palette_read_policy = startup_state.obj_palette_read_policy;
        self.dmg_bgp_cpu_commit_output_palette_override = None;
        self.dmg_bgp_cpu_commit_output_delay_pixels_remaining = 0;
        self.dmg_bgp_cpu_commit_output_followup_palette_override = None;
        self.dmg_bgp_cpu_commit_output_followup_pixels_remaining = 0;
        self.dmg_bgp_cpu_commit_bg_visible_hold_palette_override = None;
        self.dmg_bgp_cpu_commit_bg_visible_hold_bg_pixels_remaining = 0;
        self.dmg_bgp_cpu_commit_bg_visible_hold_fallback_palette = None;
        self.dmg_bgp_cpu_commit_current_line_start_palette = startup_state.bgp;
        self.dmg_bgp_cpu_commit_previous_line_start_palette = startup_state.bgp;
        self.dmg_bgp_cpu_commit_current_line_writes.clear();
        self.dmg_bgp_cpu_commit_previous_line_writes.clear();
        self.dmg_recent_panel_dots.clear();
        self.blank_frame_active = false;
        self.oam_corruption_controller = OamCorruptionController;
        self.mode2_scan_state.reset();
        self.window_state.reset();
        self.bg_pipeline_state.reset();
        self.obj_pipeline_state.reset();
        self.pending_interrupts = 0;
        self.system_stop_active = false;
        self.current_scanline_pixels.fill(0);
        self.current_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
        self.current_scanline_dmg_bg_forced_white.fill(false);
        self.previous_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
        self.previous_scanline_dmg_bg_forced_white.fill(false);
        self.previous_scanline_ly = None;
        self.framebuffer.fill(0);
        self.reload_mode3_register_latches_from_mmio();
        self.startup_mode_latch = if self.lcd_state.is_enabled() {
            let startup_mode = PpuAccessMode::from_stat_bits(startup_state.stat);
            let derived_mode =
                access_mode_from_raster(self.ly, self.line_dot, self.current_mode0_start_dot());
            (startup_mode != derived_mode).then_some(startup_mode)
        } else {
            None
        };
        self.stat_state.lcd_disabled_lyc_coincidence = startup_state.ly == startup_state.lyc;
        self.stat_state.irq_line = self.compute_stat_irq_line(false);
    }

    #[cfg(test)]
    pub(crate) fn tick_t_cycle(
        &mut self,
        context: &mut CycleContext,
        oam: OamBusView<'_>,
        vram: VramBusView<'_>,
        dma_oam_active: bool,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) {
        let mut observer = NoopPpuStepObserver;
        self.tick_t_cycle_with_observer(
            context,
            oam,
            vram,
            dma_oam_active,
            dma_oam_conflict,
            &mut observer,
        );
    }

    pub(crate) fn tick_t_cycle_with_observer<O>(
        &mut self,
        _context: &mut CycleContext,
        oam: OamBusView<'_>,
        vram: VramBusView<'_>,
        dma_oam_active: bool,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
        observer: &mut O,
    ) where
        O: PpuStepObserver,
    {
        debug_assert_eq!(oam.master(), BusMaster::Ppu);
        debug_assert_eq!(vram.master(), BusMaster::Ppu);
        debug_assert_eq!(
            oam.is_acquired_by_master(),
            self.is_lcd_enabled()
                && matches!(
                    self.current_bus_access_mode(),
                    PpuAccessMode::OamScan | PpuAccessMode::Drawing
                )
        );
        debug_assert_eq!(
            oam.is_acquired(),
            oam.is_acquired_by_master() || dma_oam_active
        );
        debug_assert_eq!(
            vram.is_acquired_by_master(),
            self.is_lcd_enabled() && self.current_bus_access_mode() == PpuAccessMode::Drawing
        );

        if !self.is_lcd_enabled() {
            if self.lcd_enable_pending_delay_tcycles > 0 {
                self.lcd_enable_pending_delay_tcycles -= 1;
                if self.lcd_enable_pending_delay_tcycles == 2 {
                    self.refresh_stat_irq_line(false);
                    return;
                }

                if self.lcd_enable_pending_delay_tcycles == 0 && self.lcdc & LCDC_ENABLE_BIT != 0 {
                    self.enter_lcd_enabled_restart_state();
                    self.refresh_stat_irq_line(false);
                } else {
                    return;
                }
            } else {
                return;
            }
        }

        let step_region = self.current_step_region_after_line_advance();
        let previous_mode = observe_ppu_step_region(observer, step_region, || {
            self.advance_mode3_register_latches_from_mmio();
            let previous_mode = self.current_access_mode();
            self.startup_mode_latch = None;
            self.line_dot += 1;
            self.advance_lcd_restart_phase();
            self.prepare_visible_scanline_state();
            previous_mode
        });
        observe_ppu_step_region(observer, PpuStepRegion::Mode2Scan, || {
            self.advance_mode2_scan(&oam, dma_oam_active);
        });
        self.advance_mode3_pipeline(&oam, &vram, dma_oam_conflict, observer);

        observe_ppu_step_region(observer, step_region, || {
            if self.line_dot == self.current_scanline_length() {
                let wraps_to_frame_start = self.ly + 1 == TOTAL_SCANLINES;
                self.finalize_dmg_bgp_cpu_commit_scanline();
                if self.bg_pipeline_state.window_started_this_line {
                    self.window_state.window_line_counter =
                        self.window_state.window_line_counter.wrapping_add(1);
                }
                self.line_dot = 0;
                self.ly = if self.ly + 1 == TOTAL_SCANLINES {
                    0
                } else {
                    self.ly + 1
                };
                self.advance_lcd_restart_phase();
                if self.ly >= VISIBLE_SCANLINES {
                    self.window_state.reset();
                }
                self.mode2_scan_state.reset_scanline();
                self.bg_pipeline_state.reset();
                self.obj_pipeline_state.reset();
                self.dmg_bgp_cpu_commit_bg_visible_hold_palette_override = None;
                self.dmg_bgp_cpu_commit_bg_visible_hold_bg_pixels_remaining = 0;
                self.dmg_bgp_cpu_commit_bg_visible_hold_fallback_palette = None;
                self.dmg_bgp_cpu_commit_current_line_start_palette = self.bgp;
                self.dmg_bgp_cpu_commit_current_line_writes.clear();
                self.dmg_recent_panel_dots.clear();
                self.current_scanline_pixels.fill(0);
                self.current_scanline_mixed_pixels
                    .fill(MixedPixel::background(0));
                self.current_scanline_dmg_bg_forced_white.fill(false);
                if wraps_to_frame_start && self.blank_frame_active {
                    self.blank_frame_active = false;
                    self.refresh_visible_output();
                }
            }

            let current_mode = self.current_access_mode();
            if previous_mode != PpuAccessMode::VBlank && current_mode == PpuAccessMode::VBlank {
                self.queue_interrupt_request(InterruptSource::VBlank);
            }
            self.refresh_stat_irq_line(false);
        });
    }

    pub fn snapshot(&self) -> PpuSnapshot {
        let raster_state = self.current_raster_state();
        let mode = raster_state.access_mode();
        let current_transfer = self.current_transfer();
        let current_transfer_plan = current_transfer.map(Mode3CurrentTransfer::service_plan);
        let register_latches = self.mode3_register_latches();
        let visible_registers = register_latches.visible();
        let pipeline_registers = register_latches.pipeline();

        PpuSnapshot {
            console_model: self.console_model,
            status: self.status,
            lcdc: self.lcdc,
            stat_interrupt_enable: self.stat_interrupt_enable,
            lyc_coincidence: self.effective_lyc_coincidence(),
            stat_irq_line: self.stat_state.irq_line,
            blank_frame_active: self.blank_frame_active,
            lcd_state: self.lcd_state,
            visible_output: self.visible_output,
            mode,
            scy: self.scy,
            scx: self.scx,
            ly: self.ly,
            line_dot: self.line_dot,
            mode_dot: raster_state.mode_dot(),
            mode0_start_dot: self.current_mode0_start_dot(),
            current_oam_scan_row: self.current_mode2_oam_row(),
            mode2_scanned_entries: self.mode2_scan_state.scanned_entries(),
            selected_sprites: self.mode2_scan_state.selected_sprites_snapshot(),
            bg_fetcher_source: self.bg_pipeline_state.fetcher.source,
            bg_fetcher_stage: self.bg_pipeline_state.fetcher.stage,
            bg_fetcher_stage_dot: self.bg_pipeline_state.fetcher.stage_dot,
            bg_fetcher_tile_map_address: self.bg_pipeline_state.fetcher.tile_map_address,
            bg_fetcher_tile_data_address: self.bg_pipeline_state.fetcher.tile_data_address,
            bg_push_pending: self.bg_pipeline_state.push.pending,
            bg_push_disposition: snapshot_bg_push_disposition(
                self.bg_pipeline_state.push.disposition,
            ),
            bg_fill_pending: self.bg_pipeline_state.fill.pending,
            bg_fifo_pixels: self.bg_pipeline_state.fifo.iter().copied().collect(),
            bg_fifo_cached_pixels: self
                .bg_pipeline_state
                .fifo_cached_pixels
                .iter()
                .copied()
                .map(snapshot_bg_fifo_cached_pixel)
                .collect(),
            bg_startup_source_state: snapshot_bg_startup_source_state(
                self.bg_pipeline_state.startup_source_state,
            ),
            bg_startup_fetch_seam: snapshot_bg_startup_fetch_seam(
                self.bg_pipeline_state.startup_fetch_seam,
            ),
            bg_startup_fifo_placeholders: self.bg_pipeline_state.startup_fifo_placeholders,
            bg_push_entry_delay_remaining: self.bg_pipeline_state.push.entry_delay_remaining,
            bg_fill_startup_dummy_pixels: self.bg_pipeline_state.fill.startup_dummy_pixels,
            bg_fetcher_post_alignment_restart_delay_dots: self
                .bg_pipeline_state
                .fetcher
                .post_alignment_fetch_restart_delay_dots,
            bg_transfer_phase: snapshot_bg_transfer_phase(self.bg_pipeline_state.transfer_phase),
            bg_current_transfer_x: self.bg_pipeline_state.current_transfer_x,
            bg_current_transfer_lane: current_transfer
                .map(|transfer| snapshot_bg_transfer_lane(transfer.context.lane)),
            bg_current_transfer_source_window: current_transfer
                .map(|transfer| snapshot_bg_transfer_source_window(transfer.context.source_window)),
            bg_current_transfer_backing: current_transfer_plan
                .map(|plan| snapshot_bg_transfer_backing(plan.backing)),
            bg_current_transfer_readiness: current_transfer
                .map(|transfer| snapshot_bg_transfer_readiness(transfer.readiness)),
            bg_current_transfer_kind: current_transfer_plan
                .map(|plan| snapshot_bg_transfer_kind(plan.result_kind)),
            obj_fetcher_stage: self.obj_pipeline_state.fetch.stage,
            obj_fetcher_stage_dot: self.obj_pipeline_state.fetch.stage_dot,
            obj_pending_hit_match_x: self.obj_pipeline_state.pending_match_x,
            obj_pending_hit_len: self.obj_pipeline_state.pending_sprite_slots.len(),
            obj_pending_hit_front_sprite_slot: self
                .obj_pipeline_state
                .pending_sprite_slots
                .front()
                .copied(),
            obj_fetched_same_x_active_count: self
                .fetched_same_x_obj_sprite_count_for_active_fetch(),
            obj_fetched_same_x_pending_count: self
                .fetched_same_x_obj_sprite_count_for_pending_match_x(),
            obj_fifo_pixels: self
                .obj_pipeline_state
                .fifo
                .iter()
                .map(|pixel| (!pixel.is_transparent()).then_some(pixel.color))
                .collect(),
            scx_discard_remaining: self.bg_pipeline_state.scx_discard_remaining,
            visible_pixels_output: self.bg_pipeline_state.visible_pixels_output,
            window_wy_latch: self.bg_pipeline_state.window_wy_latch,
            window_started_this_line: self.bg_pipeline_state.window_started_this_line,
            window_line_counter: self.window_state.window_line_counter,
            current_scanline_mixed_colors: self
                .current_scanline_mixed_pixels
                .iter()
                .map(|pixel| pixel.color)
                .collect(),
            current_scanline_pixels: self.current_scanline_pixels.to_vec(),
            lyc: self.lyc,
            bgp: self.bgp,
            obp0: self.obp0,
            obp1: self.obp1,
            wy: self.wy,
            wx: self.wx,
            visible_lcdc: visible_registers.lcdc,
            visible_scy: visible_registers.scy,
            visible_scx: visible_registers.scx,
            visible_bgp: visible_registers.bgp,
            visible_obp0: visible_registers.obp0,
            visible_obp1: visible_registers.obp1,
            visible_wy: visible_registers.wy,
            visible_wx: visible_registers.wx,
            pipeline_lcdc: pipeline_registers.lcdc,
            pipeline_scy: pipeline_registers.scy,
            pipeline_scx: pipeline_registers.scx,
            pipeline_bgp: pipeline_registers.bgp,
            pipeline_obp0: pipeline_registers.obp0,
            pipeline_obp1: pipeline_registers.obp1,
            pipeline_wy: pipeline_registers.wy,
            pipeline_wx: pipeline_registers.wx,
            dmg_bgp_cpu_commit_output_palette_override: self
                .dmg_bgp_cpu_commit_output_palette_override,
            dmg_bgp_cpu_commit_output_delay_pixels_remaining: self
                .dmg_bgp_cpu_commit_output_delay_pixels_remaining,
            obj_palette_read_policy: self.obj_palette_read_policy,
        }
    }

    pub fn ly(&self) -> u8 {
        self.ly
    }

    pub fn line_dot(&self) -> u16 {
        self.line_dot
    }

    pub fn mode0_start_dot(&self) -> u16 {
        self.current_mode0_start_dot()
    }

    pub fn access_mode(&self) -> PpuAccessMode {
        self.current_access_mode()
    }

    pub fn mode_dot(&self) -> u16 {
        self.current_raster_state().mode_dot()
    }

    pub fn lcd_state(&self) -> PpuLcdState {
        self.lcd_state
    }

    pub fn is_blank_frame_active(&self) -> bool {
        self.blank_frame_active
    }

    pub fn is_restart_first_line_active(&self) -> bool {
        self.lcd_restart_phase
            .is_first_line_after_enable_active(self.ly)
    }

    pub fn is_startup_mode0_window_active(&self) -> bool {
        self.is_restart_first_line_active()
    }

    pub(super) fn current_scanline_length(&self) -> u16 {
        if self.is_restart_first_line_active() {
            LCD_REENABLE_LINE0_TOTAL_DOTS
        } else {
            DOTS_PER_SCANLINE
        }
    }

    pub(super) fn current_ly_read_advance_start_dot(&self) -> u16 {
        if self.is_restart_first_line_active() {
            LCD_REENABLE_LINE0_LY_READ_ADVANCE_START_DOT
        } else {
            LY_READ_ADVANCE_START_DOT
        }
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    pub(crate) fn set_system_stop_active(&mut self, active: bool) {
        if self.system_stop_active == active {
            return;
        }

        self.system_stop_active = active;
        if active {
            self.clear_visible_buffers();
        }
        self.refresh_visible_output();
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        let bg_fifo_front_cached = self
            .bg_pipeline_state
            .fifo_cached_pixels
            .iter()
            .flatten()
            .next()
            .copied();
        let current_transfer = self.current_transfer();
        let current_transfer_plan = current_transfer.map(Mode3CurrentTransfer::service_plan);
        format!(
            concat!(
                "t_cycle={} phase={} console_model={:?} status={:?} ",
                "lcd_state={:?} visible_output={:?} ly={} lyc={} coincidence={} ",
                "line_dot={} mode={:?} stat_irq_line={} mode2_scanned_entries={} selected_sprites={} ",
                "bg_source={:?} bg_stage={:?} bg_stage_dot={} bg_fetch_origin={:?} ",
                "bg_push_pending={} bg_push_entry_delay_remaining={} bg_push_origin={:?} ",
                "bg_fill_pending={} bg_fill_startup_dummy_pixels={} bg_fill_origin={:?} ",
                "bg_fifo_len={} bg_startup_fifo_placeholders={} bg_fifo_front_cached_origin={:?} ",
                "bg_fifo_front_cached_fetch_x={:?} bg_fifo_front_cached_pixel_index={:?} ",
                "bg_startup_source_state={:?} bg_startup_fetch_seam={:?} ",
                "bg_fetcher_post_alignment_restart_delay_dots={} bg_transfer_phase={:?} ",
                "bg_current_transfer_x={} bg_current_transfer_lane={:?} ",
                "bg_current_transfer_source_window={:?} bg_current_transfer_backing={:?} ",
                "bg_current_transfer_readiness={:?} bg_current_transfer_kind={:?} ",
                "visible_pixels_output={}"
            ),
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.lcd_state,
            self.visible_output,
            self.ly,
            self.lyc,
            self.effective_lyc_coincidence(),
            self.line_dot,
            self.current_access_mode(),
            self.stat_state.irq_line,
            self.mode2_scan_state.scanned_entries(),
            self.mode2_scan_state.selected_sprite_count(),
            self.bg_pipeline_state.fetcher.source,
            self.bg_pipeline_state.fetcher.stage,
            self.bg_pipeline_state.fetcher.stage_dot,
            self.bg_pipeline_state.fetcher.cached_origin,
            self.bg_pipeline_state.push.pending,
            self.bg_pipeline_state.push.entry_delay_remaining,
            self.bg_pipeline_state.push.cached.origin,
            self.bg_pipeline_state.fill.pending,
            self.bg_pipeline_state.fill.startup_dummy_pixels,
            self.bg_pipeline_state.fill.cached.origin,
            self.bg_pipeline_state.fifo.len(),
            self.bg_pipeline_state.startup_fifo_placeholders,
            bg_fifo_front_cached.map(|pixel| pixel.cached.origin),
            bg_fifo_front_cached.map(|pixel| pixel.cached.fetch_x),
            bg_fifo_front_cached.map(|pixel| pixel.pixel_index),
            self.bg_pipeline_state.startup_source_state,
            self.bg_pipeline_state.startup_fetch_seam,
            self.bg_pipeline_state
                .fetcher
                .post_alignment_fetch_restart_delay_dots,
            self.bg_pipeline_state.transfer_phase,
            self.bg_pipeline_state.current_transfer_x,
            current_transfer.map(|transfer| transfer.context.lane),
            current_transfer.map(|transfer| transfer.context.source_window),
            current_transfer_plan.map(|plan| plan.backing),
            current_transfer.map(|transfer| snapshot_bg_transfer_readiness(transfer.readiness)),
            current_transfer_plan.map(|plan| snapshot_bg_transfer_kind(plan.result_kind)),
            self.bg_pipeline_state.visible_pixels_output,
        )
    }

    pub fn mmio_commit_trace_message(
        &self,
        context: &CycleContext,
        address: u16,
        value: u8,
    ) -> String {
        format!(
            concat!(
                "t_cycle={} phase={} console_model={:?} status={:?} ",
                "committed_write={:#06X}<-{:#04X} lcdc={:#04X} stat={:#04X} ",
                "scy={:#04X} scx={:#04X} ly={} lyc={:#04X} bgp={:#04X} wy={:#04X} wx={:#04X}"
            ),
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            address,
            value,
            self.read_register(0xFF40),
            self.read_register(0xFF41),
            self.read_register(0xFF42),
            self.read_register(0xFF43),
            self.read_register(0xFF44),
            self.read_register(0xFF45),
            self.read_register(0xFF47),
            self.read_register(0xFF4A),
            self.read_register(0xFF4B),
        )
    }

    pub(crate) fn drain_pending_interrupt_requests(&mut self) -> Vec<InterruptSource> {
        let mut requests = Vec::with_capacity(2);
        if self.pending_interrupts & PPU_PENDING_VBLANK_INTERRUPT_BIT != 0 {
            requests.push(InterruptSource::VBlank);
        }
        if self.pending_interrupts & PPU_PENDING_LCD_STAT_INTERRUPT_BIT != 0 {
            requests.push(InterruptSource::LcdStat);
        }
        self.pending_interrupts = 0;
        requests
    }

    pub(crate) fn pending_interrupt_request_mask(&self) -> u8 {
        let mut mask = 0;
        if self.pending_interrupts & PPU_PENDING_VBLANK_INTERRUPT_BIT != 0 {
            mask |= 0x01;
        }
        if self.pending_interrupts & PPU_PENDING_LCD_STAT_INTERRUPT_BIT != 0 {
            mask |= 0x02;
        }
        mask
    }
}
