use super::*;

#[test]
fn joyp_keeps_upper_bits_high_and_uses_row_selection() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.set_button_pressed(JoypadButton::A, true);
    joypad.write_p1(0x10);

    assert_eq!(joypad.read_p1(), 0xDE);
}

#[test]
fn selecting_both_rows_combines_the_visible_matrix_without_priority() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.set_button_pressed(JoypadButton::A, true);
    joypad.set_button_pressed(JoypadButton::Right, true);
    joypad.write_p1(0x00);

    assert_eq!(joypad.read_p1(), 0xCE);
}

#[test]
fn writing_0x30_reads_back_all_released_when_no_rows_are_selected() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.set_button_pressed(JoypadButton::A, true);
    joypad.set_button_pressed(JoypadButton::Right, true);
    joypad.write_p1(0x3F);

    assert_eq!(joypad.read_p1(), 0xFF);
}

#[test]
fn button_row_and_dpad_row_are_observed_independently() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.set_button_pressed(JoypadButton::A, true);
    joypad.set_button_pressed(JoypadButton::Right, true);

    joypad.write_p1(0x10);
    assert_eq!(joypad.read_p1(), 0xDE);

    joypad.write_p1(0x20);
    assert_eq!(joypad.read_p1(), 0xEE);
}

#[test]
fn low_nibble_writes_do_not_override_the_live_matrix_view() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.set_button_pressed(JoypadButton::Start, true);
    joypad.write_p1(0x17);

    assert_eq!(joypad.read_p1(), 0xD7);
}

#[test]
fn startup_state_can_recreate_the_documented_post_boot_p1_snapshot() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.apply_startup_state(JoypadStartupState {
        selection_bits: 0x00,
        pressed_mask: 0x00,
    });

    assert_eq!(joypad.read_p1(), 0xCF);
}

#[test]
fn visible_button_press_on_the_selected_row_requests_the_joypad_interrupt() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.write_p1(0x10);
    joypad.set_button_pressed(JoypadButton::A, true);

    assert!(joypad.consume_interrupt_request());
    assert!(!joypad.consume_interrupt_request());
}

#[test]
fn unselected_row_changes_do_not_request_until_a_selection_write_makes_them_visible() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.write_p1(0x20);
    joypad.set_button_pressed(JoypadButton::A, true);
    assert!(!joypad.consume_interrupt_request());

    joypad.write_p1(0x10);
    assert!(joypad.consume_interrupt_request());
}

#[test]
fn repeated_visible_high_to_low_transitions_can_request_repeatedly() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.write_p1(0x10);
    joypad.set_button_pressed(JoypadButton::A, true);
    assert!(joypad.consume_interrupt_request());

    joypad.set_button_pressed(JoypadButton::A, false);
    assert!(!joypad.consume_interrupt_request());

    joypad.set_button_pressed(JoypadButton::A, true);
    assert!(joypad.consume_interrupt_request());
}

#[test]
fn both_rows_selected_use_the_same_combined_visible_rule_for_interrupt_edges() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.write_p1(0x00);
    joypad.set_button_pressed(JoypadButton::A, true);
    assert!(joypad.consume_interrupt_request());

    joypad.set_button_pressed(JoypadButton::Right, true);
    assert!(!joypad.consume_interrupt_request());

    joypad.set_button_pressed(JoypadButton::Up, true);
    assert!(joypad.consume_interrupt_request());
}

#[test]
fn stop_wake_uses_the_selected_visible_lines() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.write_p1(0x30);
    joypad.set_button_pressed(JoypadButton::A, true);
    assert!(!joypad.stop_wake_line_asserted());
    assert!(!joypad.consume_stop_wake_event());

    joypad.write_p1(0x10);
    assert!(joypad.stop_wake_line_asserted());
    assert!(joypad.consume_stop_wake_event());

    joypad.set_button_pressed(JoypadButton::Left, true);
    assert!(!joypad.consume_stop_wake_event());

    joypad.write_p1(0x00);
    assert!(joypad.stop_wake_line_asserted());
    assert!(joypad.consume_stop_wake_event());
}

#[test]
fn stop_wake_requires_a_new_released_to_pressed_transition() {
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    joypad.write_p1(0x10);
    joypad.set_button_pressed(JoypadButton::Start, true);
    assert!(joypad.consume_stop_wake_event());
    assert!(joypad.stop_wake_line_asserted());

    joypad.set_button_pressed(JoypadButton::Start, true);
    assert!(!joypad.consume_stop_wake_event());
    assert!(joypad.stop_wake_line_asserted());

    joypad.set_button_pressed(JoypadButton::Start, false);
    assert!(!joypad.consume_stop_wake_event());
    assert!(!joypad.stop_wake_line_asserted());

    joypad.set_button_pressed(JoypadButton::Start, true);
    assert!(joypad.consume_stop_wake_event());
    assert!(joypad.stop_wake_line_asserted());
}
