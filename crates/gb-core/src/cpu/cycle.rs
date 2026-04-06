use super::decode::{CpuInstructionKind, DecodedOpcode};
use super::*;

impl CpuCore {
    pub(crate) fn tick_t_cycle<F>(&mut self, mut bus_operation: F) -> Option<InterruptSource>
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.last_bus_activity = None;
        self.last_address_event = None;

        match self.execution_state {
            CpuExecutionState::FetchOpcode { t_cycle } => {
                if t_cycle < LAST_MACHINE_CYCLE_T {
                    self.execution_state = CpuExecutionState::FetchOpcode {
                        t_cycle: t_cycle + 1,
                    };
                    return None;
                }

                self.complete_fetch_opcode(&mut bus_operation);
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
                    return None;
                }

                self.complete_execute_machine_cycle(opcode, step, &mut bus_operation);
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
                    return None;
                }

                return self.complete_interrupt_service_machine_cycle(
                    source,
                    step,
                    &mut bus_operation,
                );
            }
            CpuExecutionState::DiagnosticTrap { .. }
            | CpuExecutionState::Halted
            | CpuExecutionState::Stopped => {}
        }

        None
    }

    fn complete_fetch_opcode<F>(&mut self, bus_operation: &mut F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let opcode = self.read_opcode_u8(bus_operation);
        self.current_opcode = Some(opcode);

        match self.decode_fetched_opcode(opcode) {
            DecodedOpcode::Complete => self.finish_instruction(),
            DecodedOpcode::Execute(kind) => self.begin_instruction(opcode, kind),
            DecodedOpcode::Unsupported => self.enter_unsupported_opcode_trap(opcode),
        }
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

    fn enter_unsupported_opcode_trap(&mut self, opcode: u8) {
        let address = self
            .last_address_event
            .and_then(|event| event.access_address)
            .unwrap_or_else(|| self.registers.pc.wrapping_sub(1));
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.execution_state = CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::UnsupportedOpcode { opcode, address },
        };
    }
}
