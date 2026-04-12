mod common;

use common::machine_driver::{step_machine_t_cycles, step_machine_until};
use common::synthetic_cartridge::{
    HEADER_MINIMUM_ROM_LEN, PROGRAM_ENTRY_ADDRESS, build_nom_bc_test_rom_with_program_entry,
};
use gb_core::{
    BootRomAssets, BootRomKind, ConsoleModel, CpuExecutionState, JoypadButton, Machine,
    MachineConfig, StartupMode,
};

const FIXTURE_ACCEPT_ENV: &str = common::fixture_env::PHASE2;
const BOOT_ROM_LEN: usize = 0x0100;
const TEST_ROM_BOOT_OPCODE: u8 = 0x12;
const PHASE_2_ENTRY_OPCODE: u8 = 0xD3;
const SENTINEL_ADDRESS: u16 = 0xC010;
const SENTINEL_VALUE: u8 = 0xA5;

const FETCH_IMMEDIATE_ROM_NAME: &str = "phase2_fetch_immediate_order.gb";
const FETCH_IMMEDIATE_TRACE_NAME: &str = "phase2_fetch_immediate_order.trace";
const CONTROL_FLOW_STACK_CB_ROM_NAME: &str = "phase2_control_flow_stack_cb.gb";
const CONTROL_FLOW_STACK_CB_TRACE_NAME: &str = "phase2_control_flow_stack_cb.trace";
const EI_DELAY_PRIORITY_ROM_NAME: &str = "phase2_ei_delay_priority.gb";
const EI_DELAY_PRIORITY_TRACE_NAME: &str = "phase2_ei_delay_priority.trace";
const HALT_STOP_AND_HALT_BUG_ROM_NAME: &str = "phase2_halt_stop_and_halt_bug.gb";
const HALT_STOP_AND_HALT_BUG_TRACE_NAME: &str = "phase2_halt_stop_and_halt_bug.trace";
const TIMER_IF_VISIBILITY_ROM_NAME: &str = "phase2_timer_if_visibility_and_service.gb";
const TIMER_IF_VISIBILITY_TRACE_NAME: &str = "phase2_timer_if_visibility_and_service.trace";

fn build_phase_2_trace_boot_rom() -> Vec<u8> {
    let mut rom = vec![0x00; BOOT_ROM_LEN];
    rom[0x0000..0x0003].copy_from_slice(&[0xC3, 0xFB, 0x00]);
    rom[0x00FB..0x00FD].copy_from_slice(&[0x3E, 0x01]);
    rom[0x00FD..0x0100].copy_from_slice(&[0xEA, 0x50, 0xFF]);
    rom
}

fn build_phase_2_real_boot_rom(entry_opcode: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = TEST_ROM_BOOT_OPCODE;
    rom[0x0100] = entry_opcode;
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn build_phase_2_fragment_rom(program: &[u8], boot_opcode: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = boot_opcode;
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
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

    fn current_address(&self) -> u16 {
        (PROGRAM_ENTRY_ADDRESS + self.bytes.len()) as u16
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

    fn ld_c_imm(&mut self, value: u8) {
        self.bytes.extend_from_slice(&[0x0E, value]);
    }

    fn ld_sp_imm(&mut self, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.bytes.extend_from_slice(&[0x31, low, high]);
    }

    fn ld_a_from_c(&mut self) {
        self.bytes.push(0x79);
    }

    fn add_a_imm(&mut self, value: u8) {
        self.bytes.extend_from_slice(&[0xC6, value]);
    }

    fn ld_a16_from_a(&mut self, address: u16) {
        let [low, high] = address.to_le_bytes();
        self.bytes.extend_from_slice(&[0xEA, low, high]);
    }

    fn ldh_a8_from_a(&mut self, offset: u8) {
        self.bytes.extend_from_slice(&[0xE0, offset]);
    }

    fn ld_ff00_plus_c_from_a(&mut self) {
        self.bytes.push(0xE2);
    }

    fn jr_offset(&mut self, offset: i8) {
        self.bytes.extend_from_slice(&[0x18, offset as u8]);
    }

    fn jr_self(&mut self) {
        self.jr_offset(-2);
    }

    fn call(&mut self, address: u16) {
        let [low, high] = address.to_le_bytes();
        self.bytes.extend_from_slice(&[0xCD, low, high]);
    }

    fn ret(&mut self) {
        self.bytes.push(0xC9);
    }

    fn cb(&mut self, opcode: u8) {
        self.bytes.extend_from_slice(&[0xCB, opcode]);
    }

    fn ei(&mut self) {
        self.bytes.push(0xFB);
    }

    fn halt(&mut self) {
        self.bytes.push(0x76);
    }

    fn stop(&mut self) {
        self.bytes.extend_from_slice(&[0x10, 0x00]);
    }

    fn inc_a(&mut self) {
        self.bytes.push(0x3C);
    }
}

fn load_fixture_machine(
    rom_name: &str,
    expected_rom: &[u8],
    console_model: ConsoleModel,
) -> Machine {
    let rom_fixture = common::fixtures::ensure_suite_binary_fixture(
        "phase2",
        rom_name,
        expected_rom,
        FIXTURE_ACCEPT_ENV,
    );
    let mut machine =
        Machine::new(MachineConfig::new(console_model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(rom_fixture)
        .expect("NoMBC test ROM should load");
    machine
}

fn assert_trace_fixture(trace_name: &str, trace: &str) {
    common::fixtures::ensure_suite_text_fixture("phase2", trace_name, trace, FIXTURE_ACCEPT_ENV);
}

fn step_until_wram_sentinel_with_driver<F>(
    machine: &mut Machine,
    address: u16,
    value: u8,
    max_steps: usize,
    driver: F,
) where
    F: FnMut(&mut Machine),
{
    common::machine_driver::step_until_wram_sentinel_with_driver(
        machine, address, value, max_steps, driver,
    );
}

fn step_until_wram_sentinel(machine: &mut Machine, address: u16, value: u8, max_steps: usize) {
    common::machine_driver::step_until_wram_sentinel(machine, address, value, max_steps);
}

fn assert_trace_fragments_in_order(trace: &str, fragments: &[&str]) {
    let mut search_start = 0;

    for fragment in fragments {
        let relative_index = trace[search_start..]
            .find(fragment)
            .unwrap_or_else(|| panic!("trace does not contain fragment: {fragment}"));
        search_start += relative_index + fragment.len();
    }
}

fn build_fetch_immediate_order_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();
    program.ld_sp_imm(0x1234);
    program.ld_a_imm(0x0F);
    program.add_a_imm(0x01);
    program.ld_a16_from_a(0xC011);
    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(SENTINEL_ADDRESS);
    program.jr_self();
    program.into_bytes()
}

fn build_control_flow_stack_cb_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();
    let subroutine_address = (PROGRAM_ENTRY_ADDRESS as u16) + 18;

    program.jr_offset(2);
    program.nop();
    program.nop();
    program.call(subroutine_address);
    program.ld_a_from_c();
    program.ld_a16_from_a(0xC011);
    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(SENTINEL_ADDRESS);
    program.jr_self();
    program.cb(0x11);
    program.ret();

    program.into_bytes()
}

fn build_ei_delay_priority_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();
    program.ld_a_imm(0x05);
    program.ld_a16_from_a(0xFFFF);
    program.ldh_a8_from_a(0x0F);
    program.ei();
    program.nop();
    program.jr_self();
    program.into_bytes()
}

fn build_ei_delay_priority_vector() -> [u8; 12] {
    [
        0xF0,
        0x0F,
        0xEA,
        0x11,
        0xC0,
        0x3E,
        SENTINEL_VALUE,
        0xEA,
        0x10,
        0xC0,
        0x18,
        0xFE,
    ]
}

fn build_halt_stop_and_halt_bug_program() -> (Vec<u8>, u16) {
    let mut program = ProgramBuilder::default();

    program.xor_a();
    program.ldh_a8_from_a(0x0F);
    program.ld_a_imm(0x04);
    program.ld_a16_from_a(0xFFFF);
    program.ld_a_imm(0xFF);
    program.ldh_a8_from_a(0x05);
    program.ld_a_imm(0x66);
    program.ldh_a8_from_a(0x06);
    program.ld_a_imm(0x05);
    program.ldh_a8_from_a(0x07);
    program.ei();
    program.nop();
    program.halt();
    program.jr_self();

    let phase_two_address = program.current_address();

    program.xor_a();
    program.ldh_a8_from_a(0x07);
    program.ldh_a8_from_a(0x0F);
    program.ld_a16_from_a(0xFFFF);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0xFFFF);
    program.ldh_a8_from_a(0x0F);
    program.halt();
    program.inc_a();
    program.ld_a16_from_a(0xC011);
    program.xor_a();
    program.ldh_a8_from_a(0x0F);
    program.ld_a16_from_a(0xFFFF);
    program.ld_a_imm(0x10);
    program.ld_c_imm(0x00);
    program.ld_ff00_plus_c_from_a();
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0xFFFF);
    program.stop();
    program.ei();
    program.nop();
    program.jr_self();

    (program.into_bytes(), phase_two_address)
}

fn build_jump_vector(address: u16) -> [u8; 3] {
    let [low, high] = address.to_le_bytes();
    [0xC3, low, high]
}

fn build_halt_stop_and_halt_bug_vblank_vector() -> [u8; 12] {
    [
        0xF0,
        0x0F,
        0xEA,
        0x12,
        0xC0,
        0x3E,
        SENTINEL_VALUE,
        0xEA,
        0x10,
        0xC0,
        0x18,
        0xFE,
    ]
}

fn build_timer_if_visibility_and_service_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();
    program.xor_a();
    program.ldh_a8_from_a(0x0F);
    program.ld_a_imm(0x04);
    program.ld_a16_from_a(0xFFFF);
    program.ld_a_imm(0xFF);
    program.ldh_a8_from_a(0x05);
    program.ld_a_imm(0x66);
    program.ldh_a8_from_a(0x06);
    program.ld_a_imm(0x05);
    program.ldh_a8_from_a(0x07);
    program.ei();
    program.nop();
    program.halt();
    program.jr_self();
    program.into_bytes()
}

fn build_timer_if_visibility_and_service_vector() -> [u8; 17] {
    [
        0xF0,
        0x05,
        0xEA,
        0x11,
        0xC0,
        0xF0,
        0x0F,
        0xEA,
        0x12,
        0xC0,
        0x3E,
        SENTINEL_VALUE,
        0xEA,
        0x10,
        0xC0,
        0x18,
        0xFE,
    ]
}

#[path = "phase2/phase2_control_flow.rs"]
mod phase2_control_flow;
#[path = "phase2/phase2_fetch_decode.rs"]
mod phase2_fetch_decode;
#[path = "phase2/phase2_halt_stop.rs"]
mod phase2_halt_stop;
#[path = "phase2/phase2_interrupts.rs"]
mod phase2_interrupts;
