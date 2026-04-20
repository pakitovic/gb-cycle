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
    primary: crate::FramebufferPanelInput<'_>,
    secondary: Option<crate::FramebufferPanelInput<'_>>,
    video_options: &crate::VideoOptions,
) -> RenderedScreenshot {
    let width = if secondary.is_some() {
        crate::FRAMEBUFFER_WIDTH * 2
    } else {
        crate::FRAMEBUFFER_WIDTH
    };
    let dimensions = crate::FramebufferDimensions {
        width,
        height: crate::FRAMEBUFFER_HEIGHT,
    };
    let mut rgb_pixels = vec![
        0_u8;
        crate::FRAMEBUFFER_HEIGHT as usize
            * crate::framebuffer_pitch_bytes_for_dimensions(dimensions)
    ];

    crate::write_monochrome_framebuffer_region(
        &mut rgb_pixels,
        dimensions,
        0,
        primary,
        video_options,
    );
    if let Some(secondary_panel) = secondary {
        crate::write_monochrome_framebuffer_region(
            &mut rgb_pixels,
            dimensions,
            crate::FRAMEBUFFER_WIDTH as usize,
            secondary_panel,
            video_options,
        );
    }

    RenderedScreenshot {
        width,
        height: crate::FRAMEBUFFER_HEIGHT,
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

    #[test]
    fn render_screenshot_uses_same_dmg_grayscale_ramp_as_desktop_presentation() {
        let mut primary =
            vec![0_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
        let primary_sources = vec![PpuFramebufferLayerSource::Background; primary.len()];
        primary[..4].copy_from_slice(&[0, 1, 2, 3]);

        let rendered = render_screenshot(
            crate::FramebufferPanelInput {
                framebuffer: &primary,
                framebuffer_layer_sources: &primary_sources,
                bgwin_framebuffer: &primary,
                backdrop_framebuffer: &primary,
                bgwin_framebuffer_layer_sources: &primary_sources,
            },
            None,
            &crate::VideoOptions::default(),
        );

        assert_eq!(rendered.width, crate::FRAMEBUFFER_WIDTH);
        assert_eq!(rendered.height, crate::FRAMEBUFFER_HEIGHT);
        assert_eq!(
            &rendered.rgb_pixels[..12],
            &[255, 255, 255, 170, 170, 170, 85, 85, 85, 0, 0, 0]
        );
    }

    #[test]
    fn render_screenshot_places_the_linked_secondary_panel_to_the_right() {
        let primary = vec![0_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
        let secondary = vec![3_u8; (crate::FRAMEBUFFER_WIDTH * crate::FRAMEBUFFER_HEIGHT) as usize];
        let primary_sources = vec![PpuFramebufferLayerSource::Background; primary.len()];
        let secondary_sources = vec![PpuFramebufferLayerSource::Background; secondary.len()];

        let rendered = render_screenshot(
            crate::FramebufferPanelInput {
                framebuffer: &primary,
                framebuffer_layer_sources: &primary_sources,
                bgwin_framebuffer: &primary,
                backdrop_framebuffer: &primary,
                bgwin_framebuffer_layer_sources: &primary_sources,
            },
            Some(crate::FramebufferPanelInput {
                framebuffer: &secondary,
                framebuffer_layer_sources: &secondary_sources,
                bgwin_framebuffer: &secondary,
                backdrop_framebuffer: &secondary,
                bgwin_framebuffer_layer_sources: &secondary_sources,
            }),
            &crate::VideoOptions::default(),
        );
        let pitch = rendered.width as usize * 3;
        let left_pixel = &rendered.rgb_pixels[..3];
        let right_pixel = &rendered.rgb_pixels[crate::FRAMEBUFFER_WIDTH as usize * 3..][..3];

        assert_eq!(rendered.width, crate::FRAMEBUFFER_WIDTH * 2);
        assert_eq!(rendered.height, crate::FRAMEBUFFER_HEIGHT);
        assert_eq!(left_pixel, &[255, 255, 255]);
        assert_eq!(right_pixel, &[0, 0, 0]);
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
        let mut buffer = vec![0; reader.output_buffer_size()];
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
            crate::FramebufferPanelInput {
                framebuffer: &primary,
                framebuffer_layer_sources: &primary_sources,
                bgwin_framebuffer: &primary_bgwin,
                backdrop_framebuffer: &primary_bgwin,
                bgwin_framebuffer_layer_sources: &primary_bgwin_sources,
            },
            None,
            &video_options,
        );

        assert_eq!(&rendered.rgb_pixels[..3], &[170, 170, 170]);
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
            crate::FramebufferPanelInput {
                framebuffer: &primary,
                framebuffer_layer_sources: &primary_sources,
                bgwin_framebuffer: &primary_bgwin,
                backdrop_framebuffer: &primary_backdrop,
                bgwin_framebuffer_layer_sources: &primary_bgwin_sources,
            },
            None,
            &video_options,
        );

        assert_eq!(&rendered.rgb_pixels[..3], &[170, 170, 170]);
        assert_eq!(&rendered.rgb_pixels[3..6], &[0, 0, 0]);
        assert_eq!(&rendered.rgb_pixels[6..9], &[85, 85, 85]);
    }
}
