use super::snapshot::*;
use super::*;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct PpuMode3LiveBackgroundWriteRoute {
    register: PpuMode3LiveBackgroundRegister,
    write_context: PpuMode3LiveRegisterWriteContext,
    scy_routing: PpuMode3LiveScyWriteRouting,
    ly: u8,
}

impl Ppu {
    pub(crate) fn owns_mmio_register(address: u16) -> bool {
        PpuRegister::from_address(address).is_some()
    }

    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: PpuStatus::RegistersReady,
            lcdc: 0,
            stat_interrupt_enable: 0,
            lcd_state: PpuLcdState::Disabled,
            lcd_enable_pending_delay_tcycles: 0,
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
            runtime: PpuRuntimeState::default(),
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

    pub(crate) fn bus_state_snapshot(&self) -> PpuBusStateSnapshot {
        if !self.is_lcd_enabled() {
            let disabled = PpuBusState::lcd_disabled();
            return PpuBusStateSnapshot {
                owner: disabled,
                cpu_read: disabled,
                cpu_write: disabled,
            };
        }

        let owner_mode = self.current_bus_access_mode();
        let cpu_read_mode = self.current_published_bus_access_mode();
        let cpu_write_mode = self.current_published_video_write_access_mode();

        PpuBusStateSnapshot {
            owner: PpuBusState::lcd_enabled(owner_mode),
            cpu_read: PpuBusState::lcd_enabled(cpu_read_mode),
            cpu_write: PpuBusState::lcd_enabled(cpu_write_mode),
        }
    }

    #[cfg(test)]
    pub(crate) fn cpu_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_published_bus_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    #[cfg(test)]
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
        self.route_post_write_mode3_effects(register, value, previous_mmio_registers);

        if !self.is_lcd_enabled() {
            self.reload_mode3_register_latches_from_mmio();
        }
    }

    fn route_post_write_mode3_effects(
        &mut self,
        register: PpuRegister,
        value: u8,
        previous_mmio_registers: PpuVisibleRegisters,
    ) {
        self.route_live_wx_write(register, value, previous_mmio_registers);

        let Some(route) =
            self.build_mode3_live_background_write_route(register, previous_mmio_registers)
        else {
            return;
        };

        self.route_mode3_live_background_write(route);
    }

    fn route_live_wx_write(
        &mut self,
        register: PpuRegister,
        value: u8,
        previous_mmio_registers: PpuVisibleRegisters,
    ) {
        if register != PpuRegister::Wx || self.current_access_mode() != PpuAccessMode::Drawing {
            return;
        }

        self.maybe_arm_dmg_previsible_wx_retarget(previous_mmio_registers.wx, value);
        self.maybe_arm_dmg_live_wx_trigger_glitch(value);
    }

    fn build_mode3_live_background_write_route(
        &self,
        register: PpuRegister,
        previous_mmio_registers: PpuVisibleRegisters,
    ) -> Option<PpuMode3LiveBackgroundWriteRoute> {
        let register = PpuMode3LiveBackgroundRegister::from_register(register)?;
        if self.current_access_mode() != PpuAccessMode::Drawing {
            return None;
        }

        Some(PpuMode3LiveBackgroundWriteRoute {
            register,
            write_context: self.current_mode3_live_register_write_context(previous_mmio_registers),
            scy_routing: self.live_scy_write_routing(register),
            ly: self.ly,
        })
    }

    fn route_mode3_live_background_write(&mut self, route: PpuMode3LiveBackgroundWriteRoute) {
        self.route_live_background_write_to_pending_cached_slices(route);
        self.route_live_lcdc_write_to_observed_bg_seams(route);
        self.route_live_scy_write_to_startup_alignment_fifo(route);
        self.route_live_background_write_to_current_fetch(route);
        self.route_live_scx_boundary_write_effects(route);
    }

    fn route_live_background_write_to_pending_cached_slices(
        &mut self,
        route: PpuMode3LiveBackgroundWriteRoute,
    ) {
        if self.bg_pipeline_state.push.pending {
            let push_entry_delay_remaining = self.bg_pipeline_state.push.entry_delay_remaining > 0;
            self.bg_pipeline_state
                .push
                .cached
                .mark_live_register_write_while_push_pending(
                    route.register,
                    route.write_context,
                    push_entry_delay_remaining,
                    route.ly,
                    route.scy_routing,
                );
        }

        if self.bg_pipeline_state.fill.pending {
            let fill_includes_real_tile_pixels =
                self.bg_pipeline_state.fill.includes_real_tile_pixels;
            let fill_startup_dummy_pixels = self.bg_pipeline_state.fill.startup_dummy_pixels;
            self.bg_pipeline_state
                .fill
                .cached
                .mark_live_register_write_while_fill_pending(
                    route.register,
                    route.write_context,
                    fill_includes_real_tile_pixels,
                    fill_startup_dummy_pixels,
                    route.ly,
                    route.scy_routing,
                );
        }
    }

    fn route_live_lcdc_write_to_observed_bg_seams(
        &mut self,
        route: PpuMode3LiveBackgroundWriteRoute,
    ) {
        if !matches!(route.register, PpuMode3LiveBackgroundRegister::Lcdc) {
            return;
        }

        let fetcher = self.bg_pipeline_state.fetcher;
        let window_line_counter = self.current_window_line_counter();
        self.bg_pipeline_state
            .latch_window_activation_tilemap_select_if_unset(route.write_context);
        self.bg_pipeline_state
            .mark_live_lcdc3_write_while_fifo_visible(
                route.write_context,
                fetcher,
                window_line_counter,
            );
        self.bg_pipeline_state
            .apply_window_activation_tilemap_select_latch_to_seam_slices();
        self.apply_dmg_lcdc3_live_bg_tilemap_write(route.write_context);
        self.apply_dmg_lcdc4_live_bg_tiledata_write(route.write_context);
        self.apply_dmg_lcdc0_live_bg_enable_write(route.write_context);
        self.apply_dmg_lcdc1_live_obj_enable_write(route.write_context);
        self.apply_dmg_lcdc2_live_obj_size_write(route.write_context);
    }

    fn route_live_scy_write_to_startup_alignment_fifo(
        &mut self,
        route: PpuMode3LiveBackgroundWriteRoute,
    ) {
        if !matches!(route.register, PpuMode3LiveBackgroundRegister::Scy) {
            return;
        }

        self.bg_pipeline_state
            .mark_live_scy_write_while_startup_alignment_fifo_visible(
                route.write_context,
                route.ly,
            );
    }

    fn route_live_background_write_to_current_fetch(
        &mut self,
        route: PpuMode3LiveBackgroundWriteRoute,
    ) {
        let window_line_counter = self.current_window_line_counter();
        self.bg_pipeline_state
            .fetcher
            .mark_live_register_write_for_current_background_fetch(
                route.register,
                route.write_context,
                route.ly,
                window_line_counter,
                route.scy_routing,
            );
    }

    fn route_live_scx_boundary_write_effects(&mut self, route: PpuMode3LiveBackgroundWriteRoute) {
        if !matches!(route.register, PpuMode3LiveBackgroundRegister::Scx)
            || !route.write_context.bg_scx_tilemap_column_changed()
        {
            return;
        }

        self.route_live_scx_full_refetch_boundary_write(route.write_context);
        self.route_live_scx_old_pixel_window_boundary_write();
        self.route_live_scx_next_tile_output_retarget_boundary_write();
    }

    fn route_live_scx_full_refetch_boundary_write(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) {
        if !write_context.bg_scx_tilemap_column_changed()
            || !self.startup_visible_tile3_scx_boundary_full_refetch_needs_next_tile()
        {
            return;
        }

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

    fn route_live_scx_old_pixel_window_boundary_write(&mut self) {
        if !self.inactive_visible_tile3_scx_push_boundary_needs_old_pixel_window() {
            return;
        }

        let current_scx = self.scx;
        let visible_scx = self.runtime.visible_registers.scx;
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

        if (0x08..=0x0E).contains(&current_scx) && current_scx & 0x07 == 0x03 {
            self.bg_pipeline_state
                .push
                .cached
                .arm_startup_visible_tile3_scx_boundary_next_tile_output_retarget(visible_scx);
        }
    }

    fn route_live_scx_next_tile_output_retarget_boundary_write(&mut self) {
        if !self.inactive_visible_tile3_scx_push_boundary_needs_next_tile_output_retarget() {
            return;
        }

        let current_scx = self.scx;
        let visible_scx = self.runtime.visible_registers.scx;
        let scx_low_bits = current_scx & 0x07;
        self.bg_pipeline_state
            .push
            .cached
            .arm_startup_visible_tile3_scx_boundary_next_tile_output_retarget(current_scx);
        if scx_low_bits >= 0x03 {
            self.bg_pipeline_state
                .push
                .cached
                .arm_startup_visible_tile3_scx_boundary_old_tail(visible_scx, current_scx);
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
                .startup_visible_tile3_scx_boundary_previous_scx = Some(visible_scx);
            self.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_old_prefix_pixels = 1;
        }
        if current_scx >= 0x60 && scx_low_bits == 0x01 {
            self.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_previous_scx = Some(visible_scx);
            self.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_old_prefix_pixels = 2;
        }
        if current_scx >= 0x78 && matches!(scx_low_bits, 0x00..=0x02) {
            self.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_previous_scx = Some(visible_scx);
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
        if current_scx >= 0x60 && matches!(scx_low_bits, 0x03..=0x05) {
            self.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_previous_scx = Some(visible_scx);
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

    pub(super) fn live_scy_write_routing(
        &self,
        register: PpuMode3LiveBackgroundRegister,
    ) -> PpuMode3LiveScyWriteRouting {
        PpuMode3LiveScyWriteRouting {
            pending_high_plane_only: self.scy_pending_refetch_prefers_high_plane_only(register),
            pending_tilemap_row_refetch: self.scy_pending_refetch_prefers_tilemap_row(register),
            startup_visible_tile2_tilemap_row_refetch: self
                .scy_startup_visible_tile2_refetch_prefers_tilemap_row(register),
            startup_visible_tile2_phase6_tilemap_row_refetch: self
                .scy_startup_visible_tile2_phase6_refetch_prefers_tilemap_row(register),
        }
    }

    fn scy_pending_refetch_prefers_high_plane_only(
        &self,
        register: PpuMode3LiveBackgroundRegister,
    ) -> bool {
        if !matches!(register, PpuMode3LiveBackgroundRegister::Scy) {
            return false;
        }

        self.scy_obj_phase_policy()
            .is_some_and(PpuMode3ScyObjPhasePolicy::pending_refetch_prefers_high_plane_only)
    }

    fn scy_pending_refetch_prefers_tilemap_row(
        &self,
        register: PpuMode3LiveBackgroundRegister,
    ) -> bool {
        if !matches!(register, PpuMode3LiveBackgroundRegister::Scy) {
            return false;
        }

        self.scy_obj_phase_policy()
            .is_some_and(PpuMode3ScyObjPhasePolicy::pending_refetch_prefers_tilemap_row)
    }

    fn scy_startup_visible_tile2_refetch_prefers_tilemap_row(
        &self,
        register: PpuMode3LiveBackgroundRegister,
    ) -> bool {
        if !matches!(register, PpuMode3LiveBackgroundRegister::Scy) {
            return false;
        }

        self.scy_obj_phase_policy().is_some_and(
            PpuMode3ScyObjPhasePolicy::startup_visible_tile2_refetch_prefers_tilemap_row,
        )
    }

    fn scy_startup_visible_tile2_phase6_refetch_prefers_tilemap_row(
        &self,
        register: PpuMode3LiveBackgroundRegister,
    ) -> bool {
        if !matches!(register, PpuMode3LiveBackgroundRegister::Scy) {
            return false;
        }

        self.scy_obj_phase_policy().is_some_and(
            PpuMode3ScyObjPhasePolicy::startup_visible_tile2_phase6_refetch_prefers_tilemap_row,
        )
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
        self.runtime.reset_for_startup(startup_state.bgp);
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
                if self.bg_pipeline_state.window_start_count_this_line != 0 {
                    self.window_state.window_line_counter = self
                        .window_state
                        .window_line_counter
                        .wrapping_add(self.bg_pipeline_state.window_start_count_this_line);
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
                let bgp = self.bgp;
                self.panel.reset_for_scanline_start(bgp);
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
        let obj_fetcher_requested_sprite = self.obj_pipeline_state.fetch.sprite;
        let obj_fetcher_resolved_sprite = self.obj_pipeline_state.fetch.resolved_sprite;
        let obj_fetcher_resolved_tile = self
            .obj_pipeline_state
            .fetch
            .resolved_tile_index
            .zip(self.obj_pipeline_state.fetch.resolved_tile_row)
            .or_else(|| {
                obj_fetcher_resolved_sprite.and_then(|sprite| self.obj_tile_index_and_row(sprite))
            });
        let obj_fetcher_tile_low_address =
            obj_fetcher_resolved_tile.map(|(tile_index, tile_row)| {
                tile_index as u16 * TILE_BYTES + tile_row as u16 * TILE_ROW_BYTES
            });
        let obj_fetcher_tile_high_address = obj_fetcher_tile_low_address.map(|address| address + 1);

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
            bg_fetcher_tile_index: self.bg_pipeline_state.fetcher.tile_index,
            bg_fetcher_tile_low: self.bg_pipeline_state.fetcher.tile_low,
            bg_fetcher_tile_high: self.bg_pipeline_state.fetcher.tile_high,
            last_unsigned_tile_data_low_fetch: self.last_unsigned_tile_data_low_fetch,
            last_unsigned_tile_data_high_fetch: self.last_unsigned_tile_data_high_fetch,
            bg_push_pending: self.bg_pipeline_state.push.pending,
            bg_push_cached: snapshot_bg_cached_slice(self.bg_pipeline_state.push.cached),
            bg_push_disposition: snapshot_bg_push_disposition(
                self.bg_pipeline_state.push.disposition,
            ),
            bg_fill_pending: self.bg_pipeline_state.fill.pending,
            bg_fill_cached: snapshot_bg_cached_slice(self.bg_pipeline_state.fill.cached),
            bg_fifo_pixels: self.bg_pipeline_state.fifo.iter().copied().collect(),
            bg_fifo_cached_pixels: self
                .bg_pipeline_state
                .fifo
                .cached_slots()
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
            obj_fetcher_requested_sprite,
            obj_fetcher_resolved_sprite,
            obj_fetcher_selected_obj_height: self.obj_pipeline_state.fetch.selected_obj_height,
            obj_fetcher_latched_obj_height: self.obj_pipeline_state.fetch.latched_obj_height,
            obj_fetcher_resolved_tile_index: obj_fetcher_resolved_tile
                .map(|(tile_index, _)| tile_index),
            obj_fetcher_resolved_tile_row: obj_fetcher_resolved_tile.map(|(_, tile_row)| tile_row),
            obj_fetcher_tile_low_address,
            obj_fetcher_tile_high_address,
            obj_fetcher_tile_low: self.obj_pipeline_state.fetch.tile_low,
            obj_fetcher_tile_high: self.obj_pipeline_state.fetch.tile_high,
            obj_mode3_line_start_obj_height: self.obj_pipeline_state.mode3_line_start_obj_height,
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
            dmg_previsible_wx_retarget_trigger_x: self
                .bg_pipeline_state
                .dmg_window_restart
                .previsible_wx_retarget
                .and_then(|retarget| retarget.trigger_x),
            dmg_previsible_wx_retarget_window_pixel_offset: self
                .bg_pipeline_state
                .dmg_window_restart
                .previsible_wx_retarget
                .map(|retarget| retarget.window_pixel_offset),
            dmg_pending_previsible_wx_carry_next_trigger_x: self
                .bg_pipeline_state
                .dmg_window_restart
                .pending_previsible_wx_carry
                .map(|carry| carry.next_trigger_x),
            dmg_pending_previsible_wx_carry_end_trigger_x: self
                .bg_pipeline_state
                .dmg_window_restart
                .pending_previsible_wx_carry
                .map(|carry| carry.end_trigger_x),
            dmg_pending_previsible_wx_carry_next_window_pixel_offset: self
                .bg_pipeline_state
                .dmg_window_restart
                .pending_previsible_wx_carry
                .map(|carry| carry.next_window_pixel_offset),
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
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .output_palette_override,
            dmg_bgp_cpu_commit_output_delay_pixels_remaining: self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .output_delay_pixels_remaining,
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

    pub fn framebuffer_layer_sources(&self) -> &[PpuFramebufferLayerSource] {
        &self.framebuffer_layer_sources
    }

    pub fn framebuffer_bgwin_panel_shades(&self) -> &[u8] {
        &self.framebuffer_bgwin_panel_shades
    }

    pub fn framebuffer_backdrop_panel_shades(&self) -> &[u8] {
        &self.framebuffer_backdrop_panel_shades
    }

    pub fn framebuffer_bgwin_layer_sources(&self) -> &[PpuFramebufferLayerSource] {
        &self.framebuffer_bgwin_layer_sources
    }

    pub(super) fn framebuffer_layer_source_for_output_pixel(
        &self,
        visible_x: usize,
        output_pixel: MixedPixel,
    ) -> PpuFramebufferLayerSource {
        match output_pixel.source {
            MixedPixelSource::Object { .. } => PpuFramebufferLayerSource::Object,
            MixedPixelSource::Background => match self.current_scanline_bg_dot_contexts[visible_x]
                .map(|context| context.source)
                .unwrap_or(PpuBgFetcherSource::Background)
            {
                PpuBgFetcherSource::Background => PpuFramebufferLayerSource::Background,
                PpuBgFetcherSource::Window => PpuFramebufferLayerSource::Window,
            },
        }
    }

    pub(super) fn write_framebuffer_pixel(
        &mut self,
        row_start: usize,
        visible_x: usize,
        output_pixel: MixedPixel,
        panel_pixel: u8,
    ) {
        let framebuffer_index = row_start + visible_x;
        self.framebuffer[framebuffer_index] = panel_pixel;
        self.framebuffer_layer_sources[framebuffer_index] =
            self.framebuffer_layer_source_for_output_pixel(visible_x, output_pixel);
    }

    pub(super) fn write_bgwin_framebuffer_pixel(
        &mut self,
        row_start: usize,
        visible_x: usize,
        bg_pixel: u8,
        bg_enabled: bool,
    ) {
        let framebuffer_index = row_start + visible_x;
        let pixel = MixedPixel::background(bg_pixel);
        let forced_white = self.dmg_bg_panel_dot_is_forced_white(bg_enabled, pixel);
        let historical_bgp =
            self.mode3_register_latches()
                .pixel_pipeline_bgp(self.console_model, None, None);
        let panel_shade = if self.visible_output == PpuVisibleOutputState::Driving {
            if forced_white {
                0
            } else {
                self.map_mixed_pixel_to_panel_shade(pixel)
            }
        } else {
            0
        };
        let backdrop_panel_shade = if self.visible_output == PpuVisibleOutputState::Driving {
            self.apply_dmg_palette(historical_bgp, 0)
        } else {
            0
        };
        let source =
            match self.current_scanline_bg_dot_contexts[visible_x].map(|context| context.source) {
                Some(PpuBgFetcherSource::Background) => PpuFramebufferLayerSource::Background,
                Some(PpuBgFetcherSource::Window) => PpuFramebufferLayerSource::Window,
                None => PpuFramebufferLayerSource::Backdrop,
            };

        self.framebuffer_bgwin_colors[framebuffer_index] = bg_pixel;
        self.framebuffer_bgwin_forced_white[framebuffer_index] = forced_white;
        self.framebuffer_bgwin_panel_shades[framebuffer_index] = panel_shade;
        self.framebuffer_backdrop_panel_shades[framebuffer_index] = backdrop_panel_shade;
        self.framebuffer_bgwin_layer_sources[framebuffer_index] = source;
    }

    pub(super) fn recolor_bgwin_framebuffer_pixel_with_palette(
        &mut self,
        framebuffer_index: usize,
        palette: u8,
    ) {
        self.framebuffer_backdrop_panel_shades[framebuffer_index] =
            self.apply_dmg_palette(palette, 0);
        if self.framebuffer_bgwin_layer_sources[framebuffer_index]
            == PpuFramebufferLayerSource::Backdrop
            || self.framebuffer_bgwin_forced_white[framebuffer_index]
        {
            self.framebuffer_bgwin_panel_shades[framebuffer_index] = 0;
            return;
        }

        self.framebuffer_bgwin_panel_shades[framebuffer_index] =
            self.apply_dmg_palette(palette, self.framebuffer_bgwin_colors[framebuffer_index]);
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
        let bg_fifo_front_cached = self.bg_pipeline_state.fifo.first_cached();
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

    pub(crate) fn take_pending_interrupt_request_mask(&mut self) -> u8 {
        let requests = self.pending_interrupt_request_mask();
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
