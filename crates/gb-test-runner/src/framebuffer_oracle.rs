use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

const FRAMEBUFFER_WIDTH: usize = 160;
const FRAMEBUFFER_HEIGHT: usize = 144;
const DMG_GRAYSCALE_SHADES: [u8; 4] = [255, 170, 85, 0];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedFramebuffer {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) palette_ranks: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrayscaleFramebuffer {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FramebufferOracleError {
    pub(crate) path: PathBuf,
    pub(crate) message: String,
}

impl FramebufferOracleError {
    pub(crate) fn into_invalid_data_error(self) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, self.message)
    }
}

pub(crate) fn decode_fixture_framebuffer_path(
    path: &Path,
) -> Result<NormalizedFramebuffer, FramebufferOracleError> {
    let bytes = std::fs::read(path).map_err(|source| FramebufferOracleError {
        path: path.to_path_buf(),
        message: source.to_string(),
    })?;
    decode_fixture_framebuffer_bytes(path, &bytes)
}

pub(crate) fn decode_fixture_framebuffer_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<NormalizedFramebuffer, FramebufferOracleError> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("pgm") => decode_pgm_framebuffer(path, bytes),
        Some("png") => decode_png_framebuffer(path, bytes),
        _ => Err(FramebufferOracleError {
            path: path.to_path_buf(),
            message: "unsupported framebuffer fixture extension".to_string(),
        }),
    }
}

pub(crate) fn decode_fixture_grayscale_framebuffer_path(
    path: &Path,
) -> Result<GrayscaleFramebuffer, FramebufferOracleError> {
    let bytes = std::fs::read(path).map_err(|source| FramebufferOracleError {
        path: path.to_path_buf(),
        message: source.to_string(),
    })?;
    decode_fixture_grayscale_framebuffer_bytes(path, &bytes)
}

pub(crate) fn decode_fixture_grayscale_framebuffer_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<GrayscaleFramebuffer, FramebufferOracleError> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("pgm") => decode_pgm_grayscale_framebuffer(path, bytes),
        Some("png") => decode_png_grayscale_framebuffer(path, bytes),
        _ => Err(FramebufferOracleError {
            path: path.to_path_buf(),
            message: "unsupported framebuffer fixture extension".to_string(),
        }),
    }
}

pub(crate) fn decode_local_pgm_framebuffer(
    case_id: &str,
    bytes: &[u8],
) -> Result<NormalizedFramebuffer, FramebufferOracleError> {
    let path = PathBuf::from(format!("<local framebuffer for {case_id}>"));
    decode_pgm_framebuffer(&path, bytes)
}

pub(crate) fn decode_local_pgm_grayscale_framebuffer(
    case_id: &str,
    bytes: &[u8],
) -> Result<GrayscaleFramebuffer, FramebufferOracleError> {
    let path = PathBuf::from(format!("<local framebuffer for {case_id}>"));
    decode_pgm_grayscale_framebuffer(&path, bytes)
}

pub(crate) fn encode_framebuffer_pgm(framebuffer: &[u8]) -> Vec<u8> {
    let mut encoded =
        format!("P5\n{} {}\n255\n", FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT).into_bytes();
    encoded.reserve(framebuffer.len());

    for &pixel in framebuffer {
        encoded.push(match pixel {
            0..=3 => DMG_GRAYSCALE_SHADES[usize::from(pixel)],
            _ => DMG_GRAYSCALE_SHADES[3],
        });
    }

    encoded
}

pub(crate) fn decode_local_rgb555_framebuffer(
    case_id: &str,
    pixels: &[u16],
) -> Result<NormalizedFramebuffer, FramebufferOracleError> {
    let path = PathBuf::from(format!("<local CGB RGB555 framebuffer for {case_id}>"));
    let expected_len = FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT;
    if pixels.len() != expected_len {
        return Err(FramebufferOracleError {
            path,
            message: format!(
                "CGB RGB555 framebuffer length {} does not match expected {expected_len}",
                pixels.len()
            ),
        });
    }

    let colors = pixels
        .iter()
        .copied()
        .map(rgb555_to_rgb888)
        .collect::<Vec<_>>();
    Ok(normalize_rgb_pixels(
        FRAMEBUFFER_WIDTH,
        FRAMEBUFFER_HEIGHT,
        &colors,
    ))
}

pub(crate) fn encode_rgb555_framebuffer_png(pixels: &[u16]) -> io::Result<Vec<u8>> {
    let colors = pixels
        .iter()
        .copied()
        .map(rgb555_to_rgb888)
        .collect::<Vec<_>>();
    encode_rgb_png(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, &colors)
}

#[cfg(test)]
pub(crate) fn encode_framebuffer_png(framebuffer: &[u8]) -> Result<Vec<u8>, io::Error> {
    let pixels = framebuffer
        .iter()
        .map(|pixel| match *pixel {
            0..=3 => DMG_GRAYSCALE_SHADES[usize::from(*pixel)],
            _ => DMG_GRAYSCALE_SHADES[3],
        })
        .collect::<Vec<_>>();
    encode_grayscale_png(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, &pixels)
}

pub(crate) fn convert_pgm_to_png(bytes: &[u8]) -> Result<Vec<u8>, FramebufferOracleError> {
    let path = PathBuf::from("<local framebuffer artifact>");
    let (width, height, pixels) = parse_pgm(&path, bytes)?;
    encode_grayscale_png(width, height, pixels).map_err(|source| FramebufferOracleError {
        path,
        message: source.to_string(),
    })
}

fn decode_pgm_framebuffer(
    path: &Path,
    bytes: &[u8],
) -> Result<NormalizedFramebuffer, FramebufferOracleError> {
    let (width, height, pixels) = parse_pgm(path, bytes)?;
    Ok(normalize_indexed_pixels(width, height, pixels))
}

fn decode_pgm_grayscale_framebuffer(
    path: &Path,
    bytes: &[u8],
) -> Result<GrayscaleFramebuffer, FramebufferOracleError> {
    let (width, height, pixels) = parse_pgm(path, bytes)?;
    Ok(GrayscaleFramebuffer {
        width,
        height,
        pixels: pixels.to_vec(),
    })
}

fn decode_png_framebuffer(
    path: &Path,
    bytes: &[u8],
) -> Result<NormalizedFramebuffer, FramebufferOracleError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|source| FramebufferOracleError {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
    let output_buffer_size = reader
        .output_buffer_size()
        .ok_or_else(|| FramebufferOracleError {
            path: path.to_path_buf(),
            message: "PNG decoder did not expose an output buffer size".to_string(),
        })?;
    let mut buffer = vec![0; output_buffer_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|source| FramebufferOracleError {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
    let pixels = &buffer[..info.buffer_size()];
    let width = info.width as usize;
    let height = info.height as usize;

    match info.color_type {
        png::ColorType::Grayscale => Ok(normalize_indexed_pixels(width, height, pixels)),
        png::ColorType::Rgb => {
            let colors = pixels
                .chunks_exact(3)
                .map(|chunk| [chunk[0], chunk[1], chunk[2]])
                .collect::<Vec<_>>();
            Ok(normalize_rgb_pixels(width, height, &colors))
        }
        png::ColorType::Rgba => {
            let colors = pixels
                .chunks_exact(4)
                .map(|chunk| [chunk[0], chunk[1], chunk[2]])
                .collect::<Vec<_>>();
            Ok(normalize_rgb_pixels(width, height, &colors))
        }
        png::ColorType::GrayscaleAlpha => {
            let shades = pixels
                .chunks_exact(2)
                .map(|chunk| chunk[0])
                .collect::<Vec<_>>();
            Ok(normalize_indexed_pixels(width, height, &shades))
        }
        png::ColorType::Indexed => Err(FramebufferOracleError {
            path: path.to_path_buf(),
            message: "indexed PNG framebuffer fixtures are not supported".to_string(),
        }),
    }
}

fn decode_png_grayscale_framebuffer(
    path: &Path,
    bytes: &[u8],
) -> Result<GrayscaleFramebuffer, FramebufferOracleError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|source| FramebufferOracleError {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
    let output_buffer_size = reader
        .output_buffer_size()
        .ok_or_else(|| FramebufferOracleError {
            path: path.to_path_buf(),
            message: "PNG decoder did not expose an output buffer size".to_string(),
        })?;
    let mut buffer = vec![0; output_buffer_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|source| FramebufferOracleError {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
    let pixels = &buffer[..info.buffer_size()];
    let width = info.width as usize;
    let height = info.height as usize;

    let pixels = match info.color_type {
        png::ColorType::Grayscale => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .map(|chunk| grayscale_luma([chunk[0], chunk[1], chunk[2]]))
            .collect(),
        png::ColorType::Rgba => pixels
            .chunks_exact(4)
            .map(|chunk| grayscale_luma([chunk[0], chunk[1], chunk[2]]))
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels.chunks_exact(2).map(|chunk| chunk[0]).collect(),
        png::ColorType::Indexed => {
            return Err(FramebufferOracleError {
                path: path.to_path_buf(),
                message: "indexed PNG framebuffer fixtures are not supported".to_string(),
            });
        }
    };

    Ok(GrayscaleFramebuffer {
        width,
        height,
        pixels,
    })
}

fn parse_pgm<'a>(
    path: &Path,
    bytes: &'a [u8],
) -> Result<(usize, usize, &'a [u8]), FramebufferOracleError> {
    let mut index = 0_usize;
    let magic = next_pgm_token(bytes, &mut index, "magic", path)?;
    if magic != b"P5" {
        return Err(FramebufferOracleError {
            path: path.to_path_buf(),
            message: format!("unsupported PGM magic {:?}", String::from_utf8_lossy(magic)),
        });
    }

    let width = parse_usize_token(
        next_pgm_token(bytes, &mut index, "width", path)?,
        path,
        "width",
    )?;
    let height = parse_usize_token(
        next_pgm_token(bytes, &mut index, "height", path)?,
        path,
        "height",
    )?;
    let max_value =
        parse_usize_token(next_pgm_token(bytes, &mut index, "max", path)?, path, "max")?;
    if max_value != 255 {
        return Err(FramebufferOracleError {
            path: path.to_path_buf(),
            message: format!("unsupported PGM max value {max_value}"),
        });
    }

    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    let expected_len = width
        .checked_mul(height)
        .ok_or_else(|| FramebufferOracleError {
            path: path.to_path_buf(),
            message: "PGM dimensions overflow".to_string(),
        })?;
    if bytes.len() < index + expected_len {
        return Err(FramebufferOracleError {
            path: path.to_path_buf(),
            message: "PGM pixel payload is shorter than declared dimensions".to_string(),
        });
    }

    Ok((width, height, &bytes[index..index + expected_len]))
}

fn next_pgm_token<'a>(
    bytes: &'a [u8],
    index: &mut usize,
    label: &str,
    path: &Path,
) -> Result<&'a [u8], FramebufferOracleError> {
    while *index < bytes.len() && bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
    let start = *index;
    while *index < bytes.len() && !bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
    if start == *index {
        return Err(FramebufferOracleError {
            path: path.to_path_buf(),
            message: format!("missing PGM {label} token"),
        });
    }
    Ok(&bytes[start..*index])
}

fn parse_usize_token(
    token: &[u8],
    path: &Path,
    label: &str,
) -> Result<usize, FramebufferOracleError> {
    std::str::from_utf8(token)
        .map_err(|source| FramebufferOracleError {
            path: path.to_path_buf(),
            message: format!("invalid UTF-8 in PGM {label} token: {source}"),
        })?
        .parse::<usize>()
        .map_err(|source| FramebufferOracleError {
            path: path.to_path_buf(),
            message: format!("invalid PGM {label} token: {source}"),
        })
}

fn normalize_indexed_pixels(width: usize, height: usize, pixels: &[u8]) -> NormalizedFramebuffer {
    let mut shades = pixels.to_vec();
    shades.sort_unstable();
    shades.dedup();
    shades.sort_by(|left, right| right.cmp(left));

    let rank_by_shade = shades
        .iter()
        .enumerate()
        .map(|(rank, shade)| (*shade, rank as u8))
        .collect::<BTreeMap<_, _>>();
    let palette_ranks = pixels
        .iter()
        .map(|shade| {
            *rank_by_shade
                .get(shade)
                .expect("rank table should contain every source shade")
        })
        .collect();

    NormalizedFramebuffer {
        width,
        height,
        palette_ranks,
    }
}

fn normalize_rgb_pixels(width: usize, height: usize, pixels: &[[u8; 3]]) -> NormalizedFramebuffer {
    let mut unique_colors = pixels.to_vec();
    unique_colors.sort_unstable();
    unique_colors.dedup();
    unique_colors
        .sort_by(|left, right| luminance(right).cmp(&luminance(left)).then(right.cmp(left)));

    let rank_by_color = unique_colors
        .iter()
        .enumerate()
        .map(|(rank, color)| (*color, rank as u8))
        .collect::<BTreeMap<_, _>>();
    let palette_ranks = pixels
        .iter()
        .map(|color| {
            *rank_by_color
                .get(color)
                .expect("rank table should contain every source color")
        })
        .collect();

    NormalizedFramebuffer {
        width,
        height,
        palette_ranks,
    }
}

fn luminance(color: &[u8; 3]) -> u16 {
    color.iter().map(|component| u16::from(*component)).sum()
}

fn grayscale_luma(color: [u8; 3]) -> u8 {
    ((u32::from(color[0]) * 299 + u32::from(color[1]) * 587 + u32::from(color[2]) * 114 + 500)
        / 1_000) as u8
}

fn rgb555_to_rgb888(color: u16) -> [u8; 3] {
    let red = (color & 0x001F) as u8;
    let green = ((color >> 5) & 0x001F) as u8;
    let blue = ((color >> 10) & 0x001F) as u8;
    [
        scale_5_bit_to_8_bit(red),
        scale_5_bit_to_8_bit(green),
        scale_5_bit_to_8_bit(blue),
    ]
}

fn scale_5_bit_to_8_bit(component: u8) -> u8 {
    (component << 3) | (component >> 2)
}

fn encode_grayscale_png(width: usize, height: usize, pixels: &[u8]) -> Result<Vec<u8>, io::Error> {
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

fn encode_rgb_png(width: usize, height: usize, pixels: &[[u8; 3]]) -> Result<Vec<u8>, io::Error> {
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

fn png_encoding_io_error(source: png::EncodingError) -> io::Error {
    io::Error::other(source.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        DMG_GRAYSCALE_SHADES, decode_fixture_framebuffer_bytes, decode_fixture_framebuffer_path,
        decode_fixture_grayscale_framebuffer_bytes, decode_local_pgm_framebuffer,
        decode_local_pgm_grayscale_framebuffer, encode_framebuffer_pgm, encode_framebuffer_png,
    };
    use std::fs;
    use std::path::Path;

    fn encode_png(
        width: u32,
        height: u32,
        color_type: png::ColorType,
        pixels: &[u8],
        palette: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut encoded = Vec::new();
        let mut encoder = png::Encoder::new(&mut encoded, width, height);
        encoder.set_color(color_type);
        encoder.set_depth(png::BitDepth::Eight);
        if let Some(palette) = palette {
            encoder.set_palette(palette);
        }
        let mut writer = encoder.write_header().expect("PNG header should encode");
        writer
            .write_image_data(pixels)
            .expect("PNG pixels should encode");
        drop(writer);
        encoded
    }

    #[test]
    fn png_encoder_round_trips_local_dmg_shades() {
        let mut framebuffer = vec![0_u8; super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT];
        framebuffer[0..4].copy_from_slice(&[0, 1, 2, 3]);
        let png = encode_framebuffer_png(&framebuffer).expect("PNG should encode");
        let decoded = decode_fixture_framebuffer_bytes(Path::new("fixture.png"), &png)
            .expect("PNG should decode");
        assert_eq!(&decoded.palette_ranks[0..4], &[0, 1, 2, 3]);
    }

    #[test]
    fn png_and_pgm_normalize_to_the_same_palette_ranks() {
        let mut framebuffer = vec![0_u8; super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT];
        framebuffer[0..6].copy_from_slice(&[0, 0, 1, 2, 3, 3]);
        let pgm = encode_framebuffer_pgm(&framebuffer);
        let png = encode_framebuffer_png(&framebuffer).expect("PNG should encode");
        let pgm_decoded = decode_local_pgm_framebuffer("fixture", &pgm).expect("PGM should decode");
        let png_decoded = decode_fixture_framebuffer_bytes(Path::new("fixture.png"), &png)
            .expect("PNG should decode");
        assert_eq!(pgm_decoded, png_decoded);
    }

    #[test]
    fn grayscale_framebuffer_decoding_preserves_absolute_shades() {
        let white = vec![0_u8; super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT];
        let black = vec![3_u8; super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT];
        let white_pgm = encode_framebuffer_pgm(&white);
        let black_pgm = encode_framebuffer_pgm(&black);
        let white_png = encode_framebuffer_png(&white).expect("white PNG should encode");
        let black_png = encode_framebuffer_png(&black).expect("black PNG should encode");

        let local_white = decode_local_pgm_grayscale_framebuffer("white", &white_pgm)
            .expect("white local PGM should decode");
        let local_black = decode_local_pgm_grayscale_framebuffer("black", &black_pgm)
            .expect("black local PGM should decode");
        let fixture_white =
            decode_fixture_grayscale_framebuffer_bytes(Path::new("white.png"), &white_png)
                .expect("white fixture PNG should decode");
        let fixture_black =
            decode_fixture_grayscale_framebuffer_bytes(Path::new("black.png"), &black_png)
                .expect("black fixture PNG should decode");

        assert!(local_white.pixels.iter().all(|pixel| *pixel == 0xFF));
        assert!(local_black.pixels.iter().all(|pixel| *pixel == 0x00));
        assert_eq!(local_white, fixture_white);
        assert_eq!(local_black, fixture_black);
        assert_ne!(local_white, local_black);
    }

    #[test]
    fn decode_fixture_framebuffer_path_and_extension_errors_are_explicit() {
        let missing =
            decode_fixture_framebuffer_path(Path::new("/definitely/missing/framebuffer.png"))
                .expect_err("missing framebuffer should fail");
        assert!(missing.path.ends_with("framebuffer.png"));

        let unsupported = decode_fixture_framebuffer_bytes(Path::new("fixture.bmp"), b"")
            .expect_err("unsupported extension should fail");
        assert!(
            unsupported
                .message
                .contains("unsupported framebuffer fixture extension")
        );
    }

    #[test]
    fn encode_framebuffer_pgm_clamps_unknown_pixels_and_convert_pgm_to_png_round_trips() {
        let mut framebuffer = vec![0_u8; super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT];
        framebuffer[0..5].copy_from_slice(&[0, 1, 2, 3, 9]);
        let pgm = encode_framebuffer_pgm(&framebuffer);

        let header_len = format!(
            "P5\n{} {}\n255\n",
            super::FRAMEBUFFER_WIDTH,
            super::FRAMEBUFFER_HEIGHT
        )
        .len();
        assert_eq!(
            &pgm[header_len..header_len + 5],
            &[
                DMG_GRAYSCALE_SHADES[0],
                DMG_GRAYSCALE_SHADES[1],
                DMG_GRAYSCALE_SHADES[2],
                DMG_GRAYSCALE_SHADES[3],
                DMG_GRAYSCALE_SHADES[3],
            ]
        );

        let png = super::convert_pgm_to_png(&pgm).expect("PGM should convert to PNG");
        let decoded = decode_fixture_framebuffer_bytes(Path::new("fixture.png"), &png)
            .expect("converted PNG should decode");
        assert_eq!(&decoded.palette_ranks[0..5], &[0, 1, 2, 3, 3]);
    }

    #[test]
    fn png_decoder_supports_rgb_rgba_and_grayscale_alpha_inputs() {
        let rgb = encode_png(
            2,
            1,
            png::ColorType::Rgb,
            &[0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00],
            None,
        );
        let rgba = encode_png(
            2,
            1,
            png::ColorType::Rgba,
            &[0xAA, 0xAA, 0xAA, 0x10, 0x11, 0x11, 0x11, 0xFF],
            None,
        );
        let gray_alpha = encode_png(
            2,
            1,
            png::ColorType::GrayscaleAlpha,
            &[0xFF, 0x00, 0x00, 0xFF],
            None,
        );

        let rgb = decode_fixture_framebuffer_bytes(Path::new("fixture.png"), &rgb)
            .expect("RGB PNG should decode");
        let rgba = decode_fixture_framebuffer_bytes(Path::new("fixture.png"), &rgba)
            .expect("RGBA PNG should decode");
        let gray_alpha = decode_fixture_framebuffer_bytes(Path::new("fixture.png"), &gray_alpha)
            .expect("grayscale-alpha PNG should decode");

        assert_eq!(rgb.width, 2);
        assert_eq!(rgb.height, 1);
        assert_eq!(rgb.palette_ranks, vec![0, 1]);
        assert_eq!(rgba.palette_ranks, vec![0, 1]);
        assert_eq!(gray_alpha.palette_ranks, vec![0, 1]);
    }

    #[test]
    fn png_decoder_accepts_indexed_pngs_after_decoder_expansion() {
        let indexed = encode_png(
            1,
            1,
            png::ColorType::Indexed,
            &[0],
            Some(&[0xFF, 0xFF, 0xFF]),
        );
        let decoded = decode_fixture_framebuffer_bytes(Path::new("fixture.png"), &indexed)
            .expect("indexed PNG should decode after expansion");
        assert_eq!(decoded.width, 1);
        assert_eq!(decoded.height, 1);
        assert_eq!(decoded.palette_ranks, vec![0]);
    }

    #[test]
    fn pgm_parser_reports_header_and_payload_errors() {
        let missing_max = decode_local_pgm_framebuffer("case", b"P5\n1 1\n")
            .expect_err("missing max token should fail");
        assert!(missing_max.message.contains("missing PGM max token"));

        let invalid_utf8 = decode_local_pgm_framebuffer("case", b"P5\n\xFF 1\n255\n\x00")
            .expect_err("invalid UTF-8 width should fail");
        assert!(
            invalid_utf8
                .message
                .contains("invalid UTF-8 in PGM width token")
        );

        let invalid_max = decode_local_pgm_framebuffer("case", b"P5\n1 1\n1\n\x00")
            .expect_err("unsupported max value should fail");
        assert!(invalid_max.message.contains("unsupported PGM max value"));

        let short_payload = decode_local_pgm_framebuffer("case", b"P5\n2 1\n255\n\x00")
            .expect_err("short payload should fail");
        assert!(
            short_payload
                .message
                .contains("shorter than declared dimensions")
        );
    }

    #[test]
    fn convert_pgm_to_png_surfaces_parse_errors_as_invalid_data() {
        let error = super::convert_pgm_to_png(b"P5\n2 1\n255\n\x00")
            .expect_err("invalid PGM should fail conversion");
        let io_error = error.clone().into_invalid_data_error();
        assert_eq!(io_error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.message.contains("shorter than declared dimensions"));
    }

    #[test]
    fn decode_fixture_framebuffer_path_reads_written_pngs() {
        let temp_dir = std::env::temp_dir().join(format!(
            "gb-cycle-framebuffer-oracle-{}-{}",
            std::process::id(),
            super::FRAMEBUFFER_WIDTH
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let png_path = temp_dir.join("fixture.png");

        let mut framebuffer = vec![0_u8; super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT];
        framebuffer[0..2].copy_from_slice(&[0, 3]);
        let png = encode_framebuffer_png(&framebuffer).expect("PNG should encode");
        fs::write(&png_path, png).expect("fixture PNG should be writable");

        let decoded =
            decode_fixture_framebuffer_path(&png_path).expect("fixture path should decode");
        assert_eq!(&decoded.palette_ranks[0..2], &[0, 1]);

        fs::remove_dir_all(&temp_dir).expect("temp dir should be removable");
    }
}
