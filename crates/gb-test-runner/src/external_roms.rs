use std::fmt;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use serde::Deserialize;

pub const EXTERNAL_ROM_STORE_DIR: &str = ".roms/external-test";
pub const EXTERNAL_ROM_SOURCE_MANIFEST_PATH: &str = "crates/gb-test-runner/data/sources.toml";
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
    pub family: Option<String>,
    pub rom: Option<PathBuf>,
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

#[cfg(test)]
mod tests {
    use super::{
        EXTERNAL_ROM_SOURCE_MANIFEST_PATH, EXTERNAL_ROM_STORE_DIR, ExternalRomSourceManifestError,
        default_external_rom_root_for_key, discover_external_rom_root_for_key,
        external_rom_source_manifest_path, external_rom_store_root,
        load_external_rom_source_manifest,
    };
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-external-roms-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore the touched variables before dropping the guard.
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore the touched variables before dropping the guard.
        unsafe {
            env::remove_var(key);
        }
    }

    fn write_manifest(workspace_root: &Path, body: &str) -> PathBuf {
        let manifest_path = external_rom_source_manifest_path(workspace_root);
        fs::create_dir_all(
            manifest_path
                .parent()
                .expect("manifest path should have a parent"),
        )
        .expect("manifest parent should be creatable");
        fs::write(&manifest_path, body).expect("manifest should be writable");
        manifest_path
    }

    #[test]
    fn workspace_path_helpers_follow_repo_local_layout() {
        let workspace_root = Path::new("/tmp/gb-cycle-workspace");
        assert_eq!(
            external_rom_store_root(workspace_root),
            workspace_root.join(EXTERNAL_ROM_STORE_DIR)
        );
        assert_eq!(
            external_rom_source_manifest_path(workspace_root),
            workspace_root.join(EXTERNAL_ROM_SOURCE_MANIFEST_PATH)
        );
    }

    #[test]
    fn manifest_loading_and_lookup_cover_supported_catalog_paths() {
        let workspace_root = unique_temp_dir("manifest-success");
        let manifest_path = write_manifest(
            &workspace_root,
            r#"
version = 1

[[source]]
id = "retrio"
git_url = "https://example.invalid/retrio.git"
git_rev = "abc123"
local_dir = "retrio-gb-test-roms"
root_env_var = "GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"

[[source.required_file]]
path = "cpu_instrs/individual/01-special.gb"
sha256 = "01"

[[source]]
id = "gbemu-shootout"
git_url = "https://example.invalid/shootout.git"
git_rev = "def456"
local_dir = "gbemu-shootout"
root_env_var = "GB_CYCLE_GBEMU_SHOOTOUT_ROOT"
"#,
        );

        let manifest =
            load_external_rom_source_manifest(&workspace_root).expect("manifest should load");
        assert_eq!(manifest.sources().len(), 2);
        assert_eq!(
            manifest
                .source_by_id("retrio")
                .expect("retrio source should exist")
                .local_dir,
            "retrio-gb-test-roms"
        );
        assert_eq!(
            manifest
                .source_by_env_var("GB_CYCLE_GBEMU_SHOOTOUT_ROOT")
                .expect("shootout source should exist")
                .id,
            "gbemu-shootout"
        );

        let default_root =
            default_external_rom_root_for_key(&workspace_root, "GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT")
                .expect("default root lookup should succeed")
                .expect("retrio default root should exist in the manifest");
        assert_eq!(
            default_root,
            external_rom_store_root(&workspace_root).join("retrio-gb-test-roms")
        );

        let display = ExternalRomSourceManifestError::UnsupportedVersion {
            path: manifest_path,
            version: 9,
        }
        .to_string();
        assert!(display.contains("unsupported version 9"));
        assert!(manifest.source_by_id("missing").is_none());
        assert!(
            manifest
                .source_by_env_var("GB_CYCLE_UNKNOWN_EXTERNAL_ROOT")
                .is_none()
        );
    }

    #[test]
    fn manifest_loading_reports_missing_parse_and_version_errors() {
        let missing_root = unique_temp_dir("manifest-missing");
        let missing_error = load_external_rom_source_manifest(&missing_root)
            .expect_err("missing manifest should fail");
        assert!(matches!(
            missing_error,
            ExternalRomSourceManifestError::Read { .. }
        ));

        let parse_root = unique_temp_dir("manifest-parse");
        write_manifest(&parse_root, "version = 1\n[[source]]\nid = [");
        let parse_error = load_external_rom_source_manifest(&parse_root)
            .expect_err("invalid manifest should fail");
        assert!(matches!(
            parse_error,
            ExternalRomSourceManifestError::Parse { .. }
        ));

        let unsupported_root = unique_temp_dir("manifest-version");
        write_manifest(
            &unsupported_root,
            r#"
version = 7

[[source]]
id = "retrio"
git_url = "https://example.invalid/retrio.git"
git_rev = "abc123"
local_dir = "retrio-gb-test-roms"
root_env_var = "GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"
"#,
        );
        let unsupported_error = load_external_rom_source_manifest(&unsupported_root)
            .expect_err("unsupported manifest version should fail");
        assert!(matches!(
            unsupported_error,
            ExternalRomSourceManifestError::UnsupportedVersion { version: 7, .. }
        ));
    }

    #[test]
    fn discover_external_rom_root_prefers_env_then_existing_default_then_none() {
        let workspace_root = unique_temp_dir("discover-root");
        write_manifest(
            &workspace_root,
            r#"
version = 1

[[source]]
id = "retrio"
git_url = "https://example.invalid/retrio.git"
git_rev = "abc123"
local_dir = "retrio-gb-test-roms"
root_env_var = "GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"
"#,
        );

        let default_root = external_rom_store_root(&workspace_root).join("retrio-gb-test-roms");
        fs::create_dir_all(&default_root).expect("default root should be creatable");

        let _guard = crate::test_support::lock_env();
        let key = "GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT";
        let previous = env::var_os(key);
        remove_env_var(key);

        let discovered_default = discover_external_rom_root_for_key(&workspace_root, key)
            .expect("default discovery should succeed");
        assert_eq!(discovered_default, Some(default_root.clone()));

        let env_root = workspace_root.join("custom-env-root");
        set_env_var(key, &env_root);
        let discovered_env = discover_external_rom_root_for_key(&workspace_root, key)
            .expect("environment discovery should succeed");
        assert_eq!(discovered_env, Some(env_root.clone()));

        remove_env_var(key);
        fs::remove_dir_all(&default_root).expect("default root should be removable");
        let missing = discover_external_rom_root_for_key(&workspace_root, key)
            .expect("missing discovery should still succeed");
        assert_eq!(missing, None);

        match previous {
            Some(value) => set_env_var(key, value),
            None => remove_env_var(key),
        }
    }

    #[test]
    fn default_external_rom_root_returns_none_for_unknown_key() {
        let workspace_root = unique_temp_dir("default-none");
        write_manifest(
            &workspace_root,
            r#"
version = 1

[[source]]
id = "retrio"
git_url = "https://example.invalid/retrio.git"
git_rev = "abc123"
local_dir = "retrio-gb-test-roms"
root_env_var = "GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"
"#,
        );

        let default_root =
            default_external_rom_root_for_key(&workspace_root, "GB_CYCLE_UNKNOWN_EXTERNAL_ROOT")
                .expect("unknown key lookup should succeed");
        assert_eq!(default_root, None);
    }

    #[test]
    fn manifest_error_display_mentions_path_and_reason() {
        let path = PathBuf::from("/tmp/test-rom-sources.toml");
        let read = ExternalRomSourceManifestError::Read {
            path: path.clone(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        };
        assert!(
            read.to_string()
                .contains("failed to read external ROM manifest")
        );
        assert!(read.to_string().contains(path.to_string_lossy().as_ref()));

        let parse = ExternalRomSourceManifestError::Parse {
            path: path.clone(),
            message: "bad toml".to_string(),
        };
        assert!(
            parse
                .to_string()
                .contains("failed to parse external ROM manifest")
        );
        assert!(parse.to_string().contains("bad toml"));
    }
}
