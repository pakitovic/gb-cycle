use crate::run::machine::CliMachine;
use gb_core::{SGB_FRAME_HEIGHT, SGB_FRAME_WIDTH};
use std::io;
use std::path::Path;

pub(crate) const FRAMEBUFFER_WIDTH: usize = 160;

pub(crate) const FRAMEBUFFER_HEIGHT: usize = 144;

pub(crate) const SGB_HOST_FRAMEBUFFER_WIDTH: usize = SGB_FRAME_WIDTH;

pub(crate) const SGB_HOST_FRAMEBUFFER_HEIGHT: usize = SGB_FRAME_HEIGHT;

pub(crate) const DMG_GRAYSCALE_SHADES: [u8; 4] = [255, 170, 85, 0];

pub(crate) const DMG_GREY_DISPLAY_PALETTE: DisplayPalette = DisplayPalette {
    shades: [
        [DMG_GRAYSCALE_SHADES[0]; 3],
        [DMG_GRAYSCALE_SHADES[1]; 3],
        [DMG_GRAYSCALE_SHADES[2]; 3],
        [DMG_GRAYSCALE_SHADES[3]; 3],
    ],
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FramebufferOutputFormat {
    Pgm,
    Png,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunDisplayPalette {
    Grey,
}

impl RunDisplayPalette {
    pub(crate) const fn display_palette(self) -> DisplayPalette {
        match self {
            Self::Grey => DMG_GREY_DISPLAY_PALETTE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DisplayPalette {
    shades: [[u8; 3]; 4],
}

impl DisplayPalette {
    pub(crate) const fn shade_rgb(self, shade: u8) -> [u8; 3] {
        match shade {
            0..=3 => self.shades[shade as usize],
            _ => self.shades[3],
        }
    }

    pub(crate) fn shade_luma(self, shade: u8) -> u8 {
        self.shade_rgb(shade)[0]
    }
}

pub(crate) fn framebuffer_output_format(path: &Path) -> FramebufferOutputFormat {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        FramebufferOutputFormat::Png
    } else {
        FramebufferOutputFormat::Pgm
    }
}

pub(crate) fn sgb_framebuffer_artifact_for_output(
    machine: &CliMachine,
    show_sgb_border: bool,
) -> Option<(usize, usize, Vec<u16>)> {
    if show_sgb_border {
        machine.sgb_framebuffer_rgb555().map(|pixels| {
            (
                SGB_HOST_FRAMEBUFFER_WIDTH,
                SGB_HOST_FRAMEBUFFER_HEIGHT,
                pixels,
            )
        })
    } else {
        machine
            .sgb_lcd_framebuffer_rgb555()
            .map(|pixels| (FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, pixels))
    }
}

pub(crate) fn encode_framebuffer_artifact(
    path: &Path,
    framebuffer: &[u8],
    sgb_framebuffer_rgb555: Option<(usize, usize, &[u16])>,
    cgb_framebuffer_rgb555: Option<&[u16]>,
    display_palette: Option<DisplayPalette>,
) -> io::Result<Vec<u8>> {
    match framebuffer_output_format(path) {
        FramebufferOutputFormat::Pgm => {
            if let Some(display_palette) = display_palette {
                Ok(encode_framebuffer_palette_pgm(framebuffer, display_palette))
            } else {
                Ok(encode_framebuffer_pgm(framebuffer))
            }
        }
        FramebufferOutputFormat::Png => {
            if let Some((width, height, sgb_framebuffer_rgb555)) = sgb_framebuffer_rgb555 {
                encode_rgb555_framebuffer_png(width, height, sgb_framebuffer_rgb555)
            } else if let Some(cgb_framebuffer_rgb555) = cgb_framebuffer_rgb555 {
                encode_rgb555_framebuffer_png(
                    FRAMEBUFFER_WIDTH,
                    FRAMEBUFFER_HEIGHT,
                    cgb_framebuffer_rgb555,
                )
            } else if let Some(display_palette) = display_palette {
                encode_framebuffer_palette_png(framebuffer, display_palette)
            } else {
                encode_framebuffer_png(framebuffer)
            }
        }
    }
}

pub(crate) fn encode_framebuffer_pgm(framebuffer: &[u8]) -> Vec<u8> {
    let mut encoded = format!("P5\n{FRAMEBUFFER_WIDTH} {FRAMEBUFFER_HEIGHT}\n3\n").into_bytes();
    encoded.extend_from_slice(framebuffer);
    encoded
}

pub(crate) fn encode_framebuffer_palette_pgm(
    framebuffer: &[u8],
    display_palette: DisplayPalette,
) -> Vec<u8> {
    let mut encoded = format!("P5\n{FRAMEBUFFER_WIDTH} {FRAMEBUFFER_HEIGHT}\n255\n").into_bytes();
    encoded.extend(
        framebuffer
            .iter()
            .map(|pixel| display_palette.shade_luma(*pixel)),
    );
    encoded
}

pub(crate) fn encode_framebuffer_png(framebuffer: &[u8]) -> io::Result<Vec<u8>> {
    let pixels = framebuffer
        .iter()
        .map(|pixel| framebuffer_pixel_to_grayscale(*pixel))
        .collect::<Vec<_>>();
    encode_grayscale_png(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, &pixels)
}

pub(crate) fn encode_framebuffer_palette_png(
    framebuffer: &[u8],
    display_palette: DisplayPalette,
) -> io::Result<Vec<u8>> {
    let pixels = framebuffer
        .iter()
        .map(|pixel| display_palette.shade_rgb(*pixel))
        .collect::<Vec<_>>();
    encode_rgb_png(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, &pixels)
}

pub(crate) fn encode_rgb555_framebuffer_png(
    width: usize,
    height: usize,
    framebuffer: &[u16],
) -> io::Result<Vec<u8>> {
    let pixels = framebuffer
        .iter()
        .copied()
        .map(rgb555_to_rgb888)
        .collect::<Vec<_>>();
    encode_rgb_png(width, height, &pixels)
}

pub(crate) fn rgb555_to_rgb888(color: u16) -> [u8; 3] {
    let red = (color & 0x001F) as u8;
    let green = ((color >> 5) & 0x001F) as u8;
    let blue = ((color >> 10) & 0x001F) as u8;
    [
        scale_5_bit_to_8_bit(red),
        scale_5_bit_to_8_bit(green),
        scale_5_bit_to_8_bit(blue),
    ]
}

pub(crate) fn scale_5_bit_to_8_bit(component: u8) -> u8 {
    (component << 3) | (component >> 2)
}

pub(crate) fn encode_grayscale_png(
    width: usize,
    height: usize,
    pixels: &[u8],
) -> io::Result<Vec<u8>> {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width as u32, height as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(png_encoding_io_error)?;
        writer
            .write_image_data(pixels)
            .map_err(png_encoding_io_error)?;
    }
    Ok(encoded)
}

pub(crate) fn encode_rgb_png(
    width: usize,
    height: usize,
    pixels: &[[u8; 3]],
) -> io::Result<Vec<u8>> {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(png_encoding_io_error)?;
        let mut bytes = Vec::with_capacity(pixels.len() * 3);
        for pixel in pixels {
            bytes.extend_from_slice(pixel);
        }
        writer
            .write_image_data(&bytes)
            .map_err(png_encoding_io_error)?;
    }
    Ok(encoded)
}

pub(crate) fn framebuffer_pixel_to_grayscale(pixel: u8) -> u8 {
    match pixel {
        0..=3 => DMG_GRAYSCALE_SHADES[usize::from(pixel)],
        _ => DMG_GRAYSCALE_SHADES[3],
    }
}

pub(crate) fn png_encoding_io_error(source: png::EncodingError) -> io::Error {
    io::Error::other(source.to_string())
}
