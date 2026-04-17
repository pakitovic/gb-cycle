use crate::interrupts::InterruptController;
use crate::joypad::Joypad;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterruptServicePhase {
    InternalWait0,
    InternalWait1,
    PrepareStackWrite,
    PushHighAndResolveVector,
    PushLowAndCommit,
}

impl InterruptServicePhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::InternalWait0),
            1 => Some(Self::InternalWait1),
            2 => Some(Self::PrepareStackWrite),
            3 => Some(Self::PushHighAndResolveVector),
            4 => Some(Self::PushLowAndCommit),
            _ => None,
        }
    }

    const fn next_step(self) -> u8 {
        match self {
            Self::InternalWait0 => 1,
            Self::InternalWait1 => 2,
            Self::PrepareStackWrite => 3,
            Self::PushHighAndResolveVector => 4,
            Self::PushLowAndCommit => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopWakeBuggedInterruptServicePhase {
    InternalWait0,
    InternalWait1,
    PrepareStackWrite,
    PushHigh,
    OverwriteHighSlotWithLowAndCommit,
}

impl StopWakeBuggedInterruptServicePhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::InternalWait0),
            1 => Some(Self::InternalWait1),
            2 => Some(Self::PrepareStackWrite),
            3 => Some(Self::PushHigh),
            4 => Some(Self::OverwriteHighSlotWithLowAndCommit),
            _ => None,
        }
    }

    const fn next_step(self) -> u8 {
        match self {
            Self::InternalWait0 => 1,
            Self::InternalWait1 => 2,
            Self::PrepareStackWrite => 3,
            Self::PushHigh => 4,
            Self::OverwriteHighSlotWithLowAndCommit => 4,
        }
    }
}

impl CpuCore {
    fn is_stop_sleep_state(&self) -> bool {
        matches!(
            self.execution_state,
            CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
        )
    }

    pub(crate) fn evaluate_wake_and_interrupts(
        &mut self,
        interrupts: &mut InterruptController,
        joypad: &mut Joypad,
    ) {
        if !self.is_stop_sleep_state() {
            let _ = joypad.consume_stop_wake_event();
        }

        if matches!(
            self.execution_state,
            CpuExecutionState::DiagnosticTrap { .. }
        ) {
            return;
        }

        if self.is_stop_sleep_state() {
            if joypad.consume_stop_wake_event() {
                if matches!(self.execution_state, CpuExecutionState::Stopped)
                    && self.ime()
                    && interrupts.pending_mask() & InterruptSource::Joypad.mask() != 0
                {
                    interrupts.clear(InterruptSource::Joypad);
                    self.begin_stop_wake_bugged_interrupt_service();
                } else {
                    self.execution_state = CpuExecutionState::fetch_opcode();
                }
            }
            return;
        }

        let pending = interrupts.pending_mask() != 0;

        if let Some(halt_request) = self.take_halt_request() {
            if !halt_request.ime_enabled && pending {
                if halt_request.had_pending_ei && self.ime() {
                    self.registers.pc = self.registers.pc.wrapping_sub(1);
                    self.accept_pending_interrupt(interrupts);
                } else {
                    self.arm_halt_bug();
                    self.execution_state = CpuExecutionState::fetch_opcode();
                }
            } else if pending {
                self.accept_pending_interrupt(interrupts);
            } else {
                self.execution_state = CpuExecutionState::Halted;
            }
            return;
        }

        if matches!(self.execution_state, CpuExecutionState::Halted) {
            if !pending {
                return;
            }

            if self.ime() {
                self.accept_pending_interrupt(interrupts);
            } else {
                self.execution_state = CpuExecutionState::fetch_opcode();
            }
            return;
        }

        if !self.ime() || !self.can_accept_interrupt() {
            return;
        }

        self.accept_pending_interrupt(interrupts);
    }

    pub(super) fn finish_and_request_halt(&mut self) {
        self.clear_in_flight_instruction_state();
        let ime_enabled = self.ime();
        let had_pending_ei = self.delayed_ime_enable();
        self.advance_delayed_ime_enable();
        self.request_halt_after_current_instruction(ime_enabled, had_pending_ei);
        self.execution_state = CpuExecutionState::fetch_opcode();
    }

    fn begin_interrupt_service(&mut self, source: InterruptSource) {
        self.set_ime_disabled();
        self.cancel_delayed_ime_enable();
        self.clear_in_flight_instruction_state();
        self.execution_state = CpuExecutionState::ServiceInterrupt {
            source,
            step: 0,
            t_cycle: 0,
        };
    }

    fn advance_interrupt_service(&mut self, source: InterruptSource, next_step: u8) {
        self.execution_state = CpuExecutionState::ServiceInterrupt {
            source,
            step: next_step,
            t_cycle: 0,
        };
    }

    fn begin_stop_wake_bugged_interrupt_service(&mut self) {
        self.set_ime_disabled();
        self.cancel_delayed_ime_enable();
        self.clear_in_flight_instruction_state();
        self.execution_state = CpuExecutionState::ServiceStopWakeBuggedInterrupt {
            step: 0,
            t_cycle: 0,
        };
    }

    fn advance_stop_wake_bugged_interrupt_service(&mut self, next_step: u8) {
        self.execution_state = CpuExecutionState::ServiceStopWakeBuggedInterrupt {
            step: next_step,
            t_cycle: 0,
        };
    }

    fn finish_interrupt_service(&mut self) {
        self.clear_in_flight_instruction_state();
        self.execution_state = CpuExecutionState::fetch_opcode();
    }

    pub(super) fn complete_interrupt_service_machine_cycle(
        &mut self,
        source: InterruptSource,
        step: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        let Some(phase) = InterruptServicePhase::from_step(step) else {
            self.advance_interrupt_service(source, step);
            return;
        };

        match phase {
            InterruptServicePhase::InternalWait0 | InterruptServicePhase::InternalWait1 => {
                self.advance_interrupt_service(source, phase.next_step());
            }
            InterruptServicePhase::PrepareStackWrite => {
                self.prepare_pc_stack_push();
                self.advance_interrupt_service(source, 3);
            }
            InterruptServicePhase::PushHighAndResolveVector => {
                let upper_pc_push_targets_ie = self.registers.sp == 0xFFFF;
                self.push_pc_high_at_sp(bus_operation);
                if upper_pc_push_targets_ie {
                    let pending_mask = self.current_pending_interrupt_mask(bus_operation);
                    let current_ie = self.current_interrupt_enable_mask(bus_operation);
                    // The accepted source was already acknowledged in IF when
                    // service began, but it remains latched internally until
                    // the upper-byte IE write has a chance to cancel or
                    // retarget the dispatch.
                    let candidate_mask = pending_mask | (current_ie & source.mask());

                    if let Some(next_source) =
                        InterruptSource::highest_priority_from_mask(candidate_mask)
                    {
                        if next_source != source {
                            self.request_interrupt(source, bus_operation);
                            self.acknowledge_interrupt(next_source, bus_operation);
                        }
                        self.advance_interrupt_service(next_source, 4);
                    } else {
                        self.request_interrupt(source, bus_operation);
                        self.registers.pc = 0x0000;
                        self.finish_interrupt_service();
                    }
                } else {
                    self.advance_interrupt_service(source, 4);
                }
            }
            InterruptServicePhase::PushLowAndCommit => {
                self.push_latched_low_with_decremented_sp(bus_operation);
                self.registers.pc = source.vector();
                self.finish_interrupt_service();
            }
        }
    }

    pub(super) fn complete_stop_wake_bugged_interrupt_service_machine_cycle(
        &mut self,
        step: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        let Some(phase) = StopWakeBuggedInterruptServicePhase::from_step(step) else {
            self.advance_stop_wake_bugged_interrupt_service(step);
            return;
        };

        match phase {
            StopWakeBuggedInterruptServicePhase::InternalWait0
            | StopWakeBuggedInterruptServicePhase::InternalWait1 => {
                self.advance_stop_wake_bugged_interrupt_service(phase.next_step());
            }
            StopWakeBuggedInterruptServicePhase::PrepareStackWrite => {
                self.prepare_pc_stack_push();
                self.advance_stop_wake_bugged_interrupt_service(3);
            }
            StopWakeBuggedInterruptServicePhase::PushHigh => {
                self.push_pc_high_at_sp(bus_operation);
                self.advance_stop_wake_bugged_interrupt_service(4);
            }
            StopWakeBuggedInterruptServicePhase::OverwriteHighSlotWithLowAndCommit => {
                // Hardware research describes STOP wake with IME=1 as a bugged
                // interrupt that vectors to 0x0000 and often corrupts the stack.
                // The current repo baseline makes that corruption deterministic
                // by dropping the final push-side SP decrement, so the low byte
                // overwrites the previously written high-byte slot.
                self.write_byte_at_sp(self.in_flight.operand8_latch, bus_operation);
                self.registers.pc = 0x0000;
                self.finish_interrupt_service();
            }
        }
    }

    fn can_accept_interrupt(&self) -> bool {
        matches!(self.execution_state, CpuExecutionState::FetchOpcode { .. })
            && self.in_flight.opcode.is_none()
    }

    fn accept_pending_interrupt(&mut self, interrupts: &mut InterruptController) {
        let Some(source) = interrupts.highest_pending() else {
            return;
        };

        // Pan Docs documents the IF acknowledge at interrupt acceptance. The
        // accepted source stays latched in ServiceInterrupt so IE@FFFF can
        // still cancel or retarget later in the push sequence.
        interrupts.clear(source);
        self.begin_interrupt_service(source);
    }

    fn request_interrupt(
        &mut self,
        source: InterruptSource,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        let _ = bus_operation(CpuExternalOperation::RequestInterrupt { source });
    }

    fn acknowledge_interrupt(
        &mut self,
        source: InterruptSource,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        let _ = bus_operation(CpuExternalOperation::AcknowledgeInterrupt { source });
    }

    pub(super) fn current_pending_interrupt_mask(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) -> u8 {
        bus_operation(CpuExternalOperation::PendingInterruptMask).unwrap_or(0)
    }

    pub(super) fn current_highest_pending_interrupt(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) -> Option<InterruptSource> {
        InterruptSource::highest_priority_from_mask(
            self.current_pending_interrupt_mask(bus_operation),
        )
    }

    pub(super) fn current_interrupt_enable_mask(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) -> u8 {
        bus_operation(CpuExternalOperation::InterruptEnableMask).unwrap_or(0)
    }

    pub(super) fn schedule_delayed_ime_enable(&mut self) {
        if self.delayed_ime_enable() {
            return;
        }

        self.ime_state = if self.ime() {
            ImeState::EnabledPendingEnable {
                instructions_remaining: 2,
            }
        } else {
            ImeState::DisabledPendingEnable {
                instructions_remaining: 2,
            }
        };
    }

    pub(super) fn cancel_delayed_ime_enable(&mut self) {
        self.ime_state = if self.ime() {
            ImeState::Enabled
        } else {
            ImeState::Disabled
        };
    }

    pub(super) fn advance_delayed_ime_enable(&mut self) {
        match self.ime_state {
            ImeState::DisabledPendingEnable {
                instructions_remaining,
            } if instructions_remaining > 1 => {
                self.ime_state = ImeState::DisabledPendingEnable {
                    instructions_remaining: instructions_remaining - 1,
                };
            }
            ImeState::EnabledPendingEnable {
                instructions_remaining,
            } if instructions_remaining > 1 => {
                self.ime_state = ImeState::EnabledPendingEnable {
                    instructions_remaining: instructions_remaining - 1,
                };
            }
            ImeState::DisabledPendingEnable { .. } | ImeState::EnabledPendingEnable { .. } => {
                self.set_ime_enabled();
            }
            _ => {}
        }
    }
}
