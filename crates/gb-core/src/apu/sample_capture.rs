use std::f64::consts::PI;
use std::mem;

use super::common::DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64;
use super::{Apu, ApuHostSample};

const BAND_LIMITED_RESAMPLER_TAPS: usize = 32;
const BAND_LIMITED_RESAMPLER_PHASES: usize = 256;
const BAND_LIMITED_RESAMPLER_HALF_TAPS: f64 = BAND_LIMITED_RESAMPLER_TAPS as f64 / 2.0;
const BAND_LIMITED_RESAMPLER_COEFFICIENT_ONE: i64 = 0x1_0000;
const BAND_LIMITED_RESAMPLER_LOWPASS_MARGIN: f64 = 15.0 / 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuSampleCaptureError {
    OutputSampleRateZero,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApuSampleCapture {
    output_sample_rate_hz: u32,
    mode: ApuSampleCaptureMode,
    pending_samples: Vec<ApuHostSample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ApuSampleCaptureMode {
    Integrated(IntegratedSampleCaptureState),
    BandLimited(Box<BandLimitedSampleCaptureState>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct IntegratedSampleCaptureState {
    sample_phase_accumulator: u64,
    integrated_left: i128,
    integrated_right: i128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BandLimitedSampleCaptureState {
    sample_phase_accumulator: u64,
    history: [ApuHostSample; BAND_LIMITED_RESAMPLER_TAPS],
    history_write_index: usize,
    initialized: bool,
    coefficients: Vec<[i32; BAND_LIMITED_RESAMPLER_TAPS]>,
}

impl ApuSampleCapture {
    pub fn new(output_sample_rate_hz: u32) -> Result<Self, ApuSampleCaptureError> {
        if output_sample_rate_hz == 0 {
            return Err(ApuSampleCaptureError::OutputSampleRateZero);
        }

        let mode = if u64::from(output_sample_rate_hz) >= DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64 {
            ApuSampleCaptureMode::Integrated(IntegratedSampleCaptureState::default())
        } else {
            ApuSampleCaptureMode::BandLimited(Box::new(BandLimitedSampleCaptureState::new(
                output_sample_rate_hz,
            )))
        };

        Ok(Self {
            output_sample_rate_hz,
            mode,
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
        match &mut self.mode {
            ApuSampleCaptureMode::Integrated(state) => {
                state.record_output_t_cycle(
                    sample,
                    self.output_sample_rate_hz,
                    &mut self.pending_samples,
                );
            }
            ApuSampleCaptureMode::BandLimited(state) => {
                state.record_output_t_cycle(
                    sample,
                    self.output_sample_rate_hz,
                    &mut self.pending_samples,
                );
            }
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

impl IntegratedSampleCaptureState {
    fn record_output_t_cycle(
        &mut self,
        sample: ApuHostSample,
        output_sample_rate_hz: u32,
        pending_samples: &mut Vec<ApuHostSample>,
    ) {
        let mut remaining_phase = u64::from(output_sample_rate_hz);

        while remaining_phase != 0 {
            let phase_until_emit =
                DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64 - self.sample_phase_accumulator;
            let phase_step = remaining_phase.min(phase_until_emit);

            self.integrated_left += i128::from(sample.left) * i128::from(phase_step);
            self.integrated_right += i128::from(sample.right) * i128::from(phase_step);

            self.sample_phase_accumulator += phase_step;
            remaining_phase -= phase_step;

            if self.sample_phase_accumulator == DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64 {
                pending_samples.push(ApuHostSample {
                    left: divide_and_round_to_i32(
                        self.integrated_left,
                        i128::from(DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64),
                    ),
                    right: divide_and_round_to_i32(
                        self.integrated_right,
                        i128::from(DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64),
                    ),
                });
                self.sample_phase_accumulator = 0;
                self.integrated_left = 0;
                self.integrated_right = 0;
            }
        }
    }
}

impl BandLimitedSampleCaptureState {
    fn new(output_sample_rate_hz: u32) -> Self {
        Self {
            sample_phase_accumulator: 0,
            history: [ApuHostSample::default(); BAND_LIMITED_RESAMPLER_TAPS],
            history_write_index: 0,
            initialized: false,
            coefficients: build_band_limited_resampler_coefficients(output_sample_rate_hz),
        }
    }

    fn record_output_t_cycle(
        &mut self,
        sample: ApuHostSample,
        output_sample_rate_hz: u32,
        pending_samples: &mut Vec<ApuHostSample>,
    ) {
        if !self.initialized {
            self.history = [sample; BAND_LIMITED_RESAMPLER_TAPS];
            self.initialized = true;
        }

        self.history[self.history_write_index] = sample;
        self.history_write_index = (self.history_write_index + 1) % BAND_LIMITED_RESAMPLER_TAPS;

        let old_phase = self.sample_phase_accumulator;
        self.sample_phase_accumulator += u64::from(output_sample_rate_hz);

        if self.sample_phase_accumulator < DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64 {
            return;
        }

        let phase_until_emit = DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64 - old_phase;
        let phase_index = phase_index_from_remaining_phase(phase_until_emit, output_sample_rate_hz);
        pending_samples.push(self.capture_band_limited_sample(phase_index));
        self.sample_phase_accumulator -= DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64;
    }

    fn capture_band_limited_sample(&self, phase_index: usize) -> ApuHostSample {
        let coefficients = &self.coefficients[phase_index];
        let mut left = 0_i128;
        let mut right = 0_i128;

        for (tap, coefficient) in coefficients.iter().copied().enumerate() {
            let history_index = (self.history_write_index + BAND_LIMITED_RESAMPLER_TAPS - 1 - tap)
                % BAND_LIMITED_RESAMPLER_TAPS;
            let sample = self.history[history_index];
            left += i128::from(sample.left) * i128::from(coefficient);
            right += i128::from(sample.right) * i128::from(coefficient);
        }

        ApuHostSample {
            left: divide_and_round_to_i32(left, i128::from(BAND_LIMITED_RESAMPLER_COEFFICIENT_ONE)),
            right: divide_and_round_to_i32(
                right,
                i128::from(BAND_LIMITED_RESAMPLER_COEFFICIENT_ONE),
            ),
        }
    }
}

fn phase_index_from_remaining_phase(remaining_phase: u64, output_sample_rate_hz: u32) -> usize {
    let rate = u64::from(output_sample_rate_hz);
    debug_assert!(remaining_phase > 0);
    debug_assert!(remaining_phase <= rate);

    (((remaining_phase * BAND_LIMITED_RESAMPLER_PHASES as u64) - 1) / rate) as usize
}

fn build_band_limited_resampler_coefficients(
    output_sample_rate_hz: u32,
) -> Vec<[i32; BAND_LIMITED_RESAMPLER_TAPS]> {
    let cutoff_cycles_per_t_cycle = 0.5
        * (f64::from(output_sample_rate_hz) / DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64 as f64)
        * BAND_LIMITED_RESAMPLER_LOWPASS_MARGIN;
    let mut phases = Vec::with_capacity(BAND_LIMITED_RESAMPLER_PHASES);

    for phase in 0..BAND_LIMITED_RESAMPLER_PHASES {
        let fractional_t_cycle = (phase as f64 + 1.0) / BAND_LIMITED_RESAMPLER_PHASES as f64;
        let mut coefficients = [0_i32; BAND_LIMITED_RESAMPLER_TAPS];
        let mut floating = [0.0_f64; BAND_LIMITED_RESAMPLER_TAPS];
        let mut floating_sum = 0.0;

        for (tap, slot) in floating.iter_mut().enumerate() {
            let distance = tap as f64 + fractional_t_cycle - BAND_LIMITED_RESAMPLER_HALF_TAPS;
            let coefficient =
                low_pass_sinc(cutoff_cycles_per_t_cycle, distance) * blackman_window(tap);
            *slot = coefficient;
            floating_sum += coefficient;
        }

        debug_assert!(floating_sum > 0.0);
        let mut quantized_sum = 0_i64;
        for (tap, coefficient) in floating.iter().copied().enumerate() {
            let quantized = ((coefficient / floating_sum)
                * BAND_LIMITED_RESAMPLER_COEFFICIENT_ONE as f64) as i32;
            coefficients[tap] = quantized;
            quantized_sum += i64::from(quantized);
        }

        coefficients[BAND_LIMITED_RESAMPLER_TAPS / 2] +=
            (BAND_LIMITED_RESAMPLER_COEFFICIENT_ONE - quantized_sum) as i32;
        phases.push(coefficients);
    }

    phases
}

fn low_pass_sinc(cutoff_cycles_per_t_cycle: f64, distance: f64) -> f64 {
    if distance == 0.0 {
        return 2.0 * cutoff_cycles_per_t_cycle;
    }

    (2.0 * cutoff_cycles_per_t_cycle * PI * distance).sin() / (PI * distance)
}

fn blackman_window(tap: usize) -> f64 {
    let angle = 2.0 * PI * tap as f64 / (BAND_LIMITED_RESAMPLER_TAPS - 1) as f64;

    let a0 = 7938.0 / 18608.0;
    let a1 = 9240.0 / 18608.0;
    let a2 = 1430.0 / 18608.0;
    a0 - a1 * angle.cos() + a2 * (2.0 * angle).cos()
}

fn divide_and_round_to_i32(value: i128, divisor: i128) -> i32 {
    debug_assert!(divisor > 0);

    let rounded = if value >= 0 {
        (value + divisor / 2) / divisor
    } else {
        (value - divisor / 2) / divisor
    };

    rounded
        .try_into()
        .expect("captured host sample must stay within i32 range")
}
