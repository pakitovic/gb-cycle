use crate::host_io::{resolve_path, validate_explicit_directory_input, writeln_checked};
use crate::options::{BootRomVerificationMode, RunOptions};
use crate::report::format_boot_rom_asset_load_error;
use gb_core::{BootRomAssetKind, BootRomAssets, StartupMode};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) fn load_boot_rom_assets(
    options: &RunOptions,
    current_dir: &Path,
    stderr: &mut dyn Write,
) -> Result<BootRomAssets, String> {
    if options.startup_mode != StartupMode::RealBoot {
        return Ok(BootRomAssets::none());
    }

    let Some(root) = resolve_boot_rom_root(options.boot_rom_dir.as_deref(), current_dir) else {
        match options.boot_rom_verify {
            BootRomVerificationMode::Off => {}
            BootRomVerificationMode::Warn => {
                writeln_checked(
                    stderr,
                    "warning: boot ROM root is not configured; use --boot-rom-dir <dir>",
                )?;
            }
            BootRomVerificationMode::Strict => {
                return Err("boot ROM root is not configured; use --boot-rom-dir <dir>".to_string());
            }
        }
        return Ok(BootRomAssets::none());
    };
    validate_explicit_directory_input("--boot-rom-dir", options.boot_rom_dir.as_deref(), &root)?;
    let asset = BootRomAssetKind::from_machine_profile(
        options.effective_revision(),
        options
            .model
            .sgb_profile_for_standard(options.sgb_video_standard),
    );
    let image_path = root.join(asset.filename());
    match options.boot_rom_verify {
        BootRomVerificationMode::Off => {}
        BootRomVerificationMode::Warn => {
            if let Err(error) = verify_boot_rom_file(&image_path, asset) {
                writeln_checked(stderr, &format!("warning: {error}"))?;
            }
        }
        BootRomVerificationMode::Strict => {
            verify_boot_rom_file(&image_path, asset)?;
        }
    }

    if !root.is_dir() {
        return Ok(BootRomAssets::none());
    }

    match BootRomAssets::from_directory(&root) {
        Ok(assets) => Ok(assets),
        Err(error) => Err(format_boot_rom_asset_load_error(&root, error)),
    }
}

pub(crate) fn resolve_boot_rom_root(
    explicit_root: Option<&Path>,
    current_dir: &Path,
) -> Option<PathBuf> {
    if let Some(explicit_root) = explicit_root {
        return Some(resolve_path(current_dir, explicit_root));
    }
    None
}

pub(crate) fn verify_boot_rom_file(
    path: &Path,
    asset: impl Into<BootRomAssetKind>,
) -> Result<(), String> {
    let asset = asset.into();
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read boot ROM asset for {:?} at {}: {}",
            asset,
            path.display(),
            error
        )
    })?;
    if bytes.len() != asset.expected_size() {
        return Err(format!(
            "boot ROM asset for {:?} at {} has unexpected size: expected {}, got {}",
            asset,
            path.display(),
            asset.expected_size(),
            bytes.len()
        ));
    }
    let actual_sha256 = sha256_hex(&bytes);
    let expected_sha256 = asset.expected_sha256();
    if actual_sha256 != expected_sha256 {
        return Err(format!(
            "boot ROM asset for {:?} at {} has unexpected sha256: expected {}, got {}",
            asset,
            path.display(),
            expected_sha256,
            actual_sha256
        ));
    }
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}
