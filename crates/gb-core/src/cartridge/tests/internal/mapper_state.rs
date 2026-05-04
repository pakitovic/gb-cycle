use super::*;

#[test]
fn mbc3_private_persistence_and_mapper_helpers_cover_remaining_variants() {
    let no_battery_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x12, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+RAM should load");
    let Some(CartridgeDevice::Mbc3(mut no_battery)) = no_battery_report.cartridge().device.clone()
    else {
        panic!("expected MBC3 cartridge");
    };
    assert!(!no_battery.has_battery());
    assert_eq!(
        no_battery.persistence_metadata(),
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
    assert_eq!(no_battery.persistent_state(), PersistentCartState::None);
    assert_eq!(
        no_battery.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        no_battery.restore_persistent_state(&PersistentCartState::Mbc3Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc3Ram",
        }),
    );
    assert_eq!(no_battery.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    let before_disabled_write = no_battery.ram.clone();
    no_battery.write_ram(0xA000, 0x44);
    assert_eq!(no_battery.ram, before_disabled_write);
    no_battery.write_rom(0x8000, 0x99);
    no_battery.write_rom(0x6000, 0x01);
    assert!(!no_battery.rtc_latch_armed);
    no_battery.write_rom(0x0000, 0x0A);
    no_battery.write_rom(0x4000, 0x05);
    assert_eq!(no_battery.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    no_battery.write_rom(0x4000, 0x08);
    assert_eq!(no_battery.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    no_battery.write_ram(0xA000, 0x12);
    assert_eq!(no_battery.rtc_live.seconds, 0);
    assert_eq!(no_battery.effective_rom_bank(0), 0);

    let rtc_only_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x0F, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+TIMER+BATTERY should load");
    let Some(CartridgeDevice::Mbc3(mut rtc_only)) = rtc_only_report.cartridge().device.clone()
    else {
        panic!("expected RTC-only MBC3 cartridge");
    };
    assert!(rtc_only.has_battery());
    assert_eq!(
        rtc_only.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        }
    );
    assert_eq!(
        rtc_only.persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 0,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        }
    );
    let restored_rtc_only = PersistentCartState::Mbc3Rtc {
        rtc: Mbc3RtcPersistentState {
            seconds: 9,
            minutes: 8,
            hours: 7,
            day_counter: 6,
            halt: true,
            carry: true,
        },
    };
    rtc_only
        .restore_persistent_state(&restored_rtc_only)
        .expect("RTC-only state should restore");
    assert_eq!(rtc_only.persistent_state(), restored_rtc_only);
    assert_eq!(
        rtc_only.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc3Rtc",
            actual: "None",
        }),
    );

    let ram_only_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x13, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+RAM+BATTERY should load");
    let Some(CartridgeDevice::Mbc3(mut ram_only)) = ram_only_report.cartridge().device.clone()
    else {
        panic!("expected RAM-only MBC3 cartridge");
    };
    assert!(ram_only.has_battery());
    assert_eq!(
        ram_only.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        }
    );
    let restored_ram_only = PersistentCartState::Mbc3Ram {
        ram: vec![0x6D; 32 * 1024],
    };
    ram_only
        .restore_persistent_state(&restored_ram_only)
        .expect("RAM-only MBC3 state should restore");
    assert_eq!(ram_only.persistent_state(), restored_ram_only);
    assert_eq!(
        ram_only.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc3Ram",
            actual: "None",
        }),
    );

    let mut ramless_no_rtc = ram_only.clone();
    ramless_no_rtc.ram = None;
    assert_eq!(
        ramless_no_rtc.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        }
    );
    assert_eq!(ramless_no_rtc.persistent_state(), PersistentCartState::None);
    assert_eq!(
        ramless_no_rtc.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        ramless_no_rtc.restore_persistent_state(&PersistentCartState::Mbc3Ram { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc3Ram",
        }),
    );
}

#[test]
fn private_mapper_helpers_cover_remaining_zero_bank_noop_and_none_profile_branches() {
    let mbc1_header = CartridgeHeader::parse(&build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00))
        .expect("header should parse");
    let mut mbc1 = Mbc1Cartridge {
        rom: vec![0; 1024 * 1024],
        ram: None,
        has_battery: false,
        header: mbc1_header,
        classification: supported(0x01, "MBC1M", SupportedCartridgeFamily::Mbc1),
        variant: Mbc1Variant::Mbc1M,
        wiring: Mbc1Wiring::LargeRom,
        ram_enabled: false,
        rom_bank_low5: 0,
        secondary_bank: 2,
        banking_mode: 0,
    };
    let mbc1_before_noop = mbc1.clone();
    mbc1.write_rom(0x8000, 0xFF);
    assert_eq!(mbc1, mbc1_before_noop);
    assert_eq!(mbc1.effective_high_rom_bank(0), 0);
    assert_eq!(mbc1.effective_low_rom_bank(0), 0);
    assert_eq!(mbc1.effective_low_rom_bank(64), 0);
    assert_eq!(mbc1.effective_ram_offset(0xA123), 0x0123);
    assert_eq!(
        mbc1.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        },
    );

    let mbc2_header = CartridgeHeader::parse(&build_banked_mbc2_rom(0x05, 0x03, 0x00))
        .expect("header should parse");
    let mbc2 = Mbc2Cartridge {
        rom: vec![0; 256 * 1024],
        ram_nibbles: [0; MBC2_RAM_CELL_COUNT],
        has_battery: false,
        header: mbc2_header,
        classification: CartridgeClassification::classify(0x05),
        ram_enabled: false,
        rom_bank_low4: 0,
    };
    assert_eq!(mbc2.effective_high_rom_bank(0), 0);

    let mbc3_header = CartridgeHeader::parse(&build_banked_mbc3_rom(0x13, 0x03, 0x03))
        .expect("header should parse");
    let mut mbc3 = Mbc3Cartridge {
        rom: vec![0; 256 * 1024],
        ram: Some(vec![0x11; 32 * 1024]),
        has_battery: true,
        has_rtc: false,
        header: mbc3_header,
        classification: CartridgeClassification::classify(0x13),
        variant: Mbc3Variant::Standard,
        ram_rtc_enabled: true,
        rom_bank: 0,
        ram_or_rtc_select: Mbc3RamRtcSelect::ReservedSelector(0x05),
        rtc_live: Mbc3RtcState::default(),
        rtc_latched: Mbc3RtcState::default(),
        rtc_latched_valid: false,
        rtc_latch_armed: false,
        rtc_subsecond_ticks: 0,
        rtc_access_ready_at: None,
    };
    mbc3.write_ram(0xA000, 0x77);
    assert_eq!(mbc3.ram.as_ref().expect("RAM should exist")[0], 0x11);
    assert_eq!(
        mbc3.restore_persistent_state(&PersistentCartState::Mbc3Ram { ram: vec![0; 8] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 8,
        }),
    );

    let rtc_only_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x0F, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("RTC-only MBC3 should load");
    assert_eq!(
        rtc_only_report.cartridge().persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        },
    );

    let mbc5_header = CartridgeHeader::parse(&build_banked_mbc5_rom(0x19, 0x03, 0x00))
        .expect("header should parse");
    let mut mbc5 = Mbc5Cartridge {
        rom: vec![0; 256 * 1024],
        ram: None,
        has_battery: false,
        has_rumble: false,
        header: mbc5_header,
        classification: CartridgeClassification::classify(0x19),
        variant: Mbc5Variant::NoRam,
        ram_enabled: false,
        rom_bank_low8: 0,
        rom_bank_high1: 0,
        ram_bank_raw: 0,
        rumble_on: false,
    };
    let mbc5_before_noop = mbc5.clone();
    mbc5.write_rom(0x8000, 0xFF);
    assert_eq!(mbc5, mbc5_before_noop);
    assert_eq!(mbc5.effective_rom_bank(0), 0);
}
