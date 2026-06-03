use gb_core::{BootRomAssetKind, BootRomAssets, StartupMode};
use gb_desktop::BootRomVerificationMode;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingBootRomAsset {
    SourceUnconfigured,
    Path(PathBuf),
}

impl fmt::Display for MissingBootRomAsset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceUnconfigured => write!(
                f,
                "boot ROM root is not configured; choose a boot ROM directory"
            ),
            Self::Path(path) => write!(f, "boot ROM asset missing at {}", path.display()),
        }
    }
}

pub fn load_boot_rom_assets(
    search_path: Option<&Path>,
    verification_mode: BootRomVerificationMode,
    asset: impl Into<BootRomAssetKind>,
    startup_mode: StartupMode,
    current_dir: &Path,
) -> Result<BootRomAssets, String> {
    let asset = asset.into();
    if startup_mode != StartupMode::RealBoot {
        return Ok(BootRomAssets::none());
    }

    let Some(source) = resolve_boot_rom_source(search_path, current_dir) else {
        match verification_mode {
            BootRomVerificationMode::Off => {}
            BootRomVerificationMode::Warn => {
                eprintln!("warning: {}", MissingBootRomAsset::SourceUnconfigured)
            }
            BootRomVerificationMode::Strict => {
                return Err(MissingBootRomAsset::SourceUnconfigured.to_string());
            }
        }
        return Ok(BootRomAssets::none());
    };
    let image_path = boot_rom_image_path(&source, asset);
    match verification_mode {
        BootRomVerificationMode::Off => {}
        BootRomVerificationMode::Warn => {
            if let Err(error) = verify_boot_rom_file(&image_path, asset) {
                eprintln!("warning: {error}");
            }
        }
        BootRomVerificationMode::Strict => {
            verify_boot_rom_file(&image_path, asset)?;
        }
    }

    if source.is_file() {
        return load_exact_boot_rom_file(&source, asset);
    }

    if !source.is_dir() {
        return Ok(BootRomAssets::none());
    }

    BootRomAssets::from_directory(&source).map_err(|error| {
        format!(
            "failed to load boot ROM assets from {}: {error}",
            source.display()
        )
    })
}

pub fn missing_boot_rom_asset(
    search_path: Option<&Path>,
    asset: impl Into<BootRomAssetKind>,
    current_dir: &Path,
) -> Result<Option<MissingBootRomAsset>, String> {
    let asset = asset.into();
    let Some(source) = resolve_boot_rom_source(search_path, current_dir) else {
        return Ok(Some(MissingBootRomAsset::SourceUnconfigured));
    };
    if !path_exists(&source)? {
        return Ok(Some(MissingBootRomAsset::Path(source)));
    }
    if source.is_file() {
        return Ok(None);
    }
    if source.is_dir() {
        let image_path = boot_rom_image_path(&source, asset);
        if !path_exists(&image_path)? {
            return Ok(Some(MissingBootRomAsset::Path(image_path)));
        }
    }

    Ok(None)
}

fn resolve_boot_rom_source(explicit_source: Option<&Path>, current_dir: &Path) -> Option<PathBuf> {
    if let Some(explicit_source) = explicit_source {
        return Some(resolve_path(current_dir, explicit_source));
    }
    None
}

pub fn resolve_path(current_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_dir.join(path)
    }
}

fn path_exists(path: &Path) -> Result<bool, String> {
    path.try_exists().map_err(|error| {
        format!(
            "failed to inspect boot ROM path {}: {}",
            path.display(),
            error
        )
    })
}

fn boot_rom_image_path(source: &Path, asset: impl Into<BootRomAssetKind>) -> PathBuf {
    if source.is_file() {
        return source.to_path_buf();
    }

    source.join(asset.into().filename())
}

fn load_exact_boot_rom_file(
    path: &Path,
    asset: impl Into<BootRomAssetKind>,
) -> Result<BootRomAssets, String> {
    let asset = asset.into();
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read boot ROM asset {:?} at {}: {}",
            asset,
            path.display(),
            error
        )
    })?;
    BootRomAssets::none()
        .with_asset_bytes(asset, bytes)
        .map_err(|error| {
            format!(
                "failed to load boot ROM asset {:?} at {}: {error}",
                asset,
                path.display()
            )
        })
}

fn verify_boot_rom_file(path: &Path, asset: impl Into<BootRomAssetKind>) -> Result<(), String> {
    let asset = asset.into();
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read boot ROM asset {:?} at {}: {}",
            asset,
            path.display(),
            error
        )
    })?;
    let expected_size = asset.expected_size();
    if bytes.len() != expected_size {
        return Err(format!(
            "boot ROM asset {:?} at {} has unexpected size: expected {} bytes, got {}",
            asset,
            path.display(),
            expected_size,
            bytes.len()
        ));
    }
    let actual_sha256 = sha256_hex(&bytes);
    let expected_sha256 = asset.expected_sha256();
    if actual_sha256 != expected_sha256 {
        return Err(format!(
            "boot ROM asset {:?} at {} has unexpected sha256: expected {}, got {}",
            asset,
            path.display(),
            expected_sha256,
            actual_sha256
        ));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod test;
