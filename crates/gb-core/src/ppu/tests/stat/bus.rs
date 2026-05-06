use super::super::*;

fn dmg_skip_boot_power_on_bus_rig() -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.apply_dmg_skip_boot_stat_irq_startup_phase();
    ppu
}

fn set_dmg_skip_boot_power_on_delay(ppu: &mut PpuTestRig, delay_mcycles: u16) {
    let absolute_dot = 36 + u32::from(delay_mcycles) * 4;
    ppu.ly = (absolute_dot / u32::from(DOTS_PER_SCANLINE)) as u8;
    ppu.line_dot = (absolute_dot % u32::from(DOTS_PER_SCANLINE)) as u16;
}

fn dmg_real_boot_handoff_power_on_bus_rig() -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 153,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 392;
    ppu.apply_dmg_real_boot_handoff_stat_irq_phase();
    ppu
}

fn set_dmg_real_boot_handoff_power_on_delay(ppu: &mut PpuTestRig, delay_mcycles: u16) {
    let frame_dots = u32::from(DOTS_PER_SCANLINE) * u32::from(TOTAL_SCANLINES);
    let base_dot = (153 * u32::from(DOTS_PER_SCANLINE) + 428) % frame_dots;
    let absolute_dot = (base_dot + u32::from(delay_mcycles) * 4) % frame_dots;
    ppu.ly = (absolute_dot / u32::from(DOTS_PER_SCANLINE)) as u8;
    ppu.line_dot = (absolute_dot % u32::from(DOTS_PER_SCANLINE)) as u16;
}

#[test]
fn cpu_oam_write_bus_state_only_opens_the_restart_probe_window_at_line_start_and_mode2_end() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
fn dmg_skip_boot_power_on_cpu_bus_access_uses_the_boot_facing_oam_and_vram_windows() {
    let mut ppu = dmg_skip_boot_power_on_bus_rig();

    for (delay_mcycles, oam_blocked, vram_blocked) in [
        (0, false, false),
        (5, false, false),
        (6, true, false),
        (25, true, false),
        (26, true, true),
        (69, true, true),
        (70, false, false),
        (119, false, false),
        (120, true, false),
        (139, true, false),
        (140, true, true),
        (183, true, true),
        (184, false, false),
        (233, false, false),
        (234, true, false),
        (235, true, false),
    ] {
        set_dmg_skip_boot_power_on_delay(&mut ppu, delay_mcycles);

        let oam_mode = ppu.cpu_oam_read_bus_state().mode();
        let vram_mode = ppu.cpu_bus_state().mode();
        assert_eq!(
            matches!(oam_mode, PpuAccessMode::OamScan | PpuAccessMode::Drawing),
            oam_blocked,
            "OAM delay {delay_mcycles}"
        );
        assert_eq!(
            vram_mode == PpuAccessMode::Drawing,
            vram_blocked,
            "VRAM delay {delay_mcycles}"
        );
    }
}

#[test]
fn dmg_real_boot_handoff_power_on_cpu_bus_access_uses_the_wrapped_boot_facing_windows() {
    let mut ppu = dmg_real_boot_handoff_power_on_bus_rig();

    for (delay_mcycles, oam_blocked, vram_blocked) in [
        (0, false, false),
        (6, true, false),
        (26, true, true),
        (70, false, false),
        (120, true, false),
        (140, true, true),
        (184, false, false),
        (234, true, false),
        (235, true, false),
    ] {
        set_dmg_real_boot_handoff_power_on_delay(&mut ppu, delay_mcycles);

        let oam_mode = ppu.cpu_oam_read_bus_state().mode();
        let vram_mode = ppu.cpu_bus_state().mode();
        assert_eq!(
            matches!(oam_mode, PpuAccessMode::OamScan | PpuAccessMode::Drawing),
            oam_blocked,
            "OAM delay {delay_mcycles}"
        );
        assert_eq!(
            vram_mode == PpuAccessMode::Drawing,
            vram_blocked,
            "VRAM delay {delay_mcycles}"
        );
    }
}

#[test]
fn cpu_oam_read_bus_state_only_opens_the_mode2_end_probe_window() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
fn sprite_extended_mode0_start_opens_cpu_oam_read_before_published_stat_catches_up() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.startup_mode_latch = None;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 17;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(ppu.baseline_mode0_start_dot(), MODE0_START_DOT);
    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT + 17);

    ppu.line_dot = ppu.current_mode0_start_dot() - 1;
    assert_eq!(ppu.owner_bus_state().mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.cpu_bus_state().mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::Drawing);
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        PpuAccessMode::Drawing.stat_bits()
    );

    ppu.line_dot = ppu.current_mode0_start_dot();
    assert_eq!(ppu.owner_bus_state().mode(), PpuAccessMode::HBlank);
    assert_eq!(ppu.cpu_bus_state().mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::HBlank);
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        PpuAccessMode::Drawing.stat_bits()
    );
}

#[test]
fn bus_state_snapshot_matches_the_individual_bus_state_helpers() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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

#[test]
fn owns_mmio_register_matches_the_ppu_register_window() {
    for address in 0xFF00..=0xFF7F {
        let expected = matches!(
            address,
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B | 0xFF68..=0xFF6C
        );
        assert_eq!(Ppu::owns_mmio_register(address), expected, "{address:#06X}");
    }
}
