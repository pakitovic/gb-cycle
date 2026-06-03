use super::*;

#[test]
fn huc3_and_mbc2_error_paths_are_reported_explicitly() {
    let mbc2_error = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 21,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 1 },
            },
        },
        persistent_state: PersistentCartState::Mbc2Ram {
            ram_nibbles: {
                let mut ram_nibbles = [0; MBC2_RAM_NIBBLE_COUNT];
                ram_nibbles[0] = 0x10;
                ram_nibbles
            },
        },
    })
    .expect_err("invalid MBC2 nibbles should fail to encode");
    assert_eq!(
        mbc2_error.to_string(),
        "invalid MBC2 nibble value 0x10 at logical cell 0"
    );

    let mut invalid_huc3_mcu_ram = [0; 256];
    invalid_huc3_mcu_ram[7] = 0x10;
    let huc3_error = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 22,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 0 },
            },
        },
        persistent_state: PersistentCartState::Huc3 {
            ram: vec![],
            mcu_ram: invalid_huc3_mcu_ram,
            rtc: Huc3RtcPersistentState {
                current_minutes_of_day: 0,
                current_days: 0,
                current_subminute_seconds: 0,
                event_minutes_of_day: 0,
                event_days: 0,
            },
            rom_bank: 0,
            ram_bank: 0,
            select_mode: 0x0D,
            access_address: 0,
            mailbox_command: 0,
            mailbox_argument: 0,
            last_response_nybble: 0,
            semaphore_ready: true,
            ir_emitter_on: false,
            ir_light_detected: false,
            last_control_write: None,
            last_unsupported_command: None,
            last_unsupported_argument: None,
        },
    })
    .expect_err("invalid HuC-3 nibbles should fail to encode");
    assert!(matches!(
        huc3_error,
        CartridgeSaveBackendError::InvalidHuc3NibbleValue {
            index: 7,
            value: 0x10,
        }
    ));
    assert_eq!(
        huc3_error.to_string(),
        "invalid HuC-3 nibble value 0x10 at logical cell 7"
    );
}

#[test]
fn huc3_decode_rejects_invalid_mcu_lengths_and_nibbles() {
    let rtc = Huc3RtcPersistentState {
        current_minutes_of_day: 1,
        current_days: 2,
        current_subminute_seconds: 3,
        event_minutes_of_day: 4,
        event_days: 5,
    };

    let mut bad_len_bytes = Vec::new();
    bad_len_bytes.push(STATE_HUC3_TAG);
    encode_linear_ram(&mut bad_len_bytes, &[0xAA], "HuC-3 RAM").expect("RAM should encode");
    write_u32_checked(
        &mut bad_len_bytes,
        255,
        "decoded HuC-3 MCU RAM nibble count",
    )
    .expect("length should encode");
    bad_len_bytes.extend(std::iter::repeat_n(0x00, 255));
    encode_huc3_rtc(&mut bad_len_bytes, rtc);
    bad_len_bytes.extend_from_slice(&[0x3F, 0x02, 0x0D, 0xA5, 0x06, 0x02, 0x01]);
    write_bool(&mut bad_len_bytes, true);
    write_bool(&mut bad_len_bytes, false);
    write_bool(&mut bad_len_bytes, true);
    encode_optional_u8(&mut bad_len_bytes, Some(0x77));
    encode_optional_u8(&mut bad_len_bytes, Some(0x06));
    encode_optional_u8(&mut bad_len_bytes, Some(0x0E));

    let mut bad_len_cursor = ByteCursor::new(&bad_len_bytes);
    let bad_len_error =
        decode_persistent_state(&mut bad_len_cursor).expect_err("invalid nibble count");
    assert!(matches!(
        bad_len_error,
        CartridgeSaveBackendError::LengthOverflow {
            field: "decoded HuC-3 MCU RAM nibble count",
            value: 255,
        }
    ));

    let mut bad_nibble_bytes = Vec::new();
    bad_nibble_bytes.push(STATE_HUC3_TAG);
    encode_linear_ram(&mut bad_nibble_bytes, &[0xBB], "HuC-3 RAM").expect("RAM should encode");
    write_u32_checked(
        &mut bad_nibble_bytes,
        256,
        "decoded HuC-3 MCU RAM nibble count",
    )
    .expect("length should encode");
    let mut nibble_bytes = [0u8; 256];
    nibble_bytes[9] = 0x10;
    bad_nibble_bytes.extend_from_slice(&nibble_bytes);
    encode_huc3_rtc(&mut bad_nibble_bytes, rtc);
    bad_nibble_bytes.extend_from_slice(&[0x3F, 0x02, 0x0D, 0xA5, 0x06, 0x02, 0x01]);
    write_bool(&mut bad_nibble_bytes, true);
    write_bool(&mut bad_nibble_bytes, false);
    write_bool(&mut bad_nibble_bytes, true);
    encode_optional_u8(&mut bad_nibble_bytes, Some(0x77));
    encode_optional_u8(&mut bad_nibble_bytes, Some(0x06));
    encode_optional_u8(&mut bad_nibble_bytes, Some(0x0E));

    let mut bad_nibble_cursor = ByteCursor::new(&bad_nibble_bytes);
    let bad_nibble_error =
        decode_persistent_state(&mut bad_nibble_cursor).expect_err("invalid HuC-3 nibble");
    assert!(matches!(
        bad_nibble_error,
        CartridgeSaveBackendError::InvalidHuc3NibbleValue {
            index: 9,
            value: 0x10,
        }
    ));
}

#[test]
fn decode_rejects_invalid_magic_version_and_truncated_payloads() {
    let mut bytes = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 123,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 1 },
            },
        },
        persistent_state: PersistentCartState::Mbc1Ram { ram: vec![0xAA] },
    })
    .expect("encode should succeed");

    bytes[0] ^= 0xFF;
    assert!(matches!(
        decode_cartridge_save_envelope(&bytes),
        Err(CartridgeSaveBackendError::InvalidMagic { .. })
    ));

    let original_bytes = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 123,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 1 },
            },
        },
        persistent_state: PersistentCartState::Mbc1Ram { ram: vec![0xAA] },
    })
    .expect("encode should succeed");
    let mut version_bytes = original_bytes.clone();
    version_bytes[8..10].copy_from_slice(&(CURRENT_SAVE_FORMAT_VERSION + 1).to_le_bytes());
    assert!(matches!(
        decode_cartridge_save_envelope(&version_bytes),
        Err(CartridgeSaveBackendError::UnsupportedFormatVersion { .. })
    ));

    let truncated = &original_bytes[..original_bytes.len() - 1];
    assert!(matches!(
        decode_cartridge_save_envelope(truncated),
        Err(CartridgeSaveBackendError::UnexpectedEof { .. })
    ));
}

#[test]
fn decode_rejects_invalid_mbc2_nibbles() {
    let mut bytes = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 123,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 512 },
            },
        },
        persistent_state: PersistentCartState::Mbc2Ram {
            ram_nibbles: [0; MBC2_RAM_NIBBLE_COUNT],
        },
    })
    .expect("encode should succeed");

    let nibble_offset = bytes.len() - MBC2_RAM_NIBBLE_COUNT;
    bytes[nibble_offset] = 0xFE;

    assert!(matches!(
        decode_cartridge_save_envelope(&bytes),
        Err(CartridgeSaveBackendError::InvalidMbc2NibbleValue {
            index: 0,
            value: 0xFE
        })
    ));
}

#[test]
fn decode_rejects_invalid_boolean_tags_unsupported_tags_and_trailing_bytes() {
    let envelope = CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 123,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 1 },
            },
        },
        persistent_state: PersistentCartState::Mbc1Ram { ram: vec![0xAA] },
    };
    let encoded = encode_cartridge_save_envelope(&envelope).expect("encode should succeed");

    let mut invalid_has_battery = encoded.clone();
    invalid_has_battery[18] = 0x02;
    assert!(matches!(
        decode_cartridge_save_envelope(&invalid_has_battery),
        Err(CartridgeSaveBackendError::InvalidBooleanTag {
            field: "has_battery",
            value: 0x02
        })
    ));

    let mut invalid_profile_tag = encoded.clone();
    invalid_profile_tag[20] = 0xFF;
    assert!(matches!(
        decode_cartridge_save_envelope(&invalid_profile_tag),
        Err(CartridgeSaveBackendError::UnsupportedPersistenceProfileTag { tag: 0xFF })
    ));

    let mut invalid_ram_kind_tag = encoded.clone();
    invalid_ram_kind_tag[21] = 0xFE;
    assert!(matches!(
        decode_cartridge_save_envelope(&invalid_ram_kind_tag),
        Err(CartridgeSaveBackendError::UnsupportedRamPayloadKindTag { tag: 0xFE })
    ));

    let mut invalid_state_tag = encoded.clone();
    invalid_state_tag[26] = 0xFD;
    assert!(matches!(
        decode_cartridge_save_envelope(&invalid_state_tag),
        Err(CartridgeSaveBackendError::UnsupportedPersistentStateTag { tag: 0xFD })
    ));

    let mut trailing_bytes = encoded;
    trailing_bytes.push(0x99);
    assert!(matches!(
        decode_cartridge_save_envelope(&trailing_bytes),
        Err(CartridgeSaveBackendError::TrailingBytes { remaining: 1 })
    ));
}

#[test]
fn encode_and_decode_reject_length_overflows_and_invalid_mbc2_lengths() {
    let overflow_profile = CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 456,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: usize::MAX,
                },
            },
        },
        persistent_state: PersistentCartState::None,
    };
    assert!(matches!(
        encode_cartridge_save_envelope(&overflow_profile),
        Err(CartridgeSaveBackendError::LengthOverflow {
            field: "linear RAM byte_len",
            value: usize::MAX
        })
    ));

    let mut mbc2_bytes = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 789,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 512 },
            },
        },
        persistent_state: PersistentCartState::Mbc2Ram {
            ram_nibbles: [0; MBC2_RAM_NIBBLE_COUNT],
        },
    })
    .expect("encode should succeed");

    let nibble_count_offset = 27;
    mbc2_bytes[nibble_count_offset..nibble_count_offset + 4]
        .copy_from_slice(&(511u32).to_le_bytes());
    assert!(matches!(
        decode_cartridge_save_envelope(&mbc2_bytes),
        Err(CartridgeSaveBackendError::LengthOverflow {
            field: "decoded MBC2 RAM nibble count",
            value: 511
        })
    ));
}
