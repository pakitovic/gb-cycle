use super::*;

#[test]
fn execute_without_instruction_kind_restalls_last_machine_cycle() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);

    cpu.complete_execute_machine_cycle(0xAA, 2, &mut |_| None);

    assert_eq!(
        cpu.execution_state,
        CpuExecutionState::Execute {
            opcode: 0xAA,
            step: 2,
            t_cycle: LAST_MACHINE_CYCLE_T,
        },
    );
}

#[test]
fn execute_add_hl_variants_cover_all_remaining_16bit_sources() {
    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0x0001);
    cpu.registers.sp = 0x0001;
    cpu.instruction_kind = Some(CpuInstructionKind::AddHl {
        source: Register16::SP,
    });
    cpu.complete_execute_machine_cycle(0x39, 0, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0002);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::BC, 0x0001);
    cpu.write_register16(Register16::HL, 0x0001);
    cpu.instruction_kind = Some(CpuInstructionKind::AddHl {
        source: Register16::BC,
    });
    cpu.complete_execute_machine_cycle(0x09, 0, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0002);

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::DE, 0x0002);
    cpu.write_register16(Register16::HL, 0x0001);
    cpu.instruction_kind = Some(CpuInstructionKind::AddHl {
        source: Register16::DE,
    });
    cpu.complete_execute_machine_cycle(0x19, 0, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0003);
}

#[test]
fn execute_memory_and_alu_variants_cover_remaining_private_paths() {
    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC000);
    cpu.instruction_kind = Some(CpuInstructionKind::DecrementHlMemory);
    cpu.complete_execute_machine_cycle(0x35, 0, &mut |_| Some(0x10));
    cpu.complete_execute_machine_cycle(0x35, 1, &mut |_| None);
    assert_eq!(cpu.registers.f, FLAG_N | FLAG_H);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.a = 0x0F;
    cpu.instruction_kind = Some(CpuInstructionKind::AluImmediate {
        operation: AluOperation::Add,
    });
    cpu.complete_execute_machine_cycle(0xC6, 0, &mut |_| Some(0x01));
    assert_eq!(cpu.registers.a, 0x10);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.a = 0xF0;
    cpu.write_register16(Register16::HL, 0xC100);
    cpu.instruction_kind = Some(CpuInstructionKind::AluFromHl {
        operation: AluOperation::And,
    });
    cpu.complete_execute_machine_cycle(0xA6, 0, &mut |_| Some(0x0F));
    assert_eq!(cpu.registers.a, 0x00);
    assert_eq!(cpu.registers.f, FLAG_Z | FLAG_H);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());
}

#[test]
fn execute_load_variants_cover_remaining_private_paths() {
    let mut cpu = power_on_cpu();
    cpu.instruction_kind = Some(CpuInstructionKind::LoadRegisterImmediate {
        target: Register8::B,
    });
    cpu.complete_execute_machine_cycle(0x06, 0, &mut |operation| {
        assert_eq!(operation, CpuBusOperation::Read { address: 0x0000 });
        Some(0x5A)
    });
    assert_eq!(cpu.registers.b, 0x5A);
    assert_eq!(cpu.registers.pc, 0x0001);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.instruction_kind = Some(CpuInstructionKind::LoadRegisterPairImmediate {
        target: Register16::DE,
    });
    let mut immediate_bytes = [0x34, 0x12].into_iter();
    cpu.complete_execute_machine_cycle(0x11, 0, &mut |_| immediate_bytes.next());
    cpu.complete_execute_machine_cycle(0x11, 1, &mut |_| immediate_bytes.next());
    assert_eq!(cpu.de(), 0x1234);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC123);
    cpu.instruction_kind = Some(CpuInstructionKind::LoadRegisterFromHl {
        target: Register8::A,
    });
    cpu.complete_execute_machine_cycle(0x7E, 0, &mut |operation| {
        assert_eq!(operation, CpuBusOperation::Read { address: 0xC123 });
        Some(0x77)
    });
    assert_eq!(cpu.registers.a, 0x77);

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC234);
    cpu.registers.c = 0x66;
    cpu.instruction_kind = Some(CpuInstructionKind::StoreRegisterToHl {
        source: Register8::C,
    });
    cpu.complete_execute_machine_cycle(0x71, 0, &mut |operation| {
        assert_eq!(
            operation,
            CpuBusOperation::Write {
                address: 0xC234,
                value: 0x66,
            }
        );
        None
    });
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC345);
    cpu.instruction_kind = Some(CpuInstructionKind::StoreImmediateToHl);
    cpu.complete_execute_machine_cycle(0x36, 0, &mut |_| Some(0x99));
    cpu.complete_execute_machine_cycle(0x36, 1, &mut |operation| {
        assert_eq!(
            operation,
            CpuBusOperation::Write {
                address: 0xC345,
                value: 0x99,
            }
        );
        None
    });
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC456);
    cpu.instruction_kind = Some(CpuInstructionKind::LoadAFromHlWithUpdate {
        direction: CpuAddressUpdateDirection::Increment,
    });
    cpu.complete_execute_machine_cycle(0x2A, 0, &mut |operation| {
        assert_eq!(operation, CpuBusOperation::Read { address: 0xC456 });
        Some(0xAB)
    });
    assert_eq!(cpu.registers.a, 0xAB);
    assert_eq!(cpu.hl(), 0xC457);

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC567);
    cpu.registers.a = 0xBC;
    cpu.instruction_kind = Some(CpuInstructionKind::StoreAToHlWithUpdate {
        direction: CpuAddressUpdateDirection::Decrement,
    });
    cpu.complete_execute_machine_cycle(0x32, 0, &mut |operation| {
        assert_eq!(
            operation,
            CpuBusOperation::Write {
                address: 0xC567,
                value: 0xBC,
            }
        );
        None
    });
    assert_eq!(cpu.hl(), 0xC566);

    let mut cpu = power_on_cpu();
    cpu.instruction_kind = Some(CpuInstructionKind::LoadAFromAddress {
        source: MemoryAddressSource::HighImmediate8,
    });
    cpu.complete_execute_machine_cycle(0xF0, 0, &mut |_| Some(0x80));
    cpu.complete_execute_machine_cycle(0xF0, 1, &mut |operation| {
        assert_eq!(operation, CpuBusOperation::Read { address: 0xFF80 });
        Some(0x42)
    });
    assert_eq!(cpu.registers.a, 0x42);

    let mut cpu = power_on_cpu();
    cpu.instruction_kind = Some(CpuInstructionKind::LoadAFromAddress {
        source: MemoryAddressSource::Immediate16,
    });
    let mut address_and_value = [0x34, 0x12, 0xCD].into_iter();
    cpu.complete_execute_machine_cycle(0xFA, 0, &mut |_| address_and_value.next());
    cpu.complete_execute_machine_cycle(0xFA, 1, &mut |_| address_and_value.next());
    cpu.complete_execute_machine_cycle(0xFA, 2, &mut |operation| {
        assert_eq!(operation, CpuBusOperation::Read { address: 0x1234 });
        address_and_value.next()
    });
    assert_eq!(cpu.registers.a, 0xCD);

    let mut cpu = power_on_cpu();
    cpu.registers.a = 0x91;
    cpu.registers.c = 0x44;
    cpu.instruction_kind = Some(CpuInstructionKind::StoreAToAddress {
        destination: MemoryAddressSource::HighC,
    });
    cpu.complete_execute_machine_cycle(0xE2, 0, &mut |operation| {
        assert_eq!(
            operation,
            CpuBusOperation::Write {
                address: 0xFF44,
                value: 0x91,
            }
        );
        None
    });
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.a = 0x37;
    cpu.instruction_kind = Some(CpuInstructionKind::StoreAToAddress {
        destination: MemoryAddressSource::Immediate16,
    });
    let mut destination_bytes = [0x78, 0x56].into_iter();
    cpu.complete_execute_machine_cycle(0xEA, 0, &mut |_| destination_bytes.next());
    cpu.complete_execute_machine_cycle(0xEA, 1, &mut |_| destination_bytes.next());
    cpu.complete_execute_machine_cycle(0xEA, 2, &mut |operation| {
        assert_eq!(
            operation,
            CpuBusOperation::Write {
                address: 0x5678,
                value: 0x37,
            }
        );
        None
    });
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xBEEF);
    cpu.instruction_kind = Some(CpuInstructionKind::LoadSpFromHl);
    cpu.complete_execute_machine_cycle(0xF9, 0, &mut |_| None);
    assert_eq!(cpu.registers.sp, 0xBEEF);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());
}

#[test]
fn execute_arithmetic_and_control_flow_variants_cover_remaining_private_paths() {
    let mut cpu = power_on_cpu();
    cpu.registers.sp = 0xFFF8;
    cpu.instruction_kind = Some(CpuInstructionKind::LoadHlFromSpPlusImmediate);
    cpu.complete_execute_machine_cycle(0xF8, 0, &mut |_| Some(0x08));
    cpu.complete_execute_machine_cycle(0xF8, 1, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0000);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.sp = 0x0008;
    cpu.instruction_kind = Some(CpuInstructionKind::AddSpImmediate);
    cpu.complete_execute_machine_cycle(0xE8, 0, &mut |_| Some(0xF8));
    cpu.complete_execute_machine_cycle(0xE8, 1, &mut |_| None);
    cpu.complete_execute_machine_cycle(0xE8, 2, &mut |_| None);
    assert_eq!(cpu.registers.sp, 0x0000);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::BC, 0x1234);
    cpu.instruction_kind = Some(CpuInstructionKind::IncrementRegisterPair {
        target: Register16::BC,
    });
    cpu.complete_execute_machine_cycle(0x03, 0, &mut |_| None);
    assert_eq!(cpu.bc(), 0x1235);

    let mut cpu = power_on_cpu();
    cpu.registers.sp = 0x1234;
    cpu.instruction_kind = Some(CpuInstructionKind::DecrementRegisterPair {
        target: Register16::SP,
    });
    cpu.complete_execute_machine_cycle(0x3B, 0, &mut |_| None);
    assert_eq!(cpu.registers.sp, 0x1233);

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0x0001);
    cpu.instruction_kind = Some(CpuInstructionKind::AddHl {
        source: Register16::HL,
    });
    cpu.complete_execute_machine_cycle(0x29, 0, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0002);

    let mut cpu = power_on_cpu();
    cpu.registers.f = FLAG_Z;
    cpu.instruction_kind = Some(CpuInstructionKind::AbsoluteJump {
        condition: Some(ConditionCode::Z),
    });
    cpu.complete_execute_machine_cycle(0xCA, 0, &mut |_| Some(0x34));
    cpu.complete_execute_machine_cycle(0xCA, 1, &mut |_| Some(0x12));
    cpu.complete_execute_machine_cycle(0xCA, 2, &mut |_| None);
    assert_eq!(cpu.registers.pc, 0x1234);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.f = 0;
    cpu.instruction_kind = Some(CpuInstructionKind::AbsoluteJump {
        condition: Some(ConditionCode::C),
    });
    cpu.complete_execute_machine_cycle(0xDA, 0, &mut |_| Some(0x78));
    cpu.complete_execute_machine_cycle(0xDA, 1, &mut |_| Some(0x56));
    assert_eq!(cpu.registers.pc, 0x0002);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.sp = 0xC000;
    cpu.ime = false;
    cpu.delayed_ime_enable = true;
    cpu.delayed_ime_enable_steps = 1;
    cpu.instruction_kind = Some(CpuInstructionKind::ReturnFromInterrupt);
    cpu.complete_execute_machine_cycle(0xD9, 0, &mut |_| Some(0x78));
    cpu.complete_execute_machine_cycle(0xD9, 1, &mut |_| Some(0x56));
    cpu.complete_execute_machine_cycle(0xD9, 2, &mut |_| None);
    assert_eq!(cpu.registers.pc, 0x5678);
    assert_eq!(cpu.registers.sp, 0xC002);
    assert!(cpu.ime);
    assert!(!cpu.delayed_ime_enable);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x0101;
    cpu.instruction_kind = Some(CpuInstructionKind::Stop);
    let mut operations = Vec::new();
    cpu.complete_execute_machine_cycle(0x10, 0, &mut |operation| {
        operations.push(operation);
        match operation {
            CpuBusOperation::StopWakeLineAsserted => Some(0x00),
            CpuBusOperation::PendingInterruptMask => Some(0x00),
            CpuBusOperation::Read { address } => {
                assert_eq!(address, 0x0101);
                Some(0x00)
            }
            other => panic!("unexpected STOP bus operation: {other:?}"),
        }
    });
    assert_eq!(
        operations,
        vec![
            CpuBusOperation::StopWakeLineAsserted,
            CpuBusOperation::PendingInterruptMask,
            CpuBusOperation::Read { address: 0x0101 },
        ]
    );
    assert_eq!(cpu.registers.pc, 0x0102);
    assert_eq!(cpu.execution_state, CpuExecutionState::Stopped);

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x0101;
    cpu.ime = false;
    cpu.instruction_kind = Some(CpuInstructionKind::Stop);
    let mut operations = Vec::new();
    cpu.complete_execute_machine_cycle(0x10, 0, &mut |operation| {
        operations.push(operation);
        match operation {
            CpuBusOperation::StopWakeLineAsserted => Some(0x00),
            CpuBusOperation::PendingInterruptMask => Some(0x01),
            other => panic!("unexpected STOP zombie bus operation: {other:?}"),
        }
    });
    assert_eq!(
        operations,
        vec![
            CpuBusOperation::StopWakeLineAsserted,
            CpuBusOperation::PendingInterruptMask,
        ]
    );
    assert_eq!(cpu.registers.pc, 0x0101);
    assert_eq!(cpu.execution_state, CpuExecutionState::ZombieStopped);

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x0101;
    cpu.ime = false;
    cpu.instruction_kind = Some(CpuInstructionKind::Stop);
    let mut operations = Vec::new();
    cpu.complete_execute_machine_cycle(0x10, 0, &mut |operation| {
        operations.push(operation);
        match operation {
            CpuBusOperation::StopWakeLineAsserted => Some(0x01),
            CpuBusOperation::PendingInterruptMask => Some(0x00),
            CpuBusOperation::Read { address } => {
                assert_eq!(address, 0x0101);
                Some(0x00)
            }
            other => panic!("unexpected STOP halt-like bus operation: {other:?}"),
        }
    });
    assert_eq!(
        operations,
        vec![
            CpuBusOperation::StopWakeLineAsserted,
            CpuBusOperation::PendingInterruptMask,
            CpuBusOperation::Read { address: 0x0101 },
        ]
    );
    assert_eq!(cpu.registers.pc, 0x0102);
    assert!(cpu.halt_request_pending);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x0101;
    cpu.ime = false;
    cpu.instruction_kind = Some(CpuInstructionKind::Stop);
    let mut operations = Vec::new();
    cpu.complete_execute_machine_cycle(0x10, 0, &mut |operation| {
        operations.push(operation);
        match operation {
            CpuBusOperation::StopWakeLineAsserted => Some(0x01),
            CpuBusOperation::PendingInterruptMask => Some(0x01),
            other => panic!("unexpected STOP nop-like bus operation: {other:?}"),
        }
    });
    assert_eq!(
        operations,
        vec![
            CpuBusOperation::StopWakeLineAsserted,
            CpuBusOperation::PendingInterruptMask,
        ]
    );
    assert_eq!(cpu.registers.pc, 0x0101);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x4567;
    cpu.registers.sp = 0xFFFE;
    cpu.instruction_kind = Some(CpuInstructionKind::Restart { vector: 0x18 });
    cpu.complete_execute_machine_cycle(0xDF, 0, &mut |_| None);
    cpu.complete_execute_machine_cycle(0xDF, 1, &mut |operation| {
        assert_eq!(
            operation,
            CpuBusOperation::Write {
                address: 0xFFFD,
                value: 0x45,
            }
        );
        None
    });
    cpu.complete_execute_machine_cycle(0xDF, 2, &mut |operation| {
        assert_eq!(
            operation,
            CpuBusOperation::Write {
                address: 0xFFFC,
                value: 0x67,
            }
        );
        None
    });
    assert_eq!(cpu.registers.pc, 0x0018);
    assert_eq!(cpu.registers.sp, 0xFFFC);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());
}

#[test]
fn conditional_call_and_return_paths_cover_remaining_control_flow_steps() {
    let mut cpu = power_on_cpu();
    cpu.registers.f = 0;
    cpu.operand16_latch = 0x0034;
    cpu.instruction_kind = Some(CpuInstructionKind::Call {
        condition: Some(ConditionCode::Z),
    });
    cpu.complete_execute_machine_cycle(0xCC, 1, &mut |_| Some(0x12));
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.f = FLAG_Z;
    cpu.registers.sp = 0xC000;
    cpu.instruction_kind = Some(CpuInstructionKind::Return {
        condition: Some(ConditionCode::Z),
    });
    cpu.complete_execute_machine_cycle(0xC8, 0, &mut |_| None);
    cpu.complete_execute_machine_cycle(0xC8, 1, &mut |_| Some(0x78));
    cpu.complete_execute_machine_cycle(0xC8, 2, &mut |_| Some(0x56));
    cpu.complete_execute_machine_cycle(0xC8, 3, &mut |_| None);
    assert_eq!(cpu.registers.pc, 0x5678);
    assert_eq!(cpu.registers.sp, 0xC002);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.f = 0;
    cpu.instruction_kind = Some(CpuInstructionKind::Return {
        condition: Some(ConditionCode::Z),
    });
    cpu.complete_execute_machine_cycle(0xC8, 0, &mut |_| None);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());
}

#[test]
fn remaining_private_decode_and_address_helpers_stay_explicit() {
    let mut cpu = power_on_cpu();

    cpu.registers.a = 0x81;
    assert!(matches!(
        cpu.decode_fetched_opcode(0x07),
        DecodedOpcode::Complete
    ));
    assert_eq!(cpu.registers.a, 0x03);
    assert_eq!(cpu.registers.f, FLAG_C);

    cpu.registers.a = 0x80;
    cpu.registers.f = FLAG_C;
    assert!(matches!(
        cpu.decode_fetched_opcode(0x17),
        DecodedOpcode::Complete
    ));
    assert_eq!(cpu.registers.a, 0x01);
    assert_eq!(cpu.registers.f, FLAG_C);

    cpu.registers.f = 0;
    cpu.registers.d = 0x01;
    assert!(matches!(
        cpu.decode_fetched_opcode(0x15),
        DecodedOpcode::Complete
    ));
    assert_eq!(cpu.registers.d, 0x00);
    assert_eq!(cpu.registers.f, FLAG_Z | FLAG_N);
    assert!(matches!(
        cpu.decode_fetched_opcode(0x35),
        DecodedOpcode::Execute(CpuInstructionKind::DecrementHlMemory)
    ));

    cpu.operand16_latch = 0xBEEF;
    assert_eq!(
        cpu.resolve_memory_address(MemoryAddressSource::Immediate16),
        0xBEEF,
    );
}
