use super::*;

#[test]
fn traced_lcdc3_write_on_visible_tile2_tail_keeps_tail_pixels_live_and_retargets_visible_tile3() {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x8B, 41, 0x96));
    ppu.write_window_tilemap_entry(1, 5, 0);
    ppu.write_window_tilemap_entry(2, 5, 0);
    ppu.write_bg_tilemap_entry(1, 5, 1);
    ppu.write_bg_tilemap_entry(2, 5, 1);
    let old_tile_row = 0x1000 + TILE_ROW_BYTES as usize;
    ppu.vram_bytes[old_tile_row] = 0x00;
    ppu.vram_bytes[old_tile_row + 1] = 0x00;
    let new_tile_row = 0x1010 + TILE_ROW_BYTES as usize;
    ppu.vram_bytes[new_tile_row] = 0xFF;
    ppu.vram_bytes[new_tile_row + 1] = 0x00;
    ppu.line_dot = 112;
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
    ppu.bg_pipeline_state.current_transfer_x = 18;
    ppu.bg_pipeline_state.visible_pixels_output = 10;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupContinuation(
                BgStartupContinuationSlice::VisibleTile2,
            ),
            fetch_x: BG_TILE_WIDTH as u16,
            tile_map_address: 0x1CA1,
            tile_data_address: 0x1003,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 3;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1CA2;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x1003;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x00;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x00;

    ppu.write_register(0xFF40, 0x83);

    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("visible tail should keep cached slice metadata after the traced write");
    assert_eq!(front_cached.pixel_index, 2);
    assert!(front_cached.cached.needs_live_tilemap_refetch);
    assert!(
        ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_refetch_on_push
    );

    for expected_visible_x in 10..14 {
        let result = advance_visible_output_step(&mut ppu);
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 1);
        let _ = ppu.advance_bg_fetcher_with_ppu_vram();
        ppu.line_dot += 1;
    }

    let remaining_visible_tile2 = ppu
        .bg_pipeline_state
        .fifo_cached_pixels
        .iter()
        .flatten()
        .filter(|cached| {
            cached.cached.origin
                == BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile2,
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(remaining_visible_tile2.len(), 2);
    assert!(
        remaining_visible_tile2
            .iter()
            .all(|cached| cached.cached.needs_live_tilemap_refetch)
    );

    let first_visible_tile3 = ppu
        .bg_pipeline_state
        .fifo_cached_pixels
        .iter()
        .flatten()
        .find(|cached| {
            cached.cached.origin
                == BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                )
        })
        .expect("traced write should still enqueue the retargeted VisibleTile3 slice");
    assert_eq!(first_visible_tile3.cached.tile_map_address, 0x18A2);
    assert_eq!(first_visible_tile3.cached.tile_index, 1);
    assert!(!first_visible_tile3.cached.needs_live_tilemap_refetch);
}

#[test]
fn traced_lcdc3_write_on_visible_tile2_earlier_tail_keeps_tail_pixels_live_and_retargets_visible_tile3()
 {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x8B, 36, 0x96));
    ppu.write_window_tilemap_entry(1, 5, 0);
    ppu.write_window_tilemap_entry(2, 5, 0);
    ppu.write_bg_tilemap_entry(1, 5, 1);
    ppu.write_bg_tilemap_entry(2, 5, 1);
    let old_tile_row = 0x1000 + TILE_ROW_BYTES as usize;
    ppu.vram_bytes[old_tile_row] = 0x00;
    ppu.vram_bytes[old_tile_row + 1] = 0x00;
    let new_tile_row = 0x1010 + TILE_ROW_BYTES as usize;
    ppu.vram_bytes[new_tile_row] = 0xFF;
    ppu.vram_bytes[new_tile_row + 1] = 0x00;
    ppu.line_dot = 112;
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
    ppu.bg_pipeline_state.current_transfer_x = 17;
    ppu.bg_pipeline_state.visible_pixels_output = 9;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupContinuation(
                BgStartupContinuationSlice::VisibleTile2,
            ),
            fetch_x: BG_TILE_WIDTH as u16,
            tile_map_address: 0x1CA1,
            tile_data_address: 0x1003,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 3;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1CA2;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x1003;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x00;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x00;

    ppu.write_register(0xFF40, 0x83);

    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("visible earlier tail should keep cached slice metadata after the traced write");
    assert_eq!(front_cached.pixel_index, 1);
    assert!(front_cached.cached.needs_live_tilemap_refetch);
    assert!(
        ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_refetch_on_push
    );

    for expected_visible_x in 9..14 {
        let result = advance_visible_output_step(&mut ppu);
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 1);
        let _ = ppu.advance_bg_fetcher_with_ppu_vram();
        ppu.line_dot += 1;
    }

    let remaining_visible_tile2 = ppu
        .bg_pipeline_state
        .fifo_cached_pixels
        .iter()
        .flatten()
        .filter(|cached| {
            cached.cached.origin
                == BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile2,
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(remaining_visible_tile2.len(), 2);
    assert!(
        remaining_visible_tile2
            .iter()
            .all(|cached| cached.cached.needs_live_tilemap_refetch)
    );

    let first_visible_tile3 = ppu
        .bg_pipeline_state
        .fifo_cached_pixels
        .iter()
        .flatten()
        .find(|cached| {
            cached.cached.origin
                == BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                )
        })
        .expect("traced earlier write should still enqueue the retargeted VisibleTile3 slice");
    assert_eq!(first_visible_tile3.cached.tile_map_address, 0x18A2);
    assert_eq!(first_visible_tile3.cached.tile_index, 1);
    assert!(!first_visible_tile3.cached.needs_live_tilemap_refetch);
}
