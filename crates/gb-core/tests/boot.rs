mod common;

use common::machine_driver::{step_machine_t_cycles, step_machine_until};
use gb_core::{
    BootController, BootRomAssetKind, BootRomAssets, CartridgeSlotState, ConsoleModel,
    CpuDiagnosticTrap, CpuExecutionState, HardwareRevision, Machine, MachineConfig, SgbHostProfile,
    StartupMemoryPolicy, StartupMode,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const BOOT_ROM_LEN: usize = 0x0100;
const CGB_BOOT_ROM_RAW_LEN: usize = 0x0800;
const ENTRY_POINT_START: usize = 0x0100;
const LOGO_START: usize = 0x0104;
const TITLE_START: usize = 0x0134;
const CGB_FLAG_ADDRESS: usize = 0x0143;
const SGB_FLAG_ADDRESS: usize = 0x0146;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
const ROM_SIZE_ADDRESS: usize = 0x0148;
const RAM_SIZE_ADDRESS: usize = 0x0149;
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;
const PHASE_2_REAL_BOOT_HANDOFF_T_CYCLE_LIMIT: usize = 256;
const PHASE_2_ENTRY_OPCODE: u8 = 0xD3;

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

fn build_cgb_boot_rom_image(low_byte: u8, upper_window_byte: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; CGB_BOOT_ROM_RAW_LEN];
    rom[0x0000] = low_byte;
    rom[0x0100] = upper_window_byte;
    rom
}

fn build_phase_2_boot_rom(expected_logo_byte: u8, expected_checksum: u8) -> Vec<u8> {
    let mut rom = vec![0x00; BOOT_ROM_LEN];
    let program = [
        0xFA,
        0x04,
        0x01,
        0xFE,
        expected_logo_byte,
        0x20,
        0xFE,
        0xFA,
        0x4D,
        0x01,
        0xFE,
        expected_checksum,
        0x20,
        0xFE,
        0x06,
        0x24,
        0x3E,
        0x42,
        0xC3,
        0xFD,
        0x00,
    ];

    rom[..program.len()].copy_from_slice(&program);
    rom[0x00FD..0x0100].copy_from_slice(&[0xEA, 0x50, 0xFF]);
    rom
}

fn build_phase_2_real_boot_rom(logo_byte: u8, header_checksum: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START] = PHASE_2_ENTRY_OPCODE;
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[LOGO_START] = logo_byte;
    rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"PHASE2.4");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[ROM_SIZE_ADDRESS] = 0x00;
    rom[RAM_SIZE_ADDRESS] = 0x00;
    rom[HEADER_CHECKSUM_ADDRESS] = header_checksum;
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

#[path = "boot/boot_cgb_windows.rs"]
mod boot_cgb_windows;
#[path = "boot/boot_diagnostics.rs"]
mod boot_diagnostics;
#[path = "boot/boot_real_handoff.rs"]
mod boot_real_handoff;
#[path = "boot/boot_skip_startup.rs"]
mod boot_skip_startup;
