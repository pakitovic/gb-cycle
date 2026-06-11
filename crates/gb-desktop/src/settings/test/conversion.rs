use super::*;

#[test]
fn persisted_conversion_helpers_round_trip_external_values() {
    assert_eq!(
        PersistedSaveDirectoryPolicy::from_external(&SaveDirectoryPolicy::Custom(PathBuf::from(
            "/tmp/saves"
        )))
        .to_external(),
        SaveDirectoryPolicy::Custom(PathBuf::from("/tmp/saves"))
    );
    assert_eq!(
        PersistedDesktopConsoleModel::from_external(gb_desktop::DesktopConsoleModel::GameBoyPocket)
            .to_external(),
        gb_desktop::DesktopConsoleModel::GameBoyPocket
    );
    assert_eq!(
        PersistedDesktopConsoleModel::from_external(
            gb_desktop::DesktopConsoleModel::GameBoyAdvance
        )
        .to_external(),
        gb_desktop::DesktopConsoleModel::GameBoyAdvance
    );
    assert_eq!(
        PersistedDesktopConsoleModel::from_external(gb_desktop::DesktopConsoleModel::SuperGameBoy)
            .to_external(),
        gb_desktop::DesktopConsoleModel::SuperGameBoy
    );
    assert_eq!(
        PersistedDesktopConsoleModel::from_external(gb_desktop::DesktopConsoleModel::SuperGameBoy2)
            .to_external(),
        gb_desktop::DesktopConsoleModel::SuperGameBoy2
    );
    assert_eq!(
        PersistedHardwareRevision::from_external(HardwareRevision::CpuCgbE).to_external(),
        HardwareRevision::CpuCgbE
    );
    assert_eq!(
        PersistedHardwareRevision::from_external(HardwareRevision::CpuAgb0).to_external(),
        HardwareRevision::CpuAgb0
    );
    assert_eq!(
        PersistedHardwareRevision::from_external(HardwareRevision::CpuAgbA).to_external(),
        HardwareRevision::CpuAgbA
    );
    assert_eq!(
        PersistedSgbVideoStandard::from_external(SgbVideoStandard::Pal).to_external(),
        SgbVideoStandard::Pal
    );
    assert_eq!(
        PersistedStartupMode::from_external(StartupMode::RealBoot).to_external(),
        StartupMode::RealBoot
    );
    assert_eq!(
        PersistedStartupMode::from_external(StartupMode::CustomBoot).to_external(),
        StartupMode::CustomBoot
    );
    assert_eq!(
        PersistedExecutionMode::from_external(ExecutionMode::Experimental).to_external(),
        ExecutionMode::Experimental
    );
    assert_eq!(
        PersistedBootRomVerificationMode::from_external(gb_desktop::BootRomVerificationMode::Warn)
            .to_external(),
        gb_desktop::BootRomVerificationMode::Warn
    );
}

#[test]
fn persisted_audio_settings_rebuild_audio_options() {
    let audio = PersistedAudioSettings {
        enabled: false,
        volume_percent: 75,
        output_sample_rate_hz: 44_100,
        buffer_frames: 256,
        muted: true,
    };

    assert_eq!(
        audio.audio_options(),
        gb_desktop::AudioOptions {
            enabled: false,
            volume_percent: 75,
            output_sample_rate_hz: 44_100,
            buffer_frames: 256,
        }
    );
}

#[test]
fn store_exposes_last_open_directory_and_gamepad_binding_updates() {
    let path = unique_test_path("gamepad-bindings");
    let mut store = DesktopSettingsStore {
        path: Some(path.clone()),
        settings: PersistedDesktopSettings::default(),
    };
    let bindings = gb_desktop::GamepadButtonBindings {
        a: GamepadButtonBinding::North,
        ..gb_desktop::GamepadButtonBindings::default()
    };

    store
        .set_gamepad_bindings(bindings)
        .expect("gamepad bindings should persist");
    store
        .set_gamepad_rumble_mode(GamepadRumbleMode::Weak)
        .expect("gamepad rumble mode should persist");
    store
        .set_gamepad_gyro_mode(GamepadGyroMode::PadGyro)
        .expect("gamepad gyro mode should persist");
    store
        .set_gamepad_gyro_mode(GamepadGyroMode::PadGyro)
        .expect("reapplying the same gyro mode should be a no-op");
    store
        .set_gamepad_rumble_mode(GamepadRumbleMode::Weak)
        .expect("reapplying the same rumble mode should be a no-op");
    store
        .remember_loaded_rom(Path::new("/tmp/roms/Alleyway.gb"))
        .expect("loaded ROM should update the last open directory");

    assert_eq!(store.last_open_directory(), Some(Path::new("/tmp/roms")));

    let reloaded = PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
    assert_eq!(reloaded.input.gamepad.bindings, bindings);
    assert_eq!(reloaded.input.gamepad.gyro_mode, GamepadGyroMode::PadGyro);
    assert_eq!(reloaded.input.gamepad.rumble_mode, GamepadRumbleMode::Weak);
    assert_eq!(
        reloaded.last_open_directory,
        Some(PathBuf::from("/tmp/roms"))
    );
}
