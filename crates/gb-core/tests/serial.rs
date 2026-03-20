use gb_core::{ConsoleModel, Machine, MachineConfig, SerialPeer, SerialTransferState, StartupMode};

fn step_machine_t_cycles(machine: &mut Machine, steps: usize) {
    for _ in 0..steps {
        machine.step_t_cycle();
    }
}

#[test]
fn dmg_master_serial_shifts_every_512_t_cycles_and_completes_on_the_eighth_pulse() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF01, 0x81);
    machine.write_bus(0xFF02, 0x81);

    step_machine_t_cycles(&mut machine, 511);
    assert_eq!(machine.read_bus(0xFF01), 0x81);
    assert_eq!(machine.read_bus(0xFF02), 0xFF);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
    assert_eq!(machine.read_bus(0xFF0F) & 0x08, 0x00);

    step_machine_t_cycles(&mut machine, 1);
    assert_eq!(machine.read_bus(0xFF01), 0x03);
    assert_eq!(machine.read_bus(0xFF02), 0xFF);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
    assert_eq!(machine.read_bus(0xFF0F) & 0x08, 0x00);

    step_machine_t_cycles(&mut machine, (7 * 512) - 1);
    assert_eq!(machine.read_bus(0xFF02), 0xFF);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 7 }
    );
    assert_eq!(machine.read_bus(0xFF0F) & 0x08, 0x00);

    step_machine_t_cycles(&mut machine, 1);
    assert_eq!(machine.read_bus(0xFF01), 0xFF);
    assert_eq!(machine.read_bus(0xFF02), 0x7F);
    assert_eq!(machine.serial().transfer_state(), SerialTransferState::Idle);
    assert_eq!(machine.read_bus(0xFF0F) & 0x08, 0x08);
}

#[test]
fn serial_slave_mode_does_not_advance_without_external_clock_pulses() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF01, 0xA5);
    machine.write_bus(0xFF02, 0x80);

    step_machine_t_cycles(&mut machine, 8 * 512);

    assert_eq!(machine.read_bus(0xFF01), 0xA5);
    assert_eq!(machine.read_bus(0xFF02), 0xFE);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
    assert_eq!(machine.read_bus(0xFF0F) & 0x08, 0x00);
}

#[test]
fn external_serial_clock_pulses_advance_slave_mode_one_shift_each() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF01, 0x81);
    machine.write_bus(0xFF02, 0x80);

    machine.queue_external_serial_clock();
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF01), 0x03);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );

    machine.queue_external_serial_clock();
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF01), 0x07);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 2 }
    );
}

#[test]
fn loopback_peer_returns_the_original_byte_after_eight_internal_shifts() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.set_serial_peer(SerialPeer::Loopback);
    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF01, 0x96);
    machine.write_bus(0xFF02, 0x81);

    step_machine_t_cycles(&mut machine, 8 * 512);

    assert_eq!(machine.read_bus(0xFF01), 0x96);
    assert_eq!(machine.read_bus(0xFF02), 0x7F);
    assert_eq!(machine.serial().transfer_state(), SerialTransferState::Idle);
    assert_eq!(machine.read_bus(0xFF0F) & 0x08, 0x08);
}

#[test]
fn machine_exposes_completed_serial_output_bytes_for_host_side_capture() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF01, 0x41);
    machine.write_bus(0xFF02, 0x81);

    step_machine_t_cycles(&mut machine, 8 * 512);

    assert_eq!(machine.take_serial_output_bytes(), vec![0x41]);
    assert!(machine.take_serial_output_bytes().is_empty());
}
