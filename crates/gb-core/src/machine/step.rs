use super::{
    Machine, MachineStepObserver, MachineStepRegion, NoopMachineStepObserver, PendingExternalEvents,
};
use crate::apu::Apu;
use crate::boot::BootController;
use crate::bus::{
    Bus, BusArbitrationState, BusIoReadView, BusIoWriteView, BusMaster, BusRequester,
    IoRegisterOwner,
};
use crate::cartridge::CartridgeSlot;
use crate::cpu::{CpuBusOperation, CpuCore, CpuExecutionState};
use crate::debugger::{TraceLevel, TraceSink, TraceSubsystem, Tracer};
use crate::dma::DmaController;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::ppu::{Ppu, PpuDmaOamConflict};
use crate::scheduler::{
    CycleContext, ExternalEvent, InterruptSource, SchedulerPhase, SchedulerSideEffect,
};
use crate::serial::Serial;
use crate::timer::Timer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PendingPpuMmioWrite {
    pub(super) address: u16,
    pub(super) value: u8,
}

fn interrupt_source_bit(source: InterruptSource) -> u8 {
    match source {
        InterruptSource::VBlank => 0x01,
        InterruptSource::LcdStat => 0x02,
        InterruptSource::Timer => 0x04,
        InterruptSource::Serial => 0x08,
        InterruptSource::Joypad => 0x10,
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

pub(super) fn cpu_write_targets_ppu_mmio(bus: &Bus, address: u16) -> bool {
    bus.describe_io_register(address)
        .is_some_and(|info| info.owner() == IoRegisterOwner::Ppu)
}

pub(super) fn commit_pending_ppu_mmio_write(
    ppu: &mut Ppu,
    pending: &mut Option<PendingPpuMmioWrite>,
) -> Option<PendingPpuMmioWrite> {
    if let Some(write) = pending.take() {
        ppu.write_register_with_source(
            write.address,
            write.value,
            crate::ppu::PpuRegisterWriteSource::CpuMmioCommit,
        );
        Some(write)
    } else {
        None
    }
}

fn apply_stop_div_reset(apu: &mut Apu, timer: &mut Timer) {
    let effects = timer.write_div_with_effects(0);
    if effects.apu_frame_sequencer_edge {
        apu.on_div_apu_edge();
    }
}

fn observe_machine_step_region<O, R>(
    observer: &mut O,
    region: MachineStepRegion,
    observe: impl FnOnce() -> R,
) -> R
where
    O: MachineStepObserver,
{
    observer.begin_region(region);
    let result = observe();
    observer.end_region(region);
    result
}

struct MachinePhaseRunner<'a> {
    cpu: &'a mut CpuCore,
    bus: &'a mut Bus,
    apu: &'a mut Apu,
    ppu: &'a mut Ppu,
    dma: &'a mut DmaController,
    timer: &'a mut Timer,
    serial: &'a mut Serial,
    boot: &'a mut BootController,
    interrupts: &'a mut InterruptController,
    joypad: &'a mut Joypad,
    cartridge: &'a mut CartridgeSlot,
    pending_external_events: &'a mut PendingExternalEvents,
    pending_ppu_mmio_write: Option<PendingPpuMmioWrite>,
}

impl MachinePhaseRunner<'_> {
    fn step_phase<S, O>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
        observer: &mut O,
    ) where
        S: TraceSink,
        O: MachineStepObserver,
    {
        match context.phase() {
            SchedulerPhase::ExternalEventIngress => {
                self.step_external_event_ingress(context, tracer, observer);
            }
            SchedulerPhase::DerivedEdgeResolution => {
                self.step_derived_edge_resolution(context, tracer, observer);
            }
            SchedulerPhase::AutonomousPeripheralTicks => {
                self.step_autonomous_peripheral_ticks(context, tracer, observer);
            }
            SchedulerPhase::BusArbitration => {
                self.step_bus_arbitration(context, tracer);
            }
            SchedulerPhase::CpuMicroOperation => {
                self.step_cpu_micro_operation(context, tracer, observer);
            }
            SchedulerPhase::MmioSideEffectCommit => {
                self.step_mmio_side_effect_commit(context, tracer, observer);
            }
            SchedulerPhase::InterruptAggregation => {
                self.step_interrupt_aggregation(context, tracer, observer);
            }
            SchedulerPhase::CpuWakeInterruptEvaluation => {
                self.step_cpu_wake_interrupt_evaluation(context, tracer, observer);
            }
            _ => {}
        }
    }

    fn step_external_event_ingress<S, O>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
        observer: &mut O,
    ) where
        S: TraceSink,
        O: MachineStepObserver,
    {
        observe_machine_step_region(observer, MachineStepRegion::ExternalEvents, || {
            if let Some(pressed_mask) = self
                .pending_external_events
                .take_pending_joypad_pressed_mask()
                && self.joypad.apply_pressed_mask(pressed_mask)
            {
                context.push_external_event(ExternalEvent::HostInputChanged);
                tracer.emit_with(TraceSubsystem::Joypad, TraceLevel::Trace, || {
                    self.joypad.scheduler_trace_message(context)
                });
            }

            if self
                .pending_external_events
                .take_external_serial_clock_pulse()
                && !self.cpu_stop_active()
                && self.serial.queue_external_clock_pulse()
            {
                context.push_external_event(ExternalEvent::ExternalSerialClock);
                tracer.emit_with(TraceSubsystem::Serial, TraceLevel::Trace, || {
                    self.serial.external_event_ingress_trace_message(context)
                });
            }
        });
    }

    fn step_derived_edge_resolution<S, O>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
        observer: &mut O,
    ) where
        S: TraceSink,
        O: MachineStepObserver,
    {
        if !self.cpu_stop_active() {
            observe_machine_step_region(observer, MachineStepRegion::Timer, || {
                self.timer.tick_t_cycle(context);
            });
        }
        tracer.emit_with(TraceSubsystem::Timer, TraceLevel::Trace, || {
            self.timer.scheduler_trace_message(context)
        });
    }

    fn step_autonomous_peripheral_ticks<S, O>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
        observer: &mut O,
    ) where
        S: TraceSink,
        O: MachineStepObserver,
    {
        if !self.cpu_stop_active() {
            observe_machine_step_region(observer, MachineStepRegion::Apu, || {
                self.apu.tick_t_cycle(context);
            });
            let dma_transfer_work =
                observe_machine_step_region(observer, MachineStepRegion::Dma, || {
                    self.dma.tick_t_cycle(context)
                });
            let dma_arbitration_state = BusArbitrationState::default()
                .with_boot_rom(self.boot.bus_state())
                .with_ppu(self.ppu.bus_state())
                .with_dma(self.dma.bus_state());
            let dma_transfer_byte =
                observe_machine_step_region(observer, MachineStepRegion::Dma, || {
                    dma_transfer_work.map(|transfer_work| {
                        self.bus.read_with_t_cycle_context(
                            transfer_work.source_address(),
                            BusRequester::Dma,
                            &dma_arbitration_state,
                            context.t_cycle(),
                            Some(&mut self.cartridge),
                            BusIoReadView {
                                apu: Some(self.apu),
                                timer: Some(self.timer),
                                serial: Some(self.serial),
                                dma: Some(self.dma),
                                boot: Some(self.boot),
                                interrupts: Some(self.interrupts),
                                interrupt_flag_pending_mask: 0,
                                joypad: Some(self.joypad),
                                ppu: Some(self.ppu),
                                ppu_cpu_visible_read: false,
                            },
                        )
                    })
                });
            let dma_oam_conflict =
                dma_transfer_work
                    .zip(dma_transfer_byte)
                    .and_then(|(transfer_work, value)| {
                        let destination_address = transfer_work.destination_address();
                        (0xFE00..=0xFE9F)
                            .contains(&destination_address)
                            .then_some(PpuDmaOamConflict::new(destination_address, value))
                    });
            let dma_oam_active = self.dma.bus_state().active_region().is_some();

            observer.begin_region(MachineStepRegion::Ppu);
            self.bus
                .sync_video_domain_ownership(self.ppu.owner_bus_state(), self.dma.bus_state());
            let (oam_view, vram_view) = self.bus.video_views(BusMaster::Ppu);
            self.ppu.tick_t_cycle_with_observer(
                context,
                oam_view,
                vram_view,
                dma_oam_active,
                dma_oam_conflict,
                observer,
            );
            self.bus
                .sync_video_domain_ownership(self.ppu.owner_bus_state(), self.dma.bus_state());
            observer.end_region(MachineStepRegion::Ppu);
            observe_machine_step_region(observer, MachineStepRegion::Serial, || {
                self.serial.tick_t_cycle(context);
            });

            if let Some((transfer_work, value)) = dma_transfer_work.zip(dma_transfer_byte) {
                observe_machine_step_region(observer, MachineStepRegion::Dma, || {
                    self.bus.write_with_context(
                        transfer_work.destination_address(),
                        value,
                        BusRequester::Dma,
                        &dma_arbitration_state,
                        None,
                        BusIoWriteView::default(),
                    );
                });
            }
        }

        tracer.emit_with(TraceSubsystem::Dma, TraceLevel::Trace, || {
            self.dma.scheduler_trace_message(context)
        });
        tracer.emit_with(TraceSubsystem::Apu, TraceLevel::Trace, || {
            self.apu.scheduler_trace_message(context)
        });
        tracer.emit_with(TraceSubsystem::Ppu, TraceLevel::Trace, || {
            self.ppu.scheduler_trace_message(context)
        });
        tracer.emit_with(TraceSubsystem::Serial, TraceLevel::Trace, || {
            self.serial.scheduler_trace_message(context)
        });
    }

    fn step_bus_arbitration<S: TraceSink>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
    ) {
        let arbitration_state = self.current_bus_arbitration_state();
        tracer.emit_with(TraceSubsystem::Bus, TraceLevel::Trace, || {
            self.bus
                .scheduler_trace_message(context, &arbitration_state)
        });
        tracer.emit_with(TraceSubsystem::Cartridge, TraceLevel::Trace, || {
            self.cartridge.scheduler_trace_message(context)
        });
    }

    fn step_cpu_micro_operation<S, O>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
        observer: &mut O,
    ) where
        S: TraceSink,
        O: MachineStepObserver,
    {
        let arbitration_state = self.current_bus_arbitration_state();
        let cpu_read_arbitration_state = arbitration_state.with_ppu(self.ppu.cpu_bus_state());
        let cpu_write_arbitration_state =
            arbitration_state.with_ppu(self.ppu.cpu_write_bus_state());
        let interrupt_flag_pending_mask =
            current_cycle_interrupt_read_mask(context, self.ppu, self.joypad);

        observe_machine_step_region(observer, MachineStepRegion::Cpu, || {
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
            let pending_ppu_mmio_write = &mut self.pending_ppu_mmio_write;

            cpu.tick_t_cycle(|operation| match operation {
                CpuBusOperation::Read { address } => {
                    let read_arbitration_state = if (0xFE00..=0xFE9F).contains(&address) {
                        cpu_read_arbitration_state.with_ppu(ppu.cpu_oam_read_bus_state())
                    } else {
                        cpu_read_arbitration_state
                    };
                    Some(bus.read_with_t_cycle_context(
                        address,
                        BusRequester::Cpu,
                        &read_arbitration_state,
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
                            ppu_cpu_visible_read: true,
                        },
                    ))
                }
                CpuBusOperation::Write { address, value } => {
                    if cpu_write_targets_ppu_mmio(bus, address) {
                        *pending_ppu_mmio_write = Some(PendingPpuMmioWrite { address, value });
                        context.queue_side_effect(SchedulerSideEffect::CommitMmioWrite);
                    } else {
                        let write_arbitration_state = if (0xFE00..=0xFE9F).contains(&address) {
                            cpu_write_arbitration_state.with_ppu(ppu.cpu_oam_write_bus_state())
                        } else {
                            cpu_write_arbitration_state
                        };
                        bus.write_with_t_cycle_context(
                            address,
                            value,
                            BusRequester::Cpu,
                            &write_arbitration_state,
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

            if let Some(event) = cpu.last_address_event() {
                bus.route_cpu_address_event(event, &arbitration_state, ppu);
            }

            if cpu.take_stop_div_reset_request() {
                apply_stop_div_reset(apu, timer);
            }
        });

        self.update_ppu_stop_state();
        tracer.emit_with(TraceSubsystem::Cpu, TraceLevel::Trace, || {
            self.cpu.scheduler_trace_message(context)
        });
    }

    fn step_mmio_side_effect_commit<S, O>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
        observer: &mut O,
    ) where
        S: TraceSink,
        O: MachineStepObserver,
    {
        if let Some(write) = observe_machine_step_region(observer, MachineStepRegion::Ppu, || {
            commit_pending_ppu_mmio_write(self.ppu, &mut self.pending_ppu_mmio_write)
        }) {
            tracer.emit_with(TraceSubsystem::Ppu, TraceLevel::Trace, || {
                self.ppu
                    .mmio_commit_trace_message(context, write.address, write.value)
            });
        } else {
            tracer.emit_with(TraceSubsystem::Boot, TraceLevel::Trace, || {
                self.boot.scheduler_trace_message(context)
            });
        }
    }

    fn step_interrupt_aggregation<S, O>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
        observer: &mut O,
    ) where
        S: TraceSink,
        O: MachineStepObserver,
    {
        if self.joypad.should_emit_scheduler_trace() {
            tracer.emit_with(TraceSubsystem::Joypad, TraceLevel::Trace, || {
                self.joypad.scheduler_trace_message(context)
            });
        }

        observe_machine_step_region(observer, MachineStepRegion::Interrupts, || {
            let interrupts = &mut self.interrupts;
            let ppu = &mut self.ppu;
            let joypad = &mut self.joypad;

            for &source in context.interrupt_requests() {
                interrupts.request(source);
            }
            for source in ppu.drain_pending_interrupt_requests() {
                interrupts.request(source);
            }
            if joypad.consume_interrupt_request() {
                interrupts.request(InterruptSource::Joypad);
            }
        });

        tracer.emit_with(TraceSubsystem::Interrupts, TraceLevel::Trace, || {
            self.interrupts.scheduler_trace_message(context)
        });
    }

    fn step_cpu_wake_interrupt_evaluation<S, O>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
        observer: &mut O,
    ) where
        S: TraceSink,
        O: MachineStepObserver,
    {
        observe_machine_step_region(observer, MachineStepRegion::Cpu, || {
            let cpu = &mut self.cpu;
            let interrupts = &mut self.interrupts;
            let joypad = &mut self.joypad;
            cpu.evaluate_wake_and_interrupts(interrupts, joypad);
        });

        self.update_ppu_stop_state();

        if self.joypad.should_emit_scheduler_trace() {
            tracer.emit_with(TraceSubsystem::Joypad, TraceLevel::Trace, || {
                self.joypad.scheduler_trace_message(context)
            });
        }
        tracer.emit_with(TraceSubsystem::Interrupts, TraceLevel::Trace, || {
            self.interrupts.scheduler_trace_message(context)
        });
        tracer.emit_with(TraceSubsystem::Cpu, TraceLevel::Trace, || {
            self.cpu.scheduler_trace_message(context)
        });
    }

    fn current_bus_arbitration_state(&self) -> BusArbitrationState {
        BusArbitrationState::default()
            .with_boot_rom(self.boot.bus_state())
            .with_ppu(self.ppu.bus_state())
            .with_dma(self.dma.bus_state())
    }

    fn cpu_stop_active(&self) -> bool {
        matches!(
            self.cpu.execution_state(),
            CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
        )
    }

    fn update_ppu_stop_state(&mut self) {
        self.ppu.set_system_stop_active(self.cpu_stop_active());
    }
}

impl<S: TraceSink> Machine<S> {
    pub fn step_t_cycle(&mut self) -> CycleContext {
        self.step_t_cycle_with_observer(&mut NoopMachineStepObserver)
    }

    pub fn step_t_cycle_with_observer<O>(&mut self, observer: &mut O) -> CycleContext
    where
        O: MachineStepObserver,
    {
        let scheduler = &mut self.scheduler;
        let tracer = &mut self.tracer;
        let mut runner = MachinePhaseRunner {
            cpu: &mut self.cpu,
            bus: &mut self.bus,
            apu: &mut self.apu,
            ppu: &mut self.ppu,
            dma: &mut self.dma,
            timer: &mut self.timer,
            serial: &mut self.serial,
            boot: &mut self.boot,
            interrupts: &mut self.interrupts,
            joypad: &mut self.joypad,
            cartridge: &mut self.cartridge,
            pending_external_events: &mut self.pending_external_events,
            pending_ppu_mmio_write: None,
        };

        scheduler.step_with_trace(tracer, |context, tracer| {
            runner.step_phase(context, tracer, observer)
        })
    }
}
