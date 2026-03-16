use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerStartupState {
    pub system_counter: u16,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timer {
    console_model: ConsoleModel,
    status: TimerStatus,
    system_counter: u16,
    tima: u8,
    tma: u8,
    tac: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerSnapshot {
    pub console_model: ConsoleModel,
    pub status: TimerStatus,
    pub system_counter: u16,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
}

impl Timer {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: TimerStatus::Ready,
            system_counter: 0,
            tima: 0,
            tma: 0,
            tac: 0,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> TimerStatus {
        self.status
    }

    pub fn read_div(&self) -> u8 {
        (self.system_counter >> 8) as u8
    }

    pub fn write_div(&mut self, _value: u8) {
        self.system_counter = 0;
    }

    pub fn read_tima(&self) -> u8 {
        self.tima
    }

    pub fn write_tima(&mut self, value: u8) {
        self.tima = value;
    }

    pub fn read_tma(&self) -> u8 {
        self.tma
    }

    pub fn write_tma(&mut self, value: u8) {
        self.tma = value;
    }

    pub fn read_tac(&self) -> u8 {
        0xF8 | self.tac
    }

    pub fn write_tac(&mut self, value: u8) {
        self.tac = value & 0x07;
    }

    pub fn apply_startup_state(&mut self, startup_state: TimerStartupState) {
        self.system_counter = startup_state.system_counter;
        self.tima = startup_state.tima;
        self.tma = startup_state.tma;
        self.tac = startup_state.tac & 0x07;
    }

    pub fn snapshot(&self) -> TimerSnapshot {
        TimerSnapshot {
            console_model: self.console_model,
            status: self.status,
            system_counter: self.system_counter,
            tima: self.tima,
            tma: self.tma,
            tac: self.tac,
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
    fn div_is_derived_from_the_internal_counter_and_reset_by_any_write() {
        let mut timer = Timer::new(ConsoleModel::Dmg);

        timer.system_counter = 0xABCD;

        assert_eq!(timer.read_div(), 0xAB);

        timer.write_div(0xFF);

        assert_eq!(timer.read_div(), 0x00);
        assert_eq!(timer.system_counter, 0);
    }

    #[test]
    fn tac_forces_unused_bits_high_and_masks_writes_to_control_bits() {
        let mut timer = Timer::new(ConsoleModel::Dmg);

        timer.write_tac(0xFF);

        assert_eq!(timer.read_tac(), 0xFF);
        assert_eq!(timer.tac, 0x07);
    }

    #[test]
    fn startup_state_applies_the_visible_post_boot_timer_snapshot() {
        let mut timer = Timer::new(ConsoleModel::Dmg);

        timer.apply_startup_state(TimerStartupState {
            system_counter: 0xAB00,
            tima: 0x00,
            tma: 0x00,
            tac: 0x00,
        });

        assert_eq!(timer.read_div(), 0xAB);
        assert_eq!(timer.read_tima(), 0x00);
        assert_eq!(timer.read_tma(), 0x00);
        assert_eq!(timer.read_tac(), 0xF8);
    }
}
