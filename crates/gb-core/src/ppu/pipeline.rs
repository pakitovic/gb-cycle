impl Ppu {
    fn compute_fetch_tile_index_address(
        &self,
        source: PpuBgFetcherSource,
        next_fetch_pixel: u16,
    ) -> u16 {
        let (tile_map_base, tile_x, tile_y) = match source {
            PpuBgFetcherSource::Background => {
                let bg_x = self
                    .visible_registers
                    .scx
                    .wrapping_add(next_fetch_pixel as u8);
                let bg_fetch_scy = self.bg_fetch_scy(next_fetch_pixel);
                let bg_fetch_lcdc = self.bg_fetch_tilemap_lcdc(next_fetch_pixel);
                let bg_y = bg_fetch_scy.wrapping_add(self.ly);
                let tile_map_base = if bg_fetch_lcdc & LCDC_BG_TILE_MAP_BIT != 0 {
                    0x1C00
                } else {
                    0x1800
                };
                (
                    tile_map_base,
                    (bg_x / BG_TILE_WIDTH) as usize,
                    (bg_y / BG_TILE_WIDTH) as usize,
                )
            }
            PpuBgFetcherSource::Window => {
                let tile_map_base = if self.window_fetch_lcdc() & LCDC_WINDOW_TILE_MAP_BIT != 0 {
                    0x1C00
                } else {
                    0x1800
                };
                (
                    tile_map_base,
                    self.bg_pipeline_state.fetcher.window_tilemap_x as usize,
                    (self.window_state.window_line_counter / BG_TILE_WIDTH) as usize,
                )
            }
        };
        (tile_map_base + tile_y * BG_TILE_MAP_WIDTH as usize + tile_x) as u16
    }

    fn compute_fetch_tile_data_address(
        &self,
        source: PpuBgFetcherSource,
        fetch_x: u16,
        tile_index: u8,
        plane: u16,
    ) -> u16 {
        let tile_row = match source {
            PpuBgFetcherSource::Background => {
                (self.bg_fetch_scy(fetch_x).wrapping_add(self.ly) % BG_TILE_WIDTH) as u16
            }
            PpuBgFetcherSource::Window => {
                (self.window_state.window_line_counter % BG_TILE_WIDTH) as u16
            }
        };
        let tile_data_base = bg_tile_data_base(
            match source {
                PpuBgFetcherSource::Background => self.bg_fetch_tiledata_lcdc(fetch_x),
                PpuBgFetcherSource::Window => self.window_fetch_lcdc(),
            },
            tile_index,
        );
        tile_data_base + tile_row * TILE_ROW_BYTES + plane
    }

    fn bg_fetch_tilemap_uses_pipeline_snapshot(&self, next_fetch_pixel: u16) -> bool {
        let _ = next_fetch_pixel;
        self.console_model.is_dmg_family()
            && self
                .bg_pipeline_state
                .startup_background_tilemap_uses_pipeline_snapshot()
    }

    fn bg_fetch_tiledata_uses_pipeline_snapshot(&self, next_fetch_pixel: u16) -> bool {
        let _ = next_fetch_pixel;
        self.console_model.is_dmg_family()
            && self
                .bg_pipeline_state
                .startup_background_tiledata_uses_pipeline_snapshot()
    }

    fn bg_fetch_tilemap_lcdc(&self, next_fetch_pixel: u16) -> u8 {
        if self.bg_fetch_tilemap_uses_pipeline_snapshot(next_fetch_pixel) {
            self.pipeline_registers.lcdc
        } else {
            self.visible_registers.lcdc
        }
    }

    fn bg_fetch_tiledata_lcdc(&self, next_fetch_pixel: u16) -> u8 {
        if self.bg_fetch_tiledata_uses_pipeline_snapshot(next_fetch_pixel) {
            self.pipeline_registers.lcdc
        } else {
            self.visible_registers.lcdc
        }
    }

    fn bg_fetch_scy(&self, next_fetch_pixel: u16) -> u8 {
        if self.bg_fetch_tiledata_uses_pipeline_snapshot(next_fetch_pixel) {
            self.pipeline_registers.scy
        } else {
            self.visible_registers.scy
        }
    }

    fn window_fetch_lcdc(&self) -> u8 {
        self.visible_registers.lcdc
    }

    fn maybe_cache_unsigned_bgwin_tile_data_fetch(
        &mut self,
        source: PpuBgFetcherSource,
        next_fetch_pixel: u16,
        plane: u16,
        tile_data: u8,
    ) {
        if self.bg_pipeline_state.startup_alignment_seed_pending() {
            return;
        }
        let lcdc = match source {
            PpuBgFetcherSource::Background => self.bg_fetch_tiledata_lcdc(next_fetch_pixel),
            PpuBgFetcherSource::Window => self.window_fetch_lcdc(),
        };
        if lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0 {
            self.last_unsigned_tile_data_fetch = tile_data;
            if plane == 0 {
                self.last_unsigned_tile_data_low_fetch = tile_data;
            } else {
                self.last_unsigned_tile_data_high_fetch = tile_data;
            }
        }
    }

    fn maybe_apply_bgwin_tile_data_selector_glitch(
        &mut self,
        vram: &VramBusView<'_>,
        source: PpuBgFetcherSource,
        plane: u16,
    ) {
        if !self.console_model.is_dmg_family() {
            return;
        }

        let previous_uses_unsigned =
            self.pipeline_registers.lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;
        let current_uses_unsigned = self.visible_registers.lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;
        if previous_uses_unsigned == current_uses_unsigned {
            return;
        }

        let tile_index = self.bg_pipeline_state.fetcher.tile_index;
        let tile_data_address = self.compute_fetch_tile_data_address(
            source,
            self.bg_pipeline_state.fetcher.fetch_x,
            tile_index,
            plane,
        );
        let tile_byte = vram.read(tile_data_address as usize).unwrap_or(0);

        let fetcher = &mut self.bg_pipeline_state.fetcher;
        fetcher.tile_data_address = tile_data_address;

        if plane == 0 {
            fetcher.tile_low = tile_byte;
        } else {
            fetcher.tile_high = tile_byte;
        }
    }

    fn maybe_apply_bgwin_tilemap_selector_glitch(
        &mut self,
        vram: &VramBusView<'_>,
        source: PpuBgFetcherSource,
    ) {
        if !self.console_model.is_dmg_family() {
            return;
        }

        let map_bit = match source {
            PpuBgFetcherSource::Background => LCDC_BG_TILE_MAP_BIT,
            PpuBgFetcherSource::Window => LCDC_WINDOW_TILE_MAP_BIT,
        };
        let previous_selects_high = self.pipeline_registers.lcdc & map_bit != 0;
        let current_selects_high = self.visible_registers.lcdc & map_bit != 0;
        if previous_selects_high == current_selects_high {
            return;
        }

        let tile_map_address =
            self.compute_fetch_tile_index_address(source, self.bg_pipeline_state.fetcher.fetch_x);
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let fetcher = &mut self.bg_pipeline_state.fetcher;
        fetcher.tile_map_address = tile_map_address;
        fetcher.tile_index = tile_index;
    }

    fn read_obj_tile_data_byte(
        &mut self,
        vram: &VramBusView<'_>,
        sprite: PpuSelectedSprite,
        plane: u16,
    ) -> u8 {
        let Some((tile_index, tile_row)) = self.obj_tile_index_and_row(sprite) else {
            return 0;
        };
        let byte_address =
            tile_index as u16 * TILE_BYTES + tile_row as u16 * TILE_ROW_BYTES + plane;
        let tile_data = vram.read(byte_address as usize).unwrap_or(0);
        self.last_unsigned_tile_data_fetch = tile_data;
        tile_data
    }

    fn obj_tile_index_and_row(&self, sprite: PpuSelectedSprite) -> Option<(u8, u8)> {
        let sprite_top = sprite.y.wrapping_sub(16);
        let height = self.current_obj_height();
        let mut row = self.ly.wrapping_sub(sprite_top);
        if row >= height {
            return None;
        }
        if sprite.attributes & 0x40 != 0 {
            row = height - 1 - row;
        }

        if height == 16 {
            let base_tile = sprite.tile_index & !0x01;
            if row < 8 {
                Some((base_tile, row))
            } else {
                Some((base_tile + 1, row - 8))
            }
        } else {
            Some((sprite.tile_index, row))
        }
    }

    fn push_obj_pixels(
        &mut self,
        sprite: PpuSelectedSprite,
        tile_low: u8,
        tile_high: u8,
        current_visible_x: u8,
    ) {
        let sprite_screen_x = sprite_screen_x(sprite);
        for tile_pixel in 0..BG_TILE_WIDTH {
            let bit = if sprite.attributes & 0x20 != 0 {
                tile_pixel
            } else {
                7 - tile_pixel
            };
            let low_bit = (tile_low >> bit) & 0x01;
            let high_bit = (tile_high >> bit) & 0x01;
            let color = (high_bit << 1) | low_bit;
            let screen_x = sprite_screen_x + tile_pixel as i16;
            if !(0..SCREEN_WIDTH as i16).contains(&screen_x) {
                continue;
            }
            if screen_x < current_visible_x as i16 {
                continue;
            }

            let offset = (screen_x as usize).saturating_sub(current_visible_x as usize);
            while self.obj_pipeline_state.fifo.len() <= offset {
                self.obj_pipeline_state
                    .fifo
                    .push_back(ObjPixel::transparent());
            }

            let candidate = ObjPixel {
                color,
                palette_obp1: sprite.attributes & 0x10 != 0,
                bg_over_obj: sprite.attributes & 0x80 != 0,
                sprite_x: sprite.x,
                oam_index: sprite.oam_index,
            };

            let slot = self
                .obj_pipeline_state
                .fifo
                .get_mut(offset)
                .expect("OBJ FIFO was extended to cover the target offset");
            if obj_pixel_has_priority(candidate, *slot) {
                *slot = candidate;
            }
        }
    }

    fn pop_obj_fifo_pixel(&mut self) -> ObjPixel {
        self.obj_pipeline_state
            .fifo
            .pop_front()
            .unwrap_or_else(ObjPixel::transparent)
    }

    fn mix_bg_and_obj(&self, bg_pixel: u8, obj_pixel: ObjPixel) -> MixedPixel {
        if !self.pixel_transfer_obj_enabled() || obj_pixel.is_transparent() {
            return MixedPixel::background(bg_pixel);
        }

        if obj_pixel.bg_over_obj && bg_pixel != 0 {
            MixedPixel::background(bg_pixel)
        } else {
            MixedPixel::object(obj_pixel.color, obj_pixel.palette_obp1)
        }
    }

    fn map_mixed_pixel_to_panel_shade(&self, pixel: MixedPixel) -> u8 {
        match pixel.source {
            MixedPixelSource::Background => {
                self.apply_dmg_palette(self.pixel_pipeline_bgp(), pixel.color)
            }
            MixedPixelSource::Object { palette_obp1 } => {
                let palette = self
                    .visible_registers
                    .obj_palette(palette_obp1, self.obj_palette_read_policy);
                self.apply_dmg_palette(palette, pixel.color)
            }
        }
    }

    fn apply_dmg_palette(&self, palette: u8, color: u8) -> u8 {
        (palette >> (u32::from(color & 0x03) * 2)) & 0x03
    }

    fn write_dmg_palette_register(
        &mut self,
        register: PpuPaletteRegister,
        value: u8,
        source: PpuRegisterWriteSource,
    ) {
        let previous_visible = match register {
            PpuPaletteRegister::Bgp => self.visible_registers.bgp,
            PpuPaletteRegister::Obp0 => self
                .visible_registers
                .obj_palette(false, self.obj_palette_read_policy),
            PpuPaletteRegister::Obp1 => self
                .visible_registers
                .obj_palette(true, self.obj_palette_read_policy),
        };

        match register {
            PpuPaletteRegister::Bgp => self.bgp = value,
            PpuPaletteRegister::Obp0 => self.obp0 = Some(value),
            PpuPaletteRegister::Obp1 => self.obp1 = Some(value),
        }

        let bgp_cpu_commit_delay_active = register == PpuPaletteRegister::Bgp
            && source == PpuRegisterWriteSource::CpuMmioCommit
            && matches!(
                self.current_raster_state(),
                PpuRasterState::Active {
                    mode: PpuAccessMode::Drawing,
                    ..
                }
            );

        if bgp_cpu_commit_delay_active {
            self.record_dmg_bgp_cpu_commit_visible_write(value);
            self.dmg_bgp_cpu_commit_output_palette_override = Some(self.pixel_pipeline_bgp());
            self.dmg_bgp_cpu_commit_output_delay_pixels_remaining = 4;
        }

        if let Some(retroactive_pixels) = self.dmg_palette_conflict_retroactive_pixels(register)
            && !bgp_cpu_commit_delay_active
        {
            self.retroactively_recolor_recent_pixels(
                register,
                previous_visible | value,
                value,
                retroactive_pixels,
            );
        }
    }

    fn retroactively_recolor_recent_pixels(
        &mut self,
        register: PpuPaletteRegister,
        transient_palette: u8,
        final_palette: u8,
        retroactive_pixels: usize,
    ) {
        if self.visible_output != PpuVisibleOutputState::Driving {
            return;
        }

        let visible_x = self.bg_pipeline_state.visible_pixels_output as usize;
        let start = visible_x.saturating_sub(retroactive_pixels);
        for x in start..visible_x {
            let mixed_pixel = self.current_scanline_mixed_pixels[x];
            if !register_affects_pixel(register, mixed_pixel) {
                continue;
            }

            let use_transient_palette = match register {
                PpuPaletteRegister::Bgp => x == start,
                PpuPaletteRegister::Obp0 | PpuPaletteRegister::Obp1 => x == start,
            };
            let palette = if use_transient_palette {
                transient_palette
            } else {
                final_palette
            };
            let panel_pixel = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                mixed_pixel,
                register,
                palette,
            );
            self.framebuffer[self.ly as usize * SCREEN_WIDTH + x] = panel_pixel;
        }
    }

    fn map_mixed_pixel_to_panel_shade_with_palette_override(
        &self,
        pixel: MixedPixel,
        register: PpuPaletteRegister,
        palette_override: u8,
    ) -> u8 {
        match pixel.source {
            MixedPixelSource::Background => {
                let palette = if register == PpuPaletteRegister::Bgp {
                    palette_override
                } else {
                    self.visible_registers.bgp
                };
                self.apply_dmg_palette(palette, pixel.color)
            }
            MixedPixelSource::Object { palette_obp1 } => {
                let palette = match (register, palette_obp1) {
                    (PpuPaletteRegister::Obp0, false) | (PpuPaletteRegister::Obp1, true) => {
                        palette_override
                    }
                    _ => self
                        .visible_registers
                        .obj_palette(palette_obp1, self.obj_palette_read_policy),
                };
                self.apply_dmg_palette(palette, pixel.color)
            }
        }
    }

    fn dmg_palette_conflict_retroactive_pixels(
        &self,
        register: PpuPaletteRegister,
    ) -> Option<usize> {
        if !self.console_model.is_dmg_family() || self.ly >= VISIBLE_SCANLINES {
            return None;
        }

        let retroactive_pixels = match register {
            PpuPaletteRegister::Bgp => DMG_PALETTE_RETROACTIVE_PIXELS,
            PpuPaletteRegister::Obp0 | PpuPaletteRegister::Obp1 => {
                DMG_PALETTE_RETROACTIVE_PIXELS + 1
            }
        };

        match self.current_raster_state() {
            PpuRasterState::Active {
                mode: PpuAccessMode::Drawing,
                ..
            } => Some(retroactive_pixels),
            PpuRasterState::Active {
                mode: PpuAccessMode::HBlank,
                mode_dot,
                ..
            } if mode_dot < 4 => Some(retroactive_pixels.saturating_sub(1)),
            PpuRasterState::Disabled
            | PpuRasterState::LcdRestartFirstLine { .. }
            | PpuRasterState::Active { .. } => None,
        }
    }

    fn record_dmg_bgp_cpu_commit_visible_write(&mut self, value: u8) {
        if !self.console_model.is_dmg_family()
            || self.ly >= VISIBLE_SCANLINES
            || self.visible_output != PpuVisibleOutputState::Driving
        {
            return;
        }

        self.dmg_bgp_cpu_commit_current_line_writes
            .push(PpuDmgBgpCpuCommitWrite {
                visible_pixels_output: self.bg_pipeline_state.visible_pixels_output,
                value,
            });
    }

    fn finalize_dmg_bgp_cpu_commit_scanline(&mut self) {
        if self.console_model.is_dmg_family()
            && self.ly < VISIBLE_SCANLINES
            && self.visible_output == PpuVisibleOutputState::Driving
        {
            if let Some(previous_ly) = self.previous_scanline_ly
                && previous_ly + 1 == self.ly
                && previous_ly % 8 == 7
                && self.ly.is_multiple_of(8)
                && !self.dmg_bgp_cpu_commit_current_line_writes.is_empty()
                && self.dmg_bgp_cpu_commit_current_line_writes
                    != self.dmg_bgp_cpu_commit_previous_line_writes
            {
                self.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(previous_ly);
            }

            self.previous_scanline_mixed_pixels = self.current_scanline_mixed_pixels;
            self.previous_scanline_ly = Some(self.ly);
            self.dmg_bgp_cpu_commit_previous_line_writes =
                self.dmg_bgp_cpu_commit_current_line_writes.clone();
        } else {
            self.previous_scanline_ly = None;
            self.dmg_bgp_cpu_commit_previous_line_writes.clear();
        }
    }

    fn recolor_previous_scanline_from_current_bgp_cpu_commit_writes(&mut self, previous_ly: u8) {
        let row_start = previous_ly as usize * SCREEN_WIDTH;
        for x in 0..SCREEN_WIDTH {
            let palette = self
                .dmg_bgp_cpu_commit_palette_for_visible_x(self.dmg_bgp_cpu_commit_current_line_start_palette, x);
            let mixed_pixel = self.previous_scanline_mixed_pixels[x];
            self.framebuffer[row_start + x] = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                mixed_pixel,
                PpuPaletteRegister::Bgp,
                palette,
            );
        }
    }

    fn dmg_bgp_cpu_commit_palette_for_visible_x(&self, start_palette: u8, x: usize) -> u8 {
        let mut palette = start_palette;
        for write in &self.dmg_bgp_cpu_commit_current_line_writes {
            let effective_x = usize::from(write.visible_pixels_output).saturating_add(4);
            if x >= effective_x {
                palette = write.value;
            } else {
                break;
            }
        }
        palette
    }
}
