use super::*;

#[test]
fn filesystem_save_store_uses_external_primary_slot_extensions() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("slot_extension").expect("key should be valid");
    let slot_extensions = [
        (CartridgeSaveFileExtension::P1, EXTERNAL_SAVE_FILE_EXTENSION),
        (
            CartridgeSaveFileExtension::P2,
            EXTERNAL_SAVE_FILE_EXTENSION_P2,
        ),
        (
            CartridgeSaveFileExtension::P3,
            EXTERNAL_SAVE_FILE_EXTENSION_P3,
        ),
        (
            CartridgeSaveFileExtension::P4,
            EXTERNAL_SAVE_FILE_EXTENSION_P4,
        ),
    ];
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let state = PersistentCartState::Mbc1Ram {
        ram: vec![0x12, 0x34],
    };

    for (file_extension, expected_suffix) in slot_extensions {
        let mut store = FilesystemCartridgeSaveStore::with_time_source_and_file_extension(
            &root,
            FixedCartridgeSaveTimeSource::new(10),
            file_extension,
        );
        assert_eq!(
            store.external_path_for_key(&key),
            root.join(format!("{}.{expected_suffix}", key.as_str()))
        );
        let written = store
            .save(&key, metadata, &state)
            .expect("external-primary state should save");
        assert_eq!(
            written.format,
            FilesystemCartridgeSaveStorageFormat::External
        );
        assert_eq!(written.path, store.external_path_for_key(&key));
        assert!(written.path.is_file());
        assert!(!store.internal_path_for_key(&key).exists());
    }

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
