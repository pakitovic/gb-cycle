mod artifacts;
mod build;
mod evaluate;
#[cfg(test)]
mod tests;

use std::io;
use std::path::{Path, PathBuf};

use gb_core::{
    BootRomAssetError, CartridgeDiagnostic, CartridgeLoadError, CpuDiagnosticTrap,
    CpuExecutionState, LinkedMachines, LinkedMachinesError, TraceBuffer, TraceSummaryBuffer,
};

use crate::framebuffer_oracle::{
    NormalizedFramebuffer, decode_fixture_framebuffer_path, normalize_dmg_framebuffer,
};
use crate::{
    BootRomVerificationIssue, LinkedSessionCaptureKind, LinkedSessionCase,
    LinkedSessionParticipant, LinkedSessionPassCondition, LinkedSessionSuite,
    LinkedSessionSuiteValidationError, RomRunner, Timeout, discard_trace_events_if_needed,
};

use artifacts::LinkedSessionRunArtifacts;
use evaluate::participant_outcome_for_session;

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
        capture: LinkedSessionCaptureKind,
        fixture_path: PathBuf,
    },
    ParticipantFramebufferCheckAtNotReached {
        participant_id: String,
        check_at_tcycles: u64,
        executed_t_cycles: u64,
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
    pub framebuffer_pgm: Option<Vec<u8>>,
    pub trace_text: Option<String>,
    pub snapshot_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkedSessionCapturedArtifacts {
    pub trace: Option<String>,
    pub snapshot_text: Option<String>,
    pub topology_trace_text: Option<String>,
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
    MissingExternalRomRoot {
        key: String,
        relative_path: PathBuf,
    },
    RomPathResolution {
        participant_id: String,
        source: Box<crate::RomExecutionError>,
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

type LoadedParticipantMachine<S> = (gb_core::Machine<S>, Vec<CartridgeDiagnostic>, PathBuf);

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkedFramebufferUntilMatchOracle {
    participant_index: usize,
    expected: NormalizedFramebuffer,
    check_interval_tcycles: u64,
    check_at_tcycles: Option<u64>,
    pending_periodic_check: bool,
    matched: bool,
    check_at_reached: bool,
}

impl RunnerLinkedMachines {
    fn with_linked<R>(
        &self,
        buffered: impl FnOnce(&LinkedMachines<TraceBuffer>) -> R,
        summary: impl FnOnce(&LinkedMachines<TraceSummaryBuffer>) -> R,
    ) -> R {
        match self {
            Self::Buffered(linked) => buffered(linked),
            Self::Summary(linked) => summary(linked),
        }
    }

    fn with_linked_mut<R>(
        &mut self,
        buffered: impl FnOnce(&mut LinkedMachines<TraceBuffer>) -> R,
        summary: impl FnOnce(&mut LinkedMachines<TraceSummaryBuffer>) -> R,
    ) -> R {
        match self {
            Self::Buffered(linked) => buffered(linked),
            Self::Summary(linked) => summary(linked),
        }
    }

    fn with_machine<R>(
        &self,
        participant_index: usize,
        buffered: impl FnOnce(&gb_core::Machine<TraceBuffer>) -> R,
        summary: impl FnOnce(&gb_core::Machine<TraceSummaryBuffer>) -> R,
    ) -> R {
        self.with_linked(
            |linked| {
                buffered(
                    linked
                        .machine(participant_index)
                        .expect("participant should exist"),
                )
            },
            |linked| {
                summary(
                    linked
                        .machine(participant_index)
                        .expect("participant should exist"),
                )
            },
        )
    }

    fn with_machine_mut<R>(
        &mut self,
        participant_index: usize,
        buffered: impl FnOnce(&mut gb_core::Machine<TraceBuffer>) -> R,
        summary: impl FnOnce(&mut gb_core::Machine<TraceSummaryBuffer>) -> R,
    ) -> R {
        self.with_linked_mut(
            |linked| {
                buffered(
                    linked
                        .machine_mut(participant_index)
                        .expect("participant should exist"),
                )
            },
            |linked| {
                summary(
                    linked
                        .machine_mut(participant_index)
                        .expect("participant should exist"),
                )
            },
        )
    }

    fn next_t_cycle(&self) -> u64 {
        self.with_linked(
            |linked| linked.next_t_cycle().get(),
            |linked| linked.next_t_cycle().get(),
        )
    }

    fn participant_at_frame_origin(&self, participant_index: usize) -> bool {
        self.with_machine(
            participant_index,
            |machine| machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
            |machine| machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
        )
    }

    fn participant_in_vblank(&self, participant_index: usize) -> bool {
        self.with_machine(
            participant_index,
            |machine| machine.ppu().ly() >= 144,
            |machine| machine.ppu().ly() >= 144,
        )
    }

    fn participant_framebuffer(&self, participant_index: usize) -> &[u8] {
        match self {
            Self::Buffered(linked) => linked
                .machine(participant_index)
                .expect("participant should exist")
                .ppu()
                .framebuffer(),
            Self::Summary(linked) => linked
                .machine(participant_index)
                .expect("participant should exist")
                .ppu()
                .framebuffer(),
        }
    }

    fn set_joypad_button_pressed(
        &mut self,
        participant_index: usize,
        button: gb_core::JoypadButton,
        pressed: bool,
    ) {
        self.with_machine_mut(
            participant_index,
            |machine| machine.set_joypad_button_pressed(button, pressed),
            |machine| machine.set_joypad_button_pressed(button, pressed),
        );
    }

    fn write_bus(&mut self, participant_index: usize, address: u16, value: u8) {
        self.with_machine_mut(
            participant_index,
            |machine| machine.write_bus(address, value),
            |machine| machine.write_bus(address, value),
        );
    }

    fn step_t_cycle(&mut self) {
        self.with_linked_mut(
            |linked| {
                linked.advance_t_cycle();
            },
            |linked| {
                linked.advance_t_cycle();
            },
        );
    }

    fn take_serial_output_bytes(&mut self, participant_index: usize) -> Vec<u8> {
        self.with_machine_mut(
            participant_index,
            |machine| machine.take_serial_output_bytes(),
            |machine| machine.take_serial_output_bytes(),
        )
    }

    fn participant_cpu_execution_state(&self, participant_index: usize) -> CpuExecutionState {
        self.with_machine(
            participant_index,
            |machine| machine.cpu().snapshot().execution_state,
            |machine| machine.cpu().snapshot().execution_state,
        )
    }

    fn participant_trace_text(&self, participant_index: usize) -> Option<String> {
        self.with_machine(
            participant_index,
            |machine| Some(machine.tracer().sink().render_text()),
            |_| None,
        )
    }

    fn participant_snapshot_text(&self, participant_index: usize) -> String {
        self.with_machine(
            participant_index,
            |machine| machine.snapshot().render_text(),
            |machine| machine.snapshot().render_text(),
        )
    }

    fn topology_trace_text(&self) -> Option<String> {
        self.with_linked(
            |linked| linked.topology_trace_text(),
            |linked| linked.topology_trace_text(),
        )
    }

    fn discard_trace_events_if_needed(&mut self, executed_t_cycles: u64) {
        self.with_linked_mut(
            |linked| {
                for participant_index in 0..linked.machine_count() {
                    let machine = linked
                        .machine_mut(participant_index)
                        .expect("participant should exist");
                    discard_trace_events_if_needed(
                        machine.tracer_mut().sink_mut(),
                        executed_t_cycles,
                    );
                }
            },
            |_| {},
        );
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
        let mut framebuffer_until_match_oracle =
            self.framebuffer_until_match_oracle(session, &linked)?;

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

            if let Some(oracle) = &mut framebuffer_until_match_oracle
                && linked_framebuffer_until_match_poll_due(
                    session.id.as_str(),
                    &linked,
                    executed_t_cycles,
                    oracle,
                )?
            {
                break;
            }

            linked.discard_trace_events_if_needed(executed_t_cycles);
        }

        let artifacts = self.capture_artifacts(session, &linked, &serial_bytes);
        let outcome = self.evaluate_session(
            session,
            &artifacts,
            diagnostic_trap,
            framebuffer_until_match_oracle
                .as_ref()
                .is_some_and(|oracle| oracle.matched),
            framebuffer_until_match_oracle
                .as_ref()
                .is_some_and(|oracle| oracle.check_at_reached),
            executed_t_cycles,
        )?;
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

    fn framebuffer_until_match_oracle(
        &self,
        session: &LinkedSessionCase,
        _linked: &RunnerLinkedMachines,
    ) -> Result<Option<LinkedFramebufferUntilMatchOracle>, LinkedSessionExecutionError> {
        let LinkedSessionPassCondition::ParticipantFramebufferFixtureUntilMatch {
            participant_id,
            fixture_path,
            check_interval_tcycles,
            check_at_tcycles,
        } = &session.pass_condition
        else {
            return Ok(None);
        };
        let participant_index = session
            .participants
            .iter()
            .position(|participant| participant.id == *participant_id)
            .expect("linked session should validate target participant existence");
        let resolved_fixture = self.runner.resolve_path(fixture_path);
        let expected = decode_fixture_framebuffer_path(&resolved_fixture).map_err(|error| {
            let path = error.path.clone();
            LinkedSessionExecutionError::FileOperation {
                path,
                operation: "decode participant framebuffer fixture",
                source: Box::new(error.into_invalid_data_error()),
            }
        })?;

        Ok(Some(LinkedFramebufferUntilMatchOracle {
            participant_index,
            expected,
            check_interval_tcycles: *check_interval_tcycles,
            check_at_tcycles: *check_at_tcycles,
            pending_periodic_check: false,
            matched: false,
            check_at_reached: false,
        }))
    }
}

fn linked_framebuffer_until_match_poll_due(
    session_id: &str,
    linked: &RunnerLinkedMachines,
    executed_t_cycles: u64,
    oracle: &mut LinkedFramebufferUntilMatchOracle,
) -> Result<bool, LinkedSessionExecutionError> {
    if let Some(check_at_tcycles) = oracle.check_at_tcycles {
        if executed_t_cycles == check_at_tcycles {
            oracle.check_at_reached = true;
            oracle.matched = linked_framebuffer_matches_fixture(session_id, linked, oracle)?;
            return Ok(true);
        }
        return Ok(false);
    }

    if executed_t_cycles != 0 && executed_t_cycles.is_multiple_of(oracle.check_interval_tcycles) {
        oracle.pending_periodic_check = true;
    }

    if oracle.pending_periodic_check && linked.participant_in_vblank(oracle.participant_index) {
        oracle.pending_periodic_check = false;
        if linked_framebuffer_matches_fixture(session_id, linked, oracle)? {
            oracle.matched = true;
            return Ok(true);
        }
    }

    Ok(false)
}

fn linked_framebuffer_matches_fixture(
    session_id: &str,
    linked: &RunnerLinkedMachines,
    oracle: &LinkedFramebufferUntilMatchOracle,
) -> Result<bool, LinkedSessionExecutionError> {
    let actual = normalize_dmg_framebuffer(
        session_id,
        linked.participant_framebuffer(oracle.participant_index),
    )
    .map_err(|error| {
        let path = error.path.clone();
        LinkedSessionExecutionError::FileOperation {
            path,
            operation: "normalize participant framebuffer",
            source: Box::new(error.into_invalid_data_error()),
        }
    })?;
    Ok(actual == oracle.expected)
}

fn linked_budget_exhausted(
    timeout: Timeout,
    executed_t_cycles: u64,
    completed_frames: &[u32],
) -> bool {
    let max_completed_frames = completed_frames.iter().copied().max().unwrap_or(0);
    super::budget_exhausted(timeout, executed_t_cycles, max_completed_frames)
}
