use super::*;

#[test]
fn external_bus_dma_restricts_machine_bus_access_to_hram_and_ff46_only_from_live_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xC000, 0x34);
    machine.write_bus(0x8000, 0x78);
    machine.write_bus(0xFF80, 0x12);
    machine.write_bus(0xFF46, 0x12);
    for _ in 0..4 {
        machine.step_t_cycle();
    }

    assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());
    assert_eq!(machine.read_bus(0xC000), 0x34);
    assert_eq!(machine.read_bus(0x8000), 0x78);
    assert_eq!(machine.read_bus(0xFF46), 0x12);
    assert_eq!(machine.dma().source_page_latch(), 0x12);
    assert_eq!(machine.read_bus(0xFF80), 0x12);

    machine.step_t_cycle();

    assert_eq!(
        machine.dma().bus_state(),
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );
    assert_eq!(machine.read_bus(0xC000), 0xFF);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFF46), 0x12);

    machine.write_bus(0xFF46, 0x34);
    assert_eq!(machine.dma().source_page_latch(), 0x34);

    machine.write_bus(0xC000, 0x99);
    machine.write_bus(0x8000, 0xBC);
    machine.write_bus(0xFF80, 0x56);

    assert_eq!(machine.read_bus(0xC000), 0xFF);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFF80), 0x56);

    for _ in 0..649 {
        machine.step_t_cycle();
    }

    assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());
    assert_eq!(machine.read_bus(0xC000), 0x34);
}

#[test]
fn video_bus_dma_leaves_wram_and_echo_accessible_while_blocking_vram_and_oam() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xC000, 0x34);
    machine.write_bus(0xFDFF, 0x7A);
    machine.write_bus(0x8000, 0x56);
    machine.write_bus(0xFE00, 0x89);
    machine.write_bus(0xFF80, 0x12);
    machine.write_bus(0xFF46, 0x80);
    for _ in 0..5 {
        machine.step_t_cycle();
    }

    assert_eq!(
        machine.dma().bus_state(),
        DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );
    assert_eq!(machine.read_bus(0xC000), 0x34);
    assert_eq!(machine.read_bus(0xFDFF), 0x7A);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);
    assert_eq!(machine.read_bus(0xFF80), 0x12);
    assert_eq!(machine.read_bus(0xFF46), 0x80);

    machine.write_bus(0xC000, 0x91);
    machine.write_bus(0xFDFF, 0x42);
    machine.write_bus(0x8000, 0xAB);
    machine.write_bus(0xFE00, 0xCD);

    assert_eq!(machine.read_bus(0xC000), 0x91);
    assert_eq!(machine.read_bus(0xFDFF), 0x42);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);
}

#[test]
fn dma_and_bus_traces_show_the_same_cycle_arbitration_constraints() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF46, 0x12);
    machine.step_t_cycle();

    let trace = machine.tracer().sink().render_text();

    assert!(trace.contains(
        "subsystem=dma level=trace message=\"t_cycle=0 phase=autonomous_peripheral_ticks console_model=GameBoy status=Ready transfer_state=Starting transfer_kind=Oam transfer_family=FullBurst block_size=1 advance_condition=EveryTCycle first_byte_delay_t_cycles=8 first_byte_delay_remaining_t_cycles=7 cpu_bus_restriction_delay_t_cycles=5 cpu_bus_restriction_delay_remaining_t_cycles=4 cpu_bus_restriction_active=false elapsed_t_cycles=1 completed_bytes=0 remaining_bytes=160 completed_blocks=0 remaining_blocks=160"
    ));
    assert!(trace.contains(
        "subsystem=bus level=trace message=\"t_cycle=0 phase=bus_arbitration console_model=GameBoy status=Ready boot_low_window_mapped=false boot_cgb_upper_window_mapped=false ppu_lcd_enabled=true ppu_mode=OamScan dma_cpu_access_policy=Unrestricted dma_active_region=None dma_cpu_conflict_source_address=None\""
    ));
}
