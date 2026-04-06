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
fn unsupported_opcode_enters_an_explicit_diagnostic_trap() {
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
            trap: CpuDiagnosticTrap::UnsupportedOpcode {
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
            trap: CpuDiagnosticTrap::UnsupportedOpcode {
                opcode: 0xD3,
                address: 0x0100,
            },
        }
    );
    assert_eq!(cpu.last_address_event(), None);
}

#[test]
fn cb_set_opcode_executes_instead_of_entering_a_diagnostic_trap() {
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
