use super::*;

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
