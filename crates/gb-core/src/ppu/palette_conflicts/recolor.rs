use super::*;

impl Ppu {
    pub(in crate::ppu) fn retroactively_recolor_recent_pixels(
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

        let mut transient_palette_pending = matches!(register, PpuPaletteRegister::Bgp);
        if !self.dmg_recent_panel_dots.is_empty()
            && !self.use_scanline_palette_conflict_positions(register)
        {
            let recent_dots = self.recent_palette_conflict_panel_dots(register, retroactive_pixels);
            for dot in recent_dots.iter().rev() {
                if register == PpuPaletteRegister::Bgp && dot.dmg_bg_forced_white {
                    continue;
                }
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
            if register == PpuPaletteRegister::Bgp && self.current_scanline_dmg_bg_forced_white[x] {
                continue;
            }
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

    pub(in crate::ppu) fn record_dmg_recent_panel_dot(
        &mut self,
        visible_x: u8,
        pixel: MixedPixel,
        dmg_bg_forced_white: bool,
    ) {
        if !self.console_model.is_dmg_family() || self.ly >= VISIBLE_SCANLINES {
            return;
        }

        if self.dmg_recent_panel_dots.len() == DMG_PALETTE_RETROACTIVE_DOT_HISTORY {
            let _ = self.dmg_recent_panel_dots.pop_front();
        }
        self.dmg_recent_panel_dots.push_back(PpuRecentPanelDot {
            visible_x,
            pixel,
            dmg_bg_forced_white,
        });
    }

    pub(in crate::ppu) fn repeat_last_dmg_recent_panel_dot(&mut self) {
        let Some(last_dot) = self.dmg_recent_panel_dots.back().copied() else {
            return;
        };
        self.record_dmg_recent_panel_dot(
            last_dot.visible_x,
            last_dot.pixel,
            last_dot.dmg_bg_forced_white,
        );
    }

    pub(in crate::ppu) fn map_mixed_pixel_to_panel_shade_with_palette_override(
        &self,
        pixel: MixedPixel,
        register: PpuPaletteRegister,
        palette_override: u8,
    ) -> u8 {
        let visible_registers = self.mode3_register_latches().visible();
        let palette = visible_registers.palette_for_mixed_pixel_with_override(
            pixel,
            register,
            palette_override,
            self.visible_palette_register_value(PpuPaletteRegister::Bgp),
            self.obj_palette_read_policy,
        );
        self.apply_dmg_palette(palette, pixel.color)
    }

    pub(in crate::ppu) fn dmg_palette_conflict_retroactive_pixels(
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
}
