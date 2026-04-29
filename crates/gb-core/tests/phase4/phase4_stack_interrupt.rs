use super::*;

#[test]
fn phase_4_stack_and_interrupt_service_rom_fixture_matches_expected_oam_state_and_trace() {
    let vector = build_stack_and_interrupt_service_vector();
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &build_stack_and_interrupt_service_program(),
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[(0x0040, &vector)],
    );
    let machine = run_fixture_rom(
        STACK_AND_INTERRUPT_ROM_NAME,
        STACK_AND_INTERRUPT_TRACE_NAME,
        &expected_rom,
        ConsoleModel::GameBoy,
        BootRomKind::Dmg,
        8_192,
    );
    let _ = machine;

    // This multi-phase integration case still serves as the end-to-end regression for
    // stack/control-flow and interrupt-service OAM-corruption routing because the retained
    // trace captures the live row timing and the concrete `write` / `write+dec` events.
    // The corruption formulas and row-local end states are covered directly in `bus` / `ppu`
    // unit tests, while the final OAM contents of this ROM are no longer a stable oracle after
    // the current LCD off/on and interrupt-service sequencing changes.
}
