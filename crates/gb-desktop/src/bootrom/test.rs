use super::{
    MissingBootRomAsset, boot_rom_image_path, load_boot_rom_assets, load_exact_boot_rom_file,
    missing_boot_rom_asset, path_exists, resolve_boot_rom_source, resolve_path, sha256_hex,
    verify_boot_rom_file,
};
use gb_core::{BootRomAssetKind, BootRomAssets, HardwareRevision, StartupMode};
use gb_desktop::BootRomVerificationMode;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_root(label: &str) -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "gb-cycle-bootrom-tests-{label}-{}-{id}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale bootrom temp root should be removable");
    }
    fs::create_dir_all(&root).expect("bootrom temp root should be creatable");
    root
}

fn write_boot_rom_image(path: &Path, byte: u8) {
    fs::write(path, vec![byte; 0x100]).expect("synthetic boot ROM image should be writable");
}

#[path = "test/load_assets.rs"]
mod load_assets;
#[path = "test/load_exact.rs"]
mod load_exact;
#[path = "test/missing.rs"]
mod missing;
#[path = "test/paths.rs"]
mod paths;
#[path = "test/verify.rs"]
mod verify;
