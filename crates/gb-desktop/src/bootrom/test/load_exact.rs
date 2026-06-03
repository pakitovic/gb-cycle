use super::*;

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
    let missing = load_exact_boot_rom_file(&root.join("missing.bin"), HardwareRevision::DmgCpuC)
        .expect_err("missing exact boot ROM files should fail");
    assert!(missing.contains("failed to read boot ROM asset"));

    let short_image = root.join("short.bin");
    fs::write(&short_image, vec![0x11; 0x40]).expect("short boot ROM image should be writable");
    let invalid_len = load_exact_boot_rom_file(&short_image, HardwareRevision::DmgCpuC)
        .expect_err("invalid boot ROM image lengths should fail");
    assert!(invalid_len.contains("failed to load boot ROM asset"));

    fs::remove_dir_all(root).expect("temp bootrom root should be removable");
}
