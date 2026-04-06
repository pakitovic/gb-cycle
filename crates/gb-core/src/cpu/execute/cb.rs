use super::super::decode::{CbInstructionKind, Register8Operand};
use super::super::*;

impl CpuCore {
    pub(super) fn execute_cb_prefixed_machine_cycle<F>(
        &mut self,
        opcode: u8,
        step: u8,
        bus_operation: &mut F,
    ) where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        match step {
            0 => {
                let cb_opcode = self.read_pc_u8(bus_operation);
                self.operand8_latch = cb_opcode;

                match self.decode_cb_opcode(cb_opcode) {
                    Some(kind) => match kind.target() {
                        Register8Operand::Register(target) => {
                            let value = self.read_register8(target);
                            if let Some(result) = self.apply_cb_operation(kind, value) {
                                self.write_register8(target, result);
                            }
                            self.finish_instruction();
                        }
                        Register8Operand::IndirectHl => {
                            self.cb_instruction_kind = Some(kind);
                            self.advance_instruction(opcode, 1);
                        }
                    },
                    None => {
                        self.execution_state = CpuExecutionState::DiagnosticTrap {
                            trap: CpuDiagnosticTrap::UnsupportedCbOpcode {
                                opcode: cb_opcode,
                                address: self.registers.pc.wrapping_sub(1),
                            },
                        };
                    }
                }
            }
            1 => match self.cb_instruction_kind {
                Some(kind) if kind.target() == Register8Operand::IndirectHl => {
                    let value = self.read_byte(self.hl(), bus_operation);
                    if let Some(result) = self.apply_cb_operation(kind, value) {
                        self.operand8_latch = result;
                        self.advance_instruction(opcode, 2);
                    } else {
                        self.finish_instruction();
                    }
                }
                _ => self.stall_instruction(opcode, step),
            },
            2 => match self.cb_instruction_kind {
                Some(kind)
                    if kind.target() == Register8Operand::IndirectHl
                        && !matches!(kind, CbInstructionKind::BitTest { .. }) =>
                {
                    self.write_byte(self.hl(), self.operand8_latch, bus_operation);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            _ => self.stall_instruction(opcode, step),
        }
    }
}
