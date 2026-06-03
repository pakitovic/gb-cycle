use super::*;

#[test]
fn filesystem_save_store_does_not_autoload_external_stable_legacy_envelopes() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("legacy_mbc1").expect("key should be valid");
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let legacy_state = PersistentCartState::Mbc1Ram {
        ram: vec![0xAA, 0xBB],
    };
    let target_state = PersistentCartState::Mbc1Ram { ram: vec![0; 2] };
    let mut legacy_backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(20),
    );
    legacy_backend
        .save(&key, metadata, &legacy_state)
        .expect("legacy internal envelope should save");

    let store = FilesystemCartridgeSaveStore::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(21),
    );
    assert_eq!(
        store
            .load(&key, metadata, &target_state)
            .expect("load should succeed"),
        None
    );

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
