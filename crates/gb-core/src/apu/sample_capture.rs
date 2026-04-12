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
