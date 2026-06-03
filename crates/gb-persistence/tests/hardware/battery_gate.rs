use super::*;

#[test]
fn battery_gated_helper_skips_non_battery_ram_cartridges_by_default() {
    let mut cartridge = load_cartridge(build_banked_mbc1_rom(0x02, 0x03, 0x03));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x01);
    cartridge.write_ram(0xA000, 0x66);

    assert!(!uses_battery_backed_hardware_persistence(
        cartridge.persistence_metadata()
    ));

    let key = CartridgeSaveKey::new("skip_non_battery").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(123));
    let save_result = save_hardware_cartridge_persistence(&mut backend, &key, &cartridge)
        .expect("save helper should not fail");

    assert_eq!(
        save_result,
        HardwarePersistenceSaveResult::SkippedNotBatteryBacked
    );
    assert_eq!(backend.len(), 0);

    let mut battery_backed_source = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    battery_backed_source.write_rom(0x0000, 0x0A);
    battery_backed_source.write_rom(0x6000, 0x01);
    battery_backed_source.write_rom(0x4000, 0x00);
    battery_backed_source.write_ram(0xA000, 0x11);
    backend
        .save(
            &key,
            battery_backed_source.persistence_metadata(),
            &battery_backed_source.persistent_state(),
        )
        .expect("raw backend save should succeed");

    let load_result = load_hardware_cartridge_persistence(&backend, &key, &mut cartridge)
        .expect("load helper should not fail");
    assert_eq!(
        load_result,
        HardwarePersistenceLoadResult::SkippedNotBatteryBacked
    );

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x66);
}
