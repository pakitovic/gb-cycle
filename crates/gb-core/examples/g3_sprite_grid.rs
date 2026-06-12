//! Per-testcase sprite mode3-penalty grid for wilbertpol
//! intr_2_mode0_timing_sprites*_nops ROMs.
//!
//! Usage:
//!   cargo run --release -p gb-core --example g3_sprite_grid -- <path-to-rom> [--trace]

use gb_core::{
    ConsoleModel, CpuExecutionState, Machine, MachineConfig, PpuAccessMode, StartupMode,
};
use std::collections::BTreeMap;

const TESTCASE_ID_ADDR: u16 = 0xC000;
const MEASURED_LY: u8 = 0x44;
const MODE3_BASELINE_FLIP_DOT: u16 = 252;
const PASS_WINDOW_DOTS: u16 = 3;
const MAX_T_CYCLES: u64 = 700 * 70224;
const TRACE_DOT_LOW: u16 = 200;
const TRACE_DOT_HIGH: u16 = 320;
const TRACE_MAX_VISITS: u32 = 24;

#[derive(Debug, Clone)]
struct TestcaseSpec {
    extra_cycles: u8,
    sprite_xs: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct Measurement {
    testcase: u8,
    internal_flip_dot: Option<u16>,
    published_flip_dot: Option<u16>,
    mode0_start_dot: u16,
    obj_enabled: bool,
    scx: u8,
}

fn parse_testcase_table(rom: &[u8]) -> BTreeMap<u8, TestcaseSpec> {
    let mut table = BTreeMap::new();
    for site in 0..rom.len().saturating_sub(10) {
        if rom[site..site + 3] != [0xEA, 0x00, 0xC0] {
            continue;
        }
        if site < 2 || rom[site - 2] != 0x3E || rom[site + 3] != 0x21 || rom[site + 6] != 0x16 {
            continue;
        }
        let testcase = rom[site - 1];
        let data_addr = usize::from(rom[site + 4]) | usize::from(rom[site + 5]) << 8;
        let extra_cycles = rom[site + 7].wrapping_sub(41);
        let count = usize::from(rom[data_addr]);
        let sprite_xs = rom[data_addr + 1..data_addr + 1 + count].to_vec();
        table.insert(
            testcase,
            TestcaseSpec {
                extra_cycles,
                sprite_xs,
            },
        );
    }
    table
}

fn patch_fail_handlers(rom: &mut [u8]) -> Vec<(u16, char)> {
    let mut handlers = Vec::new();
    for site in 0..rom.len().saturating_sub(6) {
        let round = match rom[site..site + 4] {
            [0x0E, 0x01, 0xB9, 0xC2] => 'A',
            [0x0E, 0x02, 0xB9, 0xC2] => 'B',
            _ => continue,
        };
        let target = u16::from(rom[site + 4]) | u16::from(rom[site + 5]) << 8;
        rom[usize::from(target)] = 0xC9;
        handlers.push((target, round));
    }
    handlers
}

fn main() {
    let mut args = std::env::args().skip(1);
    let rom_path = args.next().expect("usage: g3_sprite_grid <rom> [--trace]");
    let trace = args.next().as_deref() == Some("--trace");

    let mut rom_bytes = std::fs::read(&rom_path).expect("read rom");
    let table = parse_testcase_table(&rom_bytes);
    let handlers = patch_fail_handlers(&mut rom_bytes);
    eprintln!(
        "{}: {} testcases, fail handlers {:?}",
        rom_path,
        table.len(),
        handlers
            .iter()
            .map(|(addr, round)| format!("{round}@{addr:#06X}"))
            .collect::<Vec<_>>()
    );

    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom_bytes).expect("load cartridge");

    let mut measurements: Vec<Measurement> = Vec::new();
    let mut real_fails: Vec<(u8, char)> = Vec::new();
    let mut pending: Option<Measurement> = None;
    let mut prev_ly = 0xFFu8;
    let mut prev_internal = PpuAccessMode::HBlank;
    let mut prev_published = PpuAccessMode::HBlank;
    let mut prev_pc = 0u16;
    let mut trace_visits = 0u32;
    let mut trap: Option<(u16, u8)> = None;

    for _ in 0..MAX_T_CYCLES {
        machine.step_t_cycle();

        let (ly, line_dot, internal, published, mode0_start_dot, lcd_enabled) = {
            let ppu = machine.ppu();
            (
                ppu.ly(),
                ppu.line_dot(),
                ppu.access_mode(),
                ppu.cpu_visible_stat_mode(),
                ppu.mode0_start_dot(),
                ppu.lcd_state().is_enabled(),
            )
        };

        if ly == MEASURED_LY && lcd_enabled {
            if prev_ly != MEASURED_LY {
                let testcase = machine.read_bus(TESTCASE_ID_ADDR);
                let lcdc = machine.read_bus(0xFF40);
                let scx = machine.read_bus(0xFF43);
                pending = Some(Measurement {
                    testcase,
                    internal_flip_dot: None,
                    published_flip_dot: None,
                    mode0_start_dot,
                    obj_enabled: lcdc & 0x02 != 0,
                    scx,
                });
                if trace {
                    trace_visits += 1;
                }
            }
            if let Some(measurement) = pending.as_mut() {
                if prev_ly == MEASURED_LY
                    && prev_internal == PpuAccessMode::Drawing
                    && internal == PpuAccessMode::HBlank
                {
                    measurement.internal_flip_dot = Some(line_dot);
                    measurement.mode0_start_dot = mode0_start_dot;
                }
                if prev_ly == MEASURED_LY
                    && prev_published == PpuAccessMode::Drawing
                    && published == PpuAccessMode::HBlank
                {
                    measurement.published_flip_dot = Some(line_dot);
                }
            }
            if trace
                && trace_visits <= TRACE_MAX_VISITS
                && (TRACE_DOT_LOW..=TRACE_DOT_HIGH).contains(&line_dot)
                && (prev_internal != internal || prev_published != published)
            {
                println!(
                    "trace visit={trace_visits} dot={line_dot} internal={internal:?} published={published:?} m0={mode0_start_dot}"
                );
            }
            if trace
                && trace_visits <= TRACE_MAX_VISITS
                && let Some(activity) = machine.cpu().snapshot().last_bus_activity
                && activity.address == 0xFF41
            {
                println!(
                    "trace visit={trace_visits} dot={line_dot} cpu_ff41 {:?} value={:#04X}",
                    activity.kind, activity.value
                );
            }
        } else if prev_ly == MEASURED_LY
            && let Some(measurement) = pending.take()
        {
            measurements.push(measurement);
        }

        let cpu = machine.cpu().snapshot();
        let pc = cpu.registers.pc;
        if pc != prev_pc
            && cpu.current_opcode == Some(0xC9)
            && let Some((_, round)) = handlers.iter().find(|(addr, _)| addr.wrapping_add(1) == pc)
        {
            let testcase = machine.read_bus(TESTCASE_ID_ADDR);
            if trace && trace_visits <= TRACE_MAX_VISITS {
                println!(
                    "trace visit={trace_visits} ly={ly} dot={line_dot} fail_handler round={round} tc={testcase:#04X} b={}",
                    cpu.registers.b
                );
            }
            real_fails.push((testcase, *round));
        }
        prev_pc = pc;

        if let CpuExecutionState::DiagnosticTrap { trap: trap_info } =
            machine.cpu().snapshot().execution_state
        {
            let gb_core::CpuDiagnosticTrap::InvalidOpcode { opcode, address } = trap_info;
            trap = Some((address, opcode));
            break;
        }

        prev_ly = ly;
        prev_internal = internal;
        prev_published = published;
    }

    report(&table, &measurements, &real_fails, trap);
}

fn report(
    table: &BTreeMap<u8, TestcaseSpec>,
    measurements: &[Measurement],
    real_fails: &[(u8, char)],
    trap: Option<(u16, u8)>,
) {
    let mut grouped: BTreeMap<u8, Vec<&Measurement>> = BTreeMap::new();
    for measurement in measurements.iter().filter(|m| m.obj_enabled) {
        grouped
            .entry(measurement.testcase)
            .or_default()
            .push(measurement);
    }

    let mut out_of_window = 0u32;
    let mut jitter_cases = 0u32;
    for (testcase, rounds) in &grouped {
        let Some(spec) = table.get(testcase) else {
            continue;
        };
        let scx = rounds.first().map_or(0, |m| m.scx & 0x07);
        let window_low =
            MODE3_BASELINE_FLIP_DOT + u16::from(scx) + 4 * u16::from(spec.extra_cycles);
        let window_high = window_low + PASS_WINDOW_DOTS;
        let flips: Vec<String> = rounds
            .iter()
            .map(|m| {
                m.internal_flip_dot
                    .map_or("-".to_string(), |dot| dot.to_string())
            })
            .collect();
        let published: Vec<String> = rounds
            .iter()
            .map(|m| {
                m.published_flip_dot
                    .map_or("-".to_string(), |dot| dot.to_string())
            })
            .collect();
        let distinct: std::collections::BTreeSet<_> =
            rounds.iter().filter_map(|m| m.internal_flip_dot).collect();
        let jitter = distinct.len() > 1;
        let in_window = rounds.iter().all(|m| {
            m.internal_flip_dot
                .is_some_and(|dot| (window_low..=window_high).contains(&dot))
        });
        if !in_window {
            out_of_window += 1;
        }
        if jitter {
            jitter_cases += 1;
        }
        let fails: Vec<String> = real_fails
            .iter()
            .filter(|(id, _)| id == testcase)
            .map(|(_, round)| round.to_string())
            .collect();
        println!(
            "tc={testcase:#04X} e={:>2} xs={:?} window=[{window_low},{window_high}] m0={flips:?} pub={published:?}{}{}{}",
            spec.extra_cycles,
            spec.sprite_xs,
            if in_window { "" } else { " OUT_OF_WINDOW" },
            if jitter { " JITTER" } else { "" },
            if fails.is_empty() {
                String::new()
            } else {
                format!(" REAL_FAIL[{}]", fails.join(","))
            },
        );
    }

    println!(
        "summary cases={} measured={} real_fails={} out_of_window={out_of_window} jitter_cases={jitter_cases} trap={:?}",
        table.len(),
        grouped.len(),
        real_fails.len(),
        trap.map(|(address, opcode)| format!("{opcode:#04X}@{address:#06X}")),
    );
}
