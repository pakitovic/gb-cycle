mod common;

use gb_core::{
    ConsoleModel, Machine, MachineConfig, OperatingMode, SchedulerPhase, StartupMode, TCycle,
};

const FIXTURE_ACCEPT_ENV: &str = common::fixture_env::MACHINE;

fn build_header_mode_rom(cgb_flag: u8) -> Vec<u8> {
    let mut rom = common::synthetic_cartridge::build_nom_bc_test_rom(&[0x00], 0x00, &[]);
    rom[0x0143] = cgb_flag;
    rom
}

#[test]
fn machine_uses_a_single_step_t_cycle_entry_point() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    let context = machine.step_t_cycle();

    assert_eq!(context.t_cycle(), TCycle::new(0));
    assert_eq!(context.phase(), SchedulerPhase::CpuWakeInterruptEvaluation);
    assert_eq!(machine.next_t_cycle(), TCycle::new(1));
}

#[test]
fn machine_trace_includes_phase_aligned_subsystem_hooks() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();

    let fixture_path = common::paths::trace_fixture_path("machine_single_cycle_trace.txt");
    let expected = common::fixtures::ensure_text_fixture(
        &fixture_path,
        &machine.tracer().sink().render_text(),
        FIXTURE_ACCEPT_ENV,
    );

    assert_eq!(machine.tracer().sink().render_text(), expected);
}

#[test]
fn two_identical_machines_produce_the_same_two_cycle_trace() {
    let config = MachineConfig::new(ConsoleModel::GameBoy);
    let mut left = Machine::new(config.clone());
    let mut right = Machine::new(config);

    left.step_t_cycle();
    left.step_t_cycle();
    right.step_t_cycle();
    right.step_t_cycle();

    let fixture_path = common::paths::trace_fixture_path("machine_two_cycle_trace.txt");
    let expected = common::fixtures::ensure_text_fixture(
        &fixture_path,
        &left.tracer().sink().render_text(),
        FIXTURE_ACCEPT_ENV,
    );

    assert_eq!(left.tracer().sink().render_text(), expected);
    assert_eq!(right.tracer().sink().render_text(), expected);
    assert_eq!(left.next_t_cycle(), TCycle::new(2));
    assert_eq!(right.next_t_cycle(), TCycle::new(2));
}

#[test]
fn cgb_skip_boot_mode_follows_loaded_cartridge_header_without_becoming_dmg_silicon() {
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::GameBoyColor));

    machine
        .load_cartridge(build_header_mode_rom(0x00))
        .expect("DMG-compatible ROM should load on CGB");

    assert_eq!(machine.config().console_model, ConsoleModel::GameBoyColor);
    assert_eq!(machine.config().operating_mode, OperatingMode::GbCompatible);
    let capabilities = machine.config().capability_set();
    assert_eq!(capabilities.console_model(), ConsoleModel::GameBoyColor);
    assert!(capabilities.dmg_software_contract());
    assert!(!capabilities.cgb_extensions_enabled());
    assert!(!capabilities.dmg_family_quirks_enabled());
}

#[test]
fn cgb_skip_boot_mode_treats_supported_only_and_high_bit_noncanonical_as_native_cgb() {
    for cgb_flag in [0x80, 0xC0, 0xA0] {
        let mut machine = Machine::new(MachineConfig::new(ConsoleModel::GameBoyColor));

        machine
            .load_cartridge(build_header_mode_rom(cgb_flag))
            .expect("CGB ROM should load on CGB");

        assert_eq!(machine.config().console_model, ConsoleModel::GameBoyColor);
        assert_eq!(machine.config().operating_mode, OperatingMode::Cgb);
        let capabilities = machine.config().capability_set();
        assert!(!capabilities.dmg_software_contract());
        assert!(capabilities.cgb_extensions_enabled());
        assert!(!capabilities.dmg_family_quirks_enabled());
    }
}

#[test]
fn dmg_skip_boot_mode_ignores_cgb_header_without_enabling_cgb_capabilities() {
    for cgb_flag in [0x00, 0x80, 0xC0, 0xA0] {
        let mut machine = Machine::new(MachineConfig::new(ConsoleModel::GameBoy));

        machine
            .load_cartridge(build_header_mode_rom(cgb_flag))
            .expect("header matrix ROM should load on DMG");

        assert_eq!(machine.config().console_model, ConsoleModel::GameBoy);
        assert_eq!(machine.config().operating_mode, OperatingMode::Dmg);
        let capabilities = machine.config().capability_set();
        assert!(capabilities.dmg_software_contract());
        assert!(!capabilities.cgb_extensions_enabled());
        assert!(capabilities.dmg_family_quirks_enabled());
    }
}

#[test]
fn real_boot_keeps_operating_mode_boot_owned_until_cgb_handoff_work_lands() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::RealBoot),
    );

    machine
        .load_cartridge(build_header_mode_rom(0x00))
        .expect("ROM should load before real boot handoff");

    assert_eq!(machine.config().console_model, ConsoleModel::GameBoyColor);
    assert_eq!(machine.config().operating_mode, OperatingMode::Cgb);
}

#[test]
fn debug_memory_views_expose_raw_backing_storage_without_bus_reads() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.debug_vram_bytes().len(), 0x2000);
    assert_eq!(machine.debug_oam_bytes().len(), 0x00A0);
    assert_eq!(machine.debug_wram_bytes().len(), 0x2000);
    assert_eq!(machine.debug_hram_bytes().len(), 0x007F);

    machine.write_bus(0xC123, 0x42);
    machine.write_bus(0xE123, 0x99);
    machine.write_bus(0xFF80, 0x77);

    assert_eq!(machine.debug_wram_bytes()[0x0123], 0x99);
    assert_eq!(machine.debug_hram_bytes()[0], 0x77);
    assert_eq!(machine.debug_vram_bytes(), machine.bus().debug_vram_bytes());
    assert_eq!(machine.debug_oam_bytes(), machine.bus().debug_oam_bytes());
    assert_eq!(machine.debug_wram_bytes(), machine.bus().debug_wram_bytes());
    assert_eq!(machine.debug_hram_bytes(), machine.bus().debug_hram_bytes());
}
