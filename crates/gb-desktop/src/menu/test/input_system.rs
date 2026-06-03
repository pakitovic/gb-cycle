use super::*;

#[test]
fn input_submenu_cycles_the_gamepad_rumble_mode_when_supported() {
    let presentation = MenuPresentation {
        audio_available: true,
        gamepad_available: true,
        cartridge_rumble_supported: true,
        active_gamepad_rumble_supported: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::GamepadRumble);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleGamepadRumbleMode)
    );
}

#[test]
fn input_submenu_cycles_the_gamepad_gyro_mode_when_supported() {
    let presentation = MenuPresentation {
        audio_available: true,
        gamepad_available: true,
        active_gamepad_connected: true,
        cartridge_mbc7_accelerometer_supported: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::GamepadGyro);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleGamepadGyroMode)
    );
}

#[test]
fn input_submenu_resets_defaults_after_directional_source() {
    let presentation = MenuPresentation {
        audio_available: true,
        gamepad_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_input_menu(&mut menu, presentation);

    select_visible_item(&mut menu, presentation, MenuItem::InputDefaults);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ResetInputDefaults)
    );
}

#[test]
fn system_submenu_cycles_model_revision_startup_and_execution_mode() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_system_menu(&mut menu, presentation);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleConsoleModel)
    );
    let cgb_presentation = MenuPresentation {
        console_model: DesktopConsoleModel::GameBoyColor,
        ..presentation
    };
    open_system_menu(&mut menu, cgb_presentation);
    assert_eq!(menu.handle_input(MenuInput::Down, cgb_presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, cgb_presentation),
        Some(MenuAction::CycleHardwareRevision)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, cgb_presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, cgb_presentation),
        Some(MenuAction::CycleStartupMode)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, cgb_presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, cgb_presentation),
        Some(MenuAction::CycleExecutionMode)
    );
}

#[test]
fn system_submenu_toggles_sgb_border_only_for_sgb_profiles() {
    let presentation = test_presentation();
    assert!(!presentation.item_visible(MenuItem::SgbBorder));

    let mut sgb_presentation = MenuPresentation {
        console_model: DesktopConsoleModel::SuperGameBoy,
        ..presentation
    };
    assert!(sgb_presentation.item_visible(MenuItem::SgbBorder));
    assert_eq!(
        sgb_presentation.item_label(MenuItem::SgbBorder),
        "BORDER ON"
    );
    sgb_presentation.show_sgb_border = false;
    assert_eq!(
        sgb_presentation.item_label(MenuItem::SgbBorder),
        "BORDER OFF"
    );

    let mut menu = OverlayMenuState::default();
    open_system_menu(&mut menu, sgb_presentation);
    select_visible_item(&mut menu, sgb_presentation, MenuItem::SgbBorder);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, sgb_presentation),
        Some(MenuAction::ToggleSgbBorder)
    );

    let sgb2_presentation = MenuPresentation {
        console_model: DesktopConsoleModel::SuperGameBoy2,
        ..presentation
    };
    assert!(sgb2_presentation.item_visible(MenuItem::SgbBorder));
}

#[test]
fn system_submenu_cycles_sgb_video_standard_only_for_original_sgb() {
    let presentation = test_presentation();
    assert!(!presentation.item_visible(MenuItem::SgbVideoStandard));

    let mut sgb_presentation = MenuPresentation {
        console_model: DesktopConsoleModel::SuperGameBoy,
        ..presentation
    };
    assert!(sgb_presentation.item_visible(MenuItem::SgbVideoStandard));
    assert!(sgb_presentation.item_enabled(MenuItem::SgbVideoStandard));
    assert_eq!(
        sgb_presentation.item_label(MenuItem::SgbVideoStandard),
        "VIDEO NTSC"
    );
    sgb_presentation.sgb_video_standard = SgbVideoStandard::Pal;
    assert_eq!(
        sgb_presentation.item_label(MenuItem::SgbVideoStandard),
        "VIDEO PAL"
    );

    let mut menu = OverlayMenuState::default();
    open_system_menu(&mut menu, sgb_presentation);
    select_visible_item(&mut menu, sgb_presentation, MenuItem::SgbVideoStandard);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, sgb_presentation),
        Some(MenuAction::CycleSgbVideoStandard)
    );

    let sgb2_presentation = MenuPresentation {
        console_model: DesktopConsoleModel::SuperGameBoy2,
        sgb_video_standard: SgbVideoStandard::Pal,
        ..presentation
    };
    assert!(sgb2_presentation.item_visible(MenuItem::SgbVideoStandard));
    assert!(!sgb2_presentation.item_enabled(MenuItem::SgbVideoStandard));
    assert_eq!(
        sgb2_presentation.item_label(MenuItem::SgbVideoStandard),
        "VIDEO NTSC"
    );
}

#[test]
fn boot_rom_submenu_exposes_boot_path_and_verify_actions() {
    let presentation = test_presentation();
    let mut menu = OverlayMenuState::default();
    open_boot_rom_menu(&mut menu, presentation);

    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::SelectBootRomDirectoryPath)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleBootRomVerify)
    );
}

#[test]
fn system_submenu_exposes_save_actions() {
    let presentation = MenuPresentation {
        external_save_available: true,
        external_save_import_available: true,
        ..test_presentation()
    };
    let mut menu = OverlayMenuState::default();
    open_save_menu(&mut menu, presentation);

    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ExportSave)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ImportSave)
    );
    select_visible_item(&mut menu, presentation, MenuItem::SavesEnabled);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ToggleSavesEnabled)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::CycleSavePolicy)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::ClearSaveDirectoryPath)
    );
    assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, presentation),
        Some(MenuAction::SelectSaveDirectoryPath)
    );
}
