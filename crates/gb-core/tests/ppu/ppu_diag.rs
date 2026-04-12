use super::*;

#[test]
#[ignore = "diagnostic probe for mooneye intr_2_oam_ok_timing seam"]
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
#[ignore = "diagnostic probe for first FE00 reads after intr_2_oam_ok_timing wake"]
fn mode2_to_oam_release_probe_logs_first_reads() {
    let delay46 = sample_intr_2_oam_ok_reads(46, 3);
    let delay45 = sample_intr_2_oam_ok_reads(45, 3);
    println!("delay46={delay46:?}");
    println!("delay45={delay45:?}");
}

#[test]
#[ignore = "diagnostic probe for FE00 reads in the real mooneye intr_2_oam_ok_timing ROM"]
fn real_mooneye_oam_ok_logs_first_reads() {
    let reads = sample_real_mooneye_oam_ok_reads(4);
    println!("reads={reads:?}");
}

#[test]
#[ignore = "diagnostic probe for the real mooneye stat_lyc_onoff ROM"]
fn real_mooneye_stat_lyc_onoff_logs_first_accesses() {
    let accesses = sample_real_mooneye_stat_lyc_onoff_accesses(48);
    println!("accesses={accesses:?}");
}

#[test]
#[ignore = "diagnostic reduced caller matrix for the ten-sprite staggered mooneye cases"]
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
#[ignore = "diagnostic copied ROM-path probe for case1 arm/read signature"]
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
#[ignore = "diagnostic stat-mode probe no longer matches the current external mode0 oracle"]
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
#[ignore = "diagnostic first-frame FF47 writes for daid ppu_scanline_bgp"]
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
