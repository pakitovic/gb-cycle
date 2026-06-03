use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use gb_core::{BootRomAssets, HardwareRevision, PersistentCartState};
use gb_persistence::{FilesystemCartridgeSaveStore, decode_machine_save_state_envelope};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn unique_temp_dir(label: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cli-integration-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn build_test_rom_with_header(
    program: &[u8],
    cartridge_type: u8,
    rom_size: u8,
    ram_size: u8,
) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = cartridge_type;
    rom[0x0148] = rom_size;
    rom[0x0149] = ram_size;
    rom
}

fn build_single_byte_serial_rom(byte: u8) -> Vec<u8> {
    build_test_rom_with_header(
        &[
            0x3E, byte, // LD A,d8
            0xE0, 0x01, // LDH (SB),A
            0x3E, 0x81, // LD A,$81
            0xE0, 0x02, // LDH (SC),A
            0xC3, 0x08, 0x01, // JP $0108
        ],
        0x00,
        0x00,
        0x00,
    )
}

fn build_nop_loop_rom() -> Vec<u8> {
    build_test_rom_with_header(
        &[
            0x00, // NOP
            0x00, // NOP
            0xC3, 0x00, 0x01, // JP $0100
        ],
        0x00,
        0x00,
        0x00,
    )
}

fn build_battery_backed_serial_and_ram_rom(byte: u8, ram_value: u8) -> Vec<u8> {
    build_test_rom_with_header(
        &[
            0x3E, ram_value, // LD A,d8
            0xEA, 0x00, 0xA0, // LD ($A000),A
            0x3E, byte, // LD A,d8
            0xE0, 0x01, // LDH (SB),A
            0x3E, 0x81, // LD A,$81
            0xE0, 0x02, // LDH (SC),A
            0xC3, 0x0D, 0x01, // JP $010D
        ],
        0x09,
        0x00,
        0x02,
    )
}

fn set_header_flags(rom: &mut [u8], cgb_flag: u8, sgb_flag: u8) {
    rom[0x0143] = cgb_flag;
    rom[0x0146] = sgb_flag;
}

fn write_fake_boot_rom(dir: &PathBuf, revision: HardwareRevision, fill: u8) {
    fs::create_dir_all(dir).expect("boot ROM dir should be creatable");
    fs::write(
        dir.join(BootRomAssets::filename(revision)),
        vec![fill; revision.boot_rom_expected_size()],
    )
    .expect("boot ROM file should be writable");
}

#[path = "main_cli/command.rs"]
mod command;
#[path = "main_cli/inspect_rom.rs"]
mod inspect_rom;
#[path = "main_cli/run.rs"]
mod run;
