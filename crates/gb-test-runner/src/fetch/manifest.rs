use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::validate::{
    validate_family_list, validate_id, validate_relative_path, validate_source_files,
    validate_sparse_paths,
};
pub(super) const DATA_DIR: &str = "crates/gb-test-runner/data";
pub(super) const REPORTS_MANIFEST_PATH: &str = "crates/gb-test-runner/data/reports.toml";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ReportManifestFile {
    pub(super) status_dir: Option<PathBuf>,
    pub(super) artifact_dir: Option<PathBuf>,
    pub(super) report_file: Option<PathBuf>,
    #[serde(rename = "report")]
    pub(super) reports: Vec<ReportFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ReportFile {
    pub(super) id: String,
    #[serde(default)]
    pub(super) local: bool,
    pub(super) store_dir: PathBuf,
    pub(super) sources: Option<PathBuf>,
    pub(super) status_dir: Option<PathBuf>,
    pub(super) artifact_dir: Option<PathBuf>,
    pub(super) report_file: Option<PathBuf>,
    pub(super) family_order: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportManifest {
    pub(super) reports: Vec<Report>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Report {
    pub(super) id: String,
    pub(super) local: bool,
    pub(super) store_dir: PathBuf,
    pub(super) sources: Option<PathBuf>,
    pub(super) status_dir: PathBuf,
    pub(super) artifact_dir: PathBuf,
    pub(super) report_file: PathBuf,
    pub(super) family_order: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct SourceManifestFile {
    #[serde(rename = "source")]
    pub(super) sources: Vec<Source>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct Source {
    pub(super) id: String,
    pub(super) git_url: Option<String>,
    pub(super) git_rev: Option<String>,
    pub(super) file_base_url: Option<String>,
    pub(super) archive_url: Option<String>,
    pub(super) archive_sha256: Option<String>,
    pub(super) archive_format: Option<SourceArchiveFormat>,
    #[serde(default, rename = "family")]
    pub(super) families: Vec<SourceFamily>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SourceArchiveFormat {
    Zip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceLocation<'a> {
    Git {
        git_url: &'a str,
        git_rev: &'a str,
    },
    Archive {
        archive_url: &'a str,
        archive_sha256: &'a str,
        archive_format: SourceArchiveFormat,
    },
    FileBase {
        file_base_url: &'a str,
    },
}

impl Source {
    pub(super) fn location(&self) -> Result<SourceLocation<'_>, String> {
        match (
            self.git_url.as_deref(),
            self.git_rev.as_deref(),
            self.file_base_url.as_deref(),
            self.archive_url.as_deref(),
            self.archive_sha256.as_deref(),
            self.archive_format,
        ) {
            (Some(git_url), Some(git_rev), None, None, None, None) => {
                Ok(SourceLocation::Git { git_url, git_rev })
            }
            (None, None, Some(file_base_url), None, None, None) => {
                Ok(SourceLocation::FileBase { file_base_url })
            }
            (None, None, None, Some(archive_url), Some(archive_sha256), Some(archive_format)) => {
                Ok(SourceLocation::Archive {
                    archive_url,
                    archive_sha256,
                    archive_format,
                })
            }
            _ => Err(format!(
                "source {:?} must define exactly one fetch location: git_url + git_rev, file_base_url, or archive_url + archive_sha256 + archive_format",
                self.id
            )),
        }
    }

    pub(super) fn requires_sparse_paths(&self) -> bool {
        matches!(self.location(), Ok(SourceLocation::Git { .. }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct SourceFamily {
    pub(super) id: String,
    pub(super) target_root: PathBuf,
    #[serde(default)]
    pub(super) sparse_paths: Vec<PathBuf>,
    #[serde(default, rename = "file")]
    pub(super) files: Vec<SourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct SourceFile {
    pub(super) path: PathBuf,
    pub(super) target: PathBuf,
    pub(super) size: Option<u64>,
    pub(super) sha256: String,
}

pub(super) fn load_report_manifest(workspace_root: &Path) -> Result<ReportManifest, String> {
    let path = workspace_root.join(REPORTS_MANIFEST_PATH);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read report manifest {}: {error}", path.display()))?;
    let manifest: ReportManifestFile = toml::from_str(&text).map_err(|error| {
        format!(
            "failed to parse report manifest {}: {error}",
            path.display()
        )
    })?;
    resolve_report_manifest(manifest)
}

pub(super) fn load_source_manifest(
    workspace_root: &Path,
    report: &Report,
) -> Result<SourceManifestFile, String> {
    let sources = report.sources.as_ref().ok_or_else(|| {
        format!(
            "report {:?} is local and does not define fetch sources",
            report.id
        )
    })?;
    let path = workspace_root.join(DATA_DIR).join(sources);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read source manifest {}: {error}", path.display()))?;
    let manifest: SourceManifestFile = toml::from_str(&text).map_err(|error| {
        format!(
            "failed to parse source manifest {}: {error}",
            path.display()
        )
    })?;
    validate_source_manifest(report, &manifest)?;
    Ok(manifest)
}

fn resolve_report_manifest(manifest: ReportManifestFile) -> Result<ReportManifest, String> {
    if manifest.reports.is_empty() {
        return Err("report manifest must contain at least one report".to_string());
    }
    if let Some(status_dir) = &manifest.status_dir {
        validate_relative_path(status_dir, "report default status_dir", false)?;
    }
    if let Some(artifact_dir) = &manifest.artifact_dir {
        validate_relative_path(artifact_dir, "report default artifact_dir", false)?;
    }
    if let Some(report_file) = &manifest.report_file {
        validate_relative_path(report_file, "report default report_file", false)?;
    }
    let mut seen_reports = BTreeSet::new();
    let mut reports = Vec::with_capacity(manifest.reports.len());
    for report in manifest.reports {
        validate_id(&report.id, "report id")?;
        if !seen_reports.insert(report.id.clone()) {
            return Err(format!("duplicate report id {:?}", report.id));
        }
        validate_relative_path(&report.store_dir, "report store_dir", true)?;
        match (report.local, &report.sources) {
            (true, Some(_)) => {
                return Err(format!(
                    "local report {:?} must not define sources",
                    report.id
                ));
            }
            (true, None) => {}
            (false, Some(sources)) => validate_relative_path(sources, "report sources", false)?,
            (false, None) => {
                return Err(format!(
                    "report {:?} must define sources unless local = true",
                    report.id
                ));
            }
        }

        let status_dir = report
            .status_dir
            .or_else(|| manifest.status_dir.clone())
            .ok_or_else(|| {
                format!(
                    "report {:?} must define status_dir or inherit a report default status_dir",
                    report.id
                )
            })?;
        let artifact_dir = report
            .artifact_dir
            .or_else(|| manifest.artifact_dir.clone())
            .ok_or_else(|| {
                format!(
                    "report {:?} must define artifact_dir or inherit a report default artifact_dir",
                    report.id
                )
            })?;
        let report_file = report
            .report_file
            .or_else(|| manifest.report_file.clone())
            .ok_or_else(|| {
                format!(
                    "report {:?} must define report_file or inherit a report default report_file",
                    report.id
                )
            })?;
        validate_relative_path(&status_dir, "report status_dir", false)?;
        validate_relative_path(&artifact_dir, "report artifact_dir", false)?;
        validate_relative_path(&report_file, "report report_file", false)?;
        if let Some(family_order) = &report.family_order {
            validate_family_list(family_order, "family_order", &report.id)?;
        }

        reports.push(Report {
            id: report.id,
            local: report.local,
            store_dir: report.store_dir,
            sources: report.sources,
            status_dir,
            artifact_dir,
            report_file,
            family_order: report.family_order,
        });
    }

    Ok(ReportManifest { reports })
}

fn validate_source_manifest(report: &Report, manifest: &SourceManifestFile) -> Result<(), String> {
    if manifest.sources.is_empty() {
        return Err(format!(
            "source manifest for report {:?} must contain at least one source",
            report.id
        ));
    }
    let mut source_ids = BTreeSet::new();
    for source in &manifest.sources {
        validate_id(&source.id, "source id")?;
        if !source_ids.insert(source.id.as_str()) {
            return Err(format!(
                "duplicate source id {:?} in report {:?}",
                source.id, report.id
            ));
        }
        match source.location()? {
            SourceLocation::Git { git_url, git_rev } => {
                if git_url.is_empty() {
                    return Err(format!(
                        "source {:?} in report {:?} must define git_url",
                        source.id, report.id
                    ));
                }
                if git_rev.is_empty() {
                    return Err(format!(
                        "source {:?} in report {:?} must define git_rev",
                        source.id, report.id
                    ));
                }
            }
            SourceLocation::Archive {
                archive_url,
                archive_sha256,
                archive_format: _,
            } => {
                if archive_url.is_empty() {
                    return Err(format!(
                        "source {:?} in report {:?} must define archive_url",
                        source.id, report.id
                    ));
                }
                if !super::validate::is_valid_sha256(archive_sha256) {
                    return Err(format!(
                        "invalid archive_sha256 {:?} for source {:?} in report {:?}",
                        archive_sha256, source.id, report.id
                    ));
                }
            }
            SourceLocation::FileBase { file_base_url } => {
                if file_base_url.is_empty() {
                    return Err(format!(
                        "source {:?} in report {:?} must define file_base_url",
                        source.id, report.id
                    ));
                }
            }
        }
        let mut source_families = BTreeSet::new();
        for family in &source.families {
            validate_id(&family.id, "source family id")?;
            if !source_families.insert(family.id.as_str()) {
                return Err(format!(
                    "duplicate source family {:?} for source {:?} in report {:?}",
                    family.id, source.id, report.id
                ));
            }
            validate_relative_path(&family.target_root, "source family target_root", true)?;
            validate_sparse_paths(source, report, family)?;
            validate_source_files(source, report, family)?;
        }
    }
    Ok(())
}

pub(super) fn report_families(
    report: &Report,
    source_manifest: &SourceManifestFile,
) -> Result<Vec<String>, String> {
    let source_families = source_manifest_family_set(source_manifest);
    if source_families.is_empty() {
        return Err(format!(
            "source manifest for report {:?} must define at least one source family",
            report.id
        ));
    }

    if let Some(family_order) = &report.family_order {
        let ordered_families = family_order
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let missing_families = family_order
            .iter()
            .filter(|family| !source_families.contains(family.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_families.is_empty() {
            return Err(format!(
                "report {:?} family_order contains families without source files: {}",
                report.id,
                missing_families.join(", ")
            ));
        }
        let unordered_families = source_families
            .iter()
            .filter(|family| !ordered_families.contains(family.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !unordered_families.is_empty() {
            return Err(format!(
                "source manifest for report {:?} contains families missing from family_order: {}",
                report.id,
                unordered_families.join(", ")
            ));
        }
        return Ok(family_order.clone());
    }

    Ok(source_families.into_iter().collect())
}

fn source_manifest_family_set(source_manifest: &SourceManifestFile) -> BTreeSet<String> {
    source_manifest
        .sources
        .iter()
        .flat_map(|source| &source.families)
        .map(|family| family.id.clone())
        .collect()
}

pub(super) fn select_families(
    report: &Report,
    available_families: &[String],
    requested_families: &[String],
) -> Result<Vec<String>, String> {
    if requested_families.is_empty() {
        return Ok(available_families.to_vec());
    }
    let available_family_set = available_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut requested_family_set = BTreeSet::new();
    for family in requested_families {
        if !available_family_set.contains(family.as_str()) {
            return Err(format!(
                "unknown test ROM family {family:?} for report {:?}; available families: {}",
                report.id,
                available_families.join(", ")
            ));
        }
        if !requested_family_set.insert(family.as_str()) {
            return Err(format!(
                "duplicate test ROM family {family:?} in fetch selection"
            ));
        }
    }
    Ok(available_families
        .iter()
        .filter(|family| requested_family_set.contains(family.as_str()))
        .cloned()
        .collect())
}

pub(super) fn filter_sources_for_families(
    sources: &[Source],
    report: &Report,
    selected_families: &[String],
) -> Result<Vec<Source>, String> {
    let selected_family_set = selected_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut matched_families = BTreeSet::new();
    let filtered_sources = sources
        .iter()
        .filter_map(|source| {
            let families = source
                .families
                .iter()
                .filter(|family| selected_family_set.contains(family.id.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            for family in &families {
                matched_families.insert(family.id.clone());
            }
            if families.is_empty() {
                None
            } else {
                Some(Source {
                    families,
                    ..source.clone()
                })
            }
        })
        .collect::<Vec<_>>();

    if filtered_sources.is_empty() {
        return Err(format!(
            "no source files matched report {:?} family selection {}",
            report.id,
            selected_families.join(", ")
        ));
    }
    let missing_families = selected_families
        .iter()
        .filter(|family| !matched_families.contains(family.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_families.is_empty() {
        return Err(format!(
            "no source files matched report {:?} family selection {}",
            report.id,
            missing_families.join(", ")
        ));
    }
    Ok(filtered_sources)
}
