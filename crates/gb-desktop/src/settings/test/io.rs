use super::*;

#[test]
fn persisted_settings_save_reports_directory_and_write_failures() {
    let blocked_parent_file = unique_test_path("save-parent-error");
    if let Some(parent) = blocked_parent_file.parent() {
        fs::create_dir_all(parent).expect("blocked parent parent should be creatable");
    }
    fs::write(&blocked_parent_file, "not-a-directory").expect("blocking file should be writable");
    let create_dir_error = PersistedDesktopSettings::default()
        .save(&blocked_parent_file.join("desktop-settings.toml"))
        .expect_err("create_dir_all should fail when the parent is a file");
    assert!(create_dir_error.contains("failed to create desktop settings directory"));

    let unwritable_path = unique_test_path("save-write-error");
    fs::create_dir_all(&unwritable_path).expect("directory-backed settings path should exist");
    let write_error = PersistedDesktopSettings::default()
        .save(&unwritable_path)
        .expect_err("writing directly to a directory should fail");
    assert!(write_error.contains("failed to write desktop settings"));
    fs::remove_dir_all(&unwritable_path).expect("temp settings directory should be removable");
}

#[test]
fn settings_store_load_uses_the_env_override_and_defaults_missing_files() {
    let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let path = unique_test_path("load-env-override");
    let settings = PersistedDesktopSettings {
        recent_roms: vec![PathBuf::from("/tmp/roms/Kirby.gb")],
        last_open_directory: Some(PathBuf::from("/tmp/roms")),
        ..PersistedDesktopSettings::default()
    };
    settings
        .save(&path)
        .expect("settings file should be writable through the env override");

    unsafe {
        std::env::set_var(DESKTOP_SETTINGS_PATH_ENV_VAR, &path);
    }
    let loaded = DesktopSettingsStore::load().expect("settings store should load from env");
    assert_eq!(loaded.last_open_directory(), Some(Path::new("/tmp/roms")));
    assert_eq!(loaded.recent_roms(), &[PathBuf::from("/tmp/roms/Kirby.gb")]);

    let missing_path = unique_test_path("load-missing-env-override");
    unsafe {
        std::env::set_var(DESKTOP_SETTINGS_PATH_ENV_VAR, &missing_path);
    }
    let missing = DesktopSettingsStore::load().expect("missing env-backed settings should default");
    assert_eq!(missing.last_open_directory(), None);
    assert!(missing.recent_roms().is_empty());

    unsafe {
        std::env::remove_var(DESKTOP_SETTINGS_PATH_ENV_VAR);
    }
}

#[test]
fn settings_store_load_uses_in_memory_defaults_when_no_path_can_be_resolved() {
    let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
    unsafe {
        std::env::remove_var(DESKTOP_SETTINGS_PATH_ENV_VAR);
        std::env::remove_var("HOME");
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("APPDATA");
    }

    let store = DesktopSettingsStore::load().expect("settings store should fall back to defaults");
    assert_eq!(store.base_config(), DesktopConfig::default());
    assert!(!store.audio_muted());
    assert_eq!(store.last_open_directory(), None);
    assert!(store.recent_roms().is_empty());
}

#[test]
fn persisting_machine_preferences_updates_the_saved_settings() {
    let path = unique_test_path("persist-launch-boot");
    let mut store = DesktopSettingsStore {
        path: Some(path.clone()),
        settings: PersistedDesktopSettings::default(),
    };
    let mut config = DesktopConfig::default();
    config.launch.console_model = gb_desktop::DesktopConsoleModel::GameBoy;
    config.launch.sgb_video_standard = SgbVideoStandard::Pal;
    config.launch.startup_mode = StartupMode::RealBoot;
    config.launch.execution_mode = ExecutionMode::Experimental;
    config.boot_rom.search_path = Some(PathBuf::from("/tmp/firmware/dmg0_boot.bin"));
    config.boot_rom.verification = gb_desktop::BootRomVerificationMode::Off;
    config.saves.enabled = false;
    config.saves.directory_policy = SaveDirectoryPolicy::Custom(PathBuf::from("/tmp/gb-saves"));
    config.saves.flush_policy = DesktopSaveFlushPolicy::OnWrite;

    store
        .persist_machine_preferences(&config)
        .expect("machine settings should persist");

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert_eq!(
        reloaded.launch.console_model,
        PersistedDesktopConsoleModel::GameBoy
    );
    assert_eq!(
        reloaded.launch.sgb_video_standard,
        PersistedSgbVideoStandard::Pal
    );
    assert_eq!(reloaded.launch.startup_mode, PersistedStartupMode::Real);
    assert_eq!(
        reloaded.launch.execution_mode,
        PersistedExecutionMode::Experimental
    );
    assert_eq!(
        reloaded.boot_rom.search_path,
        Some(PathBuf::from("/tmp/firmware/dmg0_boot.bin"))
    );
    assert_eq!(
        reloaded.boot_rom.verification,
        PersistedBootRomVerificationMode::Off
    );
    assert!(!reloaded.saves.enabled);
    assert_eq!(
        reloaded.saves.directory_policy,
        PersistedSaveDirectoryPolicy::Custom(PathBuf::from("/tmp/gb-saves"))
    );
    assert_eq!(reloaded.saves.flush_policy, DesktopSaveFlushPolicy::OnWrite);
}
