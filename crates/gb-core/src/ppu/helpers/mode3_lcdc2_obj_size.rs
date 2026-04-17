use super::*;

impl Ppu {
    fn active_dmg_lcdc2_obj_size_write(&self) -> Option<DmgLcdc2ActiveObjSizeWrite> {
        if !self.console_model.is_dmg_family() {
            return None;
        }

        self.dmg_panel_live_write_state.lcdc2.active_write()
    }

    fn dmg_lcdc2_live_obj_size_observed_decision(
        &self,
        sprite: PpuSelectedSprite,
    ) -> Option<PpuMode3Lcdc2ObjSizeObservedDecision> {
        let active_write = self.active_dmg_lcdc2_obj_size_write()?;
        let scx = self.mode3_register_latches().visible().scx;
        let sprite_top = sprite.y.wrapping_sub(16);
        let raw_row = self.ly.wrapping_sub(sprite_top);
        PpuMode3ObservedLcdc2ObjSizePhaseTable::new(sprite.x, scx, raw_row).decision(
            usize::from(active_write.write_index),
            Some(active_write.visible_x),
        )
    }

    fn dmg_lcdc2_live_obj_size_selection_bytes(
        &mut self,
        sprite: PpuSelectedSprite,
        selection: PpuMode3Lcdc2ObjSizePlaneSelection,
        vram: &VramBusView<'_>,
    ) -> Option<(u8, u8)> {
        let live8_tile = self.obj_tile_index_and_row_for_mode3_fetch(sprite, 16, 8)?;
        let line_start16_tile = self.obj_tile_index_and_row_for_height(sprite, 16)?;
        let live8_low =
            self.read_obj_tile_data_byte_for_resolved_tile(vram, live8_tile.0, live8_tile.1, 0);
        let live8_high =
            self.read_obj_tile_data_byte_for_resolved_tile(vram, live8_tile.0, live8_tile.1, 1);
        let line_start16_low = self.read_obj_tile_data_byte_for_resolved_tile(
            vram,
            line_start16_tile.0,
            line_start16_tile.1,
            0,
        );
        let line_start16_high = self.read_obj_tile_data_byte_for_resolved_tile(
            vram,
            line_start16_tile.0,
            line_start16_tile.1,
            1,
        );

        let bytes = match selection {
            PpuMode3Lcdc2ObjSizePlaneSelection::Live8 => (live8_low, live8_high),
            PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High => {
                (live8_low, line_start16_high)
            }
            PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16LowLive8High => {
                (line_start16_low, live8_high)
            }
            PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16 => {
                (line_start16_low, line_start16_high)
            }
        };

        Some(bytes)
    }

    pub(in crate::ppu) fn apply_pending_dmg_lcdc2_observed_write_effects(
        &mut self,
        vram: &VramBusView<'_>,
    ) {
        let Some(active_write) = self.active_dmg_lcdc2_obj_size_write() else {
            return;
        };
        if !active_write.observed_effects_pending() {
            return;
        }
        let current_visible_x = self.bg_pipeline_state.visible_pixels_output;
        let active_write_visible_x = active_write.visible_x;

        for sprite_slot in 0..self.mode2_scan_state.selected_sprite_count() {
            if !self.obj_pipeline_state.has_fetched(sprite_slot) {
                continue;
            }
            let Some(sprite) = self.mode2_scan_state.selected_sprite(sprite_slot) else {
                continue;
            };
            let Some(decision) = self.dmg_lcdc2_live_obj_size_observed_decision(sprite) else {
                continue;
            };
            let Some(pending_effect) = decision.pending_effect else {
                continue;
            };
            let Some((tile_low, tile_high)) = self.dmg_lcdc2_live_obj_size_selection_bytes(
                sprite,
                decision.plane_selection,
                vram,
            ) else {
                continue;
            };

            match pending_effect {
                PpuMode3Lcdc2ObjSizeObservedEffect::RetroactiveRepaint { background_only } => {
                    self.repaint_observed_obj_scanline_overlap(
                        sprite,
                        tile_low,
                        tile_high,
                        active_write_visible_x,
                        background_only,
                    );
                }
                PpuMode3Lcdc2ObjSizeObservedEffect::FifoRewrite => {
                    self.rewrite_obj_fifo_pixels(sprite, tile_low, tile_high, current_visible_x);
                }
            }
        }

        self.dmg_panel_live_write_state
            .lcdc2
            .mark_observed_effects_applied();
    }

    pub(in crate::ppu) fn dmg_lcdc2_live_obj_size_push_bytes(
        &mut self,
        sprite: PpuSelectedSprite,
        current_low: u8,
        current_high: u8,
        vram: &VramBusView<'_>,
    ) -> (u8, u8) {
        let Some(decision) = self.dmg_lcdc2_live_obj_size_observed_decision(sprite) else {
            return (current_low, current_high);
        };

        self.dmg_lcdc2_live_obj_size_selection_bytes(sprite, decision.plane_selection, vram)
            .unwrap_or((current_low, current_high))
    }

    fn selected_sprite_for_oam_index(&self, oam_index: u8) -> Option<PpuSelectedSprite> {
        (0..self.mode2_scan_state.selected_sprite_count())
            .find_map(|slot| self.mode2_scan_state.selected_sprite(slot))
            .filter(|sprite| sprite.oam_index == oam_index)
    }

    pub(in crate::ppu) fn apply_dmg_lcdc2_live_obj_size_output_override(
        &mut self,
        obj_pixel: ObjPixel,
        visible_x: u8,
        vram: &VramBusView<'_>,
    ) -> ObjPixel {
        if obj_pixel.is_transparent() {
            return obj_pixel;
        }

        let Some(sprite) = self.selected_sprite_for_oam_index(obj_pixel.oam_index) else {
            return obj_pixel;
        };
        let Some(decision) = self.dmg_lcdc2_live_obj_size_observed_decision(sprite) else {
            return obj_pixel;
        };
        let Some((tile_low, tile_high)) =
            self.dmg_lcdc2_live_obj_size_selection_bytes(sprite, decision.plane_selection, vram)
        else {
            return obj_pixel;
        };

        let sprite_screen_x = sprite_screen_x(sprite);
        let tile_pixel = i16::from(visible_x) - sprite_screen_x;
        if !(0..BG_TILE_WIDTH as i16).contains(&tile_pixel) {
            return obj_pixel;
        }

        let bit = if sprite.attributes & 0x20 != 0 {
            tile_pixel as u8
        } else {
            7 - tile_pixel as u8
        };
        let low_bit = (tile_low >> bit) & 0x01;
        let high_bit = (tile_high >> bit) & 0x01;
        ObjPixel {
            color: (high_bit << 1) | low_bit,
            ..obj_pixel
        }
    }
}
