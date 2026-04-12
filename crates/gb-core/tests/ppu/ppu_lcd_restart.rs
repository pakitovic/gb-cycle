use super::*;

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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
fn cpu_path_lcd_enable_read_probe_matches_the_mooneye_probe_points() {
    const PROBE_M_CYCLES: [u16; 24] = [
        0, 17, 60, 110, 130, 174, 224, 244, 1, 18, 61, 111, 131, 175, 225, 245, 2, 19, 62, 112,
        132, 176, 226, 246,
    ];
    const EXPECTED_LY: [u8; 24] = [
        0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02,
        0x02, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02, 0x02,
    ];
    const EXPECTED_STAT_LYC0: [u8; 24] = [
        0x84, 0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82, 0x84, 0x87, 0x84, 0x80, 0x82, 0x80, 0x80,
        0x82, 0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82, 0x83,
    ];
    const EXPECTED_STAT_LYC1: [u8; 24] = [
        0x80, 0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82, 0x80, 0x83, 0x80, 0x80, 0x86, 0x84, 0x80,
        0x82, 0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82, 0x83,
    ];
    const EXPECTED_OAM: [u8; 24] = [
        0x00, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF,
        0xFF, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0xFF,
    ];
    const EXPECTED_VRAM: [u8; 24] = [
        0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00,
        0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF,
    ];

    let run_probe = |address: u16, delay_nops: u16| -> u8 {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_lcd_enable_read_probe_rom(
                address,
                delay_nops as usize,
            ))
            .expect("probe ROM should load");
        machine.write_bus(0xFF40, 0x00);
        run_until_halted(&mut machine, 1_000_000)
    };

    let actual_ly = PROBE_M_CYCLES.map(|delay| run_probe(0xFF44, delay));
    let actual_stat_lyc0 = PROBE_M_CYCLES.map(|delay| {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_lcd_enable_read_probe_rom(0xFF41, delay as usize))
            .expect("probe ROM should load");
        machine.write_bus(0xFF40, 0x00);
        machine.write_bus(0xFF45, 0x00);
        run_until_halted(&mut machine, 1_000_000)
    });
    let actual_stat_lyc1 = PROBE_M_CYCLES.map(|delay| {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_lcd_enable_read_probe_rom(0xFF41, delay as usize))
            .expect("probe ROM should load");
        machine.write_bus(0xFF40, 0x00);
        machine.write_bus(0xFF45, 0x01);
        run_until_halted(&mut machine, 1_000_000)
    });
    let actual_oam = PROBE_M_CYCLES.map(|delay| run_probe(0xFE00, delay));
    let actual_vram = PROBE_M_CYCLES.map(|delay| run_probe(0x8000, delay));

    if actual_ly != EXPECTED_LY
        || actual_stat_lyc0 != EXPECTED_STAT_LYC0
        || actual_stat_lyc1 != EXPECTED_STAT_LYC1
        || actual_oam != EXPECTED_OAM
        || actual_vram != EXPECTED_VRAM
    {
        panic!(
            "actual_ly={actual_ly:?}\nactual_stat_lyc0={actual_stat_lyc0:?}\nactual_stat_lyc1={actual_stat_lyc1:?}\nactual_oam={actual_oam:?}\nactual_vram={actual_vram:?}"
        );
    }
}

#[test]
#[ignore = "diag: CPU-path LCDC.7 write chronology probe is retained only for re-entry work"]
fn cpu_path_lcd_enable_write_probe_matches_the_mooneye_probe_points() {
    const NOP_COUNTS: [u16; 19] = [
        0, 17, 18, 60, 61, 110, 111, 112, 130, 131, 132, 174, 175, 224, 225, 226, 244, 245, 246,
    ];
    const EXPECTED_OAM: [u8; 19] = [
        0x81, 0x81, 0x00, 0x00, 0x81, 0x81, 0x81, 0x00, 0x00, 0x81, 0x00, 0x00, 0x81, 0x81, 0x81,
        0x00, 0x00, 0x81, 0x00,
    ];
    const EXPECTED_VRAM: [u8; 19] = [
        0x81, 0x81, 0x00, 0x00, 0x81, 0x81, 0x81, 0x81, 0x81, 0x81, 0x00, 0x00, 0x81, 0x81, 0x81,
        0x81, 0x81, 0x81, 0x00,
    ];

    let actual_oam = NOP_COUNTS
        .map(|delay| run_lcd_enable_write_probe_observation(0xFE00, delay).observed_value);
    let actual_vram = NOP_COUNTS
        .map(|delay| run_lcd_enable_write_probe_observation(0x8000, delay).observed_value);

    if actual_oam != EXPECTED_OAM || actual_vram != EXPECTED_VRAM {
        panic!("actual_oam={actual_oam:?}\nactual_vram={actual_vram:?}");
    }
}

#[test]
fn lcd_off_releases_ppu_mode_restrictions_without_overriding_dma_hram_only_blocking() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
