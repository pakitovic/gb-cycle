use super::*;

#[test]
fn battery_gated_helper_round_trips_mbc3_ram_and_rtc() {
    let mut source = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    source.write_rom(0x0000, 0x0A);
    source.write_rom(0x4000, 0x00);
    source.write_ram(0xA000, 0x44);
    source.write_rom(0x4000, 0x01);
    source.write_ram(0xA000, 0x99);

    source.write_rom(0x4000, 0x08);
    source.write_ram(0xA000, 42);
    source.write_rom(0x4000, 0x09);
    source.write_ram(0xA000, 17);
    source.write_rom(0x4000, 0x0A);
    source.write_ram(0xA000, 9);
    source.write_rom(0x4000, 0x0B);
    source.write_ram(0xA000, 1);
    source.write_rom(0x4000, 0x0C);
    source.write_ram(0xA000, 0x00);
    source.write_rom(0x0000, 0x00);

    let key = CartridgeSaveKey::new("mbc3_hardware").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(77));
    let save_result = save_hardware_cartridge_persistence(&mut backend, &key, &source)
        .expect("battery-backed MBC3 save should succeed");
    assert!(matches!(
        save_result,
        HardwarePersistenceSaveResult::Saved(_)
    ));

    let mut restored = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    let load_result = load_hardware_cartridge_persistence(&backend, &key, &mut restored)
        .expect("battery-backed MBC3 load should succeed");
    assert!(matches!(
        load_result,
        HardwarePersistenceLoadResult::Restored { .. }
    ));

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x4000, 0x00);
    assert_eq!(restored.read_ram(0xA000), 0x44);
    restored.write_rom(0x4000, 0x01);
    assert_eq!(restored.read_ram(0xA000), 0x99);

    restored.write_rom(0x6000, 0x00);
    restored.write_rom(0x6000, 0x01);

    restored.write_rom(0x4000, 0x08);
    assert_eq!(restored.read_ram(0xA000), 42);
    restored.write_rom(0x4000, 0x09);
    assert_eq!(restored.read_ram(0xA000), 17);
    restored.write_rom(0x4000, 0x0A);
    assert_eq!(restored.read_ram(0xA000), 9);
    restored.write_rom(0x4000, 0x0B);
    assert_eq!(restored.read_ram(0xA000), 1);
    restored.write_rom(0x4000, 0x0C);
    assert_eq!(restored.read_ram(0xA000), 0x00);
}

#[test]
fn battery_gated_helper_applies_elapsed_seconds_to_mbc3_on_reload() {
    let mut source = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    source.write_rom(0x0000, 0x0A);
    source.write_rom(0x4000, 0x08);
    source.write_ram(0xA000, 59);
    source.write_rom(0x4000, 0x09);
    source.write_ram(0xA000, 59);
    source.write_rom(0x4000, 0x0A);
    source.write_ram(0xA000, 23);
    source.write_rom(0x4000, 0x0B);
    source.write_ram(0xA000, 0xFF);
    source.write_rom(0x4000, 0x0C);
    source.write_ram(0xA000, 0x01);
    source.write_rom(0x0000, 0x00);

    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc3_elapsed").expect("key should be valid");
    let mut save_backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(100),
    );
    save_hardware_cartridge_persistence(&mut save_backend, &key, &source)
        .expect("save should succeed");

    let load_backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(102),
    );
    let mut restored = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    let load_result = load_hardware_cartridge_persistence(&load_backend, &key, &mut restored)
        .expect("load should succeed");
    assert!(matches!(
        load_result,
        HardwarePersistenceLoadResult::Restored {
            elapsed_off_session_seconds: 2,
            ..
        }
    ));

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x6000, 0x00);
    restored.write_rom(0x6000, 0x01);
    restored.write_rom(0x4000, 0x08);
    assert_eq!(restored.read_ram(0xA000), 1);
    restored.write_rom(0x4000, 0x09);
    assert_eq!(restored.read_ram(0xA000), 0);
    restored.write_rom(0x4000, 0x0A);
    assert_eq!(restored.read_ram(0xA000), 0);
    restored.write_rom(0x4000, 0x0B);
    assert_eq!(restored.read_ram(0xA000), 0);
    restored.write_rom(0x4000, 0x0C);
    assert_eq!(restored.read_ram(0xA000), 0x80);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn battery_gated_helper_does_not_advance_halted_mbc3_on_reload() {
    let mut source = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    source.write_rom(0x0000, 0x0A);
    source.write_rom(0x4000, 0x08);
    source.write_ram(0xA000, 12);
    source.write_rom(0x4000, 0x09);
    source.write_ram(0xA000, 34);
    source.write_rom(0x4000, 0x0A);
    source.write_ram(0xA000, 5);
    source.write_rom(0x4000, 0x0B);
    source.write_ram(0xA000, 9);
    source.write_rom(0x4000, 0x0C);
    source.write_ram(0xA000, 0x40);
    source.write_rom(0x0000, 0x00);

    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc3_halted_elapsed").expect("key should be valid");
    let mut save_backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(200),
    );
    save_hardware_cartridge_persistence(&mut save_backend, &key, &source)
        .expect("save should succeed");

    let load_backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(400),
    );
    let mut restored = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    let load_result = load_hardware_cartridge_persistence(&load_backend, &key, &mut restored)
        .expect("load should succeed");
    assert!(matches!(
        load_result,
        HardwarePersistenceLoadResult::Restored {
            elapsed_off_session_seconds: 200,
            ..
        }
    ));

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x6000, 0x00);
    restored.write_rom(0x6000, 0x01);
    restored.write_rom(0x4000, 0x08);
    assert_eq!(restored.read_ram(0xA000), 12);
    restored.write_rom(0x4000, 0x09);
    assert_eq!(restored.read_ram(0xA000), 34);
    restored.write_rom(0x4000, 0x0A);
    assert_eq!(restored.read_ram(0xA000), 5);
    restored.write_rom(0x4000, 0x0B);
    assert_eq!(restored.read_ram(0xA000), 9);
    restored.write_rom(0x4000, 0x0C);
    assert_eq!(restored.read_ram(0xA000), 0x40);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}
