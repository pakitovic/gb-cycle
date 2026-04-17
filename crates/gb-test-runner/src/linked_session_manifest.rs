use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::{fs, io};

use gb_core::{ConsoleModel, ExecutionMode, JoypadButton, StartupMode};
use serde::Deserialize;

use crate::{
    ExternalStimulus, ExternalStimulusAction, ExternalStimulusPlan, TestSubsystem, Timeout,
};

const SUPPORTED_LINKED_SESSION_SUITE_MANIFEST_VERSION: u32 = 1;
const DEFAULT_LINKED_SESSION_ORACLE: &str = "info-linked-trace";

#[derive(Debug)]
pub enum LinkedSessionSuiteManifestError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
    UnsupportedVersion { path: PathBuf, version: u32 },
    Build { path: PathBuf, message: String },
}

impl fmt::Display for LinkedSessionSuiteManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    f,
                    "failed to read linked-session suite manifest {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, message } => {
                write!(
                    f,
                    "failed to parse linked-session suite manifest {}: {message}",
                    path.display()
                )
            }
            Self::UnsupportedVersion { path, version } => {
                write!(
                    f,
                    "linked-session suite manifest {} uses unsupported version {}",
                    path.display(),
                    version
                )
            }
            Self::Build { path, message } => {
                write!(
                    f,
                    "failed to build linked-session suite manifest {}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for LinkedSessionSuiteManifestError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkedSessionTopology {
    Dmg04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LinkedSessionCaptureKind {
    Trace,
    Snapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedSessionPassCondition {
    TraceFixture(PathBuf),
    SnapshotFixture(PathBuf),
    Informational(LinkedSessionCaptureKind),
}

impl LinkedSessionPassCondition {
    pub fn required_capture(&self) -> LinkedSessionCaptureKind {
        match self {
            Self::TraceFixture(_) => LinkedSessionCaptureKind::Trace,
            Self::SnapshotFixture(_) => LinkedSessionCaptureKind::Snapshot,
            Self::Informational(capture) => *capture,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkedSessionCapturePlan {
    captures: BTreeSet<LinkedSessionCaptureKind>,
}

impl LinkedSessionCapturePlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn debugging_minimum_for(pass_condition: &LinkedSessionPassCondition) -> Self {
        Self::new()
            .with_capture(pass_condition.required_capture())
            .with_capture(LinkedSessionCaptureKind::Snapshot)
    }

    pub fn with_capture(mut self, capture: LinkedSessionCaptureKind) -> Self {
        self.captures.insert(capture);
        self
    }

    pub fn contains(&self, capture: LinkedSessionCaptureKind) -> bool {
        self.captures.contains(&capture)
    }

    pub fn captures(&self) -> &BTreeSet<LinkedSessionCaptureKind> {
        &self.captures
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkedSessionFailureArtifactPolicy {
    retained: BTreeSet<LinkedSessionCaptureKind>,
}

impl LinkedSessionFailureArtifactPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn debugging_minimum_for(pass_condition: &LinkedSessionPassCondition) -> Self {
        Self::new()
            .with_artifact(pass_condition.required_capture())
            .with_artifact(LinkedSessionCaptureKind::Snapshot)
    }

    pub fn with_artifact(mut self, artifact: LinkedSessionCaptureKind) -> Self {
        self.retained.insert(artifact);
        self
    }

    pub fn contains(&self, artifact: LinkedSessionCaptureKind) -> bool {
        self.retained.contains(&artifact)
    }

    pub fn retained(&self) -> &BTreeSet<LinkedSessionCaptureKind> {
        &self.retained
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedSessionParticipantValidationError {
    EmptyParticipantId,
    MissingRomPath,
    EmptyExternalRomRootKey,
    DuplicateExternalStimulus(ExternalStimulus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedSessionParticipant {
    pub id: String,
    pub rom_path: PathBuf,
    pub external_rom_root_key: Option<String>,
    pub console_model: ConsoleModel,
    pub startup_mode: StartupMode,
    pub execution_mode: ExecutionMode,
    pub external_stimuli: ExternalStimulusPlan,
}

impl LinkedSessionParticipant {
    pub fn new(id: impl Into<String>, rom_path: impl Into<PathBuf>) -> Self {
        Self {
            id: id.into(),
            rom_path: rom_path.into(),
            external_rom_root_key: None,
            console_model: ConsoleModel::Dmg,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: ExecutionMode::Strict,
            external_stimuli: ExternalStimulusPlan::new(),
        }
    }

    pub fn with_external_rom_root_key(mut self, external_rom_root_key: impl Into<String>) -> Self {
        self.external_rom_root_key = Some(external_rom_root_key.into());
        self
    }

    pub fn with_console_model(mut self, console_model: ConsoleModel) -> Self {
        self.console_model = console_model;
        self
    }

    pub fn with_startup_mode(mut self, startup_mode: StartupMode) -> Self {
        self.startup_mode = startup_mode;
        self
    }

    pub fn with_execution_mode(mut self, execution_mode: ExecutionMode) -> Self {
        self.execution_mode = execution_mode;
        self
    }

    pub fn with_external_stimuli(mut self, external_stimuli: ExternalStimulusPlan) -> Self {
        self.external_stimuli = external_stimuli;
        self
    }

    pub fn with_external_stimulus(mut self, stimulus: ExternalStimulus) -> Self {
        self.external_stimuli = self.external_stimuli.with_stimulus(stimulus);
        self
    }

    pub fn validate(&self) -> Result<(), LinkedSessionParticipantValidationError> {
        if self.id.trim().is_empty() {
            return Err(LinkedSessionParticipantValidationError::EmptyParticipantId);
        }

        if self.rom_path.as_os_str().is_empty() {
            return Err(LinkedSessionParticipantValidationError::MissingRomPath);
        }

        if self
            .external_rom_root_key
            .as_deref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(LinkedSessionParticipantValidationError::EmptyExternalRomRootKey);
        }

        for (index, stimulus) in self.external_stimuli.stimuli().iter().enumerate() {
            if self.external_stimuli.stimuli()[index + 1..].contains(stimulus) {
                return Err(
                    LinkedSessionParticipantValidationError::DuplicateExternalStimulus(*stimulus),
                );
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedSessionCaseValidationError {
    EmptySessionId,
    InvalidTimeout,
    MissingRequiredCapture(LinkedSessionCaptureKind),
    MissingRequiredFailureArtifact(LinkedSessionCaptureKind),
    ArtifactNotCaptured(LinkedSessionCaptureKind),
    MissingFailureArtifacts,
    UnsupportedTopologyParticipantCount {
        topology: LinkedSessionTopology,
        count: usize,
    },
    DuplicateParticipantId(String),
    InvalidParticipant {
        participant_id: String,
        error: LinkedSessionParticipantValidationError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedSessionCase {
    pub id: String,
    pub topology: LinkedSessionTopology,
    pub participants: Vec<LinkedSessionParticipant>,
    pub timeout: Timeout,
    pub pass_condition: LinkedSessionPassCondition,
    pub capture_plan: LinkedSessionCapturePlan,
    pub failure_artifacts: LinkedSessionFailureArtifactPolicy,
}

impl LinkedSessionCase {
    pub fn new(
        id: impl Into<String>,
        topology: LinkedSessionTopology,
        timeout: Timeout,
        pass_condition: LinkedSessionPassCondition,
    ) -> Self {
        let capture_plan = LinkedSessionCapturePlan::debugging_minimum_for(&pass_condition);
        let failure_artifacts =
            LinkedSessionFailureArtifactPolicy::debugging_minimum_for(&pass_condition);

        Self {
            id: id.into(),
            topology,
            participants: Vec::new(),
            timeout,
            pass_condition,
            capture_plan,
            failure_artifacts,
        }
    }

    pub fn with_participant(mut self, participant: LinkedSessionParticipant) -> Self {
        self.participants.push(participant);
        self
    }

    pub fn with_capture_plan(mut self, capture_plan: LinkedSessionCapturePlan) -> Self {
        self.capture_plan = capture_plan;
        self
    }

    pub fn with_failure_artifacts(
        mut self,
        failure_artifacts: LinkedSessionFailureArtifactPolicy,
    ) -> Self {
        self.failure_artifacts = failure_artifacts;
        self
    }

    pub fn validate(&self) -> Result<(), LinkedSessionCaseValidationError> {
        if self.id.trim().is_empty() {
            return Err(LinkedSessionCaseValidationError::EmptySessionId);
        }

        if !self.timeout.is_valid() {
            return Err(LinkedSessionCaseValidationError::InvalidTimeout);
        }

        let required_capture = self.pass_condition.required_capture();
        if !self.capture_plan.contains(required_capture) {
            return Err(LinkedSessionCaseValidationError::MissingRequiredCapture(
                required_capture,
            ));
        }

        if self.failure_artifacts.retained().is_empty() {
            return Err(LinkedSessionCaseValidationError::MissingFailureArtifacts);
        }

        if !self.failure_artifacts.contains(required_capture) {
            return Err(
                LinkedSessionCaseValidationError::MissingRequiredFailureArtifact(required_capture),
            );
        }

        for artifact in self.failure_artifacts.retained() {
            if !self.capture_plan.contains(*artifact) {
                return Err(LinkedSessionCaseValidationError::ArtifactNotCaptured(
                    *artifact,
                ));
            }
        }

        let participant_count = self.participants.len();
        match self.topology {
            LinkedSessionTopology::Dmg04 if participant_count != 2 => {
                return Err(
                    LinkedSessionCaseValidationError::UnsupportedTopologyParticipantCount {
                        topology: self.topology,
                        count: participant_count,
                    },
                );
            }
            LinkedSessionTopology::Dmg04 => {}
        }

        let mut seen_participant_ids = BTreeSet::new();
        for participant in &self.participants {
            if !seen_participant_ids.insert(participant.id.clone()) {
                return Err(LinkedSessionCaseValidationError::DuplicateParticipantId(
                    participant.id.clone(),
                ));
            }

            if let Err(error) = participant.validate() {
                return Err(LinkedSessionCaseValidationError::InvalidParticipant {
                    participant_id: participant.id.clone(),
                    error,
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedSessionSuiteValidationError {
    EmptySuiteName,
    DuplicateSessionId(String),
    InvalidSession {
        session_id: String,
        error: LinkedSessionCaseValidationError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedSessionSuite {
    pub name: String,
    pub family: Option<String>,
    pub subsystem: TestSubsystem,
    pub sessions: Vec<LinkedSessionCase>,
}

impl LinkedSessionSuite {
    pub fn new(name: impl Into<String>, subsystem: TestSubsystem) -> Self {
        Self {
            name: name.into(),
            family: None,
            subsystem,
            sessions: Vec::new(),
        }
    }

    pub fn with_family(mut self, family: impl Into<String>) -> Self {
        self.family = Some(family.into());
        self
    }

    pub fn with_session(mut self, session: LinkedSessionCase) -> Self {
        self.sessions.push(session);
        self
    }

    pub fn push_session(&mut self, session: LinkedSessionCase) {
        self.sessions.push(session);
    }

    pub fn validate(&self) -> Result<(), LinkedSessionSuiteValidationError> {
        if self.name.trim().is_empty() {
            return Err(LinkedSessionSuiteValidationError::EmptySuiteName);
        }

        let mut seen_session_ids = BTreeSet::new();
        for session in &self.sessions {
            if !seen_session_ids.insert(session.id.clone()) {
                return Err(LinkedSessionSuiteValidationError::DuplicateSessionId(
                    session.id.clone(),
                ));
            }

            if let Err(error) = session.validate() {
                return Err(LinkedSessionSuiteValidationError::InvalidSession {
                    session_id: session.id.clone(),
                    error,
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LinkedSessionSuiteManifestFile {
    version: u32,
    suite_name: Option<String>,
    family: Option<String>,
    subsystem: Option<String>,
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
    fixture: Option<PathBuf>,
    #[serde(rename = "participant", default)]
    participants: Vec<LinkedSessionParticipantManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LinkedSessionParticipantManifest {
    id: Option<String>,
    rom: PathBuf,
    external_rom_root_key: Option<String>,
    console: Option<String>,
    startup: Option<String>,
    mode: Option<String>,
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

    if parsed.version != SUPPORTED_LINKED_SESSION_SUITE_MANIFEST_VERSION {
        return Err(LinkedSessionSuiteManifestError::UnsupportedVersion {
            path: path.to_path_buf(),
            version: parsed.version,
        });
    }

    let manifest_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let suite_name = parsed
        .suite_name
        .unwrap_or_else(|| default_suite_name_for_manifest(path));
    let subsystem = parse_subsystem(parsed.subsystem.as_deref().unwrap_or("cross-subsystem"))
        .map_err(|message| LinkedSessionSuiteManifestError::Build {
            path: path.to_path_buf(),
            message,
        })?;

    let mut suite = LinkedSessionSuite::new(suite_name, subsystem);
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

    let topology = parse_topology(session.topology.as_deref().unwrap_or("dmg04"), &session_id)?;
    let timeout = parse_timeout(session.timeout_frames, session.timeout_tcycles, &session_id)?;
    let pass_condition = parse_pass_condition(
        manifest_dir,
        &session_id,
        session
            .oracle
            .as_deref()
            .unwrap_or(DEFAULT_LINKED_SESSION_ORACLE),
        session.fixture,
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

    let rom_path = if participant.external_rom_root_key.is_some() || participant.rom.is_absolute() {
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

    if let Some(external_rom_root_key) = participant.external_rom_root_key {
        built_participant = built_participant.with_external_rom_root_key(external_rom_root_key);
    }

    for stimulus in participant.stimuli {
        built_participant =
            built_participant.with_external_stimulus(parse_stimulus(stimulus, &participant_id)?);
    }

    Ok(built_participant)
}

fn parse_topology(topology: &str, session_id: &str) -> Result<LinkedSessionTopology, String> {
    match topology {
        "dmg04" => Ok(LinkedSessionTopology::Dmg04),
        other => Err(format!(
            "linked session {session_id} uses unsupported topology {other:?}"
        )),
    }
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
    oracle: &str,
    fixture: Option<PathBuf>,
) -> Result<LinkedSessionPassCondition, String> {
    match oracle {
        "linked-trace-fixture" => Ok(LinkedSessionPassCondition::TraceFixture(
            resolve_fixture_path(
                manifest_dir,
                fixture.clone().ok_or_else(|| {
                    format!("linked session {session_id} is missing fixture for {oracle}")
                })?,
            ),
        )),
        "linked-snapshot-fixture" => Ok(LinkedSessionPassCondition::SnapshotFixture(
            resolve_fixture_path(
                manifest_dir,
                fixture.ok_or_else(|| {
                    format!("linked session {session_id} is missing fixture for {oracle}")
                })?,
            ),
        )),
        "info-linked-trace" => Ok(LinkedSessionPassCondition::Informational(
            LinkedSessionCaptureKind::Trace,
        )),
        "info-linked-snapshot" => Ok(LinkedSessionPassCondition::Informational(
            LinkedSessionCaptureKind::Snapshot,
        )),
        other => Err(format!(
            "linked session {session_id} uses unsupported oracle {other:?}"
        )),
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

fn parse_console_model(console: &str, participant_id: &str) -> Result<ConsoleModel, String> {
    match console {
        "dmg0" => Ok(ConsoleModel::Dmg0),
        "dmg" => Ok(ConsoleModel::Dmg),
        "mgb" => Ok(ConsoleModel::Mgb),
        "cgb" => Ok(ConsoleModel::Cgb),
        other => Err(format!(
            "participant {participant_id} uses unsupported console {other:?}"
        )),
    }
}

fn parse_startup_mode(startup: &str, participant_id: &str) -> Result<StartupMode, String> {
    match startup {
        "skip-boot" => Ok(StartupMode::SkipBoot),
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

fn capture_plan_for_pass_condition(
    pass_condition: &LinkedSessionPassCondition,
) -> LinkedSessionCapturePlan {
    match pass_condition {
        LinkedSessionPassCondition::TraceFixture(_)
        | LinkedSessionPassCondition::SnapshotFixture(_) => {
            LinkedSessionCapturePlan::debugging_minimum_for(pass_condition)
        }
        LinkedSessionPassCondition::Informational(capture) => LinkedSessionCapturePlan::new()
            .with_capture(*capture)
            .with_capture(LinkedSessionCaptureKind::Snapshot),
    }
}

fn failure_artifacts_for_pass_condition(
    pass_condition: &LinkedSessionPassCondition,
) -> LinkedSessionFailureArtifactPolicy {
    match pass_condition {
        LinkedSessionPassCondition::TraceFixture(_)
        | LinkedSessionPassCondition::SnapshotFixture(_) => {
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

#[cfg(test)]
mod tests {
    use super::{
        LinkedSessionCaptureKind, LinkedSessionPassCondition, LinkedSessionSuiteManifestError,
        LinkedSessionTopology, capture_plan_for_pass_condition,
        failure_artifacts_for_pass_condition, load_linked_session_suite_manifest,
    };
    use crate::{ExternalStimulusAction, StimulusTime, TestSubsystem};
    use gb_core::{ConsoleModel, ExecutionMode, JoypadButton, StartupMode};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gb-cycle-linked-session-suite-manifest-{}-{}-{}",
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
    fn linked_session_manifest_defaults_to_trace_info_and_resolves_relative_rom_paths() {
        let workspace = unique_temp_dir("defaults");
        let left_rom = workspace.join("roms").join("left.gb");
        let right_rom = workspace.join("roms").join("right.gb");
        fs::create_dir_all(
            left_rom
                .parent()
                .expect("left ROM path should have a parent"),
        )
        .expect("temporary ROM parent should be creatable");
        fs::write(&left_rom, [0x00_u8]).expect("left rom should be writable");
        fs::write(&right_rom, [0x00_u8]).expect("right rom should be writable");

        let manifest_path = write_manifest(
            &workspace,
            "dmg04-defaults.toml",
            r#"
version = 1

[[session]]
id = "basic-exchange"
timeout_tcycles = 8192

  [[session.participant]]
  id = "left"
  rom = "roms/left.gb"

    [[session.participant.stimulus]]
    tcycle = 128
    button = "a"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "roms/right.gb"

    [[session.participant.stimulus]]
    frame = 2
    button = "b"
    pressed = false
"#,
        );

        let suite = load_linked_session_suite_manifest(&manifest_path)
            .expect("linked manifest should load cleanly");

        assert_eq!(suite.name, "dmg04-defaults");
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.sessions.len(), 1);

        let session = &suite.sessions[0];
        assert_eq!(session.id, "basic-exchange");
        assert_eq!(session.topology, LinkedSessionTopology::Dmg04);
        assert_eq!(session.timeout, crate::Timeout::TCycles(8192));
        assert_eq!(
            session.pass_condition,
            LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace)
        );
        assert!(
            session
                .capture_plan
                .contains(LinkedSessionCaptureKind::Trace)
        );
        assert!(
            session
                .capture_plan
                .contains(LinkedSessionCaptureKind::Snapshot)
        );
        assert_eq!(session.participants.len(), 2);

        let left = &session.participants[0];
        assert_eq!(left.id, "left");
        assert_eq!(left.rom_path, left_rom);
        assert_eq!(left.console_model, ConsoleModel::Dmg);
        assert_eq!(left.startup_mode, StartupMode::SkipBoot);
        assert_eq!(left.execution_mode, ExecutionMode::Strict);
        assert_eq!(left.external_stimuli.stimuli().len(), 1);
        assert_eq!(
            left.external_stimuli.stimuli()[0].when,
            StimulusTime::TCycle(128)
        );
        assert_eq!(
            left.external_stimuli.stimuli()[0].action,
            ExternalStimulusAction::JoypadSetButton {
                button: JoypadButton::A,
                pressed: true,
            }
        );

        let right = &session.participants[1];
        assert_eq!(right.id, "right");
        assert_eq!(right.rom_path, right_rom);
        assert_eq!(
            right.external_stimuli.stimuli()[0].when,
            StimulusTime::Frame(2)
        );
        assert_eq!(
            right.external_stimuli.stimuli()[0].action,
            ExternalStimulusAction::JoypadSetButton {
                button: JoypadButton::B,
                pressed: false,
            }
        );
    }

    #[test]
    fn linked_session_manifest_supports_explicit_metadata_and_trace_fixture_oracles() {
        let workspace = unique_temp_dir("explicit-contract");
        let absolute_fixture = workspace.join("fixtures").join("linked.trace");
        fs::create_dir_all(
            absolute_fixture
                .parent()
                .expect("fixture path should have a parent"),
        )
        .expect("fixture parent should be creatable");
        fs::write(&absolute_fixture, []).expect("absolute fixture should be writable");

        let manifest_path = write_manifest(
            &workspace,
            "linked-commercial-smoke.toml",
            &r#"
version = 1
suite_name = "linked-commercial-smoke"
family = "serial-ext"
subsystem = "serial"

[[session]]
id = "pokemon-trade"
topology = "dmg04"
timeout_frames = 90
oracle = "linked-trace-fixture"
fixture = "fixtures/pokemon.trace"

  [[session.participant]]
  id = "left"
  rom = "commercial/pokemon-red.gb"
  external_rom_root_key = "GB_CYCLE_LOCAL_COMMERCIAL_ROOT"
  console = "mgb"
  startup = "real-boot"
  mode = "permissive"

    [[session.participant.stimulus]]
    tcycle = 512
    button = "start"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "commercial/pokemon-blue.gb"
  console = "cgb"
  mode = "experimental"

[[session]]
id = "info-snapshot"
topology = "dmg04"
timeout_tcycles = 1024
oracle = "linked-snapshot-fixture"
fixture = "fixtures/snapshot.txt"

  [[session.participant]]
  id = "left2"
  rom = "commercial/left.gb"

  [[session.participant]]
  id = "right2"
  rom = "commercial/right.gb"
"#
            .replace(
                "fixtures/pokemon.trace",
                &absolute_fixture.display().to_string(),
            ),
        );

        let suite = load_linked_session_suite_manifest(&manifest_path)
            .expect("linked manifest should load cleanly");

        assert_eq!(suite.name, "linked-commercial-smoke");
        assert_eq!(suite.family.as_deref(), Some("serial-ext"));
        assert_eq!(suite.subsystem, TestSubsystem::Serial);
        assert_eq!(suite.sessions.len(), 2);

        let trace_session = &suite.sessions[0];
        assert_eq!(trace_session.timeout, crate::Timeout::Frames(90));
        assert_eq!(
            trace_session.pass_condition,
            LinkedSessionPassCondition::TraceFixture(absolute_fixture.clone())
        );
        assert!(
            trace_session
                .capture_plan
                .contains(LinkedSessionCaptureKind::Trace)
        );

        let left = &trace_session.participants[0];
        assert_eq!(left.console_model, ConsoleModel::Mgb);
        assert_eq!(left.startup_mode, StartupMode::RealBoot);
        assert_eq!(left.execution_mode, ExecutionMode::Permissive);
        assert_eq!(
            left.external_rom_root_key.as_deref(),
            Some("GB_CYCLE_LOCAL_COMMERCIAL_ROOT")
        );
        assert_eq!(
            left.external_stimuli.stimuli()[0].action,
            ExternalStimulusAction::JoypadSetButton {
                button: JoypadButton::Start,
                pressed: true,
            }
        );

        let right = &trace_session.participants[1];
        assert_eq!(right.console_model, ConsoleModel::Cgb);
        assert_eq!(right.execution_mode, ExecutionMode::Experimental);

        let info_session = &suite.sessions[1];
        assert_eq!(info_session.timeout, crate::Timeout::TCycles(1024));
        assert_eq!(
            info_session.pass_condition,
            LinkedSessionPassCondition::SnapshotFixture(
                workspace.join("fixtures").join("snapshot.txt")
            )
        );
        assert!(
            info_session
                .capture_plan
                .contains(LinkedSessionCaptureKind::Snapshot)
        );
    }

    #[test]
    fn linked_session_manifest_rejects_invalid_timeout_topology_and_participant_count() {
        let workspace = unique_temp_dir("invalid-timeout-topology");

        let bad_timeout = write_manifest(
            &workspace,
            "bad-timeout.toml",
            r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
timeout_tcycles = 2

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
        );
        let bad_timeout_error = load_linked_session_suite_manifest(&bad_timeout)
            .expect_err("invalid linked timeout should fail");
        match bad_timeout_error {
            LinkedSessionSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("cannot specify both timeout_frames and timeout_tcycles"));
            }
            other => panic!("unexpected linked manifest error: {other:?}"),
        }

        let unsupported_topology = write_manifest(
            &workspace,
            "unsupported-topology.toml",
            r#"
version = 1

[[session]]
id = "broken"
topology = "dmg07"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
        );
        let unsupported_topology_error = load_linked_session_suite_manifest(&unsupported_topology)
            .expect_err("unsupported topology should fail");
        match unsupported_topology_error {
            LinkedSessionSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("unsupported topology"));
            }
            other => panic!("unexpected linked manifest error: {other:?}"),
        }

        let bad_participant_count = write_manifest(
            &workspace,
            "bad-participant-count.toml",
            r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "solo"
  rom = "solo.gb"
"#,
        );
        let bad_participant_count_error =
            load_linked_session_suite_manifest(&bad_participant_count)
                .expect_err("dmg04 should require exactly two participants");
        match bad_participant_count_error {
            LinkedSessionSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("UnsupportedTopologyParticipantCount"));
            }
            other => panic!("unexpected linked manifest error: {other:?}"),
        }
    }

    #[test]
    fn linked_session_manifest_rejects_duplicate_ids_and_invalid_participant_metadata() {
        let workspace = unique_temp_dir("duplicate-ids");

        let duplicate_session_ids = write_manifest(
            &workspace,
            "duplicate-sessions.toml",
            r#"
version = 1

[[session]]
id = "duplicate"
timeout_frames = 1

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"

[[session]]
id = "duplicate"
timeout_frames = 1

  [[session.participant]]
  id = "left2"
  rom = "left2.gb"

  [[session.participant]]
  id = "right2"
  rom = "right2.gb"
"#,
        );
        let duplicate_session_ids_error =
            load_linked_session_suite_manifest(&duplicate_session_ids)
                .expect_err("duplicate linked session ids should fail");
        match duplicate_session_ids_error {
            LinkedSessionSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("DuplicateSessionId"));
            }
            other => panic!("unexpected linked manifest error: {other:?}"),
        }

        let duplicate_participant_ids = write_manifest(
            &workspace,
            "duplicate-participants.toml",
            r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1

  [[session.participant]]
  id = "duplicate"
  rom = "left.gb"

  [[session.participant]]
  id = "duplicate"
  rom = "right.gb"
"#,
        );
        let duplicate_participant_ids_error =
            load_linked_session_suite_manifest(&duplicate_participant_ids)
                .expect_err("duplicate participant ids should fail");
        match duplicate_participant_ids_error {
            LinkedSessionSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("DuplicateParticipantId"));
            }
            other => panic!("unexpected linked manifest error: {other:?}"),
        }

        let invalid_participant_metadata = write_manifest(
            &workspace,
            "invalid-participant-metadata.toml",
            r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "linked-trace-fixture"
fixture = "fixtures/trace.txt"

  [[session.participant]]
  id = "left"
  rom = "left.gb"
  console = "sgb2"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
        );
        let invalid_participant_metadata_error =
            load_linked_session_suite_manifest(&invalid_participant_metadata)
                .expect_err("bad console should fail");
        match invalid_participant_metadata_error {
            LinkedSessionSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("unsupported console"));
            }
            other => panic!("unexpected linked manifest error: {other:?}"),
        }
    }

    #[test]
    fn linked_session_manifest_reports_read_parse_and_remaining_stimulus_errors() {
        let missing = load_linked_session_suite_manifest(Path::new(
            "/definitely/missing/linked-session-suite.toml",
        ))
        .expect_err("missing linked manifest should fail");
        assert!(matches!(
            missing,
            LinkedSessionSuiteManifestError::Read { .. }
        ));

        let workspace = unique_temp_dir("invalid-parse");
        let invalid_toml = write_manifest(&workspace, "invalid.toml", "version = 1\n[[session]\n");
        let parse_error = load_linked_session_suite_manifest(&invalid_toml)
            .expect_err("invalid linked TOML should fail");
        assert!(matches!(
            parse_error,
            LinkedSessionSuiteManifestError::Parse { .. }
        ));

        let bad_stimulus = write_manifest(
            &workspace,
            "bad-stimulus.toml",
            r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

    [[session.participant.stimulus]]
    frame = 1
    tcycle = 2
    button = "a"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
        );
        let bad_stimulus_error = load_linked_session_suite_manifest(&bad_stimulus)
            .expect_err("bad linked stimulus should fail");
        match bad_stimulus_error {
            LinkedSessionSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("cannot specify both frame and tcycle"));
            }
            other => panic!("unexpected linked manifest error: {other:?}"),
        }

        let missing_stimulus_time = write_manifest(
            &workspace,
            "missing-stimulus-time.toml",
            r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

    [[session.participant.stimulus]]
    button = "a"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
        );
        let missing_stimulus_time_error =
            load_linked_session_suite_manifest(&missing_stimulus_time)
                .expect_err("stimulus without frame or tcycle should fail");
        match missing_stimulus_time_error {
            LinkedSessionSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("must specify either frame or tcycle"));
            }
            other => panic!("unexpected linked manifest error: {other:?}"),
        }

        let bad_button = write_manifest(
            &workspace,
            "bad-button.toml",
            r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

    [[session.participant.stimulus]]
    frame = 1
    button = "turbo"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
        );
        let bad_button_error = load_linked_session_suite_manifest(&bad_button)
            .expect_err("unsupported linked button should fail");
        match bad_button_error {
            LinkedSessionSuiteManifestError::Build { message, .. } => {
                assert!(message.contains("unsupported joypad button"));
            }
            other => panic!("unexpected linked manifest error: {other:?}"),
        }
    }

    #[test]
    fn linked_session_manifest_policy_helpers_keep_trace_and_snapshot_debugging_minimum() {
        let trace_fixture =
            LinkedSessionPassCondition::TraceFixture(PathBuf::from("expected.trace"));
        let snapshot_info =
            LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot);

        let trace_plan = capture_plan_for_pass_condition(&trace_fixture);
        assert!(trace_plan.contains(LinkedSessionCaptureKind::Trace));
        assert!(trace_plan.contains(LinkedSessionCaptureKind::Snapshot));

        let snapshot_plan = capture_plan_for_pass_condition(&snapshot_info);
        assert!(snapshot_plan.contains(LinkedSessionCaptureKind::Snapshot));

        let trace_failures = failure_artifacts_for_pass_condition(&trace_fixture);
        assert!(trace_failures.contains(LinkedSessionCaptureKind::Trace));
        assert!(trace_failures.contains(LinkedSessionCaptureKind::Snapshot));
    }
}
