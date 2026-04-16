use super::*;

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
fn mbc3_high_selector_variants_0x14_through_0x27_still_follow_low_nibble_semantics() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    for value in 0x14..=0x27 {
        cartridge.write_rom(0x4000, value);

        let expected = match value & 0x0F {
            0x00..=0x03 => Mbc3RamRtcSelect::RamBank(value & 0x0F),
            0x08 => Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::Seconds),
            0x09 => Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::Minutes),
            0x0A => Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::Hours),
            0x0B => Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::DayLow),
            0x0C => Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::DayHigh),
            other => Mbc3RamRtcSelect::ReservedSelector(other),
        };

        assert_eq!(
            cartridge.ram_or_rtc_select, expected,
            "selector {value:#04X}"
        );
    }
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
fn mbc3_2kib_sram_wraps_the_full_window_and_selected_bank_into_real_ram() {
    let rom = build_banked_mbc3_rom(0x13, 0x03, 0x01);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x00);
    cartridge.write_ram(0xA123, 0x11);

    assert_eq!(cartridge.read_ram(0xA123), 0x11);
    assert_eq!(cartridge.read_ram(0xA923), 0x11);

    cartridge.write_rom(0x4000, 0x03);
    assert_eq!(cartridge.read_ram(0xA123), 0x11);
    cartridge.write_ram(0xA923, 0x22);

    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.read_ram(0xA123), 0x22);
    assert_eq!(cartridge.read_ram(0xB123), 0x22);
}

#[test]
fn strict_validation_admits_mbc3_headers_with_2kib_ram_metadata() {
    let rom = build_banked_mbc3_rom(0x13, 0x00, 0x01);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
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
