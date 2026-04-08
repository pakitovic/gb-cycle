use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, DerivedEdge};
use std::mem;

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
pub const DMG_FAMILY_APU_CAPTURE_CLOCK_HZ: u32 = 4_194_304;
pub const APU_HOST_MAX_ABS_SAMPLE: i32 = ANALOG_ONE * 4 * 8;
const DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64: u64 = DMG_FAMILY_APU_CAPTURE_CLOCK_HZ as u64;

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
    last_register_write: Option<ApuRegisterWriteObservation>,
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
    pub last_register_write: Option<ApuRegisterWriteObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApuRegisterWriteObservation {
    pub address: u16,
    pub value: u8,
    pub before: ApuRegisterWriteState,
    pub after: ApuRegisterWriteState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApuRegisterWriteState {
    pub powered: bool,
    pub nr50: u8,
    pub nr51: u8,
    pub nr52: u8,
    pub channel_active_mask: u8,
    pub channel_dac_mask: u8,
    pub output: ApuOutputSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApuStereoOutputSnapshot {
    pub left: i32,
    pub right: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApuHostSample {
    pub left: i32,
    pub right: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuSampleCaptureError {
    OutputSampleRateZero,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApuSampleCapture {
    output_sample_rate_hz: u32,
    sample_phase_accumulator: u64,
    pending_samples: Vec<ApuHostSample>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaveRamMmioPolicy {
    DmgCurrentByteDuringFetchOnly,
    DeferredCgbActiveAccess,
}

impl ApuStereoOutputSnapshot {
    const fn new(left: i32, right: i32) -> Self {
        Self { left, right }
    }
}

fn wave_ram_mmio_policy(console_model: ConsoleModel) -> WaveRamMmioPolicy {
    if console_model.is_dmg_family() {
        WaveRamMmioPolicy::DmgCurrentByteDuringFetchOnly
    } else {
        WaveRamMmioPolicy::DeferredCgbActiveAccess
    }
}

impl From<ApuStereoOutputSnapshot> for ApuHostSample {
    fn from(value: ApuStereoOutputSnapshot) -> Self {
        Self {
            left: value.left,
            right: value.right,
        }
    }
}

impl ApuSampleCapture {
    pub fn new(output_sample_rate_hz: u32) -> Result<Self, ApuSampleCaptureError> {
        if output_sample_rate_hz == 0 {
            return Err(ApuSampleCaptureError::OutputSampleRateZero);
        }

        Ok(Self {
            output_sample_rate_hz,
            sample_phase_accumulator: 0,
            pending_samples: Vec::new(),
        })
    }

    pub fn output_sample_rate_hz(&self) -> u32 {
        self.output_sample_rate_hz
    }

    pub fn pending_sample_count(&self) -> usize {
        self.pending_samples.len()
    }

    pub fn record_t_cycle(&mut self, apu: &Apu) {
        self.record_output_t_cycle(apu.host_output_sample());
    }

    pub fn record_output_t_cycle(&mut self, sample: ApuHostSample) {
        self.sample_phase_accumulator += u64::from(self.output_sample_rate_hz);
        while self.sample_phase_accumulator >= DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64 {
            self.sample_phase_accumulator -= DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64;
            self.pending_samples.push(sample);
        }
    }

    pub fn drain_samples(&mut self) -> Vec<ApuHostSample> {
        mem::take(&mut self.pending_samples)
    }

    pub fn drain_samples_into(&mut self, destination: &mut Vec<ApuHostSample>) {
        destination.clear();
        mem::swap(destination, &mut self.pending_samples);
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

const fn envelope_write_uses_consistent_zombie_increment(value: u8) -> bool {
    value & (ENVELOPE_DIRECTION_BIT | ENVELOPE_PACE_MASK) == ENVELOPE_DIRECTION_BIT
}

fn apply_consistent_zombie_mode_increment(active: bool, current_volume: &mut u8, value: u8) {
    // Pan Docs only documents increase+pace=0 as consistent across tested units; the broader
    // zombie-mode matrix remains revision-specific and is tracked separately.
    if !active || !envelope_write_uses_consistent_zombie_increment(value) {
        return;
    }

    *current_volume = (*current_volume + 1) & MAX_ENVELOPE_VOLUME;
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

    fn apply_live_envelope_write_effect(&mut self, value: u8) {
        apply_consistent_zombie_mode_increment(
            self.runtime.active,
            &mut self.current_volume,
            value,
        );
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
        self.pulse.apply_live_envelope_write_effect(value);
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
        self.pulse.apply_live_envelope_write_effect(value);
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

        if self.runtime.active {
            match wave_ram_mmio_policy(console_model) {
                WaveRamMmioPolicy::DmgCurrentByteDuringFetchOnly => {
                    return WAVE_RAM_INACCESSIBLE_READ_VALUE;
                }
                // The DMG-family fetch-window rule is the only active-wave-RAM
                // MMIO contract modeled today. CGB-family redirection semantics
                // are intentionally deferred until the CGB APU lane exists.
                WaveRamMmioPolicy::DeferredCgbActiveAccess => {}
            }
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

        if self.runtime.active {
            match wave_ram_mmio_policy(console_model) {
                WaveRamMmioPolicy::DmgCurrentByteDuringFetchOnly => return,
                // See the read path above: this is a deliberately provisional
                // fallback, not a claimed CGB-accurate active-access contract.
                WaveRamMmioPolicy::DeferredCgbActiveAccess => {}
            }
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
        self.apply_live_envelope_write_effect(value);
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

    fn apply_live_envelope_write_effect(&mut self, value: u8) {
        apply_consistent_zombie_mode_increment(
            self.runtime.active,
            &mut self.current_volume,
            value,
        );
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
            last_register_write: None,
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
        self.last_register_write = None;
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
        self.last_register_write = None;
        if let Some(index) = self.wave_ram_index(address) {
            self.channel_3
                .write_wave_ram(self.console_model, index, value);
            self.preview_output_path();
            return;
        }

        let before_register_write =
            Self::should_observe_register_write(address).then(|| self.register_write_state());

        if address == 0xFF26 {
            self.write_nr52(value, div_apu_source_high);
            self.preview_output_path();
            self.record_register_write_observation(address, value, before_register_write);
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
            self.record_register_write_observation(address, value, before_register_write);
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
        self.record_register_write_observation(address, value, before_register_write);
    }

    pub fn apply_startup_state(&mut self, startup_state: ApuStartupState) {
        self.last_register_write = None;
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
            last_register_write: self.last_register_write.clone(),
        }
    }

    pub fn host_output_sample(&self) -> ApuHostSample {
        self.output_path.current_output.into()
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        let output = self.output_snapshot();
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} powered={} nr50={:#04X} nr51={:#04X} nr52={:#04X} div_apu={} active_mask={:#04X} dac_mask={:#04X} channel_digital_outputs={:?} mixer=({}, {}) hpf=({}, {})",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.master.powered,
            self.master.nr50,
            self.master.nr51,
            self.read_nr52(),
            self.frame_sequencer.step,
            self.channel_active_mask(),
            self.channel_dac_mask(),
            output.channel_digital_outputs,
            output.mixer_output.left,
            output.mixer_output.right,
            output.hpf_output.left,
            output.hpf_output.right,
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

    fn should_observe_register_write(address: u16) -> bool {
        (0xFF10..=0xFF26).contains(&address)
    }

    fn register_write_state(&self) -> ApuRegisterWriteState {
        ApuRegisterWriteState {
            powered: self.master.powered,
            nr50: self.master.nr50,
            nr51: self.master.nr51,
            nr52: self.read_nr52(),
            channel_active_mask: self.channel_active_mask(),
            channel_dac_mask: self.channel_dac_mask(),
            output: self.output_snapshot(),
        }
    }

    fn record_register_write_observation(
        &mut self,
        address: u16,
        value: u8,
        before: Option<ApuRegisterWriteState>,
    ) {
        let Some(before) = before else {
            return;
        };

        self.last_register_write = Some(ApuRegisterWriteObservation {
            address,
            value,
            before,
            after: self.register_write_state(),
        });
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
mod tests;
