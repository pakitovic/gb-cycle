use super::*;

impl Ppu {
    pub(in crate::ppu) fn maybe_abort_window_fetcher_to_background(
        &mut self,
        vram: &VramBusView<'_>,
    ) {
        if self.runtime.bg_pipeline_state.fetcher.source != PpuBgFetcherSource::Window {
            return;
        }

        if self.mode3_window_policy().fetcher_should_stay_windowed() {
            return;
        }

        self.maybe_record_dmg_window_reenable_resume();

        let low_wx_disable_seam = self.console_model.is_dmg_family()
            && self.runtime.bg_pipeline_state.window_started_this_line
            && self.runtime.bg_pipeline_state.visible_pixels_output == 0
            && self.mode3_register_latches().visible().wx < 8;
        if low_wx_disable_seam {
            self.maybe_arm_dmg_wx0_window_disable_prefix_override();

            if !matches!(
                (
                    self.runtime.bg_pipeline_state.fetcher.stage,
                    self.runtime.bg_pipeline_state.fetcher.stage_dot,
                ),
                (PpuBgFetcherStage::TileIndex, 0)
            ) {
                return;
            }
        }

        self.abort_window_fetcher_to_background_now(vram);
    }

    pub(super) fn abort_window_fetcher_to_background_now(&mut self, vram: &VramBusView<'_>) {
        self.runtime
            .bg_pipeline_state
            .fetcher
            .abort_window_to_background();
        let fetch_x = self.runtime.bg_pipeline_state.fetcher.fetch_x;
        let context = self.background_fetch_context(fetch_x);
        let tile_map_address = context.tile_index_address();
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let cgb_bg_attrs = self.read_cgb_bg_tile_attributes(vram, tile_map_address);
        let tile_low_address = self.compute_fetch_tile_data_address_with_attributes(
            PpuBgFetcherSource::Background,
            fetch_x,
            tile_index,
            0,
            cgb_bg_attrs,
        );
        let tile_high_address = self.compute_fetch_tile_data_address_with_attributes(
            PpuBgFetcherSource::Background,
            fetch_x,
            tile_index,
            1,
            cgb_bg_attrs,
        );
        self.runtime.bg_pipeline_state.fetcher.tile_map_address = tile_map_address;
        self.runtime.bg_pipeline_state.fetcher.tile_index = tile_index;
        self.runtime.bg_pipeline_state.fetcher.cgb_bg_attrs = cgb_bg_attrs;
        self.runtime.bg_pipeline_state.fetcher.tile_data_address = tile_low_address;
        self.runtime.bg_pipeline_state.fetcher.tile_low_address = tile_low_address;
        self.runtime.bg_pipeline_state.fetcher.tile_high_address = tile_high_address;
        self.runtime.bg_pipeline_state.fetcher.tile_low =
            self.read_bg_tile_data_byte(vram, cgb_bg_attrs, tile_low_address);
        self.runtime.bg_pipeline_state.fetcher.tile_high =
            self.read_bg_tile_data_byte(vram, cgb_bg_attrs, tile_high_address);
    }

    fn maybe_arm_dmg_wx0_window_disable_prefix_override(&mut self) {
        if !self.console_model.is_dmg_family()
            || self
                .runtime
                .bg_pipeline_state
                .dmg_wx0_window_disable_prefix_state
                .is_some()
            || !self.runtime.bg_pipeline_state.window_started_this_line
            || self.runtime.bg_pipeline_state.visible_pixels_output != 0
        {
            return;
        }

        let wx = self.mode3_register_latches().visible().wx;
        if wx >= 8 {
            return;
        }

        let desired_prefix_pixels = Self::DMG_WX0_WINDOW_DISABLE_PREFIX_PIXELS[usize::from(wx)];
        if desired_prefix_pixels == 8 {
            return;
        }

        self.runtime
            .bg_pipeline_state
            .dmg_wx0_window_disable_prefix_state =
            Some(DmgWx0WindowDisablePrefixState::new(desired_prefix_pixels));
    }

    pub(super) fn apply_dmg_wx0_window_disable_prefix_override(
        &mut self,
        visible_x: usize,
        bg_pixel: u8,
    ) {
        let Some(mut seam) = self
            .runtime
            .bg_pipeline_state
            .dmg_wx0_window_disable_prefix_state
        else {
            return;
        };

        seam.prefix_bg_pixel
            .get_or_insert(self.runtime.panel.current_scanline_bg_pixels[visible_x]);

        let desired_prefix_pixels = usize::from(seam.desired_prefix_pixels);
        if desired_prefix_pixels > 8 {
            if let Some(prefix_bg_pixel) = seam.prefix_bg_pixel
                && visible_x < desired_prefix_pixels
            {
                for target_visible_x in 0..=visible_x {
                    self.repaint_current_scanline_background_dot(target_visible_x, prefix_bg_pixel);
                }
            }

            if visible_x + 1 >= desired_prefix_pixels {
                self.runtime
                    .bg_pipeline_state
                    .dmg_wx0_window_disable_prefix_state = None;
                return;
            }
        } else if visible_x >= 8 {
            let retro_shift = 8 - desired_prefix_pixels;
            if visible_x < 8 + retro_shift {
                let target_visible_x = visible_x - retro_shift;
                self.repaint_current_scanline_background_dot(target_visible_x, bg_pixel);
            }

            if visible_x + 1 >= 8 + retro_shift {
                self.runtime
                    .bg_pipeline_state
                    .dmg_wx0_window_disable_prefix_state = None;
                return;
            }
        }

        self.runtime
            .bg_pipeline_state
            .dmg_wx0_window_disable_prefix_state = Some(seam);
    }

    fn repaint_current_scanline_background_dot(&mut self, visible_x: usize, bg_pixel: u8) {
        if self.runtime.panel.current_scanline_mixed_pixels[visible_x].source
            != MixedPixelSource::Background
        {
            return;
        }

        let bg_enabled = self.pixel_transfer_bg_enabled();
        let visible_output_driving =
            self.runtime.panel.visible_output == PpuVisibleOutputState::Driving;
        let output_pixel = MixedPixel::background(bg_pixel);
        let dmg_bg_forced_white = self.dmg_bg_panel_dot_is_forced_white(bg_enabled, output_pixel);
        let scanline_pixel = if visible_output_driving && !dmg_bg_forced_white {
            output_pixel.color
        } else {
            0
        };
        let panel_pixel = if visible_output_driving {
            if dmg_bg_forced_white {
                0
            } else {
                self.map_mixed_pixel_to_panel_shade(output_pixel)
            }
        } else {
            0
        };

        self.runtime.panel.current_scanline_bg_pixels[visible_x] = bg_pixel;
        self.write_bgwin_framebuffer_pixel(
            self.ly as usize * SCREEN_WIDTH,
            visible_x,
            bg_pixel,
            bg_enabled,
        );
        self.runtime.panel.current_scanline_mixed_pixels[visible_x] = output_pixel;
        self.runtime.panel.current_scanline_dmg_bg_forced_white[visible_x] = dmg_bg_forced_white;
        self.runtime.panel.current_scanline_pixels[visible_x] = scanline_pixel;
        self.write_framebuffer_pixel(
            self.ly as usize * SCREEN_WIDTH,
            visible_x,
            output_pixel,
            panel_pixel,
        );

        for dot in &mut self
            .runtime
            .panel
            .dmg_panel_live_write_state
            .recent_panel_dots
        {
            if usize::from(dot.visible_x) == visible_x {
                dot.pixel = output_pixel;
                dot.dmg_bg_forced_white = dmg_bg_forced_white;
            }
        }
    }

    pub(super) fn repaint_current_scanline_dot_with_bg_override(
        &mut self,
        visible_x: usize,
        bg_pixel: u8,
        vram: &VramBusView<'_>,
    ) {
        let bg_enabled = self.pixel_transfer_bg_enabled();
        let visible_output_driving =
            self.runtime.panel.visible_output == PpuVisibleOutputState::Driving;
        let obj_pixel = self.observed_obj_pixel_for_visible_x(visible_x as u8, vram);
        let effective_bg_priority_pixel = if bg_enabled { bg_pixel } else { 0 };
        let cgb_bg_attrs = self.runtime.panel.current_scanline_mixed_pixels[visible_x].cgb_bg_attrs;
        let output_pixel = self.mix_bg_and_obj(
            bg_pixel,
            cgb_bg_attrs,
            effective_bg_priority_pixel,
            obj_pixel,
        );
        let dmg_bg_forced_white = self.dmg_bg_panel_dot_is_forced_white(bg_enabled, output_pixel);
        let scanline_pixel = if visible_output_driving && !dmg_bg_forced_white {
            output_pixel.color
        } else {
            0
        };
        let panel_pixel = if visible_output_driving {
            if dmg_bg_forced_white {
                0
            } else {
                self.map_mixed_pixel_to_panel_shade(output_pixel)
            }
        } else {
            0
        };

        self.runtime.panel.current_scanline_bg_pixels[visible_x] = bg_pixel;
        self.write_bgwin_framebuffer_pixel(
            self.ly as usize * SCREEN_WIDTH,
            visible_x,
            bg_pixel,
            bg_enabled,
        );
        self.runtime.panel.current_scanline_mixed_pixels[visible_x] = output_pixel;
        self.runtime.panel.current_scanline_dmg_bg_forced_white[visible_x] = dmg_bg_forced_white;
        self.runtime.panel.current_scanline_pixels[visible_x] = scanline_pixel;
        self.write_framebuffer_pixel(
            self.ly as usize * SCREEN_WIDTH,
            visible_x,
            output_pixel,
            panel_pixel,
        );

        for dot in &mut self
            .runtime
            .panel
            .dmg_panel_live_write_state
            .recent_panel_dots
        {
            if usize::from(dot.visible_x) == visible_x {
                dot.pixel = output_pixel;
                dot.dmg_bg_forced_white = dmg_bg_forced_white;
            }
        }
    }

    fn observed_obj_pixel_for_visible_x(&self, visible_x: u8, vram: &VramBusView<'_>) -> ObjPixel {
        if !self.pixel_transfer_obj_enabled() {
            return ObjPixel::transparent();
        }

        let obj_height = match self.runtime.obj_pipeline_state.mode3_line_start_obj_height {
            0 => self.current_obj_height(),
            height => height,
        };
        let mut front = ObjPixel::transparent();
        for sprite_slot in 0..self.runtime.mode2_scan_state.selected_sprite_count() {
            let Some(sprite) = self.runtime.mode2_scan_state.selected_sprite(sprite_slot) else {
                continue;
            };
            let sprite_screen_x = sprite_screen_x(sprite);
            let tile_pixel = i16::from(visible_x) - sprite_screen_x;
            if !(0..BG_TILE_WIDTH as i16).contains(&tile_pixel) {
                continue;
            }

            let Some((tile_index, tile_row)) =
                self.obj_tile_index_and_row_for_height(sprite, obj_height)
            else {
                continue;
            };
            let tile_address = tile_index as u16 * TILE_BYTES + tile_row as u16 * TILE_ROW_BYTES;
            let bank = self.obj_tile_data_vram_bank(sprite);
            let tile_low = vram.read_bank(bank, tile_address as usize).unwrap_or(0);
            let tile_high = vram.read_bank(bank, tile_address as usize + 1).unwrap_or(0);
            let candidate = self.obj_pixel_from_sprite(
                sprite,
                obj_tile_pixel_value(tile_low, tile_high, tile_pixel as u8, sprite.attributes),
            );
            if self.obj_pixel_has_priority(candidate, front) {
                front = candidate;
            }
        }

        front
    }

    fn maybe_record_dmg_window_reenable_resume(&mut self) {
        let visible_wx = self.mode3_register_latches().visible().wx;
        if !self.console_model.is_dmg_family()
            || self
                .runtime
                .bg_pipeline_state
                .dmg_window_restart
                .pending_window_reenable_resume
                .is_some()
            || !self
                .mode3_register_latches()
                .lcdc_bit_changed(LCDC_WINDOW_ENABLE_BIT)
            || self.mode3_register_latches().visible().window_enabled()
            || !matches!(visible_wx, 28 | 29 | 35)
        {
            return;
        }

        let Some(window_origin_x) = self.visible_window_origin_x() else {
            return;
        };
        let emitted_window_pixels = self.count_emitted_window_pixels_this_line();
        let whole_tiles_emitted = emitted_window_pixels.saturating_add(7) / 8;
        let onset_x = window_origin_x.saturating_add(whole_tiles_emitted.saturating_mul(8));
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume = Some(DmgPendingWindowReenableResume::new(
            onset_x,
            window_origin_x,
            emitted_window_pixels,
            self.runtime.bg_pipeline_state.fetcher.stage,
            self.runtime.bg_pipeline_state.fetcher.stage_dot,
        ));
    }

    pub(super) fn maybe_arm_dmg_late_window_enable_override_after_transfer_dot(
        &mut self,
        _transfer_dot: Mode3TransferDot,
    ) {
        let visible_wx = self.mode3_register_latches().visible().wx;
        if !self.console_model.is_dmg_family()
            || !self.runtime.bg_pipeline_state.window_wy_latch
            || !self
                .mode3_register_latches()
                .lcdc_bit_changed(LCDC_WINDOW_ENABLE_BIT)
            || !self.mode3_register_latches().visible().window_enabled()
            || !self.mode3_register_latches().visible().bg_enabled()
            || visible_wx < 15
        {
            return;
        }

        let visible_output = self.runtime.bg_pipeline_state.visible_pixels_output;
        if let Some(pending_resume) = self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume
            .take()
        {
            let segment_pixels = match visible_wx {
                28 | 29 => 8,
                35 => 8,
                _ => 0,
            };
            let end_x = pending_resume.onset_x.saturating_add(segment_pixels);
            self.arm_dmg_late_window_enable_override(
                pending_resume.onset_x,
                end_x,
                pending_resume.window_origin_x,
            );
            return;
        }

        if self.count_emitted_window_pixels_this_line() != 0 {
            return;
        }

        let Some(window_origin_x) = self.visible_window_origin_x() else {
            return;
        };

        if (13..=14).contains(&visible_output) && matches!(visible_wx, 15..=21) {
            if window_origin_x == 8 {
                self.repaint_current_scanline_background_dot(8, 0);
                return;
            }

            let onset_x = window_origin_x.max(10);
            self.arm_dmg_late_window_enable_override(
                onset_x,
                onset_x.saturating_add(Self::DMG_LATE_WINDOW_ENABLE_SEGMENT_PIXELS),
                window_origin_x,
            );
            return;
        }

        if (33..=34).contains(&visible_output) && visible_wx == 39 {
            self.repaint_current_scanline_background_dot(32, 0);
            return;
        }

        if (41..=42).contains(&visible_output) && matches!(visible_wx, 44..=49) {
            let onset_x = window_origin_x.max(38);
            self.arm_dmg_late_window_enable_override(onset_x, SCREEN_WIDTH as u8, window_origin_x);
        }
    }

    fn arm_dmg_late_window_enable_override(&mut self, onset_x: u8, end_x: u8, window_origin_x: u8) {
        let clamped_end = end_x.min(SCREEN_WIDTH as u8);
        if onset_x >= clamped_end {
            return;
        }

        self.runtime
            .bg_pipeline_state
            .dmg_late_window_enable_override = Some(DmgLateWindowEnableOverride::new(
            onset_x,
            clamped_end,
            window_origin_x,
        ));
    }

    pub(super) fn apply_dmg_late_window_enable_override_repaint_up_to(
        &mut self,
        visible_limit: usize,
        vram: &VramBusView<'_>,
    ) {
        let Some(override_state) = self
            .runtime
            .bg_pipeline_state
            .dmg_late_window_enable_override
        else {
            return;
        };

        let repaint_end = visible_limit.min(usize::from(override_state.end_x));
        for visible_x in usize::from(override_state.onset_x)..repaint_end {
            let Some(bg_pixel) = self.compute_window_override_pixel_for_screen_x(
                override_state.window_origin_x,
                visible_x as u8,
                vram,
            ) else {
                continue;
            };
            self.repaint_current_scanline_background_dot(visible_x, bg_pixel);
        }

        if visible_limit >= usize::from(override_state.end_x) {
            self.runtime
                .bg_pipeline_state
                .dmg_late_window_enable_override = None;
        }
    }

    fn compute_window_override_pixel_for_screen_x(
        &self,
        window_origin_x: u8,
        visible_x: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        if visible_x < window_origin_x {
            return None;
        }

        let window_x = visible_x - window_origin_x;
        self.compute_window_pixel_for_logical_offset(
            self.current_window_line_counter(),
            u16::from(window_x),
            vram,
        )
    }

    pub(super) fn compute_window_pixel_for_logical_offset(
        &self,
        window_line_counter: u8,
        window_x: u16,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        let window_tilemap_x = (window_x / u16::from(BG_TILE_WIDTH)) as u8;
        let pixel_index = (window_x % u16::from(BG_TILE_WIDTH)) as u8;
        let context = self
            .mode3_bgwin_fetch_policy()
            .window_fetch_context(window_line_counter, window_tilemap_x);
        let tile_index = vram
            .read(context.tile_index_address() as usize)
            .unwrap_or(0);
        let cgb_bg_attrs = self.read_cgb_bg_tile_attributes(vram, context.tile_index_address());
        let attributes = cgb_bg_attrs.unwrap_or_default();
        let tile_row = cgb_bg_effective_tile_row(context.tile_data_row(), attributes);
        let tile_low_address = context.tile_data_address_for_row(tile_index, tile_row, 0);
        let tile_high_address = context.tile_data_address_for_row(tile_index, tile_row, 1);
        let tile_low = self.read_bg_tile_data_byte(vram, cgb_bg_attrs, tile_low_address);
        let tile_high = self.read_bg_tile_data_byte(vram, cgb_bg_attrs, tile_high_address);
        Some(bg_tile_pixel_value_with_cgb_attrs(
            tile_low,
            tile_high,
            pixel_index,
            attributes,
        ))
    }

    fn visible_window_origin_x(&self) -> Option<u8> {
        if self.runtime.bg_pipeline_state.window_force_x0_this_line {
            return Some(0);
        }

        match self.mode3_register_latches().visible().wx {
            0..=166 => Some(self.mode3_register_latches().visible().wx.saturating_sub(7)),
            _ => None,
        }
    }

    fn count_emitted_window_pixels_this_line(&self) -> u8 {
        self.runtime.panel.current_scanline_bg_dot_contexts
            [..usize::from(self.runtime.bg_pipeline_state.visible_pixels_output)]
            .iter()
            .filter(|context| {
                context.is_some_and(|context| context.source == PpuBgFetcherSource::Window)
            })
            .count() as u8
    }
    pub(super) fn compute_window_activation_tilemap_override(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        let previous_tilemap_select =
            cached.window_activation_first_pixel_previous_tilemap_select?;
        if cached.source != PpuBgFetcherSource::Window {
            return None;
        }

        let window_tile_row = self.current_window_line_counter();
        if let Some(transient_tilemap_mask) = self
            .cgb_dmg_software_window_activation_lead_in_transient_tilemap_mask(
                cached.fetch_x,
                window_tile_row,
            )
        {
            let use_transient_tilemap = transient_tilemap_mask & (0x80 >> pixel_index) != 0;
            let transient_tilemap_select = if window_tile_row < 16 {
                !previous_tilemap_select
            } else {
                previous_tilemap_select
            };
            let tilemap_select = if use_transient_tilemap {
                transient_tilemap_select
            } else {
                !transient_tilemap_select
            };
            return Some(self.read_window_activation_tilemap_pixel(
                cached,
                pixel_index,
                tilemap_select,
                vram,
            ));
        }

        if let Some(current_tilemap_mask) =
            self.window_activation_tile_current_tilemap_mask(cached.fetch_x, window_tile_row)
        {
            let use_first_write_current_tilemap = current_tilemap_mask & (0x80 >> pixel_index) != 0;
            let tilemap_select = if use_first_write_current_tilemap {
                !previous_tilemap_select
            } else {
                previous_tilemap_select
            };
            return Some(self.read_window_activation_tilemap_pixel(
                cached,
                pixel_index,
                tilemap_select,
                vram,
            ));
        }

        if cached.fetch_x != 0
            || pixel_index != 0
            || !window_activation_first_pixel_uses_previous_tilemap(window_tile_row)
        {
            return None;
        }

        Some(self.read_window_activation_tilemap_pixel(
            cached,
            pixel_index,
            previous_tilemap_select,
            vram,
        ))
    }

    fn read_window_activation_tilemap_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        tilemap_select: bool,
        vram: &VramBusView<'_>,
    ) -> u8 {
        let tile_map_offset = cached.tile_map_address & 0x03FF;
        let tile_map_base = if tilemap_select { 0x1C00 } else { 0x1800 };
        let tile_map_address = tile_map_base | tile_map_offset;
        let tile_index = vram
            .read(tile_map_address as usize)
            .unwrap_or(cached.tile_index);
        let registers = self.mode3_register_latches().visible();
        let tile_row = self
            .current_mode3_live_background_refetch_context()
            .current_window_tile_row();
        let tile_low_address =
            bg_tile_data_base(registers.lcdc, tile_index) + tile_row * TILE_ROW_BYTES;
        let tile_high_address = tile_low_address + 1;
        let tile_low = vram.read(tile_low_address as usize).unwrap_or(0);
        let tile_high = vram.read(tile_high_address as usize).unwrap_or(0);
        bg_tile_pixel_value(tile_low, tile_high, pixel_index)
    }

    fn cgb_dmg_software_window_activation_lead_in_transient_tilemap_mask(
        &self,
        fetch_x: u16,
        window_tile_row: u8,
    ) -> Option<u8> {
        if !self.console_model.is_cgb_family() || !self.operating_mode.uses_dmg_software_contract()
        {
            return None;
        }

        cgb_dmg_software_window_activation_lead_in_transient_tilemap_mask(fetch_x, window_tile_row)
    }

    fn window_activation_tile_current_tilemap_mask(
        &self,
        fetch_x: u16,
        window_tile_row: u8,
    ) -> Option<u8> {
        if !self.operating_mode.uses_dmg_software_contract() {
            return None;
        }

        dmg_window_activation_tile_current_tilemap_mask(fetch_x, window_tile_row)
    }

    pub(super) fn compute_window_lcdc4_tiledata_selector_override(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        let previous_select = cached.dmg_lcdc4_previous_tiledata_select_for_output_override?;
        if cached.source != PpuBgFetcherSource::Window {
            return None;
        }

        let previous_plane_masks = window_lcdc4_unsigned_to_signed_previous_plane_masks(
            cached.fetch_x,
            self.current_window_line_counter(),
        )?;
        Some(self.read_window_lcdc4_tiledata_selector_pixel(
            cached,
            pixel_index,
            previous_select,
            previous_plane_masks,
            vram,
        ))
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_compute_window_lcdc4_tiledata_selector_override(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.compute_window_lcdc4_tiledata_selector_override(cached, pixel_index, vram)
    }

    fn read_window_lcdc4_tiledata_selector_pixel(
        &self,
        cached: BgCachedSlice,
        pixel_index: u8,
        previous_select: BgTileDataSelect,
        previous_plane_masks: PerPlane<u8>,
        vram: &VramBusView<'_>,
    ) -> u8 {
        let bit = 0x80 >> pixel_index;
        let current_lcdc = self.mode3_register_latches().visible().lcdc;
        let previous_lcdc = previous_select.apply_to_lcdc(current_lcdc);
        let current_tile_row = (self.current_window_line_counter() & (BG_TILE_WIDTH - 1)) as u16;
        let previous_tile_low_address =
            bg_tile_data_base(previous_lcdc, cached.tile_index) + current_tile_row * TILE_ROW_BYTES;
        let previous_tile_high_address = previous_tile_low_address + 1;
        let current_tile_low_address =
            bg_tile_data_base(current_lcdc, cached.tile_index) + current_tile_row * TILE_ROW_BYTES;
        let current_tile_high_address = current_tile_low_address + 1;
        let previous_tile_low = vram.read(previous_tile_low_address as usize).unwrap_or(0);
        let previous_tile_high = vram.read(previous_tile_high_address as usize).unwrap_or(0);
        let current_tile_low = vram.read(current_tile_low_address as usize).unwrap_or(0);
        let current_tile_high = vram.read(current_tile_high_address as usize).unwrap_or(0);
        let tile_low = if previous_plane_masks.low & bit != 0 {
            previous_tile_low
        } else {
            current_tile_low
        };
        let tile_high = if previous_plane_masks.high & bit != 0 {
            previous_tile_high
        } else {
            current_tile_high
        };
        bg_tile_pixel_value(tile_low, tile_high, pixel_index)
    }

    fn compute_window_lcdc4_tiledata_selector_override_from_context(
        &self,
        context: PpuRecentBgDotContext,
        previous_select: BgTileDataSelect,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        if context.source != PpuBgFetcherSource::Window {
            return None;
        }

        let previous_plane_masks = window_lcdc4_unsigned_to_signed_previous_plane_masks(
            context.fetch_x,
            self.current_window_line_counter(),
        )?;
        let bit = 0x80 >> context.pixel_index;
        let current_lcdc = self.mode3_register_latches().visible().lcdc;
        let previous_lcdc = previous_select.apply_to_lcdc(current_lcdc);
        let current_tile_row = (self.current_window_line_counter() & (BG_TILE_WIDTH - 1)) as u16;
        let previous_tile_low_address = bg_tile_data_base(previous_lcdc, context.tile_index)
            + current_tile_row * TILE_ROW_BYTES;
        let previous_tile_high_address = previous_tile_low_address + 1;
        let current_tile_low_address =
            bg_tile_data_base(current_lcdc, context.tile_index) + current_tile_row * TILE_ROW_BYTES;
        let current_tile_high_address = current_tile_low_address + 1;
        let previous_tile_low = vram.read(previous_tile_low_address as usize).unwrap_or(0);
        let previous_tile_high = vram.read(previous_tile_high_address as usize).unwrap_or(0);
        let current_tile_low = vram.read(current_tile_low_address as usize).unwrap_or(0);
        let current_tile_high = vram.read(current_tile_high_address as usize).unwrap_or(0);
        let tile_low = if previous_plane_masks.low & bit != 0 {
            previous_tile_low
        } else {
            current_tile_low
        };
        let tile_high = if previous_plane_masks.high & bit != 0 {
            previous_tile_high
        } else {
            current_tile_high
        };
        Some(bg_tile_pixel_value(
            tile_low,
            tile_high,
            context.pixel_index,
        ))
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_compute_window_lcdc4_tiledata_selector_override_from_context(
        &self,
        context: PpuRecentBgDotContext,
        previous_select: BgTileDataSelect,
        vram: &VramBusView<'_>,
    ) -> Option<u8> {
        self.compute_window_lcdc4_tiledata_selector_override_from_context(
            context,
            previous_select,
            vram,
        )
    }

    pub(super) fn apply_pending_dmg_window_lcdc4_output_repaint(&mut self, vram: &VramBusView<'_>) {
        let Some(previous_select) = self
            .runtime
            .panel
            .pending_dmg_window_lcdc4_output_repaint
            .take()
        else {
            return;
        };

        let bg_enabled = self.pixel_transfer_bg_enabled();
        let visible_output_driving =
            self.runtime.panel.visible_output == PpuVisibleOutputState::Driving;
        let row_start = self.ly as usize * SCREEN_WIDTH;
        let visible_limit = usize::from(self.runtime.bg_pipeline_state.visible_pixels_output);

        for visible_x in 0..visible_limit {
            let Some(context) = self.runtime.panel.current_scanline_bg_dot_contexts[visible_x]
            else {
                continue;
            };
            let Some(bg_pixel) = self.compute_window_lcdc4_tiledata_selector_override_from_context(
                context,
                previous_select,
                vram,
            ) else {
                continue;
            };

            self.runtime.panel.current_scanline_bg_pixels[visible_x] = bg_pixel;
            self.write_bgwin_framebuffer_pixel(row_start, visible_x, bg_pixel, bg_enabled);
            if self.runtime.panel.current_scanline_mixed_pixels[visible_x].source
                != MixedPixelSource::Background
            {
                continue;
            }

            let output_pixel = MixedPixel::background(bg_pixel);
            let dmg_bg_forced_white =
                self.dmg_bg_panel_dot_is_forced_white(bg_enabled, output_pixel);
            let scanline_pixel = if visible_output_driving && !dmg_bg_forced_white {
                output_pixel.color
            } else {
                0
            };
            let panel_pixel = if visible_output_driving {
                if dmg_bg_forced_white {
                    0
                } else {
                    self.map_mixed_pixel_to_panel_shade(output_pixel)
                }
            } else {
                0
            };

            self.runtime.panel.current_scanline_mixed_pixels[visible_x] = output_pixel;
            self.runtime.panel.current_scanline_dmg_bg_forced_white[visible_x] =
                dmg_bg_forced_white;
            self.runtime.panel.current_scanline_pixels[visible_x] = scanline_pixel;
            self.write_framebuffer_pixel(row_start, visible_x, output_pixel, panel_pixel);

            for dot in &mut self
                .runtime
                .panel
                .dmg_panel_live_write_state
                .recent_panel_dots
            {
                if usize::from(dot.visible_x) == visible_x {
                    dot.pixel = output_pixel;
                    dot.dmg_bg_forced_white = dmg_bg_forced_white;
                }
            }
        }
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_apply_pending_dmg_window_lcdc4_output_repaint(
        &mut self,
        vram: &VramBusView<'_>,
    ) {
        self.apply_pending_dmg_window_lcdc4_output_repaint(vram);
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_apply_dmg_late_window_enable_override_repaint_up_to(
        &mut self,
        visible_limit: usize,
        vram: &VramBusView<'_>,
    ) {
        self.apply_dmg_late_window_enable_override_repaint_up_to(visible_limit, vram);
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_apply_dmg_wx0_window_disable_prefix_override(
        &mut self,
        visible_x: usize,
        bg_pixel: u8,
    ) {
        self.apply_dmg_wx0_window_disable_prefix_override(visible_x, bg_pixel);
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_apply_pending_dmg_previsible_wx_carry(
        &mut self,
        transfer_dot: Mode3TransferDot,
        vram: &VramBusView<'_>,
    ) {
        self.maybe_apply_pending_dmg_previsible_wx_carry(transfer_dot, vram);
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_apply_pending_dmg_previsible_wx_onset_glitch_repaint(
        &mut self,
        vram: &VramBusView<'_>,
    ) {
        self.maybe_apply_pending_dmg_previsible_wx_onset_glitch_repaint(vram);
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_expire_dmg_previsible_wx_retarget(&mut self) {
        self.maybe_expire_dmg_previsible_wx_retarget();
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_apply_dmg_previsible_wx_retarget(&mut self, vram: &VramBusView<'_>) {
        self.maybe_apply_dmg_previsible_wx_retarget(vram);
    }

    #[cfg(test)]
    pub(in crate::ppu) fn test_expire_pending_dmg_live_wx_trigger_glitch(&mut self) {
        self.maybe_expire_pending_dmg_live_wx_trigger_glitch();
    }
}

const fn window_activation_first_pixel_uses_previous_tilemap(window_tile_row: u8) -> bool {
    window_tile_row >= 24 && matches!(window_tile_row & 0x07, 0x01 | 0x02 | 0x04 | 0x06)
}

const WINDOW_ACTIVATION_FIRST_TILE_CURRENT_TILEMAP_MASKS: [[u8; 8]; 15] = [
    [0x80, 0x40, 0x20, 0xA0, 0x20, 0xA0, 0x40, 0x80],
    [0xC0, 0x20, 0x90, 0x50, 0x90, 0x50, 0x20, 0xC0],
    [0xE0, 0x10, 0xC8, 0x28, 0xC8, 0x28, 0x10, 0xE0],
    [0xF0, 0x08, 0xE4, 0x94, 0xE4, 0x94, 0x08, 0xF0],
    [0x78, 0x84, 0x72, 0x4A, 0x72, 0x4A, 0x84, 0x78],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
    [0x0F, 0x10, 0x2E, 0x29, 0x2E, 0x29, 0x10, 0x0F],
    [0x07, 0x08, 0x17, 0x14, 0x17, 0x14, 0x08, 0x07],
    [0x03, 0x04, 0x0B, 0x0A, 0x0B, 0x0A, 0x04, 0x03],
    [0x01, 0x02, 0x05, 0x05, 0x05, 0x05, 0x02, 0x01],
    [0x00, 0x01, 0x02, 0x02, 0x02, 0x02, 0x01, 0x00],
    [0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
];

const WINDOW_ACTIVATION_SECOND_TILE_CURRENT_TILEMAP_MASKS: [[u8; 8]; 15] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00],
    [0x00, 0x80, 0x40, 0x40, 0x40, 0x40, 0x80, 0x00],
    [0x80, 0x40, 0x20, 0xA0, 0x20, 0xA0, 0x40, 0x80],
    [0xC0, 0x20, 0x90, 0x50, 0x90, 0x50, 0x20, 0xC0],
    [0xE0, 0x10, 0xC8, 0x28, 0xC8, 0x28, 0x10, 0xE0],
    [0xF0, 0x08, 0xE4, 0x94, 0xE4, 0x94, 0x08, 0xF0],
    [0x78, 0x84, 0x72, 0x4A, 0x72, 0x4A, 0x84, 0x78],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
];

const CGB_DMG_SOFTWARE_WINDOW_ACTIVATION_LEAD_IN_FIRST_TILE_TRANSIENT_TILEMAP_MASKS: [[u8; 8]; 3] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00],
    [0x00, 0x80, 0x40, 0x40, 0x40, 0x40, 0x80, 0x00],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_CURRENT_TILE_PREVIOUS_LOW_MASKS: [[u8; 8]; 15] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xE0, 0x10, 0xC8, 0x28, 0xC8, 0x28, 0x10, 0xE0],
    [0xF0, 0x08, 0xE4, 0x94, 0xE4, 0x94, 0x08, 0xF0],
    [0x78, 0x84, 0x72, 0x4A, 0x72, 0x4A, 0x84, 0x78],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
    [0x0F, 0x10, 0x2E, 0x29, 0x2E, 0x29, 0x10, 0x0F],
    [0x07, 0x08, 0x17, 0x14, 0x17, 0x14, 0x08, 0x07],
    [0x03, 0x04, 0x0B, 0x0A, 0x0B, 0x0A, 0x04, 0x03],
    [0x01, 0x02, 0x05, 0x05, 0x05, 0x05, 0x02, 0x01],
    [0x00, 0x01, 0x02, 0x02, 0x02, 0x02, 0x01, 0x00],
    [0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_CURRENT_TILE_PREVIOUS_HIGH_MASKS: [[u8; 8]; 15] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
    [0x0F, 0x10, 0x2E, 0x29, 0x2E, 0x29, 0x10, 0x0F],
    [0x07, 0x08, 0x17, 0x14, 0x17, 0x14, 0x08, 0x07],
    [0x03, 0x04, 0x0B, 0x0A, 0x0B, 0x0A, 0x04, 0x03],
    [0x01, 0x02, 0x05, 0x05, 0x05, 0x05, 0x02, 0x01],
    [0x00, 0x01, 0x02, 0x02, 0x02, 0x02, 0x01, 0x00],
    [0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_NEXT_TILE_PREVIOUS_LOW_MASKS: [[u8; 8]; 15] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00],
    [0x00, 0x80, 0x40, 0x40, 0x40, 0x40, 0x80, 0x00],
    [0x80, 0x40, 0x20, 0xA0, 0x20, 0xA0, 0x40, 0x80],
    [0xC0, 0x20, 0x90, 0x50, 0x90, 0x50, 0x20, 0xC0],
    [0xE0, 0x10, 0xC8, 0x28, 0xC8, 0x28, 0x10, 0xE0],
    [0xF0, 0x08, 0xE4, 0x94, 0xE4, 0x94, 0x08, 0xF0],
    [0x78, 0x84, 0x72, 0x4A, 0x72, 0x4A, 0x84, 0x78],
    [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C],
    [0x1E, 0x21, 0x5C, 0x52, 0x5C, 0x52, 0x21, 0x1E],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_NEXT_TILE_PREVIOUS_HIGH_MASKS: [[u8; 8]; 15] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_THIRD_TILE_PREVIOUS_LOW_MASKS: [[u8; 8]; 15] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
];

const WINDOW_LCDC4_UNSIGNED_TO_SIGNED_THIRD_TILE_PREVIOUS_HIGH_MASKS: [[u8; 8]; 15] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00],
];

const fn cgb_dmg_software_window_activation_lead_in_transient_tilemap_mask(
    fetch_x: u16,
    window_tile_row: u8,
) -> Option<u8> {
    if window_tile_row >= 24 {
        return None;
    }

    let block = window_tile_row / 8;
    let row = (window_tile_row & 0x07) as usize;
    match fetch_x {
        0 => Some(
            CGB_DMG_SOFTWARE_WINDOW_ACTIVATION_LEAD_IN_FIRST_TILE_TRANSIENT_TILEMAP_MASKS
                [block as usize][row],
        ),
        x if x == BG_TILE_WIDTH as u16 => Some(0xFF),
        _ => None,
    }
}

const fn dmg_window_activation_tile_current_tilemap_mask(
    fetch_x: u16,
    window_tile_row: u8,
) -> Option<u8> {
    if window_tile_row < 24 || window_tile_row >= 144 {
        return None;
    }

    let block = ((window_tile_row - 24) / 8) as usize;
    let row = (window_tile_row & 0x07) as usize;
    match fetch_x {
        0 => Some(WINDOW_ACTIVATION_FIRST_TILE_CURRENT_TILEMAP_MASKS[block][row]),
        x if x == BG_TILE_WIDTH as u16 => {
            Some(WINDOW_ACTIVATION_SECOND_TILE_CURRENT_TILEMAP_MASKS[block][row])
        }
        x if x == BG_TILE_WIDTH as u16 * 2 => match window_tile_row {
            112..=127 => Some(0x00),
            128..=143 => Some(0xFF),
            _ => None,
        },
        _ => None,
    }
}

pub(in crate::ppu) const fn window_lcdc4_unsigned_to_signed_previous_plane_masks(
    fetch_x: u16,
    window_tile_row: u8,
) -> Option<PerPlane<u8>> {
    if window_tile_row < 24 || window_tile_row >= 144 {
        return None;
    }

    let block = ((window_tile_row - 24) / 8) as usize;
    let row = (window_tile_row & 0x07) as usize;
    match fetch_x {
        0 => Some(PerPlane::new(
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_CURRENT_TILE_PREVIOUS_LOW_MASKS[block][row],
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_CURRENT_TILE_PREVIOUS_HIGH_MASKS[block][row],
        )),
        x if x == BG_TILE_WIDTH as u16 => Some(PerPlane::new(
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_NEXT_TILE_PREVIOUS_LOW_MASKS[block][row],
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_NEXT_TILE_PREVIOUS_HIGH_MASKS[block][row],
        )),
        x if x == BG_TILE_WIDTH as u16 * 2 => Some(PerPlane::new(
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_THIRD_TILE_PREVIOUS_LOW_MASKS[block][row],
            WINDOW_LCDC4_UNSIGNED_TO_SIGNED_THIRD_TILE_PREVIOUS_HIGH_MASKS[block][row],
        )),
        _ => None,
    }
}
