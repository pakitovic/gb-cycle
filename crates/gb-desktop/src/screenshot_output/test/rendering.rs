use super::*;

#[test]
fn render_screenshot_uses_same_dmg_display_palette_as_desktop_presentation() {
    let mut primary = vec![0_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
    let primary_sources = vec![PpuFramebufferLayerSource::Background; primary.len()];
    primary[..4].copy_from_slice(&[0, 1, 2, 3]);

    let rendered = render_screenshot(
        single_panel_input(crate::FramebufferPanelInput {
            dimensions: crate::FramebufferDimensions {
                width: crate::FRAMEBUFFER_WIDTH,
                height: crate::FRAMEBUFFER_HEIGHT,
            },
            framebuffer: &primary,
            framebuffer_layer_sources: &primary_sources,
            bgwin_framebuffer: &primary,
            backdrop_framebuffer: &primary,
            bgwin_framebuffer_layer_sources: &primary_sources,
            display_palette: crate::DMG_DISPLAY_PALETTE,
            cgb_framebuffer_rgb555: None,
            sgb_framebuffer_rgb555: None,
            borrowed_sgb_border: None,
        }),
        &crate::VideoOptions::default(),
    );

    assert_eq!(rendered.width, crate::FRAMEBUFFER_WIDTH);
    assert_eq!(rendered.height, crate::FRAMEBUFFER_HEIGHT);
    assert_eq!(
        &rendered.rgb_pixels[..12],
        &[
            0xC6, 0xDE, 0x8C, 0x84, 0xA5, 0x63, 0x39, 0x61, 0x39, 0x08, 0x18, 0x10,
        ]
    );
}

#[test]
fn render_screenshot_uses_cgb_rgb555_framebuffer_for_color_models() {
    let primary = vec![0_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
    let primary_sources = vec![PpuFramebufferLayerSource::Background; primary.len()];
    let mut cgb_framebuffer_rgb555 = vec![0x7FFF_u16; primary.len()];
    cgb_framebuffer_rgb555[..4].copy_from_slice(&[0x001F, 0x03E0, 0x7C00, 0x0000]);

    let rendered = render_screenshot(
        single_panel_input(crate::FramebufferPanelInput {
            dimensions: crate::FramebufferDimensions {
                width: crate::FRAMEBUFFER_WIDTH,
                height: crate::FRAMEBUFFER_HEIGHT,
            },
            framebuffer: &primary,
            framebuffer_layer_sources: &primary_sources,
            bgwin_framebuffer: &primary,
            backdrop_framebuffer: &primary,
            bgwin_framebuffer_layer_sources: &primary_sources,
            display_palette: crate::DMG_DISPLAY_PALETTE,
            cgb_framebuffer_rgb555: Some(&cgb_framebuffer_rgb555),
            sgb_framebuffer_rgb555: None,
            borrowed_sgb_border: None,
        }),
        &crate::VideoOptions {
            display_palette: crate::DesktopDisplayPalette::Light,
            ..crate::VideoOptions::default()
        },
    );

    assert_eq!(rendered.width, crate::FRAMEBUFFER_WIDTH);
    assert_eq!(rendered.height, crate::FRAMEBUFFER_HEIGHT);
    assert_eq!(
        &rendered.rgb_pixels[..12],
        &[
            0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00,
        ]
    );
}

#[test]
fn render_screenshot_uses_sgb_rgb555_host_frame_dimensions() {
    let primary = vec![0_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
    let primary_sources = vec![PpuFramebufferLayerSource::Background; primary.len()];
    let mut sgb_framebuffer_rgb555 = vec![
        0x7FFF_u16;
        (crate::SGB_HOST_FRAMEBUFFER_WIDTH * crate::SGB_HOST_FRAMEBUFFER_HEIGHT)
            as usize
    ];
    sgb_framebuffer_rgb555[..4].copy_from_slice(&[0x001F, 0x03E0, 0x7C00, 0x0000]);

    let rendered = render_screenshot(
        render_input(
            crate::FramebufferDimensions {
                width: crate::SGB_HOST_FRAMEBUFFER_WIDTH,
                height: crate::SGB_HOST_FRAMEBUFFER_HEIGHT,
            },
            [
                Some(crate::FramebufferPanelInput {
                    dimensions: crate::FramebufferDimensions {
                        width: crate::SGB_HOST_FRAMEBUFFER_WIDTH,
                        height: crate::SGB_HOST_FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &primary,
                    framebuffer_layer_sources: &primary_sources,
                    bgwin_framebuffer: &primary,
                    backdrop_framebuffer: &primary,
                    bgwin_framebuffer_layer_sources: &primary_sources,
                    display_palette: crate::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: Some(sgb_framebuffer_rgb555),
                    borrowed_sgb_border: None,
                }),
                None,
                None,
                None,
            ],
        ),
        &crate::VideoOptions::default(),
    );

    assert_eq!(rendered.width, crate::SGB_HOST_FRAMEBUFFER_WIDTH);
    assert_eq!(rendered.height, crate::SGB_HOST_FRAMEBUFFER_HEIGHT);
    assert_eq!(
        &rendered.rgb_pixels[..12],
        &[
            0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00,
        ]
    );
}

#[test]
fn render_screenshot_places_the_linked_secondary_panel_to_the_right() {
    let primary = vec![0_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
    let secondary = vec![3_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
    let primary_sources = vec![PpuFramebufferLayerSource::Background; primary.len()];
    let secondary_sources = vec![PpuFramebufferLayerSource::Background; secondary.len()];

    let rendered = render_screenshot(
        render_input(
            crate::FramebufferDimensions {
                width: crate::FRAMEBUFFER_WIDTH * 2,
                height: crate::FRAMEBUFFER_HEIGHT,
            },
            [
                Some(crate::FramebufferPanelInput {
                    dimensions: crate::FramebufferDimensions {
                        width: crate::FRAMEBUFFER_WIDTH,
                        height: crate::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &primary,
                    framebuffer_layer_sources: &primary_sources,
                    bgwin_framebuffer: &primary,
                    backdrop_framebuffer: &primary,
                    bgwin_framebuffer_layer_sources: &primary_sources,
                    display_palette: crate::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
                Some(crate::FramebufferPanelInput {
                    dimensions: crate::FramebufferDimensions {
                        width: crate::FRAMEBUFFER_WIDTH,
                        height: crate::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &secondary,
                    framebuffer_layer_sources: &secondary_sources,
                    bgwin_framebuffer: &secondary,
                    backdrop_framebuffer: &secondary,
                    bgwin_framebuffer_layer_sources: &secondary_sources,
                    display_palette: crate::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
                None,
                None,
            ],
        ),
        &crate::VideoOptions::default(),
    );
    let pitch = rendered.width as usize * 3;
    let left_pixel = &rendered.rgb_pixels[..3];
    let right_pixel = &rendered.rgb_pixels[crate::FRAMEBUFFER_WIDTH as usize * 3..][..3];

    assert_eq!(rendered.width, crate::FRAMEBUFFER_WIDTH * 2);
    assert_eq!(rendered.height, crate::FRAMEBUFFER_HEIGHT);
    assert_eq!(left_pixel, &crate::DMG_DISPLAY_PALETTE.shade_rgb(0));
    assert_eq!(right_pixel, &crate::DMG_DISPLAY_PALETTE.shade_rgb(3));
    assert_eq!(rendered.rgb_pixels.len(), rendered.height as usize * pitch);
}

#[test]
fn resolve_next_screenshot_output_path_uses_screenshots_subdirectory_and_unique_names() {
    let root = temp_screenshot_root("paths");
    let rom_path = root.join("pokemon.gb");

    let first = resolve_next_screenshot_output_path(Some(&rom_path), root.as_path())
        .expect("first screenshot output path should resolve");
    assert_eq!(
        first.file_name().and_then(|name| name.to_str()),
        Some("pokemon-0.png")
    );

    fs::create_dir_all(
        first
            .parent()
            .expect("screenshot path should have a parent"),
    )
    .expect("screenshot output directory should be creatable");
    fs::write(&first, b"placeholder").expect("first screenshot output should be writable");

    let second = resolve_next_screenshot_output_path(Some(&rom_path), root.as_path())
        .expect("second screenshot output path should resolve");
    assert_eq!(
        second.file_name().and_then(|name| name.to_str()),
        Some("pokemon-1.png")
    );

    fs::remove_dir_all(root).expect("temporary screenshot root should be removable");
}
