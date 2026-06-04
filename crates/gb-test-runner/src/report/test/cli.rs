use std::fs;

use super::super::cli::{
    parse_report_arguments_for_test, report_help_text, run_report_command_with_workspace_for_test,
};
use super::super::model::{
    REPORT_STATUS_FAIL_EMOJI, REPORT_STATUS_INFO_EMOJI, REPORT_STATUS_PASS_EMOJI,
};
use super::common::{
    unique_temp_dir, write_basic_reports, write_basic_reports_with_sources,
    write_local_report_with_missing_rom_suite, write_status,
};

#[test]
fn help_mentions_report_contract() {
    let help = report_help_text();
    assert!(help.contains("<report-id>"));
    assert!(help.contains("--html"));
    assert!(help.contains("cargo rom-suite <report-id>"));
}

#[test]
fn parse_accepts_report_and_html() {
    let action = parse_report_arguments_for_test(["gb-emulator-shootout", "--html"])
        .expect("arguments should parse");
    assert!(format!("{action:?}").contains("report_id: Some"));
    assert!(format!("{action:?}").contains("html: true"));
}

#[test]
fn parse_rejects_extra_positionals() {
    assert!(
        parse_report_arguments_for_test(["gb-emulator-shootout", "docboy"])
            .expect_err("extra positional should fail")
            .contains("unexpected extra positional argument")
    );
}

#[test]
fn run_requires_report_and_lists_available_reports() {
    let workspace = unique_temp_dir("missing-report");
    write_basic_reports(&workspace);
    let mut output = Vec::new();

    let error = run_report_command_with_workspace_for_test(
        std::iter::empty::<&str>(),
        &workspace,
        &mut output,
    )
    .expect_err("missing report should fail");

    assert!(output.is_empty());
    assert_eq!(
        error,
        "test ROM report must be provided; available reports: sample-report"
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn run_rejects_unknown_report_and_lists_available_reports() {
    let workspace = unique_temp_dir("unknown-report");
    write_basic_reports(&workspace);
    let mut output = Vec::new();

    let error = run_report_command_with_workspace_for_test(["unknown"], &workspace, &mut output)
        .expect_err("unknown report should fail");

    assert!(output.is_empty());
    assert_eq!(
        error,
        "unknown test ROM report \"unknown\"; available reports: sample-report"
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn existing_status_writes_markdown_without_running_suite() {
    let workspace = unique_temp_dir("markdown-existing");
    write_basic_reports(&workspace);
    write_status(
        &workspace,
        "sample-report",
        "blargg",
        r#"suite_name = "blargg"
family = "blargg"

[[cases]]
rom = "halt_bug.gb"
status = "FAIL"

[[cases]]
family = "acid"
rom = "which.gb (DMG)"
status = "PASS"
"#,
    );
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(["sample-report"], &workspace, &mut output)
        .expect("report should render");

    let report = fs::read_to_string(workspace.join("test/sample-report/test-report.md"))
        .expect("markdown report should be written");
    assert!(report.contains("# Test Report: sample-report (1/2)"));
    assert!(report.contains("Command: `cargo rom-report sample-report`"));
    assert!(report.contains(&format!(
        "| acid | which.gb (DMG) | {REPORT_STATUS_PASS_EMOJI} |"
    )));
    assert!(report.contains(&format!(
        "| blargg | halt_bug.gb | {REPORT_STATUS_FAIL_EMOJI} |"
    )));
    assert!(
        !String::from_utf8(output)
            .expect("output should be utf-8")
            .contains("running cargo rom-suite")
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn existing_status_uses_source_order_before_suite_order() {
    let workspace = unique_temp_dir("source-order");
    write_basic_reports_with_sources(&workspace);
    fs::create_dir_all(workspace.join("crates/gb-test-runner/data/sample-report"))
        .expect("source manifest dir should be created");
    fs::write(
        workspace.join("crates/gb-test-runner/data/sample-report/sources.report.toml"),
        r#"[[source]]
id = "sample-source"
git_url = "https://example.test/sample.git"
git_rev = "0123456789abcdef"

[[source.family]]
id = "acid"
target_root = "acid"
sparse_paths = ["acid"]

[[source.family.file]]
path = "acid/which.gb"
target = "which.gb"
sha256 = "sha"

[[source.family.file]]
path = "acid/which.png"
target = "which.png"
sha256 = "sha"

[[source.family.file]]
path = "acid/later.gb"
target = "later.gb"
sha256 = "sha"
"#,
    )
    .expect("source manifest should be writable");
    write_status(
        &workspace,
        "sample-report",
        "a-suite",
        r#"suite_name = "a-suite"
family = "acid"

[[cases]]
rom = "later.gb"
status = "FAIL"
"#,
    );
    write_status(
        &workspace,
        "sample-report",
        "z-suite",
        r#"suite_name = "z-suite"
family = "acid"

[[cases]]
rom = "which.gb (GBC)"
status = "INFO"

[[cases]]
rom = "which.gb (DMG)"
status = "PASS"
"#,
    );
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(["sample-report"], &workspace, &mut output)
        .expect("report should render");

    let report = fs::read_to_string(workspace.join("test/sample-report/test-report.md"))
        .expect("markdown report should be written");
    let which_dmg = report
        .find(&format!(
            "| acid | which.gb (DMG) | {REPORT_STATUS_PASS_EMOJI} |"
        ))
        .expect("DMG variant row should be rendered");
    let which_gbc = report
        .find(&format!(
            "| acid | which.gb (GBC) | {REPORT_STATUS_INFO_EMOJI} |"
        ))
        .expect("GBC variant row should be rendered");
    let later = report
        .find(&format!("| acid | later.gb | {REPORT_STATUS_FAIL_EMOJI} |"))
        .expect("later ROM row should be rendered");
    assert!(which_dmg < which_gbc);
    assert!(which_gbc < later);
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn html_report_escapes_status_data() {
    let workspace = unique_temp_dir("html-escaping");
    write_basic_reports(&workspace);
    write_status(
        &workspace,
        "sample-report",
        "blargg",
        r#"suite_name = "blargg"
family = "blargg"

[[cases]]
rom = "evil<&>.gb"
status = "INFO"
"#,
    );
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(
        ["sample-report", "--html"],
        &workspace,
        &mut output,
    )
    .expect("HTML report should render");

    let html = fs::read_to_string(workspace.join("test/sample-report/test-report.html"))
        .expect("HTML report should be written");
    assert!(html.contains("evil&lt;&amp;&gt;.gb"));
    assert!(!html.contains("evil<&>.gb"));
    assert!(html.contains(REPORT_STATUS_INFO_EMOJI));
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn missing_status_auto_runs_suite_and_renders_written_status() {
    let workspace = unique_temp_dir("missing-status");
    write_local_report_with_missing_rom_suite(&workspace);
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(["sample-report"], &workspace, &mut output)
        .expect("report should render after auto-running suite");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("running cargo rom-suite sample-report"));
    assert!(output.contains("cargo rom-suite sample-report returned"));
    let report = fs::read_to_string(workspace.join("test/sample-report/test-report.md"))
        .expect("markdown report should be written");
    assert!(report.contains("# Test Report: sample-report (0/1)"));
    assert!(report.contains(&format!(
        "| sample | missing.gb | {REPORT_STATUS_FAIL_EMOJI} |"
    )));
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn empty_status_dir_auto_runs_suite_and_renders_written_status() {
    let workspace = unique_temp_dir("empty-status");
    write_local_report_with_missing_rom_suite(&workspace);
    fs::create_dir_all(workspace.join("test/sample-report/.status"))
        .expect("empty status dir should be created");
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(["sample-report"], &workspace, &mut output)
        .expect("report should render after auto-running suite");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("running cargo rom-suite sample-report"));
    assert!(
        workspace
            .join("test/sample-report/.status/sample-suite.toml")
            .is_file()
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}
