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
fn mode3_initial_scx_capture_uses_the_visible_scx_after_startup_dummy_dots() {
    let mut ppu = PpuTestRig::dmg();

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
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
    assert_eq!(ppu.snapshot().line_dot, 80);
    assert!(ppu.bg_pipeline_state.initial_scx_capture_pending);
    assert_eq!(ppu.bg_pipeline_state.initial_scx_discard, 0);
    assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);

    ppu.tick_n(2);
    assert_eq!(ppu.snapshot().line_dot, 82);
    assert_eq!(ppu.snapshot().visible_scx, 0x00);
    ppu.write_register(0xFF43, 0x05);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);

    ppu.tick();
    assert_eq!(ppu.snapshot().line_dot, 83);
    assert_eq!(ppu.snapshot().visible_scx, 0x05);
    assert!(!ppu.bg_pipeline_state.initial_scx_capture_pending);
    assert_eq!(ppu.bg_pipeline_state.initial_scx_discard, 0x05);
    assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0x05);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 5);
}

#[test]
fn cpu_mmio_scx_write_during_alignment_seed_retunes_the_current_line_discard_budget() {
    let mut ppu = PpuTestRig::dmg();

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
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

    ppu.tick_n(84);

    let before = ppu.snapshot();
    assert_eq!(before.mode, PpuAccessMode::Drawing);
    assert_eq!(before.visible_scx, 0x00);
    assert_eq!(before.bg_current_transfer_x, 1);
    assert_eq!(before.visible_pixels_output, 0);
    assert_eq!(before.mode0_start_dot, MODE0_START_DOT);
    assert!(matches!(
        before.bg_startup_fetch_seam,
        PpuBgStartupFetchSeamSnapshot::AlignmentSeedPending
    ));

    ppu.write_register_with_source(0xFF43, 0x02, PpuRegisterWriteSource::CpuMmioCommit);

    let pending = ppu.snapshot();
    assert_eq!(pending.visible_scx, 0x00);
    assert_eq!(pending.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(pending.scx_discard_remaining, 0);

    ppu.tick();

    let after = ppu.snapshot();
    assert_eq!(after.line_dot, 85);
    assert_eq!(after.visible_scx, 0x02);
    assert_eq!(after.bg_current_transfer_x, 0);
    assert_eq!(after.visible_pixels_output, 0);
    assert_eq!(after.mode0_start_dot, MODE0_START_DOT + 2);
    assert_eq!(after.scx_discard_remaining, 0);
}

#[test]
fn cpu_mmio_scx_write_after_alignment_seed_does_not_retune_the_current_line_discard_budget() {
    let mut ppu = PpuTestRig::dmg();

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
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

    ppu.tick_n(88);

    let before = ppu.snapshot();
    assert_eq!(before.mode, PpuAccessMode::Drawing);
    assert_eq!(before.visible_scx, 0x00);
    assert_eq!(before.bg_current_transfer_x, 5);
    assert_eq!(before.visible_pixels_output, 0);
    assert_eq!(before.mode0_start_dot, MODE0_START_DOT);
    assert!(matches!(
        before.bg_startup_fetch_seam,
        PpuBgStartupFetchSeamSnapshot::PostAlignment { .. }
    ));

    ppu.write_register_with_source(0xFF43, 0x02, PpuRegisterWriteSource::CpuMmioCommit);
    ppu.tick();

    let after = ppu.snapshot();
    assert_eq!(after.line_dot, 89);
    assert_eq!(after.visible_scx, 0x02);
    assert_eq!(after.bg_current_transfer_x, 6);
    assert_eq!(after.visible_pixels_output, 0);
    assert_eq!(after.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(after.scx_discard_remaining, 0);
}

#[test]
fn previsible_live_scx_retarget_accounts_for_already_consumed_discard_dots() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.initial_scx_discard = 5;
    ppu.bg_pipeline_state.scx_discard_remaining = 3;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 5;
    ppu.visible_registers.scx = 0x02;
    ppu.pipeline_registers.scx = 0x05;

    let visible_scx = ppu.mode3_register_latches().visible().scx;
    ppu.bg_pipeline_state
        .retune_previsible_scx_discard(visible_scx);

    assert_eq!(ppu.bg_pipeline_state.initial_scx_discard, 2);
    assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 2);
}

#[test]
#[ignore = "diagnostic cpu-commit SCX write against a startup SCX=2 baseline on the same line"]
fn cpu_commit_scx_alignment_seed_tail_matches_startup_scx2_baseline() {
    fn seeded_ppu(scx: u8) -> PpuTestRig {
        let mut ppu = PpuTestRig::dmg();
        ppu.write_bg_tile_row(0x00, 0, 0x00, 0x00);
        ppu.write_bg_tile_row(0x19, 0, 0xFF, 0xFF);
        for tile_x in 0..32 {
            ppu.write_bg_tilemap_entry(tile_x, 0, if tile_x == 19 { 0x19 } else { 0x00 });
        }
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x82,
            scy: 0x00,
            scx,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu
    }

    let mut startup = seeded_ppu(0x02);
    while startup.snapshot().mode != PpuAccessMode::HBlank {
        startup.tick();
    }

    let mut live = seeded_ppu(0x00);
    live.tick_n(84);
    live.write_register_with_source(0xFF43, 0x02, PpuRegisterWriteSource::CpuMmioCommit);
    while live.snapshot().mode != PpuAccessMode::HBlank {
        live.tick();
    }

    println!(
        "startup tail={:?} live tail={:?}",
        &startup.snapshot().current_scanline_pixels[148..160],
        &live.snapshot().current_scanline_pixels[148..160]
    );
}

#[test]
#[ignore = "diagnostic startup scx=2 versus scx=0 state at line_dot=84"]
fn startup_scx2_state_at_line84() {
    fn seeded_ppu(scx: u8) -> PpuTestRig {
        let mut ppu = PpuTestRig::dmg();
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x82,
            scy: 0x00,
            scx,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu
    }

    let mut scx0 = seeded_ppu(0x00);
    scx0.tick_n(84);
    println!(
        "scx0 dot84 x={} vpo={} placeholders={} stage={:?} stage_dot={} seam={:?} discard={} mode0={}",
        scx0.snapshot().bg_current_transfer_x,
        scx0.snapshot().visible_pixels_output,
        scx0.snapshot().bg_startup_fifo_placeholders,
        scx0.snapshot().bg_fetcher_stage,
        scx0.snapshot().bg_fetcher_stage_dot,
        scx0.snapshot().bg_startup_fetch_seam,
        scx0.snapshot().scx_discard_remaining,
        scx0.snapshot().mode0_start_dot,
    );

    let mut scx2 = seeded_ppu(0x02);
    scx2.tick_n(84);
    println!(
        "scx2 dot84 x={} vpo={} placeholders={} stage={:?} stage_dot={} seam={:?} discard={} mode0={}",
        scx2.snapshot().bg_current_transfer_x,
        scx2.snapshot().visible_pixels_output,
        scx2.snapshot().bg_startup_fifo_placeholders,
        scx2.snapshot().bg_fetcher_stage,
        scx2.snapshot().bg_fetcher_stage_dot,
        scx2.snapshot().bg_startup_fetch_seam,
        scx2.snapshot().scx_discard_remaining,
        scx2.snapshot().mode0_start_dot,
    );
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
fn mode3_startup_scx_low_bits_shift_the_first_visible_background_pixels() {
    fn patterned_startup(scx: u8) -> PpuTestRig {
        let mut ppu = PpuTestRig::dmg();
        ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
        for tile_x in 0..32 {
            ppu.write_bg_tilemap_entry(tile_x, 0, 0);
        }
        ppu.apply_startup_state(PpuStartupState {
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
        });
        ppu
    }

    let mut scx0 = patterned_startup(0x00);
    while scx0.snapshot().visible_pixels_output != 8 {
        assert!(scx0.t_cycle < 140);
        scx0.tick();
    }

    let mut scx2 = patterned_startup(0x02);
    while scx2.snapshot().visible_pixels_output != 8 {
        assert!(scx2.t_cycle < 140);
        scx2.tick();
    }

    assert_eq!(
        scx0.snapshot().current_scanline_pixels[..8],
        [0, 1, 2, 3, 0, 1, 2, 3]
    );
    assert_eq!(
        scx2.snapshot().current_scanline_pixels[..8],
        [2, 3, 0, 1, 2, 3, 0, 1]
    );
}

#[test]
fn cpu_mmio_scx_write_during_alignment_seed_matches_startup_scx2_pixel_phase() {
    fn patterned_startup(scx: u8) -> PpuTestRig {
        let mut ppu = PpuTestRig::dmg();
        ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
        for tile_x in 0..32 {
            ppu.write_bg_tilemap_entry(tile_x, 0, 0);
        }
        ppu.apply_startup_state(PpuStartupState {
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
        });
        ppu
    }

    let mut startup = patterned_startup(0x02);
    while startup.snapshot().visible_pixels_output != 8 {
        assert!(startup.t_cycle < 140);
        startup.tick();
    }

    let mut live = patterned_startup(0x00);
    live.tick_n(84);
    live.write_register_with_source(0xFF43, 0x02, PpuRegisterWriteSource::CpuMmioCommit);
    while live.snapshot().visible_pixels_output != 8 {
        assert!(live.t_cycle < 140);
        live.tick();
    }

    assert_eq!(
        live.snapshot().current_scanline_pixels[..8],
        startup.snapshot().current_scanline_pixels[..8]
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
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
