mod common;

use common::*;
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveKey, FilesystemCartridgeSaveBackend,
    FixedCartridgeSaveTimeSource, HardwarePersistenceActionResult, HardwarePersistenceError,
    HardwarePersistenceFlushPolicy, HardwarePersistenceLoadResult, HardwarePersistenceManager,
    HardwarePersistenceTrigger, InMemoryCartridgeSaveBackend,
};
use std::fs;

#[test]
fn manual_flush_policy_requires_explicit_flush_and_supports_force_save() {
    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x09);
    cartridge.write_rom(0x0000, 0x00);

    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(300));
    let key = CartridgeSaveKey::new("manual_policy").expect("key should be valid");
    let mut manager =
        HardwarePersistenceManager::new(backend, key, HardwarePersistenceFlushPolicy::Manual);

    let write_result = manager
        .note_persistible_write(&cartridge)
        .expect("manual policy write notification should succeed");
    assert_eq!(write_result, HardwarePersistenceActionResult::Deferred);
    assert!(manager.is_dirty());
    assert_eq!(manager.backend().len(), 0);

    let close_result = manager
        .close(&cartridge)
        .expect("manual policy close should not fail");
    assert_eq!(
        close_result,
        HardwarePersistenceActionResult::SkippedByFlushPolicy {
            trigger: HardwarePersistenceTrigger::Close,
        }
    );
    assert!(manager.is_dirty());
    assert_eq!(manager.backend().len(), 0);

    let flush_result = manager.flush(&cartridge).expect("flush should succeed");
    assert!(matches!(
        flush_result,
        HardwarePersistenceActionResult::Saved {
            trigger: HardwarePersistenceTrigger::ManualFlush,
            ..
        }
    ));
    assert!(!manager.is_dirty());
    assert_eq!(manager.backend().len(), 1);

    let force_result = manager
        .force_save(&cartridge)
        .expect("force save should succeed even when clean");
    assert!(matches!(
        force_result,
        HardwarePersistenceActionResult::Saved {
            trigger: HardwarePersistenceTrigger::ForcedSave,
            ..
        }
    ));
    assert!(!manager.is_dirty());
    assert_eq!(manager.backend().len(), 1);
}

#[test]
fn save_on_close_policy_flushes_when_session_closes() {
    let mut cartridge = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x01);
    cartridge.write_ram(0xA000, 0x5A);

    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(400));
    let key = CartridgeSaveKey::new("save_on_close").expect("key should be valid");
    let mut manager =
        HardwarePersistenceManager::new(backend, key, HardwarePersistenceFlushPolicy::SaveOnClose);

    assert_eq!(
        manager
            .note_persistible_write(&cartridge)
            .expect("write notification should succeed"),
        HardwarePersistenceActionResult::Deferred
    );
    assert!(manager.is_dirty());
    assert_eq!(manager.backend().len(), 0);

    let close_result = manager.close(&cartridge).expect("close should succeed");
    assert!(matches!(
        close_result,
        HardwarePersistenceActionResult::Saved {
            trigger: HardwarePersistenceTrigger::Close,
            ..
        }
    ));
    assert!(!manager.is_dirty());
    assert_eq!(manager.backend().len(), 1);
}

#[test]
fn auto_flush_policy_saves_immediately_after_persistible_writes() {
    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x07);

    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(500));
    let key = CartridgeSaveKey::new("auto_flush").expect("key should be valid");
    let mut manager = HardwarePersistenceManager::new(
        backend,
        key,
        HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite,
    );

    let write_result = manager
        .note_persistible_write(&cartridge)
        .expect("auto-flush write notification should succeed");
    assert!(matches!(
        write_result,
        HardwarePersistenceActionResult::Saved {
            trigger: HardwarePersistenceTrigger::PersistibleWrite,
            ..
        }
    ));
    assert!(!manager.is_dirty());
    assert_eq!(manager.backend().len(), 1);

    assert_eq!(
        manager.close(&cartridge).expect("close should succeed"),
        HardwarePersistenceActionResult::NoPendingSave
    );
}

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
