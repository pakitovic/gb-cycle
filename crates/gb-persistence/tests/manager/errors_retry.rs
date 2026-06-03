use super::*;

#[test]
fn auto_flush_errors_surface_and_leave_the_manager_dirty_for_retry() {
    let root = temp_save_root();
    let occupied_path = root.join("occupied");
    fs::write(&occupied_path, b"not a directory").expect("occupied file should be creatable");

    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x03);

    let backend = FilesystemCartridgeSaveBackend::with_time_source(
        &occupied_path,
        FixedCartridgeSaveTimeSource::new(700),
    );
    let key = CartridgeSaveKey::new("flush_error").expect("key should be valid");
    let mut manager = HardwarePersistenceManager::new(
        backend,
        key,
        HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite,
    );

    let error = manager
        .note_persistible_write(&cartridge)
        .expect_err("auto-flush should surface filesystem errors");
    assert!(format!("{error}").contains("create save directory"));
    assert!(manager.is_dirty());

    let close_error = manager
        .close(&cartridge)
        .expect_err("close should retry and surface the same error");
    assert!(format!("{close_error}").contains("create save directory"));
    assert!(manager.is_dirty());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
