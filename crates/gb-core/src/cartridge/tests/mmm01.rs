use super::*;

#[test]
fn loading_mmm01_uses_the_menu_header_and_starts_in_unmapped_mode() {
    let rom = build_mmm01_rom(0x03, 0x00, 0x0B);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MMM01 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mmm01);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mmm01)
    );

    let Some(CartridgeDevice::Mmm01(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MMM01 cartridge");
    };

    assert!(!cartridge.mapped);
    assert!(!cartridge.ram_enabled);
    assert_eq!(cartridge.read_rom(0x0000), 0x0E);
    assert_eq!(cartridge.read_rom(0x4000), 0x0F);
}

#[test]
fn mmm01_mapping_enable_switches_from_menu_rom_to_the_selected_game_window() {
    let rom = build_mmm01_rom(0x03, 0x00, 0x0B);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MMM01 should load");
    let Some(CartridgeDevice::Mmm01(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MMM01 cartridge");
    };

    cartridge.write_rom(0x2000, 0x04);
    cartridge.write_rom(0x6000, 0x38);
    cartridge.write_rom(0x0000, 0x40);

    assert!(cartridge.mapped);
    assert_eq!(cartridge.read_rom(0x0000), 0x04);
    assert_eq!(cartridge.read_rom(0x4000), 0x05);

    cartridge.write_rom(0x2000, 0x06);
    assert_eq!(cartridge.read_rom(0x0000), 0x04);
    assert_eq!(cartridge.read_rom(0x4000), 0x06);
}

#[test]
fn mmm01_rom_bank_mask_preserves_game_select_bits_after_mapping() {
    let rom = build_mmm01_rom(0x03, 0x00, 0x0B);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MMM01 should load");
    let Some(CartridgeDevice::Mmm01(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MMM01 cartridge");
    };

    cartridge.write_rom(0x2000, 0x04);
    cartridge.write_rom(0x6000, 0x38);
    cartridge.write_rom(0x0000, 0x40);
    assert_eq!(cartridge.read_rom(0x0000), 0x04);
    assert_eq!(cartridge.read_rom(0x4000), 0x05);

    cartridge.write_rom(0x2000, 0x00);
    assert_eq!(cartridge.read_rom(0x0000), 0x04);
    assert_eq!(cartridge.read_rom(0x4000), 0x05);
}

#[test]
fn mmm01_ram_enable_and_banked_ram_follow_the_non_multiplex_mode_rules() {
    let rom = build_mmm01_rom(0x03, 0x03, 0x0D);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MMM01 should load");
    let Some(CartridgeDevice::Mmm01(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MMM01 cartridge");
    };

    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_rom(0x0000, 0x2A);
    cartridge.write_rom(0x0000, 0x6A);
    assert!(cartridge.mapped);
    assert!(cartridge.ram_enabled);

    cartridge.write_ram(0xA000, 0x11);
    assert_eq!(cartridge.read_ram(0xA000), 0x11);

    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_ram(0xA000, 0x22);
    assert_eq!(cartridge.read_ram(0xA000), 0x22);

    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);
    assert_eq!(cartridge.read_ram(0xA000), 0x33);

    cartridge.write_rom(0x4000, 0x02);
    assert_eq!(cartridge.read_ram(0xA000), 0x22);
}
