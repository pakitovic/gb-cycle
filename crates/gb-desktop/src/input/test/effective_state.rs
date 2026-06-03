use super::*;

#[test]
fn effective_input_neutralizes_opposite_horizontal_directions_until_the_conflict_clears() {
    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();

    input_state.set_keyboard_button_for_target(
        &mut machine,
        FrontendJoypadTarget::Local,
        JoypadButton::Left,
        true,
    );
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Left));

    input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Right, true);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), 0);

    input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Right, false);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Left));
}

#[test]
fn effective_input_neutralizes_opposite_gamepad_directions_between_dpad_and_left_stick() {
    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();

    input_state.set_gamepad_button(&mut machine, JoypadButton::Left, true);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Left));

    input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Right, true);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), 0);

    input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Right, false);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Left));
}

#[test]
fn effective_input_neutralizes_opposite_vertical_directions_from_the_same_source() {
    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();

    input_state.set_keyboard_button_for_target(
        &mut machine,
        FrontendJoypadTarget::Local,
        JoypadButton::Up,
        true,
    );
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Up));

    input_state.set_keyboard_button_for_target(
        &mut machine,
        FrontendJoypadTarget::Local,
        JoypadButton::Down,
        true,
    );
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), 0);

    input_state.set_keyboard_button_for_target(
        &mut machine,
        FrontendJoypadTarget::Local,
        JoypadButton::Up,
        false,
    );
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Down));
}

#[test]
fn frontend_input_state_updates_machine_state_and_clears_each_source() {
    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();

    input_state.set_keyboard_button_for_target(
        &mut machine,
        FrontendJoypadTarget::Local,
        JoypadButton::A,
        true,
    );
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::A));

    input_state.set_gamepad_button(&mut machine, JoypadButton::A, true);
    input_state.set_keyboard_button_for_target(
        &mut machine,
        FrontendJoypadTarget::Local,
        JoypadButton::A,
        false,
    );
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::A));

    input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Left, true);
    ingest_host_input(&mut machine);
    assert_eq!(
        pressed_mask(&machine),
        joypad_mask(JoypadButton::A) | joypad_mask(JoypadButton::Left)
    );

    input_state.clear_keyboard_for_target(&mut machine, FrontendJoypadTarget::Local);
    ingest_host_input(&mut machine);
    assert_eq!(
        pressed_mask(&machine),
        joypad_mask(JoypadButton::A) | joypad_mask(JoypadButton::Left)
    );

    input_state.clear_gamepad(&mut machine);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), 0);

    input_state.set_keyboard_button_for_target(
        &mut machine,
        FrontendJoypadTarget::Local,
        JoypadButton::Start,
        true,
    );
    input_state.set_gamepad_button(&mut machine, JoypadButton::B, true);
    input_state.clear_all_for_target(&mut machine, FrontendJoypadTarget::Local);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), 0);
}

#[test]
fn clear_all_forces_machine_buttons_released_after_external_restore() {
    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();

    machine.set_joypad_button_pressed(JoypadButton::Right, true);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Right));

    input_state.clear_all_for_target(&mut machine, FrontendJoypadTarget::Local);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine), 0);
    assert!(!input_state.is_effectively_pressed(JoypadButton::Right));
}

#[test]
fn gamepad_button_helpers_round_trip_supported_buttons() {
    for (binding, button) in [
        (GamepadButtonBinding::South, Button::South),
        (GamepadButtonBinding::East, Button::East),
        (GamepadButtonBinding::West, Button::West),
        (GamepadButtonBinding::North, Button::North),
        (GamepadButtonBinding::Back, Button::Back),
        (GamepadButtonBinding::Start, Button::Start),
        (GamepadButtonBinding::Guide, Button::Guide),
        (GamepadButtonBinding::LeftShoulder, Button::LeftShoulder),
        (GamepadButtonBinding::RightShoulder, Button::RightShoulder),
        (GamepadButtonBinding::LeftStickClick, Button::LeftStick),
        (GamepadButtonBinding::RightStickClick, Button::RightStick),
        (GamepadButtonBinding::DPadUp, Button::DPadUp),
        (GamepadButtonBinding::DPadDown, Button::DPadDown),
        (GamepadButtonBinding::DPadLeft, Button::DPadLeft),
        (GamepadButtonBinding::DPadRight, Button::DPadRight),
        (GamepadButtonBinding::Misc1, Button::Misc1),
    ] {
        assert_eq!(sdl_button_for_binding(binding), Some(button));
        assert_eq!(
            gamepad_button_binding_from_sdl_button(button),
            Some(binding)
        );
    }
    assert_eq!(
        gamepad_button_binding_from_sdl_button(Button::Touchpad),
        None
    );
    assert_eq!(joystick_id_from_event(77).0, 77);
    assert_eq!(rumble_intensity(GamepadRumbleMode::Off), None);
    assert_eq!(
        rumble_intensity(GamepadRumbleMode::Strong),
        Some((
            STRONG_GAMEPAD_RUMBLE_INTENSITY,
            STRONG_GAMEPAD_RUMBLE_INTENSITY,
        ))
    );
    assert_eq!(
        rumble_intensity(GamepadRumbleMode::Weak),
        Some((WEAK_GAMEPAD_RUMBLE_INTENSITY, WEAK_GAMEPAD_RUMBLE_INTENSITY))
    );
}
