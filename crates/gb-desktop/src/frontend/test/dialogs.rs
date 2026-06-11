use super::*;

#[test]
fn open_rom_dialog_result_uses_the_first_selected_path() {
    assert_eq!(
        map_path_dialog_result(Ok(vec![
            PathBuf::from("/tmp/tetris.gb"),
            PathBuf::from("/tmp/other.gb"),
        ])),
        PathDialogResult::Selected(PathBuf::from("/tmp/tetris.gb"))
    );
}

#[test]
fn open_rom_dialog_result_preserves_cancel_as_a_non_selection() {
    assert_eq!(
        map_path_dialog_result(Err(DialogError::Canceled)),
        PathDialogResult::Canceled
    );
}

#[test]
fn open_rom_dialog_filters_include_supported_game_boy_extensions() {
    assert_eq!(ROM_FILE_DIALOG_FILTERS.len(), 1);
    assert_eq!(ROM_FILE_DIALOG_FILTERS[0].name, "Game Boy ROMs");
    assert_eq!(ROM_FILE_DIALOG_FILTERS[0].pattern, "gb;gbc");
}

#[test]
fn camera_image_dialog_filters_include_png_and_all_files() {
    assert_eq!(CAMERA_IMAGE_FILE_DIALOG_FILTERS[0].name, "PNG images");
    assert_eq!(CAMERA_IMAGE_FILE_DIALOG_FILTERS[0].pattern, "png");
    assert_eq!(CAMERA_IMAGE_FILE_DIALOG_FILTERS[1].name, "All files");
    assert_eq!(CAMERA_IMAGE_FILE_DIALOG_FILTERS[1].pattern, "*");
}

#[test]
fn desktop_session_path_helpers_cover_linked_and_directory_fallbacks() {
    let root = temp_test_root("desktop-session-path-helpers");
    let current_dir = root.join("current");
    let last_open = root.join("last-open");
    let primary_path = root.join("roms").join("primary.gb");
    let linked_path = root.join("linked").join("secondary.gb");
    let mut session = super::super::DesktopSession {
        config: DesktopConfig::default(),
        test_runner: false,
        benchmark: None,
        current_dir: current_dir.clone(),
        loaded_rom: None,
        linked_secondary_rom: None,
        dmg07_player_count: None,
        cgb_infrared_link_active: false,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: PokemonPikachuColorGift::default(),
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: PokemonMysteryGiftKind::default(),
        pokemon_mystery_gift_code: PokemonMysteryGiftCode::default(),
        last_open_directory: None,
        recent_roms: vec![primary_path.clone()],
        pocket_camera_frame: None,
        external_port_selection: DesktopExternalPortSelection::None,
    };

    assert_eq!(session.linked_secondary_rom_path(), None);
    assert_eq!(session.linked_secondary_rom_bytes(), None);
    assert_eq!(session.rom_directory_hint(), current_dir.as_path());
    assert_eq!(session.recent_roms(), [primary_path.clone()].as_slice());

    session.last_open_directory = Some(last_open.clone());
    assert_eq!(session.rom_directory_hint(), last_open.as_path());

    session.loaded_rom = Some(super::super::LoadedRom {
        path: primary_path.clone(),
        bytes: vec![0x01, 0x02],
    });
    session.linked_secondary_rom = Some(super::super::LoadedRom {
        path: linked_path.clone(),
        bytes: vec![0x03, 0x04],
    });

    assert_eq!(
        session.rom_directory_hint(),
        primary_path
            .parent()
            .expect("primary ROM should have a parent")
    );
    assert_eq!(
        session.linked_secondary_rom_path(),
        Some(linked_path.as_path())
    );
    assert_eq!(
        session.linked_secondary_rom_bytes(),
        Some([0x03, 0x04].as_slice())
    );
}

#[test]
fn sdl_dialog_default_location_keeps_macos_directory_hints_inside_the_directory() {
    let root = temp_test_root("sdl-dialog-default-location");
    let directory = root.join("roms");
    fs::create_dir_all(&directory).expect("dialog directory should be creatable");
    let file_path = directory.join("tetris.gb");
    fs::write(&file_path, b"rom").expect("dialog file should be writable");

    let directory_location = super::super::sdl_dialog_default_location(&directory);
    let file_location = super::super::sdl_dialog_default_location(&file_path);

    #[cfg(target_os = "macos")]
    {
        let mut expected_directory = directory.clone();
        expected_directory.push("");
        assert_eq!(
            directory_location.as_os_str(),
            expected_directory.as_os_str()
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(directory_location.as_os_str(), directory.as_os_str());
    }
    assert_eq!(file_location.as_os_str(), file_path.as_os_str());
}

#[test]
fn path_selection_dialog_reports_disconnected_results_as_empty() {
    let mut dialog = super::super::PathSelectionDialog::new();
    dialog.pending = true;
    let (replacement_sender, _replacement_receiver) = std::sync::mpsc::channel();
    let original_sender = std::mem::replace(&mut dialog.sender, replacement_sender);

    drop(original_sender);

    assert_eq!(dialog.take_result(), None);
    assert!(!dialog.is_pending());
}

#[test]
fn system_option_cycle_helpers_wrap_in_the_expected_order() {
    assert_eq!(
        next_console_model(DesktopConsoleModel::GameBoy),
        DesktopConsoleModel::GameBoyPocket
    );
    assert_eq!(
        next_console_model(DesktopConsoleModel::GameBoyPocket),
        DesktopConsoleModel::GameBoyLight
    );
    assert_eq!(
        next_console_model(DesktopConsoleModel::GameBoyLight),
        DesktopConsoleModel::GameBoyColor
    );
    assert_eq!(
        next_console_model(DesktopConsoleModel::GameBoyColor),
        DesktopConsoleModel::GameBoyAdvance
    );
    assert_eq!(
        next_console_model(DesktopConsoleModel::GameBoyAdvance),
        DesktopConsoleModel::SuperGameBoy
    );
    assert_eq!(
        next_console_model(DesktopConsoleModel::SuperGameBoy),
        DesktopConsoleModel::SuperGameBoy2
    );
    assert_eq!(
        next_console_model(DesktopConsoleModel::SuperGameBoy2),
        DesktopConsoleModel::GameBoy
    );
    assert_eq!(
        next_revision(DesktopConsoleModel::GameBoyColor, HardwareRevision::CpuCgb0),
        HardwareRevision::CpuCgbC
    );
    assert_eq!(
        next_revision(DesktopConsoleModel::GameBoyColor, HardwareRevision::CpuCgbC),
        HardwareRevision::CpuCgbD
    );
    assert_eq!(
        next_revision(DesktopConsoleModel::GameBoyColor, HardwareRevision::CpuCgbD),
        HardwareRevision::CpuCgbE
    );
    assert_eq!(
        next_revision(DesktopConsoleModel::GameBoyColor, HardwareRevision::CpuCgbE),
        HardwareRevision::CpuCgb0
    );
    assert_eq!(
        next_revision(
            DesktopConsoleModel::GameBoyAdvance,
            HardwareRevision::CpuAgb0
        ),
        HardwareRevision::CpuAgbA
    );
    assert_eq!(
        next_revision(
            DesktopConsoleModel::GameBoyAdvance,
            HardwareRevision::CpuAgbA
        ),
        HardwareRevision::CpuAgb0
    );
    assert_eq!(
        next_revision(DesktopConsoleModel::GameBoy, HardwareRevision::DmgCpu0),
        HardwareRevision::DmgCpuC
    );
    assert_eq!(
        next_revision(DesktopConsoleModel::GameBoy, HardwareRevision::DmgCpuC),
        HardwareRevision::DmgCpu0
    );
    assert_eq!(
        next_revision(DesktopConsoleModel::GameBoyPocket, HardwareRevision::CpuMgb),
        HardwareRevision::CpuMgb
    );
    assert_eq!(
        next_revision(DesktopConsoleModel::SuperGameBoy, HardwareRevision::DmgCpu0),
        HardwareRevision::DmgCpuC
    );
    assert_eq!(
        next_revision(DesktopConsoleModel::SuperGameBoy, HardwareRevision::DmgCpuC),
        HardwareRevision::DmgCpuC
    );
    assert_eq!(
        next_sgb_video_standard(SgbVideoStandard::Ntsc),
        SgbVideoStandard::Pal
    );
    assert_eq!(
        next_sgb_video_standard(SgbVideoStandard::Pal),
        SgbVideoStandard::Ntsc
    );
    assert_eq!(
        next_startup_mode(StartupMode::SkipBoot),
        StartupMode::CustomBoot
    );
    assert_eq!(
        next_startup_mode(StartupMode::CustomBoot),
        StartupMode::RealBoot
    );
    assert_eq!(
        next_startup_mode(StartupMode::RealBoot),
        StartupMode::SkipBoot
    );
    assert_eq!(
        next_execution_mode(ExecutionMode::Strict),
        ExecutionMode::Permissive
    );
    assert_eq!(
        next_execution_mode(ExecutionMode::Permissive),
        ExecutionMode::Experimental
    );
    assert_eq!(
        next_execution_mode(ExecutionMode::Experimental),
        ExecutionMode::Strict
    );
    assert_eq!(
        next_boot_rom_verification_mode(BootRomVerificationMode::Strict),
        BootRomVerificationMode::Warn
    );
    assert_eq!(
        next_boot_rom_verification_mode(BootRomVerificationMode::Warn),
        BootRomVerificationMode::Off
    );
    assert_eq!(
        next_boot_rom_verification_mode(BootRomVerificationMode::Off),
        BootRomVerificationMode::Strict
    );
    assert_eq!(
        next_save_flush_policy(DesktopSaveFlushPolicy::Manual),
        DesktopSaveFlushPolicy::OnClose
    );
    assert_eq!(
        next_save_flush_policy(DesktopSaveFlushPolicy::OnClose),
        DesktopSaveFlushPolicy::OnWrite
    );
    assert_eq!(
        next_save_flush_policy(DesktopSaveFlushPolicy::OnWrite),
        DesktopSaveFlushPolicy::Debounced
    );
    assert_eq!(
        next_save_flush_policy(DesktopSaveFlushPolicy::Debounced),
        DesktopSaveFlushPolicy::Manual
    );
    assert_eq!(
        next_gamepad_rumble_mode(GamepadRumbleMode::Off),
        GamepadRumbleMode::Strong
    );
    assert_eq!(
        next_gamepad_rumble_mode(GamepadRumbleMode::Strong),
        GamepadRumbleMode::Weak
    );
    assert_eq!(
        next_gamepad_rumble_mode(GamepadRumbleMode::Weak),
        GamepadRumbleMode::Off
    );
    assert_eq!(
        next_gamepad_gyro_mode(GamepadGyroMode::Off),
        GamepadGyroMode::PadGyro
    );
    assert_eq!(
        next_gamepad_gyro_mode(GamepadGyroMode::PadGyro),
        GamepadGyroMode::PadInput
    );
    assert_eq!(
        next_gamepad_gyro_mode(GamepadGyroMode::PadInput),
        GamepadGyroMode::Off
    );
    assert_eq!(next_fast_forward_speed_multiplier(4), 8);
    assert_eq!(next_fast_forward_speed_multiplier(8), 16);
    assert_eq!(next_fast_forward_speed_multiplier(16), 4);
}
