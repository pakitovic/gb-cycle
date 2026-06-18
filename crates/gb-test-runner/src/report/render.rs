use std::fmt::Write as _;

use askama::Template;

use super::model::{
    REPORT_STATUS_FAIL_EMOJI, REPORT_STATUS_PASS_EMOJI, ReportDocument, ReportSummary,
    report_status_display,
};

#[derive(Debug, Clone, Template)]
#[template(path = "report/index.html")]
pub(super) struct ReportIndexDocument {
    pub(super) generated_at_epoch_seconds: u64,
    pub(super) generated_at_datetime: String,
    pub(super) generated_at_utc: String,
    pub(super) rows: Vec<ReportIndexRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportIndexRow {
    pub(super) report_id: String,
    pub(super) href: String,
    pub(super) non_failing_cases: usize,
    pub(super) total_cases: usize,
    pub(super) status_emoji: &'static str,
    pub(super) status_class: &'static str,
}

#[derive(Debug, Clone, Template)]
#[template(path = "report/report.html")]
struct ReportHtmlDocument {
    report_id: String,
    command: String,
    non_failing_cases: usize,
    total_cases: usize,
    rows: Vec<ReportHtmlRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportHtmlRow {
    family: String,
    rom: String,
    status_display: &'static str,
    status_class: &'static str,
}

impl ReportIndexRow {
    pub(super) fn new(summary: ReportSummary) -> Self {
        let all_non_failing = summary.all_non_failing();
        Self {
            href: format!("reports/{}/index.html", summary.report_id),
            report_id: summary.report_id,
            non_failing_cases: summary.non_failing_cases,
            total_cases: summary.total_cases,
            status_emoji: if all_non_failing {
                REPORT_STATUS_PASS_EMOJI
            } else {
                REPORT_STATUS_FAIL_EMOJI
            },
            status_class: if all_non_failing {
                "status-pass"
            } else {
                "status-fail"
            },
        }
    }
}

impl ReportHtmlDocument {
    fn from_report_document(document: &ReportDocument) -> Result<Self, String> {
        let mut rows = Vec::with_capacity(document.rows.len());
        for row in &document.rows {
            rows.push(ReportHtmlRow {
                family: row.family.clone(),
                rom: row.rom.clone(),
                status_display: report_status_display(&row.status)?,
                status_class: status_class(&row.status),
            });
        }
        Ok(Self {
            report_id: document.report_id.clone(),
            command: document.command.clone(),
            non_failing_cases: document.non_failing_cases,
            total_cases: document.total_cases,
            rows,
        })
    }
}

pub(super) fn render_markdown(document: &ReportDocument) -> String {
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Test Report: {} ({}/{})",
        document.report_id, document.non_failing_cases, document.total_cases
    );
    let _ = writeln!(&mut report);
    let _ = writeln!(&mut report, "Command: `{}`", document.command);
    let _ = writeln!(&mut report);
    let _ = writeln!(&mut report, "| family | rom | status |");
    let _ = writeln!(&mut report, "| --- | --- | --- |");
    for row in &document.rows {
        let _ = writeln!(
            &mut report,
            "| {} | {} | {} |",
            markdown_cell(&row.family),
            markdown_cell(&row.rom),
            report_status_display(&row.status).expect("document rows are validated")
        );
    }
    report
}

pub(super) fn render_html(document: &ReportDocument) -> Result<String, String> {
    ReportHtmlDocument::from_report_document(document)?
        .render()
        .map_err(|error| format!("failed to render HTML test ROM report: {error}"))
}

pub(super) fn render_index(document: &ReportIndexDocument) -> Result<String, String> {
    document
        .render()
        .map_err(|error| format!("failed to render ROM report index: {error}"))
}

fn markdown_cell(value: &str) -> String {
    value.replace('\n', " ").replace('|', "\\|")
}

fn status_class(status: &str) -> &'static str {
    match status {
        "PASS" => "status-pass",
        "FAIL" => "status-fail",
        "INFO" => "status-info",
        _ => "status-info",
    }
}
