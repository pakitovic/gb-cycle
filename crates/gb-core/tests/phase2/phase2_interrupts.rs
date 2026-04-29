use super::*;

#[test]
fn phase_2_ei_delay_priority_rom_fixture_matches_expected_trace_and_state() {
    let vector = build_ei_delay_priority_vector();
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_ei_delay_priority_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[(0x0040, &vector)],
    );
    let mut machine = load_fixture_machine(
        EI_DELAY_PRIORITY_ROM_NAME,
        &expected_rom,
        ConsoleModel::GameBoy,
    );

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
        ConsoleModel::GameBoy,
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
            "subsystem=interrupts level=trace message=\"t_cycle=15 phase=interrupt_aggregation console_model=GameBoy status=Ready if=0xE1 ie=0x01\"",
            "subsystem=interrupts level=trace message=\"t_cycle=15 phase=cpu_wake_interrupt_evaluation console_model=GameBoy status=Ready if=0xE0 ie=0x01\"",
            "subsystem=cpu level=trace message=\"t_cycle=15 phase=cpu_wake_interrupt_evaluation",
            "execution_state=ServiceInterrupt { source: VBlank, step: 0, t_cycle: 0 }",
        ],
    );
}
