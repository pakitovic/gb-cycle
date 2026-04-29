use super::*;

#[test]
fn loading_supported_mbc2_families_constructs_the_mapper_device() {
    for cartridge_type in [0x05, 0x06] {
        let rom = build_banked_mbc2_rom(cartridge_type, 0x03, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc2);
    }
}

#[test]
fn mbc2_address_bit_8_decode_and_bank_zero_translation_are_visible_through_the_bus() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x01
    );

    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0xFF
    );

    bus.write_partial_harness_with_cartridge(
        0x0100,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x01
    );

    bus.write_partial_harness_with_cartridge(
        0x2100,
        0x03,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x03
    );
}

#[test]
fn mbc2_internal_nibble_ram_aliases_and_honors_the_repo_readback_policy() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default();

    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0xA000,
        0xAB,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0xFB
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA200,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0xFB
    );

    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0xA000,
        0x0C,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0xFF
    );

    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0xFB
    );
}

#[test]
fn strict_validation_rejects_mbc2_images_above_256kib() {
    let rom = build_banked_mbc2_rom(0x05, 0x04, 0x00);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("oversized MBC2 should fail validation");

    let reason = match error {
        gb_core::CartridgeLoadError::Rejected { reason, .. } => reason,
        other => panic!("unexpected error: {other:?}"),
    };
    assert!(reason.contains("exceeds the current MBC2 ROM limit"));
}

#[test]
fn permissive_validation_can_warn_on_nonzero_mbc2_ram_size_metadata() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::permissive())
        .expect("permissive mode should admit nonzero MBC2 RAM metadata");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc2);
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expects RAM size code 0x00 because MBC2 RAM is internal")
    }));
}

#[test]
fn mbc2_persistent_state_round_trips_the_nibble_array_and_rejects_invalid_values() {
    let report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x06, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2+BATTERY should load");
    let (mut cartridge, _) = report.into_parts();

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA123, 0x0B);
    cartridge.write_rom(0x0000, 0x00);

    let state = cartridge.persistent_state();
    match &state {
        PersistentCartState::Mbc2Ram { ram_nibbles } => {
            assert_eq!(ram_nibbles[0x123], 0x0B);
        }
        other => panic!("unexpected persistent state: {other:?}"),
    }

    let fresh_report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x06, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("fresh MBC2 should load");
    let (mut restored, _) = fresh_report.into_parts();
    restored
        .restore_persistent_state(&state)
        .expect("MBC2 persistence should restore");
    restored.write_rom(0x0000, 0x0A);
    assert_eq!(restored.read_ram(0xA123), 0xFB);

    let mut invalid_nibbles = [0u8; 512];
    invalid_nibbles[3] = 0x1F;
    let error = restored
        .restore_persistent_state(&PersistentCartState::Mbc2Ram {
            ram_nibbles: invalid_nibbles,
        })
        .expect_err("high bits in MBC2 persistent nibbles should fail");
    assert_eq!(
        error,
        CartridgePersistentStateError::InvalidMbc2NibbleValue {
            index: 3,
            value: 0x1F
        }
    );
}
