use crate::apu::Apu;
use crate::boot::BootController;
use crate::bus::{Bus, BusArbitrationState, BusIoReadView, BusIoWriteView, BusRequester};
use crate::cartridge::{CartridgeDiagnostic, CartridgeLoadError, CartridgeSlot};
use crate::cpu::{CpuBusOperation, CpuCore};
use crate::debugger::{
    DebugControl, MachineSnapshot, TraceBuffer, TraceLevel, TraceSink, TraceSubsystem, Tracer,
};
use crate::dma::DmaController;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::joypad::JoypadButton;
use crate::model::MachineConfig;
use crate::ppu::Ppu;
use crate::scheduler::{CycleContext, GlobalScheduler, SchedulerPhase, TCycle};
use crate::serial::Serial;
use crate::timer::Timer;

#[derive(Debug, Clone)]
pub struct Machine<S = TraceBuffer> {
    config: MachineConfig,
    scheduler: GlobalScheduler,
    tracer: Tracer<S>,
    debug_controls: DebugControl,
    cpu: CpuCore,
    bus: Bus,
    apu: Apu,
    ppu: Ppu,
    dma: DmaController,
    timer: Timer,
    serial: Serial,
    boot: BootController,
    interrupts: InterruptController,
    joypad: Joypad,
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
    pub apu: Apu,
    pub ppu: Ppu,
    pub dma: DmaController,
    pub timer: Timer,
    pub serial: Serial,
    pub boot: BootController,
    pub interrupts: InterruptController,
    pub joypad: Joypad,
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
            apu: self.apu.snapshot(),
            ppu: self.ppu.snapshot(),
            dma: self.dma.snapshot(),
            timer: self.timer.snapshot(),
            serial: self.serial.snapshot(),
            boot: self.boot.snapshot(),
            interrupts: self.interrupts.snapshot(),
            joypad: self.joypad.snapshot(),
            cartridge: self.cartridge.snapshot(),
        }
    }
}

impl<S: TraceSink> Machine<S> {
    pub fn with_tracer(config: MachineConfig, tracer: Tracer<S>) -> Self {
        let console_model = config.console_model;
        let startup_mode = config.startup_mode;
        let boot_rom_assets = config.boot_rom_assets.clone();

        let mut machine = Self {
            config,
            scheduler: GlobalScheduler::new(),
            tracer,
            debug_controls: DebugControl::new(),
            cpu: CpuCore::new(console_model),
            bus: Bus::new(console_model),
            apu: Apu::new(console_model),
            ppu: Ppu::new(console_model),
            dma: DmaController::new(console_model),
            timer: Timer::new(console_model),
            serial: Serial::new(console_model),
            boot: BootController::new(console_model, startup_mode, boot_rom_assets),
            interrupts: InterruptController::new(console_model),
            joypad: Joypad::new(console_model),
            cartridge: CartridgeSlot::empty(),
        };

        machine.apply_startup_configuration();
        machine
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

    pub fn apu(&self) -> &Apu {
        &self.apu
    }

    pub fn dma(&self) -> &DmaController {
        &self.dma
    }

    pub fn timer(&self) -> &Timer {
        &self.timer
    }

    pub fn serial(&self) -> &Serial {
        &self.serial
    }

    pub fn boot(&self) -> &BootController {
        &self.boot
    }

    pub fn interrupts(&self) -> &InterruptController {
        &self.interrupts
    }

    pub fn joypad(&self) -> &Joypad {
        &self.joypad
    }

    pub fn set_joypad_button_pressed(&mut self, button: JoypadButton, pressed: bool) {
        self.joypad.set_button_pressed(button, pressed);
    }

    pub fn cartridge(&self) -> &CartridgeSlot {
        &self.cartridge
    }

    pub fn read_bus(&mut self, address: u16) -> u8 {
        let state = self.current_bus_arbitration_state();

        self.bus.read_with_context(
            address,
            BusRequester::Cpu,
            &state,
            Some(&self.cartridge),
            crate::bus::BusIoReadView {
                apu: Some(&self.apu),
                timer: Some(&self.timer),
                serial: Some(&self.serial),
                dma: Some(&self.dma),
                boot: Some(&self.boot),
                interrupts: Some(&self.interrupts),
                joypad: Some(&self.joypad),
                ppu: Some(&self.ppu),
            },
        )
    }

    pub fn write_bus(&mut self, address: u16, value: u8) {
        let state = self.current_bus_arbitration_state();

        self.bus.write_with_context(
            address,
            value,
            BusRequester::Cpu,
            &state,
            Some(&mut self.cartridge),
            crate::bus::BusIoWriteView {
                apu: Some(&mut self.apu),
                timer: Some(&mut self.timer),
                serial: Some(&mut self.serial),
                dma: Some(&mut self.dma),
                boot: Some(&mut self.boot),
                interrupts: Some(&mut self.interrupts),
                joypad: Some(&mut self.joypad),
                ppu: Some(&mut self.ppu),
            },
        );
    }

    pub fn load_cartridge(
        &mut self,
        rom_bytes: Vec<u8>,
    ) -> Result<Vec<CartridgeDiagnostic>, CartridgeLoadError> {
        let report = CartridgeSlot::load(rom_bytes, &self.config.compatibility)?;
        let (cartridge, diagnostics) = report.into_parts();
        self.cartridge = cartridge;
        self.apply_startup_configuration();
        Ok(diagnostics)
    }

    pub fn next_t_cycle(&self) -> TCycle {
        self.scheduler.next_t_cycle()
    }

    pub fn step_t_cycle(&mut self) -> CycleContext {
        let cpu = &mut self.cpu;
        let bus = &mut self.bus;
        let apu = &mut self.apu;
        let ppu = &mut self.ppu;
        let dma = &mut self.dma;
        let timer = &mut self.timer;
        let serial = &mut self.serial;
        let boot = &mut self.boot;
        let interrupts = &mut self.interrupts;
        let joypad = &mut self.joypad;
        let cartridge = &mut self.cartridge;

        self.scheduler
            .step_with_trace(&mut self.tracer, |context, tracer| match context.phase() {
                SchedulerPhase::DerivedEdgeResolution => {
                    timer.tick_t_cycle(context);
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
                    let arbitration_state = BusArbitrationState::default()
                        .with_boot_rom(boot.bus_state())
                        .with_ppu(ppu.bus_state());
                    cpu.tick_t_cycle(|operation| match operation {
                        CpuBusOperation::Read { address } => Some(bus.read_with_context(
                            address,
                            BusRequester::Cpu,
                            &arbitration_state,
                            Some(&*cartridge),
                            BusIoReadView {
                                apu: Some(apu),
                                timer: Some(timer),
                                serial: Some(serial),
                                dma: Some(dma),
                                boot: Some(boot),
                                interrupts: Some(interrupts),
                                joypad: Some(joypad),
                                ppu: Some(ppu),
                            },
                        )),
                        CpuBusOperation::Write { address, value } => {
                            bus.write_with_context(
                                address,
                                value,
                                BusRequester::Cpu,
                                &arbitration_state,
                                Some(cartridge),
                                BusIoWriteView {
                                    apu: Some(apu),
                                    timer: Some(timer),
                                    serial: Some(serial),
                                    dma: Some(dma),
                                    boot: Some(boot),
                                    interrupts: Some(interrupts),
                                    joypad: Some(joypad),
                                    ppu: Some(ppu),
                                },
                            );
                            None
                        }
                    });
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
                SchedulerPhase::InterruptAggregation => {
                    for &source in context.interrupt_requests() {
                        interrupts.request(source);
                    }
                    tracer.emit(
                        TraceSubsystem::Interrupts,
                        TraceLevel::Trace,
                        interrupts.scheduler_trace_message(context),
                    );
                }
                SchedulerPhase::CpuWakeInterruptEvaluation => {
                    cpu.evaluate_wake_and_interrupts(interrupts, joypad);
                    tracer.emit(
                        TraceSubsystem::Interrupts,
                        TraceLevel::Trace,
                        interrupts.scheduler_trace_message(context),
                    );
                    tracer.emit(
                        TraceSubsystem::Cpu,
                        TraceLevel::Trace,
                        cpu.scheduler_trace_message(context),
                    );
                }
                _ => {}
            })
    }

    fn current_bus_arbitration_state(&self) -> BusArbitrationState {
        BusArbitrationState::default()
            .with_boot_rom(self.boot.bus_state())
            .with_ppu(self.ppu.bus_state())
    }

    fn apply_startup_configuration(&mut self) {
        if let Some(startup_state) = self.boot.direct_boot_state(Some(&self.cartridge)) {
            self.cpu.apply_startup_state(startup_state.cpu);
            self.apu.apply_startup_state(startup_state.apu);
            self.ppu.apply_startup_state(startup_state.ppu);
            self.timer.apply_startup_state(startup_state.timer);
            self.serial.apply_startup_state(startup_state.serial);
            self.dma.apply_startup_state(startup_state.dma);
            self.interrupts
                .apply_startup_state(startup_state.interrupts);
            self.joypad.apply_startup_state(startup_state.joypad);
            self.bus
                .apply_startup_memory_policy(startup_state.startup_memory_policy);
        }
    }

    pub fn into_parts(self) -> MachineParts<S> {
        MachineParts {
            config: self.config,
            scheduler: self.scheduler,
            tracer: self.tracer,
            debug_controls: self.debug_controls,
            cpu: self.cpu,
            bus: self.bus,
            apu: self.apu,
            ppu: self.ppu,
            dma: self.dma,
            timer: self.timer,
            serial: self.serial,
            boot: self.boot,
            interrupts: self.interrupts,
            joypad: self.joypad,
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
    fn machine_parts_keep_the_current_subsystem_boundaries_explicit() {
        let machine = Machine::new(
            MachineConfig::new(ConsoleModel::Mgb).with_startup_mode(StartupMode::RealBoot),
        );

        let parts = machine.into_parts();

        assert!(parts.debug_controls.breakpoints().is_empty());
        assert!(parts.debug_controls.watchpoints().is_empty());
        assert_eq!(parts.cpu.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.bus.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.apu.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.ppu.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.dma.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.timer.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.serial.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.boot.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.interrupts.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.joypad.console_model(), ConsoleModel::Mgb);
        assert_eq!(parts.boot.startup_mode(), StartupMode::RealBoot);
        assert!(parts.cartridge.is_empty());
    }

    #[test]
    fn machine_snapshot_exposes_scheduler_trace_and_live_phase_1_subsystems() {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );

        machine.step_t_cycle();
        machine.step_t_cycle();

        let snapshot = machine.snapshot();

        assert_eq!(snapshot.config.console_model, ConsoleModel::Dmg);
        assert_eq!(snapshot.scheduler.next_t_cycle, TCycle::new(2));
        assert_eq!(snapshot.trace.buffered_event_count, 38);
        assert_eq!(snapshot.debug_controls.breakpoint_count, 0);
        assert_eq!(snapshot.debug_controls.watchpoint_count, 0);
        assert_eq!(snapshot.cpu.console_model, ConsoleModel::Dmg);
        assert_eq!(snapshot.apu.console_model, ConsoleModel::Dmg);
        assert_eq!(snapshot.serial.console_model, ConsoleModel::Dmg);
        assert_eq!(snapshot.interrupts.console_model, ConsoleModel::Dmg);
        assert_eq!(snapshot.joypad.console_model, ConsoleModel::Dmg);
        assert!(matches!(
            snapshot.cartridge.state,
            crate::CartridgeSlotState::Empty
        ));
    }
}
