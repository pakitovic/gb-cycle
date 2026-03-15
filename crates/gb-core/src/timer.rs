use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerStatus {
    Stub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timer {
    console_model: ConsoleModel,
    status: TimerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerSnapshot {
    pub console_model: ConsoleModel,
    pub status: TimerStatus,
}

impl Timer {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: TimerStatus::Stub,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> TimerStatus {
        self.status
    }

    pub fn snapshot(&self) -> TimerSnapshot {
        TimerSnapshot {
            console_model: self.console_model,
            status: self.status,
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
