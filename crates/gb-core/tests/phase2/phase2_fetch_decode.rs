use super::*;

#[test]
fn phase_2_fetch_immediate_order_rom_fixture_matches_expected_trace_and_state() {
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_fetch_immediate_order_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[],
    );
    let mut machine = load_fixture_machine(
        FETCH_IMMEDIATE_ROM_NAME,
        &expected_rom,
        ConsoleModel::GameBoy,
    );

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
fn phase_2_trace_shows_boot_handoff_before_the_first_cartridge_fetch() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy)
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
            "subsystem=boot level=trace message=\"t_cycle=39 phase=mmio_side_effect_commit console_model=GameBoy startup_mode=RealBoot status=Ready boot_rom_kind=Dmg boot_rom_mapped=false\"",
            "subsystem=cpu level=trace message=\"t_cycle=43 phase=cpu_micro_operation",
            "last_bus_activity=opcode_fetch@0x0100=0xD3",
        ],
    );
}
