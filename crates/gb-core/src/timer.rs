use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, DerivedEdge, InterruptSource};

const TIMER_ENABLE_MASK: u8 = 0x04;
const TIMER_CONTROL_MASK: u8 = 0x07;
const TIMER_RELOAD_DELAY_T_CYCLES: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TimerStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TimerStartupState {
    pub system_counter: u16,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Timer {
    console_model: ConsoleModel,
    status: TimerStatus,
    system_counter: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    previous_timer_signal: bool,
    overflow_state: TimerOverflowState,
    reloaded_this_t_cycle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimerSaveState {
    console_model: ConsoleModel,
    status: TimerStatus,
    system_counter: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    previous_timer_signal: bool,
    overflow_state: TimerOverflowState,
    reloaded_this_t_cycle: bool,
}

impl TimerSaveState {
    pub(crate) const fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimerSnapshot {
    pub console_model: ConsoleModel,
    pub status: TimerStatus,
    pub system_counter: u16,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct DividerResetEffects {
    pub apu_frame_sequencer_edge: bool,
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
            previous_timer_signal: false,
            overflow_state: TimerOverflowState::Idle,
            reloaded_this_t_cycle: false,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> TimerStatus {
        self.status
    }

    pub(crate) fn capture_save_state(&self) -> TimerSaveState {
        TimerSaveState {
            console_model: self.console_model,
            status: self.status,
            system_counter: self.system_counter,
            tima: self.tima,
            tma: self.tma,
            tac: self.tac,
            previous_timer_signal: self.previous_timer_signal,
            overflow_state: self.overflow_state,
            reloaded_this_t_cycle: self.reloaded_this_t_cycle,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &TimerSaveState) {
        self.console_model = state.console_model;
        self.status = state.status;
        self.system_counter = state.system_counter;
        self.tima = state.tima;
        self.tma = state.tma;
        self.tac = state.tac;
        self.previous_timer_signal = state.previous_timer_signal;
        self.overflow_state = state.overflow_state;
        self.reloaded_this_t_cycle = state.reloaded_this_t_cycle;
    }

    pub fn read_div(&self) -> u8 {
        (self.system_counter >> 8) as u8
    }

    pub fn write_div(&mut self, value: u8) {
        let _ = self.write_div_with_effects(value);
    }

    pub(crate) fn write_div_with_effects(&mut self, _value: u8) -> DividerResetEffects {
        let previous_signal = self.current_timer_signal();
        let previous_div_apu_signal = self.current_div_apu_signal();
        self.system_counter = 0;
        self.apply_timer_signal_change(previous_signal, TimerSignalChangeOrigin::MmioWrite);
        DividerResetEffects {
            apu_frame_sequencer_edge: previous_div_apu_signal && !self.current_div_apu_signal(),
        }
    }

    pub fn read_tima(&self) -> u8 {
        self.tima
    }

    pub fn write_tima(&mut self, value: u8) {
        if self.reloaded_this_t_cycle {
            return;
        }

        self.tima = value;
        if matches!(self.overflow_state, TimerOverflowState::Pending { .. }) {
            self.overflow_state = TimerOverflowState::Idle;
        }
    }

    pub fn read_tma(&self) -> u8 {
        self.tma
    }

    pub fn write_tma(&mut self, value: u8) {
        self.tma = value;
        if self.reloaded_this_t_cycle {
            self.tima = value;
        }
    }

    pub fn read_tac(&self) -> u8 {
        0xF8 | self.tac
    }

    pub fn write_tac(&mut self, value: u8) {
        let previous_signal = self.current_timer_signal();
        self.tac = value & TIMER_CONTROL_MASK;
        self.apply_timer_signal_change(previous_signal, TimerSignalChangeOrigin::MmioWrite);
    }

    pub fn apply_startup_state(&mut self, startup_state: TimerStartupState) {
        self.system_counter = startup_state.system_counter;
        self.tima = startup_state.tima;
        self.tma = startup_state.tma;
        self.tac = startup_state.tac & TIMER_CONTROL_MASK;
        self.previous_timer_signal = self.current_timer_signal();
        self.overflow_state = TimerOverflowState::Idle;
        self.reloaded_this_t_cycle = false;
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

    pub(crate) fn tick_t_cycle(&mut self, context: &mut CycleContext) {
        self.reloaded_this_t_cycle = false;
        self.advance_overflow_pipeline(context);

        let previous_timer_signal = self.current_timer_signal();
        let previous_div_apu_signal = self.current_div_apu_signal();
        self.system_counter = self.system_counter.wrapping_add(1);
        context.push_derived_edge(DerivedEdge::DividerTick);
        let current_signal = self.current_timer_signal();
        self.process_timer_signal_edge(
            previous_timer_signal,
            current_signal,
            TimerSignalChangeOrigin::AutonomousTick,
        );
        self.previous_timer_signal = current_signal;

        if previous_timer_signal && !current_signal {
            context.push_derived_edge(DerivedEdge::TimerInputFallingEdge);
        }

        if previous_div_apu_signal && !self.current_div_apu_signal() {
            context.push_derived_edge(DerivedEdge::ApuFrameSequencerEdge);
        }
    }

    fn advance_overflow_pipeline(&mut self, context: &mut CycleContext) {
        match self.overflow_state {
            TimerOverflowState::Idle => {}
            TimerOverflowState::Pending { ticks_until_reload } if ticks_until_reload > 1 => {
                self.overflow_state = TimerOverflowState::Pending {
                    ticks_until_reload: ticks_until_reload - 1,
                };
            }
            TimerOverflowState::Pending { .. } => {
                self.tima = self.tma;
                self.overflow_state = TimerOverflowState::Idle;
                self.reloaded_this_t_cycle = true;
                context.queue_interrupt_request(InterruptSource::Timer);
            }
        }
    }

    fn apply_timer_signal_change(
        &mut self,
        previous_signal: bool,
        origin: TimerSignalChangeOrigin,
    ) {
        let current_signal = self.current_timer_signal();
        self.process_timer_signal_edge(previous_signal, current_signal, origin);
        self.previous_timer_signal = current_signal;
    }

    fn process_timer_signal_edge(
        &mut self,
        previous_signal: bool,
        current_signal: bool,
        origin: TimerSignalChangeOrigin,
    ) {
        if previous_signal && !current_signal {
            self.increment_tima_on_falling_edge(origin);
        }
    }

    fn increment_tima_on_falling_edge(&mut self, origin: TimerSignalChangeOrigin) {
        if matches!(self.overflow_state, TimerOverflowState::Pending { .. }) {
            return;
        }

        let (result, overflowed) = self.tima.overflowing_add(1);
        self.tima = result;

        if overflowed {
            self.overflow_state = TimerOverflowState::Pending {
                ticks_until_reload: match origin {
                    TimerSignalChangeOrigin::AutonomousTick => TIMER_RELOAD_DELAY_T_CYCLES,
                    // MMIO writes happen after the timer's autonomous phase for the current
                    // shared T-cycle, so a glitch-driven overflow must not lose an extra full
                    // cycle before entering the documented reload / IRQ window.
                    TimerSignalChangeOrigin::MmioWrite => TIMER_RELOAD_DELAY_T_CYCLES - 1,
                },
            };
        }
    }

    fn current_timer_signal(&self) -> bool {
        self.tac & TIMER_ENABLE_MASK != 0 && self.selected_counter_bit_is_high()
    }

    fn selected_counter_bit_is_high(&self) -> bool {
        let bit = selected_timer_bit(self.tac);
        self.system_counter & (1 << bit) != 0
    }

    fn current_div_apu_signal(&self) -> bool {
        self.system_counter & (1 << 12) != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum TimerOverflowState {
    Idle,
    Pending { ticks_until_reload: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum TimerSignalChangeOrigin {
    AutonomousTick,
    MmioWrite,
}

const fn selected_timer_bit(tac: u8) -> u8 {
    match tac & 0x03 {
        0x00 => 9,
        0x01 => 3,
        0x02 => 5,
        _ => 7,
    }
}

#[cfg(test)]
mod tests;
