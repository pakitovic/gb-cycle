use gb_core::debugger::{TraceBuffer, TraceLevel, TraceSubsystem, Tracer};
use gb_core::{
    BreakpointCondition, BreakpointProbe, CartridgeWatchTarget, ConsoleModel, Machine,
    MachineConfig, WatchpointCondition, WatchpointObservation, WatchpointProbe, WatchpointTarget,
};

#[test]
fn public_debug_controls_register_stable_breakpoint_ids() {
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::Dmg));

    let first = machine
        .debug_controls_mut()
        .add_breakpoint(BreakpointCondition::ProgramCounter(0x0100));
    let second = machine
        .debug_controls_mut()
        .add_breakpoint(BreakpointCondition::ProgramCounter(0x0150));

    assert_eq!(first.get(), 0);
    assert_eq!(second.get(), 1);
    assert_eq!(
        machine
            .debug_controls()
            .matching_breakpoints(BreakpointProbe::at_program_counter(0x0150)),
        vec![second]
    );
}

#[test]
fn public_debug_controls_cover_memory_mmio_and_cartridge_watch_targets() {
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::Dmg));

    let memory = machine.debug_controls_mut().add_watchpoint(
        WatchpointTarget::MemoryAddress(0xC100),
        WatchpointCondition::Read,
    );
    let mmio = machine.debug_controls_mut().add_watchpoint(
        WatchpointTarget::MmioRegister(0xFF44),
        WatchpointCondition::Write,
    );
    let cartridge = machine.debug_controls_mut().add_watchpoint(
        WatchpointTarget::Cartridge(CartridgeWatchTarget::RomBank),
        WatchpointCondition::Change,
    );

    assert_eq!(
        machine
            .debug_controls()
            .matching_watchpoints(WatchpointProbe::new(
                WatchpointTarget::MemoryAddress(0xC100),
                WatchpointObservation::Read,
            )),
        vec![memory]
    );
    assert_eq!(
        machine
            .debug_controls()
            .matching_watchpoints(WatchpointProbe::new(
                WatchpointTarget::MmioRegister(0xFF44),
                WatchpointObservation::Write,
            )),
        vec![mmio]
    );
    assert_eq!(
        machine
            .debug_controls()
            .matching_watchpoints(WatchpointProbe::new(
                WatchpointTarget::Cartridge(CartridgeWatchTarget::RomBank),
                WatchpointObservation::Change,
            )),
        vec![cartridge]
    );
}

#[test]
fn machine_snapshot_reports_debug_control_counts() {
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::Mgb));

    let breakpoint_id = machine
        .debug_controls_mut()
        .add_breakpoint(BreakpointCondition::ProgramCounter(0x0100));
    machine.debug_controls_mut().add_watchpoint(
        WatchpointTarget::Cartridge(CartridgeWatchTarget::MapperState),
        WatchpointCondition::Change,
    );
    assert!(
        machine
            .debug_controls_mut()
            .set_breakpoint_enabled(breakpoint_id, false)
    );

    let snapshot = machine.snapshot();

    assert_eq!(snapshot.debug_controls.breakpoint_count, 1);
    assert_eq!(snapshot.debug_controls.enabled_breakpoint_count, 0);
    assert_eq!(snapshot.debug_controls.watchpoint_count, 1);
    assert_eq!(snapshot.debug_controls.enabled_watchpoint_count, 1);
}

#[test]
fn debug_controls_allow_removal_and_reject_unknown_ids() {
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::Dmg));

    let breakpoint_id = machine
        .debug_controls_mut()
        .add_breakpoint(BreakpointCondition::ProgramCounter(0x0150));
    let watchpoint_id = machine.debug_controls_mut().add_watchpoint(
        WatchpointTarget::MemoryAddress(0xC000),
        WatchpointCondition::ReadWrite,
    );

    let removed_breakpoint = machine
        .debug_controls_mut()
        .remove_breakpoint(breakpoint_id);
    let removed_watchpoint = machine
        .debug_controls_mut()
        .remove_watchpoint(watchpoint_id);

    assert!(removed_breakpoint.is_some());
    assert!(removed_watchpoint.is_some());
    assert!(machine.debug_controls().breakpoints().is_empty());
    assert!(machine.debug_controls().watchpoints().is_empty());
    assert!(
        !machine
            .debug_controls_mut()
            .set_breakpoint_enabled(breakpoint_id, true)
    );
    assert!(
        !machine
            .debug_controls_mut()
            .set_watchpoint_enabled(watchpoint_id, true)
    );
    assert!(
        machine
            .debug_controls_mut()
            .remove_breakpoint(breakpoint_id)
            .is_none()
    );
    assert!(
        machine
            .debug_controls_mut()
            .remove_watchpoint(watchpoint_id)
            .is_none()
    );
}

#[test]
fn watchpoint_conditions_stay_distinct_across_read_write_and_change() {
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::Dmg));

    let read_watchpoint = machine.debug_controls_mut().add_watchpoint(
        WatchpointTarget::MmioRegister(0xFF44),
        WatchpointCondition::Read,
    );
    let write_watchpoint = machine.debug_controls_mut().add_watchpoint(
        WatchpointTarget::MmioRegister(0xFF44),
        WatchpointCondition::Write,
    );
    let change_watchpoint = machine.debug_controls_mut().add_watchpoint(
        WatchpointTarget::MmioRegister(0xFF44),
        WatchpointCondition::Change,
    );

    assert_eq!(
        machine
            .debug_controls()
            .matching_watchpoints(WatchpointProbe::new(
                WatchpointTarget::MmioRegister(0xFF44),
                WatchpointObservation::Read,
            )),
        vec![read_watchpoint]
    );
    assert_eq!(
        machine
            .debug_controls()
            .matching_watchpoints(WatchpointProbe::new(
                WatchpointTarget::MmioRegister(0xFF44),
                WatchpointObservation::Write,
            )),
        vec![write_watchpoint]
    );
    assert_eq!(
        machine
            .debug_controls()
            .matching_watchpoints(WatchpointProbe::new(
                WatchpointTarget::MmioRegister(0xFF44),
                WatchpointObservation::Change,
            )),
        vec![change_watchpoint]
    );
}

#[test]
fn tracer_and_trace_buffer_support_empty_and_owned_buffer_access() {
    let tracer = Tracer::in_memory();

    assert_eq!(tracer.sink().events().len(), 0);
    assert_eq!(tracer.sink().render_text(), "");
    assert_eq!(tracer.snapshot().buffered_event_count, 0);
    assert!(tracer.snapshot().last_event.is_none());

    let mut tracer = Tracer::new(TraceBuffer::new());
    tracer.emit(TraceSubsystem::Core, TraceLevel::Info, "trace ready");
    let sink = tracer.into_sink();

    assert_eq!(sink.events().len(), 1);
    assert_eq!(
        sink.render_text(),
        "seq=0 subsystem=core level=info message=\"trace ready\"\n"
    );
}
