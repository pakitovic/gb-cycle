use crate::model::ConsoleModel;

use super::channels::ChannelOutputState;
use super::common::{
    ANALOG_ONE, CHANNEL_COUNT, CHANNEL_MASKS, DAC_ANALOG_STEP, DAC_DIGITAL_OUTPUT_MASK,
    DMG_FAMILY_APU_CAPTURE_CLOCK_HZ, DMG_FAMILY_HPF_CHARGE_FACTOR_NUMERATOR,
    HPF_CHARGE_FACTOR_DENOMINATOR, MGB_CGB_HPF_CHARGE_FACTOR_NUMERATOR, NR50_LEFT_VOLUME_SHIFT,
    NR50_VIN_LEFT_BIT, NR50_VIN_RIGHT_BIT, NR50_VOLUME_BIAS, NR50_VOLUME_MASK,
    NR51_LEFT_ROUTE_BITS, NR51_RIGHT_ROUTE_BITS,
};

const DAC_FADE_REFERENCE_RATE_HZ: u32 = 20_000;
const DAC_FADE_FACTOR_ONE: i64 = 1 << 16;
const DAC_OFF_FADE_T_CYCLES: u16 =
    DMG_FAMILY_APU_CAPTURE_CLOCK_HZ.div_ceil(DAC_FADE_REFERENCE_RATE_HZ) as u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ApuStereoOutputSnapshot {
    pub left: i32,
    pub right: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ApuHostSample {
    pub left: i32,
    pub right: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ApuHostDcBlocker {
    charge_factor: f64,
    previous_input_left: f64,
    previous_input_right: f64,
    previous_output_left: f64,
    previous_output_right: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApuHostHpf {
    charge_model: HpfChargeModel,
    capacitor: ApuHpfCapacitorSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ApuHpfCapacitorSnapshot {
    pub left: i64,
    pub right: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ApuOutputSnapshot {
    pub channel_digital_outputs: [u8; CHANNEL_COUNT],
    pub channel_dac_outputs: [i32; CHANNEL_COUNT],
    pub vin_analog_output: ApuStereoOutputSnapshot,
    pub mixer_output: ApuStereoOutputSnapshot,
    pub master_output: ApuStereoOutputSnapshot,
    pub hpf_output: ApuStereoOutputSnapshot,
    pub hpf_capacitor: ApuHpfCapacitorSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct MasterControlState {
    pub(super) powered: bool,
    pub(super) nr50: u8,
    pub(super) nr51: u8,
    pub(super) vin_input: ApuStereoOutputSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum HpfChargeModel {
    Dmg0Dmg,
    MgbCgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct OutputPathState {
    pub(super) hpf_charge_model: HpfChargeModel,
    dac_off_fade_model: DacOffFadeModel,
    channel_dac_states: [DacChannelState; CHANNEL_COUNT],
    pub(super) channel_dac_outputs: [i32; CHANNEL_COUNT],
    pub(super) vin_analog_output: ApuStereoOutputSnapshot,
    pub(super) mixer_output: ApuStereoOutputSnapshot,
    pub(super) master_output: ApuStereoOutputSnapshot,
    pub(super) hpf_capacitor: ApuHpfCapacitorSnapshot,
    pub(super) current_output: ApuStereoOutputSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DacOffFadeModel {
    duration_t_cycles: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
struct DacChannelState {
    current_output: i32,
    discharge_source_output: i32,
    discharge_remaining_t_cycles: u16,
}

impl ApuStereoOutputSnapshot {
    pub(super) const fn new(left: i32, right: i32) -> Self {
        Self { left, right }
    }
}

impl HpfChargeModel {
    const fn for_console_model(console_model: ConsoleModel) -> Self {
        match console_model {
            ConsoleModel::Dmg0 | ConsoleModel::Dmg => Self::Dmg0Dmg,
            ConsoleModel::Mgb | ConsoleModel::Cgb => Self::MgbCgb,
        }
    }

    const fn numerator(self) -> i64 {
        match self {
            Self::Dmg0Dmg => DMG_FAMILY_HPF_CHARGE_FACTOR_NUMERATOR,
            Self::MgbCgb => MGB_CGB_HPF_CHARGE_FACTOR_NUMERATOR,
        }
    }
}

impl DacOffFadeModel {
    const fn for_console_model(_console_model: ConsoleModel) -> Self {
        Self {
            duration_t_cycles: DAC_OFF_FADE_T_CYCLES,
        }
    }

    const fn duration_t_cycles(self) -> u16 {
        self.duration_t_cycles
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

impl ApuHostDcBlocker {
    pub fn new(console_model: ConsoleModel, output_sample_rate_hz: u32) -> Self {
        assert_ne!(
            output_sample_rate_hz, 0,
            "ApuHostDcBlocker requires a non-zero output sample rate"
        );

        Self {
            charge_factor: host_hpf_charge_factor(console_model, output_sample_rate_hz),
            previous_input_left: 0.0,
            previous_input_right: 0.0,
            previous_output_left: 0.0,
            previous_output_right: 0.0,
        }
    }

    pub fn filter_sample(&mut self, sample: ApuHostSample) -> ApuHostSample {
        ApuHostSample {
            left: self.filter_channel(sample.left as f64, true),
            right: self.filter_channel(sample.right as f64, false),
        }
    }

    pub fn reset(&mut self) {
        self.previous_input_left = 0.0;
        self.previous_input_right = 0.0;
        self.previous_output_left = 0.0;
        self.previous_output_right = 0.0;
    }

    fn filter_channel(&mut self, input: f64, left: bool) -> i32 {
        let (previous_input, previous_output) = if left {
            (
                &mut self.previous_input_left,
                &mut self.previous_output_left,
            )
        } else {
            (
                &mut self.previous_input_right,
                &mut self.previous_output_right,
            )
        };

        let output = input - *previous_input + self.charge_factor * *previous_output;
        *previous_input = input;
        *previous_output = output;
        output.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
    }
}

impl ApuHostHpf {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            charge_model: HpfChargeModel::for_console_model(console_model),
            capacitor: ApuHpfCapacitorSnapshot::default(),
        }
    }

    pub fn filter_t_cycle(
        &mut self,
        sample: ApuHostSample,
        any_output_connected: bool,
    ) -> ApuHostSample {
        if !any_output_connected {
            return ApuHostSample::default();
        }

        let left_output = i64::from(sample.left) - self.capacitor.left;
        let right_output = i64::from(sample.right) - self.capacitor.right;
        let hpf_charge_factor_numerator = self.charge_model.numerator();

        self.capacitor.left = i64::from(sample.left)
            - (left_output * hpf_charge_factor_numerator) / HPF_CHARGE_FACTOR_DENOMINATOR;
        self.capacitor.right = i64::from(sample.right)
            - (right_output * hpf_charge_factor_numerator) / HPF_CHARGE_FACTOR_DENOMINATOR;

        ApuHostSample {
            left: left_output as i32,
            right: right_output as i32,
        }
    }

    pub fn reset(&mut self) {
        self.capacitor = ApuHpfCapacitorSnapshot::default();
    }
}

impl OutputPathState {
    pub(super) const fn new(console_model: ConsoleModel) -> Self {
        Self {
            hpf_charge_model: HpfChargeModel::for_console_model(console_model),
            dac_off_fade_model: DacOffFadeModel::for_console_model(console_model),
            channel_dac_states: [DacChannelState {
                current_output: 0,
                discharge_source_output: 0,
                discharge_remaining_t_cycles: 0,
            }; CHANNEL_COUNT],
            channel_dac_outputs: [0; CHANNEL_COUNT],
            vin_analog_output: ApuStereoOutputSnapshot { left: 0, right: 0 },
            mixer_output: ApuStereoOutputSnapshot { left: 0, right: 0 },
            master_output: ApuStereoOutputSnapshot { left: 0, right: 0 },
            hpf_capacitor: ApuHpfCapacitorSnapshot { left: 0, right: 0 },
            current_output: ApuStereoOutputSnapshot { left: 0, right: 0 },
        }
    }

    pub(super) fn snapshot(
        &self,
        channel_digital_outputs: [u8; CHANNEL_COUNT],
    ) -> ApuOutputSnapshot {
        ApuOutputSnapshot {
            channel_digital_outputs,
            channel_dac_outputs: self.channel_dac_outputs,
            vin_analog_output: self.vin_analog_output,
            mixer_output: self.mixer_output,
            master_output: self.master_output,
            hpf_output: self.current_output,
            hpf_capacitor: self.hpf_capacitor,
        }
    }

    pub(super) fn preview(
        &mut self,
        master: &MasterControlState,
        channel_output: ChannelOutputState,
    ) {
        let any_channel_output_connected = self.resolve_mix_state(master, channel_output, false);
        if !any_channel_output_connected {
            self.current_output = ApuStereoOutputSnapshot::default();
            return;
        }

        self.current_output = ApuStereoOutputSnapshot::new(
            (self.master_output.left as i64 - self.hpf_capacitor.left) as i32,
            (self.master_output.right as i64 - self.hpf_capacitor.right) as i32,
        );
    }

    pub(super) fn tick(&mut self, master: &MasterControlState, channel_output: ChannelOutputState) {
        let any_channel_output_connected = self.resolve_mix_state(master, channel_output, true);
        if !any_channel_output_connected {
            self.current_output = ApuStereoOutputSnapshot::default();
            return;
        }

        let left_output = self.master_output.left as i64 - self.hpf_capacitor.left;
        let right_output = self.master_output.right as i64 - self.hpf_capacitor.right;
        let hpf_charge_factor_numerator = self.hpf_charge_model.numerator();

        self.current_output = ApuStereoOutputSnapshot::new(left_output as i32, right_output as i32);
        self.hpf_capacitor.left = self.master_output.left as i64
            - (left_output * hpf_charge_factor_numerator) / HPF_CHARGE_FACTOR_DENOMINATOR;
        self.hpf_capacitor.right = self.master_output.right as i64
            - (right_output * hpf_charge_factor_numerator) / HPF_CHARGE_FACTOR_DENOMINATOR;
    }

    fn resolve_mix_state(
        &mut self,
        master: &MasterControlState,
        channel_output: ChannelOutputState,
        advance_fade: bool,
    ) -> bool {
        if !master.powered {
            self.reset_analog_path();
            return false;
        }

        for (index, channel_mask) in CHANNEL_MASKS.iter().copied().enumerate() {
            let dac_enabled = channel_output.dac_mask & channel_mask != 0;
            let digital_output = channel_output.digital_outputs[index];
            self.channel_dac_outputs[index] = if advance_fade {
                self.channel_dac_states[index].tick(
                    dac_enabled,
                    digital_output,
                    self.dac_off_fade_model,
                )
            } else {
                self.channel_dac_states[index].preview(
                    dac_enabled,
                    digital_output,
                    self.dac_off_fade_model,
                )
            };
        }

        let any_channel_output_connected = self.channel_dac_outputs.iter().any(|&value| value != 0);

        self.vin_analog_output = vin_analog_output(master);
        self.mixer_output = mixer_output(
            master.nr51,
            self.vin_analog_output,
            self.channel_dac_outputs,
        );
        self.master_output =
            master_output(master.nr50, self.mixer_output, any_channel_output_connected);

        any_channel_output_connected
    }

    fn reset_analog_path(&mut self) {
        self.channel_dac_states = [DacChannelState::default(); CHANNEL_COUNT];
        self.channel_dac_outputs = [0; CHANNEL_COUNT];
        self.vin_analog_output = ApuStereoOutputSnapshot::default();
        self.mixer_output = ApuStereoOutputSnapshot::default();
        self.master_output = ApuStereoOutputSnapshot::default();
    }
}

impl DacChannelState {
    fn preview(
        &mut self,
        dac_enabled: bool,
        digital_output: u8,
        fade_model: DacOffFadeModel,
    ) -> i32 {
        if dac_enabled {
            return self.set_enabled_output(digital_output, fade_model);
        }

        self.current_output
    }

    fn tick(&mut self, dac_enabled: bool, digital_output: u8, fade_model: DacOffFadeModel) -> i32 {
        if dac_enabled {
            return self.set_enabled_output(digital_output, fade_model);
        }

        if self.current_output == 0 || self.discharge_remaining_t_cycles == 0 {
            self.current_output = 0;
            self.discharge_source_output = 0;
            self.discharge_remaining_t_cycles = 0;
            return 0;
        }

        self.discharge_remaining_t_cycles -= 1;
        if self.discharge_remaining_t_cycles == 0 {
            self.current_output = 0;
            self.discharge_source_output = 0;
            return 0;
        }

        self.current_output = scale_by_q16(
            self.discharge_source_output,
            smooth_factor_q16(
                self.discharge_remaining_t_cycles,
                fade_model.duration_t_cycles(),
            ),
        );
        self.current_output
    }

    fn set_enabled_output(&mut self, digital_output: u8, fade_model: DacOffFadeModel) -> i32 {
        let analog_output = dac_analog_output(digital_output);
        self.current_output = analog_output;
        self.discharge_source_output = analog_output;
        self.discharge_remaining_t_cycles = fade_model.duration_t_cycles();
        analog_output
    }
}

pub(super) const fn dac_analog_output(digital_output: u8) -> i32 {
    ANALOG_ONE - ((digital_output & DAC_DIGITAL_OUTPUT_MASK) as i32) * DAC_ANALOG_STEP
}

pub(super) const fn nr50_left_volume_factor(nr50: u8) -> i32 {
    (((nr50 >> NR50_LEFT_VOLUME_SHIFT) & NR50_VOLUME_MASK) as i32) + NR50_VOLUME_BIAS
}

pub(super) const fn nr50_right_volume_factor(nr50: u8) -> i32 {
    ((nr50 & NR50_VOLUME_MASK) as i32) + NR50_VOLUME_BIAS
}

fn vin_analog_output(master: &MasterControlState) -> ApuStereoOutputSnapshot {
    ApuStereoOutputSnapshot::new(
        if master.nr50 & NR50_VIN_LEFT_BIT != 0 {
            master.vin_input.left
        } else {
            0
        },
        if master.nr50 & NR50_VIN_RIGHT_BIT != 0 {
            master.vin_input.right
        } else {
            0
        },
    )
}

fn routed_channel_sum(
    nr51: u8,
    route_bits: [u8; CHANNEL_COUNT],
    channel_dac_outputs: [i32; CHANNEL_COUNT],
) -> i32 {
    let mut sum = 0;

    for index in 0..CHANNEL_COUNT {
        if nr51 & route_bits[index] != 0 {
            sum += channel_dac_outputs[index];
        }
    }

    sum
}

fn mixer_output(
    nr51: u8,
    vin_analog_output: ApuStereoOutputSnapshot,
    channel_dac_outputs: [i32; CHANNEL_COUNT],
) -> ApuStereoOutputSnapshot {
    let left = vin_analog_output.left
        + routed_channel_sum(nr51, NR51_LEFT_ROUTE_BITS, channel_dac_outputs);
    let right = vin_analog_output.right
        + routed_channel_sum(nr51, NR51_RIGHT_ROUTE_BITS, channel_dac_outputs);

    ApuStereoOutputSnapshot::new(left, right)
}

fn master_output(
    nr50: u8,
    mixer_output: ApuStereoOutputSnapshot,
    any_channel_output_connected: bool,
) -> ApuStereoOutputSnapshot {
    if !any_channel_output_connected {
        return ApuStereoOutputSnapshot::default();
    }

    ApuStereoOutputSnapshot::new(
        mixer_output.left * nr50_left_volume_factor(nr50),
        mixer_output.right * nr50_right_volume_factor(nr50),
    )
}

fn smooth_factor_q16(remaining_t_cycles: u16, total_t_cycles: u16) -> i32 {
    if remaining_t_cycles == 0 {
        return 0;
    }

    if remaining_t_cycles >= total_t_cycles {
        return DAC_FADE_FACTOR_ONE as i32;
    }

    let x = ((remaining_t_cycles as i64) << 16) / total_t_cycles as i64;
    let x2 = (x * x) / DAC_FADE_FACTOR_ONE;
    let x3 = (x2 * x) / DAC_FADE_FACTOR_ONE;

    (3 * x2 - 2 * x3) as i32
}

fn scale_by_q16(value: i32, factor_q16: i32) -> i32 {
    divide_and_round_i64(value as i64 * factor_q16 as i64, DAC_FADE_FACTOR_ONE)
}

fn host_hpf_charge_factor(console_model: ConsoleModel, output_sample_rate_hz: u32) -> f64 {
    let t_cycle_charge_factor = HpfChargeModel::for_console_model(console_model).numerator() as f64
        / HPF_CHARGE_FACTOR_DENOMINATOR as f64;
    t_cycle_charge_factor
        .powf(DMG_FAMILY_APU_CAPTURE_CLOCK_HZ as f64 / output_sample_rate_hz as f64)
}

fn divide_and_round_i64(value: i64, divisor: i64) -> i32 {
    if value >= 0 {
        ((value + divisor / 2) / divisor) as i32
    } else {
        ((value - divisor / 2) / divisor) as i32
    }
}
