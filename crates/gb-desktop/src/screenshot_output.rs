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
mod test;
