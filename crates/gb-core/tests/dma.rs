use gb_core::{
    Bus, BusArbitrationState, BusRequester, ConsoleModel, DmaAdvanceCondition, DmaBusState,
    DmaCpuAccessPolicy, DmaCpuImpactPolicy, DmaMemoryRegionImpact, DmaTransferFamily,
    DmaTransferKind, DmaTransferLifecycle, DmaTransferState, Machine, MachineConfig, StartupMode,
};

fn read_cartridgeless_bus_harness(bus: &mut Bus, address: u16) -> u8 {
    let state = BusArbitrationState::default();
    bus.read_partial_harness_with_cartridge(address, BusRequester::Cpu, &state, None)
}

fn seed_dma_source_page(machine: &mut Machine, source_page: u8, seed: u8) {
    let source_start = (source_page as u16) << 8;

    seed_dma_source_range(machine, source_start, seed);
}

fn seed_dma_source_range(machine: &mut Machine, source_start: u16, seed: u8) {
    for byte_index in 0..160u16 {
        machine.write_bus(source_start + byte_index, dma_source_byte(seed, byte_index));
    }
}

fn dma_source_byte(seed: u8, byte_index: u16) -> u8 {
    seed.wrapping_mul(17)
        .wrapping_add(byte_index as u8)
        .rotate_left(1)
}

#[path = "dma/dma_base.rs"]
mod dma_base;
#[path = "dma/dma_hram_blocking.rs"]
mod dma_hram_blocking;
#[path = "dma/dma_oam_timing_conflicts.rs"]
mod dma_oam_timing_conflicts;
