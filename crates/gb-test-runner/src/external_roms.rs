use std::fmt;
use std::path::{Path, PathBuf};
use std::{fs, io};

use serde::Deserialize;

pub const EXTERNAL_ROM_SOURCE_MANIFEST_PATH: &str = "crates/gb-test-runner/data/sources.toml";
pub const DOCBOY_REPORT_ID: &str = "docboy";
pub const DOCBOY_SOURCE_MANIFEST_PATH: &str = "crates/gb-test-runner/data/docboy/sources.toml";
pub const GBMICROTEST_REPORT_ID: &str = "gbmicrotest";
pub const GBMICROTEST_SOURCE_MANIFEST_PATH: &str =
    "crates/gb-test-runner/data/gbmicrotest/sources.toml";
pub const GB_EMULATOR_SHOOTOUT_REPORT_ID: &str = "gb-emulator-shootout";
pub const GB_EMULATOR_SHOOTOUT_SOURCE_MANIFEST_PATH: &str =
    "crates/gb-test-runner/data/gb-emulator-shootout/sources.toml";

#[derive(Debug)]
pub enum ExternalRomSourceManifestError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
    UnknownReport { report_id: String },
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
            Self::UnknownReport { report_id } => {
                write!(f, "unknown external ROM report {report_id:?}")
            }
        }
    }
}

impl std::error::Error for ExternalRomSourceManifestError {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ExternalRomSourceManifestFile {
    #[serde(rename = "source")]
    sources: Vec<ExternalRomSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExternalRomSource {
    pub id: String,
    pub git_url: String,
    pub git_rev: String,
    #[serde(default, rename = "required_file")]
    pub required_files: Vec<ExternalRomRequiredFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExternalRomRequiredFile {
    pub path: PathBuf,
    pub family: Option<String>,
    pub rom: Option<PathBuf>,
    pub target: Option<PathBuf>,
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
}

pub fn external_rom_source_manifest_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(EXTERNAL_ROM_SOURCE_MANIFEST_PATH)
}

pub fn external_rom_source_manifest_path_for_report(
    workspace_root: &Path,
    report_id: Option<&str>,
) -> Result<PathBuf, ExternalRomSourceManifestError> {
    match report_id {
        Some(DOCBOY_REPORT_ID) => Ok(workspace_root.join(DOCBOY_SOURCE_MANIFEST_PATH)),
        Some(GBMICROTEST_REPORT_ID) => Ok(workspace_root.join(GBMICROTEST_SOURCE_MANIFEST_PATH)),
        Some(GB_EMULATOR_SHOOTOUT_REPORT_ID) => {
            Ok(workspace_root.join(GB_EMULATOR_SHOOTOUT_SOURCE_MANIFEST_PATH))
        }
        Some(report_id) => Err(ExternalRomSourceManifestError::UnknownReport {
            report_id: report_id.to_string(),
        }),
        None => Ok(external_rom_source_manifest_path(workspace_root)),
    }
}

pub fn load_external_rom_source_manifest(
    workspace_root: &Path,
) -> Result<ExternalRomSourceManifest, ExternalRomSourceManifestError> {
    load_external_rom_source_manifest_from_path(&external_rom_source_manifest_path(workspace_root))
}

pub fn load_external_rom_source_manifest_for_report(
    workspace_root: &Path,
    report_id: Option<&str>,
) -> Result<ExternalRomSourceManifest, ExternalRomSourceManifestError> {
    let path = external_rom_source_manifest_path_for_report(workspace_root, report_id)?;
    load_external_rom_source_manifest_from_path(&path)
}

fn load_external_rom_source_manifest_from_path(
    path: &Path,
) -> Result<ExternalRomSourceManifest, ExternalRomSourceManifestError> {
    let manifest_text =
        fs::read_to_string(path).map_err(|source| ExternalRomSourceManifestError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let parsed: ExternalRomSourceManifestFile =
        toml::from_str(&manifest_text).map_err(|error| ExternalRomSourceManifestError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    Ok(ExternalRomSourceManifest {
        sources: parsed.sources,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        DOCBOY_REPORT_ID, DOCBOY_SOURCE_MANIFEST_PATH, EXTERNAL_ROM_SOURCE_MANIFEST_PATH,
        ExternalRomSourceManifestError, GB_EMULATOR_SHOOTOUT_REPORT_ID,
        GB_EMULATOR_SHOOTOUT_SOURCE_MANIFEST_PATH, GBMICROTEST_REPORT_ID,
        GBMICROTEST_SOURCE_MANIFEST_PATH, external_rom_source_manifest_path,
        external_rom_source_manifest_path_for_report, load_external_rom_source_manifest,
        load_external_rom_source_manifest_for_report,
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
            external_rom_source_manifest_path(workspace_root),
            workspace_root.join(EXTERNAL_ROM_SOURCE_MANIFEST_PATH)
        );
        assert_eq!(
            external_rom_source_manifest_path_for_report(workspace_root, Some(DOCBOY_REPORT_ID))
                .expect("DocBoy report manifest path should resolve"),
            workspace_root.join(DOCBOY_SOURCE_MANIFEST_PATH)
        );
        assert_eq!(
            external_rom_source_manifest_path_for_report(
                workspace_root,
                Some(GB_EMULATOR_SHOOTOUT_REPORT_ID)
            )
            .expect("GB Emulator Shootout report manifest path should resolve"),
            workspace_root.join(GB_EMULATOR_SHOOTOUT_SOURCE_MANIFEST_PATH)
        );
        assert_eq!(
            external_rom_source_manifest_path_for_report(
                workspace_root,
                Some(GBMICROTEST_REPORT_ID)
            )
            .expect("gbmicrotest report manifest path should resolve"),
            workspace_root.join(GBMICROTEST_SOURCE_MANIFEST_PATH)
        );
        assert!(matches!(
            external_rom_source_manifest_path_for_report(workspace_root, Some("unknown-report")),
            Err(ExternalRomSourceManifestError::UnknownReport { .. })
        ));
    }

    #[test]
    fn manifest_loading_and_lookup_cover_supported_catalog_paths() {
        let workspace_root = unique_temp_dir("manifest-success");
        write_manifest(
            &workspace_root,
            r#"

[[source]]
id = "retrio"
git_url = "https://example.invalid/retrio.git"
git_rev = "abc123"

[[source.required_file]]
path = "cpu_instrs/individual/01-special.gb"
sha256 = "01"

[[source]]
id = "gbemu-shootout"
git_url = "https://example.invalid/shootout.git"
git_rev = "def456"
"#,
        );

        let manifest =
            load_external_rom_source_manifest(&workspace_root).expect("manifest should load");
        assert_eq!(manifest.sources().len(), 2);
        assert_eq!(
            manifest
                .source_by_id("retrio")
                .expect("retrio source should exist")
                .git_rev,
            "abc123"
        );
        assert!(manifest.source_by_id("missing").is_none());
    }

    #[test]
    fn manifest_loading_reports_missing_and_parse_errors() {
        let missing_root = unique_temp_dir("manifest-missing");
        let missing_error = load_external_rom_source_manifest(&missing_root)
            .expect_err("missing manifest should fail");
        assert!(matches!(
            missing_error,
            ExternalRomSourceManifestError::Read { .. }
        ));

        let parse_root = unique_temp_dir("manifest-parse");
        write_manifest(&parse_root, "[[source]]\nid = [");
        let parse_error = load_external_rom_source_manifest(&parse_root)
            .expect_err("invalid manifest should fail");
        assert!(matches!(
            parse_error,
            ExternalRomSourceManifestError::Parse { .. }
        ));
    }

    #[test]
    fn report_manifest_loading_uses_the_report_local_sources_file() {
        let workspace_root = unique_temp_dir("report-manifest-success");
        let report_manifest_path = external_rom_source_manifest_path_for_report(
            &workspace_root,
            Some(GB_EMULATOR_SHOOTOUT_REPORT_ID),
        )
        .expect("report manifest path should resolve");
        fs::create_dir_all(
            report_manifest_path
                .parent()
                .expect("report manifest path should have a parent"),
        )
        .expect("report manifest parent should be creatable");
        fs::write(
            &report_manifest_path,
            r#"

[[source]]
id = "gbemu-shootout"
git_url = "https://example.invalid/shootout.git"
git_rev = "def456"

[[source.required_file]]
path = "testroms/acid/which.gb"
family = "acid"
rom = "which.gb"
sha256 = "01"
"#,
        )
        .expect("report manifest should be writable");

        let manifest = load_external_rom_source_manifest_for_report(
            &workspace_root,
            Some(GB_EMULATOR_SHOOTOUT_REPORT_ID),
        )
        .expect("report manifest should load");
        assert_eq!(manifest.sources().len(), 1);
        assert_eq!(manifest.sources()[0].id, "gbemu-shootout");
        assert_eq!(
            manifest.sources()[0].required_files[0].path,
            PathBuf::from("testroms/acid/which.gb")
        );

        let legacy_error = load_external_rom_source_manifest(&workspace_root)
            .expect_err("legacy manifest path should remain independent");
        assert!(matches!(
            legacy_error,
            ExternalRomSourceManifestError::Read { .. }
        ));

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
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
