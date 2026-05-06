use super::*;

#[test]
fn tracer_assigns_monotonic_sequence_numbers() {
    let mut tracer = Tracer::in_memory();

    let first = tracer.emit(TraceSubsystem::Core, TraceLevel::Info, "trace ready");
    let second = tracer.emit(
        TraceSubsystem::Scheduler,
        TraceLevel::Debug,
        "hook reserved",
    );

    assert_eq!(first, 0);
    assert_eq!(second, 1);
    assert_eq!(tracer.next_sequence(), 2);
}

#[test]
fn snapshot_reports_buffered_state() {
    let mut tracer = Tracer::in_memory();
    tracer.emit(TraceSubsystem::Core, TraceLevel::Info, "trace ready");

    let snapshot = tracer.snapshot();

    assert_eq!(snapshot.trace_format_version, TRACE_FORMAT_VERSION);
    assert_eq!(snapshot.next_sequence, 1);
    assert_eq!(snapshot.buffered_event_count, 1);
    assert_eq!(
        snapshot.last_event.as_ref().map(TraceEvent::message),
        Some("trace ready")
    );
}

#[test]
fn trace_buffer_clear_drops_buffered_events() {
    let mut buffer = TraceBuffer::new();
    buffer.push(TraceEvent {
        sequence: 0,
        subsystem: TraceSubsystem::Core,
        level: TraceLevel::Info,
        message: "trace ready".to_string(),
    });

    buffer.clear();

    assert!(buffer.events().is_empty());
}

#[test]
fn summary_tracer_skips_sequence_and_buffering_events() {
    let mut tracer = Tracer::summary();

    assert!(!tracer.records_events());

    tracer.emit_with(TraceSubsystem::Core, TraceLevel::Info, || "trace ready");

    let snapshot = tracer.snapshot();

    assert_eq!(snapshot.trace_format_version, TRACE_FORMAT_VERSION);
    assert_eq!(snapshot.next_sequence, 0);
    assert_eq!(snapshot.buffered_event_count, 0);
    assert_eq!(snapshot.last_event, None);
}

#[test]
fn summary_tracer_skips_lazy_message_construction() {
    use std::cell::Cell;

    let mut tracer = Tracer::summary();
    let built = Cell::new(false);

    tracer.emit_with(TraceSubsystem::Core, TraceLevel::Info, || {
        built.set(true);
        "trace ready"
    });

    assert!(!built.get());
}

#[test]
fn in_memory_tracer_reports_that_it_records_events() {
    let tracer = Tracer::in_memory();

    assert!(tracer.records_events());
}

#[test]
fn debug_control_matches_program_counter_breakpoints() {
    let mut debug_control = DebugControl::new();
    let breakpoint_id = debug_control.add_breakpoint(BreakpointCondition::ProgramCounter(0x0150));

    assert_eq!(
        debug_control.matching_breakpoints(BreakpointProbe::at_program_counter(0x0150)),
        vec![breakpoint_id]
    );
    assert!(
        debug_control
            .matching_breakpoints(BreakpointProbe::at_program_counter(0x0151))
            .is_empty()
    );

    assert!(debug_control.set_breakpoint_enabled(breakpoint_id, false));
    assert!(
        debug_control
            .matching_breakpoints(BreakpointProbe::at_program_counter(0x0150))
            .is_empty()
    );
}

#[test]
fn debug_control_matches_memory_mmio_and_cartridge_watchpoints() {
    let mut debug_control = DebugControl::new();
    let memory_watchpoint = debug_control.add_watchpoint(
        WatchpointTarget::MemoryAddress(0xC123),
        WatchpointCondition::ReadWrite,
    );
    let mmio_watchpoint = debug_control.add_watchpoint(
        WatchpointTarget::MmioRegister(0xFF46),
        WatchpointCondition::Write,
    );
    let cartridge_watchpoint = debug_control.add_watchpoint(
        WatchpointTarget::Cartridge(CartridgeWatchTarget::RomBank),
        WatchpointCondition::Change,
    );

    assert_eq!(
        debug_control.matching_watchpoints(WatchpointProbe::new(
            WatchpointTarget::MemoryAddress(0xC123),
            WatchpointObservation::Read,
        )),
        vec![memory_watchpoint]
    );
    assert_eq!(
        debug_control.matching_watchpoints(WatchpointProbe::new(
            WatchpointTarget::MmioRegister(0xFF46),
            WatchpointObservation::Write,
        )),
        vec![mmio_watchpoint]
    );
    assert_eq!(
        debug_control.matching_watchpoints(WatchpointProbe::new(
            WatchpointTarget::Cartridge(CartridgeWatchTarget::RomBank),
            WatchpointObservation::Change,
        )),
        vec![cartridge_watchpoint]
    );

    assert!(debug_control.set_watchpoint_enabled(mmio_watchpoint, false));
    assert!(
        debug_control
            .matching_watchpoints(WatchpointProbe::new(
                WatchpointTarget::MmioRegister(0xFF46),
                WatchpointObservation::Write,
            ))
            .is_empty()
    );
}

#[test]
fn debug_control_snapshot_reports_registered_counts() {
    let mut debug_control = DebugControl::new();
    let breakpoint_id = debug_control.add_breakpoint(BreakpointCondition::ProgramCounter(0x0100));
    let watchpoint_id = debug_control.add_watchpoint(
        WatchpointTarget::Cartridge(CartridgeWatchTarget::MapperState),
        WatchpointCondition::Change,
    );

    assert!(debug_control.set_breakpoint_enabled(breakpoint_id, false));
    assert!(debug_control.set_watchpoint_enabled(watchpoint_id, false));

    let snapshot = debug_control.snapshot();

    assert_eq!(snapshot.next_breakpoint_id, 1);
    assert_eq!(snapshot.next_watchpoint_id, 1);
    assert_eq!(snapshot.breakpoint_count, 1);
    assert_eq!(snapshot.enabled_breakpoint_count, 0);
    assert_eq!(snapshot.watchpoint_count, 1);
    assert_eq!(snapshot.enabled_watchpoint_count, 0);
}
