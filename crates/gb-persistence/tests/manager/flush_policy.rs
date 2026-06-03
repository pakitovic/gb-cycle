use super::*;

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
