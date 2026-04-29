use super::*;

#[test]
fn resolve_access_surfaces_disabled_mbc1_external_ram_state() {
    let rom = build_banked_mbc1_rom(0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (cartridge, _) = report.into_parts();
    let bus = Bus::new(ConsoleModel::GameBoy);

    let resolution = bus.resolve_access(
        BusAccessKind::Read,
        0xA000,
        &BusArbitrationState::default(),
        Some(&cartridge),
    );
    let external = resolution
        .cartridge_external()
        .expect("cartridge external aperture should be described");

    assert_eq!(
        resolution.target().region(),
        gb_core::BusRegion::CartridgeExternal
    );
    assert_eq!(resolution.nominal_cartridge_external(), Some(external));
    assert_eq!(
        external,
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::BankedRam { bank: 0 },
            CartridgeExternalAvailability::Disabled,
            CartridgeExternalReadBehavior::FallbackValue(0xFF),
            CartridgeExternalWriteBehavior::Ignored,
        )
    );
}

#[test]
fn loading_supported_mbc1_family_constructs_the_mapper_device() {
    let rom = build_banked_mbc1_rom(0x02, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
}

#[test]
fn loading_mbc1m_signature_in_strict_mode_keeps_the_distinct_multicart_variant() {
    let mut rom = build_banked_mbc1_rom(0x05, 0x00);
    mark_mbc1_multicart_subheaders(&mut rom);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1M should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .detected_name(),
        "MBC1M"
    );
}

#[test]
fn loading_32kib_mbc1_family_constructs_the_mapper_device() {
    let rom = build_banked_mbc1_rom(0x00, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
    assert_eq!(report.cartridge().read_rom(0x0000), 0x00);
    assert_eq!(report.cartridge().read_rom(0x4000), 0x01);
}

#[test]
fn mbc1_rom_bank_writes_take_effect_immediately_for_later_bus_reads() {
    let rom = build_banked_mbc1_rom(0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
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
        0x2000,
        0x02,
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
        0x02
    );

    bus.write_partial_harness_with_cartridge(
        0x2000,
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
}

#[test]
fn mbc1_standard_high_window_supports_bank_0x1f_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x04, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default();

    bus.write_partial_harness_with_cartridge(
        0x2000,
        0x1F,
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
        0x1F
    );
}

#[test]
fn mbc1_small_rom_masking_can_surface_bank_zero_in_the_high_window_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x01, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
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
        0x2000,
        0x04,
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
}

#[test]
fn mbc1_large_rom_high_window_exposes_0x21_0x41_and_0x61_not_0x20_0x40_or_0x60() {
    let rom = build_banked_mbc1_rom(0x06, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default();

    bus.write_partial_harness_with_cartridge(
        0x2000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
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
        0x21
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x02,
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
        0x41
    );

    bus.write_partial_harness_with_cartridge(
        0x4000,
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
        0x61
    );
}

#[test]
fn mbc1_large_rom_mode_one_remaps_the_low_window_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x06, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default();

    bus.write_partial_harness_with_cartridge(
        0x2000,
        0x01,
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

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x0000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x00
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x41
    );

    bus.write_partial_harness_with_cartridge(
        0x6000,
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
            Some(&cartridge)
        ),
        0x40
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x41
    );
}

#[test]
fn mbc1m_with_battery_ram_keeps_a_fixed_8kib_window_through_the_bus() {
    let mut rom = build_banked_mbc1_rom(0x05, 0x02);
    rom[CARTRIDGE_TYPE_ADDRESS] = 0x03;
    mark_mbc1_multicart_subheaders_in_banks(&mut rom, &[0x10, 0x20]);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1M should load");
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
        0x44,
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
        0x6000,
        0x01,
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
        0x44
    );
}

#[test]
fn mbc1_ram_enable_controls_external_ram_visibility_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x02, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default();

    bus.write_partial_harness_with_cartridge(
        0xA000,
        0x9A,
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
        0xA000,
        0x9A,
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
        0x9A
    );
}

#[test]
fn mbc1_standard_ram_mode_zero_and_mode_one_select_the_expected_ram_banks() {
    let rom = build_banked_mbc1_rom(0x02, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
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
        0x4000,
        0x02,
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
        0x6000,
        0x01,
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

    bus.write_partial_harness_with_cartridge(
        0x6000,
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
        0x6000,
        0x01,
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
        0x22
    );
}

#[test]
fn mbc1_large_rom_keeps_a_fixed_8kib_ram_window_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x05, 0x02);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
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
        0x33,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x4000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x6000,
        0x01,
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
        0x33
    );
}

#[test]
fn mbc1_persistent_state_round_trips_the_full_banked_ram_backing_store() {
    let report = CartridgeSlot::load(
        build_banked_mbc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_ram(0xA000, 0x66);
    cartridge.write_rom(0x0000, 0x00);

    let state = cartridge.persistent_state();
    match &state {
        PersistentCartState::Mbc1Ram { ram } => {
            assert_eq!(ram[2 * 0x2000], 0x66);
        }
        other => panic!("unexpected persistent state: {other:?}"),
    }

    let fresh_report = CartridgeSlot::load(
        build_banked_mbc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("fresh MBC1 should load");
    let (mut restored, _) = fresh_report.into_parts();
    restored
        .restore_persistent_state(&state)
        .expect("MBC1 persistence should restore");

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x6000, 0x01);
    restored.write_rom(0x4000, 0x02);
    assert_eq!(restored.read_ram(0xA000), 0x66);
}
