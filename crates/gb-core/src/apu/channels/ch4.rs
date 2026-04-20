use crate::model::ConsoleModel;

use super::super::common::{
    ChannelRuntimeState, DAC_ENABLE_REGISTER_MASK, EnvelopeState, ExtraLengthClockingContext,
    LENGTH_ENABLE_BIT, NOISE_CLOCK_SHIFT_MASK, NOISE_CLOCK_SHIFT_SHIFT, NOISE_COUNTER_MASK,
    NOISE_DIVIDER_CODE_MASK, NOISE_LFSR_INITIAL_STATE, NOISE_LFSR_OUTPUT_BIT,
    NOISE_SHORT_WIDTH_BIT, NR41_WRITE_ONLY_READ_VALUE, NR44_FORCED_HIGH_MASK, NR44_READ_MASK,
    NR44_WRITABLE_MASK, PULSE_LENGTH_COUNTER_RELOAD, apply_extra_length_clocking_u8,
    begin_nrx4_write, clock_length_counter_u8, noise_clocking_suppressed,
    noise_counter_phase_after_trigger, noise_counter_timer_reload, noise_timer_reload,
    pulse_length_counter_from_load,
};
use super::super::registers::Channel4Register;
use super::super::{ApuCh4DebugSnapshot, ApuCh4Nr43LiveWriteTrace};
use super::ch4_live_write::{step_channel4_lfsr, trace_channel4_live_nr43_write};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::apu) struct Channel4NoiseSignalState {
    pub(in crate::apu) clock_shift: u8,
    pub(in crate::apu) short_width_mode: bool,
    pub(in crate::apu) clock_divider_code: u8,
    pub(in crate::apu) period_timer: u32,
    pub(in crate::apu) lfsr_state: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::apu) struct Channel4Nr43LiveWriteState {
    pub(in crate::apu) alignment: u8,
    pub(in crate::apu) counter_timer: u32,
    pub(in crate::apu) noise_counter: u16,
    pub(in crate::apu) countdown_reloaded: bool,
    pub(in crate::apu) last_trace: Option<ApuCh4Nr43LiveWriteTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::apu) struct Channel4State {
    nr41: u8,
    nr42: u8,
    nr43: u8,
    nr44: u8,
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
            Channel4Register::Nr42 => self.write_nr42(value),
            Channel4Register::Nr43 => self.write_nr43(value),
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

    fn write_nr42(&mut self, value: u8) {
        self.apply_live_envelope_write_effect(value);
        self.nr42 = value;
        self.runtime.set_dac_enabled(self.derived_dac_enabled());
    }

    pub(in crate::apu) fn write_nr43(&mut self, value: u8) {
        let old_nr43 = self.nr43;
        let trace = trace_channel4_live_nr43_write(
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
            write_plan.observe_trigger_reloaded_zero_length(
                self.trigger(write_plan.context.next_step_clocks_envelope),
            );
            write_plan.observe_length_enabled_after_trigger(self.length_enabled);
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
        self.length_counter = pulse_length_counter_from_load(self.nr41);
        self.length_enabled = self.nr44 & LENGTH_ENABLE_BIT != 0;
        self.apply_envelope_write(self.nr42);
        self.decode_nr43(self.nr43);
        self.envelope.reload(false);
        self.nr43_live_write.alignment = 0;
        self.nr43_live_write.counter_timer = self.noise_counter_timer_reload();
        self.nr43_live_write.noise_counter =
            noise_counter_phase_after_trigger(self.noise.clock_shift);
        self.nr43_live_write.countdown_reloaded = false;
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
        self.length_counter = 0;
        self.length_enabled = false;
        self.envelope = EnvelopeState::default();
        self.noise.clock_shift = 0;
        self.noise.short_width_mode = false;
        self.noise.clock_divider_code = 0;
        self.nr43_live_write.alignment = 0;
        self.nr43_live_write.counter_timer = 0;
        self.nr43_live_write.noise_counter = 0;
        self.nr43_live_write.countdown_reloaded = false;
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

    fn derived_dac_enabled(&self) -> bool {
        self.nr42 & DAC_ENABLE_REGISTER_MASK != 0
    }

    fn apply_envelope_write(&mut self, value: u8) {
        self.envelope.apply_write(value);
    }

    fn apply_live_envelope_write_effect(&mut self, value: u8) {
        self.envelope
            .apply_live_write_effect(self.runtime.active, value);
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

    fn trigger(&mut self, next_step_clocks_envelope: bool) -> bool {
        let reloaded_zero_length = self.length_counter == 0;
        if self.length_counter == 0 {
            self.length_counter = PULSE_LENGTH_COUNTER_RELOAD;
            self.length_enabled = false;
        }

        self.apply_envelope_write(self.nr42);
        self.nr43_live_write.alignment = 0;
        self.nr43_live_write.counter_timer = self.noise_counter_timer_reload();
        self.nr43_live_write.noise_counter =
            noise_counter_phase_after_trigger(self.noise.clock_shift);
        self.nr43_live_write.countdown_reloaded = false;
        self.noise.period_timer = self.noise_timer_reload();
        self.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;
        self.envelope.reload(next_step_clocks_envelope);
        self.runtime.trigger();
        reloaded_zero_length
    }

    pub(in crate::apu) fn tick_fast_timer(&mut self) {
        self.nr43_live_write.alignment = (self.nr43_live_write.alignment + 1) & 0x03;
        self.tick_noise_counter_phase();

        if noise_clocking_suppressed(self.noise.clock_shift) {
            return;
        }

        if self.noise.period_timer > 0 {
            self.noise.period_timer -= 1;
        }

        if self.noise.period_timer == 0 {
            self.noise.period_timer = self.noise_timer_reload();
            step_channel4_lfsr(&mut self.noise);
        }
    }

    fn tick_noise_counter_phase(&mut self) {
        if self.nr43_live_write.counter_timer == 0 {
            self.nr43_live_write.counter_timer = self.noise_counter_timer_reload();
        }

        self.nr43_live_write.counter_timer -= 1;
        if self.nr43_live_write.counter_timer != 0 {
            self.nr43_live_write.countdown_reloaded = false;
            return;
        }

        self.nr43_live_write.counter_timer = self.noise_counter_timer_reload();
        self.nr43_live_write.noise_counter =
            self.nr43_live_write.noise_counter.wrapping_add(1) & NOISE_COUNTER_MASK;
        self.nr43_live_write.countdown_reloaded = true;
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

        if self.noise.lfsr_state & (1 << NOISE_LFSR_OUTPUT_BIT) == 0 {
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
            period_timer: self.noise.period_timer,
            lfsr_state: self.noise.lfsr_state,
            current_digital_output: self.current_digital_output(),
            last_nr43_live_write: self.nr43_live_write.last_trace,
        }
    }
}
