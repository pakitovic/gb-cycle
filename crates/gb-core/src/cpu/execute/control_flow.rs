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
enum CallPhase {
    ReadLow,
    ReadHigh,
    DecrementSp,
    PushHigh,
    PushLowAndCommit,
}

impl CallPhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::ReadLow),
            1 => Some(Self::ReadHigh),
            2 => Some(Self::DecrementSp),
            3 => Some(Self::PushHigh),
            4 => Some(Self::PushLowAndCommit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReturnPhase {
    ReadLow,
    ReadHigh,
    CommitTarget,
}

impl ReturnPhase {
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
enum ConditionalReturnPhase {
    CheckCondition,
    ReadLow,
    ReadHigh,
    CommitTarget,
}

impl ConditionalReturnPhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::CheckCondition),
            1 => Some(Self::ReadLow),
            2 => Some(Self::ReadHigh),
            3 => Some(Self::CommitTarget),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestartPhase {
    DecrementSp,
    PushHigh,
    PushLowAndCommit,
}

impl RestartPhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::DecrementSp),
            1 => Some(Self::PushHigh),
            2 => Some(Self::PushLowAndCommit),
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
            CpuInstructionKind::Call => match CallPhase::from_step(step) {
                Some(CallPhase::ReadLow) => {
                    self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                Some(CallPhase::ReadHigh) => {
                    let high = self.read_pc_u8(bus_operation);
                    self.in_flight.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                Some(CallPhase::DecrementSp) => {
                    self.prepare_pc_stack_push();
                    self.advance_instruction(opcode, 3);
                }
                Some(CallPhase::PushHigh) => {
                    self.push_pc_high_at_sp(bus_operation);
                    self.advance_instruction(opcode, 4);
                }
                Some(CallPhase::PushLowAndCommit) => {
                    self.push_latched_low_with_decremented_sp(bus_operation);
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.finish_instruction();
                }
                None => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::ConditionalCall { condition } => match CallPhase::from_step(step) {
                Some(CallPhase::ReadLow) => {
                    self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                Some(CallPhase::ReadHigh) => {
                    let high = self.read_pc_u8(bus_operation);
                    self.in_flight.operand16_latch |= u16::from(high) << 8;
                    if self.condition_is_met(Some(condition)) {
                        self.advance_instruction(opcode, 2);
                    } else {
                        self.finish_instruction();
                    }
                }
                Some(CallPhase::DecrementSp) => {
                    self.prepare_pc_stack_push();
                    self.advance_instruction(opcode, 3);
                }
                Some(CallPhase::PushHigh) => {
                    self.push_pc_high_at_sp(bus_operation);
                    self.advance_instruction(opcode, 4);
                }
                Some(CallPhase::PushLowAndCommit) => {
                    self.push_latched_low_with_decremented_sp(bus_operation);
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.finish_instruction();
                }
                None => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Return => match ReturnPhase::from_step(step) {
                Some(ReturnPhase::ReadLow) => {
                    self.read_latched_stack_low_byte(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                Some(ReturnPhase::ReadHigh) => {
                    self.read_latched_stack_high_byte(bus_operation);
                    self.advance_instruction(opcode, 2);
                }
                Some(ReturnPhase::CommitTarget) => {
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.finish_instruction();
                }
                None => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::ConditionalReturn { condition } => {
                match ConditionalReturnPhase::from_step(step) {
                    Some(ConditionalReturnPhase::CheckCondition) => {
                        if self.condition_is_met(Some(condition)) {
                            self.advance_instruction(opcode, 1);
                        } else {
                            self.finish_instruction();
                        }
                    }
                    Some(ConditionalReturnPhase::ReadLow) => {
                        self.read_latched_stack_low_byte(bus_operation);
                        self.advance_instruction(opcode, 2);
                    }
                    Some(ConditionalReturnPhase::ReadHigh) => {
                        self.read_latched_stack_high_byte(bus_operation);
                        self.advance_instruction(opcode, 3);
                    }
                    Some(ConditionalReturnPhase::CommitTarget) => {
                        self.registers.pc = self.in_flight.operand16_latch;
                        self.finish_instruction();
                    }
                    None => self.stall_instruction(opcode, step),
                }
            }
            CpuInstructionKind::ReturnFromInterrupt => match ReturnPhase::from_step(step) {
                Some(ReturnPhase::ReadLow) => {
                    self.read_latched_stack_low_byte(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                Some(ReturnPhase::ReadHigh) => {
                    self.read_latched_stack_high_byte(bus_operation);
                    self.advance_instruction(opcode, 2);
                }
                Some(ReturnPhase::CommitTarget) => {
                    self.registers.pc = self.in_flight.operand16_latch;
                    self.set_ime_enabled();
                    self.cancel_delayed_ime_enable();
                    self.finish_instruction();
                }
                None => self.stall_instruction(opcode, step),
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
            CpuInstructionKind::Restart { vector } => match RestartPhase::from_step(step) {
                Some(RestartPhase::DecrementSp) => {
                    self.prepare_pc_stack_push();
                    self.advance_instruction(opcode, 1);
                }
                Some(RestartPhase::PushHigh) => {
                    self.push_pc_high_at_sp(bus_operation);
                    self.advance_instruction(opcode, 2);
                }
                Some(RestartPhase::PushLowAndCommit) => {
                    self.push_latched_low_with_decremented_sp(bus_operation);
                    self.registers.pc = vector;
                    self.finish_instruction();
                }
                None => self.stall_instruction(opcode, step),
            },
            _ => unreachable!("non-control-flow instruction dispatched to control-flow executor"),
        }
    }
}
