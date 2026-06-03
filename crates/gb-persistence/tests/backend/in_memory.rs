use super::*;

#[test]
fn in_memory_backend_round_trips_full_mbc1_ram_backing_store() {
    let mut cartridge = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x00);
    cartridge.write_ram(0xA000, 0x11);
    cartridge.write_rom(0x4000, 0x01);
    cartridge.write_ram(0xA000, 0x22);
    cartridge.write_rom(0x0000, 0x00);

    let key = CartridgeSaveKey::new("mbc1_roundtrip").expect("key should be valid");
    let mut backend = InMemoryCartridgeSaveBackend::with_time_source(
        FixedCartridgeSaveTimeSource::new(1_700_000_000),
    );
    let saved = backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    assert_eq!(saved.backend_metadata.saved_at_unix_seconds, 1_700_000_000);
    assert_eq!(
        saved.cartridge_metadata.profile,
        CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024,
            },
        }
    );

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    let mut restored = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    restored
        .restore_persistent_state(&loaded.persistent_state)
        .expect("restore should accept the persisted payload");

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x6000, 0x01);
    restored.write_rom(0x4000, 0x00);
    assert_eq!(restored.read_ram(0xA000), 0x11);
    restored.write_rom(0x4000, 0x01);
    assert_eq!(restored.read_ram(0xA000), 0x22);
}

#[test]
fn in_memory_backend_round_trips_mmm01_battery_ram_backing_store() {
    let mut cartridge = load_cartridge(build_mmm01_rom(0x03, 0x03, 0x0D));
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_rom(0x0000, 0x2A);
    cartridge.write_rom(0x0000, 0x6A);
    cartridge.write_ram(0xA000, 0x11);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_ram(0xA000, 0x22);
    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);
    cartridge.write_rom(0x0000, 0x00);

    let key = CartridgeSaveKey::new("mmm01_roundtrip").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(808));
    let saved = backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    assert_eq!(
        saved.cartridge_metadata.profile,
        CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024,
            },
        }
    );
    match saved.persistent_state {
        PersistentCartState::Mmm01Ram { ref ram } => {
            assert_eq!(ram[2 * 0x2000], 0x22);
            assert_eq!(ram[3 * 0x2000], 0x33);
        }
        ref other => panic!("expected MMM01 RAM payload, got {other:?}"),
    }

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    let mut restored = load_cartridge(build_mmm01_rom(0x03, 0x03, 0x0D));
    restored
        .restore_persistent_state(&loaded.persistent_state)
        .expect("restore should accept the persisted payload");

    restored.write_rom(0x4000, 0x02);
    restored.write_rom(0x0000, 0x2A);
    restored.write_rom(0x0000, 0x6A);
    assert_eq!(restored.read_ram(0xA000), 0x22);
    restored.write_rom(0x6000, 0x01);
    assert_eq!(restored.read_ram(0xA000), 0x22);
    restored.write_rom(0x4000, 0x03);
    assert_eq!(restored.read_ram(0xA000), 0x33);
}

#[test]
fn in_memory_backend_round_trips_huc1_battery_ram_backing_store() {
    let mut cartridge = load_cartridge(build_banked_huc1_rom(0x03, 0x03));
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_ram(0xA000, 0x22);
    cartridge.write_rom(0x0000, 0x0E);
    cartridge.write_ram(0xA000, 0x01);
    cartridge.write_rom(0x0000, 0x00);
    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);

    let key = CartridgeSaveKey::new("huc1_roundtrip").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(909));
    let saved = backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    assert_eq!(
        saved.cartridge_metadata.profile,
        CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024,
            },
        }
    );
    match saved.persistent_state {
        PersistentCartState::Huc1Ram { ref ram } => {
            assert_eq!(ram[2 * 0x2000], 0x22);
            assert_eq!(ram[3 * 0x2000], 0x33);
        }
        ref other => panic!("expected HuC1 RAM payload, got {other:?}"),
    }

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    let mut restored = load_cartridge(build_banked_huc1_rom(0x03, 0x03));
    restored
        .restore_persistent_state(&loaded.persistent_state)
        .expect("restore should accept the persisted payload");

    restored.write_rom(0x4000, 0x02);
    assert_eq!(restored.read_ram(0xA000), 0x22);
    restored.write_rom(0x0000, 0x0E);
    assert_eq!(restored.read_ram(0xA000), 0xC0);
    restored.write_rom(0x0000, 0x00);
    restored.write_rom(0x4000, 0x03);
    assert_eq!(restored.read_ram(0xA000), 0x33);
}

#[test]
fn in_memory_backend_round_trips_huc3_ram_and_rtc_state() {
    let mut cartridge = load_cartridge(build_banked_huc3_rom(0x03, 0x03));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_ram(0xA000, 0x22);
    cartridge.write_rom(0x0000, 0x0B);
    cartridge.write_ram(0xA000, 0x62);
    cartridge.write_rom(0x0000, 0x0D);
    cartridge.write_ram(0xA000, 0xFE);

    let key = CartridgeSaveKey::new("huc3_roundtrip").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(1001));
    let saved = backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    assert_eq!(
        saved.cartridge_metadata.profile,
        CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024,
            },
        }
    );
    match saved.persistent_state {
        PersistentCartState::Huc3 { ref ram, rtc, .. } => {
            assert_eq!(ram[2 * 0x2000], 0x22);
            assert_eq!(
                rtc,
                Huc3RtcPersistentState {
                    current_minutes_of_day: 0,
                    current_days: 0,
                    current_subminute_seconds: 0,
                    event_minutes_of_day: 0,
                    event_days: 0,
                }
            );
        }
        ref other => panic!("expected HuC-3 payload, got {other:?}"),
    }

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    let mut restored = load_cartridge(build_banked_huc3_rom(0x03, 0x03));
    restored
        .restore_persistent_state(&loaded.persistent_state)
        .expect("restore should accept the persisted payload");

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x4000, 0x02);
    assert_eq!(restored.read_ram(0xA000), 0x22);
    restored.write_rom(0x0000, 0x0C);
    assert_eq!(restored.read_ram(0xA000), 0xE1);
}

#[test]
fn backend_can_store_non_persistent_metadata_without_forcing_auto_save_policy() {
    let cartridge = load_cartridge(build_banked_mbc1_rom(0x02, 0x03, 0x03));
    let key = CartridgeSaveKey::new("non_persistent").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(99));

    let saved = backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    assert_eq!(
        saved.cartridge_metadata.profile,
        CartridgePersistenceProfile::NonPersistentRam {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024,
            },
        }
    );
    assert_eq!(saved.persistent_state, PersistentCartState::None);
}

#[test]
fn in_memory_backend_reports_queries_and_missing_saves_explicitly() {
    let key = CartridgeSaveKey::new("in_memory_queries").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(321));

    assert!(backend.is_empty());
    assert_eq!(backend.len(), 0);
    assert_eq!(backend.current_unix_seconds(), 321);
    assert_eq!(backend.load(&key).expect("load should succeed"), None);

    backend.delete(&key).expect("delete should succeed");
    assert!(backend.is_empty());
}
