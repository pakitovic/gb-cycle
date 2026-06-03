use super::*;

#[test]
fn explicit_settings_path_override_wins_over_platform_defaults() {
    assert_eq!(
        resolve_desktop_settings_path_from_locations(
            Some(PathBuf::from("/tmp/custom-desktop-settings.toml").into_os_string()),
            Some(PathBuf::from("/Users/example-user").into_os_string()),
            None,
            Some(PathBuf::from("C:/Users/example-user/AppData/Roaming").into_os_string()),
        ),
        Some(PathBuf::from("/tmp/custom-desktop-settings.toml"))
    );
}

#[cfg(target_os = "macos")]
#[test]
fn platform_default_settings_path_matches_macos_conventions() {
    assert_eq!(
        resolve_desktop_settings_path_from_locations(
            None,
            Some(PathBuf::from("/Users/example-user").into_os_string()),
            None,
            None,
        ),
        Some(PathBuf::from(
            "/Users/example-user/Library/Application Support/gb-cycle/desktop-settings.toml"
        ))
    );
}

#[cfg(target_os = "windows")]
#[test]
fn platform_default_settings_path_matches_windows_conventions() {
    assert_eq!(
        resolve_desktop_settings_path_from_locations(
            None,
            None,
            None,
            Some(PathBuf::from("C:/Users/example-user/AppData/Roaming").into_os_string()),
        ),
        Some(PathBuf::from(
            "C:/Users/example-user/AppData/Roaming/gb-cycle/desktop-settings.toml"
        ))
    );
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
#[test]
fn platform_default_settings_path_matches_xdg_conventions() {
    assert_eq!(
        resolve_desktop_settings_path_from_locations(
            None,
            Some(PathBuf::from("/home/example-user").into_os_string()),
            Some(PathBuf::from("/tmp/xdg-config").into_os_string()),
            None,
        ),
        Some(PathBuf::from(
            "/tmp/xdg-config/gb-cycle/desktop-settings.toml"
        ))
    );
    assert_eq!(
        resolve_desktop_settings_path_from_locations(
            None,
            Some(PathBuf::from("/home/example-user").into_os_string()),
            None,
            None,
        ),
        Some(PathBuf::from(
            "/home/example-user/.config/gb-cycle/desktop-settings.toml"
        ))
    );
    assert_eq!(
        resolve_desktop_settings_path_from_locations(None, None, None, None),
        None
    );
}

#[test]
fn missing_settings_file_falls_back_to_defaults() {
    let path = unique_test_path("missing-settings");
    let settings = PersistedDesktopSettings::load(&path).expect("missing settings should default");

    assert_eq!(settings.version, DESKTOP_SETTINGS_VERSION);
    assert_eq!(
        settings.machine_state,
        gb_desktop::MachineStateOptions::default()
    );
    assert_eq!(settings.rewind, RewindOptions::default());
    assert_eq!(settings.video, DesktopConfig::default().video);
    assert_eq!(settings.input, DesktopConfig::default().input);
    assert_eq!(settings.last_open_directory, None);
    assert!(settings.recent_roms.is_empty());
}

#[test]
fn loading_settings_without_rewind_block_uses_rewind_defaults() {
    let path = unique_test_path("missing-rewind-block");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("settings parent should be creatable");
    }
    fs::write(&path, "version = 1\n")
        .expect("legacy settings without rewind block should be writable");

    let settings = PersistedDesktopSettings::load(&path).expect("legacy settings should reload");

    assert_eq!(settings.rewind, RewindOptions::default());
}

#[test]
fn loading_settings_without_machine_state_block_uses_state_defaults() {
    let path = unique_test_path("missing-machine-state-block");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("settings parent should be creatable");
    }
    fs::write(&path, "version = 1\n")
        .expect("legacy settings without machine_state block should be writable");

    let settings = PersistedDesktopSettings::load(&path).expect("legacy settings should reload");

    assert_eq!(
        settings.machine_state,
        gb_desktop::MachineStateOptions::default()
    );
}

#[test]
fn loading_video_block_without_display_palette_uses_game_boy_palette() {
    let path = unique_test_path("missing-display-palette");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("settings parent should be creatable");
    }
    fs::write(
        &path,
        "\
version = 1

[video]
window_scale = 5
integer_scale = true
presentation_filter = false
show_background = true
show_window = true
show_objects = true
vsync = true
fullscreen = false
show_performance_hud = true
",
    )
    .expect("legacy video settings should be writable");

    let settings =
        PersistedDesktopSettings::load(&path).expect("legacy video settings should reload");

    assert_eq!(settings.video.window_scale, 5);
    assert_eq!(
        settings.video.display_palette,
        DesktopDisplayPalette::GameBoy
    );
    assert_eq!(settings.video.frame_blending, DesktopFrameBlendingMode::Off);
    assert!(settings.video.show_sgb_border);
    assert!(!settings.video.show_cgb_infrared_helper);
}

#[test]
fn display_palette_settings_round_trip_each_palette() {
    for (index, display_palette) in [
        DesktopDisplayPalette::Grey,
        DesktopDisplayPalette::GameBoy,
        DesktopDisplayPalette::Pocket,
        DesktopDisplayPalette::Light,
    ]
    .into_iter()
    .enumerate()
    {
        let path = unique_test_path(&format!("display-palette-{index}"));
        let mut settings = PersistedDesktopSettings::default();
        settings.video.display_palette = display_palette;
        settings
            .save(&path)
            .expect("display palette setting should be writable");

        let reloaded =
            PersistedDesktopSettings::load(&path).expect("display palette should reload");
        assert_eq!(reloaded.video.display_palette, display_palette);
    }
}
