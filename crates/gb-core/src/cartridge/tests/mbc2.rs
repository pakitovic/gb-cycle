use super::*;

#[test]
fn mbc2_power_up_state_is_explicit_and_starts_the_high_window_on_bank_one() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");

    let Some(CartridgeDevice::Mbc2(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC2 cartridge");
    };

    assert!(!cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low4, 0);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
    assert!(cartridge.has_battery);
}

#[test]
fn mbc2_address_bit_8_decode_controls_enable_and_bank_registers_separately() {
    let rom = build_banked_mbc2_rom(0x05, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low4, 0);

    cartridge.write_rom(0x0100, 0x03);
    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low4, 0x03);
    assert_eq!(cartridge.read_rom(0x4000), 0x03);
}

#[test]
fn mbc2_internal_ram_masks_to_low_nibbles_and_aliases_on_low_9_bits() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };

    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    cartridge.write_ram(0xA000, 0xAB);
    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0xAB);

    assert_eq!(cartridge.read_ram(0xA000), 0xFB);
    assert_eq!(cartridge.read_ram(0xA200), 0xFB);
    assert_eq!(cartridge.read_ram(0xBFFF), 0xF0);
}

#[test]
fn mbc2_ignores_rom_space_writes_outside_the_control_window() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x0B);
    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.read_ram(0xA000), 0xFB);

    cartridge.write_rom(0x0100, 0x03);
    assert_eq!(cartridge.read_rom(0x4000), 0x03);

    cartridge.write_rom(0x4000, 0x00);
    cartridge.write_rom(0x4100, 0x01);

    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.read_ram(0xA000), 0xFB);
    assert_eq!(cartridge.read_rom(0x4000), 0x03);
}

#[test]
fn strict_validation_rejects_oversized_mbc2_roms() {
    let rom = build_banked_mbc2_rom(0x05, 0x04, 0x00);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("oversized MBC2 should fail validation");

    match error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("exceeds the current MBC2 ROM limit"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn permissive_validation_can_warn_on_nonzero_mbc2_ram_size_metadata() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &warn_policy())
        .expect("warn policy should admit nonzero MBC2 RAM metadata");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc2);
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expects RAM size code 0x00 because MBC2 RAM is internal")
    }));
}
