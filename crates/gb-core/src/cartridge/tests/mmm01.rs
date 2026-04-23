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
fn loading_mani_like_mmm01_uses_the_trailing_set_menu_header_and_supported_signature_path() {
    let report = CartridgeSlot::load(build_mani_mmm01_rom(0x04), &CompatibilityPolicy::strict())
        .expect("later Mani MMM01 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mmm01);
    let classification = report
        .cartridge()
        .classification()
        .expect("classification should exist");
    assert_eq!(
        classification.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mmm01)
    );
    assert_eq!(
        classification.reason(),
        "MMM01 classification came from the explicit later Mani trailing-menu signature path"
    );
    assert_eq!(
        report
            .cartridge()
            .header()
            .expect("header should exist")
            .title,
        "SAGAIA SET"
    );
    assert_eq!(
        report
            .cartridge()
            .header()
            .expect("header should exist")
            .cartridge_type,
        MANI_MMM01_MENU_TYPE
    );

    let Some(CartridgeDevice::Mmm01(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MMM01 cartridge");
    };

    assert!(!cartridge.mapped);
    assert_eq!(cartridge.read_rom(0x0000), 0x1E);
    assert_eq!(cartridge.read_rom(0x4000), 0x1F);
}

#[test]
fn loading_mani_like_mmm01_1mib_stays_on_mmm01_instead_of_mbc1m() {
    let report = CartridgeSlot::load(build_mani_mmm01_rom(0x05), &CompatibilityPolicy::strict())
        .expect("1 MiB later Mani MMM01 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mmm01);
    let classification = report
        .cartridge()
        .classification()
        .expect("classification should exist");
    assert_eq!(classification.detected_name(), "MMM01");
    assert_eq!(
        classification.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mmm01)
    );
    assert_eq!(
        classification.reason(),
        "MMM01 classification came from the explicit later Mani trailing-menu signature path"
    );
}

#[test]
fn mani_like_mmm01_runtime_reuses_the_standard_mapping_enable_path() {
    let report = CartridgeSlot::load(build_mani_mmm01_rom(0x04), &CompatibilityPolicy::strict())
        .expect("later Mani MMM01 should load");
    let Some(CartridgeDevice::Mmm01(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MMM01 cartridge");
    };

    cartridge.write_rom(0x2000, 0x04);
    cartridge.write_rom(0x6000, 0x38);
    cartridge.write_rom(0x0000, 0x40);

    assert!(cartridge.mapped);
    assert_eq!(cartridge.read_rom(0x0000), 0x04);
    assert_eq!(cartridge.read_rom(0x4000), 0x05);
}

#[test]
fn mani_like_mmm01_signature_accepts_mixed_no_mbc_and_mbc1_embedded_games_seen_in_local_dumps() {
    let mut rom = build_mani_mmm01_rom(0x04);
    rom[0x60000 + CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[0x60000 + ROM_SIZE_ADDRESS] = 0x00;

    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect("later Mani MMM01 should keep loading with one embedded NoMbc title");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mmm01);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .reason(),
        "MMM01 classification came from the explicit later Mani trailing-menu signature path"
    );
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

#[test]
fn mmm01_slot_timed_ram_helpers_surface_disabled_then_accessible_external_ram() {
    let rom = build_mmm01_rom(0x03, 0x03, 0x0D);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MMM01 should load");
    let (mut cartridge, _) = report.into_parts();

    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::BankedRam { bank: 0 },
            CartridgeExternalAvailability::Disabled,
            CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE),
            CartridgeExternalWriteBehavior::Ignored,
        )
    );
    assert_eq!(
        cartridge.read_ram_timed(0xA000, crate::scheduler::TCycle::new(1)),
        RAM_ABSENT_READ_VALUE
    );

    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_rom(0x0000, 0x2A);
    cartridge.write_rom(0x0000, 0x6A);
    cartridge.write_ram_timed(0xA000, 0x55, crate::scheduler::TCycle::new(7));

    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::BankedRam { bank: 2 },
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::Storage,
            CartridgeExternalWriteBehavior::Storage,
        )
    );
    assert_eq!(
        cartridge.read_ram_timed(0xA000, crate::scheduler::TCycle::new(11)),
        0x55
    );
}
