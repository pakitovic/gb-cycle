use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use super::model::{
    PersistedSuiteStatus, Report, ReportDocument, ReportRow, TEST_ROM_STORE_DIR,
    is_non_failing_status, report_status_display,
};

pub(super) fn load_statuses(
    workspace_root: &Path,
    report: &Report,
) -> Result<Vec<PersistedSuiteStatus>, String> {
    let status_root = status_root_for_report(workspace_root, report);
    let status_files = status_files(&status_root)?;
    let mut statuses = Vec::with_capacity(status_files.len());
    for path in status_files {
        let text = fs::read_to_string(&path).map_err(|error| {
            format!("failed to read test ROM status {}: {error}", path.display())
        })?;
        let status: PersistedSuiteStatus = toml::from_str(&text).map_err(|error| {
            format!(
                "failed to parse test ROM status {}: {error}",
                path.display()
            )
        })?;
        statuses.push(status);
    }
    Ok(statuses)
}

pub(super) fn build_report_document(
    report: &Report,
    statuses: Vec<PersistedSuiteStatus>,
) -> Result<ReportDocument, String> {
    let mut rows = Vec::new();
    let mut non_failing_cases = 0;
    let mut total_cases = 0;
    for suite in statuses {
        for (case_index, case) in suite.cases.into_iter().enumerate() {
            report_status_display(&case.status)?;
            total_cases += 1;
            if is_non_failing_status(&case.status) {
                non_failing_cases += 1;
            }
            rows.push(ReportRow {
                family: case.family.unwrap_or_else(|| suite.family.clone()),
                rom: case.rom,
                status: case.status,
                suite_name: suite.suite_name.clone(),
                case_index,
            });
        }
    }
    rows.sort_by(|left, right| compare_report_rows(left, right, report.family_order.as_deref()));

    Ok(ReportDocument {
        report_id: report.id.clone(),
        command: format!("cargo rom-report {}", report.id),
        non_failing_cases,
        total_cases,
        rows,
    })
}

pub(super) fn store_root_for_report(workspace_root: &Path, report: &Report) -> PathBuf {
    workspace_root
        .join(TEST_ROM_STORE_DIR)
        .join(&report.store_dir)
}

fn status_files(status_root: &Path) -> Result<Vec<PathBuf>, String> {
    if !status_root.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(status_root).map_err(|error| {
        format!(
            "failed to read test ROM status directory {}: {error}",
            status_root.display()
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read test ROM status entry in {}: {error}",
                status_root.display()
            )
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn compare_report_rows(
    left: &ReportRow,
    right: &ReportRow,
    family_order: Option<&[String]>,
) -> Ordering {
    let left_rank = report_family_rank(&left.family, family_order);
    let right_rank = report_family_rank(&right.family, family_order);
    (left_rank.is_none(), left_rank.unwrap_or(usize::MAX))
        .cmp(&(right_rank.is_none(), right_rank.unwrap_or(usize::MAX)))
        .then_with(|| left.family.cmp(&right.family))
        .then_with(|| left.suite_name.cmp(&right.suite_name))
        .then_with(|| left.case_index.cmp(&right.case_index))
        .then_with(|| left.rom.cmp(&right.rom))
}

fn report_family_rank(family: &str, family_order: Option<&[String]>) -> Option<usize> {
    family_order.and_then(|order| order.iter().position(|known| known == family))
}

fn status_root_for_report(workspace_root: &Path, report: &Report) -> PathBuf {
    store_root_for_report(workspace_root, report).join(&report.status_dir)
}
