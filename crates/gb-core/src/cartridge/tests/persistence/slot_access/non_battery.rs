use super::*;

#[test]
fn slot_accessors_and_restore_paths_cover_non_battery_and_ramless_variants() {
    let no_mbc_report = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+RAM should load");
    assert!(!no_mbc_report.cartridge().rumble_on());
    let Some(CartridgeDevice::NoMbc(mut no_mbc)) = no_mbc_report.cartridge().device.clone() else {
        panic!("expected NoMBC cartridge");
    };
    assert!(!no_mbc.has_battery());
    assert_eq!(
        no_mbc.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: NO_MBC_SUPPORTED_RAM_BYTES,
                },
            },
        }
    );
    assert_eq!(no_mbc.persistent_state(), PersistentCartState::None);
    assert_eq!(
        no_mbc.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        no_mbc.restore_persistent_state(&PersistentCartState::NoMbcRam {
            ram: vec![0; NO_MBC_SUPPORTED_RAM_BYTES],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "NoMbcRam",
        }),
    );

    let no_mbc_battery_report = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+RAM+BATTERY should load");
    let Some(CartridgeDevice::NoMbc(mut no_mbc_battery)) =
        no_mbc_battery_report.cartridge().device.clone()
    else {
        panic!("expected NoMBC battery cartridge");
    };
    no_mbc_battery.ram = None;
    assert!(no_mbc_battery.has_battery());
    assert_eq!(
        no_mbc_battery.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        no_mbc_battery.restore_persistent_state(&PersistentCartState::NoMbcRam { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "NoMbcRam",
        }),
    );

    let mbc1_report = CartridgeSlot::load(
        build_banked_mbc1_rom_with_type(0x02, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC1+RAM should load");
    let Some(CartridgeDevice::Mbc1(mut mbc1)) = mbc1_report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };
    assert!(!mbc1.has_battery());
    assert_eq!(
        mbc1.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        }
    );
    assert_eq!(mbc1.persistent_state(), PersistentCartState::None);
    assert_eq!(
        mbc1.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc1.restore_persistent_state(&PersistentCartState::Mbc1Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc1Ram",
        }),
    );

    let mbc1_battery_report = CartridgeSlot::load(
        build_banked_mbc1_rom_with_type(0x03, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC1+RAM+BATTERY should load");
    let Some(CartridgeDevice::Mbc1(mut mbc1_battery)) =
        mbc1_battery_report.cartridge().device.clone()
    else {
        panic!("expected MBC1 battery cartridge");
    };
    mbc1_battery.ram = None;
    assert!(mbc1_battery.has_battery());
    assert_eq!(
        mbc1_battery.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc1_battery.restore_persistent_state(&PersistentCartState::Mbc1Ram { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc1Ram",
        }),
    );

    let mbc2_report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x05, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2 should load");
    let Some(CartridgeDevice::Mbc2(mut mbc2)) = mbc2_report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };
    assert!(!mbc2.has_battery());
    assert_eq!(mbc2.persistent_state(), PersistentCartState::None);
    assert_eq!(
        mbc2.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc2.restore_persistent_state(&PersistentCartState::Mbc2Ram {
            ram_nibbles: [0; MBC2_RAM_CELL_COUNT],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc2Ram",
        }),
    );

    let mbc5_report = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1A, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RAM should load");
    let Some(CartridgeDevice::Mbc5(mut mbc5)) = mbc5_report.cartridge().device.clone() else {
        panic!("expected MBC5 cartridge");
    };
    assert!(!mbc5.has_battery());
    assert_eq!(
        mbc5.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        }
    );
    assert_eq!(mbc5.persistent_state(), PersistentCartState::None);
    assert_eq!(
        mbc5.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc5.restore_persistent_state(&PersistentCartState::Mbc5Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc5Ram",
        }),
    );

    let mbc5_battery_report = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1B, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RAM+BATTERY should load");
    let Some(CartridgeDevice::Mbc5(mut mbc5_battery)) =
        mbc5_battery_report.cartridge().device.clone()
    else {
        panic!("expected MBC5 battery cartridge");
    };
    mbc5_battery.ram = None;
    assert!(mbc5_battery.has_battery());
    assert_eq!(
        mbc5_battery.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        }
    );
    assert_eq!(
        mbc5_battery.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc5_battery.restore_persistent_state(&PersistentCartState::Mbc5Ram { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc5Ram",
        }),
    );
}
