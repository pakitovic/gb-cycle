use super::super::CpuCore;
use super::{CbInstructionKind, decode_register8_operand};

impl CpuCore {
    pub(in crate::cpu) fn decode_cb_opcode(&self, opcode: u8) -> CbInstructionKind {
        if opcode & 0xF8 == 0x00 {
            return CbInstructionKind::RotateLeftCarry {
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        if opcode & 0xF8 == 0x08 {
            return CbInstructionKind::RotateRightCarry {
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        if opcode & 0xF8 == 0x10 {
            return CbInstructionKind::RotateLeftThroughCarry {
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        if opcode & 0xF8 == 0x18 {
            return CbInstructionKind::RotateRightThroughCarry {
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        if opcode & 0xF8 == 0x20 {
            return CbInstructionKind::ShiftLeftArithmetic {
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        if opcode & 0xF8 == 0x28 {
            return CbInstructionKind::ShiftRightArithmetic {
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        if opcode & 0xF8 == 0x30 {
            return CbInstructionKind::SwapNibbles {
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        if opcode & 0xF8 == 0x38 {
            return CbInstructionKind::ShiftRightLogical {
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        if opcode & 0xC0 == 0x40 {
            return CbInstructionKind::BitTest {
                bit: (opcode >> 3) & 0x07,
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        if opcode & 0xC0 == 0x80 {
            return CbInstructionKind::ResetBit {
                bit: (opcode >> 3) & 0x07,
                target: decode_register8_operand(opcode & 0x07),
            };
        }

        CbInstructionKind::SetBit {
            bit: (opcode >> 3) & 0x07,
            target: decode_register8_operand(opcode & 0x07),
        }
    }
}
