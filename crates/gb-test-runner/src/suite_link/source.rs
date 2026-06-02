use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::model::{DATA_DIR, Report};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FamilyTargetRoots {
    loaded: bool,
    roots: BTreeMap<String, PathBuf>,
}

impl FamilyTargetRoots {
    fn fallback() -> Self {
        Self {
            loaded: false,
            roots: BTreeMap::new(),
        }
    }

    pub(super) fn target_root_for_family(&self, family: &str) -> Result<PathBuf, String> {
        if let Some(target_root) = self.roots.get(family) {
            return Ok(target_root.clone());
        }
        if self.loaded {
            return Err(format!(
                "source manifest does not define target_root for family {family:?}"
            ));
        }
        Ok(PathBuf::from(family))
    }

    #[cfg(test)]
    pub(super) fn fallback_for_test() -> Self {
        Self::fallback()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SourceManifestFile {
    #[serde(rename = "source")]
    sources: Vec<SourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SourceFile {
    #[serde(default, rename = "family")]
    families: Vec<SourceFamilyFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SourceFamilyFile {
    id: String,
    target_root: PathBuf,
}

pub(super) fn load_family_target_roots(
    workspace_root: &Path,
    report: &Report,
) -> Result<FamilyTargetRoots, String> {
    if report.local {
        return Ok(FamilyTargetRoots::fallback());
    }
    let sources = report.sources.as_ref().ok_or_else(|| {
        format!(
            "report {:?} must define sources unless local = true",
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
    let mut roots = BTreeMap::new();
    for source in manifest.sources {
        for family in source.families {
            validate_family_target_root(&family.target_root, &family.id, &path)?;
            match roots.entry(family.id.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(family.target_root);
                }
                Entry::Occupied(entry) => {
                    if entry.get() != &family.target_root {
                        return Err(format!(
                            "duplicate source family {:?} in source manifest {} uses target_root {} and {}",
                            family.id,
                            path.display(),
                            entry.get().display(),
                            family.target_root.display()
                        ));
                    }
                }
            }
        }
    }
    Ok(FamilyTargetRoots {
        loaded: true,
        roots,
    })
}

fn validate_family_target_root(
    path: &Path,
    family: &str,
    source_manifest: &Path,
) -> Result<(), String> {
    if path.is_absolute() {
        return Err(format!(
            "source family {family:?} target_root {} in {} must be relative",
            path.display(),
            source_manifest.display()
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::ParentDir => {
                return Err(format!(
                    "source family {family:?} target_root {} in {} must not contain parent components",
                    path.display(),
                    source_manifest.display()
                ));
            }
            Component::CurDir => {
                return Err(format!(
                    "source family {family:?} target_root {} in {} must not contain current-directory components",
                    path.display(),
                    source_manifest.display()
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "source family {family:?} target_root {} in {} must be relative",
                    path.display(),
                    source_manifest.display()
                ));
            }
        }
    }
    Ok(())
}
