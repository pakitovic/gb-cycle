use super::*;

#[test]
fn build_banked_mbc2_rom_maps_supported_size_codes_to_expected_lengths() {
    let cases = [
        (0x00, 32 * 1024usize),
        (0x01, 64 * 1024usize),
        (0x02, 128 * 1024usize),
        (0x03, 256 * 1024usize),
        (0x04, 512 * 1024usize),
    ];
    for (rom_size_code, expected_len) in cases {
        let rom = build_banked_mbc2_rom(0x06, rom_size_code, 0x00);
        assert_eq!(rom.len(), expected_len);
    }
}

#[test]
#[should_panic(expected = "unsupported MBC2 ROM size code for test")]
fn build_banked_mbc2_rom_rejects_unsupported_size_codes() {
    let _ = build_banked_mbc2_rom(0x06, 0x05, 0x00);
}

#[test]
fn open_returns_none_without_a_root_key_or_battery_backed_cartridge() {
    let root = temp_save_root();
    let mut battery_machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    assert!(
        DesktopSaveSession::open(
            None,
            DesktopSaveFlushPolicy::Manual,
            Some(CartridgeSaveKey::new("unused").expect("key should be valid")),
            &mut battery_machine,
        )
        .expect("omitting the save root should not fail")
        .is_none()
    );
    assert!(
        DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::Manual,
            None,
            &mut battery_machine,
        )
        .expect("omitting the save key should not fail")
        .is_none()
    );

    let mut no_battery_machine = load_machine(build_test_rom(32 * 1024, 0x00, 0x00, 0x00));
    assert!(
        DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::Manual,
            Some(CartridgeSaveKey::new("nobattery").expect("key should be valid")),
            &mut no_battery_machine,
        )
        .expect("non-battery cartridges should not error")
        .is_none()
    );

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
