use super::super::*;

#[test]
fn visible_mode3_registers_lag_enabled_writes_until_the_next_t_cycle() {
    let mut ppu = PpuTestRig::dmg();

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.tick_n(80);

    let before = ppu.snapshot();
    assert_eq!(before.mode, PpuAccessMode::Drawing);
    assert_eq!(before.visible_lcdc, 0x80);
    assert_eq!(before.visible_scy, 0x00);
    assert_eq!(before.visible_scx, 0x00);
    assert_eq!(before.visible_bgp, 0xFC);
    assert_eq!(before.visible_wy, 0x00);
    assert_eq!(before.visible_wx, 0x00);

    ppu.write_register(0xFF40, 0x91);
    ppu.write_register(0xFF42, 0x12);
    ppu.write_register(0xFF43, 0x34);
    ppu.write_register(0xFF47, 0x1B);
    ppu.write_register(0xFF4A, 0x56);
    ppu.write_register(0xFF4B, 0x78);

    let pending = ppu.snapshot();
    assert_eq!(pending.lcdc, 0x91);
    assert_eq!(pending.scy, 0x12);
    assert_eq!(pending.scx, 0x34);
    assert_eq!(pending.bgp, 0x1B);
    assert_eq!(pending.wy, 0x56);
    assert_eq!(pending.wx, 0x78);
    assert_eq!(pending.visible_lcdc, 0x80);
    assert_eq!(pending.visible_scy, 0x00);
    assert_eq!(pending.visible_scx, 0x00);
    assert_eq!(pending.visible_bgp, 0xFC);
    assert_eq!(pending.visible_wy, 0x00);
    assert_eq!(pending.visible_wx, 0x00);

    ppu.tick();

    let after = ppu.snapshot();
    assert_eq!(after.visible_lcdc, 0x91);
    assert_eq!(after.visible_scy, 0x12);
    assert_eq!(after.visible_scx, 0x34);
    assert_eq!(after.visible_bgp, 0x1B);
    assert_eq!(after.visible_wy, 0x56);
    assert_eq!(after.visible_wx, 0x78);
}

#[test]
fn mode3_startup_keeps_dummy_occupancy_out_of_the_fifo_until_alignment_push() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.write_bg_tilemap_entry(0, 0, 0);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.tick_n(80);

    let drawing_start = ppu.snapshot();
    assert_eq!(drawing_start.mode, PpuAccessMode::Drawing);
    assert_eq!(drawing_start.line_dot, 80);
    assert_eq!(drawing_start.mode_dot, 0);
    assert_eq!(drawing_start.mode0_start_dot, 252);
    assert_eq!(drawing_start.bg_fetcher_stage, PpuBgFetcherStage::TileIndex);
    assert_eq!(drawing_start.bg_fetcher_stage_dot, 1);
    assert!(drawing_start.bg_fifo_pixels.is_empty());
    assert_eq!(drawing_start.visible_pixels_output, 0);

    ppu.tick_n(7);

    let after_first_push = ppu.snapshot();
    assert_eq!(after_first_push.line_dot, 87);
    assert_eq!(
        after_first_push.bg_fetcher_stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(after_first_push.bg_fetcher_stage_dot, 1);
    assert_eq!(
        after_first_push.bg_fifo_pixels,
        vec![0, 0, 0, 0, 0, 1, 2, 3, 0, 1, 2, 3]
    );
    assert!(!after_first_push.bg_push_pending);
    assert!(!after_first_push.bg_fill_pending);
    assert_eq!(after_first_push.visible_pixels_output, 0);

    while ppu.snapshot().visible_pixels_output != 1 {
        assert!(ppu.t_cycle < 110);
        ppu.tick();
    }

    let first_visible = ppu.snapshot();
    assert_eq!(first_visible.visible_pixels_output, 1);
    assert!(first_visible.line_dot >= 92);
    assert_eq!(first_visible.current_scanline_pixels[0], 0);
}

#[test]
fn mode3_startup_fetches_the_first_three_visible_background_tiles_in_order() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tile_row(0, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(1, 0, 0xFF, 0x00);
    ppu.write_bg_tile_row(2, 0, 0x00, 0xFF);
    ppu.write_bg_tilemap_entry(0, 0, 0);
    ppu.write_bg_tilemap_entry(1, 0, 1);
    ppu.write_bg_tilemap_entry(2, 0, 2);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    while ppu.snapshot().visible_pixels_output != 24 {
        assert!(ppu.t_cycle < 140);
        ppu.tick();
    }

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.visible_pixels_output, 24);
    assert_eq!(snapshot.current_scanline_pixels[..8], [0; 8]);
    assert_eq!(snapshot.current_scanline_pixels[8..16], [1; 8]);
    assert_eq!(snapshot.current_scanline_pixels[16..24], [2; 8]);
}

#[test]
fn sprite_coupled_line10_tile_sel_replay_matches_trace_signature() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_oam_entry(0, 26, 1, 0);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x00,
        stat: 0xA4,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF40, 0x83);

    ppu.advance_until_tile_sel_replay_position(10, 85);

    let startup = ppu.snapshot();
    assert_eq!(startup.visible_lcdc, 0x83);
    assert_eq!(startup.pipeline_lcdc, 0x83);
    assert_eq!(startup.visible_pixels_output, 0);
    assert_eq!(startup.bg_current_transfer_x, 1);
    assert!(startup.bg_fill_pending);
    assert_eq!(startup.bg_fill_startup_dummy_pixels, 7);
    assert_eq!(startup.bg_startup_fifo_placeholders, 7);
    assert_eq!(startup.selected_sprites.len(), 1);
    assert_eq!(
        startup.bg_startup_fetch_seam,
        PpuBgStartupFetchSeamSnapshot::PostAlignment {
            first_real_push_skips_entry_delay: true,
            next_startup_continuation_slice: PpuBgStartupContinuationSliceSnapshot::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        }
    );
}

#[test]
fn sprite_coupled_line10_startup_tail_renders_correctly_once_panel_blank_is_lifted() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_oam_entry(0, 26, 1, 0);
    for row in 0..BG_TILE_WIDTH {
        ppu.write_bg_tile_row(0, row, 0x00, 0x00);
        let signed_tile_row = 0x1000 + row as usize * TILE_ROW_BYTES as usize;
        ppu.vram_bytes[signed_tile_row] = 0xFF;
        ppu.vram_bytes[signed_tile_row + 1] = 0xFF;
    }

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x00,
        stat: 0xA4,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF40, 0x83);

    ppu.advance_until_tile_sel_replay_position(10, 85);
    ppu.blank_frame_active = false;
    ppu.refresh_visible_output();
    assert_eq!(ppu.visible_output, PpuVisibleOutputState::Driving);
    ppu.advance_until_tile_sel_replay_position(10, 99);
    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0].expect(
        "the first visible startup-tail pixel should already be materialized before line_dot 100",
    );
    assert_eq!(
        front_cached.cached.origin,
        BgCachedSliceOrigin::StartupAlignmentFill
    );
    assert_eq!(ppu.bg_pipeline_state.fifo[0], 3);
    assert!(!front_cached.cached.needs_live_tilemap_refetch);
    assert!(!front_cached.cached.needs_live_tile_data_refetch);
    assert!(!front_cached.cached.needs_live_tile_data_current_row_refetch);
    assert!(!front_cached.cached.needs_live_tile_data_unsigned_reuse);
    assert_eq!(
        front_cached.cached.tile_low, 0xFF,
        "tile_high={:#04X} tile_data_address={:#06X}",
        front_cached.cached.tile_high, front_cached.cached.tile_data_address,
    );
    assert_eq!(front_cached.cached.tile_high, 0xFF);
    while !(ppu.snapshot().ly == 10 && ppu.snapshot().visible_pixels_output == 1) {
        apply_tile_sel_line_write_replay(&mut ppu);
        assert!(ppu.t_cycle < 11000);
        ppu.tick();
    }

    let first_visible = ppu.snapshot();
    assert_eq!(
        first_visible.current_scanline_pixels[0],
        3,
        "line_dot={} visible_lcdc={:#04X} pipeline_lcdc={:#04X} visible_output={:?} current_transfer_x={}",
        first_visible.line_dot,
        first_visible.visible_lcdc,
        first_visible.pipeline_lcdc,
        first_visible.visible_output,
        first_visible.bg_current_transfer_x,
    );
}

#[test]
fn startup_post_alignment_seam_labels_only_the_second_and_third_visible_tiles() {
    let mut pipeline = BgPipelineState::default();

    pipeline.begin_post_alignment_followup();
    assert_eq!(
        pipeline
            .peek_startup_background_fetch_origin()
            .startup_continuation_slice(),
        BgStartupContinuationSlice::VisibleTile2
    );
    pipeline.advance_startup_background_fetch_tile();

    assert_eq!(
        pipeline
            .peek_startup_background_fetch_origin()
            .startup_continuation_slice(),
        BgStartupContinuationSlice::VisibleTile3
    );
    assert!(pipeline.take_startup_first_real_push_skip_entry_delay());
    pipeline.advance_startup_background_fetch_tile();

    assert_eq!(
        pipeline.peek_startup_background_fetch_origin(),
        BgCachedSliceOrigin::Ordinary
    );
    assert_eq!(
        pipeline.startup_fetch_seam,
        BgStartupFetchSeamState::Inactive
    );
}

#[test]
fn startup_post_alignment_seam_skips_the_first_real_push_entry_delay_once() {
    let mut pipeline = BgPipelineState::default();

    pipeline.begin_post_alignment_followup();

    assert_eq!(
        pipeline.startup_fetch_seam,
        BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: true,
            next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        }
    );
    assert!(pipeline.take_startup_first_real_push_skip_entry_delay());
    assert_eq!(
        pipeline.startup_fetch_seam,
        BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: false,
            next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        }
    );
    assert!(!pipeline.take_startup_first_real_push_skip_entry_delay());
}

#[test]
fn first_real_background_push_after_startup_alignment_skips_entry_delay() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; TEST_VRAM_BYTES]);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.bg_pipeline_state.begin_post_alignment_followup();
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x9801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x8010;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x55;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x33;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.fetch_x,
        BG_TILE_WIDTH as u16 * 2
    );
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert_eq!(
        ppu.bg_pipeline_state.startup_fetch_seam,
        BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: false,
            next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
            startup_continuation_visible_tiles_remaining: 1,
            delayed_background_tileindex_read_tiles_remaining: 0,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 0,
        }
    );
}

#[test]
fn startup_dummy_fifo_pixels_do_not_block_the_first_real_bg_fill() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend([0, 0, 0, 0]);

    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFill
    );
}

#[test]
fn mode3_started_uses_explicit_startup_entry_delay_before_transfer_service() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + 1;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = MODE3_ABSTRACT_SOURCE_WINDOW_DOTS;
    ppu.bg_pipeline_state.startup_source_state =
        Mode3StartupSourceState::EntryDelay { remaining: 2 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 5;

    let first = ppu.advance_mode3_output_phase();
    assert_eq!(first.kind, Mode3TransferDotKind::NotServed);
    assert_eq!(
        ppu.bg_pipeline_state.startup_source_state,
        Mode3StartupSourceState::EntryDelay { remaining: 1 }
    );
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);

    let second = ppu.advance_mode3_output_phase();
    assert_eq!(second.kind, Mode3TransferDotKind::NotServed);
    assert_eq!(
        ppu.bg_pipeline_state.startup_source_state,
        Mode3StartupSourceState::Abstract {
            remaining: MODE3_ABSTRACT_SOURCE_WINDOW_DOTS
        }
    );
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);

    let third = ppu.advance_mode3_output_phase();
    assert_eq!(third.kind, Mode3TransferDotKind::ServedPreVisibleTransfer);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 6);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
}

#[test]
fn mode3_started_keeps_an_explicit_abstract_source_window_before_fifo_backed_transfer() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 1 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 5;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
            execution: Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop,
            backing: Mode3TransferBacking::Abstract,
        })
    );

    let transfer_dot = ppu.advance_mode3_output_phase();
    assert_eq!(
        transfer_dot.kind,
        Mode3TransferDotKind::ServedPreVisibleTransfer
    );
    assert_eq!(
        ppu.bg_pipeline_state.startup_source_state,
        Mode3StartupSourceState::FifoBacked
    );

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
            execution: Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );
}

#[test]
fn mode3_started_keeps_an_explicit_previsible_lane_before_hidden_transfer() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 1;
    ppu.bg_pipeline_state.current_transfer_x = 5;
    ppu.bg_pipeline_state.fifo.push_back(0);

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
            execution: Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );

    let transfer_dot = ppu.advance_mode3_output_phase();
    assert_eq!(
        transfer_dot.kind,
        Mode3TransferDotKind::ServedPreVisibleTransfer
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining,
        0
    );

    ppu.bg_pipeline_state.fifo.push_back(0);
    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );
}

#[test]
fn late_hidden_dot_can_consume_a_startup_placeholder_before_the_first_real_fill() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedHiddenTransfer);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 8);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn late_hidden_scx_discard_can_consume_a_startup_placeholder_before_real_fifo_backing() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;

    ppu.advance_mode3_output_phase();

    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
    assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}
