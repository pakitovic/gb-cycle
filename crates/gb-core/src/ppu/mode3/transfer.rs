use super::*;

impl Ppu {
    pub(in crate::ppu) fn execute_transfer_service_plan(
        &mut self,
        plan: Mode3TransferServicePlan,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
        if self.console_model.is_dmg_family() {
            self.apply_pending_dmg_window_lcdc4_output_repaint(vram);
        }
        let pixel = self.take_transfer_service_bg_pixel(plan);
        self.begin_transfer_service_execution(plan);
        self.execute_transfer_service_execution(plan, pixel, vram)
    }

    fn take_transfer_service_bg_pixel(&mut self, plan: Mode3TransferServicePlan) -> Option<u8> {
        if matches!(
            plan.execution,
            Mode3TransferServiceExecution::EmitVisiblePixel
        ) {
            None
        } else if plan.requires_real_bg_fifo_pixel() {
            self.runtime.bg_pipeline_state.pop_real_fifo_pixel()
        } else if plan.requires_effective_bg_fifo_pixel() {
            self.runtime
                .bg_pipeline_state
                .consume_effective_fifo_pixel()
        } else {
            None
        }
    }

    fn begin_transfer_service_execution(&mut self, plan: Mode3TransferServicePlan) {
        self.runtime.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        if !matches!(
            plan.execution,
            Mode3TransferServiceExecution::ConsumeScxDiscard
                | Mode3TransferServiceExecution::EmitVisiblePixel
        ) {
            self.runtime
                .bg_pipeline_state
                .consume_startup_pre_visible_transfer_dot();
        }
    }

    fn execute_transfer_service_execution(
        &mut self,
        plan: Mode3TransferServicePlan,
        pixel: Option<u8>,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
        match plan.execution {
            Mode3TransferServiceExecution::ConsumeScxDiscard => {
                self.execute_transfer_scx_discard(plan, pixel)
            }
            Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop => {
                self.execute_transfer_previsible_bg_pop(plan, pixel)
            }
            Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop => {
                self.execute_transfer_hidden_bg_and_obj_pop(plan, pixel)
            }
            Mode3TransferServiceExecution::EmitVisiblePixel => {
                self.execute_transfer_visible_pixel(plan, vram)
            }
        }
    }

    fn execute_transfer_scx_discard(
        &mut self,
        plan: Mode3TransferServicePlan,
        pixel: Option<u8>,
    ) -> Mode3TransferDot {
        let _ = pixel
            .expect("startup scx discard must consume one effective BG FIFO slot before output");
        self.runtime.bg_pipeline_state.scx_discard_remaining -= 1;
        Mode3TransferDot::served(plan.result_kind, true)
    }

    fn execute_transfer_previsible_bg_pop(
        &mut self,
        plan: Mode3TransferServicePlan,
        pixel: Option<u8>,
    ) -> Mode3TransferDot {
        let _ =
            pixel.expect("pre-visible startup transfer must consume one effective BG FIFO slot");
        self.runtime.bg_pipeline_state.current_transfer_x += 1;
        Mode3TransferDot::served(plan.result_kind, false)
    }

    fn execute_transfer_hidden_bg_and_obj_pop(
        &mut self,
        plan: Mode3TransferServicePlan,
        pixel: Option<u8>,
    ) -> Mode3TransferDot {
        let _ = pixel.expect("hidden transfer must consume one effective BG FIFO slot");
        self.runtime.bg_pipeline_state.current_transfer_x += 1;
        let _ = self.pop_obj_fifo_pixel();
        Mode3TransferDot::served(plan.result_kind, false)
    }

    fn execute_transfer_visible_pixel(
        &mut self,
        plan: Mode3TransferServicePlan,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
        let bg_pixel = self
            .pop_visible_bg_fifo_pixel(vram)
            .expect("visible transfer plans must carry a BG pixel");
        let bg_enabled = self.pixel_transfer_bg_enabled();
        let visible_x = self.runtime.bg_pipeline_state.visible_pixels_output;
        let bg_pixel = if let Some(override_color) =
            self.compute_startup_visible_tile2_scy_placeholder_pixel(visible_x, vram)
        {
            BgOutputPixel::new(override_color, None)
        } else {
            bg_pixel
        };
        let dmg_family = self.console_model.is_dmg_family();
        let effective_bg_priority_pixel = if bg_enabled { bg_pixel.color } else { 0 };
        let obj_pixel = self.pop_obj_fifo_pixel();
        let obj_pixel = if dmg_family {
            self.apply_dmg_lcdc2_live_obj_size_output_override(obj_pixel, visible_x, vram)
        } else {
            obj_pixel
        };
        let output_pixel = self.mix_bg_and_obj(
            bg_pixel.color,
            bg_pixel.cgb_bg_attrs,
            effective_bg_priority_pixel,
            obj_pixel,
        );
        let dmg_bg_forced_white =
            dmg_family && self.dmg_bg_panel_dot_is_forced_white(bg_enabled, output_pixel);
        let panel_pixel = if self.runtime.panel.visible_output == PpuVisibleOutputState::Driving {
            if dmg_bg_forced_white {
                0
            } else {
                self.map_mixed_pixel_to_panel_shade(output_pixel)
            }
        } else {
            0
        };
        let scanline_pixel = if self.runtime.panel.visible_output == PpuVisibleOutputState::Driving
            && !dmg_bg_forced_white
        {
            output_pixel.color
        } else {
            0
        };
        let visible_x_index = visible_x as usize;
        self.runtime.panel.current_scanline_bg_pixels[visible_x_index] = bg_pixel.color;
        self.write_bgwin_framebuffer_pixel(
            self.ly as usize * SCREEN_WIDTH,
            visible_x_index,
            bg_pixel.color,
            bg_enabled,
        );
        self.runtime.panel.current_scanline_mixed_pixels[visible_x_index] = output_pixel;
        self.runtime.panel.current_scanline_dmg_bg_forced_white[visible_x_index] =
            dmg_bg_forced_white;
        self.runtime.panel.current_scanline_pixels[visible_x_index] = scanline_pixel;
        self.write_framebuffer_pixel(
            self.ly as usize * SCREEN_WIDTH,
            visible_x_index,
            output_pixel,
            panel_pixel,
        );
        if dmg_family {
            self.record_dmg_recent_panel_dot(
                visible_x_index as u8,
                output_pixel,
                dmg_bg_forced_white,
            );
            self.apply_dmg_wx0_window_disable_prefix_override(visible_x_index, bg_pixel.color);
            self.apply_dmg_late_window_enable_override_repaint_up_to(visible_x_index + 1, vram);
            self.consume_dmg_lcdc0_bg_enable_visible_hold();
            self.consume_dmg_lcdc1_obj_enable_visible_hold();
            self.consume_dmg_bgp_cpu_commit_bg_visible_hold(output_pixel);
        }
        self.runtime.bg_pipeline_state.current_transfer_x = self
            .runtime
            .bg_pipeline_state
            .current_transfer_x
            .saturating_add(1);
        self.runtime.bg_pipeline_state.visible_pixels_output += 1;
        Mode3TransferDot::served(plan.result_kind, false)
    }

    pub(in crate::ppu) fn pop_visible_bg_fifo_pixel(
        &mut self,
        vram: &VramBusView<'_>,
    ) -> Option<BgOutputPixel> {
        let visible_x = self.runtime.bg_pipeline_state.visible_pixels_output as usize;
        let mut pixel = self.runtime.bg_pipeline_state.pop_visible_fifo_pixel()?;
        let mut cgb_bg_attrs = pixel.cgb_bg_attrs();
        if self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_background_override_onset_x
            .is_some_and(|onset_x| self.runtime.bg_pipeline_state.visible_pixels_output >= onset_x)
        {
            self.runtime.panel.current_scanline_bg_dot_contexts[visible_x] = None;
            return Some(BgOutputPixel::new(
                self.dmg_bg_color_for_panel_shade(0),
                None,
            ));
        }
        let Some(cached) = pixel.cached.as_mut() else {
            self.runtime.panel.current_scanline_bg_dot_contexts[visible_x] = None;
            if let Some(override_pixel) = self.compute_startup_visible_tile2_scy_placeholder_pixel(
                self.runtime.bg_pipeline_state.visible_pixels_output,
                vram,
            ) {
                return Some(BgOutputPixel::new(override_pixel, None));
            }
            return Some(BgOutputPixel::new(pixel.color, cgb_bg_attrs));
        };
        cgb_bg_attrs = cached.cached.cgb_bg_attrs;
        let visible_tile2_scy_tilemap_override = self
            .compute_startup_visible_tile2_scy_tilemap_retarget_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let window_activation_tilemap_override = self.compute_window_activation_tilemap_override(
            cached.cached,
            cached.pixel_index,
            vram,
        );
        let window_tiledata_selector_override = self
            .compute_window_lcdc4_tiledata_selector_override(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let next_tile_output_retarget = self
            .compute_startup_visible_tile3_scx_boundary_next_tile_output_retarget_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let old_pixel_override = self.compute_startup_visible_tile3_scx_boundary_old_pixel(
            cached.cached,
            cached.pixel_index,
            vram,
        );
        let low_band_shifted_override = self
            .compute_startup_visible_tile3_scx_low_band_shifted_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let visible_tile2_previous_row_override = self
            .compute_startup_visible_tile2_previous_row_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let visible_tile3_previous_row_override = self
            .compute_startup_visible_tile3_previous_row_pixel(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let Some(recomputed) = recompute_live_background_cached_slice(
            cached.cached,
            vram,
            self.current_mode3_live_background_refetch_context(),
        ) else {
            self.runtime.panel.current_scanline_bg_dot_contexts[visible_x] =
                Some(PpuRecentBgDotContext {
                    source: cached.cached.source,
                    fetch_x: cached.cached.fetch_x,
                    pixel_index: cached.pixel_index,
                    tile_index: cached.cached.tile_index,
                });
            let color = old_pixel_override
                .or(window_activation_tilemap_override)
                .or(window_tiledata_selector_override)
                .or(low_band_shifted_override)
                .or(visible_tile2_scy_tilemap_override)
                .or(visible_tile2_previous_row_override)
                .or(visible_tile3_previous_row_override)
                .or(next_tile_output_retarget)
                .unwrap_or(pixel.color);
            return Some(BgOutputPixel::new(color, cgb_bg_attrs));
        };

        cached.cached = recomputed;
        cgb_bg_attrs = cached.cached.cgb_bg_attrs;
        self.runtime.panel.current_scanline_bg_dot_contexts[visible_x] =
            Some(PpuRecentBgDotContext {
                source: cached.cached.source,
                fetch_x: cached.cached.fetch_x,
                pixel_index: cached.pixel_index,
                tile_index: cached.cached.tile_index,
            });
        pixel.color = old_pixel_override
            .or(window_activation_tilemap_override)
            .or(window_tiledata_selector_override)
            .or(low_band_shifted_override)
            .or(visible_tile2_scy_tilemap_override)
            .or(visible_tile2_previous_row_override)
            .or(visible_tile3_previous_row_override)
            .or(next_tile_output_retarget)
            .unwrap_or_else(|| recomputed.pixel_value(cached.pixel_index));
        Some(BgOutputPixel::new(pixel.color, cgb_bg_attrs))
    }

    pub(in crate::ppu) fn compute_startup_visible_tile2_scy_placeholder_pixel(
        &self,
        visible_x: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.runtime.bg_pipeline_state.startup_scy_tiledata_latch?;

        let sprite_phase = self.scy_obj_phase_policy()?;
        if !sprite_phase
            .startup_visible_tile2_placeholder_uses_previous_tilemap_row(self.ly, visible_x)
        {
            return None;
        }

        let registers = self.mode3_register_latches().visible();
        let tile_map_base = if registers.lcdc & LCDC_BG_TILE_MAP_BIT != 0 {
            0x1C00
        } else {
            0x1800
        };
        let tile_map_row =
            ((self.ly.wrapping_add(self.scy) / BG_TILE_WIDTH) as u16).wrapping_sub(1) & 0x1F;
        let tile_map_column = u16::from(visible_x / BG_TILE_WIDTH);
        let tile_map_address = tile_map_base + tile_map_row * 32 + tile_map_column;
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let tile_data_base = bg_tile_data_base(registers.lcdc, tile_index);
        let tile_low_address = tile_data_base + 7 * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(
            tile_low,
            tile_high,
            visible_x & (BG_TILE_WIDTH - 1),
        ))
    }
    pub(in crate::ppu) fn compute_startup_visible_tile2_scy_tilemap_retarget_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.runtime.bg_pipeline_state.startup_scy_tiledata_latch?;

        if !matches!(
            cached.origin,
            BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
        ) || cached.needs_live_tilemap_refetch
            || cached.needs_live_tile_data_refetch
        {
            return None;
        }

        let retarget = self
            .scy_obj_phase_policy()?
            .startup_visible_tile2_tilemap_retarget(self.ly, pixel_index)?;

        Some(self.read_startup_visible_tile2_scy_retargeted_pixel(
            cached,
            pixel_index,
            retarget.tilemap_row_delta,
            retarget.tiledata_row_delta,
            vram,
        ))
    }

    fn read_startup_visible_tile2_scy_retargeted_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        tilemap_row_delta: i8,
        tiledata_row_delta: i8,
        vram: &VramBusView<'_>,
    ) -> u8 {
        let tile_map_offset = cached.tile_map_address & 0x03FF;
        let tile_map_base = cached.tile_map_address & !0x03FF;
        let tile_map_row =
            ((tile_map_offset / 32) as i16 + i16::from(tilemap_row_delta)).rem_euclid(32) as u16;
        let tile_map_column = tile_map_offset & 0x1F;
        let tile_map_address = tile_map_base + tile_map_row * 32 + tile_map_column;
        let tile_index = vram
            .read(tile_map_address as usize)
            .unwrap_or(cached.tile_index);

        let registers = self.mode3_register_latches().visible();
        let cached_tile_data_base = bg_tile_data_base(registers.lcdc, cached.tile_index);
        let tile_data_row = (((cached.tile_high_address - cached_tile_data_base) / TILE_ROW_BYTES)
            as i16
            + i16::from(tiledata_row_delta))
        .rem_euclid(8) as u16;
        let tile_data_base = bg_tile_data_base(registers.lcdc, tile_index);
        let tile_low_address = tile_data_base + tile_data_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        bg_tile_pixel_value(tile_low, tile_high, pixel_index)
    }

    pub(in crate::ppu) fn current_transfer_selected_sprite_x(&self) -> Option<u8> {
        let current_transfer_x = self.runtime.bg_pipeline_state.current_transfer_x;
        (0..self.runtime.mode2_scan_state.selected_sprite_count())
            .filter(|&slot| !self.runtime.obj_pipeline_state.has_fetched(slot))
            .filter_map(|slot| self.runtime.mode2_scan_state.selected_sprite(slot))
            .find(|sprite| sprite_trigger_x(*sprite) == Some(current_transfer_x))
            .map(|sprite| sprite.x)
    }

    pub(in crate::ppu) fn startup_line_lead_sprite_x(&self) -> Option<u8> {
        (0..self.runtime.mode2_scan_state.selected_sprite_count())
            .filter_map(|slot| self.runtime.mode2_scan_state.selected_sprite(slot))
            .min_by_key(|sprite| sprite.x)
            .map(|sprite| sprite.x)
    }

    pub(in crate::ppu) fn scy_startup_line_lead_owner_window_open(&self) -> bool {
        self.current_transfer().is_some()
            || self.runtime.bg_pipeline_state.mode3_started
                && !matches!(
                    self.runtime.bg_pipeline_state.startup_fetch_seam,
                    BgStartupFetchSeamState::Inactive
                )
    }

    pub(in crate::ppu) fn scy_obj_phase_owner(&self) -> Option<PpuMode3ScyObjPhaseOwner> {
        if self.current_dot_has_pending_obj_hit() {
            return Some(PpuMode3ScyObjPhaseOwner::PendingHit {
                match_x: self.runtime.bg_pipeline_state.current_transfer_x,
            });
        }

        if self.obj_enabled()
            && self.runtime.obj_pipeline_state.fetch.stage != PpuObjFetcherStage::Idle
        {
            let sprite = self.runtime.obj_pipeline_state.fetch.sprite?;
            return Some(PpuMode3ScyObjPhaseOwner::ActiveFetch { sprite_x: sprite.x });
        }

        self.current_transfer_selected_sprite_x()
            .map(|sprite_x| PpuMode3ScyObjPhaseOwner::CurrentTransferSprite { sprite_x })
            .or_else(|| {
                if !self.scy_startup_line_lead_owner_window_open() {
                    return None;
                }
                self.startup_line_lead_sprite_x()
                    .map(|sprite_x| PpuMode3ScyObjPhaseOwner::StartupLineLead { sprite_x })
            })
    }

    pub(in crate::ppu) fn scy_obj_phase_policy(&self) -> Option<PpuMode3ScyObjPhasePolicy> {
        let phase_owner = self.scy_obj_phase_owner()?;
        let context = PpuMode3ScyObjPhaseContext {
            phase_owner,
            current_transfer_x: self.runtime.bg_pipeline_state.current_transfer_x,
            current_transfer: self.current_transfer(),
            bg_fetcher_stage: self.runtime.bg_pipeline_state.fetcher.stage,
            bg_fetcher_stage_dot: self.runtime.bg_pipeline_state.fetcher.stage_dot,
            bg_fifo_len: self.runtime.bg_pipeline_state.fifo.len(),
            startup_fifo_placeholders: self.runtime.bg_pipeline_state.startup_fifo_placeholders,
            obj_fetcher_stage: self.runtime.obj_pipeline_state.fetch.stage,
            obj_fetcher_stage_dot: self.runtime.obj_pipeline_state.fetch.stage_dot,
        };

        Some(PpuMode3ScyObjPhasePolicy::new(context))
    }

    pub(in crate::ppu) fn compute_startup_visible_tile3_previous_row_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.runtime.bg_pipeline_state.startup_scy_tiledata_latch?;

        if !matches!(
            cached.origin,
            BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
        ) || cached.needs_live_tilemap_refetch
            || cached.needs_live_tile_data_refetch
            || self
                .scy_obj_phase_policy()
                .is_none_or(|phase| !phase.startup_visible_tile3_uses_previous_tiledata_row())
        {
            return None;
        }

        let tile_data_base = bg_tile_data_base(
            self.mode3_register_latches().visible().lcdc,
            cached.tile_index,
        );
        let current_row = ((cached.tile_high_address - tile_data_base) / TILE_ROW_BYTES) & 0x07;
        let previous_row = current_row.wrapping_sub(1) & 0x07;
        let tile_low_address = tile_data_base + previous_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(tile_low, tile_high, pixel_index))
    }

    pub(in crate::ppu) fn compute_startup_visible_tile2_previous_row_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.runtime.bg_pipeline_state.startup_scy_tiledata_latch?;

        if !matches!(
            cached.origin,
            BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
        ) || cached.needs_live_tilemap_refetch
            || cached.needs_live_tile_data_refetch
            || self
                .scy_obj_phase_policy()
                .is_none_or(|phase| !phase.startup_visible_tile2_uses_previous_tiledata_row())
        {
            return None;
        }

        let tile_data_base = bg_tile_data_base(
            self.mode3_register_latches().visible().lcdc,
            cached.tile_index,
        );
        let current_row = ((cached.tile_high_address - tile_data_base) / TILE_ROW_BYTES) & 0x07;
        let previous_row = current_row.wrapping_sub(1) & 0x07;
        let tile_low_address = tile_data_base + previous_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(tile_low, tile_high, pixel_index))
    }

    pub(in crate::ppu) fn compute_startup_visible_tile3_scx_boundary_next_tile_output_retarget_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        let scx = cached.startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx?;
        if (0x08..=0x0E).contains(&self.scx) {
            let retarget_low_band_pixel = matches!(self.scx & 0x07, 0x03) && pixel_index == 6;
            if !retarget_low_band_pixel {
                return None;
            }
        }
        let mut registers = self
            .current_mode3_live_background_refetch_context()
            .registers();
        registers.scx = scx;
        let fetch_x = cached.fetch_x + BG_TILE_WIDTH as u16;
        let tile_map_address =
            PpuMode3BackgroundFetchContext::new(registers, registers, fetch_x, self.ly)
                .tile_index_address();
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let tile_row = self
            .current_mode3_live_background_refetch_context()
            .current_scanline_tile_row();
        let tile_low_address =
            bg_tile_data_base(registers.lcdc, tile_index) + tile_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(tile_low, tile_high, pixel_index))
    }

    pub(in crate::ppu) fn compute_startup_visible_tile3_scx_low_band_shifted_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        if !matches!(
            cached.origin,
            BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
        ) || cached.fetch_x != BG_TILE_WIDTH as u16 * 2
            || !(0x08..=0x0E).contains(&self.scx)
        {
            return None;
        }

        match self.scx & 0x07 {
            0x00 | 0x06 if pixel_index == 0 => {
                Some(bg_tile_pixel_value(cached.tile_low, cached.tile_high, 1))
            }
            0x01 | 0x05 if pixel_index == 1 => {
                Some(bg_tile_pixel_value(cached.tile_low, cached.tile_high, 2))
            }
            0x03 if pixel_index == 5 => self
                .compute_startup_visible_tile3_scx_boundary_next_tile_output_retarget_pixel(
                    cached, 6, vram,
                ),
            _ => None,
        }
    }

    fn compute_startup_visible_tile3_scx_boundary_old_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        let previous_scx =
            cached.preserve_old_startup_visible_tile3_scx_boundary_pixel(pixel_index)?;
        let mut registers = self
            .current_mode3_live_background_refetch_context()
            .registers();
        registers.scx = previous_scx;
        let tile_map_address =
            PpuMode3BackgroundFetchContext::new(registers, registers, cached.fetch_x, self.ly)
                .tile_index_address();
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let tile_row = self
            .current_mode3_live_background_refetch_context()
            .current_scanline_tile_row();
        let tile_low_address =
            bg_tile_data_base(registers.lcdc, tile_index) + tile_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        Some(bg_tile_pixel_value(tile_low, tile_high, pixel_index))
    }

    pub(in crate::ppu) fn obj_enabled(&self) -> bool {
        self.mode3_register_latches().visible().obj_enabled()
    }
}
