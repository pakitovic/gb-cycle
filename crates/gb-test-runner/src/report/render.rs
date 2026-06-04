use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::model::{ReportDocument, report_status_display};

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

pub(super) fn render_html(document: &ReportDocument) -> String {
    let mut report = String::new();
    let _ = writeln!(&mut report, "<!doctype html>");
    let _ = writeln!(&mut report, "<html lang=\"en\">");
    let _ = writeln!(&mut report, "<head>");
    let _ = writeln!(&mut report, "<meta charset=\"utf-8\">");
    let _ = writeln!(
        &mut report,
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
    );
    let _ = writeln!(
        &mut report,
        "<title>gb-cycle ROM report - {}</title>",
        html_escape(&document.report_id)
    );
    let _ = writeln!(
        &mut report,
        "<style>body {{ font-family: system-ui, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif; margin: 2rem; color: #111; }} table {{ border-collapse: collapse; width: 100%; }} th, td {{ border: 1px solid #ccc; padding: .5rem; text-align: left; }} th {{ background: #f4f4f4; position: sticky; top: 0; }} code {{ background: #f4f4f4; padding: .1rem .25rem; }} .meta {{ color: #555; }} .status-pass {{ color: #12622a; }} .status-fail {{ color: #9f1d20; }} .status-info {{ color: #555; }}</style>"
    );
    let _ = writeln!(&mut report, "</head>");
    let _ = writeln!(&mut report, "<body>");
    let _ = writeln!(
        &mut report,
        "<h1>gb-cycle ROM report: {}</h1>",
        html_escape(&document.report_id)
    );
    let _ = writeln!(
        &mut report,
        "<p class=\"meta\">Summary: <strong>{}/{}</strong>. Command: <code>{}</code>.</p>",
        document.non_failing_cases,
        document.total_cases,
        html_escape(&document.command)
    );
    let _ = writeln!(&mut report, "<table>");
    let _ = writeln!(
        &mut report,
        "<thead><tr><th>family</th><th>rom</th><th>status</th></tr></thead>"
    );
    let _ = writeln!(&mut report, "<tbody>");
    if document.rows.is_empty() {
        let _ = writeln!(
            &mut report,
            "<tr><td colspan=\"3\">No status rows found.</td></tr>"
        );
    } else {
        for row in &document.rows {
            let _ = writeln!(
                &mut report,
                "<tr><td>{}</td><td>{}</td><td class=\"{}\">{}</td></tr>",
                html_escape(&row.family),
                html_escape(&row.rom),
                html_escape(status_class(&row.status)),
                report_status_display(&row.status).expect("document rows are validated")
            );
        }
    }
    let _ = writeln!(&mut report, "</tbody>");
    let _ = writeln!(&mut report, "</table>");
    let _ = writeln!(&mut report, "</body>");
    let _ = writeln!(&mut report, "</html>");
    report
}

pub(super) fn html_report_path(markdown_path: &Path) -> PathBuf {
    let mut path = markdown_path.to_path_buf();
    path.set_extension("html");
    path
}

fn markdown_cell(value: &str) -> String {
    value.replace('\n', " ").replace('|', "\\|")
}

fn html_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn status_class(status: &str) -> &'static str {
    match status {
        "PASS" => "status-pass",
        "FAIL" => "status-fail",
        "INFO" => "status-info",
        _ => "status-info",
    }
}
