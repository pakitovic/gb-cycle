use super::*;

#[test]
fn settings_store_base_config_applies_revision_for_cgb_model() {
    let path = unique_test_path("applies-cgb-revision");
    let mut settings = PersistedDesktopSettings::default();
    settings.launch.console_model = PersistedDesktopConsoleModel::GameBoyColor;
    settings.launch.revision = PersistedHardwareRevision::CpuCgbE;
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
        gb_desktop::DesktopConsoleModel::GameBoyColor
    );
    assert_eq!(config.launch.revision, HardwareRevision::CpuCgbE);
}

#[test]
fn persisted_settings_load_reports_read_and_parse_failures() {
    let unreadable_path = unique_test_path("load-read-error");
    fs::create_dir_all(&unreadable_path).expect("directory-backed settings path should exist");
    let read_error = PersistedDesktopSettings::load(&unreadable_path)
        .expect_err("reading a directory as settings should fail");
    assert!(read_error.contains("failed to read desktop settings"));
    fs::remove_dir_all(&unreadable_path).expect("temp settings directory should be removable");

    let invalid_toml_path = unique_test_path("load-parse-error");
    if let Some(parent) = invalid_toml_path.parent() {
        fs::create_dir_all(parent).expect("invalid toml parent should be creatable");
    }
    fs::write(&invalid_toml_path, "version = 1\nrecent_roms = [")
        .expect("invalid desktop settings should be writable");
    let parse_error = PersistedDesktopSettings::load(&invalid_toml_path)
        .expect_err("invalid TOML should fail to parse");
    assert!(parse_error.contains("failed to parse desktop settings"));
}
