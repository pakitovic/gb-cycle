use super::*;

#[test]
fn cgb_real_boot_uses_a_cgb_boot_asset_for_the_split_boot_windows() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(
                        HardwareRevision::CpuCgbE,
                        build_cgb_boot_rom_image(0x99, 0x77),
                    )
                    .expect("configured CGB boot ROM should validate"),
            ),
    );

    machine
        .load_cartridge(build_test_rom(0x7F))
        .expect("supported NoMBC image should load");

    assert_eq!(machine.boot().revision(), HardwareRevision::CpuCgbE);
    assert!(machine.boot().is_boot_rom_mapped());
    assert!(machine.boot().has_boot_rom_asset());
    assert_eq!(machine.read_bus(0x0000), 0x99);
    assert_eq!(machine.read_bus(0x0200), 0x77);
    assert_eq!(machine.read_bus(0x0100), 0x31);
}
