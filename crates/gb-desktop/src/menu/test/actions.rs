use super::*;

#[test]
fn root_menu_exposes_quit_as_the_last_first_level_action() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    menu.open(presentation);

    assert_eq!(menu.handle_input(MenuInput::Up, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::Quit)
    );
}

#[test]
fn menu_panel_layout_centers_on_dmg_and_sgb_frames() {
    let dmg_layout = MenuPanelLayout::centered(160, 144);
    assert_eq!(dmg_layout.panel_x, 20);
    assert_eq!(dmg_layout.panel_y, 16);

    let sgb_layout = MenuPanelLayout::centered(256, 224);
    assert_eq!(sgb_layout.panel_x, 68);
    assert_eq!(sgb_layout.panel_y, 56);
    assert_eq!(
        sgb_layout.item_text_x,
        sgb_layout.panel_x + MENU_ITEM_TEXT_OFFSET_X
    );
    assert_eq!(
        sgb_layout.item_text_y,
        sgb_layout.panel_y + MENU_ITEM_AREA_TOP_OFFSET
    );
}
