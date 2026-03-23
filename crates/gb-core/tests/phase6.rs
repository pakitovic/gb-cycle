mod common;

use std::env;
use std::fs;
use std::path::Path;

use common::synthetic_cartridge::{BankedCartridgeBuilder, ProgramBuilder};
use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode};

const FIXTURE_ACCEPT_ENV: &str = "GB_CYCLE_ACCEPT_PHASE6_FIXTURES";
const MBC1_STANDARD_ROM_NAME: &str = "phase6_mbc1_standard_banking.gb";
const MBC1_STANDARD_TRACE_NAME: &str = "phase6_mbc1_standard_banking.trace";
const MBC1_SMALL_ROM_NAME: &str = "phase6_mbc1_small_rom_mask_and_ram.gb";
const MBC1_SMALL_TRACE_NAME: &str = "phase6_mbc1_small_rom_mask_and_ram.trace";
const MBC2_ROM_NAME: &str = "phase6_mbc2_control_decode_and_nibble_ram.gb";
const MBC2_TRACE_NAME: &str = "phase6_mbc2_control_decode_and_nibble_ram.trace";
const MBC3_ROM_NAME: &str = "phase6_mbc3_banking_ram_and_rtc.gb";
const MBC3_TRACE_NAME: &str = "phase6_mbc3_banking_ram_and_rtc.trace";
const MBC5_ROM_NAME: &str = "phase6_mbc5_rom_banking_rumble_and_ram.gb";
const MBC5_TRACE_NAME: &str = "phase6_mbc5_rom_banking_rumble_and_ram.trace";
const STANDARD_SENTINEL_ADDRESS: u16 = 0xC10F;
const SMALL_SENTINEL_ADDRESS: u16 = 0xC11F;
const MBC2_SENTINEL_ADDRESS: u16 = 0xC12F;
const MBC3_SENTINEL_ADDRESS: u16 = 0xC13F;
const MBC5_SENTINEL_ADDRESS: u16 = 0xC14F;
const SENTINEL_VALUE: u8 = 0xA5;

fn fixture_accept_writes_enabled() -> bool {
    env::var_os(FIXTURE_ACCEPT_ENV).is_some()
}

fn ensure_binary_fixture(path: &Path, expected: &[u8]) -> Vec<u8> {
    if fixture_accept_writes_enabled() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directory should be creatable");
        }
        fs::write(path, expected).expect("binary fixture should be writable");
    }

    let fixture = common::read_binary_fixture(path).expect("binary fixture should be readable");
    assert_eq!(fixture, expected);
    fixture
}

fn ensure_text_fixture(path: &Path, expected: &str) -> String {
    if fixture_accept_writes_enabled() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directory should be creatable");
        }
        fs::write(path, expected).expect("text fixture should be writable");
    }

    let fixture = common::read_text_fixture(path).expect("text fixture should be readable");
    assert_eq!(fixture, expected);
    fixture
}

fn load_fixture_machine(rom_name: &str, expected_rom: &[u8]) -> Machine {
    let rom_fixture_path = common::rom_fixtures_dir().join("phase6").join(rom_name);
    let rom_fixture = ensure_binary_fixture(&rom_fixture_path, expected_rom);
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom_fixture)
        .expect("synthetic cartridge test ROM should load");
    machine
}

fn assert_trace_fixture(trace_name: &str, trace: &str) {
    let trace_fixture_path = common::trace_fixtures_dir().join("phase6").join(trace_name);
    ensure_text_fixture(&trace_fixture_path, trace);
}

fn step_until_wram_sentinel(machine: &mut Machine, address: u16) {
    for _ in 0..1000 {
        if machine.read_bus(address) == SENTINEL_VALUE {
            return;
        }
        machine.step_t_cycle();
    }

    panic!("sentinel at {address:#06X} was not reached within 1000 T-cycles");
}

fn build_standard_mbc1_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC100);

    program.ld_a_imm(0x1F);
    program.ld_a16_from_a(0x2000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC101);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);
    program.ld_a_imm(0x11);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x22);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC102);

    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC103);

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(STANDARD_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_small_mask_mbc1_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC110);

    program.ld_a_imm(0x04);
    program.ld_a16_from_a(0x2000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC111);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);
    program.ld_a_imm(0x33);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_imm(0x03);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x44);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC112);

    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC113);

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(SMALL_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_mbc2_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC120);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x0100);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC121);

    program.ld_a_imm(0x03);
    program.ld_a16_from_a(0x2100);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC122);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);
    program.ld_a_imm(0xAB);
    program.ld_a16_from_a(0xA000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC123);
    program.ld_a_from_a16(0xA200);
    program.ld_a16_from_a(0xC124);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x0000);
    program.ld_a_imm(0x0C);
    program.ld_a16_from_a(0xA000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC125);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC126);

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(MBC2_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_mbc3_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC130);

    for (bank, destination) in [(0x20, 0xC131), (0x40, 0xC132), (0x60, 0xC133)] {
        program.ld_a_imm(bank);
        program.ld_a16_from_a(0x2000);
        program.ld_a_from_a16(0x4000);
        program.ld_a16_from_a(destination);
    }

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x33);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x55);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC134);

    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC135);

    program.ld_a_imm(0x08);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x6000);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC136);

    program.ld_a_imm(0x09);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC137);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC138);

    program.ld_a_imm(0x0B);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC139);

    program.ld_a_imm(0x08);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x2A);
    program.ld_a16_from_a(0xA000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC13A);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x6000);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC13B);

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(MBC3_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_mbc5_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC140);
    program.ld_a_from_a16(0x4001);
    program.ld_a16_from_a(0xC141);

    program.ld_a_imm(0xFF);
    program.ld_a16_from_a(0x2000);
    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x3000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC142);
    program.ld_a_from_a16(0x4001);
    program.ld_a16_from_a(0xC143);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x2000);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x3000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC144);
    program.ld_a_from_a16(0x4001);
    program.ld_a16_from_a(0xC145);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x11);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x03);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x33);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x0B);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC146);

    program.ld_a_imm(0x08);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC147);

    program.ld_a_imm(0x0B);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC148);

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(MBC5_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_standard_mbc1_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x04, 0x03, 0x03)
        .stamp_bank_start_markers()
        .write_program(&build_standard_mbc1_program())
        .build()
}

fn build_small_mask_mbc1_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x01, 0x03, 0x03)
        .stamp_bank_start_markers()
        .write_program(&build_small_mask_mbc1_program())
        .build()
}

fn build_mbc2_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x03, 0x06, 0x00)
        .stamp_bank_start_markers()
        .write_program(&build_mbc2_program())
        .build()
}

fn build_mbc3_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x06, 0x10, 0x03)
        .stamp_bank_start_markers()
        .write_program(&build_mbc3_program())
        .build()
}

fn build_mbc5_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x08, 0x1E, 0x03)
        .stamp_bank_identity_markers()
        .write_program(&build_mbc5_program())
        .build()
}

#[test]
fn phase_6_mbc1_standard_banking_rom_fixture_matches_expected_state_and_trace() {
    let expected_rom = build_standard_mbc1_rom();
    let mut machine = load_fixture_machine(MBC1_STANDARD_ROM_NAME, &expected_rom);

    step_until_wram_sentinel(&mut machine, STANDARD_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC100), 0x01);
    assert_eq!(machine.read_bus(0xC101), 0x1F);
    assert_eq!(machine.read_bus(0xC102), 0x11);
    assert_eq!(machine.read_bus(0xC103), 0x22);

    assert_trace_fixture(
        MBC1_STANDARD_TRACE_NAME,
        &machine.tracer().sink().render_text(),
    );
}

#[test]
fn phase_6_mbc1_small_rom_mask_and_ram_fixture_matches_expected_state_and_trace() {
    let expected_rom = build_small_mask_mbc1_rom();
    let mut machine = load_fixture_machine(MBC1_SMALL_ROM_NAME, &expected_rom);

    step_until_wram_sentinel(&mut machine, SMALL_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC110), 0x01);
    assert_eq!(machine.read_bus(0xC111), 0x00);
    assert_eq!(machine.read_bus(0xC112), 0x33);
    assert_eq!(machine.read_bus(0xC113), 0x44);

    assert_trace_fixture(
        MBC1_SMALL_TRACE_NAME,
        &machine.tracer().sink().render_text(),
    );
}

#[test]
fn phase_6_mbc2_control_decode_and_nibble_ram_fixture_matches_expected_state_and_trace() {
    let expected_rom = build_mbc2_rom();
    let mut machine = load_fixture_machine(MBC2_ROM_NAME, &expected_rom);

    step_until_wram_sentinel(&mut machine, MBC2_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC120), 0x01);
    assert_eq!(machine.read_bus(0xC121), 0x01);
    assert_eq!(machine.read_bus(0xC122), 0x03);
    assert_eq!(machine.read_bus(0xC123), 0xFB);
    assert_eq!(machine.read_bus(0xC124), 0xFB);
    assert_eq!(machine.read_bus(0xC125), 0xFF);
    assert_eq!(machine.read_bus(0xC126), 0xFB);

    assert_trace_fixture(MBC2_TRACE_NAME, &machine.tracer().sink().render_text());
}

#[test]
fn phase_6_mbc3_banking_ram_and_rtc_fixture_matches_expected_state_and_trace() {
    let expected_rom = build_mbc3_rom();
    let mut machine = load_fixture_machine(MBC3_ROM_NAME, &expected_rom);
    machine.advance_cartridge_rtc_seconds(93_784);

    step_until_wram_sentinel(&mut machine, MBC3_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC130), 0x01);
    assert_eq!(machine.read_bus(0xC131), 0x20);
    assert_eq!(machine.read_bus(0xC132), 0x40);
    assert_eq!(machine.read_bus(0xC133), 0x60);
    assert_eq!(machine.read_bus(0xC134), 0x33);
    assert_eq!(machine.read_bus(0xC135), 0x55);
    assert_eq!(machine.read_bus(0xC136), 0x04);
    assert_eq!(machine.read_bus(0xC137), 0x03);
    assert_eq!(machine.read_bus(0xC138), 0x02);
    assert_eq!(machine.read_bus(0xC139), 0x01);
    assert_eq!(machine.read_bus(0xC13A), 0x04);
    assert_eq!(machine.read_bus(0xC13B), 0x2A);

    assert_trace_fixture(MBC3_TRACE_NAME, &machine.tracer().sink().render_text());
}

#[test]
fn phase_6_mbc5_rom_banking_rumble_and_ram_fixture_matches_expected_state_and_trace() {
    let expected_rom = build_mbc5_rom();
    let mut machine = load_fixture_machine(MBC5_ROM_NAME, &expected_rom);

    step_until_wram_sentinel(&mut machine, MBC5_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC140), 0x01);
    assert_eq!(machine.read_bus(0xC141), 0x00);
    assert_eq!(machine.read_bus(0xC142), 0xFF);
    assert_eq!(machine.read_bus(0xC143), 0x00);
    assert_eq!(machine.read_bus(0xC144), 0x00);
    assert_eq!(machine.read_bus(0xC145), 0x01);
    assert_eq!(machine.read_bus(0xC146), 0x33);
    assert_eq!(machine.read_bus(0xC147), 0x11);
    assert_eq!(machine.read_bus(0xC148), 0x33);
    assert!(machine.cartridge().rumble_on());

    assert_trace_fixture(MBC5_TRACE_NAME, &machine.tracer().sink().render_text());
}
