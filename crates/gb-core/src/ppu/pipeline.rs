use super::*;

impl Ppu {
    pub(super) fn compute_fetch_tile_index_address(
        &self,
        source: PpuBgFetcherSource,
        next_fetch_pixel: u16,
    ) -> u16 {
        match source {
            PpuBgFetcherSource::Background => self
                .background_fetch_context(next_fetch_pixel)
                .tile_index_address(),
            PpuBgFetcherSource::Window => self.window_fetch_context().tile_index_address(),
        }
    }

    pub(super) fn compute_fetch_tile_data_address(
        &self,
        source: PpuBgFetcherSource,
        fetch_x: u16,
        tile_index: u8,
        plane: u16,
    ) -> u16 {
        match source {
            PpuBgFetcherSource::Background => self
                .background_fetch_context(fetch_x)
                .tile_data_address(tile_index, plane),
            PpuBgFetcherSource::Window => self
                .window_fetch_context()
                .tile_data_address(tile_index, plane),
        }
    }

    pub(super) fn maybe_cache_unsigned_bgwin_tile_data_fetch(
        &mut self,
        source: PpuBgFetcherSource,
        next_fetch_pixel: u16,
        plane: u16,
        tile_data: u8,
    ) {
        if self.bg_pipeline_state.startup_alignment_seed_pending() {
            return;
        }
        let uses_unsigned_tile_data = match source {
            PpuBgFetcherSource::Background => self
                .background_fetch_context(next_fetch_pixel)
                .uses_unsigned_tile_data(),
            PpuBgFetcherSource::Window => self.window_fetch_context().uses_unsigned_tile_data(),
        };
        if uses_unsigned_tile_data {
            self.last_unsigned_tile_data_fetch = tile_data;
            if plane == 0 {
                self.last_unsigned_tile_data_low_fetch = tile_data;
            } else {
                self.last_unsigned_tile_data_high_fetch = tile_data;
            }
        }
    }

    pub(super) fn maybe_apply_bgwin_tile_data_selector_glitch(
        &mut self,
        vram: &VramBusView<'_>,
        source: PpuBgFetcherSource,
        plane: u16,
    ) {
        let fetch_policy = self.mode3_bgwin_fetch_policy();
        if !fetch_policy.tile_data_selector_changed() {
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

    pub(super) fn maybe_apply_bgwin_tilemap_selector_glitch(
        &mut self,
        vram: &VramBusView<'_>,
        source: PpuBgFetcherSource,
    ) {
        let fetch_policy = self.mode3_bgwin_fetch_policy();
        if !fetch_policy.tilemap_selector_changed(source) {
            return;
        }

        let tile_map_address =
            self.compute_fetch_tile_index_address(source, self.bg_pipeline_state.fetcher.fetch_x);
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let fetcher = &mut self.bg_pipeline_state.fetcher;
        fetcher.tile_map_address = tile_map_address;
        fetcher.tile_index = tile_index;
    }

    pub(super) fn read_obj_tile_data_byte(
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

    pub(super) fn obj_tile_index_and_row(&self, sprite: PpuSelectedSprite) -> Option<(u8, u8)> {
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

    pub(super) fn push_obj_pixels(
        &mut self,
        sprite: PpuSelectedSprite,
        tile_low: u8,
        tile_high: u8,
        current_visible_x: u8,
    ) {
        let sprite_screen_x = sprite_screen_x(sprite);
        let fifo_front_screen_x = self.obj_fifo_front_screen_x(current_visible_x);
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
            if screen_x < fifo_front_screen_x || screen_x >= SCREEN_WIDTH as i16 {
                continue;
            }
            if current_visible_x > 0 && screen_x < current_visible_x as i16 {
                continue;
            }

            let offset = (screen_x - fifo_front_screen_x) as usize;
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

    fn obj_fifo_front_screen_x(&self, current_visible_x: u8) -> i16 {
        current_visible_x as i16 - self.obj_fifo_hidden_pops_before_first_visible_pixel() as i16
    }

    pub(super) fn obj_fifo_hidden_pops_before_first_visible_pixel(&self) -> usize {
        if self.bg_pipeline_state.visible_pixels_output > 0 {
            return 0;
        }

        let mut current_transfer_x = self.bg_pipeline_state.current_transfer_x;
        let mut scx_discard_remaining = self.bg_pipeline_state.scx_discard_remaining;
        let mut startup_pre_visible_transfer_dots_remaining = self
            .bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining;
        let mut hidden_pops = 0usize;

        while scx_discard_remaining > 0 || current_transfer_x < 8 {
            if scx_discard_remaining > 0 {
                scx_discard_remaining -= 1;
                continue;
            }

            current_transfer_x += 1;
            if startup_pre_visible_transfer_dots_remaining > 0 {
                startup_pre_visible_transfer_dots_remaining -= 1;
            } else {
                hidden_pops += 1;
            }
        }

        hidden_pops
    }

    pub(super) fn pop_obj_fifo_pixel(&mut self) -> ObjPixel {
        self.obj_pipeline_state
            .fifo
            .pop_front()
            .unwrap_or_else(ObjPixel::transparent)
    }

    pub(super) fn mix_bg_and_obj(&self, bg_pixel: u8, obj_pixel: ObjPixel) -> MixedPixel {
        if !self.pixel_transfer_obj_enabled() || obj_pixel.is_transparent() {
            return MixedPixel::background(bg_pixel);
        }

        if obj_pixel.bg_over_obj && bg_pixel != 0 {
            MixedPixel::background(bg_pixel)
        } else {
            MixedPixel::object(obj_pixel.color, obj_pixel.palette_obp1)
        }
    }

    pub(super) fn map_mixed_pixel_to_panel_shade(&self, pixel: MixedPixel) -> u8 {
        let visible_registers = self.mode3_register_latches().visible();
        let palette = visible_registers.palette_for_mixed_pixel(
            pixel,
            self.pixel_pipeline_bgp(),
            self.obj_palette_read_policy,
        );
        self.apply_dmg_palette(palette, pixel.color)
    }

    pub(super) fn dmg_bg_panel_dot_is_forced_white(
        &self,
        bg_enabled: bool,
        pixel: MixedPixel,
    ) -> bool {
        self.visible_output == PpuVisibleOutputState::Driving
            && self.console_model.is_dmg_family()
            && !bg_enabled
            && matches!(pixel.source, MixedPixelSource::Background)
    }

    pub(super) fn apply_dmg_palette(&self, palette: u8, color: u8) -> u8 {
        (palette >> (u32::from(color & 0x03) * 2)) & 0x03
    }
}
