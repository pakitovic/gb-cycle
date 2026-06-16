use super::super::*;

fn dmg_blank_frame_rig_with_line0_sprites(sprite_xs: &[u8]) -> PpuTestRig {
    let mut rig = PpuTestRig::dmg().with_startup_state(PpuStartupState {
        lcdc: 0x13,
        stat: 0x00,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    for (sprite_slot, sprite_x) in sprite_xs.iter().enumerate() {
        rig.write_oam_entry(sprite_slot as u8, 16, *sprite_x, sprite_slot as u8);
    }
    rig.write_register(0xFF40, 0x93);
    rig
}

#[test]
fn ten_step8_sprites_starting_at_x8_pay_eleven_dots_each_on_a_blank_frame_line() {
    let sprite_xs: Vec<u8> = (0..MAX_SELECTED_SPRITES_PER_LINE as u8)
        .map(|sprite_slot| 8 + sprite_slot * 8)
        .collect();
    let mut rig = dmg_blank_frame_rig_with_line0_sprites(&sprite_xs);

    rig.advance_until_line_start(3);
    assert!(rig.blank_frame_active);
    rig.advance_until_hblank();
    let mode0_start_dot = rig.current_mode0_start_dot();
    assert_eq!(mode0_start_dot, MODE0_START_DOT + 110);

    rig.advance_until_line_start(4);
    for _ in 0..DOTS_PER_SCANLINE {
        if rig.snapshot().line_dot == mode0_start_dot {
            break;
        }
        rig.tick();
    }
    assert_eq!(rig.snapshot().line_dot, mode0_start_dot);
    assert_eq!(
        rig.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03,
        "blank-frame STAT read at the internal mode0 boundary still sees drawing"
    );
    rig.tick();
    assert_eq!(
        rig.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00,
        "blank-frame STAT read one dot past the internal mode0 boundary sees hblank"
    );
}

#[test]
fn two_same_x_sprites_pay_one_alignment_stall_then_a_bare_fetch_on_a_blank_frame_line() {
    let mut rig = dmg_blank_frame_rig_with_line0_sprites(&[8, 8]);

    rig.advance_until_line_start(3);
    assert!(rig.blank_frame_active);
    rig.advance_until_hblank();
    assert_eq!(rig.current_mode0_start_dot(), MODE0_START_DOT + 17);
}

#[test]
fn cpu_stat_read_reads_mode0_one_dot_early_for_two_sprite_staggered_x8_x10_preterminal_tail() {
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 17;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 0x10,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 17,
        "staggered x=8/16 pair still has one live drawing dot before internal HBlank"
    );
    // CPU-first reorder: the Drawing→HBlank boundary is observed one dot early, so the
    // same-cycle CPU STAT read at mode0_start_dot-1 already yields mode0 (HBlank).
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

fn cgb_ten_sprite_step8_fifo_tail_without_pending_push(operating_mode: OperatingMode) -> Ppu {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(operating_mode);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 99;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 127;
    ppu.bg_pipeline_state.visible_pixels_output = 119;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.line_dot = MODE0_START_DOT + 68;

    for sprite_slot in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: 4 + sprite_slot * 8,
            tile_index: sprite_slot,
            attributes: 0,
        });
    }

    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::Ready(_))
    ));
    assert!(!ppu.bg_pipeline_state.push.pending);
    ppu
}

#[test]
fn cgb_native_stat_keeps_drawing_for_step8_fifo_tail_without_pending_push() {
    let ppu = cgb_ten_sprite_step8_fifo_tail_without_pending_push(OperatingMode::Cgb);

    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03,
        "native CGB STAT should not reuse the compatibility-mode step-8 FIFO tail publication seam"
    );
}
