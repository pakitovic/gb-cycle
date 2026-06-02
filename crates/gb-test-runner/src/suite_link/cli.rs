use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use gb_core::StartupMode;

use crate::boot_rom::{BootRomProfile, load_verified_boot_rom_assets};
use crate::default_workspace_root;
use crate::fetch::ensure_report_families_materialized;

use super::manifest::{load_reports, load_selected_link_suites};
use super::model::{LinkRunConfig, LinkSuiteManifest, Report};
use super::run::run_link_suite_with_config;
use super::status::write_link_suite_status;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SuiteLinkAction {
    ShowHelp,
    Run(SuiteLinkOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SuiteLinkOptions {
    report_id: Option<String>,
    suite_name: Option<String>,
    case_id: Option<String>,
    threads: Option<usize>,
    boot_rom_dir: Option<PathBuf>,
}

pub fn suite_link_help_text() -> &'static str {
    concat!(
        "Usage: cargo run -p gb-test-runner --bin suite_link -- <report-id> [--suite <suite-name>] [--case <case-id>] [--threads <n>] [--boot-rom-dir <dir>]\n",
        "\n",
        "Runs report-local *.link.suite.toml linked-session manifests through the new linked suite runner.\n",
    )
}

pub fn run_suite_link_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_suite_link_command_with_workspace(arguments, &default_workspace_root(), output)
}

fn run_suite_link_command_with_workspace<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_suite_link_arguments(arguments)? {
        SuiteLinkAction::ShowHelp => write_all(output, suite_link_help_text()),
        SuiteLinkAction::Run(options) => run_options(options, workspace_root, output),
    }
}

fn parse_suite_link_arguments<I, S>(arguments: I) -> Result<SuiteLinkAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut report_id = None;
    let mut suite_name = None;
    let mut case_id = None;
    let mut threads = None;
    let mut boot_rom_dir = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(SuiteLinkAction::ShowHelp),
            "--suite" => {
                let Some(value) = arguments.next() else {
                    return Err("--suite requires a value".to_string());
                };
                suite_name = Some(value.as_ref().to_string());
            }
            "--case" => {
                let Some(value) = arguments.next() else {
                    return Err("--case requires a value".to_string());
                };
                case_id = Some(value.as_ref().to_string());
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
            "--boot-rom-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--boot-rom-dir requires a value".to_string());
                };
                boot_rom_dir = Some(PathBuf::from(value.as_ref()));
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown argument {value:?}; run with --help"));
            }
            value => {
                if report_id.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument {value:?}; run with --help"
                    ));
                }
                report_id = Some(value.to_string());
            }
        }
    }

    if case_id.is_some() && suite_name.is_none() {
        return Err("--case requires --suite <suite-name>".to_string());
    }

    Ok(SuiteLinkAction::Run(SuiteLinkOptions {
        report_id,
        suite_name,
        case_id,
        threads,
        boot_rom_dir,
    }))
}

fn run_options<W: Write>(
    options: SuiteLinkOptions,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String> {
    let reports = load_reports(workspace_root)?;
    let Some(report_id) = options.report_id else {
        return Err(missing_report_error(&reports));
    };
    let report = report_for_id(&report_id, &reports)?;
    let mut suites = load_selected_link_suites(
        workspace_root,
        report,
        options.suite_name.as_deref(),
        options.case_id.as_deref(),
    )?;
    if suites.is_empty() {
        return Err(format!(
            "report {report_id:?} does not contain linked suite manifests"
        ));
    }
    ensure_report_families_materialized(
        workspace_root,
        &report_id,
        &selected_families(&suites),
        output,
    )?;

    let boot_rom_assets = match options.boot_rom_dir.as_deref() {
        Some(root) => {
            force_real_boot(&mut suites);
            let profiles = boot_rom_profiles(&suites);
            Some(
                load_verified_boot_rom_assets(root, &profiles)
                    .map_err(|error| format!("failed to load boot ROM assets: {error}"))?,
            )
        }
        None => {
            reject_manifest_real_boot_without_assets(&suites)?;
            None
        }
    };
    let run_config = LinkRunConfig { boot_rom_assets };

    let pool = if let Some(threads) = options.threads {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|error| format!("failed to configure rayon thread pool: {error}"))?;
        Some(pool)
    } else {
        None
    };

    let mut all_passed = true;
    for suite in &suites {
        writeln_checked(
            output,
            &format!(
                "suite {}: running {} cases",
                suite.suite_name,
                suite.cases.len()
            ),
        )?;
        let suite_report = if let Some(pool) = &pool {
            pool.install(|| run_link_suite_with_config(workspace_root, report, suite, &run_config))
        } else {
            run_link_suite_with_config(workspace_root, report, suite, &run_config)
        };
        write_link_suite_status(workspace_root, report, &suite_report)?;
        writeln_checked(
            output,
            &format!(
                "suite {}: {}/{} passed",
                suite_report.suite_name,
                suite_report.passed_count(),
                suite_report.cases.len()
            ),
        )?;
        for case in suite_report
            .cases
            .iter()
            .filter(|case| case.passed && !case.informational)
        {
            writeln_checked(
                output,
                &format!(
                    "case {}: PASS after {} T-cycles",
                    case.id, case.executed_tcycles
                ),
            )?;
        }
        for case in suite_report.cases.iter().filter(|case| case.informational) {
            writeln_checked(
                output,
                &format!(
                    "case {}: Informational after {} T-cycles",
                    case.id, case.executed_tcycles
                ),
            )?;
        }
        for case in suite_report.cases.iter().filter(|case| !case.passed) {
            writeln_checked(
                output,
                &format!(
                    "case {}: FAIL after {} T-cycles: {}",
                    case.id,
                    case.executed_tcycles,
                    case.failure.as_deref().unwrap_or("unknown failure")
                ),
            )?;
            if let Some(artifact_dir) = &case.failure_artifact_dir {
                writeln_checked(
                    output,
                    &format!("case {}: artifact_dir={}", case.id, artifact_dir.display()),
                )?;
            }
        }
        all_passed &= suite_report.all_passed();
    }

    if all_passed {
        Ok(())
    } else {
        Err("one or more linked suite cases failed".to_string())
    }
}

fn selected_families(suites: &[LinkSuiteManifest]) -> Vec<String> {
    let mut families = Vec::new();
    let mut seen = BTreeSet::new();
    for suite in suites {
        if seen.insert(suite.family.as_str()) {
            families.push(suite.family.clone());
        }
    }
    families
}

fn report_for_id<'a>(report_id: &str, reports: &'a [Report]) -> Result<&'a Report, String> {
    reports
        .iter()
        .find(|report| report.id == report_id)
        .ok_or_else(|| unknown_report_error(report_id, reports))
}

fn missing_report_error(reports: &[Report]) -> String {
    format!(
        "test ROM report must be provided; available reports: {}",
        available_reports(reports)
    )
}

fn unknown_report_error(report_id: &str, reports: &[Report]) -> String {
    format!(
        "unknown linked suite report {report_id:?}; available reports: {}",
        available_reports(reports)
    )
}

fn available_reports(reports: &[Report]) -> String {
    reports
        .iter()
        .map(|report| report.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn force_real_boot(suites: &mut [LinkSuiteManifest]) {
    for participant in suites
        .iter_mut()
        .flat_map(|suite| suite.cases.iter_mut())
        .flat_map(|case| case.participants.iter_mut())
    {
        participant.startup_mode = StartupMode::RealBoot;
    }
}

fn boot_rom_profiles(suites: &[LinkSuiteManifest]) -> Vec<BootRomProfile> {
    let mut profiles = Vec::new();
    for participant in suites
        .iter()
        .flat_map(|suite| suite.cases.iter())
        .flat_map(|case| case.participants.iter())
    {
        let profile = BootRomProfile::new(
            participant.console_model,
            participant.hardware_revision,
            participant.host_platform,
        );
        if !profiles.contains(&profile) {
            profiles.push(profile);
        }
    }
    profiles
}

fn reject_manifest_real_boot_without_assets(suites: &[LinkSuiteManifest]) -> Result<(), String> {
    let Some((case, participant)) = suites
        .iter()
        .flat_map(|suite| suite.cases.iter())
        .flat_map(|case| {
            case.participants
                .iter()
                .map(move |participant| (case, participant))
        })
        .find(|(_, participant)| participant.startup_mode == StartupMode::RealBoot)
    else {
        return Ok(());
    };
    Err(format!(
        "participant {:?} in case {:?} uses startup = \"real-boot\"; pass --boot-rom-dir <dir> to load verified boot ROM assets",
        participant.id, case.id
    ))
}

fn write_all<W: Write>(output: &mut W, text: &str) -> Result<(), String> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write linked suite output: {error}"))
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}")
        .map_err(|error| format!("failed to write linked suite output: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("failed to flush linked suite output: {error}"))
}

#[cfg(test)]
pub(super) fn parse_suite_link_arguments_for_test<I, S>(
    arguments: I,
) -> Result<SuiteLinkAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    parse_suite_link_arguments(arguments)
}

#[cfg(test)]
pub(super) fn run_suite_link_command_with_workspace_for_test<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_suite_link_command_with_workspace(arguments, workspace_root, output)
}
