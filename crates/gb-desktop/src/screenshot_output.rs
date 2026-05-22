use png::{BitDepth, ColorType, SrgbRenderingIntent};
use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

const SCREENSHOT_OUTPUT_SUBDIRECTORY: &str = "screenshots";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedScreenshot {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgb_pixels: Vec<u8>,
}

pub(crate) fn render_screenshot(
    framebuffer: crate::FramebufferRenderInput<'_>,
    video_options: &crate::VideoOptions,
) -> RenderedScreenshot {
    let dimensions = framebuffer.dimensions;
    let mut rgb_pixels = vec![
        0_u8;
        dimensions.height as usize
            * crate::framebuffer_pitch_bytes_for_dimensions(dimensions)
    ];

    let cell_dimensions = crate::framebuffer_cell_dimensions_for_panels(&framebuffer.panels);
    let columns = (dimensions.width / cell_dimensions.width).max(1) as usize;
    for (panel_index, panel) in framebuffer.panels.into_iter().enumerate() {
        let Some(panel) = panel else {
            continue;
        };
        let column = panel_index % columns;
        let row = panel_index / columns;
        crate::write_framebuffer_region(
            &mut rgb_pixels,
            dimensions,
            column * cell_dimensions.width as usize,
            row * cell_dimensions.height as usize,
            panel,
            video_options,
        );
    }

    RenderedScreenshot {
        width: dimensions.width,
        height: dimensions.height,
        rgb_pixels,
    }
}

pub(crate) fn resolve_next_screenshot_output_path(
    rom_path: Option<&Path>,
    current_dir: &Path,
) -> Result<PathBuf, String> {
    let output_dir = screenshot_output_directory(rom_path, current_dir);
    fs::create_dir_all(&output_dir).map_err(|error| {
        crate::format_path_error(
            "failed to create screenshot output directory",
            &output_dir,
            &error.to_string(),
        )
    })?;

    let stem = screenshot_output_stem(rom_path);
    for index in 0..=u16::MAX {
        let candidate = output_dir.join(format!("{stem}-{index}.png"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(crate::format_path_error(
        "failed to allocate a free screenshot path in",
        &output_dir,
        "directory is full",
    ))
}

pub(crate) fn save_rendered_screenshot_png(
    rendered: &RenderedScreenshot,
    output_path: &Path,
) -> Result<(), String> {
    let file = File::create(output_path).map_err(|error| {
        crate::format_path_error(
            "failed to create screenshot output file",
            output_path,
            &error.to_string(),
        )
    })?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), rendered.width, rendered.height);
    encoder.set_color(ColorType::Rgb);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_source_srgb(SrgbRenderingIntent::Perceptual);
    let mut writer = encoder.write_header().map_err(|error| {
        crate::format_path_error(
            "failed to encode screenshot PNG header",
            output_path,
            &png_encoding_io_error(error).to_string(),
        )
    })?;
    writer
        .write_image_data(&rendered.rgb_pixels)
        .map_err(|error| {
            crate::format_path_error(
                "failed to write screenshot PNG",
                output_path,
                &png_encoding_io_error(error).to_string(),
            )
        })?;
    Ok(())
}

fn screenshot_output_directory(rom_path: Option<&Path>, current_dir: &Path) -> PathBuf {
    let base_dir = match rom_path.and_then(Path::parent) {
        Some(parent) => parent.to_path_buf(),
        None => current_dir.to_path_buf(),
    };
    base_dir.join(SCREENSHOT_OUTPUT_SUBDIRECTORY)
}

fn screenshot_output_stem(rom_path: Option<&Path>) -> String {
    rom_path
        .and_then(Path::file_stem)
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("gb-cycle")
        .to_string()
}

fn png_encoding_io_error(source: png::EncodingError) -> io::Error {
    match source {
        png::EncodingError::IoError(error) => error,
        other => io::Error::other(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RenderedScreenshot, render_screenshot, resolve_next_screenshot_output_path,
        save_rendered_screenshot_png,
    };
    use gb_core::PpuFramebufferLayerSource;
    use png::ColorType;
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_screenshot_root(name: &str) -> PathBuf {
        let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "gb-cycle-desktop-screenshot-tests-{name}-{counter}"
        ));
        fs::create_dir_all(&root).expect("temporary screenshot root should be creatable");
        root
    }

    fn render_input(
        dimensions: crate::FramebufferDimensions,
        panels: [Option<crate::FramebufferPanelInput<'_>>; crate::PLAYER_SLOT_COUNT],
    ) -> crate::FramebufferRenderInput<'_> {
        crate::FramebufferRenderInput { dimensions, panels }
    }

    fn single_panel_input(
        panel: crate::FramebufferPanelInput<'_>,
    ) -> crate::FramebufferRenderInput<'_> {
        render_input(
            crate::FramebufferDimensions {
                width: crate::FRAMEBUFFER_WIDTH,
                height: crate::FRAMEBUFFER_HEIGHT,
            },
            [Some(panel), None, None, None],
        )
    }

    #[test]
    fn render_screenshot_uses_same_dmg_display_palette_as_desktop_presentation() {
        let mut primary =
            vec![0_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
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

    #[test]
    fn save_rendered_screenshot_png_writes_an_8bit_rgb_png() {
        let root = temp_screenshot_root("png");
        let output_path = root.join("shot.png");
        let rendered = RenderedScreenshot {
            width: 2,
            height: 1,
            rgb_pixels: vec![255, 255, 255, 0, 0, 0],
        };

        save_rendered_screenshot_png(&rendered, &output_path).expect("screenshot PNG should save");

        let encoded = fs::read(&output_path).expect("screenshot PNG should exist");
        let decoder = png::Decoder::new(Cursor::new(encoded));
        let mut reader = decoder.read_info().expect("PNG header should decode");
        let mut buffer = vec![
            0;
            reader
                .output_buffer_size()
                .expect("PNG output buffer size should fit in memory")
        ];
        let info = reader
            .next_frame(&mut buffer)
            .expect("PNG payload should decode");

        assert_eq!(info.width, 2);
        assert_eq!(info.height, 1);
        assert_eq!(info.color_type, ColorType::Rgb);
        assert_eq!(&buffer[..info.buffer_size()], &[255, 255, 255, 0, 0, 0]);

        fs::remove_dir_all(root).expect("temporary screenshot root should be removable");
    }

    #[test]
    fn resolve_next_screenshot_output_path_falls_back_to_current_dir_without_a_rom() {
        let root = temp_screenshot_root("fallback-path");
        let expected_relative = PathBuf::from("screenshots").join("gb-cycle-0.png");

        let first = resolve_next_screenshot_output_path(None, root.as_path())
            .expect("launcher screenshot path should resolve");
        assert_eq!(
            first
                .strip_prefix(&root)
                .expect("path should live under the temp root"),
            expected_relative.as_path()
        );

        fs::write(&first, b"placeholder").expect("first launcher screenshot should be writable");
        let second = resolve_next_screenshot_output_path(None, root.as_path())
            .expect("second launcher screenshot path should resolve");
        assert_eq!(
            second.file_name().and_then(|name| name.to_str()),
            Some("gb-cycle-1.png")
        );

        fs::remove_dir_all(root).expect("temporary screenshot root should be removable");
    }

    #[test]
    fn resolve_next_screenshot_output_path_reports_directory_creation_failures() {
        let root = temp_screenshot_root("path-error");
        let blocking_path = root.join("blocking");
        fs::write(&blocking_path, b"not-a-directory").expect("blocking file should be writable");

        let error = resolve_next_screenshot_output_path(None, blocking_path.as_path())
            .expect_err("non-directory screenshot root should fail");
        assert!(error.contains("failed to create screenshot output directory"));

        fs::remove_dir_all(root).expect("temporary screenshot root should be removable");
    }

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

    #[test]
    fn render_screenshot_uses_dynamic_backdrop_when_bgwin_layers_are_hidden() {
        let mut primary =
            vec![0_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
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
}
