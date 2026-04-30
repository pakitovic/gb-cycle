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
            in_flight: InFlightInstruction::default(),
            ime_state: ImeState::Disabled,
            halt_control: HaltControlState::Idle,
            last_bus_activity: None,
            last_address_event: None,
            stop_div_reset_requested: false,
            cgb_speed_switch_requested: false,
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
        self.in_flight.opcode
    }

    pub fn ime(&self) -> bool {
        self.ime_enabled()
    }

    pub fn delayed_ime_enable(&self) -> bool {
        self.ime_state.delayed_enable_pending()
    }

    pub fn last_address_event(&self) -> Option<CpuAddressEvent> {
        self.last_address_event
    }

    pub(crate) fn take_stop_div_reset_request(&mut self) -> bool {
        let requested = self.stop_div_reset_requested;
        self.stop_div_reset_requested = false;
        requested
    }

    pub(crate) fn take_cgb_speed_switch_request(&mut self) -> bool {
        let requested = self.cgb_speed_switch_requested;
        self.cgb_speed_switch_requested = false;
        requested
    }

    pub fn apply_startup_state(&mut self, startup_state: CpuStartupState) {
        self.startup_state = startup_state;
        self.registers = CpuRegisters::from_startup_state(startup_state);
        self.execution_state = CpuExecutionState::fetch_opcode();
        self.ime_state = ImeState::Disabled;
        self.halt_control = HaltControlState::Idle;
        self.clear_in_flight_instruction_state();
        self.last_bus_activity = None;
        self.last_address_event = None;
        self.stop_div_reset_requested = false;
        self.cgb_speed_switch_requested = false;
    }

    pub fn snapshot(&self) -> CpuSnapshot {
        CpuSnapshot {
            console_model: self.console_model,
            status: self.status,
            startup_state: self.startup_state,
            registers: self.registers,
            execution_state: self.execution_state,
            current_opcode: self.current_opcode(),
            ime: self.ime(),
            delayed_ime_enable: self.delayed_ime_enable(),
            last_bus_activity: self
                .last_bus_activity
                .map(|activity| CpuBusActivitySnapshot {
                    kind: match activity.kind {
                        trace::CpuTraceBusAccessKind::OpcodeFetch => CpuBusAccessKind::OpcodeFetch,
                        trace::CpuTraceBusAccessKind::OperandRead => CpuBusAccessKind::OperandRead,
                        trace::CpuTraceBusAccessKind::DataRead => CpuBusAccessKind::DataRead,
                        trace::CpuTraceBusAccessKind::DataWrite => CpuBusAccessKind::DataWrite,
                    },
                    address: activity.address,
                    value: activity.value,
                }),
            last_address_event: self.last_address_event,
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
            self.current_opcode(),
            self.ime(),
            self.delayed_ime_enable(),
            self.last_bus_activity_trace_value(),
            self.last_address_event_trace_value(),
        )
    }
}
