use crate::model::ConsoleModel;
use crate::speed::CgbSpeedMode;

use super::super::common::{
    CGB_CH1_SWEEP_DECREASE_RESTART_HOLD_T_CYCLES, CGB_SWEEP_DELAYED_CALCULATION_MIN_T_CYCLES,
    CGB_SWEEP_DELAYED_CALCULATION_T_CYCLES_PER_SHIFT_STEP,
    CGB_SWEEP_TRIGGER_DELAYED_CALCULATION_EXTRA_T_CYCLES, ChannelRuntimeState,
    DAC_ENABLE_REGISTER_MASK, NR10_FORCED_HIGH_MASK, NR10_WRITABLE_MASK, NR11_WRITE_ONLY_MASK,
    NR13_WRITE_ONLY_READ_VALUE, NR14_FORCED_HIGH_MASK, NR14_READ_MASK, NRX4_WRITABLE_MASK,
    PERIOD_HIGH_MASK, PULSE_DUTY_MASK, SWEEP_PHASE_BOUNDARY, SWEEP_PHASE_MASK, SWEEP_TIMER_RELOAD,
    begin_nrx4_write, pulse_period_from_registers, sweep_decreases_from_nr10, sweep_pace_from_nr10,
    sweep_shift_from_nr10,
};
use super::super::registers::Channel1Register;
use super::pulse::PulseChannelState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct SweepCalculation {
    candidate_sum: u16,
    addend: u16,
    decreases: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::apu) struct Channel1SweepState {
    pub(in crate::apu) timer: u8,
    phase: u8,
    pub(in crate::apu) enabled: bool,
    pub(in crate::apu) shadow_period: u16,
    completed_addend: u16,
    pub(in crate::apu) negate_calculated_since_trigger: bool,
    #[serde(default)]
    delayed_calculation_t_cycles: u16,
    #[serde(default)]
    delayed_calculation_shadow_period: u16,
    #[serde(default)]
    delayed_calculation_addend: u16,
    #[serde(default)]
    delayed_calculation_decreases: bool,
    #[serde(default)]
    decreasing_writeback_since_trigger: bool,
}

impl Channel1SweepState {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn apply_powered_startup(&mut self, nr10: u8, period_value: u16, active: bool) {
        self.clear();
        self.shadow_period = period_value;
        self.phase = Self::phase_from_pace(sweep_pace_from_nr10(nr10));
        self.timer = Self::timer_from_phase(self.phase);
        self.enabled =
            active && (sweep_pace_from_nr10(nr10) != 0 || sweep_shift_from_nr10(nr10) != 0);
    }

    fn write_nr10(
        &mut self,
        console_model: ConsoleModel,
        old_nr10: u8,
        new_nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        if console_model.is_cgb_family()
            && sweep_pace_from_nr10(new_nr10) == 0
            && sweep_shift_from_nr10(new_nr10) == 0
        {
            self.clear_delayed_calculation();
        }

        if sweep_decreases_from_nr10(old_nr10)
            && !sweep_decreases_from_nr10(new_nr10)
            && self.negate_calculated_since_trigger
        {
            runtime.active = false;
        }

        self.maybe_fire_sweep_boundary(console_model, new_nr10, nr13, nr14, runtime);
    }

    fn trigger(
        &mut self,
        console_model: ConsoleModel,
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
        self.decreasing_writeback_since_trigger = false;
        self.clear_delayed_calculation();

        if let Some(calculation) = self.calculate_candidate_sum(nr10, self.shadow_period, false) {
            self.observe_calculation(calculation);
            if console_model.is_cgb_family() {
                self.schedule_delayed_calculation(
                    nr10,
                    self.shadow_period,
                    calculation,
                    CGB_SWEEP_TRIGGER_DELAYED_CALCULATION_EXTRA_T_CYCLES,
                );
            } else if calculation.candidate_sum > super::super::common::PULSE_PERIOD_MAX
                && !calculation.decreases
            {
                runtime.active = false;
            }
        }
    }

    fn tick_delayed_calculation(
        &mut self,
        clock_generation_timer: bool,
        runtime: &mut ChannelRuntimeState,
    ) {
        if !clock_generation_timer || self.delayed_calculation_t_cycles == 0 {
            return;
        }

        self.delayed_calculation_t_cycles -= 1;
        if self.delayed_calculation_t_cycles != 0 {
            return;
        }

        if !self.delayed_calculation_decreases
            && self.delayed_calculation_shadow_period + self.delayed_calculation_addend
                > super::super::common::PULSE_PERIOD_MAX
        {
            runtime.active = false;
        }
        self.completed_addend = self.delayed_calculation_addend;
        self.clear_delayed_calculation();
    }

    fn clock(
        &mut self,
        console_model: ConsoleModel,
        nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        if !self.enabled {
            return;
        }

        self.phase = (self.phase + 1) & SWEEP_PHASE_MASK;
        self.timer = Self::timer_from_phase(self.phase);
        if self.phase != SWEEP_PHASE_BOUNDARY {
            return;
        }

        self.maybe_fire_sweep_boundary(console_model, nr10, nr13, nr14, runtime);
    }

    fn maybe_fire_sweep_boundary(
        &mut self,
        console_model: ConsoleModel,
        nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        let pace = sweep_pace_from_nr10(nr10);
        if self.phase != SWEEP_PHASE_BOUNDARY || pace == 0 || !self.enabled {
            return;
        }

        self.phase = Self::phase_from_pace(pace);
        self.timer = Self::timer_from_phase(self.phase);

        let shift = sweep_shift_from_nr10(nr10);
        let Some(calculation) = self.calculate_candidate_sum(nr10, self.shadow_period, true) else {
            return;
        };
        self.observe_calculation(calculation);

        if !calculation.decreases
            && calculation.candidate_sum > super::super::common::PULSE_PERIOD_MAX
        {
            if console_model.is_cgb_family() {
                self.schedule_delayed_calculation(nr10, self.shadow_period, calculation, 0);
            } else {
                runtime.active = false;
            }
            return;
        }

        if shift == 0 {
            if console_model.is_cgb_family() {
                self.schedule_delayed_calculation(nr10, self.shadow_period, calculation, 0);
            }
            return;
        }

        let candidate = calculation.candidate_sum & super::super::common::PULSE_PERIOD_MAX;
        self.shadow_period = candidate;
        *nr13 = candidate as u8;
        *nr14 = (*nr14 & !PERIOD_HIGH_MASK) | (((candidate >> 8) as u8) & PERIOD_HIGH_MASK);
        self.decreasing_writeback_since_trigger |= calculation.decreases;

        if let Some(next_calculation) = self.calculate_candidate_sum(nr10, self.shadow_period, true)
        {
            self.observe_calculation(next_calculation);
            if console_model.is_cgb_family() {
                self.schedule_delayed_calculation(nr10, self.shadow_period, next_calculation, 0);
            } else if next_calculation.candidate_sum > super::super::common::PULSE_PERIOD_MAX
                && !next_calculation.decreases
            {
                runtime.active = false;
            }
        }
    }

    fn schedule_delayed_calculation(
        &mut self,
        nr10: u8,
        shadow_period: u16,
        calculation: SweepCalculation,
        extra_t_cycles: u16,
    ) {
        let shift_delay = u16::from(sweep_shift_from_nr10(nr10) + 1)
            * CGB_SWEEP_DELAYED_CALCULATION_T_CYCLES_PER_SHIFT_STEP;
        self.delayed_calculation_t_cycles =
            shift_delay.max(CGB_SWEEP_DELAYED_CALCULATION_MIN_T_CYCLES) + extra_t_cycles;
        self.delayed_calculation_shadow_period = shadow_period;
        self.delayed_calculation_addend = calculation.addend;
        self.delayed_calculation_decreases = calculation.decreases;
    }

    fn clear_delayed_calculation(&mut self) {
        self.delayed_calculation_t_cycles = 0;
        self.delayed_calculation_shadow_period = 0;
        self.delayed_calculation_addend = 0;
        self.delayed_calculation_decreases = false;
    }

    const fn phase_from_pace(pace: u8) -> u8 {
        pace ^ SWEEP_PHASE_MASK
    }

    const fn timer_from_phase(phase: u8) -> u8 {
        if phase == SWEEP_PHASE_BOUNDARY {
            SWEEP_TIMER_RELOAD
        } else {
            SWEEP_PHASE_BOUNDARY - (phase & SWEEP_PHASE_MASK)
        }
    }

    fn calculate_candidate_sum(
        &self,
        nr10: u8,
        shadow_period: u16,
        allow_shift_zero: bool,
    ) -> Option<SweepCalculation> {
        let shift = sweep_shift_from_nr10(nr10);
        if shift == 0 && !allow_shift_zero {
            return None;
        }

        let delta = shadow_period >> shift;
        let decreases = sweep_decreases_from_nr10(nr10);
        let addend = if decreases {
            (!delta) & super::super::common::PULSE_PERIOD_MAX
        } else {
            delta
        };

        Some(SweepCalculation {
            candidate_sum: shadow_period + addend + u16::from(decreases),
            addend,
            decreases,
        })
    }

    fn observe_calculation(&mut self, calculation: SweepCalculation) {
        self.completed_addend = calculation.addend;
        self.negate_calculated_since_trigger |= calculation.decreases;
    }

    fn cgb_decrease_restart_hold_t_cycles(
        &self,
        console_model: ConsoleModel,
        was_active: bool,
    ) -> u16 {
        if console_model.is_cgb_family() && was_active && self.decreasing_writeback_since_trigger {
            CGB_CH1_SWEEP_DECREASE_RESTART_HOLD_T_CYCLES
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::apu) struct Channel1State {
    nr10: u8,
    nr11: u8,
    nr12: u8,
    nr13: u8,
    nr14: u8,
    pub(in crate::apu) pulse: PulseChannelState,
    pub(in crate::apu) sweep: Channel1SweepState,
}

impl Channel1State {
    pub(in crate::apu) fn read_register(&self, register: Channel1Register) -> u8 {
        match register {
            Channel1Register::Nr10 => self.read_nr10(),
            Channel1Register::Nr11 => self.read_nr11(),
            Channel1Register::Nr12 => self.nr12,
            Channel1Register::Nr13 => NR13_WRITE_ONLY_READ_VALUE,
            Channel1Register::Nr14 => self.read_nr14(),
        }
    }

    pub(in crate::apu) fn write_register(
        &mut self,
        register: Channel1Register,
        value: u8,
        console_model: ConsoleModel,
        speed_mode: CgbSpeedMode,
        next_frame_sequencer_step: u8,
    ) {
        match register {
            Channel1Register::Nr10 => self.write_nr10(value, console_model),
            Channel1Register::Nr11 => self.write_nr11(value),
            Channel1Register::Nr12 => self.write_nr12(value, console_model),
            Channel1Register::Nr13 => self.write_nr13(value),
            Channel1Register::Nr14 => {
                self.write_nr14(value, console_model, speed_mode, next_frame_sequencer_step)
            }
        }
    }

    pub(in crate::apu) fn write_powered_off_register(
        &mut self,
        register: Channel1Register,
        value: u8,
        console_model: ConsoleModel,
    ) {
        if !console_model.is_dmg_family() {
            return;
        }

        if matches!(register, Channel1Register::Nr11) {
            self.write_length_while_powered_off(value);
        }
    }

    fn read_nr10(&self) -> u8 {
        self.nr10 | NR10_FORCED_HIGH_MASK
    }

    fn read_nr11(&self) -> u8 {
        (self.nr11 & PULSE_DUTY_MASK) | NR11_WRITE_ONLY_MASK
    }

    fn read_nr14(&self) -> u8 {
        (self.nr14 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK
    }

    fn write_nr10(&mut self, value: u8, console_model: ConsoleModel) {
        let old_nr10 = self.nr10;
        self.nr10 = value & NR10_WRITABLE_MASK;
        self.sweep.write_nr10(
            console_model,
            old_nr10,
            self.nr10,
            &mut self.nr13,
            &mut self.nr14,
            &mut self.pulse.runtime,
        );
    }

    fn write_nr11(&mut self, value: u8) {
        self.nr11 = value;
        self.pulse.apply_length_duty_write(value);
    }

    fn write_nr12(&mut self, value: u8, console_model: ConsoleModel) {
        self.pulse
            .apply_live_envelope_write_effect(console_model, value);
        self.nr12 = value;
        self.pulse.apply_dac_enabled(self.derived_dac_enabled());
    }

    fn write_nr13(&mut self, value: u8) {
        self.nr13 = value;
    }

    fn write_nr14(
        &mut self,
        value: u8,
        console_model: ConsoleModel,
        speed_mode: CgbSpeedMode,
        next_frame_sequencer_step: u8,
    ) {
        let mut write_plan = begin_nrx4_write(
            &mut self.nr14,
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
        nr10: u8,
        nr11: u8,
        nr12: u8,
        nr13: u8,
        nr14: u8,
        active: bool,
    ) {
        self.nr10 = nr10 & NR10_WRITABLE_MASK;
        self.nr11 = nr11;
        self.nr12 = nr12;
        self.nr13 = nr13;
        self.nr14 = nr14 & NRX4_WRITABLE_MASK;
        self.pulse.apply_channel_startup(
            self.nr11,
            self.nr12,
            self.nr14,
            self.period_value(),
            self.derived_dac_enabled(),
            active,
        );
        self.sweep
            .apply_powered_startup(self.nr10, self.period_value(), self.pulse.runtime.active);
    }

    pub(in crate::apu) fn write_length_while_powered_off(&mut self, value: u8) {
        self.pulse.write_length_counter_while_powered_off(value);
    }

    pub(in crate::apu) fn power_off_registers(&mut self, console_model: ConsoleModel) {
        self.nr10 = 0;
        self.nr11 = 0;
        self.nr12 = 0;
        self.nr13 = 0;
        self.nr14 = 0;
        self.pulse.power_off(console_model);
        self.sweep.clear();
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr12 & DAC_ENABLE_REGISTER_MASK != 0
    }

    pub(in crate::apu) fn period_value(&self) -> u16 {
        pulse_period_from_registers(self.nr13, self.nr14)
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
        let period_value = self.period_value();
        let was_active = self.pulse.runtime.active;
        let sweep_restart_hold_t_cycles = self
            .sweep
            .cgb_decrease_restart_hold_t_cycles(console_model, was_active);
        let trigger_reloaded_zero_length = self.pulse.trigger(
            console_model,
            speed_mode,
            period_value,
            self.nr12,
            next_step_clocks_envelope,
        );
        self.pulse.extend_trigger_delay(sweep_restart_hold_t_cycles);
        self.sweep.trigger(
            console_model,
            self.nr10,
            period_value,
            &mut self.pulse.runtime,
        );
        trigger_reloaded_zero_length
    }

    #[cfg(test)]
    pub(in crate::apu) fn tick_fast_timer(&mut self) {
        self.tick_fast_timer_with_clock_gate(true);
    }

    pub(in crate::apu) fn tick_fast_timer_with_clock_gate(&mut self, clock_period_timer: bool) {
        self.sweep
            .tick_delayed_calculation(clock_period_timer, &mut self.pulse.runtime);
        self.pulse
            .tick_fast_timer_with_clock_gate(self.period_value(), clock_period_timer);
    }

    pub(in crate::apu) fn clock_length(&mut self) {
        self.pulse.clock_length();
    }

    pub(in crate::apu) fn clock_envelope(&mut self) {
        self.pulse.clock_envelope();
    }

    pub(in crate::apu) fn clock_sweep(&mut self, console_model: ConsoleModel) {
        self.sweep.clock(
            console_model,
            self.nr10,
            &mut self.nr13,
            &mut self.nr14,
            &mut self.pulse.runtime,
        );
    }
}
