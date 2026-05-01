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
