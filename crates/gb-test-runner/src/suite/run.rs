use std::fs;
use std::path::Path;

use gb_core::{
    CompatibilityPolicy, ConsoleModel, DMG_T_CYCLES_PER_FRAME, Machine, MachineConfig, StartupMode,
};

use crate::oracle::OracleObservations;

use super::model::{CaseRunReport, Report, SuiteCase, SuiteManifest, SuiteRunReport};
use super::status::store_root_for_report;

pub(super) fn run_suite(
    workspace_root: &Path,
    report: &Report,
    suite: &SuiteManifest,
) -> SuiteRunReport {
    let cases = suite
        .cases
        .iter()
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

    let config = MachineConfig::new(ConsoleModel::GameBoy)
        .with_startup_mode(StartupMode::SkipBoot)
        .with_compatibility(CompatibilityPolicy::strict());
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
    for executed_tcycles in 1..=timeout_tcycles {
        machine.step_t_cycle();
        serial_bytes.extend(machine.take_serial_output_bytes());
        if case.oracle.matched(OracleObservations {
            serial: &serial_bytes,
        }) {
            return CaseRunReport {
                id: case.id.clone(),
                rom: case.rom.clone(),
                passed: true,
                failure: None,
                executed_tcycles,
            };
        }
    }

    CaseRunReport {
        id: case.id.clone(),
        rom: case.rom.clone(),
        passed: false,
        failure: Some(case.oracle.failure_message(OracleObservations {
            serial: &serial_bytes,
        })),
        executed_tcycles: timeout_tcycles,
    }
}
