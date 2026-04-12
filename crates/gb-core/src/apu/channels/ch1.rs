use crate::model::ConsoleModel;

use super::super::common::{
    CHANNEL_TRIGGER_BIT, ChannelRuntimeState, NR10_FORCED_HIGH_MASK, NR11_WRITE_ONLY_MASK,
    NR14_FORCED_HIGH_MASK, NR14_READ_MASK, NRX4_WRITABLE_MASK, PERIOD_HIGH_MASK,
    frame_sequencer_step_clocks_envelope, frame_sequencer_step_clocks_length,
    pulse_period_from_registers, sweep_decreases_from_nr10, sweep_pace_from_nr10,
    sweep_shift_from_nr10,
};
use super::pulse::{PulseChannelState, PulseStartupState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::apu) struct Channel1SweepState {
    pub(in crate::apu) timer: u8,
    pub(in crate::apu) phase: u8,
    pub(in crate::apu) enabled: bool,
    pub(in crate::apu) shadow_period: u16,
    pub(in crate::apu) completed_addend: u16,
    pub(in crate::apu) negate_calculated_since_trigger: bool,
}

impl Channel1SweepState {
    pub(in crate::apu) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(in crate::apu) fn apply_powered_startup(
        &mut self,
        nr10: u8,
        period_value: u16,
        active: bool,
    ) {
        self.clear();
        self.shadow_period = period_value;
        self.phase = Self::phase_from_pace(sweep_pace_from_nr10(nr10));
        self.timer = Self::timer_from_phase(self.phase);
        self.enabled =
            active && (sweep_pace_from_nr10(nr10) != 0 || sweep_shift_from_nr10(nr10) != 0);
    }

    pub(in crate::apu) fn write_nr10(
        &mut self,
        old_nr10: u8,
        new_nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        if sweep_decreases_from_nr10(old_nr10)
            && !sweep_decreases_from_nr10(new_nr10)
            && self.negate_calculated_since_trigger
        {
            runtime.active = false;
        }

        self.maybe_fire_sweep_boundary(new_nr10, nr13, nr14, runtime);
    }

    pub(in crate::apu) fn trigger(
        &mut self,
        nr10: u8,
        period_value: u16,
        runtime: &mut ChannelRuntimeState,
    ) {
        self.shadow_period = period_value;
        self.phase = Self::phase_from_pace(sweep_pace_from_nr10(nr10));
        self.timer = Self::timer_from_phase(self.phase);
        self.enabled = sweep_pace_from_nr10(nr10) != 0 || sweep_shift_from_nr10(nr10) != 0;
        self.completed_addend = 0;
        self.negate_calculated_since_trigger = false;

        if self
            .calculate_candidate_sum(nr10, false)
            .is_some_and(|candidate| {
                candidate > super::super::common::PULSE_PERIOD_MAX
                    && !sweep_decreases_from_nr10(nr10)
            })
        {
            runtime.active = false;
        }
    }

    pub(in crate::apu) fn clock(
        &mut self,
        nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        if !self.enabled {
            return;
        }

        self.phase = (self.phase + 1) & 0x07;
        self.timer = Self::timer_from_phase(self.phase);
        if self.phase != 7 {
            return;
        }

        self.maybe_fire_sweep_boundary(nr10, nr13, nr14, runtime);
    }

    fn maybe_fire_sweep_boundary(
        &mut self,
        nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        let pace = sweep_pace_from_nr10(nr10);
        if self.phase != 7 || pace == 0 || !self.enabled {
            return;
        }

        self.phase = Self::phase_from_pace(pace);
        self.timer = Self::timer_from_phase(self.phase);

        let shift = sweep_shift_from_nr10(nr10);
        let Some(candidate_sum) = self.calculate_candidate_sum(nr10, true) else {
            return;
        };

        if !sweep_decreases_from_nr10(nr10)
            && candidate_sum > super::super::common::PULSE_PERIOD_MAX
        {
            runtime.active = false;
            return;
        }

        if shift == 0 {
            return;
        }

        let candidate = candidate_sum & super::super::common::PULSE_PERIOD_MAX;
        self.shadow_period = candidate;
        *nr13 = candidate as u8;
        *nr14 = (*nr14 & !PERIOD_HIGH_MASK) | (((candidate >> 8) as u8) & PERIOD_HIGH_MASK);

        if self
            .calculate_candidate_sum(nr10, true)
            .is_some_and(|next_candidate| {
                next_candidate > super::super::common::PULSE_PERIOD_MAX
                    && !sweep_decreases_from_nr10(nr10)
            })
        {
            runtime.active = false;
        }
    }

    const fn phase_from_pace(pace: u8) -> u8 {
        pace ^ 0x07
    }

    const fn timer_from_phase(phase: u8) -> u8 {
        if phase == 7 { 8 } else { 7 - (phase & 0x07) }
    }

    fn calculate_candidate_sum(&mut self, nr10: u8, allow_shift_zero: bool) -> Option<u16> {
        let shift = sweep_shift_from_nr10(nr10);
        if shift == 0 && !allow_shift_zero {
            return None;
        }

        let delta = self.shadow_period >> shift;
        let decreases = sweep_decreases_from_nr10(nr10);
        self.completed_addend = if decreases {
            (!delta) & super::super::common::PULSE_PERIOD_MAX
        } else {
            delta
        };
        self.negate_calculated_since_trigger |= decreases;

        Some(self.shadow_period + self.completed_addend + u16::from(decreases))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::apu) struct Channel1State {
    pub(in crate::apu) nr10: u8,
    pub(in crate::apu) nr11: u8,
    pub(in crate::apu) nr12: u8,
    pub(in crate::apu) nr13: u8,
    pub(in crate::apu) nr14: u8,
    pub(in crate::apu) pulse: PulseChannelState,
    pub(in crate::apu) sweep: Channel1SweepState,
}

impl Channel1State {
    pub(in crate::apu) fn read_nr10(&self) -> u8 {
        self.nr10 | NR10_FORCED_HIGH_MASK
    }

    pub(in crate::apu) fn read_nr11(&self) -> u8 {
        (self.nr11 & 0xC0) | NR11_WRITE_ONLY_MASK
    }

    pub(in crate::apu) fn read_nr14(&self) -> u8 {
        (self.nr14 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK
    }

    pub(in crate::apu) fn write_nr10(&mut self, value: u8) {
        let old_nr10 = self.nr10;
        self.nr10 = value & 0x7F;
        self.sweep.write_nr10(
            old_nr10,
            self.nr10,
            &mut self.nr13,
            &mut self.nr14,
            &mut self.pulse.runtime,
        );
    }

    pub(in crate::apu) fn write_nr11(&mut self, value: u8) {
        self.nr11 = value;
        self.pulse.apply_length_duty_write(value);
    }

    pub(in crate::apu) fn write_nr12(&mut self, value: u8) {
        self.pulse.apply_live_envelope_write_effect(value);
        self.nr12 = value;
        self.pulse
            .runtime
            .set_dac_enabled(self.derived_dac_enabled());
    }

    pub(in crate::apu) fn write_nr13(&mut self, value: u8) {
        self.nr13 = value;
    }

    pub(in crate::apu) fn write_nr14(
        &mut self,
        value: u8,
        console_model: ConsoleModel,
        next_frame_sequencer_step: u8,
    ) {
        let trigger = value & CHANNEL_TRIGGER_BIT != 0;
        let next_step_clocks_length = frame_sequencer_step_clocks_length(next_frame_sequencer_step);
        let next_step_clocks_envelope =
            frame_sequencer_step_clocks_envelope(next_frame_sequencer_step);
        self.nr14 = value & NRX4_WRITABLE_MASK;
        let mut was_length_enabled = self.pulse.length_enabled;
        let mut trigger_reloaded_zero_length = false;

        if trigger {
            trigger_reloaded_zero_length = self.trigger(next_step_clocks_envelope);
            was_length_enabled = self.pulse.length_enabled;
        }

        self.pulse.apply_length_enable(self.nr14);
        self.pulse.apply_extra_length_clocking_on_enable(
            console_model,
            was_length_enabled,
            next_step_clocks_length,
            trigger,
            trigger_reloaded_zero_length,
        );
    }

    pub(in crate::apu) fn apply_powered_startup(
        &mut self,
        nr10: u8,
        nr11: u8,
        nr12: u8,
        nr13: u8,
        nr14: u8,
        active: bool,
    ) {
        self.nr10 = nr10 & 0x7F;
        self.nr11 = nr11;
        self.nr12 = nr12;
        self.nr13 = nr13;
        self.nr14 = nr14 & NRX4_WRITABLE_MASK;
        let mut runtime = ChannelRuntimeState::default();
        runtime.set_dac_enabled(self.derived_dac_enabled());
        runtime.set_active_from_startup(active);
        self.pulse.apply_powered_startup(PulseStartupState {
            length_duty_value: self.nr11,
            envelope_value: self.nr12,
            nrx4: self.nr14,
            period_value: self.period_value(),
            runtime,
            first_trigger_after_power_on_pending: !active,
        });
        self.sweep
            .apply_powered_startup(self.nr10, self.period_value(), self.pulse.runtime.active);
    }

    pub(in crate::apu) fn write_length_while_powered_off(&mut self, value: u8) {
        self.pulse.length_counter = super::super::common::pulse_length_counter_from_load(value);
    }

    fn clear_registers(&mut self) {
        self.nr10 = 0;
        self.nr11 = 0;
        self.nr12 = 0;
        self.nr13 = 0;
        self.nr14 = 0;
        self.pulse.clear();
        self.sweep.clear();
    }

    pub(in crate::apu) fn power_off_registers(&mut self, console_model: ConsoleModel) {
        if console_model.is_dmg_family() {
            self.nr10 = 0;
            self.nr11 = 0;
            self.nr12 = 0;
            self.nr13 = 0;
            self.nr14 = 0;
            self.pulse.clear_preserving_length();
            self.sweep.clear();
            return;
        }

        self.clear_registers();
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr12 & 0xF8 != 0
    }

    pub(in crate::apu) fn period_value(&self) -> u16 {
        pulse_period_from_registers(self.nr13, self.nr14)
    }

    fn trigger(&mut self, next_step_clocks_envelope: bool) -> bool {
        let period_value = self.period_value();
        let trigger_reloaded_zero_length =
            self.pulse
                .trigger(period_value, self.nr12, next_step_clocks_envelope);
        self.sweep
            .trigger(self.nr10, period_value, &mut self.pulse.runtime);
        trigger_reloaded_zero_length
    }

    pub(in crate::apu) fn tick_fast_timer(&mut self) {
        self.pulse.tick_fast_timer(self.period_value());
    }

    pub(in crate::apu) fn clock_length(&mut self) {
        self.pulse.clock_length();
    }

    pub(in crate::apu) fn clock_envelope(&mut self) {
        self.pulse.clock_envelope();
    }

    pub(in crate::apu) fn clock_sweep(&mut self) {
        self.sweep.clock(
            self.nr10,
            &mut self.nr13,
            &mut self.nr14,
            &mut self.pulse.runtime,
        );
    }
}
