use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaTransferState {
    Idle,
    OamStartRequested { source_page: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DmaStartupState {
    pub source_page_latch: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaController {
    console_model: ConsoleModel,
    status: DmaStatus,
    source_page_latch: u8,
    transfer_state: DmaTransferState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaSnapshot {
    pub console_model: ConsoleModel,
    pub status: DmaStatus,
    pub source_page_latch: u8,
    pub transfer_state: DmaTransferState,
}

impl DmaController {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: DmaStatus::Ready,
            source_page_latch: 0,
            transfer_state: DmaTransferState::Idle,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> DmaStatus {
        self.status
    }

    pub fn source_page_latch(&self) -> u8 {
        self.source_page_latch
    }

    pub fn transfer_state(&self) -> DmaTransferState {
        self.transfer_state
    }

    pub fn read_ff46(&self) -> u8 {
        self.source_page_latch
    }

    pub fn write_ff46(&mut self, value: u8) {
        self.source_page_latch = value;
        self.transfer_state = DmaTransferState::OamStartRequested { source_page: value };
    }

    pub fn apply_startup_state(&mut self, startup_state: DmaStartupState) {
        self.source_page_latch = startup_state.source_page_latch;
        self.transfer_state = DmaTransferState::Idle;
    }

    pub fn snapshot(&self) -> DmaSnapshot {
        DmaSnapshot {
            console_model: self.console_model,
            status: self.status,
            source_page_latch: self.source_page_latch,
            transfer_state: self.transfer_state,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ff46_latches_the_source_page_and_requests_oam_dma_immediately() {
        let mut dma = DmaController::new(ConsoleModel::Dmg);

        dma.write_ff46(0x12);

        assert_eq!(dma.read_ff46(), 0x12);
        assert_eq!(
            dma.transfer_state(),
            DmaTransferState::OamStartRequested { source_page: 0x12 }
        );
    }

    #[test]
    fn startup_state_preserves_idle_dma_while_setting_visible_ff46() {
        let mut dma = DmaController::new(ConsoleModel::Dmg);

        dma.apply_startup_state(DmaStartupState {
            source_page_latch: 0xFF,
        });

        assert_eq!(dma.read_ff46(), 0xFF);
        assert_eq!(dma.transfer_state(), DmaTransferState::Idle);
    }
}
