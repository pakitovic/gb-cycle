use super::super::decode::CpuInstructionKind;
use super::super::*;

impl CpuCore {
    pub(super) fn execute_stack_machine_cycle(
        &mut self,
        kind: CpuInstructionKind,
        opcode: u8,
        step: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        match kind {
            CpuInstructionKind::PushRegisterPair { source } => match step {
                0 => {
                    self.in_flight.operand16_latch = self.read_stack_register16(source);
                    self.decrement_sp_and_record_idu_event();
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let [low, _high] = self.in_flight.operand16_latch.to_le_bytes();
                    self.push_latched_u16_high_at_sp(bus_operation);
                    self.in_flight.operand8_latch = low;
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    self.push_latched_low_with_decremented_sp(bus_operation);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::PopRegisterPair { target } => match step {
                0 => {
                    self.read_latched_stack_low_byte(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    self.read_latched_stack_high_byte(bus_operation);
                    self.write_stack_register16(target, self.in_flight.operand16_latch);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            _ => unreachable!("non-stack instruction dispatched to stack executor"),
        }
    }
}
