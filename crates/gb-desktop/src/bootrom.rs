use gb_core::{BootRomAssets, BootRomKind, StartupMode};
use gb_desktop::{BootRomVerificationMode, DEFAULT_BOOT_ROM_DIR, DesktopConsoleModel};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_BOOT_ROM_ROOT_ENV_VAR: &str = "GB_CYCLE_BOOT_ROM_ROOT";

pub fn load_boot_rom_assets(
    search_path: Option<&Path>,
    verification_mode: BootRomVerificationMode,
    console_model: DesktopConsoleModel,
    startup_mode: StartupMode,
    current_dir: &Path,
) -> Result<BootRomAssets, String> {
    if startup_mode != StartupMode::RealBoot {
        return Ok(BootRomAssets::none());
    }

    let source = resolve_boot_rom_source(search_path, current_dir);
    let kind = console_model.boot_rom_kind();
    let image_path = boot_rom_image_path(&source, kind);
    match verification_mode {
        BootRomVerificationMode::Off => {}
        BootRomVerificationMode::Warn => {
            if let Err(error) = verify_boot_rom_file(&image_path, kind) {
                eprintln!("warning: {error}");
            }
        }
        BootRomVerificationMode::Strict => {
            verify_boot_rom_file(&image_path, kind)?;
        }
    }

    if source.is_file() {
        return load_exact_boot_rom_file(&source, kind);
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

fn resolve_boot_rom_source(explicit_source: Option<&Path>, current_dir: &Path) -> PathBuf {
    if let Some(explicit_source) = explicit_source {
        return resolve_path(current_dir, explicit_source);
    }
    if let Some(root) = env::var_os(DEFAULT_BOOT_ROM_ROOT_ENV_VAR) {
        return PathBuf::from(root);
    }
    current_dir.join(DEFAULT_BOOT_ROM_DIR)
}

pub fn resolve_path(current_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_dir.join(path)
    }
}

fn boot_rom_image_path(source: &Path, kind: BootRomKind) -> PathBuf {
    if source.is_file() {
        return source.to_path_buf();
    }

    source.join(BootRomAssets::filename(kind))
}

fn load_exact_boot_rom_file(path: &Path, kind: BootRomKind) -> Result<BootRomAssets, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read boot ROM asset {:?} at {}: {}",
            kind,
            path.display(),
            error
        )
    })?;
    BootRomAssets::none()
        .with_bytes(kind, bytes)
        .map_err(|error| {
            format!(
                "failed to load boot ROM asset {:?} at {}: {error}",
                kind,
                path.display()
            )
        })
}

fn verify_boot_rom_file(path: &Path, kind: BootRomKind) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read boot ROM asset {:?} at {}: {}",
            kind,
            path.display(),
            error
        )
    })?;
    let actual_sha256 = sha256_hex(&bytes);
    let expected_sha256 = expected_boot_rom_sha256(kind);
    if actual_sha256 != expected_sha256 {
        return Err(format!(
            "boot ROM asset {:?} at {} has unexpected sha256: expected {}, got {}",
            kind,
            path.display(),
            expected_sha256,
            actual_sha256
        ));
    }
    Ok(())
}

fn expected_boot_rom_sha256(kind: BootRomKind) -> &'static str {
    match kind {
        BootRomKind::Dmg0 => "26e71cf01e301e5dc40e987cd2ecbf6d0276245890ac829db2a25323da86818e",
        BootRomKind::Dmg => "cf053eccb4ccafff9e67339d4e78e98dce7d1ed59be819d2a1ba2232c6fce1c7",
        BootRomKind::Mgb => "a8cb5f4f1f16f2573ed2ecd8daedb9c5d1dd2c30a481f9b179b5d725d95eafe2",
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
