use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, DerivedEdge};

const CHANNEL_ACTIVE_CH1: u8 = 0x01;
const CHANNEL_ACTIVE_CH2: u8 = 0x02;
const CHANNEL_ACTIVE_CH3: u8 = 0x04;
const CHANNEL_ACTIVE_CH4: u8 = 0x08;
const CHANNEL_ACTIVE_MASK: u8 = 0x0F;
const NR10_FORCED_HIGH_MASK: u8 = 0x80;
const NR11_WRITE_ONLY_MASK: u8 = 0x3F;
const NR13_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
const NR14_READ_MASK: u8 = 0x40;
const NR14_FORCED_HIGH_MASK: u8 = 0xBF;
const NR30_FORCED_HIGH_MASK: u8 = 0x7F;
const NR31_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
const NR32_READ_MASK: u8 = 0x60;
const NR32_FORCED_HIGH_MASK: u8 = 0x9F;
const NR33_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
const NR41_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
const NR44_READ_MASK: u8 = 0x40;
const NR44_FORCED_HIGH_MASK: u8 = 0xBF;
const NR52_FORCED_HIGH_MASK: u8 = 0x70;
const NR52_MASTER_POWER_BIT: u8 = 0x80;
const NR30_DAC_POWER_BIT: u8 = 0x80;
const CHANNEL_TRIGGER_BIT: u8 = 0x80;
const LENGTH_ENABLE_BIT: u8 = 0x40;
const NRX4_WRITABLE_MASK: u8 = 0x47;
const NR44_WRITABLE_MASK: u8 = 0x40;
const PERIOD_HIGH_MASK: u8 = 0x07;
const PULSE_DUTY_MASK: u8 = 0xC0;
const PULSE_LENGTH_LOAD_MASK: u8 = 0x3F;
const SWEEP_PACE_MASK: u8 = 0x70;
const SWEEP_DIRECTION_BIT: u8 = 0x08;
const SWEEP_SHIFT_MASK: u8 = 0x07;
const ENVELOPE_INITIAL_VOLUME_MASK: u8 = 0xF0;
const ENVELOPE_DIRECTION_BIT: u8 = 0x08;
const ENVELOPE_PACE_MASK: u8 = 0x07;
const PULSE_LENGTH_COUNTER_RELOAD: u8 = 64;
const WAVE_LENGTH_COUNTER_RELOAD: u16 = 256;
const PULSE_PERIOD_MAX: u16 = 0x07FF;
const MAX_ENVELOPE_VOLUME: u8 = 0x0F;
const WAVE_RAM_LEN: usize = 0x10;
const WAVE_SAMPLE_COUNT: u8 = 32;
const WAVE_RAM_INACCESSIBLE_READ_VALUE: u8 = 0xFF;
const WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES: u16 = 6;
const NOISE_LFSR_INITIAL_STATE: u16 = 0x7FFF;
const ANALOG_ONE: i32 = 15_000_000;
const DAC_ANALOG_STEP: i32 = 2_000_000;
const HPF_CHARGE_FACTOR_NUMERATOR: i64 = 999_958;
const HPF_CHARGE_FACTOR_DENOMINATOR: i64 = 1_000_000;

const PULSE_DUTY_PATTERNS: [[bool; 8]; 4] = [
    [false, false, false, false, false, false, false, true],
    [true, false, false, false, false, false, false, true],
    [true, false, false, false, false, true, true, true],
    [false, true, true, true, true, true, true, false],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WaveRamStartupPolicy {
    #[default]
    DeterministicZeroed,
}

impl WaveRamStartupPolicy {
    pub const fn initial_bytes(self) -> [u8; WAVE_RAM_LEN] {
        match self {
            Self::DeterministicZeroed => [0; WAVE_RAM_LEN],
        }
    }
}

pub(crate) const fn div_apu_phase_from_system_counter(system_counter: u16) -> u8 {
    ((system_counter >> 13) & 0x07) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApuStartupState {
    pub powered: bool,
    pub nr10: u8,
    pub nr11: u8,
    pub nr12: u8,
    pub nr13: u8,
    pub nr14: u8,
    pub nr21: u8,
    pub nr22: u8,
    pub nr23: u8,
    pub nr24: u8,
    pub nr30: u8,
    pub nr31: u8,
    pub nr32: u8,
    pub nr33: u8,
    pub nr34: u8,
    pub nr41: u8,
    pub nr42: u8,
    pub nr43: u8,
    pub nr44: u8,
    pub nr50: u8,
    pub nr51: u8,
    pub channel_active_mask: u8,
    pub div_apu: u8,
    pub wave_ram_startup_policy: WaveRamStartupPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apu {
    console_model: ConsoleModel,
    status: ApuStatus,
    master: MasterControlState,
    frame_sequencer: FrameSequencerState,
    skip_next_div_apu_edge: bool,
    output_path: OutputPathState,
    channel_1: Channel1State,
    channel_2: Channel2State,
    channel_3: Channel3State,
    channel_4: Channel4State,
    wave_ram_startup_policy: WaveRamStartupPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApuSnapshot {
    pub console_model: ConsoleModel,
    pub status: ApuStatus,
    pub powered: bool,
    pub nr50: u8,
    pub nr51: u8,
    pub channel_active_mask: u8,
    pub channel_dac_mask: u8,
    pub div_apu: u8,
    pub wave_ram: [u8; WAVE_RAM_LEN],
    pub wave_ram_startup_policy: WaveRamStartupPolicy,
    pub output: ApuOutputSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApuStereoOutputSnapshot {
    pub left: i32,
    pub right: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApuHpfCapacitorSnapshot {
    pub left: i64,
    pub right: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApuOutputSnapshot {
    pub channel_digital_outputs: [u8; 4],
    pub channel_dac_outputs: [i32; 4],
    pub mixer_output: ApuStereoOutputSnapshot,
    pub master_output: ApuStereoOutputSnapshot,
    pub hpf_output: ApuStereoOutputSnapshot,
    pub hpf_capacitor: ApuHpfCapacitorSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct MasterControlState {
    powered: bool,
    nr50: u8,
    nr51: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct OutputPathState {
    hpf_capacitor: ApuHpfCapacitorSnapshot,
    current_output: ApuStereoOutputSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FrameSequencerState {
    step: u8,
    length_clock_count: u64,
    sweep_clock_count: u64,
    envelope_clock_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FrameSequencerClocks {
    length: bool,
    sweep: bool,
    envelope: bool,
}

impl ApuStereoOutputSnapshot {
    const fn new(left: i32, right: i32) -> Self {
        Self { left, right }
    }
}

impl OutputPathState {
    fn preview(&mut self, input: ApuStereoOutputSnapshot, any_dac_enabled: bool) {
        if any_dac_enabled {
            self.current_output = ApuStereoOutputSnapshot::new(
                (input.left as i64 - self.hpf_capacitor.left) as i32,
                (input.right as i64 - self.hpf_capacitor.right) as i32,
            );
        } else {
            self.current_output = ApuStereoOutputSnapshot::default();
        }
    }

    fn tick(&mut self, input: ApuStereoOutputSnapshot, any_dac_enabled: bool) {
        if !any_dac_enabled {
            self.current_output = ApuStereoOutputSnapshot::default();
            return;
        }

        let left_output = input.left as i64 - self.hpf_capacitor.left;
        let right_output = input.right as i64 - self.hpf_capacitor.right;

        self.current_output = ApuStereoOutputSnapshot::new(left_output as i32, right_output as i32);
        self.hpf_capacitor.left = input.left as i64
            - (left_output * HPF_CHARGE_FACTOR_NUMERATOR) / HPF_CHARGE_FACTOR_DENOMINATOR;
        self.hpf_capacitor.right = input.right as i64
            - (right_output * HPF_CHARGE_FACTOR_NUMERATOR) / HPF_CHARGE_FACTOR_DENOMINATOR;
    }
}

impl FrameSequencerState {
    fn apply_startup_phase(&mut self, div_apu: u8) {
        self.step = div_apu & 0x07;
        self.length_clock_count = 0;
        self.sweep_clock_count = 0;
        self.envelope_clock_count = 0;
    }

    fn advance(&mut self) -> FrameSequencerClocks {
        let clocks = match self.step {
            0 => FrameSequencerClocks {
                length: true,
                ..FrameSequencerClocks::default()
            },
            2 | 6 => FrameSequencerClocks {
                length: true,
                sweep: true,
                ..FrameSequencerClocks::default()
            },
            4 => FrameSequencerClocks {
                length: true,
                ..FrameSequencerClocks::default()
            },
            7 => FrameSequencerClocks {
                envelope: true,
                ..FrameSequencerClocks::default()
            },
            _ => FrameSequencerClocks::default(),
        };

        if clocks.length {
            self.length_clock_count += 1;
        }
        if clocks.sweep {
            self.sweep_clock_count += 1;
        }
        if clocks.envelope {
            self.envelope_clock_count += 1;
        }

        self.step = (self.step + 1) & 0x07;
        clocks
    }
}

const fn pulse_period_from_registers(period_low: u8, period_high: u8) -> u16 {
    ((((period_high & PERIOD_HIGH_MASK) as u16) << 8) | period_low as u16) & PULSE_PERIOD_MAX
}

const fn pulse_length_counter_from_load(value: u8) -> u8 {
    PULSE_LENGTH_COUNTER_RELOAD - (value & PULSE_LENGTH_LOAD_MASK)
}

const fn pulse_timer_reload(period_value: u16) -> u16 {
    (2048 - (period_value & PULSE_PERIOD_MAX)) * 4
}

const fn wave_length_counter_from_load(value: u8) -> u16 {
    WAVE_LENGTH_COUNTER_RELOAD - value as u16
}

const fn wave_timer_reload(period_value: u16) -> u16 {
    (2048 - (period_value & PULSE_PERIOD_MAX)) * 2
}

const fn noise_divisor_base(clock_divider_code: u8) -> u32 {
    match clock_divider_code & 0x07 {
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

const fn noise_timer_reload(clock_shift: u8, clock_divider_code: u8) -> u32 {
    noise_divisor_base(clock_divider_code) << (clock_shift & 0x0F)
}

const fn frame_sequencer_step_clocks_length(step: u8) -> bool {
    matches!(step & 0x07, 0 | 2 | 4 | 6)
}

const fn frame_sequencer_step_clocks_envelope(step: u8) -> bool {
    step & 0x07 == 7
}

const fn envelope_timer_reload(envelope_pace: u8) -> u8 {
    if envelope_pace == 0 { 8 } else { envelope_pace }
}

const fn dac_analog_output(digital_output: u8) -> i32 {
    ANALOG_ONE - ((digital_output & 0x0F) as i32) * DAC_ANALOG_STEP
}

const fn nr50_left_volume_factor(nr50: u8) -> i32 {
    (((nr50 >> 4) & 0x07) as i32) + 1
}

const fn nr50_right_volume_factor(nr50: u8) -> i32 {
    ((nr50 & 0x07) as i32) + 1
}

const fn sweep_pace_from_nr10(nr10: u8) -> u8 {
    (nr10 & SWEEP_PACE_MASK) >> 4
}

const fn sweep_shift_from_nr10(nr10: u8) -> u8 {
    nr10 & SWEEP_SHIFT_MASK
}

const fn sweep_decreases_from_nr10(nr10: u8) -> bool {
    nr10 & SWEEP_DIRECTION_BIT != 0
}

const fn pulse_waveform_high(duty: u8, duty_step: u8) -> bool {
    PULSE_DUTY_PATTERNS[(duty & 0x03) as usize][(duty_step & 0x07) as usize]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ChannelRuntimeState {
    dac_enabled: bool,
    active: bool,
}

impl ChannelRuntimeState {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn set_dac_enabled(&mut self, dac_enabled: bool) {
        self.dac_enabled = dac_enabled;

        if !dac_enabled {
            self.active = false;
        }
    }

    fn set_active_from_startup(&mut self, active: bool) {
        self.active = self.dac_enabled && active;
    }

    fn trigger(&mut self) {
        if self.dac_enabled {
            self.active = true;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct PulseChannelState {
    runtime: ChannelRuntimeState,
    duty: u8,
    duty_step: u8,
    first_trigger_after_power_on_pending: bool,
    suppress_initial_trigger_output: bool,
    period_timer: u16,
    length_counter: u8,
    length_enabled: bool,
    initial_volume: u8,
    envelope_increase: bool,
    envelope_pace: u8,
    envelope_timer: u8,
    current_volume: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PulseStartupState {
    length_duty_value: u8,
    envelope_value: u8,
    nrx4: u8,
    period_value: u16,
    runtime: ChannelRuntimeState,
    first_trigger_after_power_on_pending: bool,
}

impl PulseChannelState {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn clear_preserving_length(&mut self) {
        let length_counter = self.length_counter;
        self.clear();
        self.length_counter = length_counter;
    }

    fn mark_powered_on(&mut self) {
        self.first_trigger_after_power_on_pending = true;
        self.suppress_initial_trigger_output = false;
    }

    fn apply_length_duty_write(&mut self, value: u8) {
        self.duty = (value & PULSE_DUTY_MASK) >> 6;
        self.length_counter = pulse_length_counter_from_load(value);
    }

    fn apply_envelope_write(&mut self, value: u8) {
        self.initial_volume = (value & ENVELOPE_INITIAL_VOLUME_MASK) >> 4;
        self.envelope_increase = value & ENVELOPE_DIRECTION_BIT != 0;
        self.envelope_pace = value & ENVELOPE_PACE_MASK;
    }

    fn apply_length_enable(&mut self, value: u8) {
        self.length_enabled = value & LENGTH_ENABLE_BIT != 0;
    }

    fn apply_extra_length_clocking_on_enable(
        &mut self,
        was_length_enabled: bool,
        next_step_clocks_length: bool,
        trigger: bool,
        trigger_reloaded_zero_length: bool,
    ) {
        if next_step_clocks_length || !self.length_enabled || self.length_counter == 0 {
            return;
        }

        let enabling_length = !was_length_enabled;
        if !enabling_length && !trigger_reloaded_zero_length {
            return;
        }

        self.length_counter -= 1;
        if self.length_counter == 0 {
            if trigger {
                self.length_counter = PULSE_LENGTH_COUNTER_RELOAD - 1;
            } else {
                self.runtime.active = false;
            }
        }
    }

    fn apply_powered_startup(&mut self, startup: PulseStartupState) {
        self.clear();
        self.apply_length_duty_write(startup.length_duty_value);
        self.apply_envelope_write(startup.envelope_value);
        self.apply_length_enable(startup.nrx4);
        self.first_trigger_after_power_on_pending = startup.first_trigger_after_power_on_pending;
        self.period_timer = pulse_timer_reload(startup.period_value);
        self.envelope_timer = envelope_timer_reload(self.envelope_pace);
        self.current_volume = self.initial_volume;
        self.runtime = startup.runtime;
    }

    fn trigger(&mut self, period_value: u16, next_step_clocks_envelope: bool) -> bool {
        let reloaded_zero_length = self.length_counter == 0;
        if self.length_counter == 0 {
            self.length_counter = PULSE_LENGTH_COUNTER_RELOAD;
            self.length_enabled = false;
        }

        let preserved_period_timer_low_bits = self.period_timer & 0x03;
        self.period_timer = pulse_timer_reload(period_value) | preserved_period_timer_low_bits;
        self.envelope_timer =
            envelope_timer_reload(self.envelope_pace) + u8::from(next_step_clocks_envelope);
        self.current_volume = self.initial_volume;
        if self.first_trigger_after_power_on_pending {
            self.suppress_initial_trigger_output = true;
            self.first_trigger_after_power_on_pending = false;
        }
        self.runtime.trigger();
        reloaded_zero_length
    }

    fn tick_fast_timer(&mut self, period_value: u16) {
        if !self.runtime.active {
            return;
        }

        if self.period_timer > 0 {
            self.period_timer -= 1;
        }

        if self.period_timer == 0 {
            self.period_timer = pulse_timer_reload(period_value);
            self.duty_step = (self.duty_step + 1) & 0x07;
            self.suppress_initial_trigger_output = false;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_enabled || self.length_counter == 0 {
            return;
        }

        self.length_counter -= 1;
        if self.length_counter == 0 {
            self.runtime.active = false;
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_pace == 0 || !self.runtime.active {
            return;
        }

        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }

        if self.envelope_timer != 0 {
            return;
        }

        self.envelope_timer = envelope_timer_reload(self.envelope_pace);
        if self.envelope_increase {
            if self.current_volume < MAX_ENVELOPE_VOLUME {
                self.current_volume += 1;
            }
        } else if self.current_volume > 0 {
            self.current_volume -= 1;
        }
    }

    fn current_digital_output(&self) -> u8 {
        if !self.runtime.active {
            return 0;
        }

        if self.suppress_initial_trigger_output {
            return 0;
        }

        if pulse_waveform_high(self.duty, self.duty_step) {
            self.current_volume
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Channel1SweepState {
    timer: u8,
    phase: u8,
    enabled: bool,
    shadow_period: u16,
    completed_addend: u16,
    negate_calculated_since_trigger: bool,
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
        old_nr10: u8,
        new_nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        if sweep_decreases_from_nr10(old_nr10)
            && !sweep_decreases_from_nr10(new_nr10)
            && self.negate_calculated_since_trigger
            && self.shadow_period + self.completed_addend + 1 > PULSE_PERIOD_MAX
        {
            runtime.active = false;
        }

        self.maybe_fire_sweep_boundary(new_nr10, nr13, nr14, runtime);
    }

    fn trigger(&mut self, nr10: u8, period_value: u16, runtime: &mut ChannelRuntimeState) {
        self.shadow_period = period_value;
        self.phase = Self::phase_from_pace(sweep_pace_from_nr10(nr10));
        self.timer = Self::timer_from_phase(self.phase);
        self.enabled = sweep_pace_from_nr10(nr10) != 0 || sweep_shift_from_nr10(nr10) != 0;
        self.completed_addend = 0;
        self.negate_calculated_since_trigger = false;

        if self
            .calculate_candidate_sum(nr10, false)
            .is_some_and(|candidate| {
                candidate > PULSE_PERIOD_MAX && !sweep_decreases_from_nr10(nr10)
            })
        {
            runtime.active = false;
        }
    }

    fn clock(&mut self, nr10: u8, nr13: &mut u8, nr14: &mut u8, runtime: &mut ChannelRuntimeState) {
        if !self.enabled || !runtime.active {
            return;
        }

        self.phase = (self.phase + 1) & 0x07;
        self.timer = Self::timer_from_phase(self.phase);
        if self.phase != 7 {
            return;
        }

        self.maybe_fire_sweep_boundary(nr10, nr13, nr14, runtime);
    }

    fn maybe_fire_sweep_boundary(
        &mut self,
        nr10: u8,
        nr13: &mut u8,
        nr14: &mut u8,
        runtime: &mut ChannelRuntimeState,
    ) {
        let pace = sweep_pace_from_nr10(nr10);
        if self.phase != 7 || pace == 0 || !self.enabled || !runtime.active {
            return;
        }

        self.phase = Self::phase_from_pace(pace);
        self.timer = Self::timer_from_phase(self.phase);

        let shift = sweep_shift_from_nr10(nr10);
        let Some(candidate_sum) = self.calculate_candidate_sum(nr10, true) else {
            return;
        };

        if !sweep_decreases_from_nr10(nr10) && candidate_sum > PULSE_PERIOD_MAX {
            runtime.active = false;
            return;
        }

        if shift == 0 {
            return;
        }

        let candidate = candidate_sum & PULSE_PERIOD_MAX;
        self.shadow_period = candidate;
        *nr13 = candidate as u8;
        *nr14 = (*nr14 & !PERIOD_HIGH_MASK) | (((candidate >> 8) as u8) & PERIOD_HIGH_MASK);

        if self
            .calculate_candidate_sum(nr10, true)
            .is_some_and(|next_candidate| {
                next_candidate > PULSE_PERIOD_MAX && !sweep_decreases_from_nr10(nr10)
            })
        {
            runtime.active = false;
        }
    }

    const fn phase_from_pace(pace: u8) -> u8 {
        pace ^ 0x07
    }

    const fn timer_from_phase(phase: u8) -> u8 {
        if phase == 7 { 8 } else { 7 - (phase & 0x07) }
    }

    fn calculate_candidate_sum(&mut self, nr10: u8, allow_shift_zero: bool) -> Option<u16> {
        let shift = sweep_shift_from_nr10(nr10);
        if shift == 0 && !allow_shift_zero {
            return None;
        }

        let delta = self.shadow_period >> shift;
        let decreases = sweep_decreases_from_nr10(nr10);
        self.completed_addend = if decreases {
            (!delta) & PULSE_PERIOD_MAX
        } else {
            delta
        };
        self.negate_calculated_since_trigger |= decreases;

        Some(self.shadow_period + self.completed_addend + u16::from(decreases))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Channel1State {
    nr10: u8,
    nr11: u8,
    nr12: u8,
    nr13: u8,
    nr14: u8,
    pulse: PulseChannelState,
    sweep: Channel1SweepState,
}

impl Channel1State {
    fn read_nr10(&self) -> u8 {
        self.nr10 | NR10_FORCED_HIGH_MASK
    }

    fn read_nr11(&self) -> u8 {
        (self.nr11 & 0xC0) | NR11_WRITE_ONLY_MASK
    }

    fn read_nr14(&self) -> u8 {
        (self.nr14 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK
    }

    fn write_nr10(&mut self, value: u8) {
        let old_nr10 = self.nr10;
        self.nr10 = value & 0x7F;
        self.sweep.write_nr10(
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

    fn write_nr12(&mut self, value: u8) {
        self.nr12 = value;
        self.pulse.apply_envelope_write(value);
        self.pulse
            .runtime
            .set_dac_enabled(self.derived_dac_enabled());
    }

    fn write_nr13(&mut self, value: u8) {
        self.nr13 = value;
    }

    fn write_nr14(&mut self, value: u8, next_frame_sequencer_step: u8) {
        let trigger = value & CHANNEL_TRIGGER_BIT != 0;
        let next_step_clocks_length = frame_sequencer_step_clocks_length(next_frame_sequencer_step);
        let next_step_clocks_envelope =
            frame_sequencer_step_clocks_envelope(next_frame_sequencer_step);
        self.nr14 = value & NRX4_WRITABLE_MASK;
        let mut was_length_enabled = self.pulse.length_enabled;
        let mut trigger_reloaded_zero_length = false;

        if trigger {
            trigger_reloaded_zero_length = self.trigger(next_step_clocks_envelope);
            was_length_enabled = self.pulse.length_enabled;
        }

        self.pulse.apply_length_enable(self.nr14);
        self.pulse.apply_extra_length_clocking_on_enable(
            was_length_enabled,
            next_step_clocks_length,
            trigger,
            trigger_reloaded_zero_length,
        );
    }

    fn apply_powered_startup(
        &mut self,
        nr10: u8,
        nr11: u8,
        nr12: u8,
        nr13: u8,
        nr14: u8,
        active: bool,
    ) {
        self.nr10 = nr10 & 0x7F;
        self.nr11 = nr11;
        self.nr12 = nr12;
        self.nr13 = nr13;
        self.nr14 = nr14 & NRX4_WRITABLE_MASK;
        let mut runtime = ChannelRuntimeState::default();
        runtime.set_dac_enabled(self.derived_dac_enabled());
        runtime.set_active_from_startup(active);
        self.pulse.apply_powered_startup(PulseStartupState {
            length_duty_value: self.nr11,
            envelope_value: self.nr12,
            nrx4: self.nr14,
            period_value: self.period_value(),
            runtime,
            first_trigger_after_power_on_pending: !active,
        });
        self.sweep
            .apply_powered_startup(self.nr10, self.period_value(), self.pulse.runtime.active);
    }

    fn write_length_while_powered_off(&mut self, value: u8) {
        self.pulse.length_counter = pulse_length_counter_from_load(value);
    }

    fn clear_registers(&mut self) {
        self.nr10 = 0;
        self.nr11 = 0;
        self.nr12 = 0;
        self.nr13 = 0;
        self.nr14 = 0;
        self.pulse.clear();
        self.sweep.clear();
    }

    fn power_off_registers(&mut self, console_model: ConsoleModel) {
        if console_model.is_dmg_family() {
            self.nr10 = 0;
            self.nr11 = 0;
            self.nr12 = 0;
            self.nr13 = 0;
            self.nr14 = 0;
            self.pulse.clear_preserving_length();
            self.sweep.clear();
            return;
        }

        self.clear_registers();
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr12 & 0xF8 != 0
    }

    fn period_value(&self) -> u16 {
        pulse_period_from_registers(self.nr13, self.nr14)
    }

    fn trigger(&mut self, next_step_clocks_envelope: bool) -> bool {
        let period_value = self.period_value();
        let trigger_reloaded_zero_length =
            self.pulse.trigger(period_value, next_step_clocks_envelope);
        self.sweep
            .trigger(self.nr10, period_value, &mut self.pulse.runtime);
        trigger_reloaded_zero_length
    }

    fn tick_fast_timer(&mut self) {
        self.pulse.tick_fast_timer(self.period_value());
    }

    fn clock_length(&mut self) {
        self.pulse.clock_length();
    }

    fn clock_envelope(&mut self) {
        self.pulse.clock_envelope();
    }

    fn clock_sweep(&mut self) {
        self.sweep.clock(
            self.nr10,
            &mut self.nr13,
            &mut self.nr14,
            &mut self.pulse.runtime,
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Channel2State {
    nr21: u8,
    nr22: u8,
    nr23: u8,
    nr24: u8,
    pulse: PulseChannelState,
}

impl Channel2State {
    fn read_nr21(&self) -> u8 {
        (self.nr21 & 0xC0) | NR11_WRITE_ONLY_MASK
    }

    fn read_nr24(&self) -> u8 {
        (self.nr24 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK
    }

    fn write_nr21(&mut self, value: u8) {
        self.nr21 = value;
        self.pulse.apply_length_duty_write(value);
    }

    fn write_nr22(&mut self, value: u8) {
        self.nr22 = value;
        self.pulse.apply_envelope_write(value);
        self.pulse
            .runtime
            .set_dac_enabled(self.derived_dac_enabled());
    }

    fn write_nr23(&mut self, value: u8) {
        self.nr23 = value;
    }

    fn write_nr24(&mut self, value: u8, next_frame_sequencer_step: u8) {
        let trigger = value & CHANNEL_TRIGGER_BIT != 0;
        let next_step_clocks_length = frame_sequencer_step_clocks_length(next_frame_sequencer_step);
        let next_step_clocks_envelope =
            frame_sequencer_step_clocks_envelope(next_frame_sequencer_step);
        self.nr24 = value & NRX4_WRITABLE_MASK;
        let mut was_length_enabled = self.pulse.length_enabled;
        let mut trigger_reloaded_zero_length = false;

        if trigger {
            trigger_reloaded_zero_length = self.trigger(next_step_clocks_envelope);
            was_length_enabled = self.pulse.length_enabled;
        }

        self.pulse.apply_length_enable(self.nr24);
        self.pulse.apply_extra_length_clocking_on_enable(
            was_length_enabled,
            next_step_clocks_length,
            trigger,
            trigger_reloaded_zero_length,
        );
    }

    fn apply_powered_startup(&mut self, nr21: u8, nr22: u8, nr23: u8, nr24: u8, active: bool) {
        self.nr21 = nr21;
        self.nr22 = nr22;
        self.nr23 = nr23;
        self.nr24 = nr24 & NRX4_WRITABLE_MASK;
        let mut runtime = ChannelRuntimeState::default();
        runtime.set_dac_enabled(self.derived_dac_enabled());
        runtime.set_active_from_startup(active);
        self.pulse.apply_powered_startup(PulseStartupState {
            length_duty_value: self.nr21,
            envelope_value: self.nr22,
            nrx4: self.nr24,
            period_value: self.period_value(),
            runtime,
            first_trigger_after_power_on_pending: !active,
        });
    }

    fn write_length_while_powered_off(&mut self, value: u8) {
        self.pulse.length_counter = pulse_length_counter_from_load(value);
    }

    fn clear_registers(&mut self) {
        self.nr21 = 0;
        self.nr22 = 0;
        self.nr23 = 0;
        self.nr24 = 0;
        self.pulse.clear();
    }

    fn power_off_registers(&mut self, console_model: ConsoleModel) {
        if console_model.is_dmg_family() {
            self.nr21 = 0;
            self.nr22 = 0;
            self.nr23 = 0;
            self.nr24 = 0;
            self.pulse.clear_preserving_length();
            return;
        }

        self.clear_registers();
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr22 & 0xF8 != 0
    }

    fn period_value(&self) -> u16 {
        pulse_period_from_registers(self.nr23, self.nr24)
    }

    fn trigger(&mut self, next_step_clocks_envelope: bool) -> bool {
        self.pulse
            .trigger(self.period_value(), next_step_clocks_envelope)
    }

    fn tick_fast_timer(&mut self) {
        self.pulse.tick_fast_timer(self.period_value());
    }

    fn clock_length(&mut self) {
        self.pulse.clock_length();
    }

    fn clock_envelope(&mut self) {
        self.pulse.clock_envelope();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Channel3State {
    nr30: u8,
    nr31: u8,
    nr32: u8,
    nr33: u8,
    nr34: u8,
    wave_ram: [u8; WAVE_RAM_LEN],
    runtime: ChannelRuntimeState,
    sample_index: u8,
    sample_buffer: u8,
    period_timer: u16,
    length_counter: u16,
    length_enabled: bool,
    wave_ram_access_window_byte_index: Option<u8>,
}

impl Channel3State {
    fn read_nr30(&self) -> u8 {
        (self.nr30 & NR30_DAC_POWER_BIT) | NR30_FORCED_HIGH_MASK
    }

    fn read_nr32(&self) -> u8 {
        (self.nr32 & NR32_READ_MASK) | NR32_FORCED_HIGH_MASK
    }

    fn read_nr34(&self) -> u8 {
        (self.nr34 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK
    }

    fn write_nr30(&mut self, value: u8) {
        self.nr30 = value & NR30_DAC_POWER_BIT;
        self.runtime.set_dac_enabled(self.derived_dac_enabled());
    }

    fn write_nr31(&mut self, value: u8) {
        self.nr31 = value;
        self.length_counter = wave_length_counter_from_load(value);
    }

    fn write_nr32(&mut self, value: u8) {
        self.nr32 = value & NR32_READ_MASK;
    }

    fn write_nr33(&mut self, value: u8) {
        self.nr33 = value;
    }

    fn write_nr34(
        &mut self,
        value: u8,
        console_model: ConsoleModel,
        next_frame_sequencer_step: u8,
    ) {
        let trigger = value & CHANNEL_TRIGGER_BIT != 0;
        let next_step_clocks_length = frame_sequencer_step_clocks_length(next_frame_sequencer_step);
        self.nr34 = value & NRX4_WRITABLE_MASK;
        let was_length_enabled = self.length_enabled;
        let mut trigger_reloaded_zero_length = false;

        if trigger {
            trigger_reloaded_zero_length = self.trigger(console_model);
        }

        self.length_enabled = self.nr34 & LENGTH_ENABLE_BIT != 0;
        self.apply_extra_length_clocking_on_enable(
            was_length_enabled,
            next_step_clocks_length,
            trigger,
            trigger_reloaded_zero_length,
        );
    }

    fn apply_powered_startup(
        &mut self,
        nr30: u8,
        nr31: u8,
        nr32: u8,
        nr33: u8,
        nr34: u8,
        active: bool,
    ) {
        self.nr30 = nr30 & NR30_DAC_POWER_BIT;
        self.nr31 = nr31;
        self.nr32 = nr32 & NR32_READ_MASK;
        self.nr33 = nr33;
        self.nr34 = nr34 & NRX4_WRITABLE_MASK;
        self.sample_index = 0;
        self.sample_buffer = 0;
        self.period_timer = wave_timer_reload(self.period_value());
        self.length_counter = wave_length_counter_from_load(self.nr31);
        self.length_enabled = self.nr34 & LENGTH_ENABLE_BIT != 0;
        self.wave_ram_access_window_byte_index = None;
        self.runtime.clear();
        self.runtime.set_dac_enabled(self.derived_dac_enabled());
        self.runtime.set_active_from_startup(active);
    }

    fn clear_registers(&mut self) {
        self.nr30 = 0;
        self.nr31 = 0;
        self.nr32 = 0;
        self.nr33 = 0;
        self.nr34 = 0;
        self.sample_index = 0;
        self.sample_buffer = 0;
        self.period_timer = 0;
        self.length_counter = 0;
        self.length_enabled = false;
        self.wave_ram_access_window_byte_index = None;
        self.runtime.clear();
    }

    fn write_length_while_powered_off(&mut self, value: u8) {
        self.length_counter = wave_length_counter_from_load(value);
    }

    fn power_off_registers(&mut self, console_model: ConsoleModel) {
        let preserved_length = if console_model.is_dmg_family() {
            self.length_counter
        } else {
            0
        };
        self.clear_registers();
        self.length_counter = preserved_length;
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr30 & NR30_DAC_POWER_BIT != 0
    }

    fn period_value(&self) -> u16 {
        pulse_period_from_registers(self.nr33, self.nr34)
    }

    fn begin_t_cycle(&mut self) {
        self.wave_ram_access_window_byte_index = None;
    }

    fn apply_extra_length_clocking_on_enable(
        &mut self,
        was_length_enabled: bool,
        next_step_clocks_length: bool,
        trigger: bool,
        trigger_reloaded_zero_length: bool,
    ) {
        if next_step_clocks_length || !self.length_enabled || self.length_counter == 0 {
            return;
        }

        let enabling_length = !was_length_enabled;
        if !enabling_length && !trigger_reloaded_zero_length {
            return;
        }

        self.length_counter -= 1;
        if self.length_counter == 0 {
            if trigger {
                self.length_counter = WAVE_LENGTH_COUNTER_RELOAD - 1;
            } else {
                self.runtime.active = false;
            }
        }
    }

    fn trigger(&mut self, console_model: ConsoleModel) -> bool {
        self.apply_dmg_retrigger_wave_ram_corruption(console_model);
        let reloaded_zero_length = self.length_counter == 0;

        if self.length_counter == 0 {
            self.length_counter = WAVE_LENGTH_COUNTER_RELOAD;
            self.length_enabled = false;
        }

        self.period_timer =
            wave_timer_reload(self.period_value()) + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES;
        self.sample_index = 0;
        self.runtime.trigger();
        reloaded_zero_length
    }

    fn tick_fast_timer(&mut self) {
        if !self.runtime.active {
            return;
        }

        if self.period_timer > 0 {
            self.period_timer -= 1;
        }

        if self.period_timer == 0 {
            self.period_timer = wave_timer_reload(self.period_value());
            self.advance_sample();
        }
    }

    fn clock_length(&mut self) {
        if !self.length_enabled || self.length_counter == 0 {
            return;
        }

        self.length_counter -= 1;
        if self.length_counter == 0 {
            self.runtime.active = false;
        }
    }

    fn read_wave_ram(&self, console_model: ConsoleModel, index: usize) -> u8 {
        if let Some(active_wave_ram_byte_index) =
            self.active_wave_ram_access_byte_index(console_model)
        {
            return self.wave_ram[active_wave_ram_byte_index];
        }

        if self.runtime.active && console_model.is_dmg_family() {
            return WAVE_RAM_INACCESSIBLE_READ_VALUE;
        }

        self.wave_ram[index]
    }

    fn write_wave_ram(&mut self, console_model: ConsoleModel, index: usize, value: u8) {
        if let Some(active_wave_ram_byte_index) =
            self.active_wave_ram_access_byte_index(console_model)
        {
            self.wave_ram[active_wave_ram_byte_index] = value;
            return;
        }

        if self.runtime.active && console_model.is_dmg_family() {
            return;
        }

        self.wave_ram[index] = value;
    }

    fn initialize_wave_ram(&mut self, wave_ram: [u8; WAVE_RAM_LEN]) {
        self.wave_ram = wave_ram;
    }

    fn active_wave_ram_access_byte_index(&self, console_model: ConsoleModel) -> Option<usize> {
        if self.runtime.active && console_model.is_dmg_family() {
            return self
                .wave_ram_access_window_byte_index
                .map(|byte_index| byte_index as usize);
        }

        None
    }

    fn current_wave_ram_byte_index(&self) -> usize {
        ((self.sample_index >> 1) as usize) % WAVE_RAM_LEN
    }

    fn advance_sample(&mut self) {
        self.sample_index = (self.sample_index + 1) % WAVE_SAMPLE_COUNT;
        let current_wave_ram_byte_index = self.current_wave_ram_byte_index() as u8;
        self.wave_ram_access_window_byte_index = Some(current_wave_ram_byte_index);
        self.sample_buffer = self.wave_sample(self.sample_index);
    }

    fn apply_dmg_retrigger_wave_ram_corruption(&mut self, console_model: ConsoleModel) {
        if !console_model.is_dmg_family() || !self.runtime.active || self.period_timer != 2 {
            return;
        }

        let current_byte_index = (((self.sample_index as usize) + 1) >> 1) % WAVE_RAM_LEN;

        if current_byte_index < 4 {
            self.wave_ram[0] = self.wave_ram[current_byte_index];
            return;
        }

        let block_start = current_byte_index & !0x03;
        let aligned_block = [
            self.wave_ram[block_start],
            self.wave_ram[block_start + 1],
            self.wave_ram[block_start + 2],
            self.wave_ram[block_start + 3],
        ];
        self.wave_ram[..aligned_block.len()].copy_from_slice(&aligned_block);
    }

    fn wave_sample(&self, sample_index: u8) -> u8 {
        let byte = self.wave_ram[((sample_index >> 1) as usize) % WAVE_RAM_LEN];
        if sample_index & 0x01 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        }
    }

    fn current_digital_output(&self) -> u8 {
        if !self.runtime.active {
            return 0;
        }

        match (self.nr32 & NR32_READ_MASK) >> 5 {
            0 => 0,
            1 => self.sample_buffer,
            2 => self.sample_buffer >> 1,
            3 => self.sample_buffer >> 2,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Channel4State {
    nr41: u8,
    nr42: u8,
    nr43: u8,
    nr44: u8,
    runtime: ChannelRuntimeState,
    length_counter: u8,
    length_enabled: bool,
    initial_volume: u8,
    envelope_increase: bool,
    envelope_pace: u8,
    envelope_timer: u8,
    current_volume: u8,
    clock_shift: u8,
    short_width_mode: bool,
    clock_divider_code: u8,
    period_timer: u32,
    lfsr_state: u16,
}

impl Channel4State {
    fn read_nr44(&self) -> u8 {
        (self.nr44 & NR44_READ_MASK) | NR44_FORCED_HIGH_MASK
    }

    fn write_nr41(&mut self, value: u8) {
        self.nr41 = value;
        self.length_counter = pulse_length_counter_from_load(value);
    }

    fn write_nr42(&mut self, value: u8) {
        self.nr42 = value;
        self.apply_envelope_write(value);
        self.runtime.set_dac_enabled(self.derived_dac_enabled());
    }

    fn write_nr43(&mut self, value: u8) {
        self.nr43 = value;
        self.decode_nr43(value);
    }

    fn write_nr44(&mut self, value: u8, next_frame_sequencer_step: u8) {
        let trigger = value & CHANNEL_TRIGGER_BIT != 0;
        let next_step_clocks_length = frame_sequencer_step_clocks_length(next_frame_sequencer_step);
        let next_step_clocks_envelope =
            frame_sequencer_step_clocks_envelope(next_frame_sequencer_step);
        self.nr44 = value & NR44_WRITABLE_MASK;
        let mut was_length_enabled = self.length_enabled;
        let mut trigger_reloaded_zero_length = false;

        if trigger {
            trigger_reloaded_zero_length = self.trigger(next_step_clocks_envelope);
            was_length_enabled = self.length_enabled;
        }

        self.length_enabled = self.nr44 & LENGTH_ENABLE_BIT != 0;
        self.apply_extra_length_clocking_on_enable(
            was_length_enabled,
            next_step_clocks_length,
            trigger,
            trigger_reloaded_zero_length,
        );
    }

    fn apply_powered_startup(&mut self, nr41: u8, nr42: u8, nr43: u8, nr44: u8, active: bool) {
        self.nr41 = nr41;
        self.nr42 = nr42;
        self.nr43 = nr43;
        self.nr44 = nr44 & NR44_WRITABLE_MASK;
        self.length_counter = pulse_length_counter_from_load(self.nr41);
        self.length_enabled = self.nr44 & LENGTH_ENABLE_BIT != 0;
        self.apply_envelope_write(self.nr42);
        self.decode_nr43(self.nr43);
        self.envelope_timer = envelope_timer_reload(self.envelope_pace);
        self.current_volume = self.initial_volume;
        self.period_timer = self.noise_timer_reload();
        self.lfsr_state = NOISE_LFSR_INITIAL_STATE;
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
        self.initial_volume = 0;
        self.envelope_increase = false;
        self.envelope_pace = 0;
        self.envelope_timer = 0;
        self.current_volume = 0;
        self.clock_shift = 0;
        self.short_width_mode = false;
        self.clock_divider_code = 0;
        self.period_timer = 0;
        self.lfsr_state = 0;
        self.runtime.clear();
    }

    fn write_length_while_powered_off(&mut self, value: u8) {
        self.length_counter = pulse_length_counter_from_load(value);
    }

    fn power_off_registers(&mut self, console_model: ConsoleModel) {
        let preserved_length = if console_model.is_dmg_family() {
            self.length_counter
        } else {
            0
        };
        self.clear_registers();
        self.length_counter = preserved_length;
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr42 & 0xF8 != 0
    }

    fn apply_envelope_write(&mut self, value: u8) {
        self.initial_volume = (value & ENVELOPE_INITIAL_VOLUME_MASK) >> 4;
        self.envelope_increase = value & ENVELOPE_DIRECTION_BIT != 0;
        self.envelope_pace = value & ENVELOPE_PACE_MASK;
    }

    fn decode_nr43(&mut self, value: u8) {
        self.clock_shift = (value >> 4) & 0x0F;
        self.short_width_mode = value & 0x08 != 0;
        self.clock_divider_code = value & 0x07;
    }

    fn noise_timer_reload(&self) -> u32 {
        noise_timer_reload(self.clock_shift, self.clock_divider_code)
    }

    fn apply_extra_length_clocking_on_enable(
        &mut self,
        was_length_enabled: bool,
        next_step_clocks_length: bool,
        trigger: bool,
        trigger_reloaded_zero_length: bool,
    ) {
        if next_step_clocks_length || !self.length_enabled || self.length_counter == 0 {
            return;
        }

        let enabling_length = !was_length_enabled;
        if !enabling_length && !trigger_reloaded_zero_length {
            return;
        }

        self.length_counter -= 1;
        if self.length_counter == 0 {
            if trigger {
                self.length_counter = PULSE_LENGTH_COUNTER_RELOAD - 1;
            } else {
                self.runtime.active = false;
            }
        }
    }

    fn trigger(&mut self, next_step_clocks_envelope: bool) -> bool {
        let reloaded_zero_length = self.length_counter == 0;
        if self.length_counter == 0 {
            self.length_counter = PULSE_LENGTH_COUNTER_RELOAD;
            self.length_enabled = false;
        }

        self.period_timer = self.noise_timer_reload();
        self.lfsr_state = NOISE_LFSR_INITIAL_STATE;
        self.envelope_timer =
            envelope_timer_reload(self.envelope_pace) + u8::from(next_step_clocks_envelope);
        self.current_volume = self.initial_volume;
        self.runtime.trigger();
        reloaded_zero_length
    }

    fn tick_fast_timer(&mut self) {
        if !self.runtime.active || self.clock_shift >= 14 {
            return;
        }

        if self.period_timer > 0 {
            self.period_timer -= 1;
        }

        if self.period_timer == 0 {
            self.period_timer = self.noise_timer_reload();
            self.step_lfsr();
        }
    }

    fn step_lfsr(&mut self) {
        let feedback_bit = ((self.lfsr_state & 0x01) ^ ((self.lfsr_state >> 1) & 0x01)) & 0x01;
        self.lfsr_state >>= 1;
        self.lfsr_state = (self.lfsr_state & !(1 << 14)) | (feedback_bit << 14);
        if self.short_width_mode {
            // In short mode the feedback path also overwrites bit 6, so a live
            // 15-bit -> 7-bit switch can trap the active 7-bit window at zero
            // until a retrigger reloads the LFSR.
            self.lfsr_state = (self.lfsr_state & !(1 << 6)) | (feedback_bit << 6);
        }
    }

    fn clock_length(&mut self) {
        if !self.length_enabled || self.length_counter == 0 {
            return;
        }

        self.length_counter -= 1;
        if self.length_counter == 0 {
            self.runtime.active = false;
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_pace == 0 || !self.runtime.active {
            return;
        }

        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }

        if self.envelope_timer != 0 {
            return;
        }

        self.envelope_timer = envelope_timer_reload(self.envelope_pace);
        if self.envelope_increase {
            if self.current_volume < MAX_ENVELOPE_VOLUME {
                self.current_volume += 1;
            }
        } else if self.current_volume > 0 {
            self.current_volume -= 1;
        }
    }

    fn current_digital_output(&self) -> u8 {
        if !self.runtime.active {
            return 0;
        }

        if self.lfsr_state & 0x01 != 0 {
            self.current_volume
        } else {
            0
        }
    }
}

impl Apu {
    pub fn new(console_model: ConsoleModel) -> Self {
        let wave_ram_startup_policy = WaveRamStartupPolicy::DeterministicZeroed;

        Self {
            console_model,
            status: ApuStatus::Ready,
            master: MasterControlState::default(),
            frame_sequencer: FrameSequencerState::default(),
            skip_next_div_apu_edge: false,
            output_path: OutputPathState::default(),
            channel_1: Channel1State::default(),
            channel_2: Channel2State::default(),
            channel_3: Channel3State::default(),
            channel_4: Channel4State::default(),
            wave_ram_startup_policy,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> ApuStatus {
        self.status
    }

    pub(crate) fn tick_t_cycle(&mut self, context: &CycleContext) {
        self.channel_3.begin_t_cycle();

        if self.master.powered {
            self.channel_1.tick_fast_timer();
            self.channel_2.tick_fast_timer();
            self.channel_3.tick_fast_timer();
            self.channel_4.tick_fast_timer();
        }

        for edge in context.derived_edges() {
            if matches!(edge, DerivedEdge::ApuFrameSequencerEdge) {
                self.advance_frame_sequencer();
            }
        }

        self.tick_output_path();
    }

    pub(crate) fn on_div_apu_edge(&mut self) {
        self.advance_frame_sequencer();
    }

    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            0xFF10 => self.channel_1.read_nr10(),
            0xFF11 => self.channel_1.read_nr11(),
            0xFF12 => self.channel_1.nr12,
            0xFF13 => NR13_WRITE_ONLY_READ_VALUE,
            0xFF14 => self.channel_1.read_nr14(),
            0xFF15 => 0xFF,
            0xFF16 => self.channel_2.read_nr21(),
            0xFF17 => self.channel_2.nr22,
            0xFF18 => NR13_WRITE_ONLY_READ_VALUE,
            0xFF19 => self.channel_2.read_nr24(),
            0xFF1A => self.channel_3.read_nr30(),
            0xFF1B => NR31_WRITE_ONLY_READ_VALUE,
            0xFF1C => self.channel_3.read_nr32(),
            0xFF1D => NR33_WRITE_ONLY_READ_VALUE,
            0xFF1E => self.channel_3.read_nr34(),
            0xFF1F => 0xFF,
            0xFF20 => NR41_WRITE_ONLY_READ_VALUE,
            0xFF21 => self.channel_4.nr42,
            0xFF22 => self.channel_4.nr43,
            0xFF23 => self.channel_4.read_nr44(),
            0xFF24 => self.master.nr50,
            0xFF25 => self.master.nr51,
            0xFF26 => self.read_nr52(),
            0xFF27..=0xFF2F => 0xFF,
            0xFF30..=0xFF3F => self
                .channel_3
                .read_wave_ram(self.console_model, (address - 0xFF30) as usize),
            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        self.write_register_with_div_apu_source(address, value, false);
    }

    pub(crate) fn write_register_with_div_apu_source(
        &mut self,
        address: u16,
        value: u8,
        div_apu_source_high: bool,
    ) {
        if let Some(index) = self.wave_ram_index(address) {
            self.channel_3
                .write_wave_ram(self.console_model, index, value);
            self.preview_output_path();
            return;
        }

        if address == 0xFF26 {
            self.write_nr52(value, div_apu_source_high);
            return;
        }

        if !self.master.powered {
            if self.console_model.is_dmg_family() {
                match address {
                    0xFF11 => self.channel_1.write_length_while_powered_off(value),
                    0xFF16 => self.channel_2.write_length_while_powered_off(value),
                    0xFF1B => self.channel_3.write_length_while_powered_off(value),
                    0xFF20 => self.channel_4.write_length_while_powered_off(value),
                    _ => {}
                }
            }
            self.preview_output_path();
            return;
        }

        match address {
            0xFF10 => self.channel_1.write_nr10(value),
            0xFF11 => self.channel_1.write_nr11(value),
            0xFF12 => self.channel_1.write_nr12(value),
            0xFF13 => self.channel_1.write_nr13(value),
            0xFF14 => self.channel_1.write_nr14(value, self.frame_sequencer.step),
            0xFF15 => {}
            0xFF16 => self.channel_2.write_nr21(value),
            0xFF17 => self.channel_2.write_nr22(value),
            0xFF18 => self.channel_2.write_nr23(value),
            0xFF19 => self.channel_2.write_nr24(value, self.frame_sequencer.step),
            0xFF1A => self.channel_3.write_nr30(value),
            0xFF1B => self.channel_3.write_nr31(value),
            0xFF1C => self.channel_3.write_nr32(value),
            0xFF1D => self.channel_3.write_nr33(value),
            0xFF1E => {
                self.channel_3
                    .write_nr34(value, self.console_model, self.frame_sequencer.step)
            }
            0xFF1F => {}
            0xFF20 => self.channel_4.write_nr41(value),
            0xFF21 => self.channel_4.write_nr42(value),
            0xFF22 => self.channel_4.write_nr43(value),
            0xFF23 => self.channel_4.write_nr44(value, self.frame_sequencer.step),
            0xFF24 => self.master.nr50 = value,
            0xFF25 => self.master.nr51 = value,
            0xFF27..=0xFF3F => {}
            _ => {}
        }

        self.preview_output_path();
    }

    pub fn apply_startup_state(&mut self, startup_state: ApuStartupState) {
        self.wave_ram_startup_policy = startup_state.wave_ram_startup_policy;
        self.channel_3
            .initialize_wave_ram(startup_state.wave_ram_startup_policy.initial_bytes());
        self.skip_next_div_apu_edge = false;
        self.output_path = OutputPathState::default();

        if startup_state.powered {
            self.master.powered = true;
            self.master.nr50 = startup_state.nr50;
            self.master.nr51 = startup_state.nr51;
            self.channel_1.apply_powered_startup(
                startup_state.nr10,
                startup_state.nr11,
                startup_state.nr12,
                startup_state.nr13,
                startup_state.nr14,
                startup_state.channel_active_mask & CHANNEL_ACTIVE_CH1 != 0,
            );
            self.channel_2.apply_powered_startup(
                startup_state.nr21,
                startup_state.nr22,
                startup_state.nr23,
                startup_state.nr24,
                startup_state.channel_active_mask & CHANNEL_ACTIVE_CH2 != 0,
            );
            self.channel_3.apply_powered_startup(
                startup_state.nr30,
                startup_state.nr31,
                startup_state.nr32,
                startup_state.nr33,
                startup_state.nr34,
                startup_state.channel_active_mask & CHANNEL_ACTIVE_CH3 != 0,
            );
            self.channel_4.apply_powered_startup(
                startup_state.nr41,
                startup_state.nr42,
                startup_state.nr43,
                startup_state.nr44,
                startup_state.channel_active_mask & CHANNEL_ACTIVE_CH4 != 0,
            );
        } else {
            self.power_off();
        }

        self.frame_sequencer
            .apply_startup_phase(startup_state.div_apu);
        self.preview_output_path();
    }

    pub fn snapshot(&self) -> ApuSnapshot {
        ApuSnapshot {
            console_model: self.console_model,
            status: self.status,
            powered: self.master.powered,
            nr50: self.master.nr50,
            nr51: self.master.nr51,
            channel_active_mask: self.channel_active_mask(),
            channel_dac_mask: self.channel_dac_mask(),
            div_apu: self.frame_sequencer.step,
            wave_ram: self.channel_3.wave_ram,
            wave_ram_startup_policy: self.wave_ram_startup_policy,
            output: self.output_snapshot(),
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
        )
    }

    fn read_nr52(&self) -> u8 {
        NR52_FORCED_HIGH_MASK
            | if self.master.powered {
                NR52_MASTER_POWER_BIT
            } else {
                0
            }
            | self.channel_active_mask()
    }

    fn write_nr52(&mut self, value: u8, div_apu_source_high: bool) {
        let next_powered = value & NR52_MASTER_POWER_BIT != 0;

        match (self.master.powered, next_powered) {
            (true, false) => self.power_off(),
            (false, true) => {
                self.master.powered = true;
                self.frame_sequencer.apply_startup_phase(0);
                self.skip_next_div_apu_edge = div_apu_source_high;
                self.channel_1.pulse.mark_powered_on();
                self.channel_2.pulse.mark_powered_on();
            }
            _ => {}
        }

        self.preview_output_path();
    }

    fn power_off(&mut self) {
        self.master.powered = false;
        self.skip_next_div_apu_edge = false;
        self.master.nr50 = 0;
        self.master.nr51 = 0;
        self.channel_1.power_off_registers(self.console_model);
        self.channel_2.power_off_registers(self.console_model);
        self.channel_3.power_off_registers(self.console_model);
        self.channel_4.power_off_registers(self.console_model);
    }

    fn channel_active_mask(&self) -> u8 {
        self.channel_mask_for_runtime(|runtime| runtime.active)
    }

    fn channel_dac_mask(&self) -> u8 {
        self.channel_mask_for_runtime(|runtime| runtime.dac_enabled)
    }

    fn channel_mask_for_runtime(&self, select: impl Fn(ChannelRuntimeState) -> bool) -> u8 {
        let mut mask = 0;

        if select(self.channel_1.pulse.runtime) {
            mask |= CHANNEL_ACTIVE_CH1;
        }
        if select(self.channel_2.pulse.runtime) {
            mask |= CHANNEL_ACTIVE_CH2;
        }
        if select(self.channel_3.runtime) {
            mask |= CHANNEL_ACTIVE_CH3;
        }
        if select(self.channel_4.runtime) {
            mask |= CHANNEL_ACTIVE_CH4;
        }

        mask & CHANNEL_ACTIVE_MASK
    }

    fn channel_digital_outputs(&self) -> [u8; 4] {
        [
            self.channel_1.pulse.current_digital_output(),
            self.channel_2.pulse.current_digital_output(),
            self.channel_3.current_digital_output(),
            self.channel_4.current_digital_output(),
        ]
    }

    fn channel_dac_outputs(&self, channel_digital_outputs: [u8; 4]) -> [i32; 4] {
        let channel_dac_mask = self.channel_dac_mask();

        [
            if channel_dac_mask & CHANNEL_ACTIVE_CH1 != 0 {
                dac_analog_output(channel_digital_outputs[0])
            } else {
                0
            },
            if channel_dac_mask & CHANNEL_ACTIVE_CH2 != 0 {
                dac_analog_output(channel_digital_outputs[1])
            } else {
                0
            },
            if channel_dac_mask & CHANNEL_ACTIVE_CH3 != 0 {
                dac_analog_output(channel_digital_outputs[2])
            } else {
                0
            },
            if channel_dac_mask & CHANNEL_ACTIVE_CH4 != 0 {
                dac_analog_output(channel_digital_outputs[3])
            } else {
                0
            },
        ]
    }

    fn mixer_output(&self, channel_dac_outputs: [i32; 4]) -> ApuStereoOutputSnapshot {
        let mut left = 0;
        let mut right = 0;

        if self.master.nr51 & 0x10 != 0 {
            left += channel_dac_outputs[0];
        }
        if self.master.nr51 & 0x20 != 0 {
            left += channel_dac_outputs[1];
        }
        if self.master.nr51 & 0x40 != 0 {
            left += channel_dac_outputs[2];
        }
        if self.master.nr51 & 0x80 != 0 {
            left += channel_dac_outputs[3];
        }

        if self.master.nr51 & 0x01 != 0 {
            right += channel_dac_outputs[0];
        }
        if self.master.nr51 & 0x02 != 0 {
            right += channel_dac_outputs[1];
        }
        if self.master.nr51 & 0x04 != 0 {
            right += channel_dac_outputs[2];
        }
        if self.master.nr51 & 0x08 != 0 {
            right += channel_dac_outputs[3];
        }

        ApuStereoOutputSnapshot::new(left, right)
    }

    fn master_output(&self, mixer_output: ApuStereoOutputSnapshot) -> ApuStereoOutputSnapshot {
        ApuStereoOutputSnapshot::new(
            mixer_output.left * nr50_left_volume_factor(self.master.nr50),
            mixer_output.right * nr50_right_volume_factor(self.master.nr50),
        )
    }

    fn output_snapshot(&self) -> ApuOutputSnapshot {
        let channel_digital_outputs = self.channel_digital_outputs();
        let channel_dac_outputs = self.channel_dac_outputs(channel_digital_outputs);
        let mixer_output = self.mixer_output(channel_dac_outputs);
        let master_output = self.master_output(mixer_output);

        ApuOutputSnapshot {
            channel_digital_outputs,
            channel_dac_outputs,
            mixer_output,
            master_output,
            hpf_output: self.output_path.current_output,
            hpf_capacitor: self.output_path.hpf_capacitor,
        }
    }

    fn preview_output_path(&mut self) {
        let master_output = self.output_snapshot().master_output;
        self.output_path
            .preview(master_output, self.channel_dac_mask() != 0);
    }

    fn tick_output_path(&mut self) {
        let master_output = self.output_snapshot().master_output;
        self.output_path
            .tick(master_output, self.channel_dac_mask() != 0);
    }

    fn wave_ram_index(&self, address: u16) -> Option<usize> {
        match address {
            0xFF30..=0xFF3F => Some((address - 0xFF30) as usize),
            _ => None,
        }
    }

    fn advance_frame_sequencer(&mut self) {
        if self.skip_next_div_apu_edge {
            self.skip_next_div_apu_edge = false;
            return;
        }

        let clocks = self.frame_sequencer.advance();
        if !self.master.powered {
            self.preview_output_path();
            return;
        }

        if clocks.length {
            self.channel_1.clock_length();
            self.channel_2.clock_length();
            self.channel_3.clock_length();
            self.channel_4.clock_length();
        }
        if clocks.sweep {
            self.channel_1.clock_sweep();
        }
        if clocks.envelope {
            self.channel_1.clock_envelope();
            self.channel_2.clock_envelope();
            self.channel_4.clock_envelope();
        }

        self.preview_output_path();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::TCycle;

    fn tick_apu_with_edges(apu: &mut Apu, t_cycle: u64, edges: &[DerivedEdge]) {
        let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
        for &edge in edges {
            context.push_derived_edge(edge);
        }
        apu.tick_t_cycle(&context);
    }

    const fn pulse_length_load(counter: u8) -> u8 {
        0xC0 | ((PULSE_LENGTH_COUNTER_RELOAD - counter) & PULSE_LENGTH_LOAD_MASK)
    }

    fn prime_channel_2_trigger_test(apu: &mut Apu, length_counter: u8) {
        apu.write_register(0xFF17, 0x08);
        apu.write_register(0xFF16, pulse_length_load(PULSE_LENGTH_COUNTER_RELOAD));
        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);
        apu.write_register(0xFF16, pulse_length_load(length_counter));
    }

    fn prime_channel_1_trigger_test(apu: &mut Apu, length_counter: u8) {
        apu.write_register(0xFF12, 0x08);
        apu.write_register(0xFF11, pulse_length_load(PULSE_LENGTH_COUNTER_RELOAD));
        apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT);
        apu.write_register(0xFF11, pulse_length_load(length_counter));
    }

    #[test]
    fn nr52_tracks_channel_active_state_separately_from_dac_state() {
        let mut apu = Apu::new(ConsoleModel::Dmg);

        apu.write_register(0xFF26, 0x80);

        assert_eq!(apu.read_register(0xFF26), 0xF0);
        assert_eq!(apu.snapshot().channel_dac_mask, 0x00);

        apu.write_register(0xFF12, 0xF3);
        assert_eq!(apu.read_register(0xFF26), 0xF0);
        assert_eq!(apu.snapshot().channel_dac_mask, CHANNEL_ACTIVE_CH1);
        assert_eq!(apu.snapshot().channel_active_mask, 0x00);

        apu.write_register(0xFF14, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xF1);
        assert_eq!(apu.snapshot().channel_dac_mask, CHANNEL_ACTIVE_CH1);
        assert_eq!(apu.snapshot().channel_active_mask, CHANNEL_ACTIVE_CH1);

        apu.write_register(0xFF12, 0x00);
        assert_eq!(apu.read_register(0xFF26), 0xF0);
        assert_eq!(apu.snapshot().channel_dac_mask, 0x00);
        assert_eq!(apu.snapshot().channel_active_mask, 0x00);
    }

    #[test]
    fn enabled_dac_output_remains_distinct_from_dac_off_even_when_the_channel_is_inactive() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);

        apu.write_register(0xFF12, 0x08);
        let enabled_snapshot = apu.snapshot();

        assert_eq!(enabled_snapshot.output.channel_digital_outputs[0], 0);
        assert_eq!(enabled_snapshot.output.channel_dac_outputs[0], ANALOG_ONE);
        assert_eq!(enabled_snapshot.channel_dac_mask, CHANNEL_ACTIVE_CH1);
        assert_eq!(enabled_snapshot.channel_active_mask, 0x00);

        apu.write_register(0xFF12, 0x00);
        let disabled_snapshot = apu.snapshot();

        assert_eq!(disabled_snapshot.output.channel_digital_outputs[0], 0);
        assert_eq!(disabled_snapshot.output.channel_dac_outputs[0], 0);
        assert_eq!(disabled_snapshot.channel_dac_mask, 0x00);
        assert_eq!(disabled_snapshot.channel_active_mask, 0x00);
    }

    #[test]
    fn nr51_routes_channel_dac_outputs_independently_to_left_and_right_buses() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF12, 0x08);
        apu.write_register(0xFF17, 0x08);
        apu.write_register(0xFF25, 0x12);

        let snapshot = apu.snapshot();

        assert_eq!(
            snapshot.output.channel_dac_outputs,
            [ANALOG_ONE, ANALOG_ONE, 0, 0]
        );
        assert_eq!(snapshot.output.mixer_output.left, ANALOG_ONE);
        assert_eq!(snapshot.output.mixer_output.right, ANALOG_ONE);
        assert_eq!(snapshot.output.master_output.left, ANALOG_ONE);
        assert_eq!(snapshot.output.master_output.right, ANALOG_ONE);
    }

    #[test]
    fn nr50_volume_zero_still_scales_by_one_and_seven_scales_by_eight() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF12, 0x08);
        apu.write_register(0xFF25, 0x11);

        apu.write_register(0xFF24, 0x00);
        let quiet_snapshot = apu.snapshot();
        assert_eq!(quiet_snapshot.output.master_output.left, ANALOG_ONE);
        assert_eq!(quiet_snapshot.output.master_output.right, ANALOG_ONE);

        apu.write_register(0xFF24, 0x77);
        let loud_snapshot = apu.snapshot();
        assert_eq!(loud_snapshot.output.master_output.left, ANALOG_ONE * 8);
        assert_eq!(loud_snapshot.output.master_output.right, ANALOG_ONE * 8);
    }

    #[test]
    fn hpf_state_persists_across_t_cycles_and_pulls_the_output_towards_zero() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF12, 0x08);
        apu.write_register(0xFF24, 0x00);
        apu.write_register(0xFF25, 0x11);

        let before = apu.snapshot().output;
        assert_eq!(before.hpf_output.left, ANALOG_ONE);
        assert_eq!(before.hpf_output.right, ANALOG_ONE);
        assert_eq!(before.hpf_capacitor.left, 0);
        assert_eq!(before.hpf_capacitor.right, 0);

        tick_apu_with_edges(&mut apu, 0, &[]);
        let after_first_tick = apu.snapshot().output;
        assert_eq!(after_first_tick.hpf_output.left, ANALOG_ONE);
        assert_eq!(after_first_tick.hpf_output.right, ANALOG_ONE);
        assert!(after_first_tick.hpf_capacitor.left > 0);
        assert!(after_first_tick.hpf_capacitor.right > 0);

        tick_apu_with_edges(&mut apu, 1, &[]);
        let after_second_tick = apu.snapshot().output;
        assert!(after_second_tick.hpf_output.left < after_first_tick.hpf_output.left);
        assert!(after_second_tick.hpf_output.right < after_first_tick.hpf_output.right);
        assert!(after_second_tick.hpf_capacitor.left > after_first_tick.hpf_capacitor.left);
        assert!(after_second_tick.hpf_capacitor.right > after_first_tick.hpf_capacitor.right);
    }

    #[test]
    fn mixer_and_hpf_output_change_immediately_when_routing_changes() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF12, 0x08);
        apu.write_register(0xFF24, 0x00);
        apu.write_register(0xFF25, 0x01);

        let right_only = apu.snapshot().output;
        assert_eq!(right_only.master_output.left, 0);
        assert_eq!(right_only.master_output.right, ANALOG_ONE);
        assert_eq!(right_only.hpf_output.left, 0);
        assert_eq!(right_only.hpf_output.right, ANALOG_ONE);

        apu.write_register(0xFF25, 0x10);
        let left_only = apu.snapshot().output;
        assert_eq!(left_only.master_output.left, ANALOG_ONE);
        assert_eq!(left_only.master_output.right, 0);
        assert_eq!(left_only.hpf_output.left, ANALOG_ONE);
        assert_eq!(left_only.hpf_output.right, 0);
    }

    #[test]
    fn audio_register_readback_keeps_write_only_and_mixed_fields_explicit() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);

        apu.write_register(0xFF10, 0x00);
        apu.write_register(0xFF11, 0x00);
        apu.write_register(0xFF13, 0x12);
        apu.write_register(0xFF14, 0x40);
        apu.write_register(0xFF1C, 0x00);
        apu.write_register(0xFF20, 0x34);
        apu.write_register(0xFF23, 0x00);

        assert_eq!(apu.read_register(0xFF10), 0x80);
        assert_eq!(apu.read_register(0xFF11), 0x3F);
        assert_eq!(apu.read_register(0xFF13), 0xFF);
        assert_eq!(apu.read_register(0xFF14), 0xFF);
        assert_eq!(apu.read_register(0xFF1C), 0x9F);
        assert_eq!(apu.read_register(0xFF20), 0xFF);
        assert_eq!(apu.read_register(0xFF23), 0xBF);
        assert_eq!(apu.read_register(0xFF15), 0xFF);
    }

    #[test]
    fn nr52_power_off_clears_audio_registers_but_preserves_wave_ram() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF12, 0xF3);
        apu.write_register(0xFF14, 0x80);
        apu.write_register(0xFF24, 0x77);
        apu.write_register(0xFF25, 0xF3);
        apu.write_register(0xFF30, 0x12);
        apu.write_register(0xFF31, 0x34);

        apu.write_register(0xFF26, 0x00);

        assert_eq!(apu.read_register(0xFF26), 0x70);
        assert_eq!(apu.read_register(0xFF12), 0x00);
        assert_eq!(apu.read_register(0xFF14), 0xBF);
        assert_eq!(apu.read_register(0xFF24), 0x00);
        assert_eq!(apu.read_register(0xFF25), 0x00);
        assert_eq!(apu.read_register(0xFF30), 0x12);
        assert_eq!(apu.read_register(0xFF31), 0x34);

        apu.write_register(0xFF12, 0xF3);
        apu.write_register(0xFF24, 0x77);
        apu.write_register(0xFF25, 0xF3);
        assert_eq!(apu.read_register(0xFF12), 0x00);
        assert_eq!(apu.read_register(0xFF24), 0x00);
        assert_eq!(apu.read_register(0xFF25), 0x00);
    }

    #[test]
    fn frame_sequencer_advances_only_on_the_shared_div_apu_edge() {
        let mut apu = Apu::new(ConsoleModel::Dmg);

        tick_apu_with_edges(&mut apu, 0, &[]);
        assert_eq!(apu.snapshot().div_apu, 0x00);

        tick_apu_with_edges(&mut apu, 1, &[DerivedEdge::DividerTick]);
        assert_eq!(apu.snapshot().div_apu, 0x00);

        tick_apu_with_edges(&mut apu, 2, &[DerivedEdge::ApuFrameSequencerEdge]);
        assert_eq!(apu.snapshot().div_apu, 0x01);
        assert_eq!(apu.frame_sequencer.length_clock_count, 1);
        assert_eq!(apu.frame_sequencer.sweep_clock_count, 0);
        assert_eq!(apu.frame_sequencer.envelope_clock_count, 0);
    }

    #[test]
    fn powering_on_with_the_div_apu_source_high_skips_the_next_frame_sequencer_edge() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.apply_startup_state(ApuStartupState {
            powered: false,
            nr10: 0x00,
            nr11: 0x00,
            nr12: 0x00,
            nr13: 0x00,
            nr14: 0x00,
            nr21: 0x00,
            nr22: 0x00,
            nr23: 0x00,
            nr24: 0x00,
            nr30: 0x00,
            nr31: 0x00,
            nr32: 0x00,
            nr33: 0x00,
            nr34: 0x00,
            nr41: 0x00,
            nr42: 0x00,
            nr43: 0x00,
            nr44: 0x00,
            nr50: 0x00,
            nr51: 0x00,
            channel_active_mask: 0x00,
            div_apu: 0x05,
            wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
        });

        apu.write_register_with_div_apu_source(0xFF26, 0x80, true);
        assert!(apu.snapshot().powered);
        assert_eq!(apu.snapshot().div_apu, 0x00);

        tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
        assert_eq!(apu.snapshot().div_apu, 0x00);

        tick_apu_with_edges(&mut apu, 1, &[DerivedEdge::ApuFrameSequencerEdge]);
        assert_eq!(apu.snapshot().div_apu, 0x01);
    }

    #[test]
    fn frame_sequencer_emits_length_sweep_and_envelope_clocks_on_the_documented_steps() {
        let mut apu = Apu::new(ConsoleModel::Dmg);

        for t_cycle in 0..8 {
            tick_apu_with_edges(&mut apu, t_cycle, &[DerivedEdge::ApuFrameSequencerEdge]);
        }

        assert_eq!(apu.snapshot().div_apu, 0x00);
        assert_eq!(apu.frame_sequencer.length_clock_count, 4);
        assert_eq!(apu.frame_sequencer.sweep_clock_count, 2);
        assert_eq!(apu.frame_sequencer.envelope_clock_count, 1);
    }

    #[test]
    fn channel_1_trigger_reloads_period_envelope_and_sweep_without_resetting_duty_step() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF10, 0x11);
        apu.write_register(0xFF11, 0xBF);
        apu.write_register(0xFF12, 0xA2);
        apu.write_register(0xFF13, 0xAB);
        apu.channel_1.pulse.duty_step = 5;

        apu.write_register(0xFF14, 0xC4);

        assert!(apu.channel_1.pulse.runtime.active);
        assert_eq!(apu.channel_1.pulse.duty_step, 5);
        assert_eq!(apu.channel_1.pulse.length_counter, 1);
        assert_eq!(apu.channel_1.pulse.current_volume, 0x0A);
        assert_eq!(apu.channel_1.pulse.envelope_timer, 0x02);
        assert_eq!(apu.channel_1.pulse.period_timer, pulse_timer_reload(0x04AB));
        assert_eq!(apu.channel_1.sweep.shadow_period, 0x04AB);
        assert_eq!(apu.channel_1.sweep.timer, 0x01);
        assert!(apu.channel_1.sweep.enabled);
    }

    #[test]
    fn channel_1_first_trigger_after_power_on_suppresses_the_initial_high_duty_output() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF11, 0x40);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF13, 0xFF);

        apu.write_register(0xFF14, 0x87);

        assert!(apu.channel_1.pulse.runtime.active);
        assert_eq!(apu.channel_1.pulse.duty_step, 0);
        assert!(pulse_waveform_high(
            apu.channel_1.pulse.duty,
            apu.channel_1.pulse.duty_step,
        ));
        assert!(apu.channel_1.pulse.suppress_initial_trigger_output);
        assert_eq!(apu.channel_1.pulse.current_digital_output(), 0);

        for _ in 0..4 {
            apu.channel_1.tick_fast_timer();
        }

        assert_eq!(apu.channel_1.pulse.duty_step, 1);
        assert!(!apu.channel_1.pulse.suppress_initial_trigger_output);
    }

    #[test]
    fn channel_2_retrigger_after_the_first_post_power_on_trigger_does_not_resuppress_output() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF16, 0x40);
        apu.write_register(0xFF17, 0xF0);
        apu.write_register(0xFF18, 0xFF);

        apu.write_register(0xFF19, 0x87);
        assert!(apu.channel_2.pulse.suppress_initial_trigger_output);

        for _ in 0..4 {
            apu.channel_2.tick_fast_timer();
        }

        assert!(!apu.channel_2.pulse.suppress_initial_trigger_output);

        apu.channel_2.pulse.duty_step = 0;
        apu.write_register(0xFF19, 0x87);

        assert!(!apu.channel_2.pulse.suppress_initial_trigger_output);
        assert_eq!(apu.channel_2.pulse.current_digital_output(), 0x0F);
    }

    #[test]
    fn nr52_power_cycle_rearms_the_first_trigger_after_power_on_pulse_suppression() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF11, 0x40);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF13, 0xFF);
        apu.write_register(0xFF14, 0x87);

        assert!(apu.channel_1.pulse.suppress_initial_trigger_output);

        for _ in 0..4 {
            apu.channel_1.tick_fast_timer();
        }

        assert!(!apu.channel_1.pulse.suppress_initial_trigger_output);

        apu.write_register(0xFF26, 0x00);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF11, 0x40);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF13, 0xFF);
        apu.write_register(0xFF14, 0x87);

        assert!(apu.channel_1.pulse.suppress_initial_trigger_output);
        assert_eq!(apu.channel_1.pulse.current_digital_output(), 0);
    }

    #[test]
    fn triggering_a_pulse_channel_preserves_the_low_two_bits_of_the_frequency_timer() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF16, 0x80);
        apu.write_register(0xFF17, 0xF0);
        apu.write_register(0xFF18, 0xFF);
        apu.channel_2.pulse.period_timer = 0x0003;

        apu.write_register(0xFF19, 0x87);

        assert_eq!(
            apu.channel_2.pulse.period_timer,
            pulse_timer_reload(0x07FF) | 0x0003
        );
    }

    #[test]
    fn triggering_a_pulse_channel_just_before_an_envelope_step_reloads_the_timer_with_plus_one() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        apu.write_register(0xFF16, 0x80);
        apu.write_register(0xFF17, 0xF2);

        apu.write_register(0xFF19, 0x80);

        assert_eq!(
            apu.channel_2.pulse.envelope_timer,
            envelope_timer_reload(0x02) + 1
        );
    }

    #[test]
    fn enabling_pulse_length_on_a_non_length_step_clocks_it_immediately() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(1);
        apu.write_register(0xFF11, 0xBF);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF14, 0x80);

        assert!(apu.channel_1.pulse.runtime.active);
        assert!(!apu.channel_1.pulse.length_enabled);
        assert_eq!(apu.channel_1.pulse.length_counter, 1);

        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);

        assert!(apu.channel_1.pulse.length_enabled);
        assert_eq!(apu.channel_1.pulse.length_counter, 0);
        assert!(!apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn enabling_pulse_length_on_a_length_step_does_not_clock_it() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(0);
        apu.write_register(0xFF11, 0xBF);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF14, 0x80);

        assert!(apu.channel_1.pulse.runtime.active);
        assert!(!apu.channel_1.pulse.length_enabled);
        assert_eq!(apu.channel_1.pulse.length_counter, 1);

        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);

        assert!(apu.channel_1.pulse.length_enabled);
        assert_eq!(apu.channel_1.pulse.length_counter, 1);
        assert!(apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn pulse_trigger_rom_second_half_enable_keeps_length_unchanged_before_retrigger() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(6);
        prime_channel_2_trigger_test(&mut apu, 2);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 2);
        assert!(apu.channel_2.pulse.runtime.active);

        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_2.pulse.length_counter, 2);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn pulse_trigger_rom_first_half_enable_clocks_once_and_survives_the_intervening_non_length_edge()
     {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_2_trigger_test(&mut apu, 2);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 1);
        assert!(apu.channel_2.pulse.runtime.active);

        tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
        assert_eq!(apu.snapshot().div_apu, 0x00);
        assert_eq!(apu.channel_2.pulse.length_counter, 1);
        assert!(apu.channel_2.pulse.runtime.active);

        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_2.pulse.length_counter, 1);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn triggering_a_zero_length_pulse_with_length_enabled_reloads_and_clocks_it() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_2_trigger_test(&mut apu, 1);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 0);
        assert!(!apu.channel_2.pulse.runtime.active);

        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_2.pulse.length_counter, 63);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn triggering_a_length_one_pulse_with_enable_on_the_same_first_half_write_matches_the_unfrozen_case()
     {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_2_trigger_test(&mut apu, 1);

        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_2.pulse.length_counter, 63);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn triggering_a_nonzero_length_pulse_does_not_change_its_length_counter() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(6);
        prime_channel_2_trigger_test(&mut apu, 2);
        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);

        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_2.pulse.length_counter, 2);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn writes_other_than_disabling_to_enabled_do_not_extra_clock_pulse_length() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_2_trigger_test(&mut apu, 2);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 1);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 1);

        apu.write_register(0xFF19, 0x00);
        assert_eq!(apu.channel_2.pulse.length_counter, 1);

        apu.write_register(0xFF19, 0x00);
        assert_eq!(apu.channel_2.pulse.length_counter, 1);
    }

    #[test]
    fn writing_length_after_enabling_it_matches_the_trigger_rom_sequence() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        apu.write_register(0xFF17, 0x08);
        apu.write_register(0xFF16, pulse_length_load(PULSE_LENGTH_COUNTER_RELOAD));
        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        apu.write_register(0xFF16, pulse_length_load(2));
        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        apu.write_register(0xFF19, 0x00);
        apu.write_register(0xFF19, 0x00);

        assert_eq!(apu.channel_2.pulse.length_counter, 2);
        assert!(!apu.channel_2.pulse.length_enabled);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn extra_length_clocking_to_zero_disables_the_pulse_channel() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_2_trigger_test(&mut apu, 1);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_2.pulse.length_counter, 0);
        assert!(!apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn enabling_length_again_after_it_reached_zero_does_not_clock_or_unfreeze_it() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_2_trigger_test(&mut apu, 1);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 0);

        apu.write_register(0xFF19, 0x00);
        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 0);

        apu.write_register(0xFF19, 0x00);
        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 0);
    }

    #[test]
    fn triggering_a_zero_length_pulse_with_length_disabled_unfreezes_it_to_the_full_reload() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_2_trigger_test(&mut apu, 1);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        apu.write_register(0xFF19, 0x00);
        assert_eq!(apu.channel_2.pulse.length_counter, 0);
        assert!(!apu.channel_2.pulse.length_enabled);

        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);

        assert_eq!(apu.channel_2.pulse.length_counter, 64);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn disabled_dac_still_allows_trigger_to_reload_and_clock_pulse_length() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_2_trigger_test(&mut apu, 1);

        apu.write_register(0xFF17, 0x00);
        assert!(!apu.channel_2.pulse.runtime.dac_enabled);
        assert!(!apu.channel_2.pulse.runtime.active);

        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_2.pulse.length_counter, 63);
        assert!(!apu.channel_2.pulse.runtime.active);

        apu.write_register(0xFF17, 0x08);
        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);

        assert_eq!(apu.channel_2.pulse.length_counter, 63);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn channel_1_first_half_enable_clocks_length_once_before_retrigger() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_1_trigger_test(&mut apu, 2);

        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 1);
        assert!(apu.channel_1.pulse.runtime.active);

        tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
        apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_1.pulse.length_counter, 1);
        assert!(apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn channel_1_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_1_trigger_test(&mut apu, 1);

        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 0);
        assert!(!apu.channel_1.pulse.runtime.active);

        apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_1.pulse.length_counter, 63);
        assert!(apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn channel_1_trigger_unfreezes_zero_length_and_clocks_it_after_disabling_length() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_1_trigger_test(&mut apu, 1);

        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 0);
        assert!(!apu.channel_1.pulse.runtime.active);

        apu.write_register(0xFF14, 0x00);
        assert_eq!(apu.channel_1.pulse.length_counter, 0);
        assert!(!apu.channel_1.pulse.length_enabled);

        apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_1.pulse.length_counter, 63);
        assert!(apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn channel_1_retrigger_after_unfreezing_zero_length_does_not_extra_clock_again() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_1_trigger_test(&mut apu, 1);

        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 0);
        assert!(!apu.channel_1.pulse.runtime.active);

        apu.write_register(0xFF14, 0x00);
        apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 63);
        assert!(apu.channel_1.pulse.runtime.active);

        apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_1.pulse.length_counter, 63);
        assert!(apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn trigger_unfreezes_zero_length_then_a_later_enable_allows_normal_length_clocks() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_2_trigger_test(&mut apu, 1);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 0);
        assert!(!apu.channel_2.pulse.runtime.active);

        apu.write_register(0xFF19, 0x00);
        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 64);
        assert!(apu.channel_2.pulse.runtime.active);

        tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
        assert_eq!(apu.snapshot().div_apu, 0x00);
        assert_eq!(apu.channel_2.pulse.length_counter, 64);

        apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_2.pulse.length_counter, 64);

        tick_apu_with_edges(&mut apu, 1, &[DerivedEdge::ApuFrameSequencerEdge]);
        tick_apu_with_edges(&mut apu, 2, &[DerivedEdge::ApuFrameSequencerEdge]);
        assert_eq!(apu.channel_2.pulse.length_counter, 63);
        assert!(apu.channel_2.pulse.runtime.active);

        tick_apu_with_edges(&mut apu, 3, &[DerivedEdge::ApuFrameSequencerEdge]);
        tick_apu_with_edges(&mut apu, 4, &[DerivedEdge::ApuFrameSequencerEdge]);
        assert_eq!(apu.channel_2.pulse.length_counter, 62);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn channel_1_retrigger_after_two_zero_length_freezes_only_extra_clocks_on_real_unfreeze_points()
    {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        prime_channel_1_trigger_test(&mut apu, 1);

        apu.write_register(0xFF14, 0x00);
        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 0);
        assert!(!apu.channel_1.pulse.runtime.active);

        apu.write_register(0xFF14, 0x00);
        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 0);
        assert!(!apu.channel_1.pulse.runtime.active);

        apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 64);
        assert!(apu.channel_1.pulse.runtime.active);
        assert!(!apu.channel_1.pulse.length_enabled);

        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 63);
        assert!(apu.channel_1.pulse.runtime.active);

        apu.write_register(0xFF14, 0x00);
        assert_eq!(apu.channel_1.pulse.length_counter, 63);
        assert!(apu.channel_1.pulse.runtime.active);
        assert!(!apu.channel_1.pulse.length_enabled);

        apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 62);
        assert!(apu.channel_1.pulse.runtime.active);

        apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_1.pulse.length_counter, 62);
        assert!(apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn triggering_a_zero_length_pulse_on_a_non_length_step_reloads_it_to_63() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(1);
        apu.write_register(0xFF16, 0x80);
        apu.write_register(0xFF17, 0xF0);
        apu.channel_2.pulse.length_counter = 0;

        apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert!(apu.channel_2.pulse.runtime.active);
        assert!(apu.channel_2.pulse.length_enabled);
        assert_eq!(apu.channel_2.pulse.length_counter, 63);
    }

    #[test]
    fn pulse_period_writes_take_effect_only_after_the_current_sample_finishes() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF16, 0x80);
        apu.write_register(0xFF17, 0xF0);
        apu.write_register(0xFF18, 0xFF);
        apu.write_register(0xFF19, 0x87);

        assert_eq!(apu.channel_2.pulse.period_timer, 4);

        apu.channel_2.tick_fast_timer();
        apu.channel_2.tick_fast_timer();
        assert_eq!(apu.channel_2.pulse.period_timer, 2);

        apu.write_register(0xFF18, 0xFE);
        apu.write_register(0xFF19, 0x07);
        assert_eq!(apu.channel_2.period_value(), 0x07FE);
        assert_eq!(apu.channel_2.pulse.period_timer, 2);

        apu.channel_2.tick_fast_timer();
        assert_eq!(apu.channel_2.pulse.period_timer, 1);
        apu.channel_2.tick_fast_timer();

        assert_eq!(apu.channel_2.pulse.period_timer, 8);
        assert_eq!(apu.channel_2.pulse.duty_step, 1);
        assert_eq!(apu.channel_2.pulse.current_digital_output(), 0);
    }

    #[test]
    fn frame_sequencer_length_and_envelope_clocks_drive_pulse_channel_state() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF11, 0xBF);
        apu.write_register(0xFF12, 0x11);
        apu.write_register(0xFF14, 0xC0);
        apu.write_register(0xFF16, 0x3F);
        apu.write_register(0xFF17, 0x21);
        apu.write_register(0xFF19, 0xC0);

        apu.frame_sequencer.apply_startup_phase(7);
        tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);

        assert_eq!(apu.channel_1.pulse.current_volume, 0);
        assert_eq!(apu.channel_2.pulse.current_volume, 1);
        assert!(apu.channel_1.pulse.runtime.active);
        assert!(apu.channel_2.pulse.runtime.active);

        apu.frame_sequencer.apply_startup_phase(0);
        tick_apu_with_edges(&mut apu, 1, &[DerivedEdge::ApuFrameSequencerEdge]);

        assert!(!apu.channel_1.pulse.runtime.active);
        assert!(!apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn channel_1_sweep_clock_writes_back_shadow_period_and_runs_the_second_overflow_check() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF10, 0x11);
        apu.write_register(0xFF11, 0x80);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF13, 0x00);
        apu.write_register(0xFF14, 0x85);

        assert_eq!(apu.channel_1.period_value(), 0x0500);
        assert!(apu.channel_1.pulse.runtime.active);

        apu.channel_1.clock_sweep();

        assert_eq!(apu.channel_1.period_value(), 0x0780);
        assert_eq!(apu.channel_1.sweep.shadow_period, 0x0780);
        assert!(!apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn channel_1_shift_zero_sweep_does_not_calculate_on_trigger_but_can_overflow_on_sweep_clock() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF10, 0x10);
        apu.write_register(0xFF11, 0x80);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF13, 0x00);
        apu.write_register(0xFF14, 0x86);

        assert_eq!(apu.channel_1.period_value(), 0x0600);
        assert!(apu.channel_1.pulse.runtime.active);

        apu.channel_1.clock_sweep();

        assert_eq!(apu.channel_1.period_value(), 0x0600);
        assert!(!apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn channel_1_zero_sweep_pace_reloads_to_eight_and_rearms_on_a_non_zero_write() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF10, 0x11);
        apu.write_register(0xFF11, 0x80);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF13, 0x00);
        apu.write_register(0xFF14, 0x82);

        assert_eq!(apu.channel_1.period_value(), 0x0200);
        apu.channel_1.clock_sweep();
        assert_eq!(apu.channel_1.period_value(), 0x0300);

        apu.write_register(0xFF10, 0x01);
        for _ in 0..8 {
            apu.channel_1.clock_sweep();
            assert_eq!(apu.channel_1.period_value(), 0x0300);
            assert!(apu.channel_1.pulse.runtime.active);
        }

        assert_eq!(apu.channel_1.sweep.shadow_period, 0x0300);
        assert_eq!(apu.channel_1.sweep.timer, 1);

        apu.write_register(0xFF10, 0x11);
        apu.channel_1.clock_sweep();

        assert_eq!(apu.channel_1.period_value(), 0x0480);
        assert_eq!(apu.channel_1.sweep.shadow_period, 0x0480);
        assert!(apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn clearing_negate_after_a_negate_calculation_disables_channel_1() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF10, 0x09);
        apu.write_register(0xFF11, 0x80);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF13, 0x00);
        apu.write_register(0xFF14, 0x84);

        assert!(apu.channel_1.pulse.runtime.active);
        assert!(apu.channel_1.sweep.negate_calculated_since_trigger);

        apu.write_register(0xFF10, 0x10);

        assert!(!apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn clearing_negate_without_a_negate_calculation_keeps_channel_1_active() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF10, 0x08);
        apu.write_register(0xFF11, 0x80);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF13, 0x00);
        apu.write_register(0xFF14, 0x84);

        assert!(apu.channel_1.pulse.runtime.active);
        assert!(!apu.channel_1.sweep.negate_calculated_since_trigger);

        apu.write_register(0xFF10, 0x10);

        assert!(apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn channel_1_negate_sweep_uses_eleven_bit_twos_complement_subtraction() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF10, 0x1C);
        apu.write_register(0xFF11, 0x80);
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF13, 0xB0);
        apu.write_register(0xFF14, 0x85);

        apu.channel_1.clock_sweep();

        assert_eq!(apu.channel_1.period_value(), 0x0555);
        assert_eq!(apu.channel_1.sweep.shadow_period, 0x0555);
        assert!(apu.channel_1.pulse.runtime.active);
    }

    #[test]
    fn envelope_reaching_zero_does_not_disable_the_pulse_channel() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF16, 0x80);
        apu.write_register(0xFF17, 0x11);
        apu.write_register(0xFF19, 0x80);

        apu.frame_sequencer.apply_startup_phase(7);
        tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);

        assert_eq!(apu.channel_2.pulse.current_volume, 0);
        assert!(apu.channel_2.pulse.runtime.active);
    }

    #[test]
    fn channel_3_trigger_preserves_the_buffered_sample_until_the_next_wave_fetch() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.channel_3.wave_ram[0] = 0x12;
        apu.channel_3.wave_ram[1] = 0x34;
        apu.channel_3.sample_buffer = 0x0E;

        apu.write_register(0xFF1A, 0x80);
        apu.write_register(0xFF1C, 0x20);
        apu.write_register(0xFF1D, 0xFF);
        apu.write_register(0xFF1E, 0x87);

        assert!(apu.channel_3.runtime.active);
        assert_eq!(apu.channel_3.sample_index, 0);
        assert_eq!(apu.channel_3.sample_buffer, 0x0E);
        assert_eq!(
            apu.channel_3.period_timer,
            2 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
        );

        for expected_timer in (1..=1 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES).rev() {
            apu.channel_3.tick_fast_timer();
            assert_eq!(apu.channel_3.sample_buffer, 0x0E);
            assert_eq!(apu.channel_3.period_timer, expected_timer);
        }

        apu.channel_3.tick_fast_timer();
        assert_eq!(apu.channel_3.sample_index, 1);
        assert_eq!(apu.channel_3.sample_buffer, 0x02);
        assert_eq!(apu.channel_3.period_timer, 2);
    }

    #[test]
    fn channel_3_period_writes_take_effect_only_after_the_next_wave_fetch_boundary() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.channel_3.wave_ram[0] = 0x12;

        apu.write_register(0xFF1A, 0x80);
        apu.write_register(0xFF1C, 0x20);
        apu.write_register(0xFF1D, 0xFF);
        apu.write_register(0xFF1E, 0x87);

        apu.channel_3.tick_fast_timer();
        assert_eq!(
            apu.channel_3.period_timer,
            1 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
        );

        apu.write_register(0xFF1D, 0xFE);
        apu.write_register(0xFF1E, 0x07);

        assert_eq!(apu.channel_3.period_value(), 0x07FE);
        assert_eq!(
            apu.channel_3.period_timer,
            1 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
        );
        assert_eq!(apu.channel_3.sample_buffer, 0);

        for expected_timer in (1..=WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES).rev() {
            apu.channel_3.tick_fast_timer();
            assert_eq!(apu.channel_3.sample_index, 0);
            assert_eq!(apu.channel_3.sample_buffer, 0);
            assert_eq!(apu.channel_3.period_timer, expected_timer);
        }

        apu.channel_3.tick_fast_timer();
        assert_eq!(apu.channel_3.sample_index, 1);
        assert_eq!(apu.channel_3.sample_buffer, 0x02);
        assert_eq!(apu.channel_3.period_timer, 4);
    }

    #[test]
    fn channel_3_output_level_applies_immediate_digital_attenuation_without_disabling_it() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF1A, 0x80);
        apu.channel_3.runtime.active = true;
        apu.channel_3.sample_buffer = 0x0C;

        apu.write_register(0xFF1C, 0x00);
        assert_eq!(apu.channel_3.current_digital_output(), 0);
        assert!(apu.channel_3.runtime.active);

        apu.write_register(0xFF1C, 0x20);
        assert_eq!(apu.channel_3.current_digital_output(), 0x0C);

        apu.write_register(0xFF1C, 0x40);
        assert_eq!(apu.channel_3.current_digital_output(), 0x06);

        apu.write_register(0xFF1C, 0x60);
        assert_eq!(apu.channel_3.current_digital_output(), 0x03);
        assert!(apu.channel_3.runtime.active);
    }

    #[test]
    fn active_channel_3_wave_ram_reads_return_ff_outside_the_dmg_fetch_window() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF1A, 0x80);
        apu.channel_3.runtime.active = true;
        apu.channel_3.wave_ram[0] = 0x12;
        apu.channel_3.wave_ram[1] = 0x34;

        assert_eq!(apu.read_register(0xFF30), WAVE_RAM_INACCESSIBLE_READ_VALUE);
        assert_eq!(apu.read_register(0xFF3F), WAVE_RAM_INACCESSIBLE_READ_VALUE);
    }

    #[test]
    fn active_channel_3_wave_ram_writes_are_ignored_outside_the_dmg_fetch_window() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF1A, 0x80);
        apu.channel_3.runtime.active = true;
        apu.channel_3.wave_ram[0] = 0x12;
        apu.channel_3.wave_ram[1] = 0x34;

        apu.write_register(0xFF30, 0xAB);

        assert_eq!(apu.channel_3.wave_ram[0], 0x12);
        assert_eq!(apu.channel_3.wave_ram[1], 0x34);
    }

    #[test]
    fn dmg_channel_3_wave_ram_access_uses_the_internal_byte_only_during_the_fetch_window() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF1A, 0x80);
        apu.write_register(0xFF1C, 0x20);
        apu.write_register(0xFF1D, 0xFF);
        apu.write_register(0xFF1E, 0x87);
        apu.channel_3.wave_ram[0] = 0x12;
        apu.channel_3.wave_ram[1] = 0x34;

        for _ in 0..2 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES {
            apu.channel_3.begin_t_cycle();
            apu.channel_3.tick_fast_timer();
        }

        assert_eq!(apu.channel_3.sample_index, 1);
        assert_eq!(apu.read_register(0xFF30), 0x12);
        assert_eq!(apu.read_register(0xFF3F), 0x12);

        apu.write_register(0xFF30, 0xAB);

        assert_eq!(apu.channel_3.wave_ram[0], 0xAB);
        assert_eq!(apu.channel_3.wave_ram[1], 0x34);

        apu.channel_3.begin_t_cycle();

        assert_eq!(apu.read_register(0xFF30), WAVE_RAM_INACCESSIBLE_READ_VALUE);
    }

    #[test]
    fn cgb_channel_3_wave_ram_remains_addressable_while_active() {
        let mut apu = Apu::new(ConsoleModel::Cgb);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF1A, 0x80);
        apu.channel_3.runtime.active = true;
        apu.channel_3.wave_ram[0] = 0x12;
        apu.channel_3.wave_ram[1] = 0x34;

        assert_eq!(apu.read_register(0xFF30), 0x12);
        assert_eq!(apu.read_register(0xFF31), 0x34);

        apu.write_register(0xFF30, 0xAB);

        assert_eq!(apu.channel_3.wave_ram[0], 0xAB);
        assert_eq!(apu.channel_3.wave_ram[1], 0x34);
    }

    fn active_channel_3_test_state() -> Channel3State {
        Channel3State {
            nr30: NR30_DAC_POWER_BIT,
            runtime: ChannelRuntimeState {
                dac_enabled: true,
                active: true,
            },
            ..Channel3State::default()
        }
    }

    #[test]
    fn dmg_channel_3_retrigger_corrupts_wave_ram_byte_zero_two_t_cycles_before_the_next_fetch() {
        let mut channel = active_channel_3_test_state();
        channel.wave_ram = [
            0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ];
        channel.sample_index = 1;
        channel.period_timer = 2;

        channel.write_nr34(CHANNEL_TRIGGER_BIT, ConsoleModel::Dmg, 0);

        assert_eq!(channel.wave_ram[0], 0x11);
        assert_eq!(channel.wave_ram[1], 0x11);
        assert_eq!(channel.wave_ram[2], 0x22);
        assert_eq!(channel.wave_ram[3], 0x33);
    }

    #[test]
    fn dmg_channel_3_retrigger_corrupts_wave_ram_from_the_next_aligned_four_byte_block() {
        let mut channel = active_channel_3_test_state();
        channel.wave_ram = [
            0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ];
        channel.sample_index = 7;
        channel.period_timer = 2;

        channel.write_nr34(CHANNEL_TRIGGER_BIT, ConsoleModel::Dmg, 0);

        assert_eq!(channel.wave_ram[0], 0x44);
        assert_eq!(channel.wave_ram[1], 0x55);
        assert_eq!(channel.wave_ram[2], 0x66);
        assert_eq!(channel.wave_ram[3], 0x77);
    }

    #[test]
    fn channel_3_retrigger_corruption_is_gated_to_dmg_family_behavior() {
        let mut channel = active_channel_3_test_state();
        channel.wave_ram = [
            0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ];
        channel.sample_index = 7;
        channel.period_timer = 2;

        channel.write_nr34(CHANNEL_TRIGGER_BIT, ConsoleModel::Cgb, 0);

        assert_eq!(
            channel.wave_ram,
            [
                0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
                0xEE, 0xFF,
            ]
        );
    }

    #[test]
    fn dmg_channel_3_retrigger_corruption_requires_the_two_t_cycle_window() {
        let mut channel = active_channel_3_test_state();
        channel.wave_ram = [
            0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ];
        channel.sample_index = 7;
        channel.period_timer = 1;

        channel.write_nr34(CHANNEL_TRIGGER_BIT, ConsoleModel::Dmg, 0);

        assert_eq!(
            channel.wave_ram,
            [
                0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
                0xEE, 0xFF,
            ]
        );
    }

    #[test]
    fn channel_3_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        apu.write_register(0xFF1A, 0x80);
        apu.write_register(0xFF1B, 0xFF);
        apu.write_register(0xFF1E, LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_3.length_counter, 0);
        assert!(!apu.channel_3.runtime.active);

        apu.write_register(0xFF1E, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_3.length_counter, 255);
        assert!(apu.channel_3.runtime.active);
    }

    #[test]
    fn channel_4_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        apu.write_register(0xFF21, 0x08);
        apu.write_register(0xFF20, pulse_length_load(1));
        apu.write_register(0xFF23, LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_4.length_counter, 0);
        assert!(!apu.channel_4.runtime.active);

        apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_4.length_counter, 63);
        assert!(apu.channel_4.runtime.active);
    }

    #[test]
    fn channel_4_retrigger_after_unfreezing_zero_length_does_not_extra_clock_again() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.frame_sequencer.apply_startup_phase(7);
        apu.write_register(0xFF21, 0x08);
        apu.write_register(0xFF20, pulse_length_load(1));
        apu.write_register(0xFF23, LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_4.length_counter, 0);
        assert!(!apu.channel_4.runtime.active);

        apu.write_register(0xFF23, 0x00);
        apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
        assert_eq!(apu.channel_4.length_counter, 63);
        assert!(apu.channel_4.runtime.active);

        apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

        assert_eq!(apu.channel_4.length_counter, 63);
        assert!(apu.channel_4.runtime.active);
    }

    #[test]
    fn channel_4_trigger_reloads_envelope_lfsr_and_noise_timer() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF21, 0xF2);
        apu.write_register(0xFF22, 0x15);
        apu.write_register(0xFF23, 0x80);

        assert!(apu.channel_4.runtime.active);
        assert_eq!(apu.channel_4.current_volume, 0x0F);
        assert_eq!(apu.channel_4.envelope_timer, 2);
        assert_eq!(apu.channel_4.lfsr_state, NOISE_LFSR_INITIAL_STATE);
        assert_eq!(apu.channel_4.period_timer, 160);
    }

    #[test]
    fn channel_4_noise_timer_steps_the_lfsr_and_short_width_mode_copies_feedback_into_bit_six() {
        let mut channel = Channel4State::default();
        channel.runtime.dac_enabled = true;
        channel.runtime.active = true;
        channel.short_width_mode = true;
        channel.clock_shift = 0;
        channel.clock_divider_code = 0;
        channel.period_timer = 1;
        channel.current_volume = 0x0F;
        channel.lfsr_state = NOISE_LFSR_INITIAL_STATE;

        channel.tick_fast_timer();

        assert_eq!(channel.period_timer, 8);
        assert_eq!(channel.lfsr_state, 0x3FBF);
        assert_eq!(channel.current_digital_output(), 0x0F);
    }

    #[test]
    fn channel_4_envelope_reaching_zero_does_not_disable_the_channel() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF21, 0x11);
        apu.write_register(0xFF22, 0x00);
        apu.write_register(0xFF23, 0x80);

        assert!(apu.channel_4.runtime.active);
        assert_eq!(apu.channel_4.current_volume, 1);

        apu.channel_4.clock_envelope();

        assert_eq!(apu.channel_4.current_volume, 0);
        assert!(apu.channel_4.runtime.active);

        apu.channel_4.clock_envelope();

        assert_eq!(apu.channel_4.current_volume, 0);
        assert!(apu.channel_4.runtime.active);
    }

    #[test]
    fn channel_4_live_15_bit_to_7_bit_switch_can_lock_the_active_lfsr_window() {
        let mut wide = Channel4State::default();
        wide.runtime.dac_enabled = true;
        wide.runtime.active = true;
        wide.write_nr43(0x00);
        wide.period_timer = 1;
        wide.current_volume = 0x0F;
        wide.lfsr_state = 0x0080;

        let mut narrow = wide.clone();
        narrow.write_nr43(0x08);

        wide.tick_fast_timer();
        narrow.tick_fast_timer();

        assert_eq!(wide.lfsr_state & 0x7F, 0x40);
        assert_eq!(narrow.lfsr_state & 0x7F, 0x00);
        assert_eq!(narrow.current_digital_output(), 0);
        assert!(narrow.runtime.active);

        narrow.period_timer = 1;
        narrow.tick_fast_timer();

        assert_eq!(narrow.lfsr_state & 0x7F, 0x00);
        assert_eq!(narrow.current_digital_output(), 0);
        assert!(narrow.runtime.active);
    }

    #[test]
    fn channel_4_retrigger_recovers_from_short_width_lockup_without_clearing_activity() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF21, 0xF0);
        apu.write_register(0xFF22, 0x08);

        apu.channel_4.runtime.active = true;
        apu.channel_4.lfsr_state = 0x0000;
        apu.channel_4.current_volume = 0x0F;

        assert_eq!(apu.read_register(0xFF26) & 0x08, 0x08);
        assert_eq!(apu.channel_4.current_digital_output(), 0);

        apu.write_register(0xFF23, 0x80);

        assert_eq!(apu.channel_4.lfsr_state, NOISE_LFSR_INITIAL_STATE);
        assert!(apu.channel_4.runtime.active);
        assert_eq!(apu.read_register(0xFF26) & 0x08, 0x08);

        apu.channel_4.period_timer = 1;
        apu.channel_4.tick_fast_timer();

        assert_ne!(apu.channel_4.lfsr_state & 0x7F, 0x00);
    }

    #[test]
    fn dmg_powered_off_length_writes_preserve_internal_length_counters_without_restoring_register_state()
     {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF26, 0x00);

        apu.write_register(0xFF11, 0xD5);
        apu.write_register(0xFF16, 0xEA);
        apu.write_register(0xFF1B, 0x44);
        apu.write_register(0xFF20, 0xCD);

        assert_eq!(
            apu.channel_1.pulse.length_counter,
            pulse_length_counter_from_load(0xD5)
        );
        assert_eq!(
            apu.channel_2.pulse.length_counter,
            pulse_length_counter_from_load(0xEA)
        );
        assert_eq!(
            apu.channel_3.length_counter,
            wave_length_counter_from_load(0x44)
        );
        assert_eq!(
            apu.channel_4.length_counter,
            pulse_length_counter_from_load(0xCD)
        );
        assert_eq!(apu.read_register(0xFF11), 0x3F);
        assert_eq!(apu.read_register(0xFF16), 0x3F);
        assert_eq!(apu.read_register(0xFF1B), 0xFF);
        assert_eq!(apu.read_register(0xFF20), 0xFF);
    }

    #[test]
    fn startup_state_recreates_the_published_post_boot_audio_snapshot() {
        let mut apu = Apu::new(ConsoleModel::Dmg);

        apu.apply_startup_state(ApuStartupState {
            powered: true,
            nr10: 0x00,
            nr11: 0x80,
            nr12: 0xF3,
            nr13: 0x00,
            nr14: 0x00,
            nr21: 0x00,
            nr22: 0x00,
            nr23: 0x00,
            nr24: 0x00,
            nr30: 0x00,
            nr31: 0x00,
            nr32: 0x00,
            nr33: 0x00,
            nr34: 0x00,
            nr41: 0x00,
            nr42: 0x00,
            nr43: 0x00,
            nr44: 0x00,
            nr50: 0x77,
            nr51: 0xF3,
            channel_active_mask: CHANNEL_ACTIVE_CH1,
            div_apu: 0,
            wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
        });

        assert_eq!(apu.read_register(0xFF10), 0x80);
        assert_eq!(apu.read_register(0xFF11), 0xBF);
        assert_eq!(apu.read_register(0xFF12), 0xF3);
        assert_eq!(apu.read_register(0xFF13), 0xFF);
        assert_eq!(apu.read_register(0xFF14), 0xBF);
        assert_eq!(apu.read_register(0xFF16), 0x3F);
        assert_eq!(apu.read_register(0xFF17), 0x00);
        assert_eq!(apu.read_register(0xFF18), 0xFF);
        assert_eq!(apu.read_register(0xFF19), 0xBF);
        assert_eq!(apu.read_register(0xFF1A), 0x7F);
        assert_eq!(apu.read_register(0xFF1B), 0xFF);
        assert_eq!(apu.read_register(0xFF1C), 0x9F);
        assert_eq!(apu.read_register(0xFF1D), 0xFF);
        assert_eq!(apu.read_register(0xFF1E), 0xBF);
        assert_eq!(apu.read_register(0xFF20), 0xFF);
        assert_eq!(apu.read_register(0xFF21), 0x00);
        assert_eq!(apu.read_register(0xFF22), 0x00);
        assert_eq!(apu.read_register(0xFF23), 0xBF);
        assert_eq!(apu.read_register(0xFF24), 0x77);
        assert_eq!(apu.read_register(0xFF25), 0xF3);
        assert_eq!(apu.read_register(0xFF26), 0xF1);
        assert_eq!(apu.read_register(0xFF30), 0x00);

        let snapshot = apu.snapshot();
        assert_eq!(snapshot.channel_active_mask, CHANNEL_ACTIVE_CH1);
        assert_eq!(snapshot.channel_dac_mask, CHANNEL_ACTIVE_CH1);
        assert_eq!(snapshot.div_apu, 0);
        assert_eq!(
            snapshot.wave_ram_startup_policy,
            WaveRamStartupPolicy::DeterministicZeroed
        );
    }

    #[test]
    fn channel_2_3_and_4_register_paths_keep_dac_enable_and_trigger_distinct() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);

        apu.write_register(0xFF16, 0xC7);
        apu.write_register(0xFF17, 0x00);
        apu.write_register(0xFF18, 0x12);
        apu.write_register(0xFF19, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xF0);

        apu.write_register(0xFF17, 0xF3);
        apu.write_register(0xFF19, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xF2);

        apu.write_register(0xFF1A, 0x80);
        apu.write_register(0xFF1B, 0x55);
        apu.write_register(0xFF1D, 0x34);
        apu.write_register(0xFF1E, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xF6);

        apu.write_register(0xFF21, 0xF3);
        apu.write_register(0xFF22, 0x20);
        apu.write_register(0xFF23, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xFE);

        let snapshot = apu.snapshot();
        assert_eq!(snapshot.channel_active_mask, 0x0E);
        assert_eq!(snapshot.channel_dac_mask, 0x0E);

        apu.write_register(0xFF24, 0x77);
        apu.write_register(0xFF25, 0xF3);
        apu.write_register(0xFF15, 0x34);
        apu.write_register(0xFF1F, 0x12);
        apu.write_register(0xFF27, 0x56);
        apu.write_register(0xFF40, 0x78);

        assert_eq!(apu.read_register(0xFF16), 0xFF);
        assert_eq!(apu.read_register(0xFF17), 0xF3);
        assert_eq!(apu.read_register(0xFF18), 0xFF);
        assert_eq!(apu.read_register(0xFF19), 0xBF);
        assert_eq!(apu.read_register(0xFF1A), 0xFF);
        assert_eq!(apu.read_register(0xFF1B), 0xFF);
        assert_eq!(apu.read_register(0xFF1D), 0xFF);
        assert_eq!(apu.read_register(0xFF1E), 0xBF);
        assert_eq!(apu.read_register(0xFF21), 0xF3);
        assert_eq!(apu.read_register(0xFF22), 0x20);
        assert_eq!(apu.read_register(0xFF24), 0x77);
        assert_eq!(apu.read_register(0xFF25), 0xF3);
        assert_eq!(apu.read_register(0xFF1F), 0xFF);
        assert_eq!(apu.read_register(0xFF40), 0xFF);
    }

    #[test]
    fn powered_off_startup_state_matches_the_nr52_power_off_contract() {
        let mut apu = Apu::new(ConsoleModel::Dmg);

        apu.apply_startup_state(ApuStartupState {
            powered: false,
            nr10: 0x7F,
            nr11: 0xFF,
            nr12: 0xF3,
            nr13: 0x12,
            nr14: 0xFF,
            nr21: 0xFF,
            nr22: 0xF3,
            nr23: 0x34,
            nr24: 0xFF,
            nr30: 0xFF,
            nr31: 0x56,
            nr32: 0xFF,
            nr33: 0x78,
            nr34: 0xFF,
            nr41: 0x9A,
            nr42: 0xF3,
            nr43: 0xBC,
            nr44: 0xFF,
            nr50: 0x77,
            nr51: 0xF3,
            channel_active_mask: 0x0F,
            div_apu: 0xFF,
            wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
        });

        assert_eq!(apu.read_register(0xFF10), 0x80);
        assert_eq!(apu.read_register(0xFF12), 0x00);
        assert_eq!(apu.read_register(0xFF1A), 0x7F);
        assert_eq!(apu.read_register(0xFF24), 0x00);
        assert_eq!(apu.read_register(0xFF25), 0x00);
        assert_eq!(apu.read_register(0xFF26), 0x70);

        let snapshot = apu.snapshot();
        assert!(!snapshot.powered);
        assert_eq!(snapshot.channel_active_mask, 0x00);
        assert_eq!(snapshot.channel_dac_mask, 0x00);
        assert_eq!(snapshot.div_apu, 0x07);

        let context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);
        let trace = apu.scheduler_trace_message(&context);
        assert_eq!(
            trace,
            "t_cycle=0 phase=external_event_ingress console_model=Dmg status=Ready"
        );
    }

    #[test]
    fn powered_off_apu_keeps_div_apu_phase_in_sync_with_shared_edges() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.apply_startup_state(ApuStartupState {
            powered: false,
            nr10: 0x00,
            nr11: 0x00,
            nr12: 0x00,
            nr13: 0x00,
            nr14: 0x00,
            nr21: 0x00,
            nr22: 0x00,
            nr23: 0x00,
            nr24: 0x00,
            nr30: 0x00,
            nr31: 0x00,
            nr32: 0x00,
            nr33: 0x00,
            nr34: 0x00,
            nr41: 0x00,
            nr42: 0x00,
            nr43: 0x00,
            nr44: 0x00,
            nr50: 0x00,
            nr51: 0x00,
            channel_active_mask: 0x00,
            div_apu: 0x05,
            wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
        });

        tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);

        assert!(!apu.snapshot().powered);
        assert_eq!(apu.snapshot().div_apu, 0x06);
    }

    #[test]
    fn div_apu_phase_can_be_derived_from_the_shared_system_counter() {
        assert_eq!(div_apu_phase_from_system_counter(0x0000), 0x00);
        assert_eq!(div_apu_phase_from_system_counter(0x2000), 0x01);
        assert_eq!(div_apu_phase_from_system_counter(0xABC8), 0x05);
    }
}
