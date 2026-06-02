use std::fs;
use std::path::Path;

use gb_core::{
    CartridgeLoadError, CompatibilityPolicy, Dmg07Participant, LinkedMachines, Machine,
    MachineConfig, StartupMode, TraceSummaryBuffer,
};
use rayon::prelude::*;

use crate::oracle::{
    FramebufferObservation, LinkedParticipantObservation, LinkedSessionObservation,
    OracleObservations, OracleOutcome, OracleStep,
};
use crate::rtc::DeterministicMbc3RtcClock;

use super::artifact::{
    LinkFailureArtifactRequest, clean_case_artifacts, persist_failure_artifacts,
};
use super::manifest::report_data_dir;
use super::model::{
    LinkCaseRunReport, LinkParticipantArtifacts, LinkParticipantRunReport, LinkRunArtifacts,
    LinkRunConfig, LinkSuiteCase, LinkSuiteManifest, LinkSuiteRunReport, LinkTopology, Report,
};
use super::status::runtime_root_for_report;

const REAL_BOOT_HANDOFF_T_CYCLE_LIMIT: u64 = 25_000_000;

pub(super) fn run_link_suite_with_config(
    workspace_root: &Path,
    report: &Report,
    suite: &LinkSuiteManifest,
    config: &LinkRunConfig,
) -> LinkSuiteRunReport {
    let cases = suite
        .cases
        .par_iter()
        .map(|case| run_case(workspace_root, report, &suite.suite_name, case, config))
        .collect();
    LinkSuiteRunReport {
        suite_name: suite.suite_name.clone(),
        family: suite.family.clone(),
        cases,
    }
}

fn run_case(
    workspace_root: &Path,
    report: &Report,
    suite_name: &str,
    case: &LinkSuiteCase,
    run_config: &LinkRunConfig,
) -> LinkCaseRunReport {
    let context = LinkCaseContext {
        workspace_root,
        report,
        suite_name,
        case,
    };
    if let Err(error) = clean_case_artifacts(workspace_root, report, suite_name, &case.id) {
        return failed_case_report(context, error, 0, None, None);
    }

    let mut built = match build_linked_machines(workspace_root, report, case, run_config) {
        Ok(built) => built,
        Err(error) => return failed_case_report(context, error, 0, None, None),
    };
    let mut oracle = case.oracle.clone();
    let informational = oracle.is_informational();

    for executed_tcycles in 1..=case.timeout_tcycles {
        built.linked.step_t_cycle();
        tick_mbc3_rtc(&mut built);
        collect_serial_output(&mut built);
        match with_live_observations(&built, executed_tcycles, |observations| {
            oracle.observe(observations)
        }) {
            Ok(OracleStep::Continue) => {}
            Ok(OracleStep::Stop) => {
                return finish_case(context, oracle, built, executed_tcycles, informational);
            }
            Err(error) => {
                let artifacts = capture_artifacts(&built);
                return failed_case_report(
                    context,
                    error,
                    executed_tcycles,
                    Some(artifacts),
                    case.oracle.framebuffer_artifact_descriptor(),
                );
            }
        }
    }

    finish_case(context, oracle, built, case.timeout_tcycles, informational)
}

fn build_linked_machines(
    workspace_root: &Path,
    report: &Report,
    case: &LinkSuiteCase,
    run_config: &LinkRunConfig,
) -> Result<BuiltLinkedMachines, String> {
    let mut machines = Vec::with_capacity(case.participants.len());
    let mut rtc_clocks = Vec::with_capacity(case.participants.len());
    for participant in &case.participants {
        let rom_path = participant_rom_path(workspace_root, report, case, &participant.rom);
        let rom_bytes = fs::read(&rom_path)
            .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;
        let mut config = MachineConfig::new(participant.console_model)
            .with_revision(participant.hardware_revision)
            .with_host_platform(participant.host_platform)
            .with_startup_mode(participant.startup_mode)
            .with_compatibility(CompatibilityPolicy::strict());
        if let Some(boot_rom_assets) = &run_config.boot_rom_assets {
            config = config.with_boot_rom_assets(boot_rom_assets.clone());
        } else if participant.startup_mode == StartupMode::RealBoot {
            return Err(format!(
                "participant {:?} uses startup = \"real-boot\"; pass --boot-rom-dir <dir> to load verified boot ROM assets",
                participant.id
            ));
        }
        let mut machine = Machine::new_summary(config);
        load_cartridge(&mut machine, rom_bytes, &rom_path)?;
        let mut rtc_clock = DeterministicMbc3RtcClock::default();
        advance_real_boot_to_handoff_if_needed(
            &mut machine,
            participant.startup_mode,
            &mut rtc_clock,
        )?;
        machines.push(machine);
        rtc_clocks.push(rtc_clock);
    }

    let mut linked = LinkedMachines::new(machines)
        .map_err(|error| format!("failed to build linked machines: {error:?}"))?;
    attach_topology(&mut linked, case)?;
    Ok(BuiltLinkedMachines {
        linked,
        participant_ids: case
            .participants
            .iter()
            .map(|participant| participant.id.clone())
            .collect(),
        serial_bytes: vec![Vec::new(); case.participants.len()],
        rtc_clocks,
    })
}

fn load_cartridge(
    machine: &mut Machine<TraceSummaryBuffer>,
    rom_bytes: Vec<u8>,
    rom_path: &Path,
) -> Result<(), String> {
    machine
        .load_cartridge(rom_bytes)
        .map(|_| ())
        .map_err(|error: CartridgeLoadError| {
            format!("failed to load cartridge {}: {error:?}", rom_path.display())
        })
}

fn advance_real_boot_to_handoff_if_needed(
    machine: &mut Machine<TraceSummaryBuffer>,
    startup_mode: StartupMode,
    rtc_clock: &mut DeterministicMbc3RtcClock,
) -> Result<(), String> {
    if startup_mode != StartupMode::RealBoot || !machine.boot().is_boot_rom_mapped() {
        return Ok(());
    }

    for _ in 0..REAL_BOOT_HANDOFF_T_CYCLE_LIMIT {
        machine.step_t_cycle();
        let ticks = rtc_clock.tick_t_cycle_for_speed(machine.speed().current_speed());
        if ticks != 0 {
            machine.advance_mbc3_cartridge_rtc_clock_ticks(ticks);
        }
        if !machine.boot().is_boot_rom_mapped() {
            let _ = machine.take_serial_output_bytes();
            return Ok(());
        }
    }

    Err(format!(
        "real-boot handoff did not unmap boot ROM within {REAL_BOOT_HANDOFF_T_CYCLE_LIMIT} T-cycles"
    ))
}

fn attach_topology(
    linked: &mut LinkedMachines<TraceSummaryBuffer>,
    case: &LinkSuiteCase,
) -> Result<(), String> {
    match case.topology {
        LinkTopology::Dmg04 => linked
            .attach_dmg04_cable()
            .map_err(|error| format!("failed to attach DMG-04 cable: {error:?}")),
        LinkTopology::Dmg07 => {
            let participants = case
                .participants
                .iter()
                .enumerate()
                .map(|(machine_index, participant)| {
                    Dmg07Participant::new(
                        machine_index,
                        participant
                            .adapter_port
                            .expect("dmg07 participants should validate adapter_port"),
                    )
                })
                .collect::<Vec<_>>();
            linked
                .attach_dmg07_adapter(&participants)
                .map_err(|error| format!("failed to attach DMG-07 adapter: {error:?}"))
        }
        LinkTopology::CgbIr => linked
            .attach_cgb_infrared_pair()
            .map_err(|error| format!("failed to attach CGB infrared pair: {error:?}")),
    }
}

fn participant_rom_path(
    workspace_root: &Path,
    report: &Report,
    case: &LinkSuiteCase,
    rom: &Path,
) -> std::path::PathBuf {
    if report.local {
        return report_data_dir(workspace_root, report).join(rom);
    }
    runtime_root_for_report(workspace_root, report)
        .join(&case.target_root)
        .join(rom)
}

fn tick_mbc3_rtc(built: &mut BuiltLinkedMachines) {
    for (machine, rtc_clock) in built
        .linked
        .machines_mut()
        .iter_mut()
        .zip(built.rtc_clocks.iter_mut())
    {
        let ticks = rtc_clock.tick_t_cycle_for_speed(machine.speed().current_speed());
        if ticks != 0 {
            machine.advance_mbc3_cartridge_rtc_clock_ticks(ticks);
        }
    }
}

fn collect_serial_output(built: &mut BuiltLinkedMachines) {
    for (machine, serial_bytes) in built
        .linked
        .machines_mut()
        .iter_mut()
        .zip(built.serial_bytes.iter_mut())
    {
        serial_bytes.extend(machine.take_serial_output_bytes());
    }
}

fn finish_case(
    context: LinkCaseContext<'_>,
    mut oracle: crate::oracle::Oracle,
    built: BuiltLinkedMachines,
    executed_tcycles: u64,
    informational: bool,
) -> LinkCaseRunReport {
    let artifacts = capture_artifacts(&built);
    let observation_data = artifact_observation_data(&artifacts);
    match with_observations(&observation_data, executed_tcycles, |observations| {
        oracle.finish(observations)
    }) {
        Ok(OracleOutcome::Passed) => passed_case_report(context, informational, executed_tcycles),
        Ok(OracleOutcome::Failed(failure)) => failed_case_report(
            context,
            failure,
            executed_tcycles,
            Some(artifacts),
            oracle.framebuffer_artifact_descriptor(),
        ),
        Err(error) => failed_case_report(
            context,
            error,
            executed_tcycles,
            Some(artifacts),
            oracle.framebuffer_artifact_descriptor(),
        ),
    }
}

fn passed_case_report(
    context: LinkCaseContext<'_>,
    informational: bool,
    executed_tcycles: u64,
) -> LinkCaseRunReport {
    LinkCaseRunReport {
        id: context.case.id.clone(),
        passed: true,
        informational,
        failure: None,
        executed_tcycles,
        participants: participant_reports(context.case),
        failure_artifact_dir: None,
    }
}

fn failed_case_report(
    context: LinkCaseContext<'_>,
    failure: String,
    executed_tcycles: u64,
    artifacts: Option<LinkRunArtifacts>,
    framebuffer: Option<crate::oracle::FramebufferArtifactDescriptor>,
) -> LinkCaseRunReport {
    let (failure, failure_artifact_dir) = match artifacts {
        Some(artifacts) => match persist_failure_artifacts(LinkFailureArtifactRequest {
            workspace_root: context.workspace_root,
            report: context.report,
            suite_name: context.suite_name,
            case: context.case,
            failure: &failure,
            executed_tcycles,
            artifacts: &artifacts,
            framebuffer,
        }) {
            Ok(path) => (failure, Some(path)),
            Err(error) => (
                format!("{failure}; failed to write failure artifacts: {error}"),
                None,
            ),
        },
        None => (failure, None),
    };

    LinkCaseRunReport {
        id: context.case.id.clone(),
        passed: false,
        informational: false,
        failure: Some(failure),
        executed_tcycles,
        participants: participant_reports(context.case),
        failure_artifact_dir,
    }
}

fn participant_reports(case: &LinkSuiteCase) -> Vec<LinkParticipantRunReport> {
    case.participants
        .iter()
        .map(|participant| LinkParticipantRunReport {
            id: participant.id.clone(),
            rom: participant.rom.clone(),
        })
        .collect()
}

fn capture_artifacts(built: &BuiltLinkedMachines) -> LinkRunArtifacts {
    let participants = built
        .linked
        .machines()
        .iter()
        .zip(built.serial_bytes.iter())
        .enumerate()
        .map(|(index, (machine, serial_bytes))| {
            let id = index.to_string();
            LinkParticipantArtifacts {
                id,
                serial: serial_bytes.clone(),
                serial_hex: encode_bytes_as_upper_hex(serial_bytes),
                snapshot: Some(machine.snapshot().render_text()),
                trace: None,
                dmg_framebuffer: machine.ppu().framebuffer().to_vec(),
                cgb_framebuffer: machine
                    .ppu()
                    .cgb_framebuffer_rgb555()
                    .map(|framebuffer| framebuffer.to_vec())
                    .or_else(|| machine.sgb_lcd_framebuffer_rgb555()),
                in_vblank: machine.ppu().ly() >= 144,
            }
        })
        .collect::<Vec<_>>();
    let participants = participants_with_manifest_ids(built, participants);
    let topology_trace = built.linked.topology_trace_text();
    let session_trace = Some(render_combined_trace(
        topology_trace.as_deref(),
        &participants,
    ));
    let session_snapshot = Some(render_combined_snapshot(&participants));
    LinkRunArtifacts {
        session_snapshot,
        session_trace,
        topology_trace,
        participants,
    }
}

fn participants_with_manifest_ids(
    built: &BuiltLinkedMachines,
    mut participants: Vec<LinkParticipantArtifacts>,
) -> Vec<LinkParticipantArtifacts> {
    for (index, participant) in participants.iter_mut().enumerate() {
        participant.id = built.participant_ids[index].clone();
    }
    participants
}

fn render_combined_trace(
    topology_trace: Option<&str>,
    participants: &[LinkParticipantArtifacts],
) -> String {
    let mut rendered = String::new();
    if let Some(topology_trace) = topology_trace {
        rendered.push_str("== link topology trace ==\n");
        rendered.push_str(topology_trace);
        if !topology_trace.ends_with('\n') {
            rendered.push('\n');
        }
    }
    for participant in participants {
        rendered.push_str("== participant ");
        rendered.push_str(&participant.id);
        rendered.push_str(" trace ==\n");
        if let Some(trace) = &participant.trace {
            rendered.push_str(trace);
            if !trace.ends_with('\n') {
                rendered.push('\n');
            }
        }
    }
    rendered
}

fn render_combined_snapshot(participants: &[LinkParticipantArtifacts]) -> String {
    let mut rendered = String::new();
    for participant in participants {
        rendered.push_str("== participant ");
        rendered.push_str(&participant.id);
        rendered.push_str(" snapshot ==\n");
        if let Some(snapshot) = &participant.snapshot {
            rendered.push_str(snapshot);
            if !snapshot.ends_with('\n') {
                rendered.push('\n');
            }
        }
    }
    rendered
}

fn with_live_observations<R>(
    built: &BuiltLinkedMachines,
    executed_tcycles: u64,
    f: impl FnOnce(OracleObservations<'_>) -> R,
) -> R {
    let machines = built.linked.machines();
    let serial_hex = built
        .serial_bytes
        .iter()
        .map(|serial| encode_bytes_as_upper_hex(serial))
        .collect::<Vec<_>>();
    let sgb_framebuffers = machines
        .iter()
        .map(|machine| machine.sgb_lcd_framebuffer_rgb555())
        .collect::<Vec<_>>();
    let participants = machines
        .iter()
        .enumerate()
        .map(|(index, machine)| LinkedParticipantObservation {
            id: &built.participant_ids[index],
            serial: &built.serial_bytes[index],
            serial_hex: &serial_hex[index],
            snapshot: None,
            trace: None,
            framebuffer: FramebufferObservation {
                dmg: Some(machine.ppu().framebuffer()),
                cgb_rgb555: machine
                    .ppu()
                    .cgb_framebuffer_rgb555()
                    .or(sgb_framebuffers[index].as_deref()),
                in_vblank: machine.ppu().ly() >= 144,
            },
        })
        .collect::<Vec<_>>();
    let linked = LinkedSessionObservation {
        snapshot: None,
        trace: None,
        topology_trace: None,
        participants: &participants,
    };
    f(OracleObservations {
        serial: &[],
        cpu: None,
        memory: &[],
        executed_tcycles,
        framebuffer: FramebufferObservation {
            dmg: None,
            cgb_rgb555: None,
            in_vblank: false,
        },
        participants: &[],
        linked: Some(linked),
    })
}

fn artifact_observation_data(artifacts: &LinkRunArtifacts) -> LinkObservationData {
    LinkObservationData {
        participant_ids: artifacts
            .participants
            .iter()
            .map(|participant| participant.id.clone())
            .collect(),
        serial_bytes: artifacts
            .participants
            .iter()
            .map(|participant| participant.serial.clone())
            .collect(),
        serial_hex: artifacts
            .participants
            .iter()
            .map(|participant| participant.serial_hex.clone())
            .collect(),
        snapshots: artifacts
            .participants
            .iter()
            .map(|participant| participant.snapshot.clone())
            .collect(),
        traces: artifacts
            .participants
            .iter()
            .map(|participant| participant.trace.clone())
            .collect(),
        dmg_framebuffers: artifacts
            .participants
            .iter()
            .map(|participant| participant.dmg_framebuffer.clone())
            .collect(),
        cgb_framebuffers: artifacts
            .participants
            .iter()
            .map(|participant| participant.cgb_framebuffer.clone())
            .collect(),
        in_vblank: artifacts
            .participants
            .iter()
            .map(|participant| participant.in_vblank)
            .collect(),
        session_snapshot: artifacts.session_snapshot.clone(),
        session_trace: artifacts.session_trace.clone(),
        topology_trace: artifacts.topology_trace.clone(),
    }
}

struct LinkObservationData {
    participant_ids: Vec<String>,
    serial_bytes: Vec<Vec<u8>>,
    serial_hex: Vec<String>,
    snapshots: Vec<Option<String>>,
    traces: Vec<Option<String>>,
    dmg_framebuffers: Vec<Vec<u8>>,
    cgb_framebuffers: Vec<Option<Vec<u16>>>,
    in_vblank: Vec<bool>,
    session_snapshot: Option<String>,
    session_trace: Option<String>,
    topology_trace: Option<String>,
}

fn with_observations<R>(
    data: &LinkObservationData,
    executed_tcycles: u64,
    f: impl FnOnce(OracleObservations<'_>) -> R,
) -> R {
    let participants = data
        .participant_ids
        .iter()
        .enumerate()
        .map(|(index, id)| LinkedParticipantObservation {
            id,
            serial: &data.serial_bytes[index],
            serial_hex: &data.serial_hex[index],
            snapshot: data.snapshots[index].as_deref(),
            trace: data.traces[index].as_deref(),
            framebuffer: FramebufferObservation {
                dmg: Some(&data.dmg_framebuffers[index]),
                cgb_rgb555: data.cgb_framebuffers[index].as_deref(),
                in_vblank: data.in_vblank[index],
            },
        })
        .collect::<Vec<_>>();
    let linked = LinkedSessionObservation {
        snapshot: data.session_snapshot.as_deref(),
        trace: data.session_trace.as_deref(),
        topology_trace: data.topology_trace.as_deref(),
        participants: &participants,
    };
    f(OracleObservations {
        serial: &[],
        cpu: None,
        memory: &[],
        executed_tcycles,
        framebuffer: FramebufferObservation {
            dmg: None,
            cgb_rgb555: None,
            in_vblank: false,
        },
        participants: &[],
        linked: Some(linked),
    })
}

fn encode_bytes_as_upper_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0F)] as char);
    }
    encoded
}

#[derive(Clone, Copy)]
struct LinkCaseContext<'a> {
    workspace_root: &'a Path,
    report: &'a Report,
    suite_name: &'a str,
    case: &'a LinkSuiteCase,
}

struct BuiltLinkedMachines {
    linked: LinkedMachines<TraceSummaryBuffer>,
    participant_ids: Vec<String>,
    serial_bytes: Vec<Vec<u8>>,
    rtc_clocks: Vec<DeterministicMbc3RtcClock>,
}
