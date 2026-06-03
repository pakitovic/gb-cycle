use super::*;

#[test]
fn save_rendered_screenshot_png_reports_file_creation_failures() {
    let root = temp_screenshot_root("png-create-error");
    let output_path = root.join("missing").join("shot.png");
    let rendered = RenderedScreenshot {
        width: 1,
        height: 1,
        rgb_pixels: vec![255, 255, 255],
    };

    let error = save_rendered_screenshot_png(&rendered, &output_path)
        .expect_err("missing parent directory should fail");
    assert!(error.contains("failed to create screenshot output file"));

    fs::remove_dir_all(root).expect("temporary screenshot root should be removable");
}

#[test]
fn save_rendered_screenshot_png_reports_encoding_failures() {
    let root = temp_screenshot_root("png-encode-error");
    let output_path = root.join("shot.png");
    let rendered = RenderedScreenshot {
        width: 2,
        height: 1,
        rgb_pixels: vec![255, 255, 255],
    };

    let error = save_rendered_screenshot_png(&rendered, &output_path)
        .expect_err("short pixel payload should fail PNG encoding");
    assert!(error.contains("failed to write screenshot PNG"));

    fs::remove_dir_all(root).expect("temporary screenshot root should be removable");
}

#[test]
fn render_screenshot_reveals_bgwin_pixels_when_objects_are_hidden() {
    let primary = vec![3_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
    let primary_sources = vec![PpuFramebufferLayerSource::Object; primary.len()];
    let primary_bgwin = vec![1_u8; primary.len()];
    let primary_bgwin_sources = vec![PpuFramebufferLayerSource::Window; primary.len()];
    let video_options = crate::VideoOptions {
        show_objects: false,
        ..crate::VideoOptions::default()
    };

    let rendered = render_screenshot(
        single_panel_input(crate::FramebufferPanelInput {
            dimensions: crate::FramebufferDimensions {
                width: crate::FRAMEBUFFER_WIDTH,
                height: crate::FRAMEBUFFER_HEIGHT,
            },
            framebuffer: &primary,
            framebuffer_layer_sources: &primary_sources,
            bgwin_framebuffer: &primary_bgwin,
            backdrop_framebuffer: &primary_bgwin,
            bgwin_framebuffer_layer_sources: &primary_bgwin_sources,
            display_palette: crate::DMG_DISPLAY_PALETTE,
            cgb_framebuffer_rgb555: None,
            sgb_framebuffer_rgb555: None,
        }),
        &video_options,
    );

    assert_eq!(
        &rendered.rgb_pixels[..3],
        &crate::DMG_DISPLAY_PALETTE.shade_rgb(1)
    );
}
