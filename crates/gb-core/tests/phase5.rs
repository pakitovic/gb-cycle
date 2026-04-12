mod common;

use common::synthetic_cartridge::build_nom_bc_test_rom;
use gb_core::{ConsoleModel, CpuExecutionState, Machine, MachineConfig, SerialPeer, StartupMode};

const FIXTURE_ACCEPT_ENV: &str = "GB_CYCLE_ACCEPT_PHASE5_FIXTURES";
const JOYPAD_STOP_TRACE_NAME: &str = "phase5_joypad_stop_wake_and_irq.trace";
const SERIAL_EXTERNAL_CLOCK_TRACE_NAME: &str = "phase5_serial_external_clock_progress.trace";

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
        .load_cartridge(build_nom_bc_test_rom(&[0x10, 0x00, 0xFB, 0x00], 0x12, &[]))
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
    common::ensure_text_fixture(&fixture_path, &trace, FIXTURE_ACCEPT_ENV);
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
    common::ensure_text_fixture(&fixture_path, &trace, FIXTURE_ACCEPT_ENV);
}
