use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use super::manifest::{Report, Source, SourceFamily, SourceFile};
use super::materialize::family_destination_relative_path;

pub(super) fn validate_id(id: &str, field: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    Ok(())
}

pub(super) fn validate_family_list(
    families: &[String],
    field: &str,
    report_id: &str,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for family in families {
        validate_id(family, field)?;
        if !seen.insert(family.as_str()) {
            return Err(format!(
                "duplicate family {family:?} in {field} for report {report_id:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_sparse_paths(
    source: &Source,
    report: &Report,
    family: &SourceFamily,
) -> Result<(), String> {
    if family.sparse_paths.is_empty() && source.requires_sparse_paths() {
        return Err(format!(
            "source family {:?} for source {:?} in report {:?} must define sparse_paths",
            family.id, source.id, report.id
        ));
    }
    let mut seen = BTreeSet::new();
    for sparse_path in &family.sparse_paths {
        validate_relative_path(sparse_path, "source family sparse path", false)?;
        let sparse_path_key = sparse_path.to_string_lossy();
        if !seen.insert(sparse_path_key.to_string()) {
            return Err(format!(
                "duplicate sparse path {} for source family {:?} in source {:?}",
                sparse_path.display(),
                family.id,
                source.id
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_source_files(
    source: &Source,
    report: &Report,
    family: &SourceFamily,
) -> Result<(), String> {
    if family.files.is_empty() {
        return Err(format!(
            "source family {:?} for source {:?} in report {:?} must define files",
            family.id, source.id, report.id
        ));
    }
    let mut source_paths = BTreeSet::new();
    for file in &family.files {
        validate_relative_path(&file.path, "source family file path", false)?;
        validate_relative_path(&file.target, "source family file target", false)?;
        validate_sha256(&file.sha256, source, report, family, file)?;
        if !source_paths.insert(file.path.to_string_lossy().to_string()) {
            return Err(format!(
                "duplicate source path {} for family {:?} in source {:?}",
                file.path.display(),
                family.id,
                source.id
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_sha256(
    sha256: &str,
    source: &Source,
    report: &Report,
    family: &SourceFamily,
    file: &SourceFile,
) -> Result<(), String> {
    if !is_valid_sha256(sha256) {
        return Err(format!(
            "invalid sha256 {:?} for source {} file {} in report {:?} family {:?}",
            sha256,
            source.id,
            file.path.display(),
            report.id,
            family.id
        ));
    }
    Ok(())
}

pub(super) fn is_valid_sha256(sha256: &str) -> bool {
    sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn validate_relative_path(
    path: &Path,
    field: &str,
    allow_empty: bool,
) -> Result<(), String> {
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

pub(super) fn validate_materialization_targets(
    report: &Report,
    sources: &[Source],
) -> Result<(), String> {
    let mut targets = BTreeMap::new();
    for source in sources {
        for family in &source.families {
            for file in &family.files {
                let target = family_destination_relative_path(family, file)?;
                validate_materialized_target_is_not_reserved(report, &target)?;
                if let Some(previous) = targets.insert(target.clone(), (&source.id, &family.id)) {
                    return Err(format!(
                        "duplicate materialization target {} for source {:?} family {:?}; already used by source {:?} family {:?}",
                        target.display(),
                        source.id,
                        family.id,
                        previous.0,
                        previous.1
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_materialized_target_is_not_reserved(
    report: &Report,
    target: &Path,
) -> Result<(), String> {
    let Some(first_component) = first_normal_component(target) else {
        return Err("materialization target must not be empty".to_string());
    };
    let reserved_roots = [
        report.status_dir.as_path(),
        report.artifact_dir.as_path(),
        report.report_file.as_path(),
    ];
    for reserved_root in reserved_roots {
        if let Some(reserved_component) = first_normal_component(reserved_root)
            && first_component == reserved_component
        {
            return Err(format!(
                "materialization target {} would overwrite reserved report path {} for report {:?}",
                target.display(),
                reserved_root.display(),
                report.id
            ));
        }
    }
    Ok(())
}

pub(super) fn first_normal_component(path: &Path) -> Option<&std::ffi::OsStr> {
    path.components().find_map(|component| match component {
        Component::Normal(component) => Some(component),
        _ => None,
    })
}
