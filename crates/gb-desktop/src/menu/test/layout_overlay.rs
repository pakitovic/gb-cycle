use super::*;

#[test]
fn overlay_actions_cover_binding_capture_targets_and_screen_titles() {
    let mut presentation = MenuPresentation {
        recent_rom_count: 1,
        audio_available: true,
        manual_save_available: true,
        cartridge_pocket_camera_supported: true,
        pocket_camera_live_enabled: true,
        gamepad_available: true,
        ..test_presentation()
    };
    presentation.recent_rom_labels[0] = CompactRecentRomLabel::from_text("TETRIS");

    assert_eq!(MenuScreen::Root.title(test_presentation()), "MENU");
    assert_eq!(
        MenuScreen::Root.title(MenuPresentation {
            rom_loaded: false,
            ..test_presentation()
        }),
        "NO ROM"
    );
    assert_eq!(MenuScreen::Recent.title(presentation), "RECENT");
    assert_eq!(MenuScreen::Config.title(presentation), "CONFIG");
    assert_eq!(MenuScreen::Video.title(presentation), "VIDEO");
    assert_eq!(MenuScreen::Audio.title(presentation), "AUDIO");
    assert_eq!(MenuScreen::Input.title(presentation), "INPUT");
    assert_eq!(MenuScreen::ExtPort.title(presentation), "EXT PORT");
    assert_eq!(MenuScreen::GameLink.title(presentation), "GAME LINK");
    assert_eq!(
        MenuScreen::FourPlayerAdapter.title(presentation),
        "4P ADAPTER"
    );
    assert_eq!(MenuScreen::CgbInfrared.title(presentation), "GBC IR");
    assert_eq!(MenuScreen::Gamepad.title(presentation), "GAMEPAD");
    assert_eq!(
        MenuScreen::GamepadMenuControls.title(presentation),
        "PAD MENU"
    );
    assert_eq!(MenuScreen::Keyboard.title(presentation), "KEYBOARD");
    assert_eq!(
        MenuScreen::KeyboardMenuControls.title(presentation),
        "KB MENU"
    );
    assert_eq!(MenuScreen::Hotkeys.title(presentation), "HOTKEYS");
    assert_eq!(MenuScreen::System.title(presentation), "SYSTEM");
    assert_eq!(MenuScreen::BootRom.title(presentation), "BOOT ROM");
    assert_eq!(MenuScreen::Save.title(presentation), "SAVE");
    assert_eq!(MenuScreen::Rewind.title(presentation), "REWIND");
    assert_eq!(MenuScreen::FastForward.title(presentation), "F-FORWARD");

    let mut menu = OverlayMenuState::default();
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::OpenRom, presentation),
        Some(MenuAction::OpenRom)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::RecentMenu, presentation),
        None
    );
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::OpenRecentRom(0))
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::ClearRecentList, presentation),
        Some(MenuAction::ClearRecentList)
    );
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::ConfigMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Config);
    assert_eq!(
        menu.apply_item_action(MenuItem::VideoMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Video);
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::ConfigMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Config);
    assert_eq!(
        menu.apply_item_action(MenuItem::AudioMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Audio);
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::ConfigMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Config);
    assert_eq!(
        menu.apply_item_action(MenuItem::InputMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Input);
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::ExtPortMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::ExtPort);
    assert_eq!(
        menu.apply_item_action(MenuItem::ExternalPortGameLink, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::GameLink);
    assert_eq!(
        menu.apply_item_action(MenuItem::GameLinkSameGame, presentation),
        Some(MenuAction::SetGameLinkSameGame)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::GameLinkSelectGame, presentation),
        Some(MenuAction::SelectGameLinkRom)
    );
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::ExtPortMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::ExtPort);
    assert_eq!(
        menu.apply_item_action(MenuItem::ExternalPortFourPlayerAdapter, presentation),
        None
    );
    assert_eq!(
        menu.current_screen_state().screen,
        MenuScreen::FourPlayerAdapter
    );
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfrared, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::CgbInfrared);
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfraredNone, presentation),
        Some(MenuAction::SetCgbInfraredNone)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfraredSameGame, presentation),
        Some(MenuAction::SetCgbInfraredSameGame)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfraredSelectGame, presentation),
        Some(MenuAction::SelectCgbInfraredSecondary)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfraredPikachuColor, presentation),
        Some(MenuAction::SetCgbInfraredPikachuColor)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfraredPikachuGift, presentation),
        Some(MenuAction::CycleCgbInfraredPikachuGift)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfraredMysteryGift, presentation),
        Some(MenuAction::SetCgbInfraredMysteryGift)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfraredMysteryGiftKind, presentation),
        Some(MenuAction::CycleCgbInfraredMysteryGiftKind)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfraredMysteryGiftSelection, presentation),
        Some(MenuAction::CycleCgbInfraredMysteryGiftCode)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CgbInfraredHelper, presentation),
        Some(MenuAction::ToggleCgbInfraredHelper)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::FourPlayerAdapterTwoPlayers, presentation),
        Some(MenuAction::SetFourPlayerAdapter(
            DesktopDmg07PlayerCount::Two
        ))
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::FourPlayerAdapterThreePlayers, presentation),
        Some(MenuAction::SetFourPlayerAdapter(
            DesktopDmg07PlayerCount::Three
        ))
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::FourPlayerAdapterFourPlayers, presentation),
        Some(MenuAction::SetFourPlayerAdapter(
            DesktopDmg07PlayerCount::Four
        ))
    );
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::ConfigMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Config);
    assert_eq!(
        menu.apply_item_action(MenuItem::SystemMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::System);
    assert_eq!(
        menu.apply_item_action(MenuItem::BootRomMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::BootRom);
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::ConfigMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Config);
    assert_eq!(
        menu.apply_item_action(MenuItem::SystemMenu, presentation),
        None
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::SaveMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Save);
    assert_eq!(
        menu.apply_item_action(MenuItem::ExportSave, presentation),
        Some(MenuAction::ExportSave)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::ImportSave, presentation),
        Some(MenuAction::ImportSave)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::SaveBattery, presentation),
        Some(MenuAction::SaveBattery)
    );
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::ConfigMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Config);
    assert_eq!(
        menu.apply_item_action(MenuItem::SystemMenu, presentation),
        None
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::RewindMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Rewind);
    assert_eq!(
        menu.apply_item_action(MenuItem::RewindEnabled, presentation),
        Some(MenuAction::ToggleRewindEnabled)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::RewindHistory, presentation),
        Some(MenuAction::CycleRewindHistory)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::RewindSubframes, presentation),
        Some(MenuAction::CycleRewindSubframes)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::RewindSpeed, presentation),
        Some(MenuAction::CycleRewindSpeed)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::RewindMemory, presentation),
        Some(MenuAction::CycleRewindMemory)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::RewindDefaults, presentation),
        Some(MenuAction::ResetRewindDefaults)
    );
    menu.open(presentation);
    assert_eq!(
        menu.apply_item_action(MenuItem::ConfigMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::Config);
    assert_eq!(
        menu.apply_item_action(MenuItem::SystemMenu, presentation),
        None
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::FastForwardMenu, presentation),
        None
    );
    assert_eq!(menu.current_screen_state().screen, MenuScreen::FastForward);
    assert_eq!(
        menu.apply_item_action(MenuItem::FastForwardEnabled, presentation),
        Some(MenuAction::ToggleFastForwardEnabled)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::FastForwardSpeed, presentation),
        Some(MenuAction::CycleFastForwardSpeed)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::FastForwardDefaults, presentation),
        Some(MenuAction::ResetFastForwardDefaults)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CameraImage, presentation),
        Some(MenuAction::SelectCameraImage)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CameraLive, presentation),
        Some(MenuAction::ToggleCameraLive)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::CameraReset, presentation),
        Some(MenuAction::ResetCameraImage)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::GamepadActive, presentation),
        None
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::GamepadPreferred, presentation),
        Some(MenuAction::TogglePreferredGamepad)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::GamepadGyro, presentation),
        Some(MenuAction::CycleGamepadGyroMode)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::GamepadRumble, presentation),
        Some(MenuAction::CycleGamepadRumbleMode)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::Reset, presentation),
        Some(MenuAction::Reset)
    );
    assert_eq!(
        menu.apply_item_action(MenuItem::Quit, presentation),
        Some(MenuAction::Quit)
    );

    for (item, expected) in [
        (MenuItem::KeyboardUp, MenuItem::KeyboardUp),
        (MenuItem::KeyboardDown, MenuItem::KeyboardDown),
        (MenuItem::KeyboardLeft, MenuItem::KeyboardLeft),
        (MenuItem::KeyboardRight, MenuItem::KeyboardRight),
        (MenuItem::KeyboardA, MenuItem::KeyboardA),
        (MenuItem::KeyboardB, MenuItem::KeyboardB),
        (MenuItem::KeyboardSelect, MenuItem::KeyboardSelect),
        (MenuItem::KeyboardStart, MenuItem::KeyboardStart),
        (MenuItem::KeyboardMenuUp, MenuItem::KeyboardMenuUp),
        (MenuItem::KeyboardMenuDown, MenuItem::KeyboardMenuDown),
        (MenuItem::KeyboardMenuConfirm, MenuItem::KeyboardMenuConfirm),
        (MenuItem::KeyboardMenuCancel, MenuItem::KeyboardMenuCancel),
        (MenuItem::HotkeyPause, MenuItem::HotkeyPause),
        (MenuItem::HotkeySaveState, MenuItem::HotkeySaveState),
        (MenuItem::HotkeyLoadState, MenuItem::HotkeyLoadState),
        (MenuItem::HotkeyStateSlot1, MenuItem::HotkeyStateSlot1),
        (MenuItem::HotkeyStateSlot2, MenuItem::HotkeyStateSlot2),
        (MenuItem::HotkeyStateSlot3, MenuItem::HotkeyStateSlot3),
        (MenuItem::HotkeyStateSlot4, MenuItem::HotkeyStateSlot4),
        (MenuItem::HotkeyReset, MenuItem::HotkeyReset),
        (MenuItem::HotkeyRewind, MenuItem::HotkeyRewind),
        (MenuItem::HotkeyFastForward, MenuItem::HotkeyFastForward),
        (MenuItem::HotkeyFullscreen, MenuItem::HotkeyFullscreen),
        (
            MenuItem::HotkeyPerformanceHud,
            MenuItem::HotkeyPerformanceHud,
        ),
        (MenuItem::HotkeySaveBattery, MenuItem::HotkeySaveBattery),
        (MenuItem::GamepadUp, MenuItem::GamepadUp),
        (MenuItem::GamepadDown, MenuItem::GamepadDown),
        (MenuItem::GamepadLeft, MenuItem::GamepadLeft),
        (MenuItem::GamepadRight, MenuItem::GamepadRight),
        (MenuItem::GamepadA, MenuItem::GamepadA),
        (MenuItem::GamepadB, MenuItem::GamepadB),
        (MenuItem::GamepadSelect, MenuItem::GamepadSelect),
        (MenuItem::GamepadStart, MenuItem::GamepadStart),
        (MenuItem::GamepadSaveState, MenuItem::GamepadSaveState),
        (MenuItem::GamepadLoadState, MenuItem::GamepadLoadState),
        (MenuItem::GamepadRewind, MenuItem::GamepadRewind),
        (MenuItem::GamepadFastForward, MenuItem::GamepadFastForward),
        (MenuItem::GamepadMenuUp, MenuItem::GamepadMenuUp),
        (MenuItem::GamepadMenuDown, MenuItem::GamepadMenuDown),
        (MenuItem::GamepadMenuConfirm, MenuItem::GamepadMenuConfirm),
        (MenuItem::GamepadMenuCancel, MenuItem::GamepadMenuCancel),
    ] {
        menu.pending_binding_capture = None;
        assert_eq!(menu.apply_item_action(item, presentation), None);
        assert_eq!(menu.pending_binding_item(), Some(expected));
    }
}

#[test]
fn gamepad_action_binding_capture_emits_host_action_targets() {
    let mut menu = OverlayMenuState::default();

    menu.begin_gamepad_action_binding_capture_for_tests(GamepadActionBindingTarget::SaveState);
    assert_eq!(
        menu.pending_gamepad_action_binding_target(),
        Some(GamepadActionBindingTarget::SaveState)
    );
    assert_eq!(
        menu.handle_gamepad_binding_capture(GamepadButtonBinding::LeftShoulder),
        Some(MenuAction::SetGamepadActionBinding(
            GamepadActionBindingTarget::SaveState,
            GamepadButtonBinding::LeftShoulder
        ))
    );
    assert_eq!(menu.pending_gamepad_action_binding_target(), None);

    menu.begin_gamepad_action_binding_capture_for_tests(GamepadActionBindingTarget::FastForward);
    assert_eq!(
        menu.pending_binding_item(),
        Some(MenuItem::GamepadFastForward)
    );
    assert_eq!(
        menu.handle_gamepad_binding_capture(GamepadButtonBinding::RightShoulder),
        Some(MenuAction::SetGamepadActionBinding(
            GamepadActionBindingTarget::FastForward,
            GamepadButtonBinding::RightShoulder
        ))
    );
}
