use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::{
    CapturedArtifacts, EarlyHardeningStatus, RomRunner, RomSuite, RomSuiteReport,
    TEST_ROM_ROOT_ENV_VAR, Timeout, built_in_rom_suite_by_name, built_in_rom_suites,
    early_phase_9_partial_checklist, load_local_rom_suite_manifest, render_memory_bytes,
    update_curated_test_report,
};

const TEST_ROM_STARTUP_ENV_VAR: &str = "GB_CYCLE_TEST_ROM_STARTUP";

#[derive(Debug, Clone, PartialEq, Eq)]
enum RomSuiteCliAction {
    ShowHelp,
    ListSuites,
    ListSuitesDetailed,
    ShowEarlyChecklist,
    Run(RomSuiteCliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RomSuiteCliTarget {
    BuiltIn { suite_name: String },
    Manifest { manifest_path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RomSuiteCliOptions {
    target: RomSuiteCliTarget,
    case_id: Option<String>,
    failure_artifact_root: Option<PathBuf>,
    timeout_override: Option<Timeout>,
    threads: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfiguredRomSuiteStartup {
    Manifest,
    SkipBoot,
    CustomBoot,
    RealBoot,
}

pub fn rom_suite_cli_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo run -p gb-test-runner --bin run_rom_suite -- --list\n",
        "  cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed\n",
        "  cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist\n",
        "  cargo run -p gb-test-runner --bin run_rom_suite -- --suite <suite-name> [--case <case-id>] [--failure-artifact-root <dir>] [--timeout-frames <n> | --timeout-tcycles <n>] [--threads <n>]\n",
        "  cargo run -p gb-test-runner --bin run_rom_suite -- --manifest <path> [--case <case-id>] [--failure-artifact-root <dir>] [--timeout-frames <n> | --timeout-tcycles <n>] [--threads <n>]\n",
    )
}

pub fn run_rom_suite_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_rom_suite_command_with_runner(arguments, RomRunner::new(), output)
}

fn run_rom_suite_command_with_runner<I, S, W>(
    arguments: I,
    runner: RomRunner,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_rom_suite_arguments(arguments)? {
        RomSuiteCliAction::ShowHelp => write_all(output, rom_suite_cli_help_text()),
        RomSuiteCliAction::ListSuites => write_suite_catalog(output),
        RomSuiteCliAction::ListSuitesDetailed => write_detailed_suite_catalog(output),
        RomSuiteCliAction::ShowEarlyChecklist => write_early_hardening_checklist(output),
        RomSuiteCliAction::Run(options) => run_selected_suite(options, runner, output),
    }
}

fn parse_rom_suite_arguments<I, S>(arguments: I) -> Result<RomSuiteCliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut suite_name = None;
    let mut manifest_path = None;
    let mut case_id = None;
    let mut failure_artifact_root = None;
    let mut timeout_override = None;
    let mut threads = None;

    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--suite" => {
                let Some(value) = arguments.next() else {
                    return Err("--suite requires a value".to_string());
                };
                suite_name = Some(value.as_ref().to_string());
            }
            "--manifest" => {
                let Some(value) = arguments.next() else {
                    return Err("--manifest requires a value".to_string());
                };
                manifest_path = Some(PathBuf::from(value.as_ref()));
            }
            "--case" => {
                let Some(value) = arguments.next() else {
                    return Err("--case requires a value".to_string());
                };
                case_id = Some(value.as_ref().to_string());
            }
            "--failure-artifact-root" => {
                let Some(value) = arguments.next() else {
                    return Err("--failure-artifact-root requires a value".to_string());
                };
                failure_artifact_root = Some(PathBuf::from(value.as_ref()));
            }
            "--timeout-frames" => {
                let Some(value) = arguments.next() else {
                    return Err("--timeout-frames requires a value".to_string());
                };
                let parsed = value.as_ref().parse::<u32>().map_err(|error| {
                    format!(
                        "invalid --timeout-frames value {:?}: {error}",
                        value.as_ref()
                    )
                })?;
                timeout_override = Some(Timeout::Frames(parsed));
            }
            "--timeout-tcycles" => {
                let Some(value) = arguments.next() else {
                    return Err("--timeout-tcycles requires a value".to_string());
                };
                let parsed = value.as_ref().parse::<u64>().map_err(|error| {
                    format!(
                        "invalid --timeout-tcycles value {:?}: {error}",
                        value.as_ref()
                    )
                })?;
                timeout_override = Some(Timeout::TCycles(parsed));
            }
            "--threads" => {
                let Some(value) = arguments.next() else {
                    return Err("--threads requires a value".to_string());
                };
                let parsed = value.as_ref().parse::<usize>().map_err(|error| {
                    format!("invalid --threads value {:?}: {error}", value.as_ref())
                })?;
                if parsed == 0 {
                    return Err("--threads value must be greater than zero".to_string());
                }
                threads = Some(parsed);
            }
            "--list" => return Ok(RomSuiteCliAction::ListSuites),
            "--list-detailed" => return Ok(RomSuiteCliAction::ListSuitesDetailed),
            "--early-checklist" => return Ok(RomSuiteCliAction::ShowEarlyChecklist),
            "--help" | "-h" => return Ok(RomSuiteCliAction::ShowHelp),
            other => return Err(format!("unknown argument {other:?}; run with --help")),
        }
    }

    let target = match (suite_name, manifest_path) {
        (Some(_), Some(_)) => {
            return Err(
                "--suite <suite-name> and --manifest <path> are mutually exclusive".to_string(),
            );
        }
        (Some(suite_name), None) => RomSuiteCliTarget::BuiltIn { suite_name },
        (None, Some(manifest_path)) => RomSuiteCliTarget::Manifest { manifest_path },
        (None, None) => {
            return Err(
                "missing required --suite <suite-name> or --manifest <path>; run with --help"
                    .to_string(),
            );
        }
    };

    Ok(RomSuiteCliAction::Run(RomSuiteCliOptions {
        target,
        case_id,
        failure_artifact_root,
        timeout_override,
        threads,
    }))
}

fn run_selected_suite<W: Write>(
    options: RomSuiteCliOptions,
    runner: RomRunner,
    output: &mut W,
) -> Result<(), String> {
    let mut suite = select_suite_for_options(&options)?;
    apply_configured_startup_override(&mut suite)?;

    if let Some(timeout_override) = options.timeout_override {
        for case in &mut suite.cases {
            case.timeout = timeout_override;
        }
    }

    let mut runner = runner;
    if let Some(root) = options.failure_artifact_root {
        runner = runner.with_failure_artifact_root(root);
    }

    let report = if let Some(threads) = options.threads {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|error| format!("failed to configure rayon thread pool: {error}"))?;
        pool.install(|| runner.run_suite(&suite))
    } else {
        runner.run_suite(&suite)
    }
    .map_err(|error| format!("failed to execute suite {}: {error:?}", suite.name))?;
    write_suite_report(output, &report)?;
    match &options.target {
        RomSuiteCliTarget::BuiltIn { .. } => {
            if let Some(report_path) = update_curated_test_report(runner.workspace_root(), &report)?
            {
                writeln_checked(output, &format!("test_report={}", report_path.display()))?;
            }
        }
        RomSuiteCliTarget::Manifest { .. } => {
            write_manifest_framebuffer_exports(output, &runner, &suite, &report)?;
        }
    }

    if report.all_non_failing() {
        Ok(())
    } else {
        Err("one or more ROM cases failed".to_string())
    }
}

fn select_suite_for_options(options: &RomSuiteCliOptions) -> Result<RomSuite, String> {
    let suite = match &options.target {
        RomSuiteCliTarget::BuiltIn { suite_name } => {
            let Some(suite) = built_in_rom_suite_by_name(suite_name) else {
                return Err(format!(
                    "unknown suite {:?}; run with --list for the built-in catalog",
                    suite_name
                ));
            };
            suite
        }
        RomSuiteCliTarget::Manifest { manifest_path } => {
            load_local_rom_suite_manifest(manifest_path).map_err(|error| error.to_string())?
        }
    };

    select_case_for_options(suite, options.case_id.as_deref())
}

fn select_case_for_options(mut suite: RomSuite, case_id: Option<&str>) -> Result<RomSuite, String> {
    if let Some(case_id) = case_id {
        let Some(case) = suite.cases.into_iter().find(|case| case.id == case_id) else {
            return Err(format!(
                "suite {:?} does not contain case {:?}",
                suite.name, case_id
            ));
        };
        suite = if let Some(family) = suite.family {
            RomSuite::new(suite.name, suite.subsystem)
                .with_family(family)
                .with_case(case)
        } else {
            RomSuite::new(suite.name, suite.subsystem).with_case(case)
        };
    }

    Ok(suite)
}

fn configured_rom_suite_startup() -> Result<ConfiguredRomSuiteStartup, String> {
    configured_rom_suite_startup_from_env_value(env::var(TEST_ROM_STARTUP_ENV_VAR))
}

fn configured_rom_suite_startup_from_env_value(
    value: Result<String, env::VarError>,
) -> Result<ConfiguredRomSuiteStartup, String> {
    match value {
        Ok(value) => match value.as_str() {
            "skip-boot" => Ok(ConfiguredRomSuiteStartup::SkipBoot),
            "custom-boot" => Ok(ConfiguredRomSuiteStartup::CustomBoot),
            "real-boot" => Ok(ConfiguredRomSuiteStartup::RealBoot),
            other => Err(format!(
                "unsupported {TEST_ROM_STARTUP_ENV_VAR} value {other:?}; expected \"skip-boot\", \"custom-boot\", or \"real-boot\""
            )),
        },
        Err(env::VarError::NotPresent) => Ok(ConfiguredRomSuiteStartup::Manifest),
        Err(env::VarError::NotUnicode(_)) => Err(format!(
            "{TEST_ROM_STARTUP_ENV_VAR} must be valid UTF-8; expected \"skip-boot\", \"custom-boot\", or \"real-boot\""
        )),
    }
}

fn apply_configured_startup_override(suite: &mut RomSuite) -> Result<(), String> {
    apply_configured_startup_override_for(suite, configured_rom_suite_startup()?)
}

fn apply_configured_startup_override_for(
    suite: &mut RomSuite,
    startup: ConfiguredRomSuiteStartup,
) -> Result<(), String> {
    match startup {
        ConfiguredRomSuiteStartup::Manifest => {}
        ConfiguredRomSuiteStartup::SkipBoot => {
            for case in &mut suite.cases {
                case.startup_mode = gb_core::StartupMode::SkipBoot;
            }
        }
        ConfiguredRomSuiteStartup::CustomBoot => {
            for case in &mut suite.cases {
                case.startup_mode = gb_core::StartupMode::CustomBoot;
            }
        }
        ConfiguredRomSuiteStartup::RealBoot => {
            for case in &mut suite.cases {
                case.startup_mode = gb_core::StartupMode::RealBoot;
                case.startup_timer_state = None;
                case.startup_memory_writes.clear();
            }
        }
    }

    Ok(())
}

fn write_manifest_framebuffer_exports<W: Write>(
    output: &mut W,
    runner: &RomRunner,
    suite: &RomSuite,
    report: &RomSuiteReport,
) -> Result<(), String> {
    for case_report in &report.cases {
        let Some((framebuffer_png, channel)) =
            encode_manifest_framebuffer_export(&case_report.artifacts)?
        else {
            continue;
        };

        let Some(case) = suite
            .cases
            .iter()
            .find(|case| case.id == case_report.case_id)
        else {
            return Err(format!(
                "internal error: case {:?} was reported but not found in suite {:?}",
                case_report.case_id, suite.name
            ));
        };

        let rom_path = runner.resolve_case_rom_path(case).map_err(|error| {
            format!("failed to resolve ROM path for case {}: {error:?}", case.id)
        })?;
        let png_path = rom_path.with_extension("png");
        fs::write(&png_path, framebuffer_png).map_err(|error| {
            format!(
                "failed to write framebuffer PNG {}: {error}",
                png_path.display()
            )
        })?;
        writeln_checked(
            output,
            &format!(
                "case={} framebuffer_png={} channel={channel}",
                case.id,
                png_path.display()
            ),
        )?;
    }

    Ok(())
}

fn encode_manifest_framebuffer_export(
    artifacts: &CapturedArtifacts,
) -> Result<Option<(Vec<u8>, &'static str)>, String> {
    if let Some(framebuffer_rgb555) = &artifacts.framebuffer_rgb555 {
        return crate::framebuffer_oracle::encode_rgb555_framebuffer_png(framebuffer_rgb555)
            .map(|png| Some((png, "rgb555")))
            .map_err(|error| format!("failed to encode CGB RGB555 framebuffer capture: {error}"));
    }

    let Some(framebuffer_pgm) = &artifacts.framebuffer_pgm else {
        return Ok(None);
    };

    crate::framebuffer_oracle::convert_pgm_to_png(framebuffer_pgm)
        .map(|png| Some((png, "grayscale")))
        .map_err(|error| format!("failed to convert framebuffer capture: {}", error.message))
}

fn write_suite_catalog<W: Write>(output: &mut W) -> Result<(), String> {
    for suite in built_in_rom_suites() {
        writeln_checked(
            output,
            &format!(
                "suite={} family={} subsystem={:?}",
                suite.name,
                suite.family.as_deref().unwrap_or("-"),
                suite.subsystem,
            ),
        )?;
        for case in &suite.cases {
            writeln_checked(output, &format!("  case={}", case.id))?;
        }
    }

    Ok(())
}

fn write_detailed_suite_catalog<W: Write>(output: &mut W) -> Result<(), String> {
    for suite in built_in_rom_suites() {
        let (suite_sources, suite_oracles, suite_captures, suite_artifacts) =
            summarize_suite_contract(&suite);
        writeln_checked(
            output,
            &format!(
                "suite={} family={} subsystem={} cases={} sources={} oracles={} captures={} artifacts={}",
                suite.name,
                suite.family.as_deref().unwrap_or("-"),
                subsystem_name(suite.subsystem),
                suite.cases.len(),
                join_csv(&suite_sources),
                join_csv(&suite_oracles),
                join_csv(&suite_captures),
                join_csv(&suite_artifacts),
            ),
        )?;
        for case in &suite.cases {
            writeln_checked(
                output,
                &format!(
                    "  case={} family={} source={} oracle={} console={} revision={:?} startup={} mode={} timeout={} rom={} external_root_key={} captures={} artifacts={}",
                    case.id,
                    case_catalog_family(&suite, case),
                    case_source_name(case),
                    pass_condition_name(&case.pass_condition),
                    case_console_name(case),
                    case.revision,
                    startup_mode_name(case.startup_mode),
                    execution_mode_name(case.execution_mode),
                    timeout_name(case.timeout),
                    case.rom_path.display(),
                    case.external_rom_root_key.as_deref().unwrap_or("-"),
                    join_csv(&capture_names(case.capture_plan.captures().iter().copied())),
                    join_csv(&capture_names(
                        case.failure_artifacts.retained().iter().copied()
                    )),
                ),
            )?;
        }
    }

    Ok(())
}

fn write_early_hardening_checklist<W: Write>(output: &mut W) -> Result<(), String> {
    for entry in early_phase_9_partial_checklist() {
        writeln_checked(
            output,
            &format!(
                "subsystem={} status={} evidence={} oracles={} gaps={}",
                subsystem_name(entry.subsystem),
                early_hardening_status_name(entry.status),
                entry.current_evidence.join(","),
                entry.active_oracles.join(","),
                entry.remaining_gaps.join(","),
            ),
        )?;
    }

    Ok(())
}

fn write_suite_report<W: Write>(output: &mut W, report: &RomSuiteReport) -> Result<(), String> {
    writeln_checked(
        output,
        &format!(
            "suite={} subsystem={:?}",
            report.suite_name, report.subsystem
        ),
    )?;

    for case in &report.cases {
        writeln_checked(
            output,
            &format!(
                "case={} outcome={:?} t_cycles={} frames={}",
                case.case_id, case.outcome, case.executed_t_cycles, case.completed_frames
            ),
        )?;
        if !case.diagnostics.is_empty() {
            writeln_checked(output, &format!("diagnostics={:#?}", case.diagnostics))?;
        }
        write_artifacts(output, &case.artifacts)?;
        if !case.retained_failure_artifacts.is_empty() {
            writeln_checked(
                output,
                &format!(
                    "retained_failure_artifacts={:#?}",
                    case.retained_failure_artifacts
                ),
            )?;
        }
    }

    Ok(())
}

fn write_artifacts<W: Write>(output: &mut W, artifacts: &CapturedArtifacts) -> Result<(), String> {
    if let Some(serial) = &artifacts.serial {
        writeln_checked(output, &format!("serial=\n{serial}"))?;
    }

    if let Some(memory) = &artifacts.memory_text_output {
        writeln_checked(
            output,
            &format!(
                "memory_text_output=status=0x{status:02X} signature={signature:02X?}\n{text}",
                status = memory.status,
                signature = memory.signature,
                text = memory.text
            ),
        )?;
    }

    if let Some(memory_bytes) = &artifacts.memory_bytes {
        writeln_checked(
            output,
            &format!("memory_bytes=\n{}", render_memory_bytes(memory_bytes)),
        )?;
    }

    if let Some(console_text) = &artifacts.blargg_console_text {
        writeln_checked(output, &format!("blargg_console_text=\n{console_text}"))?;
    }

    if let Some(snapshot) = &artifacts.snapshot_text {
        writeln_checked(output, &format!("snapshot=\n{snapshot}"))?;
    }

    if let Some(trace) = &artifacts.trace {
        writeln_checked(output, &format!("trace=\n{trace}"))?;
    }

    if let Some(framebuffer_rgb555) = &artifacts.framebuffer_rgb555 {
        writeln_checked(
            output,
            &format!("framebuffer_rgb555_pixels={}", framebuffer_rgb555.len()),
        )?;
    } else if let Some(framebuffer) = &artifacts.framebuffer_pgm {
        writeln_checked(
            output,
            &format!("framebuffer_pgm_bytes={}", framebuffer.len()),
        )?;
    }

    Ok(())
}

fn summarize_suite_contract(
    suite: &crate::RomSuite,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut sources = Vec::new();
    let mut oracles = Vec::new();
    let mut captures = Vec::new();
    let mut artifacts = Vec::new();

    for case in &suite.cases {
        push_unique(&mut sources, case_source_name(case).to_string());
        push_unique(
            &mut oracles,
            pass_condition_name(&case.pass_condition).to_string(),
        );

        for capture in case.capture_plan.captures().iter().copied() {
            push_unique(&mut captures, capture_name(capture).to_string());
        }

        for artifact in case.failure_artifacts.retained().iter().copied() {
            push_unique(&mut artifacts, capture_name(artifact).to_string());
        }
    }

    (sources, oracles, captures, artifacts)
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn join_csv(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
    }
}

fn subsystem_name(subsystem: crate::TestSubsystem) -> &'static str {
    match subsystem {
        crate::TestSubsystem::Cpu => "cpu",
        crate::TestSubsystem::Interrupts => "interrupts",
        crate::TestSubsystem::Bus => "bus",
        crate::TestSubsystem::Cartridge => "cartridge",
        crate::TestSubsystem::Timer => "timer",
        crate::TestSubsystem::Ppu => "ppu",
        crate::TestSubsystem::Dma => "dma",
        crate::TestSubsystem::Apu => "apu",
        crate::TestSubsystem::Boot => "boot",
        crate::TestSubsystem::Joypad => "joypad",
        crate::TestSubsystem::Serial => "serial",
        crate::TestSubsystem::Scheduler => "scheduler",
        crate::TestSubsystem::CrossSubsystem => "cross-subsystem",
    }
}

fn early_hardening_status_name(status: EarlyHardeningStatus) -> &'static str {
    match status {
        EarlyHardeningStatus::InternalGateOnly => "internal-gate-only",
        EarlyHardeningStatus::RepoGatePresent => "repo-gate-present",
    }
}

fn pass_condition_name(pass_condition: &crate::PassCondition) -> &'static str {
    match pass_condition {
        crate::PassCondition::SerialExact(_) => "serial-exact",
        crate::PassCondition::SerialContains(_) => "serial-contains",
        crate::PassCondition::SerialHexExact(_) => "serial-hex-exact",
        crate::PassCondition::MemoryBytesEqual(_) => "memory-byte-equals",
        crate::PassCondition::MemoryTextOutputContains { .. } => "memory-text-output",
        crate::PassCondition::BlarggConsoleTextContains(_) => "blargg-console-text",
        crate::PassCondition::MooneyeResult => "mooneye-result",
        crate::PassCondition::Informational(capture) => match capture {
            crate::CaptureKind::Serial => "info-serial",
            crate::CaptureKind::SerialHex => "info-serial-hex",
            crate::CaptureKind::MemoryBytes => "info-memory-bytes",
            crate::CaptureKind::MemoryTextOutput => "info-memory-text-output",
            crate::CaptureKind::BlarggConsoleText => "info-blargg-console-text",
            crate::CaptureKind::Framebuffer => "info-framebuffer",
            crate::CaptureKind::Trace => "info-trace",
            crate::CaptureKind::Snapshot => "info-snapshot",
        },
        crate::PassCondition::FramebufferFixture(_) => "framebuffer-fixture",
        crate::PassCondition::FramebufferFixtureUntilMatch { .. } => {
            "framebuffer-fixture-until-match"
        }
        crate::PassCondition::FramebufferGrayscaleFixture(_) => "framebuffer-grayscale-fixture",
        crate::PassCondition::FramebufferRgb555Fixture(_) => "framebuffer-rgb555-fixture",
        crate::PassCondition::FramebufferRgb555FixtureUntilMatch { .. } => {
            "framebuffer-rgb555-fixture-until-match"
        }
        crate::PassCondition::FramebufferRgb555GrayscaleFixture(_) => {
            "framebuffer-rgb555-grayscale-fixture"
        }
        crate::PassCondition::FramebufferFixtureSet(_) => "framebuffer-fixture-set",
        crate::PassCondition::TraceFixture(_) => "trace-fixture",
    }
}

fn capture_name(capture: crate::CaptureKind) -> &'static str {
    match capture {
        crate::CaptureKind::Serial => "serial",
        crate::CaptureKind::SerialHex => "serial-hex",
        crate::CaptureKind::MemoryBytes => "memory-bytes",
        crate::CaptureKind::MemoryTextOutput => "memory-text-output",
        crate::CaptureKind::BlarggConsoleText => "blargg-console-text",
        crate::CaptureKind::Framebuffer => "framebuffer",
        crate::CaptureKind::Trace => "trace",
        crate::CaptureKind::Snapshot => "snapshot",
    }
}

fn capture_names<I>(captures: I) -> Vec<String>
where
    I: IntoIterator<Item = crate::CaptureKind>,
{
    captures
        .into_iter()
        .map(|capture| capture_name(capture).to_string())
        .collect()
}

fn case_source_name(case: &crate::RomTestCase) -> &'static str {
    if case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR) {
        "test-rom-store"
    } else if case.external_rom_root_key.is_some() {
        "external-rom"
    } else {
        "repo-fixture"
    }
}

fn case_catalog_family<'a>(suite: &'a RomSuite, case: &'a crate::RomTestCase) -> &'a str {
    if case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
        && let Some(family) = case.rom_path.components().next().and_then(|component| {
            component
                .as_os_str()
                .to_str()
                .filter(|value| !value.is_empty())
        })
    {
        family
    } else {
        suite.family.as_deref().unwrap_or("-")
    }
}

fn case_console_name(case: &crate::RomTestCase) -> &'static str {
    match case.host_platform {
        gb_core::HostPlatform::Sgb => "sgb",
        gb_core::HostPlatform::Sgb2 => "sgb2",
        gb_core::HostPlatform::Handheld => match case.console_model {
            gb_core::ConsoleModel::GameBoy => "game-boy",
            gb_core::ConsoleModel::GameBoyPocket => "pocket",
            gb_core::ConsoleModel::GameBoyLight => "light",
            gb_core::ConsoleModel::GameBoyColor => "color",
        },
    }
}

fn startup_mode_name(startup_mode: gb_core::StartupMode) -> &'static str {
    match startup_mode {
        gb_core::StartupMode::SkipBoot => "skip-boot",
        gb_core::StartupMode::CustomBoot => "custom-boot",
        gb_core::StartupMode::RealBoot => "real-boot",
    }
}

fn execution_mode_name(execution_mode: gb_core::ExecutionMode) -> &'static str {
    match execution_mode {
        gb_core::ExecutionMode::Strict => "strict",
        gb_core::ExecutionMode::Permissive => "permissive",
        gb_core::ExecutionMode::Experimental => "experimental",
    }
}

fn timeout_name(timeout: crate::Timeout) -> String {
    match timeout {
        crate::Timeout::TCycles(limit) => format!("tcycles:{limit}"),
        crate::Timeout::Frames(limit) => format!("frames:{limit}"),
    }
}

fn write_all<W: Write>(output: &mut W, text: &str) -> Result<(), String> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write command output: {error}"))
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}").map_err(|error| format!("failed to write command output: {error}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{CapturedMemoryByte, CapturedMemoryBytes, CapturedMemoryTextOutput, TestSubsystem};

    use super::{
        ConfiguredRomSuiteStartup, RomSuiteCliAction, RomSuiteCliOptions, RomSuiteCliTarget,
        TEST_ROM_STARTUP_ENV_VAR, apply_configured_startup_override_for,
        configured_rom_suite_startup_from_env_value, parse_rom_suite_arguments,
        rom_suite_cli_help_text, run_rom_suite_command_with_runner, select_case_for_options,
        select_suite_for_options, write_suite_report,
    };
    use crate::{
        CapturedArtifacts, PassCondition, RomCaseOutcome, RomCaseReport, RomRunner, RomSuite,
        RomSuiteReport, RomTestCase, Timeout, default_workspace_root,
    };

    fn unique_temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gb-cycle-run-rom-suite-cli-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn write_manifest(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::create_dir_all(dir).expect("manifest parent should be creatable");
        fs::write(&path, body).expect("manifest should be writable");
        path
    }

    #[test]
    fn parse_arguments_supports_help_list_and_timeout_overrides() {
        assert_eq!(
            parse_rom_suite_arguments(["--help"]).expect("help should parse"),
            RomSuiteCliAction::ShowHelp
        );
        assert_eq!(
            parse_rom_suite_arguments(["--list"]).expect("list should parse"),
            RomSuiteCliAction::ListSuites
        );
        assert_eq!(
            parse_rom_suite_arguments(["--list-detailed"]).expect("detailed list should parse"),
            RomSuiteCliAction::ListSuitesDetailed
        );
        assert_eq!(
            parse_rom_suite_arguments(["--early-checklist"]).expect("checklist should parse"),
            RomSuiteCliAction::ShowEarlyChecklist
        );
        assert_eq!(
            parse_rom_suite_arguments([
                "--suite",
                "phase-2-cpu-timing",
                "--case",
                "phase2-fetch-immediate-order",
                "--failure-artifact-root",
                "/tmp/artifacts",
                "--timeout-tcycles",
                "1234",
            ])
            .expect("run args should parse"),
            RomSuiteCliAction::Run(RomSuiteCliOptions {
                target: RomSuiteCliTarget::BuiltIn {
                    suite_name: "phase-2-cpu-timing".to_string(),
                },
                case_id: Some("phase2-fetch-immediate-order".to_string()),
                failure_artifact_root: Some(PathBuf::from("/tmp/artifacts")),
                timeout_override: Some(crate::Timeout::TCycles(1234)),
                threads: None,
            })
        );
        assert_eq!(
            parse_rom_suite_arguments(["--manifest", "/tmp/local-case.toml"])
                .expect("manifest args should parse"),
            RomSuiteCliAction::Run(RomSuiteCliOptions {
                target: RomSuiteCliTarget::Manifest {
                    manifest_path: PathBuf::from("/tmp/local-case.toml"),
                },
                case_id: None,
                failure_artifact_root: None,
                timeout_override: None,
                threads: None,
            })
        );
    }

    #[test]
    fn parse_arguments_rejects_missing_and_invalid_values() {
        let missing_suite = parse_rom_suite_arguments(std::iter::empty::<&str>())
            .expect_err("missing suite should be rejected");
        assert!(missing_suite.contains("missing required --suite"));

        let invalid_timeout =
            parse_rom_suite_arguments(["--suite", "phase-2-cpu-timing", "--timeout-frames", "NaN"])
                .expect_err("invalid timeout should be rejected");
        assert!(invalid_timeout.contains("invalid --timeout-frames value"));

        let conflicting_targets = parse_rom_suite_arguments([
            "--suite",
            "phase-2-cpu-timing",
            "--manifest",
            "/tmp/local-case.toml",
        ])
        .expect_err("conflicting target selectors should be rejected");
        assert!(conflicting_targets.contains("mutually exclusive"));
    }

    #[test]
    fn parse_arguments_accepts_explicit_threads_count() {
        assert_eq!(
            parse_rom_suite_arguments(["--suite", "phase-2-cpu-timing", "--threads", "4"])
                .expect("explicit threads should parse"),
            RomSuiteCliAction::Run(RomSuiteCliOptions {
                target: RomSuiteCliTarget::BuiltIn {
                    suite_name: "phase-2-cpu-timing".to_string(),
                },
                case_id: None,
                failure_artifact_root: None,
                timeout_override: None,
                threads: Some(4),
            })
        );
    }

    #[test]
    fn parse_arguments_rejects_missing_and_invalid_threads_values() {
        let missing_value =
            parse_rom_suite_arguments(["--suite", "phase-2-cpu-timing", "--threads"])
                .expect_err("missing threads value should be rejected");
        assert!(missing_value.contains("--threads requires a value"));

        let invalid_value =
            parse_rom_suite_arguments(["--suite", "phase-2-cpu-timing", "--threads", "NaN"])
                .expect_err("invalid threads value should be rejected");
        assert!(invalid_value.contains("invalid --threads value"));

        let zero_value =
            parse_rom_suite_arguments(["--suite", "phase-2-cpu-timing", "--threads", "0"])
                .expect_err("zero threads should be rejected");
        assert!(zero_value.contains("greater than zero"));
    }

    #[test]
    fn select_suite_rejects_unknown_suites_and_unknown_cases() {
        let unknown_suite = select_suite_for_options(&RomSuiteCliOptions {
            target: RomSuiteCliTarget::BuiltIn {
                suite_name: "unknown".to_string(),
            },
            case_id: None,
            failure_artifact_root: None,
            timeout_override: None,
            threads: None,
        })
        .expect_err("unknown suite should be rejected");
        assert!(unknown_suite.contains("unknown suite"));

        let unknown_case = select_suite_for_options(&RomSuiteCliOptions {
            target: RomSuiteCliTarget::BuiltIn {
                suite_name: "phase-2-cpu-timing".to_string(),
            },
            case_id: Some("missing-case".to_string()),
            failure_artifact_root: None,
            timeout_override: None,
            threads: None,
        })
        .expect_err("unknown case should be rejected");
        assert!(unknown_case.contains("does not contain case"));
    }

    #[test]
    fn select_suite_can_filter_a_family_backed_built_in_suite_to_one_case() {
        let suite = select_suite_for_options(&RomSuiteCliOptions {
            target: RomSuiteCliTarget::BuiltIn {
                suite_name: "acid-dmg-curated".to_string(),
            },
            case_id: Some("dmg-acid2".to_string()),
            failure_artifact_root: None,
            timeout_override: None,
            threads: None,
        })
        .expect("known curated case should be selectable");

        assert_eq!(suite.name, "acid-dmg-curated");
        assert_eq!(suite.family.as_deref(), Some("acid"));
        assert_eq!(suite.cases.len(), 1);
        assert_eq!(suite.cases[0].id, "dmg-acid2");
    }

    #[test]
    fn select_case_for_options_leaves_familyless_suites_unchanged_without_a_case_filter() {
        let suite = select_case_for_options(crate::phase_6_cartridge_oracle_suite(), None)
            .expect("no case filter should keep the original suite");

        assert_eq!(suite.name, "phase-6-cartridge-oracle");
        assert!(suite.family.is_none());
        assert_eq!(suite.cases.len(), 5);
    }

    #[test]
    fn startup_env_default_preserves_manifest_startup_modes_and_synthetic_state() {
        let original_suite = crate::cgb_ppu_basic_suite();
        let mut configured_suite = original_suite.clone();

        apply_configured_startup_override_for(
            &mut configured_suite,
            ConfiguredRomSuiteStartup::Manifest,
        )
        .expect("default startup override should parse");

        assert_eq!(configured_suite, original_suite);
    }

    #[test]
    fn startup_env_skip_boot_overrides_real_and_custom_boot_cases_without_requiring_assets() {
        let mut suite = RomSuite::new("real-boot-fixture", TestSubsystem::CrossSubsystem)
            .with_case(
                RomTestCase::new(
                    "real-boot-case",
                    "fixture.gb",
                    Timeout::Frames(1),
                    PassCondition::MooneyeResult,
                )
                .with_startup_mode(gb_core::StartupMode::RealBoot),
            )
            .with_case(
                RomTestCase::new(
                    "custom-boot-case",
                    "fixture.gb",
                    Timeout::Frames(1),
                    PassCondition::MooneyeResult,
                )
                .with_startup_mode(gb_core::StartupMode::CustomBoot),
            );

        apply_configured_startup_override_for(&mut suite, ConfiguredRomSuiteStartup::SkipBoot)
            .expect("skip-boot startup override should parse");

        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.startup_mode == gb_core::StartupMode::SkipBoot)
        );
    }

    #[test]
    fn startup_env_custom_boot_overrides_suite_without_requiring_assets() {
        let mut suite = crate::cgb_ppu_basic_suite();

        apply_configured_startup_override_for(&mut suite, ConfiguredRomSuiteStartup::CustomBoot)
            .expect("custom-boot startup override should parse");

        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.startup_mode == gb_core::StartupMode::CustomBoot)
        );
    }

    #[test]
    fn startup_env_real_boot_overrides_suite_and_clears_direct_start_state() {
        let mut suite = RomSuite::new("startup-state-fixture", TestSubsystem::CrossSubsystem)
            .with_case(
                RomTestCase::new(
                    "custom-boot-state-case",
                    "fixture.gb",
                    Timeout::Frames(1),
                    PassCondition::MooneyeResult,
                )
                .with_startup_mode(gb_core::StartupMode::CustomBoot)
                .with_startup_timer_state(gb_core::TimerStartupState {
                    system_counter: 0x1234,
                    tima: 0x00,
                    tma: 0x00,
                    tac: 0x00,
                }),
            );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.startup_timer_state.is_some()),
            "fixture should cover synthetic startup timer state"
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.startup_mode == gb_core::StartupMode::CustomBoot),
            "fixture should cover a custom-boot manifest case"
        );

        apply_configured_startup_override_for(&mut suite, ConfiguredRomSuiteStartup::RealBoot)
            .expect("real-boot startup override should parse");

        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.startup_mode == gb_core::StartupMode::RealBoot)
        );
        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.startup_timer_state.is_none())
        );
        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.startup_memory_writes.is_empty())
        );
    }

    #[test]
    fn startup_env_rejects_unknown_values() {
        let error = configured_rom_suite_startup_from_env_value(Ok("warm-boot".to_string()))
            .expect_err("unknown startup mode should fail");

        assert!(error.contains(TEST_ROM_STARTUP_ENV_VAR));
        assert!(error.contains("skip-boot"));
        assert!(error.contains("custom-boot"));
        assert!(error.contains("real-boot"));
    }

    #[test]
    fn list_and_help_commands_render_human_readable_output() {
        let mut help_output = Vec::new();
        run_rom_suite_command_with_runner(["--help"], RomRunner::new(), &mut help_output)
            .expect("help command should succeed");
        assert_eq!(
            String::from_utf8(help_output).expect("help output should be utf-8"),
            rom_suite_cli_help_text()
        );

        let mut list_output = Vec::new();
        run_rom_suite_command_with_runner(["--list"], RomRunner::new(), &mut list_output)
            .expect("list command should succeed");
        let list_output = String::from_utf8(list_output).expect("list output should be utf-8");
        assert!(list_output.contains("suite=phase-2-cpu-timing"));
        assert!(list_output.contains("case=phase2-fetch-immediate-order"));

        let mut detailed_output = Vec::new();
        run_rom_suite_command_with_runner(
            ["--list-detailed"],
            RomRunner::new(),
            &mut detailed_output,
        )
        .expect("detailed list command should succeed");
        let detailed_output =
            String::from_utf8(detailed_output).expect("detailed output should be utf-8");
        assert!(
            detailed_output.contains(
                "suite=blargg-dmg-curated family=blargg subsystem=cross-subsystem cases=38 sources=test-rom-store"
            )
        );
        assert!(
            detailed_output.contains(
                "oracles=serial-contains,blargg-console-text,memory-text-output captures=serial,snapshot,blargg-console-text,memory-text-output"
            )
        );
        assert!(detailed_output.contains(
            "case=blargg-oam-bug-1-lcd-sync family=blargg source=test-rom-store oracle=memory-text-output"
        ));
        assert!(detailed_output.contains(
            "suite=cgb-smoke family=cgb-smoke subsystem=cross-subsystem cases=2 sources=test-rom-store oracles=mooneye-result,info-framebuffer"
        ));
        assert!(detailed_output.contains(
            "case=cgb-smoke-boot-regs-cgb family=mooneye source=test-rom-store oracle=mooneye-result console=color"
        ));
        assert!(detailed_output.contains(
            "case=cgb-smoke-which-gbc family=acid source=test-rom-store oracle=info-framebuffer console=color"
        ));

        let mut checklist_output = Vec::new();
        run_rom_suite_command_with_runner(
            ["--early-checklist"],
            RomRunner::new(),
            &mut checklist_output,
        )
        .expect("checklist command should succeed");
        let checklist_output =
            String::from_utf8(checklist_output).expect("checklist output should be utf-8");
        assert!(checklist_output.contains("subsystem=cpu status=repo-gate-present"));
        assert!(checklist_output.contains("subsystem=ppu status=repo-gate-present"));
        assert!(checklist_output.contains("subsystem=timer status=repo-gate-present"));
        assert!(checklist_output.contains("subsystem=cartridge status=repo-gate-present"));
    }

    #[test]
    fn report_writer_covers_all_artifact_channels() {
        let report = RomSuiteReport {
            suite_name: "synthetic".to_string(),
            family: None,
            subsystem: TestSubsystem::Cpu,
            cases: vec![RomCaseReport {
                case_id: "case-a".to_string(),
                rom_path: PathBuf::from("synthetic/case-a.gb"),
                outcome: RomCaseOutcome::Failed(crate::RomCaseFailure::TimeoutExceeded),
                executed_t_cycles: 64,
                completed_frames: 0,
                diagnostics: Vec::new(),
                artifacts: CapturedArtifacts {
                    serial: Some("serial-text".to_string()),
                    serial_hex: Some("73657269616C2D74657874".to_string()),
                    memory_bytes: Some(CapturedMemoryBytes {
                        bytes: vec![CapturedMemoryByte {
                            address: 0xFF82,
                            expected: 0x01,
                            fail_value: None,
                            actual: 0x56,
                        }],
                    }),
                    memory_text_output: Some(CapturedMemoryTextOutput {
                        status: 0,
                        signature: [0xDE, 0xB0, 0x61],
                        text: "Passed".to_string(),
                    }),
                    blargg_console_text: Some("console-text".to_string()),
                    framebuffer_pgm: Some(vec![0; 8]),
                    framebuffer_rgb555: None,
                    trace: Some("trace-text".to_string()),
                    snapshot_text: Some("snapshot-text".to_string()),
                },
                retained_failure_artifacts: vec![PathBuf::from("/tmp/trace.txt")],
            }],
        };

        let mut output = Vec::new();
        write_suite_report(&mut output, &report).expect("report writer should succeed");
        let output = String::from_utf8(output).expect("report output should be utf-8");

        assert!(output.contains("suite=synthetic subsystem=Cpu"));
        assert!(output.contains("case=case-a outcome=Failed"));
        assert!(output.contains("serial=\nserial-text"));
        assert!(output.contains("memory_bytes=\naddress=0xFF82 expected=0x01 actual=0x56"));
        assert!(output.contains("memory_text_output=status=0x00"));
        assert!(output.contains("blargg_console_text=\nconsole-text"));
        assert!(output.contains("snapshot=\nsnapshot-text"));
        assert!(output.contains("trace=\ntrace-text"));
        assert!(output.contains("framebuffer_pgm_bytes=8"));
        assert!(output.contains("retained_failure_artifacts="));
    }

    #[test]
    fn report_writer_prefers_cgb_rgb555_framebuffer_channel() {
        let report = RomSuiteReport {
            suite_name: "synthetic-cgb".to_string(),
            family: None,
            subsystem: TestSubsystem::Ppu,
            cases: vec![RomCaseReport {
                case_id: "case-cgb".to_string(),
                rom_path: PathBuf::from("synthetic/case-cgb.gbc"),
                outcome: RomCaseOutcome::Passed,
                executed_t_cycles: 64,
                completed_frames: 1,
                diagnostics: Vec::new(),
                artifacts: CapturedArtifacts {
                    framebuffer_pgm: Some(vec![0; 8]),
                    framebuffer_rgb555: Some(vec![0x7FFF; 160 * 144]),
                    ..CapturedArtifacts::default()
                },
                retained_failure_artifacts: Vec::new(),
            }],
        };

        let mut output = Vec::new();
        write_suite_report(&mut output, &report).expect("report writer should succeed");
        let output = String::from_utf8(output).expect("report output should be utf-8");

        assert!(output.contains("framebuffer_rgb555_pixels=23040"));
        assert!(
            !output.contains("framebuffer_pgm_bytes="),
            "CGB reports should not advertise the legacy grayscale framebuffer channel"
        );
    }

    #[test]
    fn run_command_can_execute_a_known_built_in_case() {
        let mut output = Vec::new();
        run_rom_suite_command_with_runner(
            [
                "--suite",
                "phase-2-cpu-timing",
                "--case",
                "phase2-fetch-immediate-order",
            ],
            RomRunner::new().with_workspace_root(default_workspace_root()),
            &mut output,
        )
        .expect("built-in phase2 case should execute");

        let output = String::from_utf8(output).expect("command output should be utf-8");
        assert!(output.contains("suite=phase-2-cpu-timing"));
        assert!(output.contains("case=phase2-fetch-immediate-order outcome=Passed"));
    }

    #[test]
    fn run_command_can_reconfigure_explicit_threads_per_invocation() {
        let workspace_root = default_workspace_root();
        let temp_root = unique_temp_dir("manifest-threads");
        let rom_path = temp_root
            .join("fixtures")
            .join("phase2_fetch_immediate_order.gb");
        fs::create_dir_all(
            rom_path
                .parent()
                .expect("temporary ROM path should have a parent"),
        )
        .expect("temporary ROM parent should be creatable");
        fs::copy(
            workspace_root
                .join("crates/gb-core/tests/fixtures/roms/phase2/phase2_fetch_immediate_order.gb"),
            &rom_path,
        )
        .expect("fixture ROM should copy into the temporary manifest workspace");

        let manifest_path = write_manifest(
            &temp_root,
            "local-case.toml",
            &format!(
                r#"
version = 1

[[case]]
id = "phase2-threaded"
rom = "{}"
timeout_tcycles = 32
oracle = "info-framebuffer"
"#,
                rom_path.display()
            ),
        );
        let manifest = manifest_path
            .to_str()
            .expect("manifest path should be utf-8");

        for threads in ["2", "1"] {
            let mut output = Vec::new();
            run_rom_suite_command_with_runner(
                ["--manifest", manifest, "--threads", threads],
                RomRunner::new().with_workspace_root(workspace_root.clone()),
                &mut output,
            )
            .expect("explicit thread count should apply to this invocation only");

            let rendered = String::from_utf8(output).expect("manifest output should be utf-8");
            assert!(rendered.contains("suite=local-case subsystem=CrossSubsystem"));
            assert!(rendered.contains("case=phase2-threaded outcome=Informational"));
        }

        fs::remove_dir_all(temp_root).expect("temporary manifest workspace should be removable");
    }

    #[test]
    fn manifest_command_executes_local_case_and_exports_framebuffer_png_next_to_the_rom() {
        let workspace_root = default_workspace_root();
        let temp_root = unique_temp_dir("manifest-export");
        let rom_path = temp_root
            .join("fixtures")
            .join("phase2_fetch_immediate_order.gb");
        fs::create_dir_all(
            rom_path
                .parent()
                .expect("temporary ROM path should have a parent"),
        )
        .expect("temporary ROM parent should be creatable");
        fs::copy(
            workspace_root
                .join("crates/gb-core/tests/fixtures/roms/phase2/phase2_fetch_immediate_order.gb"),
            &rom_path,
        )
        .expect("fixture ROM should copy into the temporary manifest workspace");

        let manifest_path = write_manifest(
            &temp_root,
            "local-case.toml",
            &format!(
                r#"
version = 1

[[case]]
id = "phase2-export"
rom = "{}"
timeout_tcycles = 32
oracle = "info-framebuffer"
"#,
                rom_path.display()
            ),
        );

        let mut output = Vec::new();
        run_rom_suite_command_with_runner(
            [
                "--manifest",
                manifest_path
                    .to_str()
                    .expect("manifest path should be utf-8"),
            ],
            RomRunner::new().with_workspace_root(workspace_root),
            &mut output,
        )
        .expect("manifest-driven local case should execute");

        let rendered = String::from_utf8(output).expect("manifest output should be utf-8");
        let png_path = rom_path.with_extension("png");
        assert!(rendered.contains("suite=local-case subsystem=CrossSubsystem"));
        assert!(rendered.contains("case=phase2-export outcome=Informational"));
        assert!(rendered.contains(&format!(
            "case=phase2-export framebuffer_png={} channel=grayscale",
            png_path.display()
        )));
        assert!(png_path.is_file());
    }
}
