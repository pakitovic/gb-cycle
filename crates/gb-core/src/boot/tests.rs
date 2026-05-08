use super::*;
use crate::cartridge::CartridgeHeader;
use std::env;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn empty_assets() -> BootRomAssets {
    BootRomAssets::none()
}

fn boot(
    console_model: ConsoleModel,
    startup_mode: StartupMode,
    assets: BootRomAssets,
) -> BootController {
    BootController::new(
        console_model,
        startup_mode,
        console_model.default_boot_rom_kind(),
        assets,
    )
}

fn unique_temp_dir() -> PathBuf {
    static UNIQUE_TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    env::temp_dir().join(format!(
        "gb-cycle-boot-assets-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos(),
        UNIQUE_TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn startup_mode_controls_initial_boot_mapping_state() {
    let real_boot = boot(ConsoleModel::GameBoy, StartupMode::RealBoot, empty_assets());
    let skip_boot = boot(ConsoleModel::GameBoy, StartupMode::SkipBoot, empty_assets());

    assert!(real_boot.is_boot_rom_mapped());
    assert!(!skip_boot.is_boot_rom_mapped());
}

#[test]
fn ff50_only_unmaps_on_non_zero_writes() {
    let mut boot = boot(ConsoleModel::GameBoy, StartupMode::RealBoot, empty_assets());

    assert!(!boot.write_ff50(0x00));
    assert!(boot.is_boot_rom_mapped());

    assert!(boot.write_ff50(0x01));
    assert!(!boot.is_boot_rom_mapped());

    assert!(!boot.write_ff50(0x00));
    assert!(!boot.is_boot_rom_mapped());
}

#[test]
fn bus_state_publishes_model_specific_boot_overlay_windows() {
    let dmg = boot(ConsoleModel::GameBoy, StartupMode::RealBoot, empty_assets());
    let cgb = boot(
        ConsoleModel::GameBoyColor,
        StartupMode::RealBoot,
        empty_assets(),
    );
    let skip_boot = boot(
        ConsoleModel::GameBoyColor,
        StartupMode::SkipBoot,
        empty_assets(),
    );

    let dmg_bus_state = dmg.bus_state();
    let cgb_bus_state = cgb.bus_state();

    assert!(dmg_bus_state.maps_dmg_low_bytes());
    assert!(!dmg_bus_state.maps_cgb_upper_window());
    assert!(cgb_bus_state.maps_low_window());
    assert!(cgb_bus_state.maps_cgb_upper_window());
    assert!(!skip_boot.bus_state().maps_low_window());
    assert!(!skip_boot.bus_state().maps_cgb_upper_window());
}

#[test]
fn console_model_defaults_to_the_expected_boot_kind() {
    assert_eq!(
        boot(ConsoleModel::GameBoy, StartupMode::RealBoot, empty_assets()).boot_rom_kind(),
        BootRomKind::Dmg
    );
    assert_eq!(
        boot(
            ConsoleModel::GameBoyPocket,
            StartupMode::RealBoot,
            empty_assets()
        )
        .boot_rom_kind(),
        BootRomKind::Mgb
    );
    assert_eq!(
        boot(
            ConsoleModel::GameBoyLight,
            StartupMode::RealBoot,
            empty_assets()
        )
        .boot_rom_kind(),
        BootRomKind::Mgb
    );
    assert_eq!(
        boot(
            ConsoleModel::GameBoyColor,
            StartupMode::RealBoot,
            empty_assets()
        )
        .boot_rom_kind(),
        BootRomKind::Cgb
    );
}

#[test]
fn boot_controller_uses_the_explicit_boot_kind() {
    assert_eq!(
        BootController::new(
            ConsoleModel::GameBoy,
            StartupMode::RealBoot,
            BootRomKind::Dmg0,
            empty_assets(),
        )
        .boot_rom_kind(),
        BootRomKind::Dmg0
    );
}

#[test]
fn boot_rom_reads_come_from_the_configured_selected_model_image() {
    let assets = BootRomAssets::none()
        .with_bytes(BootRomKind::Dmg, vec![0xC0; DMG_FAMILY_BOOT_ROM_LEN])
        .expect("dmg image should validate")
        .with_bytes(BootRomKind::Mgb, vec![0xB0; DMG_FAMILY_BOOT_ROM_LEN])
        .expect("mgb image should validate");
    let dmg = BootController::new(
        ConsoleModel::GameBoy,
        StartupMode::RealBoot,
        BootRomKind::Dmg,
        assets.clone(),
    );
    let mgb = BootController::new(
        ConsoleModel::GameBoyPocket,
        StartupMode::RealBoot,
        BootRomKind::Mgb,
        assets,
    );

    assert_eq!(dmg.read_boot_rom(0x0000), 0xC0);
    assert_eq!(mgb.read_boot_rom(0x0000), 0xB0);
    assert_ne!(dmg.read_boot_rom(0x0000), mgb.read_boot_rom(0x0000));
}

#[test]
fn cgb_boot_rom_reads_cover_both_overlay_windows_without_aliasing_to_dmg_assets() {
    let mut cgb_boot = vec![0x00; CGB_BOOT_ROM_RAW_LEN];
    cgb_boot[0x0000] = 0xC0;
    cgb_boot[0x0100] = 0xD2;
    cgb_boot[CGB_BOOT_ROM_RAW_LEN - 1] = 0xE4;

    let assets = BootRomAssets::none()
        .with_bytes(BootRomKind::Cgb, cgb_boot)
        .expect("cgb image should validate");
    let cgb = BootController::new(
        ConsoleModel::GameBoyColor,
        StartupMode::RealBoot,
        BootRomKind::Cgb,
        assets,
    );

    assert_eq!(cgb.read_boot_rom(0x0000), 0xC0);
    assert_eq!(cgb.read_boot_rom(0x0100), 0xFF);
    assert_eq!(cgb.read_boot_rom(0x01FF), 0xFF);
    assert_eq!(cgb.read_boot_rom(0x0200), 0xD2);
    assert_eq!(cgb.read_boot_rom(0x08FF), 0xE4);
}

#[test]
fn cgb_boot_rom_assets_also_accept_sparse_address_space_images() {
    let mut cgb_boot = vec![0xFF; CGB_BOOT_ROM_MAPPED_LEN];
    cgb_boot[0x0000] = 0x44;
    cgb_boot[0x0200] = 0x55;
    cgb_boot[0x08FF] = 0x66;

    let assets = BootRomAssets::none()
        .with_bytes(BootRomKind::Cgb, cgb_boot)
        .expect("sparse cgb image should validate");

    assert_eq!(assets.read_byte(BootRomKind::Cgb, 0x0000), Some(0x44));
    assert_eq!(assets.read_byte(BootRomKind::Cgb, 0x0100), None);
    assert_eq!(assets.read_byte(BootRomKind::Cgb, 0x01FF), None);
    assert_eq!(assets.read_byte(BootRomKind::Cgb, 0x0200), Some(0x55));
    assert_eq!(assets.read_byte(BootRomKind::Cgb, 0x08FF), Some(0x66));
}

#[test]
fn cgb_revision_boot_rom_assets_are_stored_independently() {
    let assets = BootRomAssets::none()
        .with_bytes(BootRomKind::Cgb0, vec![0xC0; CGB_BOOT_ROM_RAW_LEN])
        .expect("CGB0 image should validate")
        .with_bytes(BootRomKind::CgbE, vec![0xCE; CGB_BOOT_ROM_RAW_LEN])
        .expect("CGBE image should validate");

    assert_eq!(assets.dynamic_payload_bytes(), CGB_BOOT_ROM_RAW_LEN * 2);
    assert_eq!(assets.read_byte(BootRomKind::Cgb0, 0x0000), Some(0xC0));
    assert_eq!(assets.read_byte(BootRomKind::CgbE, 0x0000), Some(0xCE));
    assert_eq!(assets.read_byte(BootRomKind::Cgb, 0x0000), None);
}

#[test]
fn direct_boot_state_uses_model_specific_verified_entry_presets() {
    let boot = boot(ConsoleModel::GameBoy, StartupMode::SkipBoot, empty_assets());
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
    assert_eq!(direct_boot.ppu.stat, 0x81);
    assert_eq!(direct_boot.ppu.ly, 153);
    assert_eq!(direct_boot.io.dma, 0xFF);
    assert_eq!(direct_boot.io.interrupt_flag, 0xE1);
    assert_eq!(
        direct_boot.startup_memory_policy,
        StartupMemoryPolicy::DeterministicPatterned
    );
}

#[test]
fn real_boot_power_on_state_seeds_model_specific_hidden_clock_phases() {
    let real_boot = boot(ConsoleModel::GameBoy, StartupMode::RealBoot, empty_assets());
    let startup_state = real_boot
        .real_boot_power_on_state()
        .expect("RealBoot should expose power-on hidden clock state");

    assert_eq!(startup_state.timer.system_counter, 0x0064);
    assert_eq!(startup_state.timer.tima, 0x00);
    assert_eq!(startup_state.timer.tma, 0x00);
    assert_eq!(startup_state.timer.tac, 0x00);
    assert_eq!(startup_state.serial.sb, 0x00);
    assert_eq!(
        startup_state.serial.clock_mode,
        crate::serial::SerialClockMode::External
    );
    assert_eq!(startup_state.serial.clock_counter, 0x0068);
    assert_eq!(startup_state.dma.source_page_latch, 0xFF);
    assert_eq!(startup_state.joypad.selection_bits, 0x00);
    assert_eq!(startup_state.joypad.pressed_mask, 0x00);
    assert!(real_boot.direct_boot_state(None).is_none());

    let cgb_real_boot = boot(
        ConsoleModel::GameBoyColor,
        StartupMode::RealBoot,
        empty_assets(),
    );
    let cgb_startup_state = cgb_real_boot
        .real_boot_power_on_state()
        .expect("CGB RealBoot should expose power-on hidden clock state");
    assert_eq!(cgb_startup_state.timer.system_counter, 0xFFFB);
    assert_eq!(cgb_startup_state.timer.tima, 0x00);
    assert_eq!(cgb_startup_state.timer.tma, 0x00);
    assert_eq!(cgb_startup_state.timer.tac, 0x00);

    let skip_boot = boot(ConsoleModel::GameBoy, StartupMode::SkipBoot, empty_assets());
    assert!(skip_boot.real_boot_power_on_state().is_none());
}

#[test]
fn cgb_skip_boot_cpu_state_matches_boot_regs_cgb_entry_contract() {
    let boot = boot(
        ConsoleModel::GameBoyColor,
        StartupMode::SkipBoot,
        empty_assets(),
    );

    for startup_state in [
        boot.direct_boot_state(None)
            .expect("SkipBoot should expose a direct-boot state"),
        boot.machine_skip_boot_state(None)
            .expect("SkipBoot should expose the machine skip-boot state"),
    ] {
        assert_eq!(startup_state.cpu.pc, 0x0100);
        assert_eq!(startup_state.cpu.sp, 0xFFFE);
        assert_eq!(startup_state.cpu.a, 0x11);
        assert_eq!(startup_state.cpu.f, 0x80);
        assert_eq!(startup_state.cpu.b, 0x00);
        assert_eq!(startup_state.cpu.c, 0x00);
        assert_eq!(startup_state.cpu.d, 0x00);
        assert_eq!(startup_state.cpu.e, 0x08);
        assert_eq!(startup_state.cpu.h, 0x00);
        assert_eq!(startup_state.cpu.l, 0x7C);
        assert_eq!(startup_state.io.p1, 0xFF);
        assert_eq!(startup_state.joypad.selection_bits, 0x30);
        assert_eq!(startup_state.io.div, 0x26);
        assert_eq!(startup_state.timer.system_counter, 0x2674);
    }
}

#[test]
fn direct_boot_helpers_share_the_common_skip_boot_builder() {
    let boot = boot(ConsoleModel::GameBoy, StartupMode::SkipBoot, empty_assets());

    let direct_boot = boot
        .direct_boot_state(None)
        .expect("SkipBoot should expose a direct-boot state");
    let machine_skip_boot = boot
        .machine_skip_boot_state(None)
        .expect("SkipBoot should expose the machine skip-boot state");

    assert_eq!(direct_boot.cpu, machine_skip_boot.cpu);
    assert_eq!(direct_boot.serial, machine_skip_boot.serial);
    assert_eq!(direct_boot.dma, machine_skip_boot.dma);
    assert_eq!(direct_boot.interrupts, machine_skip_boot.interrupts);
    assert_eq!(
        direct_boot.startup_memory_policy,
        machine_skip_boot.startup_memory_policy
    );

    assert_eq!(direct_boot.io.div, 0xAB);
    assert_eq!(machine_skip_boot.io.div, 0xAB);
    assert_eq!(direct_boot.timer.system_counter, 0xABC8);
    assert_eq!(machine_skip_boot.timer.system_counter, 0xABC8);
    assert_eq!(direct_boot.ppu.ly, 153);
    assert_eq!(machine_skip_boot.ppu.ly, 0);
    assert_eq!(direct_boot.joypad.selection_bits, direct_boot.io.p1 & 0x30);
    assert_eq!(
        machine_skip_boot.joypad.selection_bits,
        machine_skip_boot.io.p1 & 0x30
    );
}

#[test]
fn patterned_startup_memory_policy_is_deterministic_without_zero_filling_wram_or_hram() {
    let mut first_wram = [0; 8];
    let mut second_wram = [0; 8];
    let mut first_hram = [0; 8];
    let mut second_hram = [0; 8];

    StartupMemoryPolicy::DeterministicPatterned.initialize_wram(&mut first_wram);
    StartupMemoryPolicy::DeterministicPatterned.initialize_wram(&mut second_wram);
    StartupMemoryPolicy::DeterministicPatterned.initialize_hram(&mut first_hram);
    StartupMemoryPolicy::DeterministicPatterned.initialize_hram(&mut second_hram);

    assert_eq!(first_wram, second_wram);
    assert_eq!(first_hram, second_hram);
    assert_ne!(first_wram, [0; 8]);
    assert_ne!(first_hram, [0; 8]);
}

#[test]
fn cgb_real_boot_entry_memory_policy_uses_boot_visible_prefixes() {
    let mut vram = [0xFF; 40];
    let mut wram = [0xFF; 8];
    let mut hram = [0xFF; 56];
    let mut no_op = [0xAA; 4];

    StartupMemoryPolicy::CgbRealBootEntry.initialize_vram(&mut vram);
    StartupMemoryPolicy::CgbRealBootEntry.initialize_wram(&mut wram);
    StartupMemoryPolicy::CgbRealBootEntry.initialize_hram(&mut hram);
    StartupMemoryPolicy::CgbRealBootEntry.fill_bytes(&mut no_op, 0xC000);

    assert_eq!(
        &vram[..CGB_REAL_BOOT_VRAM_PREFIX.len()],
        CGB_REAL_BOOT_VRAM_PREFIX
    );
    assert_eq!(vram[CGB_REAL_BOOT_VRAM_PREFIX.len()], 0x00);
    assert_eq!(wram, [0x00; 8]);
    assert_eq!(
        &hram[..CGB_BOOT_LOGO_HRAM_PREFIX.len()],
        CGB_BOOT_LOGO_HRAM_PREFIX
    );
    assert_eq!(hram[CGB_BOOT_LOGO_HRAM_PREFIX.len()], 0x00);
    assert_eq!(no_op, [0xAA; 4]);
}

#[test]
fn dmg_family_skip_boot_flags_follow_the_header_checksum_rule() {
    let boot = boot(ConsoleModel::GameBoy, StartupMode::SkipBoot, empty_assets());
    let mut rom = vec![0x00; 0x150];
    rom[0x014D] = 0x00;
    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(
        boot.direct_boot_state(None).unwrap().cpu.f,
        0xB0,
        "empty-slot fallback keeps the common DMG preset"
    );
    assert_eq!(
        build_skip_boot_cpu_state(ConsoleModel::GameBoy, Some(&header)).f,
        0x80
    );
    rom[0x014D] = 0x7F;
    let header = CartridgeHeader::parse(&rom).expect("header should parse");
    assert_eq!(
        build_skip_boot_cpu_state(ConsoleModel::GameBoy, Some(&header)).f,
        0xB0
    );
}

#[test]
fn missing_boot_rom_assets_fall_back_to_ff_reads_without_fake_placeholder_data() {
    let boot = boot(ConsoleModel::GameBoy, StartupMode::RealBoot, empty_assets());

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
        vec![0x42; DMG_FAMILY_BOOT_ROM_LEN],
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
fn boot_rom_assets_can_load_all_directory_images_independently() {
    let directory = unique_temp_dir();
    fs::create_dir_all(&directory).expect("temporary asset directory should be creatable");
    for (kind, marker, len) in [
        (BootRomKind::Dmg0, 0xD0, DMG_FAMILY_BOOT_ROM_LEN),
        (BootRomKind::Dmg, 0xD1, DMG_FAMILY_BOOT_ROM_LEN),
        (BootRomKind::Mgb, 0xD2, DMG_FAMILY_BOOT_ROM_LEN),
        (BootRomKind::Cgb0, 0xC0, CGB_BOOT_ROM_RAW_LEN),
        (BootRomKind::Cgb, 0xC1, CGB_BOOT_ROM_RAW_LEN),
        (BootRomKind::CgbE, 0xCE, CGB_BOOT_ROM_RAW_LEN),
    ] {
        fs::write(
            directory.join(BootRomAssets::filename(kind)),
            vec![marker; len],
        )
        .expect("boot ROM file should be writable");
    }

    let assets = BootRomAssets::from_directory(&directory)
        .expect("complete directory-backed boot ROM assets should load");

    assert_eq!(
        assets.dynamic_payload_bytes(),
        DMG_FAMILY_BOOT_ROM_LEN * 3 + CGB_BOOT_ROM_RAW_LEN * 3
    );
    assert_eq!(assets.read_byte(BootRomKind::Dmg0, 0x0000), Some(0xD0));
    assert_eq!(assets.read_byte(BootRomKind::Dmg, 0x0000), Some(0xD1));
    assert_eq!(assets.read_byte(BootRomKind::Mgb, 0x0000), Some(0xD2));
    assert_eq!(assets.read_byte(BootRomKind::Cgb0, 0x0000), Some(0xC0));
    assert_eq!(assets.read_byte(BootRomKind::Cgb, 0x0000), Some(0xC1));
    assert_eq!(assets.read_byte(BootRomKind::CgbE, 0x0000), Some(0xCE));

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
fn boot_rom_asset_directory_length_errors_cover_each_model_slot() {
    for (kind, len) in [
        (BootRomKind::Dmg0, DMG_FAMILY_BOOT_ROM_LEN - 1),
        (BootRomKind::Dmg, DMG_FAMILY_BOOT_ROM_LEN - 1),
        (BootRomKind::Mgb, DMG_FAMILY_BOOT_ROM_LEN - 1),
        (BootRomKind::Cgb0, CGB_BOOT_ROM_RAW_LEN - 1),
        (BootRomKind::Cgb, CGB_BOOT_ROM_RAW_LEN - 1),
        (BootRomKind::CgbE, CGB_BOOT_ROM_RAW_LEN - 1),
    ] {
        let directory = unique_temp_dir();
        fs::create_dir_all(&directory).expect("temporary asset directory should be creatable");
        fs::write(
            directory.join(BootRomAssets::filename(kind)),
            vec![0xFF; len],
        )
        .expect("short boot ROM file should be writable");

        let error = BootRomAssets::from_directory(&directory)
            .expect_err("too-short boot ROM files should be rejected");
        assert!(matches!(
            error,
            BootRomAssetError::ImageTooShort {
                kind: error_kind,
                ..
            } if error_kind == kind
        ));
        assert!(error.to_string().contains("too short"));
        assert!(std::error::Error::source(&error).is_none());

        fs::remove_dir_all(&directory).expect("temporary asset directory should be removable");
    }
}

#[test]
fn boot_rom_assets_cover_all_kind_slots_and_exact_filenames() {
    let assets = BootRomAssets::none()
        .with_bytes(BootRomKind::Dmg0, vec![0x10; DMG_FAMILY_BOOT_ROM_LEN])
        .expect("dmg0 image should validate")
        .with_bytes(BootRomKind::Dmg, vec![0x20; DMG_FAMILY_BOOT_ROM_LEN])
        .expect("dmg image should validate")
        .with_bytes(BootRomKind::Mgb, vec![0x30; DMG_FAMILY_BOOT_ROM_LEN])
        .expect("mgb image should validate")
        .with_bytes(BootRomKind::Cgb, vec![0x40; CGB_BOOT_ROM_RAW_LEN])
        .expect("cgb image should validate");

    assert_eq!(BootRomAssets::filename(BootRomKind::Dmg0), "dmg0_boot.bin");
    assert_eq!(BootRomAssets::filename(BootRomKind::Dmg), "dmg_boot.bin");
    assert_eq!(BootRomAssets::filename(BootRomKind::Mgb), "mgb_boot.bin");
    assert_eq!(BootRomAssets::filename(BootRomKind::Cgb), "cgb_boot.bin");
    assert!(!assets.is_empty());
    assert!(assets.has_image(BootRomKind::Dmg0));
    assert!(assets.has_image(BootRomKind::Dmg));
    assert!(assets.has_image(BootRomKind::Mgb));
    assert!(assets.has_image(BootRomKind::Cgb));
    assert_eq!(assets.read_byte(BootRomKind::Dmg0, 0x0000), Some(0x10));
    assert_eq!(assets.read_byte(BootRomKind::Dmg, 0x0000), Some(0x20));
    assert_eq!(assets.read_byte(BootRomKind::Mgb, 0x0000), Some(0x30));
    assert_eq!(assets.read_byte(BootRomKind::Cgb, 0x0000), Some(0x40));
}
