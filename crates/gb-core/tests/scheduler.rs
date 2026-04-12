mod common;

use gb_core::debugger::Tracer;
use gb_core::{
    BusOwner, DerivedEdge, ExternalEvent, GlobalScheduler, InterruptSource, SCHEDULER_PHASE_COUNT,
    SchedulerPhase, SchedulerSideEffect, TCycle,
};

#[test]
fn scheduler_steps_a_t_cycle_in_the_documented_phase_order() {
    let mut scheduler = GlobalScheduler::new();
    let mut observed_phases = Vec::new();

    let context = scheduler.step(|context| {
        observed_phases.push(context.phase());
    });

    assert_eq!(context.t_cycle(), TCycle::new(0));
    assert_eq!(observed_phases, SchedulerPhase::all());
    assert_eq!(observed_phases.len(), SCHEDULER_PHASE_COUNT);
}

#[test]
fn cycle_context_is_fresh_for_each_t_cycle() {
    let mut scheduler = GlobalScheduler::new();

    scheduler.step(|context| {
        context.push_external_event(ExternalEvent::HostInputChanged);
        context.push_derived_edge(DerivedEdge::DividerTick);
        context.set_bus_owner(BusOwner::Cpu);
        context.queue_side_effect(SchedulerSideEffect::StartOamDma);
        context.queue_interrupt_request(InterruptSource::Timer);
    });

    scheduler.step(|context| {
        if context.phase() == SchedulerPhase::ExternalEventIngress {
            assert_eq!(context.t_cycle(), TCycle::new(1));
            assert!(context.external_events().is_empty());
            assert!(context.derived_edges().is_empty());
            assert_eq!(context.bus_owner(), None);
            assert!(context.queued_side_effects().is_empty());
            assert!(context.interrupt_requests().is_empty());
        }
    });
}

#[test]
fn scheduler_trace_output_matches_the_phase_order_fixture() {
    let mut scheduler = GlobalScheduler::new();
    let mut tracer = Tracer::in_memory();

    scheduler.step_with_trace(&mut tracer, |_, _| {});

    let fixture_path = common::paths::trace_fixture_path("scheduler_cycle_trace.txt");
    let expected =
        common::fixtures::read_text_fixture(&fixture_path).expect("fixture should be readable");

    assert_eq!(tracer.sink().render_text(), expected);
}
