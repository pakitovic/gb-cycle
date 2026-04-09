use crate::model::ConsoleModel;

use super::channels::ChannelOutputState;
use super::common::{
    ANALOG_ONE, CHANNEL_ACTIVE_CH1, CHANNEL_ACTIVE_CH2, CHANNEL_ACTIVE_CH3, CHANNEL_ACTIVE_CH4,
    DAC_ANALOG_STEP, DMG_FAMILY_HPF_CHARGE_FACTOR_NUMERATOR, HPF_CHARGE_FACTOR_DENOMINATOR,
    MGB_CGB_HPF_CHARGE_FACTOR_NUMERATOR, NR50_VIN_LEFT_BIT, NR50_VIN_RIGHT_BIT,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApuHpfCapacitorSnapshot {
    pub left: i64,
    pub right: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApuOutputSnapshot {
    pub channel_digital_outputs: [u8; 4],
    pub channel_dac_outputs: [i32; 4],
    pub vin_analog_output: ApuStereoOutputSnapshot,
    pub mixer_output: ApuStereoOutputSnapshot,
    pub master_output: ApuStereoOutputSnapshot,
    pub hpf_output: ApuStereoOutputSnapshot,
    pub hpf_capacitor: ApuHpfCapacitorSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct MasterControlState {
    pub(super) powered: bool,
    pub(super) nr50: u8,
    pub(super) nr51: u8,
    pub(super) vin_input: ApuStereoOutputSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HpfChargeModel {
    Dmg0Dmg,
    MgbCgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OutputPathState {
    pub(super) hpf_charge_model: HpfChargeModel,
    pub(super) hpf_capacitor: ApuHpfCapacitorSnapshot,
    pub(super) current_output: ApuStereoOutputSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OutputMixState {
    pub(super) channel_digital_outputs: [u8; 4],
    pub(super) channel_dac_outputs: [i32; 4],
    pub(super) vin_analog_output: ApuStereoOutputSnapshot,
    pub(super) mixer_output: ApuStereoOutputSnapshot,
    pub(super) master_output: ApuStereoOutputSnapshot,
    pub(super) any_dac_enabled: bool,
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

impl From<ApuStereoOutputSnapshot> for ApuHostSample {
    fn from(value: ApuStereoOutputSnapshot) -> Self {
        Self {
            left: value.left,
            right: value.right,
        }
    }
}

impl OutputPathState {
    pub(super) const fn new(console_model: ConsoleModel) -> Self {
        Self {
            hpf_charge_model: HpfChargeModel::for_console_model(console_model),
            hpf_capacitor: ApuHpfCapacitorSnapshot { left: 0, right: 0 },
            current_output: ApuStereoOutputSnapshot { left: 0, right: 0 },
        }
    }

    pub(super) fn preview(&mut self, input: ApuStereoOutputSnapshot, any_dac_enabled: bool) {
        if !any_dac_enabled {
            self.current_output = ApuStereoOutputSnapshot::default();
            return;
        }

        self.current_output = ApuStereoOutputSnapshot::new(
            (input.left as i64 - self.hpf_capacitor.left) as i32,
            (input.right as i64 - self.hpf_capacitor.right) as i32,
        );
    }

    pub(super) fn tick(&mut self, input: ApuStereoOutputSnapshot, any_dac_enabled: bool) {
        if !any_dac_enabled {
            self.current_output = ApuStereoOutputSnapshot::default();
            return;
        }

        let left_output = input.left as i64 - self.hpf_capacitor.left;
        let right_output = input.right as i64 - self.hpf_capacitor.right;
        let hpf_charge_factor_numerator = self.hpf_charge_model.numerator();

        self.current_output = ApuStereoOutputSnapshot::new(left_output as i32, right_output as i32);
        self.hpf_capacitor.left = input.left as i64
            - (left_output * hpf_charge_factor_numerator) / HPF_CHARGE_FACTOR_DENOMINATOR;
        self.hpf_capacitor.right = input.right as i64
            - (right_output * hpf_charge_factor_numerator) / HPF_CHARGE_FACTOR_DENOMINATOR;
    }
}

impl OutputMixState {
    pub(super) fn snapshot(self, output_path: &OutputPathState) -> ApuOutputSnapshot {
        ApuOutputSnapshot {
            channel_digital_outputs: self.channel_digital_outputs,
            channel_dac_outputs: self.channel_dac_outputs,
            vin_analog_output: self.vin_analog_output,
            mixer_output: self.mixer_output,
            master_output: self.master_output,
            hpf_output: output_path.current_output,
            hpf_capacitor: output_path.hpf_capacitor,
        }
    }
}

pub(super) fn mix_output(
    master: &MasterControlState,
    channel_output: ChannelOutputState,
) -> OutputMixState {
    let any_dac_enabled = channel_output.dac_mask != 0;
    let channel_dac_outputs =
        channel_dac_outputs(channel_output.dac_mask, channel_output.digital_outputs);
    let vin_analog_output = vin_analog_output(master);
    let mixer_output = mixer_output(master.nr51, vin_analog_output, channel_dac_outputs);
    let master_output = master_output(master.nr50, mixer_output, any_dac_enabled);

    OutputMixState {
        channel_digital_outputs: channel_output.digital_outputs,
        channel_dac_outputs,
        vin_analog_output,
        mixer_output,
        master_output,
        any_dac_enabled,
    }
}

pub(super) fn preview_output_path(output_path: &mut OutputPathState, mix: OutputMixState) {
    output_path.preview(mix.master_output, mix.any_dac_enabled);
}

pub(super) fn tick_output_path(output_path: &mut OutputPathState, mix: OutputMixState) {
    output_path.tick(mix.master_output, mix.any_dac_enabled);
}

pub(super) const fn dac_analog_output(digital_output: u8) -> i32 {
    ANALOG_ONE - ((digital_output & 0x0F) as i32) * DAC_ANALOG_STEP
}

pub(super) const fn nr50_left_volume_factor(nr50: u8) -> i32 {
    (((nr50 >> 4) & 0x07) as i32) + 1
}

pub(super) const fn nr50_right_volume_factor(nr50: u8) -> i32 {
    ((nr50 & 0x07) as i32) + 1
}

fn channel_dac_outputs(channel_dac_mask: u8, channel_digital_outputs: [u8; 4]) -> [i32; 4] {
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

fn mixer_output(
    nr51: u8,
    vin_analog_output: ApuStereoOutputSnapshot,
    channel_dac_outputs: [i32; 4],
) -> ApuStereoOutputSnapshot {
    let mut left = vin_analog_output.left;
    let mut right = vin_analog_output.right;

    if nr51 & 0x10 != 0 {
        left += channel_dac_outputs[0];
    }
    if nr51 & 0x20 != 0 {
        left += channel_dac_outputs[1];
    }
    if nr51 & 0x40 != 0 {
        left += channel_dac_outputs[2];
    }
    if nr51 & 0x80 != 0 {
        left += channel_dac_outputs[3];
    }

    if nr51 & 0x01 != 0 {
        right += channel_dac_outputs[0];
    }
    if nr51 & 0x02 != 0 {
        right += channel_dac_outputs[1];
    }
    if nr51 & 0x04 != 0 {
        right += channel_dac_outputs[2];
    }
    if nr51 & 0x08 != 0 {
        right += channel_dac_outputs[3];
    }

    ApuStereoOutputSnapshot::new(left, right)
}

fn master_output(
    nr50: u8,
    mixer_output: ApuStereoOutputSnapshot,
    any_dac_enabled: bool,
) -> ApuStereoOutputSnapshot {
    if !any_dac_enabled {
        return ApuStereoOutputSnapshot::default();
    }

    ApuStereoOutputSnapshot::new(
        mixer_output.left * nr50_left_volume_factor(nr50),
        mixer_output.right * nr50_right_volume_factor(nr50),
    )
}
