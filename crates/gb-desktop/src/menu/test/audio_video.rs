use super::*;

#[test]
fn audio_item_toggles_mute_inside_the_audio_submenu() {
    let presentation = MenuPresentation {
        audio_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_audio_menu(&mut menu, presentation);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ToggleMute)
    );
}

#[test]
fn audio_submenu_cycles_volume_after_mute() {
    let presentation = MenuPresentation {
        audio_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_audio_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::AudioVolume);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleAudioVolume)
    );
}

#[test]
fn video_submenu_cycles_scale_and_toggles_integer_presentation() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_video_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::WindowScale);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleWindowScale)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ToggleIntegerScale)
    );
}

#[test]
fn video_submenu_toggles_the_performance_hud() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_video_menu(&mut menu, presentation);

    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::TogglePerformanceHud)
    );
}

#[test]
fn video_submenu_toggles_the_presentation_filter() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_video_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::PresentationFilter);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::TogglePresentationFilter)
    );
}

#[test]
fn video_submenu_cycles_frame_blending_after_filter() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_video_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::FrameBlending);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleFrameBlending)
    );
}

#[test]
fn video_submenu_cycles_the_display_palette_after_filter() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_video_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::DisplayPalette);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleDisplayPalette)
    );
}

#[test]
fn video_submenu_disables_display_palette_for_rgb555_models() {
    let mut presentation = test_presentation();

    for (console_model, expected_label) in [
        (DesktopConsoleModel::GameBoyColor, "PALETTE RGB555"),
        (DesktopConsoleModel::SuperGameBoy, "PALETTE SGB"),
        (DesktopConsoleModel::SuperGameBoy2, "PALETTE SGB2"),
    ] {
        presentation.console_model = console_model;
        presentation.display_palette = DesktopDisplayPalette::Grey;

        assert!(!presentation.item_enabled(MenuItem::DisplayPalette));
        assert_eq!(
            presentation.item_label(MenuItem::DisplayPalette),
            expected_label
        );
        assert_eq!(
            super::super::next_enabled_index(MenuScreen::Video, 1, presentation),
            2
        );
    }
}

#[test]
fn video_submenu_saves_a_screenshot_before_layer_toggles() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_video_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::Screenshot);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::SaveScreenshot)
    );
}

#[test]
fn video_submenu_exposes_layer_toggles_after_filter() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_video_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::ShowBackground);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ToggleBackgroundLayer)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ToggleWindowLayer)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ToggleObjectLayer)
    );
}

#[test]
fn video_submenu_toggles_vsync_before_scale() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_video_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::Vsync);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ToggleVsync)
    );
}

#[test]
fn video_submenu_resets_defaults_after_the_host_toggles() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_video_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::VideoDefaults);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ResetVideoDefaults)
    );
}

#[test]
fn audio_submenu_resets_defaults_after_volume() {
    let presentation = MenuPresentation {
        audio_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_audio_menu(&mut menu, presentation);

    while visible_item_at(
        menu.current_screen_state().screen,
        menu.current_screen_state().selected_index,
        presentation,
    ) != Some(MenuItem::AudioDefaults)
    {
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    }
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ResetAudioDefaults)
    );
}

#[test]
fn input_submenu_cycles_the_gamepad_directional_source() {
    let presentation = MenuPresentation {
        audio_available: true,
        gamepad_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::GamepadDirection);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleGamepadDirectionalSource)
    );
}
