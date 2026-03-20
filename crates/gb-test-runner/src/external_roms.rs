use std::fmt;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use serde::Deserialize;

pub const EXTERNAL_ROM_STORE_DIR: &str = ".roms/external-test";
pub const LOCAL_COMMERCIAL_ROM_STORE_DIR: &str = ".roms/local-commercial";
pub const EXTERNAL_ROM_SOURCE_MANIFEST_PATH: &str =
    "crates/gb-test-runner/external-rom-sources.toml";
const SUPPORTED_EXTERNAL_ROM_SOURCE_MANIFEST_VERSION: u32 = 1;

#[derive(Debug)]
pub enum ExternalRomSourceManifestError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
    UnsupportedVersion { path: PathBuf, version: u32 },
}

impl fmt::Display for ExternalRomSourceManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    f,
                    "failed to read external ROM manifest {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, message } => {
                write!(
                    f,
                    "failed to parse external ROM manifest {}: {message}",
                    path.display()
                )
            }
            Self::UnsupportedVersion { path, version } => write!(
                f,
                "external ROM manifest {} uses unsupported version {}",
                path.display(),
                version
            ),
        }
    }
}

impl std::error::Error for ExternalRomSourceManifestError {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ExternalRomSourceManifestFile {
    version: u32,
    #[serde(rename = "source")]
    sources: Vec<ExternalRomSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExternalRomSource {
    pub id: String,
    pub git_url: String,
    pub git_rev: String,
    pub local_dir: String,
    pub root_env_var: String,
    #[serde(default, rename = "required_file")]
    pub required_files: Vec<ExternalRomRequiredFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExternalRomRequiredFile {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRomSourceManifest {
    sources: Vec<ExternalRomSource>,
}

impl ExternalRomSourceManifest {
    pub fn sources(&self) -> &[ExternalRomSource] {
        &self.sources
    }

    pub fn source_by_id(&self, id: &str) -> Option<&ExternalRomSource> {
        self.sources.iter().find(|source| source.id == id)
    }

    pub fn source_by_env_var(&self, env_var: &str) -> Option<&ExternalRomSource> {
        self.sources
            .iter()
            .find(|source| source.root_env_var == env_var)
    }
}

pub fn external_rom_store_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(EXTERNAL_ROM_STORE_DIR)
}

pub fn local_commercial_rom_store_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(LOCAL_COMMERCIAL_ROM_STORE_DIR)
}

pub fn external_rom_source_manifest_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(EXTERNAL_ROM_SOURCE_MANIFEST_PATH)
}

pub fn load_external_rom_source_manifest(
    workspace_root: &Path,
) -> Result<ExternalRomSourceManifest, ExternalRomSourceManifestError> {
    let path = external_rom_source_manifest_path(workspace_root);
    let manifest_text =
        fs::read_to_string(&path).map_err(|source| ExternalRomSourceManifestError::Read {
            path: path.clone(),
            source,
        })?;
    let parsed: ExternalRomSourceManifestFile =
        toml::from_str(&manifest_text).map_err(|error| ExternalRomSourceManifestError::Parse {
            path: path.clone(),
            message: error.to_string(),
        })?;

    if parsed.version != SUPPORTED_EXTERNAL_ROM_SOURCE_MANIFEST_VERSION {
        return Err(ExternalRomSourceManifestError::UnsupportedVersion {
            path,
            version: parsed.version,
        });
    }

    Ok(ExternalRomSourceManifest {
        sources: parsed.sources,
    })
}

pub fn default_external_rom_root_for_key(
    workspace_root: &Path,
    key: &str,
) -> Result<Option<PathBuf>, ExternalRomSourceManifestError> {
    let manifest = load_external_rom_source_manifest(workspace_root)?;
    Ok(manifest
        .source_by_env_var(key)
        .map(|source| external_rom_store_root(workspace_root).join(&source.local_dir)))
}

pub fn discover_external_rom_root_for_key(
    workspace_root: &Path,
    key: &str,
) -> Result<Option<PathBuf>, ExternalRomSourceManifestError> {
    if let Some(root) = env::var_os(key) {
        return Ok(Some(PathBuf::from(root)));
    }

    if let Some(default_root) = default_external_rom_root_for_key(workspace_root, key)?
        && default_root.exists()
    {
        return Ok(Some(default_root));
    }

    Ok(None)
}
