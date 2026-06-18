use std::fs;
use std::path::{Path, PathBuf};

use super::super::model::REPORTS_MANIFEST_PATH;

pub(super) fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gb-cycle-report-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

pub(super) fn write_reports(workspace_root: &Path, text: &str) {
    let path = workspace_root.join(REPORTS_MANIFEST_PATH);
    fs::create_dir_all(path.parent().expect("reports should have parent"))
        .expect("reports parent should be creatable");
    fs::write(path, text).expect("reports manifest should be writable");
}

pub(super) fn write_basic_reports(workspace_root: &Path) {
    write_reports(
        workspace_root,
        r#"status_dir = ".status"
artifact_dir = ".artifacts"
report_file = "test-report.md"

[[report]]
id = "sample-report"
store_dir = "sample-report"
family_order = ["acid", "blargg", "mooneye"]
"#,
    );
}

pub(super) fn write_basic_reports_with_sources(workspace_root: &Path) {
    write_reports(
        workspace_root,
        r#"status_dir = ".status"
artifact_dir = ".artifacts"
report_file = "test-report.md"

[[report]]
id = "sample-report"
store_dir = "sample-report"
sources = "sample-report/sources.report.toml"
family_order = ["acid", "blargg", "mooneye"]
"#,
    );
}

pub(super) fn write_status(workspace_root: &Path, report_id: &str, suite_name: &str, text: &str) {
    let status_root = workspace_root.join("test").join(report_id).join(".status");
    fs::create_dir_all(&status_root).expect("status dir should be created");
    fs::write(status_root.join(format!("{suite_name}.json")), text)
        .expect("status should be writable");
}

pub(super) fn write_local_report_with_missing_rom_suite(workspace_root: &Path) {
    write_reports(
        workspace_root,
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
    let suite_root = workspace_root.join("crates/gb-test-runner/data/sample-report");
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
id = "missing-rom"
rom = "missing.gb"
"#,
    )
    .expect("suite manifest should be writable");
}
