mod channels;
mod common;
mod control;
mod frame_sequencer;
mod mmio;
mod output;
mod sample_capture;

use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, DerivedEdge};

use self::channels::{
    Channel1State, Channel2State, Channel3State, Channel4State, ChannelOutputState, output_state,
};
use self::common::*;
pub use self::common::{APU_HOST_MAX_ABS_SAMPLE, DMG_FAMILY_APU_CAPTURE_CLOCK_HZ};
pub use self::control::{ApuRegisterWriteObservation, ApuRegisterWriteState, ApuStartupState};
use self::frame_sequencer::FrameSequencerState;
pub(crate) use self::frame_sequencer::div_apu_phase_from_system_counter;
#[cfg(test)]
use self::output::HpfChargeModel;
pub use self::output::{
    ApuHostSample, ApuHpfCapacitorSnapshot, ApuOutputSnapshot, ApuStereoOutputSnapshot,
};
use self::output::{
    MasterControlState, OutputMixState, OutputPathState, mix_output, preview_output_path,
    tick_output_path,
};
pub use self::sample_capture::{ApuSampleCapture, ApuSampleCaptureError};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apu {
    console_model: ConsoleModel,
    status: ApuStatus,
    master: MasterControlState,
    frame_sequencer: FrameSequencerState,
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

impl Apu {
    pub fn new(console_model: ConsoleModel) -> Self {
        let wave_ram_startup_policy = WaveRamStartupPolicy::DeterministicZeroed;

        Self {
            console_model,
            status: ApuStatus::Ready,
            master: MasterControlState::default(),
            frame_sequencer: FrameSequencerState::default(),
            output_path: OutputPathState::new(console_model),
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

    fn channel_active_mask(&self) -> u8 {
        self.channel_output_state().active_mask
    }

    fn channel_dac_mask(&self) -> u8 {
        self.channel_output_state().dac_mask
    }

    fn channel_output_state(&self) -> ChannelOutputState {
        output_state(
            &self.channel_1,
            &self.channel_2,
            &self.channel_3,
            &self.channel_4,
        )
    }

    fn output_mix(&self) -> OutputMixState {
        mix_output(&self.master, self.channel_output_state())
    }

    fn output_snapshot(&self) -> ApuOutputSnapshot {
        self.output_mix().snapshot(&self.output_path)
    }

    fn preview_output_path(&mut self) {
        let output_mix = self.output_mix();
        preview_output_path(&mut self.output_path, output_mix);
    }

    fn tick_output_path(&mut self) {
        let output_mix = self.output_mix();
        tick_output_path(&mut self.output_path, output_mix);
    }

    fn advance_frame_sequencer(&mut self) {
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
