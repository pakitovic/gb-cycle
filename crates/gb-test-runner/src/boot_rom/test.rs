use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use gb_core::{BootRomAssetKind, ConsoleModel, HardwareRevision, HostPlatform};

use super::assets::required_assets_for_test;
use super::{BootRomLoadError, BootRomProfile, asset_for_profile, load_verified_boot_rom_assets};

fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gb-cycle-boot-rom-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn write_asset(root: &Path, asset: BootRomAssetKind, size: usize, byte: u8) {
    fs::create_dir_all(root).expect("boot ROM root should be creatable");
    fs::write(root.join(asset.filename()), vec![byte; size]).expect("boot ROM asset should write");
}

#[test]
fn resolves_required_assets_for_console_and_host_profiles() {
    assert_eq!(
        asset_for_profile(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpu0,
            HostPlatform::Handheld
        ),
        BootRomAssetKind::Dmg0
    );
    assert_eq!(
        asset_for_profile(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            HostPlatform::Handheld
        ),
        BootRomAssetKind::Dmg
    );
    assert_eq!(
        asset_for_profile(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbE,
            HostPlatform::Handheld
        ),
        BootRomAssetKind::CgbE
    );
    assert_eq!(
        asset_for_profile(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbD,
            HostPlatform::Handheld
        ),
        BootRomAssetKind::Cgb
    );
    assert_eq!(
        asset_for_profile(
            ConsoleModel::GameBoyAdvance,
            HardwareRevision::CpuAgbA,
            HostPlatform::Handheld
        ),
        BootRomAssetKind::CgbAgb
    );
    assert_eq!(
        asset_for_profile(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            HostPlatform::Sgb
        ),
        BootRomAssetKind::Sgb
    );
    assert_eq!(
        asset_for_profile(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            HostPlatform::Sgb2
        ),
        BootRomAssetKind::Sgb2
    );
}

#[test]
fn deduplicates_required_assets_and_does_not_require_unselected_profiles() {
    let profiles = [
        BootRomProfile::new(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpu0,
            HostPlatform::Handheld,
        ),
        BootRomProfile::new(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            HostPlatform::Handheld,
        ),
        BootRomProfile::new(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            HostPlatform::Handheld,
        ),
        BootRomProfile::new(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbD,
            HostPlatform::Handheld,
        ),
        BootRomProfile::new(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbE,
            HostPlatform::Handheld,
        ),
        BootRomProfile::new(
            ConsoleModel::GameBoyAdvance,
            HardwareRevision::CpuAgbA,
            HostPlatform::Handheld,
        ),
    ];

    assert_eq!(
        required_assets_for_test(&profiles),
        vec![
            BootRomAssetKind::Dmg0,
            BootRomAssetKind::Dmg,
            BootRomAssetKind::Cgb,
            BootRomAssetKind::CgbE,
            BootRomAssetKind::CgbAgb
        ]
    );
}

#[test]
fn load_verified_boot_rom_assets_accepts_empty_profile_set() {
    let root = unique_temp_dir("empty");
    fs::create_dir_all(&root).expect("boot ROM root should be creatable");

    let assets = load_verified_boot_rom_assets(&root, &[]).expect("empty profile set should load");
    assert!(assets.is_empty());

    fs::remove_dir_all(root).expect("boot ROM root should be removable");
}

#[test]
fn load_verified_boot_rom_assets_rejects_missing_directory_and_non_directory() {
    let missing = unique_temp_dir("missing-directory");
    assert!(matches!(
        load_verified_boot_rom_assets(
            &missing,
            &[BootRomProfile::new(
                ConsoleModel::GameBoy,
                HardwareRevision::DmgCpuC,
                HostPlatform::Handheld
            )],
        )
        .expect_err("missing directory should fail"),
        BootRomLoadError::DirectoryNotFound { .. }
    ));

    let file_path = unique_temp_dir("not-directory");
    fs::write(&file_path, b"not a directory").expect("file should be writable");
    assert!(matches!(
        load_verified_boot_rom_assets(
            &file_path,
            &[BootRomProfile::new(
                ConsoleModel::GameBoy,
                HardwareRevision::DmgCpuC,
                HostPlatform::Handheld
            )],
        )
        .expect_err("file root should fail"),
        BootRomLoadError::NotADirectory { .. }
    ));

    fs::remove_file(file_path).expect("file should be removable");
}

#[test]
fn load_verified_boot_rom_assets_rejects_missing_required_file() {
    let root = unique_temp_dir("missing-file");
    fs::create_dir_all(&root).expect("boot ROM root should be creatable");

    let error = load_verified_boot_rom_assets(
        &root,
        &[BootRomProfile::new(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            HostPlatform::Sgb2,
        )],
    )
    .expect_err("missing SGB2 asset should fail");
    assert!(matches!(
        error,
        BootRomLoadError::MissingFile {
            asset: BootRomAssetKind::Sgb2,
            ..
        }
    ));
    assert!(error.to_string().contains("sgb2_boot.bin"));

    fs::remove_dir_all(root).expect("boot ROM root should be removable");
}

#[test]
fn load_verified_boot_rom_assets_rejects_invalid_size() {
    let root = unique_temp_dir("invalid-size");
    write_asset(&root, BootRomAssetKind::Dmg, 1, 0);

    assert!(matches!(
        load_verified_boot_rom_assets(
            &root,
            &[BootRomProfile::new(
                ConsoleModel::GameBoy,
                HardwareRevision::DmgCpuC,
                HostPlatform::Handheld
            )],
        )
        .expect_err("invalid size should fail"),
        BootRomLoadError::SizeMismatch {
            asset: BootRomAssetKind::Dmg,
            expected: 0x0100,
            actual: 1,
            ..
        }
    ));

    fs::remove_dir_all(root).expect("boot ROM root should be removable");
}

#[test]
fn load_verified_boot_rom_assets_rejects_invalid_hash_after_size_matches() {
    let root = unique_temp_dir("invalid-hash");
    write_asset(
        &root,
        BootRomAssetKind::Dmg,
        BootRomAssetKind::Dmg.expected_size(),
        0,
    );

    let error = load_verified_boot_rom_assets(
        &root,
        &[BootRomProfile::new(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            HostPlatform::Handheld,
        )],
    )
    .expect_err("invalid hash should fail");
    assert!(matches!(
        error,
        BootRomLoadError::HashMismatch {
            asset: BootRomAssetKind::Dmg,
            ..
        }
    ));
    assert!(
        error
            .to_string()
            .contains(BootRomAssetKind::Dmg.expected_sha256())
    );

    fs::remove_dir_all(root).expect("boot ROM root should be removable");
}

#[test]
fn load_verified_boot_rom_assets_verifies_only_selected_assets() {
    let root = unique_temp_dir("selected-only");
    write_asset(
        &root,
        BootRomAssetKind::Dmg,
        BootRomAssetKind::Dmg.expected_size(),
        0,
    );

    let error = load_verified_boot_rom_assets(
        &root,
        &[BootRomProfile::new(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            HostPlatform::Handheld,
        )],
    )
    .expect_err("selected invalid hash should fail");
    assert!(matches!(
        error,
        BootRomLoadError::HashMismatch {
            asset: BootRomAssetKind::Dmg,
            ..
        }
    ));
    assert!(!error.to_string().contains("cgbE_boot.bin"));

    fs::remove_dir_all(root).expect("boot ROM root should be removable");
}
