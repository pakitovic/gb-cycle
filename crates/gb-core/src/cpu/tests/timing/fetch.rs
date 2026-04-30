use super::*;

#[test]
fn opcode_fetch_reads_bus_at_pc_on_the_fourth_t_cycle() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut cartridge = build_test_cartridge(&[0xCB]);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(cpu.current_opcode(), Some(0xCB));
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0x0100),
            idu_address: Some(0x0101),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );
}

#[test]
fn stop_nop_like_completes_on_the_opcode_fetch_machine_cycle() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut operations = Vec::new();

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_disabled();

    for _ in 0..4 {
        cpu.tick_t_cycle(|operation| {
            operations.push(operation);
            match operation {
                CpuExternalOperation::Bus(CpuBusOperation::Read { address }) => {
                    assert_eq!(address, 0x0100);
                    Some(0x10)
                }
                CpuExternalOperation::CgbSpeedSwitchPrepared => Some(0x00),
                CpuExternalOperation::StopWakeLineAsserted => Some(0x01),
                CpuExternalOperation::PendingInterruptMask => Some(0x01),
                other => panic!("unexpected STOP fetch operation: {other:?}"),
            }
        });
    }

    assert_eq!(
        operations,
        vec![
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0100 }),
            CpuExternalOperation::CgbSpeedSwitchPrepared,
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::PendingInterruptMask,
        ]
    );
    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(cpu.current_opcode(), None);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn stop_with_ime_enabled_and_wake_high_also_completes_on_the_opcode_fetch_machine_cycle() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut operations = Vec::new();

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();

    for _ in 0..4 {
        cpu.tick_t_cycle(|operation| {
            operations.push(operation);
            match operation {
                CpuExternalOperation::Bus(CpuBusOperation::Read { address }) => {
                    assert_eq!(address, 0x0100);
                    Some(0x10)
                }
                CpuExternalOperation::CgbSpeedSwitchPrepared => Some(0x00),
                CpuExternalOperation::StopWakeLineAsserted => Some(0x01),
                other => panic!("unexpected IME=1 STOP fetch operation: {other:?}"),
            }
        });
    }

    assert_eq!(
        operations,
        vec![
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0100 }),
            CpuExternalOperation::CgbSpeedSwitchPrepared,
            CpuExternalOperation::StopWakeLineAsserted,
        ]
    );
    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(cpu.current_opcode(), None);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn stop_real_path_fetches_the_padding_byte_before_entering_stopped() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut operations = Vec::new();

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_disabled();

    for _ in 0..8 {
        cpu.tick_t_cycle(|operation| {
            operations.push(operation);
            match operation {
                CpuExternalOperation::Bus(CpuBusOperation::Read { address }) => match address {
                    0x0100 => Some(0x10),
                    0x0101 => Some(0x00),
                    other => panic!("unexpected STOP real-path read address: {other:#06X}"),
                },
                CpuExternalOperation::CgbSpeedSwitchPrepared => Some(0x00),
                CpuExternalOperation::StopWakeLineAsserted => Some(0x00),
                CpuExternalOperation::PendingInterruptMask => Some(0x00),
                other => panic!("unexpected STOP real-path operation: {other:?}"),
            }
        });
    }

    assert_eq!(
        operations,
        vec![
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0100 }),
            CpuExternalOperation::CgbSpeedSwitchPrepared,
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::CgbSpeedSwitchPrepared,
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::PendingInterruptMask,
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0101 }),
        ]
    );
    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(cpu.current_opcode(), None);
    assert_eq!(cpu.execution_state(), CpuExecutionState::Stopped);
}

#[test]
fn stop_zombie_path_skips_the_padding_fetch_and_keeps_pc_at_post_opcode() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut operations = Vec::new();

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_disabled();

    for _ in 0..8 {
        cpu.tick_t_cycle(|operation| {
            operations.push(operation);
            match operation {
                CpuExternalOperation::Bus(CpuBusOperation::Read { address }) => {
                    assert_eq!(address, 0x0100);
                    Some(0x10)
                }
                CpuExternalOperation::CgbSpeedSwitchPrepared => Some(0x00),
                CpuExternalOperation::StopWakeLineAsserted => Some(0x00),
                CpuExternalOperation::PendingInterruptMask => Some(0x01),
                other => panic!("unexpected STOP zombie-path operation: {other:?}"),
            }
        });
    }

    assert_eq!(
        operations,
        vec![
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0100 }),
            CpuExternalOperation::CgbSpeedSwitchPrepared,
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::CgbSpeedSwitchPrepared,
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::PendingInterruptMask,
        ]
    );
    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(cpu.current_opcode(), None);
    assert_eq!(cpu.execution_state(), CpuExecutionState::ZombieStopped);
}

#[test]
fn stop_halt_like_path_fetches_the_padding_byte_before_halting() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut interrupts = InterruptController::new(ConsoleModel::GameBoy);
    let mut joypad = Joypad::new(ConsoleModel::GameBoy);
    let mut operations = Vec::new();

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_disabled();

    for _ in 0..8 {
        cpu.tick_t_cycle(|operation| {
            operations.push(operation);
            match operation {
                CpuExternalOperation::Bus(CpuBusOperation::Read { address }) => match address {
                    0x0100 => Some(0x10),
                    0x0101 => Some(0x00),
                    other => panic!("unexpected STOP halt-like read address: {other:#06X}"),
                },
                CpuExternalOperation::CgbSpeedSwitchPrepared => Some(0x00),
                CpuExternalOperation::StopWakeLineAsserted => Some(0x01),
                CpuExternalOperation::PendingInterruptMask => Some(0x00),
                other => panic!("unexpected STOP halt-like operation: {other:?}"),
            }
        });
    }

    assert_eq!(
        operations,
        vec![
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0100 }),
            CpuExternalOperation::CgbSpeedSwitchPrepared,
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::PendingInterruptMask,
            CpuExternalOperation::CgbSpeedSwitchPrepared,
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::PendingInterruptMask,
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0101 }),
        ]
    );
    assert_eq!(cpu.registers().pc, 0x0102);
    assert!(cpu.halt_request_pending_for_test());
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert_eq!(cpu.execution_state(), CpuExecutionState::Halted);
}

#[test]
fn invalid_opcode_hole_enters_an_explicit_diagnostic_trap() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut cartridge = build_test_cartridge(&[0xD3]);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(cpu.current_opcode(), Some(0xD3));
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::InvalidOpcode {
                opcode: 0xD3,
                address: 0x0100,
            },
        }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0x0100),
            idu_address: Some(0x0101),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::InvalidOpcode {
                opcode: 0xD3,
                address: 0x0100,
            },
        }
    );
    assert_eq!(cpu.last_address_event(), None);
}

#[test]
fn cb_set_opcode_executes_as_a_normal_prefixed_instruction() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut cartridge = build_test_cartridge(&[0xCB, 0xFF]);

    cpu.apply_startup_state(CpuStartupState {
        f: FLAG_Z | FLAG_C,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(cpu.registers().a, 0x80);
    assert_eq!(cpu.registers().f, FLAG_Z | FLAG_C);
    assert_eq!(cpu.current_opcode(), None);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0x0101),
            idu_address: Some(0x0102),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );
}
