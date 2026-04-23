use super::*;

#[test]
fn huc1_bank_registers_drive_rom_ram_and_ir_modes_without_mbc1_ram_enable_gating() {
    let report = CartridgeSlot::load(
        build_banked_huc1_rom(0x05, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC1 should load");
    let Some(CartridgeDevice::Huc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected HuC1 cartridge");
    };

    assert_eq!(cartridge.read_rom(0x0000), 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);

    cartridge.write_rom(0x2000, 0x3F);
    assert_eq!(cartridge.read_rom(0x4000), 0x3F);

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_ram(0xA000, 0x22);
    assert_eq!(cartridge.read_ram(0xA000), 0x22);

    cartridge.write_rom(0x0000, 0x0E);
    assert_eq!(cartridge.read_ram(0xA000), 0xC0);
    cartridge.write_ram(0xA000, 0x01);
    assert!(cartridge.ir_emitter_on);

    cartridge.write_rom(0x0000, 0x00);
    assert_eq!(cartridge.read_ram(0xA000), 0x22);

    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);
    assert_eq!(cartridge.read_ram(0xA000), 0x33);

    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x3F);
    assert_eq!(cartridge.read_ram(0xA000), 0x33);
}

#[test]
fn huc1_persistence_and_external_access_follow_the_dedicated_mapper_contract() {
    let report = CartridgeSlot::load(
        build_banked_huc1_rom(0x04, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC1 should load");
    let Some(CartridgeDevice::Huc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected HuC1 cartridge");
    };

    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::BankedRam { bank: 0 },
            CartridgeExternalAvailability::Accessible,
            CartridgeExternalReadBehavior::Storage,
            CartridgeExternalWriteBehavior::Storage,
        )
    );

    cartridge.write_rom(0x0000, 0x0E);
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

    assert_eq!(
        cartridge.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        }
    );

    cartridge.write_rom(0x0000, 0x00);
    cartridge.write_rom(0x4000, 0x01);
    cartridge.write_ram(0xA000, 0x44);
    let restored = match cartridge.persistent_state() {
        PersistentCartState::Huc1Ram { ram } => PersistentCartState::Huc1Ram { ram },
        other => panic!("expected HuC1 RAM state, got {other:?}"),
    };
    cartridge
        .restore_persistent_state(&restored)
        .expect("HuC1 RAM state should restore");
    assert_eq!(cartridge.persistent_state(), restored);
    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::Mbc1Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Huc1Ram",
            actual: "Mbc1Ram",
        }),
    );
    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::Huc1Ram { ram: vec![0; 8] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 8,
        }),
    );
}

#[test]
fn huc1_scheduler_trace_surfaces_mode_and_bank_state_for_hang_debugging() {
    let report = CartridgeSlot::load(
        build_banked_huc1_rom(0x05, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC1 should load");
    let (mut cartridge, _) = report.into_parts();

    cartridge.write_rom(0x2000, 0x3F);
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_rom(0x0000, 0x0E);
    cartridge.write_ram(0xA000, 0x01);

    let trace = cartridge.scheduler_trace_message(&crate::scheduler::CycleContext::for_cycle(
        crate::scheduler::TCycle::new(123),
    ));
    assert!(trace.contains("state=Huc1"));
    assert!(trace.contains("io_mode=Ir"));
    assert!(trace.contains("rom_bank_raw=0x3F"));
    assert!(trace.contains("effective_rom_bank=0x3F"));
    assert!(trace.contains("ram_bank_raw=0x02"));
    assert!(trace.contains("effective_ram_bank=2"));
    assert!(trace.contains("ir_emitter_on=true"));
}
