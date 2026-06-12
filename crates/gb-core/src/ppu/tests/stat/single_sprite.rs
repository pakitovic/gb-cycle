use super::super::*;

const SINGLE_SPRITE_PENALTY_CASES: [(u8, u16); 13] = [
    (0, 11),
    (8, 11),
    (9, 10),
    (10, 9),
    (11, 8),
    (12, 7),
    (13, 6),
    (14, 6),
    (15, 6),
    (0xA0, 11),
    (0xA7, 6),
    (0xA8, 0),
    (0xFF, 0),
];

fn single_sprite_line_rig(sprite_x: u8) -> PpuTestRig {
    let mut rig = PpuTestRig::dmg().with_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT | 0x02,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    rig.write_oam_entry(0, 17, sprite_x, 0);
    rig.advance_until_line_start(1);
    rig
}

#[test]
fn cpu_stat_read_publishes_hblank_at_the_single_sprite_penalty_mode0_boundary() {
    for (sprite_x, penalty) in SINGLE_SPRITE_PENALTY_CASES {
        let mut rig = single_sprite_line_rig(sprite_x);
        rig.advance_until_hblank();

        assert_eq!(
            rig.current_mode0_start_dot(),
            MODE0_START_DOT + penalty,
            "sprite x={sprite_x} pays the phase-aligned mode3 penalty"
        );
        assert_eq!(
            rig.line_dot,
            MODE0_START_DOT + penalty,
            "sprite x={sprite_x} reaches hblank exactly at the mode0 boundary"
        );
        assert_eq!(
            rig.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
            0x00,
            "sprite x={sprite_x} boundary stat read publishes hblank"
        );
    }
}

#[test]
fn cpu_stat_read_publishes_hblank_on_the_single_xa2_mode0_boundary() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
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
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 6;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.line_dot = MODE0_START_DOT + 6;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0xA2,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 6,
        "single offscreen-right x=0xA2 case reaches the mode0 boundary directly"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}
