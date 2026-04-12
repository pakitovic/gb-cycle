use super::*;

#[test]
fn halt_bug_suppresses_the_next_pc_increment_without_servicing_the_irq() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x76, 0x3C, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(machine.cpu().registers().a, 0x02);
    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().a, 0x03);
    assert_eq!(machine.cpu().registers().pc, 0x0102);
}
