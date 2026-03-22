use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gb_core::ExecutionMode;
use serde::Deserialize;

use crate::{
    CaptureKind, CapturedArtifacts, CapturedMemoryTextOutput, RomCaseReport, RomExecutionError,
    RomRunner, RomSuite, RomSuiteValidationError, TestSubsystem, artifact_file_name,
    render_memory_text_output,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferentialOracle {
    SameBoy,
    Gambatte,
}

impl DifferentialOracle {
    pub fn name(self) -> &'static str {
        match self {
            Self::SameBoy => "sameboy",
            Self::Gambatte => "gambatte",
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
    FramebufferMismatch {
        oracle_artifact_path: PathBuf,
    },
}

impl DifferentialCaseMismatch {
    pub fn name(&self) -> &'static str {
        match self {
            Self::MissingOracleArtifact { .. } => "missing-oracle-artifact",
            Self::SerialMismatch { .. } => "serial-mismatch",
            Self::MemoryTextOutputMismatch { .. } => "memory-text-output-mismatch",
            Self::BlarggConsoleTextMismatch { .. } => "blargg-console-text-mismatch",
            Self::TraceMismatch { .. } => "trace-mismatch",
            Self::FramebufferMismatch { .. } => "framebuffer-mismatch",
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
            DifferentialOracleLayout::CaseBundle => Ok(self
                .oracle_artifact_root
                .join(&case.id)
                .join(artifact_file_name(capture))),
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

                let matched = match self.oracle_layout {
                    DifferentialOracleLayout::CaseBundle => {
                        let oracle = fs::read(oracle_artifact_path).map_err(|source| {
                            DifferentialExecutionError::ReadOracleArtifact {
                                path: oracle_artifact_path.to_path_buf(),
                                operation: "read framebuffer oracle artifact",
                                source,
                            }
                        })?;
                        local == oracle.as_slice()
                    }
                    DifferentialOracleLayout::SameBoyTester => {
                        let local_normalized =
                            decode_local_pgm_framebuffer(local_report.case_id.as_str(), local)?;
                        let oracle_normalized =
                            decode_sameboy_tester_framebuffer(oracle_artifact_path)?;
                        local_normalized == oracle_normalized
                    }
                };

                if matched {
                    DifferentialCaseOutcome::Matched
                } else {
                    DifferentialCaseOutcome::Diverged(
                        DifferentialCaseMismatch::FramebufferMismatch {
                            oracle_artifact_path: oracle_artifact_path.to_path_buf(),
                        },
                    )
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
                    DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::TraceMismatch {
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
            fs::write(&path, framebuffer_pgm).map_err(|source| {
                DifferentialExecutionError::WriteArtifact {
                    path: path.clone(),
                    operation: "write framebuffer artifact",
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedFramebuffer {
    width: usize,
    height: usize,
    palette_ranks: Vec<u8>,
}

fn decode_local_pgm_framebuffer(
    case_id: &str,
    bytes: &[u8],
) -> Result<NormalizedFramebuffer, DifferentialExecutionError> {
    let (width, height, pixels) = parse_pgm(case_id, bytes)?;
    Ok(normalize_indexed_pixels(width, height, pixels))
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

fn parse_pgm<'a>(
    case_id: &str,
    bytes: &'a [u8],
) -> Result<(usize, usize, &'a [u8]), DifferentialExecutionError> {
    let mut index = 0_usize;
    let magic = next_pgm_token(bytes, &mut index, "magic", case_id)?;
    if magic != b"P5" {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: PathBuf::from(format!("<local framebuffer for {case_id}>")),
            message: format!("unsupported PGM magic {:?}", String::from_utf8_lossy(magic)),
        });
    }

    let width = parse_usize_token(
        next_pgm_token(bytes, &mut index, "width", case_id)?,
        case_id,
        "width",
    )?;
    let height = parse_usize_token(
        next_pgm_token(bytes, &mut index, "height", case_id)?,
        case_id,
        "height",
    )?;
    let max_value = parse_usize_token(
        next_pgm_token(bytes, &mut index, "max", case_id)?,
        case_id,
        "max",
    )?;
    if max_value != 255 {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: PathBuf::from(format!("<local framebuffer for {case_id}>")),
            message: format!("unsupported PGM max value {max_value}"),
        });
    }

    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    let expected_len = width.checked_mul(height).ok_or_else(|| {
        DifferentialExecutionError::ParseOracleArtifact {
            path: PathBuf::from(format!("<local framebuffer for {case_id}>")),
            message: "PGM dimensions overflow".to_string(),
        }
    })?;
    if bytes.len() < index + expected_len {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: PathBuf::from(format!("<local framebuffer for {case_id}>")),
            message: "PGM pixel payload is shorter than declared dimensions".to_string(),
        });
    }

    Ok((width, height, &bytes[index..index + expected_len]))
}

fn next_pgm_token<'a>(
    bytes: &'a [u8],
    index: &mut usize,
    label: &str,
    case_id: &str,
) -> Result<&'a [u8], DifferentialExecutionError> {
    while *index < bytes.len() && bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
    let start = *index;
    while *index < bytes.len() && !bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
    if start == *index {
        return Err(DifferentialExecutionError::ParseOracleArtifact {
            path: PathBuf::from(format!("<local framebuffer for {case_id}>")),
            message: format!("missing PGM {label} token"),
        });
    }
    Ok(&bytes[start..*index])
}

fn parse_usize_token(
    token: &[u8],
    case_id: &str,
    label: &str,
) -> Result<usize, DifferentialExecutionError> {
    std::str::from_utf8(token)
        .map_err(|error| DifferentialExecutionError::ParseOracleArtifact {
            path: PathBuf::from(format!("<local framebuffer for {case_id}>")),
            message: format!("invalid UTF-8 in PGM {label}: {error}"),
        })?
        .parse::<usize>()
        .map_err(|error| DifferentialExecutionError::ParseOracleArtifact {
            path: PathBuf::from(format!("<local framebuffer for {case_id}>")),
            message: format!("failed to parse PGM {label}: {error}"),
        })
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
        }
    }
    summary
}

fn capture_name(capture: CaptureKind) -> &'static str {
    match capture {
        CaptureKind::Serial => "serial",
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
        DifferentialOracleLayout, DifferentialRunner, NormalizedFramebuffer,
        decode_local_pgm_framebuffer, decode_sameboy_tester_framebuffer, parse_pgm,
        parse_sameboy_tester_bmp, render_differential_summary,
    };
    use crate::{
        CaptureKind, CapturedArtifacts, RomCaseOutcome, RomCaseReport, gbdev_dmg_acid2_suite,
        phase_2_cpu_timing_suite,
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

    #[test]
    fn sameboy_tester_bmp_decoder_matches_local_pgm_palette_ranks() {
        let pgm = fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/external/acid/dmg-acid2-dmg.pgm"),
        )
        .expect("fixture should be readable");
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
        let suite = gbdev_dmg_acid2_suite();
        let case = &suite.cases[0];
        let runner = DifferentialRunner::new(DifferentialOracle::SameBoy, "/tmp/oracle")
            .with_oracle_layout(DifferentialOracleLayout::SameBoyTester);

        let path = runner
            .resolve_oracle_artifact_path(case, CaptureKind::Framebuffer)
            .expect("framebuffer path should resolve");

        assert!(path.ends_with("testroms/acid/dmg-acid2.bmp"));
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
                outcome: RomCaseOutcome::Passed,
                executed_t_cycles: 0,
                completed_frames: 0,
                diagnostics: Vec::new(),
                artifacts: CapturedArtifacts::default(),
                retained_failure_artifacts: Vec::new(),
            },
            &DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::FramebufferMismatch {
                oracle_artifact_path: PathBuf::from("/tmp/framebuffer.bmp"),
            }),
            PathBuf::from("/tmp/framebuffer.bmp").as_path(),
        );

        assert!(summary.contains("oracle_layout=sameboy-tester"));
    }

    #[test]
    fn parse_pgm_rejects_bad_magic() {
        let error = parse_pgm("case", b"P2\n1 1\n255\n\x00").expect_err("bad magic should fail");
        assert!(matches!(
            error,
            super::DifferentialExecutionError::ParseOracleArtifact { .. }
        ));
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
}
