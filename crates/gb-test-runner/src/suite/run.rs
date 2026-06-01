use std::fs;
use std::path::Path;

use gb_core::{
    CompatibilityPolicy, DMG_T_CYCLES_PER_FRAME, ExecutionMode, Machine, MachineConfig,
    StartupMode, TraceSummaryBuffer,
};
use rayon::prelude::*;

use crate::oracle::{FramebufferObservation, OracleObservations, OracleOutcome, OracleStep};

use super::model::{CaseRunReport, Report, SuiteCase, SuiteManifest, SuiteRunReport};
use super::status::store_root_for_report;

pub(super) fn run_suite(
    workspace_root: &Path,
    report: &Report,
    suite: &SuiteManifest,
) -> SuiteRunReport {
    let cases = suite
        .cases
        .par_iter()
        .map(|case| run_case(workspace_root, report, case))
        .collect();
    SuiteRunReport {
        suite_name: suite.suite_name.clone(),
        family: suite.family.clone(),
        cases,
    }
}

fn run_case(workspace_root: &Path, report: &Report, case: &SuiteCase) -> CaseRunReport {
    let rom_path = store_root_for_report(workspace_root, report)
        .join(&case.family)
        .join(&case.rom);
    let rom_bytes = match fs::read(&rom_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return CaseRunReport {
                id: case.id.clone(),
                rom: case.rom.clone(),
                passed: false,
                failure: Some(format!(
                    "failed to read ROM {}: {error}",
                    rom_path.display()
                )),
                executed_tcycles: 0,
            };
        }
    };

    let config = MachineConfig::new(case.console_model)
        .with_startup_mode(StartupMode::SkipBoot)
        .with_compatibility(compatibility_for_execution_mode(case.execution_mode));
    let mut machine = Machine::new_summary(config);
    if let Err(error) = machine.load_cartridge(rom_bytes) {
        return CaseRunReport {
            id: case.id.clone(),
            rom: case.rom.clone(),
            passed: false,
            failure: Some(format!(
                "failed to load cartridge {}: {error:?}",
                rom_path.display()
            )),
            executed_tcycles: 0,
        };
    }

    let timeout_tcycles = u64::from(case.timeout_frames).saturating_mul(DMG_T_CYCLES_PER_FRAME);
    let mut serial_bytes = Vec::new();
    let mut oracle = case.oracle.clone();
    for executed_tcycles in 1..=timeout_tcycles {
        machine.step_t_cycle();
        serial_bytes.extend(machine.take_serial_output_bytes());
        match oracle.observe(OracleObservations {
            serial: &serial_bytes,
            executed_tcycles,
            framebuffer: framebuffer_observation(&machine),
            participants: &[],
        }) {
            Ok(OracleStep::Continue) => {}
            Ok(OracleStep::Stop) => {
                return finish_case(case, oracle, &machine, &serial_bytes, executed_tcycles);
            }
            Err(error) => {
                return CaseRunReport {
                    id: case.id.clone(),
                    rom: case.rom.clone(),
                    passed: false,
                    failure: Some(error),
                    executed_tcycles,
                };
            }
        }
    }

    finish_case(case, oracle, &machine, &serial_bytes, timeout_tcycles)
}

fn compatibility_for_execution_mode(execution_mode: ExecutionMode) -> CompatibilityPolicy {
    match execution_mode {
        ExecutionMode::Strict => CompatibilityPolicy::strict(),
        ExecutionMode::Permissive => CompatibilityPolicy::permissive(),
        ExecutionMode::Experimental => CompatibilityPolicy::experimental(),
    }
}

fn finish_case(
    case: &SuiteCase,
    mut oracle: crate::oracle::Oracle,
    machine: &Machine<TraceSummaryBuffer>,
    serial_bytes: &[u8],
    executed_tcycles: u64,
) -> CaseRunReport {
    match oracle.finish(OracleObservations {
        serial: serial_bytes,
        executed_tcycles,
        framebuffer: framebuffer_observation(machine),
        participants: &[],
    }) {
        Ok(OracleOutcome::Passed) => CaseRunReport {
            id: case.id.clone(),
            rom: case.rom.clone(),
            passed: true,
            failure: None,
            executed_tcycles,
        },
        Ok(OracleOutcome::Failed(failure)) => CaseRunReport {
            id: case.id.clone(),
            rom: case.rom.clone(),
            passed: false,
            failure: Some(failure),
            executed_tcycles,
        },
        Err(error) => CaseRunReport {
            id: case.id.clone(),
            rom: case.rom.clone(),
            passed: false,
            failure: Some(error),
            executed_tcycles,
        },
    }
}

fn framebuffer_observation(machine: &Machine<TraceSummaryBuffer>) -> FramebufferObservation<'_> {
    FramebufferObservation {
        dmg: Some(machine.ppu().framebuffer()),
        cgb_rgb555: machine.ppu().cgb_framebuffer_rgb555(),
        in_vblank: machine.ppu().ly() >= 144,
    }
}
