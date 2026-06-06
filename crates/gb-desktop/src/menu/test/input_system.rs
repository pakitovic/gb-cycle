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
    select_visible_item(&mut menu, cgb_presentation, MenuItem::HardwareRevision);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, cgb_presentation),
        Some(MenuAction::CycleHardwareRevision)
    );
    select_visible_item(&mut menu, cgb_presentation, MenuItem::StartupMode);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, cgb_presentation),
        Some(MenuAction::CycleStartupMode)
    );
    select_visible_item(&mut menu, cgb_presentation, MenuItem::ExecutionMode);
    assert_eq!(
        menu.handle_input(MenuInput::Confirm, cgb_presentation),
        Some(MenuAction::CycleExecutionMode)
    );
}

#[test]
fn system_submenu_toggles_sgb_border_for_sgb_and_handheld_profiles() {
    let presentation = test_presentation();
    assert!(presentation.item_visible(MenuItem::SgbBorder));
    assert_eq!(presentation.item_label(MenuItem::SgbBorder), "BORDER AUTO");

    let mut sgb_presentation = MenuPresentation {
        console_model: DesktopConsoleModel::SuperGameBoy,
        ..presentation
    };
    assert!(sgb_presentation.item_visible(MenuItem::SgbBorder));
    assert_eq!(
        sgb_presentation.item_label(MenuItem::SgbBorder),
        "BORDER AUTO"
    );
    sgb_presentation.sgb_border = SgbBorderPresentationMode::Off;
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

    for console_model in [
        DesktopConsoleModel::GameBoy,
        DesktopConsoleModel::GameBoyPocket,
        DesktopConsoleModel::GameBoyLight,
        DesktopConsoleModel::GameBoyColor,
        DesktopConsoleModel::GameBoyAdvance,
    ] {
        let handheld_presentation = MenuPresentation {
            console_model,
            ..presentation
        };
        assert!(handheld_presentation.item_visible(MenuItem::SgbBorder));
    }
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
