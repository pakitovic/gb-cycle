use super::support::*;

#[test]
fn helper_edges_keep_sgb_contract_helpers_observable() {
    let address = SgbSnesAddress::new(0x12, 0x3456);
    assert_eq!(address.raw24(), 0x12_3456);

    let mut data = [0; SGB_DATA_SND_INLINE_BYTES];
    for (index, byte) in data.iter_mut().enumerate() {
        *byte = index as u8;
    }
    let oversized = SgbDataSendRequest {
        destination: address,
        declared_len: (SGB_DATA_SND_INLINE_BYTES + 4) as u8,
        data,
    };
    assert_eq!(oversized.payload_len(), SGB_DATA_SND_INLINE_BYTES);
    assert_eq!(oversized.payload(), &data);

    assert_eq!(SgbJoypLineState::Idle.data_bit(), None);
    assert_eq!(SgbJoypLineState::Invalid.data_bit(), None);
    assert_eq!(
        SgbChrTransferSelection::from_command_byte(0x03),
        SgbChrTransferSelection {
            tile_block: 1,
            tile_type: SgbChrTransferTileType::Obj,
        }
    );
    assert_eq!(
        SgbChrTransferSelection::from_command_byte(0x03).destination_offset(),
        SGB_VRAM_TRANSFER_BYTES
    );
    assert_eq!(
        gb_tile_data_offset(SGB_LCDC_BG_WINDOW_TILE_DATA_BIT, 0x12),
        0x120
    );
    assert!(!sgb_title_bytes_match(&[0; 16], &[0; 17]));

    let palette_state = SgbPaletteState::default();
    assert_eq!(palette_state.map_lcd_shade(2), SGB_RGB555_DARK_GRAY);
    let mut invalid_direct_palette = palette_state;
    invalid_direct_palette.apply_direct_palette_command(0x1F, &sgb_command_packet(0x1F, 1));
    assert_eq!(invalid_direct_palette, palette_state);
    assert_eq!(
        SgbScreenPalette::default(),
        SgbScreenPalette::dmg_grayscale()
    );
}

#[test]
fn vram_transfer_helpers_report_source_edges_and_display_order() {
    assert_eq!(
        SgbVramTransferBuffer::from_source_bytes(&[0; SGB_VRAM_TRANSFER_BYTES - 1]),
        Err(SgbVramTransferError::SourceLength {
            expected: SGB_VRAM_TRANSFER_BYTES,
            actual: SGB_VRAM_TRANSFER_BYTES - 1,
        })
    );
    assert_eq!(
        SgbVramTransferBuffer::from_display_memory(
            &[0; SGB_GB_VRAM_BYTES - 1],
            SgbVramTransferDisplayState::new(0x81, 0, 0, SGB_TRANSFER_REQUIRED_BGP),
        ),
        Err(SgbVramTransferError::SourceLength {
            expected: SGB_GB_VRAM_BYTES,
            actual: SGB_GB_VRAM_BYTES - 1,
        })
    );

    let mut vram = vec![0; SGB_GB_VRAM_BYTES];
    let lcdc = SGB_LCDC_ENABLE_BIT
        | SGB_LCDC_BG_ENABLE_BIT
        | SGB_LCDC_BG_TILE_MAP_BIT
        | SGB_LCDC_BG_WINDOW_TILE_DATA_BIT;
    vram[SGB_GB_BG_MAP_9C00_OFFSET] = 2;
    vram[gb_tile_data_offset(lcdc, 2)] = 0x5A;
    let payload = SgbVramTransferBuffer::from_display_memory(
        &vram,
        SgbVramTransferDisplayState::new(lcdc, 0, 0, SGB_TRANSFER_REQUIRED_BGP),
    )
    .expect("prepared 9C00 transfer display should extract tile data");
    assert_eq!(payload.bytes[0], 0x5A);
    assert_eq!(
        SgbVramTransferBuffer::default().dynamic_payload_bytes(),
        SGB_VRAM_TRANSFER_BYTES
    );
}

#[test]
fn sound_transfer_jump_and_dynamic_payload_accounting_are_explicit() {
    let empty = SgbVramTransferBuffer { bytes: Vec::new() };
    assert_eq!(
        SgbSoundTransferRequest::from_vram_transfer_payload(&empty),
        SgbSoundTransferRequest {
            first_packet: SgbSoundTransferPacket::Jump {
                address: SgbApuRamAddress::new(0),
            },
            payload_bytes: 0,
        }
    );

    let mut video = SgbVideoState::default_for_active_host(true);
    assert!(video.dynamic_payload_bytes() > 0);
    video.frozen_lcd = Some(SgbLcdRgb555Frame::default());
    video.vram_transfer.last_completed = Some(SgbCompletedVramTransfer {
        command_id: SGB_COMMAND_PAL_TRN,
        target: SgbVramTransferTarget::Pal,
        payload: SgbVramTransferBuffer::default(),
    });
    assert!(
        video.dynamic_payload_bytes()
            >= SGB_LCD_PIXELS * std::mem::size_of::<u16>() + SGB_VRAM_TRANSFER_BYTES
    );

    let saved = SgbHost::new(HostPlatform::Sgb).capture_save_state();
    assert!(saved.dynamic_payload_bytes() > 0);
}

#[test]
fn vram_transfer_display_extraction_follows_signed_tiledata_transfer_screen() {
    let mut vram = vec![0; SGB_GB_VRAM_BYTES];
    for transfer_tile_index in 0..SGB_TRANSFER_DISPLAY_TILE_COUNT {
        let tile_index = 0x80u8.wrapping_add(transfer_tile_index as u8);
        write_transfer_screen_tile(&mut vram, transfer_tile_index, tile_index);
        let tile_offset = gb_tile_data_offset(0x81, tile_index);
        for byte_index in 0..SGB_GB_TILE_BYTES {
            vram[tile_offset + byte_index] = transfer_tile_index.wrapping_add(byte_index) as u8;
        }
    }

    let payload = SgbVramTransferBuffer::from_display_memory(
        &vram,
        SgbVramTransferDisplayState::new(0x81, 0, 0, SGB_TRANSFER_REQUIRED_BGP),
    )
    .expect("SGB transfer display should decode into a 4 KiB payload");

    assert_eq!(payload.bytes[0], 0);
    assert_eq!(payload.bytes[15], 15);
    assert_eq!(payload.bytes[128 * SGB_GB_TILE_BYTES], 128);
    assert_eq!(payload.bytes[255 * SGB_GB_TILE_BYTES], 255);
}

#[test]
fn vram_transfer_display_extraction_falls_back_to_raw_when_display_is_not_prepared() {
    let mut vram = vec![0; SGB_GB_VRAM_BYTES];
    vram[0] = 0x5A;
    vram[gb_tile_data_offset(0x81, 0x80)] = 0xA5;
    write_transfer_screen_tile(&mut vram, 0, 0x80);

    let payload = SgbVramTransferBuffer::from_display_memory(
        &vram,
        SgbVramTransferDisplayState::new(0x81, 1, 0, SGB_TRANSFER_REQUIRED_BGP),
    )
    .expect("unprepared transfer display falls back to the legacy raw payload path");

    assert_eq!(payload.bytes[0], 0x5A);
}
