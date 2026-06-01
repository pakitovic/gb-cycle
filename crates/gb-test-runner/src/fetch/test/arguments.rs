use std::path::Path;

use super::super::cli::{FetchAction, FetchRequest, parse_fetch_arguments, resolve_fetch_options};
use super::super::{fetch_help_text, run_fetch_command};
use super::common::basic_report;

#[test]
fn help_mentions_reports_registry() {
    let help = fetch_help_text();
    assert!(help.contains("fetch"));
    assert!(help.contains("reports.toml"));
}

#[test]
fn parses_help_without_manifests() {
    let mut output = Vec::new();
    run_fetch_command(["--help"], Path::new("."), &mut output)
        .expect("help should not need manifests");
    let output = String::from_utf8(output).expect("help should be utf-8");
    assert!(output.contains("Usage:"));
    assert!(output.contains("fetch"));
}

#[test]
fn parse_accepts_all_and_null_as_regular_family_names() {
    assert_eq!(
        parse_fetch_arguments(["sample-report", "all", "null"]).expect("arguments should parse"),
        FetchAction::Fetch(FetchRequest {
            report_id: Some("sample-report".to_string()),
            requested_families: vec!["all".to_string(), "null".to_string()],
        })
    );
}

#[test]
fn parse_rejects_flagged_report() {
    assert!(
        parse_fetch_arguments(["--report", "sample-report", "family-a"])
            .expect_err("flagged report should fail")
            .contains("first positional")
    );
}

#[test]
fn parse_accepts_empty_report_request_for_contextual_resolution() {
    assert_eq!(
        parse_fetch_arguments(std::iter::empty::<&str>()).expect("empty request should parse"),
        FetchAction::Fetch(FetchRequest {
            report_id: None,
            requested_families: Vec::new(),
        })
    );
}

#[test]
fn resolve_requires_report_and_lists_available_reports() {
    let reports = vec![basic_report()];
    let error = resolve_fetch_options(
        FetchRequest {
            report_id: None,
            requested_families: Vec::new(),
        },
        &reports,
    )
    .expect_err("missing report should fail");
    assert!(error.contains("test ROM report must be provided"));
    assert!(error.contains("available reports: sample-report"));
}

#[test]
fn resolve_accepts_report_without_family_selection() {
    let reports = vec![basic_report()];
    let options = resolve_fetch_options(
        FetchRequest {
            report_id: Some("sample-report".to_string()),
            requested_families: Vec::new(),
        },
        &reports,
    )
    .expect("report without families should resolve");
    assert_eq!(options.report.id, "sample-report");
    assert!(options.requested_families.is_empty());
}

#[test]
fn resolve_accepts_report_and_ordered_family_selection() {
    let reports = vec![basic_report()];
    let action =
        parse_fetch_arguments(["sample-report", "family-b"]).expect("arguments should parse");
    let FetchAction::Fetch(request) = action else {
        panic!("arguments should resolve to fetch request");
    };
    let options = resolve_fetch_options(request, &reports).expect("request should resolve");
    assert_eq!(options.report.id, "sample-report");
    assert_eq!(options.requested_families, vec!["family-b"]);
}
