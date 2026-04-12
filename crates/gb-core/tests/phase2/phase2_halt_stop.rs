use super::*;

#[test]
fn phase_2_halt_stop_and_halt_bug_rom_fixture_matches_expected_trace_and_state() {
    let (program, phase_two_address) = build_halt_stop_and_halt_bug_program();
    let timer_vector = build_jump_vector(phase_two_address);
    let vblank_vector = build_halt_stop_and_halt_bug_vblank_vector();
    let expected_rom = build_nom_bc_test_rom_with_program_entry(
        &program,
        TEST_ROM_BOOT_OPCODE,
        PROGRAM_ENTRY_ADDRESS,
        &[(0x0040, &vblank_vector), (0x0050, &timer_vector)],
    );
    let mut machine = load_fixture_machine(
        HALT_STOP_AND_HALT_BUG_ROM_NAME,
        &expected_rom,
        ConsoleModel::Dmg,
    );
    let mut stop_wake_injected = false;
    let mut stop_irq_injected = false;

    step_until_wram_sentinel_with_driver(
        &mut machine,
        SENTINEL_ADDRESS,
        SENTINEL_VALUE,
        2_048,
        |machine| {
            if !stop_wake_injected
                && matches!(machine.cpu().execution_state(), CpuExecutionState::Stopped)
            {
                machine.set_joypad_button_pressed(JoypadButton::A, true);
                stop_wake_injected = true;
            } else if stop_wake_injected
                && !stop_irq_injected
                && !matches!(machine.cpu().execution_state(), CpuExecutionState::Stopped)
            {
                machine.write_bus(0xFF0F, 0x01);
                stop_irq_injected = true;
            }
        },
    );

    assert!(stop_wake_injected);
    assert!(stop_irq_injected);
    assert_eq!(machine.read_bus(0xC011), 0x03);
    assert_eq!(machine.read_bus(0xC012), 0xE0);
    assert_trace_fixture(
        HALT_STOP_AND_HALT_BUG_TRACE_NAME,
        &machine.tracer().sink().render_text(),
    );
}
