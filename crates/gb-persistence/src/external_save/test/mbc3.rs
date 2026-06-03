use super::*;

#[test]
fn external_save_round_trips_mbc3_rtc_only_suffix() {
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRtc,
    };
    let state = PersistentCartState::Mbc3Rtc {
        rtc: Mbc3RtcPersistentState {
            seconds: 1,
            minutes: 2,
            hours: 3,
            day_counter: 4,
            halt: false,
            carry: false,
        },
    };
    let external = encode_external_cartridge_save(
        metadata,
        &state,
        1_700_000_010,
        ExternalSaveExportFormat::default(),
    )
    .expect("MBC3 RTC-only state should export");
    assert_eq!(external.len(), MBC3_EXTERNAL_RTC_SUFFIX_LEN);

    let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_012)
        .expect("MBC3 RTC-only state should import");
    assert_eq!(
        imported,
        PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 3,
                minutes: 2,
                hours: 3,
                day_counter: 4,
                halt: false,
                carry: false,
            },
        }
    );
}

#[test]
fn external_save_round_trips_mbc3_rtc_suffix_with_elapsed_time() {
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let state = PersistentCartState::Mbc3RamRtc {
        ram: vec![0xAB, 0xCD],
        rtc: Mbc3RtcPersistentState {
            seconds: 58,
            minutes: 59,
            hours: 23,
            day_counter: 7,
            halt: false,
            carry: false,
        },
    };

    let envelope = CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 100,
        },
        cartridge_metadata: metadata,
        persistent_state: state.clone(),
    };
    let external =
        export_external_cartridge_save(&envelope, 103).expect("MBC3 RAM+RTC should export");
    assert_eq!(external.len(), 2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN);
    assert_eq!(&external[..2], &[0xAB, 0xCD]);
    assert_eq!(external[2], 1);
    assert_eq!(external[6], 0);
    assert_eq!(external[10], 0);
    assert_eq!(
        u64::from_le_bytes(external[42..50].try_into().unwrap()),
        103
    );

    let imported = import_external_cartridge_save(metadata, &state, &external, 105)
        .expect("MBC3 RAM+RTC should import");
    assert_eq!(
        imported,
        PersistentCartState::Mbc3RamRtc {
            ram: vec![0xAB, 0xCD],
            rtc: Mbc3RtcPersistentState {
                seconds: 3,
                minutes: 0,
                hours: 0,
                day_counter: 8,
                halt: false,
                carry: false,
            },
        }
    );
}

#[test]
fn external_save_imports_mbc3_rtc_suffixes_with_32_bit_timestamps() {
    let rtc_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRtc,
    };
    let rtc_state = PersistentCartState::Mbc3Rtc {
        rtc: Mbc3RtcPersistentState {
            seconds: 1,
            minutes: 2,
            hours: 3,
            day_counter: 4,
            halt: false,
            carry: false,
        },
    };
    let mut rtc_external = encode_external_cartridge_save(
        rtc_metadata,
        &rtc_state,
        1_700_000_010,
        ExternalSaveExportFormat::default(),
    )
    .expect("MBC3 RTC-only state should export");
    assert_eq!(rtc_external.len(), MBC3_EXTERNAL_RTC_SUFFIX_LEN);
    rtc_external.truncate(MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP);

    let rtc_imported =
        import_external_cartridge_save(rtc_metadata, &rtc_state, &rtc_external, 1_700_000_012)
            .expect("MBC3 RTC-only 32-bit timestamp suffix should import");
    assert_eq!(
        rtc_imported,
        PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 3,
                minutes: 2,
                hours: 3,
                day_counter: 4,
                halt: false,
                carry: false,
            },
        }
    );

    let ram_rtc_metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let ram_rtc_state = PersistentCartState::Mbc3RamRtc {
        ram: vec![0xAB, 0xCD],
        rtc: Mbc3RtcPersistentState {
            seconds: 58,
            minutes: 59,
            hours: 23,
            day_counter: 7,
            halt: false,
            carry: false,
        },
    };
    let envelope = CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 100,
        },
        cartridge_metadata: ram_rtc_metadata,
        persistent_state: ram_rtc_state.clone(),
    };
    let mut ram_rtc_external =
        export_external_cartridge_save(&envelope, 103).expect("MBC3 RAM+RTC should export");
    assert_eq!(ram_rtc_external.len(), 2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN);
    ram_rtc_external.truncate(2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP);

    let ram_rtc_imported =
        import_external_cartridge_save(ram_rtc_metadata, &ram_rtc_state, &ram_rtc_external, 105)
            .expect("MBC3 RAM+RTC 32-bit timestamp suffix should import");
    assert_eq!(
        ram_rtc_imported,
        PersistentCartState::Mbc3RamRtc {
            ram: vec![0xAB, 0xCD],
            rtc: Mbc3RtcPersistentState {
                seconds: 3,
                minutes: 0,
                hours: 0,
                day_counter: 8,
                halt: false,
                carry: false,
            },
        }
    );
}

#[test]
fn external_save_round_trips_mbc30_sized_ram_plus_mbc3_rtc_suffix() {
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 64 * 1024,
            },
        },
    };
    let mut ram = vec![0; 64 * 1024];
    ram[0] = 0x30;
    ram[0x3FFF] = 0x3F;
    ram[0x8000] = 0x80;
    ram[0xFFFF] = 0xFF;
    let state = PersistentCartState::Mbc3RamRtc {
        ram: ram.clone(),
        rtc: Mbc3RtcPersistentState {
            seconds: 7,
            minutes: 8,
            hours: 9,
            day_counter: 10,
            halt: false,
            carry: false,
        },
    };

    let external = encode_external_cartridge_save(
        metadata,
        &state,
        1_700_000_000,
        ExternalSaveExportFormat::default(),
    )
    .expect("MBC30-sized MBC3 RAM+RTC should export");
    assert_eq!(external.len(), 64 * 1024 + MBC3_EXTERNAL_RTC_SUFFIX_LEN);

    let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_000)
        .expect("MBC30-sized MBC3 RAM+RTC should import");
    assert_eq!(imported, state);
}
