use super::*;

#[test]
fn skip_boot_uses_the_centralized_synthetic_startup_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.boot().boot_rom_kind(), BootRomKind::Dmg);
    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.cpu().startup_state().pc, 0x0100);
    assert_eq!(machine.cpu().startup_state().a, 0x01);
    assert_eq!(machine.cpu().startup_state().f, 0xB0);
    assert_eq!(
        machine.boot().startup_memory_policy(),
        StartupMemoryPolicy::DeterministicPatterned
    );

    assert_eq!(machine.read_bus(0xFF00), 0xCF);
    assert_eq!(machine.read_bus(0xFF01), 0x00);
    assert_eq!(machine.read_bus(0xFF02), 0x7E);
    assert_eq!(machine.read_bus(0xFF04), 0xAB);
    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF06), 0x00);
    assert_eq!(machine.read_bus(0xFF07), 0xF8);
    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(machine.read_bus(0xFF40), 0x91);
    assert_eq!(machine.read_bus(0xFF41), 0x85);
    assert_eq!(machine.read_bus(0xFF42), 0x00);
    assert_eq!(machine.read_bus(0xFF43), 0x00);
    assert_eq!(machine.read_bus(0xFF44), 0x00);
    assert_eq!(machine.read_bus(0xFF45), 0x00);
    assert_eq!(machine.read_bus(0xFF46), 0xFF);
    assert_eq!(machine.read_bus(0xFF47), 0xFC);
    assert_eq!(machine.read_bus(0xFF4A), 0x00);
    assert_eq!(machine.read_bus(0xFF4B), 0x00);
    assert_eq!(machine.read_bus(0xFFFF), 0x00);
    let wram_seed = machine.read_bus(0xC000);
    let hram_seed = machine.read_bus(0xFF80);
    assert_ne!(wram_seed, 0x00);
    assert_ne!(hram_seed, 0x00);

    let mut second_machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    assert_eq!(
        second_machine.boot().startup_memory_policy(),
        StartupMemoryPolicy::DeterministicPatterned
    );
    assert_eq!(second_machine.read_bus(0xC000), wram_seed);
    assert_eq!(second_machine.read_bus(0xFF80), hram_seed);
}

#[test]
fn skip_boot_recomputes_the_checksum_derived_f_register_when_a_cartridge_is_loaded() {
    let mut zero_checksum_machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut non_zero_checksum_machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    zero_checksum_machine
        .load_cartridge(build_test_rom(0x00))
        .expect("supported NoMBC image should load");
    non_zero_checksum_machine
        .load_cartridge(build_test_rom(0x7F))
        .expect("supported NoMBC image should load");

    assert_eq!(zero_checksum_machine.cpu().startup_state().f, 0x80);
    assert_eq!(non_zero_checksum_machine.cpu().startup_state().f, 0xB0);
}
