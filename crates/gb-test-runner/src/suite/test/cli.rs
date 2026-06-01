use super::super::cli::{parse_suite_arguments_for_test, run_suite_command, suite_help_text};

#[test]
fn help_mentions_suite_contract() {
    let help = suite_help_text();
    assert!(help.contains("<report-id>"));
    assert!(help.contains("--suite <suite-name>"));
    assert!(help.contains("--case <case-id>"));
}

#[test]
fn parse_accepts_report_and_suite_case_selection() {
    let action = parse_suite_arguments_for_test([
        "gb-emulator-shootout",
        "--suite",
        "blargg-cpu-instrs",
        "--case",
        "blargg-cpu-instrs-01-special",
    ])
    .expect("arguments should parse");
    assert!(format!("{action:?}").contains("blargg-cpu-instrs-01-special"));
}

#[test]
fn parse_accepts_missing_report_for_contextual_resolution() {
    let action = parse_suite_arguments_for_test(std::iter::empty::<&str>())
        .expect("missing report should parse before contextual resolution");
    assert!(format!("{action:?}").contains("report_id: None"));
}

#[test]
fn run_requires_report_and_lists_available_reports() {
    let mut output = Vec::new();
    let error = run_suite_command(std::iter::empty::<&str>(), &mut output)
        .expect_err("missing report should fail");
    assert!(output.is_empty());
    assert_eq!(
        error,
        "test ROM report must be provided; available reports: gb-emulator-shootout, docboy, gbmicrotest"
    );
}

#[test]
fn parse_rejects_case_without_suite() {
    assert!(
        parse_suite_arguments_for_test(["gb-emulator-shootout", "--case", "case-a"])
            .expect_err("case without suite should fail")
            .contains("--case requires --suite")
    );
}
