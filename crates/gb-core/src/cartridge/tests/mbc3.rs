use super::*;
use crate::scheduler::TCycle;

#[test]
fn mbc3_power_up_state_is_explicit_and_starts_the_high_window_on_bank_one() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");

    let Some(CartridgeDevice::Mbc3(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC3 cartridge");
    };

    assert_eq!(cartridge.variant, Mbc3Variant::Standard);
    assert!(cartridge.has_rtc);
    assert!(!cartridge.ram_rtc_enabled);
    assert_eq!(cartridge.rom_bank, 0);
    assert_eq!(cartridge.ram_or_rtc_select, Mbc3RamRtcSelect::RamBank(0));
    assert!(!cartridge.rtc_latched_valid);
    assert!(!cartridge.rtc_latch_armed);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
}

#[test]
fn mbc3_reaches_banks_0x20_0x40_and_0x60_without_mbc1_style_anomalies() {
    let rom = build_banked_mbc3_rom(0x13, 0x06, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    for bank in [0x20, 0x40, 0x60] {
        cartridge.write_rom(0x2000, bank);
        assert_eq!(cartridge.read_rom(0x4000), bank);
    }
}

#[test]
fn mbc3_selector_keeps_ram_reserved_and_rtc_targets_distinct() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x4000, 0x02);
    assert_eq!(cartridge.ram_or_rtc_select, Mbc3RamRtcSelect::RamBank(0x02));

    cartridge.write_rom(0x4000, 0x05);
    assert_eq!(
        cartridge.ram_or_rtc_select,
        Mbc3RamRtcSelect::ReservedSelector(0x05)
    );

    cartridge.write_rom(0x4000, 0x0C);
    assert_eq!(
        cartridge.ram_or_rtc_select,
        Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::DayHigh)
    );
}

#[test]
fn mbc3_selector_ignores_upper_data_bits_and_decodes_from_the_low_nibble() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x4000, 0x12);
    assert_eq!(cartridge.ram_or_rtc_select, Mbc3RamRtcSelect::RamBank(0x02));

    cartridge.write_rom(0x4000, 0x1C);
    assert_eq!(
        cartridge.ram_or_rtc_select,
        Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::DayHigh)
    );

    cartridge.write_rom(0x4000, 0x17);
    assert_eq!(
        cartridge.ram_or_rtc_select,
        Mbc3RamRtcSelect::ReservedSelector(0x07)
    );
}

#[test]
fn mbc3_reserved_selectors_do_not_alias_ram_banks() {
    let rom = build_banked_mbc3_rom(0x13, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);

    for bank in 0x00..=0x03 {
        cartridge.write_rom(0x4000, bank);
        cartridge.write_ram(0xA000, 0xA0 | bank);
    }

    for selector in 0x04..=0x07 {
        cartridge.write_rom(0x4000, selector);
        cartridge.write_ram(0xA000, selector);
        assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    }

    for bank in 0x00..=0x03 {
        cartridge.write_rom(0x4000, bank);
        assert_eq!(cartridge.read_ram(0xA000), 0xA0 | bank);
    }
}

#[test]
fn mbc3_timed_rtc_accesses_record_the_recommended_ready_t_cycle() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    assert_eq!(cartridge.rtc_access_ready_at, None);

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x08);
    cartridge.write_ram_timed(0xA000, 0x12, TCycle::new(100));
    assert_eq!(cartridge.rtc_access_ready_at, Some(TCycle::new(116)));

    assert_eq!(cartridge.read_ram_timed(0xA000, TCycle::new(140)), 0x00);
    assert_eq!(cartridge.rtc_access_ready_at, Some(TCycle::new(156)));

    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_ram_timed(0xA000, 0x44, TCycle::new(200));
    assert_eq!(cartridge.rtc_access_ready_at, Some(TCycle::new(156)));

    cartridge.write_rom(0x4000, 0x05);
    assert_eq!(
        cartridge.read_ram_timed(0xA000, TCycle::new(220)),
        RAM_ABSENT_READ_VALUE
    );
    assert_eq!(cartridge.rtc_access_ready_at, Some(TCycle::new(156)));
}

#[test]
fn strict_validation_admits_mbc3_headers_with_2kib_ram_metadata() {
    let rom = build_banked_mbc3_rom(0x13, 0x00, 0x01);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
}

#[test]
fn mbc3_rtc_latch_reads_from_snapshot_while_writes_hit_live_state() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.advance_rtc_seconds(93_784);
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x08);

    assert_eq!(cartridge.read_ram(0xA000), 0x00);

    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x00);

    cartridge.write_rom(0x6000, 0x00);
    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x04);

    cartridge.advance_rtc_seconds(1);
    assert_eq!(cartridge.read_ram(0xA000), 0x04);

    cartridge.write_rom(0x6000, 0x00);
    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x05);

    cartridge.write_ram(0xA000, 0x2A);
    assert_eq!(cartridge.rtc_live.seconds, 0x2A);
    assert_eq!(cartridge.read_ram(0xA000), 0x05);

    cartridge.write_rom(0x6000, 0x55);
    assert_eq!(cartridge.read_ram(0xA000), 0x2A);
}

#[test]
fn mbc3_rtc_reads_use_an_explicit_zero_snapshot_before_the_first_valid_latch() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x08);
    cartridge.write_ram(0xA000, 0x2A);

    assert!(!cartridge.rtc_latched_valid);
    assert_eq!(cartridge.read_ram(0xA000), 0x00);
}

#[test]
fn mbc3_rtc_register_writes_echo_raw_bytes_until_time_advances() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);

    cartridge.write_rom(0x4000, 0x08);
    cartridge.write_ram(0xA000, 0x74);

    cartridge.write_rom(0x4000, 0x09);
    cartridge.write_ram(0xA000, 0xF2);

    cartridge.write_rom(0x4000, 0x0A);
    cartridge.write_ram(0xA000, 0x62);
    assert_eq!(cartridge.read_ram(0xA000), 0x00);

    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x00);

    cartridge.write_rom(0x6000, 0x00);
    cartridge.write_rom(0x6000, 0x01);

    cartridge.write_rom(0x4000, 0x08);
    assert_eq!(cartridge.read_ram(0xA000), 0x34);

    cartridge.advance_rtc_seconds(1);

    cartridge.write_rom(0x4000, 0x08);
    assert_eq!(cartridge.read_ram(0xA000), 0x34);

    cartridge.write_rom(0x4000, 0x09);
    assert_eq!(cartridge.read_ram(0xA000), 0x32);

    cartridge.write_rom(0x4000, 0x0A);
    assert_eq!(cartridge.read_ram(0xA000), 0x02);
}

#[test]
fn mbc3_latch_keeps_legacy_follow_up_non_zero_refreshes_after_a_valid_edge() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x08);

    cartridge.write_ram(0xA000, 0x11);
    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x00);

    cartridge.write_rom(0x6000, 0x55);
    assert_eq!(cartridge.read_ram(0xA000), 0x00);

    cartridge.write_rom(0x6000, 0x00);
    assert!(cartridge.rtc_latch_armed);
    cartridge.write_rom(0x6000, 0x01);
    assert!(!cartridge.rtc_latch_armed);
    assert_eq!(cartridge.read_ram(0xA000), 0x11);

    cartridge.write_ram(0xA000, 0x37);
    assert_eq!(cartridge.read_ram(0xA000), 0x11);

    cartridge.write_rom(0x6000, 0x44);
    assert_eq!(cartridge.read_ram(0xA000), 0x37);
}

#[test]
fn mbc3_halt_and_carry_behavior_follow_the_live_rtc_rules() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x0C);
    cartridge.write_ram(0xA000, 0x40);
    cartridge.advance_rtc_seconds(86_400);
    assert_eq!(cartridge.rtc_live.day_counter, 0);

    cartridge.write_ram(0xA000, 0x00);
    cartridge.advance_rtc_seconds(86_400);
    assert_eq!(cartridge.rtc_live.day_counter, 1);

    cartridge.rtc_live.day_counter = 511;
    cartridge.rtc_live.hours = 23;
    cartridge.rtc_live.minutes = 59;
    cartridge.rtc_live.seconds = 59;
    cartridge.advance_rtc_seconds(1);
    assert_eq!(cartridge.rtc_live.day_counter, 0);
    assert!(cartridge.rtc_live.carry);

    cartridge.write_ram(0xA000, 0x00);
    assert!(!cartridge.rtc_live.carry);
}

#[test]
fn mbc3_persistent_rtc_elapsed_seconds_follow_the_live_rules() {
    let mut rtc = Mbc3RtcPersistentState {
        seconds: 59,
        minutes: 59,
        hours: 23,
        day_counter: 511,
        halt: false,
        carry: false,
    };

    rtc.apply_elapsed_seconds(2);

    assert_eq!(rtc.seconds, 1);
    assert_eq!(rtc.minutes, 0);
    assert_eq!(rtc.hours, 0);
    assert_eq!(rtc.day_counter, 0);
    assert!(rtc.carry);

    let halted = rtc;
    let mut halted = Mbc3RtcPersistentState {
        halt: true,
        ..halted
    };
    halted.apply_elapsed_seconds(86_400);
    assert_eq!(halted.seconds, rtc.seconds);
    assert_eq!(halted.minutes, rtc.minutes);
    assert_eq!(halted.hours, rtc.hours);
    assert_eq!(halted.day_counter, rtc.day_counter);
    assert!(halted.carry);
}

#[test]
fn strict_validation_rejects_mbc30_like_64kib_sram_configurations() {
    let rom = build_banked_mbc3_rom(0x13, 0x06, 0x05);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("MBC30-like SRAM should fail the standard MBC3 baseline");

    match error {
        CartridgeLoadError::Rejected {
            classification,
            reason,
            ..
        } => {
            assert_eq!(classification.detected_name(), "MBC30");
            assert_eq!(
                classification.selection(),
                CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::PlannedVariant)
            );
            assert!(reason.contains("known reserved variant"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn permissive_validation_can_warn_when_no_ram_mbc3_headers_still_declare_ram() {
    let rom = build_banked_mbc3_rom(0x11, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &warn_policy())
        .expect("warn policy should admit a no-RAM MBC3 mismatch");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
    );
}

#[test]
fn mbc3_nonpersistent_ram_and_rtc_only_profiles_cover_remaining_runtime_paths() {
    let ram_only_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x12, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+RAM should load");
    let Some(CartridgeDevice::Mbc3(mut ram_only)) = ram_only_report.cartridge().device.clone()
    else {
        panic!("expected MBC3 cartridge");
    };

    assert_eq!(
        ram_only.persistence_metadata(),
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
    assert_eq!(ram_only.persistent_state(), PersistentCartState::None);
    assert_eq!(
        ram_only.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        ram_only.restore_persistent_state(&PersistentCartState::Mbc3Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc3Ram",
        }),
    );

    ram_only.write_rom(0x0000, 0x0A);
    ram_only.write_rom(0x4000, 0x08);
    ram_only.write_ram(0xA000, 0x2A);
    assert_eq!(ram_only.read_ram(0xA000), RAM_ABSENT_READ_VALUE);

    ram_only.rtc_latch_armed = true;
    ram_only.write_rom(0x6000, 0x01);
    assert!(!ram_only.rtc_latch_armed);
    ram_only.write_rom(0x6000, 0x00);
    assert!(ram_only.rtc_latch_armed);

    let rtc_only_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x0F, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+TIMER+BATTERY should load");
    let Some(CartridgeDevice::Mbc3(mut rtc_only)) = rtc_only_report.cartridge().device.clone()
    else {
        panic!("expected MBC3 cartridge");
    };

    assert_eq!(
        rtc_only.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        },
    );

    rtc_only.advance_rtc_seconds(61);
    assert_eq!(
        rtc_only.persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 1,
                minutes: 1,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
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
        .expect("RTC-only persistent state should restore");
    assert_eq!(rtc_only.persistent_state(), restored_rtc_only);
    assert_eq!(
        rtc_only.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc3Rtc",
            actual: "None",
        }),
    );
}
