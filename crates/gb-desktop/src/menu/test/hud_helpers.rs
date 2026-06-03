use super::*;

#[test]
fn viewport_scrolls_to_keep_the_last_visible_items_in_view() {
    assert_eq!(MENU_VISIBLE_ITEM_CAPACITY, 5);
    assert_eq!(viewport_start_index(0, 6), 0);
    assert_eq!(viewport_start_index(4, 6), 0);
    assert_eq!(viewport_start_index(5, 6), 1);
    assert_eq!(viewport_start_index(0, 7), 0);
    assert_eq!(viewport_start_index(4, 7), 0);
    assert_eq!(viewport_start_index(5, 7), 1);
    assert_eq!(viewport_start_index(6, 7), 2);
}

#[test]
fn scroll_indicators_point_toward_the_hidden_items() {
    assert_eq!(
        scroll_indicator_rows(ScrollIndicatorDirection::Up),
        [(0, 1), (1, 3), (2, 5)]
    );
    assert_eq!(
        scroll_indicator_rows(ScrollIndicatorDirection::Down),
        [(0, 5), (1, 3), (2, 1)]
    );
}

#[test]
fn compact_labels_binding_labels_and_previous_navigation_cover_overlay_helpers() {
    assert!(CompactMenuLabel::default().is_empty());
    assert!(!CompactMenuLabel::from_text("PAD").is_empty());
    assert_eq!(CompactMenuLabel::from_text("PAD!? 12").as_str(), "PAD 12");
    assert_eq!(
        CompactRecentRomLabel::from_text("ROM!? 7").as_str(),
        "ROM 7"
    );
    assert_eq!(
        gamepad_binding_label(GamepadButtonBinding::RightShoulder),
        "R1"
    );
    assert_eq!(
        gamepad_binding_label(GamepadButtonBinding::LeftTrigger),
        "L2"
    );
    assert_eq!(
        gamepad_binding_label(GamepadButtonBinding::RightTrigger),
        "R2"
    );
    assert_eq!(desktop_key_label(DesktopKey::Tab), "TAB");
    assert_eq!(desktop_key_label(DesktopKey::LeftShift), "L SHIFT");
    assert_eq!(desktop_key_label(DesktopKey::RightShift), "R SHIFT");
    assert_eq!(desktop_key_label(DesktopKey::LeftControl), "L CTRL");
    assert_eq!(desktop_key_label(DesktopKey::RightControl), "R CTRL");
    assert_eq!(desktop_key_label(DesktopKey::Digit1), "1");
    assert_eq!(desktop_key_label(DesktopKey::Digit4), "4");
    assert_eq!(desktop_key_label(DesktopKey::F1), "F1");
    assert_eq!(desktop_key_label(DesktopKey::F2), "F2");
    assert_eq!(desktop_key_label(DesktopKey::F6), "F6");
    assert_eq!(desktop_key_label(DesktopKey::F9), "F9");
    assert_eq!(desktop_key_label(DesktopKey::F12), "F12");
    #[cfg(target_os = "macos")]
    {
        assert_eq!(desktop_key_label(DesktopKey::LeftAlt), "L OPT");
        assert_eq!(desktop_key_label(DesktopKey::RightAlt), "R OPT");
        assert_eq!(desktop_key_label(DesktopKey::LeftGui), "L CMD");
        assert_eq!(desktop_key_label(DesktopKey::RightGui), "R CMD");
    }
    #[cfg(target_os = "windows")]
    {
        assert_eq!(desktop_key_label(DesktopKey::LeftAlt), "L ALT");
        assert_eq!(desktop_key_label(DesktopKey::RightAlt), "R ALT");
        assert_eq!(desktop_key_label(DesktopKey::LeftGui), "L WIN");
        assert_eq!(desktop_key_label(DesktopKey::RightGui), "R WIN");
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        assert_eq!(desktop_key_label(DesktopKey::LeftAlt), "L ALT");
        assert_eq!(desktop_key_label(DesktopKey::RightAlt), "R ALT");
        assert_eq!(desktop_key_label(DesktopKey::LeftGui), "L SUPER");
        assert_eq!(desktop_key_label(DesktopKey::RightGui), "R SUPER");
    }
    assert_eq!(
        previous_enabled_index(MenuScreen::Root, 0, test_presentation()),
        7
    );

    let mut presentation = test_presentation();
    presentation.recent_rom_count = 2;
    presentation.recent_rom_labels[0] = CompactRecentRomLabel::from_text("TETRIS");
    presentation.recent_rom_labels[1] = CompactRecentRomLabel::from_text("MARIO");
    assert_eq!(presentation.item_label(MenuItem::RecentRom2), "MARIO");
}

#[test]
fn machine_state_root_items_label_cycle_and_disable_when_unavailable() {
    let presentation = MenuPresentation {
        machine_state_slot: 3,
        ..test_presentation()
    };

    assert_eq!(presentation.item_label(MenuItem::SaveState), "SAVE STATE");
    assert_eq!(presentation.item_label(MenuItem::LoadState), "LOAD STATE");
    assert_eq!(presentation.item_label(MenuItem::StateSlot), "STATE SLOT 3");
    assert_eq!(
        presentation.item_label(MenuItem::StateAutoloadSlot),
        "AUTOLOAD OFF"
    );
    assert!(presentation.item_enabled(MenuItem::SaveState));
    assert!(presentation.item_enabled(MenuItem::LoadState));
    assert!(presentation.item_enabled(MenuItem::StateSlot));
    assert!(presentation.item_enabled(MenuItem::StateAutoloadSlot));

    let launcher = MenuPresentation {
        rom_loaded: false,
        ..presentation
    };
    assert!(!launcher.item_visible(MenuItem::SaveState));
    assert!(!launcher.item_visible(MenuItem::LoadState));
    assert!(!launcher.item_visible(MenuItem::StateSlot));
    assert!(!launcher.item_visible(MenuItem::StateAutoloadSlot));

    let blocked = MenuPresentation {
        machine_state_available: false,
        ..presentation
    };
    assert!(blocked.item_visible(MenuItem::LoadState));
    assert!(!blocked.item_enabled(MenuItem::SaveState));
    assert!(!blocked.item_enabled(MenuItem::LoadState));
    assert!(blocked.item_enabled(MenuItem::StateSlot));
    assert!(blocked.item_enabled(MenuItem::StateAutoloadSlot));

    let no_slot_file = MenuPresentation {
        machine_state_load_available: false,
        ..presentation
    };
    assert!(no_slot_file.item_enabled(MenuItem::SaveState));
    assert!(no_slot_file.item_visible(MenuItem::LoadState));
    assert!(!no_slot_file.item_enabled(MenuItem::LoadState));
    assert!(no_slot_file.item_enabled(MenuItem::StateSlot));
    assert!(no_slot_file.item_enabled(MenuItem::StateAutoloadSlot));
    assert_eq!(
        visible_item_at(MenuScreen::Root, 2, no_slot_file),
        Some(MenuItem::LoadState)
    );
    assert_eq!(
        visible_item_at(MenuScreen::Root, 4, no_slot_file),
        Some(MenuItem::StateAutoloadSlot)
    );

    let mut menu = OverlayMenuState::default();
    menu.open(presentation);
    select_visible_item(&mut menu, presentation, MenuItem::SaveState);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::SaveState)
    );
    select_visible_item(&mut menu, presentation, MenuItem::LoadState);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::LoadState)
    );
    select_visible_item(&mut menu, presentation, MenuItem::StateSlot);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleStateSlot)
    );
    select_visible_item(&mut menu, presentation, MenuItem::StateAutoloadSlot);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleStateAutoloadSlot)
    );
}
