use super::*;

#[test]
fn manager_operations_keep_non_battery_cartridges_on_the_explicit_skipped_path() {
    let mut cartridge = load_cartridge(build_banked_mbc1_rom(0x02, 0x03, 0x03));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x44);

    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(321));
    let key = CartridgeSaveKey::new("manager_skip_non_battery").expect("key should be valid");
    let mut manager =
        HardwarePersistenceManager::new(backend, key, HardwarePersistenceFlushPolicy::Manual);

    assert_eq!(
        manager
            .note_persistible_write(&cartridge)
            .expect("note should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert!(!manager.is_dirty());

    assert_eq!(
        manager.flush(&cartridge).expect("flush should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert_eq!(
        manager
            .force_save(&cartridge)
            .expect("force save should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert_eq!(
        manager.close(&cartridge).expect("close should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert_eq!(manager.backend().len(), 0);
}
