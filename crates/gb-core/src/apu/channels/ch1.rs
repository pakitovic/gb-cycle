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

// DMG canonical recalculation state mirrored from DocBoy's `period_sweep.recalculation`
// struct (see `Apéndice A` of the Phase 1 plan and DocBoy `apu.cpp` lines 397-412).
//
// `countdown` is the M-cycle (encoded in t-cycles for direct ticking from
// `tick_fast_timer_with_clock_gate`) wait until the second overflow check fires.
// `target_trigger_counter` and `trigger_counter` model the post-trigger window
// during which write_nr10 has special semantics. `instant` matches DocBoy's flag
// for step==0 reload paths. `increment` is the canonical 1-complement increment
// that `update_nr10`/`update_nr14` reload from the live NR13/NR14. `from_trigger`
// distinguishes the very first DMG recalculation (where complement_bit is 0)
// from later ones (where complement_bit is forced to 1 on DMG).
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
    // DocBoy `period_sweep.reload` sub-state. After every sweep_done writeback,
    // the canonical hardware waits 2 M-cycles before reloading the increment
    // from the (potentially updated by NR13/NR14) NR14:NR13 and starting the
    // recalculation countdown. NR13/NR14 writes during the first M-cycle of
    // this window (countdown == 2) are ignored entirely; writes during either
    // M-cycle re-derive the next increment from the new register values.
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
    // DocBoy `ch1.period_sweep.increment` (apu.h:393): the raw `period >> step`
    // value latched at trigger and re-derived at every reload_done. This is
    // distinct from `recalculation.increment` (which is the canonical 1's
    // complement signed increment, populated only by recalc_done). The
    // writeback at sweep_done uses THIS field, NOT shadow >> step, so an NR10
    // step change between sweeps doesn't immediately accelerate the period
    // growth -- the next reload_done will refresh it.
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
            // DocBoy gates sweep work only by `nr52.ch1` (channel power); our
            // model has a separate `enabled` flag set at trigger from the
            // initial NR10 pace/shift. Keep it in sync with the live NR10 so a
            // pace 0 -> non-zero transition (which DocBoy treats via glitch 4
            // when pace_countdown was 8) re-arms the sweep boundary path.
            self.enabled =
                sweep_pace_from_nr10(new_nr10) != 0 || sweep_shift_from_nr10(new_nr10) != 0;
            self.apply_dmg_nr10_glitches(old_nr10, new_nr10, nr13, nr14, runtime);
        }

        self.maybe_fire_sweep_boundary(console_model, new_nr10, nr13, nr14, runtime);
    }

    // Canonical DMG NR10 glitches (DocBoy `update_nr10`, lines 1651-1734; SameBoy
    // `Core/apu.c` `square_sweep_calculate_countdown_*`). Runs only while CH1 is
    // active and powered, mirroring the `nr52.ch1` guard in DocBoy. Each glitch is
    // gated by the current state of the recalculation pipeline.
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

        // Glitch 1: writing NR10 with direction=increase (decreases bit clear) on
        // DMG always re-runs the second overflow check using the current canonical
        // increment with complement_bit forced to 1. The first NR10 write after a
        // trigger keeps the trigger-time complement bit (0), modeled via
        // `recalculation.from_trigger`.
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

        // Glitch 2: NR10 written soon after the channel was triggered reloads the
        // recalculation countdown with the new step value. If the new step is zero
        // the recalculation is aborted entirely (forever), matching DocBoy's
        // line 1697-1702 behavior.
        if self.recalculation.target_trigger_counter > 0 && self.recalculation.trigger_counter < 2 {
            self.recalculation.countdown = u16::from(new_step);
            if new_step == 0 {
                self.recalculation.trigger_counter = 0;
                self.recalculation.target_trigger_counter = 0;
                self.recalculation.countdown = 0;
            }
        } else {
            // Glitch 3: when the previous step was 0 and the new step is positive,
            // the recalculation countdown is ticked once. If this brings it to zero
            // the recalculation completes immediately. DocBoy line 1717 (DMG path).
            if self.recalculation.countdown > 0 && prev_step == 0 && new_step != 0 {
                self.recalculation.countdown -= 1;
                if self.recalculation.countdown == 0 {
                    self.period_sweep_recalculation_done(new_nr10, nr13, nr14, runtime);
                }
            }
        }

        // Glitch 4: if the writing happens with a pace_countdown of 8 (mod-8 == 0),
        // the period sweep is ticked as if a DIV-APU sweep edge had fired.
        // gb-cycle models the pace via `phase` (counts down to SWEEP_PHASE_BOUNDARY),
        // so the equivalent guard is `phase == SWEEP_PHASE_BOUNDARY` AND the sweep
        // is enabled. The boundary is the same moment the existing `clock` would
        // call `maybe_fire_sweep_boundary`.
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
            // DocBoy `update_nr14` line 1778 explicitly resets the shadow
            // period to zero on every trigger. The canonical recalculation
            // pipeline reloads shadow from NR14:NR13 inside
            // `period_sweep_recalculation_done`, so any sweep_done firing in
            // the meantime computes its candidate from shadow=0 (and so does
            // not overflow against the just-written NR14:NR13).
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
        // DMG canonical: no synchronous overflow check at trigger time, and no
        // observe_calculation either (which would set
        // negate_calculated_since_trigger and prematurely arm the
        // direction-toggle disable). The first canonical calculation happens
        // inside `period_sweep_recalculation_done` after the trigger
        // countdown elapses; that path observes its own calculation correctly.
    }

    // Trigger-time recalculation initialization (DMG only). Mirrors DocBoy
    // `update_nr14`'s trigger branch (lines 1772-1820): the shadow period and
    // increment registers are reset, `from_trigger` is set, and a new
    // recalculation countdown is staged. The `target_trigger_counter` window
    // (2-4 M-cycles) determines whether subsequent NR10/NR14 writes can stomp
    // on the still-pending countdown.
    fn dmg_apply_trigger_recalculation(&mut self, nr10: u8) {
        let step = sweep_shift_from_nr10(nr10);
        let prev_target = self.recalculation.target_trigger_counter;
        let prev_trigger_counter = self.recalculation.trigger_counter;
        let prev_countdown_m = self.recalculation.countdown;

        // The shadow period and increment registers are reset on every trigger.
        // `recalculation.from_trigger` distinguishes the very first DMG
        // recalculation after a trigger from later ones for `complement_bit`.
        self.recalculation.from_trigger = true;
        self.recalculation.increment = 0;
        self.recalculation.instant = false;
        self.period_increment = 0;

        if prev_target == 0 || prev_trigger_counter == prev_target {
            self.restart_countdown_t_cycles = DMG_SWEEP_RESTART_DELAY_T_CYCLES;
        }

        if step != 0 {
            // DocBoy stores `period >> step` in `ch1.period_sweep.increment`,
            // separate from `recalculation.increment`. The trigger latches the
            // raw increment from the just-written NR14:NR13 (= shadow_period
            // BEFORE we reset it to zero -- so capture it from the live shadow
            // value passed in here).
            self.period_increment = self.shadow_period >> step;

            if prev_target > 0 && prev_trigger_counter < 2 {
                // Within the trigger window: do not reset trigger_counter, but
                // still load the countdown from the new step.
            } else {
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
        // Note: step==0 triggers in DocBoy `update_nr14` (apu.cpp:1792) do NOT
        // touch target_trigger_counter, trigger_counter, or countdown -- the
        // outer `if (nr10.step)` guards the entire reload block. Preserving
        // those values across a step==0 retrigger is canon and matters for
        // tests like `change_period_nr14_during_recalc_step0_*_round2`.
        // Note: `recalc_apu_clock_phase` intentionally is not reset here. DocBoy
        // aligns the recalculation countdown to a free-running global APU clock
        // (not to the trigger time), so the round1/round2 distinction in test
        // ROMs reflects the t-cycle phase the trigger happens to land on.
    }

    // Decrement the recalculation countdown at M-cycle granularity. Called from
    // `tick_fast_timer_with_clock_gate` per t-cycle, this advances the M-cycle
    // alignment phase first; the actual countdown edges fire once every 4
    // t-cycles. DocBoy splits this into two functions (`tick_period_sweep_reload`
    // and `tick_period_sweep_recalculation`) that both run on `tick_odd`, so
    // their counters advance in parallel; we mirror that behavior here.
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

        // DocBoy structures the APU tick into four sub-phases per M-cycle:
        // tick_t0 / tick_t2 (`tick_even`, increments apu_clock) and tick_t1 /
        // tick_t3 (`tick_odd`, runs reload + maybe recalc). The recalculation
        // pipeline only fires on the odd sub-phases (DocBoy `tick_odd` calls
        // tick_period_sweep_recalculation, never `tick_even`).
        let is_tick_odd = t_cycle_phase & 0x01 == 1;
        if !is_tick_odd {
            return;
        }

        // DocBoy `tick_odd` (apu.cpp lines 1137-1140) calls
        // `tick_period_sweep_recalculation` BEFORE `tick_period_sweep_reload`.
        // Mirroring that ordering keeps the canonical pipeline shape aligned
        // with DocBoy even though our apu_clock alignment makes the same-tick
        // recalc-after-reload fall-through observe identical behavior on the
        // round1/round2 boundary tests we currently fail (the recalc tick
        // skips on its apu_clock check at reload_done's tick in either order).
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

        // Gated on `test_bit<0>(apu_clock)` per DocBoy apu.cpp line 1372.
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
            // Recalculation is paused while step is 0 (DocBoy line 1379-1385).
            return;
        }

        self.recalculation.countdown -= 1;
        if self.recalculation.countdown == 0 {
            self.period_sweep_recalculation_done(nr10, nr13, nr14, runtime);
        }
    }

    fn tick_period_sweep_reload(&mut self, nr10: u8, nr13: &mut u8, nr14: &mut u8) {
        // Decrements restart_countdown / reload_countdown by 1 per call. Mirror
        // of DocBoy `tick_period_sweep_reload` (apu.cpp:1355).
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

    // Canonical DocBoy `period_sweep_reload_done` (apu.cpp line 1422). After the
    // 2 M-cycle reload window expires, the canonical 1-complement increment is
    // re-derived from the (potentially updated) NR14:NR13, and the recalculation
    // countdown is loaded with `nr10.step` M-cycles. For step==0 the
    // recalculation completes instantly on the next tick.
    fn period_sweep_reload_done(&mut self, nr10: u8, nr13: &mut u8, nr14: &mut u8) {
        let step = sweep_shift_from_nr10(nr10);
        let decreases = sweep_decreases_from_nr10(nr10);

        let live_period =
            ((u16::from(*nr14) & u16::from(PERIOD_HIGH_MASK)) << 8) | u16::from(*nr13);
        // DocBoy line 1444 always reloads `period_sweep.increment = period >> step`
        // (raw, not signed). For step=0 that is `period` itself, which primes
        // recalculation_done with the DMG glitch increment.
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

    // Canonical DocBoy `period_sweep_recalculation_done` (apu.cpp line 1451): the
    // shadow period is reloaded from the live NR14:NR13, the canonical increment
    // is recomputed, and (if the direction is increasing) a final overflow check
    // is performed using the DMG-specific complement_bit selection.
    fn period_sweep_recalculation_done(
        &mut self,
        nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        // DocBoy `period_sweep_recalculation_done` (apu.cpp:1453) reloads the
        // shadow period from the live NR14:NR13 first, before recomputing the
        // canonical signed increment.
        let live_period =
            ((u16::from(*nr14) & u16::from(PERIOD_HIGH_MASK)) << 8) | u16::from(*nr13);
        self.shadow_period = live_period;
        // DocBoy `compute_period_sweep_signed_increment` (apu.cpp:1501) returns
        // `ch1.period_sweep.increment ^ (decrease ? 0x7FF : 0)`. The input is
        // the persistent `period_increment` field (set at trigger or
        // reload_done), NOT a fresh `shadow >> step`. This matters when NR10
        // step changes between reload_done and recalc_done -- the latched
        // increment uses the step that was active when it was last loaded.
        let raw_increment = self.period_increment;
        let signed_increment = if sweep_decreases_from_nr10(nr10) {
            (!raw_increment) & PULSE_PERIOD_MAX
        } else {
            raw_increment
        };
        self.recalculation.increment = signed_increment;

        if !sweep_decreases_from_nr10(nr10) {
            // DMG glitch: the complement_bit is forced to 1 unless this is the
            // very first recalculation following a trigger.
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

        // Pan Docs documents a DMG glitch where toggling NR10 from decrease to
        // increase disables CH1, but only if a calculation has been made since
        // the channel was triggered. Recalculation_done IS that canonical
        // calculation, so latch the flag here when the active direction is
        // negative -- subsequent NR10 writes that flip direction will then see
        // a "calc happened" and disable correctly.
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
        // SameBoy's `square_sweep_countdown` advances at every FS sweep edge
        // regardless of pace (apu.c:756-760). Mirroring that — phase tracks
        // independent of `enabled` — keeps the post-trigger sweep window
        // aligned for tests like `change_pace_0_to_2_step0_delayed_*`, where a
        // pace 0 -> non-zero NR10 write must not land on phase=BOUNDARY
        // because the FS edges between trigger and the write have already
        // moved phase off BOUNDARY. The boundary fire below still gates on
        // `enabled` and `pace != 0`.
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

        // DMG canonical (DocBoy `period_sweep_done`): no inline overflow check.
        // The writeback uses the latched `period_increment` (set at the most
        // recent trigger or reload_done), NOT shadow >> step -- this preserves
        // the canonical glitch where an NR10 step change between sweeps
        // doesn't immediately accelerate the period growth.
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
            // DocBoy `period_sweep_done` line 1402: skip writeback for step==0
            // but still arm the reload window so reload_done eventually fires
            // with `instant=true` (gated on restart_countdown==0).
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

        // DMG canonical: the second overflow check is deferred via the
        // recalculation countdown. The reload window will refresh
        // period_increment from the post-writeback NR14:NR13.
        self.dmg_schedule_recalculation_post_writeback(nr10);
    }

    fn dmg_schedule_recalculation_post_writeback(&mut self, nr10: u8) {
        // Per DocBoy `period_sweep_done` (apu.cpp lines 1390-1419): if a
        // recalculation countdown is already pending (typically from a recent
        // trigger), the canonical "increment is reset to zero" glitch fires
        // here -- both `period_sweep.increment` (our `period_increment`) and
        // `recalculation.increment` are zeroed so the next reload_done /
        // recalc_done re-derive them from the live NR14:NR13. The actual
        // recalculation countdown is left alone; it will either be reloaded
        // by reload_done (gated on restart_countdown == 0) or continue ticking.
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
        // Glitch (DocBoy `update_nr13`, lines 1744-1755): NR13 writes during the
        // first M-cycle of the post-writeback reload window are dropped. The
        // pending second writeback is also aborted regardless.
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
        // Glitch (DocBoy `update_nr14`, lines 1762-1768): when NR14 is written
        // during the first M-cycle of the post-writeback reload window, the
        // period_high bits are dropped (the rest of the value is preserved so
        // the trigger and length-enable bits still apply).
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
            // Glitch (DocBoy `update_nr14`, lines 1822-1827): non-trigger NR14
            // writes during the reload window re-derive the canonical increment
            // from the freshly-written NR14:NR13.
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

        // Aborts any pending second period writeback (DocBoy line 1831).
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
            // DMG canonical recalculation pipeline: gated on the global APU
            // sub-cycle scheduler so the recalc countdown ticks at the same
            // moments DocBoy's `tick_period_sweep_recalculation` would (t1 or
            // t3 of each M-cycle, gated by `test_bit<0>(apu_clock)`).
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
