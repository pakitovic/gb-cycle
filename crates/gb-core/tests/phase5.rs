mod common;

use common::machine_driver::step_machine_t_cycles;
use common::synthetic_cartridge::build_nom_bc_test_rom;
use gb_core::{
    ConsoleModel, CpuExecutionState, ExternalPortAttachmentKind, Machine, MachineConfig,
    StartupMode,
};

const FIXTURE_ACCEPT_ENV: &str = common::fixture_env::PHASE5;
const JOYPAD_STOP_TRACE_NAME: &str = "phase5_joypad_stop_wake_and_irq.trace";
const SERIAL_EXTERNAL_CLOCK_TRACE_NAME: &str = "phase5_serial_external_clock_progress.trace";

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
    common::fixtures::ensure_suite_text_fixture(
        "phase5",
        JOYPAD_STOP_TRACE_NAME,
        &trace,
        FIXTURE_ACCEPT_ENV,
    );
}

#[test]
fn phase_5_serial_external_clock_progress_trace_fixture_matches() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.set_external_port_attachment(ExternalPortAttachmentKind::Loopback);
    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF01, 0x96);
    machine.write_bus(0xFF02, 0x80);

    for _ in 0..8 {
        machine.queue_external_serial_clock();
        machine.step_t_cycle();
    }

    let trace = machine.tracer().sink().render_text();
    common::fixtures::ensure_suite_text_fixture(
        "phase5",
        SERIAL_EXTERNAL_CLOCK_TRACE_NAME,
        &trace,
        FIXTURE_ACCEPT_ENV,
    );
}
