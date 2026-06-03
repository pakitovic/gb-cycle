use super::*;

#[test]
fn gamepad_manager_respects_preferred_devices_and_handle_event_transitions() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let first = VirtualGamepad::attach("First Pad");
    let second = VirtualGamepad::attach("Second Pad");
    subsystem.update();

    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();
    let options = GamepadOptions {
        preferred_device: PreferredGamepadIdentity {
            path: None,
            name: Some("Second Pad".to_string()),
        },
        ..GamepadOptions::default()
    };
    let mut manager =
        GamepadManager::new(&subsystem, options, &mut input_state, &mut machine).expect("manager");

    assert_eq!(manager.active_gamepad_name(), Some("Second Pad"));
    assert_eq!(
        manager.preferred_device().name.as_deref(),
        Some("Second Pad")
    );
    assert!(manager.active_matches_preferred());
    assert_eq!(manager.preferred_device_name(), Some("Second Pad"));
    assert!(!manager.activate_gamepad_from_input(
        first.joystick_id,
        &mut input_state,
        &mut machine
    ));

    manager.set_preferred_device(
        PreferredGamepadIdentity::default(),
        &mut input_state,
        &mut machine,
    );
    assert!(!manager.active_matches_preferred());
    let activated_joystick = if manager.is_active_gamepad(first.joystick_id) {
        second.joystick_id
    } else {
        first.joystick_id
    };
    assert!(manager.activate_gamepad_from_input(
        activated_joystick,
        &mut input_state,
        &mut machine
    ));
    assert!(manager.is_active_gamepad(activated_joystick));

    let activated_gamepad = if activated_joystick == first.joystick_id {
        &first
    } else {
        &second
    };
    activated_gamepad.set_button(Button::East, true);
    subsystem.update();
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    ingest_host_input(&mut machine);
    assert!(pressed_mask(&machine) & joypad_mask(JoypadButton::A) != 0);

    manager
        .handle_event(
            &Event::ControllerDeviceRemapped {
                timestamp: 0,
                which: activated_joystick.0,
            },
            &mut input_state,
            &mut machine,
        )
        .expect("remap event");

    manager
        .handle_event(
            &Event::ControllerDeviceRemoved {
                timestamp: 0,
                which: activated_joystick.0,
            },
            &mut input_state,
            &mut machine,
        )
        .expect("remove event");
    ingest_host_input(&mut machine);
    assert!(!manager.is_active_gamepad(activated_joystick));
    assert!(manager.has_connected_gamepad());
    assert_eq!(pressed_mask(&machine), 0);
}

#[test]
fn gamepad_manager_can_open_new_virtual_devices_from_added_events() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();
    let options = GamepadOptions {
        preferred_device: PreferredGamepadIdentity {
            path: None,
            name: Some("Hot Plugged".to_string()),
        },
        ..GamepadOptions::default()
    };
    let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
        .expect("gamepad manager");

    let added = VirtualGamepad::attach("Hot Plugged");
    subsystem.update();
    manager
        .handle_event(
            &Event::ControllerDeviceAdded {
                timestamp: 0,
                which: added.joystick_id.0,
            },
            &mut input_state,
            &mut machine,
        )
        .expect("added event");

    assert!(manager.has_connected_gamepad());
    assert_eq!(manager.active_gamepad_name(), Some("Hot Plugged"));
    assert!(manager.active_matches_preferred());
}

#[test]
fn gamepad_manager_added_event_can_keep_the_existing_active_device() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let first = VirtualGamepad::attach("First Pad");
    subsystem.update();

    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();
    let mut manager = GamepadManager::new(
        &subsystem,
        GamepadOptions::default(),
        &mut input_state,
        &mut machine,
    )
    .expect("gamepad manager");
    let active_before = manager.active.expect("active SDL gamepad");
    let active_name_before = manager.active_gamepad_name().map(str::to_owned);

    let second = VirtualGamepad::attach("Second Pad");
    subsystem.update();
    manager
        .handle_event(
            &Event::ControllerDeviceAdded {
                timestamp: 0,
                which: second.joystick_id.0,
            },
            &mut input_state,
            &mut machine,
        )
        .expect("added event");

    assert!(manager.has_connected_gamepad());
    assert!(manager.is_active_gamepad(active_before));
    assert_eq!(manager.active_gamepad_name(), active_name_before.as_deref());
    assert!(manager.opened.contains_key(&first.joystick_id));
    assert!(manager.opened.contains_key(&second.joystick_id));
}

#[test]
fn gamepad_manager_remove_unknown_device_keeps_the_active_gamepad() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let first = VirtualGamepad::attach("First Pad");
    subsystem.update();

    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();
    let mut manager = GamepadManager::new(
        &subsystem,
        GamepadOptions::default(),
        &mut input_state,
        &mut machine,
    )
    .expect("gamepad manager");

    let active_before = manager.active.expect("active SDL gamepad");
    let active_name_before = manager.active_gamepad_name().map(str::to_owned);
    let unknown_joystick_id = (1..=10_000)
        .map(joystick_id_from_event)
        .find(|joystick_id| !manager.opened.contains_key(joystick_id))
        .expect("unused SDL joystick id");

    manager
        .handle_event(
            &Event::ControllerDeviceRemoved {
                timestamp: 0,
                which: unknown_joystick_id.0,
            },
            &mut input_state,
            &mut machine,
        )
        .expect("remove event");

    assert!(manager.has_connected_gamepad());
    assert!(manager.is_active_gamepad(active_before));
    assert_eq!(manager.active_gamepad_name(), active_name_before.as_deref());
    assert!(manager.opened.contains_key(&first.joystick_id));
}

#[test]
fn gamepad_manager_can_match_a_preferred_device_by_path() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let first = VirtualGamepad::attach("Path Pad");
    subsystem.update();

    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();
    let mut manager = GamepadManager::new(
        &subsystem,
        GamepadOptions::default(),
        &mut input_state,
        &mut machine,
    )
    .expect("gamepad manager");
    manager
        .opened
        .get_mut(&first.joystick_id)
        .expect("virtual gamepad should be opened")
        .path = Some("/dev/input/path-pad".to_string());

    manager.set_preferred_device(
        PreferredGamepadIdentity {
            path: Some("/dev/input/path-pad".to_string()),
            name: None,
        },
        &mut input_state,
        &mut machine,
    );

    assert!(manager.active_matches_preferred());
    assert_eq!(
        manager.preferred_device().path.as_deref(),
        Some("/dev/input/path-pad")
    );
}
