use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use super::catalog::{
    FramebufferObservation, OracleConfig, OracleFixtureRoots, OracleObservations, OracleOutcome,
    OracleStep,
};

const FRAMEBUFFER_WIDTH: usize = 160;
const FRAMEBUFFER_HEIGHT: usize = 144;
const DMG_GRAYSCALE_SHADES: [u8; 4] = [255, 170, 85, 0];
const DEFAULT_CHECK_INTERVAL_T_CYCLES: u64 = 100_000;
const DEFAULT_GRAYSCALE_TOLERANCE: u8 = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FramebufferOracle {
    mode: FramebufferMode,
    source: FramebufferSource,
    comparison: FramebufferComparison,
    target_participant: Option<String>,
    check_interval_tcycles: u64,
    check_at_tcycles: Option<u64>,
    pending_periodic_check: bool,
    matched: bool,
    check_at_reached: bool,
}

impl FramebufferOracle {
    pub(super) fn from_manifest(
        config: &OracleConfig,
        fixture_roots: OracleFixtureRoots<'_>,
    ) -> Result<Self, String> {
        config.reject_unknown_parameters(&[
            "mode",
            "source",
            "projection",
            "compare",
            "fixture",
            "local",
            "tolerance",
            "check_interval_tcycles",
            "check_at_tcycles",
            "target_participant",
        ])?;

        let mode = FramebufferMode::parse(config.optional_string("mode")?.as_deref())?;
        let source = FramebufferSource::parse(config.optional_string("source")?.as_deref())?;
        let projection =
            FramebufferProjection::parse(config.optional_string("projection")?.as_deref())?;
        let compare = FramebufferCompare::parse(config.optional_string("compare")?.as_deref())?;
        let tolerance = config.optional_u8("tolerance")?;
        let check_interval_tcycles = config
            .optional_u64("check_interval_tcycles")?
            .unwrap_or(DEFAULT_CHECK_INTERVAL_T_CYCLES);
        let check_at_tcycles = config.optional_u64("check_at_tcycles")?;
        let target_participant = config.optional_string("target_participant")?;
        let local_fixtures = config.optional_bool("local")?.unwrap_or(false);

        if mode != FramebufferMode::UntilMatch {
            if config.has_parameter("check_interval_tcycles") || check_at_tcycles.is_some() {
                return Err(
                    "framebuffer check timing parameters require mode \"until-match\"".to_string(),
                );
            }
        } else if check_interval_tcycles == 0 {
            return Err("framebuffer check_interval_tcycles must be greater than 0".to_string());
        }

        if compare == FramebufferCompare::GrayscaleTolerance
            && projection != FramebufferProjection::Grayscale
        {
            return Err(
                "framebuffer compare \"grayscale-tolerance\" requires projection \"grayscale\""
                    .to_string(),
            );
        }
        if compare == FramebufferCompare::Exact && tolerance.is_some() {
            return Err(
                "framebuffer tolerance requires compare \"grayscale-tolerance\"".to_string(),
            );
        }

        let comparison = if mode == FramebufferMode::Info {
            if config.has_parameter("fixture") {
                return Err("framebuffer mode \"info\" does not use fixture".to_string());
            }
            if local_fixtures {
                return Err("framebuffer mode \"info\" does not use local fixtures".to_string());
            }
            if config.has_parameter("tolerance") {
                return Err("framebuffer mode \"info\" does not use tolerance".to_string());
            }
            FramebufferComparison::Info
        } else {
            let fixtures = config
                .string_or_string_array("fixture")?
                .ok_or_else(|| "framebuffer oracle requires fixture".to_string())?;
            if fixtures.is_empty() {
                return Err("framebuffer fixture array must not be empty".to_string());
            }
            let fixture_paths = fixtures
                .into_iter()
                .map(|path| resolve_fixture_path(fixture_roots, local_fixtures, &path))
                .collect::<Result<Vec<_>, String>>()?;
            match projection {
                FramebufferProjection::PaletteRank => FramebufferComparison::PaletteRank {
                    fixtures: fixture_paths
                        .into_iter()
                        .map(|path| {
                            Ok(FramebufferPaletteFixture {
                                framebuffer: read_palette_fixture(&path)?,
                                path,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                },
                FramebufferProjection::Grayscale => FramebufferComparison::Grayscale {
                    compare,
                    tolerance: tolerance.unwrap_or(DEFAULT_GRAYSCALE_TOLERANCE),
                    fixtures: fixture_paths
                        .into_iter()
                        .map(|path| {
                            Ok(FramebufferGrayscaleFixture {
                                framebuffer: read_grayscale_fixture(&path)?,
                                path,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                },
            }
        };

        Ok(Self {
            mode,
            source,
            comparison,
            target_participant,
            check_interval_tcycles,
            check_at_tcycles,
            pending_periodic_check: false,
            matched: mode == FramebufferMode::Info,
            check_at_reached: false,
        })
    }

    pub(crate) fn observe(
        &mut self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleStep, String> {
        match self.mode {
            FramebufferMode::Final | FramebufferMode::Info => Ok(OracleStep::Continue),
            FramebufferMode::UntilMatch => self.observe_until_match(observations),
        }
    }

    pub(crate) fn finish(
        &mut self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleOutcome, String> {
        match self.mode {
            FramebufferMode::Info => Ok(OracleOutcome::Passed),
            FramebufferMode::Final => {
                if self.framebuffer_matches(observations)? {
                    Ok(OracleOutcome::Passed)
                } else {
                    Ok(OracleOutcome::Failed(self.mismatch_message()))
                }
            }
            FramebufferMode::UntilMatch => {
                if self.matched {
                    Ok(OracleOutcome::Passed)
                } else if let Some(check_at_tcycles) = self.check_at_tcycles
                    && !self.check_at_reached
                {
                    Ok(OracleOutcome::Failed(format!(
                        "framebuffer check_at_tcycles {check_at_tcycles} was not reached; executed {} T-cycles",
                        observations.executed_tcycles
                    )))
                } else if self.framebuffer_matches(observations)? {
                    Ok(OracleOutcome::Passed)
                } else {
                    Ok(OracleOutcome::Failed(self.mismatch_message()))
                }
            }
        }
    }

    fn observe_until_match(
        &mut self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleStep, String> {
        if let Some(check_at_tcycles) = self.check_at_tcycles {
            if observations.executed_tcycles == check_at_tcycles {
                self.check_at_reached = true;
                self.matched = self.framebuffer_matches(observations)?;
                return Ok(OracleStep::Stop);
            }
            return Ok(OracleStep::Continue);
        }

        if observations.executed_tcycles != 0
            && observations
                .executed_tcycles
                .is_multiple_of(self.check_interval_tcycles)
        {
            self.pending_periodic_check = true;
        }

        if self.pending_periodic_check && self.target_framebuffer(observations)?.in_vblank {
            self.pending_periodic_check = false;
            if self.framebuffer_matches(observations)? {
                self.matched = true;
                return Ok(OracleStep::Stop);
            }
        }

        Ok(OracleStep::Continue)
    }

    fn framebuffer_matches(&self, observations: OracleObservations<'_>) -> Result<bool, String> {
        let framebuffer = self.target_framebuffer(observations)?;
        match &self.comparison {
            FramebufferComparison::Info => Ok(true),
            FramebufferComparison::PaletteRank { fixtures } => {
                let actual = match self.source {
                    FramebufferSource::Dmg => normalize_dmg_framebuffer(
                        framebuffer
                            .dmg
                            .ok_or_else(|| "DMG framebuffer is not available".to_string())?,
                    )?,
                    FramebufferSource::Cgb => {
                        normalize_rgb555_framebuffer(framebuffer.cgb_rgb555.ok_or_else(|| {
                            "CGB RGB555 framebuffer is not available".to_string()
                        })?)?
                    }
                };
                Ok(fixtures.iter().any(|fixture| fixture.framebuffer == actual))
            }
            FramebufferComparison::Grayscale {
                compare,
                tolerance,
                fixtures,
            } => {
                let actual = match self.source {
                    FramebufferSource::Dmg => dmg_grayscale_framebuffer(
                        framebuffer
                            .dmg
                            .ok_or_else(|| "DMG framebuffer is not available".to_string())?,
                    )?,
                    FramebufferSource::Cgb => {
                        rgb555_grayscale_framebuffer(framebuffer.cgb_rgb555.ok_or_else(|| {
                            "CGB RGB555 framebuffer is not available".to_string()
                        })?)?
                    }
                };
                Ok(fixtures.iter().any(|fixture| match compare {
                    FramebufferCompare::Exact => fixture.framebuffer == actual,
                    FramebufferCompare::GrayscaleTolerance => {
                        grayscale_matches_with_tolerance(&actual, &fixture.framebuffer, *tolerance)
                    }
                }))
            }
        }
    }

    fn target_framebuffer<'a>(
        &self,
        observations: OracleObservations<'a>,
    ) -> Result<FramebufferObservation<'a>, String> {
        let Some(target_participant) = &self.target_participant else {
            return Ok(observations.framebuffer);
        };
        observations
            .participants
            .iter()
            .find(|participant| participant.id == target_participant)
            .map(|participant| participant.framebuffer)
            .or_else(|| {
                observations.linked.and_then(|linked| {
                    linked
                        .participants
                        .iter()
                        .find(|participant| participant.id == target_participant)
                        .map(|participant| participant.framebuffer)
                })
            })
            .ok_or_else(|| {
                format!(
                    "framebuffer observations for participant {target_participant:?} are not available"
                )
            })
    }

    fn mismatch_message(&self) -> String {
        match &self.comparison {
            FramebufferComparison::Info => "framebuffer info oracle did not compare".to_string(),
            FramebufferComparison::PaletteRank { fixtures } => format!(
                "framebuffer did not match fixture {}",
                fixture_list(fixtures.iter().map(|fixture| &fixture.path))
            ),
            FramebufferComparison::Grayscale {
                compare,
                tolerance,
                fixtures,
            } => {
                let comparator = match compare {
                    FramebufferCompare::Exact => "exact grayscale",
                    FramebufferCompare::GrayscaleTolerance => "grayscale tolerance",
                };
                format!(
                    "framebuffer did not match {comparator} fixture {} with tolerance {tolerance}",
                    fixture_list(fixtures.iter().map(|fixture| &fixture.path))
                )
            }
        }
    }

    pub(crate) fn artifact_descriptor(&self) -> Option<FramebufferArtifactDescriptor> {
        let (projection, compare, tolerance, fixtures) = match &self.comparison {
            FramebufferComparison::Info => return None,
            FramebufferComparison::PaletteRank { fixtures } => (
                "palette-rank",
                "exact",
                None,
                fixtures
                    .iter()
                    .map(|fixture| fixture.path.clone())
                    .collect::<Vec<_>>(),
            ),
            FramebufferComparison::Grayscale {
                compare,
                tolerance,
                fixtures,
            } => (
                "grayscale",
                compare.as_str(),
                Some(*tolerance),
                fixtures
                    .iter()
                    .map(|fixture| fixture.path.clone())
                    .collect::<Vec<_>>(),
            ),
        };
        Some(FramebufferArtifactDescriptor {
            source: self.source.artifact_source(),
            mode: self.mode.as_str(),
            projection,
            compare,
            target_participant: self.target_participant.clone(),
            tolerance,
            fixtures,
        })
    }

    pub(crate) const fn is_informational(&self) -> bool {
        matches!(self.mode, FramebufferMode::Info)
    }
}

fn fixture_list<'a>(paths: impl Iterator<Item = &'a PathBuf>) -> String {
    paths
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FramebufferMode {
    Final,
    UntilMatch,
    Info,
}

impl FramebufferMode {
    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("final") {
            "final" => Ok(Self::Final),
            "until-match" => Ok(Self::UntilMatch),
            "info" => Ok(Self::Info),
            other => Err(format!("unsupported framebuffer mode {other:?}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::UntilMatch => "until-match",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FramebufferSource {
    Dmg,
    Cgb,
}

impl FramebufferSource {
    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("dmg") {
            "dmg" => Ok(Self::Dmg),
            "cgb" => Ok(Self::Cgb),
            other => Err(format!("unsupported framebuffer source {other:?}")),
        }
    }

    fn artifact_source(self) -> FramebufferArtifactSource {
        match self {
            Self::Dmg => FramebufferArtifactSource::Dmg,
            Self::Cgb => FramebufferArtifactSource::Cgb,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FramebufferProjection {
    PaletteRank,
    Grayscale,
}

impl FramebufferProjection {
    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("palette-rank") {
            "palette-rank" => Ok(Self::PaletteRank),
            "grayscale" => Ok(Self::Grayscale),
            other => Err(format!("unsupported framebuffer projection {other:?}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FramebufferCompare {
    Exact,
    GrayscaleTolerance,
}

impl FramebufferCompare {
    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("exact") {
            "exact" => Ok(Self::Exact),
            "grayscale-tolerance" => Ok(Self::GrayscaleTolerance),
            other => Err(format!("unsupported framebuffer compare {other:?}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::GrayscaleTolerance => "grayscale-tolerance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FramebufferArtifactSource {
    Dmg,
    Cgb,
}

impl FramebufferArtifactSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Dmg => "dmg",
            Self::Cgb => "cgb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FramebufferArtifactDescriptor {
    pub(crate) source: FramebufferArtifactSource,
    pub(crate) mode: &'static str,
    pub(crate) projection: &'static str,
    pub(crate) compare: &'static str,
    pub(crate) target_participant: Option<String>,
    pub(crate) tolerance: Option<u8>,
    pub(crate) fixtures: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FramebufferComparison {
    Info,
    PaletteRank {
        fixtures: Vec<FramebufferPaletteFixture>,
    },
    Grayscale {
        compare: FramebufferCompare,
        tolerance: u8,
        fixtures: Vec<FramebufferGrayscaleFixture>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FramebufferPaletteFixture {
    path: PathBuf,
    framebuffer: PaletteRankFramebuffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FramebufferGrayscaleFixture {
    path: PathBuf,
    framebuffer: GrayscaleFramebuffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PaletteRankFramebuffer {
    width: usize,
    height: usize,
    palette_ranks: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GrayscaleFramebuffer {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

fn resolve_fixture_path(
    fixture_roots: OracleFixtureRoots<'_>,
    local_fixtures: bool,
    path: &str,
) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if local_fixtures {
        validate_local_fixture_path(path)?;
        Ok(fixture_roots.local.join(path))
    } else if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(fixture_roots.store.join(path))
    }
}

fn validate_local_fixture_path(path: &Path) -> Result<(), String> {
    if path.is_absolute() {
        return Err(format!(
            "framebuffer local fixture path must be relative and confined to the report data directory: {}",
            path.display()
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::ParentDir => {
                return Err(format!(
                    "framebuffer local fixture path must not contain '..': {}",
                    path.display()
                ));
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "framebuffer local fixture path must be normalized and confined to the report data directory: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn read_palette_fixture(path: &Path) -> Result<PaletteRankFramebuffer, String> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "failed to read framebuffer fixture {}: {error}",
            path.display()
        )
    })?;
    decode_palette_fixture_bytes(path, &bytes)
}

fn read_grayscale_fixture(path: &Path) -> Result<GrayscaleFramebuffer, String> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "failed to read framebuffer fixture {}: {error}",
            path.display()
        )
    })?;
    decode_grayscale_fixture_bytes(path, &bytes)
}

fn decode_palette_fixture_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<PaletteRankFramebuffer, String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("pgm") => {
            let (width, height, pixels) = parse_pgm(path, bytes)?;
            Ok(normalize_indexed_pixels(width, height, pixels))
        }
        Some("png") => decode_png_palette_fixture(path, bytes),
        _ => Err(format!(
            "unsupported framebuffer fixture extension for {}",
            path.display()
        )),
    }
}

fn decode_grayscale_fixture_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<GrayscaleFramebuffer, String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("pgm") => {
            let (width, height, pixels) = parse_pgm(path, bytes)?;
            Ok(GrayscaleFramebuffer {
                width,
                height,
                pixels: pixels.to_vec(),
            })
        }
        Some("png") => decode_png_grayscale_fixture(path, bytes),
        _ => Err(format!(
            "unsupported framebuffer fixture extension for {}",
            path.display()
        )),
    }
}

fn decode_png_palette_fixture(path: &Path, bytes: &[u8]) -> Result<PaletteRankFramebuffer, String> {
    let decoded = decode_png(path, bytes)?;
    match decoded.color_type {
        png::ColorType::Grayscale => Ok(normalize_indexed_pixels(
            decoded.width,
            decoded.height,
            &decoded.bytes,
        )),
        png::ColorType::Rgb => Ok(normalize_rgb_pixels(
            decoded.width,
            decoded.height,
            &decoded
                .bytes
                .chunks_exact(3)
                .map(|chunk| [chunk[0], chunk[1], chunk[2]])
                .collect::<Vec<_>>(),
        )),
        png::ColorType::Rgba => Ok(normalize_rgb_pixels(
            decoded.width,
            decoded.height,
            &decoded
                .bytes
                .chunks_exact(4)
                .map(|chunk| [chunk[0], chunk[1], chunk[2]])
                .collect::<Vec<_>>(),
        )),
        png::ColorType::GrayscaleAlpha => Ok(normalize_indexed_pixels(
            decoded.width,
            decoded.height,
            &decoded
                .bytes
                .chunks_exact(2)
                .map(|chunk| chunk[0])
                .collect::<Vec<_>>(),
        )),
        png::ColorType::Indexed => Err(format!(
            "indexed PNG framebuffer fixtures are not supported: {}",
            path.display()
        )),
    }
}

fn decode_png_grayscale_fixture(path: &Path, bytes: &[u8]) -> Result<GrayscaleFramebuffer, String> {
    let decoded = decode_png(path, bytes)?;
    let pixels = match decoded.color_type {
        png::ColorType::Grayscale => decoded.bytes,
        png::ColorType::Rgb => decoded
            .bytes
            .chunks_exact(3)
            .map(|chunk| grayscale_luma([chunk[0], chunk[1], chunk[2]]))
            .collect(),
        png::ColorType::Rgba => decoded
            .bytes
            .chunks_exact(4)
            .map(|chunk| grayscale_luma([chunk[0], chunk[1], chunk[2]]))
            .collect(),
        png::ColorType::GrayscaleAlpha => decoded
            .bytes
            .chunks_exact(2)
            .map(|chunk| chunk[0])
            .collect(),
        png::ColorType::Indexed => {
            return Err(format!(
                "indexed PNG framebuffer fixtures are not supported: {}",
                path.display()
            ));
        }
    };
    Ok(GrayscaleFramebuffer {
        width: decoded.width,
        height: decoded.height,
        pixels,
    })
}

struct DecodedPng {
    width: usize,
    height: usize,
    color_type: png::ColorType,
    bytes: Vec<u8>,
}

fn decode_png(path: &Path, bytes: &[u8]) -> Result<DecodedPng, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("failed to decode PNG fixture {}: {error}", path.display()))?;
    let output_buffer_size = reader.output_buffer_size().ok_or_else(|| {
        format!(
            "PNG decoder did not expose output size for {}",
            path.display()
        )
    })?;
    let mut buffer = vec![0; output_buffer_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| format!("failed to decode PNG fixture {}: {error}", path.display()))?;
    Ok(DecodedPng {
        width: info.width as usize,
        height: info.height as usize,
        color_type: info.color_type,
        bytes: buffer[..info.buffer_size()].to_vec(),
    })
}

fn parse_pgm<'a>(path: &Path, bytes: &'a [u8]) -> Result<(usize, usize, &'a [u8]), String> {
    let mut index = 0;
    let magic = next_pgm_token(bytes, &mut index, "magic", path)?;
    if magic != b"P5" {
        return Err(format!(
            "unsupported PGM magic {:?} in {}",
            String::from_utf8_lossy(magic),
            path.display()
        ));
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
        return Err(format!(
            "unsupported PGM max value {max_value} in {}",
            path.display()
        ));
    }
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let expected_len = width
        .checked_mul(height)
        .ok_or_else(|| format!("PGM dimensions overflow in {}", path.display()))?;
    if bytes.len() < index + expected_len {
        return Err(format!(
            "PGM pixel payload is shorter than declared dimensions in {}",
            path.display()
        ));
    }
    Ok((width, height, &bytes[index..index + expected_len]))
}

fn next_pgm_token<'a>(
    bytes: &'a [u8],
    index: &mut usize,
    label: &str,
    path: &Path,
) -> Result<&'a [u8], String> {
    while *index < bytes.len() && bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
    let start = *index;
    while *index < bytes.len() && !bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
    if start == *index {
        return Err(format!("missing PGM {label} token in {}", path.display()));
    }
    Ok(&bytes[start..*index])
}

fn parse_usize_token(token: &[u8], path: &Path, label: &str) -> Result<usize, String> {
    std::str::from_utf8(token)
        .map_err(|error| {
            format!(
                "invalid UTF-8 in PGM {label} token for {}: {error}",
                path.display()
            )
        })?
        .parse::<usize>()
        .map_err(|error| format!("invalid PGM {label} token for {}: {error}", path.display()))
}

fn normalize_dmg_framebuffer(pixels: &[u8]) -> Result<PaletteRankFramebuffer, String> {
    let grayscale = dmg_grayscale_pixels(pixels)?;
    Ok(normalize_indexed_pixels(
        FRAMEBUFFER_WIDTH,
        FRAMEBUFFER_HEIGHT,
        &grayscale,
    ))
}

fn dmg_grayscale_framebuffer(pixels: &[u8]) -> Result<GrayscaleFramebuffer, String> {
    Ok(GrayscaleFramebuffer {
        width: FRAMEBUFFER_WIDTH,
        height: FRAMEBUFFER_HEIGHT,
        pixels: dmg_grayscale_pixels(pixels)?,
    })
}

fn dmg_grayscale_pixels(pixels: &[u8]) -> Result<Vec<u8>, String> {
    let expected_len = FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT;
    if pixels.len() != expected_len {
        return Err(format!(
            "DMG framebuffer length {} does not match expected {expected_len}",
            pixels.len()
        ));
    }
    Ok(pixels
        .iter()
        .copied()
        .map(|pixel| match pixel {
            0..=3 => DMG_GRAYSCALE_SHADES[usize::from(pixel)],
            _ => DMG_GRAYSCALE_SHADES[3],
        })
        .collect())
}

fn normalize_rgb555_framebuffer(pixels: &[u16]) -> Result<PaletteRankFramebuffer, String> {
    let colors = rgb555_colors(pixels)?;
    Ok(normalize_rgb_pixels(
        FRAMEBUFFER_WIDTH,
        FRAMEBUFFER_HEIGHT,
        &colors,
    ))
}

fn rgb555_grayscale_framebuffer(pixels: &[u16]) -> Result<GrayscaleFramebuffer, String> {
    Ok(GrayscaleFramebuffer {
        width: FRAMEBUFFER_WIDTH,
        height: FRAMEBUFFER_HEIGHT,
        pixels: rgb555_colors(pixels)?
            .into_iter()
            .map(grayscale_luma)
            .collect(),
    })
}

fn rgb555_colors(pixels: &[u16]) -> Result<Vec<[u8; 3]>, String> {
    let expected_len = FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT;
    if pixels.len() != expected_len {
        return Err(format!(
            "CGB RGB555 framebuffer length {} does not match expected {expected_len}",
            pixels.len()
        ));
    }
    Ok(pixels.iter().copied().map(rgb555_to_rgb888).collect())
}

fn normalize_indexed_pixels(width: usize, height: usize, pixels: &[u8]) -> PaletteRankFramebuffer {
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

    PaletteRankFramebuffer {
        width,
        height,
        palette_ranks,
    }
}

fn normalize_rgb_pixels(width: usize, height: usize, pixels: &[[u8; 3]]) -> PaletteRankFramebuffer {
    let mut colors = pixels.to_vec();
    colors.sort_unstable();
    colors.dedup();
    colors.sort_by(|left, right| {
        color_luminance(right)
            .cmp(&color_luminance(left))
            .then(right.cmp(left))
    });

    let rank_by_color = colors
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

    PaletteRankFramebuffer {
        width,
        height,
        palette_ranks,
    }
}

fn grayscale_matches_with_tolerance(
    actual: &GrayscaleFramebuffer,
    expected: &GrayscaleFramebuffer,
    tolerance: u8,
) -> bool {
    actual.width == expected.width
        && actual.height == expected.height
        && actual.pixels.len() == expected.pixels.len()
        && actual
            .pixels
            .iter()
            .zip(&expected.pixels)
            .all(|(left, right)| left.abs_diff(*right) <= tolerance)
}

fn color_luminance(color: &[u8; 3]) -> u16 {
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
