use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::git::{FetchedSource, remove_path_if_present};
use super::manifest::{Report, Source, SourceFamily, SourceFile};
use super::validate::{
    first_normal_component, validate_materialized_target_is_not_reserved, validate_relative_path,
};

pub(super) fn replace_selected_family_roots(
    store_root: &Path,
    report: &Report,
    sources: &[Source],
) -> Result<(), String> {
    fs::create_dir_all(store_root).map_err(|error| {
        format!(
            "failed to create test ROM store {}: {error}",
            store_root.display()
        )
    })?;
    let roots = selected_materialization_roots(report, sources)?;
    for root in roots {
        let root_path = store_root.join(&root);
        remove_path_if_present(&root_path, "selected test ROM family root")?;
    }
    Ok(())
}

fn selected_materialization_roots(
    report: &Report,
    sources: &[Source],
) -> Result<BTreeSet<PathBuf>, String> {
    let mut roots = BTreeSet::new();
    for source in sources {
        for family in &source.families {
            if family.target_root.as_os_str().is_empty() {
                for file in &family.files {
                    let target = family_destination_relative_path(family, file)?;
                    validate_materialized_target_is_not_reserved(report, &target)?;
                    let Some(component) = first_normal_component(&target) else {
                        return Err(format!(
                            "source {} family {} has empty target {}",
                            source.id,
                            family.id,
                            target.display()
                        ));
                    };
                    roots.insert(PathBuf::from(component));
                }
            } else {
                validate_materialized_target_is_not_reserved(report, &family.target_root)?;
                roots.insert(family.target_root.clone());
            }
        }
    }
    Ok(roots)
}

pub(super) fn materialize_selected_sources(
    store_root: &Path,
    fetched_sources: &[FetchedSource],
) -> Result<(), String> {
    for fetched_source in fetched_sources {
        for family in &fetched_source.source.families {
            for file in &family.files {
                let target = store_root.join(family_destination_relative_path(family, file)?);
                copy_source_file(
                    &fetched_source.temp_root,
                    &file.path,
                    &target,
                    &fetched_source.source.id,
                    &family.id,
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn family_destination_relative_path(
    family: &SourceFamily,
    file: &SourceFile,
) -> Result<PathBuf, String> {
    validate_relative_path(&family.target_root, "source family target_root", true)?;
    validate_relative_path(&file.target, "source family file target", false)?;
    if family.target_root.as_os_str().is_empty() {
        Ok(file.target.clone())
    } else {
        Ok(family.target_root.join(&file.target))
    }
}

fn copy_source_file(
    source_root: &Path,
    source_path: &Path,
    target_path: &Path,
    source_id: &str,
    family_id: &str,
) -> Result<(), String> {
    let source_path = source_root.join(source_path);
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create ROM parent {} for source {} family {}: {error}",
                parent.display(),
                source_id,
                family_id
            )
        })?;
    }
    fs::copy(&source_path, target_path).map_err(|error| {
        format!(
            "failed to copy ROM {} -> {} for source {} family {}: {error}",
            source_path.display(),
            target_path.display(),
            source_id,
            family_id
        )
    })?;
    Ok(())
}

pub(super) fn store_root_for_report(workspace_root: &Path, report: &Report) -> PathBuf {
    let base = workspace_root.join("test");
    if report.store_dir.as_os_str().is_empty() {
        base
    } else {
        base.join(&report.store_dir)
    }
}
