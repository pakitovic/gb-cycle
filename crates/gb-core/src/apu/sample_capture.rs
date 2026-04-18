use std::mem;

use super::common::DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64;
use super::{Apu, ApuHostSample};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuSampleCaptureError {
    OutputSampleRateZero,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApuSampleCapture {
    output_sample_rate_hz: u32,
    sample_phase_accumulator: u64,
    integrated_left: i128,
    integrated_right: i128,
    pending_samples: Vec<ApuHostSample>,
}

impl ApuSampleCapture {
    pub fn new(output_sample_rate_hz: u32) -> Result<Self, ApuSampleCaptureError> {
        if output_sample_rate_hz == 0 {
            return Err(ApuSampleCaptureError::OutputSampleRateZero);
        }

        Ok(Self {
            output_sample_rate_hz,
            sample_phase_accumulator: 0,
            integrated_left: 0,
            integrated_right: 0,
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
        let mut remaining_phase = u64::from(self.output_sample_rate_hz);

        while remaining_phase != 0 {
            let phase_until_emit =
                DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64 - self.sample_phase_accumulator;
            let phase_step = remaining_phase.min(phase_until_emit);

            self.integrated_left += i128::from(sample.left) * i128::from(phase_step);
            self.integrated_right += i128::from(sample.right) * i128::from(phase_step);

            self.sample_phase_accumulator += phase_step;
            remaining_phase -= phase_step;

            if self.sample_phase_accumulator == DMG_FAMILY_APU_CAPTURE_CLOCK_HZ_U64 {
                self.pending_samples.push(ApuHostSample {
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

    pub fn drain_samples(&mut self) -> Vec<ApuHostSample> {
        mem::take(&mut self.pending_samples)
    }

    pub fn drain_samples_into(&mut self, destination: &mut Vec<ApuHostSample>) {
        destination.clear();
        mem::swap(destination, &mut self.pending_samples);
    }
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
