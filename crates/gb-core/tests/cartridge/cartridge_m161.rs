use super::*;

#[test]
fn loading_m161_signature_rom_constructs_the_supported_mapper() {
    let report = CartridgeSlot::load(build_m161_signature_rom(), &CompatibilityPolicy::strict())
        .expect("M161 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::M161);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::M161)
    );
}

#[test]
fn loading_m161_commercial_header_shape_constructs_the_supported_mapper() {
    let report = CartridgeSlot::load(build_m161_commercial_rom(), &CompatibilityPolicy::strict())
        .expect("commercial M161 header shape should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::M161);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::M161)
    );
    assert_eq!(
        report
            .cartridge()
            .header()
            .expect("header should exist")
            .title,
        "TETRIS SET"
    );
}

#[test]
fn m161_bus_writes_switch_the_entire_rom_window_once() {
    let report = CartridgeSlot::load(build_m161_signature_rom(), &CompatibilityPolicy::strict())
        .expect("M161 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x0000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge),
        ),
        0x00
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge),
        ),
        0x00
    );

    bus.write_partial_harness_with_cartridge(
        0x6000,
        0x03,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x0000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge),
        ),
        0x03
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge),
        ),
        0x03
    );
}

#[test]
fn m161_bus_resolution_reports_absent_external_ram_and_no_persistence() {
    let report = CartridgeSlot::load(build_m161_signature_rom(), &CompatibilityPolicy::strict())
        .expect("M161 should load");
    let (cartridge, _) = report.into_parts();
    let bus = Bus::new(ConsoleModel::Dmg);

    let resolution = bus.resolve_access(
        BusAccessKind::Read,
        0xA000,
        &BusArbitrationState::default(),
        Some(&cartridge),
    );
    assert_eq!(
        resolution
            .cartridge_external()
            .expect("M161 external aperture should be described"),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::LinearRam,
            CartridgeExternalAvailability::Absent,
            CartridgeExternalReadBehavior::FallbackValue(0xFF),
            CartridgeExternalWriteBehavior::Ignored,
        )
    );
    assert_eq!(
        cartridge.persistence_metadata().profile,
        CartridgePersistenceProfile::None
    );
    assert_eq!(cartridge.persistent_state(), PersistentCartState::None);
}
