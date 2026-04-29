use super::*;

#[test]
fn hblank_ly_scx_probe_matches_mooneye_thresholds() {
    let mut failures = Vec::new();

    for scx in 0x00..=0x08 {
        let (delay_a, delay_b) = match scx & 0x07 {
            0 => (2, 3),
            1..=4 => (1, 2),
            _ => (0, 1),
        };

        for scanline in [0x42, 0x43] {
            let before = run_hblank_ly_scx_probe(scx, scanline, delay_a);
            let after = run_hblank_ly_scx_probe(scx, scanline, delay_b);
            if before.completion_marker != 1 {
                failures.push(format!(
                    "probe halted early for scx={scx:#04X} scanline={scanline:#04X}; before={before:?}"
                ));
            }
            if after.completion_marker != 1 {
                failures.push(format!(
                    "probe halted early for scx={scx:#04X} scanline={scanline:#04X}; after={after:?}"
                ));
            }
            if before.observed_ly != scanline.wrapping_sub(1) {
                failures.push(format!(
                    "delay_a expected previous LY for scx={scx:#04X} scanline={scanline:#04X}; before={before:?} after={after:?}"
                ));
            }
            if after.observed_ly != scanline {
                failures.push(format!(
                    "delay_b expected current LY for scx={scx:#04X} scanline={scanline:#04X}; before={before:?} after={after:?}"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "hblank_ly_scx mismatches:\n{}",
        failures.join("\n")
    );
}

#[test]
fn mode2_to_mode0_stat_interrupt_probe_matches_mooneye_counts() {
    let delay4 = run_intr_2_0_probe(4);
    let delay3 = run_intr_2_0_probe(3);

    assert_eq!(
        (delay4.count, delay3.count),
        (0x07, 0x08),
        "delay4={delay4:?} delay3={delay3:?}"
    );
}

#[test]
fn mode2_to_mode3_stat_probe_matches_mooneye_counts() {
    let delay3 = run_intr_2_stat_mode_probe(3, 0x03);
    let delay2 = run_intr_2_stat_mode_probe(2, 0x03);

    assert_eq!(
        (delay3.count, delay2.count),
        (0x01, 0x02),
        "delay3={delay3:?} delay2={delay2:?}"
    );
}

#[test]
fn mode0_stat_request_can_precede_visible_hblank_while_vram_stays_blocked() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0x8000, 0x12);
    machine.write_bus(0xFF45, 0x01);
    step_until_line_dot(&mut machine, 80);
    machine.write_bus(0xFF41, 0x08);
    machine.write_bus(0xFF0F, 0x00);

    step_until_line_dot(&mut machine, 247);

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x03);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    machine.step_t_cycle();

    let stat_pretrigger = machine.ppu().snapshot();
    assert_eq!(stat_pretrigger.mode, PpuAccessMode::Drawing);
    assert_eq!(stat_pretrigger.line_dot, 248);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x03);
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    step_until_line_dot(&mut machine, 251);

    let late_drawing = machine.ppu().snapshot();
    assert_eq!(late_drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x03);
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    machine.step_t_cycle();

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.line_dot, 252);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x00);
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);
    assert_eq!(machine.read_bus(0x8000), 0x12);
}

#[test]
fn entering_vblank_can_raise_vblank_and_mode1_stat_together() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x18, 0xFE], 0x00))
        .expect("NoMBC idle ROM should load");

    machine.write_bus(0xFF45, 0xFF);
    step_until_line_dot(&mut machine, 80);
    machine.write_bus(0xFF41, 0x10);
    machine.write_bus(0xFF0F, 0x00);

    step_until_position(&mut machine, 143, 455);

    let before_vblank = machine.ppu().snapshot();
    assert_eq!(before_vblank.mode, PpuAccessMode::HBlank);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    machine.step_t_cycle();

    let vblank = machine.ppu().snapshot();
    assert_eq!(vblank.ly, 144);
    assert_eq!(vblank.line_dot, 0);
    assert_eq!(vblank.mode, PpuAccessMode::VBlank);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x01);
    assert_eq!(machine.read_bus(0xFF0F), 0xE3);
}
