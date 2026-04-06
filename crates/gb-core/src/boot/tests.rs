use super::*;
use crate::cartridge::CartridgeHeader;
use std::env;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn empty_assets() -> BootRomAssets {
    BootRomAssets::none()
}

fn unique_temp_dir() -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cycle-boot-assets-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

#[test]
fn startup_mode_controls_initial_boot_mapping_state() {
    let real_boot = BootController::new(ConsoleModel::Dmg, StartupMode::RealBoot, empty_assets());
    let skip_boot = BootController::new(ConsoleModel::Dmg, StartupMode::SkipBoot, empty_assets());

    assert!(real_boot.is_boot_rom_mapped());
    assert!(!skip_boot.is_boot_rom_mapped());
}

#[test]
fn ff50_only_unmaps_on_non_zero_writes() {
    let mut boot = BootController::new(ConsoleModel::Dmg, StartupMode::RealBoot, empty_assets());

    boot.write_ff50(0x00);
    assert!(boot.is_boot_rom_mapped());

    boot.write_ff50(0x01);
    assert!(!boot.is_boot_rom_mapped());

    boot.write_ff50(0x00);
    assert!(!boot.is_boot_rom_mapped());
}

#[test]
fn console_model_selects_the_expected_dmg_family_boot_kind() {
    assert_eq!(
        BootController::new(ConsoleModel::Dmg0, StartupMode::RealBoot, empty_assets())
            .boot_rom_kind(),
        BootRomKind::Dmg0
    );
    assert_eq!(
        BootController::new(ConsoleModel::Dmg, StartupMode::RealBoot, empty_assets())
            .boot_rom_kind(),
        BootRomKind::Dmg
    );
    assert_eq!(
        BootController::new(ConsoleModel::Mgb, StartupMode::RealBoot, empty_assets())
            .boot_rom_kind(),
        BootRomKind::Mgb
    );
}

#[test]
fn boot_rom_reads_come_from_the_configured_selected_model_image() {
    let assets = BootRomAssets::none()
        .with_bytes(BootRomKind::Dmg, vec![0xC0; BOOT_ROM_LEN])
        .expect("dmg image should validate")
        .with_bytes(BootRomKind::Mgb, vec![0xB0; BOOT_ROM_LEN])
        .expect("mgb image should validate");
    let dmg = BootController::new(ConsoleModel::Dmg, StartupMode::RealBoot, assets.clone());
    let mgb = BootController::new(ConsoleModel::Mgb, StartupMode::RealBoot, assets);

    assert_eq!(dmg.read_boot_rom(0x0000), 0xC0);
    assert_eq!(mgb.read_boot_rom(0x0000), 0xB0);
    assert_ne!(dmg.read_boot_rom(0x0000), mgb.read_boot_rom(0x0000));
}

#[test]
fn skip_boot_state_uses_model_specific_cpu_and_io_presets() {
    let boot = BootController::new(ConsoleModel::Dmg, StartupMode::SkipBoot, empty_assets());
    let direct_boot = boot
        .direct_boot_state(None)
        .expect("SkipBoot should expose a direct-boot state");

    assert_eq!(direct_boot.cpu.pc, 0x0100);
    assert_eq!(direct_boot.cpu.a, 0x01);
    assert_eq!(direct_boot.cpu.f, 0xB0);
    assert_eq!(direct_boot.io.p1, 0xCF);
    assert_eq!(direct_boot.io.div, 0xAB);
    assert_eq!(direct_boot.serial.sb, 0x00);
    assert_eq!(
        direct_boot.serial.clock_mode,
        crate::serial::SerialClockMode::External
    );
    assert_eq!(direct_boot.serial.clock_counter, 0xABCC);
    assert_eq!(direct_boot.ppu.lcdc, 0x91);
    assert_eq!(direct_boot.ppu.stat, 0x85);
    assert_eq!(direct_boot.io.dma, 0xFF);
    assert_eq!(direct_boot.io.interrupt_flag, 0xE1);
    assert_eq!(
        direct_boot.startup_memory_policy,
        StartupMemoryPolicy::DeterministicZeroed
    );
}

#[test]
fn dmg_family_skip_boot_flags_follow_the_header_checksum_rule() {
    let boot = BootController::new(ConsoleModel::Dmg, StartupMode::SkipBoot, empty_assets());
    let mut rom = vec![0x00; 0x150];
    rom[0x014D] = 0x00;
    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(
        boot.direct_boot_state(None).unwrap().cpu.f,
        0xB0,
        "empty-slot fallback keeps the common DMG preset"
    );
    assert_eq!(
        build_skip_boot_cpu_state(ConsoleModel::Dmg, Some(header.header_checksum)).f,
        0x80
    );
    assert_eq!(
        build_skip_boot_cpu_state(ConsoleModel::Dmg, Some(0x7F)).f,
        0xB0
    );
}

#[test]
fn missing_boot_rom_assets_fall_back_to_ff_reads_without_fake_placeholder_data() {
    let boot = BootController::new(ConsoleModel::Dmg, StartupMode::RealBoot, empty_assets());

    assert!(!boot.has_boot_rom_asset());
    assert_eq!(boot.read_boot_rom(0x0000), 0xFF);
    assert_eq!(boot.read_boot_rom(0x00FF), 0xFF);
}

#[test]
fn boot_rom_assets_can_load_a_configured_directory_source() {
    let directory = unique_temp_dir();
    fs::create_dir_all(&directory).expect("temporary asset directory should be creatable");
    fs::write(
        directory.join(BootRomAssets::filename(BootRomKind::Dmg)),
        vec![0x42; BOOT_ROM_LEN],
    )
    .expect("boot ROM file should be writable");

    let assets = BootRomAssets::from_directory(&directory)
        .expect("directory-backed boot ROM assets should load");

    assert!(assets.has_image(BootRomKind::Dmg));
    assert!(!assets.has_image(BootRomKind::Mgb));
    assert_eq!(assets.read_byte(BootRomKind::Dmg, 0x0000), Some(0x42));

    fs::remove_dir_all(&directory).expect("temporary asset directory should be removable");
}

#[test]
fn boot_rom_asset_errors_cover_missing_non_directory_and_read_failure_paths() {
    let missing_directory = unique_temp_dir();
    let missing_error = BootRomAssets::from_directory(&missing_directory)
        .expect_err("missing directories should be rejected");
    assert!(matches!(
        missing_error,
        BootRomAssetError::DirectoryNotFound { .. }
    ));
    assert!(missing_error.to_string().contains("does not exist"));
    assert!(std::error::Error::source(&missing_error).is_none());

    let file_path = unique_temp_dir();
    fs::write(&file_path, b"not a directory").expect("temporary file should be writable");
    let file_error = BootRomAssets::from_directory(&file_path)
        .expect_err("files should not be treated as directories");
    assert!(matches!(
        file_error,
        BootRomAssetError::NotADirectory { .. }
    ));
    assert!(file_error.to_string().contains("not a directory"));
    assert!(std::error::Error::source(&file_error).is_none());
    fs::remove_file(&file_path).expect("temporary file should be removable");

    let directory = unique_temp_dir();
    fs::create_dir_all(directory.join(BootRomAssets::filename(BootRomKind::Dmg)))
        .expect("boot ROM directory placeholder should be creatable");
    let read_error = BootRomAssets::from_directory(&directory)
        .expect_err("directory entries that are not files should surface read errors");
    assert!(matches!(read_error, BootRomAssetError::ReadFailed { .. }));
    assert!(
        read_error
            .to_string()
            .contains("failed to read boot ROM asset")
    );
    assert!(std::error::Error::source(&read_error).is_some());
    fs::remove_dir_all(&directory).expect("temporary asset directory should be removable");
}

#[test]
fn boot_rom_assets_cover_all_kind_slots_and_exact_filenames() {
    let assets = BootRomAssets::none()
        .with_bytes(BootRomKind::Dmg0, vec![0x10; BOOT_ROM_LEN])
        .expect("dmg0 image should validate")
        .with_bytes(BootRomKind::Dmg, vec![0x20; BOOT_ROM_LEN])
        .expect("dmg image should validate")
        .with_bytes(BootRomKind::Mgb, vec![0x30; BOOT_ROM_LEN])
        .expect("mgb image should validate");

    assert_eq!(BootRomAssets::filename(BootRomKind::Dmg0), "dmg0_boot.bin");
    assert_eq!(BootRomAssets::filename(BootRomKind::Dmg), "dmg_boot.bin");
    assert_eq!(BootRomAssets::filename(BootRomKind::Mgb), "mgb_boot.bin");
    assert!(!assets.is_empty());
    assert!(assets.has_image(BootRomKind::Dmg0));
    assert!(assets.has_image(BootRomKind::Dmg));
    assert!(assets.has_image(BootRomKind::Mgb));
    assert_eq!(assets.read_byte(BootRomKind::Dmg0, 0x0000), Some(0x10));
    assert_eq!(assets.read_byte(BootRomKind::Dmg, 0x0000), Some(0x20));
    assert_eq!(assets.read_byte(BootRomKind::Mgb, 0x0000), Some(0x30));
}
