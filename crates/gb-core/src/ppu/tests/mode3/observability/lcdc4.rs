use super::*;

#[test]
fn traced_lcdc4_write_behind_startup_alignment_fill_retains_visible_tile2_until_first_handoff() {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x83, 41, 0x96));
    ppu.write_bg_tile_row(0, 1, 0x00, 0x00);
    ppu.vram_bytes[0x0002] = 0xFF;
    ppu.vram_bytes[0x0003] = 0x00;
    ppu.line_dot = 104;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 263;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 3;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 10;
    ppu.bg_pipeline_state.visible_pixels_output = 2;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupAlignmentFill,
            tile_map_address: 0x18A0,
            tile_data_address: 0x1003,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x18A1;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x1003;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x00;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x00;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_map_address: 0x18A1,
        tile_data_address: 0x1003,
        tile_index: 0,
        tile_low: 0x00,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };

    ppu.write_register(0xFF40, 0x93);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_refetch
    );

    for expected_visible_x in 2..8 {
        let result = advance_visible_output_step(&mut ppu);
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 0);
        let _ = ppu.advance_bg_fetcher_with_ppu_vram();
        ppu.line_dot += 1;
    }

    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(!ppu.bg_pipeline_state.fill.pending);
    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("VisibleTile2 should be at the FIFO front before the first visible handoff");
    assert_eq!(
        front_cached.cached.origin,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
    );
    assert_eq!(front_cached.pixel_index, 0);

    let result = advance_visible_output_step(&mut ppu);
    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[8], 1);
}

#[test]
fn traced_lcdc4_write_after_first_left_edge_pixel_still_retargets_visible_tile2_handoff() {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x83, 36, 0x96));
    ppu.write_bg_tile_row(0, 1, 0x00, 0x00);
    ppu.vram_bytes[0x0002] = 0xFF;
    ppu.vram_bytes[0x0003] = 0x00;
    ppu.line_dot = 103;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 263;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupAlignmentFill,
            tile_map_address: 0x18A0,
            tile_data_address: 0x1003,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x18A1;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x1003;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x00;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x00;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_map_address: 0x18A1,
        tile_data_address: 0x1003,
        tile_index: 0,
        tile_low: 0x00,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };

    let first_dot = ppu.advance_mode3_output_phase_with_ppu_vram();
    assert_eq!(first_dot.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[0], 0);
    let _ = ppu.advance_bg_fetcher_with_ppu_vram();
    ppu.line_dot += 1;

    ppu.write_register(0xFF40, 0x93);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_refetch
    );

    for expected_visible_x in 1..8 {
        let result = advance_visible_output_step(&mut ppu);
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 0);
        let _ = ppu.advance_bg_fetcher_with_ppu_vram();
        ppu.line_dot += 1;
    }

    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("VisibleTile2 should reach the FIFO front after the alignment fill tail");
    assert_eq!(
        front_cached.cached.origin,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
    );
    assert_eq!(front_cached.pixel_index, 0);

    let handoff_dot = advance_visible_output_step(&mut ppu);
    assert_eq!(handoff_dot.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[8], 1);
}

#[test]
fn traced_startup_alignment_fill_keeps_front_visible_pixels_before_visible_tile2_handoff() {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x8B, 36, 0x96));
    ppu.line_dot = 107;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 263;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 12;
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupAlignmentFill,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    for _ in 0..4 {
        let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    }
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_map_address: 0x1CA1,
        tile_data_address: 0x1003,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };

    ppu.flush_pending_bg_fifo_fill();

    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("startup alignment fill should still be at the FIFO front after the flush");
    assert_eq!(
        front_cached.cached.origin,
        BgCachedSliceOrigin::StartupAlignmentFill
    );
    assert_eq!(front_cached.pixel_index, 4);

    for expected_visible_x in 4..8 {
        let result = ppu.advance_mode3_output_phase_with_ppu_vram();
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 0);
        ppu.line_dot += 1;
    }

    let next_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("VisibleTile2 should take ownership once the alignment fill tail is gone");
    assert_eq!(
        next_cached.cached.origin,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
    );
    assert_eq!(next_cached.pixel_index, 0);

    let result = ppu.advance_mode3_output_phase_with_ppu_vram();
    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[8], 1);
}
