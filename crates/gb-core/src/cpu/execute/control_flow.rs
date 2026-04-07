use super::super::decode::CpuInstructionKind;
use super::super::*;

impl CpuCore {
    pub(super) fn execute_control_flow_machine_cycle(
        &mut self,
        kind: CpuInstructionKind,
        opcode: u8,
        step: u8,
        bus_operation: &mut CpuBusCallback<'_>,
    ) {
        match kind {
            CpuInstructionKind::RelativeJump { condition } => match step {
                0 => {
                    self.operand8_latch = self.read_pc_u8(bus_operation);
                    if self.condition_is_met(condition) {
                        self.advance_instruction(opcode, 1);
                    } else {
                        self.finish_instruction();
                    }
                }
                1 => {
                    self.registers.pc = self
                        .registers
                        .pc
                        .wrapping_add_signed(i16::from(self.operand8_latch as i8));
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AbsoluteJump { condition } => match step {
                0 => {
                    self.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_pc_u8(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    if self.condition_is_met(condition) {
                        self.advance_instruction(opcode, 2);
                    } else {
                        self.finish_instruction();
                    }
                }
                2 => {
                    self.registers.pc = self.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Call { condition } => match step {
                0 => {
                    self.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_pc_u8(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    if self.condition_is_met(condition) {
                        self.advance_instruction(opcode, 2);
                    } else {
                        self.finish_instruction();
                    }
                }
                2 => {
                    self.decrement_sp_and_record_idu_event();
                    self.advance_instruction(opcode, 3);
                }
                3 => {
                    let [low, high] = self.registers.pc.to_le_bytes();
                    self.write_byte_at_sp(high, bus_operation);
                    self.operand8_latch = low;
                    self.advance_instruction(opcode, 4);
                }
                4 => {
                    self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                    self.registers.pc = self.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Return { condition } => match step {
                0 => {
                    if condition.is_some() {
                        if self.condition_is_met(condition) {
                            self.advance_instruction(opcode, 1);
                        } else {
                            self.finish_instruction();
                        }
                    } else {
                        let low = self.read_byte_and_increment_sp(bus_operation);
                        self.operand16_latch = u16::from(low);
                        self.advance_instruction(opcode, 1);
                    }
                }
                1 => {
                    if condition.is_some() {
                        let low = self.read_byte_and_increment_sp(bus_operation);
                        self.operand16_latch = u16::from(low);
                        self.advance_instruction(opcode, 2);
                    } else {
                        let high = self.read_byte_and_increment_sp(bus_operation);
                        self.operand16_latch |= u16::from(high) << 8;
                        self.advance_instruction(opcode, 2);
                    }
                }
                2 => {
                    if condition.is_some() {
                        let high = self.read_byte_and_increment_sp(bus_operation);
                        self.operand16_latch |= u16::from(high) << 8;
                        self.advance_instruction(opcode, 3);
                    } else {
                        self.registers.pc = self.operand16_latch;
                        self.finish_instruction();
                    }
                }
                3 => {
                    self.registers.pc = self.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::ReturnFromInterrupt => match step {
                0 => {
                    let low = self.read_byte_and_increment_sp(bus_operation);
                    self.operand16_latch = u16::from(low);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_byte_and_increment_sp(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    self.registers.pc = self.operand16_latch;
                    self.ime = true;
                    self.cancel_delayed_ime_enable();
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Stop => match step {
                0 => {
                    let wake_line_asserted = self.stop_wake_line_asserted(bus_operation);

                    if !self.ime {
                        let pending_interrupt = self
                            .current_highest_pending_interrupt(bus_operation)
                            .is_some();

                        match (wake_line_asserted, pending_interrupt) {
                            (false, false) => {
                                let _ = self.read_pc_u8(bus_operation);
                                self.enter_stopped_state();
                            }
                            (false, true) => {
                                self.enter_zombie_stopped_state();
                            }
                            (true, false) => {
                                let _ = self.read_pc_u8(bus_operation);
                                self.finish_and_request_halt();
                            }
                            (true, true) => {
                                self.finish_instruction();
                            }
                        }
                    } else if wake_line_asserted {
                        // The real fetch path finishes this branch on the opcode
                        // fetch M-cycle so STOP behaves like a one-byte NOP.
                        // Keep the execute-stage fallback aligned for direct unit
                        // tests and any future non-fetch entry paths.
                        self.finish_instruction();
                    } else {
                        let _ = self.read_pc_u8(bus_operation);
                        self.enter_stopped_state();
                    }
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Restart { vector } => match step {
                0 => {
                    self.decrement_sp_and_record_idu_event();
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let [low, high] = self.registers.pc.to_le_bytes();
                    self.write_byte_at_sp(high, bus_operation);
                    self.operand8_latch = low;
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                    self.registers.pc = vector;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            _ => unreachable!("non-control-flow instruction dispatched to control-flow executor"),
        }
    }
}
