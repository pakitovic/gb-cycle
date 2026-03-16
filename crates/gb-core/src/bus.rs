use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusStatus {
    Stub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bus {
    console_model: ConsoleModel,
    status: BusStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusSnapshot {
    pub console_model: ConsoleModel,
    pub status: BusStatus,
}

impl Bus {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: BusStatus::Stub,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> BusStatus {
        self.status
    }

    pub fn snapshot(&self) -> BusSnapshot {
        BusSnapshot {
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
