use super::*;

#[test]
fn loading_settings_migrates_old_hotkey_defaults() {
    let path = unique_test_path("migrate-reset-hotkey");
    let mut settings = PersistedDesktopSettings::default();
    settings.input.keyboard.hotkeys.reset = DesktopKey::R;
    settings.input.keyboard.hotkeys.rewind = DesktopKey::F6;
    settings.input.keyboard.hotkeys.save_battery = DesktopKey::F5;
    settings
        .save(&path)
        .expect("old reset hotkey settings should save");

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert_eq!(reloaded.input.keyboard.hotkeys.reset, DesktopKey::F12);
    assert_eq!(
        reloaded.input.keyboard.hotkeys.rewind,
        DesktopKey::LeftShift
    );
    assert_eq!(reloaded.input.keyboard.hotkeys.save_battery, DesktopKey::F9);
}

#[test]
fn loading_settings_migrates_invalid_machine_state_autoload_slot() {
    let path = unique_test_path("migrate-invalid-state-autoload");
    let mut settings = PersistedDesktopSettings::default();
    settings.machine_state.autoload_slot = Some(9);
    settings
        .save(&path)
        .expect("invalid autoload slot settings should save");

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert_eq!(reloaded.machine_state.autoload_slot, None);
}
