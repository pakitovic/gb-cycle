use super::super::*;

#[test]
fn cpu_oam_write_bus_state_only_opens_the_restart_probe_window_at_line_start_and_mode2_end() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.line_dot = 0;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    assert_eq!(ppu.cpu_oam_write_bus_state().mode(), PpuAccessMode::HBlank);

    ppu.line_dot = 4;
    assert_eq!(ppu.cpu_oam_write_bus_state().mode(), PpuAccessMode::OamScan);

    ppu.line_dot = MODE2_DOTS;
    assert_eq!(ppu.cpu_write_bus_state().mode(), PpuAccessMode::OamScan);
    assert_eq!(ppu.cpu_oam_write_bus_state().mode(), PpuAccessMode::HBlank);

    ppu.line_dot = MODE2_DOTS + 4;
    assert_eq!(ppu.cpu_oam_write_bus_state().mode(), PpuAccessMode::Drawing);
}

#[test]
fn cpu_oam_read_bus_state_only_opens_the_mode2_end_probe_window() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    ppu.line_dot = MODE2_DOTS - 1;
    assert_eq!(ppu.cpu_bus_state().mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::Drawing);

    ppu.line_dot = MODE2_DOTS;
    assert_eq!(ppu.cpu_bus_state().mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::HBlank);

    ppu.line_dot = MODE2_DOTS + 1;
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::Drawing);
}

#[test]
fn cpu_oam_read_bus_state_switches_to_hblank_on_the_exact_mode0_start_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    ppu.line_dot = MODE0_START_DOT - 1;
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::Drawing);

    ppu.line_dot = MODE0_START_DOT;
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::HBlank);

    ppu.line_dot = MODE0_START_DOT + 1;
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::HBlank);
}

#[test]
fn bus_state_snapshot_matches_the_individual_bus_state_helpers() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for &line_dot in &[
        0,
        4,
        MODE2_DOTS - 1,
        MODE2_DOTS,
        MODE0_START_DOT,
        MODE0_START_DOT + 1,
    ] {
        ppu.ly = 1;
        ppu.line_dot = line_dot;
        ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        ppu.blank_frame_active = false;

        let snapshot = ppu.bus_state_snapshot();
        assert_eq!(snapshot.owner, ppu.owner_bus_state());
        assert_eq!(snapshot.cpu_read, ppu.cpu_bus_state());
        assert_eq!(snapshot.cpu_write, ppu.cpu_write_bus_state());
    }

    ppu.write_register(0xFF40, 0x00);
    let disabled_snapshot = ppu.bus_state_snapshot();
    assert_eq!(disabled_snapshot.owner, ppu.owner_bus_state());
    assert_eq!(disabled_snapshot.cpu_read, ppu.cpu_bus_state());
    assert_eq!(disabled_snapshot.cpu_write, ppu.cpu_write_bus_state());
}
