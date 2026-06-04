use std::fs;
use std::path::{Path, PathBuf};

use super::model::{
    PersistedCaseStatus, PersistedSuiteStatus, Report, SuiteRunReport, TEST_ROM_STORE_DIR,
};

pub(super) fn store_root_for_report(workspace_root: &Path, report: &Report) -> PathBuf {
    workspace_root
        .join(TEST_ROM_STORE_DIR)
        .join(&report.store_dir)
}

pub(super) fn write_suite_status(
    workspace_root: &Path,
    report: &Report,
    suite_report: &SuiteRunReport,
) -> Result<PathBuf, String> {
    let status_root = store_root_for_report(workspace_root, report).join(&report.status_dir);
    fs::create_dir_all(&status_root).map_err(|error| {
        format!(
            "failed to create suite status directory {}: {error}",
            status_root.display()
        )
    })?;
    let path = status_root.join(format!("{}.toml", suite_report.suite_name));
    let persisted = PersistedSuiteStatus {
        suite_name: suite_report.suite_name.clone(),
        family: suite_report.family.clone(),
        cases: suite_report
            .cases
            .iter()
            .map(|case| PersistedCaseStatus {
                family: None,
                rom: case.rom.clone(),
                status: persisted_case_status(case).to_string(),
            })
            .collect(),
    };
    let text = toml::to_string(&persisted).map_err(|error| {
        format!(
            "failed to serialize suite status for {}: {error}",
            suite_report.suite_name
        )
    })?;
    fs::write(&path, text)
        .map_err(|error| format!("failed to write suite status {}: {error}", path.display()))?;
    Ok(path)
}

fn persisted_case_status(case: &super::model::CaseRunReport) -> &'static str {
    if !case.passed {
        "FAIL"
    } else if case.informational {
        "INFO"
    } else {
        "PASS"
    }
}
