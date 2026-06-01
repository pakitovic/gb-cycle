use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::oracle::{Oracle, OracleConfig};

use super::model::{DATA_DIR, REPORTS_MANIFEST_PATH, Report, SuiteCase, SuiteManifest};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ReportManifestFile {
    status_dir: Option<PathBuf>,
    #[serde(rename = "report")]
    reports: Vec<ReportFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ReportFile {
    id: String,
    store_dir: PathBuf,
    sources: PathBuf,
    status_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SuiteCaseDefaultsFile {
    family: Option<String>,
    console: Option<String>,
    timeout_frames: Option<u32>,
    oracle: Option<OracleConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct SuiteManifestFile {
    family: Option<String>,
    suite_name: String,
    report: Option<String>,
    #[serde(flatten)]
    defaults: SuiteCaseDefaultsFile,
    #[serde(rename = "case")]
    cases: Vec<SuiteCaseFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SuiteManifestHeaderFile {
    suite_name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SuiteCaseFile {
    family: Option<String>,
    id: String,
    rom: PathBuf,
    console: Option<String>,
    timeout_frames: Option<u32>,
    oracle: Option<OracleConfig>,
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
    Ok(manifest
        .reports
        .into_iter()
        .map(|report| Report {
            id: report.id,
            store_dir: report.store_dir,
            sources: report.sources,
            status_dir: report
                .status_dir
                .unwrap_or_else(|| default_status_dir.clone()),
        })
        .collect())
}

pub(super) fn load_selected_suites(
    workspace_root: &Path,
    report: &Report,
    suite_name: Option<&str>,
    case_id: Option<&str>,
) -> Result<Vec<SuiteManifest>, String> {
    let manifest_paths = suite_manifest_paths(workspace_root, report)?;
    let mut suites = Vec::new();
    for path in manifest_paths {
        let text = read_suite_manifest_text(&path)?;
        if let Some(selected_suite_name) = suite_name {
            let header = parse_suite_manifest_header(&path, &text)?;
            if header.suite_name != selected_suite_name {
                continue;
            }
        }
        suites.push(parse_suite_manifest(&path, &report.id, &text)?);
    }

    if let Some(suite_name) = suite_name
        && suites.is_empty()
    {
        return Err(format!(
            "unknown suite {suite_name:?} for report {:?}",
            report.id
        ));
    }

    if let Some(case_id) = case_id {
        for suite in &mut suites {
            suite.cases.retain(|case| case.id == case_id);
        }
        if suites.iter().all(|suite| suite.cases.is_empty()) {
            return Err(format!(
                "unknown case {case_id:?} for suite {:?}",
                suite_name.expect("case selection requires suite selection")
            ));
        }
    }

    Ok(suites)
}

fn suite_manifest_paths(workspace_root: &Path, report: &Report) -> Result<Vec<PathBuf>, String> {
    let report_data_dir = report_data_dir(workspace_root, report);
    let entries = fs::read_dir(&report_data_dir).map_err(|error| {
        format!(
            "failed to read suite manifest directory {}: {error}",
            report_data_dir.display()
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read suite manifest directory {}: {error}",
                report_data_dir.display()
            )
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".suite.toml") {
            continue;
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

fn report_data_dir(workspace_root: &Path, report: &Report) -> PathBuf {
    let source_parent = report.sources.parent().unwrap_or_else(|| Path::new(""));
    workspace_root.join(DATA_DIR).join(source_parent)
}

fn read_suite_manifest_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|error| format!("failed to read suite manifest {}: {error}", path.display()))
}

fn parse_suite_manifest_header(path: &Path, text: &str) -> Result<SuiteManifestHeaderFile, String> {
    toml::from_str(text).map_err(|error| {
        format!(
            "failed to parse suite manifest header {}: {error}",
            path.display()
        )
    })
}

fn parse_suite_manifest(path: &Path, report_id: &str, text: &str) -> Result<SuiteManifest, String> {
    let parsed: SuiteManifestFile = toml::from_str(text)
        .map_err(|error| format!("failed to parse suite manifest {}: {error}", path.display()))?;
    if let Some(declared_report) = &parsed.report
        && declared_report != report_id
    {
        return Err(format!(
            "suite manifest {} declares report {:?}, expected {:?}",
            path.display(),
            declared_report,
            report_id
        ));
    }
    let manifest_family = parsed
        .family
        .clone()
        .or_else(|| parsed.defaults.family.clone())
        .ok_or_else(|| format!("suite manifest {} must define family", path.display()))?;
    let mut seen_cases = BTreeSet::new();
    let cases = parsed
        .cases
        .into_iter()
        .map(|case| {
            if !seen_cases.insert(case.id.clone()) {
                return Err(format!(
                    "duplicate case id {:?} in suite manifest {}",
                    case.id,
                    path.display()
                ));
            }
            parse_case(path, &manifest_family, &parsed.defaults, case)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if cases.is_empty() {
        return Err(format!(
            "suite manifest {} must define at least one case",
            path.display()
        ));
    }

    Ok(SuiteManifest {
        suite_name: parsed.suite_name,
        family: manifest_family,
        cases,
    })
}

fn parse_case(
    path: &Path,
    manifest_family: &str,
    defaults: &SuiteCaseDefaultsFile,
    case: SuiteCaseFile,
) -> Result<SuiteCase, String> {
    let family = case
        .family
        .or_else(|| defaults.family.clone())
        .unwrap_or_else(|| manifest_family.to_string());
    let console = case
        .console
        .or_else(|| defaults.console.clone())
        .ok_or_else(|| {
            format!(
                "case {:?} in {} must define console",
                case.id,
                path.display()
            )
        })?;
    if console != "dmg" {
        return Err(format!(
            "case {:?} in {} uses unsupported console {:?}; suite runner only supports \"dmg\"",
            case.id,
            path.display(),
            console
        ));
    }
    let timeout_frames = case
        .timeout_frames
        .or(defaults.timeout_frames)
        .ok_or_else(|| {
            format!(
                "case {:?} in {} must define timeout_frames",
                case.id,
                path.display()
            )
        })?;
    if timeout_frames == 0 {
        return Err(format!(
            "case {:?} in {} must define a non-zero timeout_frames",
            case.id,
            path.display()
        ));
    }
    let oracle_config = case
        .oracle
        .or_else(|| defaults.oracle.clone())
        .ok_or_else(|| {
            format!(
                "case {:?} in {} must define oracle",
                case.id,
                path.display()
            )
        })?;
    let oracle = Oracle::from_manifest(&oracle_config)
        .map_err(|error| format!("case {:?} in {}: {error}", case.id, path.display()))?;

    Ok(SuiteCase {
        id: case.id,
        family,
        rom: case.rom,
        timeout_frames,
        oracle,
    })
}

#[cfg(test)]
pub(super) fn parse_suite_manifest_for_test(
    path: &Path,
    report_id: &str,
    text: &str,
) -> Result<SuiteManifest, String> {
    parse_suite_manifest(path, report_id, text)
}
