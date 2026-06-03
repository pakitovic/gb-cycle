use super::*;

#[test]
fn external_save_exports_linear_ram_as_raw_bytes() {
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 4 },
        },
    };
    let state = PersistentCartState::Mbc1Ram {
        ram: vec![0x10, 0x20, 0x30, 0x40],
    };

    let external = encode_external_cartridge_save(
        metadata,
        &state,
        1_700_000_000,
        ExternalSaveExportFormat::default(),
    )
    .expect("linear RAM should export");
    assert_eq!(external, [0x10, 0x20, 0x30, 0x40]);

    let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_001)
        .expect("linear RAM should import");
    assert_eq!(imported, state);
}

#[test]
fn external_save_round_trips_all_linear_ram_state_kinds() {
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let states = [
        PersistentCartState::NoMbcRam {
            ram: vec![0x01, 0x02],
        },
        PersistentCartState::Mmm01Ram {
            ram: vec![0x03, 0x04],
        },
        PersistentCartState::Huc1Ram {
            ram: vec![0x05, 0x06],
        },
        PersistentCartState::Mbc3Ram {
            ram: vec![0x07, 0x08],
        },
        PersistentCartState::Mbc5Ram {
            ram: vec![0x09, 0x0A],
        },
        PersistentCartState::PocketCameraRam {
            ram: vec![0x0B, 0x0C],
        },
    ];

    for state in states {
        let external = encode_external_cartridge_save(
            metadata,
            &state,
            1_700_000_000,
            ExternalSaveExportFormat::default(),
        )
        .expect("linear RAM state should export");
        let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_001)
            .expect("linear RAM state should import");
        assert_eq!(imported, state);
    }
}

#[test]
fn external_save_round_trips_mbc7_raw_eeprom_without_battery_flag() {
    let metadata = CartridgePersistenceMetadata {
        has_battery: false,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentEeprom { byte_len: 256 },
    };
    let mut eeprom = vec![0xFF; 256];
    eeprom[0] = 0x12;
    eeprom[1] = 0x34;
    eeprom[254] = 0xAB;
    eeprom[255] = 0xCD;
    let state = PersistentCartState::Mbc7Eeprom {
        eeprom: eeprom.clone(),
    };

    let external = encode_external_cartridge_save(
        metadata,
        &state,
        1_700_000_000,
        ExternalSaveExportFormat::default(),
    )
    .expect("MBC7 EEPROM should export as a raw .sav payload");
    assert_eq!(external, eeprom);

    let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_000)
        .expect("MBC7 EEPROM should import from raw .sav bytes");
    assert_eq!(imported, state);
}
