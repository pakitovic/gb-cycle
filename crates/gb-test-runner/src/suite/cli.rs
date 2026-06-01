use std::io::Write;
use std::path::Path;

use super::manifest::{load_reports, load_selected_suites};
use super::run::run_suite;
use super::status::write_suite_status;
use crate::default_workspace_root;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SuiteAction {
    ShowHelp,
    Run(SuiteOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SuiteOptions {
    report_id: Option<String>,
    suite_name: Option<String>,
    case_id: Option<String>,
    threads: Option<usize>,
}

pub fn suite_help_text() -> &'static str {
    concat!(
        "Usage: cargo run -p gb-test-runner --bin suite -- <report-id> [--suite <suite-name>] [--case <case-id>] [--threads <n>]\n",
        "\n",
        "Runs report-local *.suite.toml test ROM manifests through the new minimal suite runner.\n",
    )
}

pub fn run_suite_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_suite_command_with_workspace(arguments, &default_workspace_root(), output)
}

fn run_suite_command_with_workspace<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_suite_arguments(arguments)? {
        SuiteAction::ShowHelp => write_all(output, suite_help_text()),
        SuiteAction::Run(options) => run_options(options, workspace_root, output),
    }
}

fn parse_suite_arguments<I, S>(arguments: I) -> Result<SuiteAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut report_id = None;
    let mut suite_name = None;
    let mut case_id = None;
    let mut threads = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(SuiteAction::ShowHelp),
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

    Ok(SuiteAction::Run(SuiteOptions {
        report_id,
        suite_name,
        case_id,
        threads,
    }))
}

fn run_options<W: Write>(
    options: SuiteOptions,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String> {
    let reports = load_reports(workspace_root)?;
    let Some(report_id) = options.report_id else {
        return Err(missing_report_error(&reports));
    };
    let report = report_for_id(&report_id, &reports)?;
    let suites = load_selected_suites(
        workspace_root,
        report,
        options.suite_name.as_deref(),
        options.case_id.as_deref(),
    )?;
    if suites.is_empty() {
        return Err(format!(
            "report {report_id:?} does not contain suite manifests"
        ));
    }

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
            pool.install(|| run_suite(workspace_root, report, suite))
        } else {
            run_suite(workspace_root, report, suite)
        };
        write_suite_status(workspace_root, report, &suite_report)?;
        writeln_checked(
            output,
            &format!(
                "suite {}: {}/{} passed",
                suite_report.suite_name,
                suite_report.passed_count(),
                suite_report.cases.len()
            ),
        )?;
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
        Err("one or more suite cases failed".to_string())
    }
}

fn report_for_id<'a>(
    report_id: &str,
    reports: &'a [super::model::Report],
) -> Result<&'a super::model::Report, String> {
    reports
        .iter()
        .find(|report| report.id == report_id)
        .ok_or_else(|| unknown_report_error(report_id, reports))
}

fn missing_report_error(reports: &[super::model::Report]) -> String {
    format!(
        "test ROM report must be provided; available reports: {}",
        available_reports(reports)
    )
}

fn unknown_report_error(report_id: &str, reports: &[super::model::Report]) -> String {
    format!(
        "unknown suite report {report_id:?}; available reports: {}",
        available_reports(reports)
    )
}

fn available_reports(reports: &[super::model::Report]) -> String {
    reports
        .iter()
        .map(|report| report.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_all<W: Write>(output: &mut W, text: &str) -> Result<(), String> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write suite output: {error}"))
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}").map_err(|error| format!("failed to write suite output: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("failed to flush suite output: {error}"))
}

#[cfg(test)]
pub(super) fn parse_suite_arguments_for_test<I, S>(arguments: I) -> Result<SuiteAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    parse_suite_arguments(arguments)
}

#[cfg(test)]
pub(super) fn run_suite_command_with_workspace_for_test<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_suite_command_with_workspace(arguments, workspace_root, output)
}
