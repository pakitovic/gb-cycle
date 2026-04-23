use super::*;

fn execute_huc3_command(cartridge: &mut Huc3Cartridge, command: u8, argument: u8) {
    cartridge.write_rom(0x0000, 0x0B);
    cartridge.write_ram(0xA123, ((command & 0x07) << 4) | (argument & 0x0F));
    cartridge.write_rom(0x0000, 0x0D);
    cartridge.write_ram(0xBFFF, 0xFE);
}

#[test]
fn huc3_bank_registers_and_select_modes_keep_the_family_dedicated() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x05, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");
    let Some(CartridgeDevice::Huc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected HuC-3 cartridge");
    };

    assert_eq!(cartridge.read_rom(0x0000), 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);

    cartridge.write_rom(0x2000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);
    cartridge.write_rom(0x2000, 0x3F);
    assert_eq!(cartridge.read_rom(0x4000), 0x3F);

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_ram(0xA000, 0x22);
    assert_eq!(cartridge.read_ram(0xA000), 0x22);

    cartridge.write_rom(0x0000, 0x00);
    cartridge.write_ram(0xA000, 0x44);
    assert_eq!(cartridge.read_ram(0xA000), 0x22);

    cartridge.write_rom(0x0000, 0x0E);
    assert_eq!(cartridge.read_ram(0xA456), 0x80);
    cartridge.write_ram(0xA111, 0x01);
    assert!(cartridge.ir_emitter_on);

    cartridge.write_rom(0x0000, 0x07);
    assert_eq!(cartridge.read_ram(0xA000), 0xFF);
    cartridge.write_ram(0xA000, 0x5A);
    assert!(matches!(
        cartridge.select_mode,
        Huc3SelectMode::OpenBus(0x07)
    ));

    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.last_control_write, Some(0x01));
}

#[test]
fn huc3_mailbox_protocol_executes_commands_and_masks_d7() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");
    let Some(CartridgeDevice::Huc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected HuC-3 cartridge");
    };

    execute_huc3_command(&mut cartridge, 0x04, 0x04);
    execute_huc3_command(&mut cartridge, 0x05, 0x0A);
    assert_eq!(cartridge.access_address, 0xA4);

    cartridge.write_rom(0x0000, 0x0B);
    cartridge.write_ram(0xA000, 0xB7);
    assert_eq!(cartridge.mailbox.command, 0x03);
    assert_eq!(cartridge.mailbox.argument, 0x07);
    cartridge.write_rom(0x0000, 0x0D);
    assert_eq!(cartridge.read_ram(0xA000), 0x81);
    cartridge.write_ram(0xA000, 0xFE);
    assert_eq!(cartridge.mcu_ram[0xA4], 0x07);
    assert_eq!(cartridge.access_address, 0xA5);

    execute_huc3_command(&mut cartridge, 0x04, 0x04);
    execute_huc3_command(&mut cartridge, 0x05, 0x0A);
    execute_huc3_command(&mut cartridge, 0x01, 0x0F);
    cartridge.write_rom(0x0000, 0x0C);
    assert_eq!(cartridge.read_ram(0xA555), 0x97);
    assert_eq!(cartridge.access_address, 0xA5);

    execute_huc3_command(&mut cartridge, 0x06, 0x02);
    cartridge.write_rom(0x0000, 0x0C);
    assert_eq!(cartridge.read_ram(0xA999), 0xE1);
}

#[test]
fn huc3_extended_time_commands_copy_current_time_and_preserve_event_delta() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");
    let Some(CartridgeDevice::Huc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected HuC-3 cartridge");
    };

    cartridge.rtc = Huc3RtcState {
        current_minutes_of_day: 10,
        current_days: 1,
        current_subminute_seconds: 0,
        event_minutes_of_day: 30,
        event_days: 1,
    };
    cartridge.sync_tracked_rtc_locations();

    execute_huc3_command(&mut cartridge, 0x06, 0x00);
    assert_eq!(cartridge.read_triplet_nybbles(0x00), 10);
    assert_eq!(cartridge.read_triplet_nybbles(0x03), 1);
    assert_eq!(cartridge.mcu_ram[0x06], 0);

    cartridge.write_triplet_nybbles(0x00, 100);
    cartridge.write_triplet_nybbles(0x03, 2);
    execute_huc3_command(&mut cartridge, 0x06, 0x01);

    assert_eq!(cartridge.rtc.current_minutes_of_day, 100);
    assert_eq!(cartridge.rtc.current_days, 2);
    assert_eq!(cartridge.rtc.event_minutes_of_day, 120);
    assert_eq!(cartridge.rtc.event_days, 2);
    assert_eq!(cartridge.read_triplet_nybbles(0x10), 100);
    assert_eq!(cartridge.read_triplet_nybbles(0x13), 2);
    assert_eq!(cartridge.read_triplet_nybbles(0x58), 120);
    assert_eq!(cartridge.read_triplet_nybbles(0x5B), 2);
}

#[test]
fn huc3_persistence_and_external_access_follow_the_dedicated_contract() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x04, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");
    let Some(CartridgeDevice::Huc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected HuC-3 cartridge");
    };

    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::BankedRam { bank: 0 },
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::Storage,
            CartridgeExternalWriteBehavior::Ignored,
        )
    );

    cartridge.write_rom(0x0000, 0x0B);
    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::Huc3CommandMailbox,
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::OpenBus,
            CartridgeExternalWriteBehavior::Huc3MailboxCommandArgument,
        )
    );

    assert_eq!(
        cartridge.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        }
    );

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);
    execute_huc3_command(&mut cartridge, 0x04, 0x06);
    execute_huc3_command(&mut cartridge, 0x05, 0x00);
    execute_huc3_command(&mut cartridge, 0x03, 0x0A);
    cartridge.advance_rtc_seconds(125);

    let restored = match cartridge.persistent_state() {
        PersistentCartState::Huc3 {
            ram,
            mcu_ram,
            rtc,
            rom_bank,
            ram_bank,
            select_mode,
            access_address,
            mailbox_command,
            mailbox_argument,
            last_response_nybble,
            semaphore_ready,
            ir_emitter_on,
            ir_light_detected,
            last_control_write,
            last_unsupported_command,
            last_unsupported_argument,
        } => PersistentCartState::Huc3 {
            ram,
            mcu_ram,
            rtc,
            rom_bank,
            ram_bank,
            select_mode,
            access_address,
            mailbox_command,
            mailbox_argument,
            last_response_nybble,
            semaphore_ready,
            ir_emitter_on,
            ir_light_detected,
            last_control_write,
            last_unsupported_command,
            last_unsupported_argument,
        },
        other => panic!("expected HuC-3 state, got {other:?}"),
    };
    cartridge
        .restore_persistent_state(&restored)
        .expect("HuC-3 state should restore");
    assert_eq!(cartridge.persistent_state(), restored);
    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::Mbc3Ram {
            ram: vec![0; 32 * 1024]
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Huc3",
            actual: "Mbc3Ram",
        }),
    );

    let mut invalid = restored.clone();
    if let PersistentCartState::Huc3 { mcu_ram, .. } = &mut invalid {
        mcu_ram[7] = 0x1F;
    }
    assert_eq!(
        cartridge.restore_persistent_state(&invalid),
        Err(CartridgePersistentStateError::InvalidHuc3NibbleValue {
            index: 7,
            value: 0x1F,
        }),
    );
}

#[test]
fn huc3_trace_summary_surfaces_state_and_unsupported_commands() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");
    let (mut cartridge, _) = report.into_parts();

    cartridge.write_rom(0x2000, 0x3F);
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_rom(0x0000, 0x0B);
    cartridge.write_ram(0xA000, 0x6E);
    cartridge.write_rom(0x0000, 0x0D);
    cartridge.write_ram(0xA000, 0xFE);

    let trace = cartridge.scheduler_trace_message(&crate::scheduler::CycleContext::for_cycle(
        crate::scheduler::TCycle::new(123),
    ));
    assert!(trace.contains("state=Huc3"));
    assert!(trace.contains("select_mode=RtcSemaphore"));
    assert!(trace.contains("rom_bank_raw=0x3F"));
    assert!(trace.contains("ram_bank_raw=0x02"));
    assert!(trace.contains("last_unsupported=Some((6, 14))"));
}

#[test]
fn huc3_runtime_helpers_cover_remaining_modes_and_persistence_edges() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");
    let Some(CartridgeDevice::Huc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected HuC-3 cartridge");
    };

    let select_modes = [
        (Huc3SelectMode::RamReadOnly, 0x00),
        (Huc3SelectMode::RamReadWrite, 0x0A),
        (Huc3SelectMode::RtcCommandArgument, 0x0B),
        (Huc3SelectMode::RtcCommandResponse, 0x0C),
        (Huc3SelectMode::RtcSemaphore, 0x0D),
        (Huc3SelectMode::Ir, 0x0E),
        (Huc3SelectMode::OpenBus(0x07), 0x07),
    ];
    for (select_mode, raw_value) in select_modes {
        cartridge.select_mode = select_mode;
        let PersistentCartState::Huc3 { select_mode, .. } = cartridge.persistent_state() else {
            panic!("HuC-3 state should remain battery-backed");
        };
        assert_eq!(select_mode, raw_value);
    }

    cartridge.select_mode = Huc3SelectMode::RtcCommandArgument;
    cartridge.write_ram(0xA000, 0xB4);
    cartridge.select_mode = Huc3SelectMode::RtcCommandResponse;
    let mailbox_before = cartridge.mailbox;
    cartridge.write_ram(0xA000, 0x12);
    assert_eq!(cartridge.mailbox, mailbox_before);
    assert_eq!(cartridge.read_ram(0xA321), 0xB0);
    assert_eq!(
        cartridge.describe_external_access(0xA321),
        CartridgeExternalAccessInfo::new(
            0xA321,
            CartridgeExternalTarget::Huc3ResponseMailbox,
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::Huc3MailboxResponse,
            CartridgeExternalWriteBehavior::Ignored,
        )
    );

    cartridge.select_mode = Huc3SelectMode::RtcSemaphore;
    cartridge.mailbox.command = 0x07;
    cartridge.mailbox.argument = 0x09;
    cartridge.last_unsupported_command = None;
    cartridge.last_unsupported_argument = None;
    cartridge.write_ram(0xA000, 0x01);
    assert_eq!(cartridge.last_unsupported_command, None);
    cartridge.write_ram(0xA000, 0x00);
    assert_eq!(cartridge.last_unsupported_command, Some(0x07));
    assert_eq!(cartridge.last_unsupported_argument, Some(0x09));
    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::Huc3Semaphore,
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::Huc3SemaphoreReady,
            CartridgeExternalWriteBehavior::Huc3SemaphoreControl,
        )
    );

    cartridge.select_mode = Huc3SelectMode::Ir;
    cartridge.write_ram(0xA000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x80);
    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::IrRegister,
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::InfraredSensor,
            CartridgeExternalWriteBehavior::InfraredTransmitter,
        )
    );

    cartridge.select_mode = Huc3SelectMode::OpenBus(0x07);
    cartridge.write_ram(0xA000, 0x55);
    assert_eq!(cartridge.read_ram(0xA000), 0xFF);
    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::Huc3InvalidSelector(0x07),
            CartridgeExternalAvailability::Reserved,
            CartridgeExternalReadBehavior::OpenBus,
            CartridgeExternalWriteBehavior::Ignored,
        )
    );

    execute_huc3_command(&mut cartridge, 0x04, 0x00);
    execute_huc3_command(&mut cartridge, 0x05, 0x01);
    for nibble in [0x04, 0x03, 0x02, 0x05, 0x00, 0x00] {
        execute_huc3_command(&mut cartridge, 0x03, nibble);
    }
    assert_eq!(cartridge.rtc.current_minutes_of_day, 0x0234);
    assert_eq!(cartridge.rtc.current_days, 0x0005);

    execute_huc3_command(&mut cartridge, 0x04, 0x08);
    execute_huc3_command(&mut cartridge, 0x05, 0x05);
    for nibble in [0x06, 0x05, 0x04, 0x07, 0x00, 0x00] {
        execute_huc3_command(&mut cartridge, 0x03, nibble);
    }
    assert_eq!(cartridge.rtc.event_minutes_of_day, 0x0456);
    assert_eq!(cartridge.rtc.event_days, 0x0007);

    let persisted = cartridge.persistent_state();
    let mut wrong_ram_len = persisted.clone();
    if let PersistentCartState::Huc3 { ram, .. } = &mut wrong_ram_len {
        ram.pop();
    }
    assert_eq!(
        cartridge.restore_persistent_state(&wrong_ram_len),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 32 * 1024 - 1,
        }),
    );

    cartridge.has_battery = false;
    assert_eq!(cartridge.persistent_state(), PersistentCartState::None);
    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        cartridge.restore_persistent_state(&persisted),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Huc3",
        }),
    );
}

#[test]
fn huc3_device_dispatch_and_external_access_accessors_cover_the_dedicated_runtime() {
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");
    let mut device = report
        .cartridge()
        .device
        .clone()
        .expect("device should exist");

    assert_eq!(device.header().cartridge_type, 0xFE);
    assert_eq!(device.classification().detected_name(), "HuC-3");
    assert_eq!(device.read_rom(0x4000), 0x00);

    device.write_rom(0x2000, 0x02);
    assert_eq!(device.read_rom(0x4000), 0x02);
    device.write_rom(0x0000, 0x0A);
    device.write_ram(0xA000, 0x66);
    assert_eq!(device.read_ram(0xA000), 0x66);
    assert_eq!(device.read_ram_timed(0xA000, TCycle::new(8)), 0x66);

    device.write_rom(0x0000, 0x0D);
    device.write_ram_timed(0xA000, 0x01, TCycle::new(9));
    device.advance_rtc_seconds(61);

    let info = device
        .describe_external_access(0xA000)
        .with_rtc_access_ready_at(Some(TCycle::new(11)));
    assert_eq!(info.address(), 0xA000);
    assert_eq!(info.target(), CartridgeExternalTarget::Huc3Semaphore);
    assert_eq!(
        info.availability(),
        CartridgeExternalAvailability::Accessible
    );
    assert_eq!(
        info.read_behavior(),
        CartridgeExternalReadBehavior::Huc3SemaphoreReady
    );
    assert_eq!(
        info.write_behavior(),
        CartridgeExternalWriteBehavior::Huc3SemaphoreControl
    );
    assert_eq!(info.rtc_access_ready_at(), Some(TCycle::new(11)));

    let Some(CartridgeDevice::Huc3(cartridge)) = Some(device) else {
        panic!("expected HuC-3 device");
    };
    assert_eq!(cartridge.rtc.current_minutes_of_day, 1);
    assert_eq!(cartridge.rtc.current_subminute_seconds, 1);

    let mut zero_bank_count = cartridge.clone();
    zero_bank_count.header.rom_size.bank_count = None;
    zero_bank_count.rom_bank = 0x7F;
    assert_eq!(zero_bank_count.read_rom(0x4000), 0x00);
}
