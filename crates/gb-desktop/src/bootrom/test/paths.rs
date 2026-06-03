use super::*;

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
