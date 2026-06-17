use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use gb_core::BootRomAssetKind;

use super::git::{
    FetchedSource, cleanup_fetched_sources, fetch_sources_into_temps, sha256_hex, sha256_hex_eq,
};
use super::manifest::{DATA_DIR, Source, SourceLocation, SourceManifestFile};
use super::validate::{is_valid_sha256, validate_id, validate_relative_path};

const BOOT_ROM_SOURCE_MANIFEST_PATH: &str = "sources.boot-rom.toml";

const SUPPORTED_BOOT_ROM_ASSETS: [BootRomAssetKind; 10] = [
    BootRomAssetKind::Dmg0,
    BootRomAssetKind::Dmg,
    BootRomAssetKind::Mgb,
    BootRomAssetKind::Sgb,
    BootRomAssetKind::Sgb2,
    BootRomAssetKind::Cgb0,
    BootRomAssetKind::Cgb,
    BootRomAssetKind::CgbE,
    BootRomAssetKind::CgbAgb0,
    BootRomAssetKind::CgbAgb,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BootRomFetchRequest {
    pub(super) output_dir: PathBuf,
}

pub(super) fn run_boot_rom_fetch_request<W: Write>(
    request: BootRomFetchRequest,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String> {
    let manifest = load_boot_rom_source_manifest(workspace_root)?;
    validate_boot_rom_source_manifest(&manifest)?;
    let fetched_sources = fetch_sources_into_temps(&manifest.sources, output)?;
    let result = materialize_boot_rom_sources(&request.output_dir, &fetched_sources, output);
    let cleanup = cleanup_fetched_sources(&fetched_sources);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(format!("{error}; additionally {cleanup_error}")),
    }
}

fn load_boot_rom_source_manifest(workspace_root: &Path) -> Result<SourceManifestFile, String> {
    let path = workspace_root
        .join(DATA_DIR)
        .join(BOOT_ROM_SOURCE_MANIFEST_PATH);
    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read boot ROM source manifest {}: {error}",
            path.display()
        )
    })?;
    toml::from_str(&text).map_err(|error| {
        format!(
            "failed to parse boot ROM source manifest {}: {error}",
            path.display()
        )
    })
}

fn validate_boot_rom_source_manifest(manifest: &SourceManifestFile) -> Result<(), String> {
    if manifest.sources.len() != 1 {
        return Err(format!(
            "boot ROM source manifest must contain exactly one source, got {}",
            manifest.sources.len()
        ));
    }

    let source = &manifest.sources[0];
    validate_id(&source.id, "boot ROM source id")?;
    match source.location()? {
        SourceLocation::FileBase { file_base_url } if !file_base_url.is_empty() => {}
        SourceLocation::FileBase { .. } => {
            return Err(format!(
                "boot ROM source {:?} must define file_base_url",
                source.id
            ));
        }
        SourceLocation::Git { .. } | SourceLocation::Archive { .. } => {
            return Err(format!(
                "boot ROM source {:?} must use file_base_url",
                source.id
            ));
        }
    }

    let mut targets = BTreeMap::new();
    for family in &source.families {
        validate_id(&family.id, "boot ROM source family id")?;
        if !family.target_root.as_os_str().is_empty() {
            return Err(format!(
                "boot ROM source family {:?} must use an empty target_root",
                family.id
            ));
        }
        if !family.sparse_paths.is_empty() {
            return Err(format!(
                "boot ROM source family {:?} must not define sparse_paths",
                family.id
            ));
        }
        if family.files.is_empty() {
            return Err(format!(
                "boot ROM source family {:?} must define files",
                family.id
            ));
        }
        for file in &family.files {
            validate_relative_path(&file.path, "boot ROM source file path", false)?;
            validate_relative_path(&file.target, "boot ROM source file target", false)?;
            if !is_single_file_name(&file.target) {
                return Err(format!(
                    "boot ROM source file target {} must be a canonical filename",
                    file.target.display()
                ));
            }
            if !is_valid_sha256(&file.sha256) {
                return Err(format!(
                    "invalid sha256 {:?} for boot ROM source file {}",
                    file.sha256,
                    file.path.display()
                ));
            }
            let target = file.target.to_string_lossy().to_string();
            if targets
                .insert(target.clone(), (family.id.as_str(), file))
                .is_some()
            {
                return Err(format!("duplicate boot ROM source target {target}"));
            }
        }
    }

    let expected = supported_boot_rom_assets_by_filename();
    let actual = targets.keys().cloned().collect::<BTreeSet<_>>();
    let expected_names = expected.keys().cloned().collect::<BTreeSet<_>>();
    if actual != expected_names {
        let missing = expected_names
            .difference(&actual)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let unexpected = actual
            .difference(&expected_names)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "boot ROM source manifest target set mismatch: missing [{}], unexpected [{}]",
            missing, unexpected
        ));
    }

    for (target, (_, file)) in targets {
        let asset = expected
            .get(&target)
            .expect("target set should have been validated");
        let expected_size = u64::try_from(asset.expected_size()).expect("usize should fit u64");
        if file.size != Some(expected_size) {
            return Err(format!(
                "boot ROM source file {} must declare size {}",
                file.target.display(),
                expected_size
            ));
        }
    }

    Ok(())
}

fn materialize_boot_rom_sources<W: Write>(
    output_dir: &Path,
    fetched_sources: &[FetchedSource],
    output: &mut W,
) -> Result<(), String> {
    if output_dir.exists() && !output_dir.is_dir() {
        return Err(format!(
            "boot ROM output path is not a directory: {}",
            output_dir.display()
        ));
    }
    fs::create_dir_all(output_dir).map_err(|error| {
        format!(
            "failed to create boot ROM output directory {}: {error}",
            output_dir.display()
        )
    })?;

    let mut count = 0usize;
    for fetched_source in fetched_sources {
        for family in &fetched_source.source.families {
            for file in &family.files {
                let source_path = fetched_source.temp_root.join(&file.path);
                let target_path = output_dir.join(&file.target);
                verify_downloaded_boot_rom_file(&source_path, &fetched_source.source, file)?;
                fs::copy(&source_path, &target_path).map_err(|error| {
                    format!(
                        "failed to copy boot ROM asset {} -> {} for source {} family {}: {error}",
                        source_path.display(),
                        target_path.display(),
                        fetched_source.source.id,
                        family.id
                    )
                })?;
                count += 1;
            }
        }
    }

    writeln_checked(
        output,
        &format!(
            "materialized {count} boot ROM assets into {}",
            output_dir.display()
        ),
    )
}

fn verify_downloaded_boot_rom_file(
    path: &Path,
    source: &Source,
    file: &super::manifest::SourceFile,
) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read downloaded boot ROM asset {} for source {}: {error}",
            path.display(),
            source.id
        )
    })?;
    let Some(expected_size) = file.size else {
        return Err(format!(
            "boot ROM source file {} must declare size",
            file.target.display()
        ));
    };
    let actual_size = u64::try_from(bytes.len()).expect("usize should fit u64");
    if actual_size != expected_size {
        return Err(format!(
            "boot ROM asset {} has invalid size: expected {} bytes, got {}",
            file.target.display(),
            expected_size,
            actual_size
        ));
    }
    let actual_hash = sha256_hex(&bytes);
    if !sha256_hex_eq(&file.sha256, &actual_hash) {
        return Err(format!(
            "boot ROM asset {} has invalid SHA-256: expected {}, got {}",
            file.target.display(),
            file.sha256,
            actual_hash
        ));
    }
    Ok(())
}

fn supported_boot_rom_assets_by_filename() -> BTreeMap<String, BootRomAssetKind> {
    SUPPORTED_BOOT_ROM_ASSETS
        .into_iter()
        .map(|asset| (asset.filename().to_string(), asset))
        .collect()
}

fn is_single_file_name(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|file_name| Path::new(file_name) == path)
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}")
        .map_err(|error| format!("failed to write boot ROM fetch output: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("failed to flush boot ROM fetch output: {error}"))
}

#[cfg(test)]
pub(super) fn load_boot_rom_source_manifest_for_test(
    workspace_root: &Path,
) -> Result<SourceManifestFile, String> {
    let manifest = load_boot_rom_source_manifest(workspace_root)?;
    validate_boot_rom_source_manifest(&manifest)?;
    Ok(manifest)
}

#[cfg(test)]
pub(super) fn supported_boot_rom_asset_filenames_for_test() -> Vec<&'static str> {
    SUPPORTED_BOOT_ROM_ASSETS
        .iter()
        .map(|asset| asset.filename())
        .collect()
}
