use super::*;

#[test]
fn real_boot_reads_boot_rom_at_0000_until_ff50_handoff_restores_cartridge_visibility() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(BootRomKind::Dmg, build_boot_rom_image(0x99))
                    .expect("configured DMG boot ROM should validate"),
            ),
    );

    machine
        .load_cartridge(build_test_rom(0x7F))
        .expect("supported NoMBC image should load");

    assert_eq!(machine.cartridge().state(), CartridgeSlotState::NoMbc);
    assert!(machine.boot().is_boot_rom_mapped());
    assert!(machine.boot().has_boot_rom_asset());

    let boot_byte = machine.boot().read_boot_rom(0x0000);
    assert_eq!(boot_byte, 0x99);
    assert_eq!(machine.read_bus(0x0000), boot_byte);
    assert_eq!(machine.read_bus(0x0100), 0x31);
    assert_eq!(machine.read_bus(0x4000), 0x56);

    machine.write_bus(0xFF50, 0x01);

    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.read_bus(0x0000), 0x12);
    assert_eq!(machine.read_bus(0x0100), 0x31);
    assert_eq!(machine.read_bus(0x4000), 0x56);
}

#[test]
fn real_boot_executes_a_boot_rom_handoff_and_fetches_the_cartridge_entry_next() {
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
        .load_cartridge(build_phase_2_real_boot_rom(0xCE, 0x7F))
        .expect("supported NoMBC image should load");

    step_machine_until(
        &mut machine,
        PHASE_2_REAL_BOOT_HANDOFF_T_CYCLE_LIMIT,
        |machine| !machine.boot().is_boot_rom_mapped(),
    );

    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.cpu().registers().pc, 0x0100);
    assert_eq!(machine.cpu().registers().a, 0x42);
    assert_eq!(machine.cpu().registers().b, 0x24);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().current_opcode(), None);
    assert_eq!(machine.read_bus(0x0000), 0x12);
    assert_eq!(machine.read_bus(0x0100), PHASE_2_ENTRY_OPCODE);

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::InvalidOpcode {
                opcode: PHASE_2_ENTRY_OPCODE,
                address: 0x0100,
            },
        }
    );
    assert_eq!(machine.cpu().current_opcode(), Some(PHASE_2_ENTRY_OPCODE));
}

#[test]
fn real_boot_can_source_boot_rom_assets_from_a_directory() {
    let directory = unique_temp_dir();
    fs::create_dir_all(&directory).expect("temporary asset directory should be creatable");
    fs::write(
        directory.join(BootRomAssets::filename(BootRomKind::Dmg)),
        build_boot_rom_image(0x66),
    )
    .expect("boot ROM asset file should be writable");

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::from_directory(&directory)
                    .expect("directory-backed boot ROM assets should load"),
            ),
    );

    machine
        .load_cartridge(build_test_rom(0x7F))
        .expect("supported NoMBC image should load");

    assert!(machine.boot().has_boot_rom_asset());
    assert_eq!(machine.read_bus(0x0000), 0x66);

    fs::remove_dir_all(&directory).expect("temporary asset directory should be removable");
}
