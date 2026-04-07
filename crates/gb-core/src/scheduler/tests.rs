use super::*;

#[test]
fn scheduler_phases_follow_the_documented_order() {
    assert_eq!(
        SchedulerPhase::all(),
        &[
            SchedulerPhase::ExternalEventIngress,
            SchedulerPhase::MasterClockTick,
            SchedulerPhase::DerivedEdgeResolution,
            SchedulerPhase::AutonomousPeripheralTicks,
            SchedulerPhase::BusArbitration,
            SchedulerPhase::CpuMicroOperation,
            SchedulerPhase::MmioSideEffectCommit,
            SchedulerPhase::InterruptAggregation,
            SchedulerPhase::CpuWakeInterruptEvaluation,
        ]
    );
}

#[test]
fn cycle_context_reset_clears_cycle_local_state() {
    let mut context = CycleContext::for_cycle(TCycle::new(2));
    context.enter_phase(SchedulerPhase::CpuMicroOperation);
    context.push_external_event(ExternalEvent::HostInputChanged);
    context.push_derived_edge(DerivedEdge::DividerTick);
    context.set_bus_owner(BusOwner::Cpu);
    context.queue_side_effect(SchedulerSideEffect::CommitMmioWrite);
    context.queue_interrupt_request(InterruptSource::Timer);

    context.reset_for_cycle(TCycle::new(3));

    assert_eq!(context.t_cycle(), TCycle::new(3));
    assert_eq!(context.phase(), SchedulerPhase::ExternalEventIngress);
    assert!(context.external_events().is_empty());
    assert!(context.derived_edges().is_empty());
    assert_eq!(context.bus_owner(), None);
    assert!(context.queued_side_effects().is_empty());
    assert!(context.interrupt_requests().is_empty());
}

#[test]
fn scheduler_value_helpers_cover_display_reset_and_context_management() {
    let mut context = CycleContext::for_cycle(TCycle::new(2));
    context.set_bus_owner(BusOwner::Cpu);
    context.clear_bus_owner();
    assert_eq!(context.bus_owner(), None);

    assert_eq!(TCycle::new(7).to_string(), "7t");

    let mut scheduler = GlobalScheduler::new();
    scheduler.step(|_| {});
    scheduler.step(|_| {});
    assert_eq!(scheduler.prepare_cycle_context().t_cycle(), TCycle::new(2));
    assert_eq!(scheduler.snapshot().next_t_cycle, TCycle::new(2));

    scheduler.reset();

    assert_eq!(scheduler.next_t_cycle(), TCycle::ZERO);
}

#[test]
fn step_advances_the_global_t_cycle_counter() {
    let mut scheduler = GlobalScheduler::new();

    let first_cycle = scheduler.step(|_| {});
    let second_cycle = scheduler.step(|_| {});

    assert_eq!(first_cycle.t_cycle(), TCycle::new(0));
    assert_eq!(second_cycle.t_cycle(), TCycle::new(1));
    assert_eq!(scheduler.next_t_cycle(), TCycle::new(2));
}
