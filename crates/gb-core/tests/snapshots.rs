mod common;

use gb_core::{
    ConsoleModel, CpuExecutionState, Machine, MachineConfig, SerialClockMode, SerialTransferState,
    StartupMode, TCycle,
};

#[test]
fn machine_snapshot_captures_debug_inspection_state_after_two_cycles() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();
    machine.step_t_cycle();

    let snapshot = machine.snapshot();

    assert_eq!(snapshot.config.console_model, ConsoleModel::Dmg);
    assert_eq!(snapshot.config.startup_mode, StartupMode::SkipBoot);
    assert_eq!(snapshot.scheduler.next_t_cycle, TCycle::new(2));
    assert_eq!(snapshot.trace.buffered_event_count, 40);
    assert_eq!(snapshot.debug_controls.breakpoint_count, 0);
    assert_eq!(snapshot.debug_controls.watchpoint_count, 0);
    assert_eq!(snapshot.boot.startup_mode, StartupMode::SkipBoot);
    assert_eq!(snapshot.cpu.startup_state.pc, 0x0100);
    assert_eq!(snapshot.cpu.registers.pc, 0x0100);
    assert_eq!(
        snapshot.cpu.execution_state,
        CpuExecutionState::FetchOpcode { t_cycle: 2 }
    );
    assert_eq!(snapshot.cpu.current_opcode, None);
    assert!(snapshot.apu.powered);
    assert_eq!(snapshot.apu.div_apu, 0);
    assert_eq!(snapshot.serial.sb, 0x00);
    assert_eq!(snapshot.serial.clock_mode, SerialClockMode::External);
    assert_eq!(snapshot.serial.transfer_state, SerialTransferState::Idle);
    assert!(!snapshot.boot.boot_rom_mapped);
    assert!(!snapshot.boot.boot_rom_asset_configured);
    assert!(snapshot.cartridge.state == gb_core::CartridgeSlotState::Empty);
}

#[test]
fn machine_snapshot_rendering_matches_the_golden_fixture() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();
    machine.step_t_cycle();

    let snapshot = machine.snapshot();
    let fixture_path = common::trace_fixtures_dir().join("machine_snapshot_after_two_cycles.txt");
    let expected = common::read_text_fixture(&fixture_path).expect("fixture should be readable");

    assert_eq!(snapshot.render_text(), expected);
}
