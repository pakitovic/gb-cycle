use super::decode::{CpuInstructionKind, DecodedOpcode};
use super::*;

impl CpuCore {
    pub(crate) fn tick_t_cycle<F>(&mut self, mut bus_operation: F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.last_bus_activity = None;
        self.last_address_event = None;
        let bus_operation: &mut CpuBusCallback<'_> = &mut bus_operation;

        match self.execution_state {
            CpuExecutionState::FetchOpcode { t_cycle } => {
                if t_cycle < LAST_MACHINE_CYCLE_T {
                    self.execution_state = CpuExecutionState::FetchOpcode {
                        t_cycle: t_cycle + 1,
                    };
                    return;
                }

                self.complete_fetch_opcode(bus_operation);
            }
            CpuExecutionState::Execute {
                opcode,
                step,
                t_cycle,
            } => {
                if t_cycle < LAST_MACHINE_CYCLE_T {
                    self.execution_state = CpuExecutionState::Execute {
                        opcode,
                        step,
                        t_cycle: t_cycle + 1,
                    };
                    return;
                }

                self.complete_execute_machine_cycle(opcode, step, bus_operation);
            }
            CpuExecutionState::ServiceInterrupt {
                source,
                step,
                t_cycle,
            } => {
                if t_cycle < LAST_MACHINE_CYCLE_T {
                    self.execution_state = CpuExecutionState::ServiceInterrupt {
                        source,
                        step,
                        t_cycle: t_cycle + 1,
                    };
                    return;
                }

                self.complete_interrupt_service_machine_cycle(source, step, bus_operation);
            }
            CpuExecutionState::ServiceStopWakeBuggedInterrupt { step, t_cycle } => {
                if t_cycle < LAST_MACHINE_CYCLE_T {
                    self.execution_state = CpuExecutionState::ServiceStopWakeBuggedInterrupt {
                        step,
                        t_cycle: t_cycle + 1,
                    };
                    return;
                }

                self.complete_stop_wake_bugged_interrupt_service_machine_cycle(step, bus_operation);
            }
            CpuExecutionState::DiagnosticTrap { .. }
            | CpuExecutionState::Halted
            | CpuExecutionState::ZombieStopped
            | CpuExecutionState::Stopped => {}
        }
    }

    fn complete_fetch_opcode(&mut self, bus_operation: &mut CpuBusCallback<'_>) {
        let opcode = self.read_opcode_u8(bus_operation);
        self.current_opcode = Some(opcode);

        // STOP can collapse into a one-byte NOP-like path directly on the fetch
        // M-cycle. For the current repo baseline this covers:
        // - IME=0, WAKE=1, pending IRQ
        // - IME=1, WAKE=1
        //
        // The later joypad-owned wake event while already in the explicit
        // Stopped state follows a different contract: if IME=1 and the joypad
        // interrupt is pending, interrupt_control.rs routes it through the
        // explicit ServiceStopWakeBuggedInterrupt path.
        if opcode == 0x10 && self.stop_completes_on_fetch_machine_cycle(bus_operation) {
            self.request_stop_div_reset();
            self.finish_instruction();
            return;
        }

        match self.decode_fetched_opcode(opcode) {
            DecodedOpcode::Complete => self.finish_instruction(),
            DecodedOpcode::Execute(kind) => self.begin_instruction(opcode, kind),
            DecodedOpcode::Unsupported => self.enter_invalid_opcode_trap(opcode),
        }
    }

    fn stop_completes_on_fetch_machine_cycle(
        &mut self,
        bus_operation: &mut CpuBusCallback<'_>,
    ) -> bool {
        if !self.stop_wake_line_asserted(bus_operation) {
            return false;
        }

        if self.ime {
            return true;
        }

        self.current_highest_pending_interrupt(bus_operation)
            .is_some()
    }

    fn begin_instruction(&mut self, opcode: u8, kind: CpuInstructionKind) {
        self.instruction_kind = Some(kind);
        self.cb_instruction_kind = None;
        self.execution_state = CpuExecutionState::Execute {
            opcode,
            step: 0,
            t_cycle: 0,
        };
    }

    pub(super) fn request_stop_div_reset(&mut self) {
        self.stop_div_reset_requested = true;
    }

    pub(super) fn advance_instruction(&mut self, opcode: u8, next_step: u8) {
        self.execution_state = CpuExecutionState::Execute {
            opcode,
            step: next_step,
            t_cycle: 0,
        };
    }

    pub(super) fn stall_instruction(&mut self, opcode: u8, step: u8) {
        self.execution_state = CpuExecutionState::Execute {
            opcode,
            step,
            t_cycle: LAST_MACHINE_CYCLE_T,
        };
    }

    pub(super) fn finish_instruction(&mut self) {
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.advance_delayed_ime_enable();
        self.execution_state = CpuExecutionState::fetch_opcode();
    }

    pub(super) fn enter_stopped_state(&mut self) {
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.advance_delayed_ime_enable();
        self.execution_state = CpuExecutionState::Stopped;
    }

    pub(super) fn enter_zombie_stopped_state(&mut self) {
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.advance_delayed_ime_enable();
        self.execution_state = CpuExecutionState::ZombieStopped;
    }

    fn enter_invalid_opcode_trap(&mut self, opcode: u8) {
        let address = self
            .last_address_event
            .and_then(|event| event.access_address)
            .unwrap_or_else(|| self.registers.pc.wrapping_sub(1));
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.execution_state = CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::InvalidOpcode { opcode, address },
        };
    }
}
