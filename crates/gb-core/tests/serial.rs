use gb_core::{ConsoleModel, Machine, MachineConfig, SerialPeer, SerialTransferState, StartupMode};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn step_machine_t_cycles(machine: &mut Machine, steps: usize) {
    for _ in 0..steps {
        machine.step_t_cycle();
    }
}

fn build_test_rom(program: &[u8], patches: &[(usize, u8)]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    for &(address, value) in patches {
        rom[address] = value;
    }
    rom
}

#[test]
fn dmg_master_serial_shifts_every_512_t_cycles_and_completes_on_the_eighth_pulse() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    step_machine_t_cycles(&mut machine, 52);
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
fn external_serial_clock_pulses_queued_before_sc7_do_not_replay_into_a_later_transfer() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.queue_external_serial_clock();
    step_machine_t_cycles(&mut machine, 1);

    machine.write_bus(0xFF01, 0x81);
    machine.write_bus(0xFF02, 0x80);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF01), 0x81);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );

    machine.queue_external_serial_clock();
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF01), 0x03);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
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

#[test]
fn boot_aligned_serial_irq_matches_the_mooneye_interrupt_window() {
    let program = vec![
        0x3E, 0x08, // ld a, $08
        0xE0, 0xFF, // ldh ($FF), a
        0xAF, // xor a
        0xE0, 0x0F, // ldh ($0F), a
        0xAF, // xor a
        0x47, // ld b, a
        0x4F, // ld c, a
        0x57, // ld d, a
        0x5F, // ld e, a
        0x67, // ld h, a
        0x6F, // ld l, a
        0x3E, 0x81, // ld a, $81
        0xE0, 0x02, // ldh ($02), a
        0xFB, // ei
    ];

    let mut body = program;
    for _ in 0..2000 {
        body.extend_from_slice(&[0x3C, 0x04, 0x0C, 0x14, 0x1C, 0x24, 0x2C]);
    }

    let mut patches = Vec::with_capacity(body.len());
    for (offset, byte) in body.iter().copied().enumerate() {
        patches.push((0x0150 + offset, byte));
    }

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00, 0xC3, 0x50, 0x01], &patches))
        .expect("NoMBC test ROM should load");

    for _ in 0..20_000 {
        machine.step_t_cycle();
        if matches!(
            machine.cpu().execution_state(),
            gb_core::CpuExecutionState::ServiceInterrupt {
                source: gb_core::InterruptSource::Serial,
                step: 0,
                t_cycle: 0,
            }
        ) {
            let registers = machine.cpu().registers();
            assert_eq!(registers.a, 0x12);
            assert_eq!(registers.b, 0x91);
            assert_eq!(registers.c, 0x90);
            assert_eq!(registers.d, 0x90);
            assert_eq!(registers.e, 0x90);
            assert_eq!(registers.h, 0x90);
            assert_eq!(registers.l, 0x90);
            return;
        }
    }

    panic!("serial interrupt was not accepted within the debug window");
}
