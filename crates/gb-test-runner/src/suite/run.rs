use std::fs;
use std::path::Path;

use gb_core::{
    CartridgeMappedRomSource, CompatibilityPolicy, DMG_T_CYCLES_PER_FRAME, ExecutionMode, Machine,
    MachineConfig, TraceSummaryBuffer,
};
use rayon::prelude::*;

use crate::oracle::{
    CPU_OBSERVATION_WINDOW_BACKTRACK, CPU_OBSERVATION_WINDOW_BYTES, CpuObservation,
    FramebufferObservation, MemoryObservation, OracleObservations, OracleOutcome, OracleStep,
};

use super::artifact::{FailureArtifactRequest, clean_case_artifacts, persist_failure_artifacts};
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
        .map(|case| run_case(workspace_root, report, &suite.suite_name, case))
        .collect();
    SuiteRunReport {
        suite_name: suite.suite_name.clone(),
        family: suite.family.clone(),
        cases,
    }
}

fn run_case(
    workspace_root: &Path,
    report: &Report,
    suite_name: &str,
    case: &SuiteCase,
) -> CaseRunReport {
    let context = CaseContext {
        workspace_root,
        report,
        suite_name,
        suite_case: case,
    };
    if let Err(error) = clean_case_artifacts(workspace_root, report, suite_name, &case.id) {
        return CaseRunReport {
            id: case.id.clone(),
            rom: case.rom.clone(),
            passed: false,
            failure: Some(error),
            executed_tcycles: 0,
            failure_artifact_dir: None,
        };
    }

    let rom_path = store_root_for_report(workspace_root, report)
        .join(&case.target_root)
        .join(&case.rom);
    let needs_cpu_observation = case.oracle.needs_cpu_observation();
    let memory_addresses = case.oracle.memory_addresses();
    let rom_bytes = match fs::read(&rom_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return failed_case_report(
                FailureReportContext {
                    case: context,
                    machine: None,
                    serial_bytes: &[],
                },
                format!("failed to read ROM {}: {error}", rom_path.display()),
                0,
            );
        }
    };
    let observation_rom_bytes = needs_cpu_observation.then(|| rom_bytes.clone());

    let config = MachineConfig::new(case.console_model)
        .with_host_platform(case.host_platform)
        .with_startup_mode(case.startup_mode)
        .with_compatibility(compatibility_for_execution_mode(case.execution_mode));
    let mut machine = Machine::new_summary(config);
    if let Err(error) = machine.load_cartridge(rom_bytes) {
        return failed_case_report(
            FailureReportContext {
                case: context,
                machine: Some(&machine),
                serial_bytes: &[],
            },
            format!("failed to load cartridge {}: {error:?}", rom_path.display()),
            0,
        );
    }

    let timeout_tcycles = u64::from(case.timeout_frames).saturating_mul(DMG_T_CYCLES_PER_FRAME);
    let mut serial_bytes = Vec::new();
    let mut oracle = case.oracle.clone();
    for executed_tcycles in 1..=timeout_tcycles {
        machine.step_t_cycle();
        serial_bytes.extend(machine.take_serial_output_bytes());
        let memory_observations = memory_observations(&mut machine, &memory_addresses);
        match oracle.observe(OracleObservations {
            serial: &serial_bytes,
            cpu: observation_rom_bytes
                .as_deref()
                .map(|rom_bytes| cpu_observation(&machine, rom_bytes)),
            memory: &memory_observations,
            executed_tcycles,
            framebuffer: framebuffer_observation(&machine),
            participants: &[],
        }) {
            Ok(OracleStep::Continue) => {}
            Ok(OracleStep::Stop) => {
                return finish_case(
                    context,
                    oracle,
                    &mut machine,
                    FinishCaseContext {
                        observation_rom_bytes: observation_rom_bytes.as_deref(),
                        memory_addresses: &memory_addresses,
                        serial_bytes: &serial_bytes,
                        executed_tcycles,
                    },
                );
            }
            Err(error) => {
                return failed_case_report(
                    FailureReportContext {
                        case: context,
                        machine: Some(&machine),
                        serial_bytes: &serial_bytes,
                    },
                    error,
                    executed_tcycles,
                );
            }
        }
    }

    finish_case(
        context,
        oracle,
        &mut machine,
        FinishCaseContext {
            observation_rom_bytes: observation_rom_bytes.as_deref(),
            memory_addresses: &memory_addresses,
            serial_bytes: &serial_bytes,
            executed_tcycles: timeout_tcycles,
        },
    )
}

fn compatibility_for_execution_mode(execution_mode: ExecutionMode) -> CompatibilityPolicy {
    match execution_mode {
        ExecutionMode::Strict => CompatibilityPolicy::strict(),
        ExecutionMode::Permissive => CompatibilityPolicy::permissive(),
        ExecutionMode::Experimental => CompatibilityPolicy::experimental(),
    }
}

fn finish_case(
    context: CaseContext<'_>,
    mut oracle: crate::oracle::Oracle,
    machine: &mut Machine<TraceSummaryBuffer>,
    finish: FinishCaseContext<'_>,
) -> CaseRunReport {
    let memory_observations = memory_observations(machine, finish.memory_addresses);
    match oracle.finish(OracleObservations {
        serial: finish.serial_bytes,
        cpu: finish
            .observation_rom_bytes
            .map(|rom_bytes| cpu_observation(machine, rom_bytes)),
        memory: &memory_observations,
        executed_tcycles: finish.executed_tcycles,
        framebuffer: framebuffer_observation(machine),
        participants: &[],
    }) {
        Ok(OracleOutcome::Passed) => CaseRunReport {
            id: context.suite_case.id.clone(),
            rom: context.suite_case.rom.clone(),
            passed: true,
            failure: None,
            executed_tcycles: finish.executed_tcycles,
            failure_artifact_dir: None,
        },
        Ok(OracleOutcome::Failed(failure)) => failed_case_report(
            FailureReportContext {
                case: context,
                machine: Some(machine),
                serial_bytes: finish.serial_bytes,
            },
            failure,
            finish.executed_tcycles,
        ),
        Err(error) => failed_case_report(
            FailureReportContext {
                case: context,
                machine: Some(machine),
                serial_bytes: finish.serial_bytes,
            },
            error,
            finish.executed_tcycles,
        ),
    }
}

#[derive(Clone, Copy)]
struct CaseContext<'a> {
    workspace_root: &'a Path,
    report: &'a Report,
    suite_name: &'a str,
    suite_case: &'a SuiteCase,
}

struct FinishCaseContext<'a> {
    observation_rom_bytes: Option<&'a [u8]>,
    memory_addresses: &'a [u16],
    serial_bytes: &'a [u8],
    executed_tcycles: u64,
}

struct FailureReportContext<'a> {
    case: CaseContext<'a>,
    machine: Option<&'a Machine<TraceSummaryBuffer>>,
    serial_bytes: &'a [u8],
}

fn failed_case_report(
    context: FailureReportContext<'_>,
    failure: String,
    executed_tcycles: u64,
) -> CaseRunReport {
    let artifact_result = persist_failure_artifacts(FailureArtifactRequest {
        workspace_root: context.case.workspace_root,
        report: context.case.report,
        suite_name: context.case.suite_name,
        case: context.case.suite_case,
        failure: &failure,
        executed_tcycles,
        serial_bytes: context.serial_bytes,
        machine: context.machine,
    });
    let (failure, failure_artifact_dir) = match artifact_result {
        Ok(path) => (failure, Some(path)),
        Err(error) => (
            format!("{failure}; failed to write failure artifacts: {error}"),
            None,
        ),
    };

    CaseRunReport {
        id: context.case.suite_case.id.clone(),
        rom: context.case.suite_case.rom.clone(),
        passed: false,
        failure: Some(failure),
        executed_tcycles,
        failure_artifact_dir,
    }
}

fn memory_observations(
    machine: &mut Machine<TraceSummaryBuffer>,
    addresses: &[u16],
) -> Vec<MemoryObservation> {
    addresses
        .iter()
        .copied()
        .map(|address| MemoryObservation {
            address,
            value: machine.read_bus(address),
        })
        .collect()
}

fn framebuffer_observation(machine: &Machine<TraceSummaryBuffer>) -> FramebufferObservation<'_> {
    FramebufferObservation {
        dmg: Some(machine.ppu().framebuffer()),
        cgb_rgb555: machine.ppu().cgb_framebuffer_rgb555(),
        in_vblank: machine.ppu().ly() >= 144,
    }
}

fn cpu_observation(machine: &Machine<TraceSummaryBuffer>, rom_bytes: &[u8]) -> CpuObservation {
    let snapshot = machine.cpu().snapshot();
    let mut pc_window = [0xFF; CPU_OBSERVATION_WINDOW_BYTES];
    let window_start = snapshot
        .registers
        .pc
        .wrapping_sub(CPU_OBSERVATION_WINDOW_BACKTRACK as u16);
    for (offset, byte) in pc_window.iter_mut().enumerate() {
        let address = window_start.wrapping_add(offset as u16);
        *byte = mapped_rom_byte(machine, rom_bytes, address);
    }

    CpuObservation {
        b: snapshot.registers.b,
        c: snapshot.registers.c,
        d: snapshot.registers.d,
        e: snapshot.registers.e,
        h: snapshot.registers.h,
        l: snapshot.registers.l,
        pc: snapshot.registers.pc,
        current_opcode: snapshot.current_opcode,
        pc_window,
    }
}

fn mapped_rom_byte(machine: &Machine<TraceSummaryBuffer>, rom_bytes: &[u8], address: u16) -> u8 {
    let Some(window) = machine.cartridge().mapped_rom_window(address) else {
        return 0xFF;
    };
    if window.source != CartridgeMappedRomSource::Rom {
        return 0xFF;
    }
    let offset = window
        .bank
        .saturating_mul(window.bank_size)
        .saturating_add(window.bank_offset);
    rom_bytes.get(offset).copied().unwrap_or(0xFF)
}
