use std::io::Write;
use std::path::Path;

use super::git::{cleanup_fetched_sources, fetch_sources_into_temps};
use super::manifest::{
    Report, filter_sources_for_families, load_report_manifest, load_source_manifest,
    report_families, select_families,
};
use super::materialize::{
    materialize_selected_sources, replace_selected_family_roots, store_root_for_report,
};
use super::validate::validate_materialization_targets;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FetchAction {
    ShowHelp,
    Fetch(FetchRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FetchRequest {
    pub(super) report_id: Option<String>,
    pub(super) requested_families: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FetchOptions<'a> {
    pub(super) report: &'a Report,
    pub(super) requested_families: Vec<String>,
}

pub fn fetch_help_text() -> &'static str {
    concat!(
        "Usage: cargo run -p gb-test-runner --bin fetch -- <report-id> [family ...]\n",
        "\n",
        "Fetches pinned upstream ROM source(s) through the report registry, verifies SHA-256 hashes, materializes selected families under test/<report-store>, and removes temporary checkout(s).\n",
        "Report ids are read from crates/gb-test-runner/data/reports.toml.\n",
    )
}

pub fn run_fetch_command<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_fetch_arguments(arguments)? {
        FetchAction::ShowHelp => write_all(output, fetch_help_text()),
        FetchAction::Fetch(request) => run_fetch_request(request, workspace_root, output),
    }
}

pub(super) fn parse_fetch_arguments<I, S>(arguments: I) -> Result<FetchAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut report_id = None;
    let mut requested_families = Vec::new();

    for argument in arguments {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(FetchAction::ShowHelp),
            "--report" => {
                return Err("fetch expects the report as the first positional argument".to_string());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown fetch option {other:?}"));
            }
            other if report_id.is_none() => report_id = Some(other.to_string()),
            other => requested_families.push(other.to_string()),
        }
    }

    Ok(FetchAction::Fetch(FetchRequest {
        report_id,
        requested_families,
    }))
}

fn run_fetch_request<W: Write>(
    request: FetchRequest,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String> {
    let reports = load_report_manifest(workspace_root)?;
    let options = resolve_fetch_options(request, &reports.reports)?;
    let report = options.report;
    let source_manifest = load_source_manifest(workspace_root, report)?;
    let available_families = report_families(report, &source_manifest)?;
    let selected_families =
        select_families(report, &available_families, &options.requested_families)?;
    let filtered_sources =
        filter_sources_for_families(&source_manifest.sources, report, &selected_families)?;
    validate_materialization_targets(report, &filtered_sources)?;

    let fetched_sources = fetch_sources_into_temps(&filtered_sources, output)?;
    let store_root = store_root_for_report(workspace_root, report);
    let result = (|| {
        replace_selected_family_roots(&store_root, report, &filtered_sources)?;
        materialize_selected_sources(&store_root, &fetched_sources)?;
        writeln_checked(
            output,
            &format!(
                "materialized test ROM families {} into {}",
                selected_families.join(", "),
                store_root.display()
            ),
        )?;
        Ok(())
    })();
    let cleanup = cleanup_fetched_sources(&fetched_sources);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(format!("{error}; additionally {cleanup_error}")),
    }
}

pub(super) fn resolve_fetch_options<'a>(
    request: FetchRequest,
    reports: &'a [Report],
) -> Result<FetchOptions<'a>, String> {
    let Some(report_id) = request.report_id else {
        return Err(missing_report_error(reports));
    };
    let report = report_for_id(&report_id, reports)?;
    Ok(FetchOptions {
        report,
        requested_families: request.requested_families,
    })
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
        .map_err(|error| format!("failed to write command output: {error}"))
}

pub(super) fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}").map_err(|error| format!("failed to write command output: {error}"))
}
