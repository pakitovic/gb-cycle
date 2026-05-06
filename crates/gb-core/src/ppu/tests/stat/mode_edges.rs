use super::super::*;

#[test]
fn cpu_stat_read_switches_to_mode3_on_the_exact_mode2_end_dot() {
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
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x02
    );

    ppu.line_dot = MODE2_DOTS;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );

    ppu.line_dot = MODE2_DOTS + 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_switches_to_hblank_on_the_exact_mode0_start_dot() {
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
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );

    ppu.line_dot = MODE0_START_DOT;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );

    ppu.line_dot = MODE0_START_DOT + 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
#[ignore = "diagnostic direct-read experiment for offscreen-right mode0 publication"]
fn cpu_stat_read_switches_to_hblank_one_dot_before_mode0_start_for_offscreen_right_sprites() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    for oam_index in 0..10 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 168,
            tile_index: 0,
            attributes: 0,
        });
    }

    ppu.line_dot = MODE0_START_DOT - 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_keeps_lyc_coincidence_on_the_first_dot_of_a_new_line() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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

    ppu.ly = 1;
    ppu.lyc = 1;
    ppu.line_dot = 0;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x04
    );

    ppu.line_dot = 4;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x06
    );
}
