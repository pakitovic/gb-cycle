use std::fmt;

use crate::debugger::{TraceLevel, TraceSink, TraceSubsystem, Tracer};

pub const SCHEDULER_PHASE_COUNT: usize = 9;
const MAX_EXTERNAL_EVENTS_PER_CYCLE: usize = 16;
const MAX_DERIVED_EDGES_PER_CYCLE: usize = 16;
const MAX_SIDE_EFFECTS_PER_CYCLE: usize = 16;
const MAX_INTERRUPT_REQUESTS_PER_CYCLE: usize = 16;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct TCycle(u64);

impl TCycle {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Self {
        Self(
            self.0
                .checked_add(1)
                .expect("T-cycle counter overflowed u64"),
        )
    }
}

impl fmt::Display for TCycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}t", self.0)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SchedulerPhase {
    #[default]
    ExternalEventIngress,
    MasterClockTick,
    DerivedEdgeResolution,
    AutonomousPeripheralTicks,
    BusArbitration,
    CpuMicroOperation,
    MmioSideEffectCommit,
    InterruptAggregation,
    CpuWakeInterruptEvaluation,
}

impl SchedulerPhase {
    pub const ORDER: [Self; SCHEDULER_PHASE_COUNT] = [
        Self::ExternalEventIngress,
        Self::MasterClockTick,
        Self::DerivedEdgeResolution,
        Self::AutonomousPeripheralTicks,
        Self::BusArbitration,
        Self::CpuMicroOperation,
        Self::MmioSideEffectCommit,
        Self::InterruptAggregation,
        Self::CpuWakeInterruptEvaluation,
    ];

    pub fn all() -> &'static [Self] {
        &Self::ORDER
    }
}

impl fmt::Display for SchedulerPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::ExternalEventIngress => "external_event_ingress",
            Self::MasterClockTick => "master_clock_tick",
            Self::DerivedEdgeResolution => "derived_edge_resolution",
            Self::AutonomousPeripheralTicks => "autonomous_peripheral_ticks",
            Self::BusArbitration => "bus_arbitration",
            Self::CpuMicroOperation => "cpu_micro_operation",
            Self::MmioSideEffectCommit => "mmio_side_effect_commit",
            Self::InterruptAggregation => "interrupt_aggregation",
            Self::CpuWakeInterruptEvaluation => "cpu_wake_interrupt_evaluation",
        };

        f.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ExternalEvent {
    HostInputChanged,
    ExternalSerialClock,
    DebugCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DerivedEdge {
    DividerTick,
    TimerInputFallingEdge,
    ApuFrameSequencerEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BusOwner {
    Cpu,
    Dma,
    Ppu,
    Apu,
    Serial,
    Cartridge,
    Boot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SchedulerSideEffect {
    CommitMmioWrite,
    BootRomUnmap,
    StartOamDma,
    LcdPowerTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum InterruptSource {
    VBlank,
    LcdStat,
    Timer,
    Serial,
    Joypad,
}

#[derive(Debug, Clone)]
struct CycleRecordBuffer<T: Copy, const N: usize> {
    items: [T; N],
    len: usize,
}

impl<T: Copy, const N: usize> CycleRecordBuffer<T, N> {
    const fn new(filler: T) -> Self {
        Self {
            items: [filler; N],
            len: 0,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn push(&mut self, item: T) {
        assert!(
            self.len < N,
            "cycle record buffer capacity exceeded (capacity={N})"
        );
        self.items[self.len] = item;
        self.len += 1;
    }

    fn as_slice(&self) -> &[T] {
        &self.items[..self.len]
    }
}

impl<T: Copy + PartialEq, const N: usize> PartialEq for CycleRecordBuffer<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Copy + Eq, const N: usize> Eq for CycleRecordBuffer<T, N> {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleContext {
    t_cycle: TCycle,
    phase: SchedulerPhase,
    external_events: CycleRecordBuffer<ExternalEvent, MAX_EXTERNAL_EVENTS_PER_CYCLE>,
    derived_edges: CycleRecordBuffer<DerivedEdge, MAX_DERIVED_EDGES_PER_CYCLE>,
    bus_owner: Option<BusOwner>,
    queued_side_effects: CycleRecordBuffer<SchedulerSideEffect, MAX_SIDE_EFFECTS_PER_CYCLE>,
    interrupt_requests: CycleRecordBuffer<InterruptSource, MAX_INTERRUPT_REQUESTS_PER_CYCLE>,
}

impl CycleContext {
    pub fn for_cycle(t_cycle: TCycle) -> Self {
        Self {
            t_cycle,
            phase: SchedulerPhase::default(),
            external_events: CycleRecordBuffer::new(ExternalEvent::HostInputChanged),
            derived_edges: CycleRecordBuffer::new(DerivedEdge::DividerTick),
            bus_owner: None,
            queued_side_effects: CycleRecordBuffer::new(SchedulerSideEffect::CommitMmioWrite),
            interrupt_requests: CycleRecordBuffer::new(InterruptSource::VBlank),
        }
    }

    pub fn reset_for_cycle(&mut self, t_cycle: TCycle) {
        self.t_cycle = t_cycle;
        self.phase = SchedulerPhase::default();
        self.external_events.clear();
        self.derived_edges.clear();
        self.bus_owner = None;
        self.queued_side_effects.clear();
        self.interrupt_requests.clear();
    }

    pub fn t_cycle(&self) -> TCycle {
        self.t_cycle
    }

    pub fn phase(&self) -> SchedulerPhase {
        self.phase
    }

    pub fn external_events(&self) -> &[ExternalEvent] {
        self.external_events.as_slice()
    }

    pub fn derived_edges(&self) -> &[DerivedEdge] {
        self.derived_edges.as_slice()
    }

    pub fn bus_owner(&self) -> Option<BusOwner> {
        self.bus_owner
    }

    pub fn queued_side_effects(&self) -> &[SchedulerSideEffect] {
        self.queued_side_effects.as_slice()
    }

    pub fn interrupt_requests(&self) -> &[InterruptSource] {
        self.interrupt_requests.as_slice()
    }

    pub fn enter_phase(&mut self, phase: SchedulerPhase) {
        self.phase = phase;
    }

    pub fn push_external_event(&mut self, event: ExternalEvent) {
        self.external_events.push(event);
    }

    pub fn push_derived_edge(&mut self, edge: DerivedEdge) {
        self.derived_edges.push(edge);
    }

    pub fn set_bus_owner(&mut self, owner: BusOwner) {
        self.bus_owner = Some(owner);
    }

    pub fn clear_bus_owner(&mut self) {
        self.bus_owner = None;
    }

    pub fn queue_side_effect(&mut self, side_effect: SchedulerSideEffect) {
        self.queued_side_effects.push(side_effect);
    }

    pub fn queue_interrupt_request(&mut self, interrupt: InterruptSource) {
        self.interrupt_requests.push(interrupt);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GlobalScheduler {
    next_t_cycle: TCycle,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SchedulerSnapshot {
    pub next_t_cycle: TCycle,
}

impl GlobalScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_t_cycle(&self) -> TCycle {
        self.next_t_cycle
    }

    pub fn prepare_cycle_context(&self) -> CycleContext {
        CycleContext::for_cycle(self.next_t_cycle)
    }

    pub fn snapshot(&self) -> SchedulerSnapshot {
        SchedulerSnapshot {
            next_t_cycle: self.next_t_cycle,
        }
    }

    pub fn reset(&mut self) {
        self.next_t_cycle = TCycle::ZERO;
    }

    pub(crate) fn set_next_t_cycle(&mut self, next_t_cycle: TCycle) {
        self.next_t_cycle = next_t_cycle;
    }

    pub fn step<F>(&mut self, mut visit_phase: F) -> CycleContext
    where
        F: FnMut(&mut CycleContext),
    {
        let mut context = self.prepare_cycle_context();

        for &phase in SchedulerPhase::all() {
            context.enter_phase(phase);
            visit_phase(&mut context);
        }

        self.next_t_cycle = self.next_t_cycle.next();
        context
    }

    pub fn step_with_trace<S, F>(
        &mut self,
        tracer: &mut Tracer<S>,
        mut visit_phase: F,
    ) -> CycleContext
    where
        S: TraceSink,
        F: FnMut(&mut CycleContext, &mut Tracer<S>),
    {
        self.step(|context| {
            tracer.emit_with(TraceSubsystem::Scheduler, TraceLevel::Trace, || {
                scheduler_phase_trace_message(context)
            });
            visit_phase(context, tracer);
        })
    }
}

impl Default for GlobalScheduler {
    fn default() -> Self {
        Self {
            next_t_cycle: TCycle::ZERO,
        }
    }
}

pub fn scheduler_phase_trace_message(context: &CycleContext) -> String {
    format!(
        "t_cycle={} phase={}",
        context.t_cycle().get(),
        context.phase()
    )
}

#[cfg(test)]
mod tests;
