use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

use super::{Bus, BusArbitrationState, BusStatus};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BusSnapshot {
    pub console_model: ConsoleModel,
    pub status: BusStatus,
    pub arbitration: BusArbitrationState,
}

impl Bus {
    pub fn snapshot(&self, arbitration: BusArbitrationState) -> BusSnapshot {
        BusSnapshot {
            console_model: self.console_model,
            status: self.status,
            arbitration,
        }
    }

    pub fn scheduler_trace_message(
        &self,
        context: &CycleContext,
        state: &BusArbitrationState,
    ) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} boot_low_window_mapped={} boot_cgb_upper_window_mapped={} ppu_lcd_enabled={} ppu_mode={:?} dma_cpu_access_policy={:?} dma_active_region={:?} dma_cpu_conflict_source_address={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            state.boot_rom.maps_low_window(),
            state.boot_rom.maps_cgb_upper_window(),
            state.ppu.is_lcd_enabled(),
            state.ppu.mode(),
            state.dma.cpu_access_policy(),
            state.dma.active_region(),
            state.dma.cpu_conflict_source_address(),
        )
    }
}
