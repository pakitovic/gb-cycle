use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::PathBuf;

use gb_core::{ConsoleModel, Dmg07Port, ExecutionMode, StartupMode};

use crate::{ExternalStimulus, ExternalStimulusPlan, TestSubsystem, Timeout};

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
    Dmg07,
}

impl LinkedSessionTopology {
    pub(crate) const DMG04_MANIFEST_NAME: &str = "dmg04";
    pub(crate) const DMG07_MANIFEST_NAME: &str = "dmg07";

    pub(crate) fn manifest_name(self) -> &'static str {
        match self {
            Self::Dmg04 => Self::DMG04_MANIFEST_NAME,
            Self::Dmg07 => Self::DMG07_MANIFEST_NAME,
        }
    }

    pub(crate) fn from_manifest_name(name: &str) -> Option<Self> {
        match name {
            Self::DMG04_MANIFEST_NAME => Some(Self::Dmg04),
            Self::DMG07_MANIFEST_NAME => Some(Self::Dmg07),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LinkedSessionCaptureKind {
    Trace,
    Snapshot,
    ParticipantSerialHex,
    Framebuffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedSessionPassCondition {
    TraceFixture(PathBuf),
    SnapshotFixture(PathBuf),
    ParticipantSerialHexExact {
        participant_id: String,
        expected: String,
    },
    ParticipantTraceFixture {
        participant_id: String,
        fixture_path: PathBuf,
    },
    ParticipantSnapshotFixture {
        participant_id: String,
        fixture_path: PathBuf,
    },
    ParticipantFramebufferFixtureUntilMatch {
        participant_id: String,
        fixture_path: PathBuf,
        check_interval_tcycles: u64,
        check_at_tcycles: Option<u64>,
    },
    Informational(LinkedSessionCaptureKind),
}

impl LinkedSessionPassCondition {
    pub fn required_capture(&self) -> LinkedSessionCaptureKind {
        match self {
            Self::TraceFixture(_) => LinkedSessionCaptureKind::Trace,
            Self::SnapshotFixture(_) => LinkedSessionCaptureKind::Snapshot,
            Self::ParticipantSerialHexExact { .. } => {
                LinkedSessionCaptureKind::ParticipantSerialHex
            }
            Self::ParticipantTraceFixture { .. } => LinkedSessionCaptureKind::Trace,
            Self::ParticipantSnapshotFixture { .. } => LinkedSessionCaptureKind::Snapshot,
            Self::ParticipantFramebufferFixtureUntilMatch { .. } => {
                LinkedSessionCaptureKind::Framebuffer
            }
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
    pub adapter_port: Option<Dmg07Port>,
    pub external_stimuli: ExternalStimulusPlan,
}

impl LinkedSessionParticipant {
    pub fn new(id: impl Into<String>, rom_path: impl Into<PathBuf>) -> Self {
        Self {
            id: id.into(),
            rom_path: rom_path.into(),
            external_rom_root_key: None,
            console_model: ConsoleModel::GameBoy,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: ExecutionMode::Strict,
            adapter_port: None,
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

    pub fn with_adapter_port(mut self, adapter_port: Dmg07Port) -> Self {
        self.adapter_port = Some(adapter_port);
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedSessionCaseValidationError {
    EmptySessionId,
    InvalidTimeout,
    MissingRequiredCapture(LinkedSessionCaptureKind),
    MissingRequiredFailureArtifact(LinkedSessionCaptureKind),
    ArtifactNotCaptured(LinkedSessionCaptureKind),
    MissingFailureArtifacts,
    InvalidFramebufferCheckInterval,
    UnsupportedTopologyParticipantCount {
        topology: LinkedSessionTopology,
        count: usize,
    },
    MissingDmg07ParticipantPort {
        participant_id: String,
    },
    UnexpectedDmg04ParticipantPort {
        participant_id: String,
        port: Dmg07Port,
    },
    DuplicateDmg07ParticipantPort {
        port: Dmg07Port,
    },
    MissingDmg07PlayerOne,
    UnknownPassConditionParticipant(String),
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
}
