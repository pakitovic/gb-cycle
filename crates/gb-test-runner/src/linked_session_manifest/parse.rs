use std::fs;
use std::path::{Path, PathBuf};

use gb_core::{ConsoleModel, Dmg07Port, ExecutionMode, JoypadButton, StartupMode};
use serde::Deserialize;

use super::model::{
    LinkedSessionCaptureKind, LinkedSessionCapturePlan, LinkedSessionCase,
    LinkedSessionFailureArtifactPolicy, LinkedSessionParticipant, LinkedSessionPassCondition,
    LinkedSessionSuite, LinkedSessionSuiteManifestError, LinkedSessionTopology,
};
use crate::{ExternalStimulus, ExternalStimulusAction, Timeout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManifestOracle {
    TraceFixture,
    SnapshotFixture,
    ParticipantSerialHexExact,
    ParticipantTraceFixture,
    ParticipantSnapshotFixture,
    ParticipantFramebufferFixtureUntilMatch,
    InformationalTrace,
    InformationalSnapshot,
}

impl ManifestOracle {
    const TRACE_FIXTURE_NAME: &str = "linked-trace-fixture";
    const SNAPSHOT_FIXTURE_NAME: &str = "linked-snapshot-fixture";
    const PARTICIPANT_SERIAL_HEX_EXACT_NAME: &str = "linked-participant-serial-hex-exact";
    const PARTICIPANT_TRACE_FIXTURE_NAME: &str = "linked-participant-trace-fixture";
    const PARTICIPANT_SNAPSHOT_FIXTURE_NAME: &str = "linked-participant-snapshot-fixture";
    const PARTICIPANT_FRAMEBUFFER_FIXTURE_UNTIL_MATCH_NAME: &str =
        "linked-participant-framebuffer-fixture-until-match";
    const INFORMATIONAL_TRACE_NAME: &str = "info-linked-trace";
    const INFORMATIONAL_SNAPSHOT_NAME: &str = "info-linked-snapshot";
    const DEFAULT: Self = Self::InformationalTrace;

    fn manifest_name(self) -> &'static str {
        match self {
            Self::TraceFixture => Self::TRACE_FIXTURE_NAME,
            Self::SnapshotFixture => Self::SNAPSHOT_FIXTURE_NAME,
            Self::ParticipantSerialHexExact => Self::PARTICIPANT_SERIAL_HEX_EXACT_NAME,
            Self::ParticipantTraceFixture => Self::PARTICIPANT_TRACE_FIXTURE_NAME,
            Self::ParticipantSnapshotFixture => Self::PARTICIPANT_SNAPSHOT_FIXTURE_NAME,
            Self::ParticipantFramebufferFixtureUntilMatch => {
                Self::PARTICIPANT_FRAMEBUFFER_FIXTURE_UNTIL_MATCH_NAME
            }
            Self::InformationalTrace => Self::INFORMATIONAL_TRACE_NAME,
            Self::InformationalSnapshot => Self::INFORMATIONAL_SNAPSHOT_NAME,
        }
    }

    fn from_manifest_name(name: &str) -> Option<Self> {
        match name {
            Self::TRACE_FIXTURE_NAME => Some(Self::TraceFixture),
            Self::SNAPSHOT_FIXTURE_NAME => Some(Self::SnapshotFixture),
            Self::PARTICIPANT_SERIAL_HEX_EXACT_NAME => Some(Self::ParticipantSerialHexExact),
            Self::PARTICIPANT_TRACE_FIXTURE_NAME => Some(Self::ParticipantTraceFixture),
            Self::PARTICIPANT_SNAPSHOT_FIXTURE_NAME => Some(Self::ParticipantSnapshotFixture),
            Self::PARTICIPANT_FRAMEBUFFER_FIXTURE_UNTIL_MATCH_NAME => {
                Some(Self::ParticipantFramebufferFixtureUntilMatch)
            }
            Self::INFORMATIONAL_TRACE_NAME => Some(Self::InformationalTrace),
            Self::INFORMATIONAL_SNAPSHOT_NAME => Some(Self::InformationalSnapshot),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LinkedSessionSuiteManifestFile {
    suite_name: Option<String>,
    family: Option<String>,
    #[serde(rename = "session", default)]
    sessions: Vec<LinkedSessionCaseManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LinkedSessionCaseManifest {
    id: Option<String>,
    topology: Option<String>,
    timeout_frames: Option<u32>,
    timeout_tcycles: Option<u64>,
    oracle: Option<String>,
    expected: Option<String>,
    target_participant: Option<String>,
    fixture: Option<PathBuf>,
    check_interval_tcycles: Option<u64>,
    check_at_tcycles: Option<u64>,
    #[serde(rename = "participant", default)]
    participants: Vec<LinkedSessionParticipantManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkedSessionPassConditionFields {
    oracle: Option<String>,
    expected: Option<String>,
    participant: Option<String>,
    fixture: Option<PathBuf>,
    check_interval_tcycles: Option<u64>,
    check_at_tcycles: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LinkedSessionParticipantManifest {
    id: Option<String>,
    rom: PathBuf,
    console: Option<String>,
    startup: Option<String>,
    mode: Option<String>,
    adapter_port: Option<String>,
    #[serde(rename = "stimulus", default)]
    stimuli: Vec<LinkedSessionParticipantStimulus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LinkedSessionParticipantStimulus {
    frame: Option<u32>,
    tcycle: Option<u64>,
    button: String,
    pressed: bool,
}

pub fn load_linked_session_suite_manifest(
    path: &Path,
) -> Result<LinkedSessionSuite, LinkedSessionSuiteManifestError> {
    let manifest_text =
        fs::read_to_string(path).map_err(|source| LinkedSessionSuiteManifestError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let parsed: LinkedSessionSuiteManifestFile =
        toml::from_str(&manifest_text).map_err(|error| LinkedSessionSuiteManifestError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    let manifest_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let suite_name = parsed
        .suite_name
        .unwrap_or_else(|| default_suite_name_for_manifest(path));
    let mut suite = LinkedSessionSuite::new(suite_name);
    if let Some(family) = parsed.family {
        suite = suite.with_family(family);
    }

    for session in parsed.sessions {
        let built_session =
            build_session_from_manifest(manifest_dir, session).map_err(|message| {
                LinkedSessionSuiteManifestError::Build {
                    path: path.to_path_buf(),
                    message,
                }
            })?;
        suite.push_session(built_session);
    }

    suite
        .validate()
        .map_err(|error| LinkedSessionSuiteManifestError::Build {
            path: path.to_path_buf(),
            message: format!("invalid linked-session suite contract: {error:?}"),
        })?;

    Ok(suite)
}

fn build_session_from_manifest(
    manifest_dir: &Path,
    session: LinkedSessionCaseManifest,
) -> Result<LinkedSessionCase, String> {
    let session_id = session.id.unwrap_or_default().trim().to_string();
    if session_id.is_empty() {
        return Err("linked session id cannot be empty".to_string());
    }

    let topology = parse_topology(
        session
            .topology
            .as_deref()
            .unwrap_or(LinkedSessionTopology::Dmg04.manifest_name()),
        &session_id,
    )?;
    let timeout = parse_timeout(session.timeout_frames, session.timeout_tcycles, &session_id)?;
    let pass_condition = parse_pass_condition(
        manifest_dir,
        &session_id,
        LinkedSessionPassConditionFields {
            oracle: session.oracle,
            expected: session.expected,
            participant: session.target_participant,
            fixture: session.fixture,
            check_interval_tcycles: session.check_interval_tcycles,
            check_at_tcycles: session.check_at_tcycles,
        },
    )?;

    let mut built_session = LinkedSessionCase::new(
        session_id.clone(),
        topology,
        timeout,
        pass_condition.clone(),
    )
    .with_capture_plan(capture_plan_for_pass_condition(&pass_condition))
    .with_failure_artifacts(failure_artifacts_for_pass_condition(&pass_condition));

    for participant in session.participants {
        built_session = built_session.with_participant(build_participant_from_manifest(
            manifest_dir,
            &session_id,
            participant,
        )?);
    }

    Ok(built_session)
}

fn build_participant_from_manifest(
    manifest_dir: &Path,
    session_id: &str,
    participant: LinkedSessionParticipantManifest,
) -> Result<LinkedSessionParticipant, String> {
    let participant_id = participant.id.unwrap_or_default().trim().to_string();
    if participant_id.is_empty() {
        return Err(format!(
            "linked session {session_id} participant id cannot be empty"
        ));
    }

    let rom_path = if participant.rom.is_absolute()
        || participant.rom.starts_with(crate::TEST_ROM_STORE_DIR)
    {
        participant.rom.clone()
    } else {
        manifest_dir.join(&participant.rom)
    };

    let mut built_participant = LinkedSessionParticipant::new(participant_id.clone(), rom_path)
        .with_console_model(parse_console_model(
            participant.console.as_deref().unwrap_or("dmg"),
            &participant_id,
        )?)
        .with_startup_mode(parse_startup_mode(
            participant.startup.as_deref().unwrap_or("skip-boot"),
            &participant_id,
        )?)
        .with_execution_mode(parse_execution_mode(
            participant.mode.as_deref().unwrap_or("strict"),
            &participant_id,
        )?);

    if let Some(adapter_port) = participant.adapter_port {
        built_participant = built_participant
            .with_adapter_port(parse_adapter_port(&adapter_port, &participant_id)?);
    }

    for stimulus in participant.stimuli {
        built_participant =
            built_participant.with_external_stimulus(parse_stimulus(stimulus, &participant_id)?);
    }

    Ok(built_participant)
}

fn parse_topology(topology: &str, session_id: &str) -> Result<LinkedSessionTopology, String> {
    LinkedSessionTopology::from_manifest_name(topology).ok_or_else(|| {
        format!("linked session {session_id} uses unsupported topology {topology:?}")
    })
}

fn parse_adapter_port(adapter_port: &str, participant_id: &str) -> Result<Dmg07Port, String> {
    Dmg07Port::from_manifest_name(adapter_port).ok_or_else(|| {
        format!(
            "linked participant {participant_id} uses unsupported adapter_port {adapter_port:?}"
        )
    })
}

fn parse_timeout(
    timeout_frames: Option<u32>,
    timeout_tcycles: Option<u64>,
    session_id: &str,
) -> Result<Timeout, String> {
    match (timeout_frames, timeout_tcycles) {
        (Some(frames), None) => Ok(Timeout::Frames(frames)),
        (None, Some(tcycles)) => Ok(Timeout::TCycles(tcycles)),
        (Some(_), Some(_)) => Err(format!(
            "linked session {session_id} cannot specify both timeout_frames and timeout_tcycles"
        )),
        (None, None) => Err(format!(
            "linked session {session_id} must specify either timeout_frames or timeout_tcycles"
        )),
    }
}

fn parse_pass_condition(
    manifest_dir: &Path,
    session_id: &str,
    fields: LinkedSessionPassConditionFields,
) -> Result<LinkedSessionPassCondition, String> {
    let oracle_name = fields
        .oracle
        .as_deref()
        .unwrap_or(ManifestOracle::DEFAULT.manifest_name());
    let oracle = ManifestOracle::from_manifest_name(oracle_name).ok_or_else(|| {
        format!("linked session {session_id} uses unsupported oracle {oracle_name:?}")
    })?;

    match oracle {
        ManifestOracle::TraceFixture => Ok(LinkedSessionPassCondition::TraceFixture(
            resolve_fixture_path(
                manifest_dir,
                fields
                    .fixture
                    .clone()
                    .ok_or_else(|| missing_oracle_field(session_id, oracle, "fixture"))?,
            ),
        )),
        ManifestOracle::SnapshotFixture => Ok(LinkedSessionPassCondition::SnapshotFixture(
            resolve_fixture_path(
                manifest_dir,
                fields
                    .fixture
                    .ok_or_else(|| missing_oracle_field(session_id, oracle, "fixture"))?,
            ),
        )),
        ManifestOracle::ParticipantSerialHexExact => {
            let participant_id = fields
                .participant
                .ok_or_else(|| missing_oracle_field(session_id, oracle, "participant"))?;
            let expected = fields
                .expected
                .ok_or_else(|| missing_oracle_field(session_id, oracle, "expected"))?;
            Ok(LinkedSessionPassCondition::ParticipantSerialHexExact {
                participant_id,
                expected,
            })
        }
        ManifestOracle::ParticipantTraceFixture => {
            let participant_id = fields
                .participant
                .ok_or_else(|| missing_oracle_field(session_id, oracle, "participant"))?;
            let fixture_path = resolve_fixture_path(
                manifest_dir,
                fields
                    .fixture
                    .ok_or_else(|| missing_oracle_field(session_id, oracle, "fixture"))?,
            );
            Ok(LinkedSessionPassCondition::ParticipantTraceFixture {
                participant_id,
                fixture_path,
            })
        }
        ManifestOracle::ParticipantSnapshotFixture => {
            let participant_id = fields
                .participant
                .ok_or_else(|| missing_oracle_field(session_id, oracle, "participant"))?;
            let fixture_path = resolve_fixture_path(
                manifest_dir,
                fields
                    .fixture
                    .ok_or_else(|| missing_oracle_field(session_id, oracle, "fixture"))?,
            );
            Ok(LinkedSessionPassCondition::ParticipantSnapshotFixture {
                participant_id,
                fixture_path,
            })
        }
        ManifestOracle::ParticipantFramebufferFixtureUntilMatch => {
            let participant_id = fields
                .participant
                .ok_or_else(|| missing_oracle_field(session_id, oracle, "participant"))?;
            let fixture_path = resolve_fixture_path(
                manifest_dir,
                fields
                    .fixture
                    .ok_or_else(|| missing_oracle_field(session_id, oracle, "fixture"))?,
            );
            Ok(
                LinkedSessionPassCondition::ParticipantFramebufferFixtureUntilMatch {
                    participant_id,
                    fixture_path,
                    check_interval_tcycles: fields.check_interval_tcycles.unwrap_or(100_000),
                    check_at_tcycles: fields.check_at_tcycles,
                },
            )
        }
        ManifestOracle::InformationalTrace => Ok(LinkedSessionPassCondition::Informational(
            LinkedSessionCaptureKind::Trace,
        )),
        ManifestOracle::InformationalSnapshot => Ok(LinkedSessionPassCondition::Informational(
            LinkedSessionCaptureKind::Snapshot,
        )),
    }
}

fn missing_oracle_field(session_id: &str, oracle: ManifestOracle, field: &str) -> String {
    format!(
        "linked session {session_id} is missing {field} for {}",
        oracle.manifest_name()
    )
}

fn parse_console_model(console: &str, participant_id: &str) -> Result<ConsoleModel, String> {
    match console {
        "game-boy" | "dmg0" | "dmg" => Ok(ConsoleModel::GameBoy),
        "pocket" | "mgb" => Ok(ConsoleModel::GameBoyPocket),
        "light" => Ok(ConsoleModel::GameBoyLight),
        "color" | "cgb" => Ok(ConsoleModel::GameBoyColor),
        other => Err(format!(
            "participant {participant_id} uses unsupported console {other:?}"
        )),
    }
}

fn parse_startup_mode(startup: &str, participant_id: &str) -> Result<StartupMode, String> {
    match startup {
        "skip-boot" => Ok(StartupMode::SkipBoot),
        "custom-boot" => Ok(StartupMode::CustomBoot),
        "real-boot" => Ok(StartupMode::RealBoot),
        other => Err(format!(
            "participant {participant_id} uses unsupported startup {other:?}"
        )),
    }
}

fn parse_execution_mode(mode: &str, participant_id: &str) -> Result<ExecutionMode, String> {
    match mode {
        "strict" => Ok(ExecutionMode::Strict),
        "permissive" => Ok(ExecutionMode::Permissive),
        "experimental" => Ok(ExecutionMode::Experimental),
        other => Err(format!(
            "participant {participant_id} uses unsupported mode {other:?}"
        )),
    }
}

fn parse_stimulus(
    stimulus: LinkedSessionParticipantStimulus,
    participant_id: &str,
) -> Result<ExternalStimulus, String> {
    let button = parse_joypad_button(&stimulus.button, participant_id)?;
    let action = ExternalStimulusAction::JoypadSetButton {
        button,
        pressed: stimulus.pressed,
    };

    match (stimulus.frame, stimulus.tcycle) {
        (Some(frame), None) => Ok(ExternalStimulus::at_frame(frame, action)),
        (None, Some(tcycle)) => Ok(ExternalStimulus::at_t_cycle(tcycle, action)),
        (Some(_), Some(_)) => Err(format!(
            "participant {participant_id} cannot specify both frame and tcycle for one stimulus"
        )),
        (None, None) => Err(format!(
            "participant {participant_id} must specify either frame or tcycle for each stimulus"
        )),
    }
}

fn parse_joypad_button(button: &str, participant_id: &str) -> Result<JoypadButton, String> {
    match button {
        "a" => Ok(JoypadButton::A),
        "b" => Ok(JoypadButton::B),
        "start" => Ok(JoypadButton::Start),
        "select" => Ok(JoypadButton::Select),
        "up" => Ok(JoypadButton::Up),
        "down" => Ok(JoypadButton::Down),
        "left" => Ok(JoypadButton::Left),
        "right" => Ok(JoypadButton::Right),
        other => Err(format!(
            "participant {participant_id} uses unsupported joypad button {other:?}"
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

pub(super) fn capture_plan_for_pass_condition(
    pass_condition: &LinkedSessionPassCondition,
) -> LinkedSessionCapturePlan {
    match pass_condition {
        LinkedSessionPassCondition::TraceFixture(_)
        | LinkedSessionPassCondition::SnapshotFixture(_)
        | LinkedSessionPassCondition::ParticipantTraceFixture { .. }
        | LinkedSessionPassCondition::ParticipantSnapshotFixture { .. }
        | LinkedSessionPassCondition::ParticipantFramebufferFixtureUntilMatch { .. }
        | LinkedSessionPassCondition::ParticipantSerialHexExact { .. } => {
            LinkedSessionCapturePlan::debugging_minimum_for(pass_condition)
        }
        LinkedSessionPassCondition::Informational(capture) => LinkedSessionCapturePlan::new()
            .with_capture(*capture)
            .with_capture(LinkedSessionCaptureKind::Snapshot),
    }
}

pub(super) fn failure_artifacts_for_pass_condition(
    pass_condition: &LinkedSessionPassCondition,
) -> LinkedSessionFailureArtifactPolicy {
    match pass_condition {
        LinkedSessionPassCondition::TraceFixture(_)
        | LinkedSessionPassCondition::SnapshotFixture(_)
        | LinkedSessionPassCondition::ParticipantTraceFixture { .. }
        | LinkedSessionPassCondition::ParticipantSnapshotFixture { .. }
        | LinkedSessionPassCondition::ParticipantFramebufferFixtureUntilMatch { .. }
        | LinkedSessionPassCondition::ParticipantSerialHexExact { .. } => {
            LinkedSessionFailureArtifactPolicy::debugging_minimum_for(pass_condition)
        }
        LinkedSessionPassCondition::Informational(capture) => {
            LinkedSessionFailureArtifactPolicy::new()
                .with_artifact(*capture)
                .with_artifact(LinkedSessionCaptureKind::Snapshot)
        }
    }
}

fn default_suite_name_for_manifest(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("linked-session-suite")
        .to_string()
}
