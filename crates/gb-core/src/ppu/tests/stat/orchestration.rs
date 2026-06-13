use super::super::*;

fn dmg_stat_ppu(stat: u8) -> Ppu {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu
}

#[test]
fn published_stat_line_start_falls_back_to_visible_line_hblank() {
    let mut ppu = dmg_stat_ppu(0x08);
    ppu.ly = 1;
    ppu.line_dot = 0;

    assert_eq!(ppu.access_mode_for_line_dot(0), PpuAccessMode::OamScan);
    assert_eq!(
        ppu.current_published_stat_access_mode(),
        PpuAccessMode::HBlank
    );
}

#[test]
fn published_stat_frame_start_keeps_the_line_start_hblank_fallback() {
    let mut ppu = dmg_stat_ppu(0x08);
    ppu.ly = 0;
    ppu.line_dot = 0;

    assert_eq!(ppu.access_mode_for_line_dot(0), PpuAccessMode::OamScan);
    assert_eq!(
        ppu.current_published_stat_access_mode(),
        PpuAccessMode::HBlank
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation),
        0x8C
    );
}

#[test]
fn published_stat_line0_after_vblank_wrap_lags_mode_edges_by_four_dots() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_LYC_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 153,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.advance_until_line_start(0);

    ppu.line_dot = MODE2_DOTS;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x06
    );

    ppu.line_dot = MODE2_DOTS + LINE0_VBLANK_WRAP_STAT_READBACK_DELAY_DOTS;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x07
    );

    ppu.line_dot = MODE0_START_DOT;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x07
    );

    ppu.line_dot = MODE0_START_DOT + LINE0_VBLANK_WRAP_STAT_READBACK_DELAY_DOTS;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x04
    );
}

#[test]
fn published_stat_mode2_to_mode3_orchestrator_promotes_mode3_on_exact_boundary() {
    let mut ppu = dmg_stat_ppu(0x08);
    ppu.ly = 1;
    ppu.line_dot = MODE2_DOTS;

    assert_eq!(
        ppu.access_mode_for_line_dot(MODE2_DOTS - 1),
        PpuAccessMode::OamScan
    );
    assert_eq!(
        ppu.access_mode_for_line_dot(MODE2_DOTS),
        PpuAccessMode::Drawing
    );
    assert_eq!(
        ppu.current_published_stat_access_mode(),
        PpuAccessMode::Drawing
    );
}

#[test]
fn published_stat_terminal_boundary_orchestrator_switches_to_hblank_on_non_extended_mode0_start() {
    let mut ppu = dmg_stat_ppu(STAT_MODE0_INTERRUPT_ENABLE_BIT);
    ppu.ly = 1;
    ppu.line_dot = MODE0_START_DOT;

    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT);
    assert_eq!(
        ppu.access_mode_for_line_dot(ppu.line_dot - 1),
        PpuAccessMode::Drawing
    );
    assert_eq!(
        ppu.access_mode_for_line_dot(ppu.line_dot),
        PpuAccessMode::HBlank
    );
    assert_eq!(
        ppu.current_published_stat_access_mode(),
        PpuAccessMode::HBlank
    );
}

#[test]
fn nonzero_scx_mode2_only_terminal_boundary_keeps_published_drawing() {
    let mut ppu = dmg_stat_ppu(STAT_MODE2_INTERRUPT_ENABLE_BIT);
    ppu.scx = 4;
    ppu.ly = 1;
    ppu.line_dot = MODE0_START_DOT;

    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT);
    assert_eq!(
        ppu.access_mode_for_line_dot(ppu.line_dot - 1),
        PpuAccessMode::Drawing
    );
    assert_eq!(
        ppu.access_mode_for_line_dot(ppu.line_dot),
        PpuAccessMode::HBlank
    );
    assert_eq!(
        ppu.current_published_stat_access_mode(),
        PpuAccessMode::Drawing
    );
}

#[test]
fn published_stat_terminal_boundary_orchestrator_publishes_hblank_at_sprite_extended_mode0_start() {
    let mut ppu = dmg_stat_ppu(0x08);
    ppu.ly = 68;
    ppu.startup_mode_latch = None;
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
    ppu.line_dot = ppu.current_mode0_start_dot();

    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT + 17);
    assert_eq!(
        ppu.access_mode_for_line_dot(ppu.line_dot - 1),
        PpuAccessMode::Drawing
    );
    assert_eq!(ppu.current_access_mode(), PpuAccessMode::HBlank);
    assert_eq!(
        ppu.current_published_stat_access_mode(),
        PpuAccessMode::HBlank
    );

    ppu.blank_frame_active = true;
    assert_eq!(
        ppu.current_published_stat_access_mode(),
        PpuAccessMode::Drawing
    );

    ppu.line_dot += 1;
    assert_eq!(
        ppu.current_published_stat_access_mode(),
        PpuAccessMode::HBlank
    );
}
