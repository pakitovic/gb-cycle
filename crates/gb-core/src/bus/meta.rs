use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

use super::{Bus, BusArbitrationState, BusStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusSnapshot {
    pub console_model: ConsoleModel,
    pub status: BusStatus,
}

impl Bus {
    pub fn snapshot(&self) -> BusSnapshot {
        BusSnapshot {
            console_model: self.console_model,
            status: self.status,
        }
    }

    pub fn scheduler_trace_message(
        &self,
        context: &CycleContext,
        state: &BusArbitrationState,
    ) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} ppu_lcd_enabled={} ppu_mode={:?} dma_cpu_access_policy={:?} dma_active_region={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            state.ppu.is_lcd_enabled(),
            state.ppu.mode(),
            state.dma.cpu_access_policy(),
            state.dma.active_region(),
        )
    }
}
