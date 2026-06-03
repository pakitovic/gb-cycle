use super::*;

#[test]
fn filesystem_save_store_treats_shape_mismatches_as_hard_errors() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("bad_shape").expect("key should be valid");
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let state = PersistentCartState::Mbc2Ram {
        ram_nibbles: [0; 512],
    };
    let mut store = FilesystemCartridgeSaveStore::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(50),
    );

    let error = store
        .save(&key, metadata, &state)
        .expect_err("state/profile mismatches should not fall back internally");
    assert!(matches!(
        error,
        CartridgeSaveBackendError::ExternalSave { .. }
    ));
    assert!(!store.external_path_for_key(&key).exists());
    assert!(!store.internal_path_for_key(&key).exists());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
