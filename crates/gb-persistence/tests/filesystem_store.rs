mod common;

use common::*;
use gb_core::{
    CartridgePersistenceMetadata, CartridgePersistenceProfile, CartridgeRamPayloadKind,
    Mbc3RtcPersistentState, PersistentCartState,
};
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveBackendError, CartridgeSaveFileExtension, CartridgeSaveKey,
    EXTERNAL_SAVE_FILE_EXTENSION, EXTERNAL_SAVE_FILE_EXTENSION_P2, EXTERNAL_SAVE_FILE_EXTENSION_P3,
    EXTERNAL_SAVE_FILE_EXTENSION_P4, FilesystemCartridgeSaveBackend,
    FilesystemCartridgeSaveStorageFormat, FilesystemCartridgeSaveStore,
    FixedCartridgeSaveTimeSource,
};
use std::fs;

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

#[test]
fn filesystem_save_store_external_rtc_load_uses_current_timestamp() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc3_rtc").expect("key should be valid");
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: true,
        profile: CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
        },
    };
    let state = PersistentCartState::Mbc3RamRtc {
        ram: vec![0xAB, 0xCD],
        rtc: Mbc3RtcPersistentState {
            seconds: 58,
            minutes: 59,
            hours: 23,
            day_counter: 7,
            halt: false,
            carry: false,
        },
    };

    let mut save_store = FilesystemCartridgeSaveStore::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(100),
    );
    save_store
        .save(&key, metadata, &state)
        .expect("MBC3 RAM+RTC should save externally");
    let load_store = FilesystemCartridgeSaveStore::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(105),
    );
    let loaded = load_store
        .load(&key, metadata, &state)
        .expect("external RTC load should succeed")
        .expect("external save should exist");

    assert_eq!(
        loaded.format,
        FilesystemCartridgeSaveStorageFormat::External
    );
    assert_eq!(loaded.envelope.backend_metadata.saved_at_unix_seconds, 105);
    assert_eq!(
        loaded.envelope.persistent_state,
        PersistentCartState::Mbc3RamRtc {
            ram: vec![0xAB, 0xCD],
            rtc: Mbc3RtcPersistentState {
                seconds: 3,
                minutes: 0,
                hours: 0,
                day_counter: 8,
                halt: false,
                carry: false,
            },
        }
    );

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

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
