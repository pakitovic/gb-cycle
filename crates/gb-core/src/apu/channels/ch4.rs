use crate::model::ConsoleModel;

use super::super::common::{
    ChannelRuntimeState, DAC_ENABLE_REGISTER_MASK, EnvelopeState, ExtraLengthClockingContext,
    LENGTH_ENABLE_BIT, NOISE_CLOCK_SHIFT_MASK, NOISE_CLOCK_SHIFT_SHIFT, NOISE_COUNTER_MASK,
    NOISE_DIVIDER_CODE_MASK, NOISE_LFSR_INITIAL_STATE, NOISE_LFSR_OUTPUT_BIT,
    NOISE_SHORT_WIDTH_BIT, NR41_WRITE_ONLY_READ_VALUE, NR44_FORCED_HIGH_MASK, NR44_READ_MASK,
    NR44_WRITABLE_MASK, PULSE_LENGTH_COUNTER_RELOAD, apply_extra_length_clocking_u8,
    begin_nrx4_write, clock_length_counter_u8, noise_clocking_suppressed, noise_counter_bit,
    noise_counter_timer_reload, noise_timer_reload, pulse_length_counter_from_load,
};
use super::super::registers::Channel4Register;
use super::super::{ApuCh4DebugSnapshot, ApuCh4Nr43LiveWriteTrace};
use super::ch4_live_write::{
    Channel4Nr43LiveWriteProfile, step_channel4_lfsr, trace_channel4_live_nr43_write,
};

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::apu) struct Channel4NoiseSignalState {
    pub(in crate::apu) clock_shift: u8,
    pub(in crate::apu) short_width_mode: bool,
    pub(in crate::apu) clock_divider_code: u8,
    pub(in crate::apu) period_timer: u32,
    pub(in crate::apu) lfsr_state: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::apu) struct Channel4Nr43LiveWriteState {
    pub(in crate::apu) alignment: u8,
    pub(in crate::apu) alignment_subphase: bool,
    pub(in crate::apu) counter_timer: u32,
    pub(in crate::apu) noise_counter: u16,
    pub(in crate::apu) countdown_reloaded: bool,
    pub(in crate::apu) did_step_counter: bool,
    pub(in crate::apu) counter_active: bool,
    pub(in crate::apu) background_counting: bool,
    pub(in crate::apu) started_with_dac_disabled: bool,
    pub(in crate::apu) last_trace: Option<ApuCh4Nr43LiveWriteTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::apu) struct Channel4State {
    nr41: u8,
    nr42: u8,
    nr43: u8,
    nr44: u8,
    dmg_delayed_start: u8,
    pending_trigger_envelope_reload: bool,
    pub(in crate::apu) runtime: ChannelRuntimeState,
    pub(in crate::apu) length_counter: u8,
    length_enabled: bool,
    pub(in crate::apu) envelope: EnvelopeState,
    pub(in crate::apu) noise: Channel4NoiseSignalState,
    pub(in crate::apu) nr43_live_write: Channel4Nr43LiveWriteState,
}

impl Channel4State {
    pub(in crate::apu) fn read_register(&self, register: Channel4Register) -> u8 {
        match register {
            Channel4Register::Nr41 => NR41_WRITE_ONLY_READ_VALUE,
            Channel4Register::Nr42 => self.nr42,
            Channel4Register::Nr43 => self.nr43,
            Channel4Register::Nr44 => self.read_nr44(),
        }
    }

    pub(in crate::apu) fn write_register(
        &mut self,
        register: Channel4Register,
        value: u8,
        console_model: ConsoleModel,
        next_frame_sequencer_step: u8,
    ) {
        match register {
            Channel4Register::Nr41 => self.write_nr41(value),
            Channel4Register::Nr42 => self.write_nr42(value, console_model),
            Channel4Register::Nr43 => self.write_nr43_for_model(value, console_model),
            Channel4Register::Nr44 => {
                self.write_nr44(value, console_model, next_frame_sequencer_step)
            }
        }
    }

    pub(in crate::apu) fn write_powered_off_register(
        &mut self,
        register: Channel4Register,
        value: u8,
        console_model: ConsoleModel,
    ) {
        if !console_model.is_dmg_family() {
            return;
        }

        if matches!(register, Channel4Register::Nr41) {
            self.write_length_while_powered_off(value);
        }
    }

    fn read_nr44(&self) -> u8 {
        (self.nr44 & NR44_READ_MASK) | NR44_FORCED_HIGH_MASK
    }

    fn write_nr41(&mut self, value: u8) {
        self.nr41 = value;
        self.length_counter = pulse_length_counter_from_load(value);
    }

    fn write_nr42(&mut self, value: u8, console_model: ConsoleModel) {
        let dac_disabling = self.runtime.dac_enabled && value & DAC_ENABLE_REGISTER_MASK == 0;
        if dac_disabling {
            self.handle_hidden_counter_dac_disable();
        }
        self.apply_live_envelope_write_effect(value, console_model);
        self.nr42 = value;
        self.runtime.set_dac_enabled(self.derived_dac_enabled());
    }

    #[cfg(test)]
    pub(in crate::apu) fn write_nr43(&mut self, value: u8) {
        self.write_nr43_for_model(value, ConsoleModel::GameBoy);
    }

    pub(in crate::apu) fn write_nr43_for_model(&mut self, value: u8, console_model: ConsoleModel) {
        let old_nr43 = self.nr43;
        let profile = channel4_nr43_live_write_profile(console_model);
        let trace = trace_channel4_live_nr43_write(
            profile,
            self.runtime.active,
            old_nr43,
            value,
            &mut self.noise,
            &mut self.nr43_live_write,
        );
        self.nr43_live_write.last_trace = Some(trace);
        self.nr43 = value;
        self.decode_nr43(value);
        self.nr43_live_write.counter_timer =
            super::ch4_live_write::resolve_channel4_noise_counter_timer_after_live_write(
                profile,
                value,
                &self.nr43_live_write,
            );
        self.nr43_live_write.countdown_reloaded = false;
        self.noise.period_timer = self.noise_timer_reload();
    }

    fn write_nr44(
        &mut self,
        value: u8,
        console_model: ConsoleModel,
        next_frame_sequencer_step: u8,
    ) {
        let mut write_plan = begin_nrx4_write(
            &mut self.nr44,
            value,
            NR44_WRITABLE_MASK,
            next_frame_sequencer_step,
            self.length_enabled,
        );

        if write_plan.context.trigger {
            let delayed_dmg_trigger =
                console_model.is_dmg_family() && (self.nr43_live_write.alignment & 0x03) != 0;
            if delayed_dmg_trigger {
                let trigger_reloaded_zero_length = self.length_counter == 0;
                if trigger_reloaded_zero_length {
                    self.length_counter = PULSE_LENGTH_COUNTER_RELOAD;
                    self.length_enabled = false;
                }
                write_plan.observe_trigger_reloaded_zero_length(trigger_reloaded_zero_length);
                write_plan.observe_length_enabled_after_trigger(self.length_enabled);
                self.dmg_delayed_start = 6;
                self.pending_trigger_envelope_reload = write_plan.context.next_step_clocks_envelope;
            } else {
                write_plan.observe_trigger_reloaded_zero_length(
                    self.trigger(console_model, write_plan.context.next_step_clocks_envelope),
                );
                write_plan.observe_length_enabled_after_trigger(self.length_enabled);
            }
        }

        self.length_enabled = write_plan.context.length_enabled;
        self.apply_extra_length_clocking_on_enable(
            console_model,
            write_plan.was_length_enabled,
            write_plan.context.next_step_clocks_length,
            write_plan.context.trigger,
            write_plan.trigger_reloaded_zero_length,
        );
    }

    pub(in crate::apu) fn apply_powered_startup(
        &mut self,
        nr41: u8,
        nr42: u8,
        nr43: u8,
        nr44: u8,
        active: bool,
    ) {
        self.nr41 = nr41;
        self.nr42 = nr42;
        self.nr43 = nr43;
        self.nr44 = nr44 & NR44_WRITABLE_MASK;
        self.dmg_delayed_start = 0;
        self.pending_trigger_envelope_reload = false;
        self.length_counter = pulse_length_counter_from_load(self.nr41);
        self.length_enabled = self.nr44 & LENGTH_ENABLE_BIT != 0;
        self.apply_envelope_write(self.nr42);
        self.decode_nr43(self.nr43);
        self.envelope.reload(false);
        self.nr43_live_write.alignment = 0;
        self.nr43_live_write.alignment_subphase = false;
        self.nr43_live_write.counter_timer = 0;
        self.nr43_live_write.noise_counter = 0;
        self.nr43_live_write.countdown_reloaded = false;
        self.nr43_live_write.did_step_counter = false;
        self.nr43_live_write.counter_active = active && self.derived_dac_enabled();
        self.nr43_live_write.background_counting = active;
        self.nr43_live_write.started_with_dac_disabled = false;
        self.noise.period_timer = self.noise_timer_reload();
        self.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;
        self.nr43_live_write.last_trace = None;
        self.runtime.clear();
        self.runtime.set_dac_enabled(self.derived_dac_enabled());
        self.runtime.set_active_from_startup(active);
    }

    fn clear_registers(&mut self) {
        self.nr41 = 0;
        self.nr42 = 0;
        self.nr43 = 0;
        self.nr44 = 0;
        self.dmg_delayed_start = 0;
        self.pending_trigger_envelope_reload = false;
        self.length_counter = 0;
        self.length_enabled = false;
        self.envelope = EnvelopeState::default();
        self.noise.clock_shift = 0;
        self.noise.short_width_mode = false;
        self.noise.clock_divider_code = 0;
        self.nr43_live_write.alignment = 0;
        self.nr43_live_write.alignment_subphase = false;
        self.nr43_live_write.counter_timer = 0;
        self.nr43_live_write.noise_counter = 0;
        self.nr43_live_write.countdown_reloaded = false;
        self.nr43_live_write.did_step_counter = false;
        self.nr43_live_write.counter_active = false;
        self.nr43_live_write.background_counting = false;
        self.nr43_live_write.started_with_dac_disabled = false;
        self.noise.period_timer = 0;
        self.noise.lfsr_state = 0;
        self.nr43_live_write.last_trace = None;
        self.runtime.clear();
    }

    pub(in crate::apu) fn write_length_while_powered_off(&mut self, value: u8) {
        self.length_counter = pulse_length_counter_from_load(value);
    }

    pub(in crate::apu) fn power_off_registers(&mut self, console_model: ConsoleModel) {
        let preserved_length = if console_model.is_dmg_family() {
            self.length_counter
        } else {
            0
        };
        self.clear_registers();
        self.length_counter = preserved_length;
    }

    pub(in crate::apu) fn mark_powered_on(&mut self) {
        /*
         SameBoy's GB_apu_init() clears the APU struct on NR52 power-on. For CH4 this means the
         hidden startup phase observed by the first real trigger restarts from alignment 0 even if
         the powered-off timebase kept moving while NR52 was low.
        */
        self.nr43_live_write.alignment = 0;
        self.nr43_live_write.alignment_subphase = false;
        self.dmg_delayed_start = 0;
        self.pending_trigger_envelope_reload = false;
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr42 & DAC_ENABLE_REGISTER_MASK != 0
    }

    fn apply_envelope_write(&mut self, value: u8) {
        self.envelope.apply_write(value);
    }

    fn apply_live_envelope_write_effect(&mut self, value: u8, console_model: ConsoleModel) {
        self.envelope
            .apply_live_write_effect(console_model, self.runtime.active, value);
    }

    fn decode_nr43(&mut self, value: u8) {
        self.noise.clock_shift = (value >> NOISE_CLOCK_SHIFT_SHIFT) & NOISE_CLOCK_SHIFT_MASK;
        self.noise.short_width_mode = value & NOISE_SHORT_WIDTH_BIT != 0;
        self.noise.clock_divider_code = value & NOISE_DIVIDER_CODE_MASK;
    }

    fn noise_timer_reload(&self) -> u32 {
        noise_timer_reload(self.noise.clock_shift, self.noise.clock_divider_code)
    }

    fn noise_counter_timer_reload(&self) -> u32 {
        noise_counter_timer_reload(self.noise.clock_divider_code)
    }

    fn apply_extra_length_clocking_on_enable(
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

    fn trigger(&mut self, console_model: ConsoleModel, next_step_clocks_envelope: bool) -> bool {
        let reloaded_zero_length = self.length_counter == 0;
        if self.length_counter == 0 {
            self.length_counter = PULSE_LENGTH_COUNTER_RELOAD;
            self.length_enabled = false;
        }

        self.apply_envelope_write(self.nr42);
        self.prepare_hidden_counter_start(console_model);
        self.noise.period_timer = self.noise_timer_reload();
        self.envelope.reload(next_step_clocks_envelope);
        self.runtime.trigger();
        reloaded_zero_length
    }

    pub(in crate::apu) fn tick_fast_timer(&mut self) {
        let apu_2mhz_step = self.tick_alignment_phase_only();

        let start_after_tick = if apu_2mhz_step && self.dmg_delayed_start != 0 {
            self.dmg_delayed_start -= 1;
            self.dmg_delayed_start == 0
        } else {
            false
        };

        if apu_2mhz_step {
            self.tick_noise_counter_phase();
        }

        if start_after_tick {
            let next_step_clocks_envelope = self.pending_trigger_envelope_reload;
            self.pending_trigger_envelope_reload = false;
            self.trigger(ConsoleModel::GameBoy, next_step_clocks_envelope);
        }

        if noise_clocking_suppressed(self.noise.clock_shift) {
            return;
        }

        if self.noise.period_timer > 0 {
            self.noise.period_timer -= 1;
        }

        if self.noise.period_timer == 0 {
            self.noise.period_timer = self.noise_timer_reload();
        }
    }

    pub(in crate::apu) fn tick_alignment_phase_only(&mut self) -> bool {
        self.nr43_live_write.alignment_subphase = !self.nr43_live_write.alignment_subphase;
        let apu_2mhz_step = self.nr43_live_write.alignment_subphase;
        if apu_2mhz_step {
            self.nr43_live_write.alignment = (self.nr43_live_write.alignment + 1) & 0x03;
        }
        apu_2mhz_step
    }

    fn tick_noise_counter_phase(&mut self) {
        if !self.hidden_counter_running() {
            self.nr43_live_write.countdown_reloaded = false;
            return;
        }

        let divisor = self.noise_counter_timer_reload();
        if self.nr43_live_write.counter_timer == 0 {
            self.nr43_live_write.counter_timer = divisor;
        }

        if self.nr43_live_write.counter_timer > 1 {
            self.nr43_live_write.counter_timer -= 1;
            self.nr43_live_write.countdown_reloaded = false;
            return;
        }

        let old_bit = noise_counter_bit(self.nr43_live_write.noise_counter, self.noise.clock_shift);
        self.nr43_live_write.counter_timer = divisor;
        self.nr43_live_write.noise_counter =
            self.nr43_live_write.noise_counter.wrapping_add(1) & NOISE_COUNTER_MASK;
        self.nr43_live_write.did_step_counter = true;
        let new_bit = noise_counter_bit(self.nr43_live_write.noise_counter, self.noise.clock_shift);
        if new_bit && !old_bit && self.runtime.active {
            step_channel4_lfsr(&mut self.noise);
        }
        self.nr43_live_write.countdown_reloaded = true;
    }

    fn hidden_counter_running(&self) -> bool {
        self.nr43_live_write.counter_active || self.nr43_live_write.background_counting
    }

    fn hidden_counter_divisor_code(&self) -> u32 {
        u32::from(self.noise.clock_divider_code & NOISE_DIVIDER_CODE_MASK)
    }

    fn increment_hidden_counter(&mut self) {
        self.nr43_live_write.noise_counter =
            self.nr43_live_write.noise_counter.wrapping_add(1) & NOISE_COUNTER_MASK;
    }

    fn prepare_hidden_counter_start(&mut self, _console_model: ConsoleModel) {
        let was_started_with_dac_disabled = self.nr43_live_write.started_with_dac_disabled;
        self.nr43_live_write.counter_active = self.derived_dac_enabled();
        self.nr43_live_write.started_with_dac_disabled = !self.nr43_live_write.counter_active;
        let was_background_counting = self.nr43_live_write.background_counting;
        self.nr43_live_write.background_counting = true;
        let first_bootstrap_start = !was_background_counting
            && !self.nr43_live_write.counter_active
            && !self.runtime.active
            && self.nr43_live_write.noise_counter == 0
            && !self.nr43_live_write.did_step_counter;
        if first_bootstrap_start {
            self.nr43_live_write.noise_counter =
                self.nr43_live_write.noise_counter.wrapping_add(8) & NOISE_COUNTER_MASK;
        }

        let mut divisor = self.hidden_counter_divisor_code();
        let alignment = self.nr43_live_write.alignment & 0x03;
        let mut instant_step = false;
        let mut div_1_glitch = false;

        if divisor > 1 && self.nr43_live_write.counter_timer == 1 {
            self.increment_hidden_counter();
        } else if self.nr43_live_write.counter_timer == 2 && alignment == 0 && self.runtime.active {
            if divisor == 0 {
                divisor = 8;
            } else if divisor == 1 {
                if !self.nr43_live_write.did_step_counter {
                    div_1_glitch = true;
                }

                let old_bit =
                    noise_counter_bit(self.nr43_live_write.noise_counter, self.noise.clock_shift);
                self.increment_hidden_counter();
                let new_bit =
                    noise_counter_bit(self.nr43_live_write.noise_counter, self.noise.clock_shift);
                if new_bit && !old_bit {
                    instant_step = true;
                }
            }
        }

        let mut counter_timer = if divisor == 0 { 6 } else { divisor * 4 + 6 } as i32;

        if alignment & 1 != 0 {
            if divisor == 0 {
                counter_timer += 1;
            } else if alignment & 2 != 0 {
                if divisor == 1 && !self.runtime.active {
                    counter_timer += 1;
                } else {
                    counter_timer -= 3;
                }
            } else {
                counter_timer -= 1;
                if divisor == 1 && self.runtime.active {
                    counter_timer -= 4;
                }
            }
        } else if divisor != 0 {
            if alignment & 2 != 0 {
                counter_timer -= 2;
            } else if divisor > 1
                || (divisor == 1 && self.runtime.active && (self.nr43 & 0xF0) == 0)
            {
                counter_timer -= 4;
            }
        }

        if divisor > 1 {
            if !self.nr43_live_write.counter_active && alignment == 0 {
                counter_timer += 4;
            }
        } else if was_background_counting && !self.runtime.active && alignment == 0 {
            if divisor == 0 {
                if was_started_with_dac_disabled {
                    counter_timer += 28;
                }
            } else {
                counter_timer -= 4;
            }
        }

        if div_1_glitch {
            counter_timer -= 4;
        }

        if divisor == 0 && self.runtime.active && alignment == 3 {
            self.noise.lfsr_state = 0x0055;
        } else {
            self.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;
        }
        self.nr43_live_write.counter_timer = counter_timer.max(0) as u32;
        self.nr43_live_write.countdown_reloaded = false;
        self.nr43_live_write.did_step_counter = alignment == 2;

        if instant_step {
            step_channel4_lfsr(&mut self.noise);
        }
    }

    fn handle_hidden_counter_dac_disable(&mut self) {
        if self.runtime.active && self.noise.clock_divider_code != 0 {
            if self.nr43_live_write.counter_timer > 0 && self.nr43_live_write.counter_timer <= 2 {
                self.increment_hidden_counter();
            }
            self.nr43_live_write.background_counting = false;
        }
        self.nr43_live_write.counter_active = false;
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

    pub(in crate::apu) fn current_digital_output(&self) -> u8 {
        if !self.runtime.active {
            return 0;
        }

        if self.noise.lfsr_state & (1 << NOISE_LFSR_OUTPUT_BIT) != 0 {
            self.envelope.current_volume
        } else {
            0
        }
    }

    pub(in crate::apu) fn runtime_state(&self) -> ChannelRuntimeState {
        self.runtime
    }

    pub(in crate::apu) fn debug_snapshot(&self) -> ApuCh4DebugSnapshot {
        ApuCh4DebugSnapshot {
            nr43: self.nr43,
            clock_shift: self.noise.clock_shift,
            short_width_mode: self.noise.short_width_mode,
            clock_divider_code: self.noise.clock_divider_code,
            alignment: self.nr43_live_write.alignment,
            counter_timer: self.nr43_live_write.counter_timer,
            noise_counter: self.nr43_live_write.noise_counter,
            countdown_reloaded: self.nr43_live_write.countdown_reloaded,
            did_step_counter: self.nr43_live_write.did_step_counter,
            counter_active: self.nr43_live_write.counter_active,
            background_counting: self.nr43_live_write.background_counting,
            started_with_dac_disabled: self.nr43_live_write.started_with_dac_disabled,
            dmg_delayed_start: self.dmg_delayed_start,
            runtime_active: self.runtime.active,
            runtime_dac_enabled: self.runtime.dac_enabled,
            period_timer: self.noise.period_timer,
            lfsr_state: self.noise.lfsr_state,
            current_digital_output: self.current_digital_output(),
            last_nr43_live_write: self.nr43_live_write.last_trace,
        }
    }
}

fn channel4_nr43_live_write_profile(console_model: ConsoleModel) -> Channel4Nr43LiveWriteProfile {
    if console_model.is_cgb_family() {
        Channel4Nr43LiveWriteProfile::CgbDirect
    } else {
        Channel4Nr43LiveWriteProfile::DmgPreCgbD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powered_off_nr41_writes_are_dmg_only() {
        let mut channel = Channel4State {
            length_counter: 9,
            ..Channel4State::default()
        };

        channel.write_powered_off_register(Channel4Register::Nr42, 0x00, ConsoleModel::GameBoy);
        assert_eq!(channel.length_counter, 9);

        channel.write_powered_off_register(
            Channel4Register::Nr41,
            0x3F,
            ConsoleModel::GameBoyColor,
        );
        assert_eq!(channel.length_counter, 9);

        channel.write_powered_off_register(Channel4Register::Nr41, 0x3F, ConsoleModel::GameBoy);
        assert_eq!(channel.length_counter, 1);
    }

    #[test]
    fn powered_startup_and_power_off_cover_both_length_preservation_paths() {
        let mut dmg = Channel4State::default();
        dmg.apply_powered_startup(0x3F, 0x00, 0x01, LENGTH_ENABLE_BIT, false);
        assert_eq!(dmg.length_counter, 1);
        assert!(!dmg.nr43_live_write.counter_active);
        assert!(!dmg.nr43_live_write.background_counting);
        assert!(!dmg.nr43_live_write.started_with_dac_disabled);
        dmg.power_off_registers(ConsoleModel::GameBoy);
        assert_eq!(dmg.length_counter, 1);

        let mut cgb = Channel4State::default();
        cgb.apply_powered_startup(0x3F, 0x00, 0x01, LENGTH_ENABLE_BIT, true);
        assert_eq!(cgb.length_counter, 1);
        cgb.power_off_registers(ConsoleModel::GameBoyColor);
        assert_eq!(cgb.length_counter, 0);
    }

    #[test]
    fn write_nr42_dac_disable_can_stop_or_preserve_background_counter() {
        let mut channel = Channel4State::default();
        channel.write_nr42(0xF0, ConsoleModel::GameBoy);
        channel.runtime.active = true;
        channel.write_nr43(0x01);
        channel.nr43_live_write.noise_counter = 0x1234;
        channel.nr43_live_write.counter_timer = 2;
        channel.nr43_live_write.counter_active = true;
        channel.nr43_live_write.background_counting = true;

        channel.write_nr42(0x00, ConsoleModel::GameBoy);

        assert_eq!(channel.nr43_live_write.noise_counter, 0x1235);
        assert!(!channel.nr43_live_write.counter_active);
        assert!(!channel.nr43_live_write.background_counting);

        let mut inactive = Channel4State::default();
        inactive.write_nr42(0xF0, ConsoleModel::GameBoy);
        inactive.write_nr43(0x00);
        inactive.nr43_live_write.background_counting = true;
        inactive.nr43_live_write.counter_active = true;

        inactive.write_nr42(0x00, ConsoleModel::GameBoy);

        assert!(!inactive.nr43_live_write.counter_active);
        assert!(inactive.nr43_live_write.background_counting);
        assert_eq!(inactive.nr43_live_write.noise_counter, 0);
    }

    #[test]
    fn hidden_counter_phase_reload_path_handles_zero_timer_without_stepping() {
        let mut channel = Channel4State::default();
        channel.write_nr43(0x02);
        channel.nr43_live_write.counter_active = true;
        channel.nr43_live_write.counter_timer = 0;
        channel.nr43_live_write.noise_counter = 7;

        channel.tick_noise_counter_phase();

        assert_eq!(channel.nr43_live_write.counter_timer, 7);
        assert_eq!(channel.nr43_live_write.noise_counter, 7);
        assert!(!channel.nr43_live_write.countdown_reloaded);
    }

    #[test]
    fn alignment_phase_ticks_even_without_the_hidden_counter_running() {
        let mut channel = Channel4State::default();

        assert!(channel.tick_alignment_phase_only());
        assert_eq!(channel.nr43_live_write.alignment, 1);

        assert!(!channel.tick_alignment_phase_only());
        assert_eq!(channel.nr43_live_write.alignment, 1);

        assert!(channel.tick_alignment_phase_only());
        assert_eq!(channel.nr43_live_write.alignment, 2);
    }

    #[test]
    fn fast_timer_advances_alignment_only_once_per_2mhz_step() {
        let mut channel = Channel4State::default();

        channel.tick_fast_timer();
        assert_eq!(channel.nr43_live_write.alignment, 1);

        channel.tick_fast_timer();
        assert_eq!(channel.nr43_live_write.alignment, 1);

        channel.tick_fast_timer();
        assert_eq!(channel.nr43_live_write.alignment, 2);
    }

    #[test]
    fn prepare_hidden_counter_start_can_increment_on_pending_gt_one_edge() {
        let mut channel = Channel4State::default();
        channel.write_nr42(0xF0, ConsoleModel::GameBoy);
        channel.write_nr43(0x02);
        channel.nr43_live_write.counter_timer = 1;
        channel.nr43_live_write.noise_counter = 0x1234;

        channel.prepare_hidden_counter_start(ConsoleModel::GameBoy);

        assert_eq!(channel.nr43_live_write.noise_counter, 0x1235);
        assert_eq!(channel.nr43_live_write.counter_timer, 10);
        assert!(channel.nr43_live_write.counter_active);
        assert!(channel.nr43_live_write.background_counting);
        assert!(!channel.nr43_live_write.did_step_counter);
    }

    #[test]
    fn prepare_hidden_counter_start_can_take_divider_one_glitch_and_instant_step() {
        let mut channel = Channel4State::default();
        channel.write_nr42(0xF0, ConsoleModel::GameBoy);
        channel.write_nr43(0x01);
        channel.runtime.active = true;
        channel.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;
        channel.nr43_live_write.counter_timer = 2;
        channel.nr43_live_write.noise_counter = 0;
        channel.nr43_live_write.alignment = 0;
        channel.nr43_live_write.did_step_counter = false;

        channel.prepare_hidden_counter_start(ConsoleModel::GameBoy);

        assert_eq!(channel.nr43_live_write.noise_counter, 1);
        assert_eq!(channel.nr43_live_write.counter_timer, 2);
        assert_ne!(channel.noise.lfsr_state, NOISE_LFSR_INITIAL_STATE);
        assert!(!channel.nr43_live_write.did_step_counter);
    }

    #[test]
    fn prepare_hidden_counter_start_covers_alignment_specific_paths() {
        let mut alignment_one_div_zero = Channel4State::default();
        alignment_one_div_zero.write_nr42(0xF0, ConsoleModel::GameBoy);
        alignment_one_div_zero.write_nr43(0x00);
        alignment_one_div_zero.nr43_live_write.alignment = 1;
        alignment_one_div_zero.prepare_hidden_counter_start(ConsoleModel::GameBoy);
        assert_eq!(alignment_one_div_zero.nr43_live_write.counter_timer, 7);

        let mut alignment_three_div_one = Channel4State::default();
        alignment_three_div_one.write_nr42(0xF0, ConsoleModel::GameBoy);
        alignment_three_div_one.write_nr43(0x01);
        alignment_three_div_one.nr43_live_write.alignment = 3;
        alignment_three_div_one.prepare_hidden_counter_start(ConsoleModel::GameBoy);
        assert_eq!(alignment_three_div_one.nr43_live_write.counter_timer, 11);

        let mut alignment_one_div_one_active = Channel4State::default();
        alignment_one_div_one_active.write_nr42(0xF0, ConsoleModel::GameBoy);
        alignment_one_div_one_active.write_nr43(0x01);
        alignment_one_div_one_active.runtime.active = true;
        alignment_one_div_one_active.nr43_live_write.alignment = 1;
        alignment_one_div_one_active.prepare_hidden_counter_start(ConsoleModel::GameBoy);
        assert_eq!(
            alignment_one_div_one_active.nr43_live_write.counter_timer,
            5
        );
    }

    #[test]
    fn prepare_hidden_counter_start_covers_background_and_dac_disabled_adjustments() {
        let mut divisor_gt_one_dac_disabled = Channel4State::default();
        divisor_gt_one_dac_disabled.write_nr43(0x02);
        divisor_gt_one_dac_disabled.nr43_live_write.alignment = 0;
        divisor_gt_one_dac_disabled.prepare_hidden_counter_start(ConsoleModel::GameBoy);
        assert_eq!(
            divisor_gt_one_dac_disabled.nr43_live_write.counter_timer,
            14
        );
        assert!(
            divisor_gt_one_dac_disabled
                .nr43_live_write
                .started_with_dac_disabled
        );

        let mut divisor_zero_background = Channel4State::default();
        divisor_zero_background.write_nr43(0x00);
        divisor_zero_background.nr43_live_write.background_counting = true;
        divisor_zero_background
            .nr43_live_write
            .started_with_dac_disabled = true;
        divisor_zero_background.prepare_hidden_counter_start(ConsoleModel::GameBoy);
        assert_eq!(divisor_zero_background.nr43_live_write.counter_timer, 34);

        let mut divisor_one_background = Channel4State::default();
        divisor_one_background.write_nr43(0x01);
        divisor_one_background.nr43_live_write.background_counting = true;
        divisor_one_background.prepare_hidden_counter_start(ConsoleModel::GameBoy);
        assert_eq!(divisor_one_background.nr43_live_write.counter_timer, 6);
    }

    #[test]
    fn debug_snapshot_reports_hidden_counter_state() {
        let mut channel = Channel4State {
            nr43: 0x4C,
            runtime: ChannelRuntimeState {
                active: true,
                ..ChannelRuntimeState::default()
            },
            noise: Channel4NoiseSignalState {
                clock_shift: 4,
                short_width_mode: true,
                clock_divider_code: 3,
                period_timer: 32,
                lfsr_state: 0x4566,
            },
            nr43_live_write: Channel4Nr43LiveWriteState {
                alignment: 2,
                alignment_subphase: false,
                counter_timer: 9,
                noise_counter: 0x1234,
                countdown_reloaded: true,
                did_step_counter: true,
                counter_active: true,
                background_counting: true,
                started_with_dac_disabled: false,
                last_trace: None,
            },
            ..Channel4State::default()
        };
        channel.envelope.current_volume = 7;

        let snapshot = channel.debug_snapshot();

        assert_eq!(snapshot.nr43, 0x4C);
        assert_eq!(snapshot.clock_shift, 4);
        assert!(snapshot.short_width_mode);
        assert_eq!(snapshot.clock_divider_code, 3);
        assert_eq!(snapshot.alignment, 2);
        assert_eq!(snapshot.counter_timer, 9);
        assert_eq!(snapshot.noise_counter, 0x1234);
        assert!(snapshot.countdown_reloaded);
        assert!(snapshot.did_step_counter);
        assert!(snapshot.counter_active);
        assert!(snapshot.background_counting);
        assert!(!snapshot.started_with_dac_disabled);
        assert_eq!(snapshot.period_timer, 32);
        assert_eq!(snapshot.lfsr_state, 0x4566);
        assert_eq!(snapshot.current_digital_output, 0);
    }
}
