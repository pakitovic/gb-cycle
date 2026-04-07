use super::*;

#[test]
fn slot_accessors_and_restore_paths_cover_empty_no_mbc_mbc1_and_mbc2_families() {
    let mut empty = CartridgeSlot::empty();
    assert!(empty.is_empty());
    assert_eq!(empty.classification(), None);
    assert_eq!(empty.persistent_state(), PersistentCartState::None);
    assert_eq!(
        empty.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        empty.restore_persistent_state(&PersistentCartState::Mbc1Ram { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc1Ram",
        }),
    );

    let no_mbc_report = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+BATTERY should load");
    let (mut no_mbc, _) = no_mbc_report.into_parts();
    assert!(!no_mbc.is_empty());
    assert_eq!(
        no_mbc
            .classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::NoMbc
        )),
    );
    assert_eq!(
        no_mbc.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: NO_MBC_SUPPORTED_RAM_BYTES,
                },
            },
        },
    );
    no_mbc.write_ram(0xA000, 0x12);
    assert_eq!(no_mbc.read_ram(0xA000), 0x12);
    let no_mbc_before_rtc = no_mbc.persistent_state();
    no_mbc.advance_rtc_seconds(7);
    assert_eq!(no_mbc.persistent_state(), no_mbc_before_rtc);

    let restored_no_mbc = PersistentCartState::NoMbcRam {
        ram: vec![0x5A; NO_MBC_SUPPORTED_RAM_BYTES],
    };
    no_mbc
        .restore_persistent_state(&restored_no_mbc)
        .expect("NoMBC RAM state should restore");
    assert_eq!(no_mbc.persistent_state(), restored_no_mbc);
    assert_eq!(
        no_mbc.restore_persistent_state(&PersistentCartState::NoMbcRam { ram: vec![0; 4] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: NO_MBC_SUPPORTED_RAM_BYTES,
            actual: 4,
        }),
    );
    assert_eq!(
        no_mbc.restore_persistent_state(&PersistentCartState::Mbc1Ram {
            ram: vec![0; NO_MBC_SUPPORTED_RAM_BYTES],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "NoMbcRam",
            actual: "Mbc1Ram",
        }),
    );

    let mbc1_report = CartridgeSlot::load(
        build_banked_mbc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC1 should load");
    let (mut mbc1, _) = mbc1_report.into_parts();
    assert_eq!(
        mbc1.classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::Mbc1
        )),
    );
    mbc1.write_rom(0x0000, 0x0A);
    mbc1.write_ram(0xA000, 0x34);
    assert_eq!(mbc1.read_ram(0xA000), 0x34);
    let restored_mbc1 = PersistentCartState::Mbc1Ram {
        ram: vec![0x77; 32 * 1024],
    };
    mbc1.restore_persistent_state(&restored_mbc1)
        .expect("MBC1 RAM state should restore");
    assert_eq!(mbc1.persistent_state(), restored_mbc1);
    assert_eq!(
        mbc1.restore_persistent_state(&PersistentCartState::Mbc1Ram { ram: vec![0; 8] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 8,
        }),
    );
    assert_eq!(
        mbc1.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc1Ram",
            actual: "None",
        }),
    );

    let mbc2_report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x06, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2+BATTERY should load");
    let (mut mbc2, _) = mbc2_report.into_parts();
    assert_eq!(
        mbc2.classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::Mbc2
        )),
    );
    assert_eq!(
        mbc2.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                    cell_count: MBC2_RAM_CELL_COUNT,
                },
            },
        },
    );
    mbc2.write_rom(0x0000, 0x0A);
    mbc2.write_ram(0xA000, 0xAB);
    assert_eq!(mbc2.read_ram(0xA000), 0xFB);

    let mut restored_nibbles = [0_u8; MBC2_RAM_CELL_COUNT];
    restored_nibbles[0] = 0x0C;
    let restored_mbc2 = PersistentCartState::Mbc2Ram {
        ram_nibbles: restored_nibbles,
    };
    mbc2.restore_persistent_state(&restored_mbc2)
        .expect("MBC2 nibble state should restore");
    assert_eq!(mbc2.persistent_state(), restored_mbc2);
    assert_eq!(
        mbc2.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc2Ram",
            actual: "None",
        }),
    );
}
