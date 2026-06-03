use super::*;

#[test]
fn desktop_key_deserialization_accepts_platform_aliases() {
    #[derive(serde::Deserialize)]
    struct KeyWrapper {
        key: DesktopKey,
    }

    let cases = [
        ("1", DesktopKey::Digit1),
        ("digit-2", DesktopKey::Digit2),
        ("key-3", DesktopKey::Digit3),
        ("f1", DesktopKey::F1),
        ("f2", DesktopKey::F2),
        ("f3", DesktopKey::F3),
        ("f4", DesktopKey::F4),
        ("f5", DesktopKey::F5),
        ("f6", DesktopKey::F6),
        ("f7", DesktopKey::F7),
        ("f8", DesktopKey::F8),
        ("f9", DesktopKey::F9),
        ("f12", DesktopKey::F12),
        ("left-alt", DesktopKey::LeftAlt),
        ("left-option", DesktopKey::LeftAlt),
        ("right-option", DesktopKey::RightAlt),
        ("left-gui", DesktopKey::LeftGui),
        ("left-command", DesktopKey::LeftGui),
        ("right-command", DesktopKey::RightGui),
        ("left-super", DesktopKey::LeftGui),
        ("right-windows", DesktopKey::RightGui),
        ("left-shift", DesktopKey::LeftShift),
        ("right-control", DesktopKey::RightControl),
        ("tab", DesktopKey::Tab),
    ];

    for (serialized, expected) in cases {
        let decoded: KeyWrapper = toml::from_str(&format!("key = \"{serialized}\""))
            .expect("desktop key alias should deserialize");
        assert_eq!(decoded.key, expected);
    }
}

#[test]
fn console_model_helpers_cover_all_visible_product_models() {
    assert_eq!(
        DesktopConsoleModel::GameBoy.console_model(),
        ConsoleModel::GameBoy
    );
    assert_eq!(
        DesktopConsoleModel::GameBoyPocket.console_model(),
        ConsoleModel::GameBoyPocket
    );
    assert_eq!(
        DesktopConsoleModel::GameBoyLight.console_model(),
        ConsoleModel::GameBoyLight
    );
    assert_eq!(
        DesktopConsoleModel::GameBoyColor.console_model(),
        ConsoleModel::GameBoyColor
    );
    assert_eq!(
        DesktopConsoleModel::SuperGameBoy.console_model(),
        ConsoleModel::GameBoy
    );
    assert_eq!(
        DesktopConsoleModel::SuperGameBoy2.console_model(),
        ConsoleModel::GameBoy
    );
    assert_eq!(
        DesktopConsoleModel::SuperGameBoy.sgb_profile(),
        Some(SgbHostProfile::SgbNtsc)
    );
    assert_eq!(
        DesktopConsoleModel::SuperGameBoy.sgb_profile_for_standard(SgbVideoStandard::Pal),
        Some(SgbHostProfile::SgbPal)
    );
    assert_eq!(
        DesktopConsoleModel::SuperGameBoy2.sgb_profile(),
        Some(SgbHostProfile::Sgb2Ntsc)
    );
    assert_eq!(
        DesktopConsoleModel::SuperGameBoy2.sgb_profile_for_standard(SgbVideoStandard::Pal),
        Some(SgbHostProfile::Sgb2Ntsc)
    );
    assert!(DesktopConsoleModel::SuperGameBoy.allows_sgb_video_standard_selection());
    assert!(!DesktopConsoleModel::SuperGameBoy2.allows_sgb_video_standard_selection());
    assert!(DesktopConsoleModel::GameBoy.allows_display_palette());
    assert!(!DesktopConsoleModel::GameBoyColor.allows_display_palette());
    assert!(!DesktopConsoleModel::SuperGameBoy.allows_display_palette());
    assert!(DesktopConsoleModel::GameBoy.allows_ext_port_menu());
    assert!(DesktopConsoleModel::GameBoyPocket.allows_ext_port_menu());
    assert!(DesktopConsoleModel::GameBoyLight.allows_ext_port_menu());
    assert!(DesktopConsoleModel::GameBoyColor.allows_ext_port_menu());
    assert!(!DesktopConsoleModel::SuperGameBoy.allows_ext_port_menu());
    assert!(DesktopConsoleModel::SuperGameBoy2.allows_ext_port_menu());
    assert_eq!(DesktopConsoleModel::GameBoy.name(), "DMG");
    assert_eq!(DesktopConsoleModel::GameBoyPocket.name(), "MGB");
    assert_eq!(DesktopConsoleModel::GameBoyLight.name(), "LGB");
    assert_eq!(DesktopConsoleModel::GameBoyColor.name(), "CGB");
    assert_eq!(DesktopConsoleModel::SuperGameBoy.name(), "SGB");
    assert_eq!(DesktopConsoleModel::SuperGameBoy2.name(), "SGB2");
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::GameBoy),
        DesktopDisplayPalette::GameBoy
    );
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::GameBoyPocket),
        DesktopDisplayPalette::Pocket
    );
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::GameBoyLight),
        DesktopDisplayPalette::Light
    );
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::GameBoyColor),
        DesktopDisplayPalette::Grey
    );
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::SuperGameBoy),
        DesktopDisplayPalette::Grey
    );
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::SuperGameBoy2),
        DesktopDisplayPalette::Grey
    );
    assert_eq!(
        DesktopDisplayPalette::Grey.next(),
        DesktopDisplayPalette::GameBoy
    );
    assert_eq!(
        DesktopDisplayPalette::GameBoy.next(),
        DesktopDisplayPalette::Pocket
    );
    assert_eq!(
        DesktopDisplayPalette::Pocket.next(),
        DesktopDisplayPalette::Light
    );
    assert_eq!(
        DesktopDisplayPalette::Light.next(),
        DesktopDisplayPalette::Grey
    );
    assert_eq!(
        DesktopFrameBlendingMode::Off.next(),
        DesktopFrameBlendingMode::On
    );
    assert_eq!(
        DesktopFrameBlendingMode::On.next(),
        DesktopFrameBlendingMode::Off
    );
    assert_eq!(
        VideoOptions::default_for_console_model(DesktopConsoleModel::GameBoyColor).display_palette,
        DesktopDisplayPalette::Grey
    );
}
