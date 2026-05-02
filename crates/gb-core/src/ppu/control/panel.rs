use super::*;

impl PpuPanelState {
    pub(in crate::ppu) fn clear_runtime_scanline_state(&mut self) {
        self.current_scanline_pixels.fill(0);
        self.current_scanline_bg_pixels.fill(0);
        self.current_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
        self.current_scanline_bg_dot_contexts.fill(None);
        self.current_scanline_dmg_bg_forced_white.fill(false);
        self.pending_dmg_window_lcdc4_output_repaint = None;
    }

    pub(in crate::ppu) fn clear_visible_buffers(&mut self) {
        self.fill_visible_buffers_with_panel_shade(0);
    }

    pub(in crate::ppu) fn fill_visible_buffers_with_panel_shade(&mut self, shade: u8) {
        let shade = shade.min(3);
        self.current_scanline_bg_dot_contexts.fill(None);
        self.current_scanline_pixels.fill(shade);
        self.framebuffer.fill(shade);
        self.framebuffer_rgb555.fill(panel_shade_to_rgb555(shade));
        self.framebuffer_layer_sources
            .fill(PpuFramebufferLayerSource::Backdrop);
        self.framebuffer_bgwin_colors.fill(shade);
        self.framebuffer_bgwin_forced_white.fill(false);
        self.framebuffer_bgwin_panel_shades.fill(shade);
        self.framebuffer_backdrop_panel_shades.fill(shade);
        self.framebuffer_bgwin_layer_sources
            .fill(PpuFramebufferLayerSource::Backdrop);
        self.pending_dmg_window_lcdc4_output_repaint = None;
    }

    pub(in crate::ppu) fn reset_for_startup(&mut self, bgp: u8) {
        self.dmg_panel_live_write_state.reset_for_startup(bgp);
        self.current_scanline_pixels.fill(0);
        self.current_scanline_bg_pixels.fill(0);
        self.current_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
        self.current_scanline_bg_dot_contexts.fill(None);
        self.current_scanline_dmg_bg_forced_white.fill(false);
        self.previous_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
        self.previous_scanline_dmg_bg_forced_white.fill(false);
        self.previous_scanline_ly = None;
        self.pending_dmg_window_lcdc4_output_repaint = None;
        self.framebuffer.fill(0);
        self.framebuffer_rgb555.fill(RGB555_WHITE);
    }

    pub(in crate::ppu) fn reset_for_scanline_start(&mut self, bgp: u8) {
        self.dmg_panel_live_write_state
            .reset_for_scanline_start(bgp);
        self.clear_runtime_scanline_state();
    }
}

impl PpuRuntimeState {
    pub(in crate::ppu) fn reset_runtime_pipeline_state(&mut self) {
        self.startup_mode_latch = None;
        self.mode2_scan_state.reset();
        self.window_state.reset();
        self.bg_pipeline_state.reset();
        self.obj_pipeline_state.reset();
        self.panel.clear_runtime_scanline_state();
    }

    pub(in crate::ppu) fn reset_for_startup(&mut self, bgp: u8) {
        self.startup_mode_latch = None;
        self.pending_interrupts = 0;
        self.blank_frame_active = false;
        self.system_stop_active = false;
        self.oam_corruption_controller = OamCorruptionController;
        self.mode2_scan_state.reset();
        self.window_state.reset();
        self.bg_pipeline_state.reset();
        self.obj_pipeline_state.reset();
        self.panel.reset_for_startup(bgp);
    }
}
