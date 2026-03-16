use crate::boot::BootController;
use crate::bus::Bus;
use crate::cartridge::CartridgeSlot;
use crate::cpu::CpuCore;
use crate::debugger::{
    DebugControl, MachineSnapshot, TraceBuffer, TraceLevel, TraceSink, TraceSubsystem, Tracer,
};
use crate::dma::DmaController;
use crate::model::MachineConfig;
use crate::ppu::Ppu;
use crate::scheduler::{CycleContext, GlobalScheduler, SchedulerPhase, TCycle};
use crate::timer::Timer;

#[derive(Debug, Clone)]
pub struct Machine<S = TraceBuffer> {
    config: MachineConfig,
    scheduler: GlobalScheduler,
    tracer: Tracer<S>,
    debug_controls: DebugControl,
    cpu: CpuCore,
    bus: Bus,
    ppu: Ppu,
    dma: DmaController,
    timer: Timer,
    boot: BootController,
    cartridge: CartridgeSlot,
}

#[derive(Debug, Clone)]
pub struct MachineParts<S = TraceBuffer> {
    pub config: MachineConfig,
    pub scheduler: GlobalScheduler,
    pub tracer: Tracer<S>,
    pub debug_controls: DebugControl,
    pub cpu: CpuCore,
    pub bus: Bus,
    pub ppu: Ppu,
    pub dma: DmaController,
    pub timer: Timer,
    pub boot: BootController,
    pub cartridge: CartridgeSlot,
}

impl Machine<TraceBuffer> {
    pub fn new(config: MachineConfig) -> Self {
        Self::with_tracer(config, Tracer::in_memory())
    }

    pub fn snapshot(&self) -> MachineSnapshot {
        MachineSnapshot {
            config: self.config.clone(),
            scheduler: self.scheduler.snapshot(),
            trace: self.tracer.snapshot(),
            debug_controls: self.debug_controls.snapshot(),
            cpu: self.cpu.snapshot(),
            bus: self.bus.snapshot(),
            ppu: self.ppu.snapshot(),
            dma: self.dma.snapshot(),
            timer: self.timer.snapshot(),
            boot: self.boot.snapshot(),
            cartridge: self.cartridge.snapshot(),
        }
    }
}

impl<S: TraceSink> Machine<S> {
    pub fn with_tracer(config: MachineConfig, tracer: Tracer<S>) -> Self {
        let console_model = config.console_model;
        let startup_mode = config.startup_mode;

        Self {
            config,
            scheduler: GlobalScheduler::new(),
            tracer,
            debug_controls: DebugControl::new(),
            cpu: CpuCore::new(console_model),
            bus: Bus::new(console_model),
            ppu: Ppu::new(console_model),
            dma: DmaController::new(console_model),
            timer: Timer::new(console_model),
            boot: BootController::new(console_model, startup_mode),
            cartridge: CartridgeSlot::empty(),
        }
    }

    pub fn config(&self) -> &MachineConfig {
        &self.config
    }

    pub fn scheduler(&self) -> &GlobalScheduler {
        &self.scheduler
    }

    pub fn tracer(&self) -> &Tracer<S> {
        &self.tracer
    }

    pub fn tracer_mut(&mut self) -> &mut Tracer<S> {
        &mut self.tracer
    }

    pub fn debug_controls(&self) -> &DebugControl {
        &self.debug_controls
    }

    pub fn debug_controls_mut(&mut self) -> &mut DebugControl {
        &mut self.debug_controls
    }

    pub fn cpu(&self) -> &CpuCore {
        &self.cpu
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    pub fn dma(&self) -> &DmaController {
        &self.dma
    }

    pub fn timer(&self) -> &Timer {
        &self.timer
    }

    pub fn boot(&self) -> &BootController {
        &self.boot
    }

    pub fn cartridge(&self) -> &CartridgeSlot {
        &self.cartridge
    }

    pub fn next_t_cycle(&self) -> TCycle {
        self.scheduler.next_t_cycle()
    }

    pub fn step_t_cycle(&mut self) -> CycleContext {
        let cpu = &self.cpu;
        let bus = &self.bus;
        let ppu = &self.ppu;
        let dma = &self.dma;
        let timer = &self.timer;
        let boot = &self.boot;
        let cartridge = &self.cartridge;

        self.scheduler
            .step_with_trace(&mut self.tracer, |context, tracer| match context.phase() {
                SchedulerPhase::DerivedEdgeResolution => {
                    tracer.emit(
                        TraceSubsystem::Timer,
                        TraceLevel::Trace,
                        timer.scheduler_trace_message(context),
                    );
                }
                SchedulerPhase::AutonomousPeripheralTicks => {
                    tracer.emit(
                        TraceSubsystem::Dma,
                        TraceLevel::Trace,
                        dma.scheduler_trace_message(context),
                    );
                    tracer.emit(
                        TraceSubsystem::Ppu,
                        TraceLevel::Trace,
                        ppu.scheduler_trace_message(context),
                    );
                }
                SchedulerPhase::BusArbitration => {
                    tracer.emit(
                        TraceSubsystem::Bus,
                        TraceLevel::Trace,
                        bus.scheduler_trace_message(context),
                    );
                    tracer.emit(
                        TraceSubsystem::Cartridge,
                        TraceLevel::Trace,
                        cartridge.scheduler_trace_message(context),
                    );
                }
                SchedulerPhase::CpuMicroOperation => {
                    tracer.emit(
                        TraceSubsystem::Cpu,
                        TraceLevel::Trace,
                        cpu.scheduler_trace_message(context),
                    );
                }
                SchedulerPhase::MmioSideEffectCommit => {
                    tracer.emit(
                        TraceSubsystem::Boot,
                        TraceLevel::Trace,
                        boot.scheduler_trace_message(context),
                    );
                }
                _ => {}
            })
    }

    pub fn into_parts(self) -> MachineParts<S> {
        MachineParts {
            config: self.config,
            scheduler: self.scheduler,
            tracer: self.tracer,
            debug_controls: self.debug_controls,
            cpu: self.cpu,
            bus: self.bus,
            ppu: self.ppu,
            dma: self.dma,
            timer: self.timer,
            boot: self.boot,
            cartridge: self.cartridge,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ConsoleModel, ExecutionMode, StartupMode};

    #[test]
    fn machine_new_starts_on_the_first_t_cycle() {
        let machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );

        assert_eq!(machine.next_t_cycle(), TCycle::ZERO);
        assert_eq!(machine.config().console_model, ConsoleModel::Dmg);
        assert_eq!(machine.cpu().console_model(), ConsoleModel::Dmg);
        assert_eq!(machine.boot().startup_mode(), StartupMode::SkipBoot);
        assert!(machine.cartridge().is_empty());
    }

    #[test]
    fn step_t_cycle_advances_exactly_one_cycle_per_call() {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Mgb).with_execution_mode(ExecutionMode::Permissive),
        );

        let first = machine.step_t_cycle();
        let second = machine.step_t_cycle();

        assert_eq!(first.t_cycle(), TCycle::new(0));
        assert_eq!(second.t_cycle(), TCycle::new(1));
        assert_eq!(machine.next_t_cycle(), TCycle::new(2));
    }

    #[test]
    fn machine_parts_keep_stubbed_subsystem_boundaries_explicit() {
        let machine = Machine::new(
            MachineConfig::new(ConsoleModel::Mgb).with_startup_mode(StartupMode::RealBoot),
        );

        let parts = machine.into_parts();

        assert!(parts.debug_controls.breakpoints().is_empty());
        assert!(parts.debug_controls.watchpoints().is_empty());
        assert_eq!(parts.cpu.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.bus.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.ppu.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.dma.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.timer.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.boot.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.boot.startup_mode(), StartupMode::RealBoot);
        assert!(parts.cartridge.is_empty());
    }

    #[test]
    fn machine_snapshot_exposes_scheduler_trace_and_stubbed_subsystems() {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );

        machine.step_t_cycle();
        machine.step_t_cycle();

        let snapshot = machine.snapshot();

        assert_eq!(snapshot.config.console_model, ConsoleModel::Dmg);
        assert_eq!(snapshot.scheduler.next_t_cycle, TCycle::new(2));
        assert_eq!(snapshot.trace.buffered_event_count, 32);
        assert_eq!(snapshot.debug_controls.breakpoint_count, 0);
        assert_eq!(snapshot.debug_controls.watchpoint_count, 0);
        assert_eq!(snapshot.cpu.console_model, ConsoleModel::Dmg);
        assert!(matches!(
            snapshot.cartridge.state,
            crate::CartridgeSlotState::Empty
        ));
    }
}
