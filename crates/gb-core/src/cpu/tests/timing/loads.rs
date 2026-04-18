use super::*;

#[test]
fn register_only_and_hl_indirect_loads_have_distinct_timing_paths() {
    let startup_state = CpuStartupState {
        b: 0x12,
        c: 0x34,
        h: 0xC0,
        l: 0x00,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    };

    let mut register_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut register_bus = Bus::new(ConsoleModel::Dmg);
    let mut register_cartridge = build_test_cartridge(&[0x41]);
    register_cpu.apply_startup_state(startup_state);

    tick_cpu_n(
        &mut register_cpu,
        &mut register_bus,
        &mut register_cartridge,
        4,
    );

    assert_eq!(register_cpu.registers().b, 0x34);
    assert_eq!(
        register_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    let mut hl_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut hl_bus = Bus::new(ConsoleModel::Dmg);
    let mut hl_cartridge = build_test_cartridge(&[0x46]);
    hl_cpu.apply_startup_state(startup_state);
    hl_bus.write(0xC000, 0x77);

    tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

    assert_eq!(hl_cpu.registers().b, 0x12);
    assert_eq!(
        hl_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 0,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

    assert_eq!(hl_cpu.registers().b, 0x77);
    assert_eq!(hl_cpu.registers().pc, 0x0101);
    assert_eq!(
        hl_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn ld_bc_d16_fetches_low_then_high_in_order() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0x01, 0x34, 0x12]);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().b, 0x00);
    assert_eq!(cpu.registers().c, 0x00);
    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 1,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(cpu.registers().b, 0x12);
    assert_eq!(cpu.registers().c, 0x34);
    assert_eq!(cpu.registers().pc, 0x0103);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn ld_hl_d8_fetches_the_immediate_before_writing_memory() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0x36, 0x5A]);

    cpu.apply_startup_state(CpuStartupState {
        h: 0xC0,
        l: 0x00,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(bus.read(0xC000), 0x00);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 1,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(bus.read(0xC000), 0x5A);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn ld_a16_sp_writes_sp_little_endian_over_five_machine_cycles() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0x08, 0x00, 0xC0]);

    cpu.apply_startup_state(CpuStartupState {
        sp: 0x1234,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 12);

    assert_eq!(bus.read(0xC000), 0x00);
    assert_eq!(bus.read(0xC001), 0x00);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(bus.read(0xC000), 0x34);
    assert_eq!(bus.read(0xC001), 0x00);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 3,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(bus.read(0xC000), 0x34);
    assert_eq!(bus.read(0xC001), 0x12);
    assert_eq!(cpu.registers().pc, 0x0103);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn ld_sp_hl_uses_two_machine_cycles_and_preserves_flags() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xF9]);

    cpu.apply_startup_state(CpuStartupState {
        h: 0xC1,
        l: 0x23,
        f: FLAG_Z | FLAG_C,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().sp, 0xC123);
    assert_eq!(cpu.registers().f, FLAG_Z | FLAG_C);
    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}
