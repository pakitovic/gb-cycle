use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use super::cli::{FetchRequest, run_fetch_request, writeln_checked};
use super::git::sha256_hex;
use super::manifest::{
    Report, filter_sources_for_families, load_report_manifest, load_source_manifest,
    report_families, select_families,
};
use super::materialize::{family_destination_relative_path, store_root_for_report};
use super::validate::validate_materialization_targets;

#[derive(Debug, Clone, PartialEq, Eq)]
struct MaterializationIssue {
    family: String,
    reason: String,
}

pub(crate) fn ensure_report_families_materialized<W: Write>(
    workspace_root: &Path,
    report_id: &str,
    requested_families: &[String],
    output: &mut W,
) -> Result<(), String> {
    let reports = load_report_manifest(workspace_root)?;
    let report = report_for_id(report_id, &reports.reports)?;
    if report.local {
        return Ok(());
    }
    let source_manifest = load_source_manifest(workspace_root, report)?;
    let available_families = report_families(report, &source_manifest)?;
    let selected_families = select_families(report, &available_families, requested_families)?;
    let filtered_sources =
        filter_sources_for_families(&source_manifest.sources, report, &selected_families)?;
    validate_materialization_targets(report, &filtered_sources)?;

    let issues = materialization_issues(workspace_root, report, &filtered_sources)?;
    if issues.is_empty() {
        return Ok(());
    }

    for issue in issues.values() {
        writeln_checked(
            output,
            &format!(
                "test ROM family {} requires materialization: {}",
                issue.family, issue.reason
            ),
        )?;
    }
    run_fetch_request(
        FetchRequest {
            report_id: Some(report_id.to_string()),
            requested_families: issues.keys().cloned().collect(),
        },
        workspace_root,
        output,
    )
}

fn report_for_id<'a>(report_id: &str, reports: &'a [Report]) -> Result<&'a Report, String> {
    reports
        .iter()
        .find(|report| report.id == report_id)
        .ok_or_else(|| format!("unknown test ROM report {report_id:?}"))
}

fn materialization_issues(
    workspace_root: &Path,
    report: &Report,
    sources: &[super::manifest::Source],
) -> Result<BTreeMap<String, MaterializationIssue>, String> {
    let store_root = store_root_for_report(workspace_root, report);
    let mut issues = BTreeMap::new();
    for source in sources {
        for family in &source.families {
            for file in &family.files {
                if issues.contains_key(&family.id) {
                    continue;
                }
                let target = store_root.join(family_destination_relative_path(family, file)?);
                match fs::read(&target) {
                    Ok(bytes) => {
                        let actual_hash = sha256_hex(&bytes);
                        if actual_hash != file.sha256 {
                            issues.insert(
                                family.id.clone(),
                                MaterializationIssue {
                                    family: family.id.clone(),
                                    reason: format!(
                                        "hash mismatch for {}: expected {}, got {}",
                                        target.display(),
                                        file.sha256,
                                        actual_hash
                                    ),
                                },
                            );
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        issues.insert(
                            family.id.clone(),
                            MaterializationIssue {
                                family: family.id.clone(),
                                reason: format!("missing {}", target.display()),
                            },
                        );
                    }
                    Err(error) => {
                        return Err(format!(
                            "failed to read materialized file {} for report {:?} family {:?}: {error}",
                            target.display(),
                            report.id,
                            family.id
                        ));
                    }
                }
            }
        }
    }
    Ok(issues)
}
