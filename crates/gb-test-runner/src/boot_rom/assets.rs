use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use gb_core::{
    BootRomAssetKind, BootRomAssets, ConsoleModel, HardwareRevision, HostPlatform, MachineConfig,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BootRomProfile {
    console_model: ConsoleModel,
    hardware_revision: HardwareRevision,
    host_platform: HostPlatform,
}

impl BootRomProfile {
    pub(crate) const fn new(
        console_model: ConsoleModel,
        hardware_revision: HardwareRevision,
        host_platform: HostPlatform,
    ) -> Self {
        Self {
            console_model,
            hardware_revision,
            host_platform,
        }
    }
}

#[derive(Debug)]
pub(crate) enum BootRomLoadError {
    DirectoryNotFound {
        path: PathBuf,
    },
    NotADirectory {
        path: PathBuf,
    },
    MissingFile {
        path: PathBuf,
        asset: BootRomAssetKind,
    },
    ReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    SizeMismatch {
        path: PathBuf,
        asset: BootRomAssetKind,
        expected: usize,
        actual: usize,
    },
    HashMismatch {
        path: PathBuf,
        asset: BootRomAssetKind,
        expected: &'static str,
        actual: String,
    },
    Assets {
        path: PathBuf,
        source: gb_core::BootRomAssetError,
    },
}

impl fmt::Display for BootRomLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryNotFound { path } => write!(
                formatter,
                "boot ROM asset directory does not exist: {}",
                path.display()
            ),
            Self::NotADirectory { path } => write!(
                formatter,
                "boot ROM asset path is not a directory: {}",
                path.display()
            ),
            Self::MissingFile { path, asset } => write!(
                formatter,
                "missing boot ROM asset {:?} at {}",
                asset,
                path.display()
            ),
            Self::ReadFailed { path, .. } => {
                write!(
                    formatter,
                    "failed to read boot ROM asset: {}",
                    path.display()
                )
            }
            Self::SizeMismatch {
                path,
                asset,
                expected,
                actual,
            } => write!(
                formatter,
                "boot ROM asset {:?} at {} has invalid size: expected {} bytes, got {}",
                asset,
                path.display(),
                expected,
                actual
            ),
            Self::HashMismatch {
                path,
                asset,
                expected,
                actual,
            } => write!(
                formatter,
                "boot ROM asset {:?} at {} has invalid SHA-256: expected {}, got {}",
                asset,
                path.display(),
                expected,
                actual
            ),
            Self::Assets { path, source } => write!(
                formatter,
                "failed to load verified boot ROM asset {}: {}",
                path.display(),
                source
            ),
        }
    }
}

impl std::error::Error for BootRomLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            Self::Assets { source, .. } => Some(source),
            Self::DirectoryNotFound { .. }
            | Self::NotADirectory { .. }
            | Self::MissingFile { .. }
            | Self::SizeMismatch { .. }
            | Self::HashMismatch { .. } => None,
        }
    }
}

pub(crate) fn asset_for_profile(
    console_model: ConsoleModel,
    hardware_revision: HardwareRevision,
    host_platform: HostPlatform,
) -> BootRomAssetKind {
    MachineConfig::new(console_model)
        .with_revision(hardware_revision)
        .with_host_platform(host_platform)
        .boot_rom_asset_kind()
}

pub(crate) fn load_verified_boot_rom_assets(
    root: &Path,
    profiles: &[BootRomProfile],
) -> Result<BootRomAssets, BootRomLoadError> {
    if !root.exists() {
        return Err(BootRomLoadError::DirectoryNotFound {
            path: root.to_path_buf(),
        });
    }
    if !root.is_dir() {
        return Err(BootRomLoadError::NotADirectory {
            path: root.to_path_buf(),
        });
    }

    let mut assets = BootRomAssets::none();
    for asset in required_assets(profiles) {
        let path = root.join(asset.filename());
        let bytes = read_verified_asset(&path, asset)?;
        assets
            .insert_asset_bytes(asset, bytes)
            .map_err(|source| BootRomLoadError::Assets { path, source })?;
    }
    Ok(assets)
}

fn required_assets(profiles: &[BootRomProfile]) -> Vec<BootRomAssetKind> {
    let mut assets = Vec::new();
    for profile in profiles {
        let asset = asset_for_profile(
            profile.console_model,
            profile.hardware_revision,
            profile.host_platform,
        );
        if !assets.contains(&asset) {
            assets.push(asset);
        }
    }
    assets
}

fn read_verified_asset(path: &Path, asset: BootRomAssetKind) -> Result<Vec<u8>, BootRomLoadError> {
    if !path.is_file() {
        return Err(BootRomLoadError::MissingFile {
            path: path.to_path_buf(),
            asset,
        });
    }
    let bytes = fs::read(path).map_err(|source| BootRomLoadError::ReadFailed {
        path: path.to_path_buf(),
        source,
    })?;
    let expected_size = asset.expected_size();
    if bytes.len() != expected_size {
        return Err(BootRomLoadError::SizeMismatch {
            path: path.to_path_buf(),
            asset,
            expected: expected_size,
            actual: bytes.len(),
        });
    }
    let digest = Sha256::digest(&bytes);
    let actual = format!("{digest:x}");
    let expected = asset.expected_sha256();
    if actual != expected {
        return Err(BootRomLoadError::HashMismatch {
            path: path.to_path_buf(),
            asset,
            expected,
            actual,
        });
    }
    Ok(bytes)
}

#[cfg(test)]
pub(super) fn required_assets_for_test(profiles: &[BootRomProfile]) -> Vec<BootRomAssetKind> {
    required_assets(profiles)
}
