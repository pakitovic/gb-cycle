mod error;
mod input;
mod parser;

#[cfg(test)]
mod test;

use std::path::PathBuf;

use crate::{BenchmarkMode, BenchmarkModel, BenchmarkPalette, BenchmarkStartup, BenchmarkStimulus};

pub use error::BenchmarkConfigError;
pub use parser::{
    load_benchmark_case, load_benchmark_cases, load_benchmark_suite, parse_benchmark_case,
    parse_benchmark_cases, parse_benchmark_suite,
};

pub const BENCHMARK_CASE_VERSION: u32 = 1;
pub const DEFAULT_INPUT_HOLD_FRAMES: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkSuite {
    pub source_path: PathBuf,
    pub id: String,
    pub rom: PathBuf,
    pub model: BenchmarkModel,
    pub startup: BenchmarkStartup,
    pub mode: BenchmarkMode,
    pub palette: Option<BenchmarkPalette>,
    pub cases: Vec<BenchmarkCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkCase {
    pub source_path: PathBuf,
    pub id: String,
    pub run_id: Option<String>,
    pub run_label: Option<String>,
    pub artifact_id: String,
    pub rom: PathBuf,
    pub model: BenchmarkModel,
    pub startup: BenchmarkStartup,
    pub mode: BenchmarkMode,
    pub palette: Option<BenchmarkPalette>,
    pub duration_seconds: u32,
    pub screenshot: bool,
    pub stats: bool,
    pub stimuli: Vec<BenchmarkStimulus>,
}
