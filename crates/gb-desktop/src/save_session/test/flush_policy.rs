use super::*;

#[test]
fn debounced_policy_waits_for_the_configured_interval_before_flushing() {
    let root = temp_save_root();
    let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let mut session = DesktopSaveSession::open(
        Some(&root),
        DesktopSaveFlushPolicy::Debounced,
        Some(CartridgeSaveKey::new("debounced").expect("key should be valid")),
        &mut machine,
    )
    .expect("debounced save session should open")
    .expect("battery-backed cartridge should create a session");
    mutate_mbc2_persistent_state(&mut machine, 0x07);

    let start = Instant::now();
    assert!(
        !session
            .maybe_flush_at_frame_boundary(&machine, start)
            .expect("first frame-boundary check should succeed")
    );
    assert!(!session.save_path().exists());

    let before_deadline = (start + DEFAULT_SAVE_FLUSH_DEBOUNCE)
        .checked_sub(Duration::from_millis(1))
        .expect("deadline should exceed the pre-flush probe");
    assert!(
        !session
            .maybe_flush_at_frame_boundary(&machine, before_deadline)
            .expect("pre-deadline debounce probe should succeed")
    );
    assert!(!session.save_path().exists());

    assert!(
        session
            .maybe_flush_at_frame_boundary(&machine, start + DEFAULT_SAVE_FLUSH_DEBOUNCE)
            .expect("deadline debounce probe should succeed")
    );
    assert!(session.save_path().is_file());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn debounced_policy_still_flushes_on_close_before_the_interval_elapses() {
    let root = temp_save_root();
    let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let mut session = DesktopSaveSession::open(
        Some(&root),
        DesktopSaveFlushPolicy::Debounced,
        Some(CartridgeSaveKey::new("debounced-close").expect("key should be valid")),
        &mut machine,
    )
    .expect("debounced save session should open")
    .expect("battery-backed cartridge should create a session");
    mutate_mbc2_persistent_state(&mut machine, 0x03);

    assert!(
        !session
            .maybe_flush_at_frame_boundary(&machine, Instant::now())
            .expect("initial debounce probe should succeed")
    );
    assert!(!session.save_path().exists());

    session
        .close(&machine)
        .expect("close should flush even when debounce is still pending");
    assert!(session.save_path().is_file());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn on_write_policy_flushes_at_the_next_frame_boundary_without_waiting() {
    let root = temp_save_root();
    let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let mut session = DesktopSaveSession::open(
        Some(&root),
        DesktopSaveFlushPolicy::OnWrite,
        Some(CartridgeSaveKey::new("on-write").expect("key should be valid")),
        &mut machine,
    )
    .expect("on-write save session should open")
    .expect("battery-backed cartridge should create a session");
    mutate_mbc2_persistent_state(&mut machine, 0x0E);

    assert!(
        session
            .maybe_flush_at_frame_boundary(&machine, Instant::now())
            .expect("on-write frame-boundary check should succeed")
    );
    assert!(session.save_path().is_file());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn open_restores_existing_battery_backed_save_from_disk() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("restore".to_string()).expect("key should be valid");
    let mut saved_machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    mutate_mbc2_persistent_state(&mut saved_machine, 0x0A);
    let expected_state = saved_machine.cartridge().persistent_state();

    let mut store = FilesystemCartridgeSaveStore::new(&root);
    store
        .save(
            &key,
            saved_machine.cartridge().persistence_metadata(),
            &expected_state,
        )
        .expect("pre-existing save should write");

    let mut restored_machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let session = DesktopSaveSession::open(
        Some(&root),
        DesktopSaveFlushPolicy::Manual,
        Some(key.clone()),
        &mut restored_machine,
    )
    .expect("save session should load an existing save")
    .expect("battery-backed cartridge should create a session");

    assert_eq!(
        session.save_path(),
        root.join(format!("{}.sav", key.as_str()))
    );
    assert_eq!(
        restored_machine.cartridge().persistent_state(),
        expected_state
    );

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
