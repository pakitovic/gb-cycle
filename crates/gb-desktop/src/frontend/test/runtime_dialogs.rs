use super::*;

#[test]
fn frontend_harness_processes_dialog_results_and_recent_rom_paths() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("dialogs", false, false, false);
    let relative_rom_name = "picked.gb";
    let relative_rom_path = harness.root.join(relative_rom_name);
    fs::write(
        &relative_rom_path,
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    )
    .expect("dialog test ROM should be writable");
    let boot_dir = harness.root.join("boot-assets");
    let save_dir = harness.root.join("save-root");
    fs::create_dir_all(&boot_dir).expect("boot directory should be creatable");
    fs::create_dir_all(&save_dir).expect("save directory should be creatable");

    assert!(!harness.session.has_loaded_rom());
    harness
        .runtime
        .menu_state
        .open(super::super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        ));
    assert!(harness.runtime.menu_state.is_open());

    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(relative_rom_name)))
        .expect("open ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("selected ROM should load");
    assert_eq!(
        harness.session.rom_path(),
        Some(relative_rom_path.as_path())
    );
    assert_eq!(
        harness.session.last_open_directory.as_deref(),
        Some(harness.root.as_path())
    );
    assert!(!harness.runtime.paused);
    assert!(!harness.runtime.menu_state.is_open());
    assert_eq!(
        harness.session.recent_roms().first(),
        Some(&relative_rom_path)
    );

    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Canceled)
        .expect("open ROM cancel should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("canceled ROM dialog should be ignored");
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Failed("open failed".to_string()))
        .expect("open ROM failure should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("failed ROM dialog should be reported");

    harness
        .runtime
        .boot_rom_directory_dialog
        .sender
        .send(PathDialogResult::Selected(boot_dir.clone()))
        .expect("boot ROM directory selection should send");
    harness
        .process_pending_boot_rom_directory_dialog()
        .expect("selected boot ROM directory should update the config");
    assert_eq!(
        harness.session.config.boot_rom.search_path.as_deref(),
        Some(boot_dir.as_path())
    );

    harness
        .runtime
        .boot_rom_directory_dialog
        .sender
        .send(PathDialogResult::Failed("boot dir failed".to_string()))
        .expect("boot ROM directory failure should send");
    harness
        .process_pending_boot_rom_directory_dialog()
        .expect("failed boot ROM directory dialog should be reported");
    harness
        .runtime
        .boot_rom_directory_dialog
        .sender
        .send(PathDialogResult::Canceled)
        .expect("boot ROM directory cancel should send");
    harness
        .process_pending_boot_rom_directory_dialog()
        .expect("canceled boot ROM directory dialog should be ignored");

    harness
        .runtime
        .save_directory_dialog
        .sender
        .send(PathDialogResult::Selected(save_dir.clone()))
        .expect("save directory selection should send");
    harness
        .process_pending_save_directory_dialog()
        .expect("selected save directory should update the config");
    assert_eq!(
        harness.session.config.saves.directory_policy,
        gb_desktop::SaveDirectoryPolicy::Custom(save_dir.clone())
    );

    harness
        .runtime
        .save_directory_dialog
        .sender
        .send(PathDialogResult::Failed("save dir failed".to_string()))
        .expect("save directory failure should send");
    harness
        .process_pending_save_directory_dialog()
        .expect("failed save directory dialog should be reported");
    harness
        .runtime
        .save_directory_dialog
        .sender
        .send(PathDialogResult::Canceled)
        .expect("save directory cancel should send");
    harness
        .process_pending_save_directory_dialog()
        .expect("canceled save directory dialog should be ignored");

    let missing_recent = harness.root.join("missing.gb");
    harness.session.recent_roms = vec![missing_recent.clone()];
    assert!(
        harness
            .execute_action(super::super::MenuAction::OpenRecentRom(0))
            .expect("missing recent ROM should be handled")
            .is_none()
    );
    assert!(!harness.session.recent_roms().contains(&missing_recent));

    let persisted =
        fs::read_to_string(&harness.settings_path).expect("dialog actions should persist settings");
    assert!(persisted.contains(&boot_dir.display().to_string()));
    assert!(persisted.contains(&save_dir.display().to_string()));
}

#[test]
fn external_save_dialogs_export_current_state_and_import_runtime_save() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("external-save-dialogs", false, false, false);
    let rom_name = "battery.gb";
    let rom_path = harness.root.join(rom_name);
    fs::write(&rom_path, build_test_rom(32 * 1024, 0x09, 0x00, 0x02))
        .expect("battery-backed ROM should be writable");

    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(rom_name)))
        .expect("battery ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("battery ROM should load");
    assert_eq!(harness.session.rom_path(), Some(rom_path.as_path()));

    let presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(presentation.external_save_available);
    assert!(presentation.external_save_import_available);

    harness
        .machine
        .primary_machine_mut()
        .restore_cartridge_persistent_state(&PersistentCartState::NoMbcRam {
            ram: vec![0x12; 8 * 1024],
        })
        .expect("NoMBC RAM state should restore");

    let export_without_extension = harness.root.join("exports/current");
    harness
        .runtime
        .external_save_export_dialog
        .sender
        .send(PathDialogResult::Selected(export_without_extension.clone()))
        .expect("external save export selection should send");
    harness
        .process_pending_external_save_export_dialog()
        .expect("external save export dialog should complete");
    let exported_path = export_without_extension.with_extension("sav");
    let exported = fs::read(&exported_path).expect("exported external save should exist");
    assert_eq!(exported, vec![0x12; 8 * 1024]);

    let import_path = harness.root.join("imports/current");
    fs::create_dir_all(
        import_path
            .parent()
            .expect("import path should have a parent"),
    )
    .expect("import directory should be creatable");
    fs::write(&import_path, vec![0x34; 8 * 1024])
        .expect("external save import file should be writable");
    harness
        .runtime
        .external_save_import_dialog
        .sender
        .send(PathDialogResult::Selected(import_path.clone()))
        .expect("external save import selection should send");
    harness
        .process_pending_external_save_import_dialog()
        .expect("external save import dialog should complete");
    assert!(
        harness.runtime.save_sessions[super::super::PlayerSlot::P1.index()].is_none(),
        "import keeps the live primary save session disabled until reload"
    );

    let save_root = harness.root.join("saves");
    let save_key = harness
        .session
        .config
        .saves
        .resolve_key(&rom_path)
        .expect("save key should resolve")
        .expect("save key should be enabled");
    let store = FilesystemCartridgeSaveStore::new(save_root);
    let imported_save = fs::read(store.external_path_for_key(&save_key))
        .expect("imported external-primary save should exist");
    assert_eq!(imported_save, vec![0x34; 8 * 1024]);

    harness
        .runtime
        .external_save_export_dialog
        .sender
        .send(PathDialogResult::Failed("export failed".to_string()))
        .expect("export failure should send");
    harness
        .process_pending_external_save_export_dialog()
        .expect("failed external save export dialog should be reported");
    harness
        .runtime
        .external_save_import_dialog
        .sender
        .send(PathDialogResult::Canceled)
        .expect("import cancel should send");
    harness
        .process_pending_external_save_import_dialog()
        .expect("canceled external save import dialog should be ignored");
}
