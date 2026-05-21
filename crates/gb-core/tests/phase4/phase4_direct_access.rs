use super::*;

#[test]
fn phase_4_direct_mode2_oam_access_rom_fixture_matches_expected_oam_state_and_trace() {
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_direct_mode2_oam_access_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[],
    );
    let mut machine = run_fixture_rom(
        DIRECT_MODE2_ROM_NAME,
        DIRECT_MODE2_TRACE_NAME,
        &expected_rom,
        ConsoleModel::GameBoy,
        HardwareRevision::DmgCpuC,
        2_048,
    );

    let mut expected = [0; 160];
    write_expected_row(
        &mut expected,
        7,
        [0x5A, 0xA5, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
    );
    write_expected_row(
        &mut expected,
        8,
        [0x12, 0x34, 0x21, 0x43, 0x65, 0x87, 0xA9, 0xCB],
    );
    apply_write_corruption(&mut expected, 8);

    assert_machine_rows(&mut machine, &expected, &[7, 8]);
}

#[test]
fn phase_4_fea0_mode2_read_rom_fixture_matches_expected_oam_state_and_trace() {
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_fea0_mode2_read_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[],
    );
    let mut machine = run_fixture_rom(
        FEA0_MODE2_ROM_NAME,
        FEA0_MODE2_TRACE_NAME,
        &expected_rom,
        ConsoleModel::GameBoy,
        HardwareRevision::DmgCpuC,
        2_048,
    );

    let mut expected = [0; 160];
    write_expected_row(
        &mut expected,
        7,
        [0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB],
    );
    write_expected_row(
        &mut expected,
        8,
        [0xF0, 0xF0, 0x11, 0x11, 0x22, 0x22, 0x33, 0x33],
    );
    apply_read_corruption(&mut expected, 8);

    assert_machine_rows(&mut machine, &expected, &[7, 8]);
}
