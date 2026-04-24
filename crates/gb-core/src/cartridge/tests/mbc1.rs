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
fn mbc1m_multicart_banking_uses_the_documented_game_select_layout() {
    let mut rom = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00);
    mark_mbc1_multicart_subheaders(&mut rom);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect("MBC1M should load through the default signature path");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.variant, Mbc1Variant::Mbc1M);
    assert!(report.diagnostics().is_empty());

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
fn mbc1m_with_battery_backed_8kib_ram_keeps_a_fixed_ram_window() {
    let mut rom = build_banked_mbc1_rom_with_type(0x03, 0x05, 0x02);
    mark_mbc1_multicart_subheaders_in_banks(&mut rom, &[0x10, 0x20]);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1M should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.variant, Mbc1Variant::Mbc1M);
    assert_eq!(
        cartridge.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 8 * 1024 },
            },
        }
    );

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x11);

    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x11);

    let PersistentCartState::Mbc1Ram { ram } = cartridge.persistent_state() else {
        panic!("expected MBC1 RAM persistence");
    };
    assert_eq!(ram.len(), 8 * 1024);
    assert_eq!(ram[0], 0x11);
}

#[test]
fn strict_validation_rejects_large_rom_mbc1_with_32kib_ram_declaration() {
    let rom = build_banked_mbc1_rom(0x05, 0x03);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("invalid large-ROM MBC1 RAM config must fail");

    match error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("contradicts the current MBC1+RAM LargeRom wiring baseline"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn strict_validation_rejects_mbc1_without_ram_when_header_declares_external_ram() {
    let rom = build_banked_mbc1_rom_with_type(0x01, 0x02, 0x02);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("MBC1 without RAM must reject contradictory SRAM metadata");

    match error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(
                reason
                    .contains("contradicts the current MBC1 without RAM Standard wiring baseline")
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn strict_validation_rejects_mbc1_plus_ram_when_header_omits_required_ram_shape() {
    let rom = build_banked_mbc1_rom_with_type(0x03, 0x05, 0x00);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("MBC1+RAM must reject missing RAM metadata");

    match error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("contradicts the current MBC1+RAM LargeRom wiring baseline"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn warn_policy_uses_type_derived_mbc1_ram_capability_instead_of_header_contradictions() {
    let no_ram_rom = build_banked_mbc1_rom_with_type(0x01, 0x02, 0x02);
    let no_ram_report = CartridgeSlot::load(no_ram_rom, &warn_policy())
        .expect("warn policy should keep contradictory MBC1 no-RAM loads deterministic");
    let Some(CartridgeDevice::Mbc1(no_ram_cartridge)) = no_ram_report.cartridge().device.as_ref()
    else {
        panic!("expected MBC1 cartridge");
    };
    assert!(no_ram_cartridge.ram.is_none());
    assert!(no_ram_report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contradicts the current MBC1 without RAM Standard wiring baseline")
    }));

    let large_rom_ramless_header = build_banked_mbc1_rom_with_type(0x03, 0x05, 0x00);
    let large_rom_report = CartridgeSlot::load(large_rom_ramless_header, &warn_policy())
        .expect("warn policy should keep large-ROM MBC1 RAM on the fixed 8 KiB baseline");
    let Some(CartridgeDevice::Mbc1(large_rom_cartridge)) =
        large_rom_report.cartridge().device.as_ref()
    else {
        panic!("expected MBC1 cartridge");
    };
    assert_eq!(
        large_rom_cartridge.ram.as_ref().map(Vec::len),
        Some(MBC1_LARGE_ROM_RAM_BYTES)
    );
    assert!(large_rom_report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contradicts the current MBC1+RAM LargeRom wiring baseline")
    }));

    let standard_rom_missing_ram_header = build_banked_mbc1_rom_with_type(0x02, 0x02, 0x00);
    let standard_report = CartridgeSlot::load(standard_rom_missing_ram_header, &warn_policy())
        .expect("warn policy should keep standard MBC1 RAM on the explicit 32 KiB baseline");
    let Some(CartridgeDevice::Mbc1(mut standard_cartridge)) =
        standard_report.cartridge().device.clone()
    else {
        panic!("expected MBC1 cartridge");
    };
    assert_eq!(
        standard_cartridge.ram.as_ref().map(Vec::len),
        Some(MBC1_STANDARD_RAM_BYTES_MAX)
    );
    assert!(standard_report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contradicts the current MBC1+RAM Standard wiring baseline")
    }));

    standard_cartridge.write_rom(0x0000, 0x0A);
    standard_cartridge.write_ram(0xA000, 0x11);
    standard_cartridge.write_rom(0x6000, 0x01);
    standard_cartridge.write_rom(0x4000, 0x02);
    standard_cartridge.write_ram(0xA000, 0x22);
    standard_cartridge.write_rom(0x4000, 0x00);
    assert_eq!(standard_cartridge.read_ram(0xA000), 0x11);
    standard_cartridge.write_rom(0x4000, 0x02);
    assert_eq!(standard_cartridge.read_ram(0xA000), 0x22);
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
