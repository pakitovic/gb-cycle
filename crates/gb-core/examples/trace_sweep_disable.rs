//! Diagnostic: load a CH1 sweep ROM, step the machine, and report the t-cycle
//! when CH1 transitions enabled -> disabled, plus every DIV-APU step transition
//! between trigger and disable. Used to pinpoint timing divergences from
//! DocBoy on round1/round2 boundary tests.
//!
//! Usage:
//!   cargo run --release -p gb-core --example trace_sweep_disable \
//!     -- <path-to-rom>

use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode};
use std::path::PathBuf;

fn main() {
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".roms/test/docboy/apu/ch1_period_sweep_step0_round2.gb".to_string());
    let rom_bytes = std::fs::read(PathBuf::from(&rom_path)).expect("read rom");

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom_bytes).expect("load cartridge");

    let mut last_nr52: u8 = 0xFF;
    let mut trigger_t: Option<u64> = None;
    let mut last_div_apu = 0xFFu8;

    for _ in 0..2_000_000u64 {
        let t_before = machine.next_t_cycle().get();
        machine.step_t_cycle();
        let nr52 = machine.read_bus(0xFF26);
        let div_apu = machine.apu().snapshot().div_apu;

        if last_nr52 == 0xFF {
            last_nr52 = nr52;
        }

        if (last_nr52 & 0x01) == 0 && (nr52 & 0x01) != 0 && trigger_t.is_none() {
            trigger_t = Some(t_before);
            let sys_counter = machine.timer().snapshot().system_counter;
            eprintln!(
                "[t={}] ch1 enabled (trigger), div_apu={}, sys_counter={:#x}",
                t_before, div_apu, sys_counter
            );
            last_div_apu = div_apu;
        }

        if let Some(trigger) = trigger_t
            && div_apu != last_div_apu
        {
            let delta = t_before - trigger;
            eprintln!(
                "[t={} delta={}] div_apu {}->{}",
                t_before, delta, last_div_apu, div_apu
            );
            last_div_apu = div_apu;
        }

        if let Some(trigger) = trigger_t
            && (last_nr52 & 0x01) != 0
            && (nr52 & 0x01) == 0
        {
            let delta = t_before - trigger;
            eprintln!("[t={} delta={}] ch1 disabled", t_before, delta);
            return;
        }
        last_nr52 = nr52;
    }

    eprintln!("ch1 never disabled within 2M t-cycles");
}
