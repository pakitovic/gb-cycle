//! Diagnostic-only PPU probes.
//!
//! Policy:
//! - stable, cheap oracles stay active in the owning family module
//! - ignored ad-hoc probes live here and use `#[ignore = "diag: ..."]`
//! - stale probes should be deleted instead of preserved as historical noise

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaidPpuScanlineBgpLineObservation {
    raw_pixels_prefix: [u8; 32],
    panel_pixels_prefix: [u8; 32],
    visible_bgp_writes: Vec<(u16, u8, u8)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaidPpuScanlineBgpDotObservation {
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
    bgp: u8,
    visible_bgp: u8,
    pipeline_bgp: u8,
    bgp_cpu_commit_output_palette_override: Option<u8>,
    bgp_cpu_commit_output_delay_pixels_remaining: u8,
    visible_pixels_output: u8,
    panel_pixels_prefix: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaidPpuScanlineBgpCpuPhaseObservation {
    ly: u8,
    line_dot: u16,
    pc: u16,
    hl: u16,
    execution_state: String,
    ff47: u8,
    visible_pixels_output: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaidPpuScanlineBgpWakeObservation {
    tag: &'static str,
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
    pc: u16,
    hl: u16,
    execution_state: String,
    ime: bool,
    delayed_ime_enable: bool,
    interrupt_flags: u8,
    interrupt_enable: u8,
    ff47: u8,
    visible_pixels_output: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaidPpuScanlineBgpFullLineObservation {
    mixed_colors: Vec<u8>,
    raw_pixels: Vec<u8>,
    panel_pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaidPpuScanlineBgpBoundaryRowFamilyObservation {
    ly: u8,
    visible_bgp_row_values: Vec<u8>,
    visible_bgp_hl_range: Option<(u16, u16)>,
    panel_runs: Vec<(u8, u8, u8)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaidPpuScanlineBgpFrameBoundaryObservation {
    completed_frame: usize,
    ly: u8,
    panel_runs: Vec<(u8, u8, u8)>,
}

const DAID_SCANLINE_BGP_ACCEPTED_LY23_PANEL_PREFIXES: [[u8; 32]; 3] = [
    [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        1, 1,
    ],
    [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3,
        1, 1,
    ],
    [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1,
        1, 1,
    ],
];

fn load_daid_ppu_scanline_bgp_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/daid/ppu_scanline_bgp.gb");
    let rom = std::fs::read(&rom_path).expect("daid ppu_scanline_bgp ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn sample_daid_ppu_scanline_bgp_line(target_ly: u8) -> DaidPpuScanlineBgpLineObservation {
    sample_daid_ppu_scanline_bgp_lines(&[target_ly])
        .remove(&target_ly)
        .expect("target line should be sampled")
}

fn sample_daid_ppu_scanline_bgp_full_line(target_ly: u8) -> DaidPpuScanlineBgpFullLineObservation {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let snapshot = machine.ppu().snapshot();

        if snapshot.ly != 0 || snapshot.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps == 1 && snapshot.ly == target_ly && snapshot.mode == PpuAccessMode::HBlank {
            let framebuffer_row_start = snapshot.ly as usize * 160;
            return DaidPpuScanlineBgpFullLineObservation {
                mixed_colors: snapshot.current_scanline_mixed_colors,
                raw_pixels: snapshot.current_scanline_pixels,
                panel_pixels: machine.ppu().framebuffer()
                    [framebuffer_row_start..framebuffer_row_start + 160]
                    .to_vec(),
            };
        }
    }

    panic!("timed out before sampling requested full daid ppu_scanline_bgp line");
}

fn sample_daid_ppu_scanline_bgp_lines(
    target_lys: &[u8],
) -> std::collections::BTreeMap<u8, DaidPpuScanlineBgpLineObservation> {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut visible_bgp_writes = std::collections::BTreeMap::<u8, Vec<(u16, u8, u8)>>::new();
    let mut observations = std::collections::BTreeMap::new();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let snapshot = machine.ppu().snapshot();

        if snapshot.ly != 0 || snapshot.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF47)
            && wraps == 1
            && targets.contains(&snapshot.ly)
        {
            visible_bgp_writes.entry(snapshot.ly).or_default().push((
                snapshot.line_dot,
                snapshot.visible_pixels_output,
                machine.read_bus(0xFF47),
            ));
        }

        if wraps == 1
            && targets.contains(&snapshot.ly)
            && snapshot.mode == PpuAccessMode::HBlank
            && !observations.contains_key(&snapshot.ly)
        {
            let mut raw_pixels_prefix = [0_u8; 32];
            raw_pixels_prefix.copy_from_slice(&snapshot.current_scanline_pixels[..32]);

            let framebuffer_row_start = snapshot.ly as usize * 160;
            let mut panel_pixels_prefix = [0_u8; 32];
            panel_pixels_prefix.copy_from_slice(
                &machine.ppu().framebuffer()[framebuffer_row_start..framebuffer_row_start + 32],
            );

            observations.insert(
                snapshot.ly,
                DaidPpuScanlineBgpLineObservation {
                    raw_pixels_prefix,
                    panel_pixels_prefix,
                    visible_bgp_writes: visible_bgp_writes.remove(&snapshot.ly).unwrap_or_default(),
                },
            );
            if observations.len() == targets.len() {
                return observations;
            }
        }
    }

    panic!("timed out before sampling requested daid ppu_scanline_bgp lines");
}

fn sample_daid_ppu_scanline_bgp_dots(
    targets: &[(u8, u16)],
) -> Vec<DaidPpuScanlineBgpDotObservation> {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut observations = Vec::new();

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let snapshot = machine.ppu().snapshot();

        if snapshot.ly != 0 || snapshot.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps != 1 {
            continue;
        }

        if targets
            .iter()
            .any(|&(ly, line_dot)| ly == snapshot.ly && line_dot == snapshot.line_dot)
        {
            let framebuffer_row_start = snapshot.ly as usize * 160;
            let mut panel_pixels_prefix = [0_u8; 32];
            panel_pixels_prefix.copy_from_slice(
                &machine.ppu().framebuffer()[framebuffer_row_start..framebuffer_row_start + 32],
            );
            observations.push(DaidPpuScanlineBgpDotObservation {
                ly: snapshot.ly,
                line_dot: snapshot.line_dot,
                mode: snapshot.mode,
                bgp: snapshot.bgp,
                visible_bgp: snapshot.visible_bgp,
                pipeline_bgp: snapshot.pipeline_bgp,
                bgp_cpu_commit_output_palette_override: snapshot
                    .dmg_bgp_cpu_commit_output_palette_override,
                bgp_cpu_commit_output_delay_pixels_remaining: snapshot
                    .dmg_bgp_cpu_commit_output_delay_pixels_remaining,
                visible_pixels_output: snapshot.visible_pixels_output,
                panel_pixels_prefix,
            });
        }

        if observations.len() == targets.len() {
            break;
        }
    }

    observations
}

fn sample_daid_ppu_scanline_bgp_cpu_phase_window() -> Vec<DaidPpuScanlineBgpCpuPhaseObservation> {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut observations = Vec::new();

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();

        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps != 1 {
            continue;
        }

        let line_boundary =
            (22..=24).contains(&ppu.ly) && matches!(ppu.line_dot, 0 | 1 | 84 | 85 | 100);
        let ff47_write = machine.cpu().last_address_event().is_some_and(|event| {
            event.kind == CpuAddressEventKind::Write && event.access_address == Some(0xFF47)
        }) && (22..=24).contains(&ppu.ly);

        if line_boundary || ff47_write {
            observations.push(DaidPpuScanlineBgpCpuPhaseObservation {
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                pc: cpu.registers.pc,
                hl: u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l),
                execution_state: format!("{:?}", cpu.execution_state),
                ff47: machine.read_bus(0xFF47),
                visible_pixels_output: ppu.visible_pixels_output,
            });
        }

        if ppu.ly == 24 && ppu.mode == PpuAccessMode::HBlank && ppu.line_dot >= 252 {
            break;
        }
    }

    observations
}

fn sample_daid_ppu_scanline_bgp_line0_wake_and_first_loop_row()
-> Vec<DaidPpuScanlineBgpWakeObservation> {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut armed = false;
    let mut saw_wake = false;
    let mut saw_service = false;
    let mut writes_after_wake = 0usize;
    let mut previous_execution_state = machine.cpu().execution_state();
    let mut observations = Vec::new();

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();
        let interrupts = machine.interrupts().snapshot();
        let ff47_write = machine.cpu().last_address_event().is_some_and(|event| {
            event.kind == CpuAddressEventKind::Write && event.access_address == Some(0xFF47)
        });

        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps > 1 {
            break;
        }

        if !armed
            && wraps == 0
            && ppu.ly >= 145
            && ppu.mode == PpuAccessMode::VBlank
            && cpu.execution_state == gb_core::CpuExecutionState::Halted
        {
            armed = true;
            observations.push(DaidPpuScanlineBgpWakeObservation {
                tag: "armed",
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                pc: cpu.registers.pc,
                hl: u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l),
                execution_state: format!("{:?}", cpu.execution_state),
                ime: cpu.ime,
                delayed_ime_enable: cpu.delayed_ime_enable,
                interrupt_flags: interrupts.interrupt_flags,
                interrupt_enable: interrupts.interrupt_enable,
                ff47: machine.read_bus(0xFF47),
                visible_pixels_output: ppu.visible_pixels_output,
            });
        }

        if !armed {
            previous_execution_state = cpu.execution_state;
            continue;
        }

        if !saw_wake
            && previous_execution_state == gb_core::CpuExecutionState::Halted
            && cpu.execution_state != gb_core::CpuExecutionState::Halted
        {
            saw_wake = true;
            observations.push(DaidPpuScanlineBgpWakeObservation {
                tag: "wake",
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                pc: cpu.registers.pc,
                hl: u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l),
                execution_state: format!("{:?}", cpu.execution_state),
                ime: cpu.ime,
                delayed_ime_enable: cpu.delayed_ime_enable,
                interrupt_flags: interrupts.interrupt_flags,
                interrupt_enable: interrupts.interrupt_enable,
                ff47: machine.read_bus(0xFF47),
                visible_pixels_output: ppu.visible_pixels_output,
            });
        }

        if saw_wake
            && !saw_service
            && matches!(
                cpu.execution_state,
                gb_core::CpuExecutionState::ServiceInterrupt { .. }
            )
        {
            saw_service = true;
            observations.push(DaidPpuScanlineBgpWakeObservation {
                tag: "service",
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                pc: cpu.registers.pc,
                hl: u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l),
                execution_state: format!("{:?}", cpu.execution_state),
                ime: cpu.ime,
                delayed_ime_enable: cpu.delayed_ime_enable,
                interrupt_flags: interrupts.interrupt_flags,
                interrupt_enable: interrupts.interrupt_enable,
                ff47: machine.read_bus(0xFF47),
                visible_pixels_output: ppu.visible_pixels_output,
            });
        }

        if saw_wake && ff47_write {
            writes_after_wake += 1;
            observations.push(DaidPpuScanlineBgpWakeObservation {
                tag: "ff47",
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                pc: cpu.registers.pc,
                hl: u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l),
                execution_state: format!("{:?}", cpu.execution_state),
                ime: cpu.ime,
                delayed_ime_enable: cpu.delayed_ime_enable,
                interrupt_flags: interrupts.interrupt_flags,
                interrupt_enable: interrupts.interrupt_enable,
                ff47: machine.read_bus(0xFF47),
                visible_pixels_output: ppu.visible_pixels_output,
            });
            if writes_after_wake >= 12 {
                break;
            }
        }

        previous_execution_state = cpu.execution_state;
    }

    observations
}

fn summarize_panel_runs(panel_pixels: &[u8]) -> Vec<(u8, u8, u8)> {
    if panel_pixels.is_empty() {
        return Vec::new();
    }

    let mut runs = Vec::new();
    let mut start = 0usize;
    let mut current = panel_pixels[0];

    for (index, &pixel) in panel_pixels.iter().enumerate().skip(1) {
        if pixel != current {
            runs.push((start as u8, (index - 1) as u8, current));
            start = index;
            current = pixel;
        }
    }

    runs.push((start as u8, (panel_pixels.len() - 1) as u8, current));
    runs
}

fn sample_daid_ppu_scanline_bgp_boundary_row_family_transition(
    target_lys: &[u8],
) -> Vec<DaidPpuScanlineBgpBoundaryRowFamilyObservation> {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut visible_bgp_row_values = std::collections::BTreeMap::<u8, Vec<u8>>::new();
    let mut visible_bgp_hls = std::collections::BTreeMap::<u8, Vec<u16>>::new();
    let mut completed = std::collections::BTreeSet::new();
    let mut observations = Vec::new();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();

        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps != 1 {
            continue;
        }

        if machine.cpu().last_address_event().is_some_and(|event| {
            event.kind == CpuAddressEventKind::Write
                && event.access_address == Some(0xFF47)
                && targets.contains(&ppu.ly)
        }) {
            visible_bgp_row_values
                .entry(ppu.ly)
                .or_default()
                .push(machine.read_bus(0xFF47));
            visible_bgp_hls
                .entry(ppu.ly)
                .or_default()
                .push(u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l));
        }

        if targets.contains(&ppu.ly)
            && ppu.mode == PpuAccessMode::HBlank
            && completed.insert(ppu.ly)
        {
            let framebuffer_row_start = ppu.ly as usize * 160;
            let panel_pixels =
                &machine.ppu().framebuffer()[framebuffer_row_start..framebuffer_row_start + 160];
            let hls = visible_bgp_hls.remove(&ppu.ly).unwrap_or_default();
            let hl_range = hls.first().copied().zip(hls.last().copied());
            observations.push(DaidPpuScanlineBgpBoundaryRowFamilyObservation {
                ly: ppu.ly,
                visible_bgp_row_values: visible_bgp_row_values.remove(&ppu.ly).unwrap_or_default(),
                visible_bgp_hl_range: hl_range,
                panel_runs: summarize_panel_runs(panel_pixels),
            });
            if observations.len() == targets.len() {
                observations.sort_by_key(|observation| observation.ly);
                return observations;
            }
        }
    }

    panic!("timed out before sampling requested daid ppu_scanline_bgp boundary row families");
}

fn sample_daid_ppu_scanline_bgp_completed_frame_boundary_lines(
    frame_count: usize,
    target_lys: &[u8],
) -> Vec<DaidPpuScanlineBgpFrameBoundaryObservation> {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut observations = Vec::new();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();

        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
            continue;
        }

        if !saw_progress {
            continue;
        }

        for &target_ly in target_lys {
            let framebuffer_row_start = target_ly as usize * 160;
            let panel_pixels =
                &machine.ppu().framebuffer()[framebuffer_row_start..framebuffer_row_start + 160];
            observations.push(DaidPpuScanlineBgpFrameBoundaryObservation {
                completed_frame: wraps,
                ly: target_ly,
                panel_runs: summarize_panel_runs(panel_pixels),
            });
        }

        wraps += 1;
        if wraps == frame_count {
            return observations;
        }
    }

    panic!("timed out before sampling completed daid ppu_scanline_bgp frames");
}

#[test]
#[ignore = "diag: mooneye intr_2_oam_ok_timing seam"]
fn mode2_to_oam_release_probe_matches_mooneye_counts() {
    let delay46 = run_intr_2_oam_ok_probe(46);
    let delay45 = run_intr_2_oam_ok_probe(45);

    assert_eq!(
        (delay46.count, delay45.count),
        (0x01, 0x02),
        "delay46={delay46:?} delay45={delay45:?}"
    );
}

#[test]
#[ignore = "diag: first FE00 reads after intr_2_oam_ok_timing wake"]
fn mode2_to_oam_release_probe_logs_first_reads() {
    let delay46 = sample_intr_2_oam_ok_reads(46, 3);
    let delay45 = sample_intr_2_oam_ok_reads(45, 3);
    println!("delay46={delay46:?}");
    println!("delay45={delay45:?}");
}

#[test]
#[ignore = "diag: FE00 reads in the real mooneye intr_2_oam_ok_timing ROM"]
fn real_mooneye_oam_ok_logs_first_reads() {
    let reads = sample_real_mooneye_oam_ok_reads(4);
    println!("reads={reads:?}");
}

#[test]
#[ignore = "diag: real mooneye stat_lyc_onoff access trace"]
fn real_mooneye_stat_lyc_onoff_logs_first_accesses() {
    let accesses = sample_real_mooneye_stat_lyc_onoff_accesses(48);
    println!("accesses={accesses:?}");
}

#[test]
#[ignore = "diag: reduced caller matrix for ten-sprite staggered mooneye cases"]
fn intr_2_mode0_timing_sprites_ten_sprite_staggered_real_caller_matrix() {
    for (label, sprite_xs, delay_a, delay_b) in [
        (
            "x00_to_x48_68_67",
            [0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x40, 0x48],
            0x44_u8,
            0x43_u8,
        ),
        (
            "x01_to_x49_66_65",
            [0x01, 0x09, 0x11, 0x19, 0x21, 0x29, 0x31, 0x39, 0x41, 0x49],
            0x42_u8,
            0x41_u8,
        ),
        (
            "x02_to_x4A_63_62",
            [0x02, 0x0A, 0x12, 0x1A, 0x22, 0x2A, 0x32, 0x3A, 0x42, 0x4A],
            0x3F_u8,
            0x3E_u8,
        ),
        (
            "x03_to_x4B_61_60",
            [0x03, 0x0B, 0x13, 0x1B, 0x23, 0x2B, 0x33, 0x3B, 0x43, 0x4B],
            0x3D_u8,
            0x3C_u8,
        ),
        (
            "x04_to_x4C_58_57",
            [0x04, 0x0C, 0x14, 0x1C, 0x24, 0x2C, 0x34, 0x3C, 0x44, 0x4C],
            0x3A_u8,
            0x39_u8,
        ),
        (
            "x05_to_x4D_56_55",
            [0x05, 0x0D, 0x15, 0x1D, 0x25, 0x2D, 0x35, 0x3D, 0x45, 0x4D],
            0x38_u8,
            0x37_u8,
        ),
        (
            "x06_to_x4E_56_55",
            [0x06, 0x0E, 0x16, 0x1E, 0x26, 0x2E, 0x36, 0x3E, 0x46, 0x4E],
            0x38_u8,
            0x37_u8,
        ),
        (
            "x07_to_x4F_56_55",
            [0x07, 0x0F, 0x17, 0x1F, 0x27, 0x2F, 0x37, 0x3F, 0x47, 0x4F],
            0x38_u8,
            0x37_u8,
        ),
        (
            "x48_to_x00_68_67",
            [0x48, 0x40, 0x38, 0x30, 0x28, 0x20, 0x18, 0x10, 0x08, 0x00],
            0x44_u8,
            0x43_u8,
        ),
        (
            "x49_to_x01_66_65",
            [0x49, 0x41, 0x39, 0x31, 0x29, 0x21, 0x19, 0x11, 0x09, 0x01],
            0x42_u8,
            0x41_u8,
        ),
    ] {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(
                build_intr_2_mode0_timing_sprites_real_caller_probe_rom_with_specs(
                    &sprite_xs, delay_a, delay_b,
                ),
            )
            .expect("probe ROM should load");

        let mut outcome = None;
        for _ in 0..5_000_000 {
            machine.step_t_cycle();

            let cpu = machine.cpu().snapshot();
            if machine.read_bus(0xC20A) == 0x01 {
                let ppu = machine.ppu().snapshot();
                outcome = Some(format!(
                    "{label}: success b={:#04X} c={:#04X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                    cpu.registers.b,
                    cpu.registers.c,
                    ppu.ly,
                    ppu.line_dot,
                    ppu.mode,
                    ppu.mode0_start_dot,
                ));
                break;
            }

            if matches!(cpu.registers.pc, 0x486E | 0x486F | 0x4870 | 0x0C06) {
                let ppu = machine.ppu().snapshot();
                outcome = Some(format!(
                    "{label}: failure pc={:#06X} a={:#04X} b={:#04X} c={:#04X} d={:#04X} e={:#04X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                    cpu.registers.pc,
                    cpu.registers.a,
                    cpu.registers.b,
                    cpu.registers.c,
                    cpu.registers.d,
                    cpu.registers.e,
                    ppu.ly,
                    ppu.line_dot,
                    ppu.mode,
                    ppu.mode0_start_dot,
                ));
                break;
            }
        }

        println!("{}", outcome.unwrap_or_else(|| format!("{label}: timeout")));
    }
}

#[test]
#[ignore = "diag: copied ROM-path probe for case1 arm/read signature"]
fn mode2_to_mode0_sprites_case1_rom_path_probe_logs_arm_and_first_read() {
    let sample = sample_intr_2_mode0_sprites_case1_rom_path_probe_reads_after_arm(2);
    let Intr2Mode0SpritesCase1RomPathArmObservation {
        ly,
        line_dot,
        mode,
        pc,
    } = sample.arm;
    println!("case1_rom_path_arm ly={ly} line_dot={line_dot} mode={mode:?} pc={pc:#06X}");
    println!("case1_rom_path_reads={:?}", sample.reads);
    println!("case1_rom_path_terminal={:?}", sample.terminal);
}

#[test]
#[ignore = "diag: stale stat-mode probe that no longer matches the external mode0 oracle"]
fn mode2_to_mode0_stat_probe_matches_mooneye_counts() {
    let delay46 = run_intr_2_stat_mode_probe(46, 0x00);
    let delay45 = run_intr_2_stat_mode_probe(45, 0x00);

    assert_eq!(
        (delay46.count, delay45.count),
        (0x01, 0x02),
        "delay46={delay46:?} delay45={delay45:?}"
    );
}

#[test]
#[ignore = "diag: first-frame FF47 writes for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_first_frame_ff47_writes() {
    let rom_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/daid/ppu_scanline_bgp.gb");
    let rom = std::fs::read(&rom_path).expect("daid ppu_scanline_bgp ROM should be present");

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");

    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut write_count = 0usize;
    let mut visible_write_count = 0usize;
    let mut visible_frame0_line0 = Vec::new();
    let mut non_e4_visible_writes = Vec::new();

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let snapshot = machine.ppu().snapshot();
        if snapshot.ly != 0 || snapshot.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
            if wraps >= 2 {
                break;
            }
        }

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF47)
        {
            write_count += 1;
            let value = machine.read_bus(0xFF47);
            if snapshot.ly < 144 {
                visible_write_count += 1;
                if wraps == 1 && snapshot.ly == 0 {
                    visible_frame0_line0.push((
                        snapshot.line_dot,
                        snapshot.visible_pixels_output,
                        value,
                    ));
                }
                if value != 0xE4 {
                    non_e4_visible_writes.push((
                        wraps,
                        snapshot.ly,
                        snapshot.line_dot,
                        snapshot.visible_pixels_output,
                        value,
                    ));
                }
            }
        }
    }

    let mut grouped_non_e4 = std::collections::BTreeMap::new();
    for (frame, ly, line_dot, visible_pixels, value) in non_e4_visible_writes {
        grouped_non_e4
            .entry((frame, line_dot, visible_pixels, value))
            .or_insert_with(Vec::new)
            .push(ly);
    }

    let mut summarized_non_e4 = Vec::new();
    for ((frame, line_dot, visible_pixels, value), lys) in grouped_non_e4 {
        let mut ranges = Vec::new();
        let mut start = lys[0];
        let mut prev = lys[0];
        for ly in lys.into_iter().skip(1) {
            if ly == prev + 1 {
                prev = ly;
            } else {
                ranges.push((start, prev));
                start = ly;
                prev = ly;
            }
        }
        ranges.push((start, prev));
        summarized_non_e4.push((frame, line_dot, visible_pixels, value, ranges));
    }

    println!("ff47_write_total={write_count} visible_write_total={visible_write_count}");
    println!("frame1_line0_visible_writes={visible_frame0_line0:?}");
    println!("non_e4_visible_write_ranges={summarized_non_e4:?}");
    assert!(
        saw_progress,
        "diagnostic should advance past the initial dot"
    );
}

#[test]
#[ignore = "diag: first stable-frame FF47 write chronology for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_first_stable_frame_write_chronology() {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut writes = Vec::new();

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();

        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps != 1 {
            continue;
        }

        if machine.cpu().last_address_event().is_some_and(|event| {
            event.kind == CpuAddressEventKind::Write && event.access_address == Some(0xFF47)
        }) {
            writes.push((
                ppu.ly,
                ppu.line_dot,
                ppu.visible_pixels_output,
                cpu.registers.pc,
                u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l),
                machine.read_bus(0xFF47),
            ));
            if writes.len() >= 40 {
                break;
            }
        }
    }

    println!("first_stable_frame_ff47_writes={writes:#?}");
}

#[test]
#[ignore = "diag: wrap-boundary FF47 chronology for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_wrap_boundary_write_chronology() {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut writes = Vec::new();

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();

        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        let near_boundary = matches!(wraps, 0 | 1)
            && (ppu.ly >= 152 || ppu.ly <= 1)
            && machine.cpu().last_address_event().is_some_and(|event| {
                event.kind == CpuAddressEventKind::Write && event.access_address == Some(0xFF47)
            });
        if near_boundary {
            writes.push((
                wraps,
                ppu.ly,
                ppu.line_dot,
                ppu.visible_pixels_output,
                cpu.registers.pc,
                u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l),
                machine.read_bus(0xFF47),
            ));
        }

        if wraps == 1 && ppu.ly == 1 && ppu.line_dot >= 228 {
            break;
        }
    }

    println!("wrap_boundary_ff47_writes={writes:#?}");
}

#[test]
#[ignore = "diag: vblank handoff chronology for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_vblank_handoff() {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut events = Vec::new();

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();
        let interrupts = machine.interrupts().snapshot();
        let ff47_write = machine.cpu().last_address_event().is_some_and(|event| {
            event.kind == CpuAddressEventKind::Write && event.access_address == Some(0xFF47)
        });

        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps != 0 {
            continue;
        }

        let in_window = matches!(ppu.ly, 143..=153)
            && (ff47_write
                || ppu.line_dot == 0
                || ppu.line_dot == 84
                || ppu.line_dot == 228
                || matches!(
                    cpu.execution_state,
                    gb_core::CpuExecutionState::ServiceInterrupt { .. }
                        | gb_core::CpuExecutionState::Halted
                ));
        if in_window {
            events.push((
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                cpu.registers.pc,
                format!("{:?}", cpu.execution_state),
                cpu.ime,
                cpu.delayed_ime_enable,
                interrupts.interrupt_flags,
                interrupts.interrupt_enable,
                ff47_write.then(|| machine.read_bus(0xFF47)),
            ));
        }

        if ppu.ly == 153 && ppu.line_dot >= 448 {
            break;
        }
    }

    println!("vblank_handoff_events={events:#?}");
}

#[test]
#[ignore = "diag: local ly23 BGP phase oracle for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_ly23_phase_oracle() {
    let line15 = sample_daid_ppu_scanline_bgp_line(15);
    let line23 = sample_daid_ppu_scanline_bgp_line(23);

    assert_eq!(
        line15.raw_pixels_prefix, line23.raw_pixels_prefix,
        "ly15 raw={:?} ly23 raw={:?}",
        line15.raw_pixels_prefix, line23.raw_pixels_prefix
    );
    assert_eq!(
        line15.visible_bgp_writes, line23.visible_bgp_writes,
        "ly15 writes={:?} ly23 writes={:?}",
        line15.visible_bgp_writes, line23.visible_bgp_writes
    );
    assert_ne!(
        line15.panel_pixels_prefix, line23.panel_pixels_prefix,
        "ly15 panel={:?} ly23 panel={:?}",
        line15.panel_pixels_prefix, line23.panel_pixels_prefix
    );
    assert!(
        DAID_SCANLINE_BGP_ACCEPTED_LY23_PANEL_PREFIXES
            .iter()
            .any(|expected| expected == &line23.panel_pixels_prefix),
        "ly23 panel={:?} accepted={:?}",
        line23.panel_pixels_prefix,
        DAID_SCANLINE_BGP_ACCEPTED_LY23_PANEL_PREFIXES
    );
}

#[test]
#[ignore = "diag: ly22-24 BGP carry state around daid boundary lines"]
fn daid_ppu_scanline_bgp_logs_boundary_carry_state() {
    let targets = [
        (22, 228),
        (22, 252),
        (23, 0),
        (23, 1),
        (23, 84),
        (23, 85),
        (23, 100),
        (24, 0),
        (24, 84),
    ];
    let observations = sample_daid_ppu_scanline_bgp_dots(&targets);
    println!("boundary_carry_state={observations:#?}");
}

#[test]
#[ignore = "diag: block-end BGP carry state on daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_block_end_bgp_carry_state() {
    let targets = [
        (7, 0),
        (7, 1),
        (7, 2),
        (7, 3),
        (7, 84),
        (7, 85),
        (8, 0),
        (8, 1),
        (8, 2),
        (8, 3),
        (8, 84),
        (8, 85),
        (23, 0),
        (23, 1),
        (23, 2),
        (23, 3),
        (23, 84),
        (23, 85),
        (24, 0),
        (24, 1),
        (24, 2),
        (24, 3),
        (24, 84),
        (24, 85),
    ];
    let observations = sample_daid_ppu_scanline_bgp_dots(&targets);
    println!("block_end_bgp_carry_state={observations:#?}");
}

#[test]
#[ignore = "diag: ly22-24 CPU phase around daid BGP loop"]
fn daid_ppu_scanline_bgp_logs_boundary_cpu_phase() {
    let observations = sample_daid_ppu_scanline_bgp_cpu_phase_window();
    println!("boundary_cpu_phase={observations:#?}");
}

#[test]
#[ignore = "diag: block-end family summary for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_block_end_family() {
    let target_lys = [
        6_u8, 7, 8, 22, 23, 24, 38, 39, 40, 54, 55, 56, 78, 79, 80, 86, 87, 88, 94, 95, 96, 102,
        103, 104, 118, 119, 120, 134, 135, 136,
    ];
    let observations = sample_daid_ppu_scanline_bgp_lines(&target_lys);
    let mut summary = Vec::new();

    for &ly in &[7_u8, 23, 39, 55, 79, 87, 95, 103, 119, 135] {
        let previous = observations
            .get(&(ly - 1))
            .expect("previous line should be sampled");
        let current = observations
            .get(&ly)
            .expect("current line should be sampled");
        let next = observations
            .get(&(ly + 1))
            .expect("next line should be sampled");
        let panel_matches_previous = current.panel_pixels_prefix == previous.panel_pixels_prefix;
        let panel_matches_next = current.panel_pixels_prefix == next.panel_pixels_prefix;
        let raw_matches_previous = current.raw_pixels_prefix == previous.raw_pixels_prefix;
        let raw_matches_next = current.raw_pixels_prefix == next.raw_pixels_prefix;
        let writes_match_previous = current.visible_bgp_writes == previous.visible_bgp_writes;
        let writes_match_next = current.visible_bgp_writes == next.visible_bgp_writes;
        summary.push((
            ly,
            panel_matches_previous,
            panel_matches_next,
            raw_matches_previous,
            raw_matches_next,
            writes_match_previous,
            writes_match_next,
            current.panel_pixels_prefix,
        ));
    }

    println!("block_end_family={summary:#?}");
}

#[test]
#[ignore = "diag: line15-vs-line23 BGP register phase at visible writes"]
fn daid_ppu_scanline_bgp_logs_line15_vs_line23_write_phase() {
    let targets = [
        (15, 100),
        (15, 101),
        (15, 116),
        (15, 117),
        (15, 132),
        (15, 133),
        (23, 100),
        (23, 101),
        (23, 116),
        (23, 117),
        (23, 132),
        (23, 133),
    ];
    let observations = sample_daid_ppu_scanline_bgp_dots(&targets);
    println!("line15_vs_line23_write_phase={observations:#?}");
}

#[test]
#[ignore = "diag: line15-vs-line23 CPU HL progression at FF47 writes"]
fn daid_ppu_scanline_bgp_logs_line15_vs_line23_hl_progression() {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut observations = Vec::new();

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();

        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps != 1 {
            continue;
        }

        if machine.cpu().last_address_event().is_some_and(|event| {
            event.kind == CpuAddressEventKind::Write
                && event.access_address == Some(0xFF47)
                && matches!(ppu.ly, 15 | 23)
        }) {
            observations.push(DaidPpuScanlineBgpCpuPhaseObservation {
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                pc: cpu.registers.pc,
                hl: u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l),
                execution_state: format!("{:?}", cpu.execution_state),
                ff47: machine.read_bus(0xFF47),
                visible_pixels_output: ppu.visible_pixels_output,
            });
        }

        if observations.len() >= 20 {
            break;
        }
    }

    println!("line15_vs_line23_hl_progression={observations:#?}");
}

#[test]
#[ignore = "diag: boundary row-family transition for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_boundary_row_family_transition() {
    let observations =
        sample_daid_ppu_scanline_bgp_boundary_row_family_transition(&[22, 23, 24, 30, 31, 32]);
    println!("boundary_row_family_transition={observations:#?}");
}

#[test]
#[ignore = "diag: completed-frame boundary lines across daid frames"]
fn daid_ppu_scanline_bgp_logs_completed_frame_boundary_lines_across_frames() {
    let observations =
        sample_daid_ppu_scanline_bgp_completed_frame_boundary_lines(4, &[7, 23, 39, 55]);
    println!("completed_frame_boundary_lines={observations:#?}");
}

#[test]
#[ignore = "diag: first block boundary row-family transition for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_first_boundary_row_family_transition() {
    let observations =
        sample_daid_ppu_scanline_bgp_boundary_row_family_transition(&[6, 7, 8, 14, 15, 16]);
    println!("first_boundary_row_family_transition={observations:#?}");
}

#[test]
#[ignore = "diag: FF47 row values and HL progression for daid lines 15, 23, 31"]
fn daid_ppu_scanline_bgp_logs_line15_23_31_ff47_rows() {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut observations = Vec::new();

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();

        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps != 1 {
            continue;
        }

        if machine.cpu().last_address_event().is_some_and(|event| {
            event.kind == CpuAddressEventKind::Write
                && event.access_address == Some(0xFF47)
                && matches!(ppu.ly, 15 | 23 | 31)
        }) {
            observations.push(DaidPpuScanlineBgpCpuPhaseObservation {
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                pc: cpu.registers.pc,
                hl: u16::from(cpu.registers.h) << 8 | u16::from(cpu.registers.l),
                execution_state: format!("{:?}", cpu.execution_state),
                ff47: machine.read_bus(0xFF47),
                visible_pixels_output: ppu.visible_pixels_output,
            });
        }

        if observations.len() >= 30 {
            break;
        }
    }

    println!("line15_23_31_ff47_rows={observations:#?}");
}

#[test]
#[ignore = "diag: full raw/panel diffs for daid lines 15, 23, 24"]
fn daid_ppu_scanline_bgp_logs_line15_23_24_full_diffs() {
    let line15 = sample_daid_ppu_scanline_bgp_full_line(15);
    let line23 = sample_daid_ppu_scanline_bgp_full_line(23);
    let line24 = sample_daid_ppu_scanline_bgp_full_line(24);

    let mixed_15_23: Vec<_> = line15
        .mixed_colors
        .iter()
        .zip(&line23.mixed_colors)
        .enumerate()
        .filter_map(|(x, (&left, &right))| (left != right).then_some((x, left, right)))
        .collect();
    let mixed_23_24: Vec<_> = line23
        .mixed_colors
        .iter()
        .zip(&line24.mixed_colors)
        .enumerate()
        .filter_map(|(x, (&left, &right))| (left != right).then_some((x, left, right)))
        .collect();
    let raw_15_23: Vec<_> = line15
        .raw_pixels
        .iter()
        .zip(&line23.raw_pixels)
        .enumerate()
        .filter_map(|(x, (&left, &right))| (left != right).then_some((x, left, right)))
        .collect();
    let raw_23_24: Vec<_> = line23
        .raw_pixels
        .iter()
        .zip(&line24.raw_pixels)
        .enumerate()
        .filter_map(|(x, (&left, &right))| (left != right).then_some((x, left, right)))
        .collect();
    let panel_15_23: Vec<_> = line15
        .panel_pixels
        .iter()
        .zip(&line23.panel_pixels)
        .enumerate()
        .filter_map(|(x, (&left, &right))| (left != right).then_some((x, left, right)))
        .collect();
    let panel_23_24: Vec<_> = line23
        .panel_pixels
        .iter()
        .zip(&line24.panel_pixels)
        .enumerate()
        .filter_map(|(x, (&left, &right))| (left != right).then_some((x, left, right)))
        .collect();

    println!("mixed_15_23={mixed_15_23:?}");
    println!("mixed_23_24={mixed_23_24:?}");
    println!("raw_15_23={raw_15_23:?}");
    println!("raw_23_24={raw_23_24:?}");
    println!("panel_15_23={panel_15_23:?}");
    println!("panel_23_24={panel_23_24:?}");
}

#[test]
#[ignore = "diag: line0 wake and first FF47 row for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_line0_wake_and_first_loop_row() {
    let observations = sample_daid_ppu_scanline_bgp_line0_wake_and_first_loop_row();
    println!("line0_wake_and_first_loop_row={observations:#?}");
}

#[test]
#[ignore = "diag: lcd enable write chronology boundary snapshots"]
fn cpu_path_lcd_enable_write_probe_logs_boundary_snapshots() {
    for delay in [111_u16, 112, 131, 132, 225, 226, 245, 246] {
        let oam = run_lcd_enable_write_probe_observation(0xFE00, delay);
        let vram = run_lcd_enable_write_probe_observation(0x8000, delay);
        println!("delay={delay} oam={oam:?} vram={vram:?}");
    }
}
