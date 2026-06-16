use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, InterruptSource};

const INTERRUPT_REQUEST_MASK: u8 = 0x1F;
const INTERRUPT_FLAG_FORCED_HIGH_BITS: u8 = 0xE0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InterruptControllerStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InterruptStartupState {
    pub interrupt_flags: u8,
    pub interrupt_enable: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InterruptController {
    console_model: ConsoleModel,
    status: InterruptControllerStatus,
    interrupt_flags: u8,
    interrupt_enable: u8,
    #[serde(default)]
    cpu_if_read_suppress_mask: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InterruptSaveState {
    console_model: ConsoleModel,
    status: InterruptControllerStatus,
    interrupt_flags: u8,
    interrupt_enable: u8,
    #[serde(default)]
    cpu_if_read_suppress_mask: u8,
}

impl InterruptSaveState {
    pub(crate) const fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InterruptControllerSnapshot {
    pub console_model: ConsoleModel,
    pub status: InterruptControllerStatus,
    pub interrupt_flags: u8,
    pub interrupt_enable: u8,
}

impl InterruptSource {
    pub const fn mask(self) -> u8 {
        match self {
            Self::VBlank => 0x01,
            Self::LcdStat => 0x02,
            Self::Timer => 0x04,
            Self::Serial => 0x08,
            Self::Joypad => 0x10,
        }
    }

    pub const fn vector(self) -> u16 {
        match self {
            Self::VBlank => 0x0040,
            Self::LcdStat => 0x0048,
            Self::Timer => 0x0050,
            Self::Serial => 0x0058,
            Self::Joypad => 0x0060,
        }
    }

    pub const fn highest_priority_from_mask(mask: u8) -> Option<Self> {
        if mask & Self::VBlank.mask() != 0 {
            Some(Self::VBlank)
        } else if mask & Self::LcdStat.mask() != 0 {
            Some(Self::LcdStat)
        } else if mask & Self::Timer.mask() != 0 {
            Some(Self::Timer)
        } else if mask & Self::Serial.mask() != 0 {
            Some(Self::Serial)
        } else if mask & Self::Joypad.mask() != 0 {
            Some(Self::Joypad)
        } else {
            None
        }
    }
}

impl InterruptController {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: InterruptControllerStatus::Ready,
            interrupt_flags: 0,
            interrupt_enable: 0,
            cpu_if_read_suppress_mask: 0,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> InterruptControllerStatus {
        self.status
    }

    pub(crate) fn capture_save_state(&self) -> InterruptSaveState {
        InterruptSaveState {
            console_model: self.console_model,
            status: self.status,
            interrupt_flags: self.interrupt_flags,
            interrupt_enable: self.interrupt_enable,
            cpu_if_read_suppress_mask: self.cpu_if_read_suppress_mask,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &InterruptSaveState) {
        self.console_model = state.console_model;
        self.status = state.status;
        self.interrupt_flags = state.interrupt_flags;
        self.interrupt_enable = state.interrupt_enable;
        self.cpu_if_read_suppress_mask = state.cpu_if_read_suppress_mask;
    }

    pub fn read_if(&self) -> u8 {
        INTERRUPT_FLAG_FORCED_HIGH_BITS | self.interrupt_flags
    }

    // L2-a.1: under the CPU-first reorder a PPU IRQ edge committed in this cycle's
    // InterruptAggregation is read by the next cycle's pre-tick CPU `ldh a,(IF)` one
    // read-position too early (hardware/main observes it post-tick, in the same cycle as
    // the commit, i.e. after the read). For the VBlank-entry edge (VBlank + its co-committed
    // STAT) this mask hides the freshly committed bits from the CPU IF *read* for exactly
    // one cycle, leaving the committed scheduler IF — and therefore dispatch/service, which
    // read `pending_mask`/`highest_pending`, not this path — untouched.
    pub(crate) fn read_if_with_pending_requests(&self, pending_mask: u8) -> u8 {
        let visible = (self.interrupt_flags | (pending_mask & INTERRUPT_REQUEST_MASK))
            & !self.cpu_if_read_suppress_mask;
        INTERRUPT_FLAG_FORCED_HIGH_BITS | visible
    }

    pub(crate) fn arm_cpu_if_read_suppress(&mut self, mask: u8) {
        self.cpu_if_read_suppress_mask = mask & INTERRUPT_REQUEST_MASK;
    }

    pub(crate) fn clear_cpu_if_read_suppress(&mut self) {
        self.cpu_if_read_suppress_mask = 0;
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
        self.interrupt_flags |= source.mask();
    }

    pub fn clear(&mut self, source: InterruptSource) {
        self.interrupt_flags &= !source.mask();
    }

    pub fn pending_mask(&self) -> u8 {
        self.interrupt_enable & self.interrupt_flags
    }

    pub fn highest_pending(&self) -> Option<InterruptSource> {
        InterruptSource::highest_priority_from_mask(self.pending_mask())
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

#[cfg(test)]
mod tests;
