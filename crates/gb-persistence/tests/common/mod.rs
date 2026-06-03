#![allow(dead_code)]
// Shared helpers are compiled into several integration-test crates; each crate uses a subset.

use gb_core::{CartridgeSlot, CompatibilityPolicy, Huc3RtcPersistentState, PersistentCartState};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{env, fs};

pub(crate) const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
pub(crate) const ENTRY_POINT_START: usize = 0x0100;
pub(crate) const LOGO_START: usize = 0x0104;
pub(crate) const TITLE_START: usize = 0x0134;
pub(crate) const CGB_FLAG_ADDRESS: usize = 0x0143;
pub(crate) const SGB_FLAG_ADDRESS: usize = 0x0146;
pub(crate) const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
pub(crate) const ROM_SIZE_ADDRESS: usize = 0x0148;
pub(crate) const RAM_SIZE_ADDRESS: usize = 0x0149;
pub(crate) const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn build_test_rom(
    len: usize,
    cartridge_type: u8,
    rom_size_code: u8,
    ram_size_code: u8,
) -> Vec<u8> {
    let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"PERSIST!");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[ROM_SIZE_ADDRESS] = rom_size_code;
    rom[RAM_SIZE_ADDRESS] = ram_size_code;
    rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
    rom
}

pub(crate) fn build_banked_mbc1_rom(
    cartridge_type: u8,
    rom_size_code: u8,
    ram_size_code: u8,
) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        _ => panic!("unsupported MBC1 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

pub(crate) fn build_mmm01_rom(rom_size_code: u8, ram_size_code: u8, cartridge_type: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        0x07 => 4 * 1024 * 1024,
        0x08 => 8 * 1024 * 1024,
        _ => panic!("unsupported MMM01 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = vec![0xFF; rom_size.max(HEADER_MINIMUM_ROM_LEN)];

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"GAMEONE");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[ROM_SIZE_ADDRESS] = 0x00;
    rom[RAM_SIZE_ADDRESS] = 0x00;
    rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;

    let menu_offset = rom_size - 32 * 1024;
    let secondary_header_offset = (menu_offset / 2 / 0x4000) * 0x4000;
    rom[secondary_header_offset] = (secondary_header_offset / 0x4000) as u8;
    rom[secondary_header_offset + ENTRY_POINT_START
        ..secondary_header_offset + ENTRY_POINT_START + 4]
        .copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[secondary_header_offset + LOGO_START..secondary_header_offset + LOGO_START + 48]
        .copy_from_slice(&[0xCE; 48]);
    rom[secondary_header_offset + TITLE_START..secondary_header_offset + TITLE_START + 16]
        .fill(0x00);
    rom[secondary_header_offset + TITLE_START..secondary_header_offset + TITLE_START + 7]
        .copy_from_slice(b"GAMETWO");
    rom[secondary_header_offset + CGB_FLAG_ADDRESS] = 0x80;
    rom[secondary_header_offset + SGB_FLAG_ADDRESS] = 0x03;
    rom[secondary_header_offset + CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[secondary_header_offset + ROM_SIZE_ADDRESS] = 0x00;
    rom[secondary_header_offset + RAM_SIZE_ADDRESS] = 0x00;
    rom[secondary_header_offset + HEADER_CHECKSUM_ADDRESS] = 0x7F;

    rom[menu_offset] = ((bank_count - 2) & 0xFF) as u8;
    rom[menu_offset + ENTRY_POINT_START..menu_offset + ENTRY_POINT_START + 4]
        .copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[menu_offset + LOGO_START..menu_offset + LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[menu_offset + TITLE_START..menu_offset + TITLE_START + 7].copy_from_slice(b"MMM01!!");
    rom[menu_offset + CGB_FLAG_ADDRESS] = 0x80;
    rom[menu_offset + SGB_FLAG_ADDRESS] = 0x03;
    rom[menu_offset + CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[menu_offset + ROM_SIZE_ADDRESS] = rom_size_code;
    rom[menu_offset + RAM_SIZE_ADDRESS] = ram_size_code;
    rom[menu_offset + HEADER_CHECKSUM_ADDRESS] = 0x7F;

    rom
}

pub(crate) fn build_banked_huc1_rom(rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        _ => panic!("unsupported HuC1 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, 0xFF, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

pub(crate) fn build_banked_huc3_rom(rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        _ => panic!("unsupported HuC-3 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, 0xFE, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

pub(crate) fn build_mbc7_rom() -> Vec<u8> {
    build_test_rom(128 * 1024, 0x22, 0x02, 0x00)
}

pub(crate) fn build_banked_mbc2_rom(
    cartridge_type: u8,
    rom_size_code: u8,
    ram_size_code: u8,
) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        _ => panic!("unsupported MBC2 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

pub(crate) fn build_banked_mbc3_rom(
    cartridge_type: u8,
    rom_size_code: u8,
    ram_size_code: u8,
) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        _ => panic!("unsupported MBC3 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

pub(crate) fn temp_save_root() -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "gb-cycle-persistence-tests-{}-{id}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale temp save root should be removable");
    }
    fs::create_dir_all(&root).expect("temp save root should be creatable");
    root
}

pub(crate) fn load_cartridge(rom: Vec<u8>) -> CartridgeSlot {
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect("test cartridge should load");
    let (cartridge, _) = report.into_parts();
    cartridge
}

pub(crate) fn huc3_persistent_state(ram: Vec<u8>) -> PersistentCartState {
    PersistentCartState::Huc3 {
        ram,
        mcu_ram: [0; 256],
        rtc: Huc3RtcPersistentState {
            current_minutes_of_day: 0,
            current_days: 0,
            current_subminute_seconds: 0,
            event_minutes_of_day: 0,
            event_days: 0,
        },
        rom_bank: 0,
        ram_bank: 0,
        select_mode: 0,
        access_address: 0,
        mailbox_command: 0,
        mailbox_argument: 0,
        last_response_nybble: 0,
        semaphore_ready: true,
        ir_emitter_on: false,
        ir_light_detected: false,
        last_control_write: None,
        last_unsupported_command: None,
        last_unsupported_argument: None,
    }
}
