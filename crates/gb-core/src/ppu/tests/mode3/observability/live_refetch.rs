use super::*;

#[test]
fn visible_fifo_second_startup_tile_marks_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x91, 0x00, 0x00));
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile2,
                ),
                fetch_x: BG_TILE_WIDTH as u16,
                tile_map_address: 0x1801,
                tile_data_address: 0x0001,
                tile_low: 0x12,
                tile_high: 0x34,
                ..BgCachedSlice::default()
            },
            2,
        )));

    ppu.write_register(0xFF40, 0x99);

    let cached = ppu
        .bg_pipeline_state
        .fifo
        .cached_slot(0)
        .expect("BG FIFO cached slot must exist")
        .expect("visible FIFO pixel should keep cached slice metadata");
    assert!(cached.cached.needs_live_tilemap_refetch);
}

#[test]
fn visible_fifo_third_startup_tile_marks_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x91, 0x00, 0x00));
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                tile_map_address: 0x1802,
                tile_data_address: 0x0001,
                tile_low: 0x12,
                tile_high: 0x34,
                ..BgCachedSlice::default()
            },
            0,
        )));

    ppu.write_register(0xFF40, 0x99);

    let cached = ppu
        .bg_pipeline_state
        .fifo
        .cached_slot(0)
        .expect("BG FIFO cached slot must exist")
        .expect("visible FIFO pixel should keep cached slice metadata");
    assert!(cached.cached.needs_live_tilemap_refetch);
}

#[test]
fn visible_fifo_visible_output_recomputes_marked_second_tilemap_pixel_on_demand() {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x91, 0x00, 0x00));
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_window_tilemap_entry(1, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(1, 0, 0xFF, 0x00);
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile2,
                ),
                fetch_x: BG_TILE_WIDTH as u16,
                tile_map_address: 0x1801,
                tile_data_address: 0x0001,
                tile_index: 0,
                tile_low: 0x00,
                tile_high: 0x00,
                ..BgCachedSlice::default()
            },
            0,
        )));

    ppu.write_register(0xFF40, 0x99);

    let result = ppu.advance_mode3_output_phase_with_ppu_vram();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[0], 1);
}

#[test]
fn visible_fifo_visible_output_recomputes_marked_tilemap_pixel_on_demand() {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x91, 0x00, 0x00));
    ppu.write_bg_tilemap_entry(2, 0, 0);
    ppu.write_window_tilemap_entry(2, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(1, 0, 0xFF, 0x00);
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                tile_map_address: 0x1802,
                tile_data_address: 0x0001,
                tile_index: 0,
                tile_low: 0x00,
                tile_high: 0x00,
                ..BgCachedSlice::default()
            },
            0,
        )));

    ppu.write_register(0xFF40, 0x99);

    let result = ppu.advance_mode3_output_phase_with_ppu_vram();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[0], 1);
}
