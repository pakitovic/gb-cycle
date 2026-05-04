use super::super::*;

const CGB_TEST_VRAM_BYTES: usize = 0x4000;
const VRAM_BANK_SIZE: usize = 0x2000;

fn with_cgb_vram_view<T>(
    bytes: [u8; CGB_TEST_VRAM_BYTES],
    f: impl FnOnce(&VramBusView<'_>) -> T,
) -> T {
    let mut vram = crate::bus::VramDomain::from_bytes_for_model(ConsoleModel::GameBoyColor, &bytes);
    vram.set_acquired(BusMaster::Ppu, true);
    f(&VramBusView::new(BusMaster::Ppu, &mut vram))
}

fn cgb_vram_domain(bytes: [u8; CGB_TEST_VRAM_BYTES]) -> crate::bus::VramDomain {
    let mut vram = crate::bus::VramDomain::from_bytes_for_model(ConsoleModel::GameBoyColor, &bytes);
    vram.set_acquired(BusMaster::Ppu, true);
    vram
}

fn write_cgb_vram_bank(vram: &mut crate::bus::VramDomain, bank: u8, offset: usize, value: u8) {
    vram.write_vbk(bank);
    vram.write(offset, value);
}

fn advance_bg_fetcher_with_cgb_vram(ppu: &mut Ppu, vram: &mut crate::bus::VramDomain) -> bool {
    ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, vram))
}

fn cgb_bg_fetch_ppu() -> Ppu {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    let registers = PpuVisibleRegisters {
        lcdc: LCDC_BG_ENABLE_BIT | LCDC_BG_WINDOW_TILE_DATA_BIT,
        bgp: 0xE4,
        ..PpuVisibleRegisters::default()
    };
    ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(registers));
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.start_background();
    ppu
}

fn lcdc4_write_context(previous_lcdc: u8, current_lcdc: u8) -> PpuMode3LiveRegisterWriteContext {
    PpuMode3LiveRegisterWriteContext::new(
        PpuVisibleRegisters {
            lcdc: previous_lcdc,
            ..PpuVisibleRegisters::default()
        },
        PpuVisibleRegisters {
            lcdc: current_lcdc,
            ..PpuVisibleRegisters::default()
        },
    )
}

#[test]
fn cgb_bg_tile_attribute_byte_decodes_all_hardware_fields() {
    let attrs = CgbBgTileAttributes::new(0xFF);

    assert_eq!(attrs.raw(), 0xFF);
    assert_eq!(attrs.palette_index(), 7);
    assert_eq!(attrs.tile_vram_bank(), 1);
    assert!(attrs.ignored_bit4());
    assert!(attrs.horizontal_flip());
    assert!(attrs.vertical_flip());
    assert!(attrs.bg_priority());

    let cached = BgCachedSlice {
        cgb_bg_attrs: Some(attrs),
        ..BgCachedSlice::default()
    };
    assert_eq!(
        cached.cgb_bg_attrs.map(CgbBgTileAttributes::tile_vram_bank),
        Some(1)
    );
}

#[test]
fn cgb_lcdc4_same_cycle_set_glitch_substitutes_tile_index_for_high_plane_push() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.bg_pipeline_state.fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Background,
        stage: PpuBgFetcherStage::Push,
        stage_dot: 0,
        fetch_x: 80,
        tile_index: 0x55,
        tile_low: 0x7F,
        tile_high: 0x5D,
        ..BgFetcherState::default()
    };
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        fetch_x: 80,
        tile_index: 0x55,
        tile_low: 0x7F,
        tile_high: 0x5D,
        needs_live_tile_data_refetch: true,
        needs_live_tile_data_current_row_refetch: true,
        ..BgCachedSlice::default()
    };

    ppu.apply_cgb_lcdc4_same_cycle_tiledata_glitch(lcdc4_write_context(
        LCDC_ENABLE_BIT | LCDC_BG_ENABLE_BIT,
        LCDC_ENABLE_BIT | LCDC_BG_ENABLE_BIT | LCDC_BG_WINDOW_TILE_DATA_BIT,
    ));

    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x55);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_high, 0x55);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_refetch
    );
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_current_row_refetch
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .cgb_lcdc4_same_cycle_tile_high_override,
        Some(0x55)
    );
}

#[test]
fn cgb_lcdc4_same_cycle_push_glitch_preserves_independent_refetch() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.lcdc = LCDC_ENABLE_BIT | LCDC_BG_ENABLE_BIT | LCDC_BG_WINDOW_TILE_DATA_BIT;
    ppu.ly = 0;
    ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        fetch_x: 80,
        tile_index: 0x55,
        tile_low: 0x7F,
        tile_high: 0x55,
        tile_low_address: 0x0550,
        tile_high_address: 0x0551,
        needs_live_tile_data_refetch: true,
        needs_live_tile_data_current_row_refetch: true,
        cgb_lcdc4_same_cycle_tile_high_override: Some(0x55),
        ..BgCachedSlice::default()
    };
    let mut bytes = [0; CGB_TEST_VRAM_BYTES];
    bytes[0x0550] = 0xA6;
    bytes[0x0551] = 0x3C;

    let recomputed = with_cgb_vram_view(bytes, |vram| {
        recompute_live_background_cached_slice(
            ppu.bg_pipeline_state.push.cached,
            vram,
            ppu.current_mode3_live_background_refetch_context(),
        )
    })
    .expect("pending independent refetch should recompute");

    assert_eq!(recomputed.tile_low, 0xA6);
    assert_eq!(recomputed.tile_high, 0x55);
    assert!(!recomputed.needs_live_tile_data_refetch);
    assert!(!recomputed.needs_live_tile_data_current_row_refetch);
    assert_eq!(recomputed.cgb_lcdc4_same_cycle_tile_high_override, None);
}

#[test]
fn cgb_lcdc4_same_cycle_reset_glitch_substitutes_tile_index_for_low_plane_fetch() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.bg_pipeline_state.fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Background,
        stage: PpuBgFetcherStage::TileDataLow,
        stage_dot: 1,
        tile_index: 0xA6,
        tile_low: 0x12,
        ..BgFetcherState::default()
    };

    ppu.apply_cgb_lcdc4_same_cycle_tiledata_glitch(lcdc4_write_context(
        LCDC_ENABLE_BIT | LCDC_BG_ENABLE_BIT | LCDC_BG_WINDOW_TILE_DATA_BIT,
        LCDC_ENABLE_BIT | LCDC_BG_ENABLE_BIT,
    ));

    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xA6);
}

#[test]
fn cgb_bg_fetcher_latches_bg_attributes_from_vram_bank1() {
    let mut ppu = cgb_bg_fetch_ppu();
    let mut vram = [0; CGB_TEST_VRAM_BYTES];
    vram[0x1800] = 0x02;
    vram[VRAM_BANK_SIZE + 0x1800] = 0xB5;

    with_cgb_vram_view(vram, |vram| {
        assert!(!ppu.advance_bg_fetcher(vram));
    });

    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1800);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x02);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.cgb_bg_attrs,
        Some(CgbBgTileAttributes::new(0xB5))
    );
}

#[test]
fn cgb_bg_fetcher_samples_tile_number_and_attrs_on_tile_index_dot0() {
    let mut ppu = cgb_bg_fetch_ppu();
    let latched_attrs = CgbBgTileAttributes::new(
        0x05 | CGB_BG_ATTR_VRAM_BANK_BIT
            | CGB_BG_ATTR_X_FLIP_BIT
            | CGB_BG_ATTR_Y_FLIP_BIT
            | CGB_BG_ATTR_PRIORITY_BIT,
    );
    let rewritten_attrs = CgbBgTileAttributes::new(0x02);
    let mut bytes = [0; CGB_TEST_VRAM_BYTES];
    bytes[0x1800] = 0x02;
    bytes[VRAM_BANK_SIZE + 0x1800] = latched_attrs.raw();
    bytes[VRAM_BANK_SIZE + 0x20 + 7 * 2] = 0x55;
    bytes[VRAM_BANK_SIZE + 0x20 + 7 * 2 + 1] = 0x33;

    let mut vram = cgb_vram_domain(bytes);
    assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));

    write_cgb_vram_bank(&mut vram, 0, 0x1800, 0x03);
    write_cgb_vram_bank(&mut vram, 1, 0x1800, rewritten_attrs.raw());
    write_cgb_vram_bank(&mut vram, 0, 0x30, 0xAA);
    write_cgb_vram_bank(&mut vram, 0, 0x31, 0xCC);
    vram.write_vbk(0);

    for _ in 0..5 {
        assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    }

    let cached = ppu.bg_pipeline_state.push.cached;
    assert_eq!(cached.tile_map_address, 0x1800);
    assert_eq!(cached.tile_index, 0x02);
    assert_eq!(cached.cgb_bg_attrs, Some(latched_attrs));
    assert_eq!(cached.tile_low_address, 0x20 + 7 * 2);
    assert_eq!(cached.tile_high_address, 0x20 + 7 * 2 + 1);
    assert_eq!(cached.tile_low, 0x55);
    assert_eq!(cached.tile_high, 0x33);
}

#[test]
fn cgb_mid_scanline_attribute_write_reaches_next_fetch_not_current_slice() {
    let mut ppu = cgb_bg_fetch_ppu();
    let current_attrs = CgbBgTileAttributes::new(0x03 | CGB_BG_ATTR_VRAM_BANK_BIT);
    let rewritten_current_attrs = CgbBgTileAttributes::new(0x06 | CGB_BG_ATTR_PRIORITY_BIT);
    let next_attrs = CgbBgTileAttributes::new(
        0x04 | CGB_BG_ATTR_VRAM_BANK_BIT | CGB_BG_ATTR_X_FLIP_BIT | CGB_BG_ATTR_PRIORITY_BIT,
    );
    let mut bytes = [0; CGB_TEST_VRAM_BYTES];
    bytes[0x1800] = 0x02;
    bytes[0x1801] = 0x04;
    bytes[VRAM_BANK_SIZE + 0x1800] = current_attrs.raw();
    bytes[VRAM_BANK_SIZE + 0x1801] = 0x00;

    let mut vram = cgb_vram_domain(bytes);
    assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));

    write_cgb_vram_bank(&mut vram, 1, 0x1800, rewritten_current_attrs.raw());
    write_cgb_vram_bank(&mut vram, 1, 0x1801, next_attrs.raw());
    vram.write_vbk(0);

    for _ in 0..5 {
        assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    }
    let first_cached = ppu.bg_pipeline_state.push.cached;
    assert_eq!(first_cached.tile_index, 0x02);
    assert_eq!(first_cached.cgb_bg_attrs, Some(current_attrs));

    assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.fetch_x, 8);

    assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1801);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x04);
    assert_eq!(ppu.bg_pipeline_state.fetcher.cgb_bg_attrs, Some(next_attrs));
}

#[test]
fn cgb_cached_bg_pixels_keep_latched_attrs_after_vram_bank1_changes() {
    let mut ppu = cgb_bg_fetch_ppu();
    let latched_attrs = CgbBgTileAttributes::new(
        0x07 | CGB_BG_ATTR_X_FLIP_BIT | CGB_BG_ATTR_Y_FLIP_BIT | CGB_BG_ATTR_PRIORITY_BIT,
    );
    let mut bytes = [0; CGB_TEST_VRAM_BYTES];
    bytes[0x1800] = 0x01;
    bytes[VRAM_BANK_SIZE + 0x1800] = latched_attrs.raw();
    bytes[0x10 + 7 * 2] = 0x55;
    bytes[0x10 + 7 * 2 + 1] = 0x33;

    let mut vram = cgb_vram_domain(bytes);
    for _ in 0..6 {
        assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    }
    let cached = ppu.bg_pipeline_state.push.cached;

    write_cgb_vram_bank(&mut vram, 1, 0x1800, 0x00);
    write_cgb_vram_bank(&mut vram, 0, 0x10 + 7 * 2, 0xFF);
    write_cgb_vram_bank(&mut vram, 0, 0x10 + 7 * 2 + 1, 0x00);

    ppu.bg_pipeline_state.fifo.clear();
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let colors = ppu
        .bg_pipeline_state
        .fifo
        .iter()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(colors, vec![3, 2, 1, 0, 3, 2, 1, 0]);
    for cached_pixel in ppu.bg_pipeline_state.fifo.cached_pixels() {
        assert_eq!(cached_pixel.cached.cgb_bg_attrs, Some(latched_attrs));
    }
}

#[test]
fn cgb_window_restart_latches_window_attrs_instead_of_stale_bg_attrs() {
    let mut ppu = cgb_bg_fetch_ppu();
    let registers = PpuVisibleRegisters {
        lcdc: LCDC_BG_ENABLE_BIT
            | LCDC_WINDOW_ENABLE_BIT
            | LCDC_WINDOW_TILE_MAP_BIT
            | LCDC_BG_WINDOW_TILE_DATA_BIT,
        bgp: 0xE4,
        ..PpuVisibleRegisters::default()
    };
    ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(registers));
    let stale_bg_attrs = CgbBgTileAttributes::new(0x01 | CGB_BG_ATTR_PRIORITY_BIT);
    let window_attrs = CgbBgTileAttributes::new(0x02 | CGB_BG_ATTR_VRAM_BANK_BIT);
    ppu.bg_pipeline_state.fetcher.cgb_bg_attrs = Some(stale_bg_attrs);
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.start_window_fetcher_restart();

    let mut bytes = [0; CGB_TEST_VRAM_BYTES];
    bytes[0x1800] = 0x01;
    bytes[VRAM_BANK_SIZE + 0x1800] = stale_bg_attrs.raw();
    bytes[0x1C00] = 0x04;
    bytes[VRAM_BANK_SIZE + 0x1C00] = window_attrs.raw();
    bytes[VRAM_BANK_SIZE + 0x40] = 0xA5;
    bytes[0x40] = 0x5A;

    let mut vram = cgb_vram_domain(bytes);
    assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );

    assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C00);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x04);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.cgb_bg_attrs,
        Some(window_attrs)
    );

    assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low_address, 0x40);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xA5);
}

#[test]
fn cgb_window_fetch_uses_y_flip_attrs_for_both_tile_data_planes() {
    let mut ppu = cgb_bg_fetch_ppu();
    let registers = PpuVisibleRegisters {
        lcdc: LCDC_BG_ENABLE_BIT
            | LCDC_WINDOW_ENABLE_BIT
            | LCDC_WINDOW_TILE_MAP_BIT
            | LCDC_BG_WINDOW_TILE_DATA_BIT,
        bgp: 0xE4,
        ..PpuVisibleRegisters::default()
    };
    ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(registers));
    ppu.window_state.window_line_counter = 8;
    ppu.runtime.bg_pipeline_state.window_active_line_counter = 8;
    ppu.start_window_fetcher_restart();

    let attrs = CgbBgTileAttributes::new(CGB_BG_ATTR_VRAM_BANK_BIT | CGB_BG_ATTR_Y_FLIP_BIT);
    let mut bytes = [0; CGB_TEST_VRAM_BYTES];
    bytes[0x1C20] = 0x04;
    bytes[VRAM_BANK_SIZE + 0x1C20] = attrs.raw();
    bytes[VRAM_BANK_SIZE + 0x40] = 0x11;
    bytes[VRAM_BANK_SIZE + 0x41] = 0x22;
    bytes[VRAM_BANK_SIZE + 0x40 + 7 * 2] = 0xA5;
    bytes[VRAM_BANK_SIZE + 0x40 + 7 * 2 + 1] = 0x5A;

    let mut vram = cgb_vram_domain(bytes);
    for _ in 0..7 {
        assert!(!advance_bg_fetcher_with_cgb_vram(&mut ppu, &mut vram));
    }

    let cached = ppu.bg_pipeline_state.push.cached;
    assert_eq!(cached.tile_index, 0x04);
    assert_eq!(cached.cgb_bg_attrs, Some(attrs));
    assert_eq!(cached.tile_low_address, 0x40 + 7 * 2);
    assert_eq!(cached.tile_high_address, 0x40 + 7 * 2 + 1);
    assert_eq!(cached.tile_low, 0xA5);
    assert_eq!(cached.tile_high, 0x5A);
}

#[test]
fn cgb_bg_fetcher_uses_attribute_tile_bank_and_flips_before_rgb555_rendering() {
    let mut ppu = cgb_bg_fetch_ppu();
    let mut vram = [0; CGB_TEST_VRAM_BYTES];
    let attrs = CgbBgTileAttributes::new(
        0x05 | CGB_BG_ATTR_VRAM_BANK_BIT
            | CGB_BG_ATTR_IGNORED_BIT
            | CGB_BG_ATTR_X_FLIP_BIT
            | CGB_BG_ATTR_Y_FLIP_BIT
            | CGB_BG_ATTR_PRIORITY_BIT,
    );

    vram[0x1800] = 0x01;
    vram[VRAM_BANK_SIZE + 0x1800] = attrs.raw();
    vram[0x10 + 7 * 2] = 0xFF;
    vram[0x10 + 7 * 2 + 1] = 0xFF;
    vram[VRAM_BANK_SIZE + 0x10] = 0x00;
    vram[VRAM_BANK_SIZE + 0x10 + 1] = 0x00;
    vram[VRAM_BANK_SIZE + 0x10 + 7 * 2] = 0x55;
    vram[VRAM_BANK_SIZE + 0x10 + 7 * 2 + 1] = 0x33;

    with_cgb_vram_view(vram, |vram| {
        for _ in 0..6 {
            assert!(!ppu.advance_bg_fetcher(vram));
        }
    });

    let cached = ppu.bg_pipeline_state.push.cached;
    assert_eq!(cached.tile_index, 0x01);
    assert_eq!(cached.cgb_bg_attrs, Some(attrs));
    assert_eq!(
        cached.cgb_bg_attrs.map(CgbBgTileAttributes::tile_vram_bank),
        Some(1)
    );
    assert_eq!(cached.tile_low_address, 0x10 + 7 * 2);
    assert_eq!(cached.tile_high_address, 0x10 + 7 * 2 + 1);
    assert_eq!(cached.tile_low, 0x55);
    assert_eq!(cached.tile_high, 0x33);

    ppu.bg_pipeline_state.fifo.clear();
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);
    let colors = ppu
        .bg_pipeline_state
        .fifo
        .iter()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(colors, vec![3, 2, 1, 0, 3, 2, 1, 0]);
}
