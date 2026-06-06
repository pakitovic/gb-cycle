use super::support::*;

#[test]
fn attribute_state_rejects_invalid_inputs_and_wraps_cells_explicitly() {
    let mut attributes = SgbAttributeState::default();

    attributes.apply_attr_blk(&[]);
    assert_eq!(attributes.attr_blk_count, 0);

    attributes.apply_attr_blk(&[2, 0x01, 0x01, 1, 1, 3, 3]);
    assert_eq!(attributes.attr_blk_count, 1);
    assert_eq!(attributes.map.palette_index(1, 1), 1);

    attributes.apply_attr_blk(&[2, 0x04, 0x20, 3, 3, 1, 1]);
    assert_eq!(attributes.attr_blk_count, 2);
    assert_eq!(attributes.map.palette_index(1, 1), 2);

    attributes.apply_attr_lin(&[]);
    assert_eq!(attributes.attr_lin_count, 0);
    attributes.apply_attr_lin(&[2, 1 << 5, 0x80 | (2 << 5) | 1]);
    assert_eq!(attributes.attr_lin_count, 1);
    assert_eq!(attributes.map.palette_index(0, 0), 1);
    assert_eq!(attributes.map.palette_index(0, 1), 2);

    let mut horizontal_div = sgb_command_packet(SGB_COMMAND_ATTR_DIV, 1);
    horizontal_div[1] = 0x40 | 3 | (1 << 2) | (2 << 4);
    horizontal_div[2] = 1;
    attributes.apply_attr_div(&horizontal_div);
    assert_eq!(attributes.map.palette_index(0, 0), 1);
    assert_eq!(attributes.map.palette_index(0, 1), 2);
    assert_eq!(attributes.map.palette_index(0, 2), 3);

    attributes.apply_attr_chr(&[0, 0, 0, 0]);
    let attr_chr_count = attributes.attr_chr_count;
    attributes.apply_attr_chr(&[SGB_ATTR_MAP_WIDTH as u8, 0, 1, 0, 0, 0xFF]);
    assert_eq!(attributes.attr_chr_count, attr_chr_count);
    attributes.apply_attr_chr(&[19, 17, 2, 0, 1, 0b01_10_00_00]);
    assert_eq!(attributes.attr_chr_count, attr_chr_count + 1);
    assert_eq!(attributes.map.palette_index(19, 17), 1);
    assert_eq!(attributes.map.palette_index(0, 0), 2);

    assert!(!attributes.apply_attr_set(SGB_ATF_COUNT as u8));
    assert_eq!(attributes.invalid_atf_count, 1);
    assert!(attributes.dynamic_payload_bytes() >= SGB_ATTR_MAP_CELLS + SGB_ATF_TOTAL_BYTES);
}

#[test]
fn video_composition_and_transfer_error_edges_are_reported() {
    let mut host = accepted_sgb_host();
    let lcd = vec![0; SGB_LCD_PIXELS];
    let mut short_lcd_output = vec![0; SGB_LCD_PIXELS - 1];
    assert_eq!(
        host.compose_lcd_rgb555_into(&lcd, &mut short_lcd_output),
        Err(SgbLcdCompositionError::OutputLength {
            expected: SGB_LCD_PIXELS,
            actual: SGB_LCD_PIXELS - 1,
        })
    );

    let handheld = SgbHost::new(HostPlatform::Handheld);
    assert_eq!(
        handheld.compose_frame_rgb555(&lcd),
        Err(SgbFrameCompositionError::DisabledHost)
    );
    assert_eq!(
        host.compose_frame_rgb555(&lcd[..SGB_LCD_PIXELS - 1]),
        Err(SgbFrameCompositionError::InputLength {
            expected: SGB_LCD_PIXELS,
            actual: SGB_LCD_PIXELS - 1,
        })
    );
    let mut short_frame_output = vec![0; SGB_FRAME_PIXELS - 1];
    assert_eq!(
        host.compose_frame_rgb555_into(&lcd, &mut short_frame_output),
        Err(SgbFrameCompositionError::OutputLength {
            expected: SGB_FRAME_PIXELS,
            actual: SGB_FRAME_PIXELS - 1,
        })
    );

    assert_eq!(
        handheld.clone().capture_pending_lcd_freeze(&lcd),
        Err(SgbLcdCompositionError::DisabledHost)
    );
    assert_eq!(
        host.capture_pending_lcd_freeze(&lcd[..SGB_LCD_PIXELS - 1]),
        Err(SgbLcdCompositionError::InputLength {
            expected: SGB_LCD_PIXELS,
            actual: SGB_LCD_PIXELS - 1,
        })
    );
    assert_eq!(host.capture_pending_lcd_freeze(&lcd), Ok(()));

    assert_eq!(
        SgbHost::new(HostPlatform::Handheld)
            .capture_pending_vram_transfer(&[0; SGB_VRAM_TRANSFER_BYTES]),
        Err(SgbVramTransferError::DisabledHost)
    );
    assert_eq!(
        host.capture_pending_vram_transfer(&[0; SGB_VRAM_TRANSFER_BYTES]),
        Err(SgbVramTransferError::NoPendingTransfer)
    );
    assert_eq!(
        SgbHost::new(HostPlatform::Handheld).advance_frame_start(
            &[0; SGB_GB_VRAM_BYTES],
            SgbVramTransferDisplayState::new(0x81, 0, 0, SGB_TRANSFER_REQUIRED_BGP),
        ),
        Err(SgbVramTransferError::DisabledHost)
    );
    assert_eq!(
        host.advance_frame_start(
            &[0; SGB_GB_VRAM_BYTES],
            SgbVramTransferDisplayState::new(0x81, 0, 0, SGB_TRANSFER_REQUIRED_BGP),
        ),
        Ok(None)
    );

    host.video.request_pal_transfer(SGB_COMMAND_PAL_TRN);
    host.video
        .vram_transfer
        .pending
        .as_mut()
        .expect("test should have a pending PAL_TRN")
        .frame_starts_until_capture = 2;
    assert_eq!(
        host.advance_frame_start(
            &[0; SGB_GB_VRAM_BYTES],
            SgbVramTransferDisplayState::new(0x81, 0, 0, SGB_TRANSFER_REQUIRED_BGP),
        ),
        Ok(None)
    );
    assert_eq!(
        host.snapshot()
            .video
            .vram_transfer
            .pending
            .expect("pending transfer should remain delayed")
            .frame_starts_until_capture,
        1
    );

    host.dispatch_completed_vram_transfer(None);
    host.dispatch_completed_vram_transfer(Some(SgbVramTransferTarget::Pal));
}

#[test]
fn masks_palette_override_and_border_flips_cover_visible_pixel_edges() {
    let mut video = SgbVideoState::default_for_active_host(true);
    video.mask = SgbScreenMask::Cancel;
    assert_eq!(
        video.lcd_pixel_for_shade(1),
        video.map_lcd_shade_to_rgb555(1)
    );
    video.mask = SgbScreenMask::Freeze;
    assert_eq!(
        video.lcd_pixel_for_shade(1),
        video.map_lcd_shade_to_rgb555(1)
    );
    video.mask = SgbScreenMask::BlankBlack;
    assert_eq!(video.lcd_pixel_for_shade(1), SGB_RGB555_BLACK);
    video.mask = SgbScreenMask::BlankColor0;
    assert_eq!(
        video.lcd_pixel_for_shade(1),
        video.visible_lcd_backdrop_color()
    );

    let mut override_state = SgbPlayerPaletteOverrideState::default();
    assert!(!override_state.clear_by_player());
    assert!(!override_state.return_to_application_due_to_pal_pri());
    let player_palette = sgb_screen_palette([0x1111, 0x2222, 0x3333, 0x4444]);
    assert!(override_state.set_uniform_palette(player_palette));
    assert!(!override_state.set_uniform_palette(player_palette));
    assert!(override_state.clear_by_player());

    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());
    host.video.mask = SgbScreenMask::Freeze;
    host.video.frozen_lcd = None;
    assert_eq!(
        host.compose_lcd_rgb555(&vec![1; SGB_LCD_PIXELS])
            .expect("freeze without a captured frame falls back to live LCD")[0],
        0x03E0
    );

    let mut border = SgbBorderState::default();
    border.tile_map.entries[0] = SgbBorderMapEntry::new(0xC000 | (7 << 10));
    border.tile_data.bytes[14] = 0x01;
    border.palettes[0].colors[1] = SgbRgb555Color::new(0x1234);
    assert_eq!(border.pixel_color(0, 0), (SgbRgb555Color::new(0x1234), 1));
    assert_eq!(
        border.dynamic_payload_bytes(),
        SGB_BORDER_TILE_DATA_BYTES
            + SGB_BORDER_TILEMAP_ENTRIES * std::mem::size_of::<SgbBorderMapEntry>()
    );
}

#[test]
fn vram_transfer_display_extraction_uses_prepared_layout_when_lcd_is_disabled() {
    let payload = frame_payload(3);
    let vram = signed_display_vram_from_payload(&payload);
    let display =
        SgbVramTransferDisplayState::new(SGB_LCDC_BG_ENABLE_BIT, 0, 0, SGB_TRANSFER_REQUIRED_BGP);

    let (extracted, source_mode) = SgbVramTransferBuffer::from_display_memory_with_source_mode(
        &vram,
        display,
        SgbVramTransferSourceMode::DisplayOrder,
    )
    .expect("LCD-disabled transfer tail should still use the prepared BG layout");

    assert_eq!(extracted.bytes, payload.to_vec());
    assert_eq!(source_mode, SgbVramTransferSourceMode::DisplayOrder);
}

#[test]
fn vram_transfer_display_extraction_keeps_lcd_disabled_start_on_raw_path() {
    let mut vram = signed_display_vram_from_payload(&frame_payload(3));
    vram[0] = 0x5A;
    let display =
        SgbVramTransferDisplayState::new(SGB_LCDC_BG_ENABLE_BIT, 0, 0, SGB_TRANSFER_REQUIRED_BGP);

    let payload = SgbVramTransferBuffer::from_display_memory(&vram, display)
        .expect("LCD-disabled unclassified transfer start should fall back to raw VRAM");

    assert_eq!(payload.bytes[0], 0x5A);
}

#[test]
fn obj_trn_records_host_obj_state_and_captures_oam_from_display_frames() {
    let mut host = accepted_sgb_host();
    let mut palettes = [0; SGB_VRAM_TRANSFER_BYTES];
    write_system_palette_color(&mut palettes, 3, 0, 0x001F);
    write_system_palette_color(&mut palettes, 3, 1, 0x03E0);
    write_system_palette_color(&mut palettes, 4, 0, 0x7C00);
    write_system_palette_color(&mut palettes, 4, 1, 0x4210);
    write_joyp_packet(&mut host, sgb_pal_trn_packet());
    host.capture_pending_vram_transfer(&palettes)
        .expect("PAL_TRN should load OBJ backing palette data");

    write_joyp_packet(&mut host, sgb_obj_trn_packet(0x03, [3, 4, 5, 511]));
    let snapshot = host.snapshot();
    assert!(snapshot.video.obj.enabled);
    assert!(snapshot.video.obj.color_transfer_requested);
    assert_eq!(snapshot.video.obj.command_count, 1);
    assert_eq!(snapshot.video.obj.palette_ids, [3, 4, 5, 511]);
    assert_eq!(snapshot.video.obj.palettes[0].colors[0].raw(), 0x001F);
    assert_eq!(snapshot.video.obj.palettes[0].colors[1].raw(), 0x03E0);
    assert_eq!(snapshot.video.obj.palettes[0].colors[4].raw(), 0x7C00);
    assert_eq!(
        snapshot.packet_gate.busy_frames_remaining,
        SGB_OBJ_TRN_BUSY_FRAMES
    );

    let mut vram = [0; SGB_GB_VRAM_BYTES];
    for index in 0..SGB_OBJ_OAM_PAYLOAD_BYTES {
        vram[SGB_OBJ_OAM_SOURCE_OFFSET + index] = index as u8;
    }
    host.advance_frame_start(
        &vram,
        SgbVramTransferDisplayState::new(0x81, 0, 1, SGB_TRANSFER_REQUIRED_BGP),
    )
    .expect("OBJ_TRN frame capture should use the display-transfer seam");
    let snapshot = host.snapshot();
    assert_eq!(snapshot.packet_gate.busy_frames_remaining, 0);
    assert_eq!(snapshot.video.obj.frame_capture_count, 1);
    assert_eq!(
        snapshot
            .video
            .obj
            .last_oam_payload
            .as_ref()
            .expect("OBJ_TRN should retain the last OAM payload")
            .bytes,
        (0..SGB_OBJ_OAM_PAYLOAD_BYTES)
            .map(|index| index as u8)
            .collect::<Vec<_>>()
    );

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);
    assert_eq!(restored.snapshot().video.obj, snapshot.video.obj);
}

#[test]
fn vram_transfer_final_chunk_stays_display_order_when_lcd_turns_off() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_chr_trn_packet(1));

    let mut expected = [0; SGB_VRAM_TRANSFER_BYTES];
    let mut last_visible_payload = frame_payload(0);
    for frame_index in 0..SGB_VRAM_TRANSFER_TOTAL_FRAMES {
        let visible_frame = frame_index + 1 < SGB_VRAM_TRANSFER_TOTAL_FRAMES;
        let payload = if visible_frame {
            let payload = frame_payload(frame_index);
            last_visible_payload = payload;
            payload
        } else {
            frame_payload(0xEE)
        };
        let (chunk_start, chunk_end) =
            vram_transfer_chunk_range(frame_index, SGB_VRAM_TRANSFER_TOTAL_FRAMES);
        let expected_payload = if visible_frame {
            &payload
        } else {
            &last_visible_payload
        };
        expected[chunk_start..chunk_end].copy_from_slice(&expected_payload[chunk_start..chunk_end]);
        let display_lcdc = if visible_frame {
            SGB_LCDC_ENABLE_BIT | SGB_LCDC_BG_ENABLE_BIT
        } else {
            SGB_LCDC_BG_ENABLE_BIT
        };
        let result = host
            .advance_frame_start(
                &signed_display_vram_from_payload(&payload),
                SgbVramTransferDisplayState::new(display_lcdc, 0, 0, SGB_TRANSFER_REQUIRED_BGP),
            )
            .expect(
                "CHR_TRN capture should tolerate LCD-off tail after display-order capture starts",
            );
        if visible_frame {
            assert_eq!(result, None);
            assert!(
                host.snapshot()
                    .video
                    .vram_transfer
                    .display_order_payload
                    .is_some(),
                "display-order _TRN captures keep the last visible payload latched for LCD-off tails"
            );
        } else {
            assert_eq!(
                result,
                Some(SgbVramTransferTarget::Chr(SgbChrTransferSelection {
                    tile_block: 1,
                    tile_type: SgbChrTransferTileType::Bg,
                }))
            );
        }
    }

    let snapshot = host.snapshot();
    assert!(
        snapshot.video.vram_transfer.display_order_payload.is_none(),
        "the display-order latch is transfer-local and should not survive completion"
    );
    assert_eq!(
        snapshot
            .video
            .vram_transfer
            .last_completed
            .as_ref()
            .expect("completed CHR_TRN should retain the display-ordered payload")
            .payload
            .bytes,
        expected.to_vec()
    );
    assert_eq!(
        snapshot.video.border.tile_data.bytes[SGB_VRAM_TRANSFER_BYTES..SGB_VRAM_TRANSFER_BYTES * 2],
        expected[..]
    );
}

#[test]
fn packet_gate_rejects_packets_during_obj_trn_busy() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_obj_trn_packet(0x01, [0, 0, 0, 0]));
    let obj_snapshot = host.snapshot();
    assert_eq!(
        obj_snapshot.command.last_command_id,
        Some(SGB_COMMAND_OBJ_TRN)
    );
    assert_eq!(obj_snapshot.command.accepted_command_count, 1);
    assert_eq!(
        obj_snapshot.packet_gate.busy_frames_remaining,
        SGB_OBJ_TRN_BUSY_FRAMES
    );
    assert!(obj_snapshot.video.obj.enabled);

    let palette_state = obj_snapshot.video.palette_state;
    let palette_command_count = obj_snapshot.video.palette_command_count;
    let border_state = obj_snapshot.video.border.clone();
    let audio_state = obj_snapshot.audio;
    write_joyp_packet(&mut host, sgb_pal01_packet());
    let rejected = host.snapshot();
    assert_eq!(
        rejected.packet_transport.last_trace.status,
        SgbPacketTraceStatus::RejectedWhileBusy
    );
    assert_eq!(rejected.packet_gate.busy_rejected_packet_count, 1);
    assert_eq!(
        rejected.packet_gate.last_busy_command_id,
        Some(SGB_COMMAND_PAL01)
    );
    assert_eq!(rejected.command.accepted_command_count, 1);
    assert_eq!(rejected.command.last_command_id, Some(SGB_COMMAND_OBJ_TRN));
    assert_eq!(rejected.video.palette_state, palette_state);
    assert_eq!(rejected.video.palette_command_count, palette_command_count);
    assert_eq!(rejected.video.border, border_state);
    assert_eq!(rejected.audio, audio_state);

    host.advance_frame_start(&[0; SGB_GB_VRAM_BYTES], fallback_transfer_display())
        .expect("OBJ_TRN busy window should advance from deterministic host frames");
    assert_eq!(host.snapshot().packet_gate.busy_frames_remaining, 0);

    write_joyp_packet(&mut host, sgb_pal01_packet());
    let accepted = host.snapshot();
    assert_eq!(
        accepted.packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
    assert_eq!(accepted.packet_gate.busy_rejected_packet_count, 1);
    assert_eq!(accepted.command.last_command_id, Some(SGB_COMMAND_PAL01));
    assert_eq!(accepted.command.accepted_command_count, 2);
    assert_eq!(
        accepted.video.palette_command_count,
        palette_command_count + 1
    );
}

#[test]
fn sgb_boot_palette_seeds_default_and_title_matched_dmg_palettes() {
    let mut sgb = SgbHost::new(HostPlatform::Sgb);
    assert_eq!(
        sgb.snapshot().video.palette_state.palette(0),
        sgb_boot_palette(SGB_BOOT_PALETTE_DEFAULT_INDEX)
    );

    let alleyway = test_header_with_title(
        b"ALLEY WAY",
        SgbFlag::None,
        SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
    );
    sgb.apply_cartridge_header(Some(&alleyway));
    assert_eq!(
        sgb.command_acceptance(),
        SgbCommandAcceptance::RejectedByHeader
    );
    assert_eq!(
        sgb.snapshot().video.palette_state.palette(0),
        sgb_boot_palette(0x16)
    );
    assert_eq!(
        sgb.snapshot().video.map_lcd_shade_to_rgb555(0).raw(),
        0x65EF
    );
    assert_eq!(
        sgb.snapshot().video.map_lcd_shade_to_rgb555(3).raw(),
        0x2108
    );

    let unknown = test_header_with_title(
        b"UNKNOWN",
        SgbFlag::None,
        SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
    );
    sgb.apply_cartridge_header(Some(&unknown));
    assert_eq!(
        sgb.snapshot().video.palette_state.palette(0),
        sgb_boot_palette(SGB_BOOT_PALETTE_DEFAULT_INDEX)
    );
}

#[test]
fn sgb_boot_title_palette_requires_rejected_exact_padded_title() {
    let sgb_capable_alleyway = test_header_with_title(
        b"ALLEY WAY",
        SgbFlag::Supported,
        SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
    );
    let mut sgb = SgbHost::new(HostPlatform::Sgb);
    sgb.apply_cartridge_header(Some(&sgb_capable_alleyway));
    assert_eq!(sgb.command_acceptance(), SgbCommandAcceptance::Accepted);
    assert_eq!(
        sgb.snapshot().video.palette_state.palette(0),
        sgb_boot_palette(SGB_BOOT_PALETTE_DEFAULT_INDEX),
        "SGB-command-capable games keep the default boot palette until commands override it"
    );

    let mut space_padded = test_header_with_title(
        b"ALLEY WAY",
        SgbFlag::None,
        SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
    );
    space_padded.title_bytes[b"ALLEY WAY".len()] = b' ';
    sgb.apply_cartridge_header(Some(&space_padded));
    assert_eq!(
        sgb.snapshot().video.palette_state.palette(0),
        sgb_boot_palette(SGB_BOOT_PALETTE_DEFAULT_INDEX),
        "the SGB BIOS title table uses exact NUL-padded header title matches"
    );

    let handheld = SgbHost::new(HostPlatform::Handheld);
    assert_eq!(
        handheld.snapshot().video.palette_state.palette(0),
        SgbScreenPalette::dmg_grayscale()
    );
}

#[test]
fn palette_commands_update_host_palette_state_and_rgb555_mapping() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());

    let snapshot = host.snapshot();
    assert_eq!(snapshot.command.last_command_id, Some(SGB_COMMAND_PAL01));
    assert_eq!(
        snapshot.video.last_palette_command_id,
        Some(SGB_COMMAND_PAL01)
    );
    assert_eq!(snapshot.video.palette_command_count, 1);
    assert!(snapshot.video.colorization_active);

    let palette_0 = snapshot.video.palette_state.palette(0);
    assert_eq!(palette_0.colors[0].raw(), 0x001F);
    assert_eq!(palette_0.colors[1].raw(), 0x03E0);
    assert_eq!(palette_0.colors[2].raw(), 0x7C00);
    assert_eq!(palette_0.colors[3].raw(), 0x4210);

    let palette_1 = snapshot.video.palette_state.palette(1);
    assert_eq!(palette_1.colors[0].raw(), 0x001F);
    assert_eq!(palette_1.colors[1].raw(), 0x0001);
    assert_eq!(palette_1.colors[2].raw(), 0x0002);
    assert_eq!(
        palette_1.colors[3].raw(),
        0x0003,
        "SGB RGB555 colors mask off the ignored high bit"
    );
    assert_eq!(
        snapshot.video.palette_state.palette(2).colors[0].raw(),
        0x001F,
        "direct palette commands update the shared LCD color 0 for palettes outside the targeted color-1..3 pair"
    );
    assert_eq!(
        snapshot.video.palette_state.palette(2).colors[1..],
        SgbScreenPalette::dmg_grayscale().colors[1..]
    );
    assert_eq!(snapshot.video.map_lcd_shade_to_rgb555(0).raw(), 0x001F);
    assert_eq!(snapshot.video.map_lcd_shade_to_rgb555(1).raw(), 0x03E0);
    assert_eq!(snapshot.video.map_lcd_shade_to_rgb555(2).raw(), 0x7C00);
    assert_eq!(snapshot.video.map_lcd_shade_to_rgb555(3).raw(), 0x4210);
}

#[test]
fn direct_palette_commands_target_documented_palette_pairs() {
    for (command_id, first_palette, second_palette) in [
        (SGB_COMMAND_PAL01, 0, 1),
        (SGB_COMMAND_PAL23, 2, 3),
        (SGB_COMMAND_PAL03, 0, 3),
        (SGB_COMMAND_PAL12, 1, 2),
    ] {
        let mut host = accepted_sgb_host();
        let mut packet = sgb_command_packet(command_id, 1);
        write_packet_color(&mut packet, 1, 0x001F);
        write_packet_color(&mut packet, 3, 0x03E0);
        write_packet_color(&mut packet, 5, 0x7C00);
        write_packet_color(&mut packet, 7, 0x4210);
        write_packet_color(&mut packet, 9, 0x0001);
        write_packet_color(&mut packet, 11, 0x0002);
        write_packet_color(&mut packet, 13, 0x0003);

        write_joyp_packet(&mut host, packet);

        let palettes = host.snapshot().video.palette_state.screen_palettes;
        for palette in palettes {
            assert_eq!(
                palette.colors[0].raw(),
                0x001F,
                "SGB screen color 0 is shared across all four visible palettes"
            );
        }
        assert_eq!(palettes[first_palette].colors[1].raw(), 0x03E0);
        assert_eq!(palettes[first_palette].colors[2].raw(), 0x7C00);
        assert_eq!(palettes[first_palette].colors[3].raw(), 0x4210);
        assert_eq!(palettes[second_palette].colors[1].raw(), 0x0001);
        assert_eq!(palettes[second_palette].colors[2].raw(), 0x0002);
        assert_eq!(palettes[second_palette].colors[3].raw(), 0x0003);
    }
}

#[test]
fn sgb_lcd_composition_maps_dmg_shades_through_base_palette() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());
    let mut dmg_framebuffer = vec![0; SGB_LCD_PIXELS];
    dmg_framebuffer[..8].copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7]);

    let rgb555 = host
        .compose_lcd_rgb555(&dmg_framebuffer)
        .expect("active SGB host should compose the GB LCD image");
    assert_eq!(
        &rgb555[..8],
        &[
            0x001F, 0x03E0, 0x7C00, 0x4210, 0x001F, 0x03E0, 0x7C00, 0x4210
        ]
    );

    let handheld = SgbHost::new(HostPlatform::Handheld);
    assert_eq!(
        handheld.compose_lcd_rgb555(&dmg_framebuffer),
        Err(SgbLcdCompositionError::DisabledHost)
    );
    assert_eq!(
        host.compose_lcd_rgb555(&dmg_framebuffer[..SGB_LCD_PIXELS - 1]),
        Err(SgbLcdCompositionError::InputLength {
            expected: SGB_LCD_PIXELS,
            actual: SGB_LCD_PIXELS - 1,
        })
    );
}

#[test]
fn attribute_commands_update_host_attribute_map_and_lcd_composition() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());

    let mut attr_div = sgb_command_packet(SGB_COMMAND_ATTR_DIV, 1);
    attr_div[1] = (1 << 2) | (2 << 4);
    attr_div[2] = 1;
    write_joyp_packet(&mut host, attr_div);
    assert_eq!(host.snapshot().video.attributes.map.palette_index(0, 0), 1);
    assert_eq!(host.snapshot().video.attributes.map.palette_index(1, 0), 2);
    assert_eq!(host.snapshot().video.attributes.map.palette_index(2, 0), 0);

    let mut attr_lin = sgb_command_packet(SGB_COMMAND_ATTR_LIN, 1);
    attr_lin[1] = 1;
    attr_lin[2] = 0x80 | (3 << 5);
    write_joyp_packet(&mut host, attr_lin);
    assert_eq!(host.snapshot().video.attributes.map.palette_index(0, 0), 3);
    assert_eq!(host.snapshot().video.attributes.attr_lin_count, 1);

    let mut attr_blk = sgb_command_packet(SGB_COMMAND_ATTR_BLK, 1);
    attr_blk[1] = 1;
    attr_blk[2] = 0x03;
    attr_blk[3] = 2 | (1 << 2);
    attr_blk[4] = 1;
    attr_blk[5] = 1;
    attr_blk[6] = 3;
    attr_blk[7] = 3;
    write_joyp_packet(&mut host, attr_blk);
    assert_eq!(
        host.snapshot().video.attributes.map.palette_index(1, 1),
        1,
        "ATTR_BLK line cells use the surrounding palette"
    );
    assert_eq!(
        host.snapshot().video.attributes.map.palette_index(2, 2),
        2,
        "ATTR_BLK inner cells use the inside palette"
    );

    let mut attr_chr = sgb_command_packet(SGB_COMMAND_ATTR_CHR, 1);
    attr_chr[1] = 0;
    attr_chr[2] = 0;
    attr_chr[3] = 4;
    attr_chr[4] = 0;
    attr_chr[5] = 0;
    attr_chr[6] = 0b00_01_10_11;
    write_joyp_packet(&mut host, attr_chr);
    assert_eq!(host.snapshot().video.attributes.map.palette_index(0, 0), 0);
    assert_eq!(host.snapshot().video.attributes.map.palette_index(1, 0), 1);
    assert_eq!(host.snapshot().video.attributes.map.palette_index(2, 0), 2);
    assert_eq!(host.snapshot().video.attributes.map.palette_index(3, 0), 3);

    let dmg_framebuffer = vec![1; SGB_LCD_PIXELS];
    let rgb555 = host
        .compose_lcd_rgb555(&dmg_framebuffer)
        .expect("SGB LCD composition should use host attribute palettes");
    assert_eq!(rgb555[8], 0x0001);
}

#[test]
fn attr_chr_uses_multi_packet_payload_data_after_the_first_packet() {
    let mut host = accepted_sgb_host();
    let mut first_packet = sgb_command_packet(SGB_COMMAND_ATTR_CHR, 2);
    first_packet[1] = 0;
    first_packet[2] = 0;
    first_packet[3] = 44;
    first_packet[4] = 0;
    first_packet[5] = 0;
    let mut second_packet = [0; SGB_PACKET_BYTES];
    second_packet[0] = 0b11_00_00_00;

    write_joyp_packet(&mut host, first_packet);
    write_joyp_packet(&mut host, second_packet);

    assert_eq!(
        host.snapshot().command.last_command_id,
        Some(SGB_COMMAND_ATTR_CHR)
    );
    assert_eq!(
        host.snapshot().video.attributes.map.palette_index(0, 2),
        3,
        "data set 41 is stored in the first byte of the second SGB packet"
    );
}

#[test]
fn pal_trn_pal_set_and_pal_pri_update_system_palette_state() {
    let mut host = accepted_sgb_host();
    let mut transfer = [0; SGB_VRAM_TRANSFER_BYTES];
    write_system_palette_color(&mut transfer, 7, 0, 0x001F);
    write_system_palette_color(&mut transfer, 7, 1, 0x03E0);
    write_system_palette_color(&mut transfer, 7, 2, 0x7C00);
    write_system_palette_color(&mut transfer, 7, 3, 0x8421);

    write_joyp_packet(&mut host, sgb_pal_trn_packet());
    assert_eq!(
        host.capture_pending_vram_transfer(&transfer)
            .expect("PAL_TRN should capture system palette data"),
        Some(SgbVramTransferTarget::Pal)
    );

    let mut pal_set = sgb_command_packet(SGB_COMMAND_PAL_SET, 1);
    for palette_index in 0..SGB_SCREEN_PALETTE_COUNT {
        let [low, high] = 7u16.to_le_bytes();
        pal_set[1 + palette_index * 2] = low;
        pal_set[2 + palette_index * 2] = high;
    }
    write_joyp_packet(&mut host, pal_set);

    let snapshot = host.snapshot();
    assert!(snapshot.video.system_palettes.loaded);
    assert_eq!(snapshot.video.system_palettes.pal_trn_count, 1);
    assert_eq!(snapshot.video.system_palettes.pal_set_count, 1);
    assert_eq!(
        snapshot.video.palette_state.palette(0).colors[0].raw(),
        0x001F
    );
    assert_eq!(
        snapshot.video.palette_state.palette(0).colors[1].raw(),
        0x03E0
    );
    assert_eq!(
        snapshot.video.palette_state.palette(0).colors[3].raw(),
        0x0421,
        "PAL_TRN colors keep RGB555 bit 15 ignored"
    );

    let mut pal_pri = sgb_command_packet(SGB_COMMAND_PAL_PRI, 1);
    pal_pri[1] = 1;
    write_joyp_packet(&mut host, pal_pri);
    assert!(host.snapshot().video.system_palettes.pal_pri_enabled);
    assert_eq!(
        host.snapshot().video.system_palettes.pal_pri_command_count,
        1
    );
}

#[test]
fn pal_pri_controls_player_palette_override_priority() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());

    let player_palette = sgb_screen_palette([0x1111, 0x2222, 0x3333, 0x4444]);
    assert!(host.set_player_palette_override(player_palette));
    assert!(host.snapshot().video.player_palette_override.active);
    assert_eq!(
        host.snapshot().video.map_lcd_shade_to_rgb555(2).raw(),
        0x3333
    );

    write_joyp_packet(&mut host, sgb_pal01_packet());
    assert!(
        host.snapshot().video.player_palette_override.active,
        "with PAL_PRI disabled, application palette commands update host state but do not override the player's selected palette"
    );
    assert_eq!(
        host.snapshot().video.map_lcd_shade_to_rgb555(1).raw(),
        0x2222
    );

    let mut pal_pri = sgb_command_packet(SGB_COMMAND_PAL_PRI, 1);
    pal_pri[1] = 1;
    write_joyp_packet(&mut host, pal_pri);
    assert!(host.snapshot().video.system_palettes.pal_pri_enabled);
    assert!(
        host.snapshot().video.player_palette_override.active,
        "PAL_PRI itself only changes the priority policy; a later application palette command switches back"
    );

    write_joyp_packet(&mut host, sgb_pal01_packet());
    let snapshot = host.snapshot();
    assert!(!snapshot.video.player_palette_override.active);
    assert_eq!(
        snapshot.video.player_palette_override.pal_pri_release_count,
        1
    );
    assert_eq!(
        snapshot.video.map_lcd_shade_to_rgb555(1).raw(),
        0x03E0,
        "once PAL_PRI gives priority back to the application, visible output uses the game palette state"
    );
}

#[test]
fn pal_pri_does_not_switch_on_transfer_loads_but_switches_on_attribute_commands() {
    let mut host = accepted_sgb_host();
    let player_palette = sgb_screen_palette([0x1111, 0x2222, 0x3333, 0x4444]);
    assert!(host.set_player_palette_override(player_palette));

    let mut pal_pri = sgb_command_packet(SGB_COMMAND_PAL_PRI, 1);
    pal_pri[1] = 1;
    write_joyp_packet(&mut host, pal_pri);

    write_joyp_packet(&mut host, sgb_pal_trn_packet());
    assert!(
        host.snapshot().video.player_palette_override.active,
        "PAL_TRN loads backing system palette memory and must not by itself switch away from the player palette"
    );
    host.capture_pending_vram_transfer(&[0; SGB_VRAM_TRANSFER_BYTES])
        .expect("PAL_TRN must finish before a later command packet is accepted");
    assert!(
        host.snapshot().video.player_palette_override.active,
        "completed PAL_TRN still must not switch away from the player palette until an application command consumes PAL_PRI"
    );

    let mut attr_set = sgb_command_packet(SGB_COMMAND_ATTR_SET, 1);
    attr_set[1] = 0;
    write_joyp_packet(&mut host, attr_set);
    assert!(!host.snapshot().video.player_palette_override.active);
    assert_eq!(
        host.snapshot()
            .video
            .player_palette_override
            .pal_pri_release_count,
        1
    );
}

#[test]
fn pal_set_uses_palette_zero_color_zero_as_shared_lcd_color_zero() {
    let mut host = accepted_sgb_host();
    let mut transfer = [0; SGB_VRAM_TRANSFER_BYTES];
    write_system_palette_color(&mut transfer, 7, 0, 0x5F5F);
    write_system_palette_color(&mut transfer, 7, 1, 0x2EDB);
    write_system_palette_color(&mut transfer, 8, 0, 0x68BF);
    write_system_palette_color(&mut transfer, 8, 1, 0x0CA7);
    write_system_palette_color(&mut transfer, 9, 0, 0x001F);
    write_system_palette_color(&mut transfer, 9, 1, 0x03E0);
    write_system_palette_color(&mut transfer, 10, 0, 0x7C00);
    write_system_palette_color(&mut transfer, 10, 1, 0x4210);

    write_joyp_packet(&mut host, sgb_pal_trn_packet());
    host.capture_pending_vram_transfer(&transfer)
        .expect("PAL_TRN should capture system palette data");

    let mut pal_set = sgb_command_packet(SGB_COMMAND_PAL_SET, 1);
    for (palette_index, palette_id) in [7_u16, 8, 9, 10].into_iter().enumerate() {
        let [low, high] = palette_id.to_le_bytes();
        pal_set[1 + palette_index * 2] = low;
        pal_set[2 + palette_index * 2] = high;
    }
    write_joyp_packet(&mut host, pal_set);

    let snapshot = host.snapshot();
    for palette in snapshot.video.palette_state.screen_palettes {
        assert_eq!(
            palette.colors[0].raw(),
            0x5F5F,
            "PAL_SET takes the shared visible LCD color 0 from physical SGB palette 0, not from later palette IDs"
        );
    }
    assert_eq!(snapshot.video.backdrop_color.raw(), 0x5F5F);
    assert_eq!(
        snapshot.video.palette_state.palette(0).colors[1].raw(),
        0x2EDB
    );
    assert_eq!(
        snapshot.video.palette_state.palette(1).colors[1].raw(),
        0x0CA7
    );
    assert_eq!(
        snapshot.video.palette_state.palette(2).colors[1].raw(),
        0x03E0
    );
    assert_eq!(
        snapshot.video.palette_state.palette(3).colors[1].raw(),
        0x4210
    );
}

#[test]
fn attr_trn_attr_set_and_pal_set_apply_attribute_files() {
    let mut host = accepted_sgb_host();
    let mut transfer = [0; SGB_VRAM_TRANSFER_BYTES];
    write_atf_cell(&mut transfer, 2, 0, 0, 1);
    write_atf_cell(&mut transfer, 2, 1, 0, 2);
    write_atf_cell(&mut transfer, 2, 0, 1, 3);

    write_joyp_packet(&mut host, sgb_attr_trn_packet());
    assert_eq!(
        host.capture_pending_vram_transfer(&transfer)
            .expect("ATTR_TRN should capture ATF data"),
        Some(SgbVramTransferTarget::Attr)
    );

    write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::Freeze));
    let mut attr_set = sgb_command_packet(SGB_COMMAND_ATTR_SET, 1);
    attr_set[1] = 0x40 | 2;
    write_joyp_packet(&mut host, attr_set);

    let snapshot = host.snapshot();
    assert_eq!(snapshot.video.mask, SgbScreenMask::Cancel);
    assert_eq!(snapshot.video.attributes.last_atf_index, Some(2));
    assert_eq!(snapshot.video.attributes.attr_trn_count, 1);
    assert_eq!(snapshot.video.attributes.attr_set_count, 1);
    assert_eq!(snapshot.video.attributes.map.palette_index(0, 0), 1);
    assert_eq!(snapshot.video.attributes.map.palette_index(1, 0), 2);
    assert_eq!(snapshot.video.attributes.map.palette_index(0, 1), 3);

    let mut pal_set = sgb_command_packet(SGB_COMMAND_PAL_SET, 1);
    pal_set[9] = 0x80 | 2;
    write_joyp_packet(&mut host, pal_set);
    assert_eq!(host.snapshot().video.attributes.attr_set_count, 2);
    assert_eq!(host.snapshot().video.attributes.map.palette_index(0, 0), 1);
}

#[test]
fn chr_trn_captures_vram_transfer_into_border_tile_blocks() {
    let mut host = accepted_sgb_host();
    let first_block = solid_tile_color_1_transfer();
    write_joyp_packet(&mut host, sgb_chr_trn_packet(0));
    assert!(matches!(
        host.snapshot().video.vram_transfer.pending,
        Some(SgbPendingVramTransfer {
            target: SgbVramTransferTarget::Chr(SgbChrTransferSelection {
                tile_block: 0,
                tile_type: SgbChrTransferTileType::Bg,
            }),
            ..
        })
    ));

    let chr1_loaded_before = host.snapshot().video.border.chr1_loaded;
    assert_eq!(
        host.capture_pending_vram_transfer(&first_block)
            .expect("CHR_TRN should capture the pending VRAM payload"),
        Some(SgbVramTransferTarget::Chr(SgbChrTransferSelection {
            tile_block: 0,
            tile_type: SgbChrTransferTileType::Bg,
        }))
    );
    let snapshot = host.snapshot();
    assert!(snapshot.video.border.chr0_loaded);
    assert_eq!(snapshot.video.border.chr1_loaded, chr1_loaded_before);
    assert_eq!(snapshot.video.border.chr_transfer_count, 1);
    assert_eq!(snapshot.video.border.tile_data.bytes[0], 0xFF);
    assert_eq!(
        snapshot
            .video
            .vram_transfer
            .last_completed
            .as_ref()
            .expect("last transfer should be retained for save-state observability")
            .payload
            .bytes[0],
        0xFF
    );

    let mut second_block = [0; SGB_VRAM_TRANSFER_BYTES];
    second_block[0] = 0x80;
    write_joyp_packet(&mut host, sgb_chr_trn_packet(1));
    host.capture_pending_vram_transfer(&second_block)
        .expect("second CHR_TRN should replace the high tile block");
    let snapshot = host.snapshot();
    assert!(snapshot.video.border.chr0_loaded);
    assert!(snapshot.video.border.chr1_loaded);
    assert_eq!(snapshot.video.border.chr_transfer_count, 2);
    assert_eq!(
        snapshot.video.border.tile_data.bytes[SGB_VRAM_TRANSFER_BYTES],
        0x80
    );
}

#[test]
fn pct_trn_decodes_border_tilemap_and_border_palettes() {
    let mut host = accepted_sgb_host();
    let mut pct = [0; SGB_VRAM_TRANSFER_BYTES];
    write_border_map_entry(&mut pct, 0, (4 << 10) | 3);
    write_border_map_entry(
        &mut pct,
        SGB_BORDER_TILEMAP_WIDTH * SGB_BORDER_TILEMAP_VISIBLE_HEIGHT,
        0x8000 | (6 << 10) | 4,
    );
    write_border_palette_color(&mut pct, 0, 1, 0x03E0);
    write_border_palette_color(&mut pct, 1, 2, 0x7C00);
    write_border_palette_color(&mut pct, 2, 3, 0x801F);

    write_joyp_packet(&mut host, sgb_pct_trn_packet());
    host.capture_pending_vram_transfer(&pct)
        .expect("PCT_TRN should capture the pending VRAM payload");

    let snapshot = host.snapshot();
    assert!(snapshot.video.border_loaded);
    assert!(snapshot.video.border.pct_loaded);
    assert_eq!(snapshot.video.border.pct_transfer_count, 1);
    assert_eq!(snapshot.video.border.tile_map.entries[0].raw, (4 << 10) | 3);
    assert_eq!(
        snapshot.video.border.tile_map.entries
            [SGB_BORDER_TILEMAP_WIDTH * SGB_BORDER_TILEMAP_VISIBLE_HEIGHT]
            .raw,
        0x8000 | (6 << 10) | 4
    );
    assert_eq!(snapshot.video.border.palettes[0].colors[1].raw(), 0x03E0);
    assert_eq!(snapshot.video.border.palettes[1].colors[2].raw(), 0x7C00);
    assert_eq!(
        snapshot.video.border.palettes[2].colors[3].raw(),
        0x001F,
        "border RGB555 palette data masks off the ignored high bit"
    );
}

#[test]
fn border_color_zero_uses_shared_backdrop_outside_lcd_window() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());
    write_joyp_packet(&mut host, sgb_chr_trn_packet(0));
    host.capture_pending_vram_transfer(&solid_tile_color_1_transfer())
        .expect("CHR_TRN should load a non-zero border tile");

    let mut pct = [0; SGB_VRAM_TRANSFER_BYTES];
    write_border_map_entry(&mut pct, 0, (5 << 10) | 1);
    write_border_map_entry(&mut pct, 1, 4 << 10);
    write_border_map_entry(
        &mut pct,
        (SGB_LCD_FRAME_ORIGIN_Y / 8) * SGB_BORDER_TILEMAP_WIDTH + SGB_LCD_FRAME_ORIGIN_X / 8,
        (5 << 10) | 1,
    );
    write_border_palette_color(&mut pct, 0, 0, 0x0001);
    write_border_palette_color(&mut pct, 0, 1, 0x03E0);
    write_border_palette_color(&mut pct, 1, 0, 0x001F);
    write_border_palette_color(&mut pct, 2, 0, 0x7FFF);

    write_joyp_packet(&mut host, sgb_pct_trn_packet());
    host.capture_pending_vram_transfer(&pct)
        .expect("PCT_TRN should load border map and palette data");
    finish_shell_border_transition(&mut host);

    let dmg_framebuffer = vec![3; SGB_LCD_PIXELS];
    let frame = host
        .compose_frame_rgb555(&dmg_framebuffer)
        .expect("SGB host should compose border and LCD pixels");
    assert_eq!(host.snapshot().video.backdrop_color.raw(), 0x001F);
    assert_eq!(
        frame[0], 0x001F,
        "border color index 0 outside the GB window uses the current application backdrop, not the selected border palette's local color 0 or later PCT_TRN color-0 data"
    );
    assert_eq!(
        frame[8], 0x03E0,
        "non-zero border pixels still use their selected border palette color"
    );

    let lcd_origin = SGB_LCD_FRAME_ORIGIN_Y * SGB_FRAME_WIDTH + SGB_LCD_FRAME_ORIGIN_X;
    assert_eq!(
        frame[lcd_origin], 0x4210,
        "border color index 0 inside the GB window remains the transparent seam to the composed LCD image"
    );
    let lcd_color_zero_frame = host
        .compose_lcd_rgb555(&vec![0; SGB_LCD_PIXELS])
        .expect("SGB host should compose LCD pixels after PCT_TRN");
    assert_eq!(
        lcd_color_zero_frame[0], 0x001F,
        "PCT_TRN must not recolor LCD shade 0 away from the active screen palette"
    );
}

#[test]
fn pct_trn_keeps_transparent_border_pixels_on_current_backdrop() {
    let mut host = accepted_sgb_host();
    let initial_backdrop = host.snapshot().video.backdrop_color.raw();

    let mut pct = [0; SGB_VRAM_TRANSFER_BYTES];
    write_border_map_entry(&mut pct, 0, 6 << 10);
    write_border_palette_color(&mut pct, 2, 0, 0x7CD2);
    write_border_palette_color(&mut pct, 2, 1, 0x03E0);
    write_joyp_packet(&mut host, sgb_pct_trn_packet());
    host.capture_pending_vram_transfer(&pct)
        .expect("PCT_TRN should load border map and palette data");
    finish_shell_border_transition(&mut host);

    let frame = host
        .compose_frame_rgb555(&vec![0; SGB_LCD_PIXELS])
        .expect("SGB host should compose transparent border pixels");
    assert_eq!(
        host.snapshot().video.border.palettes[2].colors[0].raw(),
        0x7CD2
    );
    assert_eq!(host.snapshot().video.backdrop_color.raw(), initial_backdrop);
    assert_eq!(
        frame[0], initial_backdrop,
        "Pokémon Gold-style PCT_TRN palette color 0 payloads are transparent border data and must not become a temporary purple backdrop"
    );
}

#[test]
fn sgb_frame_composition_combines_border_and_lcd_window() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());
    write_joyp_packet(&mut host, sgb_chr_trn_packet(0));
    host.capture_pending_vram_transfer(&solid_tile_color_1_transfer())
        .expect("CHR_TRN should load the visible border tile");

    let mut pct = [0; SGB_VRAM_TRANSFER_BYTES];
    for y in 0..SGB_BORDER_TILEMAP_VISIBLE_HEIGHT {
        for x in 0..SGB_BORDER_TILEMAP_WIDTH {
            write_border_map_entry(&mut pct, y * SGB_BORDER_TILEMAP_WIDTH + x, 4 << 10);
        }
    }
    for y in 5..23 {
        for x in 6..26 {
            write_border_map_entry(&mut pct, y * SGB_BORDER_TILEMAP_WIDTH + x, (4 << 10) | 1);
        }
    }
    write_border_palette_color(&mut pct, 0, 0, 0x001F);
    write_border_palette_color(&mut pct, 1, 0, 0x001F);
    write_border_palette_color(&mut pct, 2, 0, 0x001F);
    write_border_palette_color(&mut pct, 0, 1, 0x03E0);
    write_joyp_packet(&mut host, sgb_pct_trn_packet());
    host.capture_pending_vram_transfer(&pct)
        .expect("PCT_TRN should load the border map and palettes");
    finish_shell_border_transition(&mut host);

    let dmg_framebuffer = vec![0; SGB_LCD_PIXELS];
    let frame = host
        .compose_frame_rgb555(&dmg_framebuffer)
        .expect("SGB host should compose the full 256x224 host frame");
    assert_eq!(frame.len(), SGB_FRAME_PIXELS);
    assert_eq!(frame[0], 0x03E0);
    let lcd_origin = SGB_LCD_FRAME_ORIGIN_Y * SGB_FRAME_WIDTH + SGB_LCD_FRAME_ORIGIN_X;
    assert_eq!(
        frame[lcd_origin], 0x001F,
        "color-index 0 border pixels inside the GB window let the SGB LCD image show through"
    );
}

#[test]
fn mask_en_blanks_and_freezes_the_host_lcd_image() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());
    let current_lcd = vec![0; SGB_LCD_PIXELS];
    let mut changed_lcd = vec![3; SGB_LCD_PIXELS];

    write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::Freeze));
    assert!(host.snapshot().video.freeze_capture_pending);
    host.capture_pending_lcd_freeze(&current_lcd)
        .expect("freeze should capture the current host LCD image");
    assert!(!host.snapshot().video.freeze_capture_pending);
    assert_eq!(
        host.compose_lcd_rgb555(&changed_lcd)
            .expect("frozen SGB LCD should still compose")[0],
        0x001F
    );

    write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::BlankBlack));
    assert_eq!(
        host.compose_lcd_rgb555(&changed_lcd)
            .expect("blank-black SGB LCD should compose")[0],
        0x0000
    );

    write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::BlankColor0));
    changed_lcd[0] = 3;
    assert_eq!(
        host.compose_lcd_rgb555(&changed_lcd)
            .expect("blank-color0 SGB LCD should compose")[0],
        0x001F
    );

    write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::Cancel));
    assert_eq!(
        host.compose_lcd_rgb555(&changed_lcd)
            .expect("unmasked SGB LCD should compose")[0],
        0x4210
    );
}
