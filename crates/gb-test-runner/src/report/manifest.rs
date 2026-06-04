use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::model::{REPORTS_MANIFEST_PATH, Report};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ReportManifestFile {
    status_dir: Option<PathBuf>,
    report_file: Option<PathBuf>,
    #[serde(rename = "report")]
    reports: Vec<ReportFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ReportFile {
    id: String,
    store_dir: PathBuf,
    sources: Option<PathBuf>,
    status_dir: Option<PathBuf>,
    report_file: Option<PathBuf>,
    family_order: Option<Vec<String>>,
}

pub(super) fn load_reports(workspace_root: &Path) -> Result<Vec<Report>, String> {
    let path = workspace_root.join(REPORTS_MANIFEST_PATH);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read report manifest {}: {error}", path.display()))?;
    let manifest: ReportManifestFile = toml::from_str(&text).map_err(|error| {
        format!(
            "failed to parse report manifest {}: {error}",
            path.display()
        )
    })?;
    let default_status_dir = manifest
        .status_dir
        .unwrap_or_else(|| PathBuf::from(".status"));
    let default_report_file = manifest
        .report_file
        .unwrap_or_else(|| PathBuf::from("test-report.md"));
    validate_relative_path(&default_status_dir, "report default status_dir", false)?;
    validate_relative_path(&default_report_file, "report default report_file", false)?;

    let mut reports = Vec::with_capacity(manifest.reports.len());
    for report in manifest.reports {
        validate_id(&report.id, "report id")?;
        validate_relative_path(&report.store_dir, "report store_dir", true)?;
        if let Some(sources) = &report.sources {
            validate_relative_path(sources, "report sources", false)?;
        }
        let status_dir = report
            .status_dir
            .unwrap_or_else(|| default_status_dir.clone());
        let report_file = report
            .report_file
            .unwrap_or_else(|| default_report_file.clone());
        validate_relative_path(&status_dir, "report status_dir", false)?;
        validate_relative_path(&report_file, "report report_file", false)?;
        if let Some(family_order) = &report.family_order {
            for family in family_order {
                validate_id(family, "family_order entry")?;
            }
        }
        reports.push(Report {
            id: report.id,
            store_dir: report.store_dir,
            sources: report.sources,
            status_dir,
            report_file,
            family_order: report.family_order,
        });
    }
    Ok(reports)
}

fn validate_id(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(format!("{field} {value:?} contains unsupported characters"));
    }
    Ok(())
}

fn validate_relative_path(path: &Path, field: &str, allow_empty: bool) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        if allow_empty {
            return Ok(());
        }
        return Err(format!("{field} must not be empty"));
    }
    if path.is_absolute() {
        return Err(format!("{field} {} must be relative", path.display()));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::ParentDir => {
                return Err(format!(
                    "{field} {} must not contain parent components",
                    path.display()
                ));
            }
            Component::CurDir => {
                return Err(format!(
                    "{field} {} must not contain current-directory components",
                    path.display()
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("{field} {} must be relative", path.display()));
            }
        }
    }
    Ok(())
}
