use super::super::common::{
    NOISE_CLOCK_SHIFT_MASK, NOISE_CLOCK_SHIFT_SHIFT, NOISE_COUNTER_MASK, NOISE_LFSR_FEEDBACK_BIT,
    NOISE_LFSR_OUTPUT_BIT, NOISE_LFSR_SHORT_WIDTH_FEEDBACK_BIT, NOISE_LFSR_TAP_BIT,
    NOISE_SHORT_WIDTH_BIT, noise_counter_bit, noise_counter_phase_after_trigger,
    noise_counter_timer_reload,
};
use super::super::{ApuCh4Nr43LfsrAction, ApuCh4Nr43LiveWriteCategory, ApuCh4Nr43LiveWriteTrace};
use super::ch4::{Channel4NoiseSignalState, Channel4Nr43LiveWriteState};

pub(super) fn trace_channel4_live_nr43_write(
    runtime_active: bool,
    old_nr43: u8,
    new_nr43: u8,
    noise: &mut Channel4NoiseSignalState,
    nr43_live_write: &mut Channel4Nr43LiveWriteState,
) -> ApuCh4Nr43LiveWriteTrace {
    seed_channel4_noise_counter_phase_if_uninitialized(old_nr43, noise, nr43_live_write);
    Channel4Nr43LiveWriteResolver::new(runtime_active, old_nr43, new_nr43, noise, nr43_live_write)
        .resolve()
}

pub(super) fn resolve_channel4_noise_counter_timer_after_live_write(
    new_nr43: u8,
    nr43_live_write: &Channel4Nr43LiveWriteState,
) -> u32 {
    if !nr43_live_write.countdown_reloaded && nr43_live_write.counter_timer != 0 {
        return nr43_live_write.counter_timer;
    }

    let divisor = decode_nr43_noise_counter_reload(new_nr43);
    if divisor == 2 {
        return divisor;
    }
    divisor + [2, 1, 4, 3][usize::from(nr43_live_write.alignment & 0x03)]
}

pub(super) fn step_channel4_lfsr(noise: &mut Channel4NoiseSignalState) {
    let feedback_bit = u16::from(
        (noise.lfsr_state & (1 << NOISE_LFSR_OUTPUT_BIT))
            == ((noise.lfsr_state >> NOISE_LFSR_TAP_BIT) & (1 << NOISE_LFSR_OUTPUT_BIT)),
    );
    noise.lfsr_state >>= 1;
    noise.lfsr_state = (noise.lfsr_state & !(1 << NOISE_LFSR_FEEDBACK_BIT))
        | (feedback_bit << NOISE_LFSR_FEEDBACK_BIT);
    if noise.short_width_mode {
        // In short mode the feedback path also overwrites bit 6, so a live
        // 15-bit -> 7-bit switch can trap the active 7-bit window at ones
        // until a retrigger reloads the LFSR.
        noise.lfsr_state = (noise.lfsr_state & !(1 << NOISE_LFSR_SHORT_WIDTH_FEEDBACK_BIT))
            | (feedback_bit << NOISE_LFSR_SHORT_WIDTH_FEEDBACK_BIT);
    }
}

fn seed_channel4_noise_counter_phase_if_uninitialized(
    old_nr43: u8,
    noise: &Channel4NoiseSignalState,
    nr43_live_write: &mut Channel4Nr43LiveWriteState,
) {
    if nr43_live_write.counter_timer != 0 || noise.period_timer != 0 {
        return;
    }

    let old_shift = decode_nr43_clock_shift(old_nr43);
    nr43_live_write.noise_counter = noise_counter_phase_after_trigger(old_shift);
    nr43_live_write.counter_timer = noise_counter_timer_reload(noise.clock_divider_code);
}

struct Channel4Nr43LiveWriteResolver<'a> {
    runtime_active: bool,
    old_nr43: u8,
    new_nr43: u8,
    noise: &'a mut Channel4NoiseSignalState,
    nr43_live_write: &'a mut Channel4Nr43LiveWriteState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Channel4SingleIntermediateDecision {
    glitch_value: u8,
    second_glitch_value: u8,
    old_shift: u8,
    new_shift: u8,
    glitch_shift: u8,
    second_glitch_shift: u8,
    old_bit: bool,
    new_bit: bool,
    glitch_bit: bool,
    second_glitch_bit: bool,
    new_short_width_mode: bool,
}

impl Channel4SingleIntermediateDecision {
    fn new(old_nr43: u8, new_nr43: u8, effective_counter: u16) -> Self {
        let old_shift = decode_nr43_clock_shift(old_nr43);
        let new_shift = decode_nr43_clock_shift(new_nr43);
        let glitch_value = (old_nr43 & 0x7F) | (new_nr43 & 0x80);
        let glitch_shift = decode_nr43_clock_shift(glitch_value);
        let second_glitch_value = candidate_second_intermediate_nr43_value(old_nr43, new_nr43);
        let second_glitch_shift = decode_nr43_clock_shift(second_glitch_value);

        Self {
            glitch_value,
            second_glitch_value,
            old_shift,
            new_shift,
            glitch_shift,
            second_glitch_shift,
            old_bit: noise_counter_bit(effective_counter, old_shift),
            new_bit: noise_counter_bit(effective_counter, new_shift),
            glitch_bit: noise_counter_bit(effective_counter, glitch_shift),
            second_glitch_bit: noise_counter_bit(effective_counter, second_glitch_shift),
            new_short_width_mode: new_nr43 & NOISE_SHORT_WIDTH_BIT != 0,
        }
    }

    fn category(self, effective_counter: u16) -> ApuCh4Nr43LiveWriteCategory {
        if self.old_bit == self.new_bit && self.new_bit != self.glitch_bit {
            if self.new_bit {
                ApuCh4Nr43LiveWriteCategory::Category1
            } else {
                ApuCh4Nr43LiveWriteCategory::Category2
            }
        } else if !self.old_bit && self.new_bit {
            ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
        } else if self.new_shift <= 2
            && !self.glitch_bit
            && !self.new_bit
            && !self.old_bit
            && effective_counter & 0x08 != 0
        {
            ApuCh4Nr43LiveWriteCategory::LowShiftFollowup
        } else {
            ApuCh4Nr43LiveWriteCategory::None
        }
    }
}

impl<'a> Channel4Nr43LiveWriteResolver<'a> {
    fn new(
        runtime_active: bool,
        old_nr43: u8,
        new_nr43: u8,
        noise: &'a mut Channel4NoiseSignalState,
        nr43_live_write: &'a mut Channel4Nr43LiveWriteState,
    ) -> Self {
        Self {
            runtime_active,
            old_nr43,
            new_nr43,
            noise,
            nr43_live_write,
        }
    }

    fn resolve(&mut self) -> ApuCh4Nr43LiveWriteTrace {
        let same_shift_group = self.old_nr43 & 0xF0 == self.new_nr43 & 0xF0;
        let old_shift = decode_nr43_clock_shift(self.old_nr43);
        let new_shift = decode_nr43_clock_shift(self.new_nr43);
        let effective_counter = self.effective_noise_counter();
        let lfsr_before = self.noise.lfsr_state;
        let mut trace = ApuCh4Nr43LiveWriteTrace {
            runtime_active: self.runtime_active,
            same_shift_group,
            old_nr43: self.old_nr43,
            new_nr43: self.new_nr43,
            glitch_value: self.old_nr43,
            second_glitch_value: self.old_nr43,
            old_shift,
            glitch_shift: old_shift,
            second_glitch_shift: old_shift,
            new_shift,
            effective_counter,
            countdown_reloaded: self.nr43_live_write.countdown_reloaded,
            old_bit: false,
            glitch_bit: false,
            second_glitch_bit: false,
            new_bit: false,
            decision_category: ApuCh4Nr43LiveWriteCategory::None,
            lfsr_action: ApuCh4Nr43LfsrAction::None,
            reload_seam_step: false,
            old_to_ff_step: false,
            old_to_ff_forced_short_width: false,
            ff_to_new_step: false,
            ff_to_new_forced_short_width: false,
            low_shift_extra_step: false,
            feedback_corruption: false,
            lfsr_before,
            lfsr_after: lfsr_before,
        };

        if !self.runtime_active || same_shift_group {
            return trace;
        }

        if self.nr43_live_write.countdown_reloaded {
            self.apply_reload_seam_glitch(&mut trace);
        }

        self.apply_single_intermediate_glitch(effective_counter, &mut trace);
        trace.lfsr_after = self.noise.lfsr_state;
        trace
    }

    fn effective_noise_counter(&self) -> u16 {
        if self.nr43_live_write.countdown_reloaded {
            self.nr43_live_write.noise_counter
                | self.nr43_live_write.noise_counter.wrapping_sub(1) & NOISE_COUNTER_MASK
        } else {
            self.nr43_live_write.noise_counter
        }
    }

    fn apply_reload_seam_glitch(&mut self, trace: &mut ApuCh4Nr43LiveWriteTrace) {
        let current_counter = self.nr43_live_write.noise_counter;
        let previous_counter = current_counter.wrapping_sub(1) & NOISE_COUNTER_MASK;
        let old_shift = decode_nr43_clock_shift(self.old_nr43);
        let new_shift = decode_nr43_clock_shift(self.new_nr43);
        let current_glitch_bit = noise_counter_bit(current_counter, 7);

        if !noise_counter_bit(current_counter, old_shift)
            && noise_counter_bit(current_counter, new_shift)
            && current_glitch_bit
            && noise_counter_bit(previous_counter, old_shift)
            && !noise_counter_bit(previous_counter, new_shift)
            && noise_counter_bit(previous_counter, 7)
        {
            step_channel4_lfsr(self.noise);
            trace.reload_seam_step = true;
        }
    }

    fn apply_single_intermediate_glitch(
        &mut self,
        effective_counter: u16,
        trace: &mut ApuCh4Nr43LiveWriteTrace,
    ) {
        let decision = Channel4SingleIntermediateDecision::new(
            self.old_nr43,
            self.new_nr43,
            effective_counter,
        );
        trace.glitch_value = decision.glitch_value;
        trace.second_glitch_value = decision.second_glitch_value;
        trace.glitch_shift = decision.glitch_shift;
        trace.second_glitch_shift = decision.second_glitch_shift;
        trace.old_bit = decision.old_bit;
        trace.glitch_bit = decision.glitch_bit;
        trace.second_glitch_bit = decision.second_glitch_bit;
        trace.new_bit = decision.new_bit;
        let category = decision.category(effective_counter);
        self.noise.short_width_mode = decision.new_short_width_mode;

        let action = match category {
            ApuCh4Nr43LiveWriteCategory::None => ApuCh4Nr43LfsrAction::None,
            ApuCh4Nr43LiveWriteCategory::Category1 => {
                self.category_1_action(decision, effective_counter)
            }
            ApuCh4Nr43LiveWriteCategory::Category2 => {
                self.category_2_action(decision, effective_counter)
            }
            ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort => {
                self.rising_edge_forced_short_action(decision, effective_counter)
            }
            ApuCh4Nr43LiveWriteCategory::LowShiftFollowup => {
                self.low_shift_followup_action(decision, effective_counter)
            }
        };
        trace.decision_category = category;
        trace.lfsr_action = action;
        self.apply_lfsr_action(action, trace);
    }

    fn category_1_action(
        &self,
        _decision: Channel4SingleIntermediateDecision,
        _effective_counter: u16,
    ) -> ApuCh4Nr43LfsrAction {
        // In the current DMG/CGB-C-oriented deterministic subset, the richer
        // SameBoy category-1 behavior only appears on later revisions.
        // Keep this path explicit (and currently inert) so future matrix work
        // can deepen it without re-entangling the classifier.
        ApuCh4Nr43LfsrAction::None
    }

    fn category_2_action(
        &self,
        _decision: Channel4SingleIntermediateDecision,
        _effective_counter: u16,
    ) -> ApuCh4Nr43LfsrAction {
        ApuCh4Nr43LfsrAction::PlainStep
    }

    fn rising_edge_forced_short_action(
        &self,
        decision: Channel4SingleIntermediateDecision,
        effective_counter: u16,
    ) -> ApuCh4Nr43LfsrAction {
        if suppress_repo_local_narrow_forced_short_step(self.old_nr43, self.new_nr43) {
            return ApuCh4Nr43LfsrAction::None;
        }

        if decision.new_shift <= 2 && decision.glitch_bit && effective_counter & 0x08 == 0 {
            ApuCh4Nr43LfsrAction::ForcedShortStepThenLowShiftCorruption
        } else {
            ApuCh4Nr43LfsrAction::ForcedShortStep
        }
    }

    fn low_shift_followup_action(
        &self,
        _decision: Channel4SingleIntermediateDecision,
        _effective_counter: u16,
    ) -> ApuCh4Nr43LfsrAction {
        ApuCh4Nr43LfsrAction::PlainStep
    }

    fn apply_lfsr_action(
        &mut self,
        action: ApuCh4Nr43LfsrAction,
        trace: &mut ApuCh4Nr43LiveWriteTrace,
    ) {
        match action {
            ApuCh4Nr43LfsrAction::None => {}
            ApuCh4Nr43LfsrAction::PlainStep => {
                step_channel4_lfsr(self.noise);
                trace.ff_to_new_step = true;
            }
            ApuCh4Nr43LfsrAction::ForcedShortStep => {
                step_channel4_lfsr_with_forced_short_width(self.noise);
                trace.ff_to_new_step = true;
                trace.ff_to_new_forced_short_width = true;
            }
            ApuCh4Nr43LfsrAction::ForcedShortStepThenLowShiftCorruption => {
                step_channel4_lfsr_with_forced_short_width(self.noise);
                trace.ff_to_new_step = true;
                trace.ff_to_new_forced_short_width = true;
                step_channel4_lfsr(self.noise);
                apply_channel4_feedback_bit_corruption(self.noise);
                trace.low_shift_extra_step = true;
                trace.feedback_corruption = true;
            }
        }
    }
}

fn decode_nr43_clock_shift(value: u8) -> u8 {
    (value >> NOISE_CLOCK_SHIFT_SHIFT) & NOISE_CLOCK_SHIFT_MASK
}

fn decode_nr43_noise_counter_reload(value: u8) -> u32 {
    let divisor = u32::from(value & 0x07) << 2;
    if divisor == 0 { 2 } else { divisor }
}

fn suppress_repo_local_narrow_forced_short_step(old_nr43: u8, new_nr43: u8) -> bool {
    if old_nr43 & NOISE_SHORT_WIDTH_BIT == 0 || new_nr43 & NOISE_SHORT_WIDTH_BIT == 0 {
        return false;
    }

    if old_nr43 & 0x0F != 0x0C || new_nr43 & 0x0F != 0x0C {
        return false;
    }

    let old_shift = decode_nr43_clock_shift(old_nr43);
    let new_shift = decode_nr43_clock_shift(new_nr43);
    matches!((old_shift, new_shift), (5, 6) | (5, 4) | (4, 3))
}

fn candidate_second_intermediate_nr43_value(old_nr43: u8, new_nr43: u8) -> u8 {
    if old_nr43 & 0x80 != new_nr43 & 0x80 {
        return (old_nr43 & 0x7F) | (new_nr43 & 0x80);
    }

    // Repo-local staged-upper-nibble candidate used only for observability in
    // the current DMG/CGB-C-oriented subset. This mirrors SameBoy's rougher
    // staged-bit migration idea closely enough to tell us whether bit7-stable
    // writes like Zelda's `0x5C -> 0x6C` tail would actually gain signal from a
    // second intermediate before we let that second stage affect behavior.
    (old_nr43 & 0xCF) | (new_nr43 & 0x30)
}

fn step_channel4_lfsr_with_forced_short_width(noise: &mut Channel4NoiseSignalState) {
    let short_width_mode = noise.short_width_mode;
    noise.short_width_mode = true;
    step_channel4_lfsr(noise);
    noise.short_width_mode = short_width_mode;
}

fn apply_channel4_feedback_bit_corruption(noise: &mut Channel4NoiseSignalState) {
    let feedback_mask = if noise.short_width_mode {
        0x4040
    } else {
        0x4000
    };
    let previous_feedback_mask = if noise.short_width_mode {
        0x2020
    } else {
        0x2000
    };
    noise.lfsr_state &= !feedback_mask;
    noise.lfsr_state |= (noise.lfsr_state & previous_feedback_mask) << 1;
}
