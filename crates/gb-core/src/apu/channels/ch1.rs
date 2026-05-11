use crate::model::ConsoleModel;
use crate::speed::CgbSpeedMode;

use super::super::common::{
    CGB_CH1_SWEEP_DECREASE_RESTART_HOLD_T_CYCLES, CGB_CH1_SWEEP_RESTART_HOLD_T_CYCLES,
    CGB_SWEEP_DELAYED_CALCULATION_MIN_T_CYCLES,
    CGB_SWEEP_DELAYED_CALCULATION_T_CYCLES_PER_SHIFT_STEP,
    CGB_SWEEP_TRIGGER_DELAYED_CALCULATION_EXTRA_T_CYCLES,
    CGB_SWEEP_UNSHIFTED_DELAYED_CALCULATION_T_CYCLES, ChannelRuntimeState,
    DAC_ENABLE_REGISTER_MASK, DMG_SWEEP_RESTART_DELAY_T_CYCLES,
    DMG_SWEEP_TRIGGER_TARGET_COUNTER_BASE, NR10_FORCED_HIGH_MASK, NR10_WRITABLE_MASK,
    NR11_WRITE_ONLY_MASK, NR13_WRITE_ONLY_READ_VALUE, NR14_FORCED_HIGH_MASK, NR14_READ_MASK,
    NRX4_WRITABLE_MASK, PERIOD_HIGH_MASK, PULSE_DUTY_MASK, PULSE_PERIOD_MAX, SWEEP_PHASE_BOUNDARY,
    SWEEP_PHASE_MASK, SWEEP_TIMER_RELOAD, begin_nrx4_write, pulse_period_from_registers,
    sweep_decreases_from_nr10, sweep_pace_from_nr10, sweep_shift_from_nr10,
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
pub(in crate::apu) struct Ch1SweepRecalculation {
    #[serde(default)]
    pub(in crate::apu) countdown: u16,
    #[serde(default)]
    pub(in crate::apu) target_trigger_counter: u8,
    #[serde(default)]
    pub(in crate::apu) trigger_counter: u8,
    #[serde(default)]
    pub(in crate::apu) instant: bool,
    #[serde(default)]
    pub(in crate::apu) increment: u16,
    #[serde(default)]
    pub(in crate::apu) from_trigger: bool,
    #[serde(default)]
    pub(in crate::apu) reload_countdown: u8,
    #[serde(default)]
    pub(in crate::apu) reload_period_reloaded: bool,
    #[serde(default)]
    pub(in crate::apu) reload_period_pending: bool,
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
    pub(in crate::apu) delayed_calculation_t_cycles: u16,
    #[serde(default)]
    delayed_calculation_shadow_period: u16,
    #[serde(default)]
    delayed_calculation_addend: u16,
    #[serde(default)]
    delayed_calculation_decreases: bool,
    #[serde(default)]
    decreasing_writeback_since_trigger: bool,
    #[serde(default)]
    pub(in crate::apu) restart_hold_t_cycles: u16,
    #[serde(default)]
    pub(in crate::apu) recalculation: Ch1SweepRecalculation,
    #[serde(default)]
    pub(in crate::apu) restart_countdown_t_cycles: u16,
    #[serde(default)]
    pub(in crate::apu) period_increment: u16,
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

        if console_model.is_dmg_family() && runtime.active {
            self.enabled =
                sweep_pace_from_nr10(new_nr10) != 0 || sweep_shift_from_nr10(new_nr10) != 0;
            self.apply_dmg_nr10_glitches(old_nr10, new_nr10, nr13, nr14, runtime);
        }

        self.maybe_fire_sweep_boundary(console_model, new_nr10, nr13, nr14, runtime);
    }

    fn apply_dmg_nr10_glitches(
        &mut self,
        old_nr10: u8,
        new_nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        let prev_step = sweep_shift_from_nr10(old_nr10);
        let new_step = sweep_shift_from_nr10(new_nr10);
        let new_decreases = sweep_decreases_from_nr10(new_nr10);

        if !new_decreases {
            let complement_bit: u16 = if self.recalculation.from_trigger {
                0
            } else {
                1
            };
            let candidate = self
                .shadow_period
                .wrapping_add(self.recalculation.increment);
            let candidate = candidate.wrapping_add(complement_bit);
            if candidate > PULSE_PERIOD_MAX {
                runtime.active = false;
            }
        }

        if !runtime.active {
            return;
        }

        if self.recalculation.target_trigger_counter > 0 && self.recalculation.trigger_counter < 2 {
            self.recalculation.countdown = u16::from(new_step);
            if new_step == 0 {
                self.recalculation.trigger_counter = 0;
                self.recalculation.target_trigger_counter = 0;
                self.recalculation.countdown = 0;
            }
        } else if self.recalculation.countdown > 0 && prev_step == 0 && new_step != 0 {
            self.recalculation.countdown -= 1;
            if self.recalculation.countdown == 0 {
                self.period_sweep_recalculation_done(new_nr10, nr13, nr14, runtime);
            }
        }

        if self.enabled && self.phase == SWEEP_PHASE_BOUNDARY {
            self.maybe_fire_sweep_boundary(ConsoleModel::GameBoy, new_nr10, nr13, nr14, runtime);
        }
    }

    fn trigger(
        &mut self,
        console_model: ConsoleModel,
        nr10: u8,
        period_value: u16,
        _runtime: &mut ChannelRuntimeState,
    ) {
        self.shadow_period = period_value;
        self.phase = Self::phase_from_pace(sweep_pace_from_nr10(nr10));
        self.timer = Self::timer_from_phase(self.phase);
        self.enabled = sweep_pace_from_nr10(nr10) != 0 || sweep_shift_from_nr10(nr10) != 0;
        self.completed_addend = 0;
        self.negate_calculated_since_trigger = false;
        self.decreasing_writeback_since_trigger = false;
        self.restart_hold_t_cycles = if console_model.is_cgb_family() {
            CGB_CH1_SWEEP_RESTART_HOLD_T_CYCLES
        } else {
            0
        };
        self.clear_delayed_calculation();

        if console_model.is_dmg_family() {
            self.dmg_apply_trigger_recalculation(nr10);
            self.shadow_period = 0;
        }

        if console_model.is_cgb_family()
            && let Some(calculation) = self.calculate_candidate_sum(nr10, self.shadow_period, false)
        {
            self.observe_calculation(calculation);
            self.schedule_delayed_calculation(
                nr10,
                self.shadow_period,
                calculation,
                CGB_SWEEP_TRIGGER_DELAYED_CALCULATION_EXTRA_T_CYCLES,
            );
        }
    }

    fn dmg_apply_trigger_recalculation(&mut self, nr10: u8) {
        let step = sweep_shift_from_nr10(nr10);
        let prev_target = self.recalculation.target_trigger_counter;
        let prev_trigger_counter = self.recalculation.trigger_counter;
        let prev_countdown_m = self.recalculation.countdown;

        self.recalculation.from_trigger = true;
        self.recalculation.increment = 0;
        self.recalculation.instant = false;
        self.period_increment = 0;

        if prev_target == 0 || prev_trigger_counter == prev_target {
            self.restart_countdown_t_cycles = DMG_SWEEP_RESTART_DELAY_T_CYCLES;
        }

        if step != 0 {
            self.period_increment = self.shadow_period >> step;

            if !(prev_target > 0 && prev_trigger_counter < 2) {
                let mut new_target = DMG_SWEEP_TRIGGER_TARGET_COUNTER_BASE;
                if prev_countdown_m < 2 {
                    new_target = new_target.saturating_add(1);
                }
                if prev_trigger_counter == 2 && prev_countdown_m == u16::from(step) {
                    new_target = new_target.saturating_add(1);
                }
                self.recalculation.target_trigger_counter = new_target;
                self.recalculation.trigger_counter = 0;
            }

            self.recalculation.countdown = u16::from(step);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tick_recalculation(
        &mut self,
        console_model: ConsoleModel,
        nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
        apu_clock: u8,
        t_cycle_phase: u8,
    ) {
        if console_model.is_cgb_family() {
            return;
        }

        let is_tick_odd = t_cycle_phase & 0x01 == 1;
        if !is_tick_odd {
            return;
        }

        self.tick_period_sweep_recalculation(nr10, nr13, nr14, runtime, apu_clock);
        self.tick_period_sweep_reload(nr10, nr13, nr14);
    }

    fn tick_period_sweep_recalculation(
        &mut self,
        nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
        apu_clock: u8,
    ) {
        if self.recalculation.instant {
            self.recalculation.instant = false;
            self.period_sweep_recalculation_done(nr10, nr13, nr14, runtime);
            return;
        }

        if apu_clock & 0x01 == 0 {
            return;
        }

        if self.recalculation.trigger_counter < self.recalculation.target_trigger_counter {
            self.recalculation.trigger_counter += 1;
            return;
        }

        if self.recalculation.countdown == 0 {
            return;
        }

        let step = sweep_shift_from_nr10(nr10);
        if step == 0 {
            return;
        }

        self.recalculation.countdown -= 1;
        if self.recalculation.countdown == 0 {
            self.period_sweep_recalculation_done(nr10, nr13, nr14, runtime);
        }
    }

    fn tick_period_sweep_reload(&mut self, nr10: u8, nr13: &mut u8, nr14: &mut u8) {
        if self.restart_countdown_t_cycles > 0 {
            self.restart_countdown_t_cycles -= 1;
        }
        if self.recalculation.reload_countdown > 0 {
            self.recalculation.reload_countdown -= 1;
            if self.recalculation.reload_countdown == 0 {
                self.period_sweep_reload_done(nr10, nr13, nr14);
            }
        }
    }

    fn period_sweep_reload_done(&mut self, nr10: u8, nr13: &mut u8, nr14: &mut u8) {
        let step = sweep_shift_from_nr10(nr10);
        let decreases = sweep_decreases_from_nr10(nr10);

        let live_period =
            ((u16::from(*nr14) & u16::from(PERIOD_HIGH_MASK)) << 8) | u16::from(*nr13);
        let raw_increment = live_period >> step;
        self.period_increment = raw_increment;
        self.recalculation.increment = if decreases {
            (!raw_increment) & PULSE_PERIOD_MAX
        } else {
            raw_increment
        };
        if self.restart_countdown_t_cycles == 0 {
            if step == 0 {
                self.recalculation.instant = true;
                self.recalculation.countdown = 0;
            } else {
                self.recalculation.countdown = u16::from(step);
            }
        }
        self.recalculation.reload_period_reloaded = false;
        self.recalculation.reload_period_pending = false;
    }

    fn period_sweep_recalculation_done(
        &mut self,
        nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        let live_period =
            ((u16::from(*nr14) & u16::from(PERIOD_HIGH_MASK)) << 8) | u16::from(*nr13);
        self.shadow_period = live_period;
        let raw_increment = self.period_increment;
        let signed_increment = if sweep_decreases_from_nr10(nr10) {
            (!raw_increment) & PULSE_PERIOD_MAX
        } else {
            raw_increment
        };
        self.recalculation.increment = signed_increment;

        if !sweep_decreases_from_nr10(nr10) {
            let complement_bit: u16 = if self.recalculation.from_trigger {
                0
            } else {
                1
            };
            let candidate = live_period
                .wrapping_add(signed_increment)
                .wrapping_add(complement_bit);
            if candidate > PULSE_PERIOD_MAX {
                runtime.active = false;
            }
        }

        if sweep_decreases_from_nr10(nr10) {
            self.negate_calculated_since_trigger = true;
        }

        self.recalculation.from_trigger = false;
    }

    fn tick_delayed_calculation(
        &mut self,
        clock_generation_timer: bool,
        runtime: &mut ChannelRuntimeState,
    ) {
        if !clock_generation_timer {
            return;
        }

        self.restart_hold_t_cycles = self.restart_hold_t_cycles.saturating_sub(1);

        if self.delayed_calculation_t_cycles == 0 {
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
        self.phase = (self.phase + 1) & SWEEP_PHASE_MASK;
        self.timer = Self::timer_from_phase(self.phase);
        if self.phase != SWEEP_PHASE_BOUNDARY {
            return;
        }

        if !self.enabled {
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
        _runtime: &mut ChannelRuntimeState,
    ) {
        let pace = sweep_pace_from_nr10(nr10);
        if self.phase != SWEEP_PHASE_BOUNDARY || pace == 0 || !self.enabled {
            return;
        }

        self.phase = Self::phase_from_pace(pace);
        self.timer = Self::timer_from_phase(self.phase);

        let shift = sweep_shift_from_nr10(nr10);
        if shift == 0 && console_model.is_cgb_family() && self.restart_hold_t_cycles > 0 {
            return;
        }

        let Some(calculation) = self.calculate_candidate_sum(nr10, self.shadow_period, true) else {
            return;
        };

        if console_model.is_cgb_family() {
            self.observe_calculation(calculation);
            if !calculation.decreases
                && calculation.candidate_sum > super::super::common::PULSE_PERIOD_MAX
            {
                self.schedule_delayed_calculation(nr10, self.shadow_period, calculation, 0);
                return;
            }
            if shift == 0 {
                self.schedule_delayed_calculation(nr10, self.shadow_period, calculation, 0);
                return;
            }
            let candidate = calculation.candidate_sum & super::super::common::PULSE_PERIOD_MAX;
            self.shadow_period = candidate;
            *nr13 = candidate as u8;
            *nr14 = (*nr14 & !PERIOD_HIGH_MASK) | (((candidate >> 8) as u8) & PERIOD_HIGH_MASK);
            self.decreasing_writeback_since_trigger |= calculation.decreases;
            if let Some(next_calculation) =
                self.calculate_candidate_sum(nr10, self.shadow_period, true)
            {
                self.observe_calculation(next_calculation);
                self.schedule_delayed_calculation(nr10, self.shadow_period, next_calculation, 0);
            }
            return;
        }

        let decreases = sweep_decreases_from_nr10(nr10);
        let dmg_addend = if decreases {
            (!self.period_increment) & PULSE_PERIOD_MAX
        } else {
            self.period_increment
        };
        let dmg_candidate_sum = self
            .shadow_period
            .wrapping_add(dmg_addend)
            .wrapping_add(u16::from(decreases));
        let dmg_calculation = SweepCalculation {
            candidate_sum: dmg_candidate_sum,
            addend: dmg_addend,
            decreases,
        };
        self.observe_calculation(dmg_calculation);

        if shift == 0 {
            self.recalculation.reload_countdown = 2;
            self.recalculation.reload_period_reloaded = false;
            self.recalculation.reload_period_pending = true;
            return;
        }

        let candidate = dmg_candidate_sum & super::super::common::PULSE_PERIOD_MAX;
        self.shadow_period = candidate;
        *nr13 = candidate as u8;
        *nr14 = (*nr14 & !PERIOD_HIGH_MASK) | (((candidate >> 8) as u8) & PERIOD_HIGH_MASK);
        self.decreasing_writeback_since_trigger |= decreases;

        self.dmg_schedule_recalculation_post_writeback(nr10);
    }

    fn dmg_schedule_recalculation_post_writeback(&mut self, nr10: u8) {
        let _ = nr10;
        if self.recalculation.countdown > 0 {
            self.recalculation.increment = 0;
            self.period_increment = 0;
        }
        // Canonical reload window: 2 M-cycles before the recalculation countdown
        // is potentially reloaded with `step`. During the first M-cycle of this
        // window, NR13/NR14 writes are ignored entirely; during either M-cycle
        // they can re-derive the canonical increment from the new register
        // values (handled in write_nr13 / write_nr14).
        let step = sweep_shift_from_nr10(nr10);
        self.recalculation.reload_countdown = 2;
        self.recalculation.reload_period_reloaded = step != 0;
        self.recalculation.reload_period_pending = true;
    }

    fn schedule_delayed_calculation(
        &mut self,
        nr10: u8,
        shadow_period: u16,
        calculation: SweepCalculation,
        extra_t_cycles: u16,
    ) {
        let shift = sweep_shift_from_nr10(nr10);
        let shift_delay = if shift == 0 {
            CGB_SWEEP_UNSHIFTED_DELAYED_CALCULATION_T_CYCLES
        } else {
            (u16::from(shift + 1) * CGB_SWEEP_DELAYED_CALCULATION_T_CYCLES_PER_SHIFT_STEP)
                .max(CGB_SWEEP_DELAYED_CALCULATION_MIN_T_CYCLES)
        };
        self.delayed_calculation_t_cycles = shift_delay + extra_t_cycles;
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

    #[cfg(test)]
    pub(in crate::apu) fn set_phase_for_test(&mut self, phase: u8) {
        self.phase = phase & SWEEP_PHASE_MASK;
        self.timer = Self::timer_from_phase(self.phase);
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
    /// Test-only mirror of the global Apu sub-cycle scheduler. The unit tests
    /// drive `tick_fast_timer()` directly without going through `Apu::tick_t_cycle`,
    /// so the channel needs its own copy of `apu_clock` and `t_cycle_phase` to
    /// keep the canonical recalc pipeline ticking with sub-M-cycle precision.
    #[cfg(test)]
    #[serde(default)]
    test_apu_clock: u8,
    #[cfg(test)]
    #[serde(default)]
    test_t_cycle_phase: u8,
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
        if self.sweep.recalculation.reload_countdown == 2
            && self.sweep.recalculation.reload_period_reloaded
        {
            self.sweep.recalculation.reload_period_pending = false;
            return;
        }
        self.nr13 = value;
        self.sweep.recalculation.reload_period_pending = false;
    }

    fn write_nr14(
        &mut self,
        value: u8,
        console_model: ConsoleModel,
        speed_mode: CgbSpeedMode,
        next_frame_sequencer_step: u8,
    ) {
        let mut effective_value = value;
        if self.sweep.recalculation.reload_countdown == 2
            && self.sweep.recalculation.reload_period_reloaded
        {
            effective_value = (value & !PERIOD_HIGH_MASK) | (self.nr14 & PERIOD_HIGH_MASK);
        }

        let mut write_plan = begin_nrx4_write(
            &mut self.nr14,
            effective_value,
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
        } else if console_model.is_dmg_family()
            && self.sweep.recalculation.reload_countdown > 0
            && self.sweep.recalculation.reload_period_reloaded
        {
            let step = sweep_shift_from_nr10(self.nr10);
            let live_period = self.period_value();
            let raw_increment = if step == 0 { 0 } else { live_period >> step };
            let decreases = sweep_decreases_from_nr10(self.nr10);
            self.sweep.recalculation.increment = if decreases {
                (!raw_increment) & PULSE_PERIOD_MAX
            } else {
                raw_increment
            };
        }

        self.sweep.recalculation.reload_period_pending = false;

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
        // Simulate the global APU sub-cycle scheduler the same way
        // `Apu::tick_t_cycle_for_speed` would: increment t_cycle_phase per
        // call, and increment apu_clock on the even sub-phases. The channel
        // owns its own copy of these counters so the unit tests can drive the
        // pipeline without wiring in the global APU.
        let t_cycle_phase = self.test_t_cycle_phase;
        if t_cycle_phase & 0x01 == 0 {
            self.test_apu_clock = (self.test_apu_clock + 1) & 0x03;
        }
        self.test_t_cycle_phase = (t_cycle_phase + 1) & 0x03;
        self.tick_fast_timer_with_clock_gate(
            ConsoleModel::GameBoy,
            true,
            self.test_apu_clock,
            t_cycle_phase,
        );
    }

    pub(in crate::apu) fn tick_fast_timer_with_clock_gate(
        &mut self,
        console_model: ConsoleModel,
        clock_period_timer: bool,
        apu_clock: u8,
        t_cycle_phase: u8,
    ) {
        self.sweep
            .tick_delayed_calculation(clock_period_timer, &mut self.pulse.runtime);
        if clock_period_timer {
            let nr10 = self.nr10;
            self.sweep.tick_recalculation(
                console_model,
                nr10,
                &mut self.nr13,
                &mut self.nr14,
                &mut self.pulse.runtime,
                apu_clock,
                t_cycle_phase,
            );
        }
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
