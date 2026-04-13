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
#[ignore = "diag: lcd enable write chronology boundary snapshots"]
fn cpu_path_lcd_enable_write_probe_logs_boundary_snapshots() {
    for delay in [111_u16, 112, 131, 132, 225, 226, 245, 246] {
        let oam = run_lcd_enable_write_probe_observation(0xFE00, delay);
        let vram = run_lcd_enable_write_probe_observation(0x8000, delay);
        println!("delay={delay} oam={oam:?} vram={vram:?}");
    }
}
