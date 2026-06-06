use super::*;

#[test]
fn runtime_updates_persist_muted_fullscreen_and_last_open_directory() {
    let path = unique_test_path("runtime-updates");
    let mut store = DesktopSettingsStore {
        path: Some(path.clone()),
        settings: PersistedDesktopSettings::default(),
    };

    store
        .set_fullscreen(true)
        .expect("fullscreen toggle should persist");
    store
        .set_window_scale(6)
        .expect("window scale should persist");
    store
        .set_integer_scale(false)
        .expect("integer scale should persist");
    store
        .set_presentation_filter(true)
        .expect("presentation filter should persist");
    store
        .set_frame_blending(DesktopFrameBlendingMode::On)
        .expect("frame blending should persist");
    store
        .set_display_palette(DesktopDisplayPalette::Light)
        .expect("display palette should persist");
    store
        .set_show_background(false)
        .expect("background layer visibility should persist");
    store
        .set_show_window(false)
        .expect("window layer visibility should persist");
    store
        .set_show_objects(false)
        .expect("object layer visibility should persist");
    store
        .set_show_performance_hud(true)
        .expect("performance HUD visibility should persist");
    store
        .set_sgb_border(SgbBorderPresentationMode::Off)
        .expect("SGB border visibility should persist");
    store
        .set_show_cgb_infrared_helper(true)
        .expect("CGB IR helper visibility should persist");
    store.set_vsync(false).expect("vsync toggle should persist");
    store
        .set_rewind_options(RewindOptions {
            enabled: false,
            history_seconds: 30,
            subframes_per_frame: 4,
            max_memory_mib: 512,
            speed_multiplier: 1,
        })
        .expect("rewind options should persist");
    store
        .set_audio_muted(true)
        .expect("audio mute toggle should persist");
    store
        .set_audio_volume_percent(75)
        .expect("audio volume should persist");
    store
        .remember_loaded_rom(Path::new("/tmp/roms/Tetris.gb"))
        .expect("loaded ROM directory should persist");
    store
        .set_gamepad_directional_source(GamepadDirectionalSource::LeftStickOnly)
        .expect("gamepad direction should persist");
    store
        .set_gamepad_gyro_mode(GamepadGyroMode::PadInput)
        .expect("gamepad gyro mode should persist");
    store
        .set_gamepad_rumble_mode(GamepadRumbleMode::Weak)
        .expect("gamepad rumble mode should persist");
    store
        .set_keyboard_joypad_bindings(JoypadKeyboardBindings {
            a: DesktopKey::LeftAlt,
            ..JoypadKeyboardBindings::default()
        })
        .expect("keyboard joypad bindings should persist");
    store
        .set_keyboard_menu_bindings(MenuKeyboardBindings {
            confirm: DesktopKey::Tab,
            ..MenuKeyboardBindings::default()
        })
        .expect("keyboard menu bindings should persist");
    store
        .set_preferred_gamepad_device(PreferredGamepadIdentity {
            name: Some("Nintendo Switch Pro Controller".to_string()),
            path: Some("bluetooth:vendor=057e,product=2009".to_string()),
        })
        .expect("preferred gamepad identity should persist");
    store
        .set_gamepad_menu_bindings(GamepadMenuBindings {
            cancel: GamepadButtonBinding::West,
            ..GamepadMenuBindings::default()
        })
        .expect("gamepad menu bindings should persist");
    store
        .set_keyboard_hotkey_bindings(HotkeyBindings {
            pause: DesktopKey::LeftGui,
            ..HotkeyBindings::default()
        })
        .expect("keyboard hotkey bindings should persist");

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    let persisted_text = fs::read_to_string(&path).expect("persisted settings text should reload");
    assert!(persisted_text.contains("sgb_border = \"off\""));
    assert!(!persisted_text.contains("show_sgb_border"));
    assert!(reloaded.video.fullscreen);
    assert_eq!(reloaded.video.window_scale, 6);
    assert!(!reloaded.video.integer_scale);
    assert!(reloaded.video.presentation_filter);
    assert_eq!(reloaded.video.frame_blending, DesktopFrameBlendingMode::On);
    assert_eq!(reloaded.video.display_palette, DesktopDisplayPalette::Light);
    assert!(!reloaded.video.show_background);
    assert!(!reloaded.video.show_window);
    assert!(!reloaded.video.show_objects);
    assert_eq!(reloaded.video.sgb_border, SgbBorderPresentationMode::Off);
    assert!(reloaded.video.show_performance_hud);
    assert!(reloaded.video.show_cgb_infrared_helper);
    assert!(!reloaded.video.vsync);
    assert_eq!(
        reloaded.rewind,
        RewindOptions {
            enabled: false,
            history_seconds: 30,
            subframes_per_frame: 4,
            max_memory_mib: 512,
            speed_multiplier: 1,
        }
    );
    assert_eq!(reloaded.audio.volume_percent, 75);
    assert!(reloaded.audio.muted);
    assert_eq!(
        reloaded.input.gamepad.directional_source,
        GamepadDirectionalSource::LeftStickOnly
    );
    assert_eq!(reloaded.input.gamepad.gyro_mode, GamepadGyroMode::PadInput);
    assert_eq!(reloaded.input.gamepad.rumble_mode, GamepadRumbleMode::Weak);
    assert_eq!(
        reloaded.input.gamepad.menu.cancel,
        GamepadButtonBinding::West
    );
    assert_eq!(
        reloaded.input.gamepad.preferred_device,
        PreferredGamepadIdentity {
            name: Some("Nintendo Switch Pro Controller".to_string()),
            path: Some("bluetooth:vendor=057e,product=2009".to_string()),
        }
    );
    assert_eq!(reloaded.input.keyboard.joypad.a, DesktopKey::LeftAlt);
    assert_eq!(reloaded.input.keyboard.menu.confirm, DesktopKey::Tab);
    assert_eq!(reloaded.input.keyboard.hotkeys.pause, DesktopKey::LeftGui);
    assert_eq!(
        reloaded.last_open_directory,
        Some(PathBuf::from("/tmp/roms"))
    );
    assert_eq!(
        reloaded.recent_roms,
        vec![PathBuf::from("/tmp/roms/Tetris.gb")]
    );
}

#[test]
fn reset_helpers_restore_default_video_audio_and_input_preferences() {
    let path = unique_test_path("reset-defaults");
    let mut store = DesktopSettingsStore {
        path: Some(path.clone()),
        settings: PersistedDesktopSettings::default(),
    };

    store
        .set_fullscreen(true)
        .expect("fullscreen toggle should persist");
    store
        .set_window_scale(6)
        .expect("window scale should persist");
    store
        .set_integer_scale(false)
        .expect("integer scale should persist");
    store
        .set_presentation_filter(true)
        .expect("presentation filter should persist");
    store
        .set_frame_blending(DesktopFrameBlendingMode::On)
        .expect("frame blending should persist");
    store
        .set_show_performance_hud(true)
        .expect("HUD visibility should persist");
    store
        .set_sgb_border(SgbBorderPresentationMode::Off)
        .expect("SGB border visibility should persist");
    store
        .set_show_cgb_infrared_helper(true)
        .expect("CGB IR helper visibility should persist");
    store.set_vsync(false).expect("vsync should persist");
    store
        .set_audio_muted(true)
        .expect("audio mute toggle should persist");
    store
        .set_audio_volume_percent(75)
        .expect("audio volume should persist");
    store
        .set_keyboard_joypad_bindings(JoypadKeyboardBindings {
            a: DesktopKey::Space,
            ..JoypadKeyboardBindings::default()
        })
        .expect("keyboard joypad bindings should persist");
    store
        .set_keyboard_menu_bindings(MenuKeyboardBindings {
            confirm: DesktopKey::X,
            ..MenuKeyboardBindings::default()
        })
        .expect("keyboard menu bindings should persist");
    store
        .set_keyboard_hotkey_bindings(HotkeyBindings {
            pause: DesktopKey::X,
            ..HotkeyBindings::default()
        })
        .expect("keyboard hotkey bindings should persist");

    store
        .reset_video_defaults(DesktopConsoleModel::GameBoy)
        .expect("video defaults should persist");
    store
        .reset_audio_defaults()
        .expect("audio defaults should persist");
    store
        .reset_input_defaults()
        .expect("input defaults should persist");

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert_eq!(reloaded.video, VideoOptions::default());
    assert_eq!(reloaded.audio, PersistedAudioSettings::default());
    assert_eq!(reloaded.input, InputOptions::default());
}

#[test]
fn reset_video_defaults_uses_the_active_console_model_palette() {
    let path = unique_test_path("reset-video-color-defaults");
    let mut store = DesktopSettingsStore {
        path: Some(path.clone()),
        settings: PersistedDesktopSettings::default(),
    };

    store
        .set_display_palette(DesktopDisplayPalette::Light)
        .expect("custom display palette should persist");
    store
        .reset_video_defaults(DesktopConsoleModel::GameBoyColor)
        .expect("video defaults should persist");

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert_eq!(
        reloaded.video,
        VideoOptions::default_for_console_model(DesktopConsoleModel::GameBoyColor)
    );
    assert_eq!(reloaded.video.display_palette, DesktopDisplayPalette::Grey);
}

#[test]
fn remembered_roms_are_deduplicated_and_kept_in_most_recent_order() {
    let path = unique_test_path("recent-roms");
    let mut store = DesktopSettingsStore {
        path: Some(path.clone()),
        settings: PersistedDesktopSettings::default(),
    };

    store
        .remember_loaded_rom(Path::new("/tmp/roms/Tetris.gb"))
        .expect("first ROM should persist");
    store
        .remember_loaded_rom(Path::new("/tmp/roms/DrMario.gb"))
        .expect("second ROM should persist");
    store
        .remember_loaded_rom(Path::new("/tmp/roms/Tetris.gb"))
        .expect("reloading a recent ROM should move it to the front");

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert_eq!(
        reloaded.recent_roms,
        vec![
            PathBuf::from("/tmp/roms/Tetris.gb"),
            PathBuf::from("/tmp/roms/DrMario.gb"),
        ]
    );
}
