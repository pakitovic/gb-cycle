use super::*;

#[test]
fn resolve_access_surfaces_mbc3_rtc_selection_in_the_external_window() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let (mut cartridge, _) = report.into_parts();
    let bus = Bus::new(ConsoleModel::GameBoy);

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x08);

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
        external,
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::RtcRegister(CartridgeRtcRegister::Seconds),
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::RtcLatched,
            CartridgeExternalWriteBehavior::RtcLive,
        )
    );
}

#[test]
fn public_mbc3_rtc_access_spacing_state_surfaces_through_descriptor_and_snapshot() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("MBC3 cartridge should load into the machine");

    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x08);
    assert_eq!(
        machine
            .cartridge()
            .describe_external_access(0xA000)
            .rtc_access_ready_at(),
        None
    );

    let access_t_cycle = machine.next_t_cycle();
    machine.write_bus(0xA000, 0x12);
    let expected_ready_at = Some(TCycle::new(access_t_cycle.get() + 16));

    let external = machine.cartridge().describe_external_access(0xA000);
    assert_eq!(
        external.target(),
        CartridgeExternalTarget::RtcRegister(CartridgeRtcRegister::Seconds)
    );
    assert_eq!(external.rtc_access_ready_at(), expected_ready_at);

    let snapshot = machine.cartridge().snapshot();
    assert_eq!(snapshot.state, CartridgeSlotState::Mbc3);
    assert_eq!(snapshot.rtc_access_ready_at, expected_ready_at);

    let bus = Bus::new(ConsoleModel::GameBoy);
    let resolution = bus.resolve_access(
        BusAccessKind::Read,
        0xA000,
        &BusArbitrationState::default(),
        Some(machine.cartridge()),
    );
    assert_eq!(
        resolution
            .cartridge_external()
            .expect("cartridge external aperture should be described")
            .rtc_access_ready_at(),
        expected_ready_at
    );
}

#[test]
fn loading_supported_mbc3_families_constructs_the_mapper_device() {
    let cases = [
        (0x0F, 0x03, 0x00),
        (0x10, 0x03, 0x03),
        (0x11, 0x03, 0x00),
        (0x12, 0x03, 0x02),
        (0x13, 0x03, 0x03),
    ];

    for (cartridge_type, rom_size_code, ram_size_code) in cases {
        let rom = build_banked_mbc3_rom(cartridge_type, rom_size_code, ram_size_code);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
    }
}

#[test]
fn mbc3_high_window_can_reach_banks_0x20_0x40_and_0x60_through_the_bus() {
    let rom = build_banked_mbc3_rom(0x13, 0x06, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default();

    for bank in [0x20, 0x40, 0x60] {
        bus.write_partial_harness_with_cartridge(
            0x2000,
            bank,
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
            bank
        );
    }
}

#[test]
fn mbc3_ram_banking_and_rtc_latch_are_visible_through_machine_access() {
    let rom = build_banked_mbc3_rom(0x10, 0x06, 0x03);
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(gb_core::StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("MBC3 test image should load");
    machine.advance_cartridge_rtc_seconds(93_784);

    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x00);
    machine.write_bus(0xA000, 0x33);
    machine.write_bus(0x4000, 0x02);
    machine.write_bus(0xA000, 0x55);

    machine.write_bus(0x4000, 0x00);
    assert_eq!(machine.read_bus(0xA000), 0x33);
    machine.write_bus(0x4000, 0x02);
    assert_eq!(machine.read_bus(0xA000), 0x55);

    machine.write_bus(0x4000, 0x08);
    machine.write_bus(0x6000, 0x00);
    machine.write_bus(0x6000, 0x01);
    assert_eq!(machine.read_bus(0xA000), 0x04);

    machine.write_bus(0xA000, 0x2A);
    assert_eq!(machine.read_bus(0xA000), 0x04);

    machine.write_bus(0x6000, 0x00);
    machine.write_bus(0x6000, 0x01);
    assert_eq!(machine.read_bus(0xA000), 0x2A);
}

#[test]
fn strict_validation_admits_mbc30_like_64kib_sram_as_supported_mbc3_family_variant() {
    let rom = build_banked_mbc3_rom(0x13, 0x06, 0x05);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect("MBC30-like SRAM should load through the explicit variant path");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
    let classification = report
        .cartridge()
        .classification()
        .expect("loaded cartridge classification");
    assert_eq!(classification.detected_name(), "MBC30");
    assert_eq!(
        classification.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc3)
    );
}

#[test]
fn mbc30_exposes_extended_rom_and_ram_banks_through_the_machine_bus() {
    let rom = build_banked_mbc3_rom(0x13, 0x07, 0x05);
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor)
            .with_startup_mode(gb_core::StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("MBC30 test image should load");
    assert_eq!(
        machine.cartridge().persistence_metadata().profile,
        CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 64 * 1024,
            },
        }
    );

    machine.write_bus(0x2000, 0x00);
    assert_eq!(machine.read_bus(0x4000), 0x01);
    machine.write_bus(0x2000, 0x80);
    assert_eq!(machine.read_bus(0x4000), 0x80);
    machine.write_bus(0x2000, 0xFF);
    assert_eq!(machine.read_bus(0x4000), 0xFF);

    machine.write_bus(0x0000, 0x0A);
    for bank in 0x00..=0x07 {
        machine.write_bus(0x4000, bank);
        machine.write_bus(0xA000, 0xC0 | bank);
    }

    for bank in 0x00..=0x07 {
        machine.write_bus(0x4000, bank);
        assert_eq!(machine.read_bus(0xA000), 0xC0 | bank);
    }
}

#[test]
fn permissive_validation_can_warn_on_no_ram_mbc3_headers_with_nonzero_ram_metadata() {
    let rom = build_banked_mbc3_rom(0x11, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::permissive())
        .expect("permissive mode should admit no-RAM MBC3 with warning");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
    );
}

#[test]
fn strict_validation_treats_no_ram_mbc3_with_64kib_code_as_a_header_mismatch_not_mbc30() {
    let rom = build_banked_mbc3_rom(0x11, 0x03, 0x05);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("no-RAM MBC3 with 64 KiB code should be rejected");

    let (classification, reason) = match error {
        gb_core::CartridgeLoadError::Rejected {
            classification,
            reason,
            ..
        } => (classification, reason),
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(classification.detected_name(), "MBC3");
    assert_eq!(
        classification.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc3)
    );
    assert!(reason.contains("does not provide external RAM"));
}

#[test]
fn mbc3_persistent_state_serializes_live_rtc_state_not_the_latched_snapshot() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(gb_core::StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("MBC3+TIMER+RAM+BATTERY should load");

    machine.advance_cartridge_rtc_seconds(93_784);
    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x08);
    machine.write_bus(0x6000, 0x00);
    machine.write_bus(0x6000, 0x01);
    assert_eq!(machine.read_bus(0xA000), 0x04);

    machine.write_bus(0xA000, 0x2A);
    let state = machine.cartridge().persistent_state();
    match &state {
        PersistentCartState::Mbc3RamRtc { rtc, .. } => {
            assert_eq!(
                *rtc,
                Mbc3RtcPersistentState {
                    seconds: 0x2A,
                    minutes: 0x03,
                    hours: 0x02,
                    day_counter: 1,
                    halt: false,
                    carry: false,
                }
            );
        }
        other => panic!("unexpected persistent state: {other:?}"),
    }

    machine
        .restore_cartridge_persistent_state(&state)
        .expect("hot MBC3 persistence restore should succeed");
    assert_eq!(machine.read_bus(0xA000), 0x00);
    machine.write_bus(0x6000, 0x00);
    machine.write_bus(0x6000, 0x01);
    assert_eq!(machine.read_bus(0xA000), 0x2A);

    let fresh_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x10, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("fresh MBC3 should load");
    let (mut restored, _) = fresh_report.into_parts();
    restored
        .restore_persistent_state(&state)
        .expect("MBC3 persistence should restore");

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x4000, 0x08);
    assert_eq!(restored.read_ram(0xA000), 0x00);
    restored.write_rom(0x6000, 0x00);
    restored.write_rom(0x6000, 0x01);
    assert_eq!(restored.read_ram(0xA000), 0x2A);
}
