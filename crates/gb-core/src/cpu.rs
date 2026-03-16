use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuStatus {
    Stub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CpuStartupState {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuCore {
    console_model: ConsoleModel,
    status: CpuStatus,
    startup_state: CpuStartupState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuSnapshot {
    pub console_model: ConsoleModel,
    pub status: CpuStatus,
    pub startup_state: CpuStartupState,
}

impl CpuCore {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: CpuStatus::Stub,
            startup_state: CpuStartupState::power_on_reset(),
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

    pub fn apply_startup_state(&mut self, startup_state: CpuStartupState) {
        self.startup_state = startup_state;
    }

    pub fn snapshot(&self) -> CpuSnapshot {
        CpuSnapshot {
            console_model: self.console_model,
            status: self.status,
            startup_state: self.startup_state,
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
        )
    }
}

impl CpuStartupState {
    pub const fn power_on_reset() -> Self {
        Self {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            sp: 0,
            pc: 0x0000,
        }
    }
}
