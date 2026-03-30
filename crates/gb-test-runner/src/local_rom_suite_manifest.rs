use std::fmt;
use std::path::{Path, PathBuf};
use std::{fs, io};

use gb_core::{ConsoleModel, ExecutionMode, JoypadButton, StartupMode};
use serde::Deserialize;

use crate::{
    CaptureKind, CapturePlan, ExternalStimulus, ExternalStimulusAction, FailureArtifactPolicy,
    PassCondition, RomSuite, RomTestCase, TestSubsystem, Timeout,
};

const SUPPORTED_LOCAL_ROM_SUITE_MANIFEST_VERSION: u32 = 1;
const DEFAULT_LOCAL_ORACLE: &str = "info-framebuffer";

#[derive(Debug)]
pub enum LocalRomSuiteManifestError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
    UnsupportedVersion { path: PathBuf, version: u32 },
    Build { path: PathBuf, message: String },
}

impl fmt::Display for LocalRomSuiteManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    f,
                    "failed to read local ROM suite manifest {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, message } => {
                write!(
                    f,
                    "failed to parse local ROM suite manifest {}: {message}",
                    path.display()
                )
            }
            Self::UnsupportedVersion { path, version } => {
                write!(
                    f,
                    "local ROM suite manifest {} uses unsupported version {}",
                    path.display(),
                    version
                )
            }
            Self::Build { path, message } => {
                write!(
                    f,
                    "failed to build local ROM suite manifest {}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for LocalRomSuiteManifestError {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LocalRomSuiteManifestFile {
    version: u32,
    suite_name: Option<String>,
    family: Option<String>,
    subsystem: Option<String>,
    #[serde(rename = "case", default)]
    cases: Vec<LocalRomSuiteCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LocalRomSuiteCase {
    id: Option<String>,
    rom: PathBuf,
    external_rom_root_key: Option<String>,
    console: Option<String>,
    startup: Option<String>,
    mode: Option<String>,
    timeout_frames: Option<u32>,
    timeout_tcycles: Option<u64>,
    oracle: Option<String>,
    expected: Option<String>,
    fixture: Option<PathBuf>,
    #[serde(default)]
    fixtures: Vec<PathBuf>,
    #[serde(rename = "stimulus", default)]
    stimuli: Vec<LocalRomStimulus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LocalRomStimulus {
    frame: Option<u32>,
    tcycle: Option<u64>,
    button: String,
    pressed: bool,
}

pub fn load_local_rom_suite_manifest(path: &Path) -> Result<RomSuite, LocalRomSuiteManifestError> {
    let manifest_text =
        fs::read_to_string(path).map_err(|source| LocalRomSuiteManifestError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let parsed: LocalRomSuiteManifestFile =
        toml::from_str(&manifest_text).map_err(|error| LocalRomSuiteManifestError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if parsed.version != SUPPORTED_LOCAL_ROM_SUITE_MANIFEST_VERSION {
        return Err(LocalRomSuiteManifestError::UnsupportedVersion {
            path: path.to_path_buf(),
            version: parsed.version,
        });
    }

    let manifest_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let suite_name = parsed
        .suite_name
        .unwrap_or_else(|| default_suite_name_for_manifest(path));
    let subsystem = parse_subsystem(parsed.subsystem.as_deref().unwrap_or("cross-subsystem"))
        .map_err(|message| LocalRomSuiteManifestError::Build {
            path: path.to_path_buf(),
            message,
        })?;

    let mut suite = RomSuite::new(suite_name, subsystem);
    if let Some(family) = parsed.family {
        suite = suite.with_family(family);
    }

    for case in parsed.cases {
        let built_case = build_case_from_manifest(manifest_dir, case).map_err(|message| {
            LocalRomSuiteManifestError::Build {
                path: path.to_path_buf(),
                message,
            }
        })?;
        suite.push_case(built_case);
    }

    suite
        .validate()
        .map_err(|error| LocalRomSuiteManifestError::Build {
            path: path.to_path_buf(),
            message: format!("invalid suite contract: {error:?}"),
        })?;

    Ok(suite)
}

fn build_case_from_manifest(
    manifest_dir: &Path,
    case: LocalRomSuiteCase,
) -> Result<RomTestCase, String> {
    let case_id = case
        .id
        .unwrap_or_else(|| default_case_id_for_rom_path(&case.rom))
        .trim()
        .to_string();
    if case_id.is_empty() {
        return Err("local case id cannot be empty".to_string());
    }

    let rom_path = if case.external_rom_root_key.is_some() || case.rom.is_absolute() {
        case.rom.clone()
    } else {
        manifest_dir.join(&case.rom)
    };
    let timeout = parse_timeout(case.timeout_frames, case.timeout_tcycles, &case_id)?;
    let pass_condition = parse_pass_condition(
        manifest_dir,
        &case_id,
        case.oracle.as_deref().unwrap_or(DEFAULT_LOCAL_ORACLE),
        case.expected,
        case.fixture,
        case.fixtures,
    )?;

    let mut rom_case = RomTestCase::new(case_id.clone(), rom_path, timeout, pass_condition.clone())
        .with_console_model(parse_console_model(
            case.console.as_deref().unwrap_or("dmg"),
            &case_id,
        )?)
        .with_startup_mode(parse_startup_mode(
            case.startup.as_deref().unwrap_or("skip-boot"),
            &case_id,
        )?)
        .with_execution_mode(parse_execution_mode(
            case.mode.as_deref().unwrap_or("strict"),
            &case_id,
        )?)
        .with_capture_plan(capture_plan_for_pass_condition(&pass_condition))
        .with_failure_artifacts(failure_artifacts_for_pass_condition(&pass_condition));

    if let Some(external_rom_root_key) = case.external_rom_root_key {
        rom_case = rom_case.with_external_rom_root_key(external_rom_root_key);
    }

    for stimulus in case.stimuli {
        rom_case = rom_case.with_external_stimulus(parse_stimulus(stimulus, &case_id)?);
    }

    Ok(rom_case)
}

fn parse_timeout(
    timeout_frames: Option<u32>,
    timeout_tcycles: Option<u64>,
    case_id: &str,
) -> Result<Timeout, String> {
    match (timeout_frames, timeout_tcycles) {
        (Some(frames), None) => Ok(Timeout::Frames(frames)),
        (None, Some(tcycles)) => Ok(Timeout::TCycles(tcycles)),
        (Some(_), Some(_)) => Err(format!(
            "case {case_id} cannot specify both timeout_frames and timeout_tcycles"
        )),
        (None, None) => Err(format!(
            "case {case_id} must specify either timeout_frames or timeout_tcycles"
        )),
    }
}

fn parse_pass_condition(
    manifest_dir: &Path,
    case_id: &str,
    oracle: &str,
    expected: Option<String>,
    fixture: Option<PathBuf>,
    fixtures: Vec<PathBuf>,
) -> Result<PassCondition, String> {
    match oracle {
        "serial-contains" => Ok(PassCondition::SerialContains(
            expected.ok_or_else(|| format!("case {case_id} is missing expected for {oracle}"))?,
        )),
        "serial-exact" => {
            Ok(PassCondition::SerialExact(expected.ok_or_else(|| {
                format!("case {case_id} is missing expected for {oracle}")
            })?))
        }
        "serial-hex-exact" => Ok(PassCondition::SerialHexExact(
            expected.ok_or_else(|| format!("case {case_id} is missing expected for {oracle}"))?,
        )),
        "info-serial" => Ok(PassCondition::Informational(CaptureKind::Serial)),
        "info-serial-hex" => Ok(PassCondition::Informational(CaptureKind::SerialHex)),
        "info-framebuffer" => Ok(PassCondition::Informational(CaptureKind::Framebuffer)),
        "info-trace" => Ok(PassCondition::Informational(CaptureKind::Trace)),
        "info-snapshot" => Ok(PassCondition::Informational(CaptureKind::Snapshot)),
        "framebuffer-fixture" => Ok(PassCondition::FramebufferFixture(resolve_fixture_path(
            manifest_dir,
            fixture.ok_or_else(|| format!("case {case_id} is missing fixture for {oracle}"))?,
        ))),
        "framebuffer-fixture-set" => {
            if fixtures.is_empty() {
                return Err(format!("case {case_id} is missing fixtures for {oracle}"));
            }
            Ok(PassCondition::FramebufferFixtureSet(
                fixtures
                    .into_iter()
                    .map(|path| resolve_fixture_path(manifest_dir, path))
                    .collect(),
            ))
        }
        "trace-fixture" => Ok(PassCondition::TraceFixture(resolve_fixture_path(
            manifest_dir,
            fixture.ok_or_else(|| format!("case {case_id} is missing fixture for {oracle}"))?,
        ))),
        other => Err(format!("case {case_id} uses unsupported oracle {other:?}")),
    }
}

fn parse_stimulus(stimulus: LocalRomStimulus, case_id: &str) -> Result<ExternalStimulus, String> {
    let button = parse_joypad_button(&stimulus.button, case_id)?;
    let action = ExternalStimulusAction::JoypadSetButton {
        button,
        pressed: stimulus.pressed,
    };

    match (stimulus.frame, stimulus.tcycle) {
        (Some(frame), None) => Ok(ExternalStimulus::at_frame(frame, action)),
        (None, Some(tcycle)) => Ok(ExternalStimulus::at_t_cycle(tcycle, action)),
        (Some(_), Some(_)) => Err(format!(
            "case {case_id} cannot specify both frame and tcycle for one stimulus"
        )),
        (None, None) => Err(format!(
            "case {case_id} must specify either frame or tcycle for each stimulus"
        )),
    }
}

fn parse_console_model(console: &str, case_id: &str) -> Result<ConsoleModel, String> {
    match console {
        "dmg0" => Ok(ConsoleModel::Dmg0),
        "dmg" => Ok(ConsoleModel::Dmg),
        "mgb" => Ok(ConsoleModel::Mgb),
        "cgb" => Ok(ConsoleModel::Cgb),
        other => Err(format!("case {case_id} uses unsupported console {other:?}")),
    }
}

fn parse_startup_mode(startup: &str, case_id: &str) -> Result<StartupMode, String> {
    match startup {
        "skip-boot" => Ok(StartupMode::SkipBoot),
        "real-boot" => Ok(StartupMode::RealBoot),
        other => Err(format!("case {case_id} uses unsupported startup {other:?}")),
    }
}

fn parse_execution_mode(mode: &str, case_id: &str) -> Result<ExecutionMode, String> {
    match mode {
        "strict" => Ok(ExecutionMode::Strict),
        "permissive" => Ok(ExecutionMode::Permissive),
        "experimental" => Ok(ExecutionMode::Experimental),
        other => Err(format!("case {case_id} uses unsupported mode {other:?}")),
    }
}

fn parse_subsystem(subsystem: &str) -> Result<TestSubsystem, String> {
    match subsystem {
        "cpu" => Ok(TestSubsystem::Cpu),
        "interrupts" => Ok(TestSubsystem::Interrupts),
        "bus" => Ok(TestSubsystem::Bus),
        "cartridge" => Ok(TestSubsystem::Cartridge),
        "timer" => Ok(TestSubsystem::Timer),
        "ppu" => Ok(TestSubsystem::Ppu),
        "dma" => Ok(TestSubsystem::Dma),
        "apu" => Ok(TestSubsystem::Apu),
        "boot" => Ok(TestSubsystem::Boot),
        "joypad" => Ok(TestSubsystem::Joypad),
        "serial" => Ok(TestSubsystem::Serial),
        "scheduler" => Ok(TestSubsystem::Scheduler),
        "cross-subsystem" => Ok(TestSubsystem::CrossSubsystem),
        other => Err(format!("unsupported subsystem {other:?}")),
    }
}

fn parse_joypad_button(button: &str, case_id: &str) -> Result<JoypadButton, String> {
    match button {
        "right" => Ok(JoypadButton::Right),
        "left" => Ok(JoypadButton::Left),
        "up" => Ok(JoypadButton::Up),
        "down" => Ok(JoypadButton::Down),
        "a" => Ok(JoypadButton::A),
        "b" => Ok(JoypadButton::B),
        "select" => Ok(JoypadButton::Select),
        "start" => Ok(JoypadButton::Start),
        other => Err(format!(
            "case {case_id} uses unsupported joypad button {other:?}"
        )),
    }
}

fn resolve_fixture_path(manifest_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    }
}

fn capture_plan_for_pass_condition(pass_condition: &PassCondition) -> CapturePlan {
    match pass_condition {
        PassCondition::SerialContains(_) | PassCondition::SerialExact(_) => CapturePlan::new()
            .with_capture(CaptureKind::Serial)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::SerialHexExact(_) => CapturePlan::new()
            .with_capture(CaptureKind::SerialHex)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::Informational(capture) => CapturePlan::new()
            .with_capture(*capture)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::FramebufferFixture(_) | PassCondition::FramebufferFixtureSet(_) => {
            CapturePlan::new()
                .with_capture(CaptureKind::Framebuffer)
                .with_capture(CaptureKind::Snapshot)
        }
        PassCondition::TraceFixture(_) => CapturePlan::debugging_minimum_for(pass_condition),
        PassCondition::MemoryTextOutputContains { .. }
        | PassCondition::BlarggConsoleTextContains(_)
        | PassCondition::MooneyeResult => CapturePlan::debugging_minimum_for(pass_condition),
    }
}

fn failure_artifacts_for_pass_condition(pass_condition: &PassCondition) -> FailureArtifactPolicy {
    match pass_condition {
        PassCondition::SerialContains(_) | PassCondition::SerialExact(_) => {
            FailureArtifactPolicy::new()
                .with_artifact(CaptureKind::Serial)
                .with_artifact(CaptureKind::Snapshot)
        }
        PassCondition::SerialHexExact(_) => FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::SerialHex)
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::Informational(capture) => FailureArtifactPolicy::new()
            .with_artifact(*capture)
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::FramebufferFixture(_) | PassCondition::FramebufferFixtureSet(_) => {
            FailureArtifactPolicy::new()
                .with_artifact(CaptureKind::Framebuffer)
                .with_artifact(CaptureKind::Snapshot)
        }
        PassCondition::TraceFixture(_) => {
            FailureArtifactPolicy::debugging_minimum_for(pass_condition)
        }
        PassCondition::MemoryTextOutputContains { .. }
        | PassCondition::BlarggConsoleTextContains(_)
        | PassCondition::MooneyeResult => {
            FailureArtifactPolicy::debugging_minimum_for(pass_condition)
        }
    }
}

fn default_suite_name_for_manifest(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("local-rom-suite")
        .to_string()
}

fn default_case_id_for_rom_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("local-rom")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{LocalRomSuiteManifestError, load_local_rom_suite_manifest};
    use crate::{CaptureKind, ExternalStimulusAction, PassCondition, StimulusTime, TestSubsystem};
    use gb_core::{ConsoleModel, ExecutionMode, JoypadButton, StartupMode};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gb-cycle-local-rom-suite-manifest-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn write_manifest(dir: &Path, name: &str, body: &str) -> PathBuf {
        let manifest_path = dir.join(name);
        fs::create_dir_all(dir).expect("manifest parent should be creatable");
        fs::write(&manifest_path, body).expect("manifest should be writable");
        manifest_path
    }

    #[test]
    fn local_manifest_defaults_to_framebuffer_info_case_and_resolves_relative_paths() {
        let workspace = unique_temp_dir("defaults");
        let rom_path = workspace.join("roms").join("tetris.gb");
        fs::create_dir_all(
            rom_path
                .parent()
                .expect("temporary ROM path should have a parent"),
        )
        .expect("temporary ROM parent should be creatable");
        fs::write(&rom_path, [0x00_u8]).expect("temporary ROM should be writable");
        let manifest_path = write_manifest(
            &workspace,
            "tetris-start.toml",
            r#"
version = 1

[[case]]
rom = "roms/tetris.gb"
timeout_frames = 450

[[case.stimulus]]
frame = 300
button = "start"
pressed = true

[[case.stimulus]]
frame = 302
button = "start"
pressed = false
"#,
        );

        let suite =
            load_local_rom_suite_manifest(&manifest_path).expect("manifest should load cleanly");
        assert_eq!(suite.name, "tetris-start");
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 1);

        let case = &suite.cases[0];
        assert_eq!(case.id, "tetris");
        assert_eq!(case.rom_path, rom_path);
        assert_eq!(case.console_model, ConsoleModel::Dmg);
        assert_eq!(case.startup_mode, StartupMode::SkipBoot);
        assert_eq!(case.execution_mode, ExecutionMode::Strict);
        assert_eq!(case.timeout, crate::Timeout::Frames(450));
        assert_eq!(
            case.pass_condition,
            PassCondition::Informational(CaptureKind::Framebuffer)
        );
        assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
        assert!(case.capture_plan.contains(CaptureKind::Snapshot));
        assert_eq!(case.external_stimuli.stimuli().len(), 2);
        assert_eq!(
            case.external_stimuli.stimuli()[0].when,
            StimulusTime::Frame(300)
        );
        assert_eq!(
            case.external_stimuli.stimuli()[0].action,
            ExternalStimulusAction::JoypadSetButton {
                button: JoypadButton::Start,
                pressed: true,
            }
        );
    }

    #[test]
    fn local_manifest_rejects_invalid_timeout_and_unknown_button_values() {
        let workspace = unique_temp_dir("invalid");
        let manifest_path = write_manifest(
            &workspace,
            "invalid.toml",
            r#"
version = 1

[[case]]
id = "broken"
rom = "roms/broken.gb"
timeout_frames = 1
timeout_tcycles = 2

[[case.stimulus]]
frame = 1
button = "turbo"
pressed = true
"#,
        );

        let error = load_local_rom_suite_manifest(&manifest_path)
            .expect_err("invalid local manifest should be rejected");
        match error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("cannot specify both timeout_frames and timeout_tcycles"));
            }
            other => panic!("unexpected manifest error: {other:?}"),
        }
    }

    #[test]
    fn local_manifest_supports_explicit_suite_metadata_tcycle_stimuli_and_fixture_oracles() {
        let workspace = unique_temp_dir("explicit-contract");
        let absolute_fixture = workspace.join("fixtures").join("absolute-frame.png");
        fs::create_dir_all(
            absolute_fixture
                .parent()
                .expect("fixture path should have a parent"),
        )
        .expect("fixture parent should be creatable");
        fs::write(&absolute_fixture, []).expect("absolute fixture placeholder should be writable");

        let manifest_path = write_manifest(
            &workspace,
            "commercial-smoke.toml",
            &format!(
                r#"
version = 1
suite_name = "commercial-smoke"
family = "local-commercial"
subsystem = "joypad"

[[case]]
id = "mgb-serial"
rom = "commercial/pokemon.gb"
external_rom_root_key = "GB_CYCLE_LOCAL_COMMERCIAL_ROOT"
console = "mgb"
startup = "real-boot"
mode = "permissive"
timeout_tcycles = 4096
oracle = "serial-exact"
expected = "OK"

[[case.stimulus]]
tcycle = 512
button = "a"
pressed = true

[[case]]
id = "dmg0-trace"
rom = "commercial/zelda.gb"
console = "dmg0"
startup = "skip-boot"
mode = "experimental"
timeout_frames = 12
oracle = "trace-fixture"
fixture = "fixtures/zelda.trace"

[[case]]
id = "cgb-framebuffer"
rom = "commercial/links-awakening.gb"
console = "cgb"
timeout_frames = 24
oracle = "framebuffer-fixture-set"
fixtures = ["fixtures/frame-a.png", "{absolute_fixture}"]

[[case.stimulus]]
tcycle = 2048
button = "select"
pressed = false

[[case]]
id = "cgb-serial-hex"
rom = "commercial/metroid2.gb"
console = "cgb"
timeout_tcycles = 8192
oracle = "serial-hex-exact"
expected = "DEADBEEF"
"#,
                absolute_fixture = absolute_fixture.display()
            ),
        );

        let suite =
            load_local_rom_suite_manifest(&manifest_path).expect("manifest should load cleanly");
        assert_eq!(suite.name, "commercial-smoke");
        assert_eq!(suite.family.as_deref(), Some("local-commercial"));
        assert_eq!(suite.subsystem, TestSubsystem::Joypad);
        assert_eq!(suite.cases.len(), 4);

        let serial_case = &suite.cases[0];
        assert_eq!(serial_case.console_model, ConsoleModel::Mgb);
        assert_eq!(serial_case.startup_mode, StartupMode::RealBoot);
        assert_eq!(serial_case.execution_mode, ExecutionMode::Permissive);
        assert_eq!(serial_case.timeout, crate::Timeout::TCycles(4096));
        assert_eq!(
            serial_case.external_rom_root_key.as_deref(),
            Some("GB_CYCLE_LOCAL_COMMERCIAL_ROOT")
        );
        assert_eq!(
            serial_case.pass_condition,
            PassCondition::SerialExact("OK".to_string())
        );
        assert!(serial_case.capture_plan.contains(CaptureKind::Serial));
        assert_eq!(
            serial_case.external_stimuli.stimuli()[0].when,
            StimulusTime::TCycle(512)
        );
        assert_eq!(
            serial_case.external_stimuli.stimuli()[0].action,
            ExternalStimulusAction::JoypadSetButton {
                button: JoypadButton::A,
                pressed: true,
            }
        );

        let trace_case = &suite.cases[1];
        assert_eq!(trace_case.console_model, ConsoleModel::Dmg0);
        assert_eq!(trace_case.execution_mode, ExecutionMode::Experimental);
        assert_eq!(
            trace_case.pass_condition,
            PassCondition::TraceFixture(workspace.join("fixtures").join("zelda.trace"))
        );
        assert!(trace_case.capture_plan.contains(CaptureKind::Trace));

        let framebuffer_case = &suite.cases[2];
        assert_eq!(framebuffer_case.console_model, ConsoleModel::Cgb);
        assert_eq!(
            framebuffer_case.pass_condition,
            PassCondition::FramebufferFixtureSet(vec![
                workspace.join("fixtures").join("frame-a.png"),
                absolute_fixture.clone(),
            ])
        );
        assert!(
            framebuffer_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert_eq!(
            framebuffer_case.external_stimuli.stimuli()[0].action,
            ExternalStimulusAction::JoypadSetButton {
                button: JoypadButton::Select,
                pressed: false,
            }
        );

        let serial_hex_case = &suite.cases[3];
        assert_eq!(
            serial_hex_case.pass_condition,
            PassCondition::SerialHexExact("DEADBEEF".to_string())
        );
        assert!(
            serial_hex_case
                .capture_plan
                .contains(CaptureKind::SerialHex)
        );
    }

    #[test]
    fn local_manifest_reports_unsupported_subsystem_and_missing_timeout_errors() {
        let workspace = unique_temp_dir("invalid-contract");

        let unsupported_subsystem_manifest = write_manifest(
            &workspace,
            "unsupported-subsystem.toml",
            r#"
version = 1
subsystem = "video"

[[case]]
rom = "commercial/tetris.gb"
timeout_frames = 1
"#,
        );
        let unsupported_subsystem = load_local_rom_suite_manifest(&unsupported_subsystem_manifest)
            .expect_err("unsupported subsystem should fail");
        match unsupported_subsystem {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("unsupported subsystem"));
            }
            other => panic!("unexpected manifest error: {other:?}"),
        }

        let missing_timeout_manifest = write_manifest(
            &workspace,
            "missing-timeout.toml",
            r#"
version = 1

[[case]]
id = "broken"
rom = "commercial/tetris.gb"
oracle = "info-serial"
"#,
        );
        let missing_timeout = load_local_rom_suite_manifest(&missing_timeout_manifest)
            .expect_err("missing timeout should fail");
        match missing_timeout {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("must specify either timeout_frames or timeout_tcycles"));
            }
            other => panic!("unexpected manifest error: {other:?}"),
        }
    }

    #[test]
    fn local_manifest_rejects_unsupported_version() {
        let workspace = unique_temp_dir("version");
        let manifest_path = write_manifest(
            &workspace,
            "unsupported.toml",
            r#"
version = 9
"#,
        );

        let error = load_local_rom_suite_manifest(&manifest_path)
            .expect_err("unsupported manifest version should fail");
        assert!(matches!(
            error,
            LocalRomSuiteManifestError::UnsupportedVersion { version: 9, .. }
        ));
    }
}
