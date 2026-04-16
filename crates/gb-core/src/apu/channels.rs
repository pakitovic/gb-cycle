mod ch1;
mod ch2;
mod ch3;
mod ch4;
mod pulse;

use super::common::{CHANNEL_ACTIVE_MASK, CHANNEL_COUNT, CHANNEL_MASKS, ChannelRuntimeState};

pub(super) use ch1::Channel1State;
pub(super) use ch2::Channel2State;
pub(super) use ch3::Channel3State;
pub(super) use ch4::Channel4State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ChannelOutputState {
    pub(super) active_mask: u8,
    pub(super) dac_mask: u8,
    pub(super) digital_outputs: [u8; CHANNEL_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChannelResolvedOutput {
    runtime: ChannelRuntimeState,
    digital_output: u8,
}

pub(super) fn output_state(
    channel_1: &Channel1State,
    channel_2: &Channel2State,
    channel_3: &Channel3State,
    channel_4: &Channel4State,
) -> ChannelOutputState {
    let resolved = [
        ChannelResolvedOutput {
            runtime: channel_1.runtime_state(),
            digital_output: channel_1.current_digital_output(),
        },
        ChannelResolvedOutput {
            runtime: channel_2.runtime_state(),
            digital_output: channel_2.current_digital_output(),
        },
        ChannelResolvedOutput {
            runtime: channel_3.runtime_state(),
            digital_output: channel_3.current_digital_output(),
        },
        ChannelResolvedOutput {
            runtime: channel_4.runtime_state(),
            digital_output: channel_4.current_digital_output(),
        },
    ];

    let mut active_mask = 0;
    let mut dac_mask = 0;
    let mut digital_outputs = [0; CHANNEL_COUNT];

    for (index, resolved_channel) in resolved.into_iter().enumerate() {
        let mask = CHANNEL_MASKS[index];
        if resolved_channel.runtime.active {
            active_mask |= mask;
        }
        if resolved_channel.runtime.dac_enabled {
            dac_mask |= mask;
        }
        digital_outputs[index] = resolved_channel.digital_output;
    }

    ChannelOutputState {
        active_mask: active_mask & CHANNEL_ACTIVE_MASK,
        dac_mask: dac_mask & CHANNEL_ACTIVE_MASK,
        digital_outputs,
    }
}
