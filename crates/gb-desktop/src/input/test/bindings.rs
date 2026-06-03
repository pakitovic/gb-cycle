use super::*;

#[test]
fn east_a_face_layout_uses_east_for_a_and_south_for_b() {
    let (a, b) = GamepadFaceLayout::EastASouthB.face_buttons();

    assert_eq!(a, GamepadButtonBinding::East);
    assert_eq!(b, GamepadButtonBinding::South);
}

#[test]
fn south_a_face_layout_uses_south_for_a_and_east_for_b() {
    let (a, b) = GamepadFaceLayout::SouthAEastB.face_buttons();

    assert_eq!(a, GamepadButtonBinding::South);
    assert_eq!(b, GamepadButtonBinding::East);
}

#[test]
fn shoulder_bindings_map_to_the_expected_sdl_buttons() {
    assert_eq!(
        sdl_button_for_binding(GamepadButtonBinding::LeftShoulder),
        Some(Button::LeftShoulder)
    );
    assert_eq!(
        sdl_button_for_binding(GamepadButtonBinding::RightShoulder),
        Some(Button::RightShoulder)
    );
}

#[test]
fn trigger_bindings_map_to_the_expected_sdl_axes() {
    assert_eq!(
        gamepad_trigger_axis_for_binding(GamepadButtonBinding::LeftTrigger),
        Some(Axis::TriggerLeft)
    );
    assert_eq!(
        gamepad_trigger_axis_for_binding(GamepadButtonBinding::RightTrigger),
        Some(Axis::TriggerRight)
    );
    assert_eq!(
        gamepad_button_binding_from_sdl_axis(Axis::TriggerLeft),
        Some(GamepadButtonBinding::LeftTrigger)
    );
    assert_eq!(
        gamepad_button_binding_from_sdl_axis(Axis::TriggerRight),
        Some(GamepadButtonBinding::RightTrigger)
    );
    assert_eq!(gamepad_button_binding_from_sdl_axis(Axis::LeftX), None);
    assert_eq!(
        sdl_button_for_binding(GamepadButtonBinding::LeftTrigger),
        None
    );
    assert!(gamepad_trigger_axis_next_pressed(17_000, false));
    assert!(gamepad_trigger_axis_next_pressed(13_000, true));
    assert!(!gamepad_trigger_axis_next_pressed(11_000, true));
    assert!(gamepad_trigger_axis_is_pressed(i16::MAX));
    assert!(!gamepad_trigger_axis_is_pressed(0));
}

#[test]
fn axis_direction_state_uses_hysteresis_before_releasing() {
    assert_eq!(axis_direction_state(17_000, false, false), (false, true));
    assert_eq!(axis_direction_state(13_000, false, true), (false, true));
    assert_eq!(axis_direction_state(11_000, false, true), (false, false));
}

#[test]
fn mbc7_gyro_helpers_map_host_units_to_milli_g() {
    assert_eq!(right_stick_axis_to_milli_g(0), 0);
    assert_eq!(right_stick_axis_to_milli_g(i16::MAX), 1_000);
    assert_eq!(right_stick_axis_to_milli_g(i16::MIN), -1_000);
    assert_eq!(
        acceleration_to_milli_g(SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED),
        1_000
    );
    assert_eq!(
        acceleration_to_milli_g(-SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED),
        -1_000
    );
    assert_eq!(
        GAMEPAD_ACCELEROMETER_SENSORS,
        [
            SensorType::Accelerometer,
            SensorType::AccelerometerLeft,
            SensorType::AccelerometerRight
        ]
    );
}

#[test]
fn effective_input_keeps_direction_pressed_while_dpad_and_stick_overlap() {
    let mut input_state = super::super::FrontendInputState::new();

    input_state
        .gamepad_buttons
        .set_pressed(JoypadButton::Left, true);
    input_state
        .gamepad_left_stick
        .set_pressed(JoypadButton::Left, true);
    assert!(input_state.is_effectively_pressed(JoypadButton::Left));

    input_state
        .gamepad_buttons
        .set_pressed(JoypadButton::Left, false);
    assert!(input_state.is_effectively_pressed(JoypadButton::Left));

    input_state
        .gamepad_left_stick
        .set_pressed(JoypadButton::Left, false);
    assert!(!input_state.is_effectively_pressed(JoypadButton::Left));
}
