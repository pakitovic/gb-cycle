mod common;

use gb_core::debugger::{
    TRACE_FORMAT_VERSION, TraceLevel, TraceSubsystem, Tracer, supported_trace_subsystems,
};

#[test]
fn in_memory_tracer_matches_the_minimal_text_fixture() {
    let mut tracer = Tracer::in_memory();
    tracer.emit(TraceSubsystem::Core, TraceLevel::Info, "trace initialized");
    tracer.emit(
        TraceSubsystem::Scheduler,
        TraceLevel::Debug,
        "scheduler hook reserved",
    );
    tracer.emit(TraceSubsystem::Cpu, TraceLevel::Trace, "cpu hook reserved");

    let fixture_path = common::trace_fixtures_dir().join("minimal_trace.txt");
    let expected = common::read_text_fixture(&fixture_path).expect("fixture should be readable");

    assert_eq!(tracer.sink().render_text(), expected);
}

#[test]
fn debugger_snapshot_exposes_the_last_recorded_event() {
    let mut tracer = Tracer::in_memory();
    tracer.emit(
        TraceSubsystem::Debugger,
        TraceLevel::Warn,
        "watchpoints pending",
    );

    let snapshot = tracer.snapshot();

    assert_eq!(snapshot.trace_format_version, TRACE_FORMAT_VERSION);
    assert_eq!(snapshot.next_sequence, 1);
    assert_eq!(snapshot.buffered_event_count, 1);

    let last_event = snapshot
        .last_event
        .expect("snapshot should include last event");
    assert_eq!(last_event.sequence(), 0);
    assert_eq!(last_event.subsystem(), TraceSubsystem::Debugger);
    assert_eq!(last_event.level(), TraceLevel::Warn);
    assert_eq!(last_event.message(), "watchpoints pending");
}

#[test]
fn supported_trace_subsystems_include_scheduler_and_debugger_hooks() {
    let subsystems = supported_trace_subsystems();

    assert!(subsystems.contains(&TraceSubsystem::Scheduler));
    assert!(subsystems.contains(&TraceSubsystem::Debugger));
    assert!(subsystems.contains(&TraceSubsystem::Cpu));
    assert!(subsystems.contains(&TraceSubsystem::Bus));
    assert!(subsystems.contains(&TraceSubsystem::Ppu));
    assert!(subsystems.contains(&TraceSubsystem::Dma));
    assert!(subsystems.contains(&TraceSubsystem::Timer));
    assert!(subsystems.contains(&TraceSubsystem::Cartridge));
    assert!(subsystems.contains(&TraceSubsystem::Boot));
    assert!(subsystems.contains(&TraceSubsystem::Interrupts));
}
