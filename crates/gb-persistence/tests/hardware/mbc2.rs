use super::*;

#[test]
fn battery_gated_helper_round_trips_mbc2_nibble_ram() {
    let mut source = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    source.write_rom(0x0000, 0x0A);
    source.write_ram(0xA000, 0x0C);
    source.write_ram(0xA001, 0x03);
    source.write_rom(0x0000, 0x00);

    let key = CartridgeSaveKey::new("mbc2_hardware").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(5));
    let save_result = save_hardware_cartridge_persistence(&mut backend, &key, &source)
        .expect("battery-backed MBC2 save should succeed");
    assert!(matches!(
        save_result,
        HardwarePersistenceSaveResult::Saved(_)
    ));

    let mut restored = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let load_result = load_hardware_cartridge_persistence(&backend, &key, &mut restored)
        .expect("battery-backed MBC2 load should succeed");
    assert!(matches!(
        load_result,
        HardwarePersistenceLoadResult::Restored { .. }
    ));

    restored.write_rom(0x0000, 0x0A);
    assert_eq!(restored.read_ram(0xA000) & 0x0F, 0x0C);
    assert_eq!(restored.read_ram(0xA001) & 0x0F, 0x03);
}
