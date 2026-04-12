use super::*;

#[test]
fn ei_halt_with_a_pending_irq_services_once_and_returns_to_halt() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom_with_patches(
            &[0xFB, 0x76, 0x3C, 0x00],
            0x12,
            &[(0x0040, 0xD9)],
        ))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);

    step_machine_t_cycles(&mut machine, 8);

    assert!(!machine.cpu().ime());
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().pc, 0x0040);
    assert_eq!(machine.cpu().registers().sp, 0xFFFC);
    assert_eq!(machine.read_bus(0xFFFD), 0x01);
    assert_eq!(machine.read_bus(0xFFFC), 0x01);

    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert!(machine.cpu().ime());
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.cpu().registers().a, 0x01);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);
}

#[test]
fn ei_halt_followed_by_rst_still_returns_to_halt_before_executing_rst() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom_with_patches(
            &[0xFB, 0x76, 0xFF, 0x00],
            0x12,
            &[(0x0040, 0xD9)],
        ))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().pc, 0x0040);
    assert_eq!(machine.cpu().registers().sp, 0xFFFC);
    assert_eq!(machine.read_bus(0xFFFD), 0x01);
    assert_eq!(machine.read_bus(0xFFFC), 0x01);

    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert!(machine.cpu().ime());
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.cpu().registers().sp, 0xFFFE);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);
}

#[test]
fn halt_with_ime_enabled_wakes_on_a_later_irq_and_services_it() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0x00, 0x76, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x00);
    step_machine_t_cycles(&mut machine, 12);

    assert!(machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);

    machine.write_bus(0xFF0F, 0x01);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().pc, 0x0040);
    assert_eq!(machine.cpu().registers().sp, 0xFFFC);
    assert_eq!(machine.read_bus(0xFFFD), 0x01);
    assert_eq!(machine.read_bus(0xFFFC), 0x03);
}

#[test]
fn halt_with_ime_disabled_wakes_without_servicing_the_pending_irq() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x76, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x00);
    step_machine_t_cycles(&mut machine, 4);

    assert!(!machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);

    machine.write_bus(0xFF0F, 0x01);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}
