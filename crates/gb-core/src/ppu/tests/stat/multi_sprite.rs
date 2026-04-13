use super::super::*;

#[test]
fn cpu_stat_read_publishes_hblank_for_two_sprite_staggered_x2_x0a_fifo_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 18;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 164;
    ppu.bg_pipeline_state.visible_pixels_output = 156;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 4));
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 2,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 0x0A,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 18,
        "staggered two-sprite tail still stretches three live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_two_sprite_staggered_x4_x0c_fifo_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 19;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 159;
    ppu.bg_pipeline_state.visible_pixels_output = 151;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.line_dot = MODE0_START_DOT + 12;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 4,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 0x0C,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 19,
        "staggered two-sprite FIFO tail still stretches eight live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_two_sprite_staggered_x8_x10_preterminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_two_sprite_staggered_x0_x08_terminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 13;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 8,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 13,
        "internal HBlank already started for the staggered x=0/8 pair"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_two_sprite_staggered_x1_x09_terminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 12;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 1,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 9,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 12,
        "internal HBlank already started for the staggered x=1/9 pair"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_two_sprite_staggered_x9_x11_terminal_boundary() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 15;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 9,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 17,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 15,
        "internal HBlank already starts one dot before the published boundary for x=9/17"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_ten_sprite_step8_terminal_tails() {
    for (min_x, placeholders, push_pending, terminal_offset) in
        [(0, 2, true, 4), (1, 1, false, 4), (2, 4, true, 4)]
    {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 20;
        ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
        ppu.bg_pipeline_state.current_transfer_x = 168;
        ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.startup_fifo_placeholders = placeholders;
        ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
        ppu.bg_pipeline_state.push.pending = push_pending;
        ppu.line_dot = ppu.bg_pipeline_state.mode0_start_dot + terminal_offset;

        for sprite_slot in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
            ppu.mode2_scan_state.push(PpuSelectedSprite {
                oam_index: sprite_slot,
                y: 16,
                x: min_x + sprite_slot * 8,
                tile_index: sprite_slot,
                attributes: 0,
            });
        }

        assert_eq!(
            ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
            0x03,
            "step-8 terminal tail with min_x={min_x} should keep published drawing"
        );
    }
}

#[test]
fn cpu_stat_read_publishes_hblank_for_ten_sprite_step8_preterminal_tails() {
    for (min_x, current_transfer_x, fifo_len) in [
        (4, 160, 8_usize),
        (5, 152, 8_usize),
        (6, 152, 8_usize),
        (7, 152, 8_usize),
    ] {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 32;
        ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
        ppu.bg_pipeline_state.current_transfer_x = current_transfer_x;
        ppu.bg_pipeline_state.visible_pixels_output = current_transfer_x - 8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.startup_fifo_placeholders = 8 - min_x;
        ppu.bg_pipeline_state
            .fifo
            .extend(std::iter::repeat_n(0, fifo_len));
        ppu.line_dot = MODE0_START_DOT + 24;

        for sprite_slot in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
            ppu.mode2_scan_state.push(PpuSelectedSprite {
                oam_index: sprite_slot,
                y: 16,
                x: min_x + sprite_slot * 8,
                tile_index: sprite_slot,
                attributes: 0,
            });
        }

        assert!(matches!(
            ppu.current_transfer().map(|transfer| transfer.readiness),
            Some(Mode3TransferReadiness::Ready(_))
        ));
        assert_eq!(
            ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
            0x00,
            "step-8 preterminal tail with min_x={min_x} should publish HBlank early"
        );
    }
}
