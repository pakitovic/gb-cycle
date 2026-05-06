use super::*;

const LCD_ENABLE_READ_PROBE_M_CYCLES: [u16; 24] = [
    0, 17, 60, 110, 130, 174, 224, 244, 1, 18, 61, 111, 131, 175, 225, 245, 2, 19, 62, 112, 132,
    176, 226, 246,
];
const EXPECTED_LCD_ENABLE_READ_LY: [u8; 24] = [
    0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02, 0x02,
    0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02, 0x02,
];
const EXPECTED_LCD_ENABLE_READ_STAT_LYC0: [u8; 24] = [
    0x84, 0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82, 0x84, 0x87, 0x84, 0x80, 0x82, 0x80, 0x80, 0x82,
    0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82, 0x83,
];
const EXPECTED_LCD_ENABLE_READ_STAT_LYC1: [u8; 24] = [
    0x80, 0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82, 0x80, 0x83, 0x80, 0x84, 0x86, 0x84, 0x80, 0x82,
    0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82, 0x83,
];
const EXPECTED_LCD_ENABLE_READ_OAM: [u8; 24] = [
    0x00, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0xFF,
    0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0xFF,
];
const EXPECTED_LCD_ENABLE_READ_VRAM: [u8; 24] = [
    0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF,
    0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF,
];

fn run_lcd_enable_read_probe(address: u16, delay_nops: u16, lyc: Option<u8>) -> u8 {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_lcd_enable_read_probe_rom(
            address,
            delay_nops as usize,
        ))
        .expect("probe ROM should load");
    machine.write_bus(0xFF40, 0x00);
    if let Some(lyc) = lyc {
        machine.write_bus(0xFF45, lyc);
    }
    run_until_halted(&mut machine, 1_000_000)
}

fn build_lcd_reenable_mode2_if_probe_rom(delay_nops: usize) -> Vec<u8> {
    let mut program = Vec::new();
    program.push(0xF3); // di
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a ; LCDC = off
    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a ; STAT Mode 2 source
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a ; IF = 0
    program.extend_from_slice(&[0x3E, 0x91]); // ld a,$91
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a ; LCDC = on
    program.extend(std::iter::repeat_n(0x00, delay_nops)); // nops
    program.extend_from_slice(&[0xF0, 0x0F]); // ldh a,($0F)
    program.push(0x47); // ld b,a
    program.push(0x76); // halt
    build_nom_bc_test_rom_with_program_entry(&program, 0x00, 0x0150, &[])
}

fn run_lcd_reenable_mode2_if_probe(delay_nops: usize) -> u8 {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_lcd_reenable_mode2_if_probe_rom(delay_nops))
        .expect("probe ROM should load");
    run_until_halted(&mut machine, 1_000_000)
}

fn build_lcd_reenable_mode2_irq_counter_rom(arm_after_lcd_on_nops: Option<usize>) -> Vec<u8> {
    let mut program = Vec::new();
    program.push(0xF3); // di
    program.push(0xAF); // xor a

    if let Some(arm_after_lcd_on_nops) = arm_after_lcd_on_nops {
        program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a ; IF = 0
        program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a ; LCDC = off
        program.extend_from_slice(&[0x3E, 0x91]); // ld a,$91
        program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a ; LCDC = on
        program.extend(std::iter::repeat_n(0x00, arm_after_lcd_on_nops));
        program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20
        program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a ; STAT Mode 2 source
        program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
        program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT
        program.push(0xFB); // ei
    } else {
        program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a ; LCDC = off
        program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20
        program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a ; STAT Mode 2 source
        program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
        program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT
        program.push(0xAF); // xor a
        program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a ; IF = 0
        program.push(0xFB); // ei
        program.extend_from_slice(&[0x3E, 0x91]); // ld a,$91
        program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a ; LCDC = on
    }

    program.push(0xAF); // xor a
    program.extend(std::iter::repeat_n(0x3C, 200)); // inc a
    program.push(0x47); // ld b,a
    program.push(0x76); // halt

    build_nom_bc_test_rom_with_program_entry(&program, 0x00, 0x0150, &[(0x0048, &[0x47, 0x76])])
}

fn run_lcd_reenable_mode2_irq_counter(arm_after_lcd_on_nops: Option<usize>) -> u8 {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_lcd_reenable_mode2_irq_counter_rom(
            arm_after_lcd_on_nops,
        ))
        .expect("probe ROM should load");
    run_until_halted(&mut machine, 1_000_000)
}

fn assert_lcd_enable_read_probe_points(label: &str, actual: [u8; 24], expected: [u8; 24]) {
    assert_eq!(actual, expected, "{label}={actual:?}");
}

#[test]
fn lcd_reenable_lyc_rise_services_lcd_stat_before_the_second_di_in_the_mooneye_round4_sequence() {
    assert!(observe_lcd_reenable_lyc_irq_service_window(0x00, None));
}

#[test]
fn lcd_reenable_retained_true_does_not_service_lcd_stat_in_the_same_sequence() {
    assert!(!observe_lcd_reenable_lyc_irq_service_window(
        0x90,
        Some(0x00)
    ));
}

#[test]
fn lcd_disabled_machine_state_keeps_the_ppu_raster_frozen() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0x00);

    for _ in 0..8 {
        machine.step_t_cycle();
    }

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.lcd_state, PpuLcdState::Disabled);
    assert_eq!(snapshot.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!snapshot.blank_frame_active);
    assert_eq!(snapshot.ly, 0);
    assert_eq!(snapshot.line_dot, 0);
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
}

#[test]
fn mid_scanline_lcdc7_disable_resets_the_raster_and_releases_ppu_bus_blocking() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0x8000, 0x12);
    step_until_line_dot(&mut machine, 100);

    assert_eq!(machine.ppu().snapshot().mode, PpuAccessMode::Drawing);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);

    machine.write_bus(0xFF40, 0x00);

    let disabled = machine.ppu().snapshot();
    assert_eq!(disabled.lcd_state, PpuLcdState::Disabled);
    assert_eq!(disabled.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!disabled.blank_frame_active);
    assert_eq!(disabled.ly, 0);
    assert_eq!(disabled.line_dot, 0);
    assert_eq!(disabled.mode, PpuAccessMode::HBlank);
    assert_eq!(machine.read_bus(0x8000), 0x12);
    assert_eq!(machine.read_bus(0xFE00), 0x00);
}

#[test]
fn lcd_reenable_restarts_immediately_but_keeps_the_first_frame_visibly_blank() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x18, 0xFE], 0x00))
        .expect("NoMBC idle ROM should load");

    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);

    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0xFF40, 0x91);

    while machine.ppu().snapshot().lcd_state != PpuLcdState::Enabled {
        machine.step_t_cycle();
    }

    let restart = machine.ppu().snapshot();
    assert_eq!(restart.lcd_state, PpuLcdState::Enabled);
    assert_eq!(restart.mode, PpuAccessMode::HBlank);
    assert_eq!(restart.ly, 0);
    assert_eq!(restart.line_dot, 1);
    assert_eq!(restart.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(restart.blank_frame_active);

    step_until_line_dot(&mut machine, 252);

    let blank_line = machine.ppu().snapshot();
    assert_eq!(blank_line.mode, PpuAccessMode::HBlank);
    assert_eq!(
        blank_line.visible_output,
        PpuVisibleOutputState::ForcedBlank
    );
    assert_eq!(blank_line.visible_pixels_output, 160);
    assert_eq!(&blank_line.current_scanline_pixels[..8], &[0; 8]);

    step_until_next_frame_start(&mut machine);

    let second_frame_start = machine.ppu().snapshot();
    assert_eq!(
        second_frame_start.visible_output,
        PpuVisibleOutputState::Driving
    );
    assert!(!second_frame_start.blank_frame_active);
    assert_eq!(second_frame_start.mode, PpuAccessMode::OamScan);

    step_until_line_dot(&mut machine, 252);

    let visible_line = machine.ppu().snapshot();
    assert_eq!(visible_line.mode, PpuAccessMode::HBlank);
    assert_eq!(visible_line.visible_output, PpuVisibleOutputState::Driving);
    assert_eq!(&visible_line.current_scanline_pixels[..8], &[2; 8]);
}

#[test]
fn lcd_reenable_mode2_pretrigger_is_not_visible_to_same_cycle_if_reads() {
    assert_eq!(run_lcd_reenable_mode2_if_probe(109), 0xE0);
    assert_eq!(run_lcd_reenable_mode2_if_probe(110), 0xE2);
}

#[test]
fn lcd_reenable_prearmed_mode2_stat_services_on_the_first_oam_pretrigger() {
    assert_eq!(run_lcd_reenable_mode2_irq_counter(None), 111);
}

#[test]
fn lcd_reenable_arming_mode2_stat_during_oam_waits_for_the_next_oam_edge() {
    assert_eq!(run_lcd_reenable_mode2_irq_counter(Some(114)), 100);
}

#[test]
fn cpu_path_lcd_enable_read_ly_probe_matches_the_mooneye_probe_points() {
    let actual =
        LCD_ENABLE_READ_PROBE_M_CYCLES.map(|delay| run_lcd_enable_read_probe(0xFF44, delay, None));
    assert_lcd_enable_read_probe_points("actual_ly", actual, EXPECTED_LCD_ENABLE_READ_LY);
}

#[test]
fn cpu_path_lcd_enable_read_stat_lyc0_probe_matches_the_mooneye_probe_points() {
    let actual = LCD_ENABLE_READ_PROBE_M_CYCLES
        .map(|delay| run_lcd_enable_read_probe(0xFF41, delay, Some(0x00)));
    assert_lcd_enable_read_probe_points(
        "actual_stat_lyc0",
        actual,
        EXPECTED_LCD_ENABLE_READ_STAT_LYC0,
    );
}

#[test]
fn cpu_path_lcd_enable_read_stat_lyc1_probe_matches_the_mooneye_probe_points() {
    let actual = LCD_ENABLE_READ_PROBE_M_CYCLES
        .map(|delay| run_lcd_enable_read_probe(0xFF41, delay, Some(0x01)));
    assert_lcd_enable_read_probe_points(
        "actual_stat_lyc1",
        actual,
        EXPECTED_LCD_ENABLE_READ_STAT_LYC1,
    );
}

#[test]
fn cpu_path_lcd_enable_read_oam_probe_matches_the_mooneye_probe_points() {
    let actual =
        LCD_ENABLE_READ_PROBE_M_CYCLES.map(|delay| run_lcd_enable_read_probe(0xFE00, delay, None));
    assert_lcd_enable_read_probe_points("actual_oam", actual, EXPECTED_LCD_ENABLE_READ_OAM);
}

#[test]
fn cpu_path_lcd_enable_read_vram_probe_matches_the_mooneye_probe_points() {
    let actual =
        LCD_ENABLE_READ_PROBE_M_CYCLES.map(|delay| run_lcd_enable_read_probe(0x8000, delay, None));
    assert_lcd_enable_read_probe_points("actual_vram", actual, EXPECTED_LCD_ENABLE_READ_VRAM);
}

#[test]
fn lcd_off_releases_ppu_mode_restrictions_without_overriding_dma_hram_only_blocking() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let initial_hram = machine.read_bus(0xFF80);

    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0x8000, 0x12);
    machine.write_bus(0xFE00, 0x34);
    machine.write_bus(0xFF46, 0x80);
    for _ in 0..5 {
        machine.step_t_cycle();
    }

    let disabled = machine.ppu().snapshot();
    assert_eq!(disabled.lcd_state, PpuLcdState::Disabled);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);
    assert_eq!(machine.read_bus(0xFF80), initial_hram);
}
