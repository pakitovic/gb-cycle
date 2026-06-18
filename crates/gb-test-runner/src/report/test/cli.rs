use std::fs;
use std::path::Path;

use super::super::cli::{
    parse_report_arguments_for_test, report_help_text, run_report_command_with_workspace_for_test,
};
use super::super::manifest::load_reports;
use super::super::model::{
    PersistedCaseStatus, PersistedSuiteStatus, REPORT_STATUS_FAIL_EMOJI, REPORT_STATUS_INFO_EMOJI,
    REPORT_STATUS_PASS_EMOJI, ROM_REPORTS_PAGES_PATH, ReportSummary,
};
use super::super::render::render_markdown;
use super::super::status::{build_report_document, load_statuses};
use super::common::{
    unique_temp_dir, write_basic_reports, write_basic_reports_with_sources,
    write_local_report_with_missing_rom_suite, write_reports, write_status,
};

#[test]
fn help_mentions_report_contract() {
    let help = report_help_text();
    assert!(help.contains("<report-id>"));
    assert!(help.contains("--html"));
    assert!(help.contains("--boot-rom-dir <dir>"));
    assert!(help.contains("--force-real-boot"));
    assert!(help.contains("--index <dir>"));
    assert!(help.contains("rom-reports-pages.json"));
    assert!(help.contains("cargo rom-suite <report-id>"));
}

#[test]
fn parse_accepts_report_and_html() {
    let action = parse_report_arguments_for_test([
        "gb-emulator-shootout",
        "--html",
        "--boot-rom-dir",
        "/tmp/bootroms",
        "--force-real-boot",
    ])
    .expect("arguments should parse");
    assert!(format!("{action:?}").contains("report_id: Some"));
    assert!(format!("{action:?}").contains("html: true"));
    assert!(format!("{action:?}").contains("boot_rom_dir: Some"));
    assert!(format!("{action:?}").contains("force_real_boot: true"));
}

#[test]
fn parse_accepts_index_mode() {
    let action = parse_report_arguments_for_test(["--index", "_site"])
        .expect("index arguments should parse");

    assert!(format!("{action:?}").contains("Index"));
    assert!(format!("{action:?}").contains("output_dir: \"_site\""));
}

#[test]
fn parse_rejects_index_mode_without_output_dir() {
    assert!(
        parse_report_arguments_for_test(["--index"])
            .expect_err("index mode without output dir should fail")
            .contains("--index requires <dir>")
    );
}

#[test]
fn parse_rejects_index_mode_combined_with_report_render_options() {
    assert!(
        parse_report_arguments_for_test(["sample-report", "--index", "_site"])
            .expect_err("index mode with positional report should fail")
            .contains("--index cannot be combined with <report-id>")
    );
    assert!(
        parse_report_arguments_for_test(["--index", "_site", "--html"])
            .expect_err("index mode with html should fail")
            .contains("--index cannot be combined with --html")
    );
    assert!(
        parse_report_arguments_for_test(["--index", "_site", "--boot-rom-dir", "/tmp/bootroms"])
            .expect_err("index mode with boot ROM dir should fail")
            .contains("--index cannot be combined with --boot-rom-dir")
    );
    assert!(
        parse_report_arguments_for_test(["--index", "_site", "--force-real-boot"])
            .expect_err("index mode with forced real boot should fail")
            .contains("--index cannot be combined with --force-real-boot")
    );
}

#[test]
fn parse_rejects_legacy_index_options() {
    assert!(
        parse_report_arguments_for_test(["sample-report", "--site-dir", "_site"])
            .expect_err("site dir should fail")
            .contains("--site-dir is not supported")
    );
    assert!(
        parse_report_arguments_for_test(["sample-report", "--report", "other"])
            .expect_err("index report should fail")
            .contains("--report is not supported")
    );
}

#[test]
fn parse_rejects_missing_boot_rom_dir_value() {
    assert!(
        parse_report_arguments_for_test(["gb-emulator-shootout", "--boot-rom-dir"])
            .expect_err("missing boot ROM dir should fail")
            .contains("--boot-rom-dir requires a value")
    );
}

#[test]
fn parse_rejects_force_real_boot_without_boot_rom_dir() {
    assert!(
        parse_report_arguments_for_test(["gb-emulator-shootout", "--force-real-boot"])
            .expect_err("force real boot without boot ROM dir should fail")
            .contains("--force-real-boot requires --boot-rom-dir <dir>")
    );
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
fn report_command_clears_selected_suite_runtime_and_preserves_link_evidence() {
    let workspace = unique_temp_dir("report-clean-selected-runtime");
    write_local_report_with_missing_rom_suite(&workspace);
    fs::write(
        workspace.join("crates/gb-test-runner/data/sample-report/docboy-dmg-link.link.suite.toml"),
        "this is intentionally not a single-machine suite manifest",
    )
    .expect("link suite manifest should be writable");
    write_status(
        &workspace,
        "sample-report",
        "sample-suite",
        r#"{
  "suite_name": "sample-suite",
  "family": "stale",
  "cases": [
    {
      "rom": "stale.gb",
      "status": "PASS"
    }
  ]
}
"#,
    );
    write_status(
        &workspace,
        "sample-report",
        "docboy-dmg-link",
        r#"{
  "suite_name": "docboy-dmg-link",
  "family": "docboy-dmg",
  "cases": [
    {
      "id": "linked-case",
      "status": "PASS"
    }
  ]
}
"#,
    );
    let selected_status = workspace.join("test/sample-report/.status/sample-suite.json");
    let stale_artifact =
        workspace.join("test/sample-report/.artifacts/sample-suite/stale-case/old.txt");
    fs::create_dir_all(
        stale_artifact
            .parent()
            .expect("artifact should have parent"),
    )
    .expect("stale artifact parent should be creatable");
    fs::write(&stale_artifact, "stale").expect("stale artifact should be writable");
    let linked_status = workspace.join("test/sample-report/.status/docboy-dmg-link.json");
    let linked_artifact =
        workspace.join("test/sample-report/.artifacts/docboy-dmg-link/linked-case/old.txt");
    fs::create_dir_all(
        linked_artifact
            .parent()
            .expect("linked artifact should have parent"),
    )
    .expect("linked artifact parent should be creatable");
    fs::write(&linked_artifact, "linked").expect("linked artifact should be writable");
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(["sample-report"], &workspace, &mut output)
        .expect("report should render after regenerating statuses");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("running cargo rom-suite sample-report"));
    let selected_status =
        fs::read_to_string(selected_status).expect("selected status should be rewritten");
    assert!(selected_status.contains("\"rom\": \"missing.gb\""));
    assert!(!selected_status.contains("stale.gb"));
    assert!(!stale_artifact.exists());
    assert!(linked_status.is_file());
    assert!(linked_artifact.is_file());
    assert!(
        workspace
            .join("test/sample-report/.status/sample-suite.json")
            .is_file()
    );
    let report = fs::read_to_string(workspace.join("test/sample-report/test-report.md"))
        .expect("markdown report should be written");
    assert!(report.contains("# Test Report: sample-report (0/1)"));
    assert!(report.contains(&format!(
        "| sample | missing.gb | {REPORT_STATUS_FAIL_EMOJI} |"
    )));
    assert!(!report.contains("stale.gb"));
    assert!(!report.contains("linked-case"));
    let summary: ReportSummary = serde_json::from_str(
        &fs::read_to_string(workspace.join("test/sample-report/.status/summary.json"))
            .expect("summary report should be written"),
    )
    .expect("summary report should parse");
    assert_eq!(
        summary,
        ReportSummary {
            report_id: "sample-report".to_string(),
            non_failing_cases: 0,
            total_cases: 1,
        }
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn report_command_forwards_boot_rom_dir_without_force_to_delegated_suite() {
    let workspace = unique_temp_dir("report-forwards-boot-rom-dir");
    write_local_report_with_missing_rom_suite(&workspace);
    let boot_rom_dir = workspace.join("missing-bootroms");
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(
        [
            "sample-report",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be UTF-8"),
        ],
        &workspace,
        &mut output,
    )
    .expect("plain boot ROM dir should not validate when no selected case uses real-boot");

    let output = String::from_utf8(output).expect("output should be UTF-8");
    assert!(output.contains("cargo rom-suite sample-report --boot-rom-dir"));
    assert!(!output.contains("--force-real-boot"));
    assert!(
        workspace
            .join("test/sample-report/test-report.md")
            .is_file()
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn report_command_forwards_force_real_boot_to_delegated_suite() {
    let workspace = unique_temp_dir("report-forwards-force-real-boot");
    write_local_report_with_missing_rom_suite(&workspace);
    let boot_rom_dir = workspace.join("missing-bootroms");
    let mut output = Vec::new();

    let error = run_report_command_with_workspace_for_test(
        [
            "sample-report",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be UTF-8"),
            "--force-real-boot",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("forced real boot should fail during delegated suite preflight");

    let output = String::from_utf8(output).expect("output should be UTF-8");
    assert!(output.contains("cargo rom-suite sample-report --boot-rom-dir"));
    assert!(output.contains("--force-real-boot"));
    assert!(error.contains("boot ROM asset directory does not exist"));
    assert!(error.contains("failed before runtime cleanup"));
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn report_command_rejects_link_only_report_before_cleanup() {
    let workspace = unique_temp_dir("report-linked-preserves-runtime");
    write_reports(
        &workspace,
        r#"status_dir = ".status"
artifact_dir = ".artifacts"
report_file = "test-report.md"

[[report]]
id = "linked"
local = true
store_dir = "linked"
"#,
    );
    let linked_data_dir = workspace.join("crates/gb-test-runner/data/linked");
    fs::create_dir_all(&linked_data_dir).expect("linked data dir should be created");
    fs::write(
        linked_data_dir.join("dmg04.link.suite.toml"),
        "this is intentionally not a single-machine suite manifest",
    )
    .expect("link manifest should be writable");
    write_status(
        &workspace,
        "linked",
        "dmg04",
        r#"{
  "suite_name": "dmg04",
  "family": "linked",
  "cases": [
    {
      "id": "linked-case",
      "status": "PASS"
    }
  ]
}
"#,
    );
    let stale_status = workspace.join("test/linked/.status/dmg04.json");
    let stale_artifact = workspace.join("test/linked/.artifacts/dmg04/linked-case/old.txt");
    fs::create_dir_all(
        stale_artifact
            .parent()
            .expect("artifact should have parent"),
    )
    .expect("stale artifact parent should be creatable");
    fs::write(&stale_artifact, "stale").expect("stale artifact should be writable");
    let mut output = Vec::new();

    let error = run_report_command_with_workspace_for_test(["linked"], &workspace, &mut output)
        .expect_err("link-only report should fail before cleanup");

    assert!(output.is_empty());
    assert!(error.contains("does not contain single-machine suite manifests"));
    assert!(stale_status.is_file());
    assert!(stale_artifact.is_file());
    assert!(!workspace.join("test/linked/test-report.md").exists());
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn report_command_preserves_runtime_when_suite_preflight_fails_before_cleanup() {
    let workspace = unique_temp_dir("report-suite-preflight-preserves-runtime");
    write_reports(
        &workspace,
        r#"status_dir = ".status"
artifact_dir = ".artifacts"
report_file = "test-report.md"

[[report]]
id = "sample-report"
local = true
store_dir = "sample-report"
"#,
    );
    let suite_root = workspace.join("crates/gb-test-runner/data/sample-report");
    fs::create_dir_all(&suite_root).expect("suite dir should be created");
    fs::write(
        suite_root.join("broken.suite.toml"),
        r#"report = "sample-report"
suite_name = "broken-suite"
family = "sample"
model = "dmg"
unknown_header = true
"#,
    )
    .expect("broken suite manifest should be writable");
    write_status(
        &workspace,
        "sample-report",
        "stale-suite",
        r#"{
  "suite_name": "stale-suite",
  "family": "stale",
  "cases": [
    {
      "family": "stale",
      "rom": "stale.gb",
      "status": "PASS"
    }
  ]
}
"#,
    );
    let stale_status = workspace.join("test/sample-report/.status/stale-suite.json");
    let stale_artifact =
        workspace.join("test/sample-report/.artifacts/stale-suite/stale-case/old.txt");
    fs::create_dir_all(
        stale_artifact
            .parent()
            .expect("artifact should have parent"),
    )
    .expect("stale artifact parent should be creatable");
    fs::write(&stale_artifact, "stale").expect("stale artifact should be writable");
    fs::write(
        workspace.join("test/sample-report/test-report.md"),
        "previous report",
    )
    .expect("previous markdown report should be writable");
    let mut output = Vec::new();

    let error =
        run_report_command_with_workspace_for_test(["sample-report"], &workspace, &mut output)
            .expect_err("bad suite manifest should fail before cleanup");

    assert!(error.contains("failed before runtime cleanup"));
    assert!(error.contains("unknown_header"));
    assert!(stale_status.is_file());
    assert!(stale_artifact.is_file());
    let report = fs::read_to_string(workspace.join("test/sample-report/test-report.md"))
        .expect("previous markdown report should be preserved");
    assert_eq!(report, "previous report");
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn report_document_uses_source_order_before_suite_order() {
    let workspace = unique_temp_dir("source-order");
    write_basic_reports_with_sources(&workspace);
    fs::create_dir_all(workspace.join("crates/gb-test-runner/data/sample-report"))
        .expect("source manifest dir should be created");
    fs::write(
        workspace.join("crates/gb-test-runner/data/sample-report/a-suite.suite.toml"),
        r#"report = "sample-report"
suite_name = "a-suite"
"#,
    )
    .expect("a-suite manifest should be writable");
    fs::write(
        workspace.join("crates/gb-test-runner/data/sample-report/z-suite.suite.toml"),
        r#"report = "sample-report"
suite_name = "z-suite"
"#,
    )
    .expect("z-suite manifest should be writable");
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
        r#"{
  "suite_name": "a-suite",
  "family": "acid",
  "cases": [
    {
      "rom": "later.gb",
      "status": "FAIL"
    }
  ]
}
"#,
    );
    write_status(
        &workspace,
        "sample-report",
        "z-suite",
        r#"{
  "suite_name": "z-suite",
  "family": "acid",
  "cases": [
    {
      "rom": "which.gb (GBC) (CPU-CGB-D)",
      "status": "INFO"
    },
    {
      "rom": "which.gb (DMG)",
      "status": "PASS"
    }
  ]
}
"#,
    );
    fs::write(
        workspace.join("test/sample-report/.status/summary.json"),
        r#"{"report_id":"sample-report","non_failing_cases":99,"total_cases":99}"#,
    )
    .expect("summary status sidecar should be writable");
    write_status(
        &workspace,
        "sample-report",
        "linked-suite",
        r#"{
  "suite_name": "linked-suite",
  "family": "linked",
  "cases": [
    {
      "rom": "linked.gb",
      "status": "PASS"
    }
  ]
}
"#,
    );
    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "sample-report")
        .expect("sample report should exist");
    let statuses = load_statuses(&workspace, report).expect("statuses should load");
    let document = build_report_document(&workspace, report, statuses, None, false)
        .expect("report document should build");
    let report = render_markdown(&document);
    let which_dmg = report
        .find(&format!(
            "| acid | which.gb (DMG) | {REPORT_STATUS_PASS_EMOJI} |"
        ))
        .expect("DMG variant row should be rendered");
    let which_gbc = report
        .find(&format!(
            "| acid | which.gb (GBC) (CPU-CGB-D) | {REPORT_STATUS_INFO_EMOJI} |"
        ))
        .expect("GBC variant row should be rendered");
    let later = report
        .find(&format!("| acid | later.gb | {REPORT_STATUS_FAIL_EMOJI} |"))
        .expect("later ROM row should be rendered");
    assert!(which_dmg < which_gbc);
    assert!(which_gbc < later);
    assert!(!report.contains("linked.gb"));
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn report_document_uses_boot_rom_dir_placeholder_in_reproduction_command() {
    let workspace = unique_temp_dir("real-boot-report-command");
    write_basic_reports(&workspace);
    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "sample-report")
        .expect("sample report should exist");
    let private_boot_rom_dir = workspace.join("private-real-boot-roms");
    let statuses = vec![PersistedSuiteStatus {
        suite_name: "sample-suite".to_string(),
        family: "acid".to_string(),
        cases: vec![PersistedCaseStatus {
            family: None,
            rom: "which.gb".to_string(),
            status: "PASS".to_string(),
        }],
    }];

    let document = build_report_document(
        &workspace,
        report,
        statuses,
        Some(private_boot_rom_dir.as_path()),
        false,
    )
    .expect("report document should build");

    assert_eq!(
        document.command,
        "cargo rom-report sample-report --boot-rom-dir <dir>"
    );
    assert!(
        !document
            .command
            .contains(&private_boot_rom_dir.display().to_string())
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn report_document_includes_force_real_boot_in_reproduction_command() {
    let workspace = unique_temp_dir("force-real-boot-report-command");
    write_basic_reports(&workspace);
    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "sample-report")
        .expect("sample report should exist");
    let private_boot_rom_dir = workspace.join("private-real-boot-roms");
    let statuses = vec![PersistedSuiteStatus {
        suite_name: "sample-suite".to_string(),
        family: "acid".to_string(),
        cases: vec![PersistedCaseStatus {
            family: None,
            rom: "which.gb".to_string(),
            status: "PASS".to_string(),
        }],
    }];

    let document = build_report_document(
        &workspace,
        report,
        statuses,
        Some(private_boot_rom_dir.as_path()),
        true,
    )
    .expect("report document should build");

    assert_eq!(
        document.command,
        "cargo rom-report sample-report --boot-rom-dir <dir> --force-real-boot"
    );
    assert!(
        !document
            .command
            .contains(&private_boot_rom_dir.display().to_string())
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn html_report_escapes_status_data() {
    let workspace = unique_temp_dir("html-escaping");
    write_reports(
        &workspace,
        r#"status_dir = ".status"
artifact_dir = ".artifacts"
report_file = "test-report.md"

[[report]]
id = "sample-report"
local = true
store_dir = "sample-report"
family_order = ["sample"]
"#,
    );
    let suite_root = workspace.join("crates/gb-test-runner/data/sample-report");
    fs::create_dir_all(&suite_root).expect("suite dir should be created");
    fs::write(
        suite_root.join("sample-suite.suite.toml"),
        r#"report = "sample-report"
suite_name = "sample-suite"
family = "sample"
model = "dmg"
timeout_frames = 1
oracle = { type = "serial-contains", expected = "Passed" }

[[case]]
id = "evil-rom"
rom = "evil<&>.gb"
"#,
    )
    .expect("suite manifest should be writable");
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(
        ["sample-report", "--html"],
        &workspace,
        &mut output,
    )
    .expect("HTML report should render");

    let html = fs::read_to_string(workspace.join("test/sample-report/.status/index.html"))
        .expect("HTML report should be written");
    assert!(html.contains("evil&#60;&#38;&#62;.gb"));
    assert!(!html.contains("evil<&>.gb"));
    assert!(html.contains(REPORT_STATUS_FAIL_EMOJI));
    assert!(
        !workspace
            .join("test/sample-report/test-report.html")
            .exists()
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn index_report_renders_counts_and_complete_status() {
    let workspace = unique_temp_dir("index-report-counts");
    let site_dir = workspace.join("_site");
    write_index_reports(
        &workspace,
        r#"status_dir = ".status"
artifact_dir = ".artifacts"
report_file = "test-report.md"

[[report]]
id = "all-pass"
local = true
store_dir = "all-pass"

[[report]]
id = "partial"
local = true
store_dir = "partial"
"#,
        &["all-pass", "partial"],
    );
    write_materialized_report(
        &workspace,
        ReportSummary {
            report_id: "all-pass".to_string(),
            non_failing_cases: 3,
            total_cases: 3,
        },
    );
    write_materialized_report(
        &workspace,
        ReportSummary {
            report_id: "partial".to_string(),
            non_failing_cases: 1,
            total_cases: 2,
        },
    );
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(
        [
            "--index",
            site_dir.to_str().expect("site path should be UTF-8"),
        ],
        &workspace,
        &mut output,
    )
    .expect("index should render");

    let output = String::from_utf8(output).expect("output should be UTF-8");
    assert!(output.contains("index.html"));
    let html = fs::read_to_string(site_dir.join("index.html")).expect("index should be written");
    assert!(html.contains("data-epoch-seconds="));
    assert!(html.contains("new Intl.DateTimeFormat"));
    assert!(!html.contains("since UNIX epoch"));
    assert!(html.contains("reports/all-pass/index.html"));
    assert!(html.contains("3/3"));
    assert!(html.contains(REPORT_STATUS_PASS_EMOJI));
    assert!(html.contains("reports/partial/index.html"));
    assert!(html.contains("1/2"));
    assert!(html.contains(REPORT_STATUS_FAIL_EMOJI));
    let all_pass = html
        .find("reports/all-pass/index.html")
        .expect("all-pass row should exist");
    let partial = html
        .find("reports/partial/index.html")
        .expect("partial row should exist");
    assert!(all_pass < partial);
    assert!(site_dir.join("reports/all-pass/index.html").is_file());
    assert!(!site_dir.join("reports/all-pass/summary.json").exists());
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn index_report_treats_empty_report_as_incomplete() {
    let workspace = unique_temp_dir("index-empty-report");
    let site_dir = workspace.join("_site");
    write_index_reports(
        &workspace,
        r#"status_dir = ".status"
artifact_dir = ".artifacts"
report_file = "test-report.md"

[[report]]
id = "empty"
local = true
store_dir = "empty"
"#,
        &["empty"],
    );
    write_materialized_report(
        &workspace,
        ReportSummary {
            report_id: "empty".to_string(),
            non_failing_cases: 0,
            total_cases: 0,
        },
    );
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(
        [
            "--index",
            site_dir.to_str().expect("site path should be UTF-8"),
        ],
        &workspace,
        &mut output,
    )
    .expect("index should render");

    let html = fs::read_to_string(site_dir.join("index.html")).expect("index should be written");
    assert!(html.contains("0/0"));
    assert!(html.contains(REPORT_STATUS_FAIL_EMOJI));
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn index_report_omits_reports_without_materialized_status_html_or_summary() {
    let workspace = unique_temp_dir("index-omits-missing-inputs");
    let site_dir = workspace.join("_site");
    write_index_reports(
        &workspace,
        r#"status_dir = ".status"
artifact_dir = ".artifacts"
report_file = "test-report.md"

[[report]]
id = "ready"
local = true
store_dir = "ready"

[[report]]
id = "missing-summary"
local = true
store_dir = "missing-summary"

[[report]]
id = "missing-html"
local = true
store_dir = "missing-html"
"#,
        &["ready", "missing-summary", "missing-html"],
    );
    write_materialized_report(
        &workspace,
        ReportSummary {
            report_id: "ready".to_string(),
            non_failing_cases: 1,
            total_cases: 1,
        },
    );
    let missing_summary_dir = workspace.join("test/missing-summary/.status");
    fs::create_dir_all(&missing_summary_dir).expect("status dir should be created");
    fs::write(missing_summary_dir.join("index.html"), "<!doctype html>")
        .expect("HTML should be written");
    let missing_html_dir = workspace.join("test/missing-html/.status");
    fs::create_dir_all(&missing_html_dir).expect("status dir should be created");
    fs::write(
        missing_html_dir.join("summary.json"),
        r#"{"report_id":"missing-html","non_failing_cases":1,"total_cases":1}"#,
    )
    .expect("summary should be written");
    let mut output = Vec::new();

    run_report_command_with_workspace_for_test(
        [
            "--index",
            site_dir.to_str().expect("site path should be UTF-8"),
        ],
        &workspace,
        &mut output,
    )
    .expect("index should render with only materialized reports");

    let output = String::from_utf8(output).expect("output should be UTF-8");
    assert!(output.contains("skipped missing-summary"));
    assert!(output.contains("skipped missing-html"));
    let html = fs::read_to_string(site_dir.join("index.html")).expect("index should be written");
    assert!(html.contains("reports/ready/index.html"));
    assert!(!html.contains("reports/missing-summary/index.html"));
    assert!(!html.contains("reports/missing-html/index.html"));
    assert!(site_dir.join("reports/ready/index.html").is_file());
    assert!(!site_dir.join("reports/missing-summary/index.html").exists());
    assert!(!site_dir.join("reports/missing-html/index.html").exists());
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

fn write_materialized_report(workspace_root: &Path, summary: ReportSummary) {
    let report_dir = workspace_root
        .join("test")
        .join(&summary.report_id)
        .join(".status");
    fs::create_dir_all(&report_dir).expect("report status dir should be created");
    fs::write(
        report_dir.join("index.html"),
        format!("<!doctype html><title>{}</title>", summary.report_id),
    )
    .expect("report HTML should be written");
    fs::write(
        report_dir.join("summary.json"),
        serde_json::to_string(&summary).expect("summary should serialize"),
    )
    .expect("summary should be written");
}

fn write_index_reports(workspace_root: &Path, reports: &str, report_ids: &[&str]) {
    write_reports(workspace_root, reports);
    let pages_path = workspace_root.join(ROM_REPORTS_PAGES_PATH);
    fs::create_dir_all(
        pages_path
            .parent()
            .expect("pages metadata should have parent"),
    )
    .expect("pages metadata parent should be created");
    let pages = report_ids
        .iter()
        .map(|report_id| format!(r#"{{"name":"{report_id}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(pages_path, format!("[{pages}]\n")).expect("pages metadata should be written");
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
            .join("test/sample-report/.status/sample-suite.json")
            .is_file()
    );
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}
