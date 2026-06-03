use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{BenchmarkMode, BenchmarkModel, BenchmarkPalette, BenchmarkStartup};

use super::input::{expand_input, resolve_duration};
use super::{BENCHMARK_CASE_VERSION, BenchmarkCase, BenchmarkConfigError, BenchmarkSuite};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BenchmarkCaseFile {
    version: u32,
    id: String,
    rom: PathBuf,
    model: BenchmarkModel,
    startup: BenchmarkStartup,
    mode: BenchmarkMode,
    palette: Option<BenchmarkPalette>,
    #[serde(default)]
    duration_seconds: Option<u32>,
    #[serde(default = "default_true")]
    screenshot: bool,
    #[serde(default = "default_true")]
    stats: bool,
    #[serde(rename = "stimulus", default)]
    legacy_stimuli: Vec<toml::Value>,
    #[serde(rename = "run", default)]
    runs: Vec<BenchmarkRunFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkRunFile {
    id: Option<String>,
    label: Option<String>,
    duration_seconds: Option<u32>,
    screenshot: Option<bool>,
    stats: Option<bool>,
    #[serde(rename = "stimulus", default)]
    legacy_stimuli: Vec<toml::Value>,
    #[serde(rename = "input", default)]
    inputs: Vec<BenchmarkInputFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BenchmarkInputFile {
    pub(super) frame: Option<u32>,
    pub(super) second: Option<u32>,
    pub(super) tcycle: Option<u64>,
    pub(super) button: Option<String>,
    pub(super) buttons: Option<Vec<String>>,
    pub(super) hold_frames: Option<u32>,
    pub(super) repeat_every_frames: Option<u32>,
}

const fn default_true() -> bool {
    true
}

pub fn load_benchmark_suite(
    path: impl AsRef<Path>,
) -> Result<BenchmarkSuite, BenchmarkConfigError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| BenchmarkConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_benchmark_suite(path, &text)
}

pub fn load_benchmark_cases(
    path: impl AsRef<Path>,
) -> Result<Vec<BenchmarkCase>, BenchmarkConfigError> {
    load_benchmark_suite(path).map(|suite| suite.cases)
}

pub fn load_benchmark_case(path: impl AsRef<Path>) -> Result<BenchmarkCase, BenchmarkConfigError> {
    let suite = load_benchmark_suite(path)?;
    single_case_from_suite(suite)
}

pub fn parse_benchmark_suite(
    path: impl AsRef<Path>,
    text: &str,
) -> Result<BenchmarkSuite, BenchmarkConfigError> {
    let path = path.as_ref();
    let mut parsed = toml::from_str::<BenchmarkCaseFile>(text).map_err(|source| {
        BenchmarkConfigError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })?;
    if parsed.version != BENCHMARK_CASE_VERSION {
        return Err(BenchmarkConfigError::UnsupportedVersion {
            path: path.to_path_buf(),
            version: parsed.version,
        });
    }
    let id = parsed.id.trim().to_string();
    if id.is_empty() {
        return Err(BenchmarkConfigError::EmptyId {
            path: path.to_path_buf(),
        });
    }
    validate_artifact_id_component(path, &id, None, &id)?;
    if parsed.duration_seconds.is_some() || !parsed.legacy_stimuli.is_empty() {
        return Err(BenchmarkConfigError::DeprecatedLegacyFormat {
            path: path.to_path_buf(),
            id,
        });
    }
    if parsed.runs.is_empty() {
        return Err(BenchmarkConfigError::MissingRuns {
            path: path.to_path_buf(),
            id,
        });
    }

    parsed.rom = resolve_rom_path(path, &parsed.rom);

    let cases = parsed
        .runs
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, run)| build_run_case(path, &id, &parsed, index, run))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(BenchmarkSuite {
        source_path: path.to_path_buf(),
        id,
        rom: parsed.rom,
        model: parsed.model,
        startup: parsed.startup,
        mode: parsed.mode,
        palette: parsed.palette,
        cases,
    })
}

pub fn parse_benchmark_cases(
    path: impl AsRef<Path>,
    text: &str,
) -> Result<Vec<BenchmarkCase>, BenchmarkConfigError> {
    parse_benchmark_suite(path, text).map(|suite| suite.cases)
}

pub fn parse_benchmark_case(
    path: impl AsRef<Path>,
    text: &str,
) -> Result<BenchmarkCase, BenchmarkConfigError> {
    let suite = parse_benchmark_suite(path, text)?;
    single_case_from_suite(suite)
}

fn single_case_from_suite(suite: BenchmarkSuite) -> Result<BenchmarkCase, BenchmarkConfigError> {
    let mut cases = suite.cases;
    if cases.len() == 1 {
        Ok(cases.remove(0))
    } else {
        Err(BenchmarkConfigError::MultipleRuns {
            path: suite.source_path,
            id: suite.id,
            count: cases.len(),
        })
    }
}

fn build_run_case(
    path: &Path,
    id: &str,
    parsed: &BenchmarkCaseFile,
    run_index: usize,
    run: BenchmarkRunFile,
) -> Result<BenchmarkCase, BenchmarkConfigError> {
    let run_id = run
        .id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if run_id.is_empty() {
        return Err(BenchmarkConfigError::EmptyRunId {
            path: path.to_path_buf(),
            id: id.to_string(),
            index: run_index,
        });
    }
    validate_artifact_id_component(path, id, Some(&run_id), &run_id)?;
    if !run.legacy_stimuli.is_empty() {
        return Err(BenchmarkConfigError::DeprecatedLegacyFormat {
            path: path.to_path_buf(),
            id: id.to_string(),
        });
    }
    let duration_seconds = resolve_duration(path, id, Some(&run_id), run.duration_seconds)?;
    let mut stimuli = Vec::new();
    for (index, input) in run.inputs.into_iter().enumerate() {
        expand_input(
            path,
            id,
            &run_id,
            index,
            duration_seconds,
            input,
            &mut stimuli,
        )?;
    }

    Ok(BenchmarkCase {
        source_path: path.to_path_buf(),
        id: id.to_string(),
        run_id: Some(run_id.clone()),
        run_label: run.label,
        artifact_id: format!("{id}-{run_id}"),
        rom: parsed.rom.clone(),
        model: parsed.model,
        startup: parsed.startup,
        mode: parsed.mode,
        palette: parsed.palette,
        duration_seconds,
        screenshot: run.screenshot.unwrap_or(parsed.screenshot),
        stats: run.stats.unwrap_or(parsed.stats),
        stimuli,
    })
}

fn resolve_rom_path(path: &Path, rom: &Path) -> PathBuf {
    if rom.is_absolute() {
        rom.to_path_buf()
    } else {
        path.parent().unwrap_or_else(|| Path::new("")).join(rom)
    }
}

fn validate_artifact_id_component(
    path: &Path,
    id: &str,
    run_id: Option<&str>,
    value: &str,
) -> Result<(), BenchmarkConfigError> {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        Ok(())
    } else {
        Err(BenchmarkConfigError::InvalidArtifactId {
            path: path.to_path_buf(),
            id: id.to_string(),
            run_id: run_id.map(ToString::to_string),
            value: value.to_string(),
        })
    }
}
