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

#[cfg(test)]
mod tests {
    use super::{
        SAMEBOY_ROOT_ENV_VAR, SAMEBOY_TESTER_BIN_ENV_VAR, SameBoyTesterExecutionError,
        SameBoyTesterImageFormat, SameBoyTesterRunner, default_sameboy_tester_relative_path,
        oracle_relative_rom_path, remove_if_present, startup_mode_note, timeout_seconds,
    };
    use crate::{PassCondition, RomSuite, RomTestCase, TestSubsystem, Timeout};
    use gb_core::{ConsoleModel, ExecutionMode, StartupMode};
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-sameboy-unit-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore the touched variables before dropping the guard.
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore the touched variables before dropping the guard.
        unsafe {
            env::remove_var(key);
        }
    }

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).expect("script should be writable");
        let mut permissions = fs::metadata(path)
            .expect("script metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("script should be executable");
    }

    fn success_binary_path(temp_dir: &Path) -> PathBuf {
        for candidate in ["/usr/bin/true", "/bin/true"] {
            let path = Path::new(candidate);
            if path.is_file() {
                return path.to_path_buf();
            }
        }

        let tester_binary = temp_dir.join("fake-success");
        write_executable(&tester_binary, "#!/bin/sh\nexit 0\n");
        tester_binary
    }

    fn sample_framebuffer_case() -> RomTestCase {
        RomTestCase::new(
            "acid2",
            "acid/dmg-acid2.gb",
            Timeout::Frames(180),
            PassCondition::FramebufferFixture(PathBuf::from("unused")),
        )
    }

    #[test]
    fn image_format_helpers_and_timeout_rounding_are_explicit() {
        assert_eq!(SameBoyTesterImageFormat::Bmp.name(), "bmp");
        assert_eq!(SameBoyTesterImageFormat::Tga.name(), "tga");
        assert_eq!(timeout_seconds(Timeout::Frames(1)), 1);
        assert_eq!(timeout_seconds(Timeout::Frames(180)), 3);
        assert_eq!(timeout_seconds(Timeout::TCycles(1)), 1);
        assert_eq!(timeout_seconds(Timeout::TCycles(4_194_304)), 1);
        assert_eq!(timeout_seconds(Timeout::TCycles(4_194_305)), 2);
        assert!(startup_mode_note(StartupMode::SkipBoot).is_some());
        assert!(startup_mode_note(StartupMode::RealBoot).is_none());
    }

    #[test]
    fn helper_paths_keep_sameboy_layout_and_absolute_roms_stable() {
        assert!(
            default_sameboy_tester_relative_path().ends_with(if cfg!(windows) {
                "sameboy_tester.exe"
            } else {
                "sameboy_tester"
            })
        );
        assert_eq!(
            oracle_relative_rom_path(Path::new("acid/dmg-acid2.gb")),
            PathBuf::from("acid/dmg-acid2.gb")
        );
        assert_eq!(
            oracle_relative_rom_path(Path::new("/tmp/mealybug/../acid/dmg-acid2.gb")),
            PathBuf::from("tmp/mealybug/parent/acid/dmg-acid2.gb")
        );
    }

    #[test]
    fn remove_if_present_is_idempotent() {
        let temp_dir = unique_temp_dir("remove");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let path = temp_dir.join("artifact.bin");
        remove_if_present(&path).expect("missing file should be ignored");
        fs::write(&path, b"x").expect("artifact should be writable");
        remove_if_present(&path).expect("existing artifact should be removable");
        assert!(!path.exists());
    }

    #[test]
    fn ensure_tester_binary_reports_missing_root_and_missing_explicit_binary() {
        let _guard = crate::test_support::lock_env();
        let old_sameboy_root = env::var_os(SAMEBOY_ROOT_ENV_VAR);
        let old_tester_binary = env::var_os(SAMEBOY_TESTER_BIN_ENV_VAR);
        remove_env_var(SAMEBOY_ROOT_ENV_VAR);
        remove_env_var(SAMEBOY_TESTER_BIN_ENV_VAR);

        let error = SameBoyTesterRunner::new("/tmp/oracle")
            .ensure_tester_binary()
            .expect_err("missing sameboy root should fail");
        assert!(matches!(
            error,
            SameBoyTesterExecutionError::MissingSameBoyRoot
        ));

        let explicit_missing = SameBoyTesterRunner::new("/tmp/oracle")
            .with_tester_binary("/tmp/missing-sameboy-tester")
            .ensure_tester_binary()
            .expect_err("missing explicit binary should fail");
        assert!(matches!(
            explicit_missing,
            SameBoyTesterExecutionError::MissingTesterBinary { .. }
        ));

        if let Some(old_sameboy_root) = old_sameboy_root {
            set_env_var(SAMEBOY_ROOT_ENV_VAR, old_sameboy_root);
        }
        if let Some(old_tester_binary) = old_tester_binary {
            set_env_var(SAMEBOY_TESTER_BIN_ENV_VAR, old_tester_binary);
        }
    }

    #[test]
    fn ensure_tester_binary_can_use_env_and_build_if_missing() {
        let _guard = crate::test_support::lock_env();
        let temp_dir = unique_temp_dir("ensure-binary");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

        let explicit_binary = temp_dir.join("explicit-tester");
        write_executable(&explicit_binary, "#!/bin/sh\nexit 0\n");
        set_env_var(SAMEBOY_TESTER_BIN_ENV_VAR, &explicit_binary);
        let env_binary = SameBoyTesterRunner::new("/tmp/oracle")
            .ensure_tester_binary()
            .expect("env binary should resolve");
        assert_eq!(env_binary, explicit_binary);
        remove_env_var(SAMEBOY_TESTER_BIN_ENV_VAR);

        let sameboy_root = temp_dir.join("SameBoy");
        let tester_path = sameboy_root.join(default_sameboy_tester_relative_path());
        fs::create_dir_all(
            tester_path
                .parent()
                .expect("tester path should have a parent"),
        )
        .expect("sameboy tester dir should be creatable");

        let fake_bin_dir = temp_dir.join("bin");
        fs::create_dir_all(&fake_bin_dir).expect("bin dir should be creatable");
        let fake_make = fake_bin_dir.join("make");
        write_executable(
            &fake_make,
            &format!(
                "#!/bin/sh\nset -eu\nmkdir -p \"{}\"\nprintf '#!/bin/sh\\nexit 0\\n' > \"{}\"\nchmod +x \"{}\"\n",
                tester_path
                    .parent()
                    .expect("tester path should have a parent")
                    .display(),
                tester_path.display(),
                tester_path.display(),
            ),
        );
        let old_path = env::var_os("PATH");
        set_env_var(
            "PATH",
            format!(
                "{}:{}",
                fake_bin_dir.display(),
                old_path.as_ref().map_or_else(
                    || "".to_string(),
                    |value| value.to_string_lossy().into_owned()
                )
            ),
        );
        set_env_var(SAMEBOY_ROOT_ENV_VAR, &sameboy_root);
        let built = SameBoyTesterRunner::new("/tmp/oracle")
            .with_build_if_missing(true)
            .ensure_tester_binary()
            .expect("build-if-missing should produce the tester");
        assert_eq!(built, tester_path);

        if let Some(old_path) = old_path {
            set_env_var("PATH", old_path);
        } else {
            remove_env_var("PATH");
        }
        remove_env_var(SAMEBOY_ROOT_ENV_VAR);
    }

    #[test]
    fn stage_rom_and_run_case_cover_artifact_paths_and_missing_image_errors() {
        let temp_dir = unique_temp_dir("run-case");
        let oracle_root = temp_dir.join("oracle");
        let rom_root = temp_dir.join("roms");
        let rom_path = rom_root.join("acid/dmg-acid2.gb");
        fs::create_dir_all(rom_path.parent().expect("rom path should have a parent"))
            .expect("rom dir should be creatable");
        fs::write(&rom_path, b"rom").expect("rom should be writable");

        let runner = SameBoyTesterRunner::new(&oracle_root).with_rom_runner(
            crate::RomRunner::new().with_external_rom_root("TEST_ROOT", &rom_root),
        );
        let case = sample_framebuffer_case().with_external_rom_root_key("TEST_ROOT");
        let staged = runner
            .stage_rom(&case, &rom_path)
            .expect("rom should stage");
        assert_eq!(
            fs::read(&staged).expect("staged rom should be readable"),
            b"rom"
        );

        let tester_binary = success_binary_path(&temp_dir);
        let error = runner
            .clone()
            .with_tester_binary(&tester_binary)
            .run_case(&case, &tester_binary)
            .expect_err("missing image output should fail");
        assert!(
            matches!(
                error,
                SameBoyTesterExecutionError::MissingImageArtifact { .. }
            ),
            "unexpected run_case error: {error:?}"
        );

        let non_strict = case.clone().with_execution_mode(ExecutionMode::Permissive);
        let error = runner
            .with_tester_binary(&tester_binary)
            .run_case(&non_strict, &tester_binary)
            .expect_err("non-strict case should fail");
        assert!(matches!(
            error,
            SameBoyTesterExecutionError::NonStrictCase { .. }
        ));
    }

    #[test]
    fn run_suite_reports_staged_cases_and_startup_notes() {
        let temp_dir = unique_temp_dir("run-suite");
        let oracle_root = temp_dir.join("oracle");
        let rom_root = temp_dir.join("roms");
        let rom_a = rom_root.join("acid/dmg-acid2.gb");
        let rom_b = rom_root.join("suite/b.gb");
        fs::create_dir_all(rom_a.parent().expect("rom path should have a parent"))
            .expect("rom dir should be creatable");
        fs::create_dir_all(rom_b.parent().expect("rom path should have a parent"))
            .expect("rom dir should be creatable");
        fs::write(&rom_a, b"a").expect("rom a should be writable");
        fs::write(&rom_b, b"b").expect("rom b should be writable");

        let tester_binary = temp_dir.join("fake-tester");
        write_executable(
            &tester_binary,
            "#!/bin/sh\nset -eu\nrom=''\next='bmp'\nfor arg in \"$@\"; do\n  if [ \"$arg\" = \"--tga\" ]; then ext='tga'; fi\n  rom=\"$arg\"\ndone\nprintf 'image' > \"${rom%.gb}.${ext}\"\nprintf 'log' > \"${rom%.gb}.log\"\n",
        );

        let suite = RomSuite::new("sameboy-suite", TestSubsystem::Ppu)
            .with_case(
                sample_framebuffer_case()
                    .with_external_rom_root_key("TEST_ROOT")
                    .with_startup_mode(StartupMode::SkipBoot),
            )
            .with_case(
                RomTestCase::new(
                    "acid2-realboot",
                    "suite/b.gb",
                    Timeout::Frames(2),
                    PassCondition::FramebufferFixture(PathBuf::from("unused")),
                )
                .with_external_rom_root_key("TEST_ROOT")
                .with_console_model(ConsoleModel::Mgb)
                .with_startup_mode(StartupMode::RealBoot),
            );
        let report = SameBoyTesterRunner::new(&oracle_root)
            .with_rom_runner(crate::RomRunner::new().with_external_rom_root("TEST_ROOT", &rom_root))
            .with_tester_binary(&tester_binary)
            .with_image_format(SameBoyTesterImageFormat::Tga)
            .run_suite(&suite)
            .expect("suite should run");

        assert_eq!(report.suite_name, "sameboy-suite");
        assert_eq!(report.subsystem, TestSubsystem::Ppu);
        assert_eq!(report.tester_binary, tester_binary);
        assert_eq!(report.image_format, SameBoyTesterImageFormat::Tga);
        assert_eq!(report.cases.len(), 2);
        assert!(
            report.cases[0]
                .staged_rom_path
                .ends_with("acid/dmg-acid2.gb")
        );
        assert!(
            report.cases[0]
                .image_artifact_path
                .ends_with("acid/dmg-acid2.tga")
        );
        assert!(
            report.cases[0]
                .log_artifact_path
                .as_ref()
                .expect("log artifact should be present")
                .is_file()
        );
        assert!(report.cases[0].startup_mode_note.is_some());
        assert_eq!(report.cases[1].startup_mode_note, None);
    }

    #[test]
    fn run_case_surfaces_tester_process_failures() {
        let temp_dir = unique_temp_dir("tester-failure");
        let oracle_root = temp_dir.join("oracle");
        let rom_root = temp_dir.join("roms");
        let rom_path = rom_root.join("acid/dmg-acid2.gb");
        fs::create_dir_all(rom_path.parent().expect("rom path should have a parent"))
            .expect("rom dir should be creatable");
        fs::write(&rom_path, b"rom").expect("rom should be writable");

        let tester_binary = temp_dir.join("failing-tester");
        write_executable(
            &tester_binary,
            "#!/bin/sh\nprintf 'stdout-marker\\n'\nprintf 'stderr-marker\\n' >&2\nexit 7\n",
        );

        let runner = SameBoyTesterRunner::new(&oracle_root).with_rom_runner(
            crate::RomRunner::new().with_external_rom_root("TEST_ROOT", &rom_root),
        );
        let case = sample_framebuffer_case().with_external_rom_root_key("TEST_ROOT");
        let error = runner
            .with_tester_binary(&tester_binary)
            .run_case(&case, &tester_binary)
            .expect_err("failing tester should surface a tester error");
        match error {
            SameBoyTesterExecutionError::TesterFailed {
                status,
                stdout,
                stderr,
                ..
            } => {
                assert_eq!(status, Some(7));
                assert!(stdout.contains("stdout-marker"));
                assert!(stderr.contains("stderr-marker"));
            }
            other => panic!("expected TesterFailed, got {other:?}"),
        }
    }

    #[test]
    fn run_suite_rejects_invalid_suite_before_running_cases() {
        let suite = RomSuite::new("invalid", TestSubsystem::Ppu)
            .with_case(sample_framebuffer_case())
            .with_case(sample_framebuffer_case());
        let error = SameBoyTesterRunner::new("/tmp/oracle")
            .run_suite(&suite)
            .expect_err("duplicate case ids should fail suite validation");
        assert!(matches!(
            error,
            SameBoyTesterExecutionError::InvalidSuite(_)
        ));
    }

    #[test]
    fn run_case_rejects_unsupported_capture_before_resolving_roms() {
        let case = RomTestCase::new(
            "trace-only",
            "missing.gb",
            Timeout::Frames(1),
            PassCondition::TraceFixture(PathBuf::from("unused")),
        );
        let error = SameBoyTesterRunner::new("/tmp/oracle")
            .with_tester_binary("/tmp/unused")
            .run_case(&case, Path::new("/tmp/unused"))
            .expect_err("non-framebuffer cases should be rejected");
        assert!(matches!(
            error,
            SameBoyTesterExecutionError::UnsupportedCapture { .. }
        ));
    }

    #[test]
    fn ensure_tester_binary_reports_make_failures() {
        let _guard = crate::test_support::lock_env();
        let temp_dir = unique_temp_dir("build-failure");
        let sameboy_root = temp_dir.join("SameBoy");
        let tester_path = sameboy_root.join(default_sameboy_tester_relative_path());
        fs::create_dir_all(
            tester_path
                .parent()
                .expect("tester path should have a parent"),
        )
        .expect("sameboy tester dir should be creatable");

        let fake_bin_dir = temp_dir.join("bin");
        fs::create_dir_all(&fake_bin_dir).expect("bin dir should be creatable");
        let fake_make = fake_bin_dir.join("make");
        write_executable(&fake_make, "#!/bin/sh\nexit 2\n");

        let old_path = env::var_os("PATH");
        set_env_var(
            "PATH",
            format!(
                "{}:{}",
                fake_bin_dir.display(),
                old_path.as_ref().map_or_else(
                    || "".to_string(),
                    |value| value.to_string_lossy().into_owned()
                )
            ),
        );

        let error = SameBoyTesterRunner::new("/tmp/oracle")
            .with_sameboy_root(&sameboy_root)
            .with_build_if_missing(true)
            .ensure_tester_binary()
            .expect_err("failed make should surface a build error");
        assert!(matches!(
            error,
            SameBoyTesterExecutionError::BuildTesterFailed {
                status: Some(2),
                ..
            }
        ));

        if let Some(old_path) = old_path {
            set_env_var("PATH", old_path);
        } else {
            remove_env_var("PATH");
        }
    }
}
