use super::*;

#[test]
fn frame_blending_settings_round_trip_each_mode() {
    for (index, frame_blending) in [DesktopFrameBlendingMode::Off, DesktopFrameBlendingMode::On]
        .into_iter()
        .enumerate()
    {
        let path = unique_test_path(&format!("frame-blending-{index}"));
        let mut settings = PersistedDesktopSettings::default();
        settings.video.frame_blending = frame_blending;
        settings
            .save(&path)
            .expect("frame blending setting should be writable");

        let reloaded = PersistedDesktopSettings::load(&path).expect("frame blending should reload");
        assert_eq!(reloaded.video.frame_blending, frame_blending);
    }
}

#[test]
fn frame_blending_settings_reject_removed_simple_and_lcd_modes() {
    for removed_mode in ["simple", "lcd"] {
        let path = unique_test_path(&format!("frame-blending-removed-{removed_mode}"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("settings parent should be creatable");
        }
        fs::write(
            &path,
            format!(
                "\
version = 1

[video]
frame_blending = \"{removed_mode}\"
"
            ),
        )
        .expect("removed frame blending setting should be writable");

        let parse_error = PersistedDesktopSettings::load(&path)
            .expect_err("removed frame blending values should fail");
        assert!(parse_error.contains("failed to parse desktop settings"));
    }
}

#[test]
fn loading_rewind_block_without_speed_uses_speed_default() {
    let path = unique_test_path("missing-rewind-speed");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("settings parent should be creatable");
    }
    fs::write(
        &path,
        "\
version = 1

[rewind]
enabled = false
history_seconds = 20
subframes_per_frame = 4
max_memory_mib = 128
",
    )
    .expect("legacy rewind settings should be writable");

    let settings =
        PersistedDesktopSettings::load(&path).expect("legacy rewind settings should reload");

    assert_eq!(
        settings.rewind,
        RewindOptions {
            enabled: false,
            history_seconds: 20,
            subframes_per_frame: 4,
            max_memory_mib: 128,
            speed_multiplier: RewindOptions::default().speed_multiplier,
        }
    );
}

#[test]
fn settings_store_base_config_applies_persisted_host_preferences() {
    let path = unique_test_path("applies-settings");
    let mut settings = PersistedDesktopSettings::default();
    settings.launch.console_model = PersistedDesktopConsoleModel::GameBoyPocket;
    settings.launch.revision = PersistedHardwareRevision::CpuCgbE;
    settings.launch.sgb_video_standard = PersistedSgbVideoStandard::Pal;
    settings.launch.startup_mode = PersistedStartupMode::Real;
    settings.launch.execution_mode = PersistedExecutionMode::Permissive;
    settings.boot_rom.search_path = Some(PathBuf::from("/tmp/firmware/mgb_boot.bin"));
    settings.boot_rom.verification = PersistedBootRomVerificationMode::Warn;
    settings.saves.enabled = false;
    settings.saves.directory_policy =
        PersistedSaveDirectoryPolicy::Custom(PathBuf::from("/tmp/saves"));
    settings.saves.flush_policy = DesktopSaveFlushPolicy::OnClose;
    settings.machine_state.autoload_slot = Some(3);
    settings.rewind = RewindOptions {
        enabled: false,
        history_seconds: 20,
        subframes_per_frame: 2,
        max_memory_mib: 128,
        speed_multiplier: 4,
    };
    settings.video.window_scale = 6;
    settings.video.integer_scale = false;
    settings.video.presentation_filter = true;
    settings.video.frame_blending = DesktopFrameBlendingMode::On;
    settings.video.display_palette = DesktopDisplayPalette::Pocket;
    settings.video.show_background = false;
    settings.video.show_window = false;
    settings.video.show_objects = false;
    settings.video.sgb_border = SgbBorderPresentationMode::Off;
    settings.video.fullscreen = true;
    settings.video.show_performance_hud = false;
    settings.video.show_cgb_infrared_helper = true;
    settings.video.vsync = false;
    settings.audio.volume_percent = 75;
    settings.audio.muted = true;
    settings.input.keyboard.joypad.a = DesktopKey::Space;
    settings.input.keyboard.menu.confirm = DesktopKey::X;
    settings.input.keyboard.hotkeys.pause = DesktopKey::X;
    settings.input.gamepad.directional_source = GamepadDirectionalSource::LeftStickOnly;
    settings.input.gamepad.gyro_mode = GamepadGyroMode::PadInput;
    settings.input.gamepad.rumble_mode = GamepadRumbleMode::Weak;
    settings.input.gamepad.bindings.a = GamepadButtonBinding::North;
    settings.input.gamepad.actions.rewind = Some(GamepadButtonBinding::LeftTrigger);
    settings.input.gamepad.actions.fast_forward = Some(GamepadButtonBinding::RightTrigger);
    settings.input.gamepad.menu.cancel = GamepadButtonBinding::West;
    settings.input.gamepad.preferred_device = PreferredGamepadIdentity {
        name: Some("Nintendo Switch Pro Controller".to_string()),
        path: Some("bluetooth:vendor=057e,product=2009".to_string()),
    };
    settings
        .save(&path)
        .expect("settings file should be writable");

    let store = DesktopSettingsStore {
        path: Some(path.clone()),
        settings: PersistedDesktopSettings::load(&path).expect("saved settings should reload"),
    };
    let config = store.base_config();

    assert_eq!(
        config.launch.console_model,
        gb_desktop::DesktopConsoleModel::GameBoyPocket
    );
    assert_eq!(config.launch.revision, HardwareRevision::CpuMgb);
    assert_eq!(config.launch.sgb_video_standard, SgbVideoStandard::Pal);
    assert_eq!(config.launch.startup_mode, StartupMode::RealBoot);
    assert_eq!(config.launch.execution_mode, ExecutionMode::Permissive);
    assert_eq!(
        config.boot_rom.search_path,
        Some(PathBuf::from("/tmp/firmware/mgb_boot.bin"))
    );
    assert_eq!(
        config.boot_rom.verification,
        gb_desktop::BootRomVerificationMode::Warn
    );
    assert!(!config.saves.enabled);
    assert_eq!(
        config.saves.directory_policy,
        SaveDirectoryPolicy::Custom(PathBuf::from("/tmp/saves"))
    );
    assert_eq!(config.saves.flush_policy, DesktopSaveFlushPolicy::OnClose);
    assert_eq!(config.machine_state.autoload_slot, Some(3));
    assert_eq!(
        config.rewind,
        RewindOptions {
            enabled: false,
            history_seconds: 20,
            subframes_per_frame: 2,
            max_memory_mib: 128,
            speed_multiplier: 4,
        }
    );
    assert_eq!(config.video.window_scale, 6);
    assert!(!config.video.integer_scale);
    assert!(config.video.presentation_filter);
    assert_eq!(config.video.frame_blending, DesktopFrameBlendingMode::On);
    assert_eq!(config.video.display_palette, DesktopDisplayPalette::Pocket);
    assert!(!config.video.show_background);
    assert!(!config.video.show_window);
    assert!(!config.video.show_objects);
    assert_eq!(config.video.sgb_border, SgbBorderPresentationMode::Off);
    assert!(config.video.fullscreen);
    assert!(!config.video.show_performance_hud);
    assert!(config.video.show_cgb_infrared_helper);
    assert!(!config.video.vsync);
    assert_eq!(config.audio.volume_percent, 75);
    assert!(store.audio_muted());
    assert_eq!(config.input.keyboard.joypad.a, DesktopKey::Space);
    assert_eq!(config.input.keyboard.menu.confirm, DesktopKey::X);
    assert_eq!(config.input.keyboard.hotkeys.pause, DesktopKey::X);
    assert_eq!(
        config.input.gamepad.directional_source,
        GamepadDirectionalSource::LeftStickOnly
    );
    assert_eq!(config.input.gamepad.gyro_mode, GamepadGyroMode::PadInput);
    assert_eq!(config.input.gamepad.rumble_mode, GamepadRumbleMode::Weak);
    assert_eq!(config.input.gamepad.bindings.a, GamepadButtonBinding::North);
    assert_eq!(
        config.input.gamepad.actions.rewind,
        Some(GamepadButtonBinding::LeftTrigger)
    );
    assert_eq!(
        config.input.gamepad.actions.fast_forward,
        Some(GamepadButtonBinding::RightTrigger)
    );
    assert_eq!(config.input.gamepad.menu.cancel, GamepadButtonBinding::West);
    assert_eq!(
        config.input.gamepad.preferred_device,
        PreferredGamepadIdentity {
            name: Some("Nintendo Switch Pro Controller".to_string()),
            path: Some("bluetooth:vendor=057e,product=2009".to_string()),
        }
    );
    assert!(store.recent_roms().is_empty());
}
