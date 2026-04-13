use super::*;

fn mode2_scan_rig(model: ConsoleModel) -> PpuTestRig {
    PpuTestRig::with_model(model).with_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    })
}

fn dmg_mode2_scan_rig() -> PpuTestRig {
    mode2_scan_rig(ConsoleModel::Dmg)
}

#[test]
fn mode2_scans_oam_in_order_and_caps_the_selected_list_at_ten_entries() {
    let mut oam_bytes = [0; 160];

    for index in 0..12 {
        let x = match index {
            0 => 0,
            1 => 168,
            _ => 8 + index,
        };
        write_oam_entry(&mut oam_bytes, index, 16, x, 0x20 + index);
    }
    write_oam_entry(&mut oam_bytes, 20, 8, 32, 0x99);

    let mut ppu = dmg_mode2_scan_rig().with_oam(oam_bytes);
    ppu.tick_n(80);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 40);
    assert_eq!(snapshot.selected_sprites.len(), 10);
    assert_eq!(
        snapshot
            .selected_sprites
            .iter()
            .map(|sprite| sprite.oam_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    assert_eq!(snapshot.selected_sprites[0].x, 0);
    assert_eq!(snapshot.selected_sprites[1].x, 168);
}

#[test]
fn dmg_mode2_oam_dma_reuses_the_last_latched_mode2_yx_word_for_selection() {
    let mut ppu = dmg_mode2_scan_rig();
    ppu.write_oam_entry(0, 16, 24, 0x20);
    ppu.write_oam_entry(1, 0, 0, 0x21);
    ppu.tick_n(2);
    ppu.set_dma_active(true);
    ppu.tick_n(2);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 2);
    assert_eq!(snapshot.selected_sprites.len(), 2);
    assert_eq!(snapshot.selected_sprites[1].oam_index, 1);
    assert_eq!(snapshot.selected_sprites[1].y, 16);
    assert_eq!(snapshot.selected_sprites[1].x, 24);
}

#[test]
fn mode2_scanline_reset_preserves_the_latched_mode2_yx_word_for_dma_blocked_reads() {
    let mut ppu = dmg_mode2_scan_rig()
        .with_dma_active(true)
        .with_dma_conflict(None);
    ppu.write_oam_entry(0, 0, 0, 0x20);
    ppu.mode2_scan_state.latch_mode2_yx_word(16, 79);
    ppu.mode2_scan_state.reset_scanline();
    ppu.tick_n(2);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 1);
    assert_eq!(snapshot.selected_sprites.len(), 1);
    assert_eq!(snapshot.selected_sprites[0].oam_index, 0);
    assert_eq!(snapshot.selected_sprites[0].y, 16);
    assert_eq!(snapshot.selected_sprites[0].x, 79);
}

#[test]
fn late_obj_metadata_fetch_does_not_poison_the_mode2_dma_yx_latch() {
    let mut ppu = dmg_mode2_scan_rig();
    ppu.write_oam_entry_with_attributes(0, 0, 0, 0xA5, 0x5A);
    ppu.mode2_scan_state.latch_mode2_yx_word(16, 79);

    let mut oam = crate::bus::OamDomain::from_bytes(&ppu.oam_bytes);
    oam.set_acquired(BusMaster::Ppu, true);
    let resolved = ppu.resolve_obj_fetch_sprite(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        PpuSelectedSprite {
            oam_index: 0,
            y: 0,
            x: 0,
            tile_index: 0,
            attributes: 0,
        },
        None,
    );

    assert_eq!(resolved.tile_index, 0xA5);
    assert_eq!(resolved.attributes, 0x5A);
    assert_eq!(ppu.mode2_scan_state.latched_mode2_yx_word(), Some((16, 79)));
    assert_eq!(
        ppu.obj_pipeline_state.late_metadata_word,
        Some((0xA5, 0x5A))
    );

    ppu.set_dma_active(true);
    ppu.tick_n(2);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 1);
    assert_eq!(snapshot.selected_sprites.len(), 1);
    assert_eq!(snapshot.selected_sprites[0].oam_index, 0);
    assert_eq!(snapshot.selected_sprites[0].y, 16);
    assert_eq!(snapshot.selected_sprites[0].x, 79);
}

#[test]
fn resetting_the_obj_pipeline_clears_the_separate_late_metadata_word() {
    let mut ppu = PpuTestRig::dmg();
    ppu.obj_pipeline_state.late_metadata_word = Some((0x12, 0x34));

    ppu.obj_pipeline_state.reset();

    assert_eq!(ppu.obj_pipeline_state.late_metadata_word, None);
}

#[test]
fn mode2_uses_the_live_lcdc2_size_when_each_oam_entry_is_scanned() {
    let mut ppu = dmg_mode2_scan_rig();
    ppu.write_oam_entry(0, 0, 24, 0x10);
    ppu.write_oam_entry(1, 1, 32, 0x11);
    ppu.tick_n(2);
    assert!(ppu.snapshot().selected_sprites.is_empty());

    ppu.write_register(0xFF40, 0x84);
    ppu.tick_n(2);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 2);
    assert_eq!(snapshot.selected_sprites.len(), 1);
    assert_eq!(snapshot.selected_sprites[0].oam_index, 1);
    assert_eq!(snapshot.selected_sprites[0].y, 1);
}

#[test]
fn current_mode2_oam_row_tracks_the_live_four_dot_slices() {
    let mut ppu = dmg_mode2_scan_rig();

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(0));

    ppu.tick_n(5);

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(1));

    ppu.tick_n(76);

    let drawing = ppu.snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(drawing.current_oam_scan_row, None);
}

#[test]
fn first_oam_row_is_immune_to_basic_read_and_write_corruption_patterns() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut read_oam = [0; 160];
    let mut write_oam = [0; 160];

    write_oam_corruption_row(&mut read_oam, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
    write_oam_corruption_row(&mut write_oam, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
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

    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Read, &mut read_oam));
    assert_eq!(
        &read_oam[..8],
        &[0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB]
    );

    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Write, &mut write_oam));
    assert_eq!(
        &write_oam[..8],
        &[0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB]
    );
}

#[test]
fn write_corruption_uses_the_documented_first_word_formula_and_previous_row_tail() {
    let mut ppu = dmg_mode2_scan_rig();
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
    write_oam_corruption_row(&mut oam_bytes, 1, [0x0F0F, 0x1111, 0x2222, 0x3333]);
    ppu.tick_n(5);

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(1));
    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Write, &mut oam_bytes));

    let expected_first = ((0x0F0F_u16 ^ 0xAAAA) & (0x1357 ^ 0xAAAA)) ^ 0xAAAA;
    assert_eq!(read_oam_word(&oam_bytes, 1, 0), expected_first);
    assert_eq!(read_oam_word(&oam_bytes, 1, 1), 0x2468);
    assert_eq!(read_oam_word(&oam_bytes, 1, 2), 0xAAAA);
    assert_eq!(read_oam_word(&oam_bytes, 1, 3), 0xBBBB);
}

#[test]
fn read_corruption_uses_the_documented_first_word_formula_and_previous_row_tail() {
    let mut ppu = dmg_mode2_scan_rig();
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
    write_oam_corruption_row(&mut oam_bytes, 1, [0x0F0F, 0x1111, 0x2222, 0x3333]);
    ppu.tick_n(5);

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(1));
    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Read, &mut oam_bytes));

    let expected_first = 0x1357_u16 | (0x0F0F & 0xAAAA);
    assert_eq!(read_oam_word(&oam_bytes, 1, 0), expected_first);
    assert_eq!(read_oam_word(&oam_bytes, 1, 1), 0x2468);
    assert_eq!(read_oam_word(&oam_bytes, 1, 2), 0xAAAA);
    assert_eq!(read_oam_word(&oam_bytes, 1, 3), 0xBBBB);
}

#[test]
fn read_plus_incdec_uses_its_dedicated_complex_path_in_rows_four_through_eighteen() {
    let mut ppu = dmg_mode2_scan_rig();
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 2, [0x0F0F, 0x1212, 0x3434, 0x5656]);
    write_oam_corruption_row(&mut oam_bytes, 3, [0xAAAA, 0x1111, 0xC0C0, 0x2222]);
    write_oam_corruption_row(&mut oam_bytes, 4, [0x00FF, 0x3333, 0x4444, 0x5555]);
    ppu.tick_n(17);

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(4));
    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::ReadWithIncDec, &mut oam_bytes));

    let expected_previous_first = 0xAAAA_u16 & (0x0F0F | 0x00FF | 0xC0C0);
    let expected_row = [expected_previous_first, 0x1111, 0xC0C0, 0x2222];

    for (word_index, expected) in expected_row.into_iter().enumerate() {
        assert_eq!(read_oam_word(&oam_bytes, 2, word_index), expected);
        assert_eq!(read_oam_word(&oam_bytes, 4, word_index), expected);
    }
    assert_eq!(read_oam_word(&oam_bytes, 3, 0), expected_previous_first);
    assert_eq!(read_oam_word(&oam_bytes, 3, 1), 0x1111);
    assert_eq!(read_oam_word(&oam_bytes, 3, 2), 0xC0C0);
    assert_eq!(read_oam_word(&oam_bytes, 3, 3), 0x2222);
}

#[test]
fn read_plus_incdec_on_the_last_row_falls_back_to_ordinary_read_corruption() {
    let mut ppu = dmg_mode2_scan_rig();
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 18, [0x1234, 0x1111, 0x00FF, 0x2222]);
    write_oam_corruption_row(&mut oam_bytes, 19, [0x0F0F, 0xAAAA, 0xBBBB, 0xCCCC]);
    ppu.tick_n(77);

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(19));
    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::ReadWithIncDec, &mut oam_bytes));

    let expected_first = 0x1234_u16 | (0x0F0F & 0x00FF);
    assert_eq!(read_oam_word(&oam_bytes, 19, 0), expected_first);
    assert_eq!(read_oam_word(&oam_bytes, 19, 1), 0x1111);
    assert_eq!(read_oam_word(&oam_bytes, 19, 2), 0x00FF);
    assert_eq!(read_oam_word(&oam_bytes, 19, 3), 0x2222);
    assert_eq!(read_oam_word(&oam_bytes, 17, 0), 0x0000);
}

#[test]
fn cgb_models_do_not_apply_dmg_family_oam_corruption() {
    let mut ppu = mode2_scan_rig(ConsoleModel::Cgb);
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
    write_oam_corruption_row(&mut oam_bytes, 1, [0x0F0F, 0x1111, 0x2222, 0x3333]);
    ppu.tick_n(4);

    let before = oam_bytes;
    assert!(!ppu.apply_oam_corruption_event(OamCorruptionEventKind::Write, &mut oam_bytes));
    assert_eq!(oam_bytes, before);
}
