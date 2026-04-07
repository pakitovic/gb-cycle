use super::super::decode::{CpuInstructionKind, Register16};
use super::super::*;

impl CpuCore {
    pub(super) fn execute_arithmetic_machine_cycle(
        &mut self,
        kind: CpuInstructionKind,
        opcode: u8,
        step: u8,
        bus_operation: &mut CpuBusCallback<'_>,
    ) {
        match kind {
            CpuInstructionKind::LoadHlFromSpPlusImmediate => match step {
                0 => {
                    self.operand8_latch = self.read_pc_u8(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let result = self.sp_plus_signed_immediate(self.operand8_latch);
                    self.write_register16(Register16::HL, result);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AddSpImmediate => match step {
                0 => {
                    self.operand8_latch = self.read_pc_u8(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    let result = self.sp_plus_signed_immediate(self.operand8_latch);
                    self.write_register16(Register16::SP, result);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AddHl { source } => match step {
                0 => {
                    let value = match source {
                        Register16::BC => self.bc(),
                        Register16::DE => self.de(),
                        Register16::HL => self.hl(),
                        Register16::SP => self.registers.sp,
                    };
                    self.add_to_hl(value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::IncrementRegisterPair { target } => match step {
                0 => {
                    self.increment_or_decrement_register_pair(
                        target,
                        CpuAddressUpdateDirection::Increment,
                    );
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::DecrementRegisterPair { target } => match step {
                0 => {
                    self.increment_or_decrement_register_pair(
                        target,
                        CpuAddressUpdateDirection::Decrement,
                    );
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::IncrementHlMemory => match step {
                0 => {
                    self.operand8_latch = self.read_byte(self.hl(), bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let before = self.operand8_latch;
                    let result = before.wrapping_add(1);
                    self.operand8_latch = result;
                    self.write_byte(self.hl(), result, bus_operation);
                    self.update_inc_flags(before, result);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::DecrementHlMemory => match step {
                0 => {
                    self.operand8_latch = self.read_byte(self.hl(), bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let before = self.operand8_latch;
                    let result = before.wrapping_sub(1);
                    self.operand8_latch = result;
                    self.write_byte(self.hl(), result, bus_operation);
                    self.update_dec_flags(before, result);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AluImmediate { operation } => match step {
                0 => {
                    let value = self.read_pc_u8(bus_operation);
                    self.apply_alu_operation(operation, value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AluFromHl { operation } => match step {
                0 => {
                    let value = self.read_byte(self.hl(), bus_operation);
                    self.apply_alu_operation(operation, value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            _ => unreachable!("non-arithmetic instruction dispatched to arithmetic executor"),
        }
    }
}
