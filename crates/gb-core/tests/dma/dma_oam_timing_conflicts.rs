use super::*;

#[test]
fn external_bus_dma_redirects_cpu_reads_to_the_most_recent_source_byte_after_the_first_copy() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_dma_source_page(&mut machine, 0xC1, 0x33);
    machine.write_bus(0xC200, 0xAA);
    machine.write_bus(0xFF46, 0xC1);
    for _ in 0..8 {
        machine.step_t_cycle();
    }

    assert_eq!(
        machine.dma().bus_state(),
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0xC100))
    );
    assert_eq!(machine.read_bus(0xC200), dma_source_byte(0x33, 0));
}

#[test]
fn external_bus_dma_redirects_cpu_writes_to_the_most_recent_source_byte_after_the_first_copy() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_dma_source_page(&mut machine, 0xC1, 0x47);
    machine.write_bus(0xC200, 0x55);
    machine.write_bus(0xFF46, 0xC1);
    for _ in 0..8 {
        machine.step_t_cycle();
    }

    machine.write_bus(0xC200, 0x99);

    for _ in 0..640 {
        machine.step_t_cycle();
    }

    assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());
    assert_eq!(machine.read_bus(0xC100), 0x99);
    assert_eq!(machine.read_bus(0xC200), 0x55);
}
