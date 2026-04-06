use super::super::decode::CpuInstructionKind;
use super::super::*;

impl CpuCore {
    pub(super) fn execute_stack_machine_cycle<F>(
        &mut self,
        kind: CpuInstructionKind,
        opcode: u8,
        step: u8,
        bus_operation: &mut F,
    ) where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        match kind {
            CpuInstructionKind::PushRegisterPair { source } => match step {
                0 => {
                    self.operand16_latch = self.read_stack_register16(source);
                    self.decrement_sp_and_record_idu_event();
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let [low, high] = self.operand16_latch.to_le_bytes();
                    self.write_byte_at_sp(high, bus_operation);
                    self.operand8_latch = low;
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::PopRegisterPair { target } => match step {
                0 => {
                    let low = self.read_byte_and_increment_sp(bus_operation);
                    self.operand16_latch = u16::from(low);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_byte_and_increment_sp(bus_operation);
                    let value = self.operand16_latch | (u16::from(high) << 8);
                    self.write_stack_register16(target, value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            _ => unreachable!("non-stack instruction dispatched to stack executor"),
        }
    }
}
