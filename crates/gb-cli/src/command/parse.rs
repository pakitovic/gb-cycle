use super::CliAction;
use crate::framebuffer::RunDisplayPalette;
use crate::options::{
    BenchmarkRunOptions, BootRomVerificationMode, DefaultRunBudget, InspectRomOptions, RunModel,
    RunOptions, SavePolicy, SavesDirection, SavesOptions, SgbBorderPresentationMode,
};
use crate::report::{revision_argument_name, supported_revision_names_on_host};
use gb_core::{ExecutionMode, HardwareRevision, SgbVideoStandard, StartupMode};
use std::path::PathBuf;

pub(crate) fn parse_cli_arguments<I, S>(arguments: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let Some(command) = arguments.next() else {
        return Ok(CliAction::ShowGeneralHelp);
    };

    match command.as_ref() {
        "--help" | "-h" => Ok(CliAction::ShowGeneralHelp),
        "run" => parse_run_arguments(arguments),
        "inspect-rom" => parse_inspect_rom_arguments(arguments),
        "saves" => parse_saves_arguments(arguments),
        other => Err(format!(
            "unknown subcommand {other:?}; run `gb-cli --help` for usage"
        )),
    }
}

pub(crate) fn parse_run_arguments<I, S>(arguments: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let mut rom_path = None;
    let mut options = None;
    let mut benchmark_path = None;
    let mut test_runner_requested = false;
    let mut save_policy_explicit = false;
    let mut revision_explicit = false;
    let mut sgb_video_standard_explicit = false;

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(CliAction::ShowRunHelp),
            "--model" => {
                let Some(value) = arguments.next() else {
                    return Err("--model requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                let options = options.as_mut().unwrap();
                options.model = parse_run_model(value.as_ref())?;
                if !revision_explicit {
                    options.revision = options.model.console_model().default_revision();
                }
            }
            "--revision" => {
                let Some(value) = arguments.next() else {
                    return Err("--revision requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().revision = parse_revision(value.as_ref())?;
                revision_explicit = true;
            }
            "--sgb-standard" => {
                let Some(value) = arguments.next() else {
                    return Err("--sgb-standard requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().sgb_video_standard =
                    parse_sgb_video_standard(value.as_ref())?;
                sgb_video_standard_explicit = true;
            }
            "--startup" => {
                let Some(value) = arguments.next() else {
                    return Err("--startup requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().startup_mode = parse_startup_mode(value.as_ref())?;
            }
            "--mode" => {
                let Some(value) = arguments.next() else {
                    return Err("--mode requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().execution_mode = parse_execution_mode(value.as_ref())?;
            }
            "--boot-rom-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--boot-rom-dir requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().boot_rom_dir = Some(PathBuf::from(value.as_ref()));
            }
            "--boot-rom-verify" => {
                let Some(value) = arguments.next() else {
                    return Err("--boot-rom-verify requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().boot_rom_verify =
                    parse_boot_rom_verification_mode(value.as_ref())?;
            }
            "--test-runner" => {
                test_runner_requested = true;
                if let Some(options) = &mut options {
                    options.test_runner = true;
                }
            }
            "--benchmark" => {
                let Some(value) = arguments.next() else {
                    return Err("--benchmark requires a value".to_string());
                };
                if benchmark_path.is_some() {
                    return Err("--benchmark can only be provided once".to_string());
                }
                benchmark_path = Some(PathBuf::from(value.as_ref()));
            }
            "--frames" => {
                let Some(value) = arguments.next() else {
                    return Err("--frames requires a value".to_string());
                };
                let parsed = parse_positive_u32("--frames", value.as_ref())?;
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().frame_limit = Some(parsed);
            }
            "--tcycles" => {
                let Some(value) = arguments.next() else {
                    return Err("--tcycles requires a value".to_string());
                };
                let parsed = parse_positive_u64("--tcycles", value.as_ref())?;
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().tcycle_limit = Some(parsed);
            }
            "--serial-stdout" => {
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().serial_stdout = true;
            }
            "--serial-out" => {
                let Some(value) = arguments.next() else {
                    return Err("--serial-out requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().serial_out = Some(PathBuf::from(value.as_ref()));
            }
            "--framebuffer-out" => {
                let Some(value) = arguments.next() else {
                    return Err("--framebuffer-out requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().framebuffer_out = Some(PathBuf::from(value.as_ref()));
            }
            "--border-off" => {
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().sgb_border = SgbBorderPresentationMode::Off;
            }
            "--palette" => {
                let Some(value) = arguments.next() else {
                    return Err("--palette requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().display_palette =
                    Some(parse_display_palette(value.as_ref())?);
            }
            "--trace-out" => {
                let Some(value) = arguments.next() else {
                    return Err("--trace-out requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().trace_out = Some(PathBuf::from(value.as_ref()));
            }
            "--state-in" => {
                let Some(value) = arguments.next() else {
                    return Err("--state-in requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().state_in = Some(PathBuf::from(value.as_ref()));
            }
            "--state-out" => {
                let Some(value) = arguments.next() else {
                    return Err("--state-out requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().state_out = Some(PathBuf::from(value.as_ref()));
            }
            "--save-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-dir requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().save_dir = Some(PathBuf::from(value.as_ref()));
            }
            "--save-key" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-key requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().save_key = Some(value.as_ref().to_string());
            }
            "--save-policy" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-policy requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().save_policy = parse_save_policy(value.as_ref())?;
                save_policy_explicit = true;
            }
            value if value.starts_with("--") => {
                return Err(format!(
                    "unknown run option {value:?}; run `gb-cli run --help`"
                ));
            }
            value => {
                if rom_path.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument {value:?}; run `gb-cli run --help`"
                    ));
                }
                rom_path = Some(PathBuf::from(value));
                let mut next_options = RunOptions::default_with_rom(
                    rom_path.clone().expect("rom_path was just assigned"),
                );
                next_options.test_runner = test_runner_requested;
                options = Some(next_options);
            }
        }
    }

    if let Some(benchmark_path) = benchmark_path {
        if rom_path.is_some() {
            return Err("--benchmark supplies the ROM path from the case TOML; omit the positional ROM path".to_string());
        }
        if options.is_some() {
            return Err("--benchmark cannot be combined with normal run options; put benchmark parameters in the case TOML".to_string());
        }
        return Ok(CliAction::RunBenchmark(BenchmarkRunOptions {
            benchmark_path,
            test_runner: test_runner_requested,
        }));
    }

    let mut options = options.ok_or_else(|| {
        "missing required ROM path; run `gb-cli run --help` for usage".to_string()
    })?;
    options.test_runner |= test_runner_requested;
    if options.test_runner {
        apply_test_runner_defaults(&mut options);
    }
    if options.save_dir.is_none() {
        if options.save_key.is_some() {
            return Err("--save-key requires --save-dir".to_string());
        }
        if save_policy_explicit {
            return Err("--save-policy requires --save-dir".to_string());
        }
    }
    if options.frame_limit.is_none() && options.tcycle_limit.is_none() {
        options.default_run_budget = Some(DefaultRunBudget::for_startup_mode(options.startup_mode));
    }
    if options.model != RunModel::GameBoy {
        options.display_palette = None;
    }
    validate_run_model_axes(&options, sgb_video_standard_explicit)?;

    Ok(CliAction::Run(Box::new(options)))
}

pub(crate) fn apply_test_runner_defaults(options: &mut RunOptions) {
    options.test_runner = true;
    options.execution_mode = ExecutionMode::Permissive;
    options.sgb_border = SgbBorderPresentationMode::Off;
    options.display_palette = Some(RunDisplayPalette::Grey);
}

pub(crate) fn validate_run_model_axes(
    options: &RunOptions,
    sgb_video_standard_explicit: bool,
) -> Result<(), String> {
    let console_model = options.model.console_model();
    let host_platform = options.model.host_platform();
    if !console_model.supports_revision_on_host(host_platform, options.revision) {
        return Err(format!(
            "--revision {} is not supported by --model {}; expected one of: {}",
            revision_argument_name(options.revision),
            options.model.name(),
            supported_revision_names_on_host(console_model, host_platform)
        ));
    }
    if sgb_video_standard_explicit && options.model != RunModel::SuperGameBoy {
        return Err("--sgb-standard requires --model SGB".to_string());
    }

    Ok(())
}

pub(crate) fn parse_inspect_rom_arguments<I, S>(arguments: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let mut rom_path = None;
    let mut execution_mode = ExecutionMode::Strict;

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(CliAction::ShowInspectHelp),
            "--mode" => {
                let Some(value) = arguments.next() else {
                    return Err("--mode requires a value".to_string());
                };
                execution_mode = parse_execution_mode(value.as_ref())?;
            }
            value if value.starts_with("--") => {
                return Err(format!(
                    "unknown inspect-rom option {value:?}; run `gb-cli inspect-rom --help`"
                ));
            }
            value => {
                if rom_path.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument {value:?}; run `gb-cli inspect-rom --help`"
                    ));
                }
                rom_path = Some(PathBuf::from(value));
            }
        }
    }

    Ok(CliAction::InspectRom(InspectRomOptions {
        rom_path: rom_path.ok_or_else(|| {
            "missing required ROM path; run `gb-cli inspect-rom --help` for usage".to_string()
        })?,
        execution_mode,
    }))
}

pub(crate) fn parse_saves_arguments<I, S>(arguments: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let Some(direction) = arguments.next() else {
        return Err("missing saves action; run `gb-cli saves --help` for usage".to_string());
    };
    let direction = match direction.as_ref() {
        "--help" | "-h" => return Ok(CliAction::ShowSavesHelp),
        "export" => SavesDirection::Export,
        "import" => SavesDirection::Import,
        other => {
            return Err(format!(
                "unknown saves action {other:?}; expected export or import"
            ));
        }
    };

    let mut positional = Vec::new();
    let mut save_dir = None;
    let mut save_key = None;

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(CliAction::ShowSavesHelp),
            "--save-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-dir requires a value".to_string());
                };
                save_dir = Some(PathBuf::from(value.as_ref()));
            }
            "--save-key" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-key requires a value".to_string());
                };
                save_key = Some(value.as_ref().to_string());
            }
            value if value.starts_with("--") => {
                return Err(format!(
                    "unknown saves option {value:?}; run `gb-cli saves --help`"
                ));
            }
            value => {
                if positional.len() >= 2 {
                    return Err(format!(
                        "unexpected extra positional argument {value:?}; run `gb-cli saves --help`"
                    ));
                }
                positional.push(PathBuf::from(value));
            }
        }
    }

    if positional.len() != 2 {
        return Err(
            "missing required ROM path or .sav path; run `gb-cli saves --help` for usage"
                .to_string(),
        );
    }

    Ok(CliAction::Saves(SavesOptions {
        direction,
        rom_path: positional.remove(0),
        external_save_path: positional.remove(0),
        save_dir: save_dir.ok_or_else(|| "--save-dir is required".to_string())?,
        save_key,
    }))
}

pub(crate) fn ensure_run_options_initialized(
    options: &mut Option<RunOptions>,
    rom_path: &Option<PathBuf>,
) -> Result<(), String> {
    if options.is_none() {
        if let Some(rom_path) = rom_path {
            *options = Some(RunOptions::default_with_rom(rom_path.clone()));
        } else {
            return Err(
                "the ROM path must be the first positional argument to `gb-cli run`".to_string(),
            );
        }
    }
    Ok(())
}

pub(crate) fn parse_run_model(value: &str) -> Result<RunModel, String> {
    match value {
        "DMG" => Ok(RunModel::GameBoy),
        "MGB" => Ok(RunModel::Pocket),
        "LGB" => Ok(RunModel::Light),
        "CGB" => Ok(RunModel::Color),
        "AGB" => Ok(RunModel::Advance),
        "SGB" => Ok(RunModel::SuperGameBoy),
        "SGB2" => Ok(RunModel::SuperGameBoy2),
        _ => Err(format!(
            "unsupported --model value {value:?}; expected one of: DMG, MGB, LGB, CGB, AGB, SGB, SGB2"
        )),
    }
}

pub(crate) fn parse_revision(value: &str) -> Result<HardwareRevision, String> {
    match value {
        "dmg-cpu-0" => Ok(HardwareRevision::DmgCpu0),
        "dmg-cpu-c" => Ok(HardwareRevision::DmgCpuC),
        "cpu-mgb" => Ok(HardwareRevision::CpuMgb),
        "cpu-cgb-c" => Ok(HardwareRevision::CpuCgbC),
        "cpu-cgb-d" => Ok(HardwareRevision::CpuCgbD),
        "cpu-cgb-e" => Ok(HardwareRevision::CpuCgbE),
        "cpu-agb-a" => Ok(HardwareRevision::CpuAgbA),
        _ => Err(format!(
            "unsupported --revision value {value:?}; expected dmg-cpu-0, dmg-cpu-c, cpu-mgb, cpu-cgb-c, cpu-cgb-d, cpu-cgb-e, or cpu-agb-a"
        )),
    }
}

pub(crate) fn parse_sgb_video_standard(value: &str) -> Result<SgbVideoStandard, String> {
    match value {
        "ntsc" => Ok(SgbVideoStandard::Ntsc),
        "pal" => Ok(SgbVideoStandard::Pal),
        _ => Err(format!(
            "unsupported --sgb-standard value {value:?}; expected ntsc or pal"
        )),
    }
}

pub(crate) fn parse_display_palette(value: &str) -> Result<RunDisplayPalette, String> {
    match value {
        "grey" => Ok(RunDisplayPalette::Grey),
        _ => Err(format!(
            "unsupported --palette value {value:?}; expected grey"
        )),
    }
}

pub(crate) fn parse_startup_mode(value: &str) -> Result<StartupMode, String> {
    match value {
        "skip-boot" => Ok(StartupMode::SkipBoot),
        "custom-boot" => Ok(StartupMode::CustomBoot),
        "real-boot" => Ok(StartupMode::RealBoot),
        _ => Err(format!(
            "unsupported --startup value {value:?}; expected skip-boot, custom-boot, or real-boot"
        )),
    }
}

pub(crate) fn parse_execution_mode(value: &str) -> Result<ExecutionMode, String> {
    match value {
        "strict" => Ok(ExecutionMode::Strict),
        "permissive" => Ok(ExecutionMode::Permissive),
        "experimental" => Ok(ExecutionMode::Experimental),
        _ => Err(format!(
            "unsupported --mode value {value:?}; expected strict, permissive, or experimental"
        )),
    }
}

pub(crate) fn parse_boot_rom_verification_mode(
    value: &str,
) -> Result<BootRomVerificationMode, String> {
    match value {
        "off" => Ok(BootRomVerificationMode::Off),
        "warn" => Ok(BootRomVerificationMode::Warn),
        "strict" => Ok(BootRomVerificationMode::Strict),
        _ => Err(format!(
            "unsupported --boot-rom-verify value {value:?}; expected off, warn, or strict"
        )),
    }
}

pub(crate) fn parse_save_policy(value: &str) -> Result<SavePolicy, String> {
    match value {
        "manual" => Ok(SavePolicy::Manual),
        "on-close" => Ok(SavePolicy::OnClose),
        "on-write" => Ok(SavePolicy::OnWrite),
        _ => Err(format!(
            "unsupported --save-policy value {value:?}; expected manual, on-close, or on-write"
        )),
    }
}

pub(crate) fn parse_positive_u32(flag: &str, value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|error| format!("invalid {flag} value {value:?}: {error}"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be greater than zero"));
    }
    Ok(parsed)
}

pub(crate) fn parse_positive_u64(flag: &str, value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|error| format!("invalid {flag} value {value:?}: {error}"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be greater than zero"));
    }
    Ok(parsed)
}
