use gb_core::{BootRomAssetKind, BootRomAssets, StartupMode};
use gb_desktop::BootRomVerificationMode;
use sha2::{Digest, Sha256};
use std::fmt;
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
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
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
    fn resolve_boot_rom_source_uses_only_explicit_paths() {
        let current_dir = Path::new("/tmp/gb-cycle");

        assert_eq!(
            resolve_boot_rom_source(Some(Path::new("firmware")), current_dir),
            Some(PathBuf::from("/tmp/gb-cycle/firmware"))
        );
        assert_eq!(resolve_boot_rom_source(None, current_dir), None);
    }

    #[test]
    fn boot_rom_image_path_uses_the_exact_file_or_revision_filename() {
        let root = temp_root("image-path");
        let exact_file = root.join("mgb_boot.bin");
        let directory = root.join("bootrom");
        write_boot_rom_image(&exact_file, 0x77);
        fs::create_dir_all(&directory).expect("bootrom test directory should be creatable");

        assert_eq!(
            boot_rom_image_path(&exact_file, HardwareRevision::CpuMgb),
            exact_file
        );
        assert_eq!(
            boot_rom_image_path(&directory, HardwareRevision::DmgCpuC),
            directory.join(BootRomAssets::filename(HardwareRevision::DmgCpuC))
        );
        assert_eq!(
            boot_rom_image_path(&directory, BootRomAssetKind::Sgb),
            directory.join("sgb_boot.bin")
        );
        assert_eq!(
            boot_rom_image_path(&directory, BootRomAssetKind::Sgb2),
            directory.join("sgb2_boot.bin")
        );

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn sha_and_expected_sha_helpers_cover_all_supported_boot_rom_revisions() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            HardwareRevision::boot_rom_expected_sha256(HardwareRevision::DmgCpu).len(),
            64
        );
        assert_eq!(
            HardwareRevision::boot_rom_expected_sha256(HardwareRevision::DmgCpuC).len(),
            64
        );
        assert_eq!(
            HardwareRevision::boot_rom_expected_sha256(HardwareRevision::CpuMgb).len(),
            64
        );
        assert_eq!(
            HardwareRevision::boot_rom_expected_sha256(HardwareRevision::CpuCgb).len(),
            64
        );
        assert_eq!(
            HardwareRevision::boot_rom_expected_sha256(HardwareRevision::CpuCgbC).len(),
            64
        );
        assert_eq!(
            HardwareRevision::boot_rom_expected_sha256(HardwareRevision::CpuCgbE).len(),
            64
        );
        assert_eq!(
            HardwareRevision::boot_rom_expected_size(HardwareRevision::DmgCpuC),
            256
        );
        assert_eq!(
            HardwareRevision::boot_rom_expected_size(HardwareRevision::CpuMgb),
            256
        );
        assert_eq!(
            HardwareRevision::boot_rom_expected_size(HardwareRevision::CpuCgbC),
            2304
        );
        assert_eq!(BootRomAssetKind::Sgb.expected_sha256().len(), 64);
        assert_eq!(BootRomAssetKind::Sgb2.expected_sha256().len(), 64);
        assert_eq!(BootRomAssetKind::Sgb.expected_size(), 256);
        assert_eq!(BootRomAssetKind::Sgb2.expected_size(), 256);
    }

    #[test]
    fn verify_boot_rom_file_reports_hash_mismatches_and_missing_files() {
        let root = temp_root("verify");
        let image_path = root.join("dmg_boot.bin");
        write_boot_rom_image(&image_path, 0xAA);

        let mismatch = verify_boot_rom_file(&image_path, HardwareRevision::DmgCpuC)
            .expect_err("synthetic image should not match the pinned SHA");
        assert!(mismatch.contains("unexpected sha256"));
        assert!(mismatch.contains("expected"));

        let missing = verify_boot_rom_file(&root.join("missing.bin"), HardwareRevision::DmgCpuC)
            .expect_err("missing file should surface a read error");
        assert!(missing.contains("failed to read boot ROM asset"));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn verify_boot_rom_file_reports_sgb_hash_mismatches() {
        let root = temp_root("verify-sgb");
        let image_path = root.join("sgb_boot.bin");
        write_boot_rom_image(&image_path, 0x51);

        let mismatch = verify_boot_rom_file(&image_path, BootRomAssetKind::Sgb)
            .expect_err("synthetic SGB image should not match the pinned SHA");
        assert!(mismatch.contains("Sgb"));
        assert!(mismatch.contains("unexpected sha256"));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn verify_boot_rom_file_reports_canonical_size_mismatches_before_hashing() {
        let root = temp_root("verify-size");
        let image_path = root.join("cgb_boot.bin");
        fs::write(&image_path, vec![0xAA; 0x0800])
            .expect("compact CGB boot ROM image should be writable");

        let mismatch = verify_boot_rom_file(&image_path, HardwareRevision::CpuCgbC)
            .expect_err("strict desktop verification should reject compact CGB images");
        assert!(mismatch.contains("unexpected size"));
        assert!(mismatch.contains("expected 2304 bytes"));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_exact_boot_rom_file_loads_bytes_for_the_requested_revision() {
        let root = temp_root("exact-file");
        let image_path = root.join("mgb_boot.bin");
        write_boot_rom_image(&image_path, 0x5A);

        let assets = load_exact_boot_rom_file(&image_path, HardwareRevision::CpuMgb)
            .expect("synthetic boot ROM file should load");
        assert!(assets.has_image(HardwareRevision::CpuMgb));
        assert_eq!(assets.read_byte(HardwareRevision::CpuMgb, 0), Some(0x5A));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_exact_boot_rom_file_loads_bytes_for_sgb_assets() {
        let root = temp_root("exact-sgb-file");
        let sgb_path = root.join("sgb_boot.bin");
        let sgb2_path = root.join("sgb2_boot.bin");
        write_boot_rom_image(&sgb_path, 0x51);
        write_boot_rom_image(&sgb2_path, 0x52);

        let sgb_assets = load_exact_boot_rom_file(&sgb_path, BootRomAssetKind::Sgb)
            .expect("synthetic SGB boot ROM file should load");
        let sgb2_assets = load_exact_boot_rom_file(&sgb2_path, BootRomAssetKind::Sgb2)
            .expect("synthetic SGB2 boot ROM file should load");

        assert!(sgb_assets.has_asset(BootRomAssetKind::Sgb));
        assert_eq!(
            sgb_assets.read_asset_byte(BootRomAssetKind::Sgb, 0),
            Some(0x51)
        );
        assert!(sgb2_assets.has_asset(BootRomAssetKind::Sgb2));
        assert_eq!(
            sgb2_assets.read_asset_byte(BootRomAssetKind::Sgb2, 0),
            Some(0x52)
        );

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_exact_boot_rom_file_reports_read_failures_and_invalid_lengths() {
        let root = temp_root("exact-errors");
        let missing =
            load_exact_boot_rom_file(&root.join("missing.bin"), HardwareRevision::DmgCpuC)
                .expect_err("missing exact boot ROM files should fail");
        assert!(missing.contains("failed to read boot ROM asset"));

        let short_image = root.join("short.bin");
        fs::write(&short_image, vec![0x11; 0x40]).expect("short boot ROM image should be writable");
        let invalid_len = load_exact_boot_rom_file(&short_image, HardwareRevision::DmgCpuC)
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
            &directory.join(BootRomAssets::filename(HardwareRevision::DmgCpuC)),
            0x42,
        );

        let assets = load_boot_rom_assets(
            Some(&directory),
            BootRomVerificationMode::Off,
            HardwareRevision::DmgCpuC,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("directory-backed boot ROM assets should load");
        assert_eq!(assets.read_byte(HardwareRevision::DmgCpuC, 0), Some(0x42));
        assert!(!assets.has_image(HardwareRevision::CpuMgb));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_boot_rom_assets_selects_sgb_directory_images_for_real_boot() {
        let root = temp_root("directory-sgb-assets");
        let directory = root.join("bootrom");
        fs::create_dir_all(&directory).expect("boot ROM directory should be creatable");
        write_boot_rom_image(&directory.join("dmg_boot.bin"), 0xD0);
        write_boot_rom_image(&directory.join("sgb_boot.bin"), 0x51);
        write_boot_rom_image(&directory.join("sgb2_boot.bin"), 0x52);

        let sgb_assets = load_boot_rom_assets(
            Some(&directory),
            BootRomVerificationMode::Off,
            BootRomAssetKind::Sgb,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("directory-backed SGB boot ROM assets should load");
        let sgb2_assets = load_boot_rom_assets(
            Some(&directory),
            BootRomVerificationMode::Off,
            BootRomAssetKind::Sgb2,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("directory-backed SGB2 boot ROM assets should load");

        assert_eq!(
            sgb_assets.read_asset_byte(BootRomAssetKind::Sgb, 0),
            Some(0x51)
        );
        assert_eq!(
            sgb2_assets.read_asset_byte(BootRomAssetKind::Sgb2, 0),
            Some(0x52)
        );
        assert_eq!(
            sgb_assets.read_byte(HardwareRevision::DmgCpuC, 0),
            Some(0xD0)
        );

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn load_boot_rom_assets_reports_directory_loading_failures() {
        let root = temp_root("directory-error");
        let directory = root.join("bootrom");
        fs::create_dir_all(&directory).expect("boot ROM directory should be creatable");
        fs::write(
            directory.join(BootRomAssets::filename(HardwareRevision::DmgCpuC)),
            vec![0x42; 0x40],
        )
        .expect("invalid boot ROM image should be writable");

        let error = load_boot_rom_assets(
            Some(&directory),
            BootRomVerificationMode::Off,
            HardwareRevision::DmgCpuC,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect_err("invalid directory-backed assets should fail");
        assert!(error.contains("failed to load boot ROM assets from"));

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[cfg(unix)]
    #[test]
    fn path_exists_reports_invalid_paths() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let invalid_path = PathBuf::from(OsString::from_vec(vec![b'i', 0, b'n', b'v']));
        let error = path_exists(&invalid_path).expect_err("invalid paths should report errors");
        assert!(error.contains("failed to inspect boot ROM path"));
    }

    #[test]
    fn load_boot_rom_assets_respects_startup_mode_and_verification_policy() {
        let root = temp_root("load-assets");
        let image_path = root.join("dmg_boot.bin");
        write_boot_rom_image(&image_path, 0xC3);

        let skip_boot = load_boot_rom_assets(
            Some(&image_path),
            BootRomVerificationMode::Strict,
            HardwareRevision::DmgCpuC,
            StartupMode::SkipBoot,
            Path::new("/unused"),
        )
        .expect("skip-boot should bypass firmware loading");
        assert!(skip_boot.is_empty());

        let off = load_boot_rom_assets(
            Some(&image_path),
            BootRomVerificationMode::Off,
            HardwareRevision::DmgCpuC,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("verification-off should load exact files");
        assert_eq!(off.read_byte(HardwareRevision::DmgCpuC, 0), Some(0xC3));

        let warn = load_boot_rom_assets(
            Some(&image_path),
            BootRomVerificationMode::Warn,
            HardwareRevision::DmgCpuC,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("warning mode should allow hash mismatches");
        assert_eq!(warn.read_byte(HardwareRevision::DmgCpuC, 0), Some(0xC3));

        let strict = load_boot_rom_assets(
            Some(&image_path),
            BootRomVerificationMode::Strict,
            HardwareRevision::DmgCpuC,
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
            HardwareRevision::DmgCpuC,
            StartupMode::RealBoot,
            Path::new("/unused"),
        )
        .expect("missing firmware directory should degrade to no assets");

        assert!(assets.is_empty());
    }

    #[test]
    fn missing_boot_rom_asset_detects_missing_exact_files() {
        let root = temp_root("missing-exact");
        let exact_file = root.join("mgb_boot.bin");

        assert_eq!(
            missing_boot_rom_asset(
                Some(&exact_file),
                HardwareRevision::CpuMgb,
                Path::new("/unused"),
            )
            .expect("missing exact files should resolve cleanly"),
            Some(MissingBootRomAsset::Path(exact_file.clone()))
        );

        write_boot_rom_image(&exact_file, 0x33);
        assert_eq!(
            missing_boot_rom_asset(
                Some(&exact_file),
                HardwareRevision::CpuMgb,
                Path::new("/unused"),
            )
            .expect("existing exact files should not trigger fallback"),
            None
        );

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn missing_boot_rom_asset_detects_missing_active_model_images_in_directories() {
        let root = temp_root("missing-directory-image");
        let directory = root.join("bootrom");
        fs::create_dir_all(&directory).expect("boot ROM directory should be creatable");
        write_boot_rom_image(
            &directory.join(BootRomAssets::filename(HardwareRevision::DmgCpuC)),
            0x44,
        );

        assert_eq!(
            missing_boot_rom_asset(
                Some(&directory),
                HardwareRevision::DmgCpuC,
                Path::new("/unused"),
            )
            .expect("present active-model image should not trigger fallback"),
            None
        );
        assert_eq!(
            missing_boot_rom_asset(
                Some(&directory),
                HardwareRevision::CpuMgb,
                Path::new("/unused"),
            )
            .expect("missing active-model image should surface the expected path"),
            Some(MissingBootRomAsset::Path(
                directory.join(BootRomAssets::filename(HardwareRevision::CpuMgb))
            ))
        );
        assert_eq!(
            missing_boot_rom_asset(
                Some(&directory),
                BootRomAssetKind::Sgb,
                Path::new("/unused"),
            )
            .expect("missing SGB image should surface the expected path"),
            Some(MissingBootRomAsset::Path(directory.join("sgb_boot.bin")))
        );

        fs::remove_dir_all(root).expect("temp bootrom root should be removable");
    }

    #[test]
    fn missing_boot_rom_asset_returns_the_missing_directory_when_the_source_root_is_gone() {
        let missing_directory = PathBuf::from("/definitely/missing/desktop-bootrom-root");

        assert_eq!(
            missing_boot_rom_asset(
                Some(&missing_directory),
                HardwareRevision::DmgCpuC,
                Path::new("/unused"),
            )
            .expect("missing directory roots should resolve cleanly"),
            Some(MissingBootRomAsset::Path(missing_directory))
        );
    }

    #[test]
    fn missing_boot_rom_asset_reports_unconfigured_sources() {
        assert_eq!(
            missing_boot_rom_asset(None, HardwareRevision::DmgCpuC, Path::new("/unused"))
                .expect("unconfigured sources should resolve cleanly"),
            Some(MissingBootRomAsset::SourceUnconfigured)
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
