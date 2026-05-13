use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use gb_core::{ConsoleModel, ExecutionMode, StartupMode};

use crate::{
    CaptureKind, RomExecutionError, RomRunner, RomSuite, RomSuiteValidationError,
    StartupMemoryWrite, Timeout,
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
    UnsupportedStartupTimerState {
        case_id: String,
    },
    UnsupportedExternalStimuli {
        case_id: String,
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
    pub capture: CaptureKind,
    pub artifact_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameBoyCaseBundleSuiteReport {
    pub suite_name: String,
    pub runner_binary: PathBuf,
    pub oracle_root: PathBuf,
    pub cases: Vec<SameBoyCaseBundleCaseReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameBoyProbeCaseReport {
    pub case_id: String,
    pub rom_path: PathBuf,
    pub probe_path: PathBuf,
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

    pub fn run_probe_case(
        &self,
        case: &crate::RomTestCase,
        probe_path: &Path,
        probe_interval_tcycles: u64,
    ) -> Result<SameBoyProbeCaseReport, SameBoyCaseBundleExecutionError> {
        if case.execution_mode != ExecutionMode::Strict {
            return Err(SameBoyCaseBundleExecutionError::NonStrictCase {
                case_id: case.id.clone(),
                actual: case.execution_mode,
            });
        }

        if !case.startup_mode.uses_direct_boot_state() {
            return Err(SameBoyCaseBundleExecutionError::UnsupportedStartupMode {
                case_id: case.id.clone(),
                actual: case.startup_mode,
            });
        }

        if case.startup_timer_state.is_some() {
            return Err(
                SameBoyCaseBundleExecutionError::UnsupportedStartupTimerState {
                    case_id: case.id.clone(),
                },
            );
        }

        if !case.external_stimuli.stimuli().is_empty() {
            return Err(
                SameBoyCaseBundleExecutionError::UnsupportedExternalStimuli {
                    case_id: case.id.clone(),
                },
            );
        }

        let runner_binary = self.ensure_runner_binary()?;
        let model = model_arg(case);
        let rom_path = self
            .rom_runner
            .resolve_case_rom_path(case)
            .map_err(|source| SameBoyCaseBundleExecutionError::ResolveRomPath {
                case_id: case.id.clone(),
                source: Box::new(source),
            })?;

        if let Some(parent) = probe_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                SameBoyCaseBundleExecutionError::CreateDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }

        let mut command = Command::new(&runner_binary);
        command
            .arg("--model")
            .arg(model)
            .arg("--rom")
            .arg(&rom_path)
            .arg("--probe-json-out")
            .arg(probe_path)
            .arg("--probe-interval-tcycles")
            .arg(probe_interval_tcycles.to_string());
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
        append_case_startup_memory_write_args(&mut command, case);

        let output =
            command
                .output()
                .map_err(|source| SameBoyCaseBundleExecutionError::SpawnRunner {
                    path: runner_binary.clone(),
                    source,
                })?;
        if !output.status.success() {
            return Err(SameBoyCaseBundleExecutionError::RunnerFailed {
                path: runner_binary,
                status: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        if !probe_path.is_file() {
            return Err(SameBoyCaseBundleExecutionError::MissingArtifact {
                path: probe_path.to_path_buf(),
            });
        }

        Ok(SameBoyProbeCaseReport {
            case_id: case.id.clone(),
            rom_path,
            probe_path: probe_path.to_path_buf(),
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
        if !matches!(capture, CaptureKind::SerialHex | CaptureKind::Framebuffer) {
            return Err(SameBoyCaseBundleExecutionError::UnsupportedCapture {
                case_id: case.id.clone(),
                capture,
            });
        }

        if !case.startup_mode.uses_direct_boot_state() {
            return Err(SameBoyCaseBundleExecutionError::UnsupportedStartupMode {
                case_id: case.id.clone(),
                actual: case.startup_mode,
            });
        }

        if case.startup_timer_state.is_some() {
            return Err(
                SameBoyCaseBundleExecutionError::UnsupportedStartupTimerState {
                    case_id: case.id.clone(),
                },
            );
        }

        let model = model_arg(case);
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
        let artifact_path = artifact_path_for_capture(&case_dir, capture);

        let mut command = Command::new(runner_binary);
        command
            .arg("--model")
            .arg(model)
            .arg("--rom")
            .arg(&rom_path);
        match capture {
            CaptureKind::SerialHex => {
                command.arg("--serial-hex-out").arg(&artifact_path);
            }
            CaptureKind::Framebuffer => {
                command.arg("--framebuffer-pgm-out").arg(&artifact_path);
            }
            _ => unreachable!("unsupported captures are rejected before command construction"),
        }
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
        append_case_startup_memory_write_args(&mut command, case);

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
        if !artifact_path.is_file() {
            return Err(SameBoyCaseBundleExecutionError::MissingArtifact {
                path: artifact_path,
            });
        }

        Ok(SameBoyCaseBundleCaseReport {
            case_id: case.id.clone(),
            rom_path,
            capture,
            artifact_path,
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
        if runner_binary.is_file()
            && (!self.build_if_missing
                || !runner_binary_needs_rebuild(sameboy_root, &runner_binary))
        {
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

fn artifact_path_for_capture(case_dir: &Path, capture: CaptureKind) -> PathBuf {
    match capture {
        CaptureKind::SerialHex => case_dir.join("serial_hex.txt"),
        CaptureKind::Framebuffer => case_dir.join("framebuffer.pgm"),
        _ => unreachable!("unsupported captures are rejected before artifact path resolution"),
    }
}

fn append_startup_memory_write_args(command: &mut Command, write: StartupMemoryWrite) {
    command
        .arg("--write-memory")
        .arg(write.address.to_string())
        .arg(write.value.to_string());
}

fn append_case_startup_memory_write_args(command: &mut Command, case: &crate::RomTestCase) {
    if case.startup_mode == StartupMode::CustomBoot {
        append_dmg_boot_logo_vram_startup_write_args(command);
    }
    for write in &case.startup_memory_writes {
        append_startup_memory_write_args(command, *write);
    }
}

fn append_dmg_boot_logo_vram_startup_write_args(command: &mut Command) {
    for (index, byte) in gb_core::boot::DMG_BOOT_LOGO_TILE_BYTES
        .iter()
        .copied()
        .enumerate()
    {
        append_startup_memory_write_args(
            command,
            StartupMemoryWrite::new(
                gb_core::boot::DMG_BOOT_LOGO_TILE_VRAM_START + (index as u16 * 2),
                byte,
            ),
        );
    }
    for (index, byte) in gb_core::boot::DMG_BOOT_LOGO_MAP_BYTES
        .iter()
        .copied()
        .enumerate()
    {
        append_startup_memory_write_args(
            command,
            StartupMemoryWrite::new(
                gb_core::boot::DMG_BOOT_LOGO_MAP_VRAM_START + index as u16,
                byte,
            ),
        );
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

fn helper_source_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("c_support/sameboy_case_bundle_runner.c")
}

fn runner_binary_needs_rebuild(sameboy_root: &Path, runner_binary: &Path) -> bool {
    let Ok(runner_modified) = runner_binary
        .metadata()
        .and_then(|metadata| metadata.modified())
    else {
        return true;
    };

    for dependency in [
        helper_source_path(),
        sameboy_root.join("build/lib").join("libsameboy.o"),
    ] {
        let Ok(dependency_modified) = dependency
            .metadata()
            .and_then(|metadata| metadata.modified())
        else {
            continue;
        };
        if dependency_modified > runner_modified {
            return true;
        }
    }

    false
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

    let mut command = Command::new("cc");
    command
        .arg("-std=c11")
        .arg("-O2")
        .arg("-I")
        .arg(sameboy_root.join("Core"))
        .arg("-o")
        .arg(runner_binary)
        .arg(helper_source_path())
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

fn model_arg(case: &crate::RomTestCase) -> &'static str {
    match case.console_model {
        ConsoleModel::GameBoy => "dmg",
        ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => "mgb",
        ConsoleModel::GameBoyColor => "cgb",
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use gb_core::StartupMode;

    use crate::{
        CaptureKind, RomRunner, TEST_ROM_ROOT_ENV_VAR, Timeout, phase_6_cartridge_oracle_suite,
    };

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
                    "framebuffer_pgm_out=''\n",
                    "probe_json_out=''\n",
                    "while [ \"$#\" -gt 0 ]; do\n",
                    "  printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "  if [ \"$1\" = '--serial-hex-out' ]; then\n",
                    "    shift\n",
                    "    printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "    serial_hex_out=\"$1\"\n",
                    "  elif [ \"$1\" = '--framebuffer-pgm-out' ]; then\n",
                    "    shift\n",
                    "    printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "    framebuffer_pgm_out=\"$1\"\n",
                    "  elif [ \"$1\" = '--probe-json-out' ]; then\n",
                    "    shift\n",
                    "    printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "    probe_json_out=\"$1\"\n",
                    "  elif [ \"$1\" = '--probe-interval-tcycles' ]; then\n",
                    "    shift\n",
                    "    printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "  elif [ \"$1\" = '--write-memory' ]; then\n",
                    "    shift\n",
                    "    printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "    shift\n",
                    "    printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "  fi\n",
                    "  shift\n",
                    "done\n",
                    "if [ -n \"$serial_hex_out\" ]; then printf 'FAKEHEX' > \"$serial_hex_out\"; fi\n",
                    "if [ -n \"$framebuffer_pgm_out\" ]; then printf 'P5\\n1 1\\n255\\n\\377' > \"$framebuffer_pgm_out\"; fi\n",
                    "if [ -n \"$probe_json_out\" ]; then printf '{{\"t_cycles\":0,\"pc\":256,\"sp\":65534,\"af\":432,\"bc\":19,\"de\":216,\"hl\":333,\"ime\":false,\"div\":171,\"tima\":0,\"tma\":0,\"tac\":248,\"interrupt_flags\":225,\"interrupt_enable\":0,\"lcdc\":145,\"stat\":133,\"ly\":0,\"line_dot\":0,\"scy\":0,\"scx\":0,\"lyc\":0,\"bgp\":252,\"obp0\":255,\"obp1\":255,\"wy\":0,\"wx\":0,\"vram_hash\":\"a\",\"oam_hash\":\"b\",\"wram_hash\":\"c\",\"hram_hash\":\"d\",\"framebuffer_hash\":\"e\",\"serial_hex\":\"\"}}\\n' > \"$probe_json_out\"; fi\n",
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

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).expect("executable should be writable");
        let mut permissions = fs::metadata(path)
            .expect("executable metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("executable should be executable");
    }

    fn dynamic_lib_name() -> &'static str {
        if cfg!(target_os = "macos") {
            "libsameboy.dylib"
        } else {
            "libsameboy.so"
        }
    }

    fn with_tool_path<T>(tool_root: &Path, action: impl FnOnce() -> T) -> T {
        let _guard = crate::test_support::lock_env();
        let previous = env::var_os("PATH");
        let mut paths = vec![tool_root.to_path_buf()];
        if let Some(path) = previous.clone() {
            paths.extend(std::env::split_paths(&path));
        }
        let joined = std::env::join_paths(paths).expect("PATH entries should join");
        unsafe {
            env::set_var("PATH", &joined);
        }
        let result = action();
        match previous {
            Some(path) => unsafe {
                env::set_var("PATH", path);
            },
            None => unsafe {
                env::remove_var("PATH");
            },
        }
        result
    }

    fn single_phase_6_case_suite() -> crate::RomSuite {
        let mut suite = phase_6_cartridge_oracle_suite();
        suite.cases.truncate(1);
        suite
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
            fs::read_to_string(&mbc3.artifact_path)
                .expect("serial hex artifact should be readable"),
            "FAKEHEX"
        );

        let args = fs::read_to_string(args_output).expect("runner args should be readable");
        assert!(args.contains("--model\ndmg\n"));
        assert!(args.contains("--timeout-tcycles\n200000\n"));
        assert!(args.contains("--startup-cartridge-rtc-seconds\n93784\n"));
        assert!(
            args.contains(
                mbc3.artifact_path
                    .to_str()
                    .expect("artifact path should be utf-8")
            )
        );
    }

    #[test]
    fn sameboy_case_bundle_runner_materializes_framebuffer_artifacts_with_startup_writes() {
        let temp_dir = unique_temp_dir("framebuffer");
        let oracle_root = temp_dir.join("oracle");
        let external_root = temp_dir.join("external");
        let acid_root = external_root.join("acid");
        fs::create_dir_all(&acid_root).expect("acid dir should be creatable");
        fs::write(acid_root.join("dmg-acid2.gb"), b"fake-rom").expect("fixture ROM should exist");

        let args_output = temp_dir.join("runner-args.txt");
        let runner_binary = temp_dir.join("fake-sameboy-case-bundle.sh");
        write_fake_runner(&runner_binary, &args_output);

        let acid_suite = crate::acid_dmg_curated_suite();
        let mut case = acid_suite
            .cases
            .into_iter()
            .find(|case| case.id == "dmg-acid2")
            .expect("acid2 case should exist")
            .with_startup_memory_write(crate::StartupMemoryWrite::new(0x8000, 0x42));
        case.timeout = Timeout::Frames(12);
        let suite = crate::RomSuite::new("acid-framebuffer-only", crate::TestSubsystem::Ppu)
            .with_family("acid")
            .with_case(case);

        let report = SameBoyCaseBundleRunner::new(&oracle_root)
            .with_rom_runner(
                RomRunner::new().with_external_rom_root(TEST_ROM_ROOT_ENV_VAR, &external_root),
            )
            .with_runner_binary(&runner_binary)
            .run_suite(&suite)
            .expect("case bundle framebuffer suite should run");

        assert_eq!(report.cases.len(), 1);
        let case = &report.cases[0];
        assert_eq!(case.capture, CaptureKind::Framebuffer);
        assert!(case.artifact_path.ends_with("framebuffer.pgm"));
        assert!(case.artifact_path.is_file());

        let args = fs::read_to_string(args_output).expect("runner args should be readable");
        assert!(args.contains("--framebuffer-pgm-out\n"));
        assert!(args.contains("--timeout-frames\n12\n"));
        assert!(args.contains("--write-memory\n32768\n66\n"));
    }

    #[test]
    fn sameboy_case_bundle_runner_materializes_probe_json() {
        let temp_dir = unique_temp_dir("probe");
        let oracle_root = temp_dir.join("oracle");
        let external_root = temp_dir.join("external");
        let hacktix_root = external_root.join("hacktix");
        fs::create_dir_all(&hacktix_root).expect("hacktix dir should be creatable");
        fs::write(hacktix_root.join("bully.gb"), b"fake-rom").expect("fixture ROM should exist");

        let args_output = temp_dir.join("runner-args.txt");
        let runner_binary = temp_dir.join("fake-sameboy-case-bundle.sh");
        write_fake_runner(&runner_binary, &args_output);

        let suite = crate::hacktix_dmg_curated_suite();
        let case = suite
            .cases
            .iter()
            .find(|case| case.id == "hacktix-bully")
            .expect("hacktix bully case should exist");
        let probe_path = oracle_root
            .join("hacktix-bully")
            .join("sameboy_probes.jsonl");
        let report = SameBoyCaseBundleRunner::new(&oracle_root)
            .with_rom_runner(
                RomRunner::new().with_external_rom_root(TEST_ROM_ROOT_ENV_VAR, &external_root),
            )
            .with_runner_binary(&runner_binary)
            .run_probe_case(case, &probe_path, 70_224)
            .expect("probe case should run");

        assert_eq!(report.case_id, "hacktix-bully");
        assert!(report.probe_path.is_file());
        assert!(
            fs::read_to_string(&report.probe_path)
                .expect("probe JSONL should be readable")
                .contains("\"framebuffer_hash\"")
        );

        let args = fs::read_to_string(args_output).expect("runner args should be readable");
        assert!(args.contains("--probe-json-out\n"));
        assert!(args.contains("--probe-interval-tcycles\n70224\n"));
    }

    #[test]
    fn sameboy_case_bundle_runner_requires_serial_hex_or_framebuffer_cases() {
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

    #[test]
    fn ensure_runner_binary_reports_missing_root_and_missing_explicit_binary() {
        let missing_root = SameBoyCaseBundleRunner::new("/tmp/oracle")
            .ensure_runner_binary()
            .expect_err("missing SameBoy root should fail");
        assert!(matches!(
            missing_root,
            SameBoyCaseBundleExecutionError::MissingSameBoyRoot
        ));

        let explicit_missing = SameBoyCaseBundleRunner::new("/tmp/oracle")
            .with_runner_binary("/tmp/definitely-missing-runner")
            .ensure_runner_binary()
            .expect_err("missing explicit runner should fail");
        assert!(matches!(
            explicit_missing,
            SameBoyCaseBundleExecutionError::MissingRunnerBinary { .. }
        ));
    }

    #[test]
    fn ensure_runner_binary_can_build_default_runner_with_fake_toolchain() {
        let temp_dir = unique_temp_dir("build-runner");
        let sameboy_root = temp_dir.join("SameBoy");
        let tool_root = temp_dir.join("tools");
        fs::create_dir_all(&sameboy_root).expect("sameboy root should be creatable");
        fs::create_dir_all(&tool_root).expect("tool root should be creatable");

        write_executable(
            &tool_root.join("make"),
            &format!(
                concat!(
                    "#!/bin/sh\n",
                    "set -eu\n",
                    "mkdir -p \"$PWD/build/lib\"\n",
                    ": > \"$PWD/build/lib/libsameboy.o\"\n",
                    ": > \"$PWD/build/lib/{}\"\n",
                ),
                dynamic_lib_name(),
            ),
        );
        write_executable(
            &tool_root.join("cc"),
            concat!(
                "#!/bin/sh\n",
                "set -eu\n",
                "out=''\n",
                "while [ \"$#\" -gt 0 ]; do\n",
                "  if [ \"$1\" = '-o' ]; then\n",
                "    shift\n",
                "    out=\"$1\"\n",
                "  fi\n",
                "  shift\n",
                "done\n",
                "mkdir -p \"$(dirname \"$out\")\"\n",
                "printf '#!/bin/sh\\nexit 0\\n' > \"$out\"\n",
                "chmod +x \"$out\"\n",
            ),
        );

        let built_runner = with_tool_path(&tool_root, || {
            SameBoyCaseBundleRunner::new("/tmp/oracle")
                .with_sameboy_root(&sameboy_root)
                .with_build_if_missing(true)
                .ensure_runner_binary()
                .expect("runner should build with fake toolchain")
        });

        assert_eq!(
            built_runner,
            default_sameboy_case_bundle_runner_path(&sameboy_root)
        );
        assert!(built_runner.is_file());
    }

    #[test]
    fn ensure_runner_binary_reports_make_and_cc_failures() {
        let temp_dir = unique_temp_dir("build-failures");
        let tool_root = temp_dir.join("tools");
        fs::create_dir_all(&tool_root).expect("tool root should be creatable");

        let make_fail_root = temp_dir.join("sameboy-make-fail");
        fs::create_dir_all(&make_fail_root).expect("make-fail root should be creatable");
        write_executable(&tool_root.join("make"), "#!/bin/sh\nset -eu\nexit 3\n");
        write_executable(&tool_root.join("cc"), "#!/bin/sh\nset -eu\nexit 0\n");

        let make_error = with_tool_path(&tool_root, || {
            SameBoyCaseBundleRunner::new("/tmp/oracle")
                .with_sameboy_root(&make_fail_root)
                .with_build_if_missing(true)
                .ensure_runner_binary()
                .expect_err("make failure should surface")
        });
        assert!(matches!(
            make_error,
            SameBoyCaseBundleExecutionError::BuildLibFailed {
                status: Some(3),
                ..
            }
        ));

        let cc_fail_root = temp_dir.join("sameboy-cc-fail");
        let lib_dir = cc_fail_root.join("build/lib");
        fs::create_dir_all(&lib_dir).expect("cc-fail lib dir should be creatable");
        fs::write(lib_dir.join(dynamic_lib_name()), b"fake-dylib")
            .expect("dynamic lib marker should be writable");
        fs::write(lib_dir.join("libsameboy.o"), b"fake-object")
            .expect("object marker should be writable");
        write_executable(
            &tool_root.join("cc"),
            "#!/bin/sh\nset -eu\nprintf 'compile failed' >&2\nexit 7\n",
        );

        let cc_error = with_tool_path(&tool_root, || {
            SameBoyCaseBundleRunner::new("/tmp/oracle")
                .with_sameboy_root(&cc_fail_root)
                .with_build_if_missing(true)
                .ensure_runner_binary()
                .expect_err("cc failure should surface")
        });
        assert!(matches!(
            cc_error,
            SameBoyCaseBundleExecutionError::BuildRunnerFailed {
                status: Some(7),
                ..
            }
        ));
        let SameBoyCaseBundleExecutionError::BuildRunnerFailed { stderr, .. } = cc_error else {
            panic!("expected build-runner failure");
        };
        assert!(stderr.contains("compile failed"));
    }

    #[test]
    fn sameboy_case_bundle_runner_rejects_invalid_suite_contracts_before_spawning() {
        let temp_dir = unique_temp_dir("invalid-contracts");
        let runner_binary = temp_dir.join("fake-runner.sh");
        let args_output = temp_dir.join("runner-args.txt");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        write_fake_runner(&runner_binary, &args_output);

        let mut non_strict = single_phase_6_case_suite();
        non_strict.cases[0].execution_mode = gb_core::ExecutionMode::Permissive;
        let error = SameBoyCaseBundleRunner::new(temp_dir.join("oracle"))
            .with_runner_binary(&runner_binary)
            .run_suite(&non_strict)
            .expect_err("non-strict suite should be rejected");
        assert!(matches!(
            error,
            SameBoyCaseBundleExecutionError::NonStrictCase { .. }
        ));

        let mut real_boot = single_phase_6_case_suite();
        real_boot.cases[0].startup_mode = StartupMode::RealBoot;
        let error = SameBoyCaseBundleRunner::new(temp_dir.join("oracle"))
            .with_runner_binary(&runner_binary)
            .run_suite(&real_boot)
            .expect_err("real-boot suite should be rejected");
        assert!(matches!(
            error,
            SameBoyCaseBundleExecutionError::UnsupportedStartupMode { .. }
        ));
    }

    #[test]
    fn sameboy_case_bundle_runner_surfaces_process_and_artifact_failures() {
        let temp_dir = unique_temp_dir("runner-failures");
        let oracle_root = temp_dir.join("oracle");
        let suite = single_phase_6_case_suite();
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

        let spawn_runner = temp_dir.join("spawn-runner.sh");
        fs::write(&spawn_runner, b"#!/bin/sh\n").expect("spawn runner should be writable");
        let spawn_error = SameBoyCaseBundleRunner::new(&oracle_root)
            .with_runner_binary(&spawn_runner)
            .run_suite(&suite)
            .expect_err("non-executable runner should fail to spawn");
        assert!(matches!(
            spawn_error,
            SameBoyCaseBundleExecutionError::SpawnRunner { .. }
        ));

        let missing_artifact_runner = temp_dir.join("missing-artifact-runner.sh");
        write_executable(&missing_artifact_runner, "#!/bin/sh\nset -eu\nexit 0\n");
        let missing_artifact = SameBoyCaseBundleRunner::new(&oracle_root)
            .with_runner_binary(&missing_artifact_runner)
            .run_suite(&suite)
            .expect_err("successful runner without artifact should fail");
        assert!(matches!(
            missing_artifact,
            SameBoyCaseBundleExecutionError::MissingArtifact { .. }
        ));

        let failing_runner = temp_dir.join("failing-runner.sh");
        write_executable(
            &failing_runner,
            "#!/bin/sh\nset -eu\nprintf 'runner-stdout' ; printf 'runner-stderr' >&2 ; exit 9\n",
        );
        let runner_failed = SameBoyCaseBundleRunner::new(&oracle_root)
            .with_runner_binary(&failing_runner)
            .run_suite(&suite)
            .expect_err("runner failure should surface");
        let SameBoyCaseBundleExecutionError::RunnerFailed {
            path,
            status: _,
            stdout,
            stderr,
        } = runner_failed
        else {
            panic!("expected runner failure, got {runner_failed:?}");
        };
        assert_eq!(path, failing_runner);
        assert_eq!(stdout, "runner-stdout");
        assert_eq!(stderr, "runner-stderr");

        let blocked_root = temp_dir.join("oracle-root-file");
        fs::write(&blocked_root, b"not-a-directory")
            .expect("blocked root marker should be writable");
        let args_output = temp_dir.join("runner-args-create-dir.txt");
        let working_runner = temp_dir.join("working-runner.sh");
        write_fake_runner(&working_runner, &args_output);
        let create_dir_error = SameBoyCaseBundleRunner::new(&blocked_root)
            .with_runner_binary(&working_runner)
            .run_suite(&suite)
            .expect_err("non-directory oracle root should fail");
        assert!(matches!(
            create_dir_error,
            SameBoyCaseBundleExecutionError::CreateDirectory { .. }
        ));
    }
}
