use super::super::decode::CpuInstructionKind;
use super::super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativeJumpPhase {
    ReadOffset,
    CommitTarget,
}

impl RelativeJumpPhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::ReadOffset),
            1 => Some(Self::CommitTarget),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AbsoluteJumpPhase {
    ReadLow,
    ReadHigh,
    CommitTarget,
}

impl AbsoluteJumpPhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::ReadLow),
            1 => Some(Self::ReadHigh),
            2 => Some(Self::CommitTarget),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopPhase {
    ResolveEntry,
}

impl StopPhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::ResolveEntry),
            _ => None,
        }
    }
}

const CALL_STEP_READ_LOW: u8 = 0;
const CALL_STEP_READ_HIGH: u8 = 1;
const CALL_STEP_PREPARE_STACK_PUSH: u8 = 2;
const CALL_STEP_PUSH_PC_HIGH: u8 = 3;
const CALL_STEP_PUSH_PC_LOW_AND_COMMIT_TARGET: u8 = 4;

const RETURN_STEP_READ_LOW: u8 = 0;
const RETURN_STEP_READ_HIGH: u8 = 1;
const RETURN_STEP_COMMIT_TARGET: u8 = 2;

const CONDITIONAL_RETURN_STEP_EVALUATE_CONDITION: u8 = 0;
const CONDITIONAL_RETURN_STEP_READ_LOW: u8 = 1;
const CONDITIONAL_RETURN_STEP_READ_HIGH: u8 = 2;
const CONDITIONAL_RETURN_STEP_COMMIT_TARGET: u8 = 3;

const RESTART_STEP_PREPARE_STACK_PUSH: u8 = 0;
const RESTART_STEP_PUSH_PC_HIGH: u8 = 1;
const RESTART_STEP_PUSH_PC_LOW_AND_COMMIT_VECTOR: u8 = 2;

impl CpuCore {
    pub(super) fn execute_control_flow_machine_cycle(
        &mut self,
        kind: CpuInstructionKind,
        opcode: u8,
        step: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        match kind {
            CpuInstructionKind::RelativeJump => match RelativeJumpPhase::from_step(step) {
                Some(RelativeJumpPhase::ReadOffset) => {
                    self.in_flight.operand8_latch = self.read_pc_u8(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                Some(RelativeJumpPhase::CommitTarget) => {
                    self.registers.pc = self
                        .registers
                        .pc
                        .wrapping_add_signed(i16::from(self.in_flight.operand8_latch as i8));
                    self.finish_instruction();
                }
                None => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::ConditionalRelativeJump { condition } => {
                match RelativeJumpPhase::from_step(step) {
                    Some(RelativeJumpPhase::ReadOffset) => {
                        self.in_flight.operand8_latch = self.read_pc_u8(bus_operation);
                        if self.condition_is_met(Some(condition)) {
                            self.advance_instruction(opcode, 1);
                        } else {
                            self.finish_instruction();
                        }
                    }
                    Some(RelativeJumpPhase::CommitTarget) => {
                        self.registers.pc = self
                            .registers
                            .pc
                            .wrapping_add_signed(i16::from(self.in_flight.operand8_latch as i8));
                        self.finish_instruction();
                    }
                    None => self.stall_instruction(opcode, step),
                }
            }
            CpuInstructionKind::AbsoluteJump => match AbsoluteJumpPhase::from_step(step) {
                Some(AbsoluteJumpPhase::ReadLow) => {
                    self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                Some(AbsoluteJumpPhase::ReadHigh) => {
                    let high = self.read_pc_u8(bus_operation);
                    self.in_flight.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                Some(AbsoluteJumpPhase::CommitTarget) => {
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.finish_instruction();
                }
                None => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::ConditionalAbsoluteJump { condition } => {
                match AbsoluteJumpPhase::from_step(step) {
                    Some(AbsoluteJumpPhase::ReadLow) => {
                        self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                        self.advance_instruction(opcode, 1);
                    }
                    Some(AbsoluteJumpPhase::ReadHigh) => {
                        let high = self.read_pc_u8(bus_operation);
                        self.in_flight.operand16_latch |= u16::from(high) << 8;
                        if self.condition_is_met(Some(condition)) {
                            self.advance_instruction(opcode, 2);
                        } else {
                            self.finish_instruction();
                        }
                    }
                    Some(AbsoluteJumpPhase::CommitTarget) => {
                        self.registers.pc = self.in_flight.operand16_latch;
                        self.finish_instruction();
                    }
                    None => self.stall_instruction(opcode, step),
                }
            }
            CpuInstructionKind::Call => match step {
                CALL_STEP_READ_LOW => {
                    self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, CALL_STEP_READ_HIGH);
                }
                CALL_STEP_READ_HIGH => {
                    let high = self.read_pc_u8(bus_operation);
                    self.in_flight.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, CALL_STEP_PREPARE_STACK_PUSH);
                }
                CALL_STEP_PREPARE_STACK_PUSH => {
                    self.prepare_pc_stack_push();
                    self.advance_instruction(opcode, CALL_STEP_PUSH_PC_HIGH);
                }
                CALL_STEP_PUSH_PC_HIGH => {
                    self.push_pc_high_at_sp(bus_operation);
                    self.advance_instruction(opcode, CALL_STEP_PUSH_PC_LOW_AND_COMMIT_TARGET);
                }
                CALL_STEP_PUSH_PC_LOW_AND_COMMIT_TARGET => {
                    self.push_latched_low_with_decremented_sp(bus_operation);
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::ConditionalCall { condition } => match step {
                CALL_STEP_READ_LOW => {
                    self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, CALL_STEP_READ_HIGH);
                }
                CALL_STEP_READ_HIGH => {
                    let high = self.read_pc_u8(bus_operation);
                    self.in_flight.operand16_latch |= u16::from(high) << 8;
                    if self.condition_is_met(Some(condition)) {
                        self.advance_instruction(opcode, CALL_STEP_PREPARE_STACK_PUSH);
                    } else {
                        self.finish_instruction();
                    }
                }
                CALL_STEP_PREPARE_STACK_PUSH => {
                    self.prepare_pc_stack_push();
                    self.advance_instruction(opcode, CALL_STEP_PUSH_PC_HIGH);
                }
                CALL_STEP_PUSH_PC_HIGH => {
                    self.push_pc_high_at_sp(bus_operation);
                    self.advance_instruction(opcode, CALL_STEP_PUSH_PC_LOW_AND_COMMIT_TARGET);
                }
                CALL_STEP_PUSH_PC_LOW_AND_COMMIT_TARGET => {
                    self.push_latched_low_with_decremented_sp(bus_operation);
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Return => match step {
                RETURN_STEP_READ_LOW => {
                    self.read_latched_stack_low_byte(bus_operation);
                    self.advance_instruction(opcode, RETURN_STEP_READ_HIGH);
                }
                RETURN_STEP_READ_HIGH => {
                    self.read_latched_stack_high_byte(bus_operation);
                    self.advance_instruction(opcode, RETURN_STEP_COMMIT_TARGET);
                }
                RETURN_STEP_COMMIT_TARGET => {
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::ConditionalReturn { condition } => match step {
                CONDITIONAL_RETURN_STEP_EVALUATE_CONDITION => {
                    if self.condition_is_met(Some(condition)) {
                        self.advance_instruction(opcode, CONDITIONAL_RETURN_STEP_READ_LOW);
                    } else {
                        self.finish_instruction();
                    }
                }
                CONDITIONAL_RETURN_STEP_READ_LOW => {
                    self.read_latched_stack_low_byte(bus_operation);
                    self.advance_instruction(opcode, CONDITIONAL_RETURN_STEP_READ_HIGH);
                }
                CONDITIONAL_RETURN_STEP_READ_HIGH => {
                    self.read_latched_stack_high_byte(bus_operation);
                    self.advance_instruction(opcode, CONDITIONAL_RETURN_STEP_COMMIT_TARGET);
                }
                CONDITIONAL_RETURN_STEP_COMMIT_TARGET => {
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::ReturnFromInterrupt => match step {
                RETURN_STEP_READ_LOW => {
                    self.read_latched_stack_low_byte(bus_operation);
                    self.advance_instruction(opcode, RETURN_STEP_READ_HIGH);
                }
                RETURN_STEP_READ_HIGH => {
                    self.read_latched_stack_high_byte(bus_operation);
                    self.advance_instruction(opcode, RETURN_STEP_COMMIT_TARGET);
                }
                RETURN_STEP_COMMIT_TARGET => {
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.set_ime_enabled();
                    self.cancel_delayed_ime_enable();
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Stop => match StopPhase::from_step(step) {
                Some(StopPhase::ResolveEntry) => {
                    self.request_stop_div_reset();
                    let wake_line_asserted = self.stop_wake_line_asserted(bus_operation);
                    let pending_interrupt = if self.ime() {
                        false
                    } else {
                        self.current_highest_pending_interrupt(bus_operation)
                            .is_some()
                    };

                    match self.stop_entry_resolution(wake_line_asserted, pending_interrupt) {
                        StopEntryResolution::CompleteOnCurrentMachineCycle => {
                            // The real fetch path finishes this branch on the opcode
                            // fetch M-cycle so STOP behaves like a one-byte NOP.
                            // Keep the execute-stage fallback aligned for direct unit
                            // tests and any future non-fetch entry paths.
                            self.finish_instruction();
                        }
                        StopEntryResolution::EnterStoppedAfterPaddingFetch => {
                            let _ = self.read_pc_u8(bus_operation);
                            self.enter_stopped_state();
                        }
                        StopEntryResolution::EnterZombieStopped => {
                            self.enter_zombie_stopped_state();
                        }
                        StopEntryResolution::EnterHaltAfterPaddingFetch => {
                            let _ = self.read_pc_u8(bus_operation);
                            self.finish_and_request_halt();
                        }
                    }
                }
                None => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Restart { vector } => match step {
                RESTART_STEP_PREPARE_STACK_PUSH => {
                    self.prepare_pc_stack_push();
                    self.advance_instruction(opcode, RESTART_STEP_PUSH_PC_HIGH);
                }
                RESTART_STEP_PUSH_PC_HIGH => {
                    self.push_pc_high_at_sp(bus_operation);
                    self.advance_instruction(opcode, RESTART_STEP_PUSH_PC_LOW_AND_COMMIT_VECTOR);
                }
                RESTART_STEP_PUSH_PC_LOW_AND_COMMIT_VECTOR => {
                    self.push_latched_low_with_decremented_sp(bus_operation);
                    self.registers.pc = vector;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            _ => unreachable!("non-control-flow instruction dispatched to control-flow executor"),
        }
    }
}
