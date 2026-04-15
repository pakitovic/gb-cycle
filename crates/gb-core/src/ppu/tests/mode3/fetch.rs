use super::super::*;

fn dmg_fetch_rig() -> PpuTestRig {
    PpuTestRig::dmg()
}

fn dmg_fetch_startup_rig(lcdc: u8) -> PpuTestRig {
    PpuTestRig::dmg().with_startup_state(PpuStartupState {
        lcdc,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    })
}

#[test]
fn bg_fetcher_stage_dot_is_an_explicit_one_dot_automaton() {
    let mut ppu = dmg_fetch_rig();
    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.write_bg_tilemap_entry(0, 0, 0);

    ppu.visible_registers.lcdc = 0x91;
    ppu.bg_pipeline_state.fetcher.start_background();

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataLow
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataLow
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);
    assert!(ppu.bg_pipeline_state.push.pending);
    assert!(!ppu.bg_pipeline_state.fill.pending);
}

#[test]
fn bg_fetcher_records_the_tilemap_address_for_the_current_phase() {
    let mut ppu = dmg_fetch_rig();
    ppu.vram_bytes[0x1C64] = 0x66;

    ppu.visible_registers.lcdc = 0x99;
    ppu.visible_registers.scx = 24;
    ppu.visible_registers.scy = 16;
    ppu.ly = 8;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 8;

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C64);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x66);
}

#[test]
fn bg_fetcher_recomputes_scy_for_each_tile_data_plane_read_on_dmg() {
    let mut ppu = dmg_fetch_rig();
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(0, 1, 0x56, 0x78);

    ppu.visible_registers.lcdc = 0x91;
    ppu.visible_registers.scy = 0;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0000);

    ppu.visible_registers.scy = 1;
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x78);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0003);
}

#[test]
fn bg_fetcher_recomputes_tile_data_address_when_tile_selector_changes_between_planes() {
    let mut ppu = dmg_fetch_rig();
    ppu.write_bg_tile_row(1, 0, 0x12, 0x34);
    ppu.vram_bytes[0x1011] = 0xAB;

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.visible_registers.scy = 0;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0010);

    ppu.visible_registers.lcdc = 0x81;
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1011);
}

#[test]
fn cached_background_push_recomputes_tilemap_and_tiledata_on_push_dot_zero_map_change() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_window_tilemap_entry(1, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C01);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 1);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0011);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0xCD);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_high, 0xCD);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
}

#[test]
fn cached_background_push_accepts_same_tcycle_tilemap_refetch_after_entry_delay_dot() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_window_tilemap_entry(1, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::EntryDelay);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );

    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0xCD);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(
        !ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
}

#[test]
fn cached_background_push_recomputes_tilemap_when_scx_tile_column_changes() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_bg_tilemap_entry(2, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.fetch_x = 8;
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::EntryDelay);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
    ppu.write_register(0xFF43, 0x08);
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0xCD);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1802);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 1);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0011);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0xCD);
}

#[test]
fn cached_background_push_ignores_scx_low_bit_only_changes() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_bg_tilemap_entry(2, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.fetch_x = 8;
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::EntryDelay);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
    ppu.write_register(0xFF43, 0x02);
    assert!(!ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0x34);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1801);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0001);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x34);
}

#[test]
fn late_second_startup_continuation_push_marks_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;

    ppu.write_register(0xFF40, 0x99);

    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
}

#[test]
fn third_startup_continuation_fetcher_carries_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };

    ppu.write_register(0xFF40, 0x99);
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());

    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.push.cached.origin,
        ppu.bg_pipeline_state.fetcher.cached_origin
    );
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
}

#[test]
fn third_startup_continuation_fetcher_carries_full_tilemap_refetch_on_scx_tile_column_change() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };

    ppu.write_register(0xFF43, 0x08);
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());

    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.push.cached.origin,
        ppu.bg_pipeline_state.fetcher.cached_origin
    );
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tilemap_full_refetch
    );
}

#[test]
fn startup_visible_tile3_scx_boundary_full_refetch_stays_narrow_to_the_late_high0_window() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(2, 0, 0);
    ppu.write_bg_tilemap_entry(5, 0, 1);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + 112;
    ppu.ly = 0;
    ppu.bg_pipeline_state.visible_pixels_output = 7;
    ppu.bg_pipeline_state.current_transfer_x = 15;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };

    ppu.write_register(0xFF43, 0x18);

    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .startup_visible_tile3_scx_boundary_full_refetch_next_tile
    );
    assert!(
        ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_full_refetch_on_push
    );

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(ppu.bg_pipeline_state.push.pending);

    ppu.with_ppu_vram(|ppu, vram| ppu.maybe_recompute_pending_background_push(vram));

    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_map_address, 0x1805);
}

fn configure_startup_visible_tile3_current_fetch_boundary(ppu: &mut PpuTestRig) {
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + 112;
    ppu.ly = 0;
    ppu.bg_pipeline_state.current_transfer_x = 16;
    ppu.bg_pipeline_state.visible_pixels_output = 8;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state
        .fetcher
        .needs_live_tilemap_refetch_on_push = true;
    ppu.bg_pipeline_state
        .fetcher
        .needs_live_tilemap_full_refetch_on_push = true;
    ppu.bg_pipeline_state
        .fetcher
        .startup_visible_tile3_scx_boundary_full_refetch_next_tile = true;
    ppu.bg_pipeline_state
        .fetcher
        .startup_visible_tile3_scx_boundary_old_tail_start_pixel = 3;
    ppu.bg_pipeline_state
        .startup_visible_tile3_scx_boundary_next_slice_previous_scx = Some(0x88);
    ppu.bg_pipeline_state
        .startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels = 3;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
}

#[test]
fn startup_visible_tile3_scx_boundary_write_clears_current_fetch_full_refetch_state() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    configure_startup_visible_tile3_current_fetch_boundary(&mut ppu);

    ppu.write_register(0xFF43, 0x18);

    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_refetch_on_push
    );
    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_full_refetch_on_push
    );
    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .startup_visible_tile3_scx_boundary_full_refetch_next_tile
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .fetcher
            .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
        BG_TILE_WIDTH
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .startup_visible_tile3_scx_boundary_next_slice_previous_scx,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels,
        0
    );
}

fn configure_startup_visible_tile3_push_boundary(
    ppu: &mut PpuTestRig,
    current_transfer_x: u8,
    visible_pixels_output: u8,
) {
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + 112;
    ppu.ly = 0;
    ppu.bg_pipeline_state.current_transfer_x = current_transfer_x;
    ppu.bg_pipeline_state.visible_pixels_output = visible_pixels_output;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::Inactive;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        needs_live_tilemap_refetch: true,
        needs_live_tilemap_full_refetch: true,
        startup_visible_tile3_scx_boundary_previous_scx: Some(0x44),
        startup_visible_tile3_scx_boundary_old_tail_start_pixel: BG_TILE_WIDTH,
        startup_visible_tile3_scx_boundary_old_prefix_pixels: 0,
        ..BgCachedSlice::default()
    };
    let visible_tile2 = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_low: 0xFF,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels_with_skip(
            visible_tile2,
            current_transfer_x.saturating_sub(16),
        );
}

#[test]
fn startup_visible_tile3_scx_low_band_old_pixel_window_clears_broad_refetch_state() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    configure_startup_visible_tile3_push_boundary(&mut ppu, 21, 13);
    ppu.bg_pipeline_state
        .push
        .cached
        .startup_visible_tile3_scx_boundary_old_tail_start_pixel = 2;
    ppu.bg_pipeline_state
        .push
        .cached
        .startup_visible_tile3_scx_boundary_old_prefix_pixels = 3;

    ppu.write_register(0xFF43, 0x0B);

    assert!(!ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
    assert!(
        !ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tilemap_full_refetch
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_previous_scx,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
        BG_TILE_WIDTH
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_old_prefix_pixels,
        0
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx,
        Some(0)
    );
}

#[test]
fn startup_visible_tile3_scx_next_tile_output_retarget_covers_low_band_classes() {
    for (scx, expected_prefix, expected_tail) in [
        (0x58, 1, BG_TILE_WIDTH),
        (0x61, 2, BG_TILE_WIDTH),
        (0x63, 5, 4),
        (0x65, 5, 4),
        (0x76, 1, 3),
        (0x78, 1, 7),
        (0x7A, 0, 5),
    ] {
        let mut ppu = dmg_fetch_startup_rig(0x91);
        configure_startup_visible_tile3_push_boundary(&mut ppu, 22, 14);

        ppu.write_register(0xFF43, scx);

        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx,
            Some(scx),
            "SCX {scx:#04X} should retarget the carried tile"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_old_prefix_pixels,
            expected_prefix,
            "SCX {scx:#04X} old-prefix span"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
            expected_tail,
            "SCX {scx:#04X} old-tail start"
        );
    }
}

#[test]
fn visible_tile3_scx_boundary_old_tail_window_preserves_old_pixels_on_output() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.visible_registers.scx = 0x1B;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.write_bg_tilemap_entry(2, 0, 0);
    ppu.write_bg_tilemap_entry(5, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(1, 0, 0xFF, 0x00);

    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        tile_map_address: 0x1805,
        tile_data_address: 0x0011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };
    cached.arm_startup_visible_tile3_scx_boundary_old_tail(0x00, 0x1B);
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let mut pixels = Vec::new();
    for _ in 0..BG_TILE_WIDTH {
        let pixel = ppu
            .with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
            .expect("queued slice should still expose a visible pixel");
        pixels.push(pixel);
    }

    assert_eq!(pixels, vec![1, 1, 1, 1, 1, 1, 0, 0]);
}

#[test]
fn ordinary_slice_after_visible_tile3_scx_boundary_preserves_old_prefix_pixel_on_output() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.visible_registers.scx = 0x1B;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.write_bg_tilemap_entry(3, 0, 0);
    ppu.write_bg_tilemap_entry(6, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(1, 0, 0xFF, 0x00);

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::Ordinary,
        fetch_x: BG_TILE_WIDTH as u16 * 3,
        tile_map_address: 0x1806,
        tile_data_address: 0x0011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0x00,
        startup_visible_tile3_scx_boundary_previous_scx: Some(0x00),
        startup_visible_tile3_scx_boundary_old_tail_start_pixel: BG_TILE_WIDTH,
        startup_visible_tile3_scx_boundary_old_prefix_pixels: 1,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let mut pixels = Vec::new();
    for _ in 0..BG_TILE_WIDTH {
        let pixel = ppu
            .with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
            .expect("queued slice should still expose a visible pixel");
        pixels.push(pixel);
    }

    assert_eq!(pixels, vec![0, 1, 1, 1, 1, 1, 1, 1]);
}

#[test]
fn ordinary_background_fetcher_carries_full_tilemap_refetch_on_scx_tile_column_change() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin = BgCachedSliceOrigin::Ordinary;
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 3;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;

    ppu.write_register(0xFF43, 0x08);
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());

    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.push.cached.origin,
        BgCachedSliceOrigin::Ordinary
    );
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tilemap_full_refetch
    );
}

fn live_write_registers(lcdc: u8, scx: u8) -> PpuVisibleRegisters {
    PpuVisibleRegisters {
        lcdc,
        scx,
        ..PpuVisibleRegisters::default()
    }
}

#[test]
fn live_background_write_effects_ignore_non_background_or_dummy_slices() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x99, 0x08),
    );

    let mut window_push = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        same_cycle_live_tilemap_refetch_window_open: true,
        ..BgCachedSlice::default()
    };
    PpuMode3LiveBackgroundWriteEffects::for_push_pending_slice(
        window_push,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
        true,
    )
    .apply_to_cached_slice(&mut window_push);
    assert!(!window_push.needs_live_tilemap_refetch);
    assert!(!window_push.needs_live_tilemap_full_refetch);

    let mut dummy_fill = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        same_cycle_live_tilemap_refetch_window_open: true,
        ..BgCachedSlice::default()
    };
    PpuMode3LiveBackgroundWriteEffects::for_fill_pending_slice(
        dummy_fill,
        PpuMode3LiveBackgroundRegister::Scx,
        write_context,
        false,
        0,
    )
    .apply_to_cached_slice(&mut dummy_fill);
    assert!(!dummy_fill.needs_live_tilemap_refetch);
    assert!(!dummy_fill.needs_live_tilemap_full_refetch);
}

#[test]
fn live_background_write_effects_mark_fill_full_refetch_on_scx_tile_column_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x91, 0x08),
    );
    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        same_cycle_live_tilemap_refetch_window_open: true,
        ..BgCachedSlice::default()
    };

    PpuMode3LiveBackgroundWriteEffects::for_fill_pending_slice(
        cached,
        PpuMode3LiveBackgroundRegister::Scx,
        write_context,
        true,
        0,
    )
    .apply_to_cached_slice(&mut cached);

    assert!(cached.needs_live_tilemap_refetch);
    assert!(cached.needs_live_tilemap_full_refetch);
}

#[test]
fn live_background_write_effects_mark_startup_fill_full_refetch_on_scx_tile_column_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x91, 0x08),
    );
    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        ..BgCachedSlice::default()
    };

    PpuMode3LiveBackgroundWriteEffects::for_fill_pending_slice(
        cached,
        PpuMode3LiveBackgroundRegister::Scx,
        write_context,
        true,
        0,
    )
    .apply_to_cached_slice(&mut cached);

    assert!(cached.needs_live_tilemap_refetch);
    assert!(cached.needs_live_tilemap_full_refetch);
}

#[test]
fn live_background_write_effects_mark_visible_tile3_current_fetch_on_lcdc3_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x99, 0x00),
    );
    let fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Background,
        stage: PpuBgFetcherStage::TileDataLow,
        cached_origin: BgCachedSliceOrigin::StartupContinuation(
            BgStartupContinuationSlice::VisibleTile3,
        ),
        ..BgFetcherState::default()
    };
    let effects = PpuMode3LiveBackgroundWriteEffects::for_current_background_fetch(
        fetcher,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
    );

    let mut cached = BgCachedSlice::default();
    effects.apply_to_cached_slice(&mut cached);
    assert!(cached.needs_live_tilemap_refetch);

    let mut fetcher = fetcher;
    effects.apply_to_fetcher(&mut fetcher);
    assert!(fetcher.needs_live_tilemap_refetch_on_push);
}

#[test]
fn live_background_write_effects_mark_visible_tile3_high_byte_fetch_on_lcdc3_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x99, 0x00),
    );
    let fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Background,
        stage: PpuBgFetcherStage::TileDataHigh,
        cached_origin: BgCachedSliceOrigin::StartupContinuation(
            BgStartupContinuationSlice::VisibleTile3,
        ),
        ..BgFetcherState::default()
    };
    let effects = PpuMode3LiveBackgroundWriteEffects::for_current_background_fetch(
        fetcher,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
    );

    let mut cached = BgCachedSlice::default();
    effects.apply_to_cached_slice(&mut cached);
    assert!(cached.needs_live_tilemap_refetch);

    let mut fetcher = fetcher;
    effects.apply_to_fetcher(&mut fetcher);
    assert!(fetcher.needs_live_tilemap_refetch_on_push);
}

#[test]
fn cached_background_fill_recomputes_tilemap_before_the_next_flush_when_same_tcycle_window_is_open()
{
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_window_tilemap_entry(1, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 0;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;
    ppu.bg_pipeline_state
        .fill
        .cached
        .same_cycle_live_tilemap_refetch_window_open = true;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.fill.cached.needs_live_tilemap_refetch);

    ppu.maybe_recompute_pending_background_fill_with_ppu_vram();
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_map_address, 0x1C01);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_index, 1);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_data_address, 0x0011);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0xCD);
}

#[test]
fn third_startup_continuation_fill_marks_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fill.cached.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;

    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.fill.cached.needs_live_tilemap_refetch);
}

#[test]
fn ordinary_cached_fill_ignores_lcdc3_write_without_the_narrow_live_window() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1803;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;

    ppu.write_register(0xFF40, 0x99);
    assert!(!ppu.bg_pipeline_state.fill.cached.needs_live_tilemap_refetch);
}

#[test]
fn queue_from_push_preserves_the_same_tcycle_tilemap_refetch_window() {
    let mut fill = BgFifoFillState::default();
    let mut push = BgPushState {
        pending: true,
        ..BgPushState::default()
    };
    push.cached.source = PpuBgFetcherSource::Background;
    push.cached.same_cycle_live_tilemap_refetch_window_open = true;

    fill.queue_from_push(push);

    assert!(fill.cached.same_cycle_live_tilemap_refetch_window_open);
}

#[test]
fn queued_fill_from_real_push_preserves_the_same_tcycle_tilemap_refetch_window() {
    let mut ppu = dmg_fetch_rig();

    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state
        .push
        .cached
        .same_cycle_live_tilemap_refetch_window_open = true;

    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::QueuedFill);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert!(
        ppu.bg_pipeline_state
            .fill
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
}

#[test]
fn cached_background_fill_keeps_fetched_tiledata_after_scy_write_before_flush() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(0, 1, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 0;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF42, 0x01);
    assert!(
        !ppu.bg_pipeline_state
            .fill
            .cached
            .needs_live_tile_data_refetch
    );
    assert!(
        !ppu.bg_pipeline_state
            .fill
            .cached
            .needs_live_tile_data_current_row_refetch
    );

    ppu.maybe_recompute_pending_background_fill_with_ppu_vram();
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_data_address, 0x0001);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0x34);
}

#[test]
fn cached_background_push_keeps_fetched_tiledata_after_scy_write_before_flush() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(0, 1, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF42, 0x01);
    assert!(
        !ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_refetch
    );
    assert!(
        !ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_current_row_refetch
    );

    ppu.with_ppu_vram(|ppu, vram| ppu.maybe_recompute_pending_background_push(vram));
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_data_address, 0x0001);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_high, 0x34);
}

#[test]
fn bg_fetcher_rereads_the_unsigned_tile_data_byte_when_tile_selector_flips_to_unsigned_on_low1() {
    let mut ppu = dmg_fetch_rig();
    ppu.vram_bytes[0x1010] = 0x12;
    ppu.vram_bytes[0x0010] = 0x56;
    ppu.visible_registers.lcdc = 0x81;
    ppu.pipeline_registers.lcdc = 0x81;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1010);

    ppu.pipeline_registers.lcdc = 0x81;
    ppu.visible_registers.lcdc = 0x91;
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x56);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0010);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
}
