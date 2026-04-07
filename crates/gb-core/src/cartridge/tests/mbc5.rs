use super::*;

#[test]
fn mbc5_power_up_state_starts_the_high_window_on_bank_one_while_keeping_bank_zero_reachable() {
    let rom = build_banked_mbc5_rom(0x1E, 0x08, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");

    let Some(CartridgeDevice::Mbc5(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC5 cartridge");
    };

    assert_eq!(cartridge.variant, Mbc5Variant::RumbleRamBattery);
    assert!(!cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low8, 1);
    assert_eq!(cartridge.rom_bank_high1, 0);
    assert_eq!(cartridge.ram_bank_raw, 0);
    assert!(!cartridge.rumble_on());
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
    assert_eq!(cartridge.read_rom(0x4001), 0x00);

    let mut cartridge = cartridge.clone();
    cartridge.write_rom(0x2000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);
    assert_eq!(cartridge.read_rom(0x4001), 0x00);
}

#[test]
fn mbc5_reaches_bank_0x1ff_without_applying_a_zero_to_one_translation() {
    let rom = build_banked_mbc5_rom(0x1B, 0x08, 0x04);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let Some(CartridgeDevice::Mbc5(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC5 cartridge");
    };

    cartridge.write_rom(0x2000, 0xFF);
    cartridge.write_rom(0x3000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
    assert_eq!(cartridge.read_rom(0x4001), 0x00);

    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x3000, 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);
    assert_eq!(cartridge.read_rom(0x4001), 0x01);

    cartridge.write_rom(0x2000, 0xFF);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
    assert_eq!(cartridge.read_rom(0x4001), 0x01);
}

#[test]
fn mbc5_rumble_control_keeps_motor_state_distinct_from_effective_ram_bank() {
    let rom = build_banked_mbc5_rom(0x1E, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let Some(CartridgeDevice::Mbc5(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC5 cartridge");
    };

    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    cartridge.write_rom(0x0000, 0x0A);

    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);

    cartridge.write_rom(0x4000, 0x0B);
    assert!(cartridge.rumble_on());
    assert_eq!(cartridge.ram_bank_raw, 0x03);
    assert_eq!(cartridge.read_ram(0xA000), 0x33);

    cartridge.write_rom(0x4000, 0x03);
    assert!(!cartridge.rumble_on());
    assert_eq!(cartridge.read_ram(0xA000), 0x33);
}

#[test]
fn strict_validation_rejects_oversized_mbc5_images_and_invalid_rumble_ram_sizes() {
    let oversized = build_test_rom(16 * 1024 * 1024, 0x1B, 0x08, 0x04);
    let oversized_error = CartridgeSlot::load(oversized, &CompatibilityPolicy::strict())
        .expect_err("oversized MBC5 should fail validation");

    match oversized_error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("exceeds the current MBC5 ROM limit"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let invalid_rumble_ram = build_banked_mbc5_rom(0x1E, 0x03, 0x04);
    let invalid_rumble_error =
        CartridgeSlot::load(invalid_rumble_ram, &CompatibilityPolicy::strict())
            .expect_err("128 KiB rumble MBC5 should fail validation");

    match invalid_rumble_error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("rumble-capable MBC5 baseline"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn permissive_validation_can_warn_when_no_ram_mbc5_headers_still_declare_ram() {
    let rom = build_banked_mbc5_rom(0x19, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &warn_policy())
        .expect("warn policy should admit a no-RAM MBC5 mismatch");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc5);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
    );
}
