use super::*;

#[test]
fn saves_commands_export_and_import_external_sav_files() {
    let temp_dir = unique_temp_dir("saves-convert");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path =
        temp_dir.join("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb");
    let save_root = temp_dir.join("saves");
    let external_path = temp_dir.join("exports/battery.sav");
    let rom = build_battery_backed_serial_and_ram_rom(b'S', 0x12);
    fs::write(&rom_path, &rom).expect("test ROM should be writable");

    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect("test ROM should load for save seeding");
    let key = derive_save_key(&rom_path).expect("save key should derive");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    let mut seeded_ram = vec![0; 8 * 1024];
    seeded_ram[0] = 0x5A;
    seeded_ram[1] = 0xC3;
    backend
        .save(
            &key,
            report.cartridge().persistence_metadata(),
            &PersistentCartState::NoMbcRam { ram: seeded_ram },
        )
        .expect("seed save should persist");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "saves",
            "export",
            rom_path.to_str().expect("path should be valid UTF-8"),
            external_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("save export should succeed");
    assert_eq!(
        &fs::read(&external_path).expect("external save should exist")[..2],
        &[0x5A, 0xC3]
    );
    let output = String::from_utf8(stdout).expect("stdout should be UTF-8");
    assert!(
        output.contains("save_key=Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)"),
        "{output}"
    );
    assert!(
        output.contains("source_save=")
            && output
                .contains("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gbsav")
    );
    assert!(output.contains("external_bytes=8192"));
    let _ = String::from_utf8(stderr).expect("stderr should be UTF-8");

    let mut imported = fs::read(&external_path).expect("external save should be readable");
    imported[0] = 0xA5;
    imported[1] = 0x3C;
    fs::write(&external_path, imported).expect("external save should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "saves",
            "import",
            rom_path.to_str().expect("path should be valid UTF-8"),
            external_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("save import should succeed");

    assert_eq!(
        &fs::read(save_root.join(format!("{}.sav", key.as_str())))
            .expect("imported external-primary save should exist")[..2],
        &[0xA5, 0x3C]
    );
    let output = String::from_utf8(stdout).expect("stdout should be UTF-8");
    assert!(output.contains("target_save="));
    let _ = String::from_utf8(stderr).expect("stderr should be UTF-8");

    let reexport_path = temp_dir.join("exports/reexported.sav");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "saves",
            "export",
            rom_path.to_str().expect("path should be valid UTF-8"),
            reexport_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("save export should prefer the external-primary runtime save");
    assert_eq!(
        &fs::read(&reexport_path).expect("re-exported external save should exist")[..2],
        &[0xA5, 0x3C]
    );
    let output = String::from_utf8(stdout).expect("stdout should be UTF-8");
    assert!(
        output.contains("source_save=") && output.contains(&format!("{}.sav", key.as_str())),
        "{output}"
    );
    let _ = String::from_utf8(stderr).expect("stderr should be UTF-8");

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn saves_commands_cover_conversion_error_paths_and_exact_session_loads() {
    let temp_dir = unique_temp_dir("saves-convert-errors");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let plain_rom_path = temp_dir.join("plain.gb");
    fs::write(&plain_rom_path, build_single_byte_serial_rom(b'N'))
        .expect("plain ROM should be writable");
    let save_root = temp_dir.join("saves");
    let external_path = temp_dir.join("exports/plain.sav");
    let plain_options = SavesOptions {
        direction: SavesDirection::Export,
        rom_path: plain_rom_path.clone(),
        external_save_path: external_path.clone(),
        save_dir: save_root.clone(),
        save_key: None,
    };
    let export_error =
        saves_export_command(plain_options.clone(), &mut Vec::new(), &mut Vec::new())
            .expect_err("plain ROMs should not export saves");
    assert!(export_error.contains("does not expose battery-backed cartridge persistence"));
    let import_error = saves_import_command(
        SavesOptions {
            direction: SavesDirection::Import,
            ..plain_options
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("plain ROMs should not import saves");
    assert!(import_error.contains("does not expose battery-backed cartridge persistence"));

    let battery_rom_path =
        temp_dir.join("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb");
    let battery_rom = build_battery_backed_serial_and_ram_rom(b'B', 0x7E);
    fs::write(&battery_rom_path, &battery_rom).expect("battery ROM should be writable");
    let battery_options = SavesOptions {
        direction: SavesDirection::Export,
        rom_path: battery_rom_path.clone(),
        external_save_path: temp_dir.join("exports/battery.sav"),
        save_dir: save_root.clone(),
        save_key: None,
    };
    let no_save_error =
        saves_export_command(battery_options.clone(), &mut Vec::new(), &mut Vec::new())
            .expect_err("missing internal saves should fail export");
    assert!(no_save_error.contains("no gb-cycle save found"));

    let report = CartridgeSlot::load(battery_rom.clone(), &CompatibilityPolicy::strict())
        .expect("battery ROM should load");
    let exact_key = derive_save_key(&battery_rom_path).expect("exact key should derive");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    backend
        .save(
            &exact_key,
            report.cartridge().persistence_metadata(),
            &PersistentCartState::Mbc2Ram {
                ram_nibbles: [0; 512],
            },
        )
        .expect("mismatched save should still encode for compatibility checks");
    let mismatch_error =
        saves_export_command(battery_options.clone(), &mut Vec::new(), &mut Vec::new())
            .expect_err("mismatched internal saves should fail restore");
    assert!(mismatch_error.contains("is not compatible with ROM"));

    backend
        .delete(&exact_key)
        .expect("mismatched exact save should be removable");

    let old_sanitized_key =
        CartridgeSaveKey::new("Legend_of_Zelda_The_-_Link_s_Awakening_USA_Europe_Rev_2")
            .expect("old sanitized key should be valid");
    let old_sanitized_path = backend.path_for_key(&old_sanitized_key);
    fs::create_dir_all(
        old_sanitized_path
            .parent()
            .expect("old sanitized parent should exist"),
    )
    .expect("old sanitized parent should be creatable");
    fs::write(&old_sanitized_path, b"not-a-valid-save")
        .expect("broken old sanitized save should be writable");
    let ignored_old_sanitized_error =
        saves_export_command(battery_options.clone(), &mut Vec::new(), &mut Vec::new())
            .expect_err("old sanitized saves should not be loaded");
    assert!(ignored_old_sanitized_error.contains("no gb-cycle save found"));
    assert!(!ignored_old_sanitized_error.contains("failed to load save"));
    fs::remove_file(&old_sanitized_path).expect("broken old sanitized save should be removable");

    let broken_exact_path = backend.path_for_key(&exact_key);
    fs::write(&broken_exact_path, b"not-a-valid-save")
        .expect("broken exact save should be writable");
    let exact_load_error =
        load_save_envelope(&backend, &exact_key).expect_err("broken exact saves should fail");
    assert!(exact_load_error.contains("failed to load save"));
    fs::remove_file(&broken_exact_path).expect("broken exact save should be removable");

    let mut exact_ram = vec![0; 8 * 1024];
    exact_ram[0] = 0x44;
    let mut store = FilesystemCartridgeSaveStore::new(&save_root);
    store
        .save(
            &exact_key,
            report.cartridge().persistence_metadata(),
            &PersistentCartState::NoMbcRam { ram: exact_ram },
        )
        .expect("exact save should persist");
    let mut machine = build_loaded_machine(battery_rom, false);
    let session = open_save_session(
        Some(&save_root),
        &RunOptions::default_with_rom(battery_rom_path.clone()),
        &battery_rom_path,
        &mut machine,
        &mut Vec::new(),
        true,
    )
    .expect("exact save session should open")
    .expect("battery-backed ROMs should create a save session");
    assert!(session.loaded_existing_save);
    assert_eq!(session.key, exact_key);

    let missing_external_error = saves_import_command(
        SavesOptions {
            direction: SavesDirection::Import,
            external_save_path: temp_dir.join("missing.sav"),
            ..battery_options.clone()
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("missing external saves should fail import");
    assert!(missing_external_error.contains("failed to read external .sav save"));

    let invalid_external_path = temp_dir.join("imports/invalid.sav");
    fs::create_dir_all(
        invalid_external_path
            .parent()
            .expect("import parent should exist"),
    )
    .expect("import parent should be creatable");
    fs::write(&invalid_external_path, [0xAA]).expect("invalid external save should be writable");
    let invalid_external_error = saves_import_command(
        SavesOptions {
            direction: SavesDirection::Import,
            external_save_path: invalid_external_path,
            save_key: Some("explicit-slot".to_string()),
            ..battery_options
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("invalid external save lengths should fail import");
    assert!(invalid_external_error.contains("failed to convert external .sav save"));
    assert!(resolve_saves_key(Some("manual-slot"), &battery_rom_path).is_ok());

    let valid_external_path = temp_dir.join("imports/valid.sav");
    fs::write(&valid_external_path, vec![0x55; 8 * 1024])
        .expect("valid external save should be writable");
    let blocked_save_root = temp_dir.join("blocked-import-save");
    let blocked_store = FilesystemCartridgeSaveStore::new(&blocked_save_root);
    let blocked_target_path = blocked_store.external_path_for_key(&exact_key);
    let mut blocked_temp_path = blocked_target_path.as_os_str().to_os_string();
    blocked_temp_path.push(".tmp");
    fs::create_dir_all(PathBuf::from(blocked_temp_path))
        .expect("blocked temporary save path should be creatable");
    let save_error = saves_import_command(
        SavesOptions {
            direction: SavesDirection::Import,
            rom_path: battery_rom_path.clone(),
            external_save_path: valid_external_path,
            save_dir: blocked_save_root,
            save_key: None,
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("import save backend failures should surface");
    assert!(save_error.contains("failed to save cartridge persistence (saves-import)"));

    let blocking_parent = temp_dir.join("blocking-parent");
    fs::write(&blocking_parent, b"file").expect("blocking parent file should be writable");
    let write_error = write_bytes_with_parent(&blocking_parent.join("child.bin"), b"bytes")
        .expect_err("file parents should block directory creation");
    assert!(write_error.contains("failed to create directory"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn saves_commands_surface_output_writer_failures() {
    let temp_dir = unique_temp_dir("saves-writer-failures");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("battery.gb");
    let rom = build_battery_backed_serial_and_ram_rom(b'S', 0x22);
    fs::write(&rom_path, &rom).expect("battery ROM should be writable");
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("battery ROM should load");
    let save_root = temp_dir.join("saves");
    let save_key = derive_save_key(&rom_path).expect("save key should derive");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    backend
        .save(
            &save_key,
            report.cartridge().persistence_metadata(),
            &PersistentCartState::NoMbcRam {
                ram: vec![0x66; 8 * 1024],
            },
        )
        .expect("internal save should persist");

    for fail_on_write in [5, 7] {
        let options = SavesOptions {
            direction: SavesDirection::Export,
            rom_path: rom_path.clone(),
            external_save_path: temp_dir.join(format!("exports/export-{fail_on_write}.sav")),
            save_dir: save_root.clone(),
            save_key: None,
        };
        let mut output = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = saves_export_command(options, &mut output, &mut Vec::new())
            .expect_err("export output write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    let import_path = temp_dir.join("imports/import.sav");
    fs::create_dir_all(import_path.parent().expect("import parent should exist"))
        .expect("import parent should be creatable");
    fs::write(&import_path, vec![0x77; 8 * 1024]).expect("external save should be writable");
    for fail_on_write in [5, 7, 9] {
        let options = SavesOptions {
            direction: SavesDirection::Import,
            rom_path: rom_path.clone(),
            external_save_path: import_path.clone(),
            save_dir: temp_dir.join(format!("import-saves-{fail_on_write}")),
            save_key: None,
        };
        let mut output = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = saves_import_command(options, &mut output, &mut Vec::new())
            .expect_err("import output write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    let wrapper_options = SavesOptions {
        direction: SavesDirection::Export,
        rom_path,
        external_save_path: temp_dir.join("exports/wrapper.sav"),
        save_dir: save_root,
        save_key: None,
    };
    saves_command(wrapper_options, &mut Vec::new(), &mut Vec::new())
        .expect("saves command wrapper should dispatch export");

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
