use crate::model::ConsoleModel;

use super::super::common::{
    ChannelRuntimeState, ExtraLengthClockingContext, LENGTH_ENABLE_BIT, PULSE_DUTY_MASK,
    PULSE_DUTY_SHIFT, PULSE_DUTY_STEP_MASK, PULSE_LENGTH_COUNTER_RELOAD,
    PULSE_PERIOD_TIMER_LOW_BITS_MASK, apply_consistent_zombie_mode_increment,
    apply_extra_length_clocking_u8, clock_envelope_unit, clock_length_counter_u8,
    decode_envelope_register, envelope_timer_reload, pulse_length_counter_from_load,
    pulse_timer_reload, pulse_waveform_high,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::apu) struct PulseChannelState {
    pub(in crate::apu) runtime: ChannelRuntimeState,
    pub(in crate::apu) duty: u8,
    pub(in crate::apu) duty_step: u8,
    pub(in crate::apu) first_trigger_after_power_on_pending: bool,
    pub(in crate::apu) suppress_initial_trigger_output: bool,
    pub(in crate::apu) period_timer: u16,
    pub(in crate::apu) length_counter: u8,
    pub(in crate::apu) length_enabled: bool,
    pub(in crate::apu) initial_volume: u8,
    pub(in crate::apu) envelope_increase: bool,
    pub(in crate::apu) envelope_pace: u8,
    pub(in crate::apu) envelope_automatic_updates_enabled: bool,
    pub(in crate::apu) envelope_timer: u8,
    pub(in crate::apu) current_volume: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::apu) struct PulseStartupState {
    pub(in crate::apu) length_duty_value: u8,
    pub(in crate::apu) envelope_value: u8,
    pub(in crate::apu) nrx4: u8,
    pub(in crate::apu) period_value: u16,
    pub(in crate::apu) runtime: ChannelRuntimeState,
    pub(in crate::apu) first_trigger_after_power_on_pending: bool,
}

impl PulseChannelState {
    pub(in crate::apu) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(in crate::apu) fn clear_preserving_length(&mut self) {
        let length_counter = self.length_counter;
        self.clear();
        self.length_counter = length_counter;
    }

    pub(in crate::apu) fn mark_powered_on(&mut self) {
        self.first_trigger_after_power_on_pending = true;
        self.suppress_initial_trigger_output = false;
    }

    pub(in crate::apu) fn runtime_state(&self) -> ChannelRuntimeState {
        self.runtime
    }

    pub(in crate::apu) fn apply_length_duty_write(&mut self, value: u8) {
        self.duty = (value & PULSE_DUTY_MASK) >> PULSE_DUTY_SHIFT;
        self.length_counter = pulse_length_counter_from_load(value);
    }

    pub(in crate::apu) fn write_length_counter_while_powered_off(&mut self, value: u8) {
        self.length_counter = pulse_length_counter_from_load(value);
    }

    pub(in crate::apu) fn apply_envelope_write(&mut self, value: u8) {
        decode_envelope_register(
            value,
            &mut self.initial_volume,
            &mut self.envelope_increase,
            &mut self.envelope_pace,
        );
    }

    pub(in crate::apu) fn apply_live_envelope_write_effect(&mut self, value: u8) {
        apply_consistent_zombie_mode_increment(
            self.runtime.active,
            &mut self.current_volume,
            value,
        );
    }

    pub(in crate::apu) fn apply_length_enable(&mut self, value: u8) {
        self.length_enabled = value & LENGTH_ENABLE_BIT != 0;
    }

    pub(in crate::apu) fn apply_extra_length_clocking_on_enable(
        &mut self,
        console_model: ConsoleModel,
        was_length_enabled: bool,
        next_step_clocks_length: bool,
        trigger: bool,
        trigger_reloaded_zero_length: bool,
    ) {
        apply_extra_length_clocking_u8(
            ExtraLengthClockingContext {
                console_model,
                length_enabled: self.length_enabled,
                was_length_enabled,
                next_step_clocks_length,
                trigger,
                trigger_reloaded_zero_length,
            },
            &mut self.length_counter,
            PULSE_LENGTH_COUNTER_RELOAD,
            &mut self.runtime.active,
        );
    }

    pub(in crate::apu) fn apply_powered_startup(&mut self, startup: PulseStartupState) {
        self.clear();
        self.apply_length_duty_write(startup.length_duty_value);
        self.apply_envelope_write(startup.envelope_value);
        self.apply_length_enable(startup.nrx4);
        self.first_trigger_after_power_on_pending = startup.first_trigger_after_power_on_pending;
        self.period_timer = pulse_timer_reload(startup.period_value);
        self.envelope_automatic_updates_enabled = self.envelope_pace != 0;
        self.envelope_timer = envelope_timer_reload(self.envelope_pace);
        self.current_volume = self.initial_volume;
        self.runtime = startup.runtime;
    }

    pub(in crate::apu) fn apply_channel_startup(
        &mut self,
        length_duty_value: u8,
        envelope_value: u8,
        nrx4: u8,
        period_value: u16,
        dac_enabled: bool,
        active: bool,
    ) {
        let mut runtime = ChannelRuntimeState::default();
        runtime.set_dac_enabled(dac_enabled);
        runtime.set_active_from_startup(active);
        self.apply_powered_startup(PulseStartupState {
            length_duty_value,
            envelope_value,
            nrx4,
            period_value,
            runtime,
            first_trigger_after_power_on_pending: !active,
        });
    }

    pub(in crate::apu) fn power_off(&mut self, console_model: ConsoleModel) {
        if console_model.is_dmg_family() {
            self.clear_preserving_length();
        } else {
            self.clear();
        }
    }

    pub(in crate::apu) fn trigger(
        &mut self,
        period_value: u16,
        envelope_value: u8,
        next_step_clocks_envelope: bool,
    ) -> bool {
        let reloaded_zero_length = self.length_counter == 0;
        if self.length_counter == 0 {
            self.length_counter = PULSE_LENGTH_COUNTER_RELOAD;
            self.length_enabled = false;
        }

        self.apply_envelope_write(envelope_value);
        let preserved_period_timer_low_bits = self.period_timer & PULSE_PERIOD_TIMER_LOW_BITS_MASK;
        self.period_timer = pulse_timer_reload(period_value) | preserved_period_timer_low_bits;
        self.envelope_automatic_updates_enabled = self.envelope_pace != 0;
        self.envelope_timer =
            envelope_timer_reload(self.envelope_pace) + u8::from(next_step_clocks_envelope);
        self.current_volume = self.initial_volume;
        if self.first_trigger_after_power_on_pending {
            self.suppress_initial_trigger_output = true;
            self.first_trigger_after_power_on_pending = false;
        }
        self.runtime.trigger();
        reloaded_zero_length
    }

    pub(in crate::apu) fn tick_fast_timer(&mut self, period_value: u16) {
        if self.first_trigger_after_power_on_pending {
            return;
        }

        if self.period_timer > 0 {
            self.period_timer -= 1;
        }

        if self.period_timer == 0 {
            self.period_timer = pulse_timer_reload(period_value);
            self.duty_step = (self.duty_step + 1) & PULSE_DUTY_STEP_MASK;
            self.suppress_initial_trigger_output = false;
        }
    }

    pub(in crate::apu) fn clock_length(&mut self) {
        clock_length_counter_u8(
            self.length_enabled,
            &mut self.length_counter,
            &mut self.runtime.active,
        );
    }

    pub(in crate::apu) fn clock_envelope(&mut self) {
        clock_envelope_unit(
            self.envelope_pace,
            self.envelope_increase,
            &mut self.envelope_timer,
            &mut self.current_volume,
            &mut self.envelope_automatic_updates_enabled,
        );
    }

    pub(in crate::apu) fn current_digital_output(&self) -> u8 {
        if !self.runtime.active {
            return 0;
        }

        if self.suppress_initial_trigger_output {
            return 0;
        }

        if pulse_waveform_high(self.duty, self.duty_step) {
            self.current_volume
        } else {
            0
        }
    }
}
