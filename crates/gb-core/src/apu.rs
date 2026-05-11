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
use crate::speed::CgbSpeedMode;

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
    ApuHostDcBlocker, ApuHostHpf, ApuHostSample, ApuHpfCapacitorSnapshot, ApuOutputSnapshot,
    ApuStereoOutputSnapshot,
};
use self::output::{
    MasterControlState, OutputPathState, nr50_left_volume_factor, nr50_right_volume_factor,
};
pub use self::sample_capture::{ApuSampleCapture, ApuSampleCaptureError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApuStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ApuRecordedChannelMask {
    bits: u8,
}

impl ApuRecordedChannelMask {
    pub const NONE: Self = Self { bits: 0 };
    pub const ALL: Self = Self {
        bits: (1 << CHANNEL_COUNT) - 1,
    };

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub const fn is_all(self) -> bool {
        self.bits == Self::ALL.bits
    }

    pub const fn contains(self, channel: ApuRecordedChannel) -> bool {
        self.bits & channel_mask_bit(channel) != 0
    }

    pub const fn with_channel(self, channel: ApuRecordedChannel, enabled: bool) -> Self {
        let channel_bit = channel_mask_bit(channel);
        if enabled {
            Self {
                bits: self.bits | channel_bit,
            }
        } else {
            Self {
                bits: self.bits & !channel_bit,
            }
        }
    }

    pub const fn toggled(self, channel: ApuRecordedChannel) -> Self {
        Self {
            bits: self.bits ^ channel_mask_bit(channel),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ApuRecordedChannelMixTap {
    pub sample: ApuHostSample,
    pub any_output_connected: bool,
}

impl Default for ApuRecordedChannelMask {
    fn default() -> Self {
        Self::ALL
    }
}

const fn channel_mask_bit(channel: ApuRecordedChannel) -> u8 {
    1 << channel.index()
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum WaveRamStartupPolicy {
    #[default]
    DeterministicZeroed,
    CgbRealBootAlternating,
}

impl WaveRamStartupPolicy {
    pub const fn initial_bytes(self) -> [u8; WAVE_RAM_LEN] {
        match self {
            Self::DeterministicZeroed => [0; WAVE_RAM_LEN],
            Self::CgbRealBootAlternating => [
                0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF,
                0x00, 0xFF,
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApuCh4Nr43LiveWriteCategory {
    None,
    Category1,
    Category2,
    RisingEdgeForcedShort,
    LowShiftFollowup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApuCh4Nr43LfsrAction {
    None,
    PlainStep,
    ForcedShortStep,
    ForcedShortStepThenLowShiftCorruption,
    StepThenAndPrevious,
    StepThenSetFeedbackBits,
    SetFeedbackBits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApuCh4Nr43PassKind {
    ReloadSeam,
    OldToFf,
    FfToGlitch1,
    Glitch1ToGlitch2,
    GlitchToNew,
    LowShiftFollowup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApuCh4Nr43PassTrace {
    pub kind: ApuCh4Nr43PassKind,
    pub value_from: u8,
    pub value_to: u8,
    pub shift_from: u8,
    pub shift_to: u8,
    pub bit_from: bool,
    pub bit_to: bool,
    pub category: ApuCh4Nr43LiveWriteCategory,
    pub action: ApuCh4Nr43LfsrAction,
    pub lfsr_before: u16,
    pub lfsr_after: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApuCh4Nr43LiveWriteTrace {
    pub runtime_active: bool,
    pub same_shift_group: bool,
    pub old_nr43: u8,
    pub ff_value: u8,
    pub glitch_1_value: u8,
    pub glitch_2_value: Option<u8>,
    pub old_shift: u8,
    pub ff_shift: u8,
    pub glitch_1_shift: u8,
    pub glitch_2_shift: Option<u8>,
    pub new_shift: u8,
    pub new_nr43: u8,
    pub effective_counter: u16,
    pub countdown_reloaded: bool,
    pub old_bit: bool,
    pub ff_bit: bool,
    pub glitch_1_bit: bool,
    pub glitch_2_bit: Option<bool>,
    pub new_bit: bool,
    // Compatibility aliases derived from the explicit per-pass trace set.
    pub decision_category: ApuCh4Nr43LiveWriteCategory,
    pub lfsr_action: ApuCh4Nr43LfsrAction,
    pub reload_seam: Option<ApuCh4Nr43PassTrace>,
    pub old_to_ff: Option<ApuCh4Nr43PassTrace>,
    pub ff_to_glitch_1: Option<ApuCh4Nr43PassTrace>,
    pub glitch_1_to_glitch_2: Option<ApuCh4Nr43PassTrace>,
    pub glitch_to_new: Option<ApuCh4Nr43PassTrace>,
    pub low_shift_followup: Option<ApuCh4Nr43PassTrace>,
    pub lfsr_before: u16,
    pub lfsr_after: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApuCh4DebugSnapshot {
    pub nr43: u8,
    pub clock_shift: u8,
    pub short_width_mode: bool,
    pub clock_divider_code: u8,
    pub alignment: u8,
    pub counter_timer: u32,
    pub noise_counter: u16,
    pub countdown_reloaded: bool,
    pub did_step_counter: bool,
    pub counter_active: bool,
    pub background_counting: bool,
    pub started_with_dac_disabled: bool,
    pub dmg_delayed_start: u8,
    pub runtime_active: bool,
    pub runtime_dac_enabled: bool,
    pub period_timer: u32,
    pub lfsr_state: u16,
    pub current_digital_output: u8,
    pub last_nr43_live_write: Option<ApuCh4Nr43LiveWriteTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Apu {
    console_model: ConsoleModel,
    status: ApuStatus,
    master: MasterControlState,
    frame_sequencer: FrameSequencerState,
    output_path: OutputPathState,
    channels: ApuChannels,
    last_register_write: Option<ApuRegisterWriteObservation>,
    wave_ram_startup_policy: WaveRamStartupPolicy,
    #[serde(default)]
    apu_clock: u8,
    #[serde(default)]
    t_cycle_phase: u8,
    #[serde(default)]
    skip_next_frame_sequencer_edge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApuSaveState {
    console_model: ConsoleModel,
    status: ApuStatus,
    master: MasterControlState,
    frame_sequencer: FrameSequencerState,
    output_path: OutputPathState,
    channels: ApuChannels,
    last_register_write: Option<ApuRegisterWriteObservation>,
    wave_ram_startup_policy: WaveRamStartupPolicy,
    #[serde(default)]
    apu_clock: u8,
    #[serde(default)]
    t_cycle_phase: u8,
    #[serde(default)]
    skip_next_frame_sequencer_edge: bool,
}

impl ApuSaveState {
    pub(crate) const fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
            apu_clock: 0,
            t_cycle_phase: 0,
            skip_next_frame_sequencer_edge: false,
        }
    }

    pub(crate) fn capture_save_state(&self) -> ApuSaveState {
        ApuSaveState {
            console_model: self.console_model,
            status: self.status,
            master: self.master,
            frame_sequencer: self.frame_sequencer,
            output_path: self.output_path,
            channels: self.channels.clone(),
            last_register_write: self.last_register_write.clone(),
            wave_ram_startup_policy: self.wave_ram_startup_policy,
            apu_clock: self.apu_clock,
            t_cycle_phase: self.t_cycle_phase,
            skip_next_frame_sequencer_edge: self.skip_next_frame_sequencer_edge,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &ApuSaveState) {
        self.console_model = state.console_model;
        self.status = state.status;
        self.master = state.master;
        self.frame_sequencer = state.frame_sequencer;
        self.output_path = state.output_path;
        self.channels = state.channels.clone();
        self.last_register_write = state.last_register_write.clone();
        self.wave_ram_startup_policy = state.wave_ram_startup_policy;
        self.apu_clock = state.apu_clock;
        self.t_cycle_phase = state.t_cycle_phase;
        self.skip_next_frame_sequencer_edge = state.skip_next_frame_sequencer_edge;
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> ApuStatus {
        self.status
    }

    pub fn read_pcm12(&self) -> u8 {
        let outputs = self.channel_output_state().digital_outputs;
        (outputs[1] << 4) | outputs[0]
    }

    pub fn read_pcm34(&self) -> u8 {
        let outputs = self.channel_output_state().digital_outputs;
        (outputs[3] << 4) | outputs[2]
    }

    #[cfg(test)]
    pub(crate) fn tick_t_cycle(&mut self, context: &CycleContext) {
        self.tick_t_cycle_for_speed(context, CgbSpeedMode::Normal);
    }

    pub(crate) fn tick_t_cycle_for_speed(
        &mut self,
        context: &CycleContext,
        speed_mode: CgbSpeedMode,
    ) {
        self.last_register_write = None;
        self.channels.begin_t_cycle();

        let clock_generation_timers =
            speed_mode.apu_tick_due_at_scheduler_t_cycle(context.t_cycle().get());

        let apu_clock = self.apu_clock;
        let t_cycle_phase = self.t_cycle_phase;
        let is_tick_even_phase = t_cycle_phase & 0x01 == 0;

        if self.master.powered {
            if is_tick_even_phase && clock_generation_timers {
                self.apu_clock = (self.apu_clock + 1) & 0x03;
            }
            self.channels.tick_fast_timers(
                self.console_model,
                clock_generation_timers,
                self.apu_clock,
                t_cycle_phase,
            );
        } else if clock_generation_timers {
            self.channels.tick_powered_off_timebase();
        }
        let _ = apu_clock;

        if clock_generation_timers {
            self.t_cycle_phase = (t_cycle_phase + 1) & 0x03;
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

    pub fn recorded_channel_tap_pre_hpf(
        &self,
        channel: ApuRecordedChannel,
    ) -> ApuRecordedChannelMixTap {
        let sample = self.recorded_channel_sample_pre_hpf(channel);
        ApuRecordedChannelMixTap {
            sample,
            any_output_connected: sample.left != 0 || sample.right != 0,
        }
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

    pub fn recorded_channel_mix_tap_pre_hpf(
        &self,
        channel_mask: ApuRecordedChannelMask,
    ) -> ApuRecordedChannelMixTap {
        if channel_mask.is_empty() {
            return ApuRecordedChannelMixTap::default();
        }

        if channel_mask.is_all() {
            return ApuRecordedChannelMixTap {
                sample: self.output_path.master_output.into(),
                any_output_connected: self
                    .output_path
                    .channel_dac_outputs
                    .iter()
                    .any(|&value| value != 0),
            };
        }

        let mut mixed = ApuHostSample::default();
        let mut any_output_connected = false;
        for channel in ApuRecordedChannel::ALL {
            if !channel_mask.contains(channel) {
                continue;
            }

            let channel_sample = self.recorded_channel_sample_pre_hpf(channel);
            mixed.left += channel_sample.left;
            mixed.right += channel_sample.right;
            any_output_connected |= channel_sample.left != 0 || channel_sample.right != 0;
        }

        ApuRecordedChannelMixTap {
            sample: mixed,
            any_output_connected,
        }
    }

    pub fn recorded_channel_mix_pre_hpf(
        &self,
        channel_mask: ApuRecordedChannelMask,
    ) -> ApuHostSample {
        self.recorded_channel_mix_tap_pre_hpf(channel_mask).sample
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
        if self.skip_next_frame_sequencer_edge {
            self.skip_next_frame_sequencer_edge = false;
            self.preview_output_path();
            return;
        }

        let clocks = self.frame_sequencer.advance();
        if !self.master.powered {
            self.preview_output_path();
            return;
        }

        if clocks.length {
            self.channels.clock_length_all();
        }
        if clocks.sweep {
            self.channels.clock_sweep_ch1(self.console_model);
        }
        if !clocks.length && self.console_model.is_cgb_family() {
            self.channels
                .clock_cgb_live_write_pending_even_envelope_all();
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
