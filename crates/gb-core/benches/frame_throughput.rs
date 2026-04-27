use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use gb_core::{
    ConsoleModel, DMG_T_CYCLES_PER_SECOND, Machine, MachineConfig, StartupMode, TraceSummaryBuffer,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const NOMBC_ROM_SIZE: usize = 32 * 1024;
const DMG_FRAME_RATE_HZ: f64 = 59.727_500_569_605_83;
const WARMUP_FRAME_ORIGIN_CROSSINGS: usize = 4;
const MEASURED_FRAME_ORIGIN_CROSSINGS: usize = 24;

fn build_nom_bc_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(NOMBC_ROM_SIZE)];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
    rom
}

fn build_idle_loop_rom() -> Vec<u8> {
    build_nom_bc_rom(&[
        0xAF, // xor a
        0xE0,
        0x26, // ldh ($26), a ; NR52 off to measure core frame throughput without APU output churn
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

fn warm_machine_to_steady_frame(
    mut machine: Machine<TraceSummaryBuffer>,
) -> Machine<TraceSummaryBuffer> {
    machine.step_frame_origin_crossings(WARMUP_FRAME_ORIGIN_CROSSINGS);
    machine
}

fn emulated_speed_multiplier(frames: usize, elapsed: Duration) -> f64 {
    let frames_per_second = frames as f64 / elapsed.as_secs_f64().max(f64::EPSILON);
    frames_per_second / DMG_FRAME_RATE_HZ
}

fn benchmark_synthetic_frame_throughput(c: &mut Criterion) {
    let base_machine = warm_machine_to_steady_frame(load_machine(build_idle_loop_rom()));
    let mut group = c.benchmark_group("frame_throughput/core_only");
    group.throughput(Throughput::Elements(MEASURED_FRAME_ORIGIN_CROSSINGS as u64));
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(6));

    group.bench_function(
        BenchmarkId::from_parameter(format!(
            "idle_loop_{}frames_1x{DMG_T_CYCLES_PER_SECOND}tps",
            MEASURED_FRAME_ORIGIN_CROSSINGS
        )),
        |b| {
            b.iter_batched_ref(
                || base_machine.clone(),
                |machine| {
                    let started_at = std::time::Instant::now();
                    let result =
                        machine.step_frame_origin_crossings(MEASURED_FRAME_ORIGIN_CROSSINGS);
                    let speed_multiplier = emulated_speed_multiplier(
                        MEASURED_FRAME_ORIGIN_CROSSINGS,
                        started_at.elapsed(),
                    );
                    black_box((result, speed_multiplier, machine.next_t_cycle()));
                },
                BatchSize::LargeInput,
            );
        },
    );

    group.finish();
}

fn frame_throughput_benchmark_config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(6))
        .sample_size(20)
}

criterion_group! {
    name = frame_throughput_benches;
    config = frame_throughput_benchmark_config();
    targets = benchmark_synthetic_frame_throughput
}
criterion_main!(frame_throughput_benches);
