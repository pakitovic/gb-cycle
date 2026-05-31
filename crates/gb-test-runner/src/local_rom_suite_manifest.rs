use std::fmt;
use std::path::{Path, PathBuf};
use std::{fs, io};

use gb_core::{ConsoleModel, ExecutionMode, HardwareRevision, JoypadButton, StartupMode};
use serde::Deserialize;

use crate::manifest_fixture::ManifestFixtureField;
use crate::{
    CaptureKind, CapturePlan, ExternalStimulus, ExternalStimulusAction, FailureArtifactPolicy,
    InformationalCaptureKind, MemoryByteExpectation, PassCondition, RomSuite, RomTestCase, Timeout,
};

const DEFAULT_LOCAL_ORACLE: &str = "info-framebuffer";

#[derive(Debug)]
pub enum LocalRomSuiteManifestError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
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
    suite_name: Option<String>,
    family: Option<String>,
    #[serde(rename = "case", default)]
    cases: Vec<LocalRomSuiteCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LocalRomSuiteCase {
    id: Option<String>,
    rom: PathBuf,
    console: Option<String>,
    revision: Option<String>,
    startup: Option<String>,
    mode: Option<String>,
    timeout_frames: Option<u32>,
    timeout_tcycles: Option<u64>,
    oracle: Option<String>,
    expected: Option<String>,
    fixture: Option<ManifestFixtureField>,
    check_interval_tcycles: Option<u64>,
    check_at_tcycles: Option<u64>,
    #[serde(default)]
    memory: Vec<LocalMemoryByteExpectation>,
    #[serde(rename = "stimulus", default)]
    stimuli: Vec<LocalRomStimulus>,
    #[serde(default)]
    disabled: bool,
    comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
struct LocalMemoryByteExpectation {
    address: u16,
    value: u8,
    fail_value: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalPassConditionFields {
    oracle: Option<String>,
    expected: Option<String>,
    fixture: Option<ManifestFixtureField>,
    check_interval_tcycles: Option<u64>,
    check_at_tcycles: Option<u64>,
    memory: Vec<LocalMemoryByteExpectation>,
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

    let manifest_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let suite_name = parsed
        .suite_name
        .unwrap_or_else(|| default_suite_name_for_manifest(path));
    let mut suite = RomSuite::new(suite_name);
    if let Some(family) = parsed.family {
        suite = suite.with_family(family);
    }

    for case in parsed.cases {
        if case.disabled {
            validate_disabled_case_comment(&case).map_err(|message| {
                LocalRomSuiteManifestError::Build {
                    path: path.to_path_buf(),
                    message,
                }
            })?;
            continue;
        }
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

fn validate_disabled_case_comment(case: &LocalRomSuiteCase) -> Result<(), String> {
    if case
        .comment
        .as_deref()
        .is_some_and(|comment| !comment.trim().is_empty())
    {
        return Ok(());
    }

    let case_id = case
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_case_id_for_rom_path(&case.rom));
    Err(format!(
        "disabled local case {case_id} must include a non-empty comment"
    ))
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

    let rom_path = if case.rom.is_absolute() || case.rom.starts_with(crate::TEST_ROM_STORE_DIR) {
        case.rom.clone()
    } else {
        manifest_dir.join(&case.rom)
    };
    let timeout = parse_timeout(case.timeout_frames, case.timeout_tcycles, &case_id)?;
    let pass_condition = parse_pass_condition(
        manifest_dir,
        &case_id,
        LocalPassConditionFields {
            oracle: case.oracle,
            expected: case.expected,
            fixture: case.fixture,
            check_interval_tcycles: case.check_interval_tcycles,
            check_at_tcycles: case.check_at_tcycles,
            memory: case.memory,
        },
    )?;

    let console_model = parse_console_model(case.console.as_deref().unwrap_or("dmg"), &case_id)?;
    let revision = case
        .revision
        .as_deref()
        .map(|revision| parse_revision(revision, &case_id))
        .transpose()?
        .unwrap_or_else(|| console_model.default_revision());
    if !console_model.supports_revision(revision) {
        return Err(format!(
            "case {case_id} uses revision {:?} with unsupported console {:?}",
            revision, console_model
        ));
    }

    let mut rom_case = RomTestCase::new(case_id.clone(), rom_path, timeout, pass_condition.clone())
        .with_console_model(console_model)
        .with_revision(revision)
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

fn required_fixture_path(
    fixture: Option<ManifestFixtureField>,
    case_id: &str,
    oracle: &str,
) -> Result<PathBuf, String> {
    fixture
        .ok_or_else(|| format!("case {case_id} is missing fixture for {oracle}"))?
        .into_single_path(case_id, oracle)
}

fn required_fixture_paths(
    fixture: Option<ManifestFixtureField>,
    case_id: &str,
    oracle: &str,
) -> Result<Vec<PathBuf>, String> {
    fixture
        .ok_or_else(|| format!("case {case_id} is missing fixture for {oracle}"))?
        .into_non_empty_paths(case_id, oracle)
}

fn framebuffer_fixture_pass_condition(
    manifest_dir: &Path,
    fixture: Option<ManifestFixtureField>,
    case_id: &str,
    oracle: &str,
) -> Result<PassCondition, String> {
    let fixture_paths = required_fixture_paths(fixture, case_id, oracle)?
        .into_iter()
        .map(|path| resolve_fixture_path(manifest_dir, path))
        .collect::<Vec<_>>();
    Ok(match fixture_paths.as_slice() {
        [fixture_path] => PassCondition::FramebufferFixture(fixture_path.clone()),
        _ => PassCondition::FramebufferFixtureSet(fixture_paths),
    })
}

fn parse_pass_condition(
    manifest_dir: &Path,
    case_id: &str,
    fields: LocalPassConditionFields,
) -> Result<PassCondition, String> {
    let LocalPassConditionFields {
        oracle,
        expected,
        fixture,
        check_interval_tcycles,
        check_at_tcycles,
        memory,
    } = fields;
    let oracle = oracle.as_deref().unwrap_or(DEFAULT_LOCAL_ORACLE);

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
        "memory-byte-equals" => {
            if memory.is_empty() {
                return Err(format!("case {case_id} is missing memory for {oracle}"));
            }
            Ok(PassCondition::MemoryBytesEqual(
                memory
                    .into_iter()
                    .map(|expectation| {
                        if let Some(fail_value) = expectation.fail_value {
                            MemoryByteExpectation::with_fail_value(
                                expectation.address,
                                expectation.value,
                                fail_value,
                            )
                        } else {
                            MemoryByteExpectation::new(expectation.address, expectation.value)
                        }
                    })
                    .collect(),
            ))
        }
        "info-serial" => Ok(PassCondition::Informational(
            InformationalCaptureKind::Serial,
        )),
        "info-serial-hex" => Ok(PassCondition::Informational(
            InformationalCaptureKind::SerialHex,
        )),
        "info-framebuffer" => Ok(PassCondition::Informational(
            InformationalCaptureKind::Framebuffer,
        )),
        "info-trace" => Ok(PassCondition::Informational(
            InformationalCaptureKind::Trace,
        )),
        "info-snapshot" => Ok(PassCondition::Informational(
            InformationalCaptureKind::Snapshot,
        )),
        "framebuffer-fixture" => {
            framebuffer_fixture_pass_condition(manifest_dir, fixture, case_id, oracle)
        }
        "framebuffer-fixture-until-match" => Ok(PassCondition::FramebufferFixtureUntilMatch {
            fixture_path: resolve_fixture_path(
                manifest_dir,
                required_fixture_path(fixture, case_id, oracle)?,
            ),
            check_interval_tcycles: check_interval_tcycles.unwrap_or(100_000),
            check_at_tcycles,
        }),
        "framebuffer-grayscale-fixture" => Ok(PassCondition::FramebufferGrayscaleFixture(
            resolve_fixture_path(
                manifest_dir,
                required_fixture_path(fixture, case_id, oracle)?,
            ),
        )),
        "framebuffer-rgb555-fixture" => Ok(PassCondition::FramebufferRgb555Fixture(
            resolve_fixture_path(
                manifest_dir,
                required_fixture_path(fixture, case_id, oracle)?,
            ),
        )),
        "framebuffer-rgb555-fixture-until-match" => {
            Ok(PassCondition::FramebufferRgb555FixtureUntilMatch {
                fixture_path: resolve_fixture_path(
                    manifest_dir,
                    required_fixture_path(fixture, case_id, oracle)?,
                ),
                check_interval_tcycles: check_interval_tcycles.unwrap_or(100_000),
                check_at_tcycles,
            })
        }
        "framebuffer-rgb555-grayscale-fixture" => Ok(
            PassCondition::FramebufferRgb555GrayscaleFixture(resolve_fixture_path(
                manifest_dir,
                required_fixture_path(fixture, case_id, oracle)?,
            )),
        ),
        "framebuffer-rgb555-grayscale-tolerance-fixture" => Ok(
            PassCondition::FramebufferRgb555GrayscaleToleranceFixture(resolve_fixture_path(
                manifest_dir,
                required_fixture_path(fixture, case_id, oracle)?,
            )),
        ),
        "trace-fixture" => Ok(PassCondition::TraceFixture(resolve_fixture_path(
            manifest_dir,
            required_fixture_path(fixture, case_id, oracle)?,
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
        "game-boy" | "dmg0" | "dmg" => Ok(ConsoleModel::GameBoy),
        "pocket" | "mgb" => Ok(ConsoleModel::GameBoyPocket),
        "light" => Ok(ConsoleModel::GameBoyLight),
        "color" | "cgb" => Ok(ConsoleModel::GameBoyColor),
        other => Err(format!("case {case_id} uses unsupported console {other:?}")),
    }
}

fn parse_revision(revision: &str, case_id: &str) -> Result<HardwareRevision, String> {
    match revision {
        "dmg-cpu-c" => Ok(HardwareRevision::DmgCpuC),
        "cpu-mgb" => Ok(HardwareRevision::CpuMgb),
        "cpu-cgb-c" => Ok(HardwareRevision::CpuCgbC),
        "cpu-cgb-d" => Ok(HardwareRevision::CpuCgbD),
        "cpu-cgb-e" => Ok(HardwareRevision::CpuCgbE),
        other => Err(format!(
            "case {case_id} uses unsupported hardware revision {other:?}"
        )),
    }
}

fn parse_startup_mode(startup: &str, case_id: &str) -> Result<StartupMode, String> {
    match startup {
        "skip-boot" => Ok(StartupMode::SkipBoot),
        "custom-boot" => Ok(StartupMode::CustomBoot),
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
        PassCondition::MemoryBytesEqual(_) => CapturePlan::new()
            .with_capture(CaptureKind::MemoryBytes)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::Informational(capture) => CapturePlan::new()
            .with_capture(capture.capture_kind())
            .with_capture(CaptureKind::Snapshot),
        PassCondition::FramebufferFixture(_)
        | PassCondition::FramebufferFixtureUntilMatch { .. }
        | PassCondition::FramebufferGrayscaleFixture(_)
        | PassCondition::FramebufferRgb555Fixture(_)
        | PassCondition::FramebufferRgb555FixtureUntilMatch { .. }
        | PassCondition::FramebufferRgb555GrayscaleFixture(_)
        | PassCondition::FramebufferRgb555GrayscaleToleranceFixture(_)
        | PassCondition::FramebufferFixtureSet(_) => CapturePlan::new()
            .with_capture(CaptureKind::Framebuffer)
            .with_capture(CaptureKind::Snapshot),
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
        PassCondition::MemoryBytesEqual(_) => FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::MemoryBytes)
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::Informational(capture) => FailureArtifactPolicy::new()
            .with_artifact(capture.capture_kind())
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::FramebufferFixture(_)
        | PassCondition::FramebufferFixtureUntilMatch { .. }
        | PassCondition::FramebufferGrayscaleFixture(_)
        | PassCondition::FramebufferRgb555Fixture(_)
        | PassCondition::FramebufferRgb555FixtureUntilMatch { .. }
        | PassCondition::FramebufferRgb555GrayscaleFixture(_)
        | PassCondition::FramebufferRgb555GrayscaleToleranceFixture(_)
        | PassCondition::FramebufferFixtureSet(_) => FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Framebuffer)
            .with_artifact(CaptureKind::Snapshot),
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
    use super::{
        LocalRomSuiteManifestError, capture_plan_for_pass_condition,
        failure_artifacts_for_pass_condition, load_local_rom_suite_manifest,
    };
    use crate::{
        CaptureKind, ExternalStimulusAction, InformationalCaptureKind, MemoryByteExpectation,
        MemoryTextOutputSpec, PassCondition, StimulusTime,
    };
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
        assert_eq!(suite.cases.len(), 1);

        let case = &suite.cases[0];
        assert_eq!(case.id, "tetris");
        assert_eq!(case.rom_path, rom_path);
        assert_eq!(case.console_model, ConsoleModel::GameBoy);
        assert_eq!(case.startup_mode, StartupMode::SkipBoot);
        assert_eq!(case.execution_mode, ExecutionMode::Strict);
        assert_eq!(case.timeout, crate::Timeout::Frames(450));
        assert_eq!(
            case.pass_condition,
            PassCondition::Informational(InformationalCaptureKind::Framebuffer)
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
    fn local_manifest_skips_disabled_cases_only_with_a_comment() {
        let workspace = unique_temp_dir("disabled");
        let manifest_path = write_manifest(
            &workspace,
            "disabled.toml",
            r#"

[[case]]
id = "kept"
rom = "kept.gb"
timeout_frames = 1
oracle = "info-framebuffer"

[[case]]
id = "skipped"
rom = "skipped.gb"
disabled = true
comment = "local reproduction is superseded by the curated oracle"
"#,
        );

        let suite =
            load_local_rom_suite_manifest(&manifest_path).expect("manifest should load cleanly");
        assert_eq!(suite.cases.len(), 1);
        assert_eq!(suite.cases[0].id, "kept");
    }

    #[test]
    fn local_manifest_rejects_disabled_cases_without_a_comment() {
        let workspace = unique_temp_dir("disabled-missing-comment");
        let manifest_path = write_manifest(
            &workspace,
            "disabled.toml",
            r#"

[[case]]
id = "skipped"
rom = "skipped.gb"
disabled = true
"#,
        );

        let error = load_local_rom_suite_manifest(&manifest_path)
            .expect_err("disabled case without comment should fail");
        match error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("disabled local case skipped"));
                assert!(message.contains("non-empty comment"));
            }
            other => panic!("unexpected manifest error: {other:?}"),
        }
    }

    #[test]
    fn local_manifest_rejects_invalid_timeout_and_unknown_button_values() {
        let workspace = unique_temp_dir("invalid");
        let manifest_path = write_manifest(
            &workspace,
            "invalid.toml",
            r#"

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
suite_name = "commercial-smoke"
family = "private-commercial"

[[case]]
id = "mgb-serial"
rom = "commercial/pokemon.gb"
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
oracle = "framebuffer-fixture"
fixture = ["fixtures/frame-a.png", "{absolute_fixture}"]

[[case.stimulus]]
tcycle = 2048
button = "select"
pressed = false

[[case]]
id = "cgb-grayscale-framebuffer"
rom = "commercial/stop-window.gb"
console = "cgb"
timeout_frames = 60
oracle = "framebuffer-grayscale-fixture"
fixture = "fixtures/grayscale.png"

[[case]]
id = "cgb-serial-hex"
rom = "commercial/metroid2.gb"
console = "cgb"
timeout_tcycles = 8192
oracle = "serial-hex-exact"
expected = "DEADBEEF"

[[case]]
id = "dmg-memory-byte"
rom = "commercial/memory-oracle.gb"
timeout_tcycles = 4096
oracle = "memory-byte-equals"
memory = [{{ address = 65410, value = 1 }}]

[[case]]
id = "dmg-framebuffer-until-match"
rom = "commercial/framebuffer-until-match.gb"
timeout_tcycles = 8192
oracle = "framebuffer-fixture-until-match"
fixture = "fixtures/until-match.png"

[[case]]
id = "cgb-rgb555-framebuffer-until-match"
rom = "commercial/cgb-framebuffer-until-match.gbc"
console = "cgb"
timeout_tcycles = 16384
oracle = "framebuffer-rgb555-fixture-until-match"
fixture = "fixtures/rgb555-until-match.png"
check_interval_tcycles = 2048
check_at_tcycles = 8192

[[case]]
id = "cgb-rgb555-grayscale-tolerance-framebuffer"
rom = "commercial/cgb-acid2.gbc"
console = "cgb"
timeout_frames = 60
oracle = "framebuffer-rgb555-grayscale-tolerance-fixture"
fixture = "fixtures/cgb-acid2.png"
	"#,
                absolute_fixture = absolute_fixture.display()
            ),
        );

        let suite =
            load_local_rom_suite_manifest(&manifest_path).expect("manifest should load cleanly");
        assert_eq!(suite.name, "commercial-smoke");
        assert_eq!(suite.family.as_deref(), Some("private-commercial"));
        assert_eq!(suite.cases.len(), 9);

        let serial_case = &suite.cases[0];
        assert_eq!(serial_case.console_model, ConsoleModel::GameBoyPocket);
        assert_eq!(serial_case.startup_mode, StartupMode::RealBoot);
        assert_eq!(serial_case.execution_mode, ExecutionMode::Permissive);
        assert_eq!(serial_case.timeout, crate::Timeout::TCycles(4096));
        assert_eq!(
            serial_case.rom_path,
            workspace.join("commercial/pokemon.gb")
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
        assert_eq!(trace_case.console_model, ConsoleModel::GameBoy);
        assert_eq!(trace_case.execution_mode, ExecutionMode::Experimental);
        assert_eq!(
            trace_case.pass_condition,
            PassCondition::TraceFixture(workspace.join("fixtures").join("zelda.trace"))
        );
        assert!(trace_case.capture_plan.contains(CaptureKind::Trace));

        let framebuffer_case = &suite.cases[2];
        assert_eq!(framebuffer_case.console_model, ConsoleModel::GameBoyColor);
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

        let grayscale_case = &suite.cases[3];
        assert_eq!(
            grayscale_case.pass_condition,
            PassCondition::FramebufferGrayscaleFixture(
                workspace.join("fixtures").join("grayscale.png")
            )
        );
        assert!(
            grayscale_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );

        let serial_hex_case = &suite.cases[4];
        assert_eq!(
            serial_hex_case.pass_condition,
            PassCondition::SerialHexExact("DEADBEEF".to_string())
        );
        assert!(
            serial_hex_case
                .capture_plan
                .contains(CaptureKind::SerialHex)
        );

        let memory_byte_case = &suite.cases[5];
        assert_eq!(
            memory_byte_case.pass_condition,
            PassCondition::MemoryBytesEqual(vec![MemoryByteExpectation::new(0xFF82, 0x01)])
        );
        assert!(
            memory_byte_case
                .capture_plan
                .contains(CaptureKind::MemoryBytes)
        );
        assert!(
            memory_byte_case
                .failure_artifacts
                .contains(CaptureKind::MemoryBytes)
        );

        let framebuffer_until_match_case = &suite.cases[6];
        assert_eq!(
            framebuffer_until_match_case.pass_condition,
            PassCondition::FramebufferFixtureUntilMatch {
                fixture_path: workspace.join("fixtures").join("until-match.png"),
                check_interval_tcycles: 100_000,
                check_at_tcycles: None,
            }
        );
        assert!(
            framebuffer_until_match_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert!(
            framebuffer_until_match_case
                .failure_artifacts
                .contains(CaptureKind::Framebuffer)
        );

        let rgb555_framebuffer_until_match_case = &suite.cases[7];
        assert_eq!(
            rgb555_framebuffer_until_match_case.console_model,
            ConsoleModel::GameBoyColor
        );
        assert_eq!(
            rgb555_framebuffer_until_match_case.pass_condition,
            PassCondition::FramebufferRgb555FixtureUntilMatch {
                fixture_path: workspace.join("fixtures").join("rgb555-until-match.png"),
                check_interval_tcycles: 2048,
                check_at_tcycles: Some(8192),
            }
        );
        assert!(
            rgb555_framebuffer_until_match_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert!(
            rgb555_framebuffer_until_match_case
                .failure_artifacts
                .contains(CaptureKind::Framebuffer)
        );

        let rgb555_grayscale_tolerance_case = &suite.cases[8];
        assert_eq!(
            rgb555_grayscale_tolerance_case.console_model,
            ConsoleModel::GameBoyColor
        );
        assert_eq!(
            rgb555_grayscale_tolerance_case.pass_condition,
            PassCondition::FramebufferRgb555GrayscaleToleranceFixture(
                workspace.join("fixtures").join("cgb-acid2.png")
            )
        );
        assert!(
            rgb555_grayscale_tolerance_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert!(
            rgb555_grayscale_tolerance_case
                .failure_artifacts
                .contains(CaptureKind::Framebuffer)
        );
    }

    #[test]
    fn local_manifest_reports_missing_timeout_errors() {
        let workspace = unique_temp_dir("invalid-contract");

        let missing_timeout_manifest = write_manifest(
            &workspace,
            "missing-timeout.toml",
            r#"

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
    fn local_manifest_supports_informational_oracles_and_absolute_rom_paths() {
        let workspace = unique_temp_dir("informational-oracles");
        let absolute_rom = workspace.join("absolute.gb");
        fs::create_dir_all(&workspace).expect("workspace should be creatable");
        fs::write(&absolute_rom, [0x00_u8]).expect("absolute rom should be writable");

        let manifest_path = write_manifest(
            &workspace,
            "info.toml",
            &format!(
                r#"

[[case]]
id = "serial-info"
rom = "{absolute_rom}"
timeout_frames = 1
oracle = "info-serial"

[[case]]
id = "serial-hex-info"
rom = "{absolute_rom}"
timeout_frames = 1
oracle = "info-serial-hex"

[[case]]
id = "trace-info"
rom = "{absolute_rom}"
timeout_tcycles = 4
oracle = "info-trace"

[[case]]
id = "snapshot-info"
rom = "{absolute_rom}"
timeout_tcycles = 8
oracle = "info-snapshot"
"#,
                absolute_rom = absolute_rom.display(),
            ),
        );

        let suite =
            load_local_rom_suite_manifest(&manifest_path).expect("manifest should load cleanly");
        assert_eq!(suite.cases.len(), 4);
        assert!(matches!(
            suite.cases[0].pass_condition,
            PassCondition::Informational(InformationalCaptureKind::Serial)
        ));
        assert!(matches!(
            suite.cases[1].pass_condition,
            PassCondition::Informational(InformationalCaptureKind::SerialHex)
        ));
        assert!(matches!(
            suite.cases[2].pass_condition,
            PassCondition::Informational(InformationalCaptureKind::Trace)
        ));
        assert!(matches!(
            suite.cases[3].pass_condition,
            PassCondition::Informational(InformationalCaptureKind::Snapshot)
        ));
        assert!(suite.cases.iter().all(|case| case.rom_path == absolute_rom));
    }

    #[test]
    fn local_manifest_reports_read_parse_and_metadata_errors() {
        let missing =
            load_local_rom_suite_manifest(Path::new("/definitely/missing/local-suite.toml"))
                .expect_err("missing manifest should fail");
        assert!(matches!(missing, LocalRomSuiteManifestError::Read { .. }));
        assert!(
            missing
                .to_string()
                .contains("failed to read local ROM suite manifest")
        );

        let workspace = unique_temp_dir("invalid-parse");
        let invalid_toml = write_manifest(&workspace, "invalid.toml", "[[case]\n");
        let parse_error =
            load_local_rom_suite_manifest(&invalid_toml).expect_err("invalid TOML should fail");
        assert!(matches!(
            parse_error,
            LocalRomSuiteManifestError::Parse { .. }
        ));
        assert!(
            parse_error
                .to_string()
                .contains("failed to parse local ROM suite manifest")
        );

        let unsupported_oracle = write_manifest(
            &workspace,
            "unsupported-oracle.toml",
            r#"

[[case]]
id = "broken"
rom = "broken.gb"
timeout_frames = 1
oracle = "magic"
"#,
        );
        let build_error = load_local_rom_suite_manifest(&unsupported_oracle)
            .expect_err("unsupported oracle should fail");
        assert!(matches!(
            build_error,
            LocalRomSuiteManifestError::Build { .. }
        ));
        assert!(
            build_error
                .to_string()
                .contains("failed to build local ROM suite manifest")
        );
    }

    #[test]
    fn local_manifest_rejects_missing_fixture_and_unsupported_console_metadata() {
        let workspace = unique_temp_dir("invalid-metadata");

        let missing_fixture = write_manifest(
            &workspace,
            "missing-fixture.toml",
            r#"

[[case]]
id = "framebuffer"
rom = "broken.gb"
timeout_frames = 1
oracle = "framebuffer-fixture"
"#,
        );
        let missing_fixture_error = load_local_rom_suite_manifest(&missing_fixture)
            .expect_err("missing fixture should fail");
        match missing_fixture_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("missing fixture"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let bad_console = write_manifest(
            &workspace,
            "bad-console.toml",
            r#"

[[case]]
id = "broken"
rom = "broken.gb"
console = "sgb2"
timeout_frames = 1
oracle = "info-framebuffer"
"#,
        );
        let bad_console_error =
            load_local_rom_suite_manifest(&bad_console).expect_err("bad console should fail");
        match bad_console_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("unsupported console"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let bad_stimulus = write_manifest(
            &workspace,
            "bad-stimulus.toml",
            r#"

[[case]]
id = "broken"
rom = "broken.gb"
timeout_frames = 1
oracle = "info-framebuffer"

[[case.stimulus]]
frame = 1
tcycle = 2
button = "a"
pressed = true
"#,
        );
        let bad_stimulus_error =
            load_local_rom_suite_manifest(&bad_stimulus).expect_err("bad stimulus should fail");
        match bad_stimulus_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("cannot specify both frame and tcycle"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn local_manifest_supports_serial_contains_and_rejects_missing_serial_contract_data() {
        let workspace = unique_temp_dir("serial-contracts");
        let manifest_path = write_manifest(
            &workspace,
            "serial-contains.toml",
            r#"

[[case]]
rom = "roms/serial-contract.gb"
timeout_frames = 8
oracle = "serial-contains"
expected = "Passed"
"#,
        );

        let suite =
            load_local_rom_suite_manifest(&manifest_path).expect("serial-contains manifest loads");
        let case = &suite.cases[0];
        assert_eq!(case.id, "serial-contract");
        assert_eq!(
            case.pass_condition,
            PassCondition::SerialContains("Passed".to_string())
        );
        assert!(case.capture_plan.contains(CaptureKind::Serial));
        assert!(case.capture_plan.contains(CaptureKind::Snapshot));
        assert!(case.failure_artifacts.contains(CaptureKind::Serial));
        assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));

        let missing_expected = write_manifest(
            &workspace,
            "missing-serial-expected.toml",
            r#"

[[case]]
id = "broken"
rom = "broken.gb"
timeout_frames = 1
oracle = "serial-exact"
"#,
        );
        let missing_expected_error = load_local_rom_suite_manifest(&missing_expected)
            .expect_err("serial-exact without expected should fail");
        match missing_expected_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("missing expected for serial-exact"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let missing_fixture = write_manifest(
            &workspace,
            "missing-framebuffer-fixture.toml",
            r#"

[[case]]
id = "broken"
rom = "broken.gb"
timeout_frames = 1
oracle = "framebuffer-fixture"
"#,
        );
        let missing_fixture_error = load_local_rom_suite_manifest(&missing_fixture)
            .expect_err("framebuffer-fixture without fixture should fail");
        match missing_fixture_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("missing fixture for framebuffer-fixture"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn local_manifest_rejects_remaining_contract_and_metadata_errors() {
        let workspace = unique_temp_dir("remaining-errors");

        let duplicate_ids = write_manifest(
            &workspace,
            "duplicate-ids.toml",
            r#"

[[case]]
id = "duplicate"
rom = "first.gb"
timeout_frames = 1
oracle = "info-serial"

[[case]]
id = "duplicate"
rom = "second.gb"
timeout_frames = 1
oracle = "info-snapshot"
"#,
        );
        let duplicate_ids_error = load_local_rom_suite_manifest(&duplicate_ids)
            .expect_err("duplicate case ids should fail suite validation");
        match duplicate_ids_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("invalid suite contract"));
                assert!(message.contains("DuplicateCaseId"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let blank_case_id = write_manifest(
            &workspace,
            "blank-id.toml",
            r#"

[[case]]
id = "   "
rom = "broken.gb"
timeout_frames = 1
oracle = "info-framebuffer"
"#,
        );
        let blank_case_id_error =
            load_local_rom_suite_manifest(&blank_case_id).expect_err("blank case id should fail");
        match blank_case_id_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("local case id cannot be empty"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let bad_startup = write_manifest(
            &workspace,
            "bad-startup.toml",
            r#"

[[case]]
id = "broken"
rom = "broken.gb"
startup = "warm-boot"
timeout_frames = 1
oracle = "info-framebuffer"
"#,
        );
        let bad_startup_error =
            load_local_rom_suite_manifest(&bad_startup).expect_err("bad startup should fail");
        match bad_startup_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("unsupported startup"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let bad_mode = write_manifest(
            &workspace,
            "bad-mode.toml",
            r#"

[[case]]
id = "broken"
rom = "broken.gb"
mode = "turbo"
timeout_frames = 1
oracle = "info-framebuffer"
"#,
        );
        let bad_mode_error =
            load_local_rom_suite_manifest(&bad_mode).expect_err("bad mode should fail");
        match bad_mode_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("unsupported mode"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let bad_button = write_manifest(
            &workspace,
            "bad-button.toml",
            r#"

[[case]]
id = "broken"
rom = "broken.gb"
timeout_frames = 1
oracle = "info-framebuffer"

[[case.stimulus]]
frame = 1
button = "turbo"
pressed = true
"#,
        );
        let bad_button_error =
            load_local_rom_suite_manifest(&bad_button).expect_err("bad button should fail");
        match bad_button_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("unsupported joypad button"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let missing_stimulus_time = write_manifest(
            &workspace,
            "missing-stimulus-time.toml",
            r#"

[[case]]
id = "broken"
rom = "broken.gb"
timeout_frames = 1
oracle = "info-framebuffer"

[[case.stimulus]]
button = "a"
pressed = true
"#,
        );
        let missing_stimulus_time_error = load_local_rom_suite_manifest(&missing_stimulus_time)
            .expect_err("stimulus without frame or tcycle should fail");
        match missing_stimulus_time_error {
            LocalRomSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("must specify either frame or tcycle"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn local_manifest_policy_helpers_keep_debugging_minimum_for_shared_oracles() {
        let memory_text = PassCondition::MemoryTextOutputContains {
            spec: MemoryTextOutputSpec::new(
                0xA000,
                0x80,
                0x00,
                0xA001,
                [0xDE, 0xB0, 0x61],
                0xA004,
                64,
            ),
            expected_substring: "Passed".to_string(),
        };
        let blargg = PassCondition::BlarggConsoleTextContains("Passed".to_string());
        let mooneye = PassCondition::MooneyeResult;

        let memory_plan = capture_plan_for_pass_condition(&memory_text);
        assert!(memory_plan.contains(CaptureKind::MemoryTextOutput));
        assert!(memory_plan.contains(CaptureKind::Trace));
        assert!(memory_plan.contains(CaptureKind::Snapshot));

        let blargg_plan = capture_plan_for_pass_condition(&blargg);
        assert!(blargg_plan.contains(CaptureKind::BlarggConsoleText));
        assert!(blargg_plan.contains(CaptureKind::Trace));
        assert!(blargg_plan.contains(CaptureKind::Snapshot));

        let mooneye_failures = failure_artifacts_for_pass_condition(&mooneye);
        assert!(mooneye_failures.contains(CaptureKind::Snapshot));
        assert!(mooneye_failures.contains(CaptureKind::Trace));

        let blargg_failures = failure_artifacts_for_pass_condition(&blargg);
        assert!(blargg_failures.contains(CaptureKind::BlarggConsoleText));
        assert!(blargg_failures.contains(CaptureKind::Trace));
        assert!(blargg_failures.contains(CaptureKind::Snapshot));
    }
}
