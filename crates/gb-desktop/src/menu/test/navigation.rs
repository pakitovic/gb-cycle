use super::*;

#[test]
fn opening_the_menu_selects_the_first_enabled_root_item() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();

    menu.open(presentation);

    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::OpenRom)
    );
}

#[test]
fn navigation_skips_disabled_root_items() {
    let presentation = MenuPresentation {
        rom_loaded: false,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    menu.open(presentation);

    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::OpenRom)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::TogglePerformanceHud)
    );
}
