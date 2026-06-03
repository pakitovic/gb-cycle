use super::*;

#[test]
fn recent_rom_labels_compact_the_stem_for_the_overlay_width() {
    assert_eq!(
        compact_recent_rom_label(Path::new(
            "/tmp/roms/Super Mario Land 2 - 6 Golden Coins (USA, Europe) (Rev 2).gb"
        ))
        .as_str(),
        "SUPER MARIO LAND 2 6 GOLDEN COINS"
    );
}

#[test]
fn menu_keyboard_input_uses_dedicated_menu_bindings() {
    let config = DesktopConfig::default();

    assert_eq!(
        menu_input_for_key(config.input.keyboard.menu, Keycode::Up),
        Some(super::super::MenuInput::Up)
    );
    assert_eq!(
        menu_input_for_key(config.input.keyboard.menu, Keycode::LGui),
        Some(super::super::MenuInput::Confirm)
    );
    assert_eq!(
        menu_input_for_key(config.input.keyboard.menu, Keycode::LAlt),
        Some(super::super::MenuInput::Cancel)
    );
    assert_eq!(
        menu_input_for_key(config.input.keyboard.menu, Keycode::Escape),
        Some(super::super::MenuInput::Cancel)
    );
    assert_eq!(
        menu_input_for_key(config.input.keyboard.menu, Keycode::X),
        None
    );
    assert_eq!(
        menu_input_for_key(config.input.keyboard.menu, Keycode::Backspace),
        None
    );
}

#[test]
fn menu_keyboard_input_tracks_remapped_menu_bindings() {
    let bindings = MenuKeyboardBindings {
        confirm: DesktopKey::Space,
        cancel: DesktopKey::Return,
        ..MenuKeyboardBindings::default()
    };

    assert_eq!(
        menu_input_for_key(bindings, Keycode::Space),
        Some(super::super::MenuInput::Confirm)
    );
    assert_eq!(
        menu_input_for_key(bindings, Keycode::Return),
        Some(super::super::MenuInput::Cancel)
    );
    assert_eq!(
        menu_input_for_key(bindings, Keycode::Escape),
        Some(super::super::MenuInput::Cancel)
    );
}

#[test]
fn menu_gamepad_input_tracks_remapped_menu_bindings() {
    let bindings = GamepadMenuBindings {
        confirm: GamepadButtonBinding::North,
        cancel: GamepadButtonBinding::West,
        ..GamepadMenuBindings::default()
    };

    assert_eq!(
        menu_input_for_gamepad_button(bindings, Button::North),
        Some(super::super::MenuInput::Confirm)
    );
    assert_eq!(
        menu_input_for_gamepad_button(bindings, Button::West),
        Some(super::super::MenuInput::Cancel)
    );
    assert_eq!(menu_input_for_gamepad_button(bindings, Button::East), None);
}

#[test]
fn keyboard_binding_assignment_swaps_existing_keys_instead_of_creating_duplicates() {
    let mut bindings = DesktopConfig::default().input.keyboard;
    assign_keyboard_binding(&mut bindings, KeyboardBindingTarget::A, DesktopKey::LeftAlt);

    assert_eq!(bindings.joypad.a, DesktopKey::LeftAlt);
    assert_eq!(bindings.joypad.b, DesktopKey::LeftGui);
    assert_eq!(
        joypad_binding_target_for_key(bindings.joypad, DesktopKey::LeftAlt),
        Some(KeyboardBindingTarget::A)
    );
    assert_eq!(
        joypad_binding_target_for_key(bindings.joypad, DesktopKey::LeftGui),
        Some(KeyboardBindingTarget::B)
    );
}

#[test]
fn hotkey_binding_assignment_swaps_existing_keys_without_touching_joypad_bindings() {
    let mut bindings = DesktopConfig::default().input.keyboard;
    let original_a = bindings.joypad.a;

    assign_keyboard_binding(&mut bindings, KeyboardBindingTarget::Pause, DesktopKey::R);

    assert_eq!(bindings.hotkeys.pause, DesktopKey::R);
    assert_eq!(bindings.hotkeys.reset, DesktopKey::F12);
    assert_eq!(bindings.joypad.a, original_a);
    assert_eq!(
        hotkey_binding_target_for_key(bindings.hotkeys, DesktopKey::R),
        Some(KeyboardBindingTarget::Pause)
    );
    assert_eq!(
        hotkey_binding_target_for_key(bindings.hotkeys, DesktopKey::F12),
        Some(KeyboardBindingTarget::Reset)
    );
    assert_eq!(
        hotkey_binding_target_for_key(bindings.hotkeys, DesktopKey::F1),
        Some(KeyboardBindingTarget::SaveState)
    );
    assign_keyboard_binding(
        &mut bindings,
        KeyboardBindingTarget::FastForward,
        DesktopKey::LeftShift,
    );
    assert_eq!(bindings.hotkeys.fast_forward, DesktopKey::LeftShift);
    assert_eq!(bindings.hotkeys.rewind, DesktopKey::RightShift);
}

#[test]
fn gamepad_binding_assignment_swaps_existing_buttons_instead_of_creating_duplicates() {
    let mut bindings = DesktopConfig::default().input.gamepad.bindings;
    assign_gamepad_binding(
        &mut bindings,
        GamepadBindingTarget::A,
        GamepadButtonBinding::South,
    );

    assert_eq!(bindings.a, GamepadButtonBinding::South);
    assert_eq!(bindings.b, GamepadButtonBinding::East);
    assert_eq!(
        gamepad_binding_target_for_binding(bindings, GamepadButtonBinding::South),
        Some(GamepadBindingTarget::A)
    );
    assert_eq!(
        gamepad_binding_target_for_binding(bindings, GamepadButtonBinding::East),
        Some(GamepadBindingTarget::B)
    );
}

#[test]
fn gamepad_action_binding_assignment_swaps_existing_buttons_instead_of_creating_duplicates() {
    let mut bindings = gb_desktop::GamepadActionBindings {
        save_state: Some(GamepadButtonBinding::LeftShoulder),
        load_state: Some(GamepadButtonBinding::RightShoulder),
        rewind: Some(GamepadButtonBinding::Back),
        fast_forward: Some(GamepadButtonBinding::Start),
    };
    assign_gamepad_action_binding(
        &mut bindings,
        GamepadActionBindingTarget::FastForward,
        GamepadButtonBinding::Back,
    );

    assert_eq!(
        bindings.save_state,
        Some(GamepadButtonBinding::LeftShoulder)
    );
    assert_eq!(
        bindings.load_state,
        Some(GamepadButtonBinding::RightShoulder)
    );
    assert_eq!(bindings.rewind, Some(GamepadButtonBinding::Start));
    assert_eq!(bindings.fast_forward, Some(GamepadButtonBinding::Back));
    assert_eq!(
        gamepad_action_binding_target_for_binding(bindings, GamepadButtonBinding::Back),
        Some(GamepadActionBindingTarget::FastForward)
    );
    assert_eq!(
        gamepad_action_for_button(bindings, Button::Back),
        HotkeyAction::FastForward
    );
    assert_eq!(
        gamepad_action_for_button(bindings, Button::Start),
        HotkeyAction::Rewind
    );
    assert_eq!(
        gamepad_action_for_button(bindings, Button::LeftShoulder),
        HotkeyAction::SaveState
    );
    assert_eq!(
        gamepad_action_for_button(bindings, Button::RightShoulder),
        HotkeyAction::LoadState
    );
    assert_eq!(
        gamepad_action_for_button(bindings, Button::South),
        HotkeyAction::None
    );
    assert_eq!(
        gamepad_action_for_button(bindings, Button::Misc2),
        HotkeyAction::None
    );

    bindings.rewind = Some(GamepadButtonBinding::LeftTrigger);
    bindings.fast_forward = Some(GamepadButtonBinding::RightTrigger);
    assert_eq!(
        gamepad_action_for_binding(bindings, GamepadButtonBinding::LeftTrigger),
        HotkeyAction::Rewind
    );
    assert_eq!(
        gamepad_action_for_binding(bindings, GamepadButtonBinding::RightTrigger),
        HotkeyAction::FastForward
    );
}

#[test]
fn keyboard_menu_binding_assignment_swaps_existing_keys_instead_of_creating_duplicates() {
    let mut bindings = MenuKeyboardBindings::default();
    assign_keyboard_menu_binding(
        &mut bindings,
        KeyboardMenuBindingTarget::Confirm,
        DesktopKey::LeftAlt,
    );

    assert_eq!(bindings.confirm, DesktopKey::LeftAlt);
    assert_eq!(bindings.cancel, DesktopKey::LeftGui);
    assert_eq!(
        keyboard_menu_binding_target_for_key(bindings, DesktopKey::LeftAlt),
        Some(KeyboardMenuBindingTarget::Confirm)
    );
    assert_eq!(
        keyboard_menu_binding_target_for_key(bindings, DesktopKey::LeftGui),
        Some(KeyboardMenuBindingTarget::Cancel)
    );
}

#[test]
fn gamepad_menu_binding_assignment_swaps_existing_buttons_instead_of_creating_duplicates() {
    let mut bindings = GamepadMenuBindings::default();
    assign_gamepad_menu_binding(
        &mut bindings,
        GamepadMenuBindingTarget::Confirm,
        GamepadButtonBinding::East,
    );

    assert_eq!(bindings.confirm, GamepadButtonBinding::East);
    assert_eq!(bindings.cancel, GamepadButtonBinding::South);
    assert_eq!(
        gamepad_menu_binding_target_for_binding(bindings, GamepadButtonBinding::East),
        Some(GamepadMenuBindingTarget::Confirm)
    );
    assert_eq!(
        gamepad_menu_binding_target_for_binding(bindings, GamepadButtonBinding::South),
        Some(GamepadMenuBindingTarget::Cancel)
    );
}

#[test]
fn joypad_key_capture_rejects_hotkey_only_function_keys() {
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::F1, KeyboardBindingTarget::A),
        None
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::F5, KeyboardBindingTarget::A),
        None
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::F6, KeyboardBindingTarget::A),
        None
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::F12, KeyboardBindingTarget::A),
        None
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::_1, KeyboardBindingTarget::A),
        None
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::F11, KeyboardBindingTarget::Start),
        None
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::Space, KeyboardBindingTarget::B),
        Some(DesktopKey::Space)
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::Tab, KeyboardBindingTarget::A),
        Some(DesktopKey::Tab)
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(
            Keycode::LShift,
            KeyboardBindingTarget::Select
        ),
        Some(DesktopKey::LeftShift)
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(
            Keycode::RShift,
            KeyboardBindingTarget::Select
        ),
        Some(DesktopKey::RightShift)
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(
            Keycode::LCtrl,
            KeyboardBindingTarget::Start
        ),
        Some(DesktopKey::LeftControl)
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::RAlt, KeyboardBindingTarget::Start),
        Some(DesktopKey::RightAlt)
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::LGui, KeyboardBindingTarget::B),
        Some(DesktopKey::LeftGui)
    );
}

#[test]
fn hotkey_key_capture_accepts_function_keys() {
    let function_keys = [
        (Keycode::F1, DesktopKey::F1),
        (Keycode::F2, DesktopKey::F2),
        (Keycode::F3, DesktopKey::F3),
        (Keycode::F4, DesktopKey::F4),
        (Keycode::F5, DesktopKey::F5),
        (Keycode::F6, DesktopKey::F6),
        (Keycode::F7, DesktopKey::F7),
        (Keycode::F8, DesktopKey::F8),
        (Keycode::F9, DesktopKey::F9),
        (Keycode::F10, DesktopKey::F10),
        (Keycode::F11, DesktopKey::F11),
        (Keycode::F12, DesktopKey::F12),
    ];
    for (keycode, key) in function_keys {
        assert_eq!(
            assignable_key_for_binding_target_from_keycode(
                keycode,
                KeyboardBindingTarget::SaveBattery
            ),
            Some(key)
        );
    }
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(
            Keycode::_1,
            KeyboardBindingTarget::StateSlot1
        ),
        Some(DesktopKey::Digit1)
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(
            Keycode::RCtrl,
            KeyboardBindingTarget::Reset
        ),
        Some(DesktopKey::RightControl)
    );
    assert_eq!(
        assignable_key_for_binding_target_from_keycode(Keycode::RGui, KeyboardBindingTarget::Pause),
        Some(DesktopKey::RightGui)
    );
}

#[test]
fn menu_key_capture_restricts_escape_to_cancel_bindings() {
    assert_eq!(
        assignable_menu_key_for_binding_target_from_keycode(
            Keycode::Escape,
            KeyboardMenuBindingTarget::Confirm
        ),
        None
    );
    assert_eq!(
        assignable_menu_key_for_binding_target_from_keycode(
            Keycode::Escape,
            KeyboardMenuBindingTarget::Cancel
        ),
        Some(DesktopKey::Escape)
    );
    assert_eq!(
        assignable_menu_key_for_binding_target_from_keycode(
            Keycode::Space,
            KeyboardMenuBindingTarget::Confirm
        ),
        Some(DesktopKey::Space)
    );
    assert_eq!(
        assignable_menu_key_for_binding_target_from_keycode(
            Keycode::Tab,
            KeyboardMenuBindingTarget::Confirm
        ),
        Some(DesktopKey::Tab)
    );
    assert_eq!(
        assignable_menu_key_for_binding_target_from_keycode(
            Keycode::LAlt,
            KeyboardMenuBindingTarget::Down
        ),
        Some(DesktopKey::LeftAlt)
    );
}

#[test]
fn gamepad_directional_source_cycles_through_the_three_supported_modes() {
    assert_eq!(
        next_gamepad_directional_source(GamepadDirectionalSource::DpadOnly),
        GamepadDirectionalSource::LeftStickOnly
    );
    assert_eq!(
        next_gamepad_directional_source(GamepadDirectionalSource::LeftStickOnly),
        GamepadDirectionalSource::DpadAndLeftStick
    );
    assert_eq!(
        next_gamepad_directional_source(GamepadDirectionalSource::DpadAndLeftStick),
        GamepadDirectionalSource::DpadOnly
    );
}

#[test]
fn window_scale_cycles_through_the_supported_overlay_values() {
    assert_eq!(next_window_scale(0), 1);
    assert_eq!(next_window_scale(1), 2);
    assert_eq!(next_window_scale(7), 8);
    assert_eq!(next_window_scale(8), 1);
}

#[test]
fn audio_volume_cycles_in_quarter_steps() {
    assert_eq!(next_audio_volume_percent(0), 25);
    assert_eq!(next_audio_volume_percent(25), 50);
    assert_eq!(next_audio_volume_percent(50), 75);
    assert_eq!(next_audio_volume_percent(75), 100);
    assert_eq!(next_audio_volume_percent(100), 25);
}

#[test]
fn binding_value_helpers_cover_all_frontend_targets() {
    let mut keyboard = gb_desktop::KeyboardBindings::default();
    let keyboard_targets = [
        (KeyboardBindingTarget::Up, DesktopKey::Escape),
        (KeyboardBindingTarget::Down, DesktopKey::ArrowUp),
        (KeyboardBindingTarget::Left, DesktopKey::ArrowDown),
        (KeyboardBindingTarget::Right, DesktopKey::ArrowLeft),
        (KeyboardBindingTarget::A, DesktopKey::ArrowRight),
        (KeyboardBindingTarget::B, DesktopKey::Backspace),
        (KeyboardBindingTarget::Select, DesktopKey::Return),
        (KeyboardBindingTarget::Start, DesktopKey::Space),
        (KeyboardBindingTarget::Pause, DesktopKey::R),
        (KeyboardBindingTarget::SaveState, DesktopKey::F1),
        (KeyboardBindingTarget::LoadState, DesktopKey::F2),
        (KeyboardBindingTarget::StateSlot1, DesktopKey::Digit1),
        (KeyboardBindingTarget::StateSlot2, DesktopKey::Digit2),
        (KeyboardBindingTarget::StateSlot3, DesktopKey::Digit3),
        (KeyboardBindingTarget::StateSlot4, DesktopKey::Digit4),
        (KeyboardBindingTarget::Reset, DesktopKey::F12),
        (KeyboardBindingTarget::Rewind, DesktopKey::LeftShift),
        (KeyboardBindingTarget::FastForward, DesktopKey::RightShift),
        (KeyboardBindingTarget::ToggleFullscreen, DesktopKey::Z),
        (KeyboardBindingTarget::TogglePerformanceHud, DesktopKey::F10),
        (KeyboardBindingTarget::SaveBattery, DesktopKey::F9),
    ];
    for (target, key) in keyboard_targets {
        super::super::set_keyboard_binding_value(&mut keyboard, target, key);
        assert_eq!(super::super::keyboard_binding_value(keyboard, target), key);
    }
    let keyboard_before = keyboard;
    assign_keyboard_binding(
        &mut keyboard,
        KeyboardBindingTarget::SaveBattery,
        keyboard_before.hotkeys.save_battery,
    );
    assert_eq!(keyboard, keyboard_before);

    let mut keyboard_menu = MenuKeyboardBindings::default();
    let keyboard_menu_targets = [
        (KeyboardMenuBindingTarget::Up, DesktopKey::Backspace),
        (KeyboardMenuBindingTarget::Down, DesktopKey::Return),
        (KeyboardMenuBindingTarget::Confirm, DesktopKey::Space),
        (KeyboardMenuBindingTarget::Cancel, DesktopKey::Escape),
    ];
    for (target, key) in keyboard_menu_targets {
        super::super::set_keyboard_menu_binding_value(&mut keyboard_menu, target, key);
        assert_eq!(
            super::super::keyboard_menu_binding_value(keyboard_menu, target),
            key
        );
    }
    let keyboard_menu_before = keyboard_menu;
    assign_keyboard_menu_binding(
        &mut keyboard_menu,
        KeyboardMenuBindingTarget::Cancel,
        keyboard_menu_before.cancel,
    );
    assert_eq!(keyboard_menu, keyboard_menu_before);

    let mut gamepad = gb_desktop::GamepadButtonBindings::default();
    let gamepad_targets = [
        (GamepadBindingTarget::Up, GamepadButtonBinding::South),
        (GamepadBindingTarget::Down, GamepadButtonBinding::East),
        (GamepadBindingTarget::Left, GamepadButtonBinding::West),
        (GamepadBindingTarget::Right, GamepadButtonBinding::North),
        (GamepadBindingTarget::A, GamepadButtonBinding::Back),
        (GamepadBindingTarget::B, GamepadButtonBinding::Start),
        (GamepadBindingTarget::Select, GamepadButtonBinding::Guide),
        (
            GamepadBindingTarget::Start,
            GamepadButtonBinding::LeftShoulder,
        ),
    ];
    for (target, binding) in gamepad_targets {
        super::super::set_gamepad_binding_value(&mut gamepad, target, binding);
        assert_eq!(
            super::super::gamepad_binding_value(gamepad, target),
            binding
        );
        assert_eq!(
            gamepad_binding_target_for_binding(gamepad, binding),
            Some(target)
        );
    }
    let gamepad_before = gamepad;
    assign_gamepad_binding(
        &mut gamepad,
        GamepadBindingTarget::Start,
        gamepad_before.start,
    );
    assert_eq!(gamepad, gamepad_before);

    let mut gamepad_actions = gb_desktop::GamepadActionBindings::default();
    let gamepad_action_targets = [
        (
            GamepadActionBindingTarget::SaveState,
            GamepadButtonBinding::LeftShoulder,
        ),
        (
            GamepadActionBindingTarget::LoadState,
            GamepadButtonBinding::RightShoulder,
        ),
        (
            GamepadActionBindingTarget::Rewind,
            GamepadButtonBinding::Back,
        ),
        (
            GamepadActionBindingTarget::FastForward,
            GamepadButtonBinding::Start,
        ),
    ];
    for (target, binding) in gamepad_action_targets {
        super::super::set_gamepad_action_binding_value(&mut gamepad_actions, target, Some(binding));
        assert_eq!(
            super::super::gamepad_action_binding_value(gamepad_actions, target),
            Some(binding)
        );
        assert_eq!(
            gamepad_action_binding_target_for_binding(gamepad_actions, binding),
            Some(target)
        );
    }
    let gamepad_actions_before = gamepad_actions;
    assign_gamepad_action_binding(
        &mut gamepad_actions,
        GamepadActionBindingTarget::FastForward,
        gamepad_actions_before
            .fast_forward
            .expect("binding is configured"),
    );
    assert_eq!(gamepad_actions, gamepad_actions_before);

    let mut gamepad_menu = GamepadMenuBindings::default();
    let gamepad_menu_targets = [
        (GamepadMenuBindingTarget::Up, GamepadButtonBinding::DPadUp),
        (
            GamepadMenuBindingTarget::Down,
            GamepadButtonBinding::DPadDown,
        ),
        (
            GamepadMenuBindingTarget::Confirm,
            GamepadButtonBinding::DPadLeft,
        ),
        (
            GamepadMenuBindingTarget::Cancel,
            GamepadButtonBinding::DPadRight,
        ),
    ];
    for (target, binding) in gamepad_menu_targets {
        super::super::set_gamepad_menu_binding_value(&mut gamepad_menu, target, binding);
        assert_eq!(
            super::super::gamepad_menu_binding_value(gamepad_menu, target),
            binding
        );
        assert_eq!(
            gamepad_menu_binding_target_for_binding(gamepad_menu, binding),
            Some(target)
        );
    }
    let gamepad_menu_before = gamepad_menu;
    assign_gamepad_menu_binding(
        &mut gamepad_menu,
        GamepadMenuBindingTarget::Cancel,
        gamepad_menu_before.cancel,
    );
    assert_eq!(gamepad_menu, gamepad_menu_before);
}

#[test]
fn key_and_button_mapping_helpers_cover_all_variants_and_fallbacks() {
    let key_pairs = [
        (
            DesktopKey::Escape,
            Keycode::Escape,
            sdl3::keyboard::Scancode::Escape,
        ),
        (
            DesktopKey::ArrowUp,
            Keycode::Up,
            sdl3::keyboard::Scancode::Up,
        ),
        (
            DesktopKey::ArrowDown,
            Keycode::Down,
            sdl3::keyboard::Scancode::Down,
        ),
        (
            DesktopKey::ArrowLeft,
            Keycode::Left,
            sdl3::keyboard::Scancode::Left,
        ),
        (
            DesktopKey::ArrowRight,
            Keycode::Right,
            sdl3::keyboard::Scancode::Right,
        ),
        (
            DesktopKey::Backspace,
            Keycode::Backspace,
            sdl3::keyboard::Scancode::Backspace,
        ),
        (
            DesktopKey::Return,
            Keycode::Return,
            sdl3::keyboard::Scancode::Return,
        ),
        (
            DesktopKey::Space,
            Keycode::Space,
            sdl3::keyboard::Scancode::Space,
        ),
        (DesktopKey::R, Keycode::R, sdl3::keyboard::Scancode::R),
        (DesktopKey::X, Keycode::X, sdl3::keyboard::Scancode::X),
        (DesktopKey::Z, Keycode::Z, sdl3::keyboard::Scancode::Z),
        (
            DesktopKey::Digit1,
            Keycode::_1,
            sdl3::keyboard::Scancode::_1,
        ),
        (
            DesktopKey::Digit2,
            Keycode::_2,
            sdl3::keyboard::Scancode::_2,
        ),
        (
            DesktopKey::Digit3,
            Keycode::_3,
            sdl3::keyboard::Scancode::_3,
        ),
        (
            DesktopKey::Digit4,
            Keycode::_4,
            sdl3::keyboard::Scancode::_4,
        ),
        (DesktopKey::F1, Keycode::F1, sdl3::keyboard::Scancode::F1),
        (DesktopKey::F2, Keycode::F2, sdl3::keyboard::Scancode::F2),
        (DesktopKey::F3, Keycode::F3, sdl3::keyboard::Scancode::F3),
        (DesktopKey::F4, Keycode::F4, sdl3::keyboard::Scancode::F4),
        (DesktopKey::F5, Keycode::F5, sdl3::keyboard::Scancode::F5),
        (DesktopKey::F6, Keycode::F6, sdl3::keyboard::Scancode::F6),
        (DesktopKey::F7, Keycode::F7, sdl3::keyboard::Scancode::F7),
        (DesktopKey::F8, Keycode::F8, sdl3::keyboard::Scancode::F8),
        (DesktopKey::F9, Keycode::F9, sdl3::keyboard::Scancode::F9),
        (DesktopKey::F10, Keycode::F10, sdl3::keyboard::Scancode::F10),
        (DesktopKey::F11, Keycode::F11, sdl3::keyboard::Scancode::F11),
        (DesktopKey::F12, Keycode::F12, sdl3::keyboard::Scancode::F12),
        (DesktopKey::Tab, Keycode::Tab, sdl3::keyboard::Scancode::Tab),
        (
            DesktopKey::LeftShift,
            Keycode::LShift,
            sdl3::keyboard::Scancode::LShift,
        ),
        (
            DesktopKey::RightShift,
            Keycode::RShift,
            sdl3::keyboard::Scancode::RShift,
        ),
        (
            DesktopKey::LeftControl,
            Keycode::LCtrl,
            sdl3::keyboard::Scancode::LCtrl,
        ),
        (
            DesktopKey::RightControl,
            Keycode::RCtrl,
            sdl3::keyboard::Scancode::RCtrl,
        ),
        (
            DesktopKey::LeftAlt,
            Keycode::LAlt,
            sdl3::keyboard::Scancode::LAlt,
        ),
        (
            DesktopKey::RightAlt,
            Keycode::RAlt,
            sdl3::keyboard::Scancode::RAlt,
        ),
        (
            DesktopKey::LeftGui,
            Keycode::LGui,
            sdl3::keyboard::Scancode::LGui,
        ),
        (
            DesktopKey::RightGui,
            Keycode::RGui,
            sdl3::keyboard::Scancode::RGui,
        ),
    ];
    for (desktop_key, keycode, scancode) in key_pairs {
        assert_eq!(desktop_key_scancode(desktop_key), scancode);
        assert_eq!(desktop_key_from_keycode(keycode), Some(desktop_key));
        assert_eq!(desktop_key_from_scancode(scancode), Some(desktop_key));
        assert!(super::super::key_matches(desktop_key, keycode));
        assert!(super::super::key_event_matches(
            desktop_key,
            Some(keycode),
            Some(scancode)
        ));
    }
    assert_eq!(desktop_key_from_keycode(Keycode::A), None);
    assert_eq!(desktop_key_from_scancode(sdl3::keyboard::Scancode::A), None);
    assert_eq!(
        desktop_key_from_key_event(Some(Keycode::X), Some(sdl3::keyboard::Scancode::Z)),
        Some(DesktopKey::Z)
    );
    assert_eq!(
        desktop_key_from_key_event(
            Some(Keycode::RShift),
            Some(sdl3::keyboard::Scancode::LShift)
        ),
        Some(DesktopKey::RightShift)
    );
    for (keycode, expected) in [
        (Keycode::LShift, DesktopKey::LeftShift),
        (Keycode::RShift, DesktopKey::RightShift),
        (Keycode::LCtrl, DesktopKey::LeftControl),
        (Keycode::RCtrl, DesktopKey::RightControl),
        (Keycode::LAlt, DesktopKey::LeftAlt),
        (Keycode::RAlt, DesktopKey::RightAlt),
        (Keycode::LGui, DesktopKey::LeftGui),
        (Keycode::RGui, DesktopKey::RightGui),
    ] {
        assert_eq!(
            desktop_key_from_key_event(Some(keycode), Some(sdl3::keyboard::Scancode::Z)),
            Some(expected)
        );
    }
    assert_eq!(
        desktop_key_from_key_event(Some(Keycode::Z), Some(sdl3::keyboard::Scancode::A)),
        None
    );
    assert!(super::super::key_event_matches(
        DesktopKey::Z,
        Some(Keycode::X),
        Some(sdl3::keyboard::Scancode::Z)
    ));
    assert!(super::super::key_event_matches(
        DesktopKey::RightShift,
        Some(Keycode::RShift),
        Some(sdl3::keyboard::Scancode::LShift)
    ));
    assert!(!super::super::key_event_matches(
        DesktopKey::LeftShift,
        Some(Keycode::RShift),
        Some(sdl3::keyboard::Scancode::LShift)
    ));
    assert!(!super::super::key_event_matches(
        DesktopKey::Z,
        Some(Keycode::Z),
        Some(sdl3::keyboard::Scancode::A)
    ));
    assert!(!super::super::key_event_matches(
        DesktopKey::X,
        Some(Keycode::X),
        Some(sdl3::keyboard::Scancode::Z)
    ));
    assert!(super::super::key_event_matches(
        DesktopKey::Z,
        Some(Keycode::Z),
        None
    ));
    assert_eq!(
        assignable_key_for_binding_target_from_key_event(
            Some(Keycode::Z),
            Some(sdl3::keyboard::Scancode::A),
            KeyboardBindingTarget::A
        ),
        None
    );
    assert_eq!(
        assignable_key_for_binding_target_from_key_event(
            None,
            Some(sdl3::keyboard::Scancode::LAlt),
            KeyboardBindingTarget::A
        ),
        Some(DesktopKey::LeftAlt)
    );

    let joypad = gb_desktop::JoypadKeyboardBindings::default();
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::Up),
        Some(gb_core::JoypadButton::Up)
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::Down),
        Some(gb_core::JoypadButton::Down)
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::Left),
        Some(gb_core::JoypadButton::Left)
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::Right),
        Some(gb_core::JoypadButton::Right)
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::LAlt),
        Some(gb_core::JoypadButton::B)
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::LGui),
        Some(gb_core::JoypadButton::A)
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::Backspace),
        Some(gb_core::JoypadButton::Select)
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::Return),
        Some(gb_core::JoypadButton::Start)
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::F1),
        None
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::F5),
        None
    );
    assert_eq!(
        super::super::joypad_button_for_key(joypad, Keycode::F6),
        None
    );

    let keyboard_bindings = gb_desktop::KeyboardBindings::default();
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::F9),
        super::super::HotkeyAction::ManualSave
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::F1),
        super::super::HotkeyAction::SaveState
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::F2),
        super::super::HotkeyAction::LoadState
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::_1),
        super::super::HotkeyAction::SelectStateSlot(1)
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::_4),
        super::super::HotkeyAction::SelectStateSlot(4)
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::F12),
        super::super::HotkeyAction::Reset
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::LShift),
        super::super::HotkeyAction::Rewind
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::RShift),
        super::super::HotkeyAction::FastForward
    ));
    assert!(matches!(
        super::super::hotkey_action_for_key_event(
            &keyboard_bindings,
            Some(Keycode::RShift),
            Some(sdl3::keyboard::Scancode::LShift),
        ),
        super::super::HotkeyAction::FastForward
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::R),
        super::super::HotkeyAction::None
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::F11),
        super::super::HotkeyAction::ToggleFullscreen
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::F10),
        super::super::HotkeyAction::TogglePerformanceHud
    ));
    assert!(matches!(
        super::super::hotkey_action(&keyboard_bindings, Keycode::Space),
        super::super::HotkeyAction::None
    ));

    let p1 = keyboard_bindings.joypad;
    let mut gameplay_bindings = vec![
        ("P1 up", desktop_key_scancode(p1.up)),
        ("P1 down", desktop_key_scancode(p1.down)),
        ("P1 left", desktop_key_scancode(p1.left)),
        ("P1 right", desktop_key_scancode(p1.right)),
        ("P1 A", desktop_key_scancode(p1.a)),
        ("P1 B", desktop_key_scancode(p1.b)),
        ("P1 select", desktop_key_scancode(p1.select)),
        ("P1 start", desktop_key_scancode(p1.start)),
    ];
    gameplay_bindings.extend(
        crate::player_slots::LINKED_DMG04_P2_KEYBOARD_BINDINGS
            .into_iter()
            .map(|(button, scancode)| {
                (
                    match button {
                        JoypadButton::Up => "P2 up",
                        JoypadButton::Down => "P2 down",
                        JoypadButton::Left => "P2 left",
                        JoypadButton::Right => "P2 right",
                        JoypadButton::A => "P2 A",
                        JoypadButton::B => "P2 B",
                        JoypadButton::Select => "P2 select",
                        JoypadButton::Start => "P2 start",
                    },
                    scancode,
                )
            }),
    );
    gameplay_bindings.extend(
        crate::player_slots::LINKED_DMG07_P3_KEYBOARD_BINDINGS
            .into_iter()
            .map(|(button, scancode)| {
                (
                    match button {
                        JoypadButton::Up => "P3 up",
                        JoypadButton::Down => "P3 down",
                        JoypadButton::Left => "P3 left",
                        JoypadButton::Right => "P3 right",
                        JoypadButton::A => "P3 A",
                        JoypadButton::B => "P3 B",
                        JoypadButton::Select => "P3 select",
                        JoypadButton::Start => "P3 start",
                    },
                    scancode,
                )
            }),
    );
    gameplay_bindings.extend(
        crate::player_slots::LINKED_DMG07_P4_KEYBOARD_BINDINGS
            .into_iter()
            .map(|(button, scancode)| {
                (
                    match button {
                        JoypadButton::Up => "P4 up",
                        JoypadButton::Down => "P4 down",
                        JoypadButton::Left => "P4 left",
                        JoypadButton::Right => "P4 right",
                        JoypadButton::A => "P4 A",
                        JoypadButton::B => "P4 B",
                        JoypadButton::Select => "P4 select",
                        JoypadButton::Start => "P4 start",
                    },
                    scancode,
                )
            }),
    );
    let hotkeys = keyboard_bindings.hotkeys;
    gameplay_bindings.extend([
        ("hotkey pause", desktop_key_scancode(hotkeys.pause)),
        (
            "hotkey save state",
            desktop_key_scancode(hotkeys.save_state),
        ),
        (
            "hotkey load state",
            desktop_key_scancode(hotkeys.load_state),
        ),
        (
            "hotkey state slot 1",
            desktop_key_scancode(hotkeys.state_slot_1),
        ),
        (
            "hotkey state slot 2",
            desktop_key_scancode(hotkeys.state_slot_2),
        ),
        (
            "hotkey state slot 3",
            desktop_key_scancode(hotkeys.state_slot_3),
        ),
        (
            "hotkey state slot 4",
            desktop_key_scancode(hotkeys.state_slot_4),
        ),
        ("hotkey reset", desktop_key_scancode(hotkeys.reset)),
        ("hotkey rewind", desktop_key_scancode(hotkeys.rewind)),
        (
            "hotkey fullscreen",
            desktop_key_scancode(hotkeys.toggle_fullscreen),
        ),
        (
            "hotkey stats",
            desktop_key_scancode(hotkeys.toggle_performance_hud),
        ),
        (
            "hotkey save battery",
            desktop_key_scancode(hotkeys.save_battery),
        ),
    ]);
    for (left_index, (left_label, left_scancode)) in gameplay_bindings.iter().enumerate() {
        for (right_label, right_scancode) in gameplay_bindings.iter().skip(left_index + 1) {
            assert_ne!(
                left_scancode, right_scancode,
                "default gameplay binding {left_label} overlaps {right_label} on {left_scancode:?}"
            );
        }
    }

    let menu_bindings = GamepadMenuBindings::default();
    assert_eq!(
        menu_input_for_gamepad_button(menu_bindings, Button::DPadUp),
        Some(super::super::MenuInput::Up)
    );
    assert_eq!(
        menu_input_for_gamepad_button(menu_bindings, Button::DPadDown),
        Some(super::super::MenuInput::Down)
    );

    assert_eq!(
        super::super::compact_recent_rom_label(Path::new("/tmp/(([])).gb")).as_str(),
        "ROM"
    );
    assert_eq!(
        map_path_dialog_result(Ok(Vec::new())),
        PathDialogResult::Canceled
    );
    assert!(matches!(
        map_path_dialog_result(Err(DialogError::SdlError(sdl3::get_error()))),
        PathDialogResult::Failed(_)
    ));
    assert_eq!(
        super::super::diagnostic_severity_name(CartridgeDiagnosticSeverity::Error),
        "error"
    );
    assert_eq!(
        super::super::execution_mode_name(ExecutionMode::Experimental),
        "experimental"
    );
    assert_eq!(
        super::super::DMG_DISPLAY_PALETTE.shade_rgb(7),
        super::super::DMG_DISPLAY_PALETTE.shade_rgb(3)
    );
}
