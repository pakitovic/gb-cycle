use super::*;

#[test]
fn visible_fifo_sideband_keeps_full_cached_slice_metadata_for_future_closure_work() {
    let mut ppu = PpuTestRig::dmg();

    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupContinuation(
                BgStartupContinuationSlice::VisibleTile3,
            ),
            fetch_x: BG_TILE_WIDTH as u16 * 2,
            tile_map_address: 0x1802,
            tile_data_address: 0x0001,
            tile_index: 3,
            tile_low: 0x12,
            tile_high: 0x34,
            same_cycle_live_tilemap_refetch_window_open: true,
            ..BgCachedSlice::default()
        });

    let cached = ppu.bg_pipeline_state.fifo_cached_pixels[3]
        .expect("visible FIFO pixel should keep cached slice metadata");
    assert_eq!(
        cached.cached.origin,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
    );
    assert_eq!(cached.cached.fetch_x, BG_TILE_WIDTH as u16 * 2);
    assert_eq!(cached.cached.tile_map_address, 0x1802);
    assert_eq!(cached.cached.tile_data_address, 0x0001);
    assert_eq!(cached.cached.tile_index, 3);
    assert!(cached.cached.same_cycle_live_tilemap_refetch_window_open);
    assert_eq!(cached.pixel_index, 3);
}

#[test]
fn snapshot_exports_visible_fifo_cached_slice_metadata() {
    let mut ppu = PpuTestRig::dmg();

    ppu.bg_pipeline_state.fifo.push_back(2);
    ppu.bg_pipeline_state
        .fifo_cached_pixels
        .push_back(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                tile_map_address: 0x1802,
                tile_data_address: 0x0001,
                tile_index: 3,
                same_cycle_live_tilemap_refetch_window_open: true,
                needs_live_tilemap_refetch: true,
                tile_low: 0x12,
                tile_high: 0x34,
                ..BgCachedSlice::default()
            },
            5,
        )));

    let snapshot = ppu.snapshot();
    let cached = snapshot.bg_fifo_cached_pixels[0]
        .expect("snapshot should export visible FIFO sideband metadata");

    assert_eq!(snapshot.bg_fifo_pixels, vec![2]);
    assert_eq!(
        cached.origin,
        PpuBgCachedSliceOriginSnapshot::StartupContinuationVisibleTile3
    );
    assert_eq!(cached.fetch_x, BG_TILE_WIDTH as u16 * 2);
    assert_eq!(cached.pixel_index, 5);
    assert!(cached.same_cycle_live_tilemap_refetch_window_open);
    assert!(cached.needs_live_tilemap_refetch);
    assert_eq!(cached.tile_map_address, 0x1802);
    assert_eq!(cached.tile_data_address, 0x0001);
    assert_eq!(cached.tile_index, 3);
}

#[test]
fn snapshot_exports_mode3_startup_seam_observability() {
    let mut ppu = PpuTestRig::dmg();
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 3 };
    ppu.bg_pipeline_state.begin_post_alignment_followup();
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 4;
    ppu.bg_pipeline_state
        .fetcher
        .post_alignment_fetch_restart_delay_dots = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 12;

    let snapshot = ppu.snapshot();

    assert_eq!(
        snapshot.bg_startup_source_state,
        PpuMode3StartupSourceStateSnapshot::Abstract { remaining: 3 }
    );
    assert_eq!(
        snapshot.bg_startup_fetch_seam,
        PpuBgStartupFetchSeamSnapshot::PostAlignment {
            first_real_push_skips_entry_delay: true,
            next_startup_continuation_slice: PpuBgStartupContinuationSliceSnapshot::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        }
    );
    assert_eq!(snapshot.bg_startup_fifo_placeholders, 2);
    assert_eq!(snapshot.bg_push_entry_delay_remaining, 1);
    assert_eq!(snapshot.bg_fill_startup_dummy_pixels, 4);
    assert_eq!(snapshot.bg_fetcher_post_alignment_restart_delay_dots, 1);
    assert_eq!(
        snapshot.bg_transfer_phase,
        PpuMode3TransferPhaseSnapshot::Output
    );
    assert_eq!(snapshot.bg_current_transfer_x, 12);
}

#[test]
fn scheduler_trace_reports_mode3_startup_and_cached_slice_observability() {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x91, 0x00, 0x00));
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state
        .fetcher
        .post_alignment_fetch_restart_delay_dots = 1;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached =
        BgCachedSlice::default().with_origin(BgCachedSliceOrigin::StartupAlignmentFill);
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 4;
    ppu.bg_pipeline_state.fill.cached = BgCachedSlice::default().with_origin(
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
    );
    ppu.bg_pipeline_state.fifo.push_back(2);
    ppu.bg_pipeline_state
        .fifo_cached_pixels
        .push_back(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                ..BgCachedSlice::default()
            },
            5,
        )));
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 3 };
    ppu.bg_pipeline_state.begin_post_alignment_followup();
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 12;
    ppu.bg_pipeline_state.visible_pixels_output = 9;

    let trace = ppu.scheduler_trace_message(&CycleContext::for_cycle(TCycle::new(123)));

    assert!(trace.contains("t_cycle=123"));
    assert!(trace.contains("bg_source=Background"));
    assert!(trace.contains("bg_stage=TileDataHigh"));
    assert!(trace.contains("bg_stage_dot=1"));
    assert!(trace.contains("bg_fetch_origin=StartupContinuation(VisibleTile2)"));
    assert!(trace.contains("bg_push_pending=true"));
    assert!(trace.contains("bg_push_entry_delay_remaining=1"));
    assert!(trace.contains("bg_push_origin=StartupAlignmentFill"));
    assert!(trace.contains("bg_fill_pending=true"));
    assert!(trace.contains("bg_fill_startup_dummy_pixels=4"));
    assert!(trace.contains("bg_fill_origin=StartupContinuation(VisibleTile3)"));
    assert!(trace.contains("bg_fifo_len=1"));
    assert!(trace.contains("bg_startup_fifo_placeholders=2"));
    assert!(trace.contains("bg_fifo_front_cached_origin=Some(StartupContinuation(VisibleTile3))"));
    assert!(trace.contains("bg_fifo_front_cached_fetch_x=Some(16)"));
    assert!(trace.contains("bg_fifo_front_cached_pixel_index=Some(5)"));
    assert!(trace.contains("bg_startup_source_state=Abstract { remaining: 3 }"));
    assert!(trace.contains("bg_startup_fetch_seam=PostAlignment"));
    assert!(trace.contains("bg_fetcher_post_alignment_restart_delay_dots=1"));
    assert!(trace.contains("bg_transfer_phase=Output"));
    assert!(trace.contains("bg_current_transfer_x=12"));
    assert!(trace.contains("bg_current_transfer_lane=Some(Visible)"));
    assert!(trace.contains("bg_current_transfer_source_window=Some(FifoBacked)"));
    assert!(trace.contains("bg_current_transfer_backing=Some(FifoBacked)"));
    assert!(trace.contains("bg_current_transfer_readiness=Some(Ready)"));
    assert!(trace.contains("bg_current_transfer_kind=Some(ServedVisiblePixel)"));
    assert!(trace.contains("visible_pixels_output=9"));
}

#[test]
fn snapshot_and_trace_export_current_transfer_context_observability() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);

    let snapshot = ppu.snapshot();

    assert_eq!(
        snapshot.bg_current_transfer_lane,
        Some(PpuMode3TransferLaneSnapshot::Visible)
    );
    assert_eq!(
        snapshot.bg_current_transfer_source_window,
        Some(PpuMode3TransferSourceWindowSnapshot::FifoBacked)
    );
    assert_eq!(
        snapshot.bg_current_transfer_backing,
        Some(PpuMode3TransferBackingSnapshot::FifoBacked)
    );
    assert_eq!(
        snapshot.bg_current_transfer_readiness,
        Some(PpuMode3TransferReadinessSnapshot::Ready)
    );
    assert_eq!(
        snapshot.bg_current_transfer_kind,
        Some(PpuMode3TransferDotKindSnapshot::ServedVisiblePixel)
    );

    let trace = ppu.scheduler_trace_message(&CycleContext::for_cycle(TCycle::new(123)));

    assert!(trace.contains("bg_current_transfer_lane=Some(Visible)"));
    assert!(trace.contains("bg_current_transfer_source_window=Some(FifoBacked)"));
    assert!(trace.contains("bg_current_transfer_backing=Some(FifoBacked)"));
    assert!(trace.contains("bg_current_transfer_readiness=Some(Ready)"));
    assert!(trace.contains("bg_current_transfer_kind=Some(ServedVisiblePixel)"));
}
