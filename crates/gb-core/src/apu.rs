mod channels;
mod common;
mod control;
mod frame_sequencer;
mod mmio;
mod output;
mod registers;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::apu) struct ResolvedApuOutputState {
    pub(in crate::apu) channel_output: ChannelOutputState,
    pub(in crate::apu) output_mix: OutputMixState,
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
            self.tick_channel_fast_timers();
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
        let resolved = self.resolved_output_state();

        ApuSnapshot {
            console_model: self.console_model,
            status: self.status,
            powered: self.master.powered,
            nr50: self.master.nr50,
            nr51: self.master.nr51,
            channel_active_mask: resolved.channel_output.active_mask,
            channel_dac_mask: resolved.channel_output.dac_mask,
            div_apu: self.frame_sequencer.step,
            wave_ram: self.channel_3.wave_ram_snapshot(),
            wave_ram_startup_policy: self.wave_ram_startup_policy,
            output: self.output_snapshot_from_resolved(resolved),
            last_register_write: self.last_register_write.clone(),
        }
    }

    pub fn host_output_sample(&self) -> ApuHostSample {
        self.output_path.current_output.into()
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        let resolved = self.resolved_output_state();
        let output = self.output_snapshot_from_resolved(resolved);

        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} powered={} nr50={:#04X} nr51={:#04X} nr52={:#04X} div_apu={} active_mask={:#04X} dac_mask={:#04X} channel_digital_outputs={:?} mixer=({}, {}) hpf=({}, {})",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.master.powered,
            self.master.nr50,
            self.master.nr51,
            self.read_nr52_from_channel_output(resolved.channel_output),
            self.frame_sequencer.step,
            resolved.channel_output.active_mask,
            resolved.channel_output.dac_mask,
            output.channel_digital_outputs,
            output.mixer_output.left,
            output.mixer_output.right,
            output.hpf_output.left,
            output.hpf_output.right,
        )
    }

    fn channel_output_state(&self) -> ChannelOutputState {
        output_state(
            &self.channel_1,
            &self.channel_2,
            &self.channel_3,
            &self.channel_4,
        )
    }

    pub(in crate::apu) fn resolved_output_state(&self) -> ResolvedApuOutputState {
        let channel_output = self.channel_output_state();
        let output_mix = mix_output(&self.master, channel_output);

        ResolvedApuOutputState {
            channel_output,
            output_mix,
        }
    }

    pub(in crate::apu) fn output_snapshot_from_resolved(
        &self,
        resolved: ResolvedApuOutputState,
    ) -> ApuOutputSnapshot {
        resolved.output_mix.snapshot(&self.output_path)
    }

    fn preview_output_path(&mut self) {
        let resolved = self.resolved_output_state();
        preview_output_path(&mut self.output_path, resolved.output_mix);
    }

    fn tick_output_path(&mut self) {
        let resolved = self.resolved_output_state();
        tick_output_path(&mut self.output_path, resolved.output_mix);
    }

    fn tick_channel_fast_timers(&mut self) {
        self.channel_1.tick_fast_timer();
        self.channel_2.tick_fast_timer();
        self.channel_3.tick_fast_timer();
        self.channel_4.tick_fast_timer();
    }

    fn clock_channel_lengths(&mut self) {
        self.channel_1.clock_length();
        self.channel_2.clock_length();
        self.channel_3.clock_length();
        self.channel_4.clock_length();
    }

    fn clock_channel_envelopes(&mut self) {
        self.channel_1.clock_envelope();
        self.channel_2.clock_envelope();
        self.channel_4.clock_envelope();
    }

    fn advance_frame_sequencer(&mut self) {
        let clocks = self.frame_sequencer.advance();
        if !self.master.powered {
            self.preview_output_path();
            return;
        }

        if clocks.length {
            self.clock_channel_lengths();
        }
        if clocks.sweep {
            self.channel_1.clock_sweep();
        }
        if clocks.envelope {
            self.clock_channel_envelopes();
        }

        self.preview_output_path();
    }
}

#[cfg(test)]
mod tests;
