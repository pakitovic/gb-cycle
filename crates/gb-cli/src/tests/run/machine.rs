use super::super::*;

#[test]
fn cli_machine_exposes_summary_and_buffered_views() {
    let mut summary = build_loaded_machine(build_single_byte_serial_rom(b'S'), false);
    assert!(summary.at_frame_origin());
    assert!(!summary.is_boot_rom_mapped());
    assert_eq!(
        summary.framebuffer().len(),
        FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT
    );
    assert_eq!(
        summary.cartridge().persistent_state(),
        PersistentCartState::None
    );
    assert!(
        summary
            .restore_cartridge_persistent_state(&PersistentCartState::None)
            .is_ok()
    );
    assert!(summary.trace_text().is_none());
    summary.set_joypad_button_pressed(JoypadButton::A, true);
    summary.set_joypad_button_pressed(JoypadButton::A, false);
    summary.step_t_cycle();
    let _ = summary.take_serial_output_bytes();

    let mut buffered = build_loaded_machine(build_single_byte_serial_rom(b'B'), true);
    buffered.set_joypad_button_pressed(JoypadButton::Start, true);
    buffered.step_t_cycle();
    let snapshot = buffered.capture_save_state();
    buffered
        .restore_save_state(&snapshot)
        .expect("buffered machines should restore their own snapshots");
    let trace_text = buffered
        .trace_text()
        .expect("buffered machines should expose trace text");
    assert!(trace_text.contains("t_cycle="));
}
