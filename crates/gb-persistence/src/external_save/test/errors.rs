use super::*;

#[test]
fn external_save_rejects_ambiguous_or_invalid_payloads() {
    let linear_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let linear_state = PersistentCartState::NoMbcRam { ram: vec![0xAA] };
    assert!(matches!(
        encode_external_cartridge_save(
            linear_metadata,
            &linear_state,
            0,
            ExternalSaveExportFormat::default(),
        ),
        Err(ExternalSaveError::InvalidLength {
            context: "linear RAM state",
            ..
        })
    ));
    assert!(matches!(
        import_external_cartridge_save(
            linear_metadata,
            &PersistentCartState::NoMbcRam { ram: vec![0; 2] },
            &[0xAA],
            0,
        ),
        Err(ExternalSaveError::InvalidLength {
            context: "linear RAM",
            ..
        })
    ));

    let mbc2_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                cell_count: MBC2_RAM_NIBBLE_COUNT,
            },
        },
    };
    let mbc2_state = PersistentCartState::Mbc2Ram {
        ram_nibbles: [0; MBC2_RAM_NIBBLE_COUNT],
    };
    assert!(matches!(
        import_external_cartridge_save(mbc2_metadata, &mbc2_state, &[0; 257], 0),
        Err(ExternalSaveError::InvalidLength {
            context: "MBC2 RAM",
            ..
        })
    ));
    let invalid_mbc2_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 511 },
        },
    };
    assert!(matches!(
        encode_external_cartridge_save(
            invalid_mbc2_metadata,
            &mbc2_state,
            0,
            ExternalSaveExportFormat::default(),
        ),
        Err(ExternalSaveError::InvalidLength {
            context: "MBC2 metadata",
            ..
        })
    ));
    assert!(matches!(
        import_external_cartridge_save(invalid_mbc2_metadata, &mbc2_state, &[0; 256], 0),
        Err(ExternalSaveError::InvalidLength {
            context: "MBC2 metadata",
            ..
        })
    ));

    let mbc3_rtc_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRtc,
    };
    let mbc3_rtc_state = PersistentCartState::Mbc3Rtc {
        rtc: Mbc3RtcPersistentState {
            seconds: 0,
            minutes: 0,
            hours: 0,
            day_counter: 0,
            halt: false,
            carry: false,
        },
    };
    assert!(matches!(
        import_external_cartridge_save(
            mbc3_rtc_metadata,
            &mbc3_rtc_state,
            &[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN - 1],
            0,
        ),
        Err(ExternalSaveError::InvalidLength {
            context: "MBC3 RTC",
            expected: ExternalSaveLengthExpectation::Either {
                first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
                second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
            },
            actual,
        }) if actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN - 1
    ));
    assert!(matches!(
        import_external_cartridge_save(
            mbc3_rtc_metadata,
            &mbc3_rtc_state,
            &[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP - 1],
            0,
        ),
        Err(ExternalSaveError::InvalidLength {
            context: "MBC3 RTC",
            expected: ExternalSaveLengthExpectation::Either {
                first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
                second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
            },
            actual,
        }) if actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP - 1
    ));
    assert!(matches!(
        decode_external_mbc3_rtc_suffix(
            &[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP - 1],
            0
        ),
        Err(ExternalSaveError::InvalidLength {
            context: "MBC3 RTC",
            expected: ExternalSaveLengthExpectation::Either {
                first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
                second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
            },
            actual,
        }) if actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP - 1
    ));
    assert!(matches!(
        decode_external_mbc3_rtc_suffix(&[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN - 1], 0),
        Err(ExternalSaveError::InvalidLength {
            context: "MBC3 RTC",
            expected: ExternalSaveLengthExpectation::Either {
                first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
                second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
            },
            actual,
        }) if actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN - 1
    ));

    let ram_rtc_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let ram_rtc_state = PersistentCartState::Mbc3RamRtc {
        ram: vec![0; 2],
        rtc: Mbc3RtcPersistentState {
            seconds: 0,
            minutes: 0,
            hours: 0,
            day_counter: 0,
            halt: false,
            carry: false,
        },
    };
    assert!(matches!(
        import_external_cartridge_save(
            ram_rtc_metadata,
            &ram_rtc_state,
            &[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN + 1],
            0,
        ),
        Err(ExternalSaveError::InvalidLength {
            context: "MBC3 RAM+RTC",
            expected: ExternalSaveLengthExpectation::Either {
                first,
                second,
            },
            actual,
        }) if first == 2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP
            && second == 2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN
            && actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN + 1
    ));

    let unsupported_profile = CartridgePersistenceMetadata {
        has_battery: false,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 1 },
        },
    };
    assert!(matches!(
        encode_external_cartridge_save(
            unsupported_profile,
            &PersistentCartState::NoMbcRam { ram: vec![0] },
            0,
            ExternalSaveExportFormat::default(),
        ),
        Err(ExternalSaveError::UnsupportedPersistenceProfile { .. })
    ));
    assert!(matches!(
        import_external_cartridge_save(
            unsupported_profile,
            &PersistentCartState::NoMbcRam { ram: vec![0] },
            &[0],
            0,
        ),
        Err(ExternalSaveError::UnsupportedPersistenceProfile { .. })
    ));

    let unsupported_mbc2_rtc_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                cell_count: MBC2_RAM_NIBBLE_COUNT,
            },
        },
    };
    assert!(matches!(
        encode_external_cartridge_save(
            unsupported_mbc2_rtc_metadata,
            &ram_rtc_state,
            0,
            ExternalSaveExportFormat::default(),
        ),
        Err(ExternalSaveError::UnsupportedPersistenceProfile { .. })
    ));
    assert!(matches!(
        import_external_cartridge_save(unsupported_mbc2_rtc_metadata, &ram_rtc_state, &[0; 2], 0,),
        Err(ExternalSaveError::UnsupportedPersistenceProfile { .. })
    ));

    let huc3_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let huc3_state = PersistentCartState::Huc3 {
        ram: vec![0; 2],
        mcu_ram: [0; 256],
        rtc: Huc3RtcPersistentState {
            current_minutes_of_day: 0,
            current_days: 0,
            current_subminute_seconds: 0,
            event_minutes_of_day: 0,
            event_days: 0,
        },
        rom_bank: 0,
        ram_bank: 0,
        select_mode: 0,
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
    };
    assert!(matches!(
        encode_external_cartridge_save(
            huc3_metadata,
            &huc3_state,
            0,
            ExternalSaveExportFormat::default(),
        ),
        Err(ExternalSaveError::UnsupportedPersistentState { state_kind: "Huc3" })
    ));
    assert!(matches!(
        import_external_cartridge_save(huc3_metadata, &huc3_state, &[0; 50], 0),
        Err(ExternalSaveError::UnsupportedPersistentState { state_kind: "Huc3" })
    ));
    assert!(matches!(
        encode_external_cartridge_save(
            linear_metadata,
            &huc3_state,
            0,
            ExternalSaveExportFormat::default(),
        ),
        Err(ExternalSaveError::UnsupportedPersistentState { state_kind: "Huc3" })
    ));
    assert!(matches!(
        encode_external_cartridge_save(
            linear_metadata,
            &mbc2_state,
            0,
            ExternalSaveExportFormat::default(),
        ),
        Err(ExternalSaveError::StateProfileMismatch {
            state_kind: "Mbc2Ram",
            ..
        })
    ));
    assert!(matches!(
        import_external_cartridge_save(linear_metadata, &mbc2_state, &[0; 2], 0),
        Err(ExternalSaveError::StateProfileMismatch {
            state_kind: "Mbc2Ram",
            ..
        })
    ));

    let mbc6_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRamAndFlash {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
            flash_byte_len: 4,
            hidden_byte_len: 2,
        },
    };
    let protected_mbc6_state = PersistentCartState::Mbc6 {
        ram: vec![0; 2],
        flash: vec![0xFF; 4],
        hidden_region: vec![0xFF; 2],
        sector0_protected: true,
    };
    assert!(matches!(
        encode_external_cartridge_save(
            mbc6_metadata,
            &protected_mbc6_state,
            0,
            ExternalSaveExportFormat::default(),
        ),
        Err(ExternalSaveError::UnsupportedStateShape {
            state_kind: "Mbc6",
            ..
        })
    ));
    assert!(matches!(
        import_external_cartridge_save(mbc6_metadata, &protected_mbc6_state, &[0; 6], 0),
        Err(ExternalSaveError::UnsupportedStateShape {
            state_kind: "Mbc6",
            ..
        })
    ));
    let mbc6_state = PersistentCartState::Mbc6 {
        ram: vec![0; 2],
        flash: vec![0xFF; 4],
        hidden_region: vec![0xFF; 2],
        sector0_protected: false,
    };
    assert!(matches!(
        import_external_cartridge_save(mbc6_metadata, &mbc6_state, &[0; 5], 0),
        Err(ExternalSaveError::InvalidLength {
            context: "MBC6 RAM+flash",
            ..
        })
    ));
}
