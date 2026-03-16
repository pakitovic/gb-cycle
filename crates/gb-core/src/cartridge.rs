use crate::scheduler::CycleContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeSlotState {
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeSlot {
    state: CartridgeSlotState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeSnapshot {
    pub state: CartridgeSlotState,
}

impl CartridgeSlot {
    pub fn empty() -> Self {
        Self {
            state: CartridgeSlotState::Empty,
        }
    }

    pub fn state(&self) -> CartridgeSlotState {
        self.state
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.state, CartridgeSlotState::Empty)
    }

    pub fn snapshot(&self) -> CartridgeSnapshot {
        CartridgeSnapshot { state: self.state }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} state={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.state,
        )
    }
}
