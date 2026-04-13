use super::super::*;

fn dmg_terminal_render_rig(scx: u8) -> PpuTestRig {
    PpuTestRig::dmg().with_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    })
}

fn saturated_terminal_tail_rig(line_dot: u16) -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = line_dot;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 303;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = 160;
    ppu.bg_pipeline_state
        .saw_right_edge_visible_same_x_cluster_this_line = true;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));

    for sprite_slot in 0..10_u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: if sprite_slot < 5 { 0 } else { 160 },
            tile_index: sprite_slot,
            attributes: 0,
        });
    }

    ppu
}

fn non_holding_terminal_tail_rig() -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = 315;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 303;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = 160;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));

    for sprite_slot in 0..10_u8 {
        let sprite = PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: 7,
            tile_index: sprite_slot,
            attributes: 0,
        };
        ppu.mode2_scan_state.push(sprite);
        ppu.obj_pipeline_state.mark_fetched(sprite_slot);
    }

    ppu
}

#[test]
fn mode3_scx_discard_shifts_visible_pixels_and_delays_hblank_entry() {
    let mut ppu = dmg_terminal_render_rig(0x03);
    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.write_bg_tile_row(1, 0, 0xAA, 0xCC);
    ppu.write_bg_tilemap_entry(0, 0, 0);
    ppu.write_bg_tilemap_entry(1, 0, 1);
    ppu.tick_n(252);

    let extended_drawing = ppu.snapshot();
    assert_eq!(extended_drawing.line_dot, 252);
    assert_eq!(extended_drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(extended_drawing.mode0_start_dot, 255);

    ppu.tick_n(3);

    let hblank = ppu.snapshot();
    assert_eq!(hblank.line_dot, 255);
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode_dot, 0);
    assert_eq!(hblank.visible_pixels_output, 160);
    assert_eq!(
        &hblank.current_scanline_pixels[..8],
        &[3, 0, 1, 2, 3, 3, 2, 1]
    );
}

#[test]
#[ignore = "diagnostic case1 terminal x167 no-obj seam from intr_2_mode0_timing_sprites"]
fn terminal_visible_bg_transfer_without_obj_work_does_not_extend_mode3_past_x167() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = MODE0_START_DOT + 16;
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

    let transfer = ppu
        .current_transfer()
        .expect("terminal visible x167 should still expose the live transfer context");
    assert_eq!(transfer.context.lane, Mode3TransferLane::Visible);
    assert_eq!(
        transfer.context.source_window,
        Mode3TransferSourceWindow::FifoBacked
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, None);
    assert!(ppu.obj_pipeline_state.pending_sprite_slots.is_empty());
    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT + 16);
}

#[test]
fn saturated_placeholder_backed_terminal_bg_tail_stays_in_mode3_during_tile_data_high() {
    let mut ppu = saturated_terminal_tail_rig(313);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, None);
    assert!(ppu.obj_pipeline_state.pending_sprite_slots.is_empty());
    assert_eq!(ppu.current_mode0_start_dot(), 314);
}

#[test]
fn saturated_placeholder_backed_terminal_bg_tail_stays_in_mode3_during_push() {
    let mut ppu = saturated_terminal_tail_rig(315);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;

    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, None);
    assert!(ppu.obj_pipeline_state.pending_sprite_slots.is_empty());
    assert_eq!(ppu.current_mode0_start_dot(), 316);
}

#[test]
fn saturated_placeholder_backed_terminal_bg_tail_holds_one_extra_dot_after_push_entry_delay() {
    let mut ppu = saturated_terminal_tail_rig(316);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state
        .push
        .terminal_placeholder_tail_extra_hold_remaining = 1;

    assert_eq!(ppu.current_mode0_start_dot(), 317);
    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::WaitingForEmptyFifo);
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .terminal_placeholder_tail_extra_hold_remaining,
        0
    );
}

#[test]
fn saturated_placeholder_backed_terminal_bg_tail_does_not_hold_without_right_edge_x160_cluster() {
    let mut ppu = non_holding_terminal_tail_rig();
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;

    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::EntryDelay);
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .terminal_placeholder_tail_extra_hold_remaining,
        0
    );
}
