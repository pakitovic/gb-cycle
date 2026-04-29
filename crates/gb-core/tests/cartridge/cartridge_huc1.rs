use super::*;

#[test]
fn loading_supported_huc1_family_constructs_the_mapper_device() {
    let report = CartridgeSlot::load(
        build_banked_huc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC1 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Huc1);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Huc1)
    );
}

#[test]
fn huc1_bus_resolution_surfaces_ram_vs_ir_mode_semantics() {
    let report = CartridgeSlot::load(
        build_banked_huc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let bus = Bus::new(ConsoleModel::GameBoy);

    let ram_resolution = bus.resolve_access(
        BusAccessKind::Read,
        0xA000,
        &BusArbitrationState::default(),
        Some(&cartridge),
    );
    assert_eq!(
        ram_resolution
            .cartridge_external()
            .expect("HuC1 external aperture should be described"),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::BankedRam { bank: 0 },
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::Storage,
            CartridgeExternalWriteBehavior::Storage,
        )
    );

    cartridge.write_rom(0x0000, 0x0E);
    let ir_resolution = bus.resolve_access(
        BusAccessKind::Read,
        0xA000,
        &BusArbitrationState::default(),
        Some(&cartridge),
    );
    assert_eq!(
        ir_resolution
            .cartridge_external()
            .expect("HuC1 IR aperture should be described"),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::IrRegister,
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::InfraredSensor,
            CartridgeExternalWriteBehavior::InfraredTransmitter,
        )
    );
}

#[test]
fn huc1_bus_writes_bank_rom_and_ram_without_a_ram_enable_gate() {
    let report = CartridgeSlot::load(
        build_banked_huc1_rom(0x05, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default();

    bus.write_partial_harness_with_cartridge(
        0x2000,
        0x3F,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge),
        ),
        0x3F
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
        0x02,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
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
            Some(&cartridge),
        ),
        0x22
    );

    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x0E,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge),
        ),
        0xC0
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
            Some(&cartridge),
        ),
        0x22
    );
}
