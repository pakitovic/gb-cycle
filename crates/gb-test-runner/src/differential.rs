use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gb_core::ExecutionMode;
use serde::Deserialize;

use crate::{
    CaptureKind, CapturedArtifacts, CapturedMemoryTextOutput, RomCaseReport, RomExecutionError,
    RomRunner, RomSuite, RomSuiteValidationError, TestSubsystem, artifact_file_name,
    framebuffer_oracle::{
        NormalizedFramebuffer, convert_pgm_to_png, decode_fixture_framebuffer_path,
        decode_local_pgm_framebuffer,
    },
    render_memory_text_output,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferentialOracle {
    SameBoy,
}

impl DifferentialOracle {
    pub fn name(self) -> &'static str {
        match self {
            Self::SameBoy => "sameboy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferentialOracleLayout {
    CaseBundle,
    SameBoyTester,
}

impl DifferentialOracleLayout {
    pub fn name(self) -> &'static str {
        match self {
            Self::CaseBundle => "case-bundle",
            Self::SameBoyTester => "sameboy-tester",
        }
    }
}

#[derive(Debug)]
pub enum DifferentialExecutionError {
    InvalidSuite(RomSuiteValidationError),
    NonStrictCase {
        case_id: String,
        actual: ExecutionMode,
    },
    UnsupportedOracleLayoutForCapture {
        case_id: String,
        layout: DifferentialOracleLayout,
        capture: CaptureKind,
    },
    RomExecution {
        source: RomExecutionError,
    },
    MissingLocalArtifact {
        case_id: String,
        capture: CaptureKind,
    },
    ReadOracleArtifact {
        path: PathBuf,
        operation: &'static str,
        source: io::Error,
    },
    ParseOracleArtifact {
        path: PathBuf,
        message: String,
    },
    CreateDirectory {
        path: PathBuf,
        source: io::Error,
    },
    WriteArtifact {
        path: PathBuf,
        operation: &'static str,
        source: io::Error,
    },
    CopyArtifact {
        source_path: PathBuf,
        destination_path: PathBuf,
        source: io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DifferentialCaseMismatch {
    MissingOracleArtifact {
        path: PathBuf,
        capture: CaptureKind,
    },
    SerialMismatch {
        oracle_artifact_path: PathBuf,
        oracle: String,
        local: String,
    },
    MemoryTextOutputMismatch {
        oracle_artifact_path: PathBuf,
        oracle: CapturedMemoryTextOutput,
        local: CapturedMemoryTextOutput,
    },
    BlarggConsoleTextMismatch {
        oracle_artifact_path: PathBuf,
        oracle: String,
        local: String,
    },
    TraceMismatch {
        oracle_artifact_path: PathBuf,
        oracle: String,
        local: String,
    },
    SnapshotMismatch {
        oracle_artifact_path: PathBuf,
        oracle: String,
        local: String,
    },
    FramebufferMismatch {
        oracle_artifact_path: PathBuf,
        local_width: usize,
        local_height: usize,
        oracle_width: usize,
        oracle_height: usize,
        first_difference: Option<FramebufferDifferencePoint>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramebufferDifferencePoint {
    pub x: usize,
    pub y: usize,
    pub local_rank: u8,
    pub oracle_rank: u8,
}

impl DifferentialCaseMismatch {
    pub fn name(&self) -> &'static str {
        match self {
            Self::MissingOracleArtifact { .. } => "missing-oracle-artifact",
            Self::SerialMismatch { .. } => "serial-mismatch",
            Self::MemoryTextOutputMismatch { .. } => "memory-text-output-mismatch",
            Self::BlarggConsoleTextMismatch { .. } => "blargg-console-text-mismatch",
            Self::TraceMismatch { .. } => "trace-mismatch",
            Self::SnapshotMismatch { .. } => "snapshot-mismatch",
            Self::FramebufferMismatch { .. } => "framebuffer-mismatch",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::MissingOracleArtifact { path, capture } => format!(
                "missing_path={} channel={}",
                path.display(),
                capture_name(*capture)
            ),
            Self::SerialMismatch { oracle, local, .. } => describe_text_difference(local, oracle),
            Self::MemoryTextOutputMismatch { oracle, local, .. } => {
                describe_memory_text_output_difference(local, oracle)
            }
            Self::BlarggConsoleTextMismatch { oracle, local, .. } => {
                describe_text_difference(local, oracle)
            }
            Self::TraceMismatch { oracle, local, .. } => describe_text_difference(local, oracle),
            Self::SnapshotMismatch { oracle, local, .. } => describe_text_difference(local, oracle),
            Self::FramebufferMismatch {
                local_width,
                local_height,
                oracle_width,
                oracle_height,
                first_difference,
                ..
            } => describe_framebuffer_difference(
                *local_width,
                *local_height,
                *oracle_width,
                *oracle_height,
                first_difference.as_ref(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DifferentialCaseOutcome {
    Matched,
    Diverged(DifferentialCaseMismatch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifferentialCaseReport {
    pub case_id: String,
    pub oracle: DifferentialOracle,
    pub oracle_layout: DifferentialOracleLayout,
    pub compared_capture: CaptureKind,
    pub oracle_artifact_path: PathBuf,
    pub local_report: RomCaseReport,
    pub outcome: DifferentialCaseOutcome,
    pub archived_context_artifacts: Vec<PathBuf>,
}

impl DifferentialCaseReport {
    pub fn matched(&self) -> bool {
        matches!(self.outcome, DifferentialCaseOutcome::Matched)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifferentialSuiteReport {
    pub suite_name: String,
    pub subsystem: TestSubsystem,
    pub oracle: DifferentialOracle,
    pub oracle_layout: DifferentialOracleLayout,
    pub cases: Vec<DifferentialCaseReport>,
}

impl DifferentialSuiteReport {
    pub fn all_matched(&self) -> bool {
        self.cases.iter().all(DifferentialCaseReport::matched)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifferentialRunner {
    oracle: DifferentialOracle,
    oracle_layout: DifferentialOracleLayout,
    oracle_artifact_root: PathBuf,
    rom_runner: RomRunner,
    failure_artifact_root: Option<PathBuf>,
}

impl DifferentialRunner {
    pub fn new(oracle: DifferentialOracle, oracle_artifact_root: impl Into<PathBuf>) -> Self {
        Self {
            oracle,
            oracle_layout: DifferentialOracleLayout::CaseBundle,
            oracle_artifact_root: oracle_artifact_root.into(),
            rom_runner: RomRunner::new(),
            failure_artifact_root: None,
        }
    }

    pub fn with_oracle_layout(mut self, oracle_layout: DifferentialOracleLayout) -> Self {
        self.oracle_layout = oracle_layout;
        self
    }

    pub fn with_rom_runner(mut self, rom_runner: RomRunner) -> Self {
        self.rom_runner = rom_runner;
        self
    }

    pub fn with_failure_artifact_root(mut self, failure_artifact_root: impl Into<PathBuf>) -> Self {
        self.failure_artifact_root = Some(failure_artifact_root.into());
        self
    }

    pub fn run_suite(
        &self,
        suite: &RomSuite,
    ) -> Result<DifferentialSuiteReport, DifferentialExecutionError> {
        suite
            .validate()
            .map_err(DifferentialExecutionError::InvalidSuite)?;
        self.ensure_strict_suite(suite)?;

        let mut case_reports = Vec::with_capacity(suite.cases.len());
        for case in &suite.cases {
            case_reports.push(self.run_case(case)?);
        }

        Ok(DifferentialSuiteReport {
            suite_name: suite.name.clone(),
            subsystem: suite.subsystem,
            oracle: self.oracle,
            oracle_layout: self.oracle_layout,
            cases: case_reports,
        })
    }

    fn ensure_strict_suite(&self, suite: &RomSuite) -> Result<(), DifferentialExecutionError> {
        for case in &suite.cases {
            if case.execution_mode != ExecutionMode::Strict {
                return Err(DifferentialExecutionError::NonStrictCase {
                    case_id: case.id.clone(),
                    actual: case.execution_mode,
                });
            }
        }

        Ok(())
    }

    fn run_case(
        &self,
        case: &crate::RomTestCase,
    ) -> Result<DifferentialCaseReport, DifferentialExecutionError> {
        let local_report = self
            .rom_runner
            .run_case(case)
            .map_err(|source| DifferentialExecutionError::RomExecution { source })?;
        let compared_capture = case.pass_condition.required_capture();
        let oracle_artifact_path = self.resolve_oracle_artifact_path(case, compared_capture)?;
        let outcome =
            self.compare_required_capture(&local_report, compared_capture, &oracle_artifact_path)?;
        let archived_context_artifacts =
            self.persist_context_if_needed(case, &local_report, &outcome, &oracle_artifact_path)?;

        Ok(DifferentialCaseReport {
            case_id: case.id.clone(),
            oracle: self.oracle,
            oracle_layout: self.oracle_layout,
            compared_capture,
            oracle_artifact_path,
            local_report,
            outcome,
            archived_context_artifacts,
        })
    }

    fn resolve_oracle_artifact_path(
        &self,
        case: &crate::RomTestCase,
        capture: CaptureKind,
    ) -> Result<PathBuf, DifferentialExecutionError> {
        match self.oracle_layout {
            DifferentialOracleLayout::CaseBundle => {
                let candidate = self
                    .oracle_artifact_root
                    .join(&case.id)
                    .join(artifact_file_name(capture));
                if capture != CaptureKind::Framebuffer || candidate.is_file() {
                    return Ok(candidate);
                }

                let legacy_pgm = replace_extension(&candidate, "pgm");
                if legacy_pgm.is_file() {
                    return Ok(legacy_pgm);
                }

                Ok(candidate)
            }
            DifferentialOracleLayout::SameBoyTester => {
                if capture != CaptureKind::Framebuffer {
                    return Err(
                        DifferentialExecutionError::UnsupportedOracleLayoutForCapture {
                            case_id: case.id.clone(),
                            layout: self.oracle_layout,
                            capture,
                        },
                    );
                }

                let bmp_candidate =
                    replace_extension(&self.oracle_artifact_root.join(&case.rom_path), "bmp");
                if bmp_candidate.is_file() {
                    return Ok(bmp_candidate);
                }

                let tga_candidate =
                    replace_extension(&self.oracle_artifact_root.join(&case.rom_path), "tga");
                if tga_candidate.is_file() {
                    return Ok(tga_candidate);
                }

                Ok(bmp_candidate)
            }
        }
    }

    fn compare_required_capture(
        &self,
        local_report: &RomCaseReport,
        compared_capture: CaptureKind,
        oracle_artifact_path: &Path,
    ) -> Result<DifferentialCaseOutcome, DifferentialExecutionError> {
        if !oracle_artifact_path.is_file() {
            return Ok(DifferentialCaseOutcome::Diverged(
                DifferentialCaseMismatch::MissingOracleArtifact {
                    path: oracle_artifact_path.to_path_buf(),
                    capture: compared_capture,
                },
            ));
        }

        let artifacts = &local_report.artifacts;
        Ok(match compared_capture {
            CaptureKind::Serial => {
                let local = artifacts.serial.clone().ok_or(
                    DifferentialExecutionError::MissingLocalArtifact {
                        case_id: local_report.case_id.clone(),
                        capture: compared_capture,
                    },
                )?;
                let oracle = fs::read_to_string(oracle_artifact_path).map_err(|source| {
                    DifferentialExecutionError::ReadOracleArtifact {
                        path: oracle_artifact_path.to_path_buf(),
                        operation: "read serial oracle artifact",
                        source,
                    }
                })?;

                if local == oracle {
                    DifferentialCaseOutcome::Matched
                } else {
                    DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::SerialMismatch {
                        oracle_artifact_path: oracle_artifact_path.to_path_buf(),
                        oracle,
                        local,
                    })
                }
            }
            CaptureKind::SerialHex => {
                let local = artifacts.serial_hex.clone().ok_or(
                    DifferentialExecutionError::MissingLocalArtifact {
                        case_id: local_report.case_id.clone(),
                        capture: compared_capture,
                    },
                )?;
                let oracle = fs::read_to_string(oracle_artifact_path).map_err(|source| {
                    DifferentialExecutionError::ReadOracleArtifact {
                        path: oracle_artifact_path.to_path_buf(),
                        operation: "read serial hex oracle artifact",
                        source,
                    }
                })?;

                if local == oracle {
                    DifferentialCaseOutcome::Matched
                } else {
                    DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::SerialMismatch {
                        oracle_artifact_path: oracle_artifact_path.to_path_buf(),
                        oracle,
                        local,
                    })
                }
            }
            CaptureKind::MemoryTextOutput => {
                let local = artifacts.memory_text_output.clone().ok_or(
                    DifferentialExecutionError::MissingLocalArtifact {
                        case_id: local_report.case_id.clone(),
                        capture: compared_capture,
                    },
                )?;
                let oracle = parse_memory_text_output_artifact(oracle_artifact_path)?;

                if local == oracle {
                    DifferentialCaseOutcome::Matched
                } else {
                    DifferentialCaseOutcome::Diverged(
                        DifferentialCaseMismatch::MemoryTextOutputMismatch {
                            oracle_artifact_path: oracle_artifact_path.to_path_buf(),
                            oracle,
                            local,
                        },
                    )
                }
            }
            CaptureKind::BlarggConsoleText => {
                let local = artifacts.blargg_console_text.clone().ok_or(
                    DifferentialExecutionError::MissingLocalArtifact {
                        case_id: local_report.case_id.clone(),
                        capture: compared_capture,
                    },
                )?;
                let oracle = fs::read_to_string(oracle_artifact_path).map_err(|source| {
                    DifferentialExecutionError::ReadOracleArtifact {
                        path: oracle_artifact_path.to_path_buf(),
                        operation: "read blargg console oracle artifact",
                        source,
                    }
                })?;

                if local == oracle {
                    DifferentialCaseOutcome::Matched
                } else {
                    DifferentialCaseOutcome::Diverged(
                        DifferentialCaseMismatch::BlarggConsoleTextMismatch {
                            oracle_artifact_path: oracle_artifact_path.to_path_buf(),
                            oracle,
                            local,
                        },
                    )
                }
            }
            CaptureKind::Framebuffer => {
                let local = artifacts.framebuffer_pgm.as_deref().ok_or(
                    DifferentialExecutionError::MissingLocalArtifact {
                        case_id: local_report.case_id.clone(),
                        capture: compared_capture,
                    },
                )?;

                match self.oracle_layout {
                    DifferentialOracleLayout::CaseBundle => {
                        let local_normalized =
                            decode_local_pgm_framebuffer(local_report.case_id.as_str(), local)
                                .map_err(|error| {
                                    DifferentialExecutionError::ParseOracleArtifact {
                                        path: error.path,
                                        message: error.message,
                                    }
                                })?;
                        let oracle_normalized =
                            decode_fixture_framebuffer_path(oracle_artifact_path).map_err(
                                |error| DifferentialExecutionError::ParseOracleArtifact {
                                    path: error.path,
                                    message: error.message,
                                },
                            )?;

                        if local_normalized == oracle_normalized {
                            DifferentialCaseOutcome::Matched
                        } else {
                            DifferentialCaseOutcome::Diverged(
                                DifferentialCaseMismatch::FramebufferMismatch {
                                    oracle_artifact_path: oracle_artifact_path.to_path_buf(),
                                    local_width: local_normalized.width,
                                    local_height: local_normalized.height,
                                    oracle_width: oracle_normalized.width,
                                    oracle_height: oracle_normalized.height,
                                    first_difference: first_framebuffer_difference(
                                        &local_normalized,
                                        &oracle_normalized,
                                    ),
                                },
                            )
                        }
                    }
                    DifferentialOracleLayout::SameBoyTester => {
                        let local_normalized =
                            decode_local_pgm_framebuffer(local_report.case_id.as_str(), local)
                                .map_err(|error| {
                                    DifferentialExecutionError::ParseOracleArtifact {
                                        path: error.path,
                                        message: error.message,
                                    }
                                })?;
                        let oracle_normalized =
                            decode_sameboy_tester_framebuffer(oracle_artifact_path)?;

                        if local_normalized == oracle_normalized {
                            DifferentialCaseOutcome::Matched
                        } else {
                            DifferentialCaseOutcome::Diverged(
                                DifferentialCaseMismatch::FramebufferMismatch {
                                    oracle_artifact_path: oracle_artifact_path.to_path_buf(),
                                    local_width: local_normalized.width,
                                    local_height: local_normalized.height,
                                    oracle_width: oracle_normalized.width,
                                    oracle_height: oracle_normalized.height,
                                    first_difference: first_framebuffer_difference(
                                        &local_normalized,
                                        &oracle_normalized,
                                    ),
                                },
                            )
                        }
                    }
                }
            }
            CaptureKind::Trace => {
                let local = artifacts.trace.clone().ok_or(
                    DifferentialExecutionError::MissingLocalArtifact {
                        case_id: local_report.case_id.clone(),
                        capture: compared_capture,
                    },
                )?;
                let oracle = fs::read_to_string(oracle_artifact_path).map_err(|source| {
                    DifferentialExecutionError::ReadOracleArtifact {
                        path: oracle_artifact_path.to_path_buf(),
                        operation: "read trace oracle artifact",
                        source,
                    }
                })?;

                if local == oracle {
                    DifferentialCaseOutcome::Matched
                } else {
                    DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::TraceMismatch {
                        oracle_artifact_path: oracle_artifact_path.to_path_buf(),
                        oracle,
                        local,
                    })
                }
            }
            CaptureKind::Snapshot => {
                let local = artifacts.snapshot_text.clone().ok_or(
                    DifferentialExecutionError::MissingLocalArtifact {
                        case_id: local_report.case_id.clone(),
                        capture: compared_capture,
                    },
                )?;
                let oracle = fs::read_to_string(oracle_artifact_path).map_err(|source| {
                    DifferentialExecutionError::ReadOracleArtifact {
                        path: oracle_artifact_path.to_path_buf(),
                        operation: "read snapshot oracle artifact",
                        source,
                    }
                })?;

                if local == oracle {
                    DifferentialCaseOutcome::Matched
                } else {
                    DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::SnapshotMismatch {
                        oracle_artifact_path: oracle_artifact_path.to_path_buf(),
                        oracle,
                        local,
                    })
                }
            }
        })
    }

    fn persist_context_if_needed(
        &self,
        case: &crate::RomTestCase,
        local_report: &RomCaseReport,
        outcome: &DifferentialCaseOutcome,
        oracle_artifact_path: &Path,
    ) -> Result<Vec<PathBuf>, DifferentialExecutionError> {
        let should_archive =
            !local_report.passed() || matches!(outcome, DifferentialCaseOutcome::Diverged(_));
        let Some(root) = &self.failure_artifact_root else {
            return Ok(Vec::new());
        };
        if !should_archive {
            return Ok(Vec::new());
        }

        let case_dir = root.join(&case.id);
        fs::create_dir_all(&case_dir).map_err(|source| {
            DifferentialExecutionError::CreateDirectory {
                path: case_dir.clone(),
                source,
            }
        })?;

        let mut written_paths = Vec::new();

        let summary_path = case_dir.join("differential_summary.txt");
        fs::write(
            &summary_path,
            render_differential_summary(
                self.oracle,
                self.oracle_layout,
                case.pass_condition.required_capture(),
                local_report,
                outcome,
                oracle_artifact_path,
            ),
        )
        .map_err(|source| DifferentialExecutionError::WriteArtifact {
            path: summary_path.clone(),
            operation: "write differential summary artifact",
            source,
        })?;
        written_paths.push(summary_path);

        let local_dir = case_dir.join("local");
        fs::create_dir_all(&local_dir).map_err(|source| {
            DifferentialExecutionError::CreateDirectory {
                path: local_dir.clone(),
                source,
            }
        })?;
        for capture in case.failure_artifacts.retained() {
            let Some(path) =
                write_captured_artifact(&local_dir, *capture, &local_report.artifacts)?
            else {
                continue;
            };
            written_paths.push(path);
        }

        if oracle_artifact_path.is_file() {
            let oracle_dir = case_dir.join("oracle");
            fs::create_dir_all(&oracle_dir).map_err(|source| {
                DifferentialExecutionError::CreateDirectory {
                    path: oracle_dir.clone(),
                    source,
                }
            })?;
            let copied_path =
                oracle_dir.join(oracle_artifact_path.file_name().unwrap_or_else(|| {
                    artifact_file_name(case.pass_condition.required_capture()).as_ref()
                }));
            fs::copy(oracle_artifact_path, &copied_path).map_err(|source| {
                DifferentialExecutionError::CopyArtifact {
                    source_path: oracle_artifact_path.to_path_buf(),
                    destination_path: copied_path.clone(),
                    source,
                }
            })?;
            written_paths.push(copied_path);
        }

        Ok(written_paths)
    }
}

fn replace_extension(path: &Path, extension: &str) -> PathBuf {
    path.with_extension(extension)
}

fn describe_memory_text_output_difference(
    local: &CapturedMemoryTextOutput,
    oracle: &CapturedMemoryTextOutput,
) -> String {
    if local.status != oracle.status {
        return format!(
            "field=status local=0x{:02X} oracle=0x{:02X}",
            local.status, oracle.status
        );
    }

    if local.signature != oracle.signature {
        for index in 0..local.signature.len() {
            if local.signature[index] != oracle.signature[index] {
                return format!(
                    "field=signature index={} local=0x{:02X} oracle=0x{:02X}",
                    index, local.signature[index], oracle.signature[index]
                );
            }
        }
    }

    format!(
        "field=text {}",
        describe_text_difference(&local.text, &oracle.text)
    )
}

fn describe_text_difference(local: &str, oracle: &str) -> String {
    let local_bytes = local.as_bytes();
    let oracle_bytes = oracle.as_bytes();
    let shared_len = local_bytes.len().min(oracle_bytes.len());
    for index in 0..shared_len {
        if local_bytes[index] != oracle_bytes[index] {
            let (line, column) = line_and_column_for_prefix(&local_bytes[..index]);
            return format!(
                "first_difference_byte={} line={} column={} local={} oracle={}",
                index,
                line,
                column,
                format_difference_byte(Some(local_bytes[index])),
                format_difference_byte(Some(oracle_bytes[index])),
            );
        }
    }

    if local_bytes.len() != oracle_bytes.len() {
        let index = shared_len;
        let (line, column) = line_and_column_for_prefix(&local_bytes[..shared_len]);
        return format!(
            "first_difference_byte={} line={} column={} local={} oracle={}",
            index,
            line,
            column,
            format_difference_byte(local_bytes.get(index).copied()),
            format_difference_byte(oracle_bytes.get(index).copied()),
        );
    }

    "content differs but no differing byte was localized".to_string()
}

fn line_and_column_for_prefix(prefix: &[u8]) -> (usize, usize) {
    let mut line = 1_usize;
    let mut column = 1_usize;
    for byte in prefix {
        if *byte == b'\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn format_difference_byte(byte: Option<u8>) -> String {
    match byte {
        Some(value) if value.is_ascii_graphic() || value == b' ' => {
            format!("0x{value:02X}({:?})", char::from(value))
        }
        Some(value) => format!("0x{value:02X}"),
        None => "eof".to_string(),
    }
}

fn write_captured_artifact(
    root: &Path,
    capture: CaptureKind,
    artifacts: &CapturedArtifacts,
) -> Result<Option<PathBuf>, DifferentialExecutionError> {
    let path = root.join(artifact_file_name(capture));
    match capture {
        CaptureKind::Serial => {
            let Some(serial) = &artifacts.serial else {
                return Ok(None);
            };
            fs::write(&path, serial).map_err(|source| {
                DifferentialExecutionError::WriteArtifact {
                    path: path.clone(),
                    operation: "write serial artifact",
                    source,
                }
            })?;
        }
        CaptureKind::SerialHex => {
            let Some(serial_hex) = &artifacts.serial_hex else {
                return Ok(None);
            };
            fs::write(&path, serial_hex).map_err(|source| {
                DifferentialExecutionError::WriteArtifact {
                    path: path.clone(),
                    operation: "write serial hex artifact",
                    source,
                }
            })?;
        }
        CaptureKind::MemoryTextOutput => {
            let Some(memory_text_output) = &artifacts.memory_text_output else {
                return Ok(None);
            };
            fs::write(&path, render_memory_text_output(memory_text_output)).map_err(|source| {
                DifferentialExecutionError::WriteArtifact {
                    path: path.clone(),
                    operation: "write memory text output artifact",
                    source,
                }
            })?;
        }
        CaptureKind::BlarggConsoleText => {
            let Some(blargg_console_text) = &artifacts.blargg_console_text else {
                return Ok(None);
            };
            fs::write(&path, blargg_console_text).map_err(|source| {
                DifferentialExecutionError::WriteArtifact {
                    path: path.clone(),
                    operation: "write blargg console artifact",
                    source,
                }
            })?;
        }
        CaptureKind::Framebuffer => {
            let Some(framebuffer_pgm) = &artifacts.framebuffer_pgm else {
                return Ok(None);
            };
            let png = convert_pgm_to_png(framebuffer_pgm).map_err(|error| {
                DifferentialExecutionError::ParseOracleArtifact {
                    path: error.path,
                    message: error.message,
                }
            })?;
            fs::write(&path, png).map_err(|source| DifferentialExecutionError::WriteArtifact {
                path: path.clone(),
                operation: "write framebuffer artifact",
                source,
            })?;
            let legacy_pgm_path = root.join("framebuffer.pgm");
            fs::write(&legacy_pgm_path, framebuffer_pgm).map_err(|source| {
                DifferentialExecutionError::WriteArtifact {
                    path: legacy_pgm_path.clone(),
                    operation: "write legacy framebuffer artifact",
                    source,
                }
            })?;
        }
        CaptureKind::Trace => {
            let Some(trace) = &artifacts.trace else {
                return Ok(None);
            };
            fs::write(&path, trace).map_err(|source| {
                DifferentialExecutionError::WriteArtifact {
                    path: path.clone(),
                    operation: "write trace artifact",
                    source,
                }
            })?;
        }
        CaptureKind::Snapshot => {
            let Some(snapshot_text) = &artifacts.snapshot_text else {
                return Ok(None);
            };
            fs::write(&path, snapshot_text).map_err(|source| {
                DifferentialExecutionError::WriteArtifact {
                    path: path.clone(),
                    operation: "write snapshot artifact",
                    source,
                }
            })?;
        }
    }

    Ok(Some(path))
}

fn parse_memory_text_output_artifact(
    path: &Path,
) -> Result<CapturedMemoryTextOutput, DifferentialExecutionError> {
    let contents = fs::read_to_string(path).map_err(|source| {
        DifferentialExecutionError::ReadOracleArtifact {
            path: path.to_path_buf(),
            operation: "read memory text output oracle artifact",
            source,
        }
    })?;
    let mut lines = contents.lines();

    let status_line =
        lines
            .next()
            .ok_or_else(|| DifferentialExecutionError::ParseOracleArtifact {
                path: path.to_path_buf(),
                message: "memory text output artifact is missing status line".to_string(),
            })?;
    let signature_line =
        lines
            .next()
            .ok_or_else(|| DifferentialExecutionError::ParseOracleArtifact {
                path: path.to_path_buf(),
                message: "memory text output artifact is missing signature line".to_string(),
            })?;
    let text_line =
        lines
            .next()
            .ok_or_else(|| DifferentialExecutionError::ParseOracleArtifact {
                path: path.to_path_buf(),
                message: "memory text output artifact is missing text line".to_string(),
            })?;

    let status = parse_hex_byte_line(path, status_line, "status=0x")?;
    let signature = parse_signature_line(path, signature_line)?;
    let text_repr = text_line.strip_prefix("text=").ok_or_else(|| {
        DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "memory text output artifact has invalid text line".to_string(),
        }
    })?;
    let text = parse_toml_string(path, text_repr)?;

    Ok(CapturedMemoryTextOutput {
        status,
        signature,
        text,
    })
}

fn decode_sameboy_tester_framebuffer(
    path: &Path,
) -> Result<NormalizedFramebuffer, DifferentialExecutionError> {
    let bytes =
        fs::read(path).map_err(|source| DifferentialExecutionError::ReadOracleArtifact {
            path: path.to_path_buf(),
            operation: "read SameBoy tester framebuffer artifact",
            source,
        })?;

    match path.extension().and_then(|extension| extension.to_str()) {
        Some("bmp") => {
            let (width, height, pixels) = parse_sameboy_tester_bmp(path, &bytes)?;
            Ok(normalize_rgb_pixels(width, height, &pixels))
        }
        Some("tga") => {
            let (width, height, pixels) = parse_sameboy_tester_tga(path, &bytes)?;
            Ok(normalize_rgb_pixels(width, height, &pixels))
        }
        _ => Err(DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "unsupported SameBoy tester framebuffer extension".to_string(),
        }),
    }
}

fn parse_sameboy_tester_bmp(
    path: &Path,
    bytes: &[u8],
) -> Result<(usize, usize, Vec<[u8; 3]>), DifferentialExecutionError> {
    if bytes.len() < 54 || &bytes[0..2] != b"BM" {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "invalid BMP header".to_string(),
        });
    }

    let data_offset = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
    let width = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
    let height = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);
    let bits_per_pixel = u16::from_le_bytes([bytes[28], bytes[29]]);
    if width <= 0 || height == 0 || bits_per_pixel != 32 {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "unsupported SameBoy tester BMP format".to_string(),
        });
    }
    let top_down = height < 0;
    let width = width as usize;
    let height_abs = height.unsigned_abs() as usize;
    let stride =
        width
            .checked_mul(4)
            .ok_or_else(|| DifferentialExecutionError::ParseOracleArtifact {
                path: path.to_path_buf(),
                message: "BMP row stride overflow".to_string(),
            })?;
    let payload_len = stride.checked_mul(height_abs).ok_or_else(|| {
        DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "BMP payload length overflow".to_string(),
        }
    })?;
    if bytes.len() < data_offset + payload_len {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "BMP payload is shorter than declared dimensions".to_string(),
        });
    }

    let mut pixels = Vec::with_capacity(width * height_abs);
    for row in 0..height_abs {
        let source_row = if top_down { row } else { height_abs - 1 - row };
        let row_offset = data_offset + source_row * stride;
        for column in 0..width {
            let pixel_offset = row_offset + column * 4;
            let blue = bytes[pixel_offset];
            let green = bytes[pixel_offset + 1];
            let red = bytes[pixel_offset + 2];
            pixels.push([red, green, blue]);
        }
    }

    Ok((width, height_abs, pixels))
}

fn parse_sameboy_tester_tga(
    path: &Path,
    bytes: &[u8],
) -> Result<(usize, usize, Vec<[u8; 3]>), DifferentialExecutionError> {
    if bytes.len() < 18 || bytes[2] != 2 || bytes[16] != 32 {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "unsupported SameBoy tester TGA format".to_string(),
        });
    }

    let width = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
    let height = u16::from_le_bytes([bytes[14], bytes[15]]) as usize;
    let top_left_origin = (bytes[17] & 0x20) != 0;
    let data_offset = 18_usize;
    let stride =
        width
            .checked_mul(4)
            .ok_or_else(|| DifferentialExecutionError::ParseOracleArtifact {
                path: path.to_path_buf(),
                message: "TGA row stride overflow".to_string(),
            })?;
    let payload_len = stride.checked_mul(height).ok_or_else(|| {
        DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "TGA payload length overflow".to_string(),
        }
    })?;
    if bytes.len() < data_offset + payload_len {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "TGA payload is shorter than declared dimensions".to_string(),
        });
    }

    let mut pixels = Vec::with_capacity(width * height);
    for row in 0..height {
        let source_row = if top_left_origin {
            row
        } else {
            height - 1 - row
        };
        let row_offset = data_offset + source_row * stride;
        for column in 0..width {
            let pixel_offset = row_offset + column * 4;
            let blue = bytes[pixel_offset];
            let green = bytes[pixel_offset + 1];
            let red = bytes[pixel_offset + 2];
            pixels.push([red, green, blue]);
        }
    }

    Ok((width, height, pixels))
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

fn first_framebuffer_difference(
    local: &NormalizedFramebuffer,
    oracle: &NormalizedFramebuffer,
) -> Option<FramebufferDifferencePoint> {
    if local.width != oracle.width || local.height != oracle.height {
        return None;
    }

    local
        .palette_ranks
        .iter()
        .zip(&oracle.palette_ranks)
        .enumerate()
        .find_map(|(index, (local_rank, oracle_rank))| {
            if local_rank == oracle_rank {
                return None;
            }

            Some(FramebufferDifferencePoint {
                x: index % local.width,
                y: index / local.width,
                local_rank: *local_rank,
                oracle_rank: *oracle_rank,
            })
        })
}

fn describe_framebuffer_difference(
    local_width: usize,
    local_height: usize,
    oracle_width: usize,
    oracle_height: usize,
    first_difference: Option<&FramebufferDifferencePoint>,
) -> String {
    if local_width != oracle_width || local_height != oracle_height {
        return format!(
            "dimensions local={}x{} oracle={}x{}",
            local_width, local_height, oracle_width, oracle_height
        );
    }

    if let Some(first_difference) = first_difference {
        return format!(
            "first_difference_pixel=x={},y={} local_rank={} oracle_rank={}",
            first_difference.x,
            first_difference.y,
            first_difference.local_rank,
            first_difference.oracle_rank,
        );
    }

    "dimensions match but no differing pixel was localized".to_string()
}

fn luminance(color: &[u8; 3]) -> u32 {
    u32::from(color[0]) * 299 + u32::from(color[1]) * 587 + u32::from(color[2]) * 114
}

fn parse_hex_byte_line(
    path: &Path,
    line: &str,
    prefix: &str,
) -> Result<u8, DifferentialExecutionError> {
    let value = line.strip_prefix(prefix).ok_or_else(|| {
        DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: format!("invalid line prefix for {line:?}"),
        }
    })?;
    u8::from_str_radix(value, 16).map_err(|error| DifferentialExecutionError::ParseOracleArtifact {
        path: path.to_path_buf(),
        message: format!("failed to parse hex byte {value:?}: {error}"),
    })
}

fn parse_signature_line(path: &Path, line: &str) -> Result<[u8; 3], DifferentialExecutionError> {
    let value = line.strip_prefix("signature=").ok_or_else(|| {
        DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: "memory text output artifact has invalid signature line".to_string(),
        }
    })?;
    let mut bytes = [0_u8; 3];
    let parts: Vec<_> = value.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: format!("expected 3 signature bytes, got {}", parts.len()),
        });
    }
    for (index, part) in parts.iter().enumerate() {
        bytes[index] = u8::from_str_radix(part, 16).map_err(|error| {
            DifferentialExecutionError::ParseOracleArtifact {
                path: path.to_path_buf(),
                message: format!("failed to parse signature byte {part:?}: {error}"),
            }
        })?;
    }
    Ok(bytes)
}

fn parse_toml_string(path: &Path, repr: &str) -> Result<String, DifferentialExecutionError> {
    #[derive(Debug, Deserialize)]
    struct Wrapper {
        value: String,
    }

    toml::from_str::<Wrapper>(&format!("value = {repr}\n"))
        .map(|wrapper| wrapper.value)
        .map_err(|error| DifferentialExecutionError::ParseOracleArtifact {
            path: path.to_path_buf(),
            message: format!("failed to parse quoted text payload: {error}"),
        })
}

fn render_differential_summary(
    oracle: DifferentialOracle,
    oracle_layout: DifferentialOracleLayout,
    capture: CaptureKind,
    local_report: &RomCaseReport,
    outcome: &DifferentialCaseOutcome,
    oracle_artifact_path: &Path,
) -> String {
    let mut summary = format!(
        "oracle={}\noracle_layout={}\nchannel={}\nlocal_outcome={:?}\noracle_artifact={}\n",
        oracle.name(),
        oracle_layout.name(),
        capture_name(capture),
        local_report.outcome,
        oracle_artifact_path.display(),
    );
    match outcome {
        DifferentialCaseOutcome::Matched => {
            summary.push_str("differential_outcome=matched\n");
        }
        DifferentialCaseOutcome::Diverged(mismatch) => {
            summary.push_str("differential_outcome=diverged\n");
            summary.push_str(&format!("mismatch={}\n", mismatch.name()));
            summary.push_str(&format!("mismatch_detail={}\n", mismatch.detail()));
        }
    }
    summary
}

fn capture_name(capture: CaptureKind) -> &'static str {
    match capture {
        CaptureKind::Serial => "serial",
        CaptureKind::SerialHex => "serial-hex",
        CaptureKind::MemoryTextOutput => "memory-text-output",
        CaptureKind::BlarggConsoleText => "blargg-console-text",
        CaptureKind::Framebuffer => "framebuffer",
        CaptureKind::Trace => "trace",
        CaptureKind::Snapshot => "snapshot",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DifferentialCaseMismatch, DifferentialCaseOutcome, DifferentialOracle,
        DifferentialOracleLayout, DifferentialRunner, FramebufferDifferencePoint,
        NormalizedFramebuffer, decode_local_pgm_framebuffer, decode_sameboy_tester_framebuffer,
        describe_framebuffer_difference, first_framebuffer_difference,
        parse_memory_text_output_artifact, parse_sameboy_tester_bmp, parse_sameboy_tester_tga,
        render_differential_summary, write_captured_artifact,
    };
    use crate::{
        CaptureKind, CapturedArtifacts, CapturedMemoryTextOutput, PassCondition, RomCaseOutcome,
        RomCaseReport, RomTestCase, Timeout, acid_dmg_curated_suite,
        framebuffer_oracle::{decode_fixture_framebuffer_path, encode_framebuffer_pgm},
        phase_2_cpu_timing_suite, render_memory_text_output,
    };
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-differential-src-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn build_sameboy_tester_bmp_from_pgm(pgm: &[u8]) -> Vec<u8> {
        let NormalizedFramebuffer {
            width,
            height,
            palette_ranks,
        } = decode_local_pgm_framebuffer("fixture", pgm).expect("PGM fixture should decode");
        let palette = [
            [0xE0_u8, 0xF8_u8, 0xD0_u8],
            [0x88_u8, 0xC0_u8, 0x70_u8],
            [0x34_u8, 0x68_u8, 0x56_u8],
            [0x08_u8, 0x18_u8, 0x20_u8],
        ];

        let mut bmp = vec![0_u8; 70];
        bmp[0..2].copy_from_slice(b"BM");
        let payload_len = width * height * 4;
        let file_size = 70 + payload_len;
        bmp[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        bmp[10..14].copy_from_slice(&(70_u32).to_le_bytes());
        bmp[14..18].copy_from_slice(&(56_u32).to_le_bytes());
        bmp[18..22].copy_from_slice(&(width as u32).to_le_bytes());
        bmp[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
        bmp[26..28].copy_from_slice(&(1_u16).to_le_bytes());
        bmp[28..30].copy_from_slice(&(32_u16).to_le_bytes());
        bmp[30..34].copy_from_slice(&(3_u32).to_le_bytes());
        bmp[34..38].copy_from_slice(&((payload_len + 2) as u32).to_le_bytes());

        let mut pixels = Vec::with_capacity(payload_len);
        for rank in palette_ranks {
            let color = palette[usize::from(rank)];
            pixels.extend_from_slice(&[color[2], color[1], color[0], 0]);
        }
        bmp.extend_from_slice(&pixels);
        bmp
    }

    fn build_sameboy_tester_tga_from_pgm(pgm: &[u8]) -> Vec<u8> {
        let NormalizedFramebuffer {
            width,
            height,
            palette_ranks,
        } = decode_local_pgm_framebuffer("fixture", pgm).expect("PGM fixture should decode");
        let palette = [
            [0xE0_u8, 0xF8_u8, 0xD0_u8],
            [0x88_u8, 0xC0_u8, 0x70_u8],
            [0x34_u8, 0x68_u8, 0x56_u8],
            [0x08_u8, 0x18_u8, 0x20_u8],
        ];

        let mut tga = vec![0_u8; 18];
        tga[2] = 2;
        tga[12..14].copy_from_slice(&(width as u16).to_le_bytes());
        tga[14..16].copy_from_slice(&(height as u16).to_le_bytes());
        tga[16] = 32;
        tga[17] = 0x20;
        for rank in palette_ranks {
            let color = palette[usize::from(rank)];
            tga.extend_from_slice(&[color[2], color[1], color[0], 0]);
        }
        tga
    }

    fn sample_case_report(case_id: &str, artifacts: CapturedArtifacts) -> RomCaseReport {
        RomCaseReport {
            case_id: case_id.to_string(),
            rom_path: PathBuf::from(format!("synthetic/{case_id}.gb")),
            outcome: RomCaseOutcome::Passed,
            executed_t_cycles: 123,
            completed_frames: 4,
            diagnostics: Vec::new(),
            artifacts,
            retained_failure_artifacts: Vec::new(),
        }
    }

    fn sample_memory_text_output() -> CapturedMemoryTextOutput {
        CapturedMemoryTextOutput {
            status: 0x80,
            signature: [0xDE, 0xB0, 0x61],
            text: "done".to_string(),
        }
    }

    fn load_fixture_as_local_pgm(relative_path: &str) -> Vec<u8> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
        let normalized = decode_fixture_framebuffer_path(&path).expect("fixture should decode");
        encode_framebuffer_pgm(&normalized.palette_ranks)
    }

    #[test]
    fn sameboy_tester_bmp_decoder_matches_local_pgm_palette_ranks() {
        let pgm = load_fixture_as_local_pgm("data/fixtures/acid/dmg-acid2-dmg.png");
        let expected = decode_local_pgm_framebuffer("acid2", &pgm).expect("PGM should decode");

        let temp_dir = unique_temp_dir("bmp-decode");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let bmp_path = temp_dir.join("dmg-acid2.bmp");
        fs::write(&bmp_path, build_sameboy_tester_bmp_from_pgm(&pgm))
            .expect("BMP should be writable");

        let actual =
            decode_sameboy_tester_framebuffer(&bmp_path).expect("SameBoy tester BMP should decode");
        assert_eq!(actual, expected);
    }

    #[test]
    fn sameboy_tester_layout_resolves_framebuffer_from_rom_path() {
        let suite = acid_dmg_curated_suite();
        let case = suite
            .cases
            .iter()
            .find(|case| case.id == "dmg-acid2")
            .expect("acid suite should include dmg-acid2");
        let runner = DifferentialRunner::new(DifferentialOracle::SameBoy, "/tmp/oracle")
            .with_oracle_layout(DifferentialOracleLayout::SameBoyTester);

        let path = runner
            .resolve_oracle_artifact_path(case, CaptureKind::Framebuffer)
            .expect("framebuffer path should resolve");

        assert!(path.ends_with("acid/dmg-acid2.bmp"));
    }

    #[test]
    fn sameboy_tester_layout_rejects_non_framebuffer_cases() {
        let suite = phase_2_cpu_timing_suite();
        let case = &suite.cases[0];
        let runner = DifferentialRunner::new(DifferentialOracle::SameBoy, "/tmp/oracle")
            .with_oracle_layout(DifferentialOracleLayout::SameBoyTester);

        let error = runner
            .resolve_oracle_artifact_path(case, CaptureKind::Trace)
            .expect_err("non-framebuffer trace case should be rejected");

        assert!(matches!(
            error,
            super::DifferentialExecutionError::UnsupportedOracleLayoutForCapture { .. }
        ));
    }

    #[test]
    fn render_summary_includes_oracle_layout() {
        let summary = render_differential_summary(
            DifferentialOracle::SameBoy,
            DifferentialOracleLayout::SameBoyTester,
            CaptureKind::Framebuffer,
            &RomCaseReport {
                case_id: "case".to_string(),
                rom_path: PathBuf::from("synthetic/case.gb"),
                outcome: RomCaseOutcome::Passed,
                executed_t_cycles: 0,
                completed_frames: 0,
                diagnostics: Vec::new(),
                artifacts: CapturedArtifacts::default(),
                retained_failure_artifacts: Vec::new(),
            },
            &DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::FramebufferMismatch {
                oracle_artifact_path: PathBuf::from("/tmp/framebuffer.bmp"),
                local_width: 160,
                local_height: 144,
                oracle_width: 160,
                oracle_height: 144,
                first_difference: Some(FramebufferDifferencePoint {
                    x: 12,
                    y: 34,
                    local_rank: 1,
                    oracle_rank: 2,
                }),
            }),
            PathBuf::from("/tmp/framebuffer.bmp").as_path(),
        );

        assert!(summary.contains("oracle_layout=sameboy-tester"));
        assert!(summary.contains("mismatch_detail=first_difference_pixel=x=12,y=34"));
    }

    #[test]
    fn trace_mismatch_detail_reports_first_difference_location() {
        let mismatch = DifferentialCaseMismatch::TraceMismatch {
            oracle_artifact_path: PathBuf::from("/tmp/oracle.trace"),
            oracle: "aa\nbd\n".to_string(),
            local: "aa\nbc\n".to_string(),
        };

        let detail = mismatch.detail();
        assert!(detail.contains("first_difference_byte=4"));
        assert!(detail.contains("line=2"));
        assert!(detail.contains("column=2"));
    }

    #[test]
    fn parse_pgm_rejects_bad_magic() {
        let error = decode_local_pgm_framebuffer("case", b"P2\n1 1\n255\n\x00")
            .expect_err("bad magic should fail");
        assert!(error.message.contains("unsupported PGM magic"));
    }

    #[test]
    fn parse_sameboy_tester_bmp_rejects_short_data() {
        let error = parse_sameboy_tester_bmp(PathBuf::from("/tmp/x.bmp").as_path(), b"BM")
            .expect_err("short BMP should fail");
        assert!(matches!(
            error,
            super::DifferentialExecutionError::ParseOracleArtifact { .. }
        ));
    }

    #[test]
    fn compare_required_capture_covers_textual_channels_and_missing_oracle_paths() {
        let temp_dir = unique_temp_dir("text-channels");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let runner = DifferentialRunner::new(DifferentialOracle::SameBoy, &temp_dir);

        let missing = runner
            .compare_required_capture(
                &sample_case_report(
                    "serial",
                    CapturedArtifacts {
                        serial: Some("ok".to_string()),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::Serial,
                &temp_dir.join("missing.txt"),
            )
            .expect("missing oracle should still produce a mismatch outcome");
        assert!(matches!(
            missing,
            DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::MissingOracleArtifact {
                capture: CaptureKind::Serial,
                ..
            })
        ));

        let serial_path = temp_dir.join("serial.txt");
        fs::write(&serial_path, "oracle").expect("serial oracle should be writable");
        let serial = runner
            .compare_required_capture(
                &sample_case_report(
                    "serial",
                    CapturedArtifacts {
                        serial: Some("local".to_string()),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::Serial,
                &serial_path,
            )
            .expect("serial comparison should succeed");
        assert!(matches!(
            serial,
            DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::SerialMismatch { .. })
        ));

        let blargg_path = temp_dir.join("console.txt");
        fs::write(&blargg_path, "same").expect("blargg oracle should be writable");
        let blargg = runner
            .compare_required_capture(
                &sample_case_report(
                    "console",
                    CapturedArtifacts {
                        blargg_console_text: Some("same".to_string()),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::BlarggConsoleText,
                &blargg_path,
            )
            .expect("blargg comparison should succeed");
        assert_eq!(blargg, DifferentialCaseOutcome::Matched);

        let trace_path = temp_dir.join("trace.txt");
        fs::write(&trace_path, "aa\nbd\n").expect("trace oracle should be writable");
        let trace = runner
            .compare_required_capture(
                &sample_case_report(
                    "trace",
                    CapturedArtifacts {
                        trace: Some("aa\nbc\n".to_string()),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::Trace,
                &trace_path,
            )
            .expect("trace comparison should succeed");
        assert!(matches!(
            trace,
            DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::TraceMismatch { .. })
        ));

        let snapshot_path = temp_dir.join("snapshot.txt");
        fs::write(&snapshot_path, "pc=0100").expect("snapshot oracle should be writable");
        let snapshot = runner
            .compare_required_capture(
                &sample_case_report(
                    "snapshot",
                    CapturedArtifacts {
                        snapshot_text: Some("pc=0100".to_string()),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::Snapshot,
                &snapshot_path,
            )
            .expect("snapshot comparison should succeed");
        assert_eq!(snapshot, DifferentialCaseOutcome::Matched);
    }

    #[test]
    fn compare_required_capture_covers_memory_text_output_parsing_and_missing_local_artifacts() {
        let temp_dir = unique_temp_dir("memory-text");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let runner = DifferentialRunner::new(DifferentialOracle::SameBoy, &temp_dir);
        let oracle_path = temp_dir.join("memory.txt");
        let oracle_value = sample_memory_text_output();
        fs::write(&oracle_path, render_memory_text_output(&oracle_value))
            .expect("memory text oracle should be writable");

        let parsed =
            parse_memory_text_output_artifact(&oracle_path).expect("oracle artifact should parse");
        assert_eq!(parsed, oracle_value);

        let matched = runner
            .compare_required_capture(
                &sample_case_report(
                    "memory",
                    CapturedArtifacts {
                        memory_text_output: Some(oracle_value.clone()),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::MemoryTextOutput,
                &oracle_path,
            )
            .expect("memory text comparison should succeed");
        assert_eq!(matched, DifferentialCaseOutcome::Matched);

        let mismatch = runner
            .compare_required_capture(
                &sample_case_report(
                    "memory",
                    CapturedArtifacts {
                        memory_text_output: Some(CapturedMemoryTextOutput {
                            text: "later".to_string(),
                            ..oracle_value.clone()
                        }),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::MemoryTextOutput,
                &oracle_path,
            )
            .expect("memory text mismatch should still succeed");
        assert!(matches!(
            mismatch,
            DifferentialCaseOutcome::Diverged(
                DifferentialCaseMismatch::MemoryTextOutputMismatch { .. }
            )
        ));

        let missing_local = runner
            .compare_required_capture(
                &sample_case_report("memory", CapturedArtifacts::default()),
                CaptureKind::MemoryTextOutput,
                &oracle_path,
            )
            .expect_err("missing local capture should fail");
        assert!(matches!(
            missing_local,
            super::DifferentialExecutionError::MissingLocalArtifact {
                capture: CaptureKind::MemoryTextOutput,
                ..
            }
        ));
    }

    #[test]
    fn compare_required_capture_covers_framebuffer_case_bundle_and_sameboy_tester_tga() {
        let temp_dir = unique_temp_dir("framebuffer");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let pgm = load_fixture_as_local_pgm("data/fixtures/acid/dmg-acid2-dmg.png");

        let case_bundle_runner = DifferentialRunner::new(DifferentialOracle::SameBoy, &temp_dir);
        let case_bundle_path = temp_dir.join("acid2.png");
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/fixtures/acid/dmg-acid2-dmg.png"),
            &case_bundle_path,
        )
        .expect("PNG oracle should be writable");

        let matched = case_bundle_runner
            .compare_required_capture(
                &sample_case_report(
                    "framebuffer",
                    CapturedArtifacts {
                        framebuffer_pgm: Some(pgm.clone()),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::Framebuffer,
                &case_bundle_path,
            )
            .expect("framebuffer comparison should succeed");
        assert_eq!(matched, DifferentialCaseOutcome::Matched);

        let mut mismatched_pgm = pgm.clone();
        let last_index = mismatched_pgm.len() - 1;
        mismatched_pgm[last_index] ^= 0xFF;
        let mismatch = case_bundle_runner
            .compare_required_capture(
                &sample_case_report(
                    "framebuffer",
                    CapturedArtifacts {
                        framebuffer_pgm: Some(mismatched_pgm),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::Framebuffer,
                &case_bundle_path,
            )
            .expect("mismatched framebuffer comparison should succeed");
        assert!(matches!(
            mismatch,
            DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::FramebufferMismatch {
                first_difference: Some(_),
                ..
            })
        ));

        let sameboy_tga_path = temp_dir.join("acid2.tga");
        fs::write(&sameboy_tga_path, build_sameboy_tester_tga_from_pgm(&pgm))
            .expect("TGA oracle should be writable");
        let sameboy_runner = DifferentialRunner::new(DifferentialOracle::SameBoy, &temp_dir)
            .with_oracle_layout(DifferentialOracleLayout::SameBoyTester);
        let sameboy_outcome = sameboy_runner
            .compare_required_capture(
                &sample_case_report(
                    "framebuffer",
                    CapturedArtifacts {
                        framebuffer_pgm: Some(pgm),
                        ..CapturedArtifacts::default()
                    },
                ),
                CaptureKind::Framebuffer,
                &sameboy_tga_path,
            )
            .expect("sameboy TGA comparison should succeed");
        assert_eq!(sameboy_outcome, DifferentialCaseOutcome::Matched);
    }

    #[test]
    fn persist_context_if_needed_writes_summary_local_and_oracle_artifacts() {
        let temp_dir = unique_temp_dir("persist-context");
        let failure_root = temp_dir.join("failures");
        let oracle_root = temp_dir.join("oracle");
        fs::create_dir_all(&oracle_root).expect("oracle root should be creatable");

        let case = RomTestCase::new(
            "phase2-fetch-immediate-order",
            "crates/gb-core/tests/fixtures/roms/phase2/phase2_fetch_immediate_order.gb",
            Timeout::TCycles(32),
            PassCondition::TraceFixture(PathBuf::from("unused")),
        );
        let oracle_artifact = oracle_root.join(&case.id).join("trace.txt");
        fs::create_dir_all(
            oracle_artifact
                .parent()
                .expect("oracle artifact should have a parent"),
        )
        .expect("oracle case dir should be creatable");
        fs::write(&oracle_artifact, "oracle-trace").expect("oracle artifact should be writable");

        let local_report = sample_case_report(
            &case.id,
            CapturedArtifacts {
                trace: Some("local-trace".to_string()),
                snapshot_text: Some("pc=0100".to_string()),
                ..CapturedArtifacts::default()
            },
        );
        let outcome = DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::TraceMismatch {
            oracle_artifact_path: oracle_artifact.clone(),
            oracle: "oracle-trace".to_string(),
            local: "local-trace".to_string(),
        });
        let runner = DifferentialRunner::new(DifferentialOracle::SameBoy, &oracle_root)
            .with_failure_artifact_root(&failure_root);

        let archived = runner
            .persist_context_if_needed(&case, &local_report, &outcome, &oracle_artifact)
            .expect("context persistence should succeed");

        assert!(
            archived
                .iter()
                .any(|path| path.ends_with("differential_summary.txt"))
        );
        assert!(
            archived
                .iter()
                .any(|path| path.ends_with("local/trace.txt"))
        );
        assert!(
            archived
                .iter()
                .any(|path| path.ends_with("local/snapshot.txt"))
        );
        assert!(
            archived
                .iter()
                .any(|path| path.ends_with("oracle/trace.txt"))
        );
    }

    #[test]
    fn write_captured_artifact_covers_all_channels_and_missing_payloads() {
        let temp_dir = unique_temp_dir("write-artifacts");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let memory = sample_memory_text_output();
        let artifacts = CapturedArtifacts {
            serial: Some("serial".to_string()),
            serial_hex: Some("73657269616C".to_string()),
            memory_text_output: Some(memory.clone()),
            blargg_console_text: Some("console".to_string()),
            framebuffer_pgm: Some(vec![
                b'P', b'5', b'\n', b'1', b' ', b'1', b'\n', b'2', b'5', b'5', b'\n', 0,
            ]),
            trace: Some("trace".to_string()),
            snapshot_text: Some("snapshot".to_string()),
        };

        for capture in [
            CaptureKind::Serial,
            CaptureKind::MemoryTextOutput,
            CaptureKind::BlarggConsoleText,
            CaptureKind::Framebuffer,
            CaptureKind::Trace,
            CaptureKind::Snapshot,
        ] {
            let path = write_captured_artifact(&temp_dir, capture, &artifacts)
                .expect("artifact write should succeed")
                .expect("artifact should be present");
            assert!(
                path.is_file(),
                "expected written artifact {}",
                path.display()
            );
        }

        let missing = write_captured_artifact(
            &temp_dir,
            CaptureKind::Serial,
            &CapturedArtifacts::default(),
        )
        .expect("missing capture should not fail");
        assert!(missing.is_none());
        assert_eq!(
            fs::read_to_string(temp_dir.join("memory_text_output.txt"))
                .expect("memory text output artifact should be readable"),
            render_memory_text_output(&memory)
        );
    }

    #[test]
    fn framebuffer_and_memory_parsers_report_useful_errors() {
        let temp_dir = unique_temp_dir("parser-errors");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

        let bad_memory_path = temp_dir.join("bad-memory.txt");
        fs::write(
            &bad_memory_path,
            "status=0xGG\nsignature=DE B0\ntext=nope\n",
        )
        .expect("bad memory oracle should be writable");
        let memory_error = parse_memory_text_output_artifact(&bad_memory_path)
            .expect_err("invalid memory text output should fail");
        assert!(matches!(
            memory_error,
            super::DifferentialExecutionError::ParseOracleArtifact { .. }
        ));

        let bad_pgm = b"P5\nx 1\n255\n\x00";
        let pgm_error =
            decode_local_pgm_framebuffer("case", bad_pgm).expect_err("invalid width should fail");
        assert!(pgm_error.message.contains("width"));

        let short_pgm = b"P5\n2 2\n255\n\x00";
        let short_error =
            decode_local_pgm_framebuffer("case", short_pgm).expect_err("short payload should fail");
        assert!(
            short_error
                .message
                .contains("shorter than declared dimensions")
        );

        let tga_path = temp_dir.join("bad.tga");
        let tga_error =
            parse_sameboy_tester_tga(&tga_path, b"not-a-tga").expect_err("bad tga should fail");
        assert!(matches!(
            tga_error,
            super::DifferentialExecutionError::ParseOracleArtifact { .. }
        ));

        let unsupported_path = temp_dir.join("framebuffer.bin");
        fs::write(&unsupported_path, b"x").expect("unsupported artifact should be writable");
        let unsupported_error = decode_sameboy_tester_framebuffer(&unsupported_path)
            .expect_err("unsupported extension should fail");
        assert!(matches!(
            unsupported_error,
            super::DifferentialExecutionError::ParseOracleArtifact { .. }
        ));
    }

    #[test]
    fn framebuffer_difference_helpers_cover_dimension_and_localized_paths() {
        let local = NormalizedFramebuffer {
            width: 2,
            height: 1,
            palette_ranks: vec![0, 1],
        };
        let oracle = NormalizedFramebuffer {
            width: 2,
            height: 1,
            palette_ranks: vec![0, 2],
        };
        let difference =
            first_framebuffer_difference(&local, &oracle).expect("difference should localize");
        assert_eq!(difference.x, 1);
        assert_eq!(difference.y, 0);

        assert_eq!(
            describe_framebuffer_difference(2, 1, 3, 1, None),
            "dimensions local=2x1 oracle=3x1"
        );
        assert_eq!(
            describe_framebuffer_difference(2, 1, 2, 1, Some(&difference)),
            "first_difference_pixel=x=1,y=0 local_rank=1 oracle_rank=2"
        );
        assert_eq!(
            describe_framebuffer_difference(2, 1, 2, 1, None),
            "dimensions match but no differing pixel was localized"
        );
    }
}
