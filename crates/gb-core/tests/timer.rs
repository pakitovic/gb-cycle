use gb_core::{ConsoleModel, CpuExecutionState, Machine, MachineConfig, StartupMode};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn build_test_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn step_machine_t_cycles(machine: &mut Machine, steps: usize) {
    for _ in 0..steps {
        machine.step_t_cycle();
    }
}

#[test]
fn skip_boot_div_continues_from_the_documented_hidden_counter_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    assert_eq!(machine.read_bus(0xFF04), 0xAB);

    step_machine_t_cycles(&mut machine, 256);

    assert_eq!(machine.read_bus(0xFF04), 0xAC);
}

#[test]
fn timer_request_becomes_visible_only_after_the_reload_delay() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF05, 0xFF);
    machine.write_bus(0xFF06, 0x42);
    machine.write_bus(0xFF07, 0x05);

    step_machine_t_cycles(&mut machine, 15);

    assert_eq!(machine.read_bus(0xFF05), 0xFF);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 3);

    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF05), 0x42);
    assert_eq!(machine.read_bus(0xFF0F), 0xE4);
}

#[test]
fn halted_cpu_services_timer_irq_only_after_the_reload_delay() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0x00, 0x76]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFFFF, 0x04);

    step_machine_t_cycles(&mut machine, 12);

    assert!(machine.cpu().ime());
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);

    machine.write_bus(0xFF04, 0x00);
    machine.write_bus(0xFF05, 0xFF);
    machine.write_bus(0xFF06, 0x66);
    machine.write_bus(0xFF07, 0x05);

    step_machine_t_cycles(&mut machine, 19);

    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);

    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::Timer,
            step: 0,
            t_cycle: 0,
        }
    );
}
