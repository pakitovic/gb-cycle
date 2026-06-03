use super::*;
use gb_core::{ConsoleModel, MachineConfig};
use gb_persistence::{CartridgeSaveBackend, FilesystemCartridgeSaveBackend};
use std::env;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const ENTRY_POINT_START: usize = 0x0100;
const LOGO_START: usize = 0x0104;
const TITLE_START: usize = 0x0134;
const CGB_FLAG_ADDRESS: usize = 0x0143;
const SGB_FLAG_ADDRESS: usize = 0x0146;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
const ROM_SIZE_ADDRESS: usize = 0x0148;
const RAM_SIZE_ADDRESS: usize = 0x0149;
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn load_machine(rom: Vec<u8>) -> Machine<TraceSummaryBuffer> {
    let mut machine = Machine::new_summary(MachineConfig::new(ConsoleModel::GameBoy));
    machine
        .load_cartridge(rom)
        .expect("test cartridge should load");
    machine
}

fn mutate_mbc2_persistent_state(machine: &mut Machine<TraceSummaryBuffer>, value: u8) {
    let mut state = machine.cartridge().persistent_state();
    assert!(matches!(state, PersistentCartState::Mbc2Ram { .. }));
    if let PersistentCartState::Mbc2Ram { ram_nibbles } = &mut state {
        ram_nibbles[0] = value & 0x0F;
    }
    machine
        .restore_cartridge_persistent_state(&state)
        .expect("restoring test persistent state should succeed");
}

fn temp_save_root() -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "gb-cycle-desktop-save-session-tests-{}-{id}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale temp save root should be removable");
    }
    fs::create_dir_all(&root).expect("temp save root should be creatable");
    root
}

fn build_test_rom(len: usize, cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"DESKTOP!");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[ROM_SIZE_ADDRESS] = rom_size_code;
    rom[RAM_SIZE_ADDRESS] = ram_size_code;
    rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
    rom
}

fn build_banked_mbc2_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
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

#[path = "test/dirty.rs"]
mod dirty;
#[path = "test/flush_policy.rs"]
mod flush_policy;
#[path = "test/helpers.rs"]
mod helpers;
#[path = "test/manual_and_rtc.rs"]
mod manual_and_rtc;
#[path = "test/open.rs"]
mod open;
