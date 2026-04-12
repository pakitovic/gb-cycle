mod common;

use common::synthetic_cartridge::{
    HEADER_MINIMUM_ROM_LEN, PROGRAM_ENTRY_ADDRESS, build_nom_bc_test_rom_with_program_entry,
};
use gb_core::{
    BootRomAssets, BootRomKind, ConsoleModel, CpuExecutionState, JoypadButton, Machine,
    MachineConfig, StartupMode,
};

const FIXTURE_ACCEPT_ENV: &str = "GB_CYCLE_ACCEPT_PHASE2_FIXTURES";
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
    let rom_fixture_path = common::rom_fixtures_dir().join("phase2").join(rom_name);
    let rom_fixture =
        common::ensure_binary_fixture(&rom_fixture_path, expected_rom, FIXTURE_ACCEPT_ENV);
    let mut machine =
        Machine::new(MachineConfig::new(console_model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(rom_fixture)
        .expect("NoMBC test ROM should load");
    machine
}

fn assert_trace_fixture(trace_name: &str, trace: &str) {
    let trace_fixture_path = common::trace_fixtures_dir().join("phase2").join(trace_name);
    common::ensure_text_fixture(&trace_fixture_path, trace, FIXTURE_ACCEPT_ENV);
}

fn step_machine_t_cycles(machine: &mut Machine, steps: usize) {
    for _ in 0..steps {
        machine.step_t_cycle();
    }
}

fn step_machine_until(
    machine: &mut Machine,
    max_steps: usize,
    predicate: impl Fn(&Machine) -> bool,
) {
    for _ in 0..max_steps {
        if predicate(machine) {
            return;
        }
        machine.step_t_cycle();
    }

    assert!(
        predicate(machine),
        "predicate was not satisfied within {max_steps} T-cycles"
    );
}

fn step_until_wram_sentinel_with_driver<F>(
    machine: &mut Machine,
    address: u16,
    value: u8,
    max_steps: usize,
    mut driver: F,
) where
    F: FnMut(&mut Machine),
{
    for _ in 0..max_steps {
        if machine.read_bus(address) == value {
            return;
        }
        driver(machine);
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

fn step_until_wram_sentinel(machine: &mut Machine, address: u16, value: u8, max_steps: usize) {
    step_until_wram_sentinel_with_driver(machine, address, value, max_steps, |_| {});
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

#[test]
fn phase_2_fetch_immediate_order_rom_fixture_matches_expected_trace_and_state() {
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_fetch_immediate_order_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[],
    );
    let mut machine =
        load_fixture_machine(FETCH_IMMEDIATE_ROM_NAME, &expected_rom, ConsoleModel::Dmg);

    step_until_wram_sentinel(&mut machine, SENTINEL_ADDRESS, SENTINEL_VALUE, 512);

    assert_eq!(machine.cpu().registers().sp, 0x1234);
    assert_eq!(machine.read_bus(0xC011), 0x10);
    assert_eq!(machine.cpu().registers().f, 0x20);
    assert_trace_fixture(
        FETCH_IMMEDIATE_TRACE_NAME,
        &machine.tracer().sink().render_text(),
    );
}

#[test]
fn phase_2_control_flow_stack_cb_rom_fixture_matches_expected_trace_and_state() {
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_control_flow_stack_cb_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[],
    );
    let mut machine = load_fixture_machine(
        CONTROL_FLOW_STACK_CB_ROM_NAME,
        &expected_rom,
        ConsoleModel::Dmg,
    );

    step_until_wram_sentinel(&mut machine, SENTINEL_ADDRESS, SENTINEL_VALUE, 1_024);

    assert_eq!(machine.read_bus(0xC011), 0x27);
    assert_eq!(machine.cpu().registers().c, 0x27);
    assert_eq!(machine.cpu().registers().sp, 0xFFFE);
    assert_eq!(machine.cpu().registers().f, 0x00);
    assert_trace_fixture(
        CONTROL_FLOW_STACK_CB_TRACE_NAME,
        &machine.tracer().sink().render_text(),
    );
}

#[test]
fn phase_2_ei_delay_priority_rom_fixture_matches_expected_trace_and_state() {
    let vector = build_ei_delay_priority_vector();
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_ei_delay_priority_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[(0x0040, &vector)],
    );
    let mut machine =
        load_fixture_machine(EI_DELAY_PRIORITY_ROM_NAME, &expected_rom, ConsoleModel::Dmg);

    step_until_wram_sentinel(&mut machine, SENTINEL_ADDRESS, SENTINEL_VALUE, 1_024);

    assert_eq!(machine.read_bus(0xC011), 0xE4);
    assert_eq!(machine.cpu().registers().sp, 0xFFFC);
    assert_eq!(machine.read_bus(0xFFFD), 0x01);
    assert_eq!(machine.read_bus(0xFFFC), 0x59);
    assert_trace_fixture(
        EI_DELAY_PRIORITY_TRACE_NAME,
        &machine.tracer().sink().render_text(),
    );
}

#[test]
fn phase_2_halt_stop_and_halt_bug_rom_fixture_matches_expected_trace_and_state() {
    let (program, phase_two_address) = build_halt_stop_and_halt_bug_program();
    let timer_vector = build_jump_vector(phase_two_address);
    let vblank_vector = build_halt_stop_and_halt_bug_vblank_vector();
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &program,
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[(0x0040, &vblank_vector), (0x0050, &timer_vector)],
    );
    let mut machine = load_fixture_machine(
        HALT_STOP_AND_HALT_BUG_ROM_NAME,
        &expected_rom,
        ConsoleModel::Dmg,
    );
    let mut stop_wake_injected = false;
    let mut stop_irq_injected = false;

    step_until_wram_sentinel_with_driver(
        &mut machine,
        SENTINEL_ADDRESS,
        SENTINEL_VALUE,
        2_048,
        |machine| {
            if !stop_wake_injected
                && matches!(machine.cpu().execution_state(), CpuExecutionState::Stopped)
            {
                machine.set_joypad_button_pressed(JoypadButton::A, true);
                stop_wake_injected = true;
            } else if stop_wake_injected
                && !stop_irq_injected
                && !matches!(machine.cpu().execution_state(), CpuExecutionState::Stopped)
            {
                machine.write_bus(0xFF0F, 0x01);
                stop_irq_injected = true;
            }
        },
    );

    assert!(stop_wake_injected);
    assert!(stop_irq_injected);
    assert_eq!(machine.read_bus(0xC011), 0x03);
    assert_eq!(machine.read_bus(0xC012), 0xE0);
    assert_trace_fixture(
        HALT_STOP_AND_HALT_BUG_TRACE_NAME,
        &machine.tracer().sink().render_text(),
    );
}

#[test]
fn phase_2_timer_if_visibility_and_service_rom_fixture_matches_expected_trace_and_state() {
    let vector = build_timer_if_visibility_and_service_vector();
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_timer_if_visibility_and_service_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[(0x0050, &vector)],
    );
    let mut machine = load_fixture_machine(
        TIMER_IF_VISIBILITY_ROM_NAME,
        &expected_rom,
        ConsoleModel::Dmg,
    );

    step_until_wram_sentinel(&mut machine, SENTINEL_ADDRESS, SENTINEL_VALUE, 1_024);

    assert_eq!(machine.read_bus(0xC011), 0x68);
    assert_eq!(machine.read_bus(0xC012), 0xE0);
    assert_eq!(machine.cpu().registers().sp, 0xFFFC);
    assert_eq!(machine.read_bus(0xFFFD), 0x01);
    assert_eq!(machine.read_bus(0xFFFC), 0x66);
    assert_trace_fixture(
        TIMER_IF_VISIBILITY_TRACE_NAME,
        &machine.tracer().sink().render_text(),
    );
}

#[test]
fn phase_2_trace_shows_fetch_operand_if_visibility_and_interrupt_acceptance() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_phase_2_fragment_rom(&[0x3E, 0x12, 0xFB, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);

    step_machine_t_cycles(&mut machine, 16);

    let trace = machine.tracer().sink().render_text();

    assert_trace_fragments_in_order(
        &trace,
        &[
            "subsystem=cpu level=trace message=\"t_cycle=3 phase=cpu_micro_operation",
            "last_bus_activity=opcode_fetch@0x0100=0x3E",
            "subsystem=cpu level=trace message=\"t_cycle=7 phase=cpu_micro_operation",
            "last_bus_activity=operand_read@0x0101=0x12",
            "subsystem=interrupts level=trace message=\"t_cycle=15 phase=interrupt_aggregation console_model=Dmg status=Ready if=0xE1 ie=0x01\"",
            "subsystem=interrupts level=trace message=\"t_cycle=15 phase=cpu_wake_interrupt_evaluation console_model=Dmg status=Ready if=0xE0 ie=0x01\"",
            "subsystem=cpu level=trace message=\"t_cycle=15 phase=cpu_wake_interrupt_evaluation",
            "execution_state=ServiceInterrupt { source: VBlank, step: 0, t_cycle: 0 }",
        ],
    );
}

#[test]
fn phase_2_trace_shows_boot_handoff_before_the_first_cartridge_fetch() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(BootRomKind::Dmg, build_phase_2_trace_boot_rom())
                    .expect("phase 2 boot-trace ROM should validate"),
            ),
    );

    machine
        .load_cartridge(build_phase_2_real_boot_rom(PHASE_2_ENTRY_OPCODE))
        .expect("NoMBC test ROM should load");

    step_machine_until(&mut machine, 48, |machine| {
        machine.cpu().current_opcode() == Some(PHASE_2_ENTRY_OPCODE)
    });

    let trace = machine.tracer().sink().render_text();

    assert_trace_fragments_in_order(
        &trace,
        &[
            "subsystem=cpu level=trace message=\"t_cycle=39 phase=cpu_micro_operation",
            "last_bus_activity=data_write@0xFF50=",
            "subsystem=boot level=trace message=\"t_cycle=39 phase=mmio_side_effect_commit console_model=Dmg startup_mode=RealBoot status=Ready boot_rom_kind=Dmg boot_rom_mapped=false\"",
            "subsystem=cpu level=trace message=\"t_cycle=43 phase=cpu_micro_operation",
            "last_bus_activity=opcode_fetch@0x0100=0xD3",
        ],
    );
}
