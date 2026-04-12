use super::*;

#[test]
fn loading_supported_mbc5_families_constructs_the_mapper_device() {
    let cases = [
        (0x19, 0x03, 0x00),
        (0x1A, 0x03, 0x02),
        (0x1B, 0x03, 0x03),
        (0x1C, 0x03, 0x00),
        (0x1D, 0x03, 0x02),
        (0x1E, 0x03, 0x03),
    ];

    for (cartridge_type, rom_size_code, ram_size_code) in cases {
        let rom = build_banked_mbc5_rom(cartridge_type, rom_size_code, ram_size_code);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc5);
    }
}

#[test]
fn mbc5_power_up_bank_one_and_the_0xff_to_0x100_boundary_are_visible_through_the_bus() {
    let rom = build_banked_mbc5_rom(0x1B, 0x08, 0x04);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
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
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4001,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x00
    );

    bus.write_partial_harness_with_cartridge(
        0x2000,
        0xFF,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x3000,
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
        0xFF
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4001,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x00
    );

    bus.write_partial_harness_with_cartridge(
        0x2000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x3000,
        0x01,
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
        0x00
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4001,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x01
    );

    bus.write_partial_harness_with_cartridge(
        0x2000,
        0xFF,
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
        0xFF
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4001,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x01
    );
}

#[test]
fn mbc5_linear_ram_banking_supports_128kib_sram_through_the_bus() {
    let rom = build_banked_mbc5_rom(0x1B, 0x03, 0x04);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

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
        0xA000,
        0x22,
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
    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0xA000,
        0x11,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x0F,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0xA000,
        0xEE,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
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
        0x11
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x0F,
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
        0xEE
    );
}

#[test]
fn rumble_capable_mbc5_keeps_motor_state_distinct_from_the_effective_ram_bank() {
    let rom = build_banked_mbc5_rom(0x1E, 0x03, 0x03);
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(gb_core::StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("MBC5 rumble test image should load");

    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x03);
    machine.write_bus(0xA000, 0x33);

    machine.write_bus(0x4000, 0x0B);
    assert!(machine.cartridge().rumble_on());
    assert_eq!(machine.read_bus(0xA000), 0x33);

    machine.write_bus(0x4000, 0x03);
    assert!(!machine.cartridge().rumble_on());
    assert_eq!(machine.read_bus(0xA000), 0x33);
}

#[test]
fn rumble_capable_mbc5_supports_64kib_sram_while_preserving_motor_control_through_the_bus() {
    let rom = build_banked_mbc5_rom(0x1E, 0x03, 0x05);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0xA000,
        0x10,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x07,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0xA000,
        0x70,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert!(!cartridge.rumble_on());
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x10
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x07,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert!(!cartridge.rumble_on());
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x70
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x0F,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert!(cartridge.rumble_on());
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x70
    );
}

#[test]
fn strict_validation_rejects_oversized_and_invalid_128kib_rumble_mbc5_configurations() {
    let oversized = build_test_rom(16 * 1024 * 1024, 0x1B, 0x08, 0x04);
    let oversized_error = CartridgeSlot::load(oversized, &CompatibilityPolicy::strict())
        .expect_err("oversized MBC5 should fail validation");

    let oversized_reason = match oversized_error {
        gb_core::CartridgeLoadError::Rejected { reason, .. } => reason,
        other => panic!("unexpected error: {other:?}"),
    };
    assert!(oversized_reason.contains("exceeds the current MBC5 ROM limit"));

    let invalid_rumble = build_banked_mbc5_rom(0x1E, 0x03, 0x04);
    let invalid_rumble_error = CartridgeSlot::load(invalid_rumble, &CompatibilityPolicy::strict())
        .expect_err("invalid rumble MBC5 RAM size should fail validation");

    let invalid_rumble_reason = match invalid_rumble_error {
        gb_core::CartridgeLoadError::Rejected { reason, .. } => reason,
        other => panic!("unexpected error: {other:?}"),
    };
    assert!(invalid_rumble_reason.contains("rumble-capable MBC5 baseline"));

    let valid_rumble_64k = build_banked_mbc5_rom(0x1E, 0x03, 0x05);
    CartridgeSlot::load(valid_rumble_64k, &CompatibilityPolicy::strict())
        .expect("64 KiB rumble MBC5 should load");
}

#[test]
fn permissive_validation_can_warn_on_no_ram_mbc5_headers_with_nonzero_ram_metadata() {
    let rom = build_banked_mbc5_rom(0x19, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::permissive())
        .expect("permissive mode should admit no-RAM MBC5 with warning");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc5);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
    );
}
