use super::*;

#[test]
fn recent_rom_history_keeps_the_latest_twelve_entries() {
    let path = unique_test_path("recent-rom-capacity");
    let mut store = DesktopSettingsStore {
        path: Some(path.clone()),
        settings: PersistedDesktopSettings::default(),
    };

    for index in 1..=14 {
        store
            .remember_loaded_rom(Path::new(&format!("/tmp/roms/ROM{index:02}.gb")))
            .expect("recent ROM should persist");
    }

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert_eq!(reloaded.recent_roms.len(), MAX_RECENT_ROMS);
    assert_eq!(reloaded.recent_roms[0], PathBuf::from("/tmp/roms/ROM14.gb"));
    assert_eq!(
        reloaded.recent_roms[11],
        PathBuf::from("/tmp/roms/ROM03.gb")
    );
}

#[test]
fn removing_a_recent_rom_updates_the_persisted_history() {
    let path = unique_test_path("remove-recent-rom");
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
        .remove_recent_rom(Path::new("/tmp/roms/Tetris.gb"))
        .expect("stale ROM should be removable");

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert_eq!(
        reloaded.recent_roms,
        vec![PathBuf::from("/tmp/roms/DrMario.gb")]
    );
}

#[test]
fn clearing_recent_roms_updates_the_persisted_history() {
    let path = unique_test_path("clear-recent-roms");
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
        .clear_recent_roms()
        .expect("recent ROM history should clear");

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert!(reloaded.recent_roms.is_empty());
}

#[test]
fn settings_path_env_var_name_stays_stable() {
    assert_eq!(
        DESKTOP_SETTINGS_PATH_ENV_VAR,
        "GB_CYCLE_DESKTOP_SETTINGS_PATH"
    );
}
