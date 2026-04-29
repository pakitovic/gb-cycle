use super::*;

#[test]
fn hli_and_hld_transfer_forms_publish_combined_access_and_idu_events() {
    let mut load_cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut load_bus = Bus::new(ConsoleModel::GameBoy);
    let mut load_cartridge = build_test_cartridge(&[0x2A]);

    load_cpu.apply_startup_state(CpuStartupState {
        h: 0xC0,
        l: 0x00,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    load_bus.write(0xC000, 0x77);

    tick_cpu_n(&mut load_cpu, &mut load_bus, &mut load_cartridge, 8);

    assert_eq!(load_cpu.registers().a, 0x77);
    assert_eq!(load_cpu.hl(), 0xC001);
    assert_eq!(
        load_cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0xC000),
            idu_address: Some(0xC001),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );

    let mut store_cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut store_bus = Bus::new(ConsoleModel::GameBoy);
    let mut store_cartridge = build_test_cartridge(&[0x32]);

    store_cpu.apply_startup_state(CpuStartupState {
        a: 0x5A,
        h: 0xC0,
        l: 0x01,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut store_cpu, &mut store_bus, &mut store_cartridge, 8);

    assert_eq!(store_bus.read(0xC001), 0x5A);
    assert_eq!(store_cpu.hl(), 0xC000);
    assert_eq!(
        store_cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: Some(0xC001),
            idu_address: Some(0xC000),
            update_direction: Some(CpuAddressUpdateDirection::Decrement),
        })
    );
}

#[test]
fn inc_and_dec_register_pairs_publish_pure_idu_events() {
    let mut inc_cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut inc_bus = Bus::new(ConsoleModel::GameBoy);
    let mut inc_cartridge = build_test_cartridge(&[0x23]);

    inc_cpu.apply_startup_state(CpuStartupState {
        h: 0xFE,
        l: 0xFF,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut inc_cpu, &mut inc_bus, &mut inc_cartridge, 8);

    assert_eq!(inc_cpu.hl(), 0xFF00);
    assert_eq!(
        inc_cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(0xFF00),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );

    let mut dec_cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut dec_bus = Bus::new(ConsoleModel::GameBoy);
    let mut dec_cartridge = build_test_cartridge(&[0x3B]);

    dec_cpu.apply_startup_state(CpuStartupState {
        sp: 0xFE00,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut dec_cpu, &mut dec_bus, &mut dec_cartridge, 8);

    assert_eq!(dec_cpu.registers().sp, 0xFDFF);
    assert_eq!(
        dec_cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(0xFDFF),
            update_direction: Some(CpuAddressUpdateDirection::Decrement),
        })
    );
}

#[test]
fn ld_hl_sp_plus_signed_immediate_uses_three_machine_cycles_and_sets_flags_from_sp_math() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut cartridge = build_test_cartridge(&[0xF8, 0x08]);

    cpu.apply_startup_state(CpuStartupState {
        sp: 0xFFF8,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 1,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(cpu.hl(), 0x0000);
    assert_eq!(cpu.registers().sp, 0xFFF8);
    assert_eq!(cpu.registers().f, FLAG_H | FLAG_C);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn add_sp_signed_immediate_uses_four_machine_cycles_and_sets_flags_from_sp_math() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut cartridge = build_test_cartridge(&[0xE8, 0x08]);

    cpu.apply_startup_state(CpuStartupState {
        sp: 0xFFF8,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 1,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(cpu.registers().sp, 0xFFF8);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(cpu.registers().sp, 0x0000);
    assert_eq!(cpu.registers().f, FLAG_H | FLAG_C);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn add_sp_negative_signed_immediate_uses_four_machine_cycles_and_sets_flags_from_sp_math() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut cartridge = build_test_cartridge(&[0xE8, 0xF8]);

    cpu.apply_startup_state(CpuStartupState {
        sp: 0x0008,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 1,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(cpu.registers().sp, 0x0008);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(cpu.registers().sp, 0x0000);
    assert_eq!(cpu.registers().f, FLAG_H | FLAG_C);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}
