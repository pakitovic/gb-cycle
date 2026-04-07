use crate::apu::Apu;
use crate::boot::BootController;
use crate::bus::{
    Bus, BusArbitrationState, BusIoReadView, BusIoWriteView, BusMaster, BusRequester,
    IoRegisterOwner,
};
use crate::cartridge::{
    CartridgeDiagnostic, CartridgeLoadError, CartridgePersistentStateError, CartridgeSlot,
    PersistentCartState,
};
use crate::cpu::{
    CpuAddressEvent, CpuAddressEventKind, CpuBusOperation, CpuCore, CpuExecutionState,
};
use crate::debugger::{
    DebugControl, MachineSnapshot, TraceBuffer, TraceLevel, TraceSink, TraceSnapshotProvider,
    TraceSubsystem, TraceSummaryBuffer, Tracer,
};
use crate::dma::DmaController;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::joypad::JoypadButton;
use crate::model::MachineConfig;
use crate::ppu::Ppu;
use crate::scheduler::{
    CycleContext, GlobalScheduler, SchedulerPhase, SchedulerSideEffect, TCycle,
};
use crate::serial::{Serial, SerialPeer};
use crate::timer::Timer;

fn interrupt_source_bit(source: crate::scheduler::InterruptSource) -> u8 {
    match source {
        crate::scheduler::InterruptSource::VBlank => 0x01,
        crate::scheduler::InterruptSource::LcdStat => 0x02,
        crate::scheduler::InterruptSource::Timer => 0x04,
        crate::scheduler::InterruptSource::Serial => 0x08,
        crate::scheduler::InterruptSource::Joypad => 0x10,
    }
}

fn current_cycle_interrupt_read_mask(context: &CycleContext, ppu: &Ppu, joypad: &Joypad) -> u8 {
    let mut mask = 0;
    for &source in context.interrupt_requests() {
        mask |= interrupt_source_bit(source);
    }
    mask |= ppu.pending_interrupt_request_mask();
    if joypad.interrupt_request_pending() {
        mask |= 0x10;
    }
    mask
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingPpuMmioWrite {
    address: u16,
    value: u8,
}

fn cpu_write_targets_ppu_mmio(bus: &Bus, address: u16) -> bool {
    bus.describe_io_register(address)
        .is_some_and(|info| info.owner() == IoRegisterOwner::Ppu)
}

fn commit_pending_ppu_mmio_write(ppu: &mut Ppu, pending: &mut Option<PendingPpuMmioWrite>) {
    if let Some(write) = pending.take() {
        ppu.write_register(write.address, write.value);
    }
}

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
}

impl Machine<TraceSummaryBuffer> {
    pub fn new_summary(config: MachineConfig) -> Self {
        Self::with_tracer(config, Tracer::summary())
    }
}

impl<S: TraceSink + TraceSnapshotProvider> Machine<S> {
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

    pub fn set_serial_peer(&mut self, peer: SerialPeer) {
        self.serial.set_peer(peer);
    }

    pub fn queue_external_serial_clock(&mut self) {
        self.serial.queue_external_clock_pulse();
    }

    pub fn take_serial_output_bytes(&mut self) -> Vec<u8> {
        self.serial.take_completed_output_bytes()
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

    pub fn restore_cartridge_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        self.cartridge.restore_persistent_state(state)
    }

    pub fn advance_cartridge_rtc_seconds(&mut self, seconds: u64) {
        self.cartridge.advance_rtc_seconds(seconds);
    }

    pub fn read_bus(&mut self, address: u16) -> u8 {
        let state = self.current_bus_arbitration_state();
        let value = self.bus.read_with_t_cycle_context(
            address,
            BusRequester::Cpu,
            &state,
            self.next_t_cycle(),
            Some(&mut self.cartridge),
            crate::bus::BusIoReadView {
                apu: Some(&self.apu),
                timer: Some(&self.timer),
                serial: Some(&self.serial),
                dma: Some(&self.dma),
                boot: Some(&self.boot),
                interrupts: Some(&self.interrupts),
                interrupt_flag_pending_mask: 0,
                joypad: Some(&self.joypad),
                ppu: Some(&self.ppu),
            },
        );
        self.bus.route_cpu_address_event(
            CpuAddressEvent {
                kind: CpuAddressEventKind::Read,
                access_address: Some(address),
                idu_address: None,
                update_direction: None,
            },
            &state,
            &mut self.ppu,
        );
        value
    }

    pub fn write_bus(&mut self, address: u16, value: u8) {
        let state = self.current_bus_arbitration_state();

        self.bus.write_with_t_cycle_context(
            address,
            value,
            BusRequester::Cpu,
            &state,
            self.next_t_cycle(),
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
        self.bus.route_cpu_address_event(
            CpuAddressEvent {
                kind: CpuAddressEventKind::Write,
                access_address: Some(address),
                idu_address: None,
                update_direction: None,
            },
            &state,
            &mut self.ppu,
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
        let mut pending_ppu_mmio_write = None;

        self.scheduler
            .step_with_trace(&mut self.tracer, |context, tracer| match context.phase() {
                SchedulerPhase::DerivedEdgeResolution => {
                    if !matches!(
                        cpu.execution_state(),
                        CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
                    ) {
                        timer.tick_t_cycle(context);
                    }
                    tracer.emit_with(TraceSubsystem::Timer, TraceLevel::Trace, || {
                        timer.scheduler_trace_message(context)
                    });
                }
                SchedulerPhase::AutonomousPeripheralTicks => {
                    if !matches!(
                        cpu.execution_state(),
                        CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
                    ) {
                        apu.tick_t_cycle(context);
                        let dma_transfer_work = dma.tick_t_cycle(context);
                        let dma_oam_conflict_address =
                            dma_transfer_work.and_then(|transfer_work| {
                                let destination_address = transfer_work.destination_address();
                                (0xFE00..=0xFE9F)
                                    .contains(&destination_address)
                                    .then_some(destination_address)
                            });
                        let dma_oam_active = dma.bus_state().active_region().is_some();
                        bus.sync_video_domain_ownership(ppu.bus_state(), dma.bus_state());
                        let (oam_view, vram_view) = bus.video_views(BusMaster::Ppu);
                        ppu.tick_t_cycle(
                            context,
                            oam_view,
                            vram_view,
                            dma_oam_active,
                            dma_oam_conflict_address,
                        );
                        bus.sync_video_domain_ownership(ppu.bus_state(), dma.bus_state());
                        serial.tick_t_cycle(context);
                        if let Some(transfer_work) = dma_transfer_work {
                            let arbitration_state = BusArbitrationState::default()
                                .with_boot_rom(boot.bus_state())
                                .with_ppu(ppu.bus_state())
                                .with_dma(dma.bus_state());
                            let value = bus.read_with_t_cycle_context(
                                transfer_work.source_address(),
                                BusRequester::Dma,
                                &arbitration_state,
                                context.t_cycle(),
                                Some(cartridge),
                                BusIoReadView {
                                    apu: Some(&*apu),
                                    timer: Some(&*timer),
                                    serial: Some(&*serial),
                                    dma: Some(&*dma),
                                    boot: Some(&*boot),
                                    interrupts: Some(&*interrupts),
                                    interrupt_flag_pending_mask: 0,
                                    joypad: Some(&*joypad),
                                    ppu: Some(&*ppu),
                                },
                            );
                            bus.write_with_context(
                                transfer_work.destination_address(),
                                value,
                                BusRequester::Dma,
                                &arbitration_state,
                                None,
                                BusIoWriteView::default(),
                            );
                        }
                    }
                    tracer.emit_with(TraceSubsystem::Dma, TraceLevel::Trace, || {
                        dma.scheduler_trace_message(context)
                    });
                    tracer.emit_with(TraceSubsystem::Ppu, TraceLevel::Trace, || {
                        ppu.scheduler_trace_message(context)
                    });
                    tracer.emit_with(TraceSubsystem::Serial, TraceLevel::Trace, || {
                        serial.scheduler_trace_message(context)
                    });
                }
                SchedulerPhase::BusArbitration => {
                    let arbitration_state = BusArbitrationState::default()
                        .with_boot_rom(boot.bus_state())
                        .with_ppu(ppu.bus_state())
                        .with_dma(dma.bus_state());
                    tracer.emit_with(TraceSubsystem::Bus, TraceLevel::Trace, || {
                        bus.scheduler_trace_message(context, &arbitration_state)
                    });
                    tracer.emit_with(TraceSubsystem::Cartridge, TraceLevel::Trace, || {
                        cartridge.scheduler_trace_message(context)
                    });
                }
                SchedulerPhase::CpuMicroOperation => {
                    let arbitration_state = BusArbitrationState::default()
                        .with_boot_rom(boot.bus_state())
                        .with_ppu(ppu.bus_state())
                        .with_dma(dma.bus_state());
                    let interrupt_flag_pending_mask =
                        current_cycle_interrupt_read_mask(context, ppu, joypad);
                    cpu.tick_t_cycle(|operation| match operation {
                        CpuBusOperation::Read { address } => Some(bus.read_with_t_cycle_context(
                            address,
                            BusRequester::Cpu,
                            &arbitration_state,
                            context.t_cycle(),
                            Some(cartridge),
                            BusIoReadView {
                                apu: Some(apu),
                                timer: Some(timer),
                                serial: Some(serial),
                                dma: Some(dma),
                                boot: Some(boot),
                                interrupts: Some(interrupts),
                                interrupt_flag_pending_mask,
                                joypad: Some(joypad),
                                ppu: Some(ppu),
                            },
                        )),
                        CpuBusOperation::Write { address, value } => {
                            if cpu_write_targets_ppu_mmio(bus, address) {
                                pending_ppu_mmio_write =
                                    Some(PendingPpuMmioWrite { address, value });
                                context.queue_side_effect(SchedulerSideEffect::CommitMmioWrite);
                            } else {
                                bus.write_with_t_cycle_context(
                                    address,
                                    value,
                                    BusRequester::Cpu,
                                    &arbitration_state,
                                    context.t_cycle(),
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
                            }
                            None
                        }
                        CpuBusOperation::PendingInterruptMask => Some(interrupts.pending_mask()),
                        CpuBusOperation::InterruptEnableMask => Some(interrupts.read_ie()),
                        CpuBusOperation::StopWakeLineAsserted => {
                            Some(u8::from(joypad.stop_wake_line_asserted()))
                        }
                        CpuBusOperation::AcknowledgeInterrupt { source } => {
                            interrupts.clear(source);
                            None
                        }
                        CpuBusOperation::RequestInterrupt { source } => {
                            interrupts.request(source);
                            None
                        }
                    });
                    ppu.set_system_stop_active(matches!(
                        cpu.execution_state(),
                        CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
                    ));
                    if let Some(event) = cpu.last_address_event() {
                        bus.route_cpu_address_event(event, &arbitration_state, ppu);
                    }
                    tracer.emit_with(TraceSubsystem::Cpu, TraceLevel::Trace, || {
                        cpu.scheduler_trace_message(context)
                    });
                }
                SchedulerPhase::MmioSideEffectCommit => {
                    commit_pending_ppu_mmio_write(ppu, &mut pending_ppu_mmio_write);
                    tracer.emit_with(TraceSubsystem::Boot, TraceLevel::Trace, || {
                        boot.scheduler_trace_message(context)
                    });
                }
                SchedulerPhase::InterruptAggregation => {
                    if joypad.should_emit_scheduler_trace() {
                        tracer.emit_with(TraceSubsystem::Joypad, TraceLevel::Trace, || {
                            joypad.scheduler_trace_message(context)
                        });
                    }
                    for &source in context.interrupt_requests() {
                        interrupts.request(source);
                    }
                    for source in ppu.drain_pending_interrupt_requests() {
                        interrupts.request(source);
                    }
                    if joypad.consume_interrupt_request() {
                        interrupts.request(crate::scheduler::InterruptSource::Joypad);
                    }
                    tracer.emit_with(TraceSubsystem::Interrupts, TraceLevel::Trace, || {
                        interrupts.scheduler_trace_message(context)
                    });
                }
                SchedulerPhase::CpuWakeInterruptEvaluation => {
                    cpu.evaluate_wake_and_interrupts(interrupts, joypad);
                    ppu.set_system_stop_active(matches!(
                        cpu.execution_state(),
                        CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
                    ));
                    if joypad.should_emit_scheduler_trace() {
                        tracer.emit_with(TraceSubsystem::Joypad, TraceLevel::Trace, || {
                            joypad.scheduler_trace_message(context)
                        });
                    }
                    tracer.emit_with(TraceSubsystem::Interrupts, TraceLevel::Trace, || {
                        interrupts.scheduler_trace_message(context)
                    });
                    tracer.emit_with(TraceSubsystem::Cpu, TraceLevel::Trace, || {
                        cpu.scheduler_trace_message(context)
                    });
                }
                _ => {}
            })
    }

    fn current_bus_arbitration_state(&self) -> BusArbitrationState {
        BusArbitrationState::default()
            .with_boot_rom(self.boot.bus_state())
            .with_ppu(self.ppu.bus_state())
            .with_dma(self.dma.bus_state())
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
mod tests;
