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

use self::channels::{ApuChannels, ChannelOutputState};
#[cfg(test)]
use self::channels::{Channel3State, Channel4State};
use self::common::*;
pub use self::common::{APU_HOST_MAX_ABS_SAMPLE, DMG_FAMILY_APU_CAPTURE_CLOCK_HZ};
pub use self::control::{ApuRegisterWriteObservation, ApuRegisterWriteState, ApuStartupState};
use self::frame_sequencer::FrameSequencerState;
pub(crate) use self::frame_sequencer::div_apu_phase_from_system_counter;
#[cfg(test)]
use self::output::HpfChargeModel;
pub use self::output::{
    ApuHostDcBlocker, ApuHostSample, ApuHpfCapacitorSnapshot, ApuOutputSnapshot,
    ApuStereoOutputSnapshot,
};
use self::output::{
    MasterControlState, OutputPathState, nr50_left_volume_factor, nr50_right_volume_factor,
};
pub use self::sample_capture::{ApuSampleCapture, ApuSampleCaptureError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApuRecordedChannel {
    Ch1,
    Ch2,
    Ch3,
    Ch4,
}

impl ApuRecordedChannel {
    pub const ALL: [Self; CHANNEL_COUNT] = [Self::Ch1, Self::Ch2, Self::Ch3, Self::Ch4];

    pub const fn index(self) -> usize {
        match self {
            Self::Ch1 => 0,
            Self::Ch2 => 1,
            Self::Ch3 => 2,
            Self::Ch4 => 3,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuCh4Nr43LiveWriteCategory {
    None,
    Category1,
    Category2,
    RisingEdgeForcedShort,
    LowShiftFollowup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuCh4Nr43LfsrAction {
    None,
    PlainStep,
    ForcedShortStep,
    ForcedShortStepThenLowShiftCorruption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApuCh4Nr43LiveWriteTrace {
    pub runtime_active: bool,
    pub same_shift_group: bool,
    pub old_nr43: u8,
    pub new_nr43: u8,
    pub glitch_value: u8,
    pub second_glitch_value: u8,
    pub old_shift: u8,
    pub glitch_shift: u8,
    pub second_glitch_shift: u8,
    pub new_shift: u8,
    pub effective_counter: u16,
    pub countdown_reloaded: bool,
    pub old_bit: bool,
    pub glitch_bit: bool,
    pub second_glitch_bit: bool,
    pub new_bit: bool,
    pub decision_category: ApuCh4Nr43LiveWriteCategory,
    pub lfsr_action: ApuCh4Nr43LfsrAction,
    pub reload_seam_step: bool,
    pub old_to_ff_step: bool,
    pub old_to_ff_forced_short_width: bool,
    pub ff_to_new_step: bool,
    pub ff_to_new_forced_short_width: bool,
    pub low_shift_extra_step: bool,
    pub feedback_corruption: bool,
    pub lfsr_before: u16,
    pub lfsr_after: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApuCh4DebugSnapshot {
    pub nr43: u8,
    pub clock_shift: u8,
    pub short_width_mode: bool,
    pub clock_divider_code: u8,
    pub alignment: u8,
    pub counter_timer: u32,
    pub noise_counter: u16,
    pub countdown_reloaded: bool,
    pub period_timer: u32,
    pub lfsr_state: u16,
    pub current_digital_output: u8,
    pub last_nr43_live_write: Option<ApuCh4Nr43LiveWriteTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apu {
    console_model: ConsoleModel,
    status: ApuStatus,
    master: MasterControlState,
    frame_sequencer: FrameSequencerState,
    output_path: OutputPathState,
    channels: ApuChannels,
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
pub(in crate::apu) struct ApuOutputResolution {
    pub(in crate::apu) channel_output: ChannelOutputState,
}

impl ApuOutputResolution {
    pub(in crate::apu) fn snapshot(self, output_path: &OutputPathState) -> ApuOutputSnapshot {
        output_path.snapshot(self.channel_output.digital_outputs)
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
            output_path: OutputPathState::new(console_model),
            channels: ApuChannels::default(),
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
        self.channels.begin_t_cycle();

        if self.master.powered {
            self.channels.tick_fast_timers();
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
        let output_resolution = self.resolve_output_state();

        ApuSnapshot {
            console_model: self.console_model,
            status: self.status,
            powered: self.master.powered,
            nr50: self.master.nr50,
            nr51: self.master.nr51,
            channel_active_mask: output_resolution.channel_output.active_mask,
            channel_dac_mask: output_resolution.channel_output.dac_mask,
            div_apu: self.frame_sequencer.step,
            wave_ram: self.channels.wave_ram_snapshot(),
            wave_ram_startup_policy: self.wave_ram_startup_policy,
            output: output_resolution.snapshot(&self.output_path),
            last_register_write: self.last_register_write.clone(),
        }
    }

    pub fn host_output_sample(&self) -> ApuHostSample {
        self.output_path.current_output.into()
    }

    pub fn recorded_channel_sample_pre_hpf(&self, channel: ApuRecordedChannel) -> ApuHostSample {
        let index = channel.index();
        let channel_dac_output = self.output_path.channel_dac_outputs[index];

        ApuHostSample {
            left: routed_recorded_channel_output(
                self.master.nr51,
                NR51_LEFT_ROUTE_BITS[index],
                channel_dac_output,
                nr50_left_volume_factor(self.master.nr50),
            ),
            right: routed_recorded_channel_output(
                self.master.nr51,
                NR51_RIGHT_ROUTE_BITS[index],
                channel_dac_output,
                nr50_right_volume_factor(self.master.nr50),
            ),
        }
    }

    pub fn last_register_write(&self) -> Option<&ApuRegisterWriteObservation> {
        self.last_register_write.as_ref()
    }

    pub fn channel_4_debug_snapshot(&self) -> ApuCh4DebugSnapshot {
        self.channels.channel_4.debug_snapshot()
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        let output_resolution = self.resolve_output_state();
        let output = output_resolution.snapshot(&self.output_path);

        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} powered={} nr50={:#04X} nr51={:#04X} nr52={:#04X} div_apu={} active_mask={:#04X} dac_mask={:#04X} channel_digital_outputs={:?} mixer=({}, {}) hpf=({}, {})",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.master.powered,
            self.master.nr50,
            self.master.nr51,
            self.read_nr52_from_channel_output(output_resolution.channel_output),
            self.frame_sequencer.step,
            output_resolution.channel_output.active_mask,
            output_resolution.channel_output.dac_mask,
            output.channel_digital_outputs,
            output.mixer_output.left,
            output.mixer_output.right,
            output.hpf_output.left,
            output.hpf_output.right,
        )
    }

    fn channel_output_state(&self) -> ChannelOutputState {
        self.channels.output_state()
    }

    pub(in crate::apu) fn resolve_output_state(&self) -> ApuOutputResolution {
        ApuOutputResolution {
            channel_output: self.channel_output_state(),
        }
    }

    fn preview_output_path(&mut self) {
        let output_resolution = self.resolve_output_state();
        self.output_path
            .preview(&self.master, output_resolution.channel_output);
    }

    fn tick_output_path(&mut self) {
        let output_resolution = self.resolve_output_state();
        self.output_path
            .tick(&self.master, output_resolution.channel_output);
    }

    fn advance_frame_sequencer(&mut self) {
        let clocks = self.frame_sequencer.advance();
        if !self.master.powered {
            self.preview_output_path();
            return;
        }

        if clocks.length {
            self.channels.clock_length_all();
        }
        if clocks.sweep {
            self.channels.clock_sweep_ch1();
        }
        if clocks.envelope {
            self.channels.clock_envelope_all();
        }

        self.preview_output_path();
    }
}

fn routed_recorded_channel_output(
    nr51: u8,
    route_bit: u8,
    channel_dac_output: i32,
    nr50_volume_factor: i32,
) -> i32 {
    if channel_dac_output == 0 || nr51 & route_bit == 0 {
        return 0;
    }

    channel_dac_output * nr50_volume_factor
}

#[cfg(test)]
mod tests;
