use super::super::model::{
    REPORT_STATUS_FAIL_EMOJI, REPORT_STATUS_INFO_EMOJI, REPORT_STATUS_PASS_EMOJI, ReportDocument,
    ReportRow,
};
use super::super::render::{render_html, render_markdown};

#[test]
fn renderers_share_validated_counts_and_status_display() {
    let document = ReportDocument {
        report_id: "sample-report".to_string(),
        command: "cargo rom-report sample-report".to_string(),
        non_failing_cases: 2,
        total_cases: 3,
        rows: vec![
            ReportRow {
                family: "acid".to_string(),
                rom: "which.gb".to_string(),
                status: "INFO".to_string(),
                suite_name: "acid".to_string(),
                case_index: 0,
            },
            ReportRow {
                family: "blargg".to_string(),
                rom: "halt_bug.gb".to_string(),
                status: "PASS".to_string(),
                suite_name: "blargg".to_string(),
                case_index: 0,
            },
            ReportRow {
                family: "mooneye".to_string(),
                rom: "acceptance/div_timing.gb".to_string(),
                status: "FAIL".to_string(),
                suite_name: "mooneye".to_string(),
                case_index: 0,
            },
        ],
    };

    let markdown = render_markdown(&document);
    let html = render_html(&document);

    assert!(markdown.contains("# Test Report: sample-report (2/3)"));
    assert!(markdown.contains(REPORT_STATUS_PASS_EMOJI));
    assert!(markdown.contains(REPORT_STATUS_FAIL_EMOJI));
    assert!(markdown.contains(REPORT_STATUS_INFO_EMOJI));
    assert!(html.contains("Summary: <strong>2/3</strong>"));
    assert!(html.contains("status-pass"));
    assert!(html.contains("status-fail"));
    assert!(html.contains("status-info"));
}
