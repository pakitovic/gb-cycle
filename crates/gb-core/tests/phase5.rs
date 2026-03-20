mod common;

use std::env;
use std::fs;
use std::path::Path;

use gb_core::{ConsoleModel, CpuExecutionState, Machine, MachineConfig, SerialPeer, StartupMode};

const FIXTURE_ACCEPT_ENV: &str = "GB_CYCLE_ACCEPT_PHASE5_FIXTURES";
const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const JOYPAD_STOP_TRACE_NAME: &str = "phase5_joypad_stop_wake_and_irq.trace";
const SERIAL_EXTERNAL_CLOCK_TRACE_NAME: &str = "phase5_serial_external_clock_progress.trace";

fn fixture_accept_writes_enabled() -> bool {
    env::var_os(FIXTURE_ACCEPT_ENV).is_some()
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

fn build_test_rom(program: &[u8], boot_opcode: u8, extra_segments: &[(usize, &[u8])]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = boot_opcode;
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    for &(address, bytes) in extra_segments {
        rom[address..address + bytes.len()].copy_from_slice(bytes);
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn step_machine_t_cycles(machine: &mut Machine, steps: usize) {
    for _ in 0..steps {
        machine.step_t_cycle();
    }
}

#[test]
fn phase_5_joypad_stop_wake_and_irq_trace_fixture_matches() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0xFB, 0x00], 0x12, &[]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x10);
    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF00, 0x10);
    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);

    machine.set_joypad_button_pressed(gb_core::JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 9);

    let trace = machine.tracer().sink().render_text();
    let fixture_path = common::trace_fixtures_dir()
        .join("phase5")
        .join(JOYPAD_STOP_TRACE_NAME);
    ensure_text_fixture(&fixture_path, &trace);
}

#[test]
fn phase_5_serial_external_clock_progress_trace_fixture_matches() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.set_serial_peer(SerialPeer::Loopback);
    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF01, 0x96);
    machine.write_bus(0xFF02, 0x80);

    for _ in 0..8 {
        machine.queue_external_serial_clock();
        machine.step_t_cycle();
    }

    let trace = machine.tracer().sink().render_text();
    let fixture_path = common::trace_fixtures_dir()
        .join("phase5")
        .join(SERIAL_EXTERNAL_CLOCK_TRACE_NAME);
    ensure_text_fixture(&fixture_path, &trace);
}
