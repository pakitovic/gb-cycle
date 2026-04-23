use super::*;

#[test]
fn loading_supported_huc3_family_constructs_the_mapper_device() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Huc3);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Huc3)
    );
}

#[test]
fn huc3_bus_resolution_surfaces_mailbox_and_ram_modes() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");
    let (mut cartridge, _) = report.into_parts();
    let bus = Bus::new(ConsoleModel::Dmg);

    let ram_resolution = bus.resolve_access(
        BusAccessKind::Read,
        0xA000,
        &BusArbitrationState::default(),
        Some(&cartridge),
    );
    assert_eq!(
        ram_resolution
            .cartridge_external()
            .expect("HuC-3 external aperture should be described"),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::BankedRam { bank: 0 },
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::Storage,
            CartridgeExternalWriteBehavior::Ignored,
        )
    );

    cartridge.write_rom(0x0000, 0x0B);
    let mailbox_resolution = bus.resolve_access(
        BusAccessKind::Write,
        0xA123,
        &BusArbitrationState::default(),
        Some(&cartridge),
    );
    assert_eq!(
        mailbox_resolution
            .cartridge_external()
            .expect("HuC-3 mailbox aperture should be described"),
        CartridgeExternalAccessInfo::new(
            0xA123,
            CartridgeExternalTarget::Huc3CommandMailbox,
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::OpenBus,
            CartridgeExternalWriteBehavior::Huc3MailboxCommandArgument,
        )
    );
}

#[test]
fn huc3_bus_can_write_ram_and_execute_basic_mailbox_commands() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");
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
        0x0B,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0xA555,
        0x62,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x0000,
        0x0D,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0xA111,
        0xFE,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_partial_harness_with_cartridge(
        0x0000,
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
            Some(&cartridge),
        ),
        0xE1
    );
}

#[test]
fn huc3_public_persistent_state_keeps_its_dedicated_shape() {
    let state = PersistentCartState::Huc3 {
        ram: vec![0x11, 0x22],
        mcu_ram: [0; 256],
        rtc: Huc3RtcPersistentState {
            current_minutes_of_day: 123,
            current_days: 4,
            current_subminute_seconds: 5,
            event_minutes_of_day: 456,
            event_days: 7,
        },
        rom_bank: 0x3F,
        ram_bank: 0x02,
        select_mode: 0x0D,
        access_address: 0xAA,
        mailbox_command: 0x06,
        mailbox_argument: 0x02,
        last_response_nybble: 0x01,
        semaphore_ready: true,
        ir_emitter_on: false,
        ir_light_detected: false,
        last_control_write: Some(0x01),
        last_unsupported_command: None,
        last_unsupported_argument: None,
    };

    match state {
        PersistentCartState::Huc3 {
            rtc,
            rom_bank,
            ram_bank,
            ..
        } => {
            assert_eq!(rtc.current_minutes_of_day, 123);
            assert_eq!(rtc.event_minutes_of_day, 456);
            assert_eq!(rom_bank, 0x3F);
            assert_eq!(ram_bank, 0x02);
        }
        other => panic!("expected HuC-3 persistent state, got {other:?}"),
    }
}
