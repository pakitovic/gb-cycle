use super::super::common::{
    NOISE_CLOCK_SHIFT_MASK, NOISE_CLOCK_SHIFT_SHIFT, NOISE_COUNTER_MASK, NOISE_LFSR_FEEDBACK_BIT,
    NOISE_LFSR_OUTPUT_BIT, NOISE_LFSR_SHORT_WIDTH_FEEDBACK_BIT, NOISE_LFSR_TAP_BIT,
    NOISE_SHORT_WIDTH_BIT, noise_counter_bit,
};
use super::super::{
    ApuCh4Nr43LfsrAction, ApuCh4Nr43LiveWriteCategory, ApuCh4Nr43LiveWriteTrace,
    ApuCh4Nr43PassKind, ApuCh4Nr43PassTrace,
};
use super::ch4::{Channel4NoiseSignalState, Channel4Nr43LiveWriteState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Channel4Nr43LiveWriteProfile {
    DmgPreCgbD,
    CgbDirect,
}

pub(super) fn trace_channel4_live_nr43_write(
    profile: Channel4Nr43LiveWriteProfile,
    runtime_active: bool,
    old_nr43: u8,
    new_nr43: u8,
    noise: &mut Channel4NoiseSignalState,
    nr43_live_write: &mut Channel4Nr43LiveWriteState,
) -> ApuCh4Nr43LiveWriteTrace {
    seed_channel4_noise_counter_phase_if_uninitialized(old_nr43, noise, nr43_live_write);
    Channel4Nr43LiveWriteResolver::new(
        profile,
        runtime_active,
        old_nr43,
        new_nr43,
        noise,
        nr43_live_write,
    )
    .resolve()
}

pub(super) fn resolve_channel4_noise_counter_timer_after_live_write(
    profile: Channel4Nr43LiveWriteProfile,
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
    let alignment_offsets = match profile {
        Channel4Nr43LiveWriteProfile::DmgPreCgbD => [2, 1, 4, 3],
        Channel4Nr43LiveWriteProfile::CgbDirect => [2, 1, 0, 3],
    };
    divisor + alignment_offsets[usize::from(nr43_live_write.alignment & 0x03)]
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
        noise.lfsr_state = (noise.lfsr_state & !(1 << NOISE_LFSR_SHORT_WIDTH_FEEDBACK_BIT))
            | (feedback_bit << NOISE_LFSR_SHORT_WIDTH_FEEDBACK_BIT);
    }
}

fn seed_channel4_noise_counter_phase_if_uninitialized(
    _old_nr43: u8,
    _noise: &Channel4NoiseSignalState,
    _nr43_live_write: &mut Channel4Nr43LiveWriteState,
) {
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Channel4Stage {
    value: u8,
    shift: u8,
    bit: bool,
    short_width_mode: bool,
}

impl Channel4Stage {
    fn from_nr43(value: u8, effective_counter: u16) -> Self {
        let shift = decode_nr43_clock_shift(value);
        Self {
            value,
            shift,
            bit: noise_counter_bit(effective_counter, shift),
            short_width_mode: value & NOISE_SHORT_WIDTH_BIT != 0,
        }
    }

    fn synthetic_ff(effective_counter: u16) -> Self {
        Self {
            value: 0xFF,
            shift: 15,
            bit: noise_counter_bit(effective_counter, 15),
            short_width_mode: true,
        }
    }
}

struct Channel4Nr43LiveWriteResolver<'a> {
    profile: Channel4Nr43LiveWriteProfile,
    runtime_active: bool,
    old_nr43: u8,
    new_nr43: u8,
    noise: &'a mut Channel4NoiseSignalState,
    nr43_live_write: &'a mut Channel4Nr43LiveWriteState,
}

impl<'a> Channel4Nr43LiveWriteResolver<'a> {
    fn new(
        profile: Channel4Nr43LiveWriteProfile,
        runtime_active: bool,
        old_nr43: u8,
        new_nr43: u8,
        noise: &'a mut Channel4NoiseSignalState,
        nr43_live_write: &'a mut Channel4Nr43LiveWriteState,
    ) -> Self {
        Self {
            profile,
            runtime_active,
            old_nr43,
            new_nr43,
            noise,
            nr43_live_write,
        }
    }

    fn resolve(&mut self) -> ApuCh4Nr43LiveWriteTrace {
        match self.profile {
            Channel4Nr43LiveWriteProfile::DmgPreCgbD => self.resolve_dmg_pre_cgb_d(),
            Channel4Nr43LiveWriteProfile::CgbDirect => self.resolve_cgb_direct(),
        }
    }

    fn resolve_dmg_pre_cgb_d(&mut self) -> ApuCh4Nr43LiveWriteTrace {
        let same_shift_group = self.old_nr43 & 0xF0 == self.new_nr43 & 0xF0;
        let effective_counter = self.effective_noise_counter();
        let old_stage = Channel4Stage::from_nr43(self.old_nr43, effective_counter);
        let ff_stage = Channel4Stage::synthetic_ff(effective_counter);
        let glitch_1_stage = Channel4Stage::from_nr43(
            first_glitch_value(ff_stage.value, self.new_nr43),
            effective_counter,
        );
        let glitch_2_stage = second_glitch_value(ff_stage.value, self.new_nr43)
            .map(|value| Channel4Stage::from_nr43(value, effective_counter));
        let new_stage = Channel4Stage::from_nr43(self.new_nr43, effective_counter);
        let lfsr_before = self.noise.lfsr_state;

        let mut trace = ApuCh4Nr43LiveWriteTrace {
            runtime_active: self.runtime_active,
            same_shift_group,
            old_nr43: self.old_nr43,
            ff_value: ff_stage.value,
            glitch_1_value: glitch_1_stage.value,
            glitch_2_value: glitch_2_stage.map(|stage| stage.value),
            old_shift: old_stage.shift,
            ff_shift: ff_stage.shift,
            glitch_1_shift: glitch_1_stage.shift,
            glitch_2_shift: glitch_2_stage.map(|stage| stage.shift),
            new_shift: new_stage.shift,
            new_nr43: self.new_nr43,
            effective_counter,
            countdown_reloaded: self.nr43_live_write.countdown_reloaded,
            old_bit: old_stage.bit,
            ff_bit: ff_stage.bit,
            glitch_1_bit: glitch_1_stage.bit,
            glitch_2_bit: glitch_2_stage.map(|stage| stage.bit),
            new_bit: new_stage.bit,
            decision_category: ApuCh4Nr43LiveWriteCategory::None,
            lfsr_action: ApuCh4Nr43LfsrAction::None,
            reload_seam: None,
            old_to_ff: None,
            ff_to_glitch_1: None,
            glitch_1_to_glitch_2: None,
            glitch_to_new: None,
            low_shift_followup: None,
            lfsr_before,
            lfsr_after: lfsr_before,
        };

        if !self.runtime_active || same_shift_group {
            return trace;
        }

        if self.nr43_live_write.countdown_reloaded {
            trace.reload_seam = Some(self.apply_reload_seam_pass(old_stage));
        }

        let old_to_ff_action =
            self.resolve_basic_live_write_transition(old_stage, ff_stage, effective_counter);
        let ff_to_new_action =
            self.resolve_basic_live_write_transition(ff_stage, new_stage, effective_counter);

        trace.old_to_ff = Some(self.materialize_actionable_pass_trace(
            ApuCh4Nr43PassKind::OldToFf,
            old_stage,
            ff_stage,
            old_to_ff_action,
            ff_stage.short_width_mode,
        ));
        trace.ff_to_glitch_1 = Some(self.materialize_actionable_pass_trace(
            ApuCh4Nr43PassKind::FfToGlitch1,
            ff_stage,
            glitch_1_stage,
            ff_to_new_action,
            new_stage.short_width_mode,
        ));
        trace.glitch_1_to_glitch_2 = glitch_2_stage.map(|stage| {
            self.materialize_descriptive_pass_trace(
                ApuCh4Nr43PassKind::Glitch1ToGlitch2,
                glitch_1_stage,
                stage,
            )
        });
        let pre_new_stage = glitch_2_stage.unwrap_or(glitch_1_stage);
        trace.glitch_to_new = Some(self.materialize_descriptive_pass_trace(
            ApuCh4Nr43PassKind::GlitchToNew,
            pre_new_stage,
            new_stage,
        ));

        if ff_to_new_action.low_shift_followup {
            trace.low_shift_followup = Some(self.execute_low_shift_followup_pass(new_stage));
        }

        self.noise.short_width_mode = new_stage.short_width_mode;
        trace.lfsr_after = self.noise.lfsr_state;
        let (decision_category, lfsr_action) = derive_compatibility_aliases(&trace);
        trace.decision_category = decision_category;
        trace.lfsr_action = lfsr_action;
        trace
    }

    fn resolve_cgb_direct(&mut self) -> ApuCh4Nr43LiveWriteTrace {
        let same_shift_group = self.old_nr43 & 0xF0 == self.new_nr43 & 0xF0;
        let effective_counter = self.nr43_live_write.noise_counter;
        let old_stage = Channel4Stage::from_nr43(self.old_nr43, effective_counter);
        let ff_stage = Channel4Stage::synthetic_ff(effective_counter);
        let glitch_stage = Channel4Stage::from_nr43(
            first_glitch_value(self.old_nr43, self.new_nr43),
            effective_counter,
        );
        let new_stage = Channel4Stage::from_nr43(self.new_nr43, effective_counter);
        let lfsr_before = self.noise.lfsr_state;

        let mut trace = ApuCh4Nr43LiveWriteTrace {
            runtime_active: self.runtime_active,
            same_shift_group,
            old_nr43: self.old_nr43,
            ff_value: ff_stage.value,
            glitch_1_value: glitch_stage.value,
            glitch_2_value: None,
            old_shift: old_stage.shift,
            ff_shift: ff_stage.shift,
            glitch_1_shift: glitch_stage.shift,
            glitch_2_shift: None,
            new_shift: new_stage.shift,
            new_nr43: self.new_nr43,
            effective_counter,
            countdown_reloaded: self.nr43_live_write.countdown_reloaded,
            old_bit: old_stage.bit,
            ff_bit: ff_stage.bit,
            glitch_1_bit: glitch_stage.bit,
            glitch_2_bit: None,
            new_bit: new_stage.bit,
            decision_category: ApuCh4Nr43LiveWriteCategory::None,
            lfsr_action: ApuCh4Nr43LfsrAction::None,
            reload_seam: None,
            old_to_ff: None,
            ff_to_glitch_1: None,
            glitch_1_to_glitch_2: None,
            glitch_to_new: None,
            low_shift_followup: None,
            lfsr_before,
            lfsr_after: lfsr_before,
        };

        if !self.runtime_active || same_shift_group {
            self.noise.short_width_mode = new_stage.short_width_mode;
            return trace;
        }

        let direct_action =
            self.resolve_basic_live_write_transition(old_stage, new_stage, effective_counter);
        trace.glitch_to_new = Some(self.materialize_actionable_pass_trace(
            ApuCh4Nr43PassKind::GlitchToNew,
            old_stage,
            new_stage,
            direct_action,
            new_stage.short_width_mode,
        ));

        self.noise.short_width_mode = new_stage.short_width_mode;
        trace.lfsr_after = self.noise.lfsr_state;
        let (decision_category, lfsr_action) = derive_compatibility_aliases(&trace);
        trace.decision_category = decision_category;
        trace.lfsr_action = lfsr_action;
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

    fn apply_reload_seam_pass(&mut self, old_stage: Channel4Stage) -> ApuCh4Nr43PassTrace {
        let current_counter = self.nr43_live_write.noise_counter;
        let previous_counter = current_counter.wrapping_sub(1) & NOISE_COUNTER_MASK;
        let current_glitch_bit = noise_counter_bit(current_counter, 7);
        let new_shift = decode_nr43_clock_shift(self.new_nr43);
        let lfsr_before = self.noise.lfsr_state;
        let should_step = !noise_counter_bit(current_counter, old_stage.shift)
            && noise_counter_bit(current_counter, new_shift)
            && current_glitch_bit
            && noise_counter_bit(previous_counter, old_stage.shift)
            && !noise_counter_bit(previous_counter, new_shift)
            && noise_counter_bit(previous_counter, 7);
        let action = if should_step {
            step_channel4_lfsr(self.noise);
            ApuCh4Nr43LfsrAction::PlainStep
        } else {
            ApuCh4Nr43LfsrAction::None
        };

        ApuCh4Nr43PassTrace {
            kind: ApuCh4Nr43PassKind::ReloadSeam,
            value_from: self.old_nr43,
            value_to: self.old_nr43,
            shift_from: old_stage.shift,
            shift_to: old_stage.shift,
            bit_from: old_stage.bit,
            bit_to: old_stage.bit,
            category: ApuCh4Nr43LiveWriteCategory::None,
            action,
            lfsr_before,
            lfsr_after: self.noise.lfsr_state,
        }
    }

    fn resolve_basic_live_write_transition(
        &self,
        old_stage: Channel4Stage,
        new_stage: Channel4Stage,
        effective_counter: u16,
    ) -> Channel4ActualWriteResolution {
        let glitch_stage = Channel4Stage::from_nr43(
            first_glitch_value(old_stage.value, new_stage.value),
            effective_counter,
        );

        if old_stage.bit == new_stage.bit && new_stage.bit != glitch_stage.bit {
            if new_stage.bit {
                Channel4ActualWriteResolution {
                    category: ApuCh4Nr43LiveWriteCategory::Category1,
                    action: ApuCh4Nr43LfsrAction::None,
                    low_shift_followup: false,
                }
            } else {
                Channel4ActualWriteResolution {
                    category: ApuCh4Nr43LiveWriteCategory::Category2,
                    action: ApuCh4Nr43LfsrAction::PlainStep,
                    low_shift_followup: false,
                }
            }
        } else if !old_stage.bit && new_stage.bit {
            Channel4ActualWriteResolution {
                category: ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
                action: if new_stage.shift <= 2 && glitch_stage.bit && effective_counter & 0x08 == 0
                {
                    ApuCh4Nr43LfsrAction::ForcedShortStepThenLowShiftCorruption
                } else {
                    ApuCh4Nr43LfsrAction::ForcedShortStep
                },
                low_shift_followup: false,
            }
        } else {
            Channel4ActualWriteResolution {
                category: ApuCh4Nr43LiveWriteCategory::None,
                action: ApuCh4Nr43LfsrAction::None,
                low_shift_followup: new_stage.shift <= 2
                    && !glitch_stage.bit
                    && !new_stage.bit
                    && !old_stage.bit
                    && effective_counter & 0x08 != 0,
            }
        }
    }

    fn execute_low_shift_followup_pass(&mut self, new_stage: Channel4Stage) -> ApuCh4Nr43PassTrace {
        self.materialize_actionable_pass_trace(
            ApuCh4Nr43PassKind::LowShiftFollowup,
            new_stage,
            new_stage,
            Channel4ActualWriteResolution {
                category: ApuCh4Nr43LiveWriteCategory::LowShiftFollowup,
                action: ApuCh4Nr43LfsrAction::PlainStep,
                low_shift_followup: true,
            },
            new_stage.short_width_mode,
        )
    }

    fn materialize_actionable_pass_trace(
        &mut self,
        kind: ApuCh4Nr43PassKind,
        from: Channel4Stage,
        to: Channel4Stage,
        resolution: Channel4ActualWriteResolution,
        action_short_width_mode: bool,
    ) -> ApuCh4Nr43PassTrace {
        let lfsr_before = self.noise.lfsr_state;
        self.noise.short_width_mode = action_short_width_mode;
        self.apply_lfsr_action(resolution.action);
        ApuCh4Nr43PassTrace {
            kind,
            value_from: from.value,
            value_to: to.value,
            shift_from: from.shift,
            shift_to: to.shift,
            bit_from: from.bit,
            bit_to: to.bit,
            category: resolution.category,
            action: resolution.action,
            lfsr_before,
            lfsr_after: self.noise.lfsr_state,
        }
    }

    fn materialize_descriptive_pass_trace(
        &self,
        kind: ApuCh4Nr43PassKind,
        from: Channel4Stage,
        to: Channel4Stage,
    ) -> ApuCh4Nr43PassTrace {
        ApuCh4Nr43PassTrace {
            kind,
            value_from: from.value,
            value_to: to.value,
            shift_from: from.shift,
            shift_to: to.shift,
            bit_from: from.bit,
            bit_to: to.bit,
            category: classify_adjacent_pass(from, to),
            action: ApuCh4Nr43LfsrAction::None,
            lfsr_before: self.noise.lfsr_state,
            lfsr_after: self.noise.lfsr_state,
        }
    }

    fn apply_lfsr_action(&mut self, action: ApuCh4Nr43LfsrAction) {
        match action {
            ApuCh4Nr43LfsrAction::None => {}
            ApuCh4Nr43LfsrAction::PlainStep => step_channel4_lfsr(self.noise),
            ApuCh4Nr43LfsrAction::ForcedShortStep => {
                step_channel4_lfsr_with_forced_short_width(self.noise)
            }
            ApuCh4Nr43LfsrAction::ForcedShortStepThenLowShiftCorruption => {
                step_channel4_lfsr_with_forced_short_width(self.noise);
                step_channel4_lfsr(self.noise);
                apply_channel4_feedback_bit_corruption(self.noise);
            }
            ApuCh4Nr43LfsrAction::StepThenAndPrevious => {
                let previous_lfsr = self.noise.lfsr_state;
                step_channel4_lfsr(self.noise);
                self.noise.lfsr_state &= previous_lfsr | 1;
            }
            ApuCh4Nr43LfsrAction::StepThenSetFeedbackBits => {
                step_channel4_lfsr(self.noise);
                set_channel4_feedback_bits(self.noise);
            }
            ApuCh4Nr43LfsrAction::SetFeedbackBits => {
                set_channel4_feedback_bits(self.noise);
            }
        }
    }
}

fn derive_compatibility_aliases(
    trace: &ApuCh4Nr43LiveWriteTrace,
) -> (ApuCh4Nr43LiveWriteCategory, ApuCh4Nr43LfsrAction) {
    let ordered_passes = [
        trace.low_shift_followup,
        trace.ff_to_glitch_1,
        trace.old_to_ff,
        trace.glitch_to_new,
        trace.glitch_1_to_glitch_2,
        trace.reload_seam,
    ];

    for pass in ordered_passes.into_iter().flatten() {
        if pass.action != ApuCh4Nr43LfsrAction::None {
            return (pass.category, pass.action);
        }
    }

    (
        ApuCh4Nr43LiveWriteCategory::None,
        ApuCh4Nr43LfsrAction::None,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Channel4ActualWriteResolution {
    category: ApuCh4Nr43LiveWriteCategory,
    action: ApuCh4Nr43LfsrAction,
    low_shift_followup: bool,
}

fn classify_adjacent_pass(from: Channel4Stage, to: Channel4Stage) -> ApuCh4Nr43LiveWriteCategory {
    if from.bit && !to.bit {
        if to.value & 0x80 != 0 {
            ApuCh4Nr43LiveWriteCategory::Category1
        } else {
            ApuCh4Nr43LiveWriteCategory::Category2
        }
    } else if !from.bit && to.bit {
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    } else {
        ApuCh4Nr43LiveWriteCategory::None
    }
}

fn decode_nr43_clock_shift(value: u8) -> u8 {
    (value >> NOISE_CLOCK_SHIFT_SHIFT) & NOISE_CLOCK_SHIFT_MASK
}

fn decode_nr43_noise_counter_reload(value: u8) -> u32 {
    let divisor = u32::from(value & 0x07) << 2;
    if divisor == 0 { 2 } else { divisor }
}

fn first_glitch_value(old_nr43: u8, new_nr43: u8) -> u8 {
    (old_nr43 & 0x7F) | (new_nr43 & 0x80)
}

fn second_glitch_value(old_nr43: u8, new_nr43: u8) -> Option<u8> {
    if old_nr43 & 0x80 != new_nr43 & 0x80 {
        return None;
    }

    Some((old_nr43 & 0xCF) | (new_nr43 & 0x30))
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

fn set_channel4_feedback_bits(noise: &mut Channel4NoiseSignalState) {
    let feedback_mask = if noise.short_width_mode {
        0x4040
    } else {
        0x4000
    };
    noise.lfsr_state |= feedback_mask;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolver_with_lfsr(
        lfsr_state: u16,
        short_width_mode: bool,
    ) -> Channel4Nr43LiveWriteResolver<'static> {
        let noise = Box::leak(Box::new(Channel4NoiseSignalState {
            clock_shift: 0,
            short_width_mode,
            clock_divider_code: 0,
            period_timer: 0,
            lfsr_state,
        }));
        let live = Box::leak(Box::new(Channel4Nr43LiveWriteState::default()));
        Channel4Nr43LiveWriteResolver::new(
            Channel4Nr43LiveWriteProfile::DmgPreCgbD,
            false,
            0,
            0,
            noise,
            live,
        )
    }

    #[test]
    fn richer_lfsr_actions_cover_feedback_bit_variants() {
        let mut set_feedback = resolver_with_lfsr(0x0001, false);
        set_feedback.apply_lfsr_action(ApuCh4Nr43LfsrAction::SetFeedbackBits);
        assert_eq!(set_feedback.noise.lfsr_state, 0x4001);

        let mut step_then_set_feedback = resolver_with_lfsr(0x4001, true);
        let previous = step_then_set_feedback.noise.lfsr_state;
        step_then_set_feedback.apply_lfsr_action(ApuCh4Nr43LfsrAction::StepThenSetFeedbackBits);
        assert_ne!(step_then_set_feedback.noise.lfsr_state, previous);
        assert_ne!(step_then_set_feedback.noise.lfsr_state & 0x4040, 0);

        let mut step_then_and_previous = resolver_with_lfsr(0x2000, false);
        let before_and = step_then_and_previous.noise.lfsr_state;
        step_then_and_previous.apply_lfsr_action(ApuCh4Nr43LfsrAction::StepThenAndPrevious);
        assert_ne!(step_then_and_previous.noise.lfsr_state, before_and);
        assert_eq!(step_then_and_previous.noise.lfsr_state & !before_and, 0);
    }

    #[test]
    fn sameboy_zelda_tail_staircase_matches_expected_pass_actions() {
        #[derive(Clone, Copy)]
        struct Case {
            old: u8,
            new: u8,
            stored_counter: u16,
            countdown_reloaded: bool,
            expected_effective_counter: u16,
            old_to_ff: (ApuCh4Nr43LiveWriteCategory, ApuCh4Nr43LfsrAction),
            ff_to_new: (ApuCh4Nr43LiveWriteCategory, ApuCh4Nr43LfsrAction),
            low_shift_followup: Option<(ApuCh4Nr43LiveWriteCategory, ApuCh4Nr43LfsrAction)>,
        }

        let cases = [
            Case {
                old: 0x03,
                new: 0x2C,
                stored_counter: 0x2078,
                countdown_reloaded: false,
                expected_effective_counter: 0x2078,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                low_shift_followup: Some((
                    ApuCh4Nr43LiveWriteCategory::LowShiftFollowup,
                    ApuCh4Nr43LfsrAction::PlainStep,
                )),
            },
            Case {
                old: 0x2C,
                new: 0x3C,
                stored_counter: 0x290B,
                countdown_reloaded: false,
                expected_effective_counter: 0x290B,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
                    ApuCh4Nr43LfsrAction::ForcedShortStep,
                ),
                low_shift_followup: None,
            },
            Case {
                old: 0x3C,
                new: 0x4C,
                stored_counter: 0x319E,
                countdown_reloaded: true,
                expected_effective_counter: 0x319F,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
                    ApuCh4Nr43LfsrAction::ForcedShortStep,
                ),
                low_shift_followup: None,
            },
            Case {
                old: 0x4C,
                new: 0x5C,
                stored_counter: 0x3A31,
                countdown_reloaded: true,
                expected_effective_counter: 0x3A31,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
                    ApuCh4Nr43LfsrAction::ForcedShortStep,
                ),
                low_shift_followup: None,
            },
            Case {
                old: 0x5C,
                new: 0x6C,
                stored_counter: 0x02C3,
                countdown_reloaded: false,
                expected_effective_counter: 0x02C3,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
                    ApuCh4Nr43LfsrAction::ForcedShortStep,
                ),
                low_shift_followup: None,
            },
            Case {
                old: 0x6C,
                new: 0x7C,
                stored_counter: 0x0B55,
                countdown_reloaded: false,
                expected_effective_counter: 0x0B55,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                low_shift_followup: None,
            },
            Case {
                old: 0x7C,
                new: 0x6C,
                stored_counter: 0x13E8,
                countdown_reloaded: false,
                expected_effective_counter: 0x13E8,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
                    ApuCh4Nr43LfsrAction::ForcedShortStep,
                ),
                low_shift_followup: None,
            },
            Case {
                old: 0x6C,
                new: 0x5C,
                stored_counter: 0x1C7A,
                countdown_reloaded: false,
                expected_effective_counter: 0x1C7A,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
                    ApuCh4Nr43LfsrAction::ForcedShortStep,
                ),
                low_shift_followup: None,
            },
            Case {
                old: 0x5C,
                new: 0x4C,
                stored_counter: 0x250D,
                countdown_reloaded: false,
                expected_effective_counter: 0x250D,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::Category2,
                    ApuCh4Nr43LfsrAction::PlainStep,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                low_shift_followup: None,
            },
            Case {
                old: 0x4C,
                new: 0x3C,
                stored_counter: 0x2D9F,
                countdown_reloaded: false,
                expected_effective_counter: 0x2D9F,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
                    ApuCh4Nr43LfsrAction::ForcedShortStep,
                ),
                low_shift_followup: None,
            },
            Case {
                old: 0x3C,
                new: 0x09,
                stored_counter: 0x3632,
                countdown_reloaded: false,
                expected_effective_counter: 0x3632,
                old_to_ff: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                ff_to_new: (
                    ApuCh4Nr43LiveWriteCategory::None,
                    ApuCh4Nr43LfsrAction::None,
                ),
                low_shift_followup: None,
            },
        ];

        for case in cases {
            let mut noise = Channel4NoiseSignalState {
                short_width_mode: case.old & NOISE_SHORT_WIDTH_BIT != 0,
                lfsr_state: 0x7BFB,
                ..Channel4NoiseSignalState::default()
            };
            let mut live = Channel4Nr43LiveWriteState {
                noise_counter: case.stored_counter,
                countdown_reloaded: case.countdown_reloaded,
                counter_active: true,
                background_counting: true,
                ..Channel4Nr43LiveWriteState::default()
            };

            let trace = trace_channel4_live_nr43_write(
                Channel4Nr43LiveWriteProfile::DmgPreCgbD,
                true,
                case.old,
                case.new,
                &mut noise,
                &mut live,
            );

            assert_eq!(
                trace.effective_counter, case.expected_effective_counter,
                "effective counter mismatch for {:#04X}->{:#04X}",
                case.old, case.new
            );
            assert_eq!(
                trace.old_to_ff.map(|pass| (pass.category, pass.action)),
                Some(case.old_to_ff),
                "old->FF mismatch for {:#04X}->{:#04X}",
                case.old,
                case.new
            );
            assert_eq!(
                trace
                    .ff_to_glitch_1
                    .map(|pass| (pass.category, pass.action)),
                Some(case.ff_to_new),
                "FF->new mismatch for {:#04X}->{:#04X}",
                case.old,
                case.new
            );
            assert_eq!(
                trace
                    .low_shift_followup
                    .map(|pass| (pass.category, pass.action)),
                case.low_shift_followup,
                "low-shift follow-up mismatch for {:#04X}->{:#04X}",
                case.old,
                case.new
            );
        }
    }
}
