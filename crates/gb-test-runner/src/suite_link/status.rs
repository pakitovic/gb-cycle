use std::fs;
use std::path::{Path, PathBuf};

use super::model::{
    LinkSuiteRunReport, PersistedLinkCaseStatus, PersistedLinkParticipantStatus,
    PersistedLinkSuiteStatus, Report, TEST_ROM_STORE_DIR,
};

pub(super) fn runtime_root_for_report(workspace_root: &Path, report: &Report) -> PathBuf {
    let root = workspace_root.join(TEST_ROM_STORE_DIR);
    if report.store_dir.as_os_str().is_empty() {
        root
    } else {
        root.join(&report.store_dir)
    }
}

pub(super) fn write_link_suite_status(
    workspace_root: &Path,
    report: &Report,
    suite_report: &LinkSuiteRunReport,
) -> Result<PathBuf, String> {
    let status_root = runtime_root_for_report(workspace_root, report).join(&report.status_dir);
    fs::create_dir_all(&status_root).map_err(|error| {
        format!(
            "failed to create linked suite status directory {}: {error}",
            status_root.display()
        )
    })?;
    let path = status_root.join(format!("{}.toml", suite_report.suite_name));
    let persisted = PersistedLinkSuiteStatus {
        suite_name: suite_report.suite_name.clone(),
        family: suite_report.family.clone(),
        cases: suite_report
            .cases
            .iter()
            .map(|case| PersistedLinkCaseStatus {
                id: case.id.clone(),
                status: persisted_link_case_status(case).to_string(),
                participants: case
                    .participants
                    .iter()
                    .map(|participant| PersistedLinkParticipantStatus {
                        id: participant.id.clone(),
                        rom: participant.rom.to_string_lossy().into_owned(),
                    })
                    .collect(),
            })
            .collect(),
    };
    let text = toml::to_string(&persisted).map_err(|error| {
        format!(
            "failed to serialize linked suite status for {}: {error}",
            suite_report.suite_name
        )
    })?;
    fs::write(&path, text).map_err(|error| {
        format!(
            "failed to write linked suite status {}: {error}",
            path.display()
        )
    })?;
    Ok(path)
}

fn persisted_link_case_status(case: &super::model::LinkCaseRunReport) -> &'static str {
    if !case.passed {
        "FAIL"
    } else if case.informational {
        "INFO"
    } else {
        "PASS"
    }
}
