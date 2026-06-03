use super::*;

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
