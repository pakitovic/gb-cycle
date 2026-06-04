use std::fs;
use std::io::Write;
use std::path::Path;

use crate::default_workspace_root;

use super::manifest::load_reports;
use super::model::{PersistedSuiteStatus, Report};
use super::render::{html_report_path, render_html, render_markdown};
use super::status::{build_report_document, load_statuses, store_root_for_report};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReportAction {
    ShowHelp,
    Run(ReportOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportOptions {
    report_id: Option<String>,
    html: bool,
}

pub fn report_help_text() -> &'static str {
    concat!(
        "Usage: cargo run -p gb-test-runner --bin report -- <report-id> [--html]\n",
        "\n",
        "Renders a report-local test ROM status snapshot into test/<report>/test-report.md.\n",
        "If test/<report>/.status is missing or empty, this command first runs cargo rom-suite <report-id>.\n",
    )
}

pub fn run_report_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_report_command_with_workspace(arguments, &default_workspace_root(), output)
}

fn run_report_command_with_workspace<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_report_arguments(arguments)? {
        ReportAction::ShowHelp => write_all(output, report_help_text()),
        ReportAction::Run(options) => run_options(options, workspace_root, output),
    }
}

fn parse_report_arguments<I, S>(arguments: I) -> Result<ReportAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut report_id = None;
    let mut html = false;
    for argument in arguments {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(ReportAction::ShowHelp),
            "--html" => html = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown report option {value:?}; run with --help"));
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

    Ok(ReportAction::Run(ReportOptions { report_id, html }))
}

fn run_options<W: Write>(
    options: ReportOptions,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String> {
    let reports = load_reports(workspace_root)?;
    let Some(report_id) = options.report_id else {
        return Err(missing_report_error(&reports));
    };
    let report = report_for_id(&report_id, &reports)?;
    let statuses = load_or_create_statuses(workspace_root, report, output)?;
    let document = build_report_document(workspace_root, report, statuses)?;
    write_report_files(workspace_root, report, &document, options.html, output)
}

fn load_or_create_statuses<W: Write>(
    workspace_root: &Path,
    report: &Report,
    output: &mut W,
) -> Result<Vec<PersistedSuiteStatus>, String> {
    let mut statuses = load_statuses(workspace_root, report)?;
    if !statuses.is_empty() {
        return Ok(statuses);
    }

    writeln_checked(
        output,
        &format!(
            "rom-report: no status files found for {}; running cargo rom-suite {}",
            report.id, report.id
        ),
    )?;
    let suite_result = crate::suite::run_suite_command_with_workspace(
        [report.id.as_str()],
        workspace_root,
        output,
    );
    if let Err(error) = &suite_result {
        writeln_checked(
            output,
            &format!(
                "rom-report: cargo rom-suite {} returned: {error}; rendering any written status",
                report.id
            ),
        )?;
    }

    statuses = load_statuses(workspace_root, report)?;
    if statuses.is_empty() {
        return match suite_result {
            Ok(()) => Err(format!(
                "cargo rom-suite {} did not write status files",
                report.id
            )),
            Err(error) => Err(format!(
                "failed to generate status for report {:?}; cargo rom-suite {} failed: {error}",
                report.id, report.id
            )),
        };
    }
    Ok(statuses)
}

fn write_report_files<W: Write>(
    workspace_root: &Path,
    report: &Report,
    document: &super::model::ReportDocument,
    html: bool,
    output: &mut W,
) -> Result<(), String> {
    let store_root = store_root_for_report(workspace_root, report);
    fs::create_dir_all(&store_root).map_err(|error| {
        format!(
            "failed to create report directory {}: {error}",
            store_root.display()
        )
    })?;

    let markdown_path = store_root.join(&report.report_file);
    fs::write(&markdown_path, render_markdown(document)).map_err(|error| {
        format!(
            "failed to write test ROM report {}: {error}",
            markdown_path.display()
        )
    })?;
    writeln_checked(output, &format!("wrote {}", markdown_path.display()))?;

    if html {
        let html_path = html_report_path(&markdown_path);
        fs::write(&html_path, render_html(document)).map_err(|error| {
            format!(
                "failed to write HTML test ROM report {}: {error}",
                html_path.display()
            )
        })?;
        writeln_checked(output, &format!("wrote {}", html_path.display()))?;
    }

    Ok(())
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
        "unknown test ROM report {report_id:?}; available reports: {}",
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

fn write_all<W: Write>(output: &mut W, text: &str) -> Result<(), String> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write report output: {error}"))
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}")
        .map_err(|error| format!("failed to write report output: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("failed to flush report output: {error}"))
}

#[cfg(test)]
pub(super) fn parse_report_arguments_for_test<I, S>(arguments: I) -> Result<ReportAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    parse_report_arguments(arguments)
}

#[cfg(test)]
pub(super) fn run_report_command_with_workspace_for_test<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_report_command_with_workspace(arguments, workspace_root, output)
}
