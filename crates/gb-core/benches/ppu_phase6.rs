use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use gb_core::{
    ConsoleModel, Machine, MachineConfig, PpuAccessMode, StartupMode, TraceSummaryBuffer,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const NOMBC_ROM_SIZE: usize = 32 * 1024;
const DOTS_PER_SCANLINE: usize = 456;
const TOTAL_SCANLINES: usize = 154;
const T_CYCLES_PER_FRAME: usize = DOTS_PER_SCANLINE * TOTAL_SCANLINES;
const SCANLINE_WINDOW_LINES: usize = 32;
const FRAME_WINDOW_FRAMES: usize = 4;
const BG_TILEMAP_BASE: u16 = 0x9800;
const TILE_DATA_BASE: u16 = 0x8000;
const OAM_BASE: u16 = 0xFE00;

struct PpuWindowBenchCase {
    name: &'static str,
    machine: Machine<TraceSummaryBuffer>,
    measured_t_cycles: usize,
    throughput_elements: u64,
}

fn build_nom_bc_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(NOMBC_ROM_SIZE)];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
    rom
}

fn build_idle_rom() -> Vec<u8> {
    build_nom_bc_rom(&[
        0xAF, // xor a
        0xE0, 0x26, // ldh ($26), a ; NR52 off
        0x00, // loop: nop
        0x18, 0xFD, // jr loop
    ])
}

fn load_machine(rom: Vec<u8>) -> Machine<TraceSummaryBuffer> {
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("benchmark ROM should load without diagnostics");
    machine
}

fn run_t_cycles(machine: &mut Machine<TraceSummaryBuffer>, t_cycles: usize) {
    for _ in 0..t_cycles {
        machine.step_t_cycle();
    }
}

fn write_bg_tile_row(
    machine: &mut Machine<TraceSummaryBuffer>,
    tile_index: u8,
    row: u8,
    low: u8,
    high: u8,
) {
    let tile_row_address = TILE_DATA_BASE + tile_index as u16 * 16 + row as u16 * 2;
    machine.write_bus(tile_row_address, low);
    machine.write_bus(tile_row_address + 1, high);
}

fn fill_tile(machine: &mut Machine<TraceSummaryBuffer>, tile_index: u8, low: u8, high: u8) {
    for row in 0..8 {
        write_bg_tile_row(machine, tile_index, row, low, high);
    }
}

fn fill_bg_tilemap(machine: &mut Machine<TraceSummaryBuffer>, tile_index: u8) {
    for y in 0..32 {
        for x in 0..32 {
            machine.write_bus(BG_TILEMAP_BASE + y * 32 + x, tile_index);
        }
    }
}

fn write_oam_entry(
    machine: &mut Machine<TraceSummaryBuffer>,
    index: u8,
    y: u8,
    x: u8,
    tile_index: u8,
    attributes: u8,
) {
    let entry_base = OAM_BASE + index as u16 * 4;
    machine.write_bus(entry_base, y);
    machine.write_bus(entry_base + 1, x);
    machine.write_bus(entry_base + 2, tile_index);
    machine.write_bus(entry_base + 3, attributes);
}

fn configure_common_scene(machine: &mut Machine<TraceSummaryBuffer>, lcdc: u8, scx: u8) {
    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0xFF42, 0x00);
    machine.write_bus(0xFF43, scx);
    machine.write_bus(0xFF47, 0xE4);
    machine.write_bus(0xFF48, 0xE4);
    machine.write_bus(0xFF49, 0xE4);
    machine.write_bus(0xFF4A, 0x00);
    machine.write_bus(0xFF4B, 0x07);

    // Tile 1: opaque color-1 background to keep BG fetch/push active and make OBJ mixing meaningful.
    fill_tile(machine, 1, 0xFF, 0x00);
    fill_bg_tilemap(machine, 1);

    machine.write_bus(0xFF40, lcdc);
}

fn configure_bg_fetch_push_scene(machine: &mut Machine<TraceSummaryBuffer>) {
    // LCDC: LCD on, BG on, unsigned tile data, OBJ off.
    configure_common_scene(machine, 0x91, 0x03);
}

fn configure_obj_fetch_push_mix_scene(machine: &mut Machine<TraceSummaryBuffer>) {
    // LCDC: LCD on, BG on, unsigned tile data, OBJ on.
    configure_common_scene(machine, 0x93, 0x03);

    // Tile 2: opaque color-3 OBJ pixels.
    fill_tile(machine, 2, 0xFF, 0xFF);

    // Four consecutive 8-line bands, each with 10 sprites arranged as five overlapping pairs.
    // This gives a stable 32-scanline window that stresses OBJ fetch, FIFO push/rewrite, and BG/OBJ mixing.
    let pair_screen_x = [8_u8, 12, 40, 44, 72, 76, 104, 108, 136, 140];
    for band in 0..4 {
        let sprite_y = 16 + band * 8;
        for (slot, screen_x) in pair_screen_x.iter().copied().enumerate() {
            let oam_index = band * 10 + slot as u8;
            let attributes = match slot % 4 {
                0 => 0x00,
                1 => 0x80,
                2 => 0x10,
                _ => 0x90,
            };
            write_oam_entry(
                machine,
                oam_index,
                sprite_y,
                screen_x.saturating_add(8),
                2,
                attributes,
            );
        }
    }
}

fn advance_until_frame_start(machine: &mut Machine<TraceSummaryBuffer>) {
    for _ in 0..(T_CYCLES_PER_FRAME * 2) {
        run_t_cycles(machine, 1);
        let snapshot = machine.ppu().snapshot();
        if snapshot.ly == 0 && snapshot.line_dot == 0 {
            return;
        }
    }

    panic!("benchmark scene did not reach frame start within expected cycles");
}

fn prepare_steady_frame_start(machine: &mut Machine<TraceSummaryBuffer>) {
    // First frame after LCD enable includes startup-specific behavior; the second frame start is our steady baseline.
    advance_until_frame_start(machine);
    advance_until_frame_start(machine);
}

fn run_mode3_windows(machine: &mut Machine<TraceSummaryBuffer>, window_count: usize) {
    for _ in 0..window_count {
        while machine.ppu().bus_state().mode() != PpuAccessMode::Drawing {
            machine.step_t_cycle();
        }
        while machine.ppu().bus_state().mode() == PpuAccessMode::Drawing {
            machine.step_t_cycle();
        }
    }
}

fn build_bg_fetch_push_machine() -> Machine<TraceSummaryBuffer> {
    let mut machine = load_machine(build_idle_rom());
    configure_bg_fetch_push_scene(&mut machine);
    prepare_steady_frame_start(&mut machine);
    machine
}

fn build_obj_fetch_push_mix_machine() -> Machine<TraceSummaryBuffer> {
    let mut machine = load_machine(build_idle_rom());
    configure_obj_fetch_push_mix_scene(&mut machine);
    prepare_steady_frame_start(&mut machine);
    machine
}

fn bench_cloned_window(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    case: &PpuWindowBenchCase,
) {
    group.throughput(Throughput::Elements(case.throughput_elements));
    group.bench_function(BenchmarkId::from_parameter(case.name), |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iters {
                let mut machine = case.machine.clone();
                let start = Instant::now();
                run_t_cycles(&mut machine, case.measured_t_cycles);
                elapsed += start.elapsed();
                black_box(machine.next_t_cycle());
            }
            elapsed
        });
    });
}

fn benchmark_scanline_cost(c: &mut Criterion) {
    let cases = [
        PpuWindowBenchCase {
            name: "bg_fetch_push_scx3",
            machine: build_bg_fetch_push_machine(),
            measured_t_cycles: SCANLINE_WINDOW_LINES * DOTS_PER_SCANLINE,
            throughput_elements: SCANLINE_WINDOW_LINES as u64,
        },
        PpuWindowBenchCase {
            name: "obj_fetch_push_mix_overlap_scx3",
            machine: build_obj_fetch_push_mix_machine(),
            measured_t_cycles: SCANLINE_WINDOW_LINES * DOTS_PER_SCANLINE,
            throughput_elements: SCANLINE_WINDOW_LINES as u64,
        },
    ];

    let mut group = c.benchmark_group("ppu_phase6_strict/scanline_cost");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));

    for case in &cases {
        bench_cloned_window(&mut group, case);
    }

    group.finish();
}

fn benchmark_frame_cost(c: &mut Criterion) {
    let cases = [
        PpuWindowBenchCase {
            name: "bg_fetch_push_scx3",
            machine: build_bg_fetch_push_machine(),
            measured_t_cycles: FRAME_WINDOW_FRAMES * T_CYCLES_PER_FRAME,
            throughput_elements: FRAME_WINDOW_FRAMES as u64,
        },
        PpuWindowBenchCase {
            name: "obj_fetch_push_mix_overlap_scx3",
            machine: build_obj_fetch_push_mix_machine(),
            measured_t_cycles: FRAME_WINDOW_FRAMES * T_CYCLES_PER_FRAME,
            throughput_elements: FRAME_WINDOW_FRAMES as u64,
        },
    ];

    let mut group = c.benchmark_group("ppu_phase6_strict/frame_cost");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));

    for case in &cases {
        bench_cloned_window(&mut group, case);
    }

    group.finish();
}

fn benchmark_mode3_hotspot_windows(c: &mut Criterion) {
    let cases = [
        PpuWindowBenchCase {
            name: "bg_fetch_push_scx3",
            machine: build_bg_fetch_push_machine(),
            measured_t_cycles: SCANLINE_WINDOW_LINES * DOTS_PER_SCANLINE,
            throughput_elements: SCANLINE_WINDOW_LINES as u64,
        },
        PpuWindowBenchCase {
            name: "obj_fetch_push_mix_overlap_scx3",
            machine: build_obj_fetch_push_mix_machine(),
            measured_t_cycles: SCANLINE_WINDOW_LINES * DOTS_PER_SCANLINE,
            throughput_elements: SCANLINE_WINDOW_LINES as u64,
        },
    ];

    let mut group = c.benchmark_group("ppu_phase6_strict/mode3_hotspot_windows");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));

    for case in &cases {
        group.throughput(Throughput::Elements(case.throughput_elements));
        group.bench_function(BenchmarkId::from_parameter(case.name), |b| {
            b.iter_custom(|iters| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iters {
                    let mut machine = case.machine.clone();
                    let start = Instant::now();
                    run_mode3_windows(&mut machine, SCANLINE_WINDOW_LINES);
                    elapsed += start.elapsed();
                    black_box(machine.next_t_cycle());
                }
                elapsed
            });
        });
    }

    group.finish();
}

fn phase6_benchmark_config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .sample_size(10)
}

criterion_group! {
    name = ppu_phase6_benches;
    config = phase6_benchmark_config();
    targets = benchmark_scanline_cost, benchmark_frame_cost, benchmark_mode3_hotspot_windows
}
criterion_main!(ppu_phase6_benches);
