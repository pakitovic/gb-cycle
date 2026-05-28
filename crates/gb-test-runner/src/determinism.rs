use std::fs;

use gb_core::{
    ConsoleModel, CpuDiagnosticTrap, CpuExecutionState, ExecutionMode, MachineSaveState,
};

use crate::{
    DMG_FAMILY_FRAME_T_CYCLES, RomExecutionError, RomRunner, RomTestCase, RunnerMachine,
    TestSubsystem, Timeout, encode_bytes_as_upper_hex,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeterminismCaseFailure {
    NonStrictExecutionMode {
        execution_mode: ExecutionMode,
    },
    ReplayStateMismatch,
    ReplaySerialMismatch {
        baseline_hex: String,
        replay_hex: String,
    },
    SaveLoadStateMismatch,
    MetadataGuardAcceptedMismatchedState,
    RestoreFailed {
        message: String,
    },
    CpuDiagnosticTrap {
        trap: CpuDiagnosticTrap,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeterminismCaseOutcome {
    Passed,
    Failed(DeterminismCaseFailure),
}

impl DeterminismCaseOutcome {
    pub const fn passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterminismCaseReport {
    pub case_id: String,
    pub outcome: DeterminismCaseOutcome,
    pub save_at_t_cycles: u64,
    pub continuation_t_cycles: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterminismSuiteReport {
    pub suite_name: String,
    pub subsystem: TestSubsystem,
    pub cases: Vec<DeterminismCaseReport>,
}

impl DeterminismSuiteReport {
    pub fn all_passed(&self) -> bool {
        self.cases.iter().all(|case| case.outcome.passed())
    }
}

#[derive(Debug)]
pub enum DeterminismExecutionError {
    InvalidCase,
    InvalidSuite,
}

struct DeterminismRunState {
    machine: RunnerMachine,
    serial_bytes: Vec<u8>,
    completed_frames: u32,
    at_frame_origin: bool,
    applied_stimuli: Vec<bool>,
}

#[derive(Debug, Clone)]
pub struct DeterminismRunner {
    rom_runner: RomRunner,
    save_at_t_cycles: Option<u64>,
    continuation_t_cycles: Option<u64>,
}

impl Default for DeterminismRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterminismRunner {
    pub fn new() -> Self {
        Self {
            rom_runner: RomRunner::new(),
            save_at_t_cycles: None,
            continuation_t_cycles: None,
        }
    }

    pub fn with_save_at_t_cycles(mut self, save_at_t_cycles: u64) -> Self {
        self.save_at_t_cycles = Some(save_at_t_cycles);
        self
    }

    pub fn with_continuation_t_cycles(mut self, continuation_t_cycles: u64) -> Self {
        self.continuation_t_cycles = Some(continuation_t_cycles);
        self
    }

    pub fn run_suite(
        &self,
        suite: &crate::RomSuite,
    ) -> Result<DeterminismSuiteReport, DeterminismExecutionError> {
        suite
            .validate()
            .map_err(|_| DeterminismExecutionError::InvalidSuite)?;

        let mut case_reports = Vec::with_capacity(suite.cases.len());
        for case in &suite.cases {
            case_reports.push(self.run_case(case)?);
        }

        Ok(DeterminismSuiteReport {
            suite_name: suite.name.clone(),
            subsystem: suite.subsystem,
            cases: case_reports,
        })
    }

    pub fn run_case(
        &self,
        case: &RomTestCase,
    ) -> Result<DeterminismCaseReport, DeterminismExecutionError> {
        case.validate()
            .map_err(|_| DeterminismExecutionError::InvalidCase)?;

        let (save_at_t_cycles, continuation_t_cycles) = self.case_windows(case);
        let outcome = self
            .evaluate_case(case, save_at_t_cycles, continuation_t_cycles)
            .unwrap_or_else(DeterminismCaseOutcome::Failed);

        Ok(DeterminismCaseReport {
            case_id: case.id.clone(),
            outcome,
            save_at_t_cycles,
            continuation_t_cycles,
        })
    }

    fn case_windows(&self, case: &RomTestCase) -> (u64, u64) {
        let default_total = match case.timeout {
            Timeout::TCycles(limit) => limit,
            Timeout::Frames(limit) => u64::from(limit).saturating_mul(DMG_FAMILY_FRAME_T_CYCLES),
        }
        .max(2);

        let default_save_at = default_total.saturating_div(2).clamp(1, 1_024);
        let default_continuation = default_total
            .saturating_sub(default_save_at)
            .clamp(1, 1_024);

        (
            self.save_at_t_cycles.unwrap_or(default_save_at),
            self.continuation_t_cycles.unwrap_or(default_continuation),
        )
    }

    fn evaluate_case(
        &self,
        case: &RomTestCase,
        save_at_t_cycles: u64,
        continuation_t_cycles: u64,
    ) -> Result<DeterminismCaseOutcome, DeterminismCaseFailure> {
        if case.execution_mode != ExecutionMode::Strict {
            return Ok(DeterminismCaseOutcome::Failed(
                DeterminismCaseFailure::NonStrictExecutionMode {
                    execution_mode: case.execution_mode,
                },
            ));
        }

        let total_t_cycles = save_at_t_cycles.saturating_add(continuation_t_cycles);
        let baseline = self.run_prefix(case, total_t_cycles)?;
        let replay = self.run_prefix(case, total_t_cycles)?;
        let baseline_state = baseline.machine.capture_save_state();
        let replay_state = replay.machine.capture_save_state();

        if baseline_state != replay_state {
            return Ok(DeterminismCaseOutcome::Failed(
                DeterminismCaseFailure::ReplayStateMismatch,
            ));
        }
        if baseline.serial_bytes != replay.serial_bytes {
            return Ok(DeterminismCaseOutcome::Failed(
                DeterminismCaseFailure::ReplaySerialMismatch {
                    baseline_hex: encode_bytes_as_upper_hex(&baseline.serial_bytes),
                    replay_hex: encode_bytes_as_upper_hex(&replay.serial_bytes),
                },
            ));
        }

        let mut restored = self.run_prefix(case, save_at_t_cycles)?;
        let save_state = restored.machine.capture_save_state();
        self.assert_mismatched_restore_rejected(case, &save_state)?;
        let saved_serial_len = restored.serial_bytes.len();
        let saved_completed_frames = restored.completed_frames;
        let saved_at_frame_origin = restored.at_frame_origin;
        let saved_applied_stimuli = restored.applied_stimuli.clone();

        self.step_run_state(case, &mut restored, 8)?;
        restored
            .machine
            .restore_save_state(&save_state)
            .map_err(|error| DeterminismCaseFailure::RestoreFailed {
                message: error.to_string(),
            })?;
        restored.serial_bytes.truncate(saved_serial_len);
        restored.completed_frames = saved_completed_frames;
        restored.at_frame_origin = saved_at_frame_origin;
        restored.applied_stimuli = saved_applied_stimuli;
        self.step_run_state(case, &mut restored, continuation_t_cycles)?;

        if restored.machine.capture_save_state() != baseline_state {
            return Ok(DeterminismCaseOutcome::Failed(
                DeterminismCaseFailure::SaveLoadStateMismatch,
            ));
        }

        Ok(DeterminismCaseOutcome::Passed)
    }

    fn run_prefix(
        &self,
        case: &RomTestCase,
        t_cycles: u64,
    ) -> Result<DeterminismRunState, DeterminismCaseFailure> {
        let machine =
            self.prepare_machine(case)
                .map_err(|error| DeterminismCaseFailure::RestoreFailed {
                    message: format!("{error:?}"),
                })?;
        let mut state = DeterminismRunState {
            at_frame_origin: machine.at_frame_origin(),
            machine,
            serial_bytes: Vec::new(),
            completed_frames: 0,
            applied_stimuli: vec![false; case.external_stimuli.stimuli().len()],
        };

        self.step_run_state(case, &mut state, t_cycles)?;
        Ok(state)
    }

    fn prepare_machine(&self, case: &RomTestCase) -> Result<RunnerMachine, RomExecutionError> {
        let rom_path = self.rom_runner.resolve_case_rom_path(case)?;
        let rom_bytes = fs::read(&rom_path).map_err(|source| RomExecutionError::ReadFile {
            path: rom_path.clone(),
            operation: "read ROM",
            source,
        })?;
        let boot_rom_assets = self.rom_runner.load_boot_rom_assets(case)?;
        let mut machine = RunnerMachine::new(case, boot_rom_assets);
        machine
            .load_cartridge(rom_bytes)
            .map_err(|source| RomExecutionError::CartridgeLoad {
                path: rom_path,
                source,
            })?;
        self.rom_runner
            .apply_startup_cartridge_state(case, &mut machine);
        self.rom_runner
            .apply_startup_timer_state(case, &mut machine);
        self.rom_runner
            .apply_startup_memory_writes(case, &mut machine);
        Ok(machine)
    }

    fn step_run_state(
        &self,
        case: &RomTestCase,
        state: &mut DeterminismRunState,
        t_cycles: u64,
    ) -> Result<(), DeterminismCaseFailure> {
        for _ in 0..t_cycles {
            self.rom_runner.apply_scheduled_stimuli(
                case,
                &mut state.machine,
                state.completed_frames,
                &mut state.applied_stimuli,
            );

            state.machine.step_t_cycle();
            state
                .serial_bytes
                .extend(state.machine.take_serial_output_bytes());

            let now_at_frame_origin = state.machine.at_frame_origin();
            if now_at_frame_origin && !state.at_frame_origin {
                state.completed_frames += 1;
            }
            state.at_frame_origin = now_at_frame_origin;

            if let CpuExecutionState::DiagnosticTrap { trap } = state.machine.cpu_execution_state()
            {
                return Err(DeterminismCaseFailure::CpuDiagnosticTrap { trap });
            }
        }

        Ok(())
    }

    fn assert_mismatched_restore_rejected(
        &self,
        case: &RomTestCase,
        save_state: &MachineSaveState,
    ) -> Result<(), DeterminismCaseFailure> {
        let mut mismatched_case = case.clone();
        mismatched_case.console_model = mismatched_console_model(case.console_model);
        let mut mismatched_machine = self.prepare_machine(&mismatched_case).map_err(|error| {
            DeterminismCaseFailure::RestoreFailed {
                message: format!("{error:?}"),
            }
        })?;

        match mismatched_machine.restore_save_state(save_state) {
            Ok(()) => Err(DeterminismCaseFailure::MetadataGuardAcceptedMismatchedState),
            Err(_) => Ok(()),
        }
    }
}

fn mismatched_console_model(model: ConsoleModel) -> ConsoleModel {
    match model {
        ConsoleModel::GameBoy => ConsoleModel::GameBoyPocket,
        ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight | ConsoleModel::GameBoyColor => {
            ConsoleModel::GameBoy
        }
    }
}

#[cfg(test)]
mod tests {
    use gb_core::{ConsoleModel, ExecutionMode};

    use crate::{PassCondition, RomSuite, RomTestCase, TestSubsystem, Timeout};

    use super::{
        DeterminismCaseFailure, DeterminismCaseOutcome, DeterminismExecutionError,
        DeterminismRunner, mismatched_console_model,
    };

    #[test]
    fn mismatched_console_model_keeps_restore_guard_inside_dmg_family() {
        assert_eq!(
            mismatched_console_model(ConsoleModel::GameBoy),
            ConsoleModel::GameBoyPocket
        );
        assert_eq!(
            mismatched_console_model(ConsoleModel::GameBoyPocket),
            ConsoleModel::GameBoy
        );
        assert_eq!(
            mismatched_console_model(ConsoleModel::GameBoyLight),
            ConsoleModel::GameBoy
        );
        assert_eq!(
            mismatched_console_model(ConsoleModel::GameBoyColor),
            ConsoleModel::GameBoy
        );
    }

    #[test]
    fn default_case_windows_are_derived_from_timeout_and_can_be_overridden() {
        let runner = DeterminismRunner::new();
        let tcycle_case = RomTestCase::new(
            "tcycles",
            "fixture.gb",
            Timeout::TCycles(256),
            PassCondition::SerialHexExact(String::new()),
        );
        assert_eq!(runner.case_windows(&tcycle_case), (128, 128));

        let tiny_case = RomTestCase::new(
            "tiny",
            "fixture.gb",
            Timeout::TCycles(1),
            PassCondition::SerialHexExact(String::new()),
        );
        assert_eq!(runner.case_windows(&tiny_case), (1, 1));

        let frame_case = RomTestCase::new(
            "frames",
            "fixture.gb",
            Timeout::Frames(2),
            PassCondition::SerialHexExact(String::new()),
        );
        assert_eq!(runner.case_windows(&frame_case), (1024, 1024));

        let overridden = DeterminismRunner::new()
            .with_save_at_t_cycles(7)
            .with_continuation_t_cycles(9);
        assert_eq!(overridden.case_windows(&frame_case), (7, 9));
    }

    #[test]
    fn run_suite_reports_invalid_suite_without_io() {
        let suite = RomSuite::new("", TestSubsystem::Cpu);
        assert!(matches!(
            DeterminismRunner::new().run_suite(&suite),
            Err(DeterminismExecutionError::InvalidSuite)
        ));
    }

    #[test]
    fn run_case_fails_non_strict_before_loading_rom() {
        let mut case = crate::phase_2_cpu_timing_suite().cases.remove(0);
        case.execution_mode = ExecutionMode::Experimental;
        let report = DeterminismRunner::new()
            .with_save_at_t_cycles(1)
            .with_continuation_t_cycles(1)
            .run_case(&case)
            .expect("valid non-strict case should produce a deterministic failure report");

        assert_eq!(
            report.outcome,
            DeterminismCaseOutcome::Failed(DeterminismCaseFailure::NonStrictExecutionMode {
                execution_mode: ExecutionMode::Experimental,
            })
        );
        assert_eq!(report.save_at_t_cycles, 1);
        assert_eq!(report.continuation_t_cycles, 1);
    }

    #[test]
    fn runner_executes_short_phase2_save_load_window() {
        let mut suite = crate::phase_2_cpu_timing_suite();
        suite.cases.truncate(1);
        suite.cases[0].timeout = Timeout::TCycles(2);

        let report = DeterminismRunner::new()
            .with_save_at_t_cycles(1)
            .with_continuation_t_cycles(1)
            .run_suite(&suite)
            .expect("short phase2 determinism suite should run");

        assert!(report.all_passed());
        assert_eq!(report.suite_name, "phase-2-cpu-timing");
        assert_eq!(report.subsystem, TestSubsystem::Cpu);
        assert_eq!(report.cases.len(), 1);
        assert_eq!(report.cases[0].case_id, "phase2-fetch-immediate-order");
        assert_eq!(report.cases[0].outcome, DeterminismCaseOutcome::Passed);
    }
}
