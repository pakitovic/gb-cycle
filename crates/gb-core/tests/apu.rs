use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode, WaveRamStartupPolicy};

fn step_until_next_div_apu_edge(machine: &mut Machine) {
    let starting_phase = machine.apu().snapshot().div_apu;

    for _ in 0..=0x2000 {
        machine.step_t_cycle();
        if machine.apu().snapshot().div_apu != starting_phase {
            return;
        }
    }

    panic!("expected the shared divider to reach the next APU frame-sequencer edge");
}

#[path = "apu/apu_div_timing.rs"]
mod apu_div_timing;
#[path = "apu/apu_mixer_output.rs"]
mod apu_mixer_output;
#[path = "apu/apu_power_readback.rs"]
mod apu_power_readback;
