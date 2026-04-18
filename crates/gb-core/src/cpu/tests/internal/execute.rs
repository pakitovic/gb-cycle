use super::*;
use crate::cpu::decode::FetchCompletionKind;

#[test]
fn execute_without_instruction_kind_restalls_last_machine_cycle() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);

    cpu.complete_execute_machine_cycle(2, &mut |_| None);

    assert_eq!(
        cpu.execution_state,
        CpuExecutionState::Execute {
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
    cpu.in_flight.kind = Some(CpuInstructionKind::AddHl {
        source: Register16::SP,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0002);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::BC, 0x0001);
    cpu.write_register16(Register16::HL, 0x0001);
    cpu.in_flight.kind = Some(CpuInstructionKind::AddHl {
        source: Register16::BC,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0002);

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::DE, 0x0002);
    cpu.write_register16(Register16::HL, 0x0001);
    cpu.in_flight.kind = Some(CpuInstructionKind::AddHl {
        source: Register16::DE,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0003);
}

#[test]
fn execute_memory_and_alu_variants_cover_remaining_private_paths() {
    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC000);
    cpu.in_flight.kind = Some(CpuInstructionKind::DecrementHlMemory);
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0x10));
    cpu.complete_execute_machine_cycle(1, &mut |_| None);
    assert_eq!(cpu.registers.f, FLAG_N | FLAG_H);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.a = 0x0F;
    cpu.in_flight.kind = Some(CpuInstructionKind::AluImmediate {
        operation: AluOperation::Add,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0x01));
    assert_eq!(cpu.registers.a, 0x10);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.a = 0xF0;
    cpu.write_register16(Register16::HL, 0xC100);
    cpu.in_flight.kind = Some(CpuInstructionKind::AluFromHl {
        operation: AluOperation::And,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0x0F));
    assert_eq!(cpu.registers.a, 0x00);
    assert_eq!(cpu.registers.f, FLAG_Z | FLAG_H);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());
}

#[test]
fn execute_load_variants_cover_remaining_private_paths() {
    let mut cpu = power_on_cpu();
    cpu.in_flight.kind = Some(CpuInstructionKind::LoadRegisterImmediate {
        target: Register8::B,
    });
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0000 })
        );
        Some(0x5A)
    });
    assert_eq!(cpu.registers.b, 0x5A);
    assert_eq!(cpu.registers.pc, 0x0001);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.in_flight.kind = Some(CpuInstructionKind::LoadRegisterPairImmediate {
        target: Register16::DE,
    });
    let mut immediate_bytes = [0x34, 0x12].into_iter();
    cpu.complete_execute_machine_cycle(0, &mut |_| immediate_bytes.next());
    cpu.complete_execute_machine_cycle(1, &mut |_| immediate_bytes.next());
    assert_eq!(cpu.de(), 0x1234);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC123);
    cpu.in_flight.kind = Some(CpuInstructionKind::LoadRegisterFromHl {
        target: Register8::A,
    });
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0xC123 })
        );
        Some(0x77)
    });
    assert_eq!(cpu.registers.a, 0x77);

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC234);
    cpu.registers.c = 0x66;
    cpu.in_flight.kind = Some(CpuInstructionKind::StoreRegisterToHl {
        source: Register8::C,
    });
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Write {
                address: 0xC234,
                value: 0x66,
            })
        );
        None
    });
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC345);
    cpu.in_flight.kind = Some(CpuInstructionKind::StoreImmediateToHl);
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0x99));
    cpu.complete_execute_machine_cycle(1, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Write {
                address: 0xC345,
                value: 0x99,
            })
        );
        None
    });
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC456);
    cpu.in_flight.kind = Some(CpuInstructionKind::LoadAFromHlWithUpdate {
        direction: CpuAddressUpdateDirection::Increment,
    });
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0xC456 })
        );
        Some(0xAB)
    });
    assert_eq!(cpu.registers.a, 0xAB);
    assert_eq!(cpu.hl(), 0xC457);

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC567);
    cpu.registers.a = 0xBC;
    cpu.in_flight.kind = Some(CpuInstructionKind::StoreAToHlWithUpdate {
        direction: CpuAddressUpdateDirection::Decrement,
    });
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Write {
                address: 0xC567,
                value: 0xBC,
            })
        );
        None
    });
    assert_eq!(cpu.hl(), 0xC566);

    let mut cpu = power_on_cpu();
    cpu.in_flight.kind = Some(CpuInstructionKind::LoadAFromHighImmediateAddress);
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0x80));
    cpu.complete_execute_machine_cycle(1, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0xFF80 })
        );
        Some(0x42)
    });
    assert_eq!(cpu.registers.a, 0x42);

    let mut cpu = power_on_cpu();
    cpu.in_flight.kind = Some(CpuInstructionKind::LoadAFromImmediate16Address);
    let mut address_and_value = [0x34, 0x12, 0xCD].into_iter();
    cpu.complete_execute_machine_cycle(0, &mut |_| address_and_value.next());
    cpu.complete_execute_machine_cycle(1, &mut |_| address_and_value.next());
    cpu.complete_execute_machine_cycle(2, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x1234 })
        );
        address_and_value.next()
    });
    assert_eq!(cpu.registers.a, 0xCD);

    let mut cpu = power_on_cpu();
    cpu.registers.a = 0x91;
    cpu.registers.c = 0x44;
    cpu.in_flight.kind = Some(CpuInstructionKind::StoreAToDirectAddress {
        destination: DirectAddressSource::HighC,
    });
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Write {
                address: 0xFF44,
                value: 0x91,
            })
        );
        None
    });
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.a = 0x37;
    cpu.in_flight.kind = Some(CpuInstructionKind::StoreAToImmediate16Address);
    let mut destination_bytes = [0x78, 0x56].into_iter();
    cpu.complete_execute_machine_cycle(0, &mut |_| destination_bytes.next());
    cpu.complete_execute_machine_cycle(1, &mut |_| destination_bytes.next());
    cpu.complete_execute_machine_cycle(2, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Write {
                address: 0x5678,
                value: 0x37,
            })
        );
        None
    });
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xBEEF);
    cpu.in_flight.kind = Some(CpuInstructionKind::LoadSpFromHl);
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    assert_eq!(cpu.registers.sp, 0xBEEF);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());
}

#[test]
fn execute_arithmetic_and_control_flow_variants_cover_remaining_private_paths() {
    let mut cpu = power_on_cpu();
    cpu.registers.sp = 0xFFF8;
    cpu.in_flight.kind = Some(CpuInstructionKind::LoadHlFromSpPlusImmediate);
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0x08));
    cpu.complete_execute_machine_cycle(1, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0000);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.sp = 0x0008;
    cpu.in_flight.kind = Some(CpuInstructionKind::AddSpImmediate);
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0xF8));
    cpu.complete_execute_machine_cycle(1, &mut |_| None);
    cpu.complete_execute_machine_cycle(2, &mut |_| None);
    assert_eq!(cpu.registers.sp, 0x0000);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::BC, 0x1234);
    cpu.in_flight.kind = Some(CpuInstructionKind::IncrementRegisterPair {
        target: Register16::BC,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    assert_eq!(cpu.bc(), 0x1235);

    let mut cpu = power_on_cpu();
    cpu.registers.sp = 0x1234;
    cpu.in_flight.kind = Some(CpuInstructionKind::DecrementRegisterPair {
        target: Register16::SP,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    assert_eq!(cpu.registers.sp, 0x1233);

    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0x0001);
    cpu.in_flight.kind = Some(CpuInstructionKind::AddHl {
        source: Register16::HL,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    assert_eq!(cpu.hl(), 0x0002);

    let mut cpu = power_on_cpu();
    cpu.registers.f = FLAG_Z;
    cpu.in_flight.kind = Some(CpuInstructionKind::ConditionalAbsoluteJump {
        condition: ConditionCode::Z,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0x34));
    cpu.complete_execute_machine_cycle(1, &mut |_| Some(0x12));
    cpu.complete_execute_machine_cycle(2, &mut |_| None);
    assert_eq!(cpu.registers.pc, 0x1234);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.f = 0;
    cpu.in_flight.kind = Some(CpuInstructionKind::ConditionalAbsoluteJump {
        condition: ConditionCode::C,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0x78));
    cpu.complete_execute_machine_cycle(1, &mut |_| Some(0x56));
    assert_eq!(cpu.registers.pc, 0x0002);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.sp = 0xC000;
    cpu.set_ime_disabled();
    cpu.force_delayed_ime_enable_for_test(1);
    cpu.in_flight.kind = Some(CpuInstructionKind::ReturnFromInterrupt);
    cpu.complete_execute_machine_cycle(0, &mut |_| Some(0x78));
    cpu.complete_execute_machine_cycle(1, &mut |_| Some(0x56));
    cpu.complete_execute_machine_cycle(2, &mut |_| None);
    assert_eq!(cpu.registers.pc, 0x5678);
    assert_eq!(cpu.registers.sp, 0xC002);
    assert!(cpu.ime());
    assert!(!cpu.delayed_ime_enable());
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x0101;
    cpu.in_flight.kind = Some(CpuInstructionKind::Stop);
    let mut operations = Vec::new();
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        operations.push(operation);
        match operation {
            CpuExternalOperation::StopWakeLineAsserted => Some(0x00),
            CpuExternalOperation::PendingInterruptMask => Some(0x00),
            CpuExternalOperation::Bus(CpuBusOperation::Read { address }) => {
                assert_eq!(address, 0x0101);
                Some(0x00)
            }
            other => panic!("unexpected STOP bus operation: {other:?}"),
        }
    });
    assert_eq!(
        operations,
        vec![
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::PendingInterruptMask,
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0101 }),
        ]
    );
    assert_eq!(cpu.registers.pc, 0x0102);
    assert_eq!(cpu.execution_state, CpuExecutionState::Stopped);

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x0101;
    cpu.set_ime_disabled();
    cpu.in_flight.kind = Some(CpuInstructionKind::Stop);
    let mut operations = Vec::new();
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        operations.push(operation);
        match operation {
            CpuExternalOperation::StopWakeLineAsserted => Some(0x00),
            CpuExternalOperation::PendingInterruptMask => Some(0x01),
            other => panic!("unexpected STOP zombie bus operation: {other:?}"),
        }
    });
    assert_eq!(
        operations,
        vec![
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::PendingInterruptMask,
        ]
    );
    assert_eq!(cpu.registers.pc, 0x0101);
    assert_eq!(cpu.execution_state, CpuExecutionState::ZombieStopped);

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x0101;
    cpu.set_ime_disabled();
    cpu.in_flight.kind = Some(CpuInstructionKind::Stop);
    let mut operations = Vec::new();
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        operations.push(operation);
        match operation {
            CpuExternalOperation::StopWakeLineAsserted => Some(0x01),
            CpuExternalOperation::PendingInterruptMask => Some(0x00),
            CpuExternalOperation::Bus(CpuBusOperation::Read { address }) => {
                assert_eq!(address, 0x0101);
                Some(0x00)
            }
            other => panic!("unexpected STOP halt-like bus operation: {other:?}"),
        }
    });
    assert_eq!(
        operations,
        vec![
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::PendingInterruptMask,
            CpuExternalOperation::Bus(CpuBusOperation::Read { address: 0x0101 }),
        ]
    );
    assert_eq!(cpu.registers.pc, 0x0102);
    assert!(cpu.halt_request_pending_for_test());
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x0101;
    cpu.set_ime_disabled();
    cpu.in_flight.kind = Some(CpuInstructionKind::Stop);
    let mut operations = Vec::new();
    cpu.complete_execute_machine_cycle(0, &mut |operation| {
        operations.push(operation);
        match operation {
            CpuExternalOperation::StopWakeLineAsserted => Some(0x01),
            CpuExternalOperation::PendingInterruptMask => Some(0x01),
            other => panic!("unexpected STOP nop-like bus operation: {other:?}"),
        }
    });
    assert_eq!(
        operations,
        vec![
            CpuExternalOperation::StopWakeLineAsserted,
            CpuExternalOperation::PendingInterruptMask,
        ]
    );
    assert_eq!(cpu.registers.pc, 0x0101);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.pc = 0x4567;
    cpu.registers.sp = 0xFFFE;
    cpu.in_flight.kind = Some(CpuInstructionKind::Restart { vector: 0x18 });
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    cpu.complete_execute_machine_cycle(1, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Write {
                address: 0xFFFD,
                value: 0x45,
            })
        );
        None
    });
    cpu.complete_execute_machine_cycle(2, &mut |operation| {
        assert_eq!(
            operation,
            CpuExternalOperation::Bus(CpuBusOperation::Write {
                address: 0xFFFC,
                value: 0x67,
            })
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
    cpu.in_flight.operand16_latch = 0x0034;
    cpu.in_flight.kind = Some(CpuInstructionKind::ConditionalCall {
        condition: ConditionCode::Z,
    });
    cpu.complete_execute_machine_cycle(1, &mut |_| Some(0x12));
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.f = FLAG_Z;
    cpu.registers.sp = 0xC000;
    cpu.in_flight.kind = Some(CpuInstructionKind::ConditionalReturn {
        condition: ConditionCode::Z,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    cpu.complete_execute_machine_cycle(1, &mut |_| Some(0x78));
    cpu.complete_execute_machine_cycle(2, &mut |_| Some(0x56));
    cpu.complete_execute_machine_cycle(3, &mut |_| None);
    assert_eq!(cpu.registers.pc, 0x5678);
    assert_eq!(cpu.registers.sp, 0xC002);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

    let mut cpu = power_on_cpu();
    cpu.registers.f = 0;
    cpu.in_flight.kind = Some(CpuInstructionKind::ConditionalReturn {
        condition: ConditionCode::Z,
    });
    cpu.complete_execute_machine_cycle(0, &mut |_| None);
    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());
}

#[test]
fn remaining_private_decode_and_address_helpers_stay_explicit() {
    let mut cpu = power_on_cpu();

    cpu.registers.a = 0x81;
    cpu.complete_fetch_completion(FetchCompletionKind::RotateLeftAccumulatorCarry);
    assert_eq!(cpu.registers.a, 0x03);
    assert_eq!(cpu.registers.f, FLAG_C);

    let mut cpu = power_on_cpu();
    cpu.registers.a = 0x80;
    cpu.registers.f = FLAG_C;
    cpu.complete_fetch_completion(FetchCompletionKind::RotateLeftAccumulatorThroughCarry);
    assert_eq!(cpu.registers.a, 0x01);
    assert_eq!(cpu.registers.f, FLAG_C);

    let mut cpu = power_on_cpu();
    cpu.registers.f = 0;
    cpu.registers.d = 0x01;
    cpu.complete_fetch_completion(FetchCompletionKind::DecrementRegister {
        target: Register8::D,
    });
    assert_eq!(cpu.registers.d, 0x00);
    assert_eq!(cpu.registers.f, FLAG_Z | FLAG_N);
    assert!(matches!(
        cpu.decode_fetched_opcode(0x35),
        DecodedOpcode::Execute(CpuInstructionKind::DecrementHlMemory)
    ));

    cpu.in_flight.operand16_latch = 0xBEEF;
    assert_eq!(cpu.resolve_immediate16_address(), 0xBEEF);
}
