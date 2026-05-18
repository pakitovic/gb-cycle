use super::{
    Machine, MachineStepObserver, MachineStepRegion, NoopMachineStepObserver, PendingExternalEvents,
};
use crate::apu::Apu;
use crate::boot::BootController;
use crate::bus::{
    Bus, BusArbitrationState, BusIoReadView, BusIoWriteView, BusMaster, BusRequester, DmaBusState,
    DmaMemoryRegionImpact,
};
use crate::cartridge::CartridgeSlot;
use crate::cpu::{CpuBusOperation, CpuCore, CpuExecutionState, CpuExternalOperation};
use crate::debugger::{TraceLevel, TraceSink, TraceSubsystem, Tracer};
use crate::dma::{DmaController, VramDmaRuntimeContext};
use crate::external_port::{ExternalPort, ExternalPortAttachmentKind};
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::model::{MachineConfig, StartupMode};
use crate::ppu::{Ppu, PpuBusStateSnapshot, PpuDmaOamConflict, PpuStepRegion};
use crate::scheduler::{
    CycleContext, ExternalEvent, InterruptSource, SchedulerPhase, SchedulerSideEffect,
    scheduler_phase_trace_message,
};
use crate::serial::{Serial, SerialTickTelemetry};
use crate::speed::CgbSpeedMode;
use crate::speed::SpeedController;
use crate::timer::Timer;

const CPU_OAM_ADDRESS_START: u16 = 0xFE00;
const CPU_OAM_ADDRESS_END: u16 = 0xFE9F;
const CPU_MMIO_ADDRESS_START: u16 = 0xFF00;
const CPU_MMIO_ADDRESS_END: u16 = 0xFF7F;
const INTERRUPT_FLAG_ADDRESS: u16 = 0xFF0F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PendingPpuMmioWrite {
    pub(super) address: u16,
    pub(super) value: u8,
}

pub(super) fn finalize_cgb_real_boot_handoff_if_needed(
    config: &mut MachineConfig,
    bus: &mut Bus,
    ppu: &mut Ppu,
    serial: &mut Serial,
    speed: &mut SpeedController,
    boot: &BootController,
    boot_rom_newly_unmapped: bool,
) {
    if boot_rom_newly_unmapped
        && !boot.is_boot_rom_mapped()
        && config.startup_mode == StartupMode::RealBoot
        && config.console_model.is_dmg_family()
    {
        ppu.apply_dmg_real_boot_handoff_stat_irq_phase();
    }

    if !boot_rom_newly_unmapped
        || boot.is_boot_rom_mapped()
        || config.startup_mode != StartupMode::RealBoot
        || !config.console_model.is_cgb_family()
    {
        return;
    }

    if let Some(operating_mode) = bus.lock_cgb_real_boot_key0_at_handoff() {
        config.operating_mode = operating_mode;
        bus.apply_operating_mode_state(operating_mode);
        serial.apply_operating_mode_state(operating_mode);
        speed.apply_operating_mode_state(operating_mode);
        ppu.apply_operating_mode_state(operating_mode);
    }
}

#[inline]
fn address_hits_cpu_oam_window(address: u16) -> bool {
    (CPU_OAM_ADDRESS_START..=CPU_OAM_ADDRESS_END).contains(&address)
}

#[inline(always)]
fn address_is_cpu_mmio(address: u16) -> bool {
    (CPU_MMIO_ADDRESS_START..=CPU_MMIO_ADDRESS_END).contains(&address)
}

fn current_cycle_interrupt_read_mask(context: &CycleContext, ppu: &Ppu, joypad: &Joypad) -> u8 {
    let mut mask = current_cycle_scheduler_interrupt_request_mask(context);
    mask |= ppu.cpu_visible_pending_interrupt_request_mask();
    if joypad.interrupt_request_pending() {
        mask |= InterruptSource::Joypad.mask();
    }
    mask
}

fn current_cycle_scheduler_interrupt_request_mask(context: &CycleContext) -> u8 {
    let mut mask = 0;
    for &source in context.interrupt_requests() {
        mask |= source.mask();
    }
    mask
}

#[inline]
fn cpu_interrupt_mask_for_if_read(
    address: u16,
    context: &CycleContext,
    ppu: &Ppu,
    joypad: &Joypad,
) -> u8 {
    if address == INTERRUPT_FLAG_ADDRESS {
        current_cycle_interrupt_read_mask(context, ppu, joypad)
    } else {
        0
    }
}

#[inline(always)]
fn cpu_read_arbitration_state(
    address: u16,
    arbitration_states: CpuBusArbitrationStates,
    ppu: &Ppu,
) -> BusArbitrationState {
    if address_hits_cpu_oam_window(address) {
        arbitration_states
            .pre_cpu
            .with_ppu(ppu.cpu_oam_read_bus_state())
    } else {
        arbitration_states.cpu_read
    }
}

#[inline]
fn cpu_write_arbitration_state(
    address: u16,
    arbitration_states: CpuBusArbitrationStates,
    ppu: &Ppu,
) -> BusArbitrationState {
    if address_hits_cpu_oam_window(address) {
        arbitration_states
            .pre_cpu
            .with_ppu(ppu.cpu_oam_write_bus_state())
    } else {
        arbitration_states.cpu_write
    }
}

#[inline]
fn finalize_cpu_micro_operation(
    cpu: &mut CpuCore,
    bus: &mut Bus,
    apu: &mut Apu,
    ppu: &mut Ppu,
    timer: &mut Timer,
    speed: &mut SpeedController,
    arbitration_state: &BusArbitrationState,
) {
    if let Some(event) = cpu.last_address_event() {
        bus.route_cpu_address_event(event, arbitration_state, ppu);
    }

    if cpu.take_stop_div_reset_request() {
        apply_stop_div_reset(apu, timer, speed.current_speed());
    }

    if cpu.take_cgb_speed_switch_request() {
        let _ = speed.begin_prepared_speed_switch();
    }
}

pub(super) fn cpu_write_targets_ppu_mmio(bus: &Bus, address: u16) -> bool {
    if !address_is_cpu_mmio(address) {
        return false;
    }

    if !Ppu::owns_mmio_register(address) {
        return false;
    }

    let Some(info) = bus.describe_io_register(address) else {
        return false;
    };

    if info.owner() != crate::bus::IoRegisterOwner::Ppu
        || info.implementation() != crate::bus::IoRegisterImplementation::Implemented
    {
        return false;
    }

    match info.availability() {
        crate::bus::IoRegisterAvailability::Shared
        | crate::bus::IoRegisterAvailability::DmgCompatible => true,
        crate::bus::IoRegisterAvailability::CgbOnly => bus.cgb_extensions_enabled(),
    }
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

fn apply_stop_div_reset(apu: &mut Apu, timer: &mut Timer, speed_mode: CgbSpeedMode) {
    let effects = timer.stop_reset_divider_with_effects_for_speed(speed_mode);
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
    if !observer.records_regions() {
        return observe();
    }

    observer.begin_region(region);
    let result = observe();
    observer.end_region(region);
    result
}

struct MachinePhaseRunner<'a> {
    config: &'a mut MachineConfig,
    cpu: &'a mut CpuCore,
    bus: &'a mut Bus,
    apu: &'a mut Apu,
    ppu: &'a mut Ppu,
    dma: &'a mut DmaController,
    timer: &'a mut Timer,
    serial: &'a mut Serial,
    speed: &'a mut SpeedController,
    external_port: &'a mut ExternalPort,
    boot: &'a mut BootController,
    interrupts: &'a mut InterruptController,
    joypad: &'a mut Joypad,
    cartridge: &'a mut CartridgeSlot,
    pending_external_events: &'a mut PendingExternalEvents,
    pending_ppu_mmio_write: &'a mut Option<PendingPpuMmioWrite>,
    cached_ppu_bus_state_snapshot: Option<PpuBusStateSnapshot>,
    cached_cpu_bus_arbitration_states: Option<CpuBusArbitrationStates>,
}

#[derive(Debug, Clone, Copy)]
struct CpuBusArbitrationStates {
    pre_cpu: BusArbitrationState,
    cpu_read: BusArbitrationState,
    cpu_write: BusArbitrationState,
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
        if !self.pending_external_events.has_pending_work() {
            return;
        }

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
                self.timer
                    .tick_t_cycle_for_speed(context, self.speed.current_speed());
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
        self.bus.tick_cgb_infrared_t_cycle();

        if !self.cpu_stop_active() {
            observe_machine_step_region(observer, MachineStepRegion::Apu, || {
                self.apu
                    .tick_t_cycle_for_speed(context, self.speed.current_speed());
            });

            let dma_requires_tick = self.dma.requires_t_cycle_tick();
            let (dma_bus_state, dma_arbitration_state, dma_transfer_work, dma_transfer_byte) =
                if dma_requires_tick {
                    let boot_bus_state = self.boot.bus_state();
                    let ppu_owner_bus_state_before = self.ppu.owner_bus_state();
                    let dma_transfer_work =
                        observe_machine_step_region(observer, MachineStepRegion::Dma, || {
                            self.dma.tick_t_cycle_with_vram_dma_context(
                                context,
                                VramDmaRuntimeContext::new(
                                    ppu_owner_bus_state_before,
                                    self.ppu.ly(),
                                    self.cpu.execution_state() == CpuExecutionState::Halted,
                                ),
                            )
                        });
                    let dma_bus_state = self.dma.bus_state();
                    let dma_arbitration_state = BusArbitrationState::default()
                        .with_boot_rom(boot_bus_state)
                        .with_ppu(ppu_owner_bus_state_before)
                        .with_dma(dma_bus_state);
                    let dma_transfer_byte =
                        observe_machine_step_region(observer, MachineStepRegion::Dma, || {
                            dma_transfer_work.map(|transfer_work| {
                                transfer_work
                                    .source_read_value_override()
                                    .unwrap_or_else(|| {
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
                                                speed: Some(self.speed),
                                                ppu_cpu_visible_read: false,
                                            },
                                        )
                                    })
                            })
                        });
                    (
                        dma_bus_state,
                        Some(dma_arbitration_state),
                        dma_transfer_work,
                        dma_transfer_byte,
                    )
                } else {
                    (DmaBusState::unrestricted(), None, None, None)
                };
            let dma_oam_conflict =
                dma_transfer_work
                    .zip(dma_transfer_byte)
                    .and_then(|(transfer_work, value)| {
                        let destination_address = transfer_work.destination_address();
                        address_hits_cpu_oam_window(destination_address)
                            .then_some(PpuDmaOamConflict::new(destination_address, value))
                    });
            let dma_oam_active = dma_bus_state.active_region() == Some(DmaMemoryRegionImpact::Oam);

            if self
                .speed
                .current_speed()
                .lcd_tick_due_at_scheduler_t_cycle(context.t_cycle().get())
            {
                self.tick_ppu_video_domain(
                    context,
                    dma_bus_state,
                    dma_oam_active,
                    dma_oam_conflict,
                    observer,
                );
            }
            if self.external_port.attachment_kind() == ExternalPortAttachmentKind::None
                && self.serial.external_wait_without_pending_clock()
            {
                let serial_telemetry =
                    observe_machine_step_region(observer, MachineStepRegion::Serial, || {
                        self.serial.tick_external_wait_t_cycle()
                    });
                observer.record_serial_tick(serial_telemetry);
            } else if self.serial.requires_full_t_cycle_tick()
                || self.external_port.requires_t_cycle_tick()
            {
                let serial_telemetry =
                    observe_machine_step_region(observer, MachineStepRegion::Serial, || {
                        let mut serial_telemetry = SerialTickTelemetry::default();
                        if self.external_port.requires_t_cycle_tick() {
                            self.external_port.tick_t_cycle();
                            serial_telemetry.accumulate(SerialTickTelemetry::external_port_tick());
                        }
                        serial_telemetry.accumulate(
                            self.serial
                                .tick_t_cycle_for_speed(context, self.speed.current_speed()),
                        );
                        if self.external_port.handles_completed_serial_byte()
                            && let Some(output_byte) = self.serial.latest_completed_output_byte()
                        {
                            self.external_port.handle_completed_serial_byte(output_byte);
                        }
                        if self
                            .external_port
                            .requires_serial_peer_refresh_after_t_cycle()
                        {
                            self.serial.set_peer(self.external_port.serial_peer());
                        }
                        serial_telemetry
                    });
                observer.record_serial_tick(serial_telemetry);
            } else {
                self.serial.tick_idle_t_cycle();
            }

            if let Some((transfer_work, value)) = dma_transfer_work.zip(dma_transfer_byte) {
                let dma_arbitration_state = dma_arbitration_state
                    .expect("DMA transfer work should have a matching arbitration state");
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
        } else if self.cpu_speed_switch_pause_active() {
            let dma_bus_state = self.dma.bus_state();
            self.tick_ppu_video_domain(
                context,
                dma_bus_state,
                dma_bus_state.active_region() == Some(DmaMemoryRegionImpact::Oam),
                None,
                observer,
            );
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

    fn tick_ppu_video_domain<O>(
        &mut self,
        context: &mut CycleContext,
        dma_bus_state: DmaBusState,
        dma_oam_active: bool,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
        observer: &mut O,
    ) where
        O: MachineStepObserver,
    {
        let records_regions = observer.records_regions();
        let records_ppu_regions = observer.records_ppu_regions();
        if records_regions {
            observer.begin_region(MachineStepRegion::Ppu);
        }
        #[cfg(any(debug_assertions, test))]
        let ppu_owner_bus_state_before = {
            if records_ppu_regions {
                observer.begin_ppu_region(PpuStepRegion::BusState);
            }
            let ppu_owner_bus_state_before = self.ppu.owner_bus_state();
            if records_ppu_regions {
                observer.end_ppu_region(PpuStepRegion::BusState);
            }
            ppu_owner_bus_state_before
        };
        #[cfg(not(any(debug_assertions, test)))]
        let ppu_owner_bus_state_before = crate::ppu::PpuBusState::default();
        if records_ppu_regions {
            observer.begin_ppu_region(PpuStepRegion::BusSync);
        }
        self.bus
            .sync_video_domain_ownership(ppu_owner_bus_state_before, dma_bus_state);
        if records_ppu_regions {
            observer.end_ppu_region(PpuStepRegion::BusSync);
        }
        if records_ppu_regions {
            observer.begin_ppu_region(PpuStepRegion::BusView);
        }
        let (oam_view, vram_view) = self.bus.video_views(BusMaster::Ppu);
        if records_ppu_regions {
            observer.end_ppu_region(PpuStepRegion::BusView);
        }
        self.ppu.tick_t_cycle_with_observer(
            context,
            oam_view,
            vram_view,
            dma_oam_active,
            dma_oam_conflict,
            observer,
        );
        let ppu_bus_states_after =
            self.ppu_bus_state_snapshot_with_observer(observer, records_ppu_regions);
        let ppu_owner_bus_state_after = ppu_bus_states_after.owner;
        if records_ppu_regions {
            observer.begin_ppu_region(PpuStepRegion::BusSync);
        }
        self.bus
            .sync_video_domain_ownership(ppu_owner_bus_state_after, dma_bus_state);
        if records_ppu_regions {
            observer.end_ppu_region(PpuStepRegion::BusSync);
        }
        if records_regions {
            observer.end_region(MachineStepRegion::Ppu);
        }
    }

    fn step_bus_arbitration<S: TraceSink>(
        &mut self,
        context: &mut CycleContext,
        tracer: &mut Tracer<S>,
    ) {
        if !tracer.records_events() {
            return;
        }

        let arbitration_state = self.cpu_bus_arbitration_states().pre_cpu;
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
        let arbitration_states = self.cpu_bus_arbitration_states();
        let stop_active_before = self.cpu_stop_active();
        observe_machine_step_region(observer, MachineStepRegion::Cpu, || {
            if self.dma.cpu_stall_active() {
                self.cpu.tick_dma_stall_t_cycle();
                return;
            }

            let cpu = &mut self.cpu;
            let bus = &mut self.bus;
            let apu = &mut self.apu;
            let ppu = &mut self.ppu;
            let dma = &mut self.dma;
            let timer = &mut self.timer;
            let serial = &mut self.serial;
            let speed = &mut self.speed;
            let boot = &mut self.boot;
            let config = &mut self.config;
            let interrupts = &mut self.interrupts;
            let joypad = &mut self.joypad;
            let cartridge = &mut self.cartridge;
            let pending_ppu_mmio_write = &mut *self.pending_ppu_mmio_write;

            if let Some(source) = cpu.evaluate_current_cycle_interrupt_requests(
                interrupts,
                current_cycle_scheduler_interrupt_request_mask(context)
                    & InterruptSource::Timer.mask(),
            ) {
                context.take_interrupt_request(source);
            }

            cpu.tick_t_cycle(|operation| match operation {
                CpuExternalOperation::Bus(CpuBusOperation::Read { address }) => {
                    let read_arbitration_state =
                        cpu_read_arbitration_state(address, arbitration_states, ppu);
                    let interrupt_flag_pending_mask =
                        cpu_interrupt_mask_for_if_read(address, context, ppu, joypad);

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
                            speed: Some(speed),
                            ppu_cpu_visible_read: true,
                        },
                    ))
                }
                CpuExternalOperation::Bus(CpuBusOperation::Write { address, value }) => {
                    if cpu_write_targets_ppu_mmio(bus, address) {
                        *pending_ppu_mmio_write = Some(PendingPpuMmioWrite { address, value });
                        context.queue_side_effect(SchedulerSideEffect::CommitMmioWrite);
                    } else {
                        let write_arbitration_state =
                            cpu_write_arbitration_state(address, arbitration_states, ppu);
                        let mut boot_rom_newly_unmapped = false;
                        bus.write_with_t_cycle_context(
                            address,
                            value,
                            BusRequester::Cpu,
                            &write_arbitration_state,
                            context.t_cycle(),
                            Some(cartridge),
                            BusIoWriteView {
                                apu: Some(&mut *apu),
                                timer: Some(&mut *timer),
                                serial: Some(&mut *serial),
                                dma: Some(&mut *dma),
                                boot: Some(&mut *boot),
                                interrupts: Some(&mut *interrupts),
                                joypad: Some(&mut *joypad),
                                ppu: Some(&mut *ppu),
                                speed: Some(&mut *speed),
                                boot_ff50_newly_unmapped: Some(&mut boot_rom_newly_unmapped),
                            },
                        );
                        finalize_cgb_real_boot_handoff_if_needed(
                            config,
                            bus,
                            ppu,
                            serial,
                            speed,
                            boot,
                            boot_rom_newly_unmapped,
                        );
                    }
                    None
                }
                CpuExternalOperation::PendingInterruptMask => Some(interrupts.pending_mask()),
                CpuExternalOperation::InterruptEnableMask => Some(interrupts.read_ie()),
                CpuExternalOperation::StopWakeLineAsserted => {
                    Some(u8::from(joypad.stop_wake_line_asserted()))
                }
                CpuExternalOperation::CgbSpeedSwitchPrepared => {
                    Some(u8::from(speed.switch_armed()))
                }
                CpuExternalOperation::AcknowledgeInterrupt { source } => {
                    interrupts.clear(source);
                    None
                }
                CpuExternalOperation::RequestInterrupt { source } => {
                    interrupts.request(source);
                    None
                }
            });

            finalize_cpu_micro_operation(
                cpu,
                bus,
                apu,
                ppu,
                timer,
                speed,
                &arbitration_states.pre_cpu,
            );
        });

        let stop_active_after = self.cpu_stop_active();
        if stop_active_before != stop_active_after {
            self.ppu.set_system_stop_active(stop_active_after);
        }
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
        if self.pending_ppu_mmio_write.is_none() {
            tracer.emit_with(TraceSubsystem::Boot, TraceLevel::Trace, || {
                self.boot.scheduler_trace_message(context)
            });
            return;
        }

        if let Some(write) = observe_machine_step_region(observer, MachineStepRegion::Ppu, || {
            commit_pending_ppu_mmio_write(self.ppu, self.pending_ppu_mmio_write)
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

        let has_interrupt_work = !context.interrupt_requests().is_empty()
            || self.ppu.pending_interrupt_request_mask() != 0
            || self.joypad.interrupt_request_pending();
        if !has_interrupt_work {
            tracer.emit_with(TraceSubsystem::Interrupts, TraceLevel::Trace, || {
                self.interrupts.scheduler_trace_message(context)
            });
            return;
        }

        observe_machine_step_region(observer, MachineStepRegion::Interrupts, || {
            let interrupts = &mut self.interrupts;
            let ppu = &mut self.ppu;
            let joypad = &mut self.joypad;

            for &source in context.interrupt_requests() {
                interrupts.request(source);
            }
            let ppu_pending_interrupts = ppu.take_pending_interrupt_request_mask();
            if ppu_pending_interrupts & 0x01 != 0 {
                interrupts.request(InterruptSource::VBlank);
            }
            if ppu_pending_interrupts & 0x02 != 0 {
                interrupts.request(InterruptSource::LcdStat);
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
        let stop_active_before = self.cpu_stop_active();
        observe_machine_step_region(observer, MachineStepRegion::Cpu, || {
            if self.dma.cpu_stall_active() {
                self.cpu.tick_dma_stall_t_cycle();
                return;
            }

            let cpu = &mut self.cpu;
            let interrupts = &mut self.interrupts;
            let joypad = &mut self.joypad;
            let ppu = &*self.ppu;
            if interrupts.highest_pending() == Some(InterruptSource::LcdStat)
                && (interrupts.read_ie() & InterruptSource::VBlank.mask()) != 0
                && ppu.dmg_mode2_vblank_entry_interrupt_service_deferred()
            {
                return;
            }
            if cpu.execution_state() == CpuExecutionState::Halted
                && interrupts.highest_pending() == Some(InterruptSource::LcdStat)
                && (ppu.dmg_lcd_reenable_mode0_halt_wake_deferred()
                    || ppu.dmg_mode2_oam_halt_wake_deferred()
                    || ppu.dmg_mode2_vblank_entry_halt_wake_deferred())
            {
                return;
            }
            cpu.evaluate_wake_and_interrupts(interrupts, joypad);
        });

        let stop_active_after = self.cpu_stop_active();
        if stop_active_before != stop_active_after {
            self.ppu.set_system_stop_active(stop_active_after);
        }

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

    fn ppu_bus_state_snapshot(&mut self) -> PpuBusStateSnapshot {
        if let Some(snapshot) = self.cached_ppu_bus_state_snapshot {
            return snapshot;
        }

        let snapshot = self.ppu.bus_state_snapshot();
        self.cached_ppu_bus_state_snapshot = Some(snapshot);
        snapshot
    }

    fn ppu_bus_state_snapshot_with_observer<O>(
        &mut self,
        observer: &mut O,
        records_ppu_regions: bool,
    ) -> PpuBusStateSnapshot
    where
        O: MachineStepObserver,
    {
        if let Some(snapshot) = self.cached_ppu_bus_state_snapshot {
            return snapshot;
        }

        let snapshot = self
            .ppu
            .bus_state_snapshot_with_observer(observer, records_ppu_regions);
        self.cached_ppu_bus_state_snapshot = Some(snapshot);
        snapshot
    }

    fn cpu_bus_arbitration_states(&mut self) -> CpuBusArbitrationStates {
        if let Some(states) = self.cached_cpu_bus_arbitration_states {
            return states;
        }

        let ppu_bus_states = self.ppu_bus_state_snapshot();
        let pre_cpu = BusArbitrationState::default()
            .with_boot_rom(self.boot.bus_state())
            .with_ppu(ppu_bus_states.owner)
            .with_dma(self.dma.bus_state());
        let states = CpuBusArbitrationStates {
            pre_cpu,
            cpu_read: pre_cpu.with_ppu(ppu_bus_states.cpu_read),
            cpu_write: pre_cpu.with_ppu(ppu_bus_states.cpu_write),
        };
        self.cached_cpu_bus_arbitration_states = Some(states);
        states
    }

    fn cpu_speed_switch_pause_active(&self) -> bool {
        matches!(
            self.cpu.execution_state(),
            CpuExecutionState::SpeedSwitchPause { .. }
        )
    }

    fn cpu_stop_active(&self) -> bool {
        matches!(
            self.cpu.execution_state(),
            CpuExecutionState::Stopped
                | CpuExecutionState::ZombieStopped
                | CpuExecutionState::SpeedSwitchPause { .. }
        )
    }
}

impl<S: TraceSink> Machine<S> {
    pub(crate) fn step_phase_with_context<O>(
        &mut self,
        context: &mut CycleContext,
        observer: &mut O,
    ) where
        O: MachineStepObserver,
    {
        let tracer = &mut self.tracer;
        tracer.emit_with(TraceSubsystem::Scheduler, TraceLevel::Trace, || {
            scheduler_phase_trace_message(context)
        });

        let mut runner = MachinePhaseRunner {
            config: &mut self.config,
            cpu: &mut self.cpu,
            bus: &mut self.bus,
            apu: &mut self.apu,
            ppu: &mut self.ppu,
            dma: &mut self.dma,
            timer: &mut self.timer,
            serial: &mut self.serial,
            speed: &mut self.speed,
            external_port: &mut self.external_port,
            boot: &mut self.boot,
            interrupts: &mut self.interrupts,
            joypad: &mut self.joypad,
            cartridge: &mut self.cartridge,
            pending_external_events: &mut self.pending_external_events,
            pending_ppu_mmio_write: &mut self.pending_ppu_mmio_write,
            cached_ppu_bus_state_snapshot: None,
            cached_cpu_bus_arbitration_states: None,
        };

        runner.step_phase(context, tracer, observer);
    }

    pub(crate) fn sync_scheduler_next_t_cycle(&mut self, next_t_cycle: crate::scheduler::TCycle) {
        self.scheduler.set_next_t_cycle(next_t_cycle);
    }

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
            config: &mut self.config,
            cpu: &mut self.cpu,
            bus: &mut self.bus,
            apu: &mut self.apu,
            ppu: &mut self.ppu,
            dma: &mut self.dma,
            timer: &mut self.timer,
            serial: &mut self.serial,
            speed: &mut self.speed,
            external_port: &mut self.external_port,
            boot: &mut self.boot,
            interrupts: &mut self.interrupts,
            joypad: &mut self.joypad,
            cartridge: &mut self.cartridge,
            pending_external_events: &mut self.pending_external_events,
            pending_ppu_mmio_write: &mut self.pending_ppu_mmio_write,
            cached_ppu_bus_state_snapshot: None,
            cached_cpu_bus_arbitration_states: None,
        };

        scheduler.step(|context| {
            tracer.emit_with(TraceSubsystem::Scheduler, TraceLevel::Trace, || {
                scheduler_phase_trace_message(context)
            });
            runner.step_phase(context, tracer, observer);
        })
    }
}
