use super::*;

#[test]
fn persistence_metadata_keeps_ram_shapes_and_battery_policy_explicit() {
    let no_mbc_report = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+BATTERY should load");
    assert_eq!(
        no_mbc_report.cartridge().persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: NO_MBC_SUPPORTED_RAM_BYTES,
                },
            },
        }
    );

    let mbc2_report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x05, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2 should load");
    assert_eq!(
        mbc2_report.cartridge().persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                    cell_count: MBC2_RAM_CELL_COUNT,
                },
            },
        }
    );
}

#[test]
fn non_battery_no_mbc_and_mbc5_profiles_stay_nonpersistent() {
    let no_mbc_report = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+RAM should load");
    let Some(CartridgeDevice::NoMbc(mut no_mbc)) = no_mbc_report.cartridge().device.clone() else {
        panic!("expected NoMBC cartridge");
    };

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
        },
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
    no_mbc.write_rom(0x1234, 0xAA);
    assert_eq!(no_mbc.read_rom(0x8000), RAM_ABSENT_READ_VALUE);

    let mbc5_report = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1A, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RAM should load");
    let Some(CartridgeDevice::Mbc5(mut mbc5)) = mbc5_report.cartridge().device.clone() else {
        panic!("expected MBC5 cartridge");
    };

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
        },
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

    let no_ram_rumble_report = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1C, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RUMBLE should load");
    let Some(CartridgeDevice::Mbc5(no_ram_rumble)) =
        no_ram_rumble_report.cartridge().device.clone()
    else {
        panic!("expected MBC5 cartridge");
    };

    assert_eq!(
        no_ram_rumble.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        },
    );
    assert!(no_ram_rumble.has_rumble());
    assert_eq!(no_ram_rumble.persistent_state(), PersistentCartState::None);
}
