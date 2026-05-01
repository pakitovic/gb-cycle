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
        self.compute_fetch_tile_data_address_with_attributes(
            source, fetch_x, tile_index, plane, None,
        )
    }

    pub(super) fn compute_fetch_tile_data_address_with_attributes(
        &self,
        source: PpuBgFetcherSource,
        fetch_x: u16,
        tile_index: u8,
        plane: u16,
        attributes: Option<CgbBgTileAttributes>,
    ) -> u16 {
        let attributes = attributes.unwrap_or_default();
        match source {
            PpuBgFetcherSource::Background => {
                let context = self.background_fetch_context(fetch_x);
                context.tile_data_address_for_row(
                    tile_index,
                    cgb_bg_effective_tile_row(context.tile_data_row(), attributes),
                    plane,
                )
            }
            PpuBgFetcherSource::Window => {
                let context = self.window_fetch_context();
                context.tile_data_address_for_row(
                    tile_index,
                    cgb_bg_effective_tile_row(context.tile_data_row(), attributes),
                    plane,
                )
            }
        }
    }

    pub(super) fn compute_window_fetch_tile_data_address_with_selector_and_attributes(
        &self,
        tile_index: u8,
        plane: u16,
        selector: BgTileDataSelect,
        attributes: Option<CgbBgTileAttributes>,
    ) -> u16 {
        let mut registers = self.mode3_register_latches().visible();
        registers.lcdc = selector.apply_to_lcdc(registers.lcdc);
        let context = PpuMode3WindowFetchContext::new(
            registers,
            self.current_window_line_counter(),
            self.bg_pipeline_state.fetcher.window_tilemap_x,
        );
        context.tile_data_address_for_row(
            tile_index,
            cgb_bg_effective_tile_row(context.tile_data_row(), attributes.unwrap_or_default()),
            plane,
        )
    }

    pub(super) fn read_cgb_bg_tile_attributes(
        &self,
        vram: &VramBusView<'_>,
        tile_map_address: u16,
    ) -> Option<CgbBgTileAttributes> {
        if !self.console_model.is_cgb_family() {
            return None;
        }

        Some(CgbBgTileAttributes::new(
            vram.read_bank(CGB_BG_ATTR_BANK, tile_map_address as usize)
                .unwrap_or(0),
        ))
    }

    pub(super) fn read_bg_tile_data_byte(
        &self,
        vram: &VramBusView<'_>,
        attributes: Option<CgbBgTileAttributes>,
        address: u16,
    ) -> u8 {
        vram.read_bank(
            attributes.unwrap_or_default().tile_vram_bank(),
            address as usize,
        )
        .unwrap_or(0)
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
        if source == PpuBgFetcherSource::Window
            && plane == 0
            && self
                .bg_pipeline_state
                .fetcher
                .dmg_lcdc4_skip_window_current_low_glitch
        {
            self.bg_pipeline_state
                .fetcher
                .dmg_lcdc4_skip_window_current_low_glitch = false;
            return;
        }

        let fetch_policy = self.mode3_bgwin_fetch_policy();
        if !fetch_policy.tile_data_selector_changed() {
            return;
        }

        let tile_index = self.bg_pipeline_state.fetcher.tile_index;
        let attributes = self.bg_pipeline_state.fetcher.cgb_bg_attrs;
        let tile_data_address = self.compute_fetch_tile_data_address_with_attributes(
            source,
            self.bg_pipeline_state.fetcher.fetch_x,
            tile_index,
            plane,
            attributes,
        );
        let tile_byte = self.read_bg_tile_data_byte(vram, attributes, tile_data_address);

        let fetcher = &mut self.bg_pipeline_state.fetcher;
        fetcher.tile_data_address = tile_data_address;

        if plane == 0 {
            fetcher.tile_low_address = tile_data_address;
            fetcher.tile_low = tile_byte;
        } else {
            fetcher.tile_high_address = tile_data_address;
            fetcher.tile_high = tile_byte;
        }
    }

    pub(super) fn maybe_apply_bgwin_tilemap_selector_glitch(
        &mut self,
        vram: &VramBusView<'_>,
        source: PpuBgFetcherSource,
    ) {
        if source == PpuBgFetcherSource::Window && self.console_model.is_dmg_family() {
            return;
        }

        let fetch_policy = self.mode3_bgwin_fetch_policy();
        if !fetch_policy.tilemap_selector_changed(source) {
            return;
        }

        let tile_map_address =
            self.compute_fetch_tile_index_address(source, self.bg_pipeline_state.fetcher.fetch_x);
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let cgb_bg_attrs = self.read_cgb_bg_tile_attributes(vram, tile_map_address);
        let fetcher = &mut self.bg_pipeline_state.fetcher;
        fetcher.tile_map_address = tile_map_address;
        fetcher.tile_index = tile_index;
        fetcher.cgb_bg_attrs = cgb_bg_attrs;
    }

    pub(super) fn read_obj_tile_data_byte_for_resolved_tile(
        &mut self,
        vram: &VramBusView<'_>,
        sprite: PpuSelectedSprite,
        tile_index: u8,
        tile_row: u8,
        plane: u16,
    ) -> u8 {
        let byte_address =
            tile_index as u16 * TILE_BYTES + tile_row as u16 * TILE_ROW_BYTES + plane;
        let tile_data = vram
            .read_bank(self.obj_tile_data_vram_bank(sprite), byte_address as usize)
            .unwrap_or(0);
        self.last_unsigned_tile_data_fetch = tile_data;
        tile_data
    }

    pub(super) fn obj_tile_data_vram_bank(&self, sprite: PpuSelectedSprite) -> u8 {
        self.cgb_obj_attributes(sprite)
            .map(CgbObjAttributes::tile_vram_bank)
            .unwrap_or(0)
    }

    pub(super) fn cgb_obj_attributes(&self, sprite: PpuSelectedSprite) -> Option<CgbObjAttributes> {
        self.console_model
            .is_cgb_family()
            .then_some(CgbObjAttributes::new(sprite.attributes))
    }

    pub(super) fn obj_pixel_from_sprite(&self, sprite: PpuSelectedSprite, color: u8) -> ObjPixel {
        ObjPixel {
            color,
            palette_obp1: sprite.attributes & CGB_OBJ_ATTR_DMG_PALETTE_BIT != 0,
            bg_over_obj: sprite.attributes & CGB_OBJ_ATTR_BG_OVER_OBJ_BIT != 0,
            cgb_obj_attrs: self.cgb_obj_attributes(sprite),
            sprite_x: sprite.x,
            oam_index: sprite.oam_index,
        }
    }

    pub(super) fn obj_tile_index_and_row(&self, sprite: PpuSelectedSprite) -> Option<(u8, u8)> {
        self.obj_tile_index_and_row_for_height(sprite, self.current_obj_height())
    }

    pub(super) fn obj_tile_index_and_row_for_height(
        &self,
        sprite: PpuSelectedSprite,
        height: u8,
    ) -> Option<(u8, u8)> {
        let sprite_top = sprite.y.wrapping_sub(16);
        let mut row = self.ly.wrapping_sub(sprite_top);
        if row >= height {
            return None;
        }
        if sprite.attributes & CGB_OBJ_ATTR_Y_FLIP_BIT != 0 {
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

    pub(super) fn obj_tile_index_and_row_for_mode3_fetch(
        &self,
        sprite: PpuSelectedSprite,
        selected_obj_height: u8,
        fetch_obj_height: u8,
    ) -> Option<(u8, u8)> {
        if fetch_obj_height != 8 || selected_obj_height != 16 {
            return self.obj_tile_index_and_row_for_height(sprite, fetch_obj_height);
        }

        let sprite_top = sprite.y.wrapping_sub(16);
        let raw_row = self.ly.wrapping_sub(sprite_top);
        if raw_row >= selected_obj_height {
            return None;
        }

        let mut row = raw_row & 0x07;
        if sprite.attributes & CGB_OBJ_ATTR_Y_FLIP_BIT != 0 {
            row = 7 - row;
        }

        Some((sprite.tile_index, row))
    }

    pub(super) fn push_obj_pixels(
        &mut self,
        sprite: PpuSelectedSprite,
        tile_low: u8,
        tile_high: u8,
        current_visible_x: u8,
    ) {
        let sprite_screen_x = sprite_screen_x(sprite);
        let fifo_front_screen_x =
            self.obj_fifo_front_screen_x_for_sprite(current_visible_x, sprite_screen_x);
        for tile_pixel in 0..BG_TILE_WIDTH {
            let color = obj_tile_pixel_value(tile_low, tile_high, tile_pixel, sprite.attributes);
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

            let candidate = self.obj_pixel_from_sprite(sprite, color);

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

    pub(super) fn rewrite_obj_fifo_pixels(
        &mut self,
        sprite: PpuSelectedSprite,
        tile_low: u8,
        tile_high: u8,
        current_visible_x: u8,
    ) {
        let sprite_screen_x = sprite_screen_x(sprite);
        let fifo_front_screen_x =
            self.obj_fifo_front_screen_x_for_sprite(current_visible_x, sprite_screen_x);
        for tile_pixel in 0..BG_TILE_WIDTH {
            let candidate = self.obj_pixel_from_sprite(
                sprite,
                obj_tile_pixel_value(tile_low, tile_high, tile_pixel, sprite.attributes),
            );
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

            let slot = self
                .obj_pipeline_state
                .fifo
                .get_mut(offset)
                .expect("OBJ FIFO was extended to cover the target offset");
            let same_sprite = slot.sprite_x == sprite.x && slot.oam_index == sprite.oam_index;
            if same_sprite || obj_pixel_has_priority(candidate, *slot) {
                *slot = candidate;
            }
        }
    }

    fn obj_fifo_front_screen_x(&self, current_visible_x: u8) -> i16 {
        current_visible_x as i16 - self.obj_fifo_hidden_pops_before_first_visible_pixel() as i16
    }

    fn obj_fifo_front_screen_x_for_sprite(
        &self,
        current_visible_x: u8,
        sprite_screen_x: i16,
    ) -> i16 {
        if sprite_screen_x < 0 {
            current_visible_x as i16
                - self.obj_fifo_hidden_pops_before_first_visible_pixel_raw() as i16
        } else {
            self.obj_fifo_front_screen_x(current_visible_x)
        }
    }

    fn obj_fifo_hidden_pops_before_first_visible_pixel_raw(&self) -> usize {
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

    pub(super) fn obj_fifo_hidden_pops_before_first_visible_pixel(&self) -> usize {
        if self.bg_pipeline_state.visible_pixels_output > 0 {
            return 0;
        }

        let mut current_transfer_x = self.bg_pipeline_state.current_transfer_x;
        let mut scx_discard_remaining = self.bg_pipeline_state.scx_discard_remaining;
        let initial_scx_discard = self.bg_pipeline_state.initial_scx_discard;
        let mut startup_pre_visible_transfer_dots_remaining = self
            .bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining;
        let mut hidden_pops = 0usize;

        while scx_discard_remaining > 0
            || current_transfer_x.saturating_add(initial_scx_discard) < 8
        {
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

    pub(super) fn mix_bg_and_obj(
        &self,
        bg_pixel: u8,
        cgb_bg_attrs: Option<CgbBgTileAttributes>,
        effective_bg_priority_pixel: u8,
        obj_pixel: ObjPixel,
    ) -> MixedPixel {
        if !self.pixel_transfer_obj_enabled() || obj_pixel.is_transparent() {
            return MixedPixel::background_with_cgb_attrs(bg_pixel, cgb_bg_attrs);
        }

        if obj_pixel.bg_over_obj && effective_bg_priority_pixel != 0 {
            MixedPixel::background_with_cgb_attrs(bg_pixel, cgb_bg_attrs)
        } else {
            MixedPixel::object_with_cgb_attrs(
                obj_pixel.color,
                obj_pixel.palette_obp1,
                obj_pixel.cgb_obj_attrs,
            )
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

    pub(super) fn map_mixed_pixel_to_cgb_rgb555(&self, pixel: MixedPixel) -> u16 {
        match pixel.source {
            MixedPixelSource::Background => {
                let attrs = pixel.cgb_bg_attrs.unwrap_or_default();
                self.cgb_palettes
                    .port(CgbPaletteKind::Background)
                    .rgb555(attrs.palette_index(), pixel.color)
            }
            MixedPixelSource::Object { .. } => {
                let attrs = pixel.cgb_obj_attrs.unwrap_or_default();
                self.cgb_palettes
                    .port(CgbPaletteKind::Object)
                    .rgb555(attrs.palette_index(), pixel.color)
            }
        }
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

    pub(super) fn dmg_bg_color_for_panel_shade(&self, panel_shade: u8) -> u8 {
        (0..=3)
            .find(|color| self.apply_dmg_palette(self.pixel_pipeline_bgp(), *color) == panel_shade)
            .unwrap_or(0)
    }
}
