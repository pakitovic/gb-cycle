use super::*;

#[test]
fn opening_rom_is_skipped_while_dialog_is_pending() {
    let presentation = MenuPresentation {
        rom_loaded: false,
        ..test_presentation()
    };
    let blocked_presentation = MenuPresentation {
        any_dialog_pending: true,
        ..presentation
    };
    let mut menu = OverlayMenuState::default();
    menu.open(presentation);

    assert_eq!(
        menu.handle_input(MenuInput::Confirm, blocked_presentation),
        None
    );
    assert_eq!(
        menu.handle_input(MenuInput::Down, blocked_presentation),
        None
    );
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, blocked_presentation),
        None
    );
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, blocked_presentation),
        None
    );
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, blocked_presentation),
        Some(MenuAction::TogglePerformanceHud)
    );
}

#[test]
fn open_rom_stays_selected_while_the_dialog_is_pending() {
    let presentation = MenuPresentation {
        rom_loaded: false,
        ..test_presentation()
    };
    let blocked_presentation = MenuPresentation {
        any_dialog_pending: true,
        ..presentation
    };
    let mut menu = OverlayMenuState::default();
    menu.open(presentation);

    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::OpenRom)
    );
    assert_eq!(
        normalized_selected_index(MenuScreen::Root, 1, blocked_presentation),
        1
    );
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, blocked_presentation),
        None
    );
}

#[test]
fn recent_roms_root_entry_opens_the_recent_submenu() {
    let mut recent_rom_labels = [CompactRecentRomLabel::default(); RECENT_ROM_MENU_CAPACITY];
    recent_rom_labels[0] = CompactRecentRomLabel::from_text("TETRIS");
    let presentation = MenuPresentation {
        recent_rom_count: 1,
        recent_rom_labels,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_recent_menu(&mut menu, presentation);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::OpenRecentRom(0))
    );
}

#[test]
fn recent_submenu_exposes_clear_list_before_return() {
    let mut recent_rom_labels = [CompactRecentRomLabel::default(); RECENT_ROM_MENU_CAPACITY];
    recent_rom_labels[0] = CompactRecentRomLabel::from_text("TETRIS");
    let presentation = MenuPresentation {
        recent_rom_count: 1,
        recent_rom_labels,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_recent_menu(&mut menu, presentation);
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ClearRecentList)
    );
}

#[test]
fn cancel_in_a_submenu_returns_to_the_previous_screen() {
    let presentation = MenuPresentation {
        audio_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_audio_menu(&mut menu, presentation);
    assert_eq!(menu.handle_input(MenuInput::Cancel, presentation), None);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
}

#[test]
fn cancel_on_the_root_screen_stays_in_the_launcher_until_a_rom_is_loaded() {
    let presentation = MenuPresentation {
        rom_loaded: false,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    menu.open(presentation);

    assert_eq!(menu.handle_input(MenuInput::Cancel, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::OpenRom)
    );
}

#[test]
fn root_title_reports_no_rom_when_the_menu_is_acting_as_a_launcher() {
    let launcher_presentation = MenuPresentation {
        rom_loaded: false,
        ..test_presentation()
    };
    let loaded_presentation = MenuPresentation {
        rom_loaded: true,
        ..launcher_presentation
    };

    assert_eq!(MenuScreen::Root.title(launcher_presentation), "NO ROM");
    assert_eq!(MenuScreen::Root.title(loaded_presentation), "MENU");
}

#[test]
fn save_battery_is_hidden_when_auto_flush_policy_is_active() {
    let presentation = MenuPresentation {
        audio_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_save_menu(&mut menu, presentation);

    assert!(
        (0..visible_item_count(MenuScreen::Save, presentation)).all(|index| visible_item_at(
            MenuScreen::Save,
            index,
            presentation
        ) != Some(
            MenuItem::SaveBattery
        ))
    );
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ToggleSavesEnabled)
    );
}

#[test]
fn save_battery_remains_available_when_manual_save_policy_is_active() {
    let presentation = MenuPresentation {
        audio_available: true,
        manual_save_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_save_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::SaveBattery);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::SaveBattery)
    );
}

#[test]
fn keyboard_submenu_starts_a_capture_and_emits_the_selected_binding() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::KeyboardMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert!(menu.is_capturing_binding());
    assert_eq!(
        menu.handle_keyboard_binding_capture(DesktopKey::Space),
        Some(MenuAction::SetKeyboardBinding(
            KeyboardBindingTarget::Up,
            DesktopKey::Space
        ))
    );
    assert!(!menu.is_capturing_binding());
}
