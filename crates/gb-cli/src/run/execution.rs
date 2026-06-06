use crate::boot_rom::load_boot_rom_assets;
use crate::framebuffer::{
    encode_framebuffer_artifact_with_borrowed_sgb_border, sgb_framebuffer_artifact_for_output,
};
use crate::host_io::{
    resolve_path, validate_directory_input, write_bytes_with_parent, write_text_file_with_parent,
    writeln_checked,
};
use crate::options::{RunOptions, SavePolicy};
use crate::report::{
    compatibility_for_execution_mode, execution_mode_name, format_cartridge_load_error,
    format_framebuffer_artifact_error, startup_mode_name, write_cartridge_diagnostics,
};
use crate::run::budget::{default_run_limit_reached, run_limit_reached};
use crate::run::machine::CliMachine;
use crate::run::save_session::{flush_save_if_changed, open_save_session};
use crate::run::state::{restore_machine_save_state_from_path, write_machine_save_state_to_path};
use gb_benchmark::{
    BenchmarkStats, BenchmarkStimulusRuntime, GB_CLI_FRONTEND, encode_stats_toml,
    frontend_screenshot_path, frontend_stats_path,
};
use gb_core::{MachineConfig, extract_initial_sgb_borrowed_border};
use std::env;
use std::fs;
use std::io::Write;
use std::time::Instant;

pub(crate) fn run_command(
    options: RunOptions,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let rom_path = resolve_path(&current_dir, &options.rom_path);
    let rom_bytes = fs::read(&rom_path)
        .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;

    let compatibility = compatibility_for_execution_mode(options.execution_mode);
    let borrowed_sgb_border = if options.sgb_border.is_auto()
        && options
            .model
            .sgb_profile_for_standard(options.sgb_video_standard)
            .is_none()
    {
        extract_initial_sgb_borrowed_border(&rom_bytes, &compatibility)
    } else {
        None
    };
    let boot_rom_assets = load_boot_rom_assets(&options, &current_dir, stderr)?;
    let mut config = MachineConfig::new(options.model.console_model())
        .with_startup_mode(options.startup_mode)
        .with_revision(options.effective_revision())
        .with_compatibility(compatibility)
        .with_boot_rom_assets(boot_rom_assets);
    if let Some(profile) = options
        .model
        .sgb_profile_for_standard(options.sgb_video_standard)
    {
        config = config.with_sgb_profile(profile);
    }
    let mut machine = CliMachine::new(config, options.trace_out.is_some());
    let diagnostics = machine
        .load_cartridge(rom_bytes)
        .map_err(format_cartridge_load_error)?;
    write_cartridge_diagnostics(stderr, &diagnostics)?;

    let save_root = options
        .save_dir
        .as_ref()
        .map(|path| resolve_path(&current_dir, path));
    if let Some(save_root) = &save_root {
        validate_directory_input("--save-dir", save_root)?;
    }
    let state_in_path = options
        .state_in
        .as_ref()
        .map(|path| resolve_path(&current_dir, path));
    let state_out_path = options
        .state_out
        .as_ref()
        .map(|path| resolve_path(&current_dir, path));

    if let Some(state_in_path) = &state_in_path {
        restore_machine_save_state_from_path(&mut machine, state_in_path)?;
    }
    let mut save_session = open_save_session(
        save_root.as_deref(),
        &options,
        &rom_path,
        &mut machine,
        stderr,
        state_in_path.is_none(),
    )?;

    let frame_limit = options.frame_limit;
    let tcycle_limit = options.tcycle_limit;
    let default_run_budget = options.default_run_budget;
    let mut executed_tcycles = 0_u64;
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.at_frame_origin();
    let mut boot_rom_was_mapped = machine.is_boot_rom_mapped();
    let mut completed_frames_at_boot_handoff = None;
    let mut serial_byte_count = 0_usize;
    let mut serial_capture = options.serial_out.as_ref().map(|_| Vec::new());
    let mut benchmark_stimuli = options
        .benchmark_case
        .as_ref()
        .map(|case| BenchmarkStimulusRuntime::new(case.stimuli.clone()));
    let benchmark_started_at = options.benchmark_case.as_ref().map(|_| Instant::now());

    while !run_limit_reached(
        frame_limit,
        tcycle_limit,
        completed_frames,
        executed_tcycles,
    ) && !default_run_limit_reached(
        default_run_budget,
        completed_frames,
        completed_frames_at_boot_handoff,
    ) {
        if let Some(benchmark_stimuli) = &mut benchmark_stimuli {
            benchmark_stimuli.apply_due(
                executed_tcycles,
                u64::from(completed_frames),
                |button, pressed| {
                    machine.set_joypad_button_pressed(button, pressed);
                },
            );
        }
        machine.step_t_cycle();
        executed_tcycles += 1;

        let boot_rom_is_mapped = machine.is_boot_rom_mapped();
        if completed_frames_at_boot_handoff.is_none() && boot_rom_was_mapped && !boot_rom_is_mapped
        {
            completed_frames_at_boot_handoff = Some(completed_frames);
        }
        boot_rom_was_mapped = boot_rom_is_mapped;

        let serial_bytes = machine.take_serial_output_bytes();
        if !serial_bytes.is_empty() {
            serial_byte_count += serial_bytes.len();
            if options.serial_stdout {
                stdout
                    .write_all(&serial_bytes)
                    .map_err(|error| format!("failed to write serial stdout: {error}"))?;
                stdout
                    .flush()
                    .map_err(|error| format!("failed to flush serial stdout: {error}"))?;
            }
            if let Some(capture) = &mut serial_capture {
                capture.extend_from_slice(&serial_bytes);
            }
        }

        let now_at_frame_origin = machine.at_frame_origin();
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
            if matches!(options.save_policy, SavePolicy::OnWrite)
                && let Some(save_session) = &mut save_session
            {
                flush_save_if_changed(save_session, &machine, "frame-boundary")?;
            }
        }
        at_frame_origin = now_at_frame_origin;
    }
    let benchmark_elapsed = benchmark_started_at.map(|started_at| started_at.elapsed());

    if let Some(serial_out) = &options.serial_out {
        let serial_bytes = serial_capture.as_deref().unwrap_or_default();
        write_bytes_with_parent(serial_out, serial_bytes)?;
    }
    if let Some(framebuffer_out) = &options.framebuffer_out {
        let sgb_framebuffer_rgb555 =
            sgb_framebuffer_artifact_for_output(&machine, options.sgb_border);
        let sgb_framebuffer_rgb555 = sgb_framebuffer_rgb555
            .as_ref()
            .map(|(width, height, pixels)| (*width, *height, pixels.as_slice()));
        let framebuffer_image = encode_framebuffer_artifact_with_borrowed_sgb_border(
            framebuffer_out,
            machine.framebuffer(),
            sgb_framebuffer_rgb555,
            borrowed_sgb_border.as_ref(),
            machine.cgb_framebuffer_rgb555(),
            options.effective_display_palette(),
        )
        .map_err(|error| format_framebuffer_artifact_error(framebuffer_out, error))?;
        write_bytes_with_parent(framebuffer_out, &framebuffer_image)?;
    }
    if let Some(trace_out) = &options.trace_out {
        let Some(trace_text) = machine.trace_text() else {
            return Err("trace output requested without an in-memory trace buffer".to_string());
        };
        write_text_file_with_parent(trace_out, &trace_text)?;
    }
    if let Some(state_out_path) = &state_out_path {
        write_machine_save_state_to_path(&machine, state_out_path)?;
    }
    if let Some(benchmark_case) = options.benchmark_case.as_ref()
        && benchmark_case.stats
    {
        let stats_path = frontend_stats_path(GB_CLI_FRONTEND, &benchmark_case.artifact_id);
        let screenshot_path = benchmark_case
            .screenshot
            .then(|| frontend_screenshot_path(GB_CLI_FRONTEND, &benchmark_case.artifact_id));
        let stats = BenchmarkStats::new(
            GB_CLI_FRONTEND,
            benchmark_case,
            options.test_runner,
            u64::from(completed_frames),
            benchmark_elapsed.unwrap_or_default().as_secs_f64(),
            Some(executed_tcycles),
            screenshot_path.as_deref(),
        );
        let encoded_stats = encode_stats_toml(&stats)
            .map_err(|error| format!("failed to encode benchmark stats TOML: {error}"))?;
        write_text_file_with_parent(&stats_path, &encoded_stats)?;
        writeln_checked(
            stderr,
            &format!("benchmark_stats_out={}", stats_path.display()),
        )?;
    }

    if let Some(save_session) = &mut save_session {
        match options.save_policy {
            SavePolicy::Manual => {}
            SavePolicy::OnClose | SavePolicy::OnWrite => {
                flush_save_if_changed(save_session, &machine, "run-complete")?;
            }
        }
    }

    writeln_checked(stderr, &format!("rom={}", rom_path.display()))?;
    writeln_checked(stderr, &format!("model={}", options.model.name()))?;
    writeln_checked(
        stderr,
        &format!("startup={}", startup_mode_name(options.startup_mode)),
    )?;
    writeln_checked(
        stderr,
        &format!("mode={}", execution_mode_name(options.execution_mode)),
    )?;
    writeln_checked(stderr, &format!("executed_tcycles={executed_tcycles}"))?;
    writeln_checked(stderr, &format!("completed_frames={completed_frames}"))?;
    writeln_checked(stderr, &format!("serial_bytes={serial_byte_count}"))?;
    if let Some(framebuffer_out) = &options.framebuffer_out {
        writeln_checked(
            stderr,
            &format!("framebuffer_out={}", framebuffer_out.display()),
        )?;
    }
    if let Some(trace_out) = &options.trace_out {
        writeln_checked(stderr, &format!("trace_out={}", trace_out.display()))?;
    }
    if let Some(serial_out) = &options.serial_out {
        writeln_checked(stderr, &format!("serial_out={}", serial_out.display()))?;
    }
    if let Some(state_in_path) = &state_in_path {
        writeln_checked(stderr, &format!("state_in={}", state_in_path.display()))?;
    }
    if let Some(state_out_path) = &state_out_path {
        writeln_checked(stderr, &format!("state_out={}", state_out_path.display()))?;
    }
    if let Some(save_session) = &save_session {
        writeln_checked(stderr, &format!("save_key={}", save_session.key.as_str()))?;
        writeln_checked(
            stderr,
            &format!("save_file={}", save_session.save_path().display()),
        )?;
        writeln_checked(
            stderr,
            &format!("save_loaded_existing={}", save_session.loaded_existing_save),
        )?;
        writeln_checked(
            stderr,
            &format!("save_policy={}", options.save_policy.name()),
        )?;
        writeln_checked(stderr, &format!("save_writes={}", save_session.save_writes))?;
    }

    Ok(())
}
