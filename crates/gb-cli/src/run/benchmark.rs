use crate::command::parse::apply_test_runner_defaults;
use crate::framebuffer::RunDisplayPalette;
use crate::host_io::resolve_path;
use crate::options::{
    BenchmarkRunOptions, BootRomVerificationMode, RunModel, RunOptions, SavePolicy,
};
use crate::run::execution::run_command;
use gb_benchmark::{
    BenchmarkCase, BenchmarkMode, BenchmarkModel, BenchmarkPalette, BenchmarkStartup,
    GB_CLI_FRONTEND, frontend_screenshot_path, load_benchmark_cases, target_frames_for_duration,
    target_tcycles_for_duration,
};
use gb_core::{ExecutionMode, SgbVideoStandard, StartupMode};
use std::env;
use std::io::Write;

pub(crate) fn run_benchmark_command(
    options: BenchmarkRunOptions,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let benchmark_path = resolve_path(&current_dir, &options.benchmark_path);
    let benchmark_cases =
        load_benchmark_cases(&benchmark_path).map_err(|error| error.to_string())?;

    for benchmark_case in benchmark_cases {
        run_benchmark_case(benchmark_case, options.test_runner, stdout, stderr)?;
    }

    Ok(())
}

pub(crate) fn run_benchmark_case(
    benchmark_case: BenchmarkCase,
    test_runner: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let framebuffer_out = benchmark_case
        .screenshot
        .then(|| frontend_screenshot_path(GB_CLI_FRONTEND, &benchmark_case.artifact_id));
    let mut run_options = RunOptions {
        rom_path: benchmark_case.rom.clone(),
        model: run_model_from_benchmark(benchmark_case.model),
        revision: run_model_from_benchmark(benchmark_case.model)
            .console_model()
            .default_revision(),
        sgb_video_standard: SgbVideoStandard::default(),
        startup_mode: startup_mode_from_benchmark(benchmark_case.startup),
        execution_mode: execution_mode_from_benchmark(benchmark_case.mode),
        boot_rom_dir: None,
        boot_rom_verify: BootRomVerificationMode::Strict,
        frame_limit: Some(target_frames_for_duration(benchmark_case.duration_seconds)),
        tcycle_limit: Some(target_tcycles_for_duration(benchmark_case.duration_seconds)),
        default_run_budget: None,
        serial_stdout: false,
        serial_out: None,
        framebuffer_out,
        show_sgb_border: true,
        display_palette: benchmark_case.palette.map(display_palette_from_benchmark),
        trace_out: None,
        state_in: None,
        state_out: None,
        save_dir: None,
        save_key: None,
        save_policy: SavePolicy::Manual,
        test_runner,
        benchmark_case: Some(benchmark_case),
    };
    if run_options.test_runner {
        apply_test_runner_defaults(&mut run_options);
    }
    if run_options.model != RunModel::GameBoy {
        run_options.display_palette = None;
    }

    run_command(run_options, stdout, stderr)
}

pub(crate) fn run_model_from_benchmark(model: BenchmarkModel) -> RunModel {
    match model {
        BenchmarkModel::Dmg => RunModel::GameBoy,
        BenchmarkModel::Mgb => RunModel::Pocket,
        BenchmarkModel::Lgb => RunModel::Light,
        BenchmarkModel::Cgb => RunModel::Color,
        BenchmarkModel::Agb => RunModel::Advance,
    }
}

pub(crate) fn startup_mode_from_benchmark(startup: BenchmarkStartup) -> StartupMode {
    match startup {
        BenchmarkStartup::SkipBoot => StartupMode::SkipBoot,
        BenchmarkStartup::CustomBoot => StartupMode::CustomBoot,
        BenchmarkStartup::RealBoot => StartupMode::RealBoot,
    }
}

pub(crate) fn execution_mode_from_benchmark(mode: BenchmarkMode) -> ExecutionMode {
    match mode {
        BenchmarkMode::Strict => ExecutionMode::Strict,
        BenchmarkMode::Permissive => ExecutionMode::Permissive,
        BenchmarkMode::Experimental => ExecutionMode::Experimental,
    }
}

pub(crate) fn display_palette_from_benchmark(palette: BenchmarkPalette) -> RunDisplayPalette {
    match palette {
        BenchmarkPalette::Grey => RunDisplayPalette::Grey,
    }
}
