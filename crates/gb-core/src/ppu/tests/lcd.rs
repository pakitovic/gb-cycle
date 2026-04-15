use super::*;

fn lcd_startup_state(lcdc: u8, stat: u8, ly: u8, lyc: u8, bgp: u8) -> PpuStartupState {
    PpuStartupState {
        lcdc,
        stat,
        scy: 0x00,
        scx: 0x00,
        ly,
        lyc,
        bgp,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    }
}

fn dmg_lcd_rig(lcdc: u8, stat: u8, ly: u8, lyc: u8, bgp: u8) -> PpuTestRig {
    PpuTestRig::dmg().with_startup_state(lcd_startup_state(lcdc, stat, ly, lyc, bgp))
}

#[test]
fn startup_state_recreates_the_documented_post_boot_lcd_snapshot() {
    let ppu = dmg_lcd_rig(0x91, 0x08, 0x00, 0x00, 0xFC);

    assert_eq!(ppu.read_register(0xFF46), 0xFF);
    assert_eq!(ppu.read_register(0xFF40), 0x91);
    assert_eq!(ppu.read_register(0xFF41), 0x8C);
    assert_eq!(ppu.read_register(0xFF42), 0x00);
    assert_eq!(ppu.read_register(0xFF43), 0x00);
    assert_eq!(ppu.read_register(0xFF44), 0x00);
    assert_eq!(ppu.read_register(0xFF45), 0x00);
    assert_eq!(ppu.read_register(0xFF47), 0xFC);
    assert_eq!(ppu.read_register(0xFF4A), 0x00);
    assert_eq!(ppu.read_register(0xFF4B), 0x00);
    assert_eq!(ppu.snapshot().lcd_state, PpuLcdState::Enabled);
    assert_eq!(
        ppu.snapshot().visible_output,
        PpuVisibleOutputState::Driving
    );
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(
        ppu.bus_state(),
        PpuBusState::lcd_enabled(PpuAccessMode::HBlank)
    );
}

#[test]
fn skip_boot_mode_latch_preserves_the_published_stat_mode_until_the_first_dot() {
    let mut ppu = dmg_lcd_rig(0x91, 0x08, 0x00, 0x00, 0xFC);

    assert_eq!(ppu.snapshot().mode, PpuAccessMode::HBlank);
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(ppu.snapshot().mode_dot, 0);

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, 0);
    assert_eq!(ppu.snapshot().line_dot, 1);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(ppu.snapshot().mode_dot, 1);
}

#[test]
fn tick_advances_the_raster_through_the_baseline_visible_line_modes() {
    let mut ppu = dmg_lcd_rig(0x80, 0x82, 0x00, 0x00, 0x00);
    ppu.tick_n(79);

    assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(ppu.snapshot().line_dot, 79);
    assert_eq!(ppu.snapshot().mode_dot, 79);

    ppu.tick();
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
    assert_eq!(ppu.snapshot().line_dot, 80);
    assert_eq!(ppu.snapshot().mode_dot, 0);

    ppu.tick_n(171);

    assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
    assert_eq!(ppu.snapshot().line_dot, 251);
    assert_eq!(ppu.snapshot().mode_dot, 171);

    ppu.tick();
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::HBlank);
    assert_eq!(ppu.snapshot().line_dot, 252);
    assert_eq!(ppu.snapshot().mode_dot, 0);

    ppu.tick_n(204);

    assert_eq!(ppu.snapshot().ly, 1);
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(ppu.snapshot().mode_dot, 0);
}

#[test]
fn lcd_disabled_state_freezes_the_raster_and_forces_blank_output() {
    let mut ppu = dmg_lcd_rig(0x80, 0x82, 0x44, 0x12, 0xFC);
    ppu.tick_n(32);

    ppu.write_register(0xFF40, 0x00);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.lcd_state, PpuLcdState::Disabled);
    assert_eq!(snapshot.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!snapshot.blank_frame_active);
    assert_eq!(snapshot.ly, 0x00);
    assert_eq!(snapshot.line_dot, 0);
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert_eq!(ppu.bus_state(), PpuBusState::lcd_disabled());
}

#[test]
fn lcd_disable_resets_the_live_pipeline_and_reenable_starts_with_mode0_readback() {
    let mut ppu = dmg_lcd_rig(0x82, 0x82, 0x00, 0x00, 0x00);
    ppu.write_oam_entry(0, 16, 8, 0);
    ppu.write_bg_tile_row(0, 0, 0x00, 0xFF);
    ppu.tick_n(100);

    let drawing = ppu.snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert!(!drawing.bg_fifo_pixels.is_empty());

    ppu.write_register(0xFF40, 0x00);

    let disabled = ppu.snapshot();
    assert_eq!(disabled.lcd_state, PpuLcdState::Disabled);
    assert_eq!(disabled.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!disabled.blank_frame_active);
    assert_eq!(disabled.ly, 0);
    assert_eq!(disabled.line_dot, 0);
    assert!(disabled.bg_fifo_pixels.is_empty());
    assert!(disabled.obj_fifo_pixels.is_empty());
    assert!(disabled.selected_sprites.is_empty());
    assert_eq!(disabled.mode2_scanned_entries, 0);
    assert_eq!(disabled.window_line_counter, 0);

    ppu.write_register(0xFF40, 0x82);

    let reenabled = ppu.snapshot();
    assert_eq!(reenabled.lcd_state, PpuLcdState::Enabled);
    assert_eq!(reenabled.mode, PpuAccessMode::HBlank);
    assert_eq!(reenabled.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(reenabled.blank_frame_active);
    assert_eq!(reenabled.ly, 0);
    assert_eq!(reenabled.line_dot, LCD_REENABLE_INITIAL_LINE_DOT);
    assert_eq!(reenabled.mode_dot, 0);
    assert!(reenabled.bg_fifo_pixels.is_empty());
    assert!(reenabled.obj_fifo_pixels.is_empty());
    assert!(reenabled.selected_sprites.is_empty());
    assert_eq!(reenabled.mode2_scanned_entries, 0);
}

#[test]
fn lcd_reenable_first_line_skips_mode2_and_enters_mode3_late() {
    let mut ppu = dmg_lcd_rig(0x00, 0x80, 0x00, 0x00, 0x00);

    ppu.write_register(0xFF40, 0x82);

    let restart = ppu.snapshot();
    assert_eq!(restart.mode, PpuAccessMode::HBlank);
    assert_eq!(restart.line_dot, LCD_REENABLE_INITIAL_LINE_DOT);
    assert_eq!(restart.mode_dot, 0);
    assert_eq!(restart.mode2_scanned_entries, 0);

    ppu.tick_n(u64::from(LCD_REENABLE_LINE0_MODE3_START_DOT - 1));

    let line0_mode0_tail = ppu.snapshot();
    assert_eq!(
        line0_mode0_tail.line_dot,
        LCD_REENABLE_LINE0_MODE3_START_DOT - 1
    );
    assert_eq!(line0_mode0_tail.mode, PpuAccessMode::HBlank);
    assert_eq!(
        line0_mode0_tail.mode_dot,
        LCD_REENABLE_LINE0_MODE3_START_DOT - 1
    );
    assert_eq!(line0_mode0_tail.mode2_scanned_entries, 0);

    ppu.tick();

    let first_mode3_dot = ppu.snapshot();
    assert_eq!(first_mode3_dot.line_dot, LCD_REENABLE_LINE0_MODE3_START_DOT);
    assert_eq!(first_mode3_dot.mode, PpuAccessMode::Drawing);
    assert_eq!(first_mode3_dot.mode_dot, 0);
    assert_eq!(first_mode3_dot.mode2_scanned_entries, 0);

    while !(ppu.snapshot().ly == 1 && ppu.snapshot().line_dot == 2) {
        assert!(ppu.t_cycle < 2 * DOTS_PER_SCANLINE as u64);
        ppu.tick();
    }

    let first_normal_mode2_dot = ppu.snapshot();
    assert_eq!(first_normal_mode2_dot.mode, PpuAccessMode::OamScan);
    assert_eq!(first_normal_mode2_dot.mode_dot, 2);
    assert_eq!(first_normal_mode2_dot.mode2_scanned_entries, 1);
}

#[test]
fn lcd_off_retains_the_lyc_bit_and_ignores_lyc_writes_until_lcd_restarts() {
    let mut ppu = dmg_lcd_rig(0x80, 0x40, 0x90, 0x90, 0x00);

    ppu.write_register(0xFF40, 0x00);
    assert_eq!(ppu.read_register(0xFF41), 0xC4);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF45, 0x01);
    assert_eq!(ppu.read_register(0xFF41), 0xC4);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF40, 0x80);
    assert_eq!(ppu.read_register(0xFF41), 0xC0);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
}

#[test]
fn lcd_reenable_requests_lcd_stat_only_when_the_retained_lyc_result_rises() {
    let mut unchanged_true = dmg_lcd_rig(0x80, 0x40, 0x90, 0x90, 0x00);

    unchanged_true.write_register(0xFF40, 0x00);
    unchanged_true.write_register(0xFF45, 0x00);
    drain_ppu_interrupts(&mut unchanged_true);

    unchanged_true.write_register(0xFF40, 0x80);

    assert_eq!(unchanged_true.read_register(0xFF41), 0xC4);
    assert!(drain_ppu_interrupts(&mut unchanged_true).is_empty());

    let mut rising = dmg_lcd_rig(0x80, 0x40, 0x90, 0x00, 0x00);

    rising.write_register(0xFF40, 0x00);
    assert_eq!(rising.read_register(0xFF41), 0xC0);
    drain_ppu_interrupts(&mut rising);

    rising.write_register(0xFF40, 0x80);

    assert_eq!(rising.read_register(0xFF41), 0xC4);
    assert_eq!(
        drain_ppu_interrupts(&mut rising),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn lcd_disable_preserves_pending_vblank_from_the_same_t_cycle() {
    let mut ppu = dmg_lcd_rig(0x80, 0x80, 143, 0x00, 0x00);
    ppu.line_dot = DOTS_PER_SCANLINE - 1;

    ppu.tick();
    assert_eq!(ppu.snapshot().ly, 144);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);

    ppu.write_register(0xFF40, 0x00);

    assert_eq!(ppu.snapshot().lcd_state, PpuLcdState::Disabled);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::VBlank]
    );
}

#[test]
fn lcd_disable_preserves_pending_lcd_stat_requests() {
    let mut ppu = dmg_lcd_rig(0x80, 0x80, 0x00, 0x00, 0x00);

    ppu.queue_interrupt_request(InterruptSource::LcdStat);
    ppu.write_register(0xFF40, 0x00);

    assert_eq!(ppu.snapshot().lcd_state, PpuLcdState::Disabled);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn first_frame_after_lcd_reenable_stays_visibly_blank_while_the_raster_runs() {
    let mut ppu = dmg_lcd_rig(0x00, 0x80, 0x00, 0x00, 0x00);
    ppu.write_bg_tile_row(0, 0, 0x00, 0xFF);
    ppu.write_bg_tilemap_entry(0, 0, 0);
    ppu.write_register(0xFF40, 0x91);

    while ppu.snapshot().mode == PpuAccessMode::HBlank {
        assert!(ppu.t_cycle < DOTS_PER_SCANLINE as u64);
        ppu.tick();
    }

    ppu.advance_until_hblank();
    let first_blank_line = ppu.snapshot();
    assert_eq!(
        first_blank_line.visible_output,
        PpuVisibleOutputState::ForcedBlank
    );
    assert!(first_blank_line.blank_frame_active);
    assert_eq!(first_blank_line.visible_pixels_output, 153);
    assert_eq!(&first_blank_line.current_scanline_pixels[..8], &[0; 8]);

    ppu.advance_until_next_frame_start();
    let second_frame_start = ppu.snapshot();
    assert_eq!(second_frame_start.ly, 0);
    assert_eq!(second_frame_start.line_dot, 0);
    assert_eq!(
        second_frame_start.visible_output,
        PpuVisibleOutputState::Driving
    );
    assert!(!second_frame_start.blank_frame_active);

    ppu.advance_until_hblank();
    let visible_line = ppu.snapshot();
    assert_eq!(visible_line.visible_output, PpuVisibleOutputState::Driving);
    assert_eq!(&visible_line.current_scanline_pixels[..8], &[2; 8]);
}
