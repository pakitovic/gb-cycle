use std::fmt;
use std::path::PathBuf;

use super::BENCHMARK_CASE_VERSION;

#[derive(Debug)]
pub enum BenchmarkConfigError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    UnsupportedVersion {
        path: PathBuf,
        version: u32,
    },
    EmptyId {
        path: PathBuf,
    },
    InvalidArtifactId {
        path: PathBuf,
        id: String,
        run_id: Option<String>,
        value: String,
    },
    EmptyRunId {
        path: PathBuf,
        id: String,
        index: usize,
    },
    MissingRuns {
        path: PathBuf,
        id: String,
    },
    DeprecatedLegacyFormat {
        path: PathBuf,
        id: String,
    },
    ZeroDuration {
        path: PathBuf,
        id: String,
        run_id: Option<String>,
    },
    MultipleRuns {
        path: PathBuf,
        id: String,
        count: usize,
    },
    InvalidInput {
        path: PathBuf,
        id: String,
        run_id: String,
        index: usize,
        reason: &'static str,
    },
}

impl fmt::Display for BenchmarkConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    f,
                    "failed to read benchmark case {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, source } => {
                write!(
                    f,
                    "failed to parse benchmark case {}: {source}",
                    path.display()
                )
            }
            Self::UnsupportedVersion { path, version } => write!(
                f,
                "unsupported benchmark case version {version} in {}; expected {BENCHMARK_CASE_VERSION}",
                path.display()
            ),
            Self::EmptyId { path } => {
                write!(
                    f,
                    "benchmark case {} must define a non-empty id",
                    path.display()
                )
            }
            Self::InvalidArtifactId {
                path,
                id,
                run_id,
                value,
            } => {
                if let Some(run_id) = run_id {
                    write!(
                        f,
                        "benchmark case {id:?} run {run_id:?} in {} uses unsafe artifact id component {value:?}; use only ASCII letters, digits, '-' and '_'",
                        path.display()
                    )
                } else {
                    write!(
                        f,
                        "benchmark case {id:?} in {} uses unsafe artifact id component {value:?}; use only ASCII letters, digits, '-' and '_'",
                        path.display()
                    )
                }
            }
            Self::EmptyRunId { path, id, index } => write!(
                f,
                "benchmark case {id:?} in {} run #{index} must define a non-empty id",
                path.display()
            ),
            Self::MissingRuns { path, id } => write!(
                f,
                "benchmark case {id:?} in {} must define at least one [[run]]",
                path.display()
            ),
            Self::DeprecatedLegacyFormat { path, id } => write!(
                f,
                "benchmark case {id:?} in {} uses the removed legacy duration_seconds + [[stimulus]] format; define one or more [[run]] entries with duration_seconds and [[run.input]] instead",
                path.display()
            ),
            Self::ZeroDuration { path, id, run_id } => {
                if let Some(run_id) = run_id {
                    write!(
                        f,
                        "benchmark case {id:?} run {run_id:?} in {} must use duration_seconds > 0",
                        path.display()
                    )
                } else {
                    write!(
                        f,
                        "benchmark case {id:?} in {} must use duration_seconds > 0",
                        path.display()
                    )
                }
            }
            Self::MultipleRuns { path, id, count } => write!(
                f,
                "benchmark case {id:?} in {} expands to {count} runs; use load_benchmark_cases for multi-run suites",
                path.display()
            ),
            Self::InvalidInput {
                path,
                id,
                run_id,
                index,
                reason,
            } => write!(
                f,
                "benchmark case {id:?} run {run_id:?} in {} input #{index} is invalid: {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for BenchmarkConfigError {}
