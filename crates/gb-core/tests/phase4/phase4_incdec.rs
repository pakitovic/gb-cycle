use super::*;

#[test]
fn phase_4_inc_hl_rom_fixture_matches_expected_oam_state_and_traces_for_all_models() {
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_inc_hl_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[],
    );
    let trace_cases = [
        (ConsoleModel::Dmg0, INC_HL_DMG0_TRACE_NAME, true),
        (ConsoleModel::Dmg, INC_HL_DMG_TRACE_NAME, true),
        (ConsoleModel::Mgb, INC_HL_MGB_TRACE_NAME, true),
        (ConsoleModel::Cgb, INC_HL_CGB_TRACE_NAME, false),
    ];

    for (console_model, trace_name, expect_corruption) in trace_cases {
        let mut machine = run_fixture_rom(
            INC_HL_ROM_NAME,
            trace_name,
            &expected_rom,
            console_model,
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
            [0x55, 0x55, 0xCC, 0xCC, 0xDD, 0xDD, 0xEE, 0xEE],
        );
        if expect_corruption {
            apply_write_corruption(&mut expected, 8);
        }

        assert_machine_rows(&mut machine, &expected, &[7, 8]);
    }
}

#[test]
fn phase_4_hli_hld_rom_fixture_matches_expected_oam_state_and_trace() {
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_hli_hld_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[],
    );
    let mut machine = run_fixture_rom(
        HLI_HLD_ROM_NAME,
        HLI_HLD_TRACE_NAME,
        &expected_rom,
        ConsoleModel::Dmg,
        4_096,
    );

    let mut expected = [0; 160];
    write_expected_row(
        &mut expected,
        6,
        [0x0F, 0x0F, 0x10, 0x10, 0x20, 0x20, 0x30, 0x30],
    );
    write_expected_row(
        &mut expected,
        7,
        [0xAA, 0xAA, 0x11, 0x11, 0xC0, 0xC0, 0x22, 0x22],
    );
    write_expected_row(
        &mut expected,
        8,
        [0xFF, 0x00, 0x33, 0x33, 0x44, 0x44, 0x55, 0x55],
    );
    apply_read_with_incdec_corruption(&mut expected, 8);

    write_expected_row(
        &mut expected,
        11,
        [0x34, 0x12, 0x66, 0x66, 0x0F, 0xF0, 0x77, 0x77],
    );
    write_expected_row(
        &mut expected,
        12,
        [0x5A, 0xA5, 0x88, 0x88, 0x99, 0x99, 0xAA, 0xAA],
    );
    apply_write_corruption(&mut expected, 12);

    assert_machine_rows(&mut machine, &expected, &[6, 7, 8, 11, 12]);
}
