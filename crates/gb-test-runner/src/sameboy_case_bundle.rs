use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use gb_core::{ConsoleModel, ExecutionMode, StartupMode};

use crate::{
    CaptureKind, RomExecutionError, RomRunner, RomSuite, RomSuiteValidationError, Timeout,
};

pub const SAMEBOY_CASE_BUNDLE_BIN_ENV_VAR: &str = "GB_CYCLE_SAMEBOY_CASE_BUNDLE_BIN";

#[derive(Debug)]
pub enum SameBoyCaseBundleExecutionError {
    InvalidSuite(RomSuiteValidationError),
    NonStrictCase {
        case_id: String,
        actual: ExecutionMode,
    },
    UnsupportedCapture {
        case_id: String,
        capture: CaptureKind,
    },
    UnsupportedStartupMode {
        case_id: String,
        actual: StartupMode,
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
    MissingRunnerBinary {
        path: PathBuf,
    },
    BuildLibFailed {
        root: PathBuf,
        status: Option<i32>,
    },
    BuildRunnerFailed {
        path: PathBuf,
        status: Option<i32>,
        stderr: String,
    },
    SpawnRunner {
        path: PathBuf,
        source: io::Error,
    },
    RunnerFailed {
        path: PathBuf,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    CreateDirectory {
        path: PathBuf,
        source: io::Error,
    },
    MissingArtifact {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameBoyCaseBundleCaseReport {
    pub case_id: String,
    pub rom_path: PathBuf,
    pub serial_hex_artifact_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameBoyCaseBundleSuiteReport {
    pub suite_name: String,
    pub runner_binary: PathBuf,
    pub oracle_root: PathBuf,
    pub cases: Vec<SameBoyCaseBundleCaseReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameBoyCaseBundleRunner {
    rom_runner: RomRunner,
    oracle_root: PathBuf,
    sameboy_root: Option<PathBuf>,
    runner_binary: Option<PathBuf>,
    build_if_missing: bool,
}

impl SameBoyCaseBundleRunner {
    pub fn new(oracle_root: impl Into<PathBuf>) -> Self {
        Self {
            rom_runner: RomRunner::new(),
            oracle_root: oracle_root.into(),
            sameboy_root: std::env::var_os(crate::sameboy_tester::SAMEBOY_ROOT_ENV_VAR)
                .map(PathBuf::from),
            runner_binary: std::env::var_os(SAMEBOY_CASE_BUNDLE_BIN_ENV_VAR).map(PathBuf::from),
            build_if_missing: false,
        }
    }

    pub fn with_rom_runner(mut self, rom_runner: RomRunner) -> Self {
        self.rom_runner = rom_runner;
        self
    }

    pub fn with_sameboy_root(mut self, sameboy_root: impl Into<PathBuf>) -> Self {
        self.sameboy_root = Some(sameboy_root.into());
        self
    }

    pub fn with_runner_binary(mut self, runner_binary: impl Into<PathBuf>) -> Self {
        self.runner_binary = Some(runner_binary.into());
        self
    }

    pub fn with_build_if_missing(mut self, build_if_missing: bool) -> Self {
        self.build_if_missing = build_if_missing;
        self
    }

    pub fn run_suite(
        &self,
        suite: &RomSuite,
    ) -> Result<SameBoyCaseBundleSuiteReport, SameBoyCaseBundleExecutionError> {
        suite
            .validate()
            .map_err(SameBoyCaseBundleExecutionError::InvalidSuite)?;

        let runner_binary = self.ensure_runner_binary()?;
        let mut cases = Vec::with_capacity(suite.cases.len());
        for case in &suite.cases {
            cases.push(self.run_case(case, &runner_binary)?);
        }

        Ok(SameBoyCaseBundleSuiteReport {
            suite_name: suite.name.clone(),
            runner_binary,
            oracle_root: self.oracle_root.clone(),
            cases,
        })
    }

    fn run_case(
        &self,
        case: &crate::RomTestCase,
        runner_binary: &Path,
    ) -> Result<SameBoyCaseBundleCaseReport, SameBoyCaseBundleExecutionError> {
        if case.execution_mode != ExecutionMode::Strict {
            return Err(SameBoyCaseBundleExecutionError::NonStrictCase {
                case_id: case.id.clone(),
                actual: case.execution_mode,
            });
        }

        let capture = case.pass_condition.required_capture();
        if capture != CaptureKind::SerialHex {
            return Err(SameBoyCaseBundleExecutionError::UnsupportedCapture {
                case_id: case.id.clone(),
                capture,
            });
        }

        if case.startup_mode != StartupMode::SkipBoot {
            return Err(SameBoyCaseBundleExecutionError::UnsupportedStartupMode {
                case_id: case.id.clone(),
                actual: case.startup_mode,
            });
        }

        let model = model_arg(case)?;
        let rom_path = self
            .rom_runner
            .resolve_case_rom_path(case)
            .map_err(|source| SameBoyCaseBundleExecutionError::ResolveRomPath {
                case_id: case.id.clone(),
                source: Box::new(source),
            })?;

        let case_dir = self.oracle_root.join(&case.id);
        fs::create_dir_all(&case_dir).map_err(|source| {
            SameBoyCaseBundleExecutionError::CreateDirectory {
                path: case_dir.clone(),
                source,
            }
        })?;
        let serial_hex_artifact_path = case_dir.join("serial_hex.txt");

        let mut command = Command::new(runner_binary);
        command
            .arg("--model")
            .arg(model)
            .arg("--rom")
            .arg(&rom_path)
            .arg("--serial-hex-out")
            .arg(&serial_hex_artifact_path);
        match case.timeout {
            Timeout::TCycles(limit) => {
                command.arg("--timeout-tcycles").arg(limit.to_string());
            }
            Timeout::Frames(limit) => {
                command.arg("--timeout-frames").arg(limit.to_string());
            }
        }
        if let Some(seconds) = case.startup_cartridge_rtc_seconds {
            command
                .arg("--startup-cartridge-rtc-seconds")
                .arg(seconds.to_string());
        }

        let output =
            command
                .output()
                .map_err(|source| SameBoyCaseBundleExecutionError::SpawnRunner {
                    path: runner_binary.to_path_buf(),
                    source,
                })?;
        if !output.status.success() {
            return Err(SameBoyCaseBundleExecutionError::RunnerFailed {
                path: runner_binary.to_path_buf(),
                status: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        if !serial_hex_artifact_path.is_file() {
            return Err(SameBoyCaseBundleExecutionError::MissingArtifact {
                path: serial_hex_artifact_path,
            });
        }

        Ok(SameBoyCaseBundleCaseReport {
            case_id: case.id.clone(),
            rom_path,
            serial_hex_artifact_path,
        })
    }

    fn ensure_runner_binary(&self) -> Result<PathBuf, SameBoyCaseBundleExecutionError> {
        if let Some(path) = &self.runner_binary {
            if path.is_file() {
                return Ok(path.clone());
            }
            return Err(SameBoyCaseBundleExecutionError::MissingRunnerBinary {
                path: path.clone(),
            });
        }

        let Some(sameboy_root) = &self.sameboy_root else {
            return Err(SameBoyCaseBundleExecutionError::MissingSameBoyRoot);
        };
        let runner_binary = default_sameboy_case_bundle_runner_path(sameboy_root);
        if runner_binary.is_file() {
            return Ok(runner_binary);
        }
        if !self.build_if_missing {
            return Err(SameBoyCaseBundleExecutionError::MissingRunnerBinary {
                path: runner_binary,
            });
        }

        build_sameboy_case_bundle_runner(sameboy_root, &runner_binary)?;
        if runner_binary.is_file() {
            Ok(runner_binary)
        } else {
            Err(SameBoyCaseBundleExecutionError::MissingRunnerBinary {
                path: runner_binary,
            })
        }
    }
}

fn default_sameboy_case_bundle_runner_path(sameboy_root: &Path) -> PathBuf {
    let executable = if cfg!(windows) {
        "gb_cycle_case_bundle_runner.exe"
    } else {
        "gb_cycle_case_bundle_runner"
    };
    sameboy_root.join("build/bin").join(executable)
}

fn build_sameboy_case_bundle_runner(
    sameboy_root: &Path,
    runner_binary: &Path,
) -> Result<(), SameBoyCaseBundleExecutionError> {
    let lib_dir = sameboy_root.join("build/lib");
    let lib_object = lib_dir.join("libsameboy.o");
    let dynamic_lib = if cfg!(target_os = "macos") {
        lib_dir.join("libsameboy.dylib")
    } else if cfg!(windows) {
        lib_dir.join("libsameboy.dll")
    } else {
        lib_dir.join("libsameboy.so")
    };
    if !dynamic_lib.is_file() {
        let output = Command::new("make")
            .arg("lib")
            .current_dir(sameboy_root)
            .output()
            .map_err(|source| SameBoyCaseBundleExecutionError::SpawnRunner {
                path: sameboy_root.join("make"),
                source,
            })?;
        if !output.status.success() {
            return Err(SameBoyCaseBundleExecutionError::BuildLibFailed {
                root: sameboy_root.to_path_buf(),
                status: output.status.code(),
            });
        }
    }

    if let Some(parent) = runner_binary.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            SameBoyCaseBundleExecutionError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }

    let helper_source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("c_support/sameboy_case_bundle_runner.c");
    let mut command = Command::new("cc");
    command
        .arg("-std=c11")
        .arg("-O2")
        .arg("-I")
        .arg(sameboy_root.join("Core"))
        .arg("-o")
        .arg(runner_binary)
        .arg(helper_source)
        .arg(lib_object);
    if !cfg!(windows) {
        command.arg("-lm");
    }

    let output =
        command
            .output()
            .map_err(|source| SameBoyCaseBundleExecutionError::SpawnRunner {
                path: PathBuf::from("cc"),
                source,
            })?;
    if output.status.success() {
        return Ok(());
    }

    Err(SameBoyCaseBundleExecutionError::BuildRunnerFailed {
        path: runner_binary.to_path_buf(),
        status: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn model_arg(case: &crate::RomTestCase) -> Result<&'static str, SameBoyCaseBundleExecutionError> {
    match case.console_model {
        ConsoleModel::Dmg | ConsoleModel::Dmg0 => Ok("dmg"),
        ConsoleModel::Mgb => Ok("mgb"),
        ConsoleModel::Cgb => Ok("cgb"),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{RomRunner, TEST_ROM_ROOT_ENV_VAR, phase_6_cartridge_oracle_suite};

    use super::{
        SAMEBOY_CASE_BUNDLE_BIN_ENV_VAR, SameBoyCaseBundleExecutionError, SameBoyCaseBundleRunner,
        default_sameboy_case_bundle_runner_path,
    };

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-sameboy-case-bundle-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn write_fake_runner(path: &Path, args_output: &Path) {
        fs::write(
            path,
            format!(
                concat!(
                    "#!/bin/sh\n",
                    "set -eu\n",
                    "args_file=\"{}\"\n",
                    "printf '%s\\n' '---' >> \"$args_file\"\n",
                    "serial_hex_out=''\n",
                    "while [ \"$#\" -gt 0 ]; do\n",
                    "  printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "  if [ \"$1\" = '--serial-hex-out' ]; then\n",
                    "    shift\n",
                    "    printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "    serial_hex_out=\"$1\"\n",
                    "  fi\n",
                    "  shift\n",
                    "done\n",
                    "printf 'FAKEHEX' > \"$serial_hex_out\"\n",
                ),
                args_output.display(),
            ),
        )
        .expect("fake runner should be writable");
        let mut permissions = fs::metadata(path)
            .expect("fake runner metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("fake runner should be executable");
    }

    #[test]
    fn sameboy_case_bundle_runner_materializes_phase_6_serial_hex_artifacts() {
        let temp_dir = unique_temp_dir("phase6");
        let oracle_root = temp_dir.join("oracle");
        let external_root = temp_dir.join("external");
        let phase6_root = external_root.join("crates/gb-core/tests/fixtures/roms/phase6");
        fs::create_dir_all(&phase6_root).expect("phase6 dir should be creatable");

        for rom_name in [
            "phase6_mbc1_standard_banking.gb",
            "phase6_mbc1_small_rom_mask_and_ram.gb",
            "phase6_mbc2_control_decode_and_nibble_ram.gb",
            "phase6_mbc3_banking_ram_and_rtc.gb",
            "phase6_mbc5_rom_banking_rumble_and_ram.gb",
        ] {
            fs::write(phase6_root.join(rom_name), b"fake-rom").expect("fixture ROM should exist");
        }

        let args_output = temp_dir.join("runner-args.txt");
        let runner_binary = temp_dir.join("fake-sameboy-case-bundle.sh");
        write_fake_runner(&runner_binary, &args_output);

        let report = SameBoyCaseBundleRunner::new(&oracle_root)
            .with_rom_runner(
                RomRunner::new().with_external_rom_root(TEST_ROM_ROOT_ENV_VAR, &external_root),
            )
            .with_runner_binary(&runner_binary)
            .run_suite(&phase_6_cartridge_oracle_suite())
            .expect("case bundle suite should run");

        assert_eq!(report.cases.len(), 5);
        let mbc3 = report
            .cases
            .iter()
            .find(|case| case.case_id == "phase6-mbc3-banking-ram-and-rtc")
            .expect("report should include mbc3");
        assert_eq!(
            fs::read_to_string(&mbc3.serial_hex_artifact_path)
                .expect("serial hex artifact should be readable"),
            "FAKEHEX"
        );

        let args = fs::read_to_string(args_output).expect("runner args should be readable");
        assert!(args.contains("--model\ndmg\n"));
        assert!(args.contains("--timeout-tcycles\n200000\n"));
        assert!(args.contains("--startup-cartridge-rtc-seconds\n93784\n"));
        assert!(
            args.contains(
                mbc3.serial_hex_artifact_path
                    .to_str()
                    .expect("artifact path should be utf-8")
            )
        );
    }

    #[test]
    fn sameboy_case_bundle_runner_requires_serial_hex_cases() {
        let temp_dir = unique_temp_dir("reject");
        let runner_binary = temp_dir.join("fake-runner.sh");
        let args_output = temp_dir.join("runner-args.txt");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        write_fake_runner(&runner_binary, &args_output);

        let error = SameBoyCaseBundleRunner::new(temp_dir.join("oracle"))
            .with_runner_binary(&runner_binary)
            .run_suite(&crate::phase_2_cpu_timing_suite())
            .expect_err("trace suite should be rejected");

        assert!(matches!(
            error,
            SameBoyCaseBundleExecutionError::UnsupportedCapture { .. }
        ));
    }

    #[test]
    fn helper_path_and_env_defaults_follow_sameboy_layout() {
        let sameboy_root = Path::new("/tmp/SameBoy");
        assert_eq!(
            default_sameboy_case_bundle_runner_path(sameboy_root),
            sameboy_root.join("build/bin").join(if cfg!(windows) {
                "gb_cycle_case_bundle_runner.exe"
            } else {
                "gb_cycle_case_bundle_runner"
            })
        );
        assert_eq!(
            crate::sameboy_tester::SAMEBOY_ROOT_ENV_VAR,
            "GB_CYCLE_SAMEBOY_ROOT"
        );
        assert_eq!(
            SAMEBOY_CASE_BUNDLE_BIN_ENV_VAR,
            "GB_CYCLE_SAMEBOY_CASE_BUNDLE_BIN"
        );
    }
}
