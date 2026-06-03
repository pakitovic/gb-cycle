use super::*;

#[test]
fn keyboard_binding_capture_can_be_canceled_without_closing_the_menu() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::KeyboardMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert!(menu.is_capturing_binding());

    menu.cancel_binding_capture();

    assert!(!menu.is_capturing_binding());
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
}

#[test]
fn hotkeys_submenu_starts_a_capture_and_emits_the_selected_binding() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::HotkeysMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert!(menu.is_capturing_binding());
    assert_eq!(
        menu.handle_keyboard_binding_capture(DesktopKey::F11),
        Some(MenuAction::SetKeyboardBinding(
            KeyboardBindingTarget::Pause,
            DesktopKey::F11
        ))
    );
    assert!(!menu.is_capturing_binding());
}

#[test]
fn hotkeys_submenu_can_capture_the_stats_hud_hotkey() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::HotkeysMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    select_visible_item(&mut menu, presentation, MenuItem::HotkeyPerformanceHud);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert!(menu.is_capturing_binding());
    assert_eq!(
        menu.handle_keyboard_binding_capture(DesktopKey::F10),
        Some(MenuAction::SetKeyboardBinding(
            KeyboardBindingTarget::TogglePerformanceHud,
            DesktopKey::F10
        ))
    );
    assert!(!menu.is_capturing_binding());
}

#[test]
fn keyboard_menu_controls_submenu_starts_a_capture_and_emits_the_selected_binding() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::KeyboardMenuControls);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert!(menu.is_capturing_binding());
    assert_eq!(
        menu.handle_keyboard_binding_capture(DesktopKey::Space),
        Some(MenuAction::SetKeyboardMenuBinding(
            KeyboardMenuBindingTarget::Up,
            DesktopKey::Space
        ))
    );
    assert!(!menu.is_capturing_binding());
}

#[test]
fn gamepad_submenu_starts_a_capture_and_emits_the_selected_binding() {
    let presentation = MenuPresentation {
        gamepad_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::GamepadMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert!(menu.is_capturing_binding());
    assert_eq!(
        menu.handle_gamepad_binding_capture(GamepadButtonBinding::North),
        Some(MenuAction::SetGamepadBinding(
            GamepadBindingTarget::Up,
            GamepadButtonBinding::North
        ))
    );
    assert!(!menu.is_capturing_binding());
}

#[test]
fn gamepad_menu_controls_submenu_starts_a_capture_and_emits_the_selected_binding() {
    let presentation = MenuPresentation {
        gamepad_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::GamepadMenuControls);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert!(menu.is_capturing_binding());
    assert_eq!(
        menu.handle_gamepad_binding_capture(GamepadButtonBinding::North),
        Some(MenuAction::SetGamepadMenuBinding(
            GamepadMenuBindingTarget::Up,
            GamepadButtonBinding::North
        ))
    );
    assert!(!menu.is_capturing_binding());
}

#[test]
fn gamepad_submenu_exposes_the_preferred_device_toggle_before_bindings() {
    let presentation = MenuPresentation {
        gamepad_available: true,
        active_gamepad_connected: true,
        active_gamepad_label: CompactMenuLabel::from_text("SWITCH"),
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::GamepadMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::TogglePreferredGamepad)
    );
}

#[test]
fn active_gamepad_labels_are_compacted_for_the_overlay_width() {
    assert_eq!(
        CompactMenuLabel::from_gamepad_name("Nintendo Switch Pro Controller").as_str(),
        "SWITC PRO"
    );
    assert_eq!(
        CompactMenuLabel::from_gamepad_name("Xbox Wireless Controller").as_str(),
        "XBOX"
    );
}

#[test]
fn performance_hud_lines_round_metrics_and_report_audio_state() {
    let snapshot = PerformanceHudSnapshot {
        fps: 59.6,
        speed_percent: 99.7,
        frame_time_ms: 16.7,
        emulation_time_ms: 11.8,
        render_time_ms: 1.4,
        pacing_time_ms: 3.1,
        audio_queue_ms: Some(18.2),
        rewind: RewindHudSnapshot {
            supported: true,
            enabled: true,
            rewinding: false,
            snapshot_count: 42,
            history_seconds: 3.24,
            accounted_bytes: 24 * 1024 * 1024,
            max_bytes: 256 * 1024 * 1024,
        },
    };

    assert_eq!(
        performance_hud_lines(snapshot),
        [
            "FPS 60 100%".to_string(),
            "FRM 17 EMU 12".to_string(),
            "REN 1 PAC 3".to_string(),
            "AUD 18".to_string(),
            "RW 3.2S 42".to_string(),
            "MEM 24/256M".to_string(),
        ]
    );

    let without_audio = PerformanceHudSnapshot {
        audio_queue_ms: None,
        ..snapshot
    };
    assert_eq!(performance_hud_lines(without_audio)[3], "AUD OFF");
    assert_eq!(
        performance_hud_lines(PerformanceHudSnapshot {
            rewind: RewindHudSnapshot {
                rewinding: true,
                history_seconds: 1.84,
                ..snapshot.rewind
            },
            ..snapshot
        })[4],
        "RW << 1.8S"
    );
    assert_eq!(
        performance_hud_lines(PerformanceHudSnapshot {
            rewind: RewindHudSnapshot {
                snapshot_count: 0,
                ..snapshot.rewind
            },
            ..snapshot
        })[4],
        "RW EMPTY"
    );
    assert_eq!(
        performance_hud_lines(PerformanceHudSnapshot {
            rewind: RewindHudSnapshot {
                enabled: false,
                ..snapshot.rewind
            },
            ..snapshot
        })[4],
        "RW OFF"
    );
}
