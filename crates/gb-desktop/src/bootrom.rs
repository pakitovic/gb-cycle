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

pub fn missing_boot_rom_asset_path(
    search_path: Option<&Path>,
    console_model: DesktopConsoleModel,
    current_dir: &Path,
) -> Result<Option<PathBuf>, String> {
    let source = resolve_boot_rom_source(search_path, current_dir);
    let kind = console_model.boot_rom_kind();

    if !path_exists(&source)? {
        return Ok(Some(source));
    }
    if source.is_file() {
        return Ok(None);
    }
    if source.is_dir() {
        let image_path = boot_rom_image_path(&source, kind);
        if !path_exists(&image_path)? {
            return Ok(Some(image_path));
        }
    }

    Ok(None)
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

fn path_exists(path: &Path) -> Result<bool, String> {
    path.try_exists().map_err(|error| {
        format!(
            "failed to inspect boot ROM path {}: {}",
            path.display(),
            error
        )
    })
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

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_BOOT_ROM_ROOT_ENV_VAR, boot_rom_image_path, expected_boot_rom_sha256,
        load_boot_rom_assets, load_exact_boot_rom_file, missing_boot_rom_asset_path,
        resolve_boot_rom_source, resolve_path, sha256_hex, verify_boot_rom_file,
    };
    use gb_core::{BootRomAssets, BootRomKind, StartupMode};
    use gb_desktop::{BootRomVerificationMode, DEFAULT_BOOT_ROM_DIR, DesktopConsoleModel};
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn resolve_path_joins_relative_paths_and_preserves_absolute_paths() {
        let current_dir = Path::new("/tmp/gb-cycle");

        assert_eq!(
            resolve_path(current_dir, Path::new("boot/dmg_boot.bin")),
            PathBuf::from("/tmp/gb-cycle/boot/dmg_boot.bin")
        );
        assert_eq!(
            resolve_path(current_dir, Path::new("/var/tmp/dmg_boot.bin")),
            PathBuf::from("/var/tmp/dmg_boot.bin")
        );
    }

    #[test]
    fn resolve_boot_rom_source_prefers_explicit_paths_then_env_then_default_directory() {
        let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let current_dir = Path::new("/tmp/gb-cycle");

        assert_eq!(
            resolve_boot_rom_source(Some(Path::new("firmware")), current_dir),
            PathBuf::from("/tmp/gb-cycle/firmware")
        );

        unsafe {
            env::set_var(DEFAULT_BOOT_ROM_ROOT_ENV_VAR, "/tmp/env-bootrom");
        }
        assert_eq!(
            resolve_boot_rom_source(None, current_dir),
            PathBuf::from("/tmp/env-bootrom")
        );
        unsafe {
            env::remove_var(DEFAULT_BOOT_ROM_ROOT_ENV_VAR);
        }

        assert_eq!(
            resolve_boot_rom_source(None, current_dir),
            PathBuf::from("/tmp/gb-cycle").join(DEFAULT_BOOT_ROM_DIR)
        );
    }

    #[test]
    fn boot_rom_image_path_uses_the_exact_file_or_kind_filename() {
        let root = temp_root("image-path");
        let exact_file = root.join("mgb_boot.bin");
        let directory = root.join("bootrom");
        write_boot_rom_image(&exact_file, 0x77);
        fs::create_dir_all(&directory).expect("bootrom test directory should be creatable");

        assert_eq!(
            boot_rom_image_path(&exact_file, BootRomKind::Mgb),
            exact_file
        );
        assert_eq!(
            boot_rom_image_path(&directory, BootRomKind::Dmg),
            directory.join(BootRomAssets::filename(BootRomKind::Dmg))
        );

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn sha_and_expected_sha_helpers_cover_all_supported_dmg_family_kinds() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(expected_boot_rom_sha256(BootRomKind::Dmg0).len(), 64);
        assert_eq!(expected_boot_rom_sha256(BootRomKind::Dmg).len(), 64);
        assert_eq!(expected_boot_rom_sha256(BootRomKind::Mgb).len(), 64);
    }

    #[test]
    fn verify_boot_rom_file_reports_hash_mismatches_and_missing_files() {
        let root = temp_root("verify");
        let image_path = root.join("dmg_boot.bin");
        write_boot_rom_image(&image_path, 0xAA);

        let mismatch = verify_boot_rom_file(&image_path, BootRomKind::Dmg)
            .expect_err("synthetic image should not match the pinned SHA");
        assert!(mismatch.contains("unexpected sha256"));
        assert!(mismatch.contains("expected"));

        let missing = verify_boot_rom_file(&root.join("missing.bin"), BootRomKind::Dmg)
            .expect_err("missing file should surface a read error");
        assert!(missing.contains("failed to read boot ROM asset"));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_exact_boot_rom_file_loads_bytes_for_the_requested_kind() {
        let root = temp_root("exact-file");
        let image_path = root.join("mgb_boot.bin");
        write_boot_rom_image(&image_path, 0x5A);

        let assets = load_exact_boot_rom_file(&image_path, BootRomKind::Mgb)
            .expect("synthetic boot ROM file should load");
        assert!(assets.has_image(BootRomKind::Mgb));
        assert_eq!(assets.read_byte(BootRomKind::Mgb, 0), Some(0x5A));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_exact_boot_rom_file_reports_read_failures_and_invalid_lengths() {
        let root = temp_root("exact-errors");
        let missing = load_exact_boot_rom_file(&root.join("missing.bin"), BootRomKind::Dmg)
            .expect_err("missing exact boot ROM files should fail");
        assert!(missing.contains("failed to read boot ROM asset"));

        let short_image = root.join("short.bin");
        fs::write(&short_image, vec![0x11; 0x40]).expect("short boot ROM image should be writable");
        let invalid_len = load_exact_boot_rom_file(&short_image, BootRomKind::Dmg)
            .expect_err("invalid boot ROM image lengths should fail");
        assert!(invalid_len.contains("failed to load boot ROM asset"));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_boot_rom_assets_can_read_a_directory_backed_boot_rom_set() {
        let root = temp_root("directory-assets");
        let directory = root.join("bootrom");
        fs::create_dir_all(&directory).expect("boot ROM directory should be creatable");
        write_boot_rom_image(
            &directory.join(BootRomAssets::filename(BootRomKind::Dmg)),
            0x42,
        );

        let assets = load_boot_rom_assets(
            Some(&directory),
            BootRomVerificationMode::Off,
            DesktopConsoleModel::Dmg,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("directory-backed boot ROM assets should load");
        assert_eq!(assets.read_byte(BootRomKind::Dmg, 0), Some(0x42));
        assert!(!assets.has_image(BootRomKind::Mgb));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_boot_rom_assets_respects_startup_mode_and_verification_policy() {
        let root = temp_root("load-assets");
        let image_path = root.join("dmg_boot.bin");
        write_boot_rom_image(&image_path, 0xC3);

        let skip_boot = load_boot_rom_assets(
            Some(&image_path),
            BootRomVerificationMode::Strict,
            DesktopConsoleModel::Dmg,
            StartupMode::SkipBoot,
            Path::new("/unused"),
        )
        .expect("skip-boot should bypass firmware loading");
        assert!(skip_boot.is_empty());

        let off = load_boot_rom_assets(
            Some(&image_path),
            BootRomVerificationMode::Off,
            DesktopConsoleModel::Dmg,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("verification-off should load exact files");
        assert_eq!(off.read_byte(BootRomKind::Dmg, 0), Some(0xC3));

        let warn = load_boot_rom_assets(
            Some(&image_path),
            BootRomVerificationMode::Warn,
            DesktopConsoleModel::Dmg,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("warning mode should allow hash mismatches");
        assert_eq!(warn.read_byte(BootRomKind::Dmg, 0), Some(0xC3));

        let strict = load_boot_rom_assets(
            Some(&image_path),
            BootRomVerificationMode::Strict,
            DesktopConsoleModel::Dmg,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect_err("strict verification should reject synthetic hashes");
        assert!(strict.contains("unexpected sha256"));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_boot_rom_assets_returns_none_when_the_source_path_does_not_exist() {
        let assets = load_boot_rom_assets(
            Some(Path::new("/definitely/missing/bootrom")),
            BootRomVerificationMode::Off,
            DesktopConsoleModel::Dmg0,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("missing firmware directory should degrade to no assets");

        assert!(assets.is_empty());
    }

    #[test]
    fn missing_boot_rom_asset_path_detects_missing_exact_files() {
        let root = temp_root("missing-exact");
        let exact_file = root.join("mgb_boot.bin");

        assert_eq!(
            missing_boot_rom_asset_path(
                Some(&exact_file),
                DesktopConsoleModel::Mgb,
                Path::new("/unused"),
            )
            .expect("missing exact files should resolve cleanly"),
            Some(exact_file.clone())
        );

        write_boot_rom_image(&exact_file, 0x33);
        assert_eq!(
            missing_boot_rom_asset_path(
                Some(&exact_file),
                DesktopConsoleModel::Mgb,
                Path::new("/unused"),
            )
            .expect("existing exact files should not trigger fallback"),
            None
        );

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn missing_boot_rom_asset_path_detects_missing_active_model_images_in_directories() {
        let root = temp_root("missing-directory-image");
        let directory = root.join("bootrom");
        fs::create_dir_all(&directory).expect("boot ROM directory should be creatable");
        write_boot_rom_image(
            &directory.join(BootRomAssets::filename(BootRomKind::Dmg)),
            0x44,
        );

        assert_eq!(
            missing_boot_rom_asset_path(
                Some(&directory),
                DesktopConsoleModel::Dmg,
                Path::new("/unused"),
            )
            .expect("present active-model image should not trigger fallback"),
            None
        );
        assert_eq!(
            missing_boot_rom_asset_path(
                Some(&directory),
                DesktopConsoleModel::Mgb,
                Path::new("/unused"),
            )
            .expect("missing active-model image should surface the expected path"),
            Some(directory.join(BootRomAssets::filename(BootRomKind::Mgb)))
        );

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn missing_boot_rom_asset_path_returns_the_missing_directory_when_the_source_root_is_gone() {
        let missing_directory = PathBuf::from("/definitely/missing/desktop-bootrom-root");

        assert_eq!(
            missing_boot_rom_asset_path(
                Some(&missing_directory),
                DesktopConsoleModel::Dmg0,
                Path::new("/unused"),
            )
            .expect("missing directory roots should resolve cleanly"),
            Some(missing_directory)
        );
    }

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
}
