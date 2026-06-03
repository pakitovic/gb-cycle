use super::*;

#[test]
fn default_save_directory_lives_under_a_saves_subdirectory_next_to_the_rom() {
    let rom_path = Path::new("/tmp/roms/Tetris.gb");
    let save_options = SaveOptions::default();

    assert_eq!(
        save_options.resolve_directory(rom_path),
        Some(PathBuf::from("/tmp/roms/saves"))
    );
}

#[test]
fn derived_save_key_preserves_the_rom_stem() {
    let rom_path =
        Path::new("/tmp/roms/Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb");
    let save_key = SaveOptions::default()
        .resolve_key(rom_path)
        .expect("save-key derivation should succeed")
        .expect("saves are enabled by default");

    assert_eq!(
        save_key.as_str(),
        "Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)"
    );
}

#[test]
fn keyboard_defaults_match_the_expected_handheld_layout() {
    let keyboard = KeyboardBindings::default();

    assert_eq!(keyboard.joypad.up, DesktopKey::ArrowUp);
    assert_eq!(keyboard.joypad.down, DesktopKey::ArrowDown);
    assert_eq!(keyboard.joypad.left, DesktopKey::ArrowLeft);
    assert_eq!(keyboard.joypad.right, DesktopKey::ArrowRight);
    assert_eq!(keyboard.joypad.a, DesktopKey::LeftGui);
    assert_eq!(keyboard.joypad.b, DesktopKey::LeftAlt);
    assert_eq!(keyboard.joypad.select, DesktopKey::Backspace);
    assert_eq!(keyboard.joypad.start, DesktopKey::Return);
    assert_eq!(keyboard.menu.up, DesktopKey::ArrowUp);
    assert_eq!(keyboard.menu.down, DesktopKey::ArrowDown);
    assert_eq!(keyboard.menu.confirm, DesktopKey::LeftGui);
    assert_eq!(keyboard.menu.cancel, DesktopKey::LeftAlt);
    assert_eq!(keyboard.hotkeys.pause, DesktopKey::Space);
    assert_eq!(keyboard.hotkeys.save_state, DesktopKey::F1);
    assert_eq!(keyboard.hotkeys.load_state, DesktopKey::F2);
    assert_eq!(keyboard.hotkeys.state_slot_1, DesktopKey::Digit1);
    assert_eq!(keyboard.hotkeys.state_slot_2, DesktopKey::Digit2);
    assert_eq!(keyboard.hotkeys.state_slot_3, DesktopKey::Digit3);
    assert_eq!(keyboard.hotkeys.state_slot_4, DesktopKey::Digit4);
    assert_eq!(keyboard.hotkeys.reset, DesktopKey::F12);
    assert_eq!(keyboard.hotkeys.rewind, DesktopKey::LeftShift);
    assert_eq!(keyboard.hotkeys.fast_forward, DesktopKey::RightShift);
    assert_eq!(keyboard.hotkeys.toggle_fullscreen, DesktopKey::F11);
    assert_eq!(keyboard.hotkeys.toggle_performance_hud, DesktopKey::F10);
    assert_eq!(keyboard.hotkeys.save_battery, DesktopKey::F9);
}
