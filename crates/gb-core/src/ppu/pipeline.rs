use super::*;

impl Ppu {
    pub(super) fn compute_fetch_tile_index_address(
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

    pub(super) fn compute_fetch_tile_data_address(
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

    pub(super) fn bg_fetch_tilemap_uses_pipeline_snapshot(&self, next_fetch_pixel: u16) -> bool {
        let _ = next_fetch_pixel;
        self.console_model.is_dmg_family()
            && self
                .bg_pipeline_state
                .startup_background_tilemap_uses_pipeline_snapshot()
    }

    pub(super) fn bg_fetch_tiledata_uses_pipeline_snapshot(&self, next_fetch_pixel: u16) -> bool {
        let _ = next_fetch_pixel;
        self.console_model.is_dmg_family()
            && self
                .bg_pipeline_state
                .startup_background_tiledata_uses_pipeline_snapshot()
    }

    pub(super) fn bg_fetch_tilemap_lcdc(&self, next_fetch_pixel: u16) -> u8 {
        if self.bg_fetch_tilemap_uses_pipeline_snapshot(next_fetch_pixel) {
            self.pipeline_registers.lcdc
        } else {
            self.visible_registers.lcdc
        }
    }

    pub(super) fn bg_fetch_tiledata_lcdc(&self, next_fetch_pixel: u16) -> u8 {
        if self.bg_fetch_tiledata_uses_pipeline_snapshot(next_fetch_pixel) {
            self.pipeline_registers.lcdc
        } else {
            self.visible_registers.lcdc
        }
    }

    pub(super) fn bg_fetch_scy(&self, next_fetch_pixel: u16) -> u8 {
        if self.bg_fetch_tiledata_uses_pipeline_snapshot(next_fetch_pixel) {
            self.pipeline_registers.scy
        } else {
            self.visible_registers.scy
        }
    }

    pub(super) fn window_fetch_lcdc(&self) -> u8 {
        self.visible_registers.lcdc
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

    pub(super) fn maybe_apply_bgwin_tile_data_selector_glitch(
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

    pub(super) fn maybe_apply_bgwin_tilemap_selector_glitch(
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

    fn obj_fifo_hidden_pops_before_first_visible_pixel(&self) -> usize {
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

    pub(super) fn apply_dmg_palette(&self, palette: u8, color: u8) -> u8 {
        (palette >> (u32::from(color & 0x03) * 2)) & 0x03
    }

    pub(super) fn write_dmg_palette_register(
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
            self.clear_dmg_bgp_cpu_commit_bg_visible_hold();
            let visible_pixels_output = self.bg_pipeline_state.visible_pixels_output;
            if let Some(retroactive_pixels) = self.dmg_palette_conflict_retroactive_pixels(register)
            {
                if let Some((transient_start_x, final_onset_x)) =
                    self.dmg_single_left_sprite_bgp_second_write_transient_range()
                {
                    self.apply_single_left_sprite_bgp_second_write_transient_range(
                        register,
                        previous_visible,
                        value,
                        visible_pixels_output,
                        transient_start_x,
                        final_onset_x,
                    );
                    return;
                }

                if let Some(desired_onset_x) = self
                    .dmg_single_left_sprite_bgp_live_write_onset_visible_x(
                        self.dmg_bgp_cpu_commit_current_line_writes.len(),
                    )
                {
                    self.apply_single_left_sprite_bgp_live_write_onset(
                        register,
                        previous_visible,
                        value,
                        visible_pixels_output,
                        desired_onset_x,
                    );
                    return;
                }

                let base_effect_kind = self.dmg_bgp_cpu_commit_effect_kind(retroactive_pixels);
                let effective_retroactive_pixels = retroactive_pixels;
                let line_has_pipeline_delayed = self
                    .dmg_bgp_cpu_commit_current_line_writes
                    .iter()
                    .any(|write| {
                        write.effect_kind == PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
                    });
                let retroactive_write_count = self
                    .dmg_bgp_cpu_commit_current_line_writes
                    .iter()
                    .filter(|write| {
                        write.effect_kind == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                    })
                    .count();
                let first_retroactive_write = self
                    .dmg_bgp_cpu_commit_current_line_writes
                    .iter()
                    .find(|write| {
                        write.effect_kind == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                    });
                let transient_palette = previous_visible | value;
                let transient_visible_x =
                    visible_pixels_output.saturating_sub(effective_retroactive_pixels as u8);
                let repaint_visible_x = visible_pixels_output.saturating_add(4);
                let selected_retroactive_line = base_effect_kind
                    == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                    && line_has_pipeline_delayed
                    && first_retroactive_write
                        .map_or(transient_visible_x, |write| write.transient_visible_x)
                        <= 8;
                let subsequent_selected_retroactive_write =
                    selected_retroactive_line && retroactive_write_count > 0;
                let delay_final_visible_commit =
                    selected_retroactive_line && !subsequent_selected_retroactive_write;
                let effect_kind = if subsequent_selected_retroactive_write {
                    PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient
                } else {
                    base_effect_kind
                };
                let recorded_transient_palette = transient_palette;
                let recorded_transient_visible_x =
                    if effect_kind == PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient {
                        visible_pixels_output
                    } else {
                        transient_visible_x
                    };
                let recorded_repaint_visible_x =
                    if effect_kind == PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient {
                        visible_pixels_output.saturating_add(1)
                    } else {
                        repaint_visible_x
                    };
                self.record_dmg_bgp_cpu_commit_visible_write(
                    effect_kind,
                    recorded_transient_visible_x,
                    recorded_transient_palette,
                    recorded_repaint_visible_x,
                    value,
                );
                match effect_kind {
                    PpuDmgBgpCpuCommitEffectKind::PipelineDelayed => {
                        let output_delay_pixels =
                            self.dmg_bgp_cpu_commit_output_delay_pixels(visible_pixels_output);
                        self.dmg_bgp_cpu_commit_output_palette_override =
                            (output_delay_pixels > 0).then(|| self.pixel_pipeline_bgp());
                        self.dmg_bgp_cpu_commit_output_delay_pixels_remaining = output_delay_pixels;
                    }
                    PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient => {
                        self.dmg_bgp_cpu_commit_output_palette_override = Some(transient_palette);
                        self.dmg_bgp_cpu_commit_output_delay_pixels_remaining = 1;
                    }
                    PpuDmgBgpCpuCommitEffectKind::RetroactivePanel => {
                        self.retroactively_recolor_recent_pixels(
                            register,
                            transient_palette,
                            value,
                            effective_retroactive_pixels,
                            delay_final_visible_commit,
                        );
                        if !delay_final_visible_commit
                            && self.dmg_bgp_cpu_commit_current_line_writes.len() == 1
                            && let Some(bg_visible_pixels) =
                                self.early_line_retroactive_obj_hold_bg_visible_pixels()
                        {
                            self.start_dmg_bgp_cpu_commit_bg_visible_hold(
                                value,
                                bg_visible_pixels,
                                value,
                            );
                            self.dmg_bgp_cpu_commit_output_palette_override = None;
                            self.dmg_bgp_cpu_commit_output_delay_pixels_remaining = 0;
                        } else if delay_final_visible_commit {
                            let output_delay_pixels = self
                                .dmg_bgp_cpu_commit_retroactive_final_delay_pixels(
                                    visible_pixels_output,
                                );
                            if output_delay_pixels == 0 {
                                self.dmg_bgp_cpu_commit_output_palette_override = Some(value);
                                self.dmg_bgp_cpu_commit_output_delay_pixels_remaining = 1;
                            } else {
                                self.dmg_bgp_cpu_commit_output_palette_override =
                                    Some(self.pixel_pipeline_bgp());
                                self.dmg_bgp_cpu_commit_output_delay_pixels_remaining =
                                    output_delay_pixels;
                            }
                        } else {
                            self.dmg_bgp_cpu_commit_output_palette_override = Some(value);
                            self.dmg_bgp_cpu_commit_output_delay_pixels_remaining = 1;
                        }
                    }
                }
            }
        }

        if !bgp_cpu_commit_delay_active
            && let Some(retroactive_pixels) = self.dmg_palette_conflict_retroactive_pixels(register)
        {
            self.retroactively_recolor_recent_pixels(
                register,
                previous_visible | value,
                value,
                retroactive_pixels,
                false,
            );
        }
    }

    pub(super) fn dmg_bgp_cpu_commit_effect_kind(
        &self,
        retroactive_pixels: usize,
    ) -> PpuDmgBgpCpuCommitEffectKind {
        let mut affected_pixel_count = 0usize;
        let mut recent_affected_pixels_are_bg_color0 = true;
        if !self.dmg_recent_panel_dots.is_empty() {
            let recent_dots = self
                .dmg_recent_panel_dots
                .iter()
                .rev()
                .take(retroactive_pixels)
                .copied()
                .collect::<Vec<_>>();
            for dot in recent_dots.iter().rev() {
                if !register_affects_pixel(PpuPaletteRegister::Bgp, dot.pixel) {
                    continue;
                }

                affected_pixel_count += 1;
                if dot.pixel.color != 0 {
                    recent_affected_pixels_are_bg_color0 = false;
                    break;
                }
            }
        } else {
            let visible_x = self.bg_pipeline_state.visible_pixels_output as usize;
            let start = visible_x.saturating_sub(retroactive_pixels);

            for pixel in &self.current_scanline_mixed_pixels[start..visible_x] {
                if !register_affects_pixel(PpuPaletteRegister::Bgp, *pixel) {
                    continue;
                }

                affected_pixel_count += 1;
                if pixel.color != 0 {
                    recent_affected_pixels_are_bg_color0 = false;
                    break;
                }
            }
        }

        if affected_pixel_count > 0 && recent_affected_pixels_are_bg_color0 {
            PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
        } else {
            PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
        }
    }

    pub(super) fn retroactively_recolor_recent_pixels(
        &mut self,
        register: PpuPaletteRegister,
        transient_palette: u8,
        final_palette: u8,
        retroactive_pixels: usize,
        delay_final_palette: bool,
    ) {
        if self.visible_output != PpuVisibleOutputState::Driving {
            return;
        }

        let mut transient_palette_pending = true;
        if !self.dmg_recent_panel_dots.is_empty()
            && !self.use_scanline_palette_conflict_positions(register)
        {
            let recent_dots = self.recent_palette_conflict_panel_dots(register, retroactive_pixels);
            for dot in recent_dots.iter().rev() {
                if !register_affects_pixel(register, dot.pixel) {
                    continue;
                }

                let use_transient_palette = transient_palette_pending;
                transient_palette_pending = false;
                if !use_transient_palette && delay_final_palette {
                    continue;
                }
                let palette = if use_transient_palette {
                    transient_palette
                } else {
                    final_palette
                };
                let panel_pixel = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                    dot.pixel, register, palette,
                );
                self.framebuffer[self.ly as usize * SCREEN_WIDTH + usize::from(dot.visible_x)] =
                    panel_pixel;
            }
            return;
        }

        for x in self.recent_palette_conflict_scanline_positions(register, retroactive_pixels) {
            let mixed_pixel = self.current_scanline_mixed_pixels[x];
            if !register_affects_pixel(register, mixed_pixel) {
                continue;
            }

            let use_transient_palette = transient_palette_pending;
            transient_palette_pending = false;
            if !use_transient_palette && delay_final_palette {
                continue;
            }
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

    pub(super) fn consume_dmg_bgp_cpu_commit_output_delay(&mut self) {
        if self.dmg_bgp_cpu_commit_output_delay_pixels_remaining == 0 {
            return;
        }

        self.dmg_bgp_cpu_commit_output_delay_pixels_remaining -= 1;
        if self.dmg_bgp_cpu_commit_output_delay_pixels_remaining == 0 {
            self.dmg_bgp_cpu_commit_output_palette_override = None;
        }
    }

    pub(super) fn consume_dmg_bgp_cpu_commit_bg_visible_hold(&mut self, output_pixel: MixedPixel) {
        if self
            .dmg_bgp_cpu_commit_bg_visible_hold_palette_override
            .is_none()
            || !matches!(output_pixel.source, MixedPixelSource::Background)
            || self.dmg_bgp_cpu_commit_bg_visible_hold_bg_pixels_remaining == 0
        {
            return;
        }

        self.dmg_bgp_cpu_commit_bg_visible_hold_bg_pixels_remaining -= 1;
        if self.dmg_bgp_cpu_commit_bg_visible_hold_bg_pixels_remaining == 0 {
            self.dmg_bgp_cpu_commit_bg_visible_hold_palette_override = self
                .dmg_bgp_cpu_commit_bg_visible_hold_fallback_palette
                .take();
        }
    }

    pub(super) fn clear_dmg_bgp_cpu_commit_bg_visible_hold(&mut self) {
        self.dmg_bgp_cpu_commit_bg_visible_hold_palette_override = None;
        self.dmg_bgp_cpu_commit_bg_visible_hold_bg_pixels_remaining = 0;
        self.dmg_bgp_cpu_commit_bg_visible_hold_fallback_palette = None;
    }

    fn start_dmg_bgp_cpu_commit_bg_visible_hold(
        &mut self,
        palette: u8,
        bg_visible_pixels: u8,
        fallback_palette: u8,
    ) {
        self.dmg_bgp_cpu_commit_bg_visible_hold_palette_override = Some(palette);
        self.dmg_bgp_cpu_commit_bg_visible_hold_bg_pixels_remaining = bg_visible_pixels;
        self.dmg_bgp_cpu_commit_bg_visible_hold_fallback_palette = Some(fallback_palette);
    }

    fn dmg_bgp_cpu_commit_output_delay_pixels(&self, visible_pixels_output: u8) -> u8 {
        if visible_pixels_output == 0 && self.dmg_recent_panel_dots.is_empty() {
            let leading_visible_obj_pixels = self.leading_visible_obj_fifo_prefix_pixels();
            if leading_visible_obj_pixels == 0 {
                return 0;
            }
            return leading_visible_obj_pixels.min(4) as u8;
        }

        4
    }

    fn dmg_bgp_cpu_commit_retroactive_final_delay_pixels(&self, visible_pixels_output: u8) -> u8 {
        let visible_obj_prefix_pixels =
            self.visible_obj_prefix_pixels_output_so_far(visible_pixels_output);
        if visible_obj_prefix_pixels >= 4 {
            0
        } else if visible_obj_prefix_pixels > 0 {
            1
        } else {
            self.dmg_bgp_cpu_commit_output_delay_pixels(visible_pixels_output)
        }
    }

    fn leading_visible_obj_fifo_prefix_pixels(&self) -> usize {
        let hidden_pops_before_visible = self.obj_fifo_hidden_pops_before_first_visible_pixel();

        self.obj_pipeline_state
            .fifo
            .iter()
            .skip(hidden_pops_before_visible)
            .take_while(|pixel| !pixel.is_transparent())
            .count()
    }

    fn visible_obj_prefix_pixels_output_so_far(&self, visible_pixels_output: u8) -> usize {
        self.current_scanline_mixed_pixels[..usize::from(visible_pixels_output)]
            .iter()
            .take_while(|pixel| matches!(pixel.source, MixedPixelSource::Object { .. }))
            .count()
    }

    fn dmg_single_left_sprite_bgp_live_write_phase(&self) -> Option<u8> {
        if self.mode2_scan_state.selected_sprite_count() != 1 {
            return None;
        }

        let sprite = self.mode2_scan_state.selected_sprite(0)?;
        if sprite.x >= 16 {
            return None;
        }

        Some((sprite.x % 8).min(5))
    }

    fn dmg_single_left_sprite_bgp_live_write_onset_visible_x(
        &self,
        write_index: usize,
    ) -> Option<u8> {
        if self.mode2_scan_state.selected_sprite_count() != 1 {
            return None;
        }

        let sprite_x = self.mode2_scan_state.selected_sprite(0)?.x as usize;

        // `m3_bgp_change_sprites` needs an explicit DMG seam for the first two
        // writes: the visible onset follows the left sprite phase, not the
        // generic 4-dot CPU-commit delay.
        const EARLY_WRITE0_ONSETS: [u8; 19] =
            [0, 0, 0, 1, 2, 1, 0, 0, 0, 1, 2, 3, 4, 5, 5, 5, 5, 5, 5];
        const EARLY_WRITE1_ONSETS: [u8; 19] =
            [0, 8, 9, 10, 11, 12, 12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 8, 9, 10];

        match write_index {
            0 if sprite_x < EARLY_WRITE0_ONSETS.len() => Some(EARLY_WRITE0_ONSETS[sprite_x]),
            1 if sprite_x < EARLY_WRITE1_ONSETS.len() => Some(EARLY_WRITE1_ONSETS[sprite_x]),
            2..=5 => self
                .dmg_single_left_sprite_bgp_live_write_phase()
                .map(|phase| match write_index {
                    2 => 46 + phase,
                    3 => 59 + phase,
                    4 => 91 + phase,
                    5 => 102 + phase,
                    _ => unreachable!(),
                }),
            _ => None,
        }
    }

    fn dmg_single_left_sprite_bgp_second_write_transient_range(&self) -> Option<(u8, u8)> {
        if self.mode2_scan_state.selected_sprite_count() != 1
            || self.dmg_bgp_cpu_commit_current_line_writes.len() != 1
        {
            return None;
        }

        let sprite_x = self.mode2_scan_state.selected_sprite(0)?.x;
        // The second write on the same scanline exposes a short transient seam
        // before the final palette reaches the left-edge boundary.
        let (transient_start_x, final_onset_x) = match sprite_x {
            7 => (5, 12),
            8..=14 => (sprite_x.saturating_sub(2), sprite_x.saturating_sub(1)),
            15 => (12, 12),
            16..=18 => (5, sprite_x.saturating_sub(8)),
            _ => return None,
        };
        let final_onset_x = match sprite_x {
            14 => 12,
            _ => final_onset_x,
        };

        Some((transient_start_x, final_onset_x))
    }

    fn apply_single_left_sprite_bgp_live_write_onset(
        &mut self,
        register: PpuPaletteRegister,
        previous_visible: u8,
        value: u8,
        visible_pixels_output: u8,
        desired_onset_x: u8,
    ) {
        let effect_kind = if desired_onset_x < visible_pixels_output {
            PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
        } else if desired_onset_x == visible_pixels_output {
            PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient
        } else {
            PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
        };
        self.record_dmg_bgp_cpu_commit_visible_write(
            effect_kind,
            desired_onset_x,
            value,
            desired_onset_x.saturating_add(1),
            value,
        );

        if desired_onset_x < visible_pixels_output {
            for x in usize::from(desired_onset_x)..usize::from(visible_pixels_output) {
                let mixed_pixel = self.current_scanline_mixed_pixels[x];
                if !register_affects_pixel(register, mixed_pixel) {
                    continue;
                }

                let panel_pixel = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                    mixed_pixel,
                    register,
                    value,
                );
                self.framebuffer[self.ly as usize * SCREEN_WIDTH + x] = panel_pixel;
            }
        }

        if desired_onset_x > visible_pixels_output {
            self.dmg_bgp_cpu_commit_output_palette_override = Some(previous_visible);
            self.dmg_bgp_cpu_commit_output_delay_pixels_remaining =
                desired_onset_x.saturating_sub(visible_pixels_output);
        } else {
            self.dmg_bgp_cpu_commit_output_palette_override = Some(value);
            self.dmg_bgp_cpu_commit_output_delay_pixels_remaining = 1;
        }
    }

    fn apply_single_left_sprite_bgp_second_write_transient_range(
        &mut self,
        register: PpuPaletteRegister,
        previous_visible: u8,
        value: u8,
        visible_pixels_output: u8,
        transient_start_x: u8,
        final_onset_x: u8,
    ) {
        let transient_palette = previous_visible | value;
        self.record_dmg_bgp_cpu_commit_visible_write(
            PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_start_x,
            transient_palette,
            final_onset_x,
            value,
        );

        let row_start = self.ly as usize * SCREEN_WIDTH;
        for x in usize::from(transient_start_x)..usize::from(visible_pixels_output) {
            let mixed_pixel = self.current_scanline_mixed_pixels[x];
            if !register_affects_pixel(register, mixed_pixel) {
                continue;
            }

            let palette = if x < usize::from(final_onset_x) {
                transient_palette
            } else {
                value
            };
            let panel_pixel = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                mixed_pixel,
                register,
                palette,
            );
            self.framebuffer[row_start + x] = panel_pixel;
        }

        self.dmg_bgp_cpu_commit_output_palette_override = Some(value);
        self.dmg_bgp_cpu_commit_output_delay_pixels_remaining = 1;
    }

    fn early_line_retroactive_obj_hold_bg_visible_pixels(&self) -> Option<u8> {
        if self.bg_pipeline_state.visible_pixels_output < 2 {
            return None;
        }

        let future_obj_pixels = self
            .obj_pipeline_state
            .fifo
            .iter()
            .map(|pixel| !pixel.is_transparent())
            .collect::<Vec<_>>();
        let leading_bg_visible_run = future_obj_pixels
            .iter()
            .take_while(|&&is_obj| !is_obj)
            .count();
        let obj_visible_run = future_obj_pixels[leading_bg_visible_run..]
            .iter()
            .take_while(|&&is_obj| is_obj)
            .count();
        if obj_visible_run == 0 {
            return None;
        }

        Some((leading_bg_visible_run + 3).min(u8::MAX as usize) as u8)
    }

    fn recent_palette_conflict_panel_dots(
        &self,
        _register: PpuPaletteRegister,
        retroactive_pixels: usize,
    ) -> Vec<PpuRecentPanelDot> {
        self.dmg_recent_panel_dots
            .iter()
            .rev()
            .take(retroactive_pixels)
            .copied()
            .collect()
    }

    fn recent_palette_conflict_scanline_positions(
        &self,
        register: PpuPaletteRegister,
        retroactive_pixels: usize,
    ) -> Vec<usize> {
        let visible_x = self.bg_pipeline_state.visible_pixels_output as usize;
        if matches!(register, PpuPaletteRegister::Bgp) {
            let start = visible_x.saturating_sub(retroactive_pixels);
            return (start..visible_x).collect();
        }

        let Some(last_affected_x) = (0..visible_x)
            .rev()
            .find(|&x| register_affects_pixel(register, self.current_scanline_mixed_pixels[x]))
        else {
            return Vec::new();
        };

        let mut start = last_affected_x
            .saturating_add(1)
            .saturating_sub(retroactive_pixels);
        let affected_pixels_in_window = (start..=last_affected_x)
            .filter(|&x| register_affects_pixel(register, self.current_scanline_mixed_pixels[x]))
            .count();
        if affected_pixels_in_window == 2 {
            start = last_affected_x
                .saturating_add(1)
                .saturating_sub(retroactive_pixels + 2);
        }
        let mut positions = (start..=last_affected_x).collect::<Vec<_>>();
        let affected_positions = (0..visible_x)
            .filter(|&x| register_affects_pixel(register, self.current_scanline_mixed_pixels[x]))
            .collect::<Vec<_>>();
        if matches!(
            register,
            PpuPaletteRegister::Obp0 | PpuPaletteRegister::Obp1
        ) && let Some((&first_affected_x, remaining)) = affected_positions.split_first()
        {
            let leading_affected_is_isolated = remaining
                .first()
                .is_none_or(|&next_affected_x| next_affected_x > first_affected_x + 1);
            let leading_affected_is_too_old = first_affected_x < visible_x.saturating_sub(3);
            if leading_affected_is_isolated && leading_affected_is_too_old {
                positions.retain(|&x| x != first_affected_x);
            }
        }
        positions
    }

    fn use_scanline_palette_conflict_positions(&self, register: PpuPaletteRegister) -> bool {
        matches!(
            register,
            PpuPaletteRegister::Obp0 | PpuPaletteRegister::Obp1
        ) && self.bg_pipeline_state.visible_pixels_output >= 10
    }

    pub(super) fn record_dmg_recent_panel_dot(&mut self, visible_x: u8, pixel: MixedPixel) {
        if !self.console_model.is_dmg_family() || self.ly >= VISIBLE_SCANLINES {
            return;
        }

        if self.dmg_recent_panel_dots.len() == DMG_PALETTE_RETROACTIVE_DOT_HISTORY {
            let _ = self.dmg_recent_panel_dots.pop_front();
        }
        self.dmg_recent_panel_dots
            .push_back(PpuRecentPanelDot { visible_x, pixel });
    }

    pub(super) fn repeat_last_dmg_recent_panel_dot(&mut self) {
        let Some(last_dot) = self.dmg_recent_panel_dots.back().copied() else {
            return;
        };
        self.record_dmg_recent_panel_dot(last_dot.visible_x, last_dot.pixel);
    }

    pub(super) fn map_mixed_pixel_to_panel_shade_with_palette_override(
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

    pub(super) fn dmg_palette_conflict_retroactive_pixels(
        &self,
        register: PpuPaletteRegister,
    ) -> Option<usize> {
        if !self.console_model.is_dmg_family() || self.ly >= VISIBLE_SCANLINES {
            return None;
        }

        let retroactive_pixels = match register {
            PpuPaletteRegister::Bgp => DMG_PALETTE_RETROACTIVE_PIXELS,
            PpuPaletteRegister::Obp0 | PpuPaletteRegister::Obp1 => {
                if self.bg_pipeline_state.visible_pixels_output < 10 {
                    return None;
                }
                DMG_PALETTE_RETROACTIVE_PIXELS
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

    pub(super) fn record_dmg_bgp_cpu_commit_visible_write(
        &mut self,
        effect_kind: PpuDmgBgpCpuCommitEffectKind,
        transient_visible_x: u8,
        transient_palette: u8,
        repaint_visible_x: u8,
        value: u8,
    ) {
        if !self.console_model.is_dmg_family()
            || self.ly >= VISIBLE_SCANLINES
            || self.visible_output != PpuVisibleOutputState::Driving
        {
            return;
        }

        let transfer_lead_pixels = self.bg_pipeline_state.current_transfer_x.saturating_sub(
            self.bg_pipeline_state
                .visible_pixels_output
                .saturating_add(8),
        );
        self.dmg_bgp_cpu_commit_current_line_writes
            .push(PpuDmgBgpCpuCommitWrite {
                effect_kind,
                transient_visible_x,
                transient_palette,
                repaint_visible_x,
                transfer_lead_pixels,
                value,
            });
    }

    pub(super) fn finalize_dmg_bgp_cpu_commit_scanline(&mut self) {
        if self.console_model.is_dmg_family()
            && self.ly < VISIBLE_SCANLINES
            && self.visible_output == PpuVisibleOutputState::Driving
        {
            if let Some(previous_ly) = self.previous_scanline_ly
                && previous_ly + 1 == self.ly
                && previous_ly % 8 == 7
                && self.ly.is_multiple_of(8)
                && (self
                    .dmg_bgp_cpu_commit_current_line_writes
                    .iter()
                    .any(|write| {
                        write.effect_kind == PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
                    })
                    || self.current_mode0_start_dot() > self.baseline_mode0_start_dot())
                && self.mode2_scan_state.selected_sprite_count() == 0
                && !self.dmg_bgp_cpu_commit_current_line_writes.is_empty()
                && self.dmg_bgp_cpu_commit_current_line_writes
                    != self.dmg_bgp_cpu_commit_previous_line_writes
            {
                self.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(
                    previous_ly,
                    self.current_mode0_start_dot() > self.baseline_mode0_start_dot(),
                );
            }

            self.previous_scanline_mixed_pixels = self.current_scanline_mixed_pixels;
            self.previous_scanline_ly = Some(self.ly);
            self.dmg_bgp_cpu_commit_previous_line_start_palette =
                self.dmg_bgp_cpu_commit_current_line_start_palette;
            self.dmg_bgp_cpu_commit_previous_line_writes =
                self.dmg_bgp_cpu_commit_current_line_writes.clone();
        } else {
            self.previous_scanline_ly = None;
            self.dmg_bgp_cpu_commit_previous_line_start_palette =
                self.dmg_bgp_cpu_commit_current_line_start_palette;
            self.dmg_bgp_cpu_commit_previous_line_writes.clear();
        }
    }

    pub(super) fn recolor_previous_scanline_from_current_bgp_cpu_commit_writes(
        &mut self,
        previous_ly: u8,
        include_retroactive_panel_writes: bool,
    ) {
        let boundary_writes = self.dmg_bgp_cpu_commit_boundary_repaint_writes();
        let allow_zero_start_retroactive_panel_writes = include_retroactive_panel_writes
            && !self.previous_scanline_mixed_pixels[..DMG_PALETTE_RETROACTIVE_PIXELS]
                .iter()
                .any(|pixel| matches!(pixel.source, MixedPixelSource::Object { .. }));
        let earliest_pipeline_delayed_repaint_x = boundary_writes
            .iter()
            .find(|boundary| {
                boundary.write.effect_kind == PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
            })
            .map(|boundary| boundary.write.repaint_visible_x);
        let row_start = previous_ly as usize * SCREEN_WIDTH;
        for x in 0..SCREEN_WIDTH {
            let palette = self.dmg_bgp_cpu_commit_palette_for_visible_x(
                self.dmg_bgp_cpu_commit_previous_line_start_palette,
                &boundary_writes,
                x,
                include_retroactive_panel_writes,
                allow_zero_start_retroactive_panel_writes,
                earliest_pipeline_delayed_repaint_x,
            );
            let mixed_pixel = self.previous_scanline_mixed_pixels[x];
            self.framebuffer[row_start + x] = self
                .map_mixed_pixel_to_panel_shade_with_palette_override(
                    mixed_pixel,
                    PpuPaletteRegister::Bgp,
                    palette,
                );
        }
    }

    pub(super) fn dmg_bgp_cpu_commit_palette_for_visible_x(
        &self,
        start_palette: u8,
        writes: &[PpuDmgBgpBoundaryRepaintWrite],
        x: usize,
        include_retroactive_panel_writes: bool,
        allow_zero_start_retroactive_panel_writes: bool,
        earliest_pipeline_delayed_repaint_x: Option<u8>,
    ) -> u8 {
        let mut palette = start_palette;
        let has_pipeline_delayed = writes.iter().any(|boundary| {
            boundary.write.effect_kind == PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
        });
        let row_uses_delayed_selected_current_retroactive_commit = has_pipeline_delayed
            && writes
                .iter()
                .find(|boundary| {
                    boundary.selected_current
                        && boundary.write.effect_kind
                            == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                })
                .is_some_and(|boundary| boundary.write.transient_visible_x <= 8);
        for boundary in writes {
            let write = boundary.write;
            match write.effect_kind {
                PpuDmgBgpCpuCommitEffectKind::PipelineDelayed => {
                    let repaint_threshold_x = write
                        .repaint_visible_x
                        .saturating_add(write.transfer_lead_pixels);
                    if x >= usize::from(repaint_threshold_x) {
                        palette = write.value;
                    } else {
                        break;
                    }
                }
                PpuDmgBgpCpuCommitEffectKind::RetroactivePanel => {
                    let include_write = include_retroactive_panel_writes
                        && (write.transient_visible_x > 0
                            || allow_zero_start_retroactive_panel_writes)
                        && earliest_pipeline_delayed_repaint_x
                            .is_none_or(|earliest| write.transient_visible_x >= earliest);
                    if !include_write {
                        continue;
                    }

                    let transient_x = usize::from(write.transient_visible_x);
                    let final_x = if boundary.selected_current
                        && row_uses_delayed_selected_current_retroactive_commit
                    {
                        usize::from(write.repaint_visible_x.saturating_sub(3))
                    } else {
                        transient_x
                    };
                    if x < transient_x {
                        break;
                    }

                    if x == transient_x {
                        palette = write.transient_palette;
                        continue;
                    }

                    if x >= final_x {
                        palette = write.value;
                    }
                }
                PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient => {
                    let transient_x = usize::from(write.transient_visible_x);
                    let final_x = usize::from(write.repaint_visible_x);
                    if x < transient_x {
                        break;
                    }

                    if x == transient_x {
                        palette = write.transient_palette;
                        continue;
                    }

                    if x >= final_x {
                        palette = write.value;
                    }
                }
            }
        }
        palette
    }

    fn dmg_bgp_cpu_commit_boundary_repaint_writes(&self) -> Vec<PpuDmgBgpBoundaryRepaintWrite> {
        fn repaint_onset_x(write: PpuDmgBgpCpuCommitWrite) -> u8 {
            match write.effect_kind {
                PpuDmgBgpCpuCommitEffectKind::PipelineDelayed => write.repaint_visible_x,
                PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                | PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient => write.transient_visible_x,
            }
        }

        if self.dmg_bgp_cpu_commit_current_line_writes.len()
            != self.dmg_bgp_cpu_commit_previous_line_writes.len()
        {
            return self
                .dmg_bgp_cpu_commit_current_line_writes
                .iter()
                .copied()
                .map(|write| PpuDmgBgpBoundaryRepaintWrite {
                    write,
                    selected_current: true,
                })
                .collect();
        }

        self.dmg_bgp_cpu_commit_current_line_writes
            .iter()
            .copied()
            .zip(self.dmg_bgp_cpu_commit_previous_line_writes.iter().copied())
            .map(|(current, previous)| {
                if repaint_onset_x(current) >= repaint_onset_x(previous) {
                    PpuDmgBgpBoundaryRepaintWrite {
                        write: current,
                        selected_current: true,
                    }
                } else {
                    PpuDmgBgpBoundaryRepaintWrite {
                        write: previous,
                        selected_current: false,
                    }
                }
            })
            .collect()
    }
}
