use super::*;

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
