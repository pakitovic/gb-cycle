mod ch1;
mod ch2;
mod ch3;
mod ch4;
mod pulse;

use super::common::{
    CHANNEL_ACTIVE_CH1, CHANNEL_ACTIVE_CH2, CHANNEL_ACTIVE_CH3, CHANNEL_ACTIVE_CH4,
    CHANNEL_ACTIVE_MASK, ChannelRuntimeState,
};

pub(super) use ch1::Channel1State;
pub(super) use ch2::Channel2State;
pub(super) use ch3::Channel3State;
pub(super) use ch4::Channel4State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ChannelOutputState {
    pub(super) active_mask: u8,
    pub(super) dac_mask: u8,
    pub(super) digital_outputs: [u8; 4],
}

pub(super) fn output_state(
    channel_1: &Channel1State,
    channel_2: &Channel2State,
    channel_3: &Channel3State,
    channel_4: &Channel4State,
) -> ChannelOutputState {
    ChannelOutputState {
        active_mask: runtime_mask(channel_1, channel_2, channel_3, channel_4, |runtime| {
            runtime.active
        }),
        dac_mask: runtime_mask(channel_1, channel_2, channel_3, channel_4, |runtime| {
            runtime.dac_enabled
        }),
        digital_outputs: [
            channel_1.pulse.current_digital_output(),
            channel_2.pulse.current_digital_output(),
            channel_3.current_digital_output(),
            channel_4.current_digital_output(),
        ],
    }
}

fn runtime_mask(
    channel_1: &Channel1State,
    channel_2: &Channel2State,
    channel_3: &Channel3State,
    channel_4: &Channel4State,
    select: impl Fn(ChannelRuntimeState) -> bool,
) -> u8 {
    let mut mask = 0;

    if select(channel_1.pulse.runtime) {
        mask |= CHANNEL_ACTIVE_CH1;
    }
    if select(channel_2.pulse.runtime) {
        mask |= CHANNEL_ACTIVE_CH2;
    }
    if select(channel_3.runtime) {
        mask |= CHANNEL_ACTIVE_CH3;
    }
    if select(channel_4.runtime) {
        mask |= CHANNEL_ACTIVE_CH4;
    }

    mask & CHANNEL_ACTIVE_MASK
}
