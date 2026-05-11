//! Diagnostic: load a CH1 length-timer ROM, step the machine, and report each
//! NR52 / FS-step transition between the post-power-on NR52 write and the
//! eventual CH1 disable. Used to compare length-tick alignment against SameBoy
//! / DocBoy on the residual `ch1_length_timer_while_off_delay*` ROMs that we
//! cannot pass yet (SameBoy passes them — see commit log for analysis).
//!
//! Usage:
//!   cargo run --release -p gb-core --example trace_length_disable \
//!     -- <path-to-rom>
//!
//! Test ROMs flow: power-on -> APU off -> wait `delay` t-cycles -> APU on ->
//! trigger CH1 with length=63 -> spin until CH1 disables, counting iterations.
//! The trace prints the NR52 off/on transitions, the CH1 trigger, every
//! `div_apu` transition, and the final disable t-cycle so the alignment can be
//! diffed against an equivalent SameBoy trace.

use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode};
use std::path::PathBuf;

fn main() {
    let rom_path = std::env::args().nth(1).unwrap_or_else(|| {
        ".roms/test/docboy/apu/ch1_length_timer_while_off_delay512.gb".to_string()
    });
    let rom_bytes = std::fs::read(PathBuf::from(&rom_path)).expect("read rom");

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom_bytes).expect("load cartridge");

    // Skip until NR52 transitions on->off first (test signature: powered ROM
    // turns APU off, then on, then triggers CH1).
    let mut last_nr52: u8 = 0x80;
    let mut saw_off = false;
    let mut nr52_on_t: Option<u64> = None;
    let mut trigger_t: Option<u64> = None;
    let mut disable_t: Option<u64> = None;
    let mut last_div_apu = 0xFFu8;
    let mut last_bus_addr: Option<u16> = None;

    for _ in 0..3_000_000u64 {
        let t_before = machine.next_t_cycle().get();
        machine.step_t_cycle();
        let nr52 = machine.read_bus(0xFF26);
        let snapshot = machine.apu().snapshot();
        let div_apu = snapshot.div_apu;
        let sys_counter = machine.timer().snapshot().system_counter;

        // Detect the NR52 off transition before tracking anything.
        if !saw_off {
            if (last_nr52 & 0x80) != 0 && (nr52 & 0x80) == 0 {
                saw_off = true;
                eprintln!(
                    "[t={t_before}] NR52 power-OFF  div_apu={div_apu} sys_counter={sys_counter:#06x}"
                );
            }
            last_nr52 = nr52;
            continue;
        }

        // NR52 power-on transition (after we've seen the off)
        if (last_nr52 & 0x80) == 0 && (nr52 & 0x80) != 0 && nr52_on_t.is_none() {
            nr52_on_t = Some(t_before);
            eprintln!(
                "[t={t_before}] NR52 power-ON   div_apu={div_apu} sys_counter={sys_counter:#06x} bit12={}",
                (sys_counter & 0x1000) != 0,
            );
            last_div_apu = div_apu;
        }

        // NR14 trigger (CH1 active flag transitions 0->1)
        if (last_nr52 & 0x01) == 0 && (nr52 & 0x01) != 0 && trigger_t.is_none() {
            trigger_t = Some(t_before);
            eprintln!(
                "[t={t_before}{}] CH1 TRIGGER     div_apu={div_apu} sys_counter={sys_counter:#06x}",
                nr52_on_t
                    .map(|t| format!(" delta_on={}", t_before - t))
                    .unwrap_or_default(),
            );
            last_div_apu = div_apu;
        }

        // FS step transitions (only after power-on)
        if nr52_on_t.is_some() && div_apu != last_div_apu {
            let delta_on = nr52_on_t.map(|t| t_before - t).unwrap_or(0);
            let delta_trig = trigger_t.map(|t| t_before - t);
            eprintln!(
                "[t={t_before} delta_on={delta_on}{}] div_apu {}->{}",
                delta_trig
                    .map(|d| format!(" delta_trig={d}"))
                    .unwrap_or_default(),
                last_div_apu,
                div_apu
            );
            last_div_apu = div_apu;
        }

        // CPU bus activity on NR52 (FF26) reads — track addresses
        if let Some(activity) = machine.cpu().snapshot().last_bus_activity
            && Some(activity.address) != last_bus_addr
        {
            last_bus_addr = Some(activity.address);
        }

        // CH1 disable
        if (last_nr52 & 0x01) != 0 && (nr52 & 0x01) == 0 && disable_t.is_none() {
            disable_t = Some(t_before);
            let delta_trig = trigger_t.map(|t| t_before - t).unwrap_or(0);
            eprintln!(
                "[t={t_before} delta_trig={delta_trig}] CH1 DISABLE  div_apu={div_apu} sys_counter={sys_counter:#06x}"
            );
            // continue a bit more to capture aftermath, then stop
        }

        if let Some(d) = disable_t
            && t_before > d + 100
        {
            return;
        }

        last_nr52 = nr52;
    }

    eprintln!("[timeout] no CH1 disable observed in 3_000_000 t-cycles");
}
