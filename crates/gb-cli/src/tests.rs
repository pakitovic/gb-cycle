use crate::boot_rom::*;
use crate::command::help::*;
use crate::command::parse::*;
use crate::command::*;
use crate::framebuffer::*;
use crate::host_io::*;
use crate::inspect_rom::*;
use crate::options::*;
use crate::report::*;
use crate::run::benchmark::*;
use crate::run::budget::*;
use crate::run::machine::*;
use crate::run::run_command;
use crate::run::save_session::*;
use crate::run::state::*;
use crate::save_key::*;
use crate::saves::*;
use gb_benchmark::*;
use gb_core::*;
use gb_persistence::*;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn unique_temp_dir(label: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cli-{label}-{}-{}",
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

fn build_lcd_off_loop_rom() -> Vec<u8> {
    build_test_rom_with_header(
        &[
            0x3E, 0x00, // LD A,$00
            0xE0, 0x40, // LDH (LCDC),A
            0xC3, 0x04, 0x01, // JP $0104
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

fn build_loaded_machine(rom: Vec<u8>, capture_trace: bool) -> CliMachine {
    let config = MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot);
    let mut machine = CliMachine::new(config, capture_trace);
    machine
        .load_cartridge(rom)
        .expect("test ROM should load into the machine");
    machine
}

fn write_fake_boot_rom(dir: &Path, revision: HardwareRevision, fill: u8) {
    write_fake_boot_rom_asset(dir, BootRomAssetKind::from_revision(revision), fill);
}

fn write_fake_boot_rom_asset(dir: &Path, asset: BootRomAssetKind, fill: u8) {
    fs::create_dir_all(dir).expect("boot ROM directory should be creatable");
    fs::write(
        dir.join(asset.filename()),
        vec![fill; asset.expected_size()],
    )
    .expect("boot ROM image should be writable");
}

#[derive(Default)]
struct FailOnWrite {
    fail_on_write: Option<usize>,
    fail_on_flush: bool,
    writes: usize,
}

struct CurrentDirGuard {
    original: PathBuf,
}

impl CurrentDirGuard {
    fn enter(path: &Path) -> Self {
        let original = env::current_dir().expect("current directory should be readable");
        env::set_current_dir(path).expect("test current directory should be selectable");
        Self { original }
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        env::set_current_dir(&self.original).expect("original current directory should restore");
    }
}

fn decode_png_info(encoded: &[u8]) -> png::OutputInfo {
    let decoder = png::Decoder::new(std::io::Cursor::new(encoded));
    let mut reader = decoder.read_info().expect("PNG should decode");
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .expect("PNG decoder should expose an output buffer size")
    ];
    reader
        .next_frame(&mut buffer)
        .expect("PNG frame should decode")
}

impl Write for FailOnWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.fail_on_write == Some(self.writes) {
            Err(io::Error::other("synthetic write failure"))
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_on_flush {
            Err(io::Error::other("synthetic flush failure"))
        } else {
            Ok(())
        }
    }
}

mod boot_rom;
mod framebuffer;
mod host_utilities;
mod inspect_rom;
mod parse;
mod run;
mod saves;
