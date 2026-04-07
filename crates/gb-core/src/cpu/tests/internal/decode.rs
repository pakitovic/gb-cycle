use super::*;

#[test]
fn interrupt_vectors_and_pending_priority_helpers_are_explicit() {
    assert_eq!(interrupt_vector(InterruptSource::VBlank), 0x0040);
    assert_eq!(interrupt_vector(InterruptSource::LcdStat), 0x0048);
    assert_eq!(interrupt_vector(InterruptSource::Timer), 0x0050);
    assert_eq!(interrupt_vector(InterruptSource::Serial), 0x0058);
    assert_eq!(interrupt_vector(InterruptSource::Joypad), 0x0060);

    assert_eq!(highest_pending_interrupt_from_mask(0x00), None);
    assert_eq!(
        highest_pending_interrupt_from_mask(0x02),
        Some(InterruptSource::LcdStat),
    );
    assert_eq!(
        highest_pending_interrupt_from_mask(0x01),
        Some(InterruptSource::VBlank),
    );
    assert_eq!(
        highest_pending_interrupt_from_mask(0x04),
        Some(InterruptSource::Timer),
    );
    assert_eq!(
        highest_pending_interrupt_from_mask(0x08),
        Some(InterruptSource::Serial),
    );
    assert_eq!(
        highest_pending_interrupt_from_mask(0x10),
        Some(InterruptSource::Joypad),
    );
}

#[test]
fn decoder_table_helpers_cover_remaining_private_paths() {
    assert_eq!(decode_register16(0), Register16::BC);
    assert_eq!(decode_register16(1), Register16::DE);
    assert_eq!(decode_register16(2), Register16::HL);
    assert_eq!(decode_register16(3), Register16::SP);
    assert_eq!(
        decode_register8_operand(2),
        Register8Operand::Register(Register8::D),
    );
    assert_eq!(
        decode_register8_operand(3),
        Register8Operand::Register(Register8::E),
    );
    assert_eq!(
        decode_register8_operand(4),
        Register8Operand::Register(Register8::H),
    );
    assert_eq!(
        decode_register8_operand(5),
        Register8Operand::Register(Register8::L),
    );
    assert_eq!(decode_register8_operand(6), Register8Operand::IndirectHl);
    assert_eq!(
        decode_register8_operand(7),
        Register8Operand::Register(Register8::A),
    );
    assert_eq!(decode_stack_register16(0), StackRegister16::BC);
    assert_eq!(decode_stack_register16(1), StackRegister16::DE);
    assert_eq!(decode_stack_register16(2), StackRegister16::HL);
    assert_eq!(decode_stack_register16(3), StackRegister16::AF);
    assert_eq!(decode_alu_operation(0), AluOperation::Add);
    assert_eq!(decode_alu_operation(1), AluOperation::Adc);
    assert_eq!(decode_alu_operation(2), AluOperation::Sub);
    assert_eq!(decode_alu_operation(3), AluOperation::Sbc);
    assert_eq!(decode_alu_operation(4), AluOperation::And);
    assert_eq!(decode_alu_operation(5), AluOperation::Xor);
    assert_eq!(decode_alu_operation(6), AluOperation::Or);
    assert_eq!(decode_alu_operation(7), AluOperation::Compare);
    assert_eq!(decode_relative_jump_condition(0x18), None);
    assert_eq!(
        decode_relative_jump_condition(0x20),
        Some(ConditionCode::Nz),
    );
    assert_eq!(decode_relative_jump_condition(0x28), Some(ConditionCode::Z));
    assert_eq!(
        decode_relative_jump_condition(0x30),
        Some(ConditionCode::Nc),
    );
    assert_eq!(decode_relative_jump_condition(0x38), Some(ConditionCode::C));
    assert_eq!(decode_absolute_jump_condition(0xC3), None);
    assert_eq!(
        decode_absolute_jump_condition(0xC2),
        Some(ConditionCode::Nz),
    );
    assert_eq!(decode_absolute_jump_condition(0xCA), Some(ConditionCode::Z));
    assert_eq!(
        decode_absolute_jump_condition(0xD2),
        Some(ConditionCode::Nc),
    );
    assert_eq!(decode_absolute_jump_condition(0xDA), Some(ConditionCode::C));
    assert_eq!(decode_call_condition(0xCD), None);
    assert_eq!(decode_call_condition(0xC4), Some(ConditionCode::Nz));
    assert_eq!(decode_call_condition(0xCC), Some(ConditionCode::Z));
    assert_eq!(decode_call_condition(0xD4), Some(ConditionCode::Nc));
    assert_eq!(decode_call_condition(0xDC), Some(ConditionCode::C));
    assert_eq!(decode_return_condition(0xC9), None);
    assert_eq!(decode_return_condition(0xC0), Some(ConditionCode::Nz));
    assert_eq!(decode_return_condition(0xC8), Some(ConditionCode::Z));
    assert_eq!(decode_return_condition(0xD0), Some(ConditionCode::Nc));
    assert_eq!(decode_return_condition(0xD8), Some(ConditionCode::C));
    assert_eq!(
        decode_hl_update_direction(0x22),
        CpuAddressUpdateDirection::Increment,
    );
    assert_eq!(
        decode_hl_update_direction(0x2A),
        CpuAddressUpdateDirection::Increment,
    );
    assert_eq!(
        decode_hl_update_direction(0x32),
        CpuAddressUpdateDirection::Decrement,
    );
    assert_eq!(
        decode_hl_update_direction(0x3A),
        CpuAddressUpdateDirection::Decrement,
    );
}

#[test]
fn decode_fetched_opcode_covers_private_complete_and_execute_variants() {
    let mut cpu = power_on_cpu();

    cpu.registers.a = 0x01;
    assert!(matches!(
        cpu.decode_fetched_opcode(0x0F),
        DecodedOpcode::Complete
    ));
    assert_eq!(cpu.registers.a, 0x80);
    assert_eq!(cpu.registers.f, FLAG_C);

    cpu.registers.a = 0x80;
    cpu.registers.f = FLAG_C;
    assert!(matches!(
        cpu.decode_fetched_opcode(0x1F),
        DecodedOpcode::Complete
    ));
    assert_eq!(cpu.registers.a, 0xC0);
    assert_eq!(cpu.registers.f, 0);

    assert!(matches!(
        cpu.decode_fetched_opcode(0x10),
        DecodedOpcode::Execute(CpuInstructionKind::Stop)
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x02),
        DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
            destination: MemoryAddressSource::BC,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x12),
        DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
            destination: MemoryAddressSource::DE,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xEA),
        DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
            destination: MemoryAddressSource::Immediate16,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xE0),
        DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
            destination: MemoryAddressSource::HighImmediate8,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xE2),
        DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
            destination: MemoryAddressSource::HighC,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x0A),
        DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
            source: MemoryAddressSource::BC,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x1A),
        DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
            source: MemoryAddressSource::DE,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xFA),
        DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
            source: MemoryAddressSource::Immediate16,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xF0),
        DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
            source: MemoryAddressSource::HighImmediate8,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xF2),
        DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
            source: MemoryAddressSource::HighC,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x08),
        DecodedOpcode::Execute(CpuInstructionKind::StoreSpToImmediate16)
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xF8),
        DecodedOpcode::Execute(CpuInstructionKind::LoadHlFromSpPlusImmediate)
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xE8),
        DecodedOpcode::Execute(CpuInstructionKind::AddSpImmediate)
    ));
}

#[test]
fn decode_fetched_opcode_covers_remaining_split_execute_variants() {
    let mut cpu = power_on_cpu();

    assert!(matches!(
        cpu.decode_fetched_opcode(0x06),
        DecodedOpcode::Execute(CpuInstructionKind::LoadRegisterImmediate {
            target: Register8::B,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x31),
        DecodedOpcode::Execute(CpuInstructionKind::LoadRegisterPairImmediate {
            target: Register16::SP,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x7E),
        DecodedOpcode::Execute(CpuInstructionKind::LoadRegisterFromHl {
            target: Register8::A,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x70),
        DecodedOpcode::Execute(CpuInstructionKind::StoreRegisterToHl {
            source: Register8::B,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x36),
        DecodedOpcode::Execute(CpuInstructionKind::StoreImmediateToHl)
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x22),
        DecodedOpcode::Execute(CpuInstructionKind::StoreAToHlWithUpdate {
            direction: CpuAddressUpdateDirection::Increment,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x32),
        DecodedOpcode::Execute(CpuInstructionKind::StoreAToHlWithUpdate {
            direction: CpuAddressUpdateDirection::Decrement,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x2A),
        DecodedOpcode::Execute(CpuInstructionKind::LoadAFromHlWithUpdate {
            direction: CpuAddressUpdateDirection::Increment,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x3A),
        DecodedOpcode::Execute(CpuInstructionKind::LoadAFromHlWithUpdate {
            direction: CpuAddressUpdateDirection::Decrement,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x03),
        DecodedOpcode::Execute(CpuInstructionKind::IncrementRegisterPair {
            target: Register16::BC,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x2B),
        DecodedOpcode::Execute(CpuInstructionKind::DecrementRegisterPair {
            target: Register16::HL,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x29),
        DecodedOpcode::Execute(CpuInstructionKind::AddHl {
            source: Register16::HL,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0x86),
        DecodedOpcode::Execute(CpuInstructionKind::AluFromHl {
            operation: AluOperation::Add,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xFE),
        DecodedOpcode::Execute(CpuInstructionKind::AluImmediate {
            operation: AluOperation::Compare,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xC3),
        DecodedOpcode::Execute(CpuInstructionKind::AbsoluteJump { condition: None })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xD9),
        DecodedOpcode::Execute(CpuInstructionKind::ReturnFromInterrupt)
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xDF),
        DecodedOpcode::Execute(CpuInstructionKind::Restart { vector: 0x18 })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xF5),
        DecodedOpcode::Execute(CpuInstructionKind::PushRegisterPair {
            source: StackRegister16::AF,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xE1),
        DecodedOpcode::Execute(CpuInstructionKind::PopRegisterPair {
            target: StackRegister16::HL,
        })
    ));
    assert!(matches!(
        cpu.decode_fetched_opcode(0xCB),
        DecodedOpcode::Execute(CpuInstructionKind::CbPrefixed)
    ));
}

#[test]
fn current_highest_pending_interrupt_queries_the_bus_mask_once() {
    let mut cpu = power_on_cpu();
    let mut pending_mask_queries = 0;

    assert_eq!(
        cpu.current_highest_pending_interrupt(&mut |operation| {
            pending_mask_queries += 1;
            assert_eq!(operation, CpuBusOperation::PendingInterruptMask);
            Some(0x10)
        }),
        Some(InterruptSource::Joypad),
    );
    assert_eq!(pending_mask_queries, 1);
}

#[test]
fn invalid_decoder_inputs_panic_in_debug_tests() {
    assert!(std::panic::catch_unwind(|| decode_register16(4)).is_err());
    assert!(std::panic::catch_unwind(|| decode_register8_operand(8)).is_err());
    assert!(std::panic::catch_unwind(|| decode_stack_register16(4)).is_err());
    assert!(std::panic::catch_unwind(|| decode_alu_operation(8)).is_err());
    assert!(std::panic::catch_unwind(|| decode_relative_jump_condition(0x00)).is_err());
    assert!(std::panic::catch_unwind(|| decode_absolute_jump_condition(0x00)).is_err());
    assert!(std::panic::catch_unwind(|| decode_call_condition(0x00)).is_err());
    assert!(std::panic::catch_unwind(|| decode_return_condition(0x00)).is_err());
    assert!(std::panic::catch_unwind(|| decode_hl_update_direction(0x00)).is_err());
}
