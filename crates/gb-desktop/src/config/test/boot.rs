use super::*;

#[test]
fn boot_rom_options_cover_default_custom_and_skip_boot_loading() {
    let mut options = BootRomOptions::default();
    assert_eq!(options.resolved_search_path(), None);

    options.search_path = Some(PathBuf::from("/tmp/firmware"));
    assert_eq!(
        options.resolved_search_path(),
        Some(PathBuf::from("/tmp/firmware"))
    );

    assert!(
        options
            .load_assets(StartupMode::SkipBoot, HardwareRevision::DmgCpuC)
            .expect("skip-boot should not attempt to read firmware")
            .is_empty()
    );
}

#[test]
fn boot_rom_options_can_load_a_single_exact_image_file() {
    let root = temp_root("boot-options");
    let image_path = root.join("dmg_boot.bin");
    fs::write(&image_path, vec![0x11; 0x100]).expect("test boot ROM image should be writable");

    let options = BootRomOptions {
        search_path: Some(image_path.clone()),
        verification: BootRomVerificationMode::Off,
    };
    let assets = options
        .load_assets(StartupMode::RealBoot, HardwareRevision::DmgCpuC)
        .expect("exact boot ROM file should load");
    assert_eq!(assets.read_byte(HardwareRevision::DmgCpuC, 0), Some(0x11));

    let sgb_path = root.join("sgb_boot.bin");
    fs::write(&sgb_path, vec![0x51; 0x100]).expect("test SGB boot ROM image should be writable");
    let sgb_options = BootRomOptions {
        search_path: Some(sgb_path),
        verification: BootRomVerificationMode::Off,
    };
    let sgb_assets = sgb_options
        .load_assets(StartupMode::RealBoot, BootRomAssetKind::Sgb)
        .expect("exact SGB boot ROM file should load");
    assert_eq!(
        sgb_assets.read_asset_byte(BootRomAssetKind::Sgb, 0),
        Some(0x51)
    );

    fs::remove_dir_all(root).expect("temp boot ROM root should be removable");
}

#[test]
fn derive_save_key_rejects_paths_without_file_names() {
    let error =
        derive_save_key(Path::new("/")).expect_err("paths without a file name should be rejected");
    assert!(matches!(
        error,
        DesktopConfigError::SaveKeyDerivationEmpty { .. }
    ));
}
