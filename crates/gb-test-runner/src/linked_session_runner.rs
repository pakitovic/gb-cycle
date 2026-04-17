use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gb_core::{
    BootRomAssetError, BootRomAssets, CartridgeDiagnostic, CartridgeLoadError, CpuDiagnosticTrap,
    CpuExecutionState, LinkedMachines, LinkedMachinesError, Machine, MachineConfig, TraceBuffer,
    TraceSummaryBuffer,
};

use crate::{
    BootRomVerificationIssue, ExternalRomSourceManifestError, ExternalStimulusAction,
    LinkedSessionCaptureKind, LinkedSessionCase, LinkedSessionParticipant,
    LinkedSessionPassCondition, LinkedSessionSuite, LinkedSessionSuiteValidationError, RomRunner,
    Timeout, boot_rom_kind_for_console_model, compatibility_for_execution_mode,
    discard_trace_events_if_needed, discover_boot_rom_store_root, encode_bytes_as_upper_hex,
    enforce_boot_rom_verification,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedSessionCaseFailure {
    CpuDiagnosticTrap {
        participant_id: String,
        trap: CpuDiagnosticTrap,
    },
    ParticipantSerialHexMismatch {
        participant_id: String,
        expected: String,
        actual: String,
    },
    ParticipantFixtureMismatch {
        participant_id: String,
        fixture_path: PathBuf,
    },
    FixtureMismatch {
        fixture_path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedSessionCaseOutcome {
    Passed,
    Informational,
    Failed(LinkedSessionCaseFailure),
}

impl LinkedSessionCaseOutcome {
    pub fn failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    pub fn non_failing(&self) -> bool {
        !self.failed()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkedSessionParticipantArtifacts {
    pub serial: String,
    pub serial_hex: String,
    pub trace_text: Option<String>,
    pub snapshot_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkedSessionCapturedArtifacts {
    pub trace: Option<String>,
    pub snapshot_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedSessionParticipantReport {
    pub participant_id: String,
    pub rom_path: PathBuf,
    pub outcome: LinkedSessionCaseOutcome,
    pub completed_frames: u32,
    pub diagnostics: Vec<CartridgeDiagnostic>,
    pub artifacts: LinkedSessionParticipantArtifacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedSessionCaseReport {
    pub session_id: String,
    pub outcome: LinkedSessionCaseOutcome,
    pub executed_t_cycles: u64,
    pub participants: Vec<LinkedSessionParticipantReport>,
    pub artifacts: LinkedSessionCapturedArtifacts,
    pub retained_failure_artifacts: Vec<PathBuf>,
}

impl LinkedSessionCaseReport {
    pub fn passed(&self) -> bool {
        matches!(self.outcome, LinkedSessionCaseOutcome::Passed)
    }

    pub fn non_failing(&self) -> bool {
        self.outcome.non_failing()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedSessionSuiteReport {
    pub suite_name: String,
    pub family: Option<String>,
    pub subsystem: crate::TestSubsystem,
    pub sessions: Vec<LinkedSessionCaseReport>,
}

impl LinkedSessionSuiteReport {
    pub fn all_passed(&self) -> bool {
        self.sessions.iter().all(LinkedSessionCaseReport::passed)
    }

    pub fn all_non_failing(&self) -> bool {
        self.sessions
            .iter()
            .all(LinkedSessionCaseReport::non_failing)
    }
}

#[derive(Debug)]
pub enum LinkedSessionExecutionError {
    InvalidSuite(LinkedSessionSuiteValidationError),
    InvalidSession(crate::LinkedSessionCaseValidationError),
    BootRomAssets {
        path: PathBuf,
        source: Box<BootRomAssetError>,
    },
    BootRomVerification {
        issue: Box<BootRomVerificationIssue>,
    },
    FileOperation {
        path: PathBuf,
        operation: &'static str,
        source: Box<io::Error>,
    },
    CartridgeLoad {
        participant_id: String,
        path: PathBuf,
        source: Box<CartridgeLoadError>,
    },
    ExternalRomSourceManifest {
        source: Box<ExternalRomSourceManifestError>,
    },
    MissingExternalRomRoot {
        key: String,
        relative_path: PathBuf,
    },
    LinkedMachines {
        source: LinkedMachinesError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedSessionRunner {
    runner: RomRunner,
}

enum RunnerLinkedMachines {
    Buffered(LinkedMachines<TraceBuffer>),
    Summary(LinkedMachines<TraceSummaryBuffer>),
}

type LinkedMachineBuild = (
    RunnerLinkedMachines,
    Vec<Vec<CartridgeDiagnostic>>,
    Vec<PathBuf>,
);

impl RunnerLinkedMachines {
    fn next_t_cycle(&self) -> u64 {
        match self {
            Self::Buffered(linked) => linked.next_t_cycle().get(),
            Self::Summary(linked) => linked.next_t_cycle().get(),
        }
    }

    fn participant_at_frame_origin(&self, participant_index: usize) -> bool {
        match self {
            Self::Buffered(linked) => {
                let machine = linked
                    .machine(participant_index)
                    .expect("participant should exist");
                machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0
            }
            Self::Summary(linked) => {
                let machine = linked
                    .machine(participant_index)
                    .expect("participant should exist");
                machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0
            }
        }
    }

    fn set_joypad_button_pressed(
        &mut self,
        participant_index: usize,
        button: gb_core::JoypadButton,
        pressed: bool,
    ) {
        match self {
            Self::Buffered(linked) => linked
                .machine_mut(participant_index)
                .expect("participant should exist")
                .set_joypad_button_pressed(button, pressed),
            Self::Summary(linked) => linked
                .machine_mut(participant_index)
                .expect("participant should exist")
                .set_joypad_button_pressed(button, pressed),
        }
    }

    fn write_bus(&mut self, participant_index: usize, address: u16, value: u8) {
        match self {
            Self::Buffered(linked) => linked
                .machine_mut(participant_index)
                .expect("participant should exist")
                .write_bus(address, value),
            Self::Summary(linked) => linked
                .machine_mut(participant_index)
                .expect("participant should exist")
                .write_bus(address, value),
        }
    }

    fn step_t_cycle(&mut self) {
        match self {
            Self::Buffered(linked) => {
                linked.step_t_cycle();
            }
            Self::Summary(linked) => {
                linked.step_t_cycle();
            }
        }
    }

    fn take_serial_output_bytes(&mut self, participant_index: usize) -> Vec<u8> {
        match self {
            Self::Buffered(linked) => linked
                .machine_mut(participant_index)
                .expect("participant should exist")
                .take_serial_output_bytes(),
            Self::Summary(linked) => linked
                .machine_mut(participant_index)
                .expect("participant should exist")
                .take_serial_output_bytes(),
        }
    }

    fn participant_cpu_execution_state(&self, participant_index: usize) -> CpuExecutionState {
        match self {
            Self::Buffered(linked) => {
                linked
                    .machine(participant_index)
                    .expect("participant should exist")
                    .cpu()
                    .snapshot()
                    .execution_state
            }
            Self::Summary(linked) => {
                linked
                    .machine(participant_index)
                    .expect("participant should exist")
                    .cpu()
                    .snapshot()
                    .execution_state
            }
        }
    }

    fn participant_trace_text(&self, participant_index: usize) -> Option<String> {
        match self {
            Self::Buffered(linked) => Some(
                linked
                    .machine(participant_index)
                    .expect("participant should exist")
                    .tracer()
                    .sink()
                    .render_text(),
            ),
            Self::Summary(_) => None,
        }
    }

    fn participant_snapshot_text(&self, participant_index: usize) -> String {
        match self {
            Self::Buffered(linked) => linked
                .machine(participant_index)
                .expect("participant should exist")
                .snapshot()
                .render_text(),
            Self::Summary(linked) => linked
                .machine(participant_index)
                .expect("participant should exist")
                .snapshot()
                .render_text(),
        }
    }

    fn discard_trace_events_if_needed(&mut self, executed_t_cycles: u64) {
        match self {
            Self::Buffered(linked) => {
                for participant_index in 0..linked.machine_count() {
                    let machine = linked
                        .machine_mut(participant_index)
                        .expect("participant should exist");
                    discard_trace_events_if_needed(
                        machine.tracer_mut().sink_mut(),
                        executed_t_cycles,
                    );
                }
            }
            Self::Summary(_) => {}
        }
    }
}

impl Default for LinkedSessionRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkedSessionRunner {
    pub fn new() -> Self {
        Self {
            runner: RomRunner::new(),
        }
    }

    pub fn with_workspace_root(mut self, workspace_root: impl Into<PathBuf>) -> Self {
        self.runner = self.runner.with_workspace_root(workspace_root);
        self
    }

    pub fn with_failure_artifact_root(mut self, failure_artifact_root: impl Into<PathBuf>) -> Self {
        self.runner = self
            .runner
            .with_failure_artifact_root(failure_artifact_root);
        self
    }

    pub fn with_boot_rom_root(mut self, boot_rom_root: impl Into<PathBuf>) -> Self {
        self.runner = self.runner.with_boot_rom_root(boot_rom_root);
        self
    }

    pub fn with_boot_rom_verification_mode(
        mut self,
        boot_rom_verification_mode: crate::BootRomVerificationMode,
    ) -> Self {
        self.runner = self
            .runner
            .with_boot_rom_verification_mode(boot_rom_verification_mode);
        self
    }

    pub fn with_external_rom_root(
        mut self,
        key: impl Into<String>,
        root: impl Into<PathBuf>,
    ) -> Self {
        self.runner = self.runner.with_external_rom_root(key, root);
        self
    }

    pub fn workspace_root(&self) -> &Path {
        self.runner.workspace_root()
    }

    pub fn run_suite(
        &self,
        suite: &LinkedSessionSuite,
    ) -> Result<LinkedSessionSuiteReport, LinkedSessionExecutionError> {
        suite
            .validate()
            .map_err(LinkedSessionExecutionError::InvalidSuite)?;

        let mut session_reports = Vec::with_capacity(suite.sessions.len());
        for session in &suite.sessions {
            session_reports.push(self.run_session(session)?);
        }

        Ok(LinkedSessionSuiteReport {
            suite_name: suite.name.clone(),
            family: suite.family.clone(),
            subsystem: suite.subsystem,
            sessions: session_reports,
        })
    }

    pub fn run_session(
        &self,
        session: &LinkedSessionCase,
    ) -> Result<LinkedSessionCaseReport, LinkedSessionExecutionError> {
        session
            .validate()
            .map_err(LinkedSessionExecutionError::InvalidSession)?;

        let needs_trace_buffer = session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Trace)
            || session
                .failure_artifacts
                .contains(LinkedSessionCaptureKind::Trace);

        let (mut linked, diagnostics, resolved_rom_paths) = if needs_trace_buffer {
            self.build_buffered_linked_machines(session)?
        } else {
            self.build_summary_linked_machines(session)?
        };

        let participant_count = session.participants.len();
        let mut executed_t_cycles = 0_u64;
        let mut completed_frames = vec![0_u32; participant_count];
        let mut at_frame_origin = (0..participant_count)
            .map(|participant_index| linked.participant_at_frame_origin(participant_index))
            .collect::<Vec<_>>();
        let mut serial_bytes = vec![Vec::new(); participant_count];
        let mut applied_stimuli = session
            .participants
            .iter()
            .map(|participant| vec![false; participant.external_stimuli.stimuli().len()])
            .collect::<Vec<_>>();
        let mut diagnostic_trap = None;

        while !linked_budget_exhausted(session.timeout, executed_t_cycles, &completed_frames) {
            self.apply_scheduled_stimuli(
                session,
                &mut linked,
                &completed_frames,
                &mut applied_stimuli,
            );

            linked.step_t_cycle();
            executed_t_cycles += 1;

            for participant_index in 0..participant_count {
                serial_bytes[participant_index]
                    .extend(linked.take_serial_output_bytes(participant_index));

                let now_at_frame_origin = linked.participant_at_frame_origin(participant_index);
                if now_at_frame_origin && !at_frame_origin[participant_index] {
                    completed_frames[participant_index] += 1;
                }
                at_frame_origin[participant_index] = now_at_frame_origin;

                if let CpuExecutionState::DiagnosticTrap { trap } =
                    linked.participant_cpu_execution_state(participant_index)
                {
                    diagnostic_trap = Some((participant_index, trap));
                    break;
                }
            }

            if diagnostic_trap.is_some() {
                break;
            }

            linked.discard_trace_events_if_needed(executed_t_cycles);
        }

        let artifacts = self.capture_artifacts(session, &linked, &serial_bytes);
        let outcome = self.evaluate_session(session, &artifacts, diagnostic_trap)?;
        let retained_failure_artifacts = if outcome.failed() {
            self.persist_failure_artifacts(session, &artifacts)?
        } else {
            Vec::new()
        };

        let mut participants = Vec::with_capacity(participant_count);
        for participant_index in 0..participant_count {
            participants.push(LinkedSessionParticipantReport {
                participant_id: session.participants[participant_index].id.clone(),
                rom_path: resolved_rom_paths[participant_index].clone(),
                outcome: participant_outcome_for_session(
                    &outcome,
                    &session.participants[participant_index].id,
                ),
                completed_frames: completed_frames[participant_index],
                diagnostics: diagnostics[participant_index].clone(),
                artifacts: artifacts.participants[participant_index].clone(),
            });
        }

        Ok(LinkedSessionCaseReport {
            session_id: session.id.clone(),
            outcome,
            executed_t_cycles,
            participants,
            artifacts: artifacts.session,
            retained_failure_artifacts,
        })
    }

    fn build_buffered_linked_machines(
        &self,
        session: &LinkedSessionCase,
    ) -> Result<LinkedMachineBuild, LinkedSessionExecutionError> {
        let mut machines = Vec::with_capacity(session.participants.len());
        let mut diagnostics = Vec::with_capacity(session.participants.len());
        let mut resolved_rom_paths = Vec::with_capacity(session.participants.len());

        for participant in &session.participants {
            let resolved_rom_path = self.resolve_participant_rom_path(participant)?;
            let rom_bytes = fs::read(&resolved_rom_path).map_err(|source| {
                LinkedSessionExecutionError::FileOperation {
                    path: resolved_rom_path.clone(),
                    operation: "read ROM",
                    source: Box::new(source),
                }
            })?;
            let boot_rom_assets = self.load_boot_rom_assets_for_participant(participant)?;
            let config = MachineConfig::new(participant.console_model)
                .with_startup_mode(participant.startup_mode)
                .with_compatibility(compatibility_for_execution_mode(participant.execution_mode))
                .with_boot_rom_assets(boot_rom_assets);
            let mut machine = Machine::new(config);
            let participant_diagnostics = machine.load_cartridge(rom_bytes).map_err(|source| {
                LinkedSessionExecutionError::CartridgeLoad {
                    participant_id: participant.id.clone(),
                    path: resolved_rom_path.clone(),
                    source: Box::new(source),
                }
            })?;
            machines.push(machine);
            diagnostics.push(participant_diagnostics);
            resolved_rom_paths.push(resolved_rom_path);
        }

        let mut linked = LinkedMachines::new(machines)
            .map_err(|source| LinkedSessionExecutionError::LinkedMachines { source })?;
        match session.topology {
            crate::LinkedSessionTopology::Dmg04 => linked
                .attach_dmg04_cable()
                .map_err(|source| LinkedSessionExecutionError::LinkedMachines { source })?,
        }
        Ok((
            RunnerLinkedMachines::Buffered(linked),
            diagnostics,
            resolved_rom_paths,
        ))
    }

    fn build_summary_linked_machines(
        &self,
        session: &LinkedSessionCase,
    ) -> Result<LinkedMachineBuild, LinkedSessionExecutionError> {
        let mut machines = Vec::with_capacity(session.participants.len());
        let mut diagnostics = Vec::with_capacity(session.participants.len());
        let mut resolved_rom_paths = Vec::with_capacity(session.participants.len());

        for participant in &session.participants {
            let resolved_rom_path = self.resolve_participant_rom_path(participant)?;
            let rom_bytes = fs::read(&resolved_rom_path).map_err(|source| {
                LinkedSessionExecutionError::FileOperation {
                    path: resolved_rom_path.clone(),
                    operation: "read ROM",
                    source: Box::new(source),
                }
            })?;
            let boot_rom_assets = self.load_boot_rom_assets_for_participant(participant)?;
            let config = MachineConfig::new(participant.console_model)
                .with_startup_mode(participant.startup_mode)
                .with_compatibility(compatibility_for_execution_mode(participant.execution_mode))
                .with_boot_rom_assets(boot_rom_assets);
            let mut machine = Machine::new_summary(config);
            let participant_diagnostics = machine.load_cartridge(rom_bytes).map_err(|source| {
                LinkedSessionExecutionError::CartridgeLoad {
                    participant_id: participant.id.clone(),
                    path: resolved_rom_path.clone(),
                    source: Box::new(source),
                }
            })?;
            machines.push(machine);
            diagnostics.push(participant_diagnostics);
            resolved_rom_paths.push(resolved_rom_path);
        }

        let mut linked = LinkedMachines::new(machines)
            .map_err(|source| LinkedSessionExecutionError::LinkedMachines { source })?;
        match session.topology {
            crate::LinkedSessionTopology::Dmg04 => linked
                .attach_dmg04_cable()
                .map_err(|source| LinkedSessionExecutionError::LinkedMachines { source })?,
        }
        Ok((
            RunnerLinkedMachines::Summary(linked),
            diagnostics,
            resolved_rom_paths,
        ))
    }

    fn resolve_participant_rom_path(
        &self,
        participant: &LinkedSessionParticipant,
    ) -> Result<PathBuf, LinkedSessionExecutionError> {
        self.runner
            .resolve_case_path(
                &participant.rom_path,
                participant.external_rom_root_key.as_deref(),
            )
            .map_err(|error| match error {
                crate::RomExecutionError::ExternalRomSourceManifest { source } => {
                    LinkedSessionExecutionError::ExternalRomSourceManifest {
                        source: Box::new(source),
                    }
                }
                crate::RomExecutionError::MissingExternalRomRoot { key, relative_path } => {
                    LinkedSessionExecutionError::MissingExternalRomRoot { key, relative_path }
                }
                other => panic!("unexpected path resolution error: {other:?}"),
            })
    }

    fn load_boot_rom_assets_for_participant(
        &self,
        participant: &LinkedSessionParticipant,
    ) -> Result<BootRomAssets, LinkedSessionExecutionError> {
        if participant.startup_mode != gb_core::StartupMode::RealBoot {
            return Ok(BootRomAssets::none());
        }

        let Some(kind) = boot_rom_kind_for_console_model(participant.console_model) else {
            return Ok(BootRomAssets::none());
        };

        let root = self
            .runner
            .boot_rom_root
            .clone()
            .or_else(|| discover_boot_rom_store_root(&self.runner.workspace_root))
            .unwrap_or_else(|| crate::boot_rom_store_root(&self.runner.workspace_root));
        let image_path = crate::boot_rom_image_path(&root, kind);
        enforce_boot_rom_verification(self.runner.boot_rom_verification_mode, &image_path, kind)
            .map_err(|issue| LinkedSessionExecutionError::BootRomVerification {
                issue: Box::new(issue),
            })?;
        if !root.is_dir() {
            return Ok(BootRomAssets::none());
        }

        BootRomAssets::from_directory(&root).map_err(|source| {
            LinkedSessionExecutionError::BootRomAssets {
                path: root,
                source: Box::new(source),
            }
        })
    }

    fn apply_scheduled_stimuli(
        &self,
        session: &LinkedSessionCase,
        linked: &mut RunnerLinkedMachines,
        completed_frames: &[u32],
        applied_stimuli: &mut [Vec<bool>],
    ) {
        let current_t_cycle = linked.next_t_cycle();

        for (participant_index, participant) in session.participants.iter().enumerate() {
            for (stimulus_index, stimulus) in
                participant.external_stimuli.stimuli().iter().enumerate()
            {
                if applied_stimuli[participant_index][stimulus_index] {
                    continue;
                }

                let should_apply = match stimulus.when {
                    crate::StimulusTime::TCycle(t_cycle) => t_cycle == current_t_cycle,
                    crate::StimulusTime::Frame(frame) => {
                        frame == completed_frames[participant_index]
                    }
                };

                if !should_apply {
                    continue;
                }

                match stimulus.action {
                    ExternalStimulusAction::JoypadSetButton { button, pressed } => {
                        linked.set_joypad_button_pressed(participant_index, button, pressed);
                    }
                    ExternalStimulusAction::WriteMemory { address, value } => {
                        linked.write_bus(participant_index, address, value);
                    }
                }

                applied_stimuli[participant_index][stimulus_index] = true;
            }
        }
    }

    fn capture_artifacts(
        &self,
        session: &LinkedSessionCase,
        linked: &RunnerLinkedMachines,
        serial_bytes: &[Vec<u8>],
    ) -> LinkedSessionRunArtifacts {
        let mut participants = Vec::with_capacity(session.participants.len());
        for (participant_index, bytes) in serial_bytes.iter().enumerate() {
            participants.push(LinkedSessionParticipantArtifacts {
                serial: String::from_utf8_lossy(bytes).into_owned(),
                serial_hex: encode_bytes_as_upper_hex(bytes),
                trace_text: linked.participant_trace_text(participant_index),
                snapshot_text: Some(linked.participant_snapshot_text(participant_index)),
            });
        }

        let session_trace = if session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Trace)
        {
            Some(render_combined_trace(session, &participants))
        } else {
            None
        };
        let session_snapshot = if session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Snapshot)
        {
            Some(render_combined_snapshot(session, &participants))
        } else {
            None
        };

        LinkedSessionRunArtifacts {
            session: LinkedSessionCapturedArtifacts {
                trace: session_trace,
                snapshot_text: session_snapshot,
            },
            participants,
        }
    }

    fn evaluate_session(
        &self,
        session: &LinkedSessionCase,
        artifacts: &LinkedSessionRunArtifacts,
        diagnostic_trap: Option<(usize, CpuDiagnosticTrap)>,
    ) -> Result<LinkedSessionCaseOutcome, LinkedSessionExecutionError> {
        if let Some((participant_index, trap)) = diagnostic_trap {
            return Ok(LinkedSessionCaseOutcome::Failed(
                LinkedSessionCaseFailure::CpuDiagnosticTrap {
                    participant_id: session.participants[participant_index].id.clone(),
                    trap,
                },
            ));
        }

        Ok(match &session.pass_condition {
            LinkedSessionPassCondition::Informational(_) => LinkedSessionCaseOutcome::Informational,
            LinkedSessionPassCondition::ParticipantSerialHexExact {
                participant_id,
                expected,
            } => {
                let participant_index = session
                    .participants
                    .iter()
                    .position(|participant| participant.id == *participant_id)
                    .expect("linked session should validate target participant existence");
                let actual = artifacts.participants[participant_index].serial_hex.clone();
                if actual == *expected {
                    LinkedSessionCaseOutcome::Passed
                } else {
                    LinkedSessionCaseOutcome::Failed(
                        LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                            participant_id: participant_id.clone(),
                            expected: expected.clone(),
                            actual,
                        },
                    )
                }
            }
            LinkedSessionPassCondition::ParticipantSnapshotFixture {
                participant_id,
                fixture_path,
            } => {
                let participant_index = session
                    .participants
                    .iter()
                    .position(|participant| participant.id == *participant_id)
                    .expect("linked session should validate target participant existence");
                let resolved_fixture = self.runner.resolve_path(fixture_path);
                let expected = fs::read_to_string(&resolved_fixture).map_err(|source| {
                    LinkedSessionExecutionError::FileOperation {
                        path: resolved_fixture.clone(),
                        operation: "read participant snapshot fixture",
                        source: Box::new(source),
                    }
                })?;
                if artifacts.participants[participant_index]
                    .snapshot_text
                    .as_deref()
                    == Some(expected.as_str())
                {
                    LinkedSessionCaseOutcome::Passed
                } else {
                    LinkedSessionCaseOutcome::Failed(
                        LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                            participant_id: participant_id.clone(),
                            fixture_path: resolved_fixture,
                        },
                    )
                }
            }
            LinkedSessionPassCondition::TraceFixture(fixture_path) => {
                let resolved_fixture = self.runner.resolve_path(fixture_path);
                let expected = fs::read_to_string(&resolved_fixture).map_err(|source| {
                    LinkedSessionExecutionError::FileOperation {
                        path: resolved_fixture.clone(),
                        operation: "read linked trace fixture",
                        source: Box::new(source),
                    }
                })?;
                if artifacts.session.trace.as_deref() == Some(expected.as_str()) {
                    LinkedSessionCaseOutcome::Passed
                } else {
                    LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
            LinkedSessionPassCondition::SnapshotFixture(fixture_path) => {
                let resolved_fixture = self.runner.resolve_path(fixture_path);
                let expected = fs::read_to_string(&resolved_fixture).map_err(|source| {
                    LinkedSessionExecutionError::FileOperation {
                        path: resolved_fixture.clone(),
                        operation: "read linked snapshot fixture",
                        source: Box::new(source),
                    }
                })?;
                if artifacts.session.snapshot_text.as_deref() == Some(expected.as_str()) {
                    LinkedSessionCaseOutcome::Passed
                } else {
                    LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
        })
    }

    fn persist_failure_artifacts(
        &self,
        session: &LinkedSessionCase,
        artifacts: &LinkedSessionRunArtifacts,
    ) -> Result<Vec<PathBuf>, LinkedSessionExecutionError> {
        let Some(root) = &self.runner.failure_artifact_root else {
            return Ok(Vec::new());
        };

        let session_dir = root.join(&session.id);
        fs::create_dir_all(&session_dir).map_err(|source| {
            LinkedSessionExecutionError::FileOperation {
                path: session_dir.clone(),
                operation: "create linked-session artifact directory",
                source: Box::new(source),
            }
        })?;

        let mut written_paths = Vec::new();
        for artifact in session.failure_artifacts.retained() {
            match artifact {
                LinkedSessionCaptureKind::ParticipantSerialHex => {
                    for (participant_index, participant) in session.participants.iter().enumerate()
                    {
                        let serial_hex_path =
                            session_dir.join(format!("{}_serial_hex.txt", participant.id));
                        fs::write(
                            &serial_hex_path,
                            &artifacts.participants[participant_index].serial_hex,
                        )
                        .map_err(|source| {
                            LinkedSessionExecutionError::FileOperation {
                                path: serial_hex_path.clone(),
                                operation: "write participant serial hex artifact",
                                source: Box::new(source),
                            }
                        })?;
                        written_paths.push(serial_hex_path);
                    }
                }
                LinkedSessionCaptureKind::Trace => {
                    if let Some(trace) = &artifacts.session.trace {
                        let path = session_dir.join("linked_trace.txt");
                        fs::write(&path, trace).map_err(|source| {
                            LinkedSessionExecutionError::FileOperation {
                                path: path.clone(),
                                operation: "write linked trace artifact",
                                source: Box::new(source),
                            }
                        })?;
                        written_paths.push(path);
                    }
                    for (participant_index, participant) in session.participants.iter().enumerate()
                    {
                        let serial_path =
                            session_dir.join(format!("{}_serial.txt", participant.id));
                        fs::write(
                            &serial_path,
                            &artifacts.participants[participant_index].serial,
                        )
                        .map_err(|source| {
                            LinkedSessionExecutionError::FileOperation {
                                path: serial_path.clone(),
                                operation: "write participant serial artifact",
                                source: Box::new(source),
                            }
                        })?;
                        written_paths.push(serial_path);

                        let serial_hex_path =
                            session_dir.join(format!("{}_serial_hex.txt", participant.id));
                        fs::write(
                            &serial_hex_path,
                            &artifacts.participants[participant_index].serial_hex,
                        )
                        .map_err(|source| {
                            LinkedSessionExecutionError::FileOperation {
                                path: serial_hex_path.clone(),
                                operation: "write participant serial hex artifact",
                                source: Box::new(source),
                            }
                        })?;
                        written_paths.push(serial_hex_path);

                        if let Some(trace_text) =
                            &artifacts.participants[participant_index].trace_text
                        {
                            let trace_path =
                                session_dir.join(format!("{}_trace.txt", participant.id));
                            fs::write(&trace_path, trace_text).map_err(|source| {
                                LinkedSessionExecutionError::FileOperation {
                                    path: trace_path.clone(),
                                    operation: "write participant trace artifact",
                                    source: Box::new(source),
                                }
                            })?;
                            written_paths.push(trace_path);
                        }
                    }
                }
                LinkedSessionCaptureKind::Snapshot => {
                    if let Some(snapshot_text) = &artifacts.session.snapshot_text {
                        let path = session_dir.join("linked_snapshot.txt");
                        fs::write(&path, snapshot_text).map_err(|source| {
                            LinkedSessionExecutionError::FileOperation {
                                path: path.clone(),
                                operation: "write linked snapshot artifact",
                                source: Box::new(source),
                            }
                        })?;
                        written_paths.push(path);
                    }
                    for (participant_index, participant) in session.participants.iter().enumerate()
                    {
                        if let Some(snapshot_text) =
                            &artifacts.participants[participant_index].snapshot_text
                        {
                            let path = session_dir.join(format!("{}_snapshot.txt", participant.id));
                            fs::write(&path, snapshot_text).map_err(|source| {
                                LinkedSessionExecutionError::FileOperation {
                                    path: path.clone(),
                                    operation: "write participant snapshot artifact",
                                    source: Box::new(source),
                                }
                            })?;
                            written_paths.push(path);
                        }
                    }
                }
            }
        }

        Ok(written_paths)
    }
}

fn participant_outcome_for_session(
    session_outcome: &LinkedSessionCaseOutcome,
    participant_id: &str,
) -> LinkedSessionCaseOutcome {
    match session_outcome {
        LinkedSessionCaseOutcome::Passed => LinkedSessionCaseOutcome::Passed,
        LinkedSessionCaseOutcome::Informational => LinkedSessionCaseOutcome::Informational,
        LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::CpuDiagnosticTrap {
            participant_id: failed_participant_id,
            trap,
        }) => {
            if failed_participant_id == participant_id {
                LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::CpuDiagnosticTrap {
                    participant_id: failed_participant_id.clone(),
                    trap: *trap,
                })
            } else {
                LinkedSessionCaseOutcome::Passed
            }
        }
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                participant_id: failed_participant_id,
                expected,
                actual,
            },
        ) => {
            if failed_participant_id == participant_id {
                LinkedSessionCaseOutcome::Failed(
                    LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                        participant_id: failed_participant_id.clone(),
                        expected: expected.clone(),
                        actual: actual.clone(),
                    },
                )
            } else {
                LinkedSessionCaseOutcome::Passed
            }
        }
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                participant_id: failed_participant_id,
                fixture_path,
            },
        ) => {
            if failed_participant_id == participant_id {
                LinkedSessionCaseOutcome::Failed(
                    LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                        participant_id: failed_participant_id.clone(),
                        fixture_path: fixture_path.clone(),
                    },
                )
            } else {
                LinkedSessionCaseOutcome::Passed
            }
        }
        LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch {
            fixture_path,
        }) => LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch {
            fixture_path: fixture_path.clone(),
        }),
    }
}

struct LinkedSessionRunArtifacts {
    session: LinkedSessionCapturedArtifacts,
    participants: Vec<LinkedSessionParticipantArtifacts>,
}

fn linked_budget_exhausted(
    timeout: Timeout,
    executed_t_cycles: u64,
    completed_frames: &[u32],
) -> bool {
    let max_completed_frames = completed_frames.iter().copied().max().unwrap_or(0);
    super::budget_exhausted(timeout, executed_t_cycles, max_completed_frames)
}

fn render_combined_trace(
    session: &LinkedSessionCase,
    participants: &[LinkedSessionParticipantArtifacts],
) -> String {
    let mut rendered = String::new();
    for (participant, artifacts) in session.participants.iter().zip(participants.iter()) {
        rendered.push_str("== participant ");
        rendered.push_str(&participant.id);
        rendered.push_str(" trace ==\n");
        if let Some(trace_text) = &artifacts.trace_text {
            rendered.push_str(trace_text);
            if !trace_text.ends_with('\n') {
                rendered.push('\n');
            }
        }
    }
    rendered
}

fn render_combined_snapshot(
    session: &LinkedSessionCase,
    participants: &[LinkedSessionParticipantArtifacts],
) -> String {
    let mut rendered = String::new();
    for (participant, artifacts) in session.participants.iter().zip(participants.iter()) {
        rendered.push_str("== participant ");
        rendered.push_str(&participant.id);
        rendered.push_str(" snapshot ==\n");
        if let Some(snapshot_text) = &artifacts.snapshot_text {
            rendered.push_str(snapshot_text);
            if !snapshot_text.ends_with('\n') {
                rendered.push('\n');
            }
        }
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ExternalStimulus, LinkedSessionCase, LinkedSessionFailureArtifactPolicy,
        LinkedSessionParticipant, LinkedSessionPassCondition, LinkedSessionSuite,
        LinkedSessionTopology,
    };
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-linked-session-runner-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn build_test_rom(program: &[u8]) -> Vec<u8> {
        let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
        for (offset, byte) in program.iter().copied().enumerate() {
            rom[0x0100 + offset] = byte;
        }
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom
    }

    fn build_single_shot_serial_from_address_rom(address: u16, sc_value: u8) -> Vec<u8> {
        build_test_rom(&[
            0xFA,
            address as u8,
            (address >> 8) as u8, // LD A,(a16)
            0xE0,
            0x01, // LDH (SB),A
            0x3E,
            sc_value, // LD A,SC
            0xE0,
            0x02, // LDH (SC),A
            0xC3,
            0x08,
            0x01, // JP 0108 (self-loop after arming transfer)
        ])
    }

    #[test]
    fn linked_session_runner_executes_a_dmg04_exchange_and_captures_session_trace() {
        let temp_dir = unique_temp_dir("dmg04-exchange");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let left_rom = temp_dir.join("left.gb");
        let right_rom = temp_dir.join("right.gb");
        fs::write(
            &left_rom,
            build_single_shot_serial_from_address_rom(0xC000, 0x81),
        )
        .expect("left ROM should be writable");
        fs::write(
            &right_rom,
            build_single_shot_serial_from_address_rom(0xC100, 0x80),
        )
        .expect("right ROM should be writable");

        let session = LinkedSessionCase::new(
            "dmg04-exchange",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace),
        )
        .with_participant(
            LinkedSessionParticipant::new("left", &left_rom).with_external_stimulus(
                ExternalStimulus::at_t_cycle(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC000,
                        value: 0xA5,
                    },
                ),
            ),
        )
        .with_participant(
            LinkedSessionParticipant::new("right", &right_rom).with_external_stimulus(
                ExternalStimulus::at_t_cycle(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC100,
                        value: 0x3C,
                    },
                ),
            ),
        );

        let report = LinkedSessionRunner::new()
            .run_session(&session)
            .expect("linked session should execute");

        assert_eq!(report.outcome, LinkedSessionCaseOutcome::Informational);
        assert_eq!(report.participants.len(), 2);
        assert_eq!(report.participants[0].artifacts.serial_hex, "A5");
        assert_eq!(report.participants[1].artifacts.serial_hex, "3C");
        let trace = report
            .artifacts
            .trace
            .as_deref()
            .expect("trace artifact should be captured");
        assert!(trace.contains("== participant left trace =="));
        assert!(trace.contains("== participant right trace =="));

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn linked_session_runner_replays_trace_fixtures_deterministically() {
        let temp_dir = unique_temp_dir("trace-fixture");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let left_rom = temp_dir.join("left.gb");
        let right_rom = temp_dir.join("right.gb");
        fs::write(
            &left_rom,
            build_single_shot_serial_from_address_rom(0xC000, 0x81),
        )
        .expect("left ROM should be writable");
        fs::write(
            &right_rom,
            build_single_shot_serial_from_address_rom(0xC100, 0x80),
        )
        .expect("right ROM should be writable");

        let info_session = LinkedSessionCase::new(
            "fixture-source",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace),
        )
        .with_participant(
            LinkedSessionParticipant::new("left", &left_rom).with_external_stimulus(
                ExternalStimulus::at_t_cycle(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC000,
                        value: 0x11,
                    },
                ),
            ),
        )
        .with_participant(
            LinkedSessionParticipant::new("right", &right_rom).with_external_stimulus(
                ExternalStimulus::at_t_cycle(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC100,
                        value: 0x22,
                    },
                ),
            ),
        );

        let info_report = LinkedSessionRunner::new()
            .run_session(&info_session)
            .expect("informational linked session should execute");
        let fixture_path = temp_dir.join("linked.trace");
        fs::write(
            &fixture_path,
            info_report
                .artifacts
                .trace
                .as_deref()
                .expect("trace artifact should exist"),
        )
        .expect("fixture should be writable");

        let fixture_session = LinkedSessionCase::new(
            "fixture-match",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::TraceFixture(fixture_path.clone()),
        )
        .with_participant(
            LinkedSessionParticipant::new("left", &left_rom).with_external_stimulus(
                ExternalStimulus::at_t_cycle(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC000,
                        value: 0x11,
                    },
                ),
            ),
        )
        .with_participant(
            LinkedSessionParticipant::new("right", &right_rom).with_external_stimulus(
                ExternalStimulus::at_t_cycle(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC100,
                        value: 0x22,
                    },
                ),
            ),
        );

        let runner = LinkedSessionRunner::new();
        let first = runner
            .run_session(&fixture_session)
            .expect("fixture session should pass");
        let second = runner
            .run_session(&fixture_session)
            .expect("fixture session should rerun deterministically");

        assert_eq!(first.outcome, LinkedSessionCaseOutcome::Passed);
        assert_eq!(second.outcome, LinkedSessionCaseOutcome::Passed);
        assert_eq!(first.artifacts, second.artifacts);
        assert_eq!(first.participants, second.participants);

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn linked_session_runner_supports_participant_serial_hex_expectations() {
        let temp_dir = unique_temp_dir("participant-serial-hex-pass");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let left_rom = temp_dir.join("left.gb");
        let right_rom = temp_dir.join("right.gb");
        fs::write(
            &left_rom,
            build_test_rom(&[
                0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
            ]),
        )
        .expect("left ROM should be writable");
        fs::write(
            &right_rom,
            build_test_rom(&[
                0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
            ]),
        )
        .expect("right ROM should be writable");

        let session = LinkedSessionCase::new(
            "participant-serial-hex-pass",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::ParticipantSerialHexExact {
                participant_id: "left".to_string(),
                expected: "A5".to_string(),
            },
        )
        .with_participant(LinkedSessionParticipant::new("left", &left_rom))
        .with_participant(LinkedSessionParticipant::new("right", &right_rom));

        let report = LinkedSessionRunner::new()
            .run_session(&session)
            .expect("participant serial hex session should execute");

        assert_eq!(report.outcome, LinkedSessionCaseOutcome::Passed);
        assert_eq!(
            report.participants[0].outcome,
            LinkedSessionCaseOutcome::Passed
        );
        assert_eq!(
            report.participants[1].outcome,
            LinkedSessionCaseOutcome::Passed
        );

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn linked_session_runner_supports_participant_snapshot_fixtures() {
        let temp_dir = unique_temp_dir("participant-snapshot-pass");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let left_rom = temp_dir.join("left.gb");
        let right_rom = temp_dir.join("right.gb");
        let fixture_path = temp_dir.join("left.snapshot");
        fs::write(
            &left_rom,
            build_test_rom(&[
                0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
            ]),
        )
        .expect("left ROM should be writable");
        fs::write(
            &right_rom,
            build_test_rom(&[
                0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
            ]),
        )
        .expect("right ROM should be writable");

        let baseline = LinkedSessionCase::new(
            "participant-snapshot-baseline",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
        )
        .with_participant(LinkedSessionParticipant::new("left", &left_rom))
        .with_participant(LinkedSessionParticipant::new("right", &right_rom));

        let baseline_report = LinkedSessionRunner::new()
            .run_session(&baseline)
            .expect("baseline linked snapshot session should execute");
        fs::write(
            &fixture_path,
            baseline_report.participants[0]
                .artifacts
                .snapshot_text
                .as_deref()
                .expect("baseline left snapshot should be captured"),
        )
        .expect("participant snapshot fixture should be writable");

        let session = LinkedSessionCase::new(
            "participant-snapshot-pass",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::ParticipantSnapshotFixture {
                participant_id: "left".to_string(),
                fixture_path: fixture_path.clone(),
            },
        )
        .with_participant(LinkedSessionParticipant::new("left", &left_rom))
        .with_participant(LinkedSessionParticipant::new("right", &right_rom));

        let report = LinkedSessionRunner::new()
            .run_session(&session)
            .expect("participant snapshot fixture session should execute");

        assert_eq!(report.outcome, LinkedSessionCaseOutcome::Passed);
        assert_eq!(
            report.participants[0].outcome,
            LinkedSessionCaseOutcome::Passed
        );
        assert_eq!(
            report.participants[1].outcome,
            LinkedSessionCaseOutcome::Passed
        );

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn linked_session_runner_persists_failure_artifacts_for_trace_mismatches() {
        let temp_dir = unique_temp_dir("failure-artifacts");
        let artifact_root = temp_dir.join("artifacts");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let left_rom = temp_dir.join("left.gb");
        let right_rom = temp_dir.join("right.gb");
        fs::write(
            &left_rom,
            build_single_shot_serial_from_address_rom(0xC000, 0x81),
        )
        .expect("left ROM should be writable");
        fs::write(
            &right_rom,
            build_single_shot_serial_from_address_rom(0xC100, 0x80),
        )
        .expect("right ROM should be writable");
        let fixture_path = temp_dir.join("wrong.trace");
        fs::write(&fixture_path, "definitely wrong\n").expect("fixture should be writable");

        let session = LinkedSessionCase::new(
            "trace-mismatch",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::TraceFixture(fixture_path.clone()),
        )
        .with_failure_artifacts(
            LinkedSessionFailureArtifactPolicy::new()
                .with_artifact(LinkedSessionCaptureKind::Trace)
                .with_artifact(LinkedSessionCaptureKind::Snapshot),
        )
        .with_participant(
            LinkedSessionParticipant::new("left", &left_rom)
                .with_external_stimulus(ExternalStimulus::at_frame(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC000,
                        value: 0x41,
                    },
                ))
                .with_external_stimulus(ExternalStimulus::at_t_cycle(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC100,
                        value: 0x5A,
                    },
                )),
        )
        .with_participant(LinkedSessionParticipant::new("right", &right_rom));

        let report = LinkedSessionRunner::new()
            .with_failure_artifact_root(&artifact_root)
            .run_session(&session)
            .expect("mismatch session should execute");

        assert!(matches!(
            report.outcome,
            LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch { .. })
        ));
        let left_serial_hex = &report.participants[0].artifacts.serial_hex;
        assert_eq!(left_serial_hex, "41");
        assert!(
            artifact_root
                .join("trace-mismatch")
                .join("linked_trace.txt")
                .is_file()
        );
        assert!(
            artifact_root
                .join("trace-mismatch")
                .join("linked_snapshot.txt")
                .is_file()
        );
        assert!(
            artifact_root
                .join("trace-mismatch")
                .join("left_serial.txt")
                .is_file()
        );
        assert!(
            artifact_root
                .join("trace-mismatch")
                .join("left_serial_hex.txt")
                .is_file()
        );
        assert!(
            artifact_root
                .join("trace-mismatch")
                .join("left_snapshot.txt")
                .is_file()
        );

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn linked_session_runner_reports_participant_snapshot_fixture_mismatches_per_participant() {
        let temp_dir = unique_temp_dir("participant-snapshot-mismatch");
        let artifact_root = temp_dir.join("artifacts");
        let expected_fixture_path = temp_dir.join("wrong.snapshot");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let left_rom = temp_dir.join("left.gb");
        let right_rom = temp_dir.join("right.gb");
        fs::write(
            &left_rom,
            build_test_rom(&[
                0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
            ]),
        )
        .expect("left ROM should be writable");
        fs::write(
            &right_rom,
            build_test_rom(&[
                0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
            ]),
        )
        .expect("right ROM should be writable");
        fs::write(&expected_fixture_path, "definitely wrong\n")
            .expect("wrong participant snapshot fixture should be writable");

        let session = LinkedSessionCase::new(
            "participant-snapshot-mismatch",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::ParticipantSnapshotFixture {
                participant_id: "left".to_string(),
                fixture_path: expected_fixture_path.clone(),
            },
        )
        .with_failure_artifacts(
            LinkedSessionFailureArtifactPolicy::new()
                .with_artifact(LinkedSessionCaptureKind::Snapshot),
        )
        .with_participant(LinkedSessionParticipant::new("left", &left_rom))
        .with_participant(LinkedSessionParticipant::new("right", &right_rom));

        let report = LinkedSessionRunner::new()
            .with_failure_artifact_root(&artifact_root)
            .run_session(&session)
            .expect("participant snapshot mismatch session should execute");

        assert!(matches!(
            report.outcome,
            LinkedSessionCaseOutcome::Failed(
                LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                    ref participant_id,
                    ref fixture_path,
                }
            ) if participant_id == "left" && fixture_path == &expected_fixture_path
        ));
        assert!(matches!(
            report.participants[0].outcome,
            LinkedSessionCaseOutcome::Failed(
                LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                    ref participant_id,
                    ..
                }
            ) if participant_id == "left"
        ));
        assert_eq!(
            report.participants[1].outcome,
            LinkedSessionCaseOutcome::Passed
        );
        assert!(
            artifact_root
                .join("participant-snapshot-mismatch")
                .join("linked_snapshot.txt")
                .is_file()
        );
        assert!(
            artifact_root
                .join("participant-snapshot-mismatch")
                .join("left_snapshot.txt")
                .is_file()
        );
        assert!(
            artifact_root
                .join("participant-snapshot-mismatch")
                .join("right_snapshot.txt")
                .is_file()
        );

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn linked_session_runner_reports_participant_serial_hex_mismatches_per_participant() {
        let temp_dir = unique_temp_dir("participant-serial-hex-mismatch");
        let artifact_root = temp_dir.join("artifacts");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let left_rom = temp_dir.join("left.gb");
        let right_rom = temp_dir.join("right.gb");
        fs::write(
            &left_rom,
            build_test_rom(&[
                0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
            ]),
        )
        .expect("left ROM should be writable");
        fs::write(
            &right_rom,
            build_test_rom(&[
                0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
            ]),
        )
        .expect("right ROM should be writable");

        let session = LinkedSessionCase::new(
            "participant-serial-hex-mismatch",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::ParticipantSerialHexExact {
                participant_id: "left".to_string(),
                expected: "FF".to_string(),
            },
        )
        .with_failure_artifacts(
            LinkedSessionFailureArtifactPolicy::new()
                .with_artifact(LinkedSessionCaptureKind::ParticipantSerialHex)
                .with_artifact(LinkedSessionCaptureKind::Snapshot),
        )
        .with_participant(LinkedSessionParticipant::new("left", &left_rom))
        .with_participant(LinkedSessionParticipant::new("right", &right_rom));

        let report = LinkedSessionRunner::new()
            .with_failure_artifact_root(&artifact_root)
            .run_session(&session)
            .expect("participant serial hex mismatch session should execute");

        assert!(matches!(
            report.outcome,
            LinkedSessionCaseOutcome::Failed(
                LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                    ref participant_id,
                    ref expected,
                    ref actual,
                }
            ) if participant_id == "left" && expected == "FF" && actual == "A5"
        ));
        assert!(matches!(
            report.participants[0].outcome,
            LinkedSessionCaseOutcome::Failed(
                LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                    ref participant_id,
                    ..
                }
            ) if participant_id == "left"
        ));
        assert_eq!(
            report.participants[1].outcome,
            LinkedSessionCaseOutcome::Passed
        );
        assert!(
            artifact_root
                .join("participant-serial-hex-mismatch")
                .join("left_serial_hex.txt")
                .is_file()
        );
        assert!(
            artifact_root
                .join("participant-serial-hex-mismatch")
                .join("right_serial_hex.txt")
                .is_file()
        );

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn linked_session_runner_reports_cpu_diagnostic_traps_per_participant() {
        let temp_dir = unique_temp_dir("diagnostic-trap");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let left_rom = temp_dir.join("left.gb");
        let right_rom = temp_dir.join("right.gb");
        fs::write(&left_rom, build_test_rom(&[0xD3, 0x00, 0x01]))
            .expect("left ROM should be writable");
        fs::write(
            &right_rom,
            build_single_shot_serial_from_address_rom(0xC100, 0x80),
        )
        .expect("right ROM should be writable");

        let session = LinkedSessionCase::new(
            "diagnostic-trap",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(64),
            LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
        )
        .with_participant(LinkedSessionParticipant::new("left", &left_rom))
        .with_participant(LinkedSessionParticipant::new("right", &right_rom));

        let report = LinkedSessionRunner::new()
            .run_session(&session)
            .expect("diagnostic session should execute");

        assert!(matches!(
            report.outcome,
            LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::CpuDiagnosticTrap {
                participant_id,
                ..
            }) if participant_id == "left"
        ));

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn linked_session_runner_executes_suites() {
        let temp_dir = unique_temp_dir("suite");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let left_rom = temp_dir.join("left.gb");
        let right_rom = temp_dir.join("right.gb");
        fs::write(
            &left_rom,
            build_single_shot_serial_from_address_rom(0xC000, 0x81),
        )
        .expect("left ROM should be writable");
        fs::write(
            &right_rom,
            build_single_shot_serial_from_address_rom(0xC100, 0x80),
        )
        .expect("right ROM should be writable");

        let session = LinkedSessionCase::new(
            "suite-session",
            LinkedSessionTopology::Dmg04,
            Timeout::TCycles(5_000),
            LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
        )
        .with_participant(
            LinkedSessionParticipant::new("left", &left_rom).with_external_stimulus(
                ExternalStimulus::at_t_cycle(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC000,
                        value: 0xAA,
                    },
                ),
            ),
        )
        .with_participant(
            LinkedSessionParticipant::new("right", &right_rom).with_external_stimulus(
                ExternalStimulus::at_t_cycle(
                    0,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC100,
                        value: 0x55,
                    },
                ),
            ),
        );

        let suite = LinkedSessionSuite::new("linked-suite", crate::TestSubsystem::Serial)
            .with_session(session);
        let report = LinkedSessionRunner::new()
            .run_suite(&suite)
            .expect("linked suite should execute");

        assert!(report.all_non_failing());
        assert_eq!(report.sessions.len(), 1);

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }
}
