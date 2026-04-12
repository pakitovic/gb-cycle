use gb_core::{
    BootRomBusState, Bus, BusAccessDisposition, BusAccessKind, BusArbitrationState, BusBlockReason,
    BusRegion, BusRegionOwner, BusRequester, ConsoleModel, CycleContext, DmaBusState,
    DmaMemoryRegionImpact, PpuAccessMode, PpuBusState, SchedulerPhase, TCycle,
    UnusableAreaReadProfile, UnusableAreaWriteProfile,
};

fn read_cartridgeless_bus_harness(bus: &mut Bus, address: u16) -> u8 {
    let state = BusArbitrationState::default();
    bus.read_partial_harness_with_cartridge(address, BusRequester::Cpu, &state, None)
}

fn write_cartridgeless_bus_harness(bus: &mut Bus, address: u16, value: u8) {
    let state = BusArbitrationState::default();
    bus.write_partial_harness_with_cartridge(address, value, BusRequester::Cpu, &state, None);
}

#[path = "bus/bus_arbitration.rs"]
mod bus_arbitration;
#[path = "bus/bus_decode_harness.rs"]
mod bus_decode_harness;
#[path = "bus/bus_unusable_trace.rs"]
mod bus_unusable_trace;
