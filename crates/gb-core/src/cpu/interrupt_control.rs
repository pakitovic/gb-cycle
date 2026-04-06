use crate::interrupts::InterruptController;
use crate::joypad::Joypad;

use super::state::{highest_pending_interrupt_from_mask, interrupt_vector};
use super::*;

impl CpuCore {
    pub(crate) fn evaluate_wake_and_interrupts(
        &mut self,
        interrupts: &mut InterruptController,
        joypad: &mut Joypad,
    ) {
        if matches!(
            self.execution_state,
            CpuExecutionState::DiagnosticTrap { .. }
        ) {
            return;
        }

        if matches!(self.execution_state, CpuExecutionState::Stopped) {
            if joypad.consume_stop_wake_event() {
                self.execution_state = CpuExecutionState::fetch_opcode();
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

    fn finish_interrupt_service(&mut self) {
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.execution_state = CpuExecutionState::fetch_opcode();
    }

    pub(super) fn complete_interrupt_service_machine_cycle<F>(
        &mut self,
        source: InterruptSource,
        step: u8,
        bus_operation: &mut F,
    ) -> Option<InterruptSource>
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        match step {
            0 | 1 => {
                self.advance_interrupt_service(source, step + 1);
                None
            }
            2 => {
                let [low, _high] = self.registers.pc.to_le_bytes();
                self.operand8_latch = low;
                self.decrement_sp_and_record_idu_event();
                self.advance_interrupt_service(source, 3);
                None
            }
            3 => {
                let [_low, high] = self.registers.pc.to_le_bytes();
                let upper_pc_push_targets_ie = self.registers.sp == 0xFFFF;
                self.write_byte_at_sp(high, bus_operation);
                // IE can be the target of the upper-byte push at 0xFFFF, so the
                // dispatch source must stay live until after this write commits.
                if upper_pc_push_targets_ie {
                    if let Some(next_source) = self.current_highest_pending_interrupt(bus_operation)
                    {
                        self.advance_interrupt_service(next_source, 4);
                    } else {
                        self.registers.pc = 0x0000;
                        self.finish_interrupt_service();
                    }
                } else {
                    self.advance_interrupt_service(source, 4);
                }
                None
            }
            4 => {
                self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                self.registers.pc = interrupt_vector(source);
                self.finish_interrupt_service();
                Some(source)
            }
            _ => {
                self.advance_interrupt_service(source, step);
                None
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

        self.begin_interrupt_service(source);
    }

    pub(super) fn current_highest_pending_interrupt<F>(
        &mut self,
        bus_operation: &mut F,
    ) -> Option<InterruptSource>
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let pending_mask = bus_operation(CpuBusOperation::PendingInterruptMask).unwrap_or(0);
        highest_pending_interrupt_from_mask(pending_mask)
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
