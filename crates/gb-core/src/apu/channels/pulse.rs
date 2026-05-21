use crate::model::ConsoleModel;
use crate::speed::CgbSpeedMode;

use super::super::common::{
    ChannelRuntimeState, EnvelopeState, ExtraLengthClockingContext, LENGTH_ENABLE_BIT,
    PULSE_DUTY_MASK, PULSE_DUTY_SHIFT, PULSE_DUTY_STEP_MASK, PULSE_LENGTH_COUNTER_RELOAD,
    apply_extra_length_clocking_u8, cgb_pulse_trigger_delay_t_cycles, clock_length_counter_u8,
    pulse_length_counter_from_load, pulse_timer_reload,
    pulse_timer_reload_preserving_trigger_phase, pulse_waveform_high,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::apu) struct PulseChannelState {
    pub(in crate::apu) runtime: ChannelRuntimeState,
    pub(in crate::apu) duty: u8,
    #[serde(default)]
    pub(in crate::apu) pending_duty: Option<u8>,
    pub(in crate::apu) duty_step: u8,
    pub(in crate::apu) first_trigger_after_power_on_pending: bool,
    pub(in crate::apu) power_on_phase: u8,
    pub(in crate::apu) suppress_initial_trigger_output: bool,
    pub(in crate::apu) trigger_delay_t_cycles: u16,
    #[serde(default)]
    pub(in crate::apu) just_sampled: bool,
    #[serde(default)]
    just_sampled_reload_clocks_remaining: u8,
    #[serde(default)]
    pub(in crate::apu) timer_stopped_by_dac_disable: bool,
    pub(in crate::apu) period_timer: u16,
    pub(in crate::apu) length_counter: u8,
    pub(in crate::apu) length_enabled: bool,
    pub(in crate::apu) envelope: EnvelopeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
        self.power_on_phase = 0;
        self.suppress_initial_trigger_output = false;
        self.trigger_delay_t_cycles = 0;
        self.just_sampled = false;
        self.just_sampled_reload_clocks_remaining = 0;
        self.timer_stopped_by_dac_disable = !self.runtime.dac_enabled;
    }

    pub(in crate::apu) fn runtime_state(&self) -> ChannelRuntimeState {
        self.runtime
    }

    pub(in crate::apu) fn apply_length_duty_write(&mut self, value: u8) {
        let duty = (value & PULSE_DUTY_MASK) >> PULSE_DUTY_SHIFT;
        if self.runtime.active {
            self.pending_duty = Some(duty);
        } else {
            self.duty = duty;
            self.pending_duty = None;
        }
        self.length_counter = pulse_length_counter_from_load(value);
    }

    pub(in crate::apu) fn write_length_counter_while_powered_off(&mut self, value: u8) {
        self.length_counter = pulse_length_counter_from_load(value);
    }

    pub(in crate::apu) fn apply_envelope_write(&mut self, value: u8) {
        self.envelope.apply_write(value);
    }

    pub(in crate::apu) fn apply_live_envelope_write_effect(
        &mut self,
        console_model: ConsoleModel,
        value: u8,
    ) {
        self.envelope
            .apply_live_write_effect(console_model, self.runtime.active, value);
    }

    pub(in crate::apu) fn apply_dac_enabled(&mut self, dac_enabled: bool) {
        if !dac_enabled {
            self.timer_stopped_by_dac_disable = true;
            self.trigger_delay_t_cycles = 0;
            self.just_sampled = false;
            self.just_sampled_reload_clocks_remaining = 0;
        }

        self.runtime.set_dac_enabled(dac_enabled);
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
        self.envelope.reload(false);
        self.runtime = startup.runtime;
        self.timer_stopped_by_dac_disable = !self.runtime.dac_enabled;
        if !self.first_trigger_after_power_on_pending {
            self.power_on_phase = 0;
        }
        self.just_sampled = false;
        self.just_sampled_reload_clocks_remaining = 0;
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
        console_model: ConsoleModel,
        speed_mode: CgbSpeedMode,
        period_value: u16,
        envelope_value: u8,
        next_step_clocks_envelope: bool,
    ) -> bool {
        let was_active = self.runtime.active;
        let reloaded_zero_length = self.length_counter == 0;
        if self.length_counter == 0 {
            self.length_counter = PULSE_LENGTH_COUNTER_RELOAD;
            self.length_enabled = false;
        }

        self.apply_envelope_write(envelope_value);
        self.period_timer =
            pulse_timer_reload_preserving_trigger_phase(period_value, self.period_timer);
        self.trigger_delay_t_cycles = cgb_pulse_trigger_delay_t_cycles(
            console_model,
            speed_mode,
            was_active,
            self.power_on_phase,
        );
        self.envelope.reload(next_step_clocks_envelope);
        self.runtime.trigger();
        if self.runtime.active && !was_active {
            self.suppress_initial_trigger_output = true;
        }
        if self.runtime.active {
            self.timer_stopped_by_dac_disable = false;
        }
        if self.first_trigger_after_power_on_pending {
            self.first_trigger_after_power_on_pending = false;
        }
        reloaded_zero_length
    }

    pub(in crate::apu) fn reload_period_after_write_if_just_sampled(&mut self, period_value: u16) {
        if self.just_sampled {
            self.period_timer = pulse_timer_reload(period_value);
        }
    }

    pub(in crate::apu) fn tick_fast_timer_with_clock_gate(
        &mut self,
        period_value: u16,
        clock_period_timer: bool,
    ) {
        if clock_period_timer && self.just_sampled {
            self.just_sampled_reload_clocks_remaining =
                self.just_sampled_reload_clocks_remaining.saturating_sub(1);
            if self.just_sampled_reload_clocks_remaining == 0 {
                self.just_sampled = false;
            }
        }

        if self.first_trigger_after_power_on_pending {
            self.power_on_phase =
                (self.power_on_phase + 1) & super::super::common::CGB_PULSE_POWER_ON_PHASE_MASK;
            return;
        }

        if self.trigger_delay_t_cycles > 0 {
            self.trigger_delay_t_cycles -= 1;
            return;
        }

        if !clock_period_timer {
            return;
        }

        if self.timer_stopped_by_dac_disable {
            return;
        }

        if self.period_timer > 0 {
            self.period_timer -= 1;
        }

        if self.period_timer == 0 {
            self.period_timer = pulse_timer_reload(period_value);
            self.duty_step = (self.duty_step + 1) & PULSE_DUTY_STEP_MASK;
            if let Some(pending_duty) = self.pending_duty.take() {
                self.duty = pending_duty;
            }
            self.suppress_initial_trigger_output = false;
            self.just_sampled = true;
            self.just_sampled_reload_clocks_remaining = 3;
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
        self.envelope.clock();
    }

    pub(in crate::apu) fn clock_cgb_live_write_pending_even_envelope_tick(&mut self) {
        self.envelope.clock_cgb_live_write_pending_even_tick();
    }

    pub(in crate::apu) fn current_digital_output(&self) -> u8 {
        if !self.runtime.active {
            return 0;
        }

        if self.suppress_initial_trigger_output {
            return 0;
        }

        if pulse_waveform_high(self.duty, self.duty_step) {
            self.envelope.current_volume
        } else {
            0
        }
    }
}
