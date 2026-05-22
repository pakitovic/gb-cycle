use super::*;

impl Ppu {
    pub(in crate::ppu) fn current_bus_access_mode(&self) -> PpuAccessMode {
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

    pub(in crate::ppu) fn current_cpu_visible_access_mode(&self) -> PpuAccessMode {
        if self.runtime.blank_frame_active && self.is_lcd_enabled() && self.line_dot != 0 {
            return self
                .lcd_restart_phase
                .raster_state(self.ly, self.line_dot - 1)
                .map(PpuRasterState::access_mode)
                .unwrap_or_else(|| self.current_access_mode());
        }

        self.current_access_mode()
    }

    pub(in crate::ppu) fn current_raster_state(&self) -> PpuRasterState {
        if !self.is_lcd_enabled() {
            return PpuRasterState::Disabled;
        }

        if let Some(raster_state) = self.lcd_restart_phase.raster_state(self.ly, self.line_dot) {
            return raster_state;
        }

        if self.runtime.startup_mode_latch.is_none() {
            if self.ly >= VISIBLE_SCANLINES {
                return PpuRasterState::Active {
                    mode: PpuAccessMode::VBlank,
                    mode_dot: self.line_dot,
                };
            }

            if self.line_dot < MODE2_DOTS {
                return PpuRasterState::Active {
                    mode: PpuAccessMode::OamScan,
                    mode_dot: self.line_dot,
                };
            }
        }

        let mode0_start_dot = self.current_mode0_start_dot();
        let mode = self
            .runtime
            .startup_mode_latch
            .unwrap_or_else(|| access_mode_from_raster(self.ly, self.line_dot, mode0_start_dot));

        PpuRasterState::Active {
            mode,
            mode_dot: mode_dot_from_raster_mode(mode, self.line_dot, mode0_start_dot),
        }
    }

    pub(in crate::ppu) fn current_mode0_start_dot(&self) -> u16 {
        if self.ly >= VISIBLE_SCANLINES {
            return MODE0_START_DOT;
        }
        if !self.runtime.bg_pipeline_state.mode3_started {
            return self.baseline_mode0_start_dot();
        }

        let line_timing_policy = self.mode3_line_timing_policy();
        let selected_sprite_count = self.runtime.mode2_scan_state.selected_sprite_count();
        let baseline_mode0_start_dot = line_timing_policy.baseline_mode0_start_dot();
        let all_selected_sprites_offscreen_right = self.runtime.bg_pipeline_state.mode0_start_dot
            == baseline_mode0_start_dot
            && self.obj_enabled()
            && selected_sprite_count > 0
            && (0..selected_sprite_count).all(|slot| {
                self.runtime
                    .mode2_scan_state
                    .selected_sprite(slot)
                    .is_some_and(|sprite| sprite.x >= 168)
            });
        let base_mode0_start_dot = self
            .runtime
            .bg_pipeline_state
            .mode0_start_dot
            .saturating_sub(u16::from(
                self.runtime.bg_pipeline_state.mode0_start_dot == baseline_mode0_start_dot
                    && self.obj_enabled()
                    && selected_sprite_count > 0
                    && all_selected_sprites_offscreen_right,
            ));
        if self.line_dot.saturating_add(1) < base_mode0_start_dot {
            return base_mode0_start_dot;
        }

        let obj_fetch_active =
            self.runtime.obj_pipeline_state.fetch.stage != PpuObjFetcherStage::Idle;
        let pending_obj_hit_owns_current_transfer_x =
            self.runtime.obj_pipeline_state.pending_match_x
                == Some(self.runtime.bg_pipeline_state.current_transfer_x)
                && !self
                    .runtime
                    .obj_pipeline_state
                    .pending_sprite_slots
                    .is_empty();
        let live_transfer_still_owned_by_mode3 =
            if obj_fetch_active || pending_obj_hit_owns_current_transfer_x {
                false
            } else {
                self.current_transfer().is_some()
            };
        let saturated_placeholder_tail_still_owned_by_mode3 = if obj_fetch_active
            || pending_obj_hit_owns_current_transfer_x
            || live_transfer_still_owned_by_mode3
        {
            false
        } else {
            self.saturated_placeholder_backed_terminal_bg_tail_still_owned_by_mode3()
        };
        line_timing_policy.current_mode0_start_dot(PpuMode3LineTimingContext {
            line_dot: self.line_dot,
            selected_sprite_count,
            all_selected_sprites_offscreen_right,
            obj_fetch_active,
            pending_obj_hit_owns_current_transfer_x,
            live_transfer_still_owned_by_mode3,
            saturated_placeholder_tail_still_owned_by_mode3,
        })
    }

    pub(in crate::ppu) fn baseline_mode0_start_dot(&self) -> u16 {
        self.mode3_line_timing_policy().baseline_mode0_start_dot()
    }

    pub(in crate::ppu) fn saturated_placeholder_backed_terminal_bg_tail_still_owned_by_mode3(
        &self,
    ) -> bool {
        let terminal_bg_tail_has_unfinished_fetch_work =
            matches!(
                self.runtime.bg_pipeline_state.fetcher.stage,
                PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
            ) || (self.runtime.bg_pipeline_state.push.pending
                && (self.runtime.bg_pipeline_state.push.entry_delay_remaining > 0
                    || self
                        .runtime
                        .bg_pipeline_state
                        .push
                        .terminal_placeholder_tail_extra_hold_remaining
                        > 0));
        self.runtime.bg_pipeline_state.mode3_started
            && self.runtime.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.runtime.bg_pipeline_state.current_transfer_x >= 168
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders > 0
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
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

        self.runtime
            .oam_corruption_controller
            .apply(self.console_model, row, event, oam_bytes)
    }

    pub(in crate::ppu) fn advance_mode2_scan(
        &mut self,
        oam: &OamBusView<'_>,
        dma_oam_active: bool,
    ) {
        if !self.mode2_scan_tick_due() {
            return;
        }

        let oam_index = self.runtime.mode2_scan_state.scanned_entries();
        self.runtime.mode2_scan_state.increment_scanned_entries();

        if self.runtime.mode2_scan_state.is_full() {
            return;
        }

        let nominal_sprite = read_oam_sprite(oam, oam_index);
        let sprite = if dma_oam_active && self.console_model.is_dmg_family() {
            let Some((y, x)) = self.runtime.mode2_scan_state.latched_mode2_yx_word() else {
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
            self.runtime
                .mode2_scan_state
                .latch_mode2_yx_word(sprite.y, sprite.x);
            sprite
        };

        if sprite_matches_line(sprite, self.ly, self.current_obj_height()) {
            self.runtime.mode2_scan_state.push(sprite);
        }
    }

    pub(in crate::ppu) fn mode2_scan_tick_due(&self) -> bool {
        self.is_lcd_enabled()
            && self.ly < VISIBLE_SCANLINES
            && self
                .lcd_restart_phase
                .raster_state(self.ly, self.line_dot)
                .is_none()
            && self.line_dot != 0
            && self.line_dot <= MODE2_DOTS
            && self.line_dot.is_multiple_of(MODE2_T_CYCLES_PER_OAM_ENTRY)
            && self.runtime.mode2_scan_state.scanned_entries() < OAM_SPRITE_COUNT
    }

    pub(in crate::ppu) fn current_obj_height(&self) -> u8 {
        self.mode3_register_latches().current_obj_height()
    }

    pub(in crate::ppu) fn visible_palette_register_value(
        &self,
        register: PpuPaletteRegister,
    ) -> u8 {
        self.mode3_register_latches()
            .visible()
            .palette_register(register, self.obj_palette_read_policy)
    }

    pub(in crate::ppu) fn write_palette_register_storage(
        &mut self,
        register: PpuPaletteRegister,
        value: u8,
    ) {
        match register {
            PpuPaletteRegister::Bgp => self.bgp = value,
            PpuPaletteRegister::Obp0 => self.obp0 = Some(value),
            PpuPaletteRegister::Obp1 => self.obp1 = Some(value),
        }
    }

    pub(in crate::ppu) fn window_activation_registers(&self) -> PpuVisibleRegisters {
        let register_latches = self.mode3_register_latches();
        let mut registers = register_latches.window_activation_registers(self.console_model);
        if self.console_model.is_dmg_family()
            && self.runtime.bg_pipeline_state.visible_pixels_output == 0
            && !self.runtime.bg_pipeline_state.window_started_this_line
            && register_latches.visible().wx < 8
            && register_latches.visible().wx != register_latches.pipeline().wx
        {
            registers.wx = register_latches.visible().wx;
        }
        if self.console_model.is_dmg_family()
            && self
                .runtime
                .bg_pipeline_state
                .dmg_window_restart
                .previsible_wx_cancel_uses_visible_wx_once
        {
            registers.wx = register_latches.visible().wx;
        }
        registers
    }

    pub(in crate::ppu) fn window_activation_state(&self) -> PpuMode3WindowActivationState {
        let registers = self.window_activation_registers();
        let window_enabled = if self.console_model.is_cgb_family() {
            self.runtime.bg_pipeline_state.window_lcdc5_latch
        } else {
            registers.window_enabled()
        };

        PpuMode3WindowActivationState::new(
            registers,
            window_enabled,
            self.runtime.bg_pipeline_state.window_force_x0_this_line,
        )
    }

    pub(in crate::ppu) fn mode3_window_policy(&self) -> PpuMode3WindowPolicy {
        let visible_registers = self.mode3_register_latches().visible();
        let fetcher_window_enabled = if self.console_model.is_cgb_family() {
            self.runtime.bg_pipeline_state.window_lcdc5_latch
        } else {
            visible_registers.window_enabled()
        };

        PpuMode3WindowPolicy::new(
            visible_registers,
            self.window_activation_state(),
            fetcher_window_enabled,
            self.runtime.bg_pipeline_state.window_wy_latch,
            self.runtime.bg_pipeline_state.window_started_this_line,
        )
    }

    pub(in crate::ppu) fn mode3_transfer_policy(&self) -> PpuMode3TransferPolicy {
        PpuMode3TransferPolicy::from_pipeline_state(&self.runtime.bg_pipeline_state, self.line_dot)
    }

    pub(in crate::ppu) fn startup_visible_tile3_scx_boundary_full_refetch_needs_next_tile(
        &self,
    ) -> bool {
        self.operating_mode.uses_dmg_software_contract()
            && self.runtime.bg_pipeline_state.fetcher.source == PpuBgFetcherSource::Background
            && matches!(
                self.runtime.bg_pipeline_state.fetcher.cached_origin,
                BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
            )
            && self.runtime.bg_pipeline_state.fetcher.stage == PpuBgFetcherStage::TileDataHigh
            && self.runtime.bg_pipeline_state.fetcher.stage_dot == 0
            && self.runtime.bg_pipeline_state.current_transfer_x == 16
            && self.runtime.bg_pipeline_state.visible_pixels_output == 8
            && matches!(
                self.runtime.bg_pipeline_state.startup_fetch_seam,
                BgStartupFetchSeamState::PostAlignment {
                    next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
                    startup_continuation_visible_tiles_remaining: 1,
                    delayed_background_tileindex_read_tiles_remaining: 0,
                    delayed_background_tilemap_tiles_remaining: 0,
                    delayed_background_tiledata_tiles_remaining: 0,
                    ..
                }
            )
    }

    pub(in crate::ppu) fn inactive_visible_tile3_scx_push_boundary_needs_old_pixel_window(
        &self,
    ) -> bool {
        let expected_visible_tile2_front_pixel = self
            .runtime
            .bg_pipeline_state
            .current_transfer_x
            .saturating_sub(16);
        self.operating_mode.uses_dmg_software_contract()
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.cached.source == PpuBgFetcherSource::Background
            && matches!(
                self.runtime.bg_pipeline_state.push.cached.origin,
                BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
            )
            && self.runtime.bg_pipeline_state.push.cached.fetch_x == BG_TILE_WIDTH as u16 * 2
            && self.runtime.bg_pipeline_state.fetcher.stage == PpuBgFetcherStage::Push
            && self.runtime.bg_pipeline_state.fetcher.stage_dot == 0
            && (18..=21).contains(&self.runtime.bg_pipeline_state.current_transfer_x)
            && self.runtime.bg_pipeline_state.visible_pixels_output
                == self
                    .runtime
                    .bg_pipeline_state
                    .current_transfer_x
                    .saturating_sub(8)
            && matches!(
                self.runtime.bg_pipeline_state.startup_fetch_seam,
                BgStartupFetchSeamState::Inactive
            )
            && self
                .runtime
                .bg_pipeline_state
                .fifo
                .cached_front()
                .flatten()
                .is_some_and(|cached| {
                    matches!(
                        cached.cached.origin,
                        BgCachedSliceOrigin::StartupContinuation(
                            BgStartupContinuationSlice::VisibleTile2
                        )
                    ) && cached.pixel_index == expected_visible_tile2_front_pixel
                })
    }

    pub(in crate::ppu) fn inactive_visible_tile3_scx_push_boundary_needs_next_tile_output_retarget(
        &self,
    ) -> bool {
        let expected_visible_tile2_front_pixel = self
            .runtime
            .bg_pipeline_state
            .current_transfer_x
            .saturating_sub(16);
        self.operating_mode.uses_dmg_software_contract()
            && self.scx >= 0x58
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.cached.source == PpuBgFetcherSource::Background
            && matches!(
                self.runtime.bg_pipeline_state.push.cached.origin,
                BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
            )
            && self.runtime.bg_pipeline_state.push.cached.fetch_x == BG_TILE_WIDTH as u16 * 2
            && self.runtime.bg_pipeline_state.fetcher.stage == PpuBgFetcherStage::Push
            && self.runtime.bg_pipeline_state.fetcher.stage_dot == 0
            && self.runtime.bg_pipeline_state.current_transfer_x == 22
            && self.runtime.bg_pipeline_state.visible_pixels_output == 14
            && matches!(
                self.runtime.bg_pipeline_state.startup_fetch_seam,
                BgStartupFetchSeamState::Inactive
            )
            && self
                .runtime
                .bg_pipeline_state
                .fifo
                .cached_front()
                .flatten()
                .is_some_and(|cached| {
                    matches!(
                        cached.cached.origin,
                        BgCachedSliceOrigin::StartupContinuation(
                            BgStartupContinuationSlice::VisibleTile2
                        )
                    ) && cached.pixel_index == expected_visible_tile2_front_pixel
                })
    }

    pub(in crate::ppu) fn mode3_line_timing_policy(&self) -> PpuMode3LineTimingPolicy {
        PpuMode3LineTimingPolicy::new(
            self.mode3_register_latches().visible(),
            self.runtime.bg_pipeline_state.mode3_started,
            self.runtime.bg_pipeline_state.mode0_start_dot,
        )
    }

    pub(in crate::ppu) fn mode3_bgwin_fetch_policy(&self) -> PpuMode3BgWinFetchPolicy {
        PpuMode3BgWinFetchPolicy::new(
            self.mode3_register_latches(),
            self.console_model,
            self.runtime
                .bg_pipeline_state
                .startup_background_tilemap_uses_pipeline_snapshot(),
            self.runtime
                .bg_pipeline_state
                .startup_background_tiledata_uses_pipeline_snapshot(),
            self.runtime
                .bg_pipeline_state
                .startup_background_tileindex_reads_on_stage_one(),
            self.runtime
                .bg_pipeline_state
                .fetcher
                .same_cycle_window_tilemap_lcdc_hold,
        )
    }

    pub(in crate::ppu) fn background_fetch_context(
        &self,
        next_fetch_pixel: u16,
    ) -> PpuMode3BackgroundFetchContext {
        self.mode3_bgwin_fetch_policy()
            .background_fetch_context(next_fetch_pixel, self.ly)
    }

    pub(in crate::ppu) fn window_fetch_context(&self) -> PpuMode3WindowFetchContext {
        self.mode3_bgwin_fetch_policy().window_fetch_context(
            self.current_window_line_counter(),
            self.runtime.bg_pipeline_state.fetcher.window_tilemap_x,
        )
    }
}
