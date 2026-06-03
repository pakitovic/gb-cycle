use super::*;

#[test]
fn gamepad_manager_pad_input_feeds_mbc7_accelerometer_from_right_stick() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let virtual_gamepad = VirtualGamepad::attach("Tilt Stick");
    subsystem.update();

    let mut machine = mbc7_machine();
    let mut input_state = FrontendInputState::new();
    let options = GamepadOptions {
        gyro_mode: GamepadGyroMode::PadInput,
        preferred_device: PreferredGamepadIdentity {
            path: None,
            name: Some("Tilt Stick".to_string()),
        },
        ..GamepadOptions::default()
    };
    let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
        .expect("gamepad manager");

    virtual_gamepad.set_axis(Axis::RightX, i16::MAX);
    virtual_gamepad.set_axis(Axis::RightY, i16::MIN);
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    assert_eq!(
        latched_mbc7_accelerometer(&mut machine),
        Mbc7AccelerometerInput::from_milli_g(1_000, -1_000)
    );

    virtual_gamepad.set_axis(Axis::RightX, 0);
    virtual_gamepad.set_axis(Axis::RightY, 0);
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    assert_eq!(
        latched_mbc7_accelerometer(&mut machine),
        Mbc7AccelerometerInput::neutral()
    );
}

#[test]
fn gamepad_manager_gyro_off_returns_mbc7_accelerometer_to_neutral() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let virtual_gamepad = VirtualGamepad::attach("Tilt Toggle");
    subsystem.update();

    let mut machine = mbc7_machine();
    let mut input_state = FrontendInputState::new();
    let options = GamepadOptions {
        gyro_mode: GamepadGyroMode::PadInput,
        preferred_device: PreferredGamepadIdentity {
            path: None,
            name: Some("Tilt Toggle".to_string()),
        },
        ..GamepadOptions::default()
    };
    let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
        .expect("gamepad manager");

    virtual_gamepad.set_axis(Axis::RightX, i16::MAX);
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    assert_ne!(
        latched_mbc7_accelerometer(&mut machine),
        Mbc7AccelerometerInput::neutral()
    );

    manager
        .set_gyro_mode(GamepadGyroMode::Off, &mut machine)
        .expect("gyro mode should change");
    assert_eq!(manager.gyro_mode(), GamepadGyroMode::Off);
    assert_eq!(
        latched_mbc7_accelerometer(&mut machine),
        Mbc7AccelerometerInput::neutral()
    );
}

#[test]
fn gamepad_manager_pad_gyro_auto_centers_virtual_accelerometer() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let virtual_gamepad = VirtualGamepad::attach_with_accelerometer("Motion Pad");
    subsystem.update();

    let mut machine = mbc7_machine();
    let mut input_state = FrontendInputState::new();
    let options = GamepadOptions {
        preferred_device: PreferredGamepadIdentity {
            path: None,
            name: Some("Motion Pad".to_string()),
        },
        ..GamepadOptions::default()
    };
    let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
        .expect("gamepad manager");
    assert!(manager.active_gamepad_has_accelerometer());

    manager
        .set_gyro_mode(GamepadGyroMode::PadGyro, &mut machine)
        .expect("PAD GYRO should enable virtual accelerometer");
    virtual_gamepad.set_accelerometer(
        2.0 * SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        -SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        0.0,
    );
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    assert_eq!(
        latched_mbc7_accelerometer(&mut machine),
        Mbc7AccelerometerInput::neutral()
    );
    assert!(manager.gyro.baseline.is_some());

    virtual_gamepad.set_accelerometer(
        3.0 * SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        -2.0 * SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        0.0,
    );
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    assert_eq!(
        latched_mbc7_accelerometer(&mut machine),
        Mbc7AccelerometerInput::from_milli_g(1_000, -1_000)
    );

    manager
        .set_gyro_mode(GamepadGyroMode::Off, &mut machine)
        .expect("gyro off should disable virtual accelerometer");
    manager
        .set_gyro_mode(GamepadGyroMode::PadGyro, &mut machine)
        .expect("gyro on should request a new baseline");
    assert!(manager.gyro.baseline.is_none());
}

#[test]
fn gamepad_manager_active_gamepad_change_resets_pad_gyro_baseline() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let first = VirtualGamepad::attach_with_accelerometer("First Motion");
    let second = VirtualGamepad::attach_with_accelerometer("Second Motion");
    subsystem.update();

    let mut machine = mbc7_machine();
    let mut input_state = FrontendInputState::new();
    let options = GamepadOptions {
        gyro_mode: GamepadGyroMode::PadGyro,
        ..GamepadOptions::default()
    };
    let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
        .expect("gamepad manager");

    first.set_accelerometer(SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED, 0.0, 0.0);
    second.set_accelerometer(0.0, SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED, 0.0);
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    assert!(manager.gyro.baseline.is_some());

    let next_active = if manager.is_active_gamepad(first.joystick_id) {
        second.joystick_id
    } else {
        first.joystick_id
    };
    assert!(manager.activate_gamepad_from_input(next_active, &mut input_state, &mut machine));
    assert_eq!(
        latched_mbc7_accelerometer(&mut machine),
        Mbc7AccelerometerInput::neutral()
    );
    assert!(manager.gyro.baseline.is_some());
}

#[test]
fn gamepad_manager_polls_virtual_gamepad_buttons_and_left_stick() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let virtual_gamepad = VirtualGamepad::attach("Player One");
    subsystem.update();

    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();
    let options = GamepadOptions {
        preferred_device: PreferredGamepadIdentity {
            path: None,
            name: Some("Player One".to_string()),
        },
        ..GamepadOptions::default()
    };
    let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
        .expect("gamepad manager");

    assert!(manager.has_connected_gamepad());
    assert_eq!(manager.active_gamepad_name(), Some("Player One"));
    assert_eq!(
        manager.active_gamepad_identity(),
        Some(PreferredGamepadIdentity {
            path: None,
            name: Some("Player One".to_string()),
        })
    );

    virtual_gamepad.set_button(Button::East, true);
    virtual_gamepad.set_button(Button::DPadLeft, true);
    virtual_gamepad.set_button(Button::Start, true);
    virtual_gamepad.set_axis(Axis::LeftY, -20_000);
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    ingest_host_input(&mut machine);

    assert_eq!(
        pressed_mask(&machine),
        joypad_mask(JoypadButton::A)
            | joypad_mask(JoypadButton::Left)
            | joypad_mask(JoypadButton::Up)
    );

    manager.set_directional_source(
        gb_desktop::GamepadDirectionalSource::DpadOnly,
        &mut input_state,
        &mut machine,
    );
    ingest_host_input(&mut machine);
    assert_eq!(
        manager.directional_source(),
        gb_desktop::GamepadDirectionalSource::DpadOnly
    );
    assert_eq!(
        pressed_mask(&machine),
        joypad_mask(JoypadButton::A) | joypad_mask(JoypadButton::Left)
    );

    let mut bindings = manager.button_bindings();
    bindings.a = GamepadButtonBinding::South;
    manager.set_button_bindings(bindings, &mut input_state, &mut machine);
    virtual_gamepad.set_button(Button::South, true);
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    ingest_host_input(&mut machine);
    assert!(pressed_mask(&machine) & joypad_mask(JoypadButton::A) != 0);

    let mut bindings = manager.button_bindings();
    bindings.b = GamepadButtonBinding::LeftTrigger;
    manager.set_button_bindings(bindings, &mut input_state, &mut machine);
    virtual_gamepad.set_axis(Axis::TriggerLeft, i16::MAX);
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    ingest_host_input(&mut machine);
    assert!(pressed_mask(&machine) & joypad_mask(JoypadButton::B) != 0);
    virtual_gamepad.set_axis(Axis::TriggerLeft, 0);
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    ingest_host_input(&mut machine);
    assert_eq!(pressed_mask(&machine) & joypad_mask(JoypadButton::B), 0);

    manager.set_menu_bindings(gb_desktop::GamepadMenuBindings {
        confirm: GamepadButtonBinding::North,
        ..manager.menu_bindings()
    });
    assert_eq!(manager.menu_bindings().confirm, GamepadButtonBinding::North);
}
