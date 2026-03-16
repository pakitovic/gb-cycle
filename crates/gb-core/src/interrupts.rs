use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, InterruptSource};

const INTERRUPT_REQUEST_MASK: u8 = 0x1F;
const INTERRUPT_FLAG_FORCED_HIGH_BITS: u8 = 0xE0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptControllerStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InterruptStartupState {
    pub interrupt_flags: u8,
    pub interrupt_enable: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptController {
    console_model: ConsoleModel,
    status: InterruptControllerStatus,
    interrupt_flags: u8,
    interrupt_enable: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptControllerSnapshot {
    pub console_model: ConsoleModel,
    pub status: InterruptControllerStatus,
    pub interrupt_flags: u8,
    pub interrupt_enable: u8,
}

impl InterruptController {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: InterruptControllerStatus::Ready,
            interrupt_flags: 0,
            interrupt_enable: 0,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> InterruptControllerStatus {
        self.status
    }

    pub fn read_if(&self) -> u8 {
        INTERRUPT_FLAG_FORCED_HIGH_BITS | self.interrupt_flags
    }

    pub fn write_if(&mut self, value: u8) {
        self.interrupt_flags = value & INTERRUPT_REQUEST_MASK;
    }

    pub fn read_ie(&self) -> u8 {
        self.interrupt_enable
    }

    pub fn write_ie(&mut self, value: u8) {
        self.interrupt_enable = value;
    }

    pub fn request(&mut self, source: InterruptSource) {
        self.interrupt_flags |= interrupt_bit(source);
    }

    pub fn clear(&mut self, source: InterruptSource) {
        self.interrupt_flags &= !interrupt_bit(source);
    }

    pub fn pending_mask(&self) -> u8 {
        self.interrupt_enable & self.interrupt_flags
    }

    pub fn highest_pending(&self) -> Option<InterruptSource> {
        const PRIORITY_ORDER: [InterruptSource; 5] = [
            InterruptSource::VBlank,
            InterruptSource::LcdStat,
            InterruptSource::Timer,
            InterruptSource::Serial,
            InterruptSource::Joypad,
        ];

        PRIORITY_ORDER
            .into_iter()
            .find(|source| self.pending_mask() & interrupt_bit(*source) != 0)
    }

    pub fn apply_startup_state(&mut self, startup_state: InterruptStartupState) {
        self.interrupt_flags = startup_state.interrupt_flags & INTERRUPT_REQUEST_MASK;
        self.interrupt_enable = startup_state.interrupt_enable;
    }

    pub fn snapshot(&self) -> InterruptControllerSnapshot {
        InterruptControllerSnapshot {
            console_model: self.console_model,
            status: self.status,
            interrupt_flags: self.interrupt_flags,
            interrupt_enable: self.interrupt_enable,
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} if={:#04X} ie={:#04X}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.read_if(),
            self.read_ie(),
        )
    }
}

const fn interrupt_bit(source: InterruptSource) -> u8 {
    match source {
        InterruptSource::VBlank => 0x01,
        InterruptSource::LcdStat => 0x02,
        InterruptSource::Timer => 0x04,
        InterruptSource::Serial => 0x08,
        InterruptSource::Joypad => 0x10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn if_forces_unused_upper_bits_high() {
        let mut interrupts = InterruptController::new(ConsoleModel::Dmg);

        interrupts.write_if(0x04);

        assert_eq!(interrupts.read_if(), 0xE4);
    }

    #[test]
    fn request_and_pending_selection_follow_dmg_priority() {
        let mut interrupts = InterruptController::new(ConsoleModel::Dmg);

        interrupts.write_ie(0x1F);
        interrupts.request(InterruptSource::Joypad);
        interrupts.request(InterruptSource::Timer);
        interrupts.request(InterruptSource::VBlank);

        assert_eq!(interrupts.pending_mask(), 0x15);
        assert_eq!(interrupts.highest_pending(), Some(InterruptSource::VBlank));
    }

    #[test]
    fn startup_state_keeps_if_upper_bits_forced_high_on_readback() {
        let mut interrupts = InterruptController::new(ConsoleModel::Dmg);

        interrupts.apply_startup_state(InterruptStartupState {
            interrupt_flags: 0x01,
            interrupt_enable: 0x00,
        });

        assert_eq!(interrupts.read_if(), 0xE1);
        assert_eq!(interrupts.read_ie(), 0x00);
    }
}
