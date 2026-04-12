use super::*;

#[test]
fn real_boot_with_an_invalid_logo_stays_mapped_and_never_reaches_cartridge_entry() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(BootRomKind::Dmg, build_phase_2_boot_rom(0xCE, 0x7F))
                    .expect("phase 2.4 synthetic DMG boot ROM should validate"),
            ),
    );

    machine
        .load_cartridge(build_phase_2_real_boot_rom(0x00, 0x7F))
        .expect("supported NoMBC image should load");

    step_machine_t_cycles(&mut machine, PHASE_2_REAL_BOOT_HANDOFF_T_CYCLE_LIMIT);

    assert!(machine.boot().is_boot_rom_mapped());
    assert!(machine.cpu().registers().pc <= 0x0007);
    assert_eq!(machine.cpu().registers().b, 0x00);
    assert_eq!(machine.read_bus(0x0000), 0xFA);
    assert_eq!(machine.read_bus(0x0100), PHASE_2_ENTRY_OPCODE);
}

#[test]
fn real_boot_with_an_invalid_checksum_stays_mapped_and_never_reaches_cartridge_entry() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(BootRomKind::Dmg, build_phase_2_boot_rom(0xCE, 0x7F))
                    .expect("phase 2.4 synthetic DMG boot ROM should validate"),
            ),
    );

    machine
        .load_cartridge(build_phase_2_real_boot_rom(0xCE, 0x00))
        .expect("supported NoMBC image should load");

    step_machine_t_cycles(&mut machine, PHASE_2_REAL_BOOT_HANDOFF_T_CYCLE_LIMIT);

    assert!(machine.boot().is_boot_rom_mapped());
    assert!(machine.cpu().registers().pc <= 0x000E);
    assert_eq!(machine.cpu().registers().b, 0x00);
    assert_eq!(machine.read_bus(0x0000), 0xFA);
    assert_eq!(machine.read_bus(0x0100), PHASE_2_ENTRY_OPCODE);
}
