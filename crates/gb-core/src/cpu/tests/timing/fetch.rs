use super::*;

#[test]
fn opcode_fetch_reads_bus_at_pc_on_the_fourth_t_cycle() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xCB]);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            opcode: 0xCB,
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
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut operations = Vec::new();

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.ime = false;

    for _ in 0..4 {
        cpu.tick_t_cycle(|operation| {
            operations.push(operation);
            match operation {
                CpuBusOperation::Read { address } => {
                    assert_eq!(address, 0x0100);
                    Some(0x10)
                }
                CpuBusOperation::StopWakeLineAsserted => Some(0x01),
                CpuBusOperation::PendingInterruptMask => Some(0x01),
                other => panic!("unexpected STOP fetch operation: {other:?}"),
            }
        });
    }

    assert_eq!(
        operations,
        vec![
            CpuBusOperation::Read { address: 0x0100 },
            CpuBusOperation::StopWakeLineAsserted,
            CpuBusOperation::PendingInterruptMask,
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
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut operations = Vec::new();

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.ime = true;

    for _ in 0..4 {
        cpu.tick_t_cycle(|operation| {
            operations.push(operation);
            match operation {
                CpuBusOperation::Read { address } => {
                    assert_eq!(address, 0x0100);
                    Some(0x10)
                }
                CpuBusOperation::StopWakeLineAsserted => Some(0x01),
                other => panic!("unexpected IME=1 STOP fetch operation: {other:?}"),
            }
        });
    }

    assert_eq!(
        operations,
        vec![
            CpuBusOperation::Read { address: 0x0100 },
            CpuBusOperation::StopWakeLineAsserted,
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
fn invalid_opcode_hole_enters_an_explicit_diagnostic_trap() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
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
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
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
