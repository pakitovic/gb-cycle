use super::*;

#[test]
fn loading_mmm01_family_uses_the_menu_header_in_the_last_32kib_window() {
    let report = CartridgeSlot::load(
        build_mmm01_rom(0x03, 0x00, 0x0B),
        &CompatibilityPolicy::strict(),
    )
    .expect("MMM01 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mmm01);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mmm01)
    );
    assert_eq!(
        report
            .cartridge()
            .header()
            .expect("header should exist")
            .cartridge_type,
        0x0B
    );
    assert_eq!(report.cartridge().read_rom(0x0000), 0x0E);
    assert_eq!(report.cartridge().read_rom(0x4000), 0x0F);
}

#[test]
fn mmm01_bus_writes_switch_from_the_menu_rom_to_the_selected_game() {
    let report = CartridgeSlot::load(
        build_mmm01_rom(0x03, 0x00, 0x0B),
        &CompatibilityPolicy::strict(),
    )
    .expect("MMM01 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x0000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x0E
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x0F
    );

    bus.write_partial_harness_with_cartridge(
        0x2000,
        0x04,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x6000,
        0x38,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x40,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x0000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x04
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x05
    );
}

#[test]
fn mani_like_mmm01_bus_writes_switch_from_the_trailing_set_menu_rom_to_the_selected_game() {
    let report = CartridgeSlot::load(build_mani_mmm01_rom(0x04), &CompatibilityPolicy::strict())
        .expect("later Mani MMM01 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x0000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x1E
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x1F
    );

    bus.write_partial_harness_with_cartridge(
        0x2000,
        0x04,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x6000,
        0x38,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x40,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x0000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x04
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x05
    );
}
