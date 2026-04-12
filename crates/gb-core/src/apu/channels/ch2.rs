use crate::model::ConsoleModel;

use super::super::common::{
    CHANNEL_TRIGGER_BIT, NR11_WRITE_ONLY_MASK, NR14_FORCED_HIGH_MASK, NR14_READ_MASK,
    NRX4_WRITABLE_MASK, frame_sequencer_step_clocks_envelope, frame_sequencer_step_clocks_length,
    pulse_length_counter_from_load, pulse_period_from_registers,
};
use super::pulse::{PulseChannelState, PulseStartupState};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::apu) struct Channel2State {
    pub(in crate::apu) nr21: u8,
    pub(in crate::apu) nr22: u8,
    pub(in crate::apu) nr23: u8,
    pub(in crate::apu) nr24: u8,
    pub(in crate::apu) pulse: PulseChannelState,
}

impl Channel2State {
    pub(in crate::apu) fn read_nr21(&self) -> u8 {
        (self.nr21 & 0xC0) | NR11_WRITE_ONLY_MASK
    }

    pub(in crate::apu) fn read_nr24(&self) -> u8 {
        (self.nr24 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK
    }

    pub(in crate::apu) fn write_nr21(&mut self, value: u8) {
        self.nr21 = value;
        self.pulse.apply_length_duty_write(value);
    }

    pub(in crate::apu) fn write_nr22(&mut self, value: u8) {
        self.pulse.apply_live_envelope_write_effect(value);
        self.nr22 = value;
        self.pulse
            .runtime
            .set_dac_enabled(self.derived_dac_enabled());
    }

    pub(in crate::apu) fn write_nr23(&mut self, value: u8) {
        self.nr23 = value;
    }

    pub(in crate::apu) fn write_nr24(
        &mut self,
        value: u8,
        console_model: ConsoleModel,
        next_frame_sequencer_step: u8,
    ) {
        let trigger = value & CHANNEL_TRIGGER_BIT != 0;
        let next_step_clocks_length = frame_sequencer_step_clocks_length(next_frame_sequencer_step);
        let next_step_clocks_envelope =
            frame_sequencer_step_clocks_envelope(next_frame_sequencer_step);
        self.nr24 = value & NRX4_WRITABLE_MASK;
        let mut was_length_enabled = self.pulse.length_enabled;
        let mut trigger_reloaded_zero_length = false;

        if trigger {
            trigger_reloaded_zero_length = self.trigger(next_step_clocks_envelope);
            was_length_enabled = self.pulse.length_enabled;
        }

        self.pulse.apply_length_enable(self.nr24);
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
        nr21: u8,
        nr22: u8,
        nr23: u8,
        nr24: u8,
        active: bool,
    ) {
        self.nr21 = nr21;
        self.nr22 = nr22;
        self.nr23 = nr23;
        self.nr24 = nr24 & NRX4_WRITABLE_MASK;
        let mut runtime = super::super::common::ChannelRuntimeState::default();
        runtime.set_dac_enabled(self.derived_dac_enabled());
        runtime.set_active_from_startup(active);
        self.pulse.apply_powered_startup(PulseStartupState {
            length_duty_value: self.nr21,
            envelope_value: self.nr22,
            nrx4: self.nr24,
            period_value: self.period_value(),
            runtime,
            first_trigger_after_power_on_pending: !active,
        });
    }

    pub(in crate::apu) fn write_length_while_powered_off(&mut self, value: u8) {
        self.pulse.length_counter = pulse_length_counter_from_load(value);
    }

    fn clear_registers(&mut self) {
        self.nr21 = 0;
        self.nr22 = 0;
        self.nr23 = 0;
        self.nr24 = 0;
        self.pulse.clear();
    }

    pub(in crate::apu) fn power_off_registers(&mut self, console_model: ConsoleModel) {
        if console_model.is_dmg_family() {
            self.nr21 = 0;
            self.nr22 = 0;
            self.nr23 = 0;
            self.nr24 = 0;
            self.pulse.clear_preserving_length();
            return;
        }

        self.clear_registers();
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr22 & 0xF8 != 0
    }

    pub(in crate::apu) fn period_value(&self) -> u16 {
        pulse_period_from_registers(self.nr23, self.nr24)
    }

    fn trigger(&mut self, next_step_clocks_envelope: bool) -> bool {
        self.pulse
            .trigger(self.period_value(), self.nr22, next_step_clocks_envelope)
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
}
