//! Per-dot BG fetcher schedule trace for a single visible line, to diff
//! gb-cycle against the DocBoy/SameBoy hardware-true fetch schedule
//! (PPU hardening campaign, docs/roadmap/04-ppu-fix.md M0/M1 probe).
//!
//! Usage:
//!   cargo run --release -p gb-core --example ppu_fetch_trace -- <rom> [ly] [frame]
//!
//! ly defaults to 0, frame (1-based occurrence of that ly while the LCD is
//! enabled) defaults to 3 so the blank first frame after LCD enable is skipped.

use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode};

const MAX_T_CYCLES: u64 = 80 * 70224;

fn main() {
    let mut args = std::env::args().skip(1);
    let rom_path = args
        .next()
        .expect("usage: ppu_fetch_trace <rom> [ly] [frame]");
    let target_ly: u8 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let target_frame: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(3);

    let rom_bytes = std::fs::read(&rom_path).expect("read rom");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom_bytes).expect("load cartridge");

    let mut prev_ly = 0xFFu8;
    let mut occurrence = 0u32;
    let mut printed_first_visible = false;

    for _ in 0..MAX_T_CYCLES {
        machine.step_t_cycle();

        let ppu = machine.ppu();
        let ly = ppu.ly();
        if !ppu.lcd_state().is_enabled() {
            prev_ly = ly;
            continue;
        }

        if ly == target_ly && prev_ly != target_ly {
            occurrence += 1;
            if occurrence == target_frame {
                printed_first_visible = false;
                println!("=== ly={target_ly} occurrence={occurrence} ===");
            }
        }

        if ly == target_ly && occurrence == target_frame {
            let s = ppu.snapshot();
            let line_dot = ppu.line_dot();
            if (78..=176).contains(&line_dot) {
                println!(
                    "dot={line_dot:>3} stage={:?}/{} tidx={:#04X} tx={} vpo={} fifo={} m0={}",
                    s.bg_fetcher_stage,
                    s.bg_fetcher_stage_dot,
                    s.bg_fetcher_tile_index,
                    s.bg_current_transfer_x,
                    s.visible_pixels_output,
                    s.bg_fifo_pixels.len(),
                    s.mode0_start_dot,
                );
            }
            if !printed_first_visible && s.visible_pixels_output >= 1 {
                printed_first_visible = true;
                println!("FIRST_VISIBLE dot={line_dot} m0={}", s.mode0_start_dot);
            }
            if line_dot > 176 {
                break;
            }
        }

        prev_ly = ly;
    }
}
