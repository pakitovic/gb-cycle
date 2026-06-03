use super::*;

#[test]
fn filesystem_backend_round_trips_mbc2_nibbles_and_cleans_temp_artifacts() {
    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x0B);
    cartridge.write_ram(0xA200, 0x05);
    cartridge.write_rom(0x0000, 0x00);

    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc2_nibbles").expect("key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(7),
    );
    backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    let save_path = backend.path_for_key(&key);
    let temp_path = PathBuf::from(format!("{}.tmp", save_path.display()));
    let backup_path = PathBuf::from(format!("{}.bak", save_path.display()));
    assert!(save_path.is_file());
    assert!(!temp_path.exists());
    assert!(!backup_path.exists());
    assert_eq!(
        save_path.extension().and_then(|ext| ext.to_str()),
        Some(SAVE_FILE_EXTENSION)
    );

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    let mut restored = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    restored
        .restore_persistent_state(&loaded.persistent_state)
        .expect("restore should accept the persisted payload");
    restored.write_rom(0x0000, 0x0A);
    assert_eq!(restored.read_ram(0xA000) & 0x0F, 0x05);
    assert_eq!(restored.read_ram(0xA200) & 0x0F, 0x05);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn filesystem_backend_round_trips_mbc3_rtc_payload_and_exposes_saved_timestamp() {
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::GameBoy));
    machine
        .load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03))
        .expect("MBC3 cartridge should load");
    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x00);
    machine.write_bus(0xA000, 0x44);
    machine.advance_cartridge_rtc_seconds(93_784);
    machine.write_bus(0x0000, 0x00);

    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc3_rtc").expect("key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(1_800_000_000),
    );
    backend
        .save(
            &key,
            machine.cartridge().persistence_metadata(),
            &machine.cartridge().persistent_state(),
        )
        .expect("save should succeed");

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    assert_eq!(loaded.backend_metadata.saved_at_unix_seconds, 1_800_000_000);
    match &loaded.persistent_state {
        PersistentCartState::Mbc3RamRtc { rtc, .. } => {
            assert_eq!(
                *rtc,
                Mbc3RtcPersistentState {
                    seconds: 4,
                    minutes: 3,
                    hours: 2,
                    day_counter: 1,
                    halt: false,
                    carry: false,
                }
            );
        }
        other => panic!("expected MBC3 RAM+RTC payload, got {other:?}"),
    }

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn filesystem_backend_replaces_existing_save_without_leaving_temp_or_backup_files() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("replace_existing").expect("key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(600),
    );

    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x01);
    backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("first save should succeed");

    cartridge.write_ram(0xA000, 0x0E);
    backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("replacement save should succeed");

    let save_path = backend.path_for_key(&key);
    let temp_path = PathBuf::from(format!("{}.tmp", save_path.display()));
    let backup_path = PathBuf::from(format!("{}.bak", save_path.display()));
    assert!(save_path.is_file());
    assert!(!temp_path.exists());
    assert!(!backup_path.exists());

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    match loaded.persistent_state {
        PersistentCartState::Mbc2Ram { ram_nibbles } => {
            assert_eq!(ram_nibbles[0], 0x0E);
        }
        other => panic!("expected MBC2 save after replacement, got {other:?}"),
    }

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn filesystem_backend_delete_is_idempotent_for_missing_and_existing_saves() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("delete_me").expect("key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::new(&root);

    backend
        .delete(&key)
        .expect("delete should ignore missing files");

    let save_path = backend.path_for_key(&key);
    fs::write(&save_path, b"placeholder").expect("placeholder save should be creatable");
    backend
        .delete(&key)
        .expect("delete should remove the existing file");
    assert!(!Path::new(&save_path).exists());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn filesystem_backend_exposes_root_path_and_missing_load_cleanly() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("missing_filesystem_save").expect("key should be valid");
    let backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(654),
    );

    assert_eq!(backend.root(), Path::new(&root));
    assert_eq!(backend.current_unix_seconds(), 654);
    assert_eq!(
        backend.path_for_key(&key),
        root.join(format!("{key}.{}", SAVE_FILE_EXTENSION, key = key.as_str()))
    );
    assert_eq!(backend.load(&key).expect("load should succeed"), None);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn filesystem_backend_uses_the_configured_slot_file_extension() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("slot_extension").expect("key should be valid");
    let slot_extensions = [
        (CartridgeSaveFileExtension::P1, SAVE_FILE_EXTENSION),
        (CartridgeSaveFileExtension::P2, SAVE_FILE_EXTENSION_P2),
        (CartridgeSaveFileExtension::P3, SAVE_FILE_EXTENSION_P3),
        (CartridgeSaveFileExtension::P4, SAVE_FILE_EXTENSION_P4),
    ];

    for (file_extension, expected_suffix) in slot_extensions {
        let backend = FilesystemCartridgeSaveBackend::with_file_extension(&root, file_extension);
        assert_eq!(backend.file_extension(), file_extension);
        assert_eq!(
            backend.path_for_key(&key),
            root.join(format!("{}.{expected_suffix}", key.as_str()))
        );
    }

    let cartridge = load_cartridge(build_test_rom(32 * 1024, 0x09, 0x00, 0x02));
    let metadata = cartridge.persistence_metadata();
    let mut p1_backend =
        FilesystemCartridgeSaveBackend::with_file_extension(&root, CartridgeSaveFileExtension::P1);
    let mut p2_backend =
        FilesystemCartridgeSaveBackend::with_file_extension(&root, CartridgeSaveFileExtension::P2);
    p1_backend
        .save(
            &key,
            metadata,
            &PersistentCartState::NoMbcRam {
                ram: vec![0x11; 8 * 1024],
            },
        )
        .expect("P1 save should write");
    p2_backend
        .save(
            &key,
            metadata,
            &PersistentCartState::NoMbcRam {
                ram: vec![0x22; 8 * 1024],
            },
        )
        .expect("P2 save should write");
    let p1_state = p1_backend
        .load(&key)
        .expect("P1 load should succeed")
        .expect("P1 save should exist")
        .persistent_state;
    let p2_state = p2_backend
        .load(&key)
        .expect("P2 load should succeed")
        .expect("P2 save should exist")
        .persistent_state;
    assert_ne!(p1_state, p2_state);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn filesystem_backend_surfaces_targeted_io_failures() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("io_failures").expect("key should be valid");
    let occupied_root = root.join("occupied");
    fs::write(&occupied_root, b"not a directory").expect("occupied file should be creatable");

    let mut occupied_backend = FilesystemCartridgeSaveBackend::new(occupied_root.as_path());
    let _ = occupied_backend.current_unix_seconds();

    let load_error = occupied_backend
        .load(&key)
        .expect_err("load should surface path errors");
    assert!(matches!(
        load_error,
        CartridgeSaveBackendError::Io {
            operation: "read save file",
            ..
        }
    ));

    let delete_error = occupied_backend
        .delete(&key)
        .expect_err("delete should surface path errors");
    assert!(matches!(
        delete_error,
        CartridgeSaveBackendError::Io {
            operation: "delete save file",
            ..
        }
    ));

    let source = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let stale_key = CartridgeSaveKey::new("stale_backup_cleanup").expect("key should be valid");
    let mut stale_backend = FilesystemCartridgeSaveBackend::with_time_source(
        root.as_path(),
        FixedCartridgeSaveTimeSource::new(1_001),
    );
    let stale_save_path = stale_backend.path_for_key(&stale_key);
    let stale_backup_path = PathBuf::from(format!("{}.bak", stale_save_path.display()));
    fs::create_dir_all(&stale_backup_path).expect("backup directory should be creatable");

    let stale_backup_error = stale_backend
        .save(
            &stale_key,
            source.persistence_metadata(),
            &source.persistent_state(),
        )
        .expect_err("save should reject stale backup paths that are not files");
    assert!(matches!(
        stale_backup_error,
        CartridgeSaveBackendError::Io {
            operation: "remove stale backup save file",
            ..
        }
    ));
    fs::remove_dir_all(&stale_backup_path).expect("backup directory should be removable");

    let temp_key = CartridgeSaveKey::new("temp_create_failure").expect("key should be valid");
    let mut temp_backend = FilesystemCartridgeSaveBackend::with_time_source(
        root.as_path(),
        FixedCartridgeSaveTimeSource::new(1_002),
    );
    let temp_save_path = temp_backend.path_for_key(&temp_key);
    let temp_path = PathBuf::from(format!("{}.tmp", temp_save_path.display()));
    fs::create_dir_all(&temp_path).expect("temporary directory should be creatable");

    let create_temp_error = temp_backend
        .save(
            &temp_key,
            source.persistence_metadata(),
            &source.persistent_state(),
        )
        .expect_err("save should fail when the temporary path is already a directory");
    assert!(matches!(
        create_temp_error,
        CartridgeSaveBackendError::Io {
            operation: "create temporary save file",
            ..
        }
    ));

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
