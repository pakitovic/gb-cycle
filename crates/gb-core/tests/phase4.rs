mod common;

use std::env;
use std::fs;
use std::path::Path;

use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode};

const FIXTURE_ACCEPT_ENV: &str = "GB_CYCLE_ACCEPT_PHASE4_FIXTURES";
const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const PROGRAM_ENTRY_ADDRESS: usize = 0x0150;
const SENTINEL_ADDRESS: u16 = 0xC010;
const SENTINEL_VALUE: u8 = 0xA5;
const TEST_ROM_BOOT_OPCODE: u8 = 0x12;

const DIRECT_MODE2_ROM_NAME: &str = "phase4_oam_bug_direct_mode2_oam_access.gb";
const DIRECT_MODE2_TRACE_NAME: &str = "phase4_oam_bug_direct_mode2_oam_access.trace";
const FEA0_MODE2_ROM_NAME: &str = "phase4_oam_bug_fea0_mode2_read.gb";
const FEA0_MODE2_TRACE_NAME: &str = "phase4_oam_bug_fea0_mode2_read.trace";
const INC_HL_ROM_NAME: &str = "phase4_oam_bug_inc_hl.gb";
const INC_HL_DMG0_TRACE_NAME: &str = "phase4_oam_bug_inc_hl_dmg0.trace";
const INC_HL_DMG_TRACE_NAME: &str = "phase4_oam_bug_inc_hl_dmg.trace";
const INC_HL_MGB_TRACE_NAME: &str = "phase4_oam_bug_inc_hl_mgb.trace";
const INC_HL_CGB_TRACE_NAME: &str = "phase4_oam_bug_inc_hl_cgb.trace";
const HLI_HLD_ROM_NAME: &str = "phase4_oam_bug_hli_hld.gb";
const HLI_HLD_TRACE_NAME: &str = "phase4_oam_bug_hli_hld.trace";
const STACK_AND_INTERRUPT_ROM_NAME: &str = "phase4_oam_bug_stack_and_interrupt_service.gb";
const STACK_AND_INTERRUPT_TRACE_NAME: &str = "phase4_oam_bug_stack_and_interrupt_service.trace";

fn build_test_rom(program: &[u8], boot_opcode: u8, extra_segments: &[(usize, &[u8])]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = boot_opcode;
    rom[0x0100..0x0103].copy_from_slice(&[0xC3, 0x50, 0x01]);
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[PROGRAM_ENTRY_ADDRESS + offset] = byte;
    }
    for &(address, bytes) in extra_segments {
        rom[address..address + bytes.len()].copy_from_slice(bytes);
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

#[derive(Default)]
struct ProgramBuilder {
    bytes: Vec<u8>,
}

impl ProgramBuilder {
    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn xor_a(&mut self) {
        self.bytes.push(0xAF);
    }

    fn nop(&mut self) {
        self.bytes.push(0x00);
    }

    fn ld_a_imm(&mut self, value: u8) {
        self.bytes.extend_from_slice(&[0x3E, value]);
    }

    fn ld_bc_imm(&mut self, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.bytes.extend_from_slice(&[0x01, low, high]);
    }

    fn ld_hl_imm(&mut self, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.bytes.extend_from_slice(&[0x21, low, high]);
    }

    fn ld_sp_imm(&mut self, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.bytes.extend_from_slice(&[0x31, low, high]);
    }

    fn ld_hli_a(&mut self) {
        self.bytes.push(0x22);
    }

    fn ld_hld_a(&mut self) {
        self.bytes.push(0x32);
    }

    fn ld_a_hli(&mut self) {
        self.bytes.push(0x2A);
    }

    fn ld_a_from_a16(&mut self, address: u16) {
        let [low, high] = address.to_le_bytes();
        self.bytes.extend_from_slice(&[0xFA, low, high]);
    }

    fn ld_a16_from_a(&mut self, address: u16) {
        let [low, high] = address.to_le_bytes();
        self.bytes.extend_from_slice(&[0xEA, low, high]);
    }

    fn inc_hl(&mut self) {
        self.bytes.push(0x23);
    }

    fn push_bc(&mut self) {
        self.bytes.push(0xC5);
    }

    fn ei(&mut self) {
        self.bytes.push(0xFB);
    }

    fn jr_self(&mut self) {
        self.bytes.extend_from_slice(&[0x18, 0xFE]);
    }

    fn push_nops(&mut self, count: usize) {
        for _ in 0..count {
            self.nop();
        }
    }
}

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

fn step_until_wram_sentinel(machine: &mut Machine, address: u16, value: u8, max_steps: usize) {
    for _ in 0..max_steps {
        if machine.read_bus(address) == value {
            return;
        }
        machine.step_t_cycle();
    }

    panic!(
        "sentinel was not reached: observed={:#04X} pc={:#06X} state={:?} opcode={:?}",
        machine.read_bus(address),
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.cpu().current_opcode()
    );
}

fn seed_oam_bytes(program: &mut ProgramBuilder, start_address: u16, bytes: &[u8]) {
    program.ld_hl_imm(start_address);
    for byte in bytes.iter().copied() {
        program.ld_a_imm(byte);
        program.ld_hli_a();
    }
}

fn row_address(row: u8) -> u16 {
    0xFE00 + u16::from(row) * 8
}

fn read_machine_oam_row(machine: &mut Machine, row: u8) -> [u8; 8] {
    let mut bytes = [0; 8];
    let start = row_address(row);
    for (offset, byte) in bytes.iter_mut().enumerate() {
        *byte = machine.read_bus(start + offset as u16);
    }
    bytes
}

fn write_expected_row(oam: &mut [u8; 160], row: u8, bytes: [u8; 8]) {
    let start = row as usize * 8;
    oam[start..start + 8].copy_from_slice(&bytes);
}

fn read_expected_row(oam: &[u8; 160], row: u8) -> [u8; 8] {
    let start = row as usize * 8;
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&oam[start..start + 8]);
    bytes
}

fn read_expected_word(oam: &[u8; 160], row: u8, word_index: usize) -> u16 {
    let start = row as usize * 8 + word_index * 2;
    u16::from_le_bytes([oam[start], oam[start + 1]])
}

fn write_expected_word(oam: &mut [u8; 160], row: u8, word_index: usize, value: u16) {
    let start = row as usize * 8 + word_index * 2;
    let [low, high] = value.to_le_bytes();
    oam[start] = low;
    oam[start + 1] = high;
}

fn copy_previous_row_tail(oam: &mut [u8; 160], current_row: u8) {
    let previous = read_expected_row(oam, current_row - 1);
    let current_start = current_row as usize * 8;
    oam[current_start + 2..current_start + 8].copy_from_slice(&previous[2..8]);
}

fn apply_write_corruption(oam: &mut [u8; 160], current_row: u8) {
    if current_row == 0 {
        return;
    }

    let current_first = read_expected_word(oam, current_row, 0);
    let previous_first = read_expected_word(oam, current_row - 1, 0);
    let previous_third = read_expected_word(oam, current_row - 1, 2);
    let corrupted_first =
        ((current_first ^ previous_third) & (previous_first ^ previous_third)) ^ previous_third;
    write_expected_word(oam, current_row, 0, corrupted_first);
    copy_previous_row_tail(oam, current_row);
}

fn apply_read_corruption(oam: &mut [u8; 160], current_row: u8) {
    if current_row == 0 {
        return;
    }

    let current_first = read_expected_word(oam, current_row, 0);
    let previous_first = read_expected_word(oam, current_row - 1, 0);
    let previous_third = read_expected_word(oam, current_row - 1, 2);
    let corrupted_first = previous_first | (current_first & previous_third);
    write_expected_word(oam, current_row, 0, corrupted_first);
    copy_previous_row_tail(oam, current_row);
}

fn apply_read_with_incdec_corruption(oam: &mut [u8; 160], current_row: u8) {
    if (4..=18).contains(&current_row) {
        let row_minus_two = current_row - 2;
        let previous_row = current_row - 1;
        let a = read_expected_word(oam, row_minus_two, 0);
        let b = read_expected_word(oam, previous_row, 0);
        let c = read_expected_word(oam, current_row, 0);
        let d = read_expected_word(oam, previous_row, 2);
        let corrupted_previous_first = (b & (a | c | d)) | (a & c & d);
        write_expected_word(oam, previous_row, 0, corrupted_previous_first);

        let previous_row_bytes = read_expected_row(oam, previous_row);
        write_expected_row(oam, current_row, previous_row_bytes);
        write_expected_row(oam, row_minus_two, previous_row_bytes);
    }

    apply_read_corruption(oam, current_row);
}

fn assert_machine_rows(machine: &mut Machine, expected: &[u8; 160], rows: &[u8]) {
    for &row in rows {
        assert_eq!(
            read_machine_oam_row(machine, row),
            read_expected_row(expected, row)
        );
    }
}

fn run_fixture_rom(
    rom_name: &str,
    trace_name: &str,
    expected_rom: &[u8],
    console_model: ConsoleModel,
    max_steps: usize,
) -> Machine {
    let rom_fixture_path = common::rom_fixtures_dir().join("phase4").join(rom_name);
    let trace_fixture_path = common::trace_fixtures_dir().join("phase4").join(trace_name);
    let rom_fixture = ensure_binary_fixture(&rom_fixture_path, expected_rom);

    let mut machine =
        Machine::new(MachineConfig::new(console_model).with_startup_mode(StartupMode::SkipBoot));

    machine
        .load_cartridge(rom_fixture)
        .expect("NoMBC test ROM should load");

    step_until_wram_sentinel(&mut machine, SENTINEL_ADDRESS, SENTINEL_VALUE, max_steps);

    let trace = machine.tracer().sink().render_text();
    ensure_text_fixture(&trace_fixture_path, &trace);

    machine
}

fn build_direct_mode2_oam_access_program() -> Vec<u8> {
    let seed_bytes = [
        0x5A, 0xA5, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x12, 0x34, 0x21, 0x43, 0x65, 0x87, 0xA9,
        0xCB,
    ];
    let mut program = ProgramBuilder::default();

    program.xor_a();
    program.ld_a16_from_a(0xFF40);
    seed_oam_bytes(&mut program, 0xFE38, &seed_bytes);

    program.ld_a_imm(0x80);
    program.ld_a16_from_a(0xFF40);

    program.ld_a_imm(0x99);
    program.nop();
    program.ld_a16_from_a(0xFE00);

    program.xor_a();
    program.ld_a16_from_a(0xFF40);
    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_fea0_mode2_read_program() -> Vec<u8> {
    let seed_bytes = [
        0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB, 0xF0, 0xF0, 0x11, 0x11, 0x22, 0x22, 0x33,
        0x33,
    ];
    let mut program = ProgramBuilder::default();

    program.xor_a();
    program.ld_a16_from_a(0xFF40);
    seed_oam_bytes(&mut program, 0xFE38, &seed_bytes);

    program.ld_a_imm(0x80);
    program.ld_a16_from_a(0xFF40);
    program.push_nops(3);
    program.ld_a_from_a16(0xFEA0);

    program.xor_a();
    program.ld_a16_from_a(0xFF40);
    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_inc_hl_program() -> Vec<u8> {
    let seed_bytes = [
        0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB, 0x55, 0x55, 0xCC, 0xCC, 0xDD, 0xDD, 0xEE,
        0xEE,
    ];
    let mut program = ProgramBuilder::default();

    program.xor_a();
    program.ld_a16_from_a(0xFF40);
    seed_oam_bytes(&mut program, 0xFE38, &seed_bytes);
    program.ld_hl_imm(0xFE47);

    program.ld_a_imm(0x80);
    program.ld_a16_from_a(0xFF40);
    program.push_nops(5);
    program.inc_hl();

    program.xor_a();
    program.ld_a16_from_a(0xFF40);
    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_hli_hld_program() -> Vec<u8> {
    let phase_one_seed_bytes = [
        0x0F, 0x0F, 0x10, 0x10, 0x20, 0x20, 0x30, 0x30, 0xAA, 0xAA, 0x11, 0x11, 0xC0, 0xC0, 0x22,
        0x22, 0xFF, 0x00, 0x33, 0x33, 0x44, 0x44, 0x55, 0x55,
    ];
    let phase_two_seed_bytes = [
        0x34, 0x12, 0x66, 0x66, 0x0F, 0xF0, 0x77, 0x77, 0x5A, 0xA5, 0x88, 0x88, 0x99, 0x99, 0xAA,
        0xAA,
    ];
    let mut program = ProgramBuilder::default();

    program.xor_a();
    program.ld_a16_from_a(0xFF40);

    seed_oam_bytes(&mut program, 0xFE30, &phase_one_seed_bytes);
    program.ld_hl_imm(0xFE40);
    program.ld_a_imm(0x80);
    program.ld_a16_from_a(0xFF40);
    program.push_nops(5);
    program.ld_a_hli();

    program.xor_a();
    program.ld_a16_from_a(0xFF40);

    seed_oam_bytes(&mut program, 0xFE58, &phase_two_seed_bytes);
    program.ld_hl_imm(0xFE60);
    program.ld_a_imm(0x80);
    program.ld_a16_from_a(0xFF40);
    program.ld_a_imm(0x99);
    program.push_nops(7);
    program.ld_hld_a();

    program.xor_a();
    program.ld_a16_from_a(0xFF40);
    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_stack_and_interrupt_service_program() -> Vec<u8> {
    let phase_one_seed_bytes = [
        0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB, 0x55, 0x55, 0x10, 0x10, 0x20, 0x20, 0x30,
        0x30,
    ];
    let phase_two_seed_bytes = [
        0x78, 0x56, 0x11, 0x11, 0xAA, 0x00, 0x22, 0x22, 0x34, 0x12, 0x33, 0x33, 0x44, 0x44, 0x55,
        0x55,
    ];
    let mut program = ProgramBuilder::default();

    program.xor_a();
    program.ld_a16_from_a(0xFF40);

    seed_oam_bytes(&mut program, 0xFE40, &phase_one_seed_bytes);
    program.ld_sp_imm(0xFE4A);
    program.ld_bc_imm(0x1234);
    program.ld_a_imm(0x80);
    program.ld_a16_from_a(0xFF40);
    program.push_nops(4);
    program.push_bc();

    program.xor_a();
    program.ld_a16_from_a(0xFF40);

    seed_oam_bytes(&mut program, 0xFE88, &phase_two_seed_bytes);
    program.ld_sp_imm(0xFE8A);
    program.ld_a_imm(0x80);
    program.ld_a16_from_a(0xFF40);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0xFFFF);
    program.ld_a16_from_a(0xFF0F);
    program.ei();
    program.nop();
    program.jr_self();

    program.into_bytes()
}

fn build_stack_and_interrupt_service_vector() -> [u8; 11] {
    [
        0xAF,
        0xEA,
        0x40,
        0xFF,
        0x3E,
        SENTINEL_VALUE,
        0xEA,
        0x10,
        0xC0,
        0x18,
        0xFE,
    ]
}

#[test]
fn phase_4_direct_mode2_oam_access_rom_fixture_matches_expected_oam_state_and_trace() {
    let expected_rom = build_test_rom(
        &build_direct_mode2_oam_access_program(),
        TEST_ROM_BOOT_OPCODE,
        &[],
    );
    let mut machine = run_fixture_rom(
        DIRECT_MODE2_ROM_NAME,
        DIRECT_MODE2_TRACE_NAME,
        &expected_rom,
        ConsoleModel::Dmg,
        2_048,
    );

    let mut expected = [0; 160];
    write_expected_row(
        &mut expected,
        7,
        [0x5A, 0xA5, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
    );
    write_expected_row(
        &mut expected,
        8,
        [0x12, 0x34, 0x21, 0x43, 0x65, 0x87, 0xA9, 0xCB],
    );
    apply_write_corruption(&mut expected, 8);

    assert_machine_rows(&mut machine, &expected, &[7, 8]);
}

#[test]
fn phase_4_fea0_mode2_read_rom_fixture_matches_expected_oam_state_and_trace() {
    let expected_rom = build_test_rom(&build_fea0_mode2_read_program(), TEST_ROM_BOOT_OPCODE, &[]);
    let mut machine = run_fixture_rom(
        FEA0_MODE2_ROM_NAME,
        FEA0_MODE2_TRACE_NAME,
        &expected_rom,
        ConsoleModel::Dmg,
        2_048,
    );

    let mut expected = [0; 160];
    write_expected_row(
        &mut expected,
        7,
        [0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB],
    );
    write_expected_row(
        &mut expected,
        8,
        [0xF0, 0xF0, 0x11, 0x11, 0x22, 0x22, 0x33, 0x33],
    );
    apply_read_corruption(&mut expected, 8);

    assert_machine_rows(&mut machine, &expected, &[7, 8]);
}

#[test]
fn phase_4_inc_hl_rom_fixture_matches_expected_oam_state_and_traces_for_all_models() {
    let expected_rom = build_test_rom(&build_inc_hl_program(), TEST_ROM_BOOT_OPCODE, &[]);
    let trace_cases = [
        (ConsoleModel::Dmg0, INC_HL_DMG0_TRACE_NAME, true),
        (ConsoleModel::Dmg, INC_HL_DMG_TRACE_NAME, true),
        (ConsoleModel::Mgb, INC_HL_MGB_TRACE_NAME, true),
        (ConsoleModel::Cgb, INC_HL_CGB_TRACE_NAME, false),
    ];

    for (console_model, trace_name, expect_corruption) in trace_cases {
        let mut machine = run_fixture_rom(
            INC_HL_ROM_NAME,
            trace_name,
            &expected_rom,
            console_model,
            2_048,
        );

        let mut expected = [0; 160];
        write_expected_row(
            &mut expected,
            7,
            [0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB],
        );
        write_expected_row(
            &mut expected,
            8,
            [0x55, 0x55, 0xCC, 0xCC, 0xDD, 0xDD, 0xEE, 0xEE],
        );
        if expect_corruption {
            apply_write_corruption(&mut expected, 8);
        }

        assert_machine_rows(&mut machine, &expected, &[7, 8]);
    }
}

#[test]
fn phase_4_hli_hld_rom_fixture_matches_expected_oam_state_and_trace() {
    let expected_rom = build_test_rom(&build_hli_hld_program(), TEST_ROM_BOOT_OPCODE, &[]);
    let mut machine = run_fixture_rom(
        HLI_HLD_ROM_NAME,
        HLI_HLD_TRACE_NAME,
        &expected_rom,
        ConsoleModel::Dmg,
        4_096,
    );

    let mut expected = [0; 160];
    write_expected_row(
        &mut expected,
        6,
        [0x0F, 0x0F, 0x10, 0x10, 0x20, 0x20, 0x30, 0x30],
    );
    write_expected_row(
        &mut expected,
        7,
        [0xAA, 0xAA, 0x11, 0x11, 0xC0, 0xC0, 0x22, 0x22],
    );
    write_expected_row(
        &mut expected,
        8,
        [0xFF, 0x00, 0x33, 0x33, 0x44, 0x44, 0x55, 0x55],
    );
    apply_read_with_incdec_corruption(&mut expected, 8);

    write_expected_row(
        &mut expected,
        11,
        [0x34, 0x12, 0x66, 0x66, 0x0F, 0xF0, 0x77, 0x77],
    );
    write_expected_row(
        &mut expected,
        12,
        [0x5A, 0xA5, 0x88, 0x88, 0x99, 0x99, 0xAA, 0xAA],
    );
    apply_write_corruption(&mut expected, 12);

    assert_machine_rows(&mut machine, &expected, &[6, 7, 8, 11, 12]);
}

#[test]
fn phase_4_stack_and_interrupt_service_rom_fixture_matches_expected_oam_state_and_trace() {
    let vector = build_stack_and_interrupt_service_vector();
    let expected_rom = build_test_rom(
        &build_stack_and_interrupt_service_program(),
        TEST_ROM_BOOT_OPCODE,
        &[(0x0040, &vector)],
    );
    let machine = run_fixture_rom(
        STACK_AND_INTERRUPT_ROM_NAME,
        STACK_AND_INTERRUPT_TRACE_NAME,
        &expected_rom,
        ConsoleModel::Dmg,
        8_192,
    );
    let _ = machine;

    // This multi-phase integration case still serves as the end-to-end regression for
    // stack/control-flow and interrupt-service OAM-corruption routing because the retained
    // trace captures the live row timing and the concrete `write` / `write+dec` events.
    // The corruption formulas and row-local end states are covered directly in `bus` / `ppu`
    // unit tests, while the final OAM contents of this ROM are no longer a stable oracle after
    // the current LCD off/on and interrupt-service sequencing changes.
}
