use super::*;

#[test]
fn gamepad_manager_rumble_helpers_cover_state_transitions() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();
    let virtual_gamepad = VirtualGamepad::attach("Rumble Pad");
    subsystem.update();

    let mut machine = test_machine();
    let mut input_state = FrontendInputState::new();
    let options = GamepadOptions {
        preferred_device: PreferredGamepadIdentity {
            path: None,
            name: Some("Rumble Pad".to_string()),
        },
        ..GamepadOptions::default()
    };
    let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
        .expect("gamepad manager");

    assert_eq!(manager.rumble_mode(), GamepadRumbleMode::Strong);
    assert!(!manager.active_gamepad_has_rumble());
    assert!(!manager.has_active_rumble_effect());
    assert!(!manager.can_deliver_rumble());

    manager.set_rumble_mode(GamepadRumbleMode::Weak);
    assert_eq!(manager.rumble_mode(), GamepadRumbleMode::Weak);
    manager
        .opened
        .get_mut(&virtual_gamepad.joystick_id)
        .expect("virtual gamepad should be opened")
        .supports_rumble = true;
    assert!(manager.active_gamepad_has_rumble());
    assert!(manager.can_deliver_rumble());

    let desired = manager
        .desired_rumble_effect()
        .expect("active rumble effect should be derived");
    assert_eq!(desired.joystick_id.0, virtual_gamepad.joystick_id.0);
    assert_eq!(
        (desired.low_frequency, desired.high_frequency),
        (WEAK_GAMEPAD_RUMBLE_INTENSITY, WEAK_GAMEPAD_RUMBLE_INTENSITY)
    );

    let now = Instant::now();
    let future_refresh = now + Duration::from_secs(1);
    manager.rumble.applied = Some(desired);
    manager.rumble.next_refresh_at = Some(future_refresh);
    manager
        .update_rumble(true, now)
        .expect("matching rumble state should be a no-op");
    let applied = manager
        .rumble
        .applied
        .expect("rumble state should remain applied");
    assert_eq!(applied.joystick_id.0, desired.joystick_id.0);
    assert_eq!(applied.low_frequency, desired.low_frequency);
    assert_eq!(applied.high_frequency, desired.high_frequency);
    assert_eq!(manager.rumble.next_refresh_at, Some(future_refresh));
    assert!(manager.has_active_rumble_effect());

    manager.rumble.applied = Some(AppliedGamepadRumble {
        joystick_id: joystick_id_from_event(9_999),
        low_frequency: 1,
        high_frequency: 2,
    });
    manager.rumble.next_refresh_at = Some(now);
    manager
        .update_rumble(false, now)
        .expect("clearing stale rumble should not require a live SDL gamepad");
    assert!(manager.rumble.applied.is_none());
    assert!(manager.rumble.next_refresh_at.is_none());
    assert!(!manager.has_active_rumble_effect());

    manager.set_rumble_mode(GamepadRumbleMode::Strong);
    let strong_effect = manager
        .desired_rumble_effect()
        .expect("strong rumble effect should be derived");
    assert_eq!(
        (strong_effect.low_frequency, strong_effect.high_frequency),
        (
            STRONG_GAMEPAD_RUMBLE_INTENSITY,
            STRONG_GAMEPAD_RUMBLE_INTENSITY
        )
    );
    let refresh_result =
        manager.update_rumble(true, future_refresh + GAMEPAD_RUMBLE_REFRESH_INTERVAL);
    if let Err(error) = refresh_result {
        assert!(error.contains("failed to set SDL3 gamepad rumble"));
    }

    manager
        .opened
        .get_mut(&virtual_gamepad.joystick_id)
        .expect("virtual gamepad should remain opened")
        .supports_rumble = false;
    assert!(!manager.active_gamepad_has_rumble());
    assert!(manager.desired_rumble_effect().is_none());
}

#[test]
fn gamepad_manager_without_active_gamepad_keeps_mbc7_accelerometer_neutral() {
    let _guard = crate::lock_sdl_test();
    let (_sdl, subsystem) = init_gamepad_subsystem();

    let mut machine = mbc7_machine();
    machine
        .set_mbc7_accelerometer_input(Mbc7AccelerometerInput::from_milli_g(1_000, 1_000))
        .expect("MBC7 accelerometer should be writable");
    let mut input_state = FrontendInputState::new();
    let options = GamepadOptions {
        gyro_mode: GamepadGyroMode::PadInput,
        ..GamepadOptions::default()
    };
    let mut manager = GamepadManager {
        subsystem,
        options,
        opened: BTreeMap::new(),
        active: None,
        left_stick_state: LeftStickDigitalState::default(),
        rumble: GamepadRumbleState::default(),
        gyro: GamepadGyroState::default(),
    };
    manager.sync_active_gamepad_state(&mut input_state, &mut machine);

    assert!(!manager.has_connected_gamepad());
    assert_eq!(
        latched_mbc7_accelerometer(&mut machine),
        Mbc7AccelerometerInput::neutral()
    );

    machine
        .set_mbc7_accelerometer_input(Mbc7AccelerometerInput::from_milli_g(-1_000, -1_000))
        .expect("MBC7 accelerometer should be writable");
    manager.poll_active_gamepad_state(&mut input_state, &mut machine);
    assert_eq!(
        latched_mbc7_accelerometer(&mut machine),
        Mbc7AccelerometerInput::neutral()
    );
}
