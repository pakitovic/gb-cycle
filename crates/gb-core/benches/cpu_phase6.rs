use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use gb_core::{Machine, MachineConfig, TraceSummaryBuffer};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const NOMBC_ROM_SIZE: usize = 32 * 1024;
const STEADY_STATE_WARMUP_T_CYCLES: usize = 4_096;
const STEADY_STATE_MEASURED_T_CYCLES: usize = 262_144;
const FIXTURE_WINDOW_T_CYCLES: usize = 131_072;

struct SteadyStateBenchCase {
    name: &'static str,
    machine: Machine<TraceSummaryBuffer>,
}

struct FixtureWindowBenchCase {
    name: &'static str,
    machine: Machine<TraceSummaryBuffer>,
    measured_t_cycles: usize,
}

fn build_nom_bc_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(NOMBC_ROM_SIZE)];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
    rom
}

fn build_register_alu_loop_rom() -> Vec<u8> {
    build_nom_bc_rom(&[
        0x06, 0x12, // ld b, $12
        0x0E, 0x34, // ld c, $34
        0xAF, // xor a
        0x04, // loop: inc b
        0x0D, // dec c
        0x80, // add a, b
        0x89, // adc a, c
        0xA8, // xor b
        0xB1, // or c
        0x18, 0xF8, // jr loop
    ])
}

fn build_hl_memory_cb_loop_rom() -> Vec<u8> {
    build_nom_bc_rom(&[
        0x21, 0x00, 0xC0, // ld hl, $C000
        0x36, 0x5A, // ld (hl), $5A
        0x34, // loop: inc (hl)
        0x35, // dec (hl)
        0xCB, 0x46, // bit 0, (hl)
        0xCB, 0x86, // res 0, (hl)
        0xCB, 0xC6, // set 0, (hl)
        0x7E, // ld a, (hl)
        0x77, // ld (hl), a
        0x18, 0xF4, // jr loop
    ])
}

fn build_stack_call_ret_loop_rom() -> Vec<u8> {
    build_nom_bc_rom(&[
        0x31, 0x00, 0xD0, // ld sp, $D000
        0x01, 0x34, 0x12, // ld bc, $1234
        0x11, 0x78, 0x56, // ld de, $5678
        0xCD, 0x0E, 0x01, // loop: call subroutine
        0x18, 0xFB, // jr loop
        0xC5, // subroutine: push bc
        0xD5, // push de
        0xD1, // pop de
        0xC1, // pop bc
        0xC9, // ret
    ])
}

fn fixture_rom(path: &str) -> Vec<u8> {
    std::fs::read(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("failed to read ROM fixture {path}: {error}"))
}

fn load_machine(rom: Vec<u8>) -> Machine<TraceSummaryBuffer> {
    let mut machine = Machine::new_summary(MachineConfig::default());
    machine
        .load_cartridge(rom)
        .expect("benchmark ROM should load without cartridge diagnostics");
    machine
}

fn run_t_cycles(machine: &mut Machine<TraceSummaryBuffer>, t_cycles: usize) {
    for _ in 0..t_cycles {
        machine.step_t_cycle();
    }
}

fn benchmark_steady_state_cpu_loops(c: &mut Criterion) {
    let cases = [
        SteadyStateBenchCase {
            name: "register_alu_loop",
            machine: load_machine(build_register_alu_loop_rom()),
        },
        SteadyStateBenchCase {
            name: "hl_memory_cb_loop",
            machine: load_machine(build_hl_memory_cb_loop_rom()),
        },
        SteadyStateBenchCase {
            name: "stack_call_ret_loop",
            machine: load_machine(build_stack_call_ret_loop_rom()),
        },
    ];

    let mut group = c.benchmark_group("cpu_phase6/steady_state");
    group.throughput(Throughput::Elements(STEADY_STATE_MEASURED_T_CYCLES as u64));

    for case in cases {
        let mut machine = case.machine;
        run_t_cycles(&mut machine, STEADY_STATE_WARMUP_T_CYCLES);

        group.bench_function(BenchmarkId::from_parameter(case.name), |b| {
            b.iter(|| {
                run_t_cycles(&mut machine, STEADY_STATE_MEASURED_T_CYCLES);
                black_box(machine.next_t_cycle());
            });
        });
    }

    group.finish();
}

fn benchmark_fixture_windows(c: &mut Criterion) {
    let cases = [
        FixtureWindowBenchCase {
            name: "phase2_control_flow_stack_cb",
            machine: load_machine(fixture_rom(
                "tests/fixtures/roms/phase2/phase2_control_flow_stack_cb.gb",
            )),
            measured_t_cycles: FIXTURE_WINDOW_T_CYCLES,
        },
        FixtureWindowBenchCase {
            name: "phase2_ei_delay_priority",
            machine: load_machine(fixture_rom(
                "tests/fixtures/roms/phase2/phase2_ei_delay_priority.gb",
            )),
            measured_t_cycles: FIXTURE_WINDOW_T_CYCLES,
        },
    ];

    let mut group = c.benchmark_group("cpu_phase6/fixture_windows");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(6));

    for case in cases {
        group.throughput(Throughput::Elements(case.measured_t_cycles as u64));
        group.bench_with_input(BenchmarkId::from_parameter(case.name), &case, |b, case| {
            b.iter_batched_ref(
                || case.machine.clone(),
                |machine| {
                    run_t_cycles(machine, case.measured_t_cycles);
                    black_box(machine.next_t_cycle());
                },
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

fn phase6_benchmark_config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .sample_size(20)
}

criterion_group! {
    name = cpu_phase6_benches;
    config = phase6_benchmark_config();
    targets = benchmark_steady_state_cpu_loops, benchmark_fixture_windows
}
criterion_main!(cpu_phase6_benches);
