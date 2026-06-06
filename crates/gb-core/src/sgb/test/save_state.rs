use super::support::*;

#[test]
fn save_state_restores_sgb_palette_state() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);

    assert_eq!(
        restored.snapshot().video.palette_state,
        host.snapshot().video.palette_state
    );
    assert_eq!(
        restored.snapshot().video.backdrop_color,
        host.snapshot().video.backdrop_color
    );
    assert_eq!(restored.snapshot().video.palette_command_count, 1);
    assert_eq!(
        restored.snapshot().video.map_lcd_shade_to_rgb555(3).raw(),
        0x4210
    );
}

#[test]
fn save_state_restores_sgb_player_palette_override_state() {
    let mut host = accepted_sgb_host();
    let player_palette = sgb_screen_palette([0x1111, 0x2222, 0x3333, 0x4444]);
    assert!(host.set_player_palette_override(player_palette));

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);

    assert_eq!(
        restored.snapshot().video.player_palette_override,
        host.snapshot().video.player_palette_override
    );
    assert_eq!(
        restored.snapshot().video.map_lcd_shade_to_rgb555(3).raw(),
        0x4444
    );
}

#[test]
fn save_state_restores_sgb_boot_title_palette_seed() {
    let mut host = SgbHost::new(HostPlatform::Sgb);
    let header = test_header_with_title(
        b"MARIOLAND2",
        SgbFlag::None,
        SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
    );
    host.apply_cartridge_header(Some(&header));

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);

    assert_eq!(
        restored.snapshot().video.palette_state,
        host.snapshot().video.palette_state
    );
    assert_eq!(
        restored.snapshot().video.map_lcd_shade_to_rgb555(0).raw(),
        0x5FFE
    );
    assert_eq!(
        restored.snapshot().video.map_lcd_shade_to_rgb555(3).raw(),
        0x0000
    );
}

#[test]
fn save_state_restores_sgb_transfer_border_and_mask_state() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());
    write_joyp_packet(&mut host, sgb_chr_trn_packet(0));
    host.capture_pending_vram_transfer(&solid_tile_color_1_transfer())
        .expect("CHR_TRN should load border tile data before save");
    let mut pct = [0; SGB_VRAM_TRANSFER_BYTES];
    write_border_map_entry(&mut pct, 0, 4 << 10);
    write_border_palette_color(&mut pct, 0, 0, 0x001F);
    write_border_palette_color(&mut pct, 1, 0, 0x001F);
    write_border_palette_color(&mut pct, 2, 0, 0x001F);
    write_border_palette_color(&mut pct, 0, 1, 0x03E0);
    write_joyp_packet(&mut host, sgb_pct_trn_packet());
    host.capture_pending_vram_transfer(&pct)
        .expect("PCT_TRN should load border map before save");
    write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::Freeze));
    host.capture_pending_lcd_freeze(&vec![0; SGB_LCD_PIXELS])
        .expect("freeze should persist its captured LCD image");

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);

    assert_eq!(
        restored
            .snapshot()
            .video
            .vram_transfer
            .completed_transfer_count,
        2
    );
    assert!(restored.snapshot().video.border_loaded);
    assert_eq!(restored.snapshot().video.border.tile_data.bytes[0], 0xFF);
    assert_eq!(
        restored.snapshot().video.border.palettes[0].colors[1].raw(),
        0x03E0
    );
    assert_eq!(restored.snapshot().video.mask, SgbScreenMask::Freeze);
    assert!(restored.snapshot().video.frozen_lcd.is_some());
    assert_eq!(
        restored
            .compose_lcd_rgb555(&vec![3; SGB_LCD_PIXELS])
            .expect("restored frozen LCD should compose")[0],
        0x001F
    );
}

#[test]
fn save_state_restores_sgb_attribute_and_system_palette_state() {
    let mut host = accepted_sgb_host();
    let mut palettes = [0; SGB_VRAM_TRANSFER_BYTES];
    write_system_palette_color(&mut palettes, 3, 0, 0x001F);
    write_system_palette_color(&mut palettes, 3, 1, 0x03E0);
    write_joyp_packet(&mut host, sgb_pal_trn_packet());
    host.capture_pending_vram_transfer(&palettes)
        .expect("PAL_TRN should load system palette memory before save");
    let mut attributes = [0; SGB_VRAM_TRANSFER_BYTES];
    write_atf_cell(&mut attributes, 4, 0, 0, 1);
    write_atf_cell(&mut attributes, 4, 1, 0, 2);
    write_joyp_packet(&mut host, sgb_attr_trn_packet());
    host.capture_pending_vram_transfer(&attributes)
        .expect("ATTR_TRN should load ATF memory before save");

    let mut pal_set = sgb_command_packet(SGB_COMMAND_PAL_SET, 1);
    for palette_index in 0..SGB_SCREEN_PALETTE_COUNT {
        let [low, high] = 3u16.to_le_bytes();
        pal_set[1 + palette_index * 2] = low;
        pal_set[2 + palette_index * 2] = high;
    }
    pal_set[9] = 0x80 | 4;
    write_joyp_packet(&mut host, pal_set);

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);

    let snapshot = restored.snapshot();
    assert!(snapshot.video.system_palettes.loaded);
    assert!(snapshot.video.attributes.files.loaded);
    assert_eq!(snapshot.video.system_palettes.last_pal_set_ids, [3; 4]);
    assert_eq!(snapshot.video.attributes.last_atf_index, Some(4));
    assert_eq!(snapshot.video.attributes.map.palette_index(0, 0), 1);
    assert_eq!(snapshot.video.attributes.map.palette_index(1, 0), 2);
    assert_eq!(
        snapshot.video.palette_state.palette(0).colors[1].raw(),
        0x03E0
    );
}
