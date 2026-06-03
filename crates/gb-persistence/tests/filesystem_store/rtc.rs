use super::*;

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
