use super::*;

#[test]
fn filesystem_save_store_uses_internal_only_for_huc3() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("huc3").expect("key should be valid");
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let state = huc3_persistent_state(vec![0x22, 0x33]);
    let mut store = FilesystemCartridgeSaveStore::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(30),
    );

    let written = store
        .save(&key, metadata, &state)
        .expect("HuC-3 should save internally");
    assert_eq!(
        written.format,
        FilesystemCartridgeSaveStorageFormat::InternalEnvelope
    );
    assert_eq!(written.path, store.internal_path_for_key(&key));
    assert!(store.internal_path_for_key(&key).is_file());
    assert!(!store.external_path_for_key(&key).exists());
    let loaded = store
        .load(&key, metadata, &state)
        .expect("HuC-3 internal load should succeed")
        .expect("HuC-3 save should exist");
    assert_eq!(
        loaded.format,
        FilesystemCartridgeSaveStorageFormat::InternalEnvelope
    );
    assert_eq!(loaded.envelope.persistent_state, state);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn filesystem_save_store_keeps_mbc6_internal_fallback_sticky() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc6").expect("key should be valid");
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRamAndFlash {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
            flash_byte_len: 4,
            hidden_byte_len: 2,
        },
    };
    let external_state = PersistentCartState::Mbc6 {
        ram: vec![0x10, 0x20],
        flash: vec![0xFF, 0x7F, 0x3F, 0x1F],
        hidden_region: vec![0xFF; 2],
        sector0_protected: false,
    };
    let internal_state = PersistentCartState::Mbc6 {
        ram: vec![0x10, 0x20],
        flash: vec![0xFF, 0x7F, 0x3F, 0x1F],
        hidden_region: vec![0xFE, 0xFF],
        sector0_protected: false,
    };
    let mut store = FilesystemCartridgeSaveStore::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(40),
    );

    let external_write = store
        .save(&key, metadata, &external_state)
        .expect("default MBC6 state should save externally");
    assert_eq!(
        external_write.format,
        FilesystemCartridgeSaveStorageFormat::External
    );
    assert!(store.external_path_for_key(&key).is_file());
    assert!(!store.internal_path_for_key(&key).exists());

    let internal_write = store
        .save(&key, metadata, &internal_state)
        .expect("non-default MBC6 hidden state should fall back internally");
    assert_eq!(
        internal_write.format,
        FilesystemCartridgeSaveStorageFormat::InternalEnvelope
    );
    assert!(store.internal_path_for_key(&key).is_file());

    let sticky_write = store
        .save(&key, metadata, &external_state)
        .expect("MBC6 should stay internal once fallback exists");
    assert_eq!(
        sticky_write.format,
        FilesystemCartridgeSaveStorageFormat::InternalEnvelope
    );
    let loaded = store
        .load(&key, metadata, &external_state)
        .expect("MBC6 load should succeed")
        .expect("MBC6 internal save should exist");
    assert_eq!(
        loaded.format,
        FilesystemCartridgeSaveStorageFormat::InternalEnvelope
    );

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
