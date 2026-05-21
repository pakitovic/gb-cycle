use crate::model::ConsoleModel;
use crate::speed::CgbSpeedMode;

use super::super::common::{
    ChannelRuntimeState, DAC_ENABLE_REGISTER_MASK, NR11_WRITE_ONLY_MASK, NR14_FORCED_HIGH_MASK,
    NR14_READ_MASK, NR23_WRITE_ONLY_READ_VALUE, NRX4_WRITABLE_MASK, PULSE_DUTY_MASK,
    begin_nrx4_write, pulse_period_from_registers,
};
use super::super::registers::Channel2Register;
use super::pulse::PulseChannelState;

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::apu) struct Channel2State {
    nr21: u8,
    nr22: u8,
    nr23: u8,
    nr24: u8,
    pub(in crate::apu) pulse: PulseChannelState,
}

impl Channel2State {
    pub(in crate::apu) fn read_register(&self, register: Channel2Register) -> u8 {
        match register {
            Channel2Register::Nr21 => self.read_nr21(),
            Channel2Register::Nr22 => self.nr22,
            Channel2Register::Nr23 => NR23_WRITE_ONLY_READ_VALUE,
            Channel2Register::Nr24 => self.read_nr24(),
        }
    }

    pub(in crate::apu) fn write_register(
        &mut self,
        register: Channel2Register,
        value: u8,
        console_model: ConsoleModel,
        speed_mode: CgbSpeedMode,
        next_frame_sequencer_step: u8,
    ) {
        match register {
            Channel2Register::Nr21 => self.write_nr21(value),
            Channel2Register::Nr22 => self.write_nr22(value, console_model),
            Channel2Register::Nr23 => self.write_nr23(value, console_model),
            Channel2Register::Nr24 => {
                self.write_nr24(value, console_model, speed_mode, next_frame_sequencer_step)
            }
        }
    }

    pub(in crate::apu) fn write_powered_off_register(
        &mut self,
        register: Channel2Register,
        value: u8,
        console_model: ConsoleModel,
    ) {
        if !console_model.is_dmg_family() {
            return;
        }

        if matches!(register, Channel2Register::Nr21) {
            self.write_length_while_powered_off(value);
        }
    }

    fn read_nr21(&self) -> u8 {
        (self.nr21 & PULSE_DUTY_MASK) | NR11_WRITE_ONLY_MASK
    }

    fn read_nr24(&self) -> u8 {
        (self.nr24 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK
    }

    fn write_nr21(&mut self, value: u8) {
        self.nr21 = value;
        self.pulse.apply_length_duty_write(value);
    }

    fn write_nr22(&mut self, value: u8, console_model: ConsoleModel) {
        self.pulse
            .apply_live_envelope_write_effect(console_model, value);
        self.nr22 = value;
        self.pulse.apply_dac_enabled(self.derived_dac_enabled());
    }

    fn write_nr23(&mut self, value: u8, console_model: ConsoleModel) {
        self.nr23 = value;
        if console_model.is_cgb_family() {
            self.pulse
                .reload_period_after_write_if_just_sampled(self.period_value());
        }
    }

    fn write_nr24(
        &mut self,
        value: u8,
        console_model: ConsoleModel,
        speed_mode: CgbSpeedMode,
        next_frame_sequencer_step: u8,
    ) {
        let mut write_plan = begin_nrx4_write(
            &mut self.nr24,
            value,
            NRX4_WRITABLE_MASK,
            next_frame_sequencer_step,
            self.pulse.length_enabled,
        );

        if write_plan.context.trigger {
            write_plan.observe_trigger_reloaded_zero_length(self.trigger(
                console_model,
                speed_mode,
                write_plan.context.next_step_clocks_envelope,
            ));
            write_plan.observe_length_enabled_after_trigger(self.pulse.length_enabled);
        } else if console_model.is_cgb_family() {
            self.pulse
                .reload_period_after_write_if_just_sampled(self.period_value());
        }

        self.pulse.length_enabled = write_plan.context.length_enabled;
        self.pulse.apply_extra_length_clocking_on_enable(
            console_model,
            write_plan.was_length_enabled,
            write_plan.context.next_step_clocks_length,
            write_plan.context.trigger,
            write_plan.trigger_reloaded_zero_length,
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
        self.pulse.apply_channel_startup(
            self.nr21,
            self.nr22,
            self.nr24,
            self.period_value(),
            self.derived_dac_enabled(),
            active,
        );
    }

    pub(in crate::apu) fn write_length_while_powered_off(&mut self, value: u8) {
        self.pulse.write_length_counter_while_powered_off(value);
    }

    pub(in crate::apu) fn power_off_registers(&mut self, console_model: ConsoleModel) {
        self.nr21 = 0;
        self.nr22 = 0;
        self.nr23 = 0;
        self.nr24 = 0;
        self.pulse.power_off(console_model);
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr22 & DAC_ENABLE_REGISTER_MASK != 0
    }

    pub(in crate::apu) fn period_value(&self) -> u16 {
        pulse_period_from_registers(self.nr23, self.nr24)
    }

    pub(in crate::apu) fn runtime_state(&self) -> ChannelRuntimeState {
        self.pulse.runtime_state()
    }

    pub(in crate::apu) fn current_digital_output(&self) -> u8 {
        self.pulse.current_digital_output()
    }

    pub(in crate::apu) fn mark_powered_on(&mut self) {
        self.pulse.mark_powered_on();
    }

    fn trigger(
        &mut self,
        console_model: ConsoleModel,
        speed_mode: CgbSpeedMode,
        next_step_clocks_envelope: bool,
    ) -> bool {
        self.pulse.trigger(
            console_model,
            speed_mode,
            self.period_value(),
            self.nr22,
            next_step_clocks_envelope,
        )
    }

    #[cfg(test)]
    pub(in crate::apu) fn tick_fast_timer(&mut self) {
        self.tick_fast_timer_with_clock_gate(true);
    }

    pub(in crate::apu) fn tick_fast_timer_with_clock_gate(&mut self, clock_period_timer: bool) {
        self.pulse
            .tick_fast_timer_with_clock_gate(self.period_value(), clock_period_timer);
    }

    pub(in crate::apu) fn clock_length(&mut self) {
        self.pulse.clock_length();
    }

    pub(in crate::apu) fn clock_envelope(&mut self) {
        self.pulse.clock_envelope();
    }

    pub(in crate::apu) fn clock_cgb_live_write_pending_even_envelope_tick(&mut self) {
        self.pulse.clock_cgb_live_write_pending_even_envelope_tick();
    }
}
