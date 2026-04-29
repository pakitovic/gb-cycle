use super::*;

#[test]
fn call_and_ret_use_bytewise_stack_transfers_in_order() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut cartridge =
        build_test_cartridge(&[0xCD, 0x08, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC9]);

    cpu.apply_startup_state(CpuStartupState {
        sp: 0xFFFE,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 20);

    assert_eq!(cpu.registers().sp, 0xFFFD);
    assert_eq!(bus.read(0xFFFD), 0x01);
    assert_eq!(bus.read(0xFFFC), 0x00);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 4,
            t_cycle: 0,
        }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::Write,
            access_address: Some(0xFFFD),
            idu_address: None,
            update_direction: None,
        })
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(cpu.registers().sp, 0xFFFC);
    assert_eq!(bus.read(0xFFFD), 0x01);
    assert_eq!(bus.read(0xFFFC), 0x03);
    assert_eq!(cpu.registers().pc, 0x0108);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: Some(0xFFFC),
            idu_address: Some(0xFFFC),
            update_direction: Some(CpuAddressUpdateDirection::Decrement),
        })
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 12);

    assert_eq!(cpu.registers().sp, 0xFFFE);
    assert_eq!(cpu.registers().pc, 0x0109);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0xFFFD),
            idu_address: Some(0xFFFE),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(cpu.registers().pc, 0x0103);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn push_and_pop_share_the_same_stack_byte_order_model() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut cartridge = build_test_cartridge(&[0xC5, 0xD1]);

    cpu.apply_startup_state(CpuStartupState {
        b: 0x12,
        c: 0x34,
        sp: 0xFFFE,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 16);

    assert_eq!(cpu.registers().sp, 0xFFFC);
    assert_eq!(bus.read(0xFFFD), 0x12);
    assert_eq!(bus.read(0xFFFC), 0x34);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: Some(0xFFFC),
            idu_address: Some(0xFFFC),
            update_direction: Some(CpuAddressUpdateDirection::Decrement),
        })
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 12);

    assert_eq!(cpu.registers().d, 0x12);
    assert_eq!(cpu.registers().e, 0x34);
    assert_eq!(cpu.registers().sp, 0xFFFE);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0xFFFD),
            idu_address: Some(0xFFFE),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );
}

#[test]
fn pop_af_masks_the_low_flag_nibble_across_the_full_blargg_special_loop() {
    for high in u8::MIN..=u8::MAX {
        for low in u8::MIN..=u8::MAX {
            let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
            let mut bus = Bus::new(ConsoleModel::GameBoy);
            let mut cartridge = build_test_cartridge(&[0xC5, 0xF1, 0xF5, 0xD1]);

            cpu.apply_startup_state(CpuStartupState {
                b: high,
                c: low,
                sp: 0xFFFE,
                pc: 0x0100,
                ..CpuStartupState::power_on_reset()
            });

            tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 56);

            assert_eq!(cpu.registers().d, high);
            assert_eq!(cpu.registers().e, low & 0xF0);
            assert_eq!(cpu.registers().sp, 0xFFFE);
            assert_eq!(
                cpu.execution_state(),
                CpuExecutionState::FetchOpcode { t_cycle: 0 }
            );
        }
    }
}
