use super::*;

impl Ppu {
    pub(in crate::ppu) fn pixel_pipeline_bgp(&self) -> u8 {
        self.mode3_register_latches().pixel_pipeline_bgp(
            self.console_model,
            self.runtime
                .panel
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .output_palette_override,
            self.runtime
                .panel
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .bg_visible_hold_palette_override,
        )
    }

    pub(in crate::ppu) fn pixel_transfer_bg_enabled(&self) -> bool {
        let bg_enabled = self.mode3_register_latches().pixel_transfer_bg_enabled(
            self.console_model,
            self.runtime.bg_pipeline_state.current_transfer_x,
        );

        if self.console_model.is_dmg_family() {
            self.runtime
                .panel
                .dmg_panel_live_write_state
                .lcdc0
                .bg_enable_visible_hold
                .override_value
                .unwrap_or(bg_enabled)
        } else {
            bg_enabled
        }
    }

    pub(in crate::ppu) fn apply_dmg_lcdc3_live_bg_tilemap_write(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) {
        if !self.console_model.is_dmg_family() || !write_context.lcdc_changed(LCDC_BG_TILE_MAP_BIT)
        {
            return;
        }

        let write_index = self
            .runtime
            .bg_pipeline_state
            .take_next_dmg_lcdc3_current_line_bg_tilemap_write_index();

        let Some(policy) = self.dmg_single_selected_sprite_phase_policy() else {
            return;
        };

        let Some(decision) = policy.observed_lcdc3_phase_table().live_write_decision(
            write_index,
            write_context.current_lcdc() & LCDC_BG_TILE_MAP_BIT != 0,
        ) else {
            return;
        };

        if decision.clear_visible_tile2_live_refetch {
            self.runtime
                .bg_pipeline_state
                .clear_dmg_lcdc3_startup_visible_tile2_live_refetch();
        }

        if let Some(tilemap_override) = decision.tilemap_override {
            self.runtime
                .bg_pipeline_state
                .latch_dmg_lcdc3_startup_continuation_tilemap_select_override(
                    tilemap_override.tilemap_select,
                    tilemap_override.applies_to_visible_tile2,
                    tilemap_override.applies_to_visible_tile3,
                );
        }
    }

    pub(in crate::ppu) fn apply_dmg_lcdc4_live_bg_tiledata_write(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) {
        if !self.console_model.is_dmg_family()
            || !write_context.lcdc_changed(LCDC_BG_WINDOW_TILE_DATA_BIT)
        {
            return;
        }

        if write_context.previous_lcdc() & LCDC_BG_WINDOW_TILE_DATA_BIT != 0
            && write_context.current_lcdc() & LCDC_BG_WINDOW_TILE_DATA_BIT == 0
            && self.current_window_line_counter() >= 24
        {
            self.runtime
                .bg_pipeline_state
                .apply_dmg_lcdc4_output_override_to_window_seam_slices(
                    BgTileDataSelect::Unsigned8000,
                );
            self.runtime.panel.pending_dmg_window_lcdc4_output_repaint =
                Some(BgTileDataSelect::Unsigned8000);
        }

        let Some(policy) = self.dmg_single_selected_sprite_phase_policy() else {
            return;
        };

        let target_select = if write_context.current_lcdc() & LCDC_BG_WINDOW_TILE_DATA_BIT != 0 {
            BgTileDataSelect::Unsigned8000
        } else {
            BgTileDataSelect::Signed8800
        };

        let Some(override_decision) = policy
            .observed_lcdc4_phase_table()
            .startup_override_for_target_select(target_select)
        else {
            return;
        };

        self.runtime
            .bg_pipeline_state
            .latch_and_apply_dmg_lcdc4_startup_tiledata_select_override(
                override_decision.slice,
                override_decision.override_select,
            );
    }

    pub(in crate::ppu) fn apply_dmg_lcdc0_live_bg_enable_write(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) {
        if !self.console_model.is_dmg_family() || !write_context.lcdc_changed(LCDC_BG_ENABLE_BIT) {
            return;
        }

        let write_index = self
            .runtime
            .panel
            .dmg_panel_live_write_state
            .lcdc0
            .take_next_bg_enable_write_index();

        let Some(policy) = self.dmg_single_selected_sprite_phase_policy() else {
            return;
        };
        let Some(onset_visible_x) = policy
            .observed_lcdc0_onset_table()
            .onset_visible_x(write_index)
        else {
            return;
        };

        let previous_bg_enabled = write_context.previous_lcdc() & LCDC_BG_ENABLE_BIT != 0;
        let visible_pixels_output = self.runtime.bg_pipeline_state.visible_pixels_output;
        if onset_visible_x >= visible_pixels_output {
            self.start_dmg_lcdc0_bg_enable_visible_hold(
                previous_bg_enabled,
                onset_visible_x.saturating_sub(visible_pixels_output),
            );
            return;
        }

        let bg_enabled = write_context.current_lcdc() & LCDC_BG_ENABLE_BIT != 0;
        self.repaint_dmg_lcdc0_panel_range(onset_visible_x, visible_pixels_output, bg_enabled);
        self.start_dmg_lcdc0_bg_enable_visible_hold(previous_bg_enabled, 0);
    }

    pub(in crate::ppu) fn apply_dmg_lcdc1_live_obj_enable_write(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) {
        if !self.console_model.is_dmg_family() || !write_context.lcdc_changed(LCDC_OBJ_ENABLE_BIT) {
            return;
        }

        let previous_obj_enabled = write_context.previous_lcdc() & LCDC_OBJ_ENABLE_BIT != 0;
        let obj_enabled = write_context.current_lcdc() & LCDC_OBJ_ENABLE_BIT != 0;
        if previous_obj_enabled && !obj_enabled {
            let visible_pixels_output = self.runtime.bg_pipeline_state.visible_pixels_output;
            let onset_visible_x = self
                .dmg_single_selected_sprite_phase_policy()
                .and_then(PpuMode3SingleSpritePhasePolicy::observed_lcdc1_disable_onset_visible_x);

            if let Some(onset_visible_x) = onset_visible_x {
                let (override_enabled, hold_pixels) = if onset_visible_x <= visible_pixels_output {
                    let repaint_end_x = visible_pixels_output
                        .saturating_add(1)
                        .min(SCREEN_WIDTH as u8);
                    self.repaint_dmg_lcdc1_panel_range(onset_visible_x, repaint_end_x);
                    (false, 1)
                } else {
                    (true, onset_visible_x.saturating_sub(visible_pixels_output))
                };
                self.start_dmg_lcdc1_obj_enable_visible_hold(override_enabled, hold_pixels);
            } else {
                self.start_dmg_lcdc1_obj_enable_visible_hold(previous_obj_enabled, 0);
            }
        } else {
            self.start_dmg_lcdc1_obj_enable_visible_hold(previous_obj_enabled, 0);
        }
    }

    pub(in crate::ppu) fn apply_dmg_lcdc2_live_obj_size_write(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) {
        if !self.console_model.is_dmg_family() || !write_context.lcdc_changed(LCDC_OBJ_SIZE_BIT) {
            return;
        }

        let write_index = self
            .runtime
            .panel
            .dmg_panel_live_write_state
            .lcdc2
            .take_next_obj_size_write_index();
        let previous_obj_height = if write_context.previous_lcdc() & LCDC_OBJ_SIZE_BIT != 0 {
            16
        } else {
            8
        };
        let current_obj_height = if write_context.current_lcdc() & LCDC_OBJ_SIZE_BIT != 0 {
            16
        } else {
            8
        };

        if previous_obj_height == 16 && current_obj_height == 8 {
            let visible_pixels_output = self.runtime.bg_pipeline_state.visible_pixels_output;
            self.runtime
                .panel
                .dmg_panel_live_write_state
                .lcdc2
                .begin_active_shrink(write_index, visible_pixels_output);
        }
    }

    fn dmg_single_selected_sprite_phase_policy(&self) -> Option<PpuMode3SingleSpritePhasePolicy> {
        if self.runtime.mode2_scan_state.selected_sprite_count() != 1 {
            return None;
        }

        self.runtime
            .mode2_scan_state
            .selected_sprite(0)
            .map(|sprite| PpuMode3SingleSpritePhasePolicy::new(sprite.x))
    }

    pub(in crate::ppu) fn consume_dmg_lcdc0_bg_enable_visible_hold(&mut self) {
        self.runtime
            .panel
            .dmg_panel_live_write_state
            .lcdc0
            .bg_enable_visible_hold
            .consume();
    }

    fn start_dmg_lcdc0_bg_enable_visible_hold(
        &mut self,
        previous_bg_enabled: bool,
        hold_pixels: u8,
    ) {
        if !self.console_model.is_dmg_family() || hold_pixels == 0 {
            self.runtime
                .panel
                .dmg_panel_live_write_state
                .lcdc0
                .clear_bg_enable_visible_hold();
            return;
        }

        self.runtime
            .panel
            .dmg_panel_live_write_state
            .lcdc0
            .bg_enable_visible_hold
            .set(previous_bg_enabled, hold_pixels);
    }

    pub(in crate::ppu) fn consume_dmg_lcdc1_obj_enable_visible_hold(&mut self) {
        self.runtime
            .panel
            .dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold
            .consume();
    }

    fn start_dmg_lcdc1_obj_enable_visible_hold(
        &mut self,
        previous_obj_enabled: bool,
        hold_pixels: u8,
    ) {
        if !self.console_model.is_dmg_family() || hold_pixels == 0 {
            self.runtime
                .panel
                .dmg_panel_live_write_state
                .lcdc1
                .clear_obj_enable_visible_hold();
            return;
        }

        self.runtime
            .panel
            .dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold
            .set(previous_obj_enabled, hold_pixels);
    }

    fn repaint_dmg_lcdc0_panel_range(&mut self, start_x: u8, end_x: u8, bg_enabled: bool) {
        self.repaint_dmg_panel_range(
            start_x,
            end_x,
            DmgPanelRangeRepaint::Lcdc0BgEnable { bg_enabled },
        );
    }

    fn repaint_dmg_lcdc1_panel_range(&mut self, start_x: u8, end_x: u8) {
        self.repaint_dmg_panel_range(start_x, end_x, DmgPanelRangeRepaint::Lcdc1ObjDisable);
    }

    fn repaint_dmg_panel_range(&mut self, start_x: u8, end_x: u8, repaint: DmgPanelRangeRepaint) {
        let context = self.dmg_panel_repaint_context();
        let bg_enabled = match repaint {
            DmgPanelRangeRepaint::Lcdc0BgEnable { bg_enabled } => bg_enabled,
            DmgPanelRangeRepaint::Lcdc1ObjDisable => self.pixel_transfer_bg_enabled(),
        };
        let dmg_bg_forced_white = context.visible_output_driving && !bg_enabled;
        let visible_range = usize::from(start_x)..usize::from(end_x);
        let current_scanline_bg_pixels = self.runtime.panel.current_scanline_bg_pixels;

        for x in visible_range.clone() {
            match repaint {
                DmgPanelRangeRepaint::Lcdc0BgEnable { .. } => {
                    let pixel = self.runtime.panel.current_scanline_mixed_pixels[x];
                    if !matches!(pixel.source, MixedPixelSource::Background) {
                        continue;
                    }
                    self.repaint_dmg_panel_output_pixel(
                        x,
                        pixel.color,
                        dmg_bg_forced_white,
                        context,
                    );
                }
                DmgPanelRangeRepaint::Lcdc1ObjDisable => {
                    let bg_pixel = self.runtime.panel.current_scanline_bg_pixels[x];
                    self.runtime.panel.current_scanline_mixed_pixels[x] =
                        MixedPixel::background(bg_pixel);
                    self.repaint_dmg_panel_output_pixel(x, bg_pixel, dmg_bg_forced_white, context);
                }
            }
        }

        for dot in &mut self
            .runtime
            .panel
            .dmg_panel_live_write_state
            .recent_panel_dots
        {
            let visible_x = dot.visible_x as usize;
            if !visible_range.contains(&visible_x) {
                continue;
            }

            match repaint {
                DmgPanelRangeRepaint::Lcdc0BgEnable { .. } => {
                    dot.dmg_bg_forced_white = dmg_bg_forced_white
                        && matches!(dot.pixel.source, MixedPixelSource::Background);
                }
                DmgPanelRangeRepaint::Lcdc1ObjDisable => {
                    dot.pixel = MixedPixel::background(current_scanline_bg_pixels[visible_x]);
                    dot.dmg_bg_forced_white = dmg_bg_forced_white;
                }
            }
        }
    }

    fn dmg_panel_repaint_context(&self) -> DmgPanelRepaintContext {
        DmgPanelRepaintContext {
            visible_output_driving: self.runtime.panel.visible_output
                == PpuVisibleOutputState::Driving,
            row_start: self.ly as usize * SCREEN_WIDTH,
            historical_bgp: self.mode3_register_latches().pixel_pipeline_bgp(
                self.console_model,
                None,
                None,
            ),
        }
    }

    fn repaint_dmg_panel_output_pixel(
        &mut self,
        x: usize,
        pixel_color: u8,
        dmg_bg_forced_white: bool,
        context: DmgPanelRepaintContext,
    ) {
        let panel_pixel = if context.visible_output_driving {
            if dmg_bg_forced_white {
                0
            } else {
                self.apply_dmg_palette(context.historical_bgp, pixel_color)
            }
        } else {
            0
        };
        let scanline_pixel = if context.visible_output_driving && !dmg_bg_forced_white {
            pixel_color
        } else {
            0
        };

        let framebuffer_index = context.row_start + x;
        let bgwin_source =
            match self.runtime.panel.current_scanline_bg_dot_contexts[x].map(|dot| dot.source) {
                Some(PpuBgFetcherSource::Background) => PpuFramebufferLayerSource::Background,
                Some(PpuBgFetcherSource::Window) => PpuFramebufferLayerSource::Window,
                None => PpuFramebufferLayerSource::Backdrop,
            };

        self.runtime.panel.current_scanline_dmg_bg_forced_white[x] = dmg_bg_forced_white;
        self.runtime.panel.current_scanline_pixels[x] = scanline_pixel;
        self.write_framebuffer_panel_shade(framebuffer_index, panel_pixel);
        self.runtime.panel.framebuffer_layer_sources[framebuffer_index] = bgwin_source;
        self.runtime.panel.framebuffer_bgwin_colors[framebuffer_index] = pixel_color;
        self.runtime.panel.framebuffer_bgwin_forced_white[framebuffer_index] = dmg_bg_forced_white;
        self.runtime.panel.framebuffer_bgwin_panel_shades[framebuffer_index] = panel_pixel;
        self.runtime.panel.framebuffer_backdrop_panel_shades[framebuffer_index] =
            if context.visible_output_driving {
                self.apply_dmg_palette(context.historical_bgp, 0)
            } else {
                0
            };
        self.runtime.panel.framebuffer_bgwin_layer_sources[framebuffer_index] = bgwin_source;
    }

    pub(in crate::ppu) fn pixel_transfer_obj_enabled(&self) -> bool {
        let obj_enabled = self.mode3_register_latches().pixel_transfer_obj_enabled(
            self.console_model,
            self.runtime.bg_pipeline_state.current_transfer_x,
        );

        if self.console_model.is_dmg_family() {
            self.runtime
                .panel
                .dmg_panel_live_write_state
                .lcdc1
                .obj_enable_visible_hold
                .override_value
                .unwrap_or(obj_enabled)
        } else {
            obj_enabled
        }
    }
}
