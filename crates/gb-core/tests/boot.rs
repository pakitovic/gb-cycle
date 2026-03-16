use gb_core::{
    BootRomAssets, BootRomKind, CartridgeSlotState, ConsoleModel, Machine, MachineConfig,
    StartupMemoryPolicy, StartupMode,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const BOOT_ROM_LEN: usize = 0x0100;
const ENTRY_POINT_START: usize = 0x0100;
const LOGO_START: usize = 0x0104;
const TITLE_START: usize = 0x0134;
const CGB_FLAG_ADDRESS: usize = 0x0143;
const SGB_FLAG_ADDRESS: usize = 0x0146;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
const ROM_SIZE_ADDRESS: usize = 0x0148;
const RAM_SIZE_ADDRESS: usize = 0x0149;
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;

fn build_test_rom(header_checksum: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"PHASE1E!");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[ROM_SIZE_ADDRESS] = 0x00;
    rom[RAM_SIZE_ADDRESS] = 0x00;
    rom[HEADER_CHECKSUM_ADDRESS] = header_checksum;
    rom[0x3FFF] = 0x34;
    rom[0x4000] = 0x56;
    rom
}

fn build_boot_rom_image(first_byte: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; BOOT_ROM_LEN];
    rom[0x0000] = first_byte;
    rom
}

fn unique_temp_dir() -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cycle-boot-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

#[test]
fn skip_boot_uses_the_centralized_post_boot_entry_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.boot().boot_rom_kind(), BootRomKind::Dmg);
    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.cpu().startup_state().pc, 0x0100);
    assert_eq!(machine.cpu().startup_state().a, 0x01);
    assert_eq!(machine.cpu().startup_state().f, 0xB0);
    assert_eq!(
        machine.boot().startup_memory_policy(),
        StartupMemoryPolicy::DeterministicZeroed
    );

    assert_eq!(machine.read_bus(0xFF00), 0xCF);
    assert_eq!(machine.read_bus(0xFF01), 0x00);
    assert_eq!(machine.read_bus(0xFF02), 0x7E);
    assert_eq!(machine.read_bus(0xFF04), 0xAB);
    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF06), 0x00);
    assert_eq!(machine.read_bus(0xFF07), 0xF8);
    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(machine.read_bus(0xFF40), 0x91);
    assert_eq!(machine.read_bus(0xFF41), 0x85);
    assert_eq!(machine.read_bus(0xFF42), 0x00);
    assert_eq!(machine.read_bus(0xFF43), 0x00);
    assert_eq!(machine.read_bus(0xFF44), 0x00);
    assert_eq!(machine.read_bus(0xFF45), 0x00);
    assert_eq!(machine.read_bus(0xFF46), 0xFF);
    assert_eq!(machine.read_bus(0xFF47), 0xFC);
    assert_eq!(machine.read_bus(0xFF4A), 0x00);
    assert_eq!(machine.read_bus(0xFF4B), 0x00);
    assert_eq!(machine.read_bus(0xFFFF), 0x00);
    assert_eq!(machine.read_bus(0xC000), 0x00);
    assert_eq!(machine.read_bus(0xFF80), 0x00);
}

#[test]
fn real_boot_reads_boot_rom_at_0000_until_ff50_handoff_restores_cartridge_visibility() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(BootRomKind::Dmg, build_boot_rom_image(0x99))
                    .expect("configured DMG boot ROM should validate"),
            ),
    );

    machine
        .load_cartridge(build_test_rom(0x7F))
        .expect("supported NoMBC image should load");

    assert_eq!(machine.cartridge().state(), CartridgeSlotState::NoMbc);
    assert!(machine.boot().is_boot_rom_mapped());
    assert!(machine.boot().has_boot_rom_asset());

    let boot_byte = machine.boot().read_boot_rom(0x0000);
    assert_eq!(boot_byte, 0x99);
    assert_eq!(machine.read_bus(0x0000), boot_byte);
    assert_eq!(machine.read_bus(0x0100), 0x31);
    assert_eq!(machine.read_bus(0x4000), 0x56);

    machine.write_bus(0xFF50, 0x01);

    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.read_bus(0x0000), 0x12);
    assert_eq!(machine.read_bus(0x0100), 0x31);
    assert_eq!(machine.read_bus(0x4000), 0x56);
}

#[test]
fn real_boot_can_source_boot_rom_assets_from_a_directory() {
    let directory = unique_temp_dir();
    fs::create_dir_all(&directory).expect("temporary asset directory should be creatable");
    fs::write(
        directory.join(BootRomAssets::filename(BootRomKind::Dmg)),
        build_boot_rom_image(0x66),
    )
    .expect("boot ROM asset file should be writable");

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::from_directory(&directory)
                    .expect("directory-backed boot ROM assets should load"),
            ),
    );

    machine
        .load_cartridge(build_test_rom(0x7F))
        .expect("supported NoMBC image should load");

    assert!(machine.boot().has_boot_rom_asset());
    assert_eq!(machine.read_bus(0x0000), 0x66);

    fs::remove_dir_all(&directory).expect("temporary asset directory should be removable");
}

#[test]
fn skip_boot_recomputes_the_checksum_derived_f_register_when_a_cartridge_is_loaded() {
    let mut zero_checksum_machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut non_zero_checksum_machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    zero_checksum_machine
        .load_cartridge(build_test_rom(0x00))
        .expect("supported NoMBC image should load");
    non_zero_checksum_machine
        .load_cartridge(build_test_rom(0x7F))
        .expect("supported NoMBC image should load");

    assert_eq!(zero_checksum_machine.cpu().startup_state().f, 0x80);
    assert_eq!(non_zero_checksum_machine.cpu().startup_state().f, 0xB0);
}
