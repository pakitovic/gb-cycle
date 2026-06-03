use super::*;

#[test]
fn frontend_harness_covers_gamepad_event_paths() {
    let _guard = crate::lock_sdl_test();
    let virtual_gamepad = VirtualGamepad::attach("Runtime Pad");
    let mut harness = FrontendHarness::new("gamepad-events", true, false, true);
    harness
        ._gamepad_subsystem
        .as_ref()
        .expect("gamepad subsystem")
        .update();
    harness
        .runtime
        .gamepad_manager
        .as_mut()
        .expect("gamepad manager")
        .set_preferred_device(
            gb_desktop::PreferredGamepadIdentity {
                path: None,
                name: Some("Runtime Pad".to_string()),
            },
            harness
                .runtime
                .player_inputs
                .input_mut(super::super::PlayerSlot::P1),
            harness
                .machine
                .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        );
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .and_then(super::super::GamepadManager::active_gamepad_name),
        Some("Runtime Pad")
    );

    let events = harness
        .sdl
        .event()
        .expect("event subsystem should initialize for controller events");
    harness
        .runtime
        .menu_state
        .open(super::super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        ));
    harness
        .runtime
        .menu_state
        .begin_gamepad_binding_capture_for_tests(GamepadBindingTarget::A);
    events
        .push_event(Event::ControllerButtonDown {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            button: Button::North,
        })
        .expect("gamepad binding event should be pushable");
    harness
        .process_events()
        .expect("gamepad binding capture should process");
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad manager")
            .button_bindings()
            .a,
        GamepadButtonBinding::North
    );

    harness
        .runtime
        .menu_state
        .begin_gamepad_action_binding_capture_for_tests(GamepadActionBindingTarget::Rewind);
    events
        .push_event(Event::ControllerAxisMotion {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            axis: Axis::TriggerLeft,
            value: i16::MAX,
        })
        .expect("gamepad trigger capture event should be pushable");
    harness
        .process_events()
        .expect("gamepad trigger binding capture should process");
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad manager")
            .action_bindings()
            .rewind,
        Some(GamepadButtonBinding::LeftTrigger)
    );

    events
        .push_event(Event::ControllerButtonDown {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            button: Button::Guide,
        })
        .expect("guide event should be pushable");
    harness
        .process_events()
        .expect("guide button should close the menu");
    assert!(!harness.runtime.menu_state.is_open());

    events
        .push_event(Event::ControllerButtonDown {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            button: Button::Guide,
        })
        .expect("second guide event should be pushable");
    harness
        .process_events()
        .expect("guide button should open the menu");
    assert!(harness.runtime.menu_state.is_open());

    harness
        .runtime
        .menu_state
        .open(super::super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        ));
    events
        .push_event(Event::ControllerButtonDown {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            button: Button::Guide,
        })
        .expect("menu close event should be pushable");
    harness
        .process_events()
        .expect("gamepad menu close should process");
    assert!(!harness.runtime.menu_state.is_open());

    harness.runtime.rewind_gamepad_active = true;
    harness.runtime.fast_forward_gamepad_active = true;
    events
        .push_event(Event::ControllerDeviceRemoved {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
        })
        .expect("gamepad removal event should be pushable");
    harness
        .process_events()
        .expect("gamepad removal should clear hold latches");
    assert!(!harness.runtime.rewind_gamepad_active);
    assert!(!harness.runtime.fast_forward_gamepad_active);
}

#[test]
fn frontend_harness_routes_gamepad_trigger_axis_actions() {
    let _guard = crate::lock_sdl_test();
    let virtual_gamepad = VirtualGamepad::attach("Trigger Pad");
    let mut harness = FrontendHarness::new("gamepad-trigger-actions", true, false, true);
    harness
        ._gamepad_subsystem
        .as_ref()
        .expect("gamepad subsystem")
        .update();
    harness
        .runtime
        .gamepad_manager
        .as_mut()
        .expect("gamepad manager")
        .set_preferred_device(
            gb_desktop::PreferredGamepadIdentity {
                path: None,
                name: Some("Trigger Pad".to_string()),
            },
            harness
                .runtime
                .player_inputs
                .input_mut(super::super::PlayerSlot::P1),
            harness
                .machine
                .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        );
    harness
        .runtime
        .gamepad_manager
        .as_mut()
        .expect("gamepad manager")
        .set_action_bindings(gb_desktop::GamepadActionBindings {
            save_state: None,
            load_state: None,
            rewind: Some(GamepadButtonBinding::LeftTrigger),
            fast_forward: Some(GamepadButtonBinding::RightTrigger),
        });

    let events = harness
        .sdl
        .event()
        .expect("event subsystem should initialize for controller events");
    events
        .push_event(Event::ControllerAxisMotion {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            axis: Axis::TriggerLeft,
            value: i16::MAX,
        })
        .expect("left trigger press should be pushable");
    harness
        .process_events()
        .expect("left trigger press should process");
    assert!(harness.runtime.rewind_gamepad_active);
    assert!(!harness.runtime.fast_forward_gamepad_active);

    events
        .push_event(Event::ControllerAxisMotion {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            axis: Axis::TriggerRight,
            value: i16::MAX,
        })
        .expect("right trigger press should be pushable");
    harness
        .process_events()
        .expect("right trigger press should process");
    assert!(harness.runtime.rewind_gamepad_active);
    assert!(harness.runtime.fast_forward_gamepad_active);

    events
        .push_event(Event::ControllerAxisMotion {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            axis: Axis::TriggerLeft,
            value: 0,
        })
        .expect("left trigger release should be pushable");
    harness
        .process_events()
        .expect("left trigger release should process");
    assert!(!harness.runtime.rewind_gamepad_active);
    assert!(harness.runtime.fast_forward_gamepad_active);

    events
        .push_event(Event::ControllerAxisMotion {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            axis: Axis::TriggerRight,
            value: 0,
        })
        .expect("right trigger release should be pushable");
    harness
        .process_events()
        .expect("right trigger release should process");
    assert!(!harness.runtime.rewind_gamepad_active);
    assert!(!harness.runtime.fast_forward_gamepad_active);
}

#[test]
fn guide_button_keeps_the_launcher_open_without_a_loaded_rom() {
    let _guard = crate::lock_sdl_test();
    let virtual_gamepad = VirtualGamepad::attach("Launcher Pad");
    let mut harness = FrontendHarness::new("launcher-guide", false, false, true);
    harness
        ._gamepad_subsystem
        .as_ref()
        .expect("gamepad subsystem")
        .update();
    harness
        .runtime
        .gamepad_manager
        .as_mut()
        .expect("gamepad manager")
        .set_preferred_device(
            gb_desktop::PreferredGamepadIdentity {
                path: None,
                name: Some("Launcher Pad".to_string()),
            },
            harness
                .runtime
                .player_inputs
                .input_mut(super::super::PlayerSlot::P1),
            harness
                .machine
                .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        );

    let events = harness
        .sdl
        .event()
        .expect("event subsystem should initialize for controller events");
    harness
        .runtime
        .menu_state
        .open(super::super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        ));
    assert!(harness.runtime.menu_state.is_open());

    events
        .push_event(Event::ControllerButtonDown {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            button: Button::Guide,
        })
        .expect("guide event should be pushable");
    harness
        .process_events()
        .expect("guide button should leave the launcher open");
    assert!(harness.runtime.menu_state.is_open());
}

#[test]
fn guide_button_matches_keyboard_cancel_behavior_inside_submenus() {
    let _guard = crate::lock_sdl_test();
    let virtual_gamepad = VirtualGamepad::attach("Overlay Pad");
    let mut harness = FrontendHarness::new("guide-cancel", true, false, true);
    harness
        ._gamepad_subsystem
        .as_ref()
        .expect("gamepad subsystem")
        .update();
    harness
        .runtime
        .gamepad_manager
        .as_mut()
        .expect("gamepad manager")
        .set_preferred_device(
            gb_desktop::PreferredGamepadIdentity {
                path: None,
                name: Some("Overlay Pad".to_string()),
            },
            harness
                .runtime
                .player_inputs
                .input_mut(super::super::PlayerSlot::P1),
            harness
                .machine
                .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        );

    let events = harness
        .sdl
        .event()
        .expect("event subsystem should initialize for controller events");
    harness
        .runtime
        .menu_state
        .open(super::super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        ));

    for button in [
        Button::DPadDown,
        Button::DPadDown,
        Button::DPadDown,
        Button::DPadDown,
        Button::South,
    ] {
        events
            .push_event(Event::ControllerButtonDown {
                timestamp: 0,
                which: virtual_gamepad.joystick_id.0,
                button,
            })
            .expect("menu navigation event should be pushable");
        harness
            .process_events()
            .expect("menu navigation should process");
    }
    assert!(harness.runtime.menu_state.is_open());

    events
        .push_event(Event::ControllerButtonDown {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            button: Button::Guide,
        })
        .expect("guide event should be pushable");
    harness
        .process_events()
        .expect("guide button should back out of the submenu");
    assert!(harness.runtime.menu_state.is_open());

    events
        .push_event(Event::ControllerButtonDown {
            timestamp: 0,
            which: virtual_gamepad.joystick_id.0,
            button: Button::East,
        })
        .expect("cancel event should be pushable");
    harness
        .process_events()
        .expect("cancel button should close the root menu");
    assert!(!harness.runtime.menu_state.is_open());
}

#[test]
fn frontend_harness_covers_keyboard_binding_capture_paths() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("keyboard-capture", true, false, false);
    let events = harness
        .sdl
        .event()
        .expect("event subsystem should initialize for keyboard capture");

    harness
        .runtime
        .menu_state
        .open(super::super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        ));
    harness
        .runtime
        .menu_state
        .begin_keyboard_binding_capture_for_tests(KeyboardBindingTarget::A);
    push_key_event(&events, Keycode::Space, true);
    harness
        .process_events()
        .expect("joypad keyboard capture should process");
    assert_eq!(
        harness.runtime.keyboard_bindings.joypad.a,
        DesktopKey::Space
    );

    harness
        .runtime
        .menu_state
        .begin_keyboard_menu_binding_capture_for_tests(KeyboardMenuBindingTarget::Confirm);
    push_key_event(&events, Keycode::F5, true);
    harness
        .process_events()
        .expect("menu keyboard capture should process");
    assert_eq!(
        harness.runtime.keyboard_bindings.menu.confirm,
        DesktopKey::F5
    );

    harness
        .runtime
        .menu_state
        .begin_keyboard_binding_capture_for_tests(KeyboardBindingTarget::B);
    push_key_event(&events, Keycode::Escape, true);
    harness
        .process_events()
        .expect("escape should cancel the active keyboard capture");
    assert!(!harness.runtime.menu_state.is_capturing_binding());
    assert_eq!(
        harness.runtime.keyboard_bindings.joypad.b,
        DesktopKey::LeftAlt
    );

    harness
        .runtime
        .menu_state
        .begin_keyboard_binding_capture_for_tests(KeyboardBindingTarget::Start);
    events
        .push_event(Event::Quit { timestamp: 0 })
        .expect("quit event should be pushable during binding capture");
    assert!(matches!(
        harness
            .process_events()
            .expect("quit should short-circuit the binding capture loop"),
        super::super::LoopSignal::Quit
    ));
}

#[test]
fn frontend_harness_covers_presentation_fallbacks_and_missing_subsystems() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("fallbacks", false, false, false);
    harness.session.config.saves.directory_policy =
        gb_desktop::SaveDirectoryPolicy::Custom(harness.root.join("custom-saves"));
    let presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(!presentation.rom_loaded);
    assert_eq!(presentation.recent_rom_count, 0);
    assert!(!presentation.save_directory_uses_default_path);
    assert!(!presentation.audio_available);
    assert!(!presentation.manual_save_available);
    assert!(!presentation.gamepad_available);
    assert!(!presentation.active_gamepad_connected);
    assert!(presentation.active_gamepad_label.is_empty());
    assert!(!presentation.preferred_gamepad_configured);
    assert!(presentation.preferred_gamepad_label.is_empty());

    assert!(
        harness
            .execute_action(super::super::MenuAction::SaveBattery)
            .expect("save action should no-op without a session")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleMute)
            .expect("mute should no-op without audio")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleAudioVolume)
            .expect("volume cycling should still update the runtime setting")
            .is_none()
    );
    assert_eq!(harness.runtime.audio_volume_percent, 25);
    assert!(
        harness
            .execute_action(super::super::MenuAction::ResetAudioDefaults)
            .expect("audio reset should no-op without audio")
            .is_none()
    );
    assert_eq!(harness.runtime.audio_volume_percent, 100);
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleGamepadDirectionalSource)
            .expect("directional source should no-op without a gamepad manager")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleGamepadRumbleMode)
            .expect("rumble mode should no-op without a gamepad manager")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleGamepadGyroMode)
            .expect("gyro mode should no-op without a gamepad manager")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::TogglePreferredGamepad)
            .expect("preferred gamepad should no-op without a gamepad manager")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetGamepadBinding(
                GamepadBindingTarget::A,
                GamepadButtonBinding::South,
            ))
            .expect("gamepad bindings should no-op without a gamepad manager")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetGamepadMenuBinding(
                GamepadMenuBindingTarget::Confirm,
                GamepadButtonBinding::North,
            ))
            .expect("gamepad menu bindings should no-op without a gamepad manager")
            .is_none()
    );
    harness.runtime.menu_state.open(presentation);
    assert!(
        harness
            .execute_action(super::super::MenuAction::Reset)
            .expect("reset should close the menu even without a loaded ROM")
            .is_none()
    );
    assert!(!harness.runtime.menu_state.is_open());

    drop(harness);

    let mut gamepad_harness = FrontendHarness::new("saved-preferred", true, false, true);
    let preferred_device = gb_desktop::PreferredGamepadIdentity {
        path: Some("saved-path".to_string()),
        name: None,
    };
    gamepad_harness
        .runtime
        .gamepad_manager
        .as_mut()
        .expect("gamepad harness should have a manager")
        .set_preferred_device(
            preferred_device,
            gamepad_harness
                .runtime
                .player_inputs
                .input_mut(super::super::PlayerSlot::P1),
            gamepad_harness
                .machine
                .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        );
    let gamepad_presentation = super::super::current_menu_presentation(
        gamepad_harness.canvas.window(),
        &gamepad_harness.runtime,
        &gamepad_harness.machine,
        &gamepad_harness.session,
    );
    assert!(gamepad_presentation.gamepad_available);
    assert!(gamepad_presentation.preferred_gamepad_configured);
    assert_eq!(
        gamepad_presentation.gamepad_rumble_mode,
        GamepadRumbleMode::Strong
    );
    assert_eq!(gamepad_presentation.gamepad_gyro_mode, GamepadGyroMode::Off);
    assert_eq!(
        gamepad_presentation.preferred_gamepad_label.as_str(),
        "SAVED"
    );
    let manager = gamepad_harness
        .runtime
        .gamepad_manager
        .as_ref()
        .expect("gamepad harness should have a manager");
    assert_eq!(
        gamepad_presentation.active_gamepad_connected,
        manager.has_connected_gamepad()
    );
    assert_eq!(
        gamepad_presentation.active_gamepad_accelerometer_supported,
        manager.active_gamepad_has_accelerometer()
    );
    assert_eq!(
        gamepad_presentation.cartridge_mbc7_accelerometer_supported,
        gamepad_harness
            .machine
            .primary_machine()
            .has_mbc7_accelerometer()
    );
    if manager.has_connected_gamepad() {
        assert!(!gamepad_presentation.active_gamepad_label.is_empty());
    } else {
        assert!(gamepad_presentation.active_gamepad_label.is_empty());
    }
}
