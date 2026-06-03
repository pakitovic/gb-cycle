use super::*;

#[test]
fn save_options_cover_disabled_custom_and_explicit_key_paths() {
    let rom_path = Path::new("/tmp/roms/Tetris.gb");
    let disabled = SaveOptions {
        enabled: false,
        ..SaveOptions::default()
    };
    assert_eq!(disabled.resolve_directory(rom_path), None);
    assert_eq!(
        disabled
            .resolve_key(rom_path)
            .expect("disabled saves should not error"),
        None
    );

    let explicit_key = CartridgeSaveKey::new("tetris".to_string())
        .expect("explicit save keys in tests should be valid");
    let custom = SaveOptions {
        directory_policy: SaveDirectoryPolicy::Custom(PathBuf::from("/tmp/custom-saves")),
        key_policy: SaveKeyPolicy::Explicit(explicit_key.clone()),
        ..SaveOptions::default()
    };
    assert_eq!(
        custom.resolve_directory(rom_path),
        Some(PathBuf::from("/tmp/custom-saves"))
    );
    assert_eq!(
        custom
            .resolve_key(rom_path)
            .expect("explicit save keys should resolve")
            .expect("saves are enabled"),
        explicit_key
    );
}

#[test]
fn input_helpers_cover_face_layout_preferred_gamepads_and_direction_sources() {
    let mut bindings = GamepadButtonBindings::default();
    bindings.apply_face_layout(GamepadFaceLayout::SouthAEastB);
    assert_eq!(bindings.a, GamepadButtonBinding::South);
    assert_eq!(bindings.b, GamepadButtonBinding::East);

    assert!(!PreferredGamepadIdentity::default().is_configured());
    assert!(
        PreferredGamepadIdentity {
            path: Some("/dev/input/js0".to_string()),
            name: None,
        }
        .is_configured()
    );

    assert!(GamepadDirectionalSource::DpadOnly.uses_dpad());
    assert!(!GamepadDirectionalSource::DpadOnly.uses_left_stick());
    assert!(!GamepadDirectionalSource::LeftStickOnly.uses_dpad());
    assert!(GamepadDirectionalSource::LeftStickOnly.uses_left_stick());
    assert!(GamepadDirectionalSource::DpadAndLeftStick.uses_dpad());
    assert!(GamepadDirectionalSource::DpadAndLeftStick.uses_left_stick());
    assert_eq!(GamepadGyroMode::default(), GamepadGyroMode::Off);
    assert_eq!(GamepadRumbleMode::default(), GamepadRumbleMode::Strong);
}

#[test]
fn gamepad_gyro_mode_serializes_as_stable_kebab_case_values() {
    #[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
    struct GyroModeWrapper {
        gyro_mode: GamepadGyroMode,
    }

    for (mode, serialized) in [
        (GamepadGyroMode::Off, "off"),
        (GamepadGyroMode::PadGyro, "pad-gyro"),
        (GamepadGyroMode::PadInput, "pad-input"),
    ] {
        let wrapper = GyroModeWrapper { gyro_mode: mode };
        let encoded = toml::to_string(&wrapper).expect("gyro mode should serialize");
        assert_eq!(encoded.trim(), format!("gyro_mode = \"{serialized}\""));
        let decoded: GyroModeWrapper =
            toml::from_str(&encoded).expect("gyro mode should deserialize");
        assert_eq!(decoded, wrapper);
    }
}
