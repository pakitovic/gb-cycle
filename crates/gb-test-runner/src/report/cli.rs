use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::default_workspace_root;

use super::manifest::load_reports;
use super::model::{
    PersistedSuiteStatus, ROM_REPORTS_PAGES_PATH, Report, ReportSummary, RomReportsPageEntry,
};
use super::render::{
    ReportIndexDocument, ReportIndexRow, render_html, render_index, render_markdown,
};
use super::status::{build_report_document, load_statuses, store_root_for_report};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReportAction {
    ShowHelp,
    Run(ReportOptions),
    Index(ReportIndexOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportOptions {
    report_id: Option<String>,
    html: bool,
    boot_rom_dir: Option<PathBuf>,
    force_real_boot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportIndexOptions {
    output_dir: PathBuf,
}

pub fn report_help_text() -> &'static str {
    concat!(
        "Usage: cargo run -p gb-test-runner --bin report -- <report-id> [--html] [--boot-rom-dir <dir>] [--force-real-boot]\n",
        "       cargo run -p gb-test-runner --bin report -- --index <dir>\n",
        "\n",
        "Validates that <report-id> has single-machine suites, runs cargo rom-suite <report-id>,\n",
        "and renders the fresh report-local status snapshot into test/<report>/test-report.md; --html also writes test/<report>/.status/index.html.\n",
        "rom-suite owns guarded runtime cleanup after preflight.\n",
        "--index <dir> passively publishes materialized test/<report>/.status/index.html pages ordered by crates/gb-test-runner/data/rom-reports-pages.json.\n",
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
        ReportAction::Index(options) => run_index_options(options, workspace_root, output),
    }
}

fn parse_report_arguments<I, S>(arguments: I) -> Result<ReportAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut report_id = None;
    let mut html = false;
    let mut boot_rom_dir = None;
    let mut force_real_boot = false;
    let mut index = false;
    let mut index_output_dir = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(ReportAction::ShowHelp),
            "--html" => html = true,
            "--index" => index = true,
            "--site-dir" => {
                let Some(_value) = arguments.next() else {
                    return Err("--site-dir requires a value".to_string());
                };
                return Err("--site-dir is not supported; use --index <dir>".to_string());
            }
            "--report" => {
                let Some(_value) = arguments.next() else {
                    return Err("--report requires a value".to_string());
                };
                return Err(
                    "--report is not supported; --index uses rom-reports-pages.json".to_string(),
                );
            }
            "--boot-rom-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--boot-rom-dir requires a value".to_string());
                };
                boot_rom_dir = Some(PathBuf::from(value.as_ref()));
            }
            "--force-real-boot" => force_real_boot = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown report option {value:?}; run with --help"));
            }
            value => {
                if index {
                    if index_output_dir.is_some() {
                        return Err(format!(
                            "unexpected extra positional argument {value:?}; run with --help"
                        ));
                    }
                    index_output_dir = Some(PathBuf::from(value));
                } else if report_id.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument {value:?}; run with --help"
                    ));
                } else {
                    report_id = Some(value.to_string());
                }
            }
        }
    }
    if index {
        if report_id.is_some() {
            return Err("--index cannot be combined with <report-id>".to_string());
        }
        if html {
            return Err("--index cannot be combined with --html".to_string());
        }
        if boot_rom_dir.is_some() {
            return Err("--index cannot be combined with --boot-rom-dir".to_string());
        }
        if force_real_boot {
            return Err("--index cannot be combined with --force-real-boot".to_string());
        }
        let Some(output_dir) = index_output_dir else {
            return Err("--index requires <dir>".to_string());
        };
        return Ok(ReportAction::Index(ReportIndexOptions { output_dir }));
    }
    if force_real_boot && boot_rom_dir.is_none() {
        return Err("--force-real-boot requires --boot-rom-dir <dir>".to_string());
    }

    Ok(ReportAction::Run(ReportOptions {
        report_id,
        html,
        boot_rom_dir,
        force_real_boot,
    }))
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
    ensure_single_machine_suite_manifests(workspace_root, report)?;
    let statuses = run_suite_and_load_statuses(
        workspace_root,
        report,
        options.boot_rom_dir.as_deref(),
        options.force_real_boot,
        output,
    )?;
    let document = build_report_document(
        workspace_root,
        report,
        statuses,
        options.boot_rom_dir.as_deref(),
        options.force_real_boot,
    )?;
    write_report_files(workspace_root, report, &document, options.html, output)
}

fn run_index_options<W: Write>(
    options: ReportIndexOptions,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String> {
    let document = build_index_document(&options, workspace_root, output)?;
    let index = render_index(&document)?;
    fs::create_dir_all(&options.output_dir).map_err(|error| {
        format!(
            "failed to create ROM report site directory {}: {error}",
            options.output_dir.display()
        )
    })?;
    let index_path = options.output_dir.join("index.html");
    fs::write(&index_path, index).map_err(|error| {
        format!(
            "failed to write ROM report index {}: {error}",
            index_path.display()
        )
    })?;
    writeln_checked(output, &format!("wrote {}", index_path.display()))
}

fn build_index_document<W: Write>(
    options: &ReportIndexOptions,
    workspace_root: &Path,
    output: &mut W,
) -> Result<ReportIndexDocument, String> {
    let configured_reports = load_rom_reports_pages(workspace_root)?;
    let reports = load_reports(workspace_root)?;
    let reports_dir = options.output_dir.join("reports");
    if reports_dir.exists() {
        fs::remove_dir_all(&reports_dir).map_err(|error| {
            format!(
                "failed to remove stale ROM report pages directory {}: {error}",
                reports_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&reports_dir).map_err(|error| {
        format!(
            "failed to create ROM report pages directory {}: {error}",
            reports_dir.display()
        )
    })?;

    let mut rows = Vec::new();
    for configured_report in configured_reports {
        let _boot_roms = configured_report.boot_roms;
        let report = report_for_id(&configured_report.name, &reports)?;
        if let Some(row) = materialize_index_row(workspace_root, report, &options.output_dir)? {
            rows.push(row);
        } else {
            writeln_checked(
                output,
                &format!(
                    "skipped {}; missing materialized .status/index.html or .status/summary.json",
                    configured_report.name
                ),
            )?;
        }
    }
    let generated_at = generated_at_timestamp();
    Ok(ReportIndexDocument {
        generated_at_epoch_seconds: generated_at.epoch_seconds,
        generated_at_datetime: generated_at.datetime,
        generated_at_utc: generated_at.utc,
        rows,
    })
}

fn load_rom_reports_pages(workspace_root: &Path) -> Result<Vec<RomReportsPageEntry>, String> {
    let path = workspace_root.join(ROM_REPORTS_PAGES_PATH);
    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read ROM reports Pages metadata {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_str(&text).map_err(|error| {
        format!(
            "failed to parse ROM reports Pages metadata {}: {error}",
            path.display()
        )
    })
}

fn materialize_index_row(
    workspace_root: &Path,
    report: &Report,
    output_dir: &Path,
) -> Result<Option<ReportIndexRow>, String> {
    let status_dir = store_root_for_report(workspace_root, report).join(&report.status_dir);
    let report_html = status_dir.join("index.html");
    let summary_path = status_dir.join("summary.json");
    if !report_html.is_file() || !summary_path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&summary_path).map_err(|error| {
        format!(
            "failed to read ROM report summary {}: {error}",
            summary_path.display()
        )
    })?;
    let summary: ReportSummary = serde_json::from_str(&text).map_err(|error| {
        format!(
            "failed to parse ROM report summary {}: {error}",
            summary_path.display()
        )
    })?;
    if summary.report_id != report.id {
        return Err(format!(
            "ROM report summary {} has report_id {:?}, expected {:?}",
            summary_path.display(),
            summary.report_id,
            report.id
        ));
    }
    let report_site_dir = output_dir.join("reports").join(&report.id);
    fs::create_dir_all(&report_site_dir).map_err(|error| {
        format!(
            "failed to create ROM report page directory {}: {error}",
            report_site_dir.display()
        )
    })?;
    let copied_html = report_site_dir.join("index.html");
    fs::copy(&report_html, &copied_html).map_err(|error| {
        format!(
            "failed to copy ROM report page {} to {}: {error}",
            report_html.display(),
            copied_html.display()
        )
    })?;
    Ok(Some(ReportIndexRow::new(summary)))
}

fn run_suite_and_load_statuses<W: Write>(
    workspace_root: &Path,
    report: &Report,
    boot_rom_dir: Option<&Path>,
    force_real_boot: bool,
    output: &mut W,
) -> Result<Vec<PersistedSuiteStatus>, String> {
    let suite_command = suite_command_display(report, boot_rom_dir, force_real_boot);
    writeln_checked(
        output,
        &format!(
            "rom-report: running {suite_command}; rom-suite will clear selected single-machine status and artifacts after preflight",
        ),
    )?;
    let suite_arguments = suite_arguments(report, boot_rom_dir, force_real_boot);
    let mut suite_runtime_cleaned = false;
    let suite_result = crate::suite::run_suite_command_with_workspace_tracking_cleanup(
        suite_arguments.iter().map(String::as_str),
        workspace_root,
        output,
        &mut suite_runtime_cleaned,
    );
    if let Err(error) = &suite_result {
        if !suite_runtime_cleaned {
            return Err(format!(
                "failed to generate status for report {:?}; {suite_command} failed before runtime cleanup: {error}",
                report.id
            ));
        }
        writeln_checked(
            output,
            &format!(
                "rom-report: {suite_command} returned after runtime cleanup: {error}; rendering written status",
            ),
        )?;
    }

    let statuses = load_statuses(workspace_root, report)?;
    if statuses.is_empty() {
        return match suite_result {
            Ok(()) => Err(format!(
                "cargo rom-suite {} did not write status files",
                report.id
            )),
            Err(error) => Err(format!(
                "failed to generate status for report {:?}; {suite_command} failed: {error}",
                report.id
            )),
        };
    }
    Ok(statuses)
}

fn suite_arguments(
    report: &Report,
    boot_rom_dir: Option<&Path>,
    force_real_boot: bool,
) -> Vec<String> {
    let mut arguments = vec![report.id.clone()];
    if let Some(boot_rom_dir) = boot_rom_dir {
        arguments.push("--boot-rom-dir".to_string());
        arguments.push(boot_rom_dir.display().to_string());
    }
    if force_real_boot {
        arguments.push("--force-real-boot".to_string());
    }
    arguments
}

fn suite_command_display(
    report: &Report,
    boot_rom_dir: Option<&Path>,
    force_real_boot: bool,
) -> String {
    let mut command = format!("cargo rom-suite {}", report.id);
    if let Some(boot_rom_dir) = boot_rom_dir {
        command.push_str(" --boot-rom-dir ");
        command.push_str(&boot_rom_dir.display().to_string());
    }
    if force_real_boot {
        command.push_str(" --force-real-boot");
    }
    command
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

    let summary_path = report_status_summary_path(&store_root, report);
    let summary_dir = summary_path
        .parent()
        .expect("report status summary path should have parent");
    fs::create_dir_all(summary_dir).map_err(|error| {
        format!(
            "failed to create test ROM report summary directory {}: {error}",
            summary_dir.display()
        )
    })?;
    let summary_text =
        serde_json::to_string_pretty(&ReportSummary::from_document(document)).map_err(|error| {
            format!(
                "failed to serialize test ROM report summary {}: {error}",
                summary_path.display()
            )
        })? + "\n";
    fs::write(&summary_path, summary_text).map_err(|error| {
        format!(
            "failed to write test ROM report summary {}: {error}",
            summary_path.display()
        )
    })?;
    writeln_checked(output, &format!("wrote {}", summary_path.display()))?;

    if html {
        let html_path = store_root.join(&report.status_dir).join("index.html");
        fs::create_dir_all(
            html_path
                .parent()
                .expect("HTML report path should have parent"),
        )
        .map_err(|error| {
            format!(
                "failed to create HTML test ROM report directory {}: {error}",
                html_path
                    .parent()
                    .expect("HTML report path should have parent")
                    .display()
            )
        })?;
        fs::write(&html_path, render_html(document)?).map_err(|error| {
            format!(
                "failed to write HTML test ROM report {}: {error}",
                html_path.display()
            )
        })?;
        writeln_checked(output, &format!("wrote {}", html_path.display()))?;
    }

    Ok(())
}

fn report_status_summary_path(store_root: &Path, report: &Report) -> PathBuf {
    store_root.join(&report.status_dir).join("summary.json")
}

fn ensure_single_machine_suite_manifests(
    workspace_root: &Path,
    report: &Report,
) -> Result<(), String> {
    let report_data_dir = report_data_dir(workspace_root, report);
    let entries = fs::read_dir(&report_data_dir).map_err(|error| {
        format!(
            "failed to read suite manifest directory {}: {error}",
            report_data_dir.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read suite manifest directory {}: {error}",
                report_data_dir.display()
            )
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            continue;
        };
        if is_single_machine_suite_manifest(file_name) {
            return Ok(());
        }
    }
    Err(format!(
        "report {:?} does not contain single-machine suite manifests",
        report.id
    ))
}

fn report_data_dir(workspace_root: &Path, report: &Report) -> PathBuf {
    let source_parent = report
        .sources
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(&report.store_dir);
    workspace_root
        .join(super::model::DATA_DIR)
        .join(source_parent)
}

fn is_single_machine_suite_manifest(file_name: &str) -> bool {
    file_name.ends_with(".suite.toml") && !file_name.ends_with(".link.suite.toml")
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedAtTimestamp {
    epoch_seconds: u64,
    datetime: String,
    utc: String,
}

fn generated_at_timestamp() -> GeneratedAtTimestamp {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    generated_at_timestamp_from_epoch(seconds)
}

fn generated_at_timestamp_from_epoch(seconds: u64) -> GeneratedAtTimestamp {
    let (date, time) = utc_date_time_from_epoch(seconds);
    GeneratedAtTimestamp {
        epoch_seconds: seconds,
        datetime: format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            date.year, date.month, date.day, time.hour, time.minute, time.second
        ),
        utc: format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            date.year, date.month, date.day, time.hour, time.minute, time.second
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UtcDate {
    year: i64,
    month: u32,
    day: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UtcTime {
    hour: u64,
    minute: u64,
    second: u64,
}

fn utc_date_time_from_epoch(seconds: u64) -> (UtcDate, UtcTime) {
    const SECONDS_PER_DAY: u64 = 86_400;
    let days = (seconds / SECONDS_PER_DAY) as i64;
    let seconds_in_day = seconds % SECONDS_PER_DAY;
    let (year, month, day) = civil_from_days(days);
    (
        UtcDate { year, month, day },
        UtcTime {
            hour: seconds_in_day / 3_600,
            minute: (seconds_in_day % 3_600) / 60,
            second: seconds_in_day % 60,
        },
    )
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u32, u32) {
    let days = days_since_unix_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_parameter = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_parameter + 2) / 5 + 1;
    let month = month_parameter + if month_parameter < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month as u32, day as u32)
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
