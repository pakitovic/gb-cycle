use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, DerivedEdge, InterruptSource};

const TIMER_ENABLE_MASK: u8 = 0x04;
const TIMER_CONTROL_MASK: u8 = 0x07;
const TIMER_RELOAD_DELAY_T_CYCLES: u8 = 4;

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
    previous_timer_signal: bool,
    overflow_state: TimerOverflowState,
    reloaded_this_t_cycle: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

    pub(crate) fn div_apu_source_high(&self) -> bool {
        self.current_div_apu_signal()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimerOverflowState {
    Idle,
    Pending { ticks_until_reload: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
mod tests {
    use super::*;
    use crate::scheduler::TCycle;

    #[test]
    fn div_is_derived_from_the_internal_counter_and_reset_by_any_write() {
        let mut timer = Timer::new(ConsoleModel::Dmg);

        timer.system_counter = 0xABCD;

        assert_eq!(timer.read_div(), 0xAB);

        let effects = timer.write_div_with_effects(0xFF);

        assert_eq!(timer.read_div(), 0x00);
        assert_eq!(timer.system_counter, 0);
        assert!(!effects.apu_frame_sequencer_edge);
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

    fn tick_timer(timer: &mut Timer, t_cycle: u64) -> CycleContext {
        let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
        timer.tick_t_cycle(&mut context);
        context
    }

    #[test]
    fn bit_3_timer_frequency_increments_tima_on_falling_edges() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.write_tac(0x05);

        for t_cycle in 0..15 {
            tick_timer(&mut timer, t_cycle);
        }

        assert_eq!(timer.read_tima(), 0x00);

        tick_timer(&mut timer, 15);
        assert_eq!(timer.read_tima(), 0x01);

        for t_cycle in 16..31 {
            tick_timer(&mut timer, t_cycle);
        }

        assert_eq!(timer.read_tima(), 0x01);

        tick_timer(&mut timer, 31);
        assert_eq!(timer.read_tima(), 0x02);
    }

    #[test]
    fn div_write_falling_edge_glitch_can_increment_tima() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.apply_startup_state(TimerStartupState {
            system_counter: 0x0008,
            tima: 0x0F,
            tma: 0x00,
            tac: 0x05,
        });

        let effects = timer.write_div_with_effects(0x00);

        assert_eq!(timer.read_tima(), 0x10);
        assert_eq!(timer.read_div(), 0x00);
        assert!(!effects.apu_frame_sequencer_edge);
    }

    #[test]
    fn div_write_reports_when_the_apu_frame_sequencer_edge_occurs() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.apply_startup_state(TimerStartupState {
            system_counter: 0x1000,
            tima: 0x00,
            tma: 0x00,
            tac: 0x00,
        });

        let effects = timer.write_div_with_effects(0x00);

        assert!(effects.apu_frame_sequencer_edge);
        assert_eq!(timer.snapshot().system_counter, 0x0000);
    }

    #[test]
    fn tac_write_falling_edge_glitch_can_increment_tima() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.apply_startup_state(TimerStartupState {
            system_counter: 0x0020,
            tima: 0x2A,
            tma: 0x00,
            tac: 0x06,
        });

        timer.write_tac(0x04);

        assert_eq!(timer.read_tima(), 0x2B);
    }

    #[test]
    fn overflow_reloads_tima_and_requests_timer_interrupt_four_t_cycles_later() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.write_tima(0xFF);
        timer.write_tma(0x77);
        timer.write_tac(0x05);

        for t_cycle in 0..15 {
            let context = tick_timer(&mut timer, t_cycle);
            assert!(context.interrupt_requests().is_empty());
        }

        let overflow_cycle = tick_timer(&mut timer, 15);
        assert!(overflow_cycle.interrupt_requests().is_empty());
        assert_eq!(timer.read_tima(), 0x00);

        for t_cycle in 16..19 {
            let context = tick_timer(&mut timer, t_cycle);
            assert!(context.interrupt_requests().is_empty());
            assert_eq!(timer.read_tima(), 0x00);
        }

        let reload_cycle = tick_timer(&mut timer, 19);
        assert_eq!(timer.read_tima(), 0x77);
        assert_eq!(reload_cycle.interrupt_requests(), &[InterruptSource::Timer]);
    }

    #[test]
    fn tac_glitch_overflow_reloads_without_slipping_an_extra_t_cycle() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.apply_startup_state(TimerStartupState {
            system_counter: 0x0200,
            tima: 0xFF,
            tma: 0x77,
            tac: 0x04,
        });

        timer.write_tac(0x00);
        assert_eq!(timer.read_tima(), 0x00);

        for t_cycle in 0..2 {
            let context = tick_timer(&mut timer, t_cycle);
            assert!(context.interrupt_requests().is_empty());
            assert_eq!(timer.read_tima(), 0x00);
        }

        let reload_cycle = tick_timer(&mut timer, 2);
        assert_eq!(timer.read_tima(), 0x77);
        assert_eq!(reload_cycle.interrupt_requests(), &[InterruptSource::Timer]);
    }

    #[test]
    fn tima_write_during_pending_reload_cancels_the_reload_and_irq_request() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.write_tima(0xFF);
        timer.write_tma(0x99);
        timer.write_tac(0x05);

        for t_cycle in 0..16 {
            tick_timer(&mut timer, t_cycle);
        }

        assert_eq!(timer.read_tima(), 0x00);
        timer.write_tima(0x44);

        for t_cycle in 16..20 {
            let context = tick_timer(&mut timer, t_cycle);
            assert!(context.interrupt_requests().is_empty());
        }

        assert_eq!(timer.read_tima(), 0x44);
    }

    #[test]
    fn tma_write_before_reload_changes_the_value_copied_into_tima() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.write_tima(0xFF);
        timer.write_tma(0x12);
        timer.write_tac(0x05);

        for t_cycle in 0..16 {
            tick_timer(&mut timer, t_cycle);
        }

        timer.write_tma(0x34);

        for t_cycle in 16..19 {
            tick_timer(&mut timer, t_cycle);
        }

        let context = tick_timer(&mut timer, 19);

        assert_eq!(timer.read_tima(), 0x34);
        assert_eq!(context.interrupt_requests(), &[InterruptSource::Timer]);
    }

    #[test]
    fn tima_write_on_the_reload_cycle_is_ignored() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.write_tima(0xFF);
        timer.write_tma(0x55);
        timer.write_tac(0x05);

        for t_cycle in 0..20 {
            tick_timer(&mut timer, t_cycle);
        }

        assert_eq!(timer.read_tima(), 0x55);

        timer.write_tima(0x77);

        assert_eq!(timer.read_tima(), 0x55);
    }

    #[test]
    fn tma_write_on_the_reload_cycle_updates_the_reloaded_tima_value() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.write_tima(0xFF);
        timer.write_tma(0x12);
        timer.write_tac(0x05);

        for t_cycle in 0..20 {
            tick_timer(&mut timer, t_cycle);
        }

        timer.write_tma(0x34);

        assert_eq!(timer.read_tma(), 0x34);
        assert_eq!(timer.read_tima(), 0x34);
    }

    #[test]
    fn shared_divider_tick_publishes_timer_and_apu_edges_into_the_cycle_context() {
        let mut timer = Timer::new(ConsoleModel::Dmg);
        timer.apply_startup_state(TimerStartupState {
            system_counter: 0x1FFF,
            tima: 0x0F,
            tma: 0x00,
            tac: 0x05,
        });

        let context = tick_timer(&mut timer, 0);

        assert_eq!(
            context.derived_edges(),
            &[
                DerivedEdge::DividerTick,
                DerivedEdge::TimerInputFallingEdge,
                DerivedEdge::ApuFrameSequencerEdge,
            ]
        );
        assert_eq!(timer.snapshot().system_counter, 0x2000);
        assert_eq!(timer.read_tima(), 0x10);
    }
}
