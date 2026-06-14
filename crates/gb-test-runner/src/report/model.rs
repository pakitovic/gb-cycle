use std::path::PathBuf;

use serde::Deserialize;

pub(super) const DATA_DIR: &str = "crates/gb-test-runner/data";
pub(super) const REPORTS_MANIFEST_PATH: &str = "crates/gb-test-runner/data/reports.toml";
pub(super) const TEST_ROM_STORE_DIR: &str = "test";
pub(super) const REPORT_STATUS_PASS_EMOJI: &str = "✅";
pub(super) const REPORT_STATUS_FAIL_EMOJI: &str = "❌";
pub(super) const REPORT_STATUS_INFO_EMOJI: &str = "ℹ️";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Report {
    pub(super) id: String,
    pub(super) store_dir: PathBuf,
    pub(super) sources: Option<PathBuf>,
    pub(super) status_dir: PathBuf,
    pub(super) artifact_dir: PathBuf,
    pub(super) report_file: PathBuf,
    pub(super) family_order: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct PersistedSuiteStatus {
    pub(super) suite_name: String,
    pub(super) family: String,
    pub(super) cases: Vec<PersistedCaseStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct PersistedCaseStatus {
    pub(super) family: Option<String>,
    pub(super) rom: String,
    pub(super) status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportDocument {
    pub(super) report_id: String,
    pub(super) command: String,
    pub(super) non_failing_cases: usize,
    pub(super) total_cases: usize,
    pub(super) rows: Vec<ReportRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportRow {
    pub(super) family: String,
    pub(super) rom: String,
    pub(super) status: String,
    pub(super) suite_name: String,
    pub(super) case_index: usize,
}

pub(super) fn report_status_display(status: &str) -> Result<&'static str, String> {
    match status {
        "PASS" => Ok(REPORT_STATUS_PASS_EMOJI),
        "FAIL" => Ok(REPORT_STATUS_FAIL_EMOJI),
        "INFO" => Ok(REPORT_STATUS_INFO_EMOJI),
        other => Err(format!("unsupported test ROM report status {other:?}")),
    }
}

pub(super) fn is_non_failing_status(status: &str) -> bool {
    matches!(status, "PASS" | "INFO")
}
