use gb_core::{
    BootStatus, Bus, ConsoleModel, DmaTransferState, IoRegisterAvailability, IoRegisterKind,
    IoRegisterOwner, Machine, MachineConfig, StartupMode,
};

#[test]
fn public_io_descriptor_table_covers_all_mmio_addresses_and_ie_without_gaps() {
    let bus = Bus::new(ConsoleModel::Dmg);

    for address in 0xFF00..=0xFF7F {
        assert!(
            bus.describe_io_register(address).is_some(),
            "missing MMIO contract for {address:#06X}"
        );
    }

    assert!(bus.describe_io_register(0xFFFF).is_some());

    assert_eq!(
        bus.describe_io_register(0xFF00).unwrap().owner(),
        IoRegisterOwner::Joypad
    );
    assert_eq!(
        bus.describe_io_register(0xFF46).unwrap().kind(),
        IoRegisterKind::OamDma
    );
    assert_eq!(
        bus.describe_io_register(0xFF50).unwrap().kind(),
        IoRegisterKind::BootRomDisable
    );
    assert_eq!(
        bus.describe_io_register(0xFF70).unwrap().availability(),
        IoRegisterAvailability::CgbOnly
    );
}

#[test]
fn machine_routes_phase_1_mmio_registers_through_real_subsystem_owners() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF00, 0x10);
    machine.write_bus(0xFF01, 0x12);
    machine.write_bus(0xFF02, 0xFF);
    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(0xFF12, 0xF3);
    machine.write_bus(0xFF14, 0x80);
    machine.write_bus(0xFF30, 0x12);
    machine.write_bus(0xFF05, 0x12);
    machine.write_bus(0xFF06, 0x34);
    machine.write_bus(0xFF07, 0xFF);
    machine.write_bus(0xFF0F, 0x04);
    machine.write_bus(0xFF40, 0x83);
    machine.write_bus(0xFF41, 0xFF);
    machine.write_bus(0xFF42, 0x56);
    machine.write_bus(0xFF43, 0x78);
    machine.write_bus(0xFF44, 0x99);
    machine.write_bus(0xFF45, 0x00);
    machine.write_bus(0xFF47, 0xE4);
    machine.write_bus(0xFF4A, 0x9A);
    machine.write_bus(0xFF4B, 0xBC);
    machine.write_bus(0xFFFF, 0x1F);

    assert_eq!(machine.read_bus(0xFF00), 0xDF);
    assert_eq!(machine.read_bus(0xFF01), 0x12);
    assert_eq!(machine.read_bus(0xFF02), 0xFF);
    assert_eq!(machine.read_bus(0xFF12), 0xF3);
    assert_eq!(machine.read_bus(0xFF14), 0xBF);
    assert_eq!(machine.read_bus(0xFF26), 0xF1);
    assert_eq!(machine.read_bus(0xFF30), 0x12);
    assert_eq!(machine.read_bus(0xFF04), 0xAB);
    assert_eq!(machine.read_bus(0xFF05), 0x12);
    assert_eq!(machine.read_bus(0xFF06), 0x34);
    assert_eq!(machine.read_bus(0xFF07), 0xFF);
    assert_eq!(machine.read_bus(0xFF0F), 0xE4);
    assert_eq!(machine.read_bus(0xFF40), 0x83);
    assert_eq!(machine.read_bus(0xFF41), 0xFD);
    assert_eq!(machine.read_bus(0xFF42), 0x56);
    assert_eq!(machine.read_bus(0xFF43), 0x78);
    assert_eq!(machine.read_bus(0xFF44), 0x00);
    assert_eq!(machine.read_bus(0xFF45), 0x00);
    assert_eq!(machine.read_bus(0xFF47), 0xE4);
    assert_eq!(machine.read_bus(0xFF4A), 0x9A);
    assert_eq!(machine.read_bus(0xFF4B), 0xBC);
    assert_eq!(machine.read_bus(0xFFFF), 0x1F);
}

#[test]
fn ff46_and_ff50_writes_take_effect_immediately_on_their_owners() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::RealBoot),
    );

    assert_eq!(machine.boot().status(), BootStatus::Ready);
    assert!(machine.boot().is_boot_rom_mapped());

    machine.write_bus(0xFF46, 0x12);
    machine.write_bus(0xFF50, 0x01);

    assert_eq!(machine.read_bus(0xFF46), 0x12);
    assert_eq!(machine.dma().source_page_latch(), 0x12);
    assert_eq!(
        machine.dma().transfer_state(),
        DmaTransferState::OamStartRequested { source_page: 0x12 }
    );
    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.read_bus(0xFF50), 0xFF);
}

#[test]
fn dmg_family_reads_ff_and_ignores_writes_for_unavailable_cgb_registers() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    for address in [0xFF4D, 0xFF4F, 0xFF70, 0xFF76] {
        machine.write_bus(address, 0xA5);
        assert_eq!(machine.read_bus(address), 0xFF);
    }
}
