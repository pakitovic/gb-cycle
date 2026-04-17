use crate::model::ConsoleModel;

use super::frame_sequencer::{
    frame_sequencer_step_clocks_envelope, frame_sequencer_step_clocks_length,
};

pub(super) const CHANNEL_COUNT: usize = 4;
pub(super) const CHANNEL_ACTIVE_CH1: u8 = 0x01;
pub(super) const CHANNEL_ACTIVE_CH2: u8 = 0x02;
pub(super) const CHANNEL_ACTIVE_CH3: u8 = 0x04;
pub(super) const CHANNEL_ACTIVE_CH4: u8 = 0x08;
pub(super) const CHANNEL_ACTIVE_MASK: u8 = 0x0F;
pub(super) const CHANNEL_MASKS: [u8; CHANNEL_COUNT] = [
    CHANNEL_ACTIVE_CH1,
    CHANNEL_ACTIVE_CH2,
    CHANNEL_ACTIVE_CH3,
    CHANNEL_ACTIVE_CH4,
];
pub(super) const APU_UNMAPPED_READ_VALUE: u8 = 0xFF;
pub(super) const NR10_FORCED_HIGH_MASK: u8 = 0x80;
pub(super) const NR10_WRITABLE_MASK: u8 = !NR10_FORCED_HIGH_MASK;
pub(super) const NR11_WRITE_ONLY_MASK: u8 = 0x3F;
pub(super) const NR13_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
pub(super) const NR23_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
pub(super) const NR14_READ_MASK: u8 = 0x40;
pub(super) const NR14_FORCED_HIGH_MASK: u8 = 0xBF;
pub(super) const NR30_FORCED_HIGH_MASK: u8 = 0x7F;
pub(super) const NR31_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
pub(super) const NR32_READ_MASK: u8 = 0x60;
pub(super) const NR32_FORCED_HIGH_MASK: u8 = 0x9F;
pub(super) const NR33_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
pub(super) const NR41_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
pub(super) const NR44_READ_MASK: u8 = 0x40;
pub(super) const NR44_FORCED_HIGH_MASK: u8 = 0xBF;
pub(super) const NR52_FORCED_HIGH_MASK: u8 = 0x70;
pub(super) const NR52_MASTER_POWER_BIT: u8 = 0x80;
pub(super) const NR30_DAC_POWER_BIT: u8 = 0x80;
pub(super) const CHANNEL_TRIGGER_BIT: u8 = 0x80;
pub(super) const LENGTH_ENABLE_BIT: u8 = 0x40;
pub(super) const NR50_VIN_LEFT_BIT: u8 = 0x80;
pub(super) const NR50_VIN_RIGHT_BIT: u8 = 0x08;
pub(super) const NR51_RIGHT_ROUTE_CH1_BIT: u8 = 0x01;
pub(super) const NR51_RIGHT_ROUTE_CH2_BIT: u8 = 0x02;
pub(super) const NR51_RIGHT_ROUTE_CH3_BIT: u8 = 0x04;
pub(super) const NR51_RIGHT_ROUTE_CH4_BIT: u8 = 0x08;
pub(super) const NR51_LEFT_ROUTE_CH1_BIT: u8 = 0x10;
pub(super) const NR51_LEFT_ROUTE_CH2_BIT: u8 = 0x20;
pub(super) const NR51_LEFT_ROUTE_CH3_BIT: u8 = 0x40;
pub(super) const NR51_LEFT_ROUTE_CH4_BIT: u8 = 0x80;
pub(super) const NR51_LEFT_ROUTE_BITS: [u8; CHANNEL_COUNT] = [
    NR51_LEFT_ROUTE_CH1_BIT,
    NR51_LEFT_ROUTE_CH2_BIT,
    NR51_LEFT_ROUTE_CH3_BIT,
    NR51_LEFT_ROUTE_CH4_BIT,
];
pub(super) const NR51_RIGHT_ROUTE_BITS: [u8; CHANNEL_COUNT] = [
    NR51_RIGHT_ROUTE_CH1_BIT,
    NR51_RIGHT_ROUTE_CH2_BIT,
    NR51_RIGHT_ROUTE_CH3_BIT,
    NR51_RIGHT_ROUTE_CH4_BIT,
];
pub(super) const NRX4_WRITABLE_MASK: u8 = 0x47;
pub(super) const NR44_WRITABLE_MASK: u8 = 0x40;
pub(super) const PERIOD_HIGH_MASK: u8 = 0x07;
pub(super) const PULSE_DUTY_MASK: u8 = 0xC0;
pub(super) const PULSE_DUTY_SHIFT: u8 = 6;
pub(super) const PULSE_DUTY_STEP_MASK: u8 = 0x07;
pub(super) const PULSE_LENGTH_LOAD_MASK: u8 = 0x3F;
pub(super) const PULSE_PERIOD_TIMER_LOW_BITS_MASK: u16 = 0x03;
pub(super) const SWEEP_PACE_MASK: u8 = 0x70;
pub(super) const SWEEP_PACE_SHIFT: u8 = 4;
pub(super) const SWEEP_DIRECTION_BIT: u8 = 0x08;
pub(super) const SWEEP_SHIFT_MASK: u8 = 0x07;
pub(super) const SWEEP_PHASE_MASK: u8 = 0x07;
pub(super) const SWEEP_PHASE_BOUNDARY: u8 = 7;
pub(super) const SWEEP_TIMER_RELOAD: u8 = 8;
pub(super) const DAC_ENABLE_REGISTER_MASK: u8 = 0xF8;
pub(super) const DAC_DIGITAL_OUTPUT_MASK: u8 = 0x0F;
pub(super) const ENVELOPE_INITIAL_VOLUME_MASK: u8 = 0xF0;
pub(super) const ENVELOPE_INITIAL_VOLUME_SHIFT: u8 = 4;
pub(super) const ENVELOPE_DIRECTION_BIT: u8 = 0x08;
pub(super) const ENVELOPE_PACE_MASK: u8 = 0x07;
pub(super) const NR50_VOLUME_MASK: u8 = 0x07;
pub(super) const NR50_LEFT_VOLUME_SHIFT: u8 = 4;
pub(super) const NR50_VOLUME_BIAS: i32 = 1;
pub(super) const NR50_MAX_VOLUME_FACTOR: i32 = NR50_VOLUME_MASK as i32 + NR50_VOLUME_BIAS;
#[cfg(test)]
pub(super) const NR50_MAX_VOLUME_BOTH: u8 =
    (NR50_VOLUME_MASK << NR50_LEFT_VOLUME_SHIFT) | NR50_VOLUME_MASK;
pub(super) const PULSE_LENGTH_COUNTER_RELOAD: u8 = 64;
pub(super) const WAVE_LENGTH_COUNTER_RELOAD: u16 = 256;
pub(super) const PULSE_PERIOD_MAX: u16 = 0x07FF;
pub(super) const MAX_ENVELOPE_VOLUME: u8 = 0x0F;
pub(super) const WAVE_RAM_LEN: usize = 0x10;
pub(super) const WAVE_SAMPLE_COUNT: u8 = 32;
pub(super) const WAVE_RAM_INACCESSIBLE_READ_VALUE: u8 = 0xFF;
pub(super) const WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES: u16 = 6;
pub(super) const WAVE_SAMPLE_BYTE_SHIFT: u8 = 1;
pub(super) const WAVE_SAMPLE_LOW_BIT: u8 = 0x01;
pub(super) const WAVE_SAMPLE_NIBBLE_MASK: u8 = 0x0F;
pub(super) const WAVE_HIGH_NIBBLE_SHIFT: u8 = 4;
pub(super) const WAVE_RETRIGGER_CORRUPTION_BLOCK_MASK: usize = 0x03;
pub(super) const WAVE_RETRIGGER_CORRUPTION_BLOCK_LEN: usize = 4;
pub(super) const WAVE_DMG_RETRIGGER_CORRUPTION_WINDOW_T_CYCLES: u16 = 2;
pub(super) const WAVE_OUTPUT_LEVEL_SHIFT: u8 = 5;
pub(super) const WAVE_OUTPUT_LEVEL_HALF_SHIFT: u8 = 1;
pub(super) const WAVE_OUTPUT_LEVEL_QUARTER_SHIFT: u8 = 2;
pub(super) const NOISE_LFSR_INITIAL_STATE: u16 = 0x0000;
pub(super) const NOISE_CLOCK_SHIFT_SHIFT: u8 = 4;
pub(super) const NOISE_CLOCK_SHIFT_MASK: u8 = 0x0F;
pub(super) const NOISE_SHORT_WIDTH_BIT: u8 = 0x08;
pub(super) const NOISE_DIVIDER_CODE_MASK: u8 = 0x07;
pub(super) const NOISE_LFSR_OUTPUT_BIT: u8 = 0;
pub(super) const NOISE_LFSR_TAP_BIT: u8 = 1;
pub(super) const NOISE_LFSR_FEEDBACK_BIT: u8 = 14;
pub(super) const NOISE_LFSR_SHORT_WIDTH_FEEDBACK_BIT: u8 = 6;
pub(super) const ANALOG_ONE: i32 = 15_000_000;
pub(super) const DAC_ANALOG_STEP: i32 = 2_000_000;
pub(super) const DMG_FAMILY_HPF_CHARGE_FACTOR_NUMERATOR: i64 = 999_958;
pub(super) const MGB_CGB_HPF_CHARGE_FACTOR_NUMERATOR: i64 = 998_943;
pub(super) const HPF_CHARGE_FACTOR_DENOMINATOR: i64 = 1_000_000;
pub const DMG_FAMILY_APU_CAPTURE_CLOCK_HZ: u32 = 4_194_304;
pub(super) const MAX_ROUTED_CHANNELS_PER_OUTPUT_BUS: i32 = CHANNEL_COUNT as i32;
pub const APU_HOST_MAX_ABS_SAMPLE: i32 =
    ANALOG_ONE * MAX_ROUTED_CHANNELS_PER_OUTPUT_BUS * NR50_MAX_VOLUME_FACTOR;
pub(super) const DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64: u64 = DMG_FAMILY_APU_CAPTURE_CLOCK_HZ as u64;

pub(super) const PULSE_DUTY_PATTERNS: [[bool; 8]; 4] = [
    [false, false, false, false, false, false, false, true],
    [true, false, false, false, false, false, false, true],
    [true, false, false, false, false, true, true, true],
    [false, true, true, true, true, true, true, false],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WaveRamMmioPolicy {
    DmgCurrentByteDuringFetchOnly,
    DeferredCgbActiveAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExtraLengthClockingPolicy {
    CurrentDmgBaseline,
    DeferredCgbRevisionBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Nrx4WriteContext {
    pub(super) trigger: bool,
    pub(super) length_enabled: bool,
    pub(super) next_step_clocks_length: bool,
    pub(super) next_step_clocks_envelope: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Nrx4WritePlan {
    pub(super) context: Nrx4WriteContext,
    pub(super) was_length_enabled: bool,
    pub(super) trigger_reloaded_zero_length: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExtraLengthClockingContext {
    pub(super) console_model: ConsoleModel,
    pub(super) length_enabled: bool,
    pub(super) was_length_enabled: bool,
    pub(super) next_step_clocks_length: bool,
    pub(super) trigger: bool,
    pub(super) trigger_reloaded_zero_length: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::apu) struct EnvelopeState {
    initial_volume: u8,
    increase: bool,
    pace: u8,
    pub(in crate::apu) automatic_updates_enabled: bool,
    pub(in crate::apu) timer: u8,
    pub(in crate::apu) current_volume: u8,
}

impl EnvelopeState {
    pub(in crate::apu) fn apply_write(&mut self, value: u8) {
        decode_envelope_register(
            value,
            &mut self.initial_volume,
            &mut self.increase,
            &mut self.pace,
        );
    }

    pub(in crate::apu) fn apply_live_write_effect(&mut self, active: bool, value: u8) {
        apply_consistent_zombie_mode_increment(active, &mut self.current_volume, value);
    }

    pub(in crate::apu) fn reload(&mut self, next_step_clocks_envelope: bool) {
        self.automatic_updates_enabled = self.pace != 0;
        self.timer = envelope_timer_reload(self.pace) + u8::from(next_step_clocks_envelope);
        self.current_volume = self.initial_volume;
    }

    pub(in crate::apu) fn clock(&mut self) {
        clock_envelope_unit(
            self.pace,
            self.increase,
            &mut self.timer,
            &mut self.current_volume,
            &mut self.automatic_updates_enabled,
        );
    }
}

pub(super) fn wave_ram_mmio_policy(console_model: ConsoleModel) -> WaveRamMmioPolicy {
    if console_model.is_dmg_family() {
        WaveRamMmioPolicy::DmgCurrentByteDuringFetchOnly
    } else {
        WaveRamMmioPolicy::DeferredCgbActiveAccess
    }
}

pub(super) fn extra_length_clocking_policy(
    console_model: ConsoleModel,
) -> ExtraLengthClockingPolicy {
    if console_model.is_cgb_family() {
        ExtraLengthClockingPolicy::DeferredCgbRevisionBehavior
    } else {
        ExtraLengthClockingPolicy::CurrentDmgBaseline
    }
}

pub(super) const fn pulse_period_from_registers(period_low: u8, period_high: u8) -> u16 {
    ((((period_high & PERIOD_HIGH_MASK) as u16) << 8) | period_low as u16) & PULSE_PERIOD_MAX
}

pub(super) const fn pulse_length_counter_from_load(value: u8) -> u8 {
    PULSE_LENGTH_COUNTER_RELOAD - (value & PULSE_LENGTH_LOAD_MASK)
}

pub(super) const fn pulse_timer_reload(period_value: u16) -> u16 {
    (2048 - (period_value & PULSE_PERIOD_MAX)) * 4
}

pub(super) const fn wave_length_counter_from_load(value: u8) -> u16 {
    WAVE_LENGTH_COUNTER_RELOAD - value as u16
}

pub(super) const fn wave_timer_reload(period_value: u16) -> u16 {
    (2048 - (period_value & PULSE_PERIOD_MAX)) * 2
}

pub(super) const fn noise_divisor_base(clock_divider_code: u8) -> u32 {
    match clock_divider_code & NOISE_DIVIDER_CODE_MASK {
        0 => 8,
        1 => 16,
        2 => 32,
        3 => 48,
        4 => 64,
        5 => 80,
        6 => 96,
        _ => 112,
    }
}

pub(super) const fn noise_timer_reload(clock_shift: u8, clock_divider_code: u8) -> u32 {
    noise_divisor_base(clock_divider_code) << (clock_shift & NOISE_CLOCK_SHIFT_MASK)
}

pub(super) const fn noise_clocking_suppressed(clock_shift: u8) -> bool {
    clock_shift >= 14
}

pub(super) const fn nrx4_write_context(
    value: u8,
    next_frame_sequencer_step: u8,
) -> Nrx4WriteContext {
    Nrx4WriteContext {
        trigger: value & CHANNEL_TRIGGER_BIT != 0,
        length_enabled: value & LENGTH_ENABLE_BIT != 0,
        next_step_clocks_length: frame_sequencer_step_clocks_length(next_frame_sequencer_step),
        next_step_clocks_envelope: frame_sequencer_step_clocks_envelope(next_frame_sequencer_step),
    }
}

pub(super) fn begin_nrx4_write(
    register: &mut u8,
    value: u8,
    writable_mask: u8,
    next_frame_sequencer_step: u8,
    was_length_enabled: bool,
) -> Nrx4WritePlan {
    *register = value & writable_mask;

    Nrx4WritePlan {
        context: nrx4_write_context(value, next_frame_sequencer_step),
        was_length_enabled,
        trigger_reloaded_zero_length: false,
    }
}

impl Nrx4WritePlan {
    pub(super) fn observe_trigger_reloaded_zero_length(
        &mut self,
        trigger_reloaded_zero_length: bool,
    ) {
        self.trigger_reloaded_zero_length = trigger_reloaded_zero_length;
    }

    pub(super) fn observe_length_enabled_after_trigger(&mut self, length_enabled: bool) {
        self.was_length_enabled = length_enabled;
    }
}

pub(super) const fn envelope_timer_reload(envelope_pace: u8) -> u8 {
    if envelope_pace == 0 { 8 } else { envelope_pace }
}

pub(super) fn decode_envelope_register(
    value: u8,
    initial_volume: &mut u8,
    envelope_increase: &mut bool,
    envelope_pace: &mut u8,
) {
    *initial_volume = (value & ENVELOPE_INITIAL_VOLUME_MASK) >> ENVELOPE_INITIAL_VOLUME_SHIFT;
    *envelope_increase = value & ENVELOPE_DIRECTION_BIT != 0;
    *envelope_pace = value & ENVELOPE_PACE_MASK;
}

pub(super) const fn envelope_write_uses_consistent_zombie_increment(value: u8) -> bool {
    value & (ENVELOPE_DIRECTION_BIT | ENVELOPE_PACE_MASK) == ENVELOPE_DIRECTION_BIT
}

pub(super) fn apply_consistent_zombie_mode_increment(
    active: bool,
    current_volume: &mut u8,
    value: u8,
) {
    // Pan Docs only documents increase+pace=0 as consistent across tested units; the broader
    // zombie-mode matrix remains revision-specific and is tracked separately.
    if !active || !envelope_write_uses_consistent_zombie_increment(value) {
        return;
    }

    *current_volume = (*current_volume + 1) & MAX_ENVELOPE_VOLUME;
}

pub(super) fn clock_envelope_unit(
    envelope_pace: u8,
    envelope_increase: bool,
    envelope_timer: &mut u8,
    current_volume: &mut u8,
    envelope_automatic_updates_enabled: &mut bool,
) {
    if envelope_pace == 0 || !*envelope_automatic_updates_enabled {
        return;
    }

    if *envelope_timer > 0 {
        *envelope_timer -= 1;
    }

    if *envelope_timer != 0 {
        return;
    }

    *envelope_timer = envelope_timer_reload(envelope_pace);
    if envelope_increase {
        if *current_volume < MAX_ENVELOPE_VOLUME {
            *current_volume += 1;
        } else {
            *envelope_automatic_updates_enabled = false;
        }
    } else if *current_volume > 0 {
        *current_volume -= 1;
    } else {
        *envelope_automatic_updates_enabled = false;
    }
}

pub(super) fn should_apply_extra_length_clocking_on_enable(
    console_model: ConsoleModel,
    length_enabled: bool,
    length_counter_is_zero: bool,
    was_length_enabled: bool,
    next_step_clocks_length: bool,
    trigger_reloaded_zero_length: bool,
) -> bool {
    if next_step_clocks_length || !length_enabled || length_counter_is_zero {
        return false;
    }

    let enabling_length = !was_length_enabled;
    match extra_length_clocking_policy(console_model) {
        ExtraLengthClockingPolicy::CurrentDmgBaseline => {
            enabling_length || trigger_reloaded_zero_length
        }
        // Pan Docs documents a CGB-02-specific deviation here, but the
        // current ConsoleModel surface does not distinguish CGB revisions.
        // Keep the seam explicit until a revision-scoped oracle can close
        // that gap.
        ExtraLengthClockingPolicy::DeferredCgbRevisionBehavior => {
            enabling_length || trigger_reloaded_zero_length
        }
    }
}

pub(super) fn apply_extra_length_clocking_u8(
    context: ExtraLengthClockingContext,
    length_counter: &mut u8,
    reload_value: u8,
    active: &mut bool,
) {
    if !should_apply_extra_length_clocking_on_enable(
        context.console_model,
        context.length_enabled,
        *length_counter == 0,
        context.was_length_enabled,
        context.next_step_clocks_length,
        context.trigger_reloaded_zero_length,
    ) {
        return;
    }

    *length_counter -= 1;
    if *length_counter == 0 {
        if context.trigger {
            *length_counter = reload_value - 1;
        } else {
            *active = false;
        }
    }
}

pub(super) fn apply_extra_length_clocking_u16(
    context: ExtraLengthClockingContext,
    length_counter: &mut u16,
    reload_value: u16,
    active: &mut bool,
) {
    if !should_apply_extra_length_clocking_on_enable(
        context.console_model,
        context.length_enabled,
        *length_counter == 0,
        context.was_length_enabled,
        context.next_step_clocks_length,
        context.trigger_reloaded_zero_length,
    ) {
        return;
    }

    *length_counter -= 1;
    if *length_counter == 0 {
        if context.trigger {
            *length_counter = reload_value - 1;
        } else {
            *active = false;
        }
    }
}

pub(super) fn clock_length_counter_u8(
    length_enabled: bool,
    length_counter: &mut u8,
    active: &mut bool,
) {
    if !length_enabled || *length_counter == 0 {
        return;
    }

    *length_counter -= 1;
    if *length_counter == 0 {
        *active = false;
    }
}

pub(super) fn clock_length_counter_u16(
    length_enabled: bool,
    length_counter: &mut u16,
    active: &mut bool,
) {
    if !length_enabled || *length_counter == 0 {
        return;
    }

    *length_counter -= 1;
    if *length_counter == 0 {
        *active = false;
    }
}

pub(super) const fn pulse_waveform_high(duty: u8, duty_step: u8) -> bool {
    PULSE_DUTY_PATTERNS[(duty & 0x03) as usize][(duty_step & PULSE_DUTY_STEP_MASK) as usize]
}

pub(super) const fn sweep_pace_from_nr10(nr10: u8) -> u8 {
    (nr10 & SWEEP_PACE_MASK) >> SWEEP_PACE_SHIFT
}

pub(super) const fn sweep_shift_from_nr10(nr10: u8) -> u8 {
    nr10 & SWEEP_SHIFT_MASK
}

pub(super) const fn sweep_decreases_from_nr10(nr10: u8) -> bool {
    nr10 & SWEEP_DIRECTION_BIT != 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct ChannelRuntimeState {
    pub(super) dac_enabled: bool,
    pub(super) active: bool,
}

impl ChannelRuntimeState {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn set_dac_enabled(&mut self, dac_enabled: bool) {
        self.dac_enabled = dac_enabled;

        if !dac_enabled {
            self.active = false;
        }
    }

    pub(super) fn set_active_from_startup(&mut self, active: bool) {
        self.active = self.dac_enabled && active;
    }

    pub(super) fn trigger(&mut self) {
        if self.dac_enabled {
            self.active = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CHANNEL_TRIGGER_BIT, LENGTH_ENABLE_BIT, Nrx4WriteContext, Nrx4WritePlan, begin_nrx4_write,
        nrx4_write_context,
    };

    #[test]
    fn nrx4_context_decodes_trigger_length_and_next_step_clocks() {
        assert_eq!(
            nrx4_write_context(CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT, 7),
            Nrx4WriteContext {
                trigger: true,
                length_enabled: true,
                next_step_clocks_length: false,
                next_step_clocks_envelope: true,
            }
        );

        assert_eq!(
            nrx4_write_context(0x00, 6),
            Nrx4WriteContext {
                trigger: false,
                length_enabled: false,
                next_step_clocks_length: true,
                next_step_clocks_envelope: false,
            }
        );
    }

    #[test]
    fn begin_nrx4_write_applies_mask_and_captures_initial_length_state() {
        let mut register = 0x00;

        assert_eq!(
            begin_nrx4_write(
                &mut register,
                CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT | 0x0F,
                0x47,
                6,
                true
            ),
            Nrx4WritePlan {
                context: Nrx4WriteContext {
                    trigger: true,
                    length_enabled: true,
                    next_step_clocks_length: true,
                    next_step_clocks_envelope: false,
                },
                was_length_enabled: true,
                trigger_reloaded_zero_length: false,
            }
        );
        assert_eq!(register, 0x47);
    }

    #[test]
    fn nrx4_write_plan_tracks_trigger_side_effect_observations() {
        let mut register = 0x00;
        let mut plan = begin_nrx4_write(&mut register, CHANNEL_TRIGGER_BIT, 0x47, 0, true);

        plan.observe_trigger_reloaded_zero_length(true);
        plan.observe_length_enabled_after_trigger(false);

        assert!(plan.trigger_reloaded_zero_length);
        assert!(!plan.was_length_enabled);
    }
}
