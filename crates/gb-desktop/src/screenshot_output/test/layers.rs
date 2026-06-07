use super::*;

#[test]
fn render_screenshot_uses_dynamic_backdrop_when_bgwin_layers_are_hidden() {
    let mut primary = vec![0_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
    let mut primary_sources = vec![PpuFramebufferLayerSource::Background; primary.len()];
    let primary_bgwin = vec![1_u8; primary.len()];
    let primary_bgwin_sources = vec![PpuFramebufferLayerSource::Window; primary.len()];
    let mut primary_backdrop = vec![2_u8; primary.len()];
    let video_options = crate::VideoOptions {
        show_background: false,
        show_window: false,
        ..crate::VideoOptions::default()
    };
    primary_backdrop[0] = 1;
    primary[1] = 3;
    primary_sources[1] = PpuFramebufferLayerSource::Object;

    let rendered = render_screenshot(
        single_panel_input(crate::FramebufferPanelInput {
            dimensions: crate::FramebufferDimensions {
                width: crate::FRAMEBUFFER_WIDTH,
                height: crate::FRAMEBUFFER_HEIGHT,
            },
            framebuffer: &primary,
            framebuffer_layer_sources: &primary_sources,
            bgwin_framebuffer: &primary_bgwin,
            backdrop_framebuffer: &primary_backdrop,
            bgwin_framebuffer_layer_sources: &primary_bgwin_sources,
            display_palette: crate::DMG_DISPLAY_PALETTE,
            cgb_framebuffer_rgb555: None,
            sgb_framebuffer_rgb555: None,
            borrowed_sgb_border: None,
        }),
        &video_options,
    );

    assert_eq!(
        &rendered.rgb_pixels[..3],
        &crate::DMG_DISPLAY_PALETTE.shade_rgb(1)
    );
    assert_eq!(
        &rendered.rgb_pixels[3..6],
        &crate::DMG_DISPLAY_PALETTE.shade_rgb(3)
    );
    assert_eq!(
        &rendered.rgb_pixels[6..9],
        &crate::DMG_DISPLAY_PALETTE.shade_rgb(2)
    );
}
