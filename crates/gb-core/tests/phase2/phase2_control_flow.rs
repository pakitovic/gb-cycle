use super::*;

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
