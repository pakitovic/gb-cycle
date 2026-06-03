use super::*;

#[test]
fn open_uses_configured_player_slot_file_extension_without_legacy_fallback() {
    let root = temp_save_root();
    let key =
        CartridgeSaveKey::new("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)")
            .expect("ROM stem key should be valid");
    let old_sanitized_key =
        CartridgeSaveKey::new("Legend_of_Zelda_The_-_Link_s_Awakening_USA_Europe_Rev_2")
            .expect("old sanitized key should be valid");

    let mut backend = FilesystemCartridgeSaveBackend::new(&root);
    backend
        .save(
            &old_sanitized_key,
            load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00))
                .cartridge()
                .persistence_metadata(),
            &PersistentCartState::Mbc2Ram {
                ram_nibbles: [0x0A; 512],
            },
        )
        .expect("old sanitized save should write");

    let mut restored_machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let mut session = DesktopSaveSession::open_with_file_extension(
        Some(&root),
        DesktopSaveFlushPolicy::OnClose,
        Some(key.clone()),
        CartridgeSaveFileExtension::P2,
        &mut restored_machine,
    )
    .expect("save session should open")
    .expect("battery-backed cartridge should create a session");

    assert_eq!(
        restored_machine.cartridge().persistent_state(),
        load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00))
            .cartridge()
            .persistent_state()
    );
    assert_eq!(
        session.save_path(),
        root.join(format!("{}.sa2", key.as_str()))
    );
    mutate_mbc2_persistent_state(&mut restored_machine, 0x0B);
    session
        .close(&restored_machine)
        .expect("closing should write through the configured slot extension");
    assert!(session.save_path().is_file());
    assert!(backend.path_for_key(&old_sanitized_key).is_file());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn open_surfaces_corrupt_existing_save_files() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("corrupt".to_string()).expect("key should be valid");
    let store = FilesystemCartridgeSaveStore::new(&root);
    fs::write(store.external_path_for_key(&key), b"not-a-valid-save")
        .expect("corrupt save payload should write");
    let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));

    let error = DesktopSaveSession::open(
        Some(&root),
        DesktopSaveFlushPolicy::Manual,
        Some(key),
        &mut machine,
    )
    .err()
    .expect("corrupt save payloads should surface as load errors");
    assert!(error.contains("failed to load save"));
    assert!(error.contains(".sav"));

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn on_close_policy_defers_frame_boundary_flushes_but_flushes_when_closed() {
    let root = temp_save_root();
    let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let mut session = DesktopSaveSession::open(
        Some(&root),
        DesktopSaveFlushPolicy::OnClose,
        Some(CartridgeSaveKey::new("on-close".to_string()).expect("key should be valid")),
        &mut machine,
    )
    .expect("on-close save session should open")
    .expect("battery-backed cartridge should create a session");
    mutate_mbc2_persistent_state(&mut machine, 0x05);

    assert!(
        !session
            .maybe_flush_at_frame_boundary(&machine, Instant::now())
            .expect("frame-boundary checks should be skipped for on-close sessions")
    );
    assert!(!session.save_path().exists());

    session
        .close(&machine)
        .expect("on-close sessions should flush when the session closes");
    assert!(session.save_path().is_file());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
