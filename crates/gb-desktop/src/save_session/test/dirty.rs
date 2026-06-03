use super::*;

#[test]
fn debounced_policy_clears_pending_deadline_when_state_returns_to_saved() {
    let root = temp_save_root();
    let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let mut session = DesktopSaveSession::open(
        Some(&root),
        DesktopSaveFlushPolicy::Debounced,
        Some(CartridgeSaveKey::new("debounce-reset".to_string()).expect("key should be valid")),
        &mut machine,
    )
    .expect("debounced save session should open")
    .expect("battery-backed cartridge should create a session");
    let original_state = machine.cartridge().persistent_state();
    mutate_mbc2_persistent_state(&mut machine, 0x09);

    let start = Instant::now();
    assert!(
        !session
            .maybe_flush_at_frame_boundary(&machine, start)
            .expect("initial debounce probe should succeed")
    );
    assert!(session.pending_debounced_flush_deadline.is_some());

    machine
        .restore_cartridge_persistent_state(&original_state)
        .expect("restoring the saved state should succeed");
    assert!(
        !session
            .maybe_flush_at_frame_boundary(&machine, start + Duration::from_millis(1))
            .expect("unchanged debounce probe should succeed")
    );
    assert!(session.pending_debounced_flush_deadline.is_none());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn flush_if_changed_surfaces_backend_save_errors() {
    let root = temp_save_root();
    let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let mut session = DesktopSaveSession::open(
        Some(&root),
        DesktopSaveFlushPolicy::OnWrite,
        Some(CartridgeSaveKey::new("save-error".to_string()).expect("key should be valid")),
        &mut machine,
    )
    .expect("on-write save session should open")
    .expect("battery-backed cartridge should create a session");
    let blocking_root = root.join("not-a-directory");
    fs::write(&blocking_root, b"occupied").expect("blocking file should exist");
    session.backend = FilesystemCartridgeSaveStore::new(&blocking_root);
    mutate_mbc2_persistent_state(&mut machine, 0x0B);

    let error = session
        .flush_if_changed(&machine, "test-save")
        .expect_err("save failures should surface through the desktop session");
    assert!(error.contains("failed to save cartridge persistence (test-save)"));
    assert!(error.contains(".sav"));

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn temp_save_root_reuses_stale_directory_ids_cleanly() {
    let saved_counter = TEMP_DIR_COUNTER.load(Ordering::Relaxed);
    TEMP_DIR_COUNTER.store(42, Ordering::Relaxed);
    let root = temp_save_root();
    fs::write(root.join("stale.bin"), b"stale").expect("stale marker should write");

    TEMP_DIR_COUNTER.store(42, Ordering::Relaxed);
    let reused_root = temp_save_root();
    assert_eq!(reused_root, root);
    assert!(!reused_root.join("stale.bin").exists());

    TEMP_DIR_COUNTER.store(saved_counter, Ordering::Relaxed);
    fs::remove_dir_all(reused_root).expect("temp save root should be removable");
}
