use super::super::CpuCore;
use super::{CbInstructionKind, decode_register8_operand};

impl CpuCore {
    pub(in crate::cpu) fn decode_cb_opcode(&self, opcode: u8) -> Option<CbInstructionKind> {
        if opcode & 0xF8 == 0x00 {
            return Some(CbInstructionKind::RotateLeftCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x08 {
            return Some(CbInstructionKind::RotateRightCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x10 {
            return Some(CbInstructionKind::RotateLeftThroughCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x18 {
            return Some(CbInstructionKind::RotateRightThroughCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x20 {
            return Some(CbInstructionKind::ShiftLeftArithmetic {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x28 {
            return Some(CbInstructionKind::ShiftRightArithmetic {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x30 {
            return Some(CbInstructionKind::SwapNibbles {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x38 {
            return Some(CbInstructionKind::ShiftRightLogical {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xC0 == 0x40 {
            return Some(CbInstructionKind::BitTest {
                bit: (opcode >> 3) & 0x07,
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xC0 == 0x80 {
            return Some(CbInstructionKind::ResetBit {
                bit: (opcode >> 3) & 0x07,
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xC0 == 0xC0 {
            return Some(CbInstructionKind::SetBit {
                bit: (opcode >> 3) & 0x07,
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        None
    }
}
