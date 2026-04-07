use super::super::CpuAddressUpdateDirection;
use super::{
    AluOperation, ConditionCode, Register8, Register8Operand, Register16, StackRegister16,
};

pub(in crate::cpu) fn decode_register16(bits: u8) -> Register16 {
    match bits {
        0 => Register16::BC,
        1 => Register16::DE,
        2 => Register16::HL,
        3 => Register16::SP,
        _ => unreachable!("2-bit register pair selector must be in 0..=3"),
    }
}

pub(in crate::cpu) fn decode_register8_operand(bits: u8) -> Register8Operand {
    match bits {
        0 => Register8Operand::Register(Register8::B),
        1 => Register8Operand::Register(Register8::C),
        2 => Register8Operand::Register(Register8::D),
        3 => Register8Operand::Register(Register8::E),
        4 => Register8Operand::Register(Register8::H),
        5 => Register8Operand::Register(Register8::L),
        6 => Register8Operand::IndirectHl,
        7 => Register8Operand::Register(Register8::A),
        _ => unreachable!("3-bit register selector must be in 0..=7"),
    }
}

pub(in crate::cpu) fn decode_stack_register16(bits: u8) -> StackRegister16 {
    match bits {
        0 => StackRegister16::BC,
        1 => StackRegister16::DE,
        2 => StackRegister16::HL,
        3 => StackRegister16::AF,
        _ => unreachable!("2-bit stack register selector must be in 0..=3"),
    }
}

pub(in crate::cpu) fn decode_alu_operation(bits: u8) -> AluOperation {
    match bits {
        0 => AluOperation::Add,
        1 => AluOperation::Adc,
        2 => AluOperation::Sub,
        3 => AluOperation::Sbc,
        4 => AluOperation::And,
        5 => AluOperation::Xor,
        6 => AluOperation::Or,
        7 => AluOperation::Compare,
        _ => unreachable!("3-bit ALU selector must be in 0..=7"),
    }
}

pub(in crate::cpu) fn decode_relative_jump_condition(opcode: u8) -> Option<ConditionCode> {
    match opcode {
        0x18 => None,
        0x20 => Some(ConditionCode::Nz),
        0x28 => Some(ConditionCode::Z),
        0x30 => Some(ConditionCode::Nc),
        0x38 => Some(ConditionCode::C),
        _ => unreachable!("opcode must be a JR form"),
    }
}

pub(in crate::cpu) fn decode_absolute_jump_condition(opcode: u8) -> Option<ConditionCode> {
    match opcode {
        0xC3 => None,
        0xC2 => Some(ConditionCode::Nz),
        0xCA => Some(ConditionCode::Z),
        0xD2 => Some(ConditionCode::Nc),
        0xDA => Some(ConditionCode::C),
        _ => unreachable!("opcode must be a JP form"),
    }
}

pub(in crate::cpu) fn decode_call_condition(opcode: u8) -> Option<ConditionCode> {
    match opcode {
        0xCD => None,
        0xC4 => Some(ConditionCode::Nz),
        0xCC => Some(ConditionCode::Z),
        0xD4 => Some(ConditionCode::Nc),
        0xDC => Some(ConditionCode::C),
        _ => unreachable!("opcode must be a CALL form"),
    }
}

pub(in crate::cpu) fn decode_return_condition(opcode: u8) -> Option<ConditionCode> {
    match opcode {
        0xC9 => None,
        0xC0 => Some(ConditionCode::Nz),
        0xC8 => Some(ConditionCode::Z),
        0xD0 => Some(ConditionCode::Nc),
        0xD8 => Some(ConditionCode::C),
        _ => unreachable!("opcode must be a RET form"),
    }
}

pub(in crate::cpu) fn decode_hl_update_direction(opcode: u8) -> CpuAddressUpdateDirection {
    match opcode {
        0x22 | 0x2A => CpuAddressUpdateDirection::Increment,
        0x32 | 0x3A => CpuAddressUpdateDirection::Decrement,
        _ => unreachable!("opcode must be an [hli]/[hld] transfer form"),
    }
}
