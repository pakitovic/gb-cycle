use super::*;

#[test]
fn default_gamepad_name_uses_the_sdl_identifier_suffix() {
    let joystick_id = JoystickId::from(sdl3::sys::joystick::SDL_JoystickID(7));

    assert_eq!(default_gamepad_name(joystick_id), "SDL gamepad 7");
}

#[test]
fn gamepad_error_formatters_include_the_host_context() {
    sdl3::clear_error();
    sdl3::set_error("enumeration failed").expect("SDL test error should be writable");
    let enumeration = format_gamepad_enumeration_error(sdl3::get_error());
    assert!(enumeration.contains("failed to enumerate SDL3 gamepads"));
    assert!(enumeration.contains("enumeration failed"));

    sdl3::clear_error();
    sdl3::set_error("open failed").expect("SDL test error should be writable");
    let joystick_id = JoystickId::from(sdl3::sys::joystick::SDL_JoystickID(9));
    let open_error = format_open_gamepad_error(joystick_id, sdl3::get_error());
    assert!(open_error.contains("failed to open SDL3 gamepad 9"));
    assert!(open_error.contains("open failed"));
}
