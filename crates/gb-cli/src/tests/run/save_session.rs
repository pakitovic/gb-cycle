use super::super::*;

#[test]
fn save_session_helpers_cover_skip_restore_and_noop_flush_paths() {
    let temp_dir = unique_temp_dir("save-session");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let mut summary_machine = build_loaded_machine(build_single_byte_serial_rom(b'N'), false);
    let mut stderr = Vec::new();
    let no_battery = open_save_session(
        Some(&temp_dir),
        &RunOptions::default_with_rom(PathBuf::from("nobattery.gb")),
        Path::new("nobattery.gb"),
        &mut summary_machine,
        &mut stderr,
        true,
    )
    .expect("non-battery cartridges should skip save sessions");
    assert!(no_battery.is_none());
    assert!(
        String::from_utf8(stderr)
            .expect("stderr should be UTF-8")
            .contains("save=skipped not_battery_backed=true")
    );

    let save_root = temp_dir.join("battery");
    fs::create_dir_all(&save_root).expect("save root should be creatable");
    let mut battery_machine =
        build_loaded_machine(build_battery_backed_serial_and_ram_rom(b'R', 0), false);
    let save_key = derive_save_key(Path::new("battery.gb")).expect("save key should derive");
    let seeded_state = PersistentCartState::NoMbcRam {
        ram: vec![0x33; 8 * 1024],
    };
    let mut store = FilesystemCartridgeSaveStore::new(&save_root);
    store
        .save(
            &save_key,
            battery_machine.cartridge().persistence_metadata(),
            &seeded_state,
        )
        .expect("seed save should persist");

    let mut stderr = Vec::new();
    let mut session = open_save_session(
        Some(&save_root),
        &RunOptions::default_with_rom(PathBuf::from("battery.gb")),
        Path::new("battery.gb"),
        &mut battery_machine,
        &mut stderr,
        true,
    )
    .expect("save session should open")
    .expect("battery-backed cartridges should open a save session");
    assert!(session.loaded_existing_save);
    assert_eq!(session.last_saved_state, seeded_state);
    assert_eq!(battery_machine.cartridge().persistent_state(), seeded_state);
    assert!(
        !flush_save_if_changed(&mut session, &battery_machine, "no-change")
            .expect("unchanged state should not be re-saved")
    );
    assert!(
        String::from_utf8(stderr)
            .expect("stderr should be UTF-8")
            .contains("save_loaded path=")
    );

    let mut failing_stderr = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    let mut failing_machine =
        build_loaded_machine(build_battery_backed_serial_and_ram_rom(b'R', 0), false);
    let save_loaded_error = open_save_session(
        Some(&save_root),
        &RunOptions::default_with_rom(PathBuf::from("battery.gb")),
        Path::new("battery.gb"),
        &mut failing_machine,
        &mut failing_stderr,
        true,
    )
    .expect_err("save-loaded status write failures should surface");
    assert!(save_loaded_error.contains("failed to write output"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn save_session_and_flush_error_paths_surface_backend_failures() {
    let mut options = None;
    let rom_path = Some(PathBuf::from("demo.gb"));
    ensure_run_options_initialized(&mut options, &rom_path)
        .expect("existing ROM paths should initialize default options");
    assert_eq!(
        options,
        Some(RunOptions::default_with_rom(PathBuf::from("demo.gb")))
    );

    let temp_dir = unique_temp_dir("save-errors");
    let save_root = temp_dir.join("saves");
    fs::create_dir_all(&save_root).expect("save root should be creatable");

    let mut battery_machine =
        build_loaded_machine(build_battery_backed_serial_and_ram_rom(b'R', 0), false);
    let mut options = RunOptions::default_with_rom(PathBuf::from("battery.gb"));
    options.save_key = Some("battery_manual".to_string());
    let key = CartridgeSaveKey::new("battery_manual").expect("save key should be valid");
    let store = FilesystemCartridgeSaveStore::new(&save_root);
    fs::write(store.external_path_for_key(&key), b"not-a-valid-save")
        .expect("broken save bytes should be writable");
    let load_error = open_save_session(
        Some(&save_root),
        &options,
        Path::new("battery.gb"),
        &mut battery_machine,
        &mut Vec::new(),
        true,
    )
    .expect_err("broken save files should surface backend load errors");
    assert!(load_error.contains("failed to load save"));

    let blocking_root = temp_dir.join("blocking-root");
    fs::write(&blocking_root, b"file").expect("blocking file should be writable");
    let mut failing_session = SaveSession {
        backend: FilesystemCartridgeSaveStore::new(&blocking_root),
        key: CartridgeSaveKey::new("battery").expect("save key should be valid"),
        save_path: blocking_root.join("battery.sav"),
        last_saved_state: PersistentCartState::None,
        loaded_existing_save: false,
        save_writes: 0,
    };
    let save_error = flush_save_if_changed(&mut failing_session, &battery_machine, "forced-save")
        .expect_err("broken save roots should surface backend save errors");
    assert!(save_error.contains("failed to save cartridge persistence (forced-save)"));

    let mut state_machine =
        build_loaded_machine(build_battery_backed_serial_and_ram_rom(b'T', 0), false);
    let missing_state_error =
        restore_machine_save_state_from_path(&mut state_machine, &temp_dir.join("missing.gbstate"))
            .expect_err("missing state files should surface read errors");
    assert!(missing_state_error.contains("failed to read .gbstate state"));

    let blocking_state_parent = temp_dir.join("blocking-state-parent");
    fs::write(&blocking_state_parent, b"file").expect("blocking state parent should be writable");
    let state_write_error =
        write_machine_save_state_to_path(&state_machine, &blocking_state_parent.join("state.bin"))
            .expect_err("non-directory state parents should block writes");
    assert!(state_write_error.contains("failed to write state"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
