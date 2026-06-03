use super::*;

#[test]
fn battery_gated_helper_round_trips_mbc7_raw_eeprom() {
    let mut source = load_cartridge(build_mbc7_rom());
    let mut eeprom = vec![0xFF; 256];
    eeprom[0] = 0x12;
    eeprom[1] = 0x34;
    eeprom[254] = 0xAB;
    eeprom[255] = 0xCD;
    source
        .restore_persistent_state(&PersistentCartState::Mbc7Eeprom {
            eeprom: eeprom.clone(),
        })
        .expect("MBC7 should accept a raw EEPROM payload");

    assert!(uses_battery_backed_hardware_persistence(
        source.persistence_metadata()
    ));

    let key = CartridgeSaveKey::new("mbc7_hardware").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(777));
    let save_result = save_hardware_cartridge_persistence(&mut backend, &key, &source)
        .expect("MBC7 EEPROM save should succeed");
    assert!(matches!(
        save_result,
        HardwarePersistenceSaveResult::Saved(_)
    ));

    let mut restored = load_cartridge(build_mbc7_rom());
    let load_result = load_hardware_cartridge_persistence(&backend, &key, &mut restored)
        .expect("MBC7 EEPROM load should succeed");
    assert!(matches!(
        load_result,
        HardwarePersistenceLoadResult::Restored { .. }
    ));

    match restored.persistent_state() {
        PersistentCartState::Mbc7Eeprom { eeprom: restored } => assert_eq!(restored, eeprom),
        other => panic!("expected MBC7 EEPROM payload, got {other:?}"),
    }
}
