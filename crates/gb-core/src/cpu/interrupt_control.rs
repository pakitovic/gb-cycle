use crate::interrupts::InterruptController;
use crate::joypad::Joypad;

use super::state::{highest_pending_interrupt_from_mask, interrupt_vector};
use super::*;

impl CpuCore {
    fn is_stop_sleep_state(&self) -> bool {
        matches!(
            self.execution_state,
            CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
        )
    }

    const fn interrupt_source_mask(source: InterruptSource) -> u8 {
        match source {
            InterruptSource::VBlank => 0x01,
            InterruptSource::LcdStat => 0x02,
            InterruptSource::Timer => 0x04,
            InterruptSource::Serial => 0x08,
            InterruptSource::Joypad => 0x10,
        }
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
                    && self.ime
                    && interrupts.pending_mask()
                        & Self::interrupt_source_mask(InterruptSource::Joypad)
                        != 0
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

        if self.halt_request_pending {
            self.halt_request_pending = false;
            let halt_request_ime = self.halt_request_ime;
            let halt_request_had_delayed_ei = self.halt_request_had_delayed_ei;
            self.halt_request_ime = false;
            self.halt_request_had_delayed_ei = false;

            if !halt_request_ime && pending {
                if halt_request_had_delayed_ei && self.ime {
                    self.registers.pc = self.registers.pc.wrapping_sub(1);
                    self.accept_pending_interrupt(interrupts);
                } else {
                    self.halt_bug_pending = true;
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

            if self.ime {
                self.accept_pending_interrupt(interrupts);
            } else {
                self.execution_state = CpuExecutionState::fetch_opcode();
            }
            return;
        }

        if !self.ime || !self.can_accept_interrupt() {
            return;
        }

        self.accept_pending_interrupt(interrupts);
    }

    pub(super) fn finish_and_request_halt(&mut self) {
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.halt_request_ime = self.ime;
        self.halt_request_had_delayed_ei = self.delayed_ime_enable;
        self.advance_delayed_ime_enable();
        self.halt_request_pending = true;
        self.execution_state = CpuExecutionState::fetch_opcode();
    }

    fn begin_interrupt_service(&mut self, source: InterruptSource) {
        self.ime = false;
        self.cancel_delayed_ime_enable();
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
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
        self.ime = false;
        self.cancel_delayed_ime_enable();
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
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
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.execution_state = CpuExecutionState::fetch_opcode();
    }

    pub(super) fn complete_interrupt_service_machine_cycle(
        &mut self,
        source: InterruptSource,
        step: u8,
        bus_operation: &mut CpuBusCallback<'_>,
    ) {
        match step {
            0 | 1 => {
                self.advance_interrupt_service(source, step + 1);
            }
            2 => {
                let [low, _high] = self.registers.pc.to_le_bytes();
                self.operand8_latch = low;
                self.decrement_sp_and_record_idu_event();
                self.advance_interrupt_service(source, 3);
            }
            3 => {
                let [_low, high] = self.registers.pc.to_le_bytes();
                let upper_pc_push_targets_ie = self.registers.sp == 0xFFFF;
                self.write_byte_at_sp(high, bus_operation);
                if upper_pc_push_targets_ie {
                    let pending_mask = self.current_pending_interrupt_mask(bus_operation);
                    let current_ie = self.current_interrupt_enable_mask(bus_operation);
                    // The accepted source was already acknowledged in IF when
                    // service began, but it remains latched internally until
                    // the upper-byte IE write has a chance to cancel or
                    // retarget the dispatch.
                    let candidate_mask =
                        pending_mask | (current_ie & Self::interrupt_source_mask(source));

                    if let Some(next_source) = highest_pending_interrupt_from_mask(candidate_mask) {
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
            4 => {
                self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                self.registers.pc = interrupt_vector(source);
                self.finish_interrupt_service();
            }
            _ => {
                self.advance_interrupt_service(source, step);
            }
        }
    }

    pub(super) fn complete_stop_wake_bugged_interrupt_service_machine_cycle(
        &mut self,
        step: u8,
        bus_operation: &mut CpuBusCallback<'_>,
    ) {
        match step {
            0 | 1 => {
                self.advance_stop_wake_bugged_interrupt_service(step + 1);
            }
            2 => {
                let [low, _high] = self.registers.pc.to_le_bytes();
                self.operand8_latch = low;
                self.decrement_sp_and_record_idu_event();
                self.advance_stop_wake_bugged_interrupt_service(3);
            }
            3 => {
                let [_low, high] = self.registers.pc.to_le_bytes();
                self.write_byte_at_sp(high, bus_operation);
                self.advance_stop_wake_bugged_interrupt_service(4);
            }
            4 => {
                // Hardware research describes STOP wake with IME=1 as a bugged
                // interrupt that vectors to 0x0000 and often corrupts the stack.
                // The current repo baseline makes that corruption deterministic
                // by dropping the final push-side SP decrement, so the low byte
                // overwrites the previously written high-byte slot.
                self.write_byte_at_sp(self.operand8_latch, bus_operation);
                self.registers.pc = 0x0000;
                self.finish_interrupt_service();
            }
            _ => {
                self.advance_stop_wake_bugged_interrupt_service(step);
            }
        }
    }

    fn can_accept_interrupt(&self) -> bool {
        matches!(self.execution_state, CpuExecutionState::FetchOpcode { .. })
            && self.current_opcode.is_none()
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
        bus_operation: &mut CpuBusCallback<'_>,
    ) {
        let _ = bus_operation(CpuBusOperation::RequestInterrupt { source });
    }

    fn acknowledge_interrupt(
        &mut self,
        source: InterruptSource,
        bus_operation: &mut CpuBusCallback<'_>,
    ) {
        let _ = bus_operation(CpuBusOperation::AcknowledgeInterrupt { source });
    }

    pub(super) fn current_pending_interrupt_mask(
        &mut self,
        bus_operation: &mut CpuBusCallback<'_>,
    ) -> u8 {
        bus_operation(CpuBusOperation::PendingInterruptMask).unwrap_or(0)
    }

    pub(super) fn current_highest_pending_interrupt(
        &mut self,
        bus_operation: &mut CpuBusCallback<'_>,
    ) -> Option<InterruptSource> {
        highest_pending_interrupt_from_mask(self.current_pending_interrupt_mask(bus_operation))
    }

    pub(super) fn current_interrupt_enable_mask(
        &mut self,
        bus_operation: &mut CpuBusCallback<'_>,
    ) -> u8 {
        bus_operation(CpuBusOperation::InterruptEnableMask).unwrap_or(0)
    }

    pub(super) fn schedule_delayed_ime_enable(&mut self) {
        if self.delayed_ime_enable {
            return;
        }

        self.delayed_ime_enable = true;
        self.delayed_ime_enable_steps = 2;
    }

    pub(super) fn cancel_delayed_ime_enable(&mut self) {
        self.delayed_ime_enable = false;
        self.delayed_ime_enable_steps = 0;
    }

    pub(super) fn advance_delayed_ime_enable(&mut self) {
        if self.delayed_ime_enable_steps == 0 {
            self.delayed_ime_enable = false;
            return;
        }

        self.delayed_ime_enable_steps -= 1;
        if self.delayed_ime_enable_steps == 0 {
            self.ime = true;
            self.delayed_ime_enable = false;
        }
    }
}
