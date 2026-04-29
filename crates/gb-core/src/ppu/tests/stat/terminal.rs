use super::super::*;

#[test]
#[ignore = "diagnostic state for the sprite-extended post-visible publication seam without startup placeholders"]
fn cpu_stat_read_logs_sprite_extended_post_visible_tail_without_startup_placeholders() {
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
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 58;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });

    for line_dot in [
        ppu.bg_pipeline_state.mode0_start_dot + 2,
        ppu.bg_pipeline_state.mode0_start_dot + 3,
    ] {
        ppu.line_dot = line_dot;
        let stat = ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation);
        println!(
            "x8_tail line_dot={} stat_mode={} current_mode={:?} current_mode0_start_dot={} bg_base_mode0_start_dot={} current_transfer_x={} bg_lane={:?} bg_source_window={:?} bg_readiness={:?} startup_fifo_placeholders={} bg_fifo_len={} obj_stage={:?} obj_pending_hit_match_x={:?} obj_pending_hit_len={}",
            line_dot,
            stat & 0x03,
            ppu.current_access_mode(),
            ppu.current_mode0_start_dot(),
            ppu.bg_pipeline_state.mode0_start_dot,
            ppu.bg_pipeline_state.current_transfer_x,
            ppu.current_transfer().map(|transfer| transfer.context.lane),
            ppu.current_transfer()
                .map(|transfer| transfer.context.source_window),
            ppu.current_transfer().map(|transfer| transfer.readiness),
            ppu.bg_pipeline_state.startup_fifo_placeholders,
            ppu.bg_pipeline_state.fifo.len(),
            ppu.obj_pipeline_state.fetch.stage,
            ppu.obj_pipeline_state.pending_match_x,
            ppu.obj_pipeline_state.pending_sprite_slots.len(),
        );
    }
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x167_visible_tail_without_obj_work() {
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 15;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
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
        x: 0,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 17,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_terminal_x167_visible_tail_with_pending_same_x_work() {
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 15;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 16;
    ppu.obj_pipeline_state.pending_match_x = Some(167);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(8);

    for slot in 0..MAX_SELECTED_SPRITES_PER_LINE {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: slot as u8,
            y: 16,
            x: 167,
            tile_index: slot as u8,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 17,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x167_visible_tail_with_ready_push_and_pending_same_x_chain()
 {
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;
    ppu.obj_pipeline_state.pending_match_x = Some(167);

    for slot in 0..5 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: slot as u8,
            y: 16,
            x: 71,
            tile_index: slot as u8,
            attributes: 0,
        });
    }
    for slot in 5..MAX_SELECTED_SPRITES_PER_LINE {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: slot as u8,
            y: 16,
            x: 167,
            tile_index: slot as u8,
            attributes: 0,
        });
    }
    for sprite_slot in 5..9 {
        ppu.obj_pipeline_state.mark_fetched(sprite_slot as u8);
    }
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(9);

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot before the CPU-visible read"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x167_visible_tail_while_blank_frame_is_active() {
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
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 15;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
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
        x: 0,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 17,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x165_visible_tail_while_blank_frame_is_active() {
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
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 54;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 165;
    ppu.bg_pipeline_state.visible_pixels_output = 157;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.line_dot = MODE0_START_DOT + 56;

    for oam_index in 0..9 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 0,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 57,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x166_visible_tail_without_blank_frame() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
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
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 59;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 166;
    ppu.bg_pipeline_state.visible_pixels_output = 158;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 2));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.line_dot = MODE0_START_DOT + 59;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 160,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 60,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_placeholder_only_visible_tail() {
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
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 24;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 164;
    ppu.bg_pipeline_state.visible_pixels_output = 156;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 4));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 24;

    for oam_index in 0..4 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 0,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 25,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x163_visible_tail_even_with_one_real_fifo_pixel() {
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
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 28;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 163;
    ppu.bg_pipeline_state.visible_pixels_output = 155;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 5));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 28;

    for oam_index in 0..5 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 0,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 29,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}
