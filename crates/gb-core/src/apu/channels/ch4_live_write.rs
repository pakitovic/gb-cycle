use super::super::ApuCh4Nr43LiveWriteTrace;
use super::super::common::{
    NOISE_CLOCK_SHIFT_MASK, NOISE_CLOCK_SHIFT_SHIFT, NOISE_COUNTER_MASK, NOISE_LFSR_FEEDBACK_BIT,
    NOISE_LFSR_OUTPUT_BIT, NOISE_LFSR_SHORT_WIDTH_FEEDBACK_BIT, NOISE_LFSR_TAP_BIT,
    NOISE_SHORT_WIDTH_BIT, noise_counter_bit, noise_counter_phase_after_trigger,
    noise_counter_timer_reload,
};
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
            old_shift,
            new_shift,
            effective_counter,
            countdown_reloaded: self.nr43_live_write.countdown_reloaded,
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

        self.apply_nr43_transition(self.old_nr43, 0xFF, effective_counter, &mut trace);
        self.apply_nr43_transition(0xFF, self.new_nr43, effective_counter, &mut trace);
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

    fn apply_nr43_transition(
        &mut self,
        old_nr43: u8,
        new_nr43: u8,
        effective_counter: u16,
        trace: &mut ApuCh4Nr43LiveWriteTrace,
    ) {
        let old_shift = decode_nr43_clock_shift(old_nr43);
        let new_shift = decode_nr43_clock_shift(new_nr43);
        let new_short_width_mode = new_nr43 & NOISE_SHORT_WIDTH_BIT != 0;
        let is_old_to_ff = new_nr43 == 0xFF;

        if old_shift == new_shift {
            self.noise.short_width_mode = new_short_width_mode;
            return;
        }

        let old_bit = noise_counter_bit(effective_counter, old_shift);
        let glitch_value = (old_nr43 & 0x7F) | (new_nr43 & 0x80);
        let glitch_shift = decode_nr43_clock_shift(glitch_value);
        let glitch_bit = noise_counter_bit(effective_counter, glitch_shift);
        let new_bit = noise_counter_bit(effective_counter, new_shift);

        self.noise.short_width_mode = new_short_width_mode;

        if old_bit == new_bit && new_bit != glitch_bit {
            if !new_bit {
                if is_old_to_ff && suppress_high_shift_narrow_old_to_ff_step(old_nr43) {
                    return;
                }
                step_channel4_lfsr(self.noise);
                if is_old_to_ff {
                    trace.old_to_ff_step = true;
                } else {
                    trace.ff_to_new_step = true;
                }
            }
            return;
        }

        if !old_bit && new_bit {
            step_channel4_lfsr_with_forced_short_width(self.noise);
            if is_old_to_ff {
                trace.old_to_ff_step = true;
                trace.old_to_ff_forced_short_width = true;
            } else {
                trace.ff_to_new_step = true;
                trace.ff_to_new_forced_short_width = true;
            }
            if new_shift <= 2 && glitch_bit && effective_counter & 0x08 == 0 {
                step_channel4_lfsr(self.noise);
                apply_channel4_feedback_bit_corruption(self.noise);
                trace.low_shift_extra_step = true;
                trace.feedback_corruption = true;
            }
            return;
        }

        if new_shift <= 2 && !glitch_bit && !new_bit && !old_bit && effective_counter & 0x08 != 0 {
            step_channel4_lfsr(self.noise);
            if is_old_to_ff {
                trace.old_to_ff_step = true;
            } else {
                trace.ff_to_new_step = true;
            }
        }
    }
}

fn decode_nr43_clock_shift(value: u8) -> u8 {
    (value >> NOISE_CLOCK_SHIFT_SHIFT) & NOISE_CLOCK_SHIFT_MASK
}

fn suppress_high_shift_narrow_old_to_ff_step(old_nr43: u8) -> bool {
    let old_shift = decode_nr43_clock_shift(old_nr43);
    old_nr43 & NOISE_SHORT_WIDTH_BIT != 0 && old_shift >= 5
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
