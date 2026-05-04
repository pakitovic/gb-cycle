use super::*;
use crate::scheduler::TCycle;
use crate::speed::CgbSpeedMode;

mod frame;
mod noise;
mod output;
mod pulse;
mod state;
mod wave;

fn tick_apu_with_edges(apu: &mut Apu, t_cycle: u64, edges: &[DerivedEdge]) {
    let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
    for &edge in edges {
        context.push_derived_edge(edge);
    }
    apu.tick_t_cycle(&context);
}

fn tick_apu_for_speed(apu: &mut Apu, t_cycle: u64, speed_mode: CgbSpeedMode) {
    let context = CycleContext::for_cycle(TCycle::new(t_cycle));
    apu.tick_t_cycle_for_speed(&context, speed_mode);
}

const fn pulse_length_load(counter: u8) -> u8 {
    0xC0 | ((PULSE_LENGTH_COUNTER_RELOAD - counter) & PULSE_LENGTH_LOAD_MASK)
}

struct PulseChannelAddresses {
    envelope: u16,
    length_duty: u16,
    trigger: u16,
}

const CHANNEL_1: PulseChannelAddresses = PulseChannelAddresses {
    envelope: 0xFF12,
    length_duty: 0xFF11,
    trigger: 0xFF14,
};

const CHANNEL_2: PulseChannelAddresses = PulseChannelAddresses {
    envelope: 0xFF17,
    length_duty: 0xFF16,
    trigger: 0xFF19,
};

fn prime_pulse_trigger_test(apu: &mut Apu, ch: &PulseChannelAddresses, length_counter: u8) {
    apu.write_register(ch.envelope, 0x08);
    apu.write_register(
        ch.length_duty,
        pulse_length_load(PULSE_LENGTH_COUNTER_RELOAD),
    );
    apu.write_register(ch.trigger, CHANNEL_TRIGGER_BIT);
    apu.write_register(ch.length_duty, pulse_length_load(length_counter));
}
