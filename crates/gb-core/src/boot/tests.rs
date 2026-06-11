use super::*;
use crate::cartridge::CartridgeHeader;
use crate::model::CompatibilityPolicy;
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
    boot_with_revision(
        console_model,
        console_model.default_revision(),
        startup_mode,
        assets,
    )
}

fn boot_with_revision(
    console_model: ConsoleModel,
    revision: HardwareRevision,
    startup_mode: StartupMode,
    assets: BootRomAssets,
) -> BootController {
    BootController::new(console_model, revision, startup_mode, assets)
}

fn test_rom_with_cgb_header(
    cgb_flag: u8,
    old_licensee_code: u8,
    new_licensee_code: [u8; 2],
) -> Vec<u8> {
    let mut title = [0; 16];
    title[..b"BULLYGB".len()].copy_from_slice(b"BULLYGB");

    test_rom_with_cgb_header_and_title(title, cgb_flag, old_licensee_code, new_licensee_code)
}

fn test_rom_with_cgb_header_and_title(
    title_bytes: [u8; 16],
    cgb_flag: u8,
    old_licensee_code: u8,
    new_licensee_code: [u8; 2],
) -> Vec<u8> {
    let mut rom = vec![0x00; 32 * 1024];
    rom[0x0134..0x0144].copy_from_slice(&title_bytes);
    rom[0x0143] = cgb_flag;
    rom[0x0144..0x0146].copy_from_slice(&new_licensee_code);
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x014B] = old_licensee_code;
    rom
}

fn test_header_with_cgb_header(
    cgb_flag: u8,
    old_licensee_code: u8,
    new_licensee_code: [u8; 2],
) -> CartridgeHeader {
    CartridgeHeader::parse(&test_rom_with_cgb_header(
        cgb_flag,
        old_licensee_code,
        new_licensee_code,
    ))
    .expect("test ROM header should parse")
}

fn test_header_with_cgb_header_and_title(
    title_bytes: [u8; 16],
    cgb_flag: u8,
    old_licensee_code: u8,
    new_licensee_code: [u8; 2],
) -> CartridgeHeader {
    CartridgeHeader::parse(&test_rom_with_cgb_header_and_title(
        title_bytes,
        cgb_flag,
        old_licensee_code,
        new_licensee_code,
    ))
    .expect("test ROM header should parse")
}

fn loaded_test_cartridge_with_cgb_header(
    cgb_flag: u8,
    old_licensee_code: u8,
    new_licensee_code: [u8; 2],
) -> CartridgeSlot {
    let report = CartridgeSlot::load(
        test_rom_with_cgb_header(cgb_flag, old_licensee_code, new_licensee_code),
        &CompatibilityPolicy::strict(),
    )
    .expect("test ROM should load as a NoMBC cartridge");
    report.into_parts().0
}

fn loaded_test_cartridge_with_cgb_header_and_title(
    title_bytes: [u8; 16],
    cgb_flag: u8,
    old_licensee_code: u8,
    new_licensee_code: [u8; 2],
) -> CartridgeSlot {
    let report = CartridgeSlot::load(
        test_rom_with_cgb_header_and_title(
            title_bytes,
            cgb_flag,
            old_licensee_code,
            new_licensee_code,
        ),
        &CompatibilityPolicy::strict(),
    )
    .expect("test ROM should load as a NoMBC cartridge");
    report.into_parts().0
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
    let custom_boot = boot(
        ConsoleModel::GameBoy,
        StartupMode::CustomBoot,
        empty_assets(),
    );

    assert!(real_boot.is_boot_rom_mapped());
    assert!(!skip_boot.is_boot_rom_mapped());
    assert!(!custom_boot.is_boot_rom_mapped());
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
    let agb = boot(
        ConsoleModel::GameBoyAdvance,
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
    assert!(agb.bus_state().maps_low_window());
    assert!(agb.bus_state().maps_cgb_upper_window());
    assert!(!skip_boot.bus_state().maps_low_window());
    assert!(!skip_boot.bus_state().maps_cgb_upper_window());
}

#[test]
fn console_model_defaults_to_the_expected_boot_kind() {
    assert_eq!(
        boot(ConsoleModel::GameBoy, StartupMode::RealBoot, empty_assets()).revision(),
        HardwareRevision::DmgCpuC
    );
    assert_eq!(
        boot(
            ConsoleModel::GameBoyPocket,
            StartupMode::RealBoot,
            empty_assets()
        )
        .revision(),
        HardwareRevision::CpuMgb
    );
    assert_eq!(
        boot(
            ConsoleModel::GameBoyLight,
            StartupMode::RealBoot,
            empty_assets()
        )
        .revision(),
        HardwareRevision::CpuMgb
    );
    assert_eq!(
        boot(
            ConsoleModel::GameBoyColor,
            StartupMode::RealBoot,
            empty_assets()
        )
        .revision(),
        HardwareRevision::CpuCgbE
    );
    assert_eq!(
        boot(
            ConsoleModel::GameBoyAdvance,
            StartupMode::RealBoot,
            empty_assets()
        )
        .revision(),
        HardwareRevision::CpuAgbA
    );
}

#[test]
fn boot_controller_uses_the_explicit_boot_kind() {
    assert_eq!(
        BootController::new(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpu0,
            StartupMode::RealBoot,
            empty_assets(),
        )
        .revision(),
        HardwareRevision::DmgCpu0
    );
}

#[test]
fn boot_rom_reads_come_from_the_configured_selected_model_image() {
    let assets = BootRomAssets::none()
        .with_bytes(
            HardwareRevision::DmgCpuC,
            vec![0xC0; DMG_FAMILY_BOOT_ROM_LEN],
        )
        .expect("dmg image should validate")
        .with_bytes(
            HardwareRevision::CpuMgb,
            vec![0xB0; DMG_FAMILY_BOOT_ROM_LEN],
        )
        .expect("mgb image should validate");
    let dmg = BootController::new(
        ConsoleModel::GameBoy,
        HardwareRevision::DmgCpuC,
        StartupMode::RealBoot,
        assets.clone(),
    );
    let mgb = BootController::new(
        ConsoleModel::GameBoyPocket,
        HardwareRevision::CpuMgb,
        StartupMode::RealBoot,
        assets,
    );

    assert_eq!(dmg.read_boot_rom(0x0000), 0xC0);
    assert_eq!(mgb.read_boot_rom(0x0000), 0xB0);
    assert_ne!(dmg.read_boot_rom(0x0000), mgb.read_boot_rom(0x0000));
}

#[test]
fn sgb_real_boot_reads_come_from_sgb_boot_assets_instead_of_dmg_revision_assets() {
    let assets = BootRomAssets::none()
        .with_bytes(
            HardwareRevision::DmgCpuC,
            vec![0xD0; DMG_FAMILY_BOOT_ROM_LEN],
        )
        .expect("dmg image should validate")
        .with_asset_bytes(BootRomAssetKind::Sgb, vec![0x51; DMG_FAMILY_BOOT_ROM_LEN])
        .expect("sgb image should validate")
        .with_asset_bytes(BootRomAssetKind::Sgb2, vec![0x52; DMG_FAMILY_BOOT_ROM_LEN])
        .expect("sgb2 image should validate");
    let sgb = BootController::new_with_sgb_profile(
        ConsoleModel::GameBoy,
        HardwareRevision::DmgCpuC,
        Some(SgbHostProfile::SgbNtsc),
        StartupMode::RealBoot,
        assets.clone(),
    );
    let sgb_pal = BootController::new_with_sgb_profile(
        ConsoleModel::GameBoy,
        HardwareRevision::DmgCpuC,
        Some(SgbHostProfile::SgbPal),
        StartupMode::RealBoot,
        assets.clone(),
    );
    let sgb2 = BootController::new_with_sgb_profile(
        ConsoleModel::GameBoy,
        HardwareRevision::DmgCpuC,
        Some(SgbHostProfile::Sgb2Ntsc),
        StartupMode::RealBoot,
        assets,
    );

    assert_eq!(sgb.boot_rom_asset(), BootRomAssetKind::Sgb);
    assert_eq!(sgb_pal.boot_rom_asset(), BootRomAssetKind::Sgb);
    assert_eq!(sgb2.boot_rom_asset(), BootRomAssetKind::Sgb2);
    assert!(sgb.has_boot_rom_asset());
    assert!(sgb2.has_boot_rom_asset());
    assert_eq!(sgb.read_boot_rom(0x0000), 0x51);
    assert_eq!(sgb_pal.read_boot_rom(0x0000), 0x51);
    assert_eq!(sgb2.read_boot_rom(0x0000), 0x52);
}

#[test]
fn cgb_boot_rom_reads_cover_both_overlay_windows_without_aliasing_to_dmg_assets() {
    let mut cgb_boot = vec![0x00; CGB_BOOT_ROM_RAW_LEN];
    cgb_boot[0x0000] = 0xC0;
    cgb_boot[0x0100] = 0xD2;
    cgb_boot[CGB_BOOT_ROM_RAW_LEN - 1] = 0xE4;

    let assets = BootRomAssets::none()
        .with_bytes(HardwareRevision::CpuCgbC, cgb_boot)
        .expect("cgb image should validate");
    let cgb = BootController::new(
        ConsoleModel::GameBoyColor,
        HardwareRevision::CpuCgbC,
        StartupMode::RealBoot,
        assets,
    );

    assert_eq!(cgb.read_boot_rom(0x0000), 0xC0);
    assert_eq!(cgb.read_boot_rom(0x0100), 0xFF);
    assert_eq!(cgb.read_boot_rom(0x01FF), 0xFF);
    assert_eq!(cgb.read_boot_rom(0x0200), 0xD2);
    assert_eq!(cgb.read_boot_rom(0x08FF), 0xE4);
}

#[test]
fn agb_real_boot_reads_use_the_dedicated_cgb_agb_boot_asset() {
    let mut agb_boot = vec![0x00; CGB_BOOT_ROM_RAW_LEN];
    agb_boot[0x0000] = 0xA0;
    agb_boot[0x0100] = 0xB0;
    agb_boot[CGB_BOOT_ROM_RAW_LEN - 1] = 0xC0;

    let assets = BootRomAssets::none()
        .with_bytes(HardwareRevision::CpuCgbE, vec![0xE0; CGB_BOOT_ROM_RAW_LEN])
        .expect("cgb-e image should validate")
        .with_bytes(HardwareRevision::CpuAgbA, agb_boot)
        .expect("agb image should validate");
    let agb = BootController::new(
        ConsoleModel::GameBoyAdvance,
        HardwareRevision::CpuAgbA,
        StartupMode::RealBoot,
        assets,
    );

    assert_eq!(agb.boot_rom_asset(), BootRomAssetKind::CgbAgb);
    assert_eq!(agb.read_boot_rom(0x0000), 0xA0);
    assert_eq!(agb.read_boot_rom(0x0100), 0xFF);
    assert_eq!(agb.read_boot_rom(0x0200), 0xB0);
    assert_eq!(agb.read_boot_rom(0x08FF), 0xC0);
}

#[test]
fn cgb_boot_rom_assets_also_accept_sparse_address_space_images() {
    let mut cgb_boot = vec![0xFF; CGB_BOOT_ROM_MAPPED_LEN];
    cgb_boot[0x0000] = 0x44;
    cgb_boot[0x0200] = 0x55;
    cgb_boot[0x08FF] = 0x66;

    let assets = BootRomAssets::none()
        .with_bytes(HardwareRevision::CpuCgbC, cgb_boot)
        .expect("sparse cgb image should validate");

    assert_eq!(
        assets.read_byte(HardwareRevision::CpuCgbC, 0x0000),
        Some(0x44)
    );
    assert_eq!(assets.read_byte(HardwareRevision::CpuCgbC, 0x0100), None);
    assert_eq!(assets.read_byte(HardwareRevision::CpuCgbC, 0x01FF), None);
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuCgbC, 0x0200),
        Some(0x55)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuCgbC, 0x08FF),
        Some(0x66)
    );
}

#[test]
fn hardware_revision_boot_rom_assets_are_stored_independently() {
    let assets = BootRomAssets::none()
        .with_bytes(HardwareRevision::CpuCgb0, vec![0xC0; CGB_BOOT_ROM_RAW_LEN])
        .expect("CGB0 image should validate")
        .with_bytes(HardwareRevision::CpuCgbE, vec![0xCE; CGB_BOOT_ROM_RAW_LEN])
        .expect("CGBE image should validate");

    assert_eq!(assets.dynamic_payload_bytes(), CGB_BOOT_ROM_RAW_LEN * 2);
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuCgb0, 0x0000),
        Some(0xC0)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuCgbE, 0x0000),
        Some(0xCE)
    );
    assert_eq!(assets.read_byte(HardwareRevision::CpuCgbC, 0x0000), None);
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
    assert_eq!(direct_boot.apu.div_apu, 0x01);
    assert_eq!(direct_boot.io.dma, 0xFF);
    assert_eq!(direct_boot.io.interrupt_flag, 0xE1);
    assert_eq!(
        direct_boot.startup_memory_policy,
        StartupMemoryPolicy::DmgBootLogoVram
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
    let custom_boot = boot(
        ConsoleModel::GameBoy,
        StartupMode::CustomBoot,
        empty_assets(),
    );
    assert!(custom_boot.real_boot_power_on_state().is_none());
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
fn agb_skip_boot_cpu_state_exposes_gba_enhanced_detection_registers() {
    let boot = boot(
        ConsoleModel::GameBoyAdvance,
        StartupMode::SkipBoot,
        empty_assets(),
    );
    let native_cartridge = loaded_test_cartridge_with_cgb_header(0x80, 0x00, *b"00");
    let dmg_compatible_cartridge = loaded_test_cartridge_with_cgb_header(0x00, 0x00, *b"00");

    let native = boot
        .direct_boot_state(Some(&native_cartridge))
        .expect("AGB SkipBoot should expose a direct-boot state");
    assert_eq!(native.cpu.a, 0x11);
    assert_eq!(native.cpu.f, 0x00);
    assert_eq!(native.cpu.b, 0x01);
    assert_eq!(native.cpu.c, 0x00);
    assert_eq!(native.cpu.d, 0xFF);
    assert_eq!(native.cpu.e, 0x56);
    assert_eq!(native.io.p1, 0xFF);
    assert_eq!(native.joypad.selection_bits, 0x30);

    let dmg_compatible = boot
        .direct_boot_state(Some(&dmg_compatible_cartridge))
        .expect("AGB SkipBoot should expose a direct-boot state");
    assert_eq!(dmg_compatible.cpu.a, 0x11);
    assert_eq!(dmg_compatible.cpu.f, 0x00);
    assert_eq!(dmg_compatible.cpu.b, 0x01);
    assert_eq!(dmg_compatible.cpu.d, 0x00);
    assert_eq!(dmg_compatible.cpu.e, 0x08);
}

#[test]
fn cgb_family_dmg_mode_boot_b_uses_nintendo_title_checksum() {
    let mut title = [0; 16];
    title[0] = 0x88;
    let cartridge = loaded_test_cartridge_with_cgb_header_and_title(title, 0x00, 0x01, *b"00");

    let cgb = boot(
        ConsoleModel::GameBoyColor,
        StartupMode::SkipBoot,
        empty_assets(),
    )
    .direct_boot_state(Some(&cartridge))
    .expect("CGB SkipBoot should expose a direct-boot state");
    assert_eq!(cgb.cpu.f, 0x80);
    assert_eq!(cgb.cpu.b, 0x88);
    assert_eq!(cgb.cpu.h, 0x00);
    assert_eq!(cgb.cpu.l, 0x7C);

    let agb = boot(
        ConsoleModel::GameBoyAdvance,
        StartupMode::SkipBoot,
        empty_assets(),
    )
    .direct_boot_state(Some(&cartridge))
    .expect("AGB SkipBoot should expose a direct-boot state");
    assert_eq!(agb.cpu.f, 0x00);
    assert_eq!(agb.cpu.b, 0x89);
    assert_eq!(agb.cpu.h, 0x00);
    assert_eq!(agb.cpu.l, 0x7C);
}

#[test]
fn agb_dmg_mode_boot_flags_follow_the_final_b_increment() {
    let mut half_carry_title = [0; 16];
    half_carry_title[0] = 0x0F;
    let half_carry_header =
        test_header_with_cgb_header_and_title(half_carry_title, 0x00, 0x01, *b"00");
    let half_carry = build_skip_boot_cpu_state(
        ConsoleModel::GameBoyAdvance,
        HardwareRevision::CpuAgbA,
        None,
        Some(&half_carry_header),
    );
    assert_eq!(half_carry.b, 0x10);
    assert_eq!(half_carry.f, 0x20);

    let mut zero_title = [0; 16];
    zero_title[0] = 0xFF;
    let zero_header = test_header_with_cgb_header_and_title(zero_title, 0x00, 0x01, *b"00");
    let zero = build_skip_boot_cpu_state(
        ConsoleModel::GameBoyAdvance,
        HardwareRevision::CpuAgbA,
        None,
        Some(&zero_header),
    );
    assert_eq!(zero.b, 0x00);
    assert_eq!(zero.f, 0xA0);
}

#[test]
fn cgb_family_dmg_mode_boot_hl_tracks_logo_tilemap_checksum_cases() {
    for (title_checksum, cgb_b, agb_b) in [(0x43, 0x43, 0x44), (0x58, 0x58, 0x59)] {
        let mut title = [0; 16];
        title[0] = title_checksum;
        let header = test_header_with_cgb_header_and_title(title, 0x00, 0x01, *b"00");

        let cgb = build_skip_boot_cpu_state(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbE,
            None,
            Some(&header),
        );
        assert_eq!(cgb.b, cgb_b);
        assert_eq!(cgb.h, 0x99);
        assert_eq!(cgb.l, 0x1A);

        let agb = build_skip_boot_cpu_state(
            ConsoleModel::GameBoyAdvance,
            HardwareRevision::CpuAgbA,
            None,
            Some(&header),
        );
        assert_eq!(agb.b, agb_b);
        assert_eq!(agb.h, 0x99);
        assert_eq!(agb.l, 0x1A);
    }

    let mut title = [0; 16];
    title[0] = 0x42;
    let header = test_header_with_cgb_header_and_title(title, 0x00, 0x01, *b"00");

    let cgb = build_skip_boot_cpu_state(
        ConsoleModel::GameBoyColor,
        HardwareRevision::CpuCgbE,
        None,
        Some(&header),
    );
    assert_eq!(cgb.b, 0x42);
    assert_eq!(cgb.h, 0x00);
    assert_eq!(cgb.l, 0x7C);

    let agb = build_skip_boot_cpu_state(
        ConsoleModel::GameBoyAdvance,
        HardwareRevision::CpuAgbA,
        None,
        Some(&header),
    );
    assert_eq!(agb.b, 0x43);
    assert_eq!(agb.h, 0x00);
    assert_eq!(agb.l, 0x7C);
}

#[test]
fn cgb_direct_start_system_counter_uses_native_non_nintendo_header_bucket() {
    let header = test_header_with_cgb_header(0x80, 0x00, *b"00");

    assert_eq!(
        direct_start_system_counter(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbE,
            Some(&header)
        ),
        0x1E84
    );
}

#[test]
fn cgb_direct_start_system_counter_uses_native_binary_zero_new_licensee_header_bucket() {
    let header = test_header_with_cgb_header(0x80, 0x33, [0x00, 0x00]);

    assert_eq!(
        direct_start_system_counter(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbE,
            Some(&header)
        ),
        0x1E98
    );
    assert_eq!(cgb_real_boot_handoff_correction_t_cycles(Some(&header)), 24);
}

#[test]
fn cgb_direct_start_system_counter_keeps_mooneye_baseline_for_missing_or_dmg_headers() {
    let dmg_compatible_header = test_header_with_cgb_header(0x00, 0x33, *b"ZZ");
    let cgb_new_licensee_header = test_header_with_cgb_header(0x80, 0x33, *b"00");
    let cgb_nintendo_header = test_header_with_cgb_header(0x80, 0x01, *b"00");

    assert_eq!(
        direct_start_system_counter(ConsoleModel::GameBoyColor, HardwareRevision::CpuCgbE, None),
        0x2674
    );
    assert_eq!(
        direct_start_system_counter(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbE,
            Some(&dmg_compatible_header)
        ),
        0x2674
    );
    assert_eq!(
        direct_start_system_counter(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbE,
            Some(&cgb_new_licensee_header)
        ),
        0x2674
    );
    assert_eq!(
        direct_start_system_counter(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgbE,
            Some(&cgb_nintendo_header)
        ),
        0x2674
    );
}

#[test]
fn cgb0_direct_start_system_counter_uses_early_boot_handoff_bucket() {
    let dmg_compatible_header = test_header_with_cgb_header(0x00, 0x33, *b"ZZ");
    let native_header = test_header_with_cgb_header(0x80, 0x00, *b"00");

    assert_eq!(
        direct_start_system_counter(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgb0,
            Some(&dmg_compatible_header)
        ),
        0x2880
    );
    assert_eq!(
        direct_start_system_counter(
            ConsoleModel::GameBoyColor,
            HardwareRevision::CpuCgb0,
            Some(&native_header)
        ),
        0x2880
    );
}

#[test]
fn cgb_skip_and_custom_boot_share_header_derived_timer_for_native_non_nintendo_roms() {
    let cartridge = loaded_test_cartridge_with_cgb_header(0x80, 0x00, *b"00");
    let skip_boot = boot(
        ConsoleModel::GameBoyColor,
        StartupMode::SkipBoot,
        empty_assets(),
    );
    let custom_boot = boot(
        ConsoleModel::GameBoyColor,
        StartupMode::CustomBoot,
        empty_assets(),
    );

    let direct_boot = skip_boot
        .direct_boot_state(Some(&cartridge))
        .expect("SkipBoot should expose a direct-boot state");
    let machine_skip_boot = skip_boot
        .machine_skip_boot_state(Some(&cartridge))
        .expect("SkipBoot should expose the machine skip-boot state");
    let machine_custom_boot = custom_boot
        .machine_skip_boot_state(Some(&cartridge))
        .expect("CustomBoot should expose the direct machine startup state");

    for startup_state in [&direct_boot, &machine_skip_boot, &machine_custom_boot] {
        assert_eq!(startup_state.timer.system_counter, 0x1E84);
        assert_eq!(startup_state.io.div, 0x1E);
        assert_eq!(startup_state.apu.div_apu, 0x00);
    }
    assert_eq!(
        machine_skip_boot.startup_memory_policy,
        StartupMemoryPolicy::CgbRealBootEntryWithDmgBootLogoTiles
    );
    assert_eq!(
        machine_custom_boot.startup_memory_policy,
        StartupMemoryPolicy::CgbRealBootEntryWithDmgBootLogoTiles
    );
}

#[test]
fn cgb0_skip_and_custom_boot_use_zeroed_wave_ram_and_early_timer_bucket() {
    let cartridge = loaded_test_cartridge_with_cgb_header(0x00, 0x33, *b"ZZ");
    let skip_boot = boot_with_revision(
        ConsoleModel::GameBoyColor,
        HardwareRevision::CpuCgb0,
        StartupMode::SkipBoot,
        empty_assets(),
    );
    let custom_boot = boot_with_revision(
        ConsoleModel::GameBoyColor,
        HardwareRevision::CpuCgb0,
        StartupMode::CustomBoot,
        empty_assets(),
    );

    for startup_state in [
        skip_boot
            .machine_skip_boot_state(Some(&cartridge))
            .expect("SkipBoot should expose CGB0 direct state"),
        custom_boot
            .machine_skip_boot_state(Some(&cartridge))
            .expect("CustomBoot should expose CGB0 direct state"),
    ] {
        assert_eq!(startup_state.timer.system_counter, 0x2880);
        assert_eq!(startup_state.io.div, 0x28);
        assert_eq!(
            startup_state.apu.wave_ram_startup_policy,
            WaveRamStartupPolicy::DeterministicZeroed
        );
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
fn custom_boot_memory_policy_seeds_the_dmg_boot_logo_vram_contract() {
    let mut vram = [0xAA; 0x2000];
    let mut wram = [0; 8];
    let mut hram = [0; 8];

    StartupMemoryPolicy::DmgBootLogoVram.initialize_vram(&mut vram);
    StartupMemoryPolicy::DmgBootLogoVram.initialize_wram(&mut wram);
    StartupMemoryPolicy::DmgBootLogoVram.initialize_hram(&mut hram);

    assert_eq!(
        vram[vram_offset(DMG_BOOT_LOGO_TILE_VRAM_START)],
        DMG_BOOT_LOGO_TILE_BYTES[0]
    );
    assert_eq!(
        vram[vram_offset(0x8190)],
        DMG_BOOT_LOGO_TILE_BYTES[DMG_BOOT_LOGO_TILE_BYTES.len() - 8]
    );
    assert_eq!(
        vram[vram_offset(DMG_BOOT_LOGO_MAP_VRAM_START)],
        DMG_BOOT_LOGO_MAP_BYTES[0]
    );
    assert_eq!(vram[vram_offset(0x9924)], DMG_BOOT_LOGO_MAP_BYTES[32]);
    assert_eq!(vram[vram_offset(0x992F)], DMG_BOOT_LOGO_MAP_BYTES[43]);
    assert_ne!(wram, [0; 8]);
    assert_ne!(hram, [0; 8]);
}

#[test]
fn cgb_custom_boot_memory_policy_preserves_cgb_base_and_overlays_only_the_dmg_boot_logo_tiles() {
    let mut vram = [0xAA; 0x4000];
    let mut wram = [0xAA; 8];
    let mut hram = [0xAA; 56];

    StartupMemoryPolicy::CgbRealBootEntryWithDmgBootLogoTiles.initialize_vram(&mut vram);
    StartupMemoryPolicy::CgbRealBootEntryWithDmgBootLogoTiles.initialize_wram(&mut wram);
    StartupMemoryPolicy::CgbRealBootEntryWithDmgBootLogoTiles.initialize_hram(&mut hram);

    assert_eq!(&vram[..16], &CGB_REAL_BOOT_VRAM_PREFIX[..16]);
    assert_eq!(
        vram[vram_offset(DMG_BOOT_LOGO_TILE_VRAM_START)],
        DMG_BOOT_LOGO_TILE_BYTES[0]
    );
    assert_eq!(vram[vram_offset(DMG_BOOT_LOGO_MAP_VRAM_START)], 0x00);
    assert_eq!(vram[vram_offset(0x9924)], 0x00);
    assert_eq!(vram[vram_offset(0x992F)], 0x00);
    assert_eq!(wram, [0x00; 8]);
    assert_eq!(
        &hram[..CGB_BOOT_LOGO_HRAM_PREFIX.len()],
        CGB_BOOT_LOGO_HRAM_PREFIX
    );
}

#[test]
fn cgb_real_boot_entry_with_dmg_logo_vram_policy_overlays_tiles_and_tilemap() {
    let mut vram = [0xAA; 0x4000];

    StartupMemoryPolicy::CgbRealBootEntryWithDmgBootLogoVram.initialize_vram(&mut vram);

    assert_eq!(&vram[..16], &CGB_REAL_BOOT_VRAM_PREFIX[..16]);
    assert_eq!(
        vram[vram_offset(DMG_BOOT_LOGO_TILE_VRAM_START)],
        DMG_BOOT_LOGO_TILE_BYTES[0]
    );
    assert_eq!(
        vram[vram_offset(DMG_BOOT_LOGO_MAP_VRAM_START)],
        DMG_BOOT_LOGO_MAP_BYTES[0]
    );
    assert_eq!(vram[vram_offset(0x9924)], DMG_BOOT_LOGO_MAP_BYTES[32]);
    assert_eq!(vram[vram_offset(0x992F)], DMG_BOOT_LOGO_MAP_BYTES[43]);
}

#[test]
fn skip_and_custom_boot_policies_seed_model_specific_boot_logo_vram() {
    let dmg_skip_boot = boot(ConsoleModel::GameBoy, StartupMode::SkipBoot, empty_assets());
    let dmg_custom_boot = boot(
        ConsoleModel::GameBoy,
        StartupMode::CustomBoot,
        empty_assets(),
    );
    let cgb_skip_boot = boot(
        ConsoleModel::GameBoyColor,
        StartupMode::SkipBoot,
        empty_assets(),
    );
    let cgb_custom_boot = boot(
        ConsoleModel::GameBoyColor,
        StartupMode::CustomBoot,
        empty_assets(),
    );

    assert_eq!(
        dmg_skip_boot.startup_memory_policy(),
        StartupMemoryPolicy::DmgBootLogoVram
    );
    assert_eq!(
        dmg_custom_boot.startup_memory_policy(),
        StartupMemoryPolicy::DmgBootLogoVram
    );
    assert_eq!(
        cgb_skip_boot.startup_memory_policy(),
        StartupMemoryPolicy::CgbRealBootEntryWithDmgBootLogoTiles
    );
    assert_eq!(
        cgb_custom_boot.startup_memory_policy(),
        StartupMemoryPolicy::CgbRealBootEntryWithDmgBootLogoTiles
    );
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
        build_skip_boot_cpu_state(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            None,
            Some(&header)
        )
        .f,
        0x80
    );
    rom[0x014D] = 0x7F;
    let header = CartridgeHeader::parse(&rom).expect("header should parse");
    assert_eq!(
        build_skip_boot_cpu_state(
            ConsoleModel::GameBoy,
            HardwareRevision::DmgCpuC,
            None,
            Some(&header)
        )
        .f,
        0xB0
    );
}

#[test]
fn dmg0_skip_boot_state_uses_revision_specific_cpu_and_hwio() {
    let boot = boot_with_revision(
        ConsoleModel::GameBoy,
        HardwareRevision::DmgCpu0,
        StartupMode::SkipBoot,
        empty_assets(),
    );
    let state = boot
        .machine_skip_boot_state(None)
        .expect("DMG0 SkipBoot should expose a direct startup state");

    assert_eq!(state.cpu.a, 0x01);
    assert_eq!(state.cpu.f, 0x00);
    assert_eq!(state.cpu.b, 0xFF);
    assert_eq!(state.cpu.c, 0x13);
    assert_eq!(state.cpu.d, 0x00);
    assert_eq!(state.cpu.e, 0xC1);
    assert_eq!(state.cpu.h, 0x84);
    assert_eq!(state.cpu.l, 0x03);
    assert_eq!(state.cpu.sp, 0xFFFE);
    assert_eq!(state.cpu.pc, 0x0100);
    assert_eq!(state.io.div, 0x18);
    assert_eq!(state.io.stat, 0x83);
    assert_eq!(state.io.ly, 0x01);
    assert_eq!(state.timer.system_counter, 0x182C);
}

fn vram_offset(address: u16) -> usize {
    usize::from(address - 0x8000)
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
        directory.join(BootRomAssets::filename(HardwareRevision::DmgCpuC)),
        vec![0x42; DMG_FAMILY_BOOT_ROM_LEN],
    )
    .expect("boot ROM file should be writable");

    let assets = BootRomAssets::from_directory(&directory)
        .expect("directory-backed boot ROM assets should load");

    assert!(assets.has_image(HardwareRevision::DmgCpuC));
    assert!(!assets.has_image(HardwareRevision::CpuMgb));
    assert_eq!(
        assets.read_byte(HardwareRevision::DmgCpuC, 0x0000),
        Some(0x42)
    );

    fs::remove_dir_all(&directory).expect("temporary asset directory should be removable");
}

#[test]
fn boot_rom_assets_can_load_all_directory_images_independently() {
    let directory = unique_temp_dir();
    fs::create_dir_all(&directory).expect("temporary asset directory should be creatable");
    for (kind, marker, len) in [
        (HardwareRevision::DmgCpu0, 0xD0, DMG_FAMILY_BOOT_ROM_LEN),
        (HardwareRevision::DmgCpuC, 0xD1, DMG_FAMILY_BOOT_ROM_LEN),
        (HardwareRevision::CpuMgb, 0xD2, DMG_FAMILY_BOOT_ROM_LEN),
        (HardwareRevision::CpuCgb0, 0xC0, CGB_BOOT_ROM_RAW_LEN),
        (HardwareRevision::CpuCgbC, 0xC1, CGB_BOOT_ROM_RAW_LEN),
        (HardwareRevision::CpuCgbE, 0xCE, CGB_BOOT_ROM_RAW_LEN),
        (HardwareRevision::CpuAgbA, 0xA0, CGB_BOOT_ROM_RAW_LEN),
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
        DMG_FAMILY_BOOT_ROM_LEN * 3 + CGB_BOOT_ROM_RAW_LEN * 4
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::DmgCpu0, 0x0000),
        Some(0xD0)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::DmgCpuC, 0x0000),
        Some(0xD1)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuMgb, 0x0000),
        Some(0xD2)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuCgb0, 0x0000),
        Some(0xC0)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuCgbC, 0x0000),
        Some(0xC1)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuCgbE, 0x0000),
        Some(0xCE)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuAgbA, 0x0000),
        Some(0xA0)
    );
    assert!(!assets.has_asset(BootRomAssetKind::Sgb));
    assert!(!assets.has_asset(BootRomAssetKind::Sgb2));

    fs::remove_dir_all(&directory).expect("temporary asset directory should be removable");
}

#[test]
fn boot_rom_assets_can_load_sgb_directory_images_independently() {
    let directory = unique_temp_dir();
    fs::create_dir_all(&directory).expect("temporary asset directory should be creatable");
    fs::write(
        directory.join(BootRomAssets::filename(HardwareRevision::DmgCpuC)),
        vec![0xD1; DMG_FAMILY_BOOT_ROM_LEN],
    )
    .expect("dmg boot ROM file should be writable");
    fs::write(
        directory.join(BootRomAssets::filename_for_asset(BootRomAssetKind::Sgb)),
        vec![0x51; DMG_FAMILY_BOOT_ROM_LEN],
    )
    .expect("sgb boot ROM file should be writable");
    fs::write(
        directory.join(BootRomAssets::filename_for_asset(BootRomAssetKind::Sgb2)),
        vec![0x52; DMG_FAMILY_BOOT_ROM_LEN],
    )
    .expect("sgb2 boot ROM file should be writable");

    let assets = BootRomAssets::from_directory(&directory)
        .expect("directory-backed SGB boot ROM assets should load");

    assert!(assets.has_asset(BootRomAssetKind::Sgb));
    assert!(assets.has_asset(BootRomAssetKind::Sgb2));
    assert_eq!(
        assets.read_asset_byte(BootRomAssetKind::Sgb, 0x0000),
        Some(0x51)
    );
    assert_eq!(
        assets.read_asset_byte(BootRomAssetKind::Sgb2, 0x0000),
        Some(0x52)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::DmgCpuC, 0x0000),
        Some(0xD1)
    );

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
    fs::create_dir_all(directory.join(BootRomAssets::filename(HardwareRevision::DmgCpuC)))
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
        (HardwareRevision::DmgCpu0, DMG_FAMILY_BOOT_ROM_LEN - 1),
        (HardwareRevision::DmgCpuC, DMG_FAMILY_BOOT_ROM_LEN - 1),
        (HardwareRevision::CpuMgb, DMG_FAMILY_BOOT_ROM_LEN - 1),
        (HardwareRevision::CpuCgb0, CGB_BOOT_ROM_RAW_LEN - 1),
        (HardwareRevision::CpuCgbC, CGB_BOOT_ROM_RAW_LEN - 1),
        (HardwareRevision::CpuCgbE, CGB_BOOT_ROM_RAW_LEN - 1),
        (HardwareRevision::CpuAgbA, CGB_BOOT_ROM_RAW_LEN - 1),
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
        assert!(matches!(error, BootRomAssetError::ImageTooShort { .. }));
        assert!(error.to_string().contains("too short"));
        assert!(std::error::Error::source(&error).is_none());

        fs::remove_dir_all(&directory).expect("temporary asset directory should be removable");
    }

    for asset in [BootRomAssetKind::Sgb, BootRomAssetKind::Sgb2] {
        let directory = unique_temp_dir();
        fs::create_dir_all(&directory).expect("temporary asset directory should be creatable");
        fs::write(
            directory.join(BootRomAssets::filename_for_asset(asset)),
            vec![0xFF; DMG_FAMILY_BOOT_ROM_LEN - 1],
        )
        .expect("short SGB boot ROM file should be writable");

        let error = BootRomAssets::from_directory(&directory)
            .expect_err("too-short SGB boot ROM files should be rejected");
        assert!(matches!(error, BootRomAssetError::ImageTooShort { .. }));
        assert!(error.to_string().contains("too short"));
        assert!(std::error::Error::source(&error).is_none());

        fs::remove_dir_all(&directory).expect("temporary asset directory should be removable");
    }
}

#[test]
fn boot_rom_assets_cover_all_kind_slots_and_exact_filenames() {
    let assets = BootRomAssets::none()
        .with_bytes(
            HardwareRevision::DmgCpu0,
            vec![0x10; DMG_FAMILY_BOOT_ROM_LEN],
        )
        .expect("dmg0 image should validate")
        .with_bytes(
            HardwareRevision::DmgCpuC,
            vec![0x20; DMG_FAMILY_BOOT_ROM_LEN],
        )
        .expect("dmg image should validate")
        .with_bytes(
            HardwareRevision::CpuMgb,
            vec![0x30; DMG_FAMILY_BOOT_ROM_LEN],
        )
        .expect("mgb image should validate")
        .with_bytes(HardwareRevision::CpuCgbC, vec![0x40; CGB_BOOT_ROM_RAW_LEN])
        .expect("cgb image should validate")
        .with_bytes(HardwareRevision::CpuAgbA, vec![0xA0; CGB_BOOT_ROM_RAW_LEN])
        .expect("agb image should validate")
        .with_asset_bytes(BootRomAssetKind::Sgb, vec![0x51; DMG_FAMILY_BOOT_ROM_LEN])
        .expect("sgb image should validate")
        .with_asset_bytes(BootRomAssetKind::Sgb2, vec![0x52; DMG_FAMILY_BOOT_ROM_LEN])
        .expect("sgb2 image should validate");

    assert_eq!(
        BootRomAssets::filename(HardwareRevision::DmgCpu0),
        "dmg0_boot.bin"
    );
    assert_eq!(
        BootRomAssets::filename(HardwareRevision::DmgCpuC),
        "dmg_boot.bin"
    );
    assert_eq!(
        BootRomAssets::filename(HardwareRevision::CpuMgb),
        "mgb_boot.bin"
    );
    assert_eq!(
        BootRomAssets::filename(HardwareRevision::CpuCgbC),
        "cgb_boot.bin"
    );
    assert_eq!(
        BootRomAssets::filename(HardwareRevision::CpuAgbA),
        "cgb_agb_boot.bin"
    );
    assert_eq!(
        BootRomAssets::filename_for_asset(BootRomAssetKind::Sgb),
        "sgb_boot.bin"
    );
    assert_eq!(
        BootRomAssets::filename_for_asset(BootRomAssetKind::Sgb2),
        "sgb2_boot.bin"
    );
    assert!(!assets.is_empty());
    assert!(assets.has_image(HardwareRevision::DmgCpu0));
    assert!(assets.has_image(HardwareRevision::DmgCpuC));
    assert!(assets.has_image(HardwareRevision::CpuMgb));
    assert!(assets.has_image(HardwareRevision::CpuCgbC));
    assert!(assets.has_image(HardwareRevision::CpuAgbA));
    assert!(assets.has_asset(BootRomAssetKind::Sgb));
    assert!(assets.has_asset(BootRomAssetKind::Sgb2));
    assert_eq!(
        assets.read_byte(HardwareRevision::DmgCpu0, 0x0000),
        Some(0x10)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::DmgCpuC, 0x0000),
        Some(0x20)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuMgb, 0x0000),
        Some(0x30)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuCgbC, 0x0000),
        Some(0x40)
    );
    assert_eq!(
        assets.read_byte(HardwareRevision::CpuAgbA, 0x0000),
        Some(0xA0)
    );
    assert_eq!(
        assets.read_asset_byte(BootRomAssetKind::Sgb, 0x0000),
        Some(0x51)
    );
    assert_eq!(
        assets.read_asset_byte(BootRomAssetKind::Sgb2, 0x0000),
        Some(0x52)
    );
}
