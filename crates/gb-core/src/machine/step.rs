use super::Machine;
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
use crate::ppu::Ppu;
use crate::scheduler::{CycleContext, InterruptSource, SchedulerPhase, SchedulerSideEffect};
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

fn cpu_write_targets_ppu_mmio(bus: &Bus, address: u16) -> bool {
    bus.describe_io_register(address)
        .is_some_and(|info| info.owner() == IoRegisterOwner::Ppu)
}

pub(super) fn commit_pending_ppu_mmio_write(
    ppu: &mut Ppu,
    pending: &mut Option<PendingPpuMmioWrite>,
) {
    if let Some(write) = pending.take() {
        ppu.write_register(write.address, write.value);
    }
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
    pending_ppu_mmio_write: Option<PendingPpuMmioWrite>,
}

impl MachinePhaseRunner<'_> {
    fn step_phase<S: TraceSink>(&mut self, context: &mut CycleContext, tracer: &mut Tracer<S>) {
        match context.phase() {
            SchedulerPhase::DerivedEdgeResolution => {
                self.step_derived_edge_resolution(context, tracer);
            }
            SchedulerPhase::AutonomousPeripheralTicks => {
                self.step_autonomous_peripheral_ticks(context, tracer);
            }
            SchedulerPhase::BusArbitration => {
                self.step_bus_arbitration(context, tracer);
            }
            SchedulerPhase::CpuMicroOperation => {
                self.step_cpu_micro_operation(context, tracer);
            }
            SchedulerPhase::MmioSideEffectCommit => {
                self.step_mmio_side_effect_commit(context, tracer);
            }
            SchedulerPhase::InterruptAggregation => {
                self.step_interrupt_aggregation(context, tracer);
            }
            SchedulerPhase::CpuWakeInterruptEvaluation => {
                self.step_cpu_wake_interrupt_evaluation(context, tracer);
            }
            _ => {}
        }
    }

    fn step_derived_edge_resolution<S: TraceSink>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
    ) {
        if !self.cpu_stop_active() {
            self.timer.tick_t_cycle(context);
        }
        tracer.emit_with(TraceSubsystem::Timer, TraceLevel::Trace, || {
            self.timer.scheduler_trace_message(context)
        });
    }

    fn step_autonomous_peripheral_ticks<S: TraceSink>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
    ) {
        if !self.cpu_stop_active() {
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

            apu.tick_t_cycle(context);
            let dma_transfer_work = dma.tick_t_cycle(context);
            let dma_oam_conflict_address = dma_transfer_work.and_then(|transfer_work| {
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
                        apu: Some(*apu),
                        timer: Some(*timer),
                        serial: Some(*serial),
                        dma: Some(*dma),
                        boot: Some(*boot),
                        interrupts: Some(*interrupts),
                        interrupt_flag_pending_mask: 0,
                        joypad: Some(*joypad),
                        ppu: Some(*ppu),
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

    fn step_cpu_micro_operation<S: TraceSink>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
    ) {
        let arbitration_state = self.current_bus_arbitration_state();
        let interrupt_flag_pending_mask =
            current_cycle_interrupt_read_mask(context, self.ppu, self.joypad);

        {
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
                        *pending_ppu_mmio_write = Some(PendingPpuMmioWrite { address, value });
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

            if let Some(event) = cpu.last_address_event() {
                bus.route_cpu_address_event(event, &arbitration_state, ppu);
            }
        }

        self.update_ppu_stop_state();
        tracer.emit_with(TraceSubsystem::Cpu, TraceLevel::Trace, || {
            self.cpu.scheduler_trace_message(context)
        });
    }

    fn step_mmio_side_effect_commit<S: TraceSink>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
    ) {
        commit_pending_ppu_mmio_write(self.ppu, &mut self.pending_ppu_mmio_write);
        tracer.emit_with(TraceSubsystem::Boot, TraceLevel::Trace, || {
            self.boot.scheduler_trace_message(context)
        });
    }

    fn step_interrupt_aggregation<S: TraceSink>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
    ) {
        if self.joypad.should_emit_scheduler_trace() {
            tracer.emit_with(TraceSubsystem::Joypad, TraceLevel::Trace, || {
                self.joypad.scheduler_trace_message(context)
            });
        }

        {
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
        }

        tracer.emit_with(TraceSubsystem::Interrupts, TraceLevel::Trace, || {
            self.interrupts.scheduler_trace_message(context)
        });
    }

    fn step_cpu_wake_interrupt_evaluation<S: TraceSink>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
    ) {
        {
            let cpu = &mut self.cpu;
            let interrupts = &mut self.interrupts;
            let joypad = &mut self.joypad;
            cpu.evaluate_wake_and_interrupts(interrupts, joypad);
        }

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
            pending_ppu_mmio_write: None,
        };

        scheduler.step_with_trace(tracer, |context, tracer| runner.step_phase(context, tracer))
    }
}
