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

pub(crate) fn decode_local_pgm_framebuffer(
    case_id: &str,
    bytes: &[u8],
) -> Result<NormalizedFramebuffer, FramebufferOracleError> {
    let path = PathBuf::from(format!("<local framebuffer for {case_id}>"));
    decode_pgm_framebuffer(&path, bytes)
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

fn png_encoding_io_error(source: png::EncodingError) -> io::Error {
    io::Error::other(source.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        decode_fixture_framebuffer_bytes, decode_local_pgm_framebuffer, encode_framebuffer_pgm,
        encode_framebuffer_png,
    };
    use std::path::Path;

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
}
