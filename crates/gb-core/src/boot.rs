use crate::model::{ConsoleModel, StartupMode};
use crate::scheduler::CycleContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStatus {
    Stub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootController {
    console_model: ConsoleModel,
    startup_mode: StartupMode,
    status: BootStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootSnapshot {
    pub console_model: ConsoleModel,
    pub startup_mode: StartupMode,
    pub status: BootStatus,
}

impl BootController {
    pub fn new(console_model: ConsoleModel, startup_mode: StartupMode) -> Self {
        Self {
            console_model,
            startup_mode,
            status: BootStatus::Stub,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn startup_mode(&self) -> StartupMode {
        self.startup_mode
    }

    pub fn status(&self) -> BootStatus {
        self.status
    }

    pub fn snapshot(&self) -> BootSnapshot {
        BootSnapshot {
            console_model: self.console_model,
            startup_mode: self.startup_mode,
            status: self.status,
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} startup_mode={:?} status={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.startup_mode,
            self.status,
        )
    }
}
