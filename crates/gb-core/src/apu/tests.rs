use super::*;
use crate::scheduler::TCycle;

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

const fn pulse_length_load(counter: u8) -> u8 {
    0xC0 | ((PULSE_LENGTH_COUNTER_RELOAD - counter) & PULSE_LENGTH_LOAD_MASK)
}

fn prime_channel_2_trigger_test(apu: &mut Apu, length_counter: u8) {
    apu.write_register(0xFF17, 0x08);
    apu.write_register(0xFF16, pulse_length_load(PULSE_LENGTH_COUNTER_RELOAD));
    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);
    apu.write_register(0xFF16, pulse_length_load(length_counter));
}

fn prime_channel_1_trigger_test(apu: &mut Apu, length_counter: u8) {
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF11, pulse_length_load(PULSE_LENGTH_COUNTER_RELOAD));
    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT);
    apu.write_register(0xFF11, pulse_length_load(length_counter));
}
