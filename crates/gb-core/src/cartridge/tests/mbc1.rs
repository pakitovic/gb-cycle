use super::*;

#[test]
fn loading_supported_mbc1_family_constructs_the_mapper_device() {
    let rom = build_banked_mbc1_rom(0x02, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
    assert_eq!(report.cartridge().read_rom(0x0000), 0x00);
    assert_eq!(report.cartridge().read_rom(0x4000), 0x01);
}

#[test]
fn loading_32kib_mbc1_images_keeps_the_switchable_window_on_bank_one() {
    let rom = build_banked_mbc1_rom(0x00, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
    assert_eq!(report.cartridge().read_rom(0x0000), 0x00);
    assert_eq!(report.cartridge().read_rom(0x4000), 0x01);
}

#[test]
fn mbc1_power_up_state_is_explicit_and_starts_the_high_window_on_bank_one() {
    let rom = build_banked_mbc1_rom(0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    let Some(CartridgeDevice::Mbc1(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.wiring, Mbc1Wiring::Standard);
    assert_eq!(cartridge.variant, Mbc1Variant::Standard);
    assert!(!cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low5, 0);
    assert_eq!(cartridge.secondary_bank, 0);
    assert_eq!(cartridge.banking_mode, 0);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
}

#[test]
fn mbc1_raw_low_bank_zero_translates_to_bank_one_before_size_masking() {
    let rom = build_banked_mbc1_rom(0x04, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x2000, 0x00);
    assert_eq!(cartridge.rom_bank_low5, 0);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);

    cartridge.write_rom(0x2000, 0x1F);
    assert_eq!(cartridge.read_rom(0x4000), 0x1F);
}

#[test]
fn mbc1_small_rom_masking_can_make_bank_zero_visible_in_the_high_window() {
    let rom = build_banked_mbc1_rom(0x01, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x2000, 0x04);

    assert_eq!(cartridge.read_rom(0x4000), 0x00);
}

#[test]
fn mbc1_control_writes_update_raw_registers_and_gate_ram_access_immediately() {
    let rom = build_banked_mbc1_rom(0x02, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    cartridge.write_ram(0xA000, 0x5A);
    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_ram(0xA000, 0x5A);

    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.secondary_bank, 0x02);
    assert_eq!(cartridge.banking_mode, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x5A);
}

#[test]
fn mbc1_standard_8kib_ram_ignores_mode_one_ram_bank_selection() {
    let rom = build_banked_mbc1_rom(0x01, 0x02);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x11);
    cartridge.write_ram(0xB000, 0x22);

    cartridge.write_rom(0x6000, 0x01);
    for bank in 0..=3 {
        cartridge.write_rom(0x4000, bank);
        assert_eq!(cartridge.read_ram(0xA000), 0x11);
        assert_eq!(cartridge.read_ram(0xB000), 0x22);
    }

    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);
    cartridge.write_ram(0xB000, 0x44);

    cartridge.write_rom(0x4000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x33);
    assert_eq!(cartridge.read_ram(0xB000), 0x44);
}

#[test]
fn mbc1_large_rom_high_window_reaches_documented_odd_bank_entries_only() {
    let rom = build_banked_mbc1_rom(0x06, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.wiring, Mbc1Wiring::LargeRom);

    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x4000, 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x21);

    cartridge.write_rom(0x4000, 0x02);
    assert_eq!(cartridge.read_rom(0x4000), 0x41);

    cartridge.write_rom(0x4000, 0x03);
    assert_eq!(cartridge.read_rom(0x4000), 0x61);
}

#[test]
fn mbc1_large_rom_mode_one_remaps_the_low_window_from_secondary_bits() {
    let rom = build_banked_mbc1_rom(0x06, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x2000, 0x01);
    cartridge.write_rom(0x4000, 0x02);
    assert_eq!(cartridge.read_rom(0x0000), 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x41);

    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_rom(0x0000), 0x40);
    assert_eq!(cartridge.read_rom(0x4000), 0x41);
}

#[test]
fn mbc1_large_rom_keeps_one_fixed_8kib_ram_window_across_modes() {
    let rom = build_banked_mbc1_rom(0x05, 0x02);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x11);

    cartridge.write_rom(0x4000, 0x01);
    cartridge.write_rom(0x6000, 0x01);

    assert_eq!(cartridge.read_ram(0xA000), 0x11);
    cartridge.write_ram(0xA000, 0x22);

    cartridge.write_rom(0x4000, 0x00);
    cartridge.write_rom(0x6000, 0x00);
    assert_eq!(cartridge.read_ram(0xA000), 0x22);
}

#[test]
fn experimental_mbc1m_multicart_banking_uses_the_documented_game_select_layout() {
    let mut rom = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00);
    mark_mbc1_multicart_subheaders(&mut rom);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::experimental())
        .expect("experimental MBC1M should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.variant, Mbc1Variant::Mbc1M);
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("explicit experimental multicart heuristic")
    }));

    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);

    cartridge.write_rom(0x2000, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);

    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x4000, 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x11);

    cartridge.write_rom(0x2000, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0x10);

    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x02);
    assert_eq!(cartridge.read_rom(0x0000), 0x20);
    assert_eq!(cartridge.read_rom(0x4000), 0x20);

    cartridge.write_rom(0x2000, 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x21);
}

#[test]
fn strict_validation_rejects_large_rom_mbc1_with_32kib_ram_declaration() {
    let rom = build_banked_mbc1_rom(0x05, 0x03);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("invalid large-ROM MBC1 RAM config must fail");

    match error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("not valid for the current LargeRom MBC1 wiring baseline"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn strict_validation_admits_32kib_mbc1_images_as_small_standard_wiring() {
    let rom = build_banked_mbc1_rom(0x00, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.wiring, Mbc1Wiring::Standard);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
}
