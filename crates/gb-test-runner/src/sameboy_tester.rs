use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use gb_core::{ConsoleModel, ExecutionMode, StartupMode};

use crate::{
    CaptureKind, RomExecutionError, RomRunner, RomSuite, RomSuiteValidationError, TestSubsystem,
    Timeout,
};

pub const SAMEBOY_ROOT_ENV_VAR: &str = "GB_CYCLE_SAMEBOY_ROOT";
pub const SAMEBOY_TESTER_BIN_ENV_VAR: &str = "GB_CYCLE_SAMEBOY_TESTER_BIN";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameBoyTesterImageFormat {
    Bmp,
    Tga,
}

impl SameBoyTesterImageFormat {
    pub fn name(self) -> &'static str {
        match self {
            Self::Bmp => "bmp",
            Self::Tga => "tga",
        }
    }

    fn file_extension(self) -> &'static str {
        self.name()
    }
}

#[derive(Debug)]
pub enum SameBoyTesterExecutionError {
    InvalidSuite(RomSuiteValidationError),
    NonStrictCase {
        case_id: String,
        actual: ExecutionMode,
    },
    UnsupportedCapture {
        case_id: String,
        capture: CaptureKind,
    },
    UnsupportedConsoleModel {
        case_id: String,
        console_model: ConsoleModel,
    },
    ResolveRomPath {
        case_id: String,
        source: Box<RomExecutionError>,
    },
    MissingSameBoyRoot,
    MissingTesterBinary {
        path: PathBuf,
    },
    BuildTesterFailed {
        root: PathBuf,
        status: Option<i32>,
    },
    SpawnTester {
        path: PathBuf,
        source: io::Error,
    },
    TesterFailed {
        path: PathBuf,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    CreateDirectory {
        path: PathBuf,
        source: io::Error,
    },
    CopyRom {
        source_path: PathBuf,
        destination_path: PathBuf,
        source: io::Error,
    },
    RemoveArtifact {
        path: PathBuf,
        source: io::Error,
    },
    MissingImageArtifact {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameBoyTesterCaseReport {
    pub case_id: String,
    pub staged_rom_path: PathBuf,
    pub image_artifact_path: PathBuf,
    pub log_artifact_path: Option<PathBuf>,
    pub startup_mode_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameBoyTesterSuiteReport {
    pub suite_name: String,
    pub subsystem: TestSubsystem,
    pub tester_binary: PathBuf,
    pub oracle_root: PathBuf,
    pub image_format: SameBoyTesterImageFormat,
    pub cases: Vec<SameBoyTesterCaseReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameBoyTesterRunner {
    rom_runner: RomRunner,
    oracle_root: PathBuf,
    tester_binary: Option<PathBuf>,
    sameboy_root: Option<PathBuf>,
    image_format: SameBoyTesterImageFormat,
    build_if_missing: bool,
}

impl SameBoyTesterRunner {
    pub fn new(oracle_root: impl Into<PathBuf>) -> Self {
        Self {
            rom_runner: RomRunner::new(),
            oracle_root: oracle_root.into(),
            tester_binary: None,
            sameboy_root: std::env::var_os(SAMEBOY_ROOT_ENV_VAR).map(PathBuf::from),
            image_format: SameBoyTesterImageFormat::Bmp,
            build_if_missing: false,
        }
    }

    pub fn with_rom_runner(mut self, rom_runner: RomRunner) -> Self {
        self.rom_runner = rom_runner;
        self
    }

    pub fn with_tester_binary(mut self, tester_binary: impl Into<PathBuf>) -> Self {
        self.tester_binary = Some(tester_binary.into());
        self
    }

    pub fn with_sameboy_root(mut self, sameboy_root: impl Into<PathBuf>) -> Self {
        self.sameboy_root = Some(sameboy_root.into());
        self
    }

    pub fn with_image_format(mut self, image_format: SameBoyTesterImageFormat) -> Self {
        self.image_format = image_format;
        self
    }

    pub fn with_build_if_missing(mut self, build_if_missing: bool) -> Self {
        self.build_if_missing = build_if_missing;
        self
    }

    pub fn run_suite(
        &self,
        suite: &RomSuite,
    ) -> Result<SameBoyTesterSuiteReport, SameBoyTesterExecutionError> {
        suite
            .validate()
            .map_err(SameBoyTesterExecutionError::InvalidSuite)?;
        let tester_binary = self.ensure_tester_binary()?;

        let mut cases = Vec::with_capacity(suite.cases.len());
        for case in &suite.cases {
            cases.push(self.run_case(case, &tester_binary)?);
        }

        Ok(SameBoyTesterSuiteReport {
            suite_name: suite.name.clone(),
            subsystem: suite.subsystem,
            tester_binary,
            oracle_root: self.oracle_root.clone(),
            image_format: self.image_format,
            cases,
        })
    }

    fn run_case(
        &self,
        case: &crate::RomTestCase,
        tester_binary: &Path,
    ) -> Result<SameBoyTesterCaseReport, SameBoyTesterExecutionError> {
        if case.execution_mode != ExecutionMode::Strict {
            return Err(SameBoyTesterExecutionError::NonStrictCase {
                case_id: case.id.clone(),
                actual: case.execution_mode,
            });
        }

        let capture = case.pass_condition.required_capture();
        if capture != CaptureKind::Framebuffer {
            return Err(SameBoyTesterExecutionError::UnsupportedCapture {
                case_id: case.id.clone(),
                capture,
            });
        }

        let rom_path = self
            .rom_runner
            .resolve_case_rom_path(case)
            .map_err(|source| SameBoyTesterExecutionError::ResolveRomPath {
                case_id: case.id.clone(),
                source: Box::new(source),
            })?;
        let staged_rom_path = self.stage_rom(case, &rom_path)?;
        let image_artifact_path =
            staged_rom_path.with_extension(self.image_format.file_extension());
        let log_artifact_path = staged_rom_path.with_extension("log");
        let sav_artifact_path = staged_rom_path.with_extension("sav");
        remove_if_present(&image_artifact_path)?;
        remove_if_present(&log_artifact_path)?;
        remove_if_present(&sav_artifact_path)?;

        let mut command = Command::new(tester_binary);
        command.arg(model_flag(case)?);
        if self.image_format == SameBoyTesterImageFormat::Tga {
            command.arg("--tga");
        }
        command
            .arg("--length")
            .arg(timeout_seconds(case.timeout).to_string());
        command.arg(&staged_rom_path);

        let output =
            command
                .output()
                .map_err(|source| SameBoyTesterExecutionError::SpawnTester {
                    path: tester_binary.to_path_buf(),
                    source,
                })?;
        if !output.status.success() {
            return Err(SameBoyTesterExecutionError::TesterFailed {
                path: tester_binary.to_path_buf(),
                status: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        if !image_artifact_path.is_file() {
            return Err(SameBoyTesterExecutionError::MissingImageArtifact {
                path: image_artifact_path,
            });
        }

        Ok(SameBoyTesterCaseReport {
            case_id: case.id.clone(),
            staged_rom_path,
            image_artifact_path,
            log_artifact_path: log_artifact_path.is_file().then_some(log_artifact_path),
            startup_mode_note: startup_mode_note(case.startup_mode),
        })
    }

    fn stage_rom(
        &self,
        case: &crate::RomTestCase,
        rom_path: &Path,
    ) -> Result<PathBuf, SameBoyTesterExecutionError> {
        let staged_rom_path = self
            .oracle_root
            .join(oracle_relative_rom_path(&case.rom_path));
        let parent = staged_rom_path
            .parent()
            .expect("staged ROM path should always have a parent");
        fs::create_dir_all(parent).map_err(|source| {
            SameBoyTesterExecutionError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            }
        })?;
        if staged_rom_path.exists() {
            fs::remove_file(&staged_rom_path).map_err(|source| {
                SameBoyTesterExecutionError::RemoveArtifact {
                    path: staged_rom_path.clone(),
                    source,
                }
            })?;
        }
        fs::copy(rom_path, &staged_rom_path).map_err(|source| {
            SameBoyTesterExecutionError::CopyRom {
                source_path: rom_path.to_path_buf(),
                destination_path: staged_rom_path.clone(),
                source,
            }
        })?;
        Ok(staged_rom_path)
    }

    fn ensure_tester_binary(&self) -> Result<PathBuf, SameBoyTesterExecutionError> {
        let explicit_binary = self
            .tester_binary
            .clone()
            .or_else(|| std::env::var_os(SAMEBOY_TESTER_BIN_ENV_VAR).map(PathBuf::from));
        if let Some(path) = explicit_binary {
            if path.is_file() {
                return Ok(path);
            }
            return Err(SameBoyTesterExecutionError::MissingTesterBinary { path });
        }

        let Some(sameboy_root) = &self.sameboy_root else {
            return Err(SameBoyTesterExecutionError::MissingSameBoyRoot);
        };
        let tester_binary = sameboy_root.join(default_sameboy_tester_relative_path());
        if tester_binary.is_file() {
            return Ok(tester_binary);
        }
        if self.build_if_missing {
            let mut command = Command::new("make");
            command
                .arg("tester")
                .arg("CONF=release")
                .current_dir(sameboy_root);
            let status =
                command
                    .status()
                    .map_err(|source| SameBoyTesterExecutionError::SpawnTester {
                        path: sameboy_root.join("make"),
                        source,
                    })?;
            if !status.success() {
                return Err(SameBoyTesterExecutionError::BuildTesterFailed {
                    root: sameboy_root.clone(),
                    status: status.code(),
                });
            }
            if tester_binary.is_file() {
                return Ok(tester_binary);
            }
        }

        Err(SameBoyTesterExecutionError::MissingTesterBinary {
            path: tester_binary,
        })
    }
}

fn default_sameboy_tester_relative_path() -> &'static str {
    if cfg!(windows) {
        "build/bin/tester/sameboy_tester.exe"
    } else {
        "build/bin/tester/sameboy_tester"
    }
}

fn oracle_relative_rom_path(rom_path: &Path) -> PathBuf {
    if rom_path.is_absolute() {
        let mut relative = PathBuf::new();
        for component in rom_path.components() {
            match component {
                std::path::Component::Normal(value) => relative.push(value),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => relative.push("parent"),
                std::path::Component::Prefix(prefix) => relative.push(prefix.as_os_str()),
                std::path::Component::RootDir => {}
            }
        }
        relative
    } else {
        rom_path.to_path_buf()
    }
}

fn remove_if_present(path: &Path) -> Result<(), SameBoyTesterExecutionError> {
    if path.exists() {
        fs::remove_file(path).map_err(|source| SameBoyTesterExecutionError::RemoveArtifact {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn model_flag(case: &crate::RomTestCase) -> Result<&'static str, SameBoyTesterExecutionError> {
    match case.console_model {
        ConsoleModel::Dmg0 | ConsoleModel::Dmg | ConsoleModel::Mgb => Ok("--dmg"),
        ConsoleModel::Cgb => Ok("--cgb"),
    }
}

fn timeout_seconds(timeout: Timeout) -> u64 {
    match timeout {
        Timeout::Frames(frames) => u64::from(frames).saturating_add(59) / 60,
        Timeout::TCycles(t_cycles) => t_cycles.saturating_add(4_194_303) / 4_194_304,
    }
    .max(1)
}

fn startup_mode_note(startup_mode: StartupMode) -> Option<String> {
    (startup_mode != StartupMode::RealBoot).then_some(
        "SameBoy Tester always executes through a boot ROM; end-of-test framebuffer comparison assumes boot-path convergence.".to_string(),
    )
}
