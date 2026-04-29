use super::*;

#[test]
fn unexpected_machine_steps_stall_in_place_instead_of_mutating_cpu_state() {
    let cases = [
        (
            0x06,
            CpuInstructionKind::LoadRegisterImmediate {
                target: Register8::B,
            },
            1,
        ),
        (
            0x01,
            CpuInstructionKind::LoadRegisterPairImmediate {
                target: Register16::BC,
            },
            2,
        ),
        (
            0x46,
            CpuInstructionKind::LoadRegisterFromHl {
                target: Register8::B,
            },
            1,
        ),
        (
            0x70,
            CpuInstructionKind::StoreRegisterToHl {
                source: Register8::B,
            },
            1,
        ),
        (0x36, CpuInstructionKind::StoreImmediateToHl, 2),
        (
            0x2A,
            CpuInstructionKind::LoadAFromHlWithUpdate {
                direction: CpuAddressUpdateDirection::Increment,
            },
            1,
        ),
        (
            0x32,
            CpuInstructionKind::StoreAToHlWithUpdate {
                direction: CpuAddressUpdateDirection::Decrement,
            },
            1,
        ),
        (0xFA, CpuInstructionKind::LoadAFromImmediate16Address, 3),
        (0xEA, CpuInstructionKind::StoreAToImmediate16Address, 3),
        (0x08, CpuInstructionKind::StoreSpToImmediate16, 4),
        (0xF8, CpuInstructionKind::LoadHlFromSpPlusImmediate, 2),
        (0xE8, CpuInstructionKind::AddSpImmediate, 3),
        (0xF9, CpuInstructionKind::LoadSpFromHl, 1),
        (
            0x39,
            CpuInstructionKind::AddHl {
                source: Register16::SP,
            },
            1,
        ),
        (
            0x33,
            CpuInstructionKind::IncrementRegisterPair {
                target: Register16::SP,
            },
            1,
        ),
        (
            0x3B,
            CpuInstructionKind::DecrementRegisterPair {
                target: Register16::SP,
            },
            1,
        ),
        (0x34, CpuInstructionKind::IncrementHlMemory, 2),
        (0x35, CpuInstructionKind::DecrementHlMemory, 2),
        (
            0xFE,
            CpuInstructionKind::AluImmediate {
                operation: AluOperation::Compare,
            },
            1,
        ),
        (
            0xB6,
            CpuInstructionKind::AluFromHl {
                operation: AluOperation::Or,
            },
            1,
        ),
        (
            0x38,
            CpuInstructionKind::ConditionalRelativeJump {
                condition: ConditionCode::C,
            },
            2,
        ),
        (
            0xDA,
            CpuInstructionKind::ConditionalAbsoluteJump {
                condition: ConditionCode::C,
            },
            3,
        ),
        (
            0xDC,
            CpuInstructionKind::ConditionalCall {
                condition: ConditionCode::C,
            },
            5,
        ),
        (
            0xD8,
            CpuInstructionKind::ConditionalReturn {
                condition: ConditionCode::C,
            },
            4,
        ),
        (0xD9, CpuInstructionKind::ReturnFromInterrupt, 3),
        (0x10, CpuInstructionKind::Stop, 1),
        (0xDF, CpuInstructionKind::Restart { vector: 0x0018 }, 3),
        (
            0xF5,
            CpuInstructionKind::PushRegisterPair {
                source: StackRegister16::AF,
            },
            3,
        ),
        (
            0xF1,
            CpuInstructionKind::PopRegisterPair {
                target: StackRegister16::AF,
            },
            2,
        ),
        (0xCB, CpuInstructionKind::CbPrefixed, 3),
    ];

    for (opcode, kind, step) in cases {
        let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
        cpu.in_flight.opcode = Some(opcode);
        cpu.in_flight.kind = Some(kind);
        cpu.execution_state = CpuExecutionState::Execute { step, t_cycle: 0 };

        cpu.complete_execute_machine_cycle(step, &mut |_| Some(0xFF));

        assert_eq!(
            cpu.execution_state,
            CpuExecutionState::Execute {
                step,
                t_cycle: LAST_MACHINE_CYCLE_T,
            },
        );
    }

    for step in [1_u8, 2, 3] {
        let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
        cpu.in_flight.opcode = Some(0xCB);
        cpu.in_flight.kind = Some(CpuInstructionKind::CbPrefixed);
        cpu.execution_state = CpuExecutionState::Execute { step, t_cycle: 0 };
        cpu.in_flight.cb_instruction_kind = None;

        cpu.complete_execute_machine_cycle(step, &mut |_| Some(0xFF));

        assert_eq!(
            cpu.execution_state,
            CpuExecutionState::Execute {
                step,
                t_cycle: LAST_MACHINE_CYCLE_T,
            },
        );
    }

    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    cpu.complete_interrupt_service_machine_cycle(InterruptSource::Serial, 5, &mut |_| None);
    assert_eq!(
        cpu.execution_state,
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::Serial,
            step: 5,
            t_cycle: 0,
        },
    );
}

#[test]
fn startup_state_resets_live_registers_and_fetch_state() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    let startup_state = CpuStartupState {
        a: 0x01,
        f: 0xB0,
        b: 0x00,
        c: 0x13,
        d: 0x00,
        e: 0xD8,
        h: 0x01,
        l: 0x4D,
        sp: 0xFFFE,
        pc: 0x0100,
    };

    cpu.apply_startup_state(startup_state);

    assert_eq!(cpu.status(), CpuStatus::Ready);
    assert_eq!(cpu.startup_state(), startup_state);
    assert_eq!(
        cpu.registers(),
        CpuRegisters::from_startup_state(startup_state)
    );
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(cpu.current_opcode(), None);
    assert!(!cpu.ime());
    assert!(!cpu.delayed_ime_enable());
}

#[test]
fn snapshot_derives_current_opcode_from_the_in_flight_instruction_record() {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);

    cpu.in_flight.opcode = Some(0xCB);
    cpu.in_flight.kind = Some(CpuInstructionKind::CbPrefixed);

    let snapshot = cpu.snapshot();
    assert_eq!(snapshot.current_opcode, Some(0xCB));

    cpu.clear_in_flight_instruction_state();

    let cleared_snapshot = cpu.snapshot();
    assert_eq!(cleared_snapshot.current_opcode, None);
}
