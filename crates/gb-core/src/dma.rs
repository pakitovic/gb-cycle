use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaStatus {
    Stub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaController {
    console_model: ConsoleModel,
    status: DmaStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaSnapshot {
    pub console_model: ConsoleModel,
    pub status: DmaStatus,
}

impl DmaController {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: DmaStatus::Stub,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> DmaStatus {
        self.status
    }

    pub fn snapshot(&self) -> DmaSnapshot {
        DmaSnapshot {
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
