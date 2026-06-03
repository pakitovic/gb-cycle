use super::*;

#[test]
fn manager_accessors_and_load_into_cover_no_save_and_skip_paths() {
    let key = CartridgeSaveKey::new("manager_accessors").expect("key should be valid");
    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(900));
    let mut manager = HardwarePersistenceManager::new(
        backend,
        key.clone(),
        HardwarePersistenceFlushPolicy::Manual,
    );

    assert_eq!(manager.key(), &key);
    assert_eq!(
        manager.flush_policy(),
        HardwarePersistenceFlushPolicy::Manual
    );
    assert_eq!(manager.backend().current_unix_seconds(), 900);
    manager.set_flush_policy(HardwarePersistenceFlushPolicy::SaveOnClose);
    assert_eq!(
        manager.flush_policy(),
        HardwarePersistenceFlushPolicy::SaveOnClose
    );

    let battery_backed = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    assert_eq!(
        manager
            .flush(&battery_backed)
            .expect("clean flush should succeed"),
        HardwarePersistenceActionResult::NoPendingSave
    );

    let non_battery = load_cartridge(build_banked_mbc1_rom(0x02, 0x03, 0x03));
    assert_eq!(
        manager
            .note_persistible_write(&non_battery)
            .expect("non-battery note should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert_eq!(
        manager
            .close(&non_battery)
            .expect("non-battery close should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert!(!manager.is_dirty());

    let mut restore_target = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    assert_eq!(
        manager
            .load_into(&mut restore_target)
            .expect("load_into should succeed"),
        HardwarePersistenceLoadResult::NoSavePresent
    );

    let backend = manager.into_backend();
    assert!(backend.is_empty());
}

#[test]
fn manager_backend_mut_can_seed_saves_and_surface_restore_failures() {
    let key = CartridgeSaveKey::new("restore_failure").expect("key should be valid");
    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(1_200));
    let mut manager = HardwarePersistenceManager::new(
        backend,
        key.clone(),
        HardwarePersistenceFlushPolicy::Manual,
    );

    let mut source = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    source.write_rom(0x0000, 0x0A);
    source.write_ram(0xA000, 0x0D);
    source.write_rom(0x0000, 0x00);

    manager
        .backend_mut()
        .save(
            &key,
            source.persistence_metadata(),
            &source.persistent_state(),
        )
        .expect("manual backend seeding should succeed");
    assert_eq!(manager.backend().len(), 1);

    let mut incompatible_target = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    let error = manager
        .load_into(&mut incompatible_target)
        .expect_err("restore should reject a mismatched persistent state kind");
    assert!(matches!(
        error,
        HardwarePersistenceError::Restore(
            gb_core::CartridgePersistentStateError::KindMismatch { .. }
        )
    ));
    assert!(format!("{error}").contains("cartridge restore failed"));
    assert!(std::error::Error::source(&error).is_none());
}
