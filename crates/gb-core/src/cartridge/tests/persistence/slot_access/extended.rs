use super::*;

#[test]
fn slot_accessors_and_restore_paths_cover_mbc3_and_mbc5_rtc_and_rumble_paths() {
    let mbc3_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x10, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+TIMER+RAM+BATTERY should load");
    let (mut mbc3, _) = mbc3_report.into_parts();
    assert_eq!(
        mbc3.classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::Mbc3
        )),
    );
    assert_eq!(
        mbc3.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        },
    );
    mbc3.advance_rtc_seconds(3_661);
    assert_eq!(
        mbc3.persistent_state(),
        PersistentCartState::Mbc3RamRtc {
            ram: vec![0; 32 * 1024],
            rtc: Mbc3RtcPersistentState {
                seconds: 1,
                minutes: 1,
                hours: 1,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );

    let restored_mbc3 = PersistentCartState::Mbc3RamRtc {
        ram: vec![0x6B; 32 * 1024],
        rtc: Mbc3RtcPersistentState {
            seconds: 9,
            minutes: 8,
            hours: 7,
            day_counter: 6,
            halt: true,
            carry: true,
        },
    };
    mbc3.restore_persistent_state(&restored_mbc3)
        .expect("MBC3 RAM+RTC state should restore");
    assert_eq!(mbc3.persistent_state(), restored_mbc3);
    assert_eq!(
        mbc3.restore_persistent_state(&PersistentCartState::Mbc3Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc3RamRtc",
            actual: "Mbc3Ram",
        }),
    );
    assert_eq!(
        mbc3.restore_persistent_state(&PersistentCartState::Mbc3RamRtc {
            ram: vec![0; 4],
            rtc: Mbc3RtcPersistentState {
                seconds: 0,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 4,
        }),
    );

    let mbc5_report = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1E, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RUMBLE+RAM+BATTERY should load");
    let (mut mbc5, _) = mbc5_report.into_parts();
    assert_eq!(
        mbc5.classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::Mbc5
        )),
    );
    assert!(mbc5.has_rumble());
    assert!(!mbc5.rumble_on());
    mbc5.advance_rtc_seconds(99);
    assert!(!mbc5.rumble_on());
    mbc5.write_rom(0x0000, 0x0A);
    mbc5.write_rom(0x4000, 0x0B);
    mbc5.write_ram(0xA000, 0x44);
    assert!(mbc5.rumble_on());
    assert_eq!(mbc5.read_ram(0xA000), 0x44);
    assert_eq!(
        mbc5.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        },
    );

    let restored_mbc5 = PersistentCartState::Mbc5Ram {
        ram: vec![0x24; 32 * 1024],
    };
    mbc5.restore_persistent_state(&restored_mbc5)
        .expect("MBC5 RAM state should restore");
    assert_eq!(mbc5.persistent_state(), restored_mbc5);
    assert_eq!(
        mbc5.restore_persistent_state(&PersistentCartState::NoMbcRam {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc5Ram",
            actual: "NoMbcRam",
        }),
    );
    assert_eq!(
        mbc5.restore_persistent_state(&PersistentCartState::Mbc5Ram { ram: vec![0; 8] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 8,
        }),
    );
    let empty = CartridgeSlot::empty();
    assert!(!empty.has_rumble());
}

#[test]
fn slot_accessors_and_restore_paths_cover_mmm01_battery_ram() {
    let mmm01_report = CartridgeSlot::load(
        build_mmm01_rom(0x03, 0x03, 0x0D),
        &CompatibilityPolicy::strict(),
    )
    .expect("MMM01+RAM+BATTERY should load");
    let (mut mmm01, _) = mmm01_report.into_parts();
    assert_eq!(
        mmm01
            .classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::Mmm01
        )),
    );
    assert_eq!(
        mmm01.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        },
    );

    mmm01.write_rom(0x4000, 0x02);
    mmm01.write_rom(0x0000, 0x2A);
    mmm01.write_rom(0x0000, 0x6A);
    mmm01.write_ram(0xA000, 0x11);
    mmm01.write_rom(0x6000, 0x01);
    mmm01.write_ram(0xA000, 0x22);
    mmm01.write_rom(0x4000, 0x03);
    mmm01.write_ram(0xA000, 0x33);

    let restored_mmm01 = match mmm01.persistent_state() {
        PersistentCartState::Mmm01Ram { ram } => PersistentCartState::Mmm01Ram { ram },
        other => panic!("expected MMM01 RAM state, got {other:?}"),
    };
    mmm01
        .restore_persistent_state(&restored_mmm01)
        .expect("MMM01 RAM state should restore");
    assert_eq!(mmm01.persistent_state(), restored_mmm01);
    assert_eq!(
        mmm01.restore_persistent_state(&PersistentCartState::Mbc1Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mmm01Ram",
            actual: "Mbc1Ram",
        }),
    );
    assert_eq!(
        mmm01.restore_persistent_state(&PersistentCartState::Mmm01Ram { ram: vec![0; 8] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 8,
        }),
    );

    let mut restored = CartridgeSlot::load(
        build_mmm01_rom(0x03, 0x03, 0x0D),
        &CompatibilityPolicy::strict(),
    )
    .expect("MMM01+RAM+BATTERY should load")
    .into_parts()
    .0;
    restored
        .restore_persistent_state(&restored_mmm01)
        .expect("separate MMM01 slot should accept the persisted payload");

    restored.write_rom(0x4000, 0x02);
    restored.write_rom(0x0000, 0x2A);
    restored.write_rom(0x0000, 0x6A);
    assert_eq!(restored.read_ram(0xA000), 0x22);
    restored.write_rom(0x6000, 0x01);
    assert_eq!(restored.read_ram(0xA000), 0x22);
    restored.write_rom(0x4000, 0x03);
    assert_eq!(restored.read_ram(0xA000), 0x33);
}
