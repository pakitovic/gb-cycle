use super::*;

#[test]
fn inc_hl_uses_distinct_read_and_write_machine_cycles() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0x34]);

    cpu.apply_startup_state(CpuStartupState {
        h: 0xC0,
        l: 0x00,
        f: FLAG_C,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    bus.write(0xC000, 0x0F);

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(bus.read(0xC000), 0x0F);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            opcode: 0x34,
            step: 1,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(bus.read(0xC000), 0x10);
    assert_eq!(cpu.registers().f, FLAG_H | FLAG_C);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn add_a_d8_updates_flags_from_the_fetched_immediate() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xC6, 0x01]);

    cpu.apply_startup_state(CpuStartupState {
        a: 0x0F,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().a, 0x10);
    assert_eq!(cpu.registers().f, FLAG_H);
    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}
