use super::*;

const CGB_TEST_VRAM_BYTES: usize = 0x4000;
const VRAM_BANK_SIZE: usize = 0x2000;

fn with_cgb_video_buses<T>(
    oam_bytes: [u8; 160],
    vram_bytes: [u8; CGB_TEST_VRAM_BYTES],
    f: impl FnOnce(&OamBusView<'_>, &VramBusView<'_>) -> T,
) -> T {
    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let mut vram =
        crate::bus::VramDomain::from_bytes_for_model(ConsoleModel::GameBoyColor, &vram_bytes);
    oam.set_acquired(BusMaster::Ppu, true);
    vram.set_acquired(BusMaster::Ppu, true);
    f(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    )
}

fn cgb_obj_fetch_ppu() -> Ppu {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    let registers = PpuVisibleRegisters {
        lcdc: LCDC_ENABLE_BIT | LCDC_OBJ_ENABLE_BIT,
        bgp: 0xE4,
        ..PpuVisibleRegisters::default()
    };
    ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(registers));
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu
}

fn priority_test_obj_pixel(color: u8, sprite_x: u8, oam_index: u8) -> ObjPixel {
    ObjPixel {
        color,
        palette_obp1: false,
        bg_over_obj: false,
        cgb_obj_attrs: Some(CgbObjAttributes::new(0)),
        sprite_x,
        oam_index,
    }
}

#[test]
fn cgb_obj_attribute_byte_decodes_all_hardware_fields() {
    let attrs = CgbObjAttributes::new(0xFF);

    assert_eq!(attrs.raw(), 0xFF);
    assert_eq!(attrs.palette_index(), 7);
    assert_eq!(attrs.tile_vram_bank(), 1);
    assert!(attrs.dmg_palette_obp1());
    assert!(attrs.horizontal_flip());
    assert!(attrs.vertical_flip());
    assert!(attrs.bg_over_obj());
}

#[test]
fn cgb_obj_priority_mode_prefers_oam_order_over_lower_x() {
    let ppu = Ppu::new(ConsoleModel::GameBoyColor);
    let current = priority_test_obj_pixel(1, 20, 0);
    let candidate = priority_test_obj_pixel(2, 18, 1);

    assert!(!ppu.obj_pixel_has_priority(candidate, current));
}

#[test]
fn cgb_compatibility_obj_priority_mode_prefers_lower_x_without_dmg_silicon() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(crate::model::OperatingMode::GbCompatible);
    let current = priority_test_obj_pixel(1, 20, 0);
    let candidate = priority_test_obj_pixel(2, 18, 1);

    assert_eq!(ppu.read_register(0xFF6C), 0xFF);
    assert!(ppu.obj_pixel_has_priority(candidate, current));
}

#[test]
fn cgb_dmg_ext_obj_priority_mode_prefers_lower_x_and_latches_opri_readback_only() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(crate::model::OperatingMode::CgbDmgExt);
    let current = priority_test_obj_pixel(1, 20, 0);
    let candidate = priority_test_obj_pixel(2, 18, 1);

    assert_eq!(ppu.read_register(0xFF6C), 0xFF);
    assert!(ppu.obj_pixel_has_priority(candidate, current));

    ppu.write_register(0xFF6C, 0x00);
    assert_eq!(ppu.read_register(0xFF6C), 0xFE);
    assert!(ppu.obj_pixel_has_priority(candidate, current));
}

#[test]
fn cgb_opri_write_updates_latch_without_runtime_visual_priority_mutation() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    let current = priority_test_obj_pixel(1, 20, 0);
    let candidate = priority_test_obj_pixel(2, 18, 1);

    assert_eq!(ppu.read_register(0xFF6C), 0xFE);
    ppu.write_register(0xFF6C, 0x01);

    assert_eq!(ppu.read_register(0xFF6C), 0xFF);
    assert!(!ppu.obj_pixel_has_priority(candidate, current));
}

#[test]
fn cgb_obj_fetch_uses_attribute_tile_bank_and_carries_palette_sideband() {
    let mut ppu = cgb_obj_fetch_ppu();
    let attrs = CgbObjAttributes::new(0x05 | CGB_OBJ_ATTR_VRAM_BANK_BIT);
    let sprite = selected_sprite(SelectedSpriteSpec::new(0, 16, 8, 0x02, attrs.raw()));
    let mut oam = [0; 160];
    let mut vram = [0; CGB_TEST_VRAM_BYTES];

    write_oam_entry_with_attributes(
        &mut oam,
        0,
        sprite.y,
        sprite.x,
        sprite.tile_index,
        attrs.raw(),
    );
    vram[0x20] = 0x00;
    vram[0x21] = 0x00;
    vram[VRAM_BANK_SIZE + 0x20] = 0x80;
    vram[VRAM_BANK_SIZE + 0x21] = 0x00;

    ppu.obj_pipeline_state.fetch = ObjFetchState {
        stage: PpuObjFetcherStage::TileDataLow,
        stage_dot: 1,
        sprite_slot: 0,
        sprite: Some(sprite),
        resolved_sprite: Some(sprite),
        selected_obj_height: 8,
        latched_obj_height: 8,
        resolved_tile_index: Some(sprite.tile_index),
        resolved_tile_row: Some(0),
        ..ObjFetchState::default()
    };

    with_cgb_video_buses(oam, vram, |oam, vram| {
        assert!(ppu.advance_object_fetch(oam, vram, None));
        assert_eq!(ppu.obj_pipeline_state.fetch.tile_low, 0x80);
        assert!(ppu.advance_object_fetch(oam, vram, None));
        assert!(ppu.advance_object_fetch(oam, vram, None));
        assert_eq!(ppu.obj_pipeline_state.fetch.tile_high, 0x00);
        assert!(ppu.advance_object_fetch(oam, vram, None));
        assert!(ppu.advance_object_fetch(oam, vram, None));
    });

    let pixel = ppu
        .obj_pipeline_state
        .fifo
        .front()
        .copied()
        .expect("CGB OBJ fetch should push one visible pixel");
    assert_eq!(pixel.color, 1);
    assert_eq!(pixel.cgb_obj_attrs, Some(attrs));
    assert_eq!(
        pixel.cgb_obj_attrs.map(CgbObjAttributes::palette_index),
        Some(5)
    );
    assert_eq!(
        pixel.cgb_obj_attrs.map(CgbObjAttributes::tile_vram_bank),
        Some(1)
    );
}

#[test]
fn dmg_obj_fetch_ignores_cgb_attribute_tile_bank_bit() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    let sprite = selected_sprite(SelectedSpriteSpec::new(
        0,
        16,
        8,
        0x02,
        CGB_OBJ_ATTR_VRAM_BANK_BIT,
    ));
    let mut vram = [0; CGB_TEST_VRAM_BYTES];
    vram[0x20] = 0x44;
    vram[VRAM_BANK_SIZE + 0x20] = 0x88;

    let byte = with_cgb_video_buses([0; 160], vram, |_oam, vram| {
        ppu.read_obj_tile_data_byte_for_resolved_tile(vram, sprite, 0x02, 0, 0)
    });

    assert_eq!(byte, 0x44);
    assert_eq!(ppu.obj_tile_data_vram_bank(sprite), 0);
    assert_eq!(ppu.cgb_obj_attributes(sprite), None);
}

#[test]
fn cgb_dmg_software_obj_fetch_ignores_native_cgb_attribute_tile_bank_bit() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
        ppu.apply_operating_mode_state(operating_mode);
        let sprite = selected_sprite(SelectedSpriteSpec::new(
            0,
            16,
            8,
            0x02,
            CGB_OBJ_ATTR_VRAM_BANK_BIT | 0x05,
        ));
        let mut vram = [0; CGB_TEST_VRAM_BYTES];
        vram[0x20] = 0x44;
        vram[VRAM_BANK_SIZE + 0x20] = 0x88;

        let byte = with_cgb_video_buses([0; 160], vram, |_oam, vram| {
            ppu.read_obj_tile_data_byte_for_resolved_tile(vram, sprite, 0x02, 0, 0)
        });

        assert_eq!(byte, 0x44);
        assert_eq!(ppu.obj_tile_data_vram_bank(sprite), 0);
        assert_eq!(ppu.cgb_obj_attributes(sprite), None);
    }
}

#[test]
fn cgb_obj_flips_apply_before_rgb555_rendering() {
    let mut ppu = cgb_obj_fetch_ppu();
    let attrs = CgbObjAttributes::new(
        0x03 | CGB_OBJ_ATTR_X_FLIP_BIT | CGB_OBJ_ATTR_Y_FLIP_BIT | CGB_OBJ_ATTR_BG_OVER_OBJ_BIT,
    );
    let sprite = selected_sprite(SelectedSpriteSpec::new(0, 16, 8, 0x10, attrs.raw()));

    ppu.ly = 0;
    assert_eq!(ppu.obj_tile_index_and_row(sprite), Some((0x10, 7)));

    ppu.push_obj_pixels(sprite, 0x80, 0x00, 0);
    let pixels = ppu
        .obj_pipeline_state
        .fifo
        .iter()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        pixels.iter().map(|pixel| pixel.color).collect::<Vec<_>>(),
        vec![0, 0, 0, 0, 0, 0, 0, 1]
    );
    assert_eq!(pixels[7].cgb_obj_attrs, Some(attrs));
    assert!(pixels[7].bg_over_obj);
}
