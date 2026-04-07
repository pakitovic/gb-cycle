use crate::scheduler::CycleContext;

use super::*;

impl CpuCore {
    pub fn new(console_model: ConsoleModel) -> Self {
        let startup_state = CpuStartupState::power_on_reset();

        Self {
            console_model,
            status: CpuStatus::Ready,
            startup_state,
            registers: CpuRegisters::from_startup_state(startup_state),
            execution_state: CpuExecutionState::fetch_opcode(),
            current_opcode: None,
            ime: false,
            delayed_ime_enable: false,
            delayed_ime_enable_steps: 0,
            halt_request_pending: false,
            halt_request_ime: false,
            halt_request_had_delayed_ei: false,
            halt_bug_pending: false,
            instruction_kind: None,
            cb_instruction_kind: None,
            operand8_latch: 0,
            operand16_latch: 0,
            last_bus_activity: None,
            last_address_event: None,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> CpuStatus {
        self.status
    }

    pub fn startup_state(&self) -> CpuStartupState {
        self.startup_state
    }

    pub fn registers(&self) -> CpuRegisters {
        self.registers
    }

    pub fn execution_state(&self) -> CpuExecutionState {
        self.execution_state
    }

    pub fn current_opcode(&self) -> Option<u8> {
        self.current_opcode
    }

    pub fn ime(&self) -> bool {
        self.ime
    }

    pub fn delayed_ime_enable(&self) -> bool {
        self.delayed_ime_enable
    }

    pub fn last_address_event(&self) -> Option<CpuAddressEvent> {
        self.last_address_event
    }

    pub fn apply_startup_state(&mut self, startup_state: CpuStartupState) {
        self.startup_state = startup_state;
        self.registers = CpuRegisters::from_startup_state(startup_state);
        self.execution_state = CpuExecutionState::fetch_opcode();
        self.current_opcode = None;
        self.ime = false;
        self.delayed_ime_enable = false;
        self.delayed_ime_enable_steps = 0;
        self.halt_request_pending = false;
        self.halt_request_ime = false;
        self.halt_request_had_delayed_ei = false;
        self.halt_bug_pending = false;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.last_bus_activity = None;
        self.last_address_event = None;
    }

    pub fn snapshot(&self) -> CpuSnapshot {
        CpuSnapshot {
            console_model: self.console_model,
            status: self.status,
            startup_state: self.startup_state,
            registers: self.registers,
            execution_state: self.execution_state,
            current_opcode: self.current_opcode,
            ime: self.ime,
            delayed_ime_enable: self.delayed_ime_enable,
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} pc={:#06X} execution_state={:?} current_opcode={:?} ime={} delayed_ime_enable={} last_bus_activity={} last_address_event={}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.registers.pc,
            self.execution_state,
            self.current_opcode,
            self.ime,
            self.delayed_ime_enable,
            self.last_bus_activity_trace_value(),
            self.last_address_event_trace_value(),
        )
    }
}
