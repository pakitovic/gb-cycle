mod access;
mod step;

use crate::apu::Apu;
use crate::boot::BootController;
use crate::bus::Bus;
use crate::cartridge::{CartridgePersistentStateError, CartridgeSlot, PersistentCartState};
use crate::cpu::CpuCore;
use crate::debugger::{
    DebugControl, MachineSnapshot, TraceBuffer, TraceSink, TraceSnapshotProvider,
    TraceSummaryBuffer, Tracer,
};
use crate::dma::DmaController;
use crate::external_port::ExternalPort;
use crate::interrupts::InterruptController;
use crate::joypad::{Joypad, JoypadButton, button_mask};
use crate::model::MachineConfig;
use crate::ppu::{Ppu, PpuStepObserver, PpuStepRegion};
use crate::scheduler::GlobalScheduler;
use crate::serial::{Serial, SerialPeer};
use crate::timer::Timer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PendingExternalEvents {
    joypad_pressed_mask: u8,
    joypad_state_dirty: bool,
    external_serial_clock_pulses_pending: u8,
}

impl PendingExternalEvents {
    const fn new(joypad_pressed_mask: u8) -> Self {
        Self {
            joypad_pressed_mask,
            joypad_state_dirty: false,
            external_serial_clock_pulses_pending: 0,
        }
    }

    fn reset(&mut self, joypad_pressed_mask: u8) {
        *self = Self::new(joypad_pressed_mask);
    }

    fn reset_for_startup(&mut self, host_joypad_pressed_mask: u8, hardware_pressed_mask: u8) {
        self.reset(host_joypad_pressed_mask);
        self.joypad_state_dirty = host_joypad_pressed_mask != hardware_pressed_mask;
    }

    fn set_joypad_button_pressed(&mut self, button: JoypadButton, pressed: bool) {
        let bit = button_mask(button);
        let previous_mask = self.joypad_pressed_mask;

        if pressed {
            self.joypad_pressed_mask |= bit;
        } else {
            self.joypad_pressed_mask &= !bit;
        }

        if self.joypad_pressed_mask != previous_mask {
            self.joypad_state_dirty = true;
        }
    }

    fn take_pending_joypad_pressed_mask(&mut self) -> Option<u8> {
        if !self.joypad_state_dirty {
            return None;
        }

        self.joypad_state_dirty = false;
        Some(self.joypad_pressed_mask)
    }

    fn queue_external_serial_clock(&mut self) {
        self.external_serial_clock_pulses_pending =
            self.external_serial_clock_pulses_pending.saturating_add(1);
    }

    fn take_external_serial_clock_pulse(&mut self) -> bool {
        if self.external_serial_clock_pulses_pending == 0 {
            return false;
        }

        self.external_serial_clock_pulses_pending -= 1;
        true
    }

    fn joypad_pressed_mask(&self) -> u8 {
        self.joypad_pressed_mask
    }
}

impl Default for PendingExternalEvents {
    fn default() -> Self {
        Self::new(0)
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
    external_port: ExternalPort,
    boot: BootController,
    interrupts: InterruptController,
    joypad: Joypad,
    cartridge: CartridgeSlot,
    pending_external_events: PendingExternalEvents,
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
    pub external_port: ExternalPort,
    pub boot: BootController,
    pub interrupts: InterruptController,
    pub joypad: Joypad,
    pub cartridge: CartridgeSlot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineStepRegion {
    ExternalEvents,
    Timer,
    Apu,
    Dma,
    Ppu,
    Serial,
    Cpu,
    Interrupts,
}

pub trait MachineStepObserver {
    fn begin_region(&mut self, _region: MachineStepRegion) {}

    fn end_region(&mut self, _region: MachineStepRegion) {}

    fn begin_ppu_region(&mut self, _region: PpuStepRegion) {}

    fn end_ppu_region(&mut self, _region: PpuStepRegion) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NoopMachineStepObserver;

impl MachineStepObserver for NoopMachineStepObserver {}

impl<T> PpuStepObserver for T
where
    T: MachineStepObserver,
{
    fn begin_ppu_region(&mut self, region: PpuStepRegion) {
        MachineStepObserver::begin_ppu_region(self, region);
    }

    fn end_ppu_region(&mut self, region: PpuStepRegion) {
        MachineStepObserver::end_ppu_region(self, region);
    }
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
            bus: self.bus.snapshot(self.current_bus_arbitration_state()),
            apu: self.apu.snapshot(),
            ppu: self.ppu.snapshot(),
            dma: self.dma.snapshot(),
            timer: self.timer.snapshot(),
            serial: self.serial.snapshot(),
            external_port: self.external_port.snapshot(),
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
            external_port: ExternalPort::new(),
            boot: BootController::new(console_model, startup_mode, boot_rom_assets),
            interrupts: InterruptController::new(console_model),
            joypad: Joypad::new(console_model),
            cartridge: CartridgeSlot::empty(),
            pending_external_events: PendingExternalEvents::default(),
        };

        machine.apply_startup_configuration(0);
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

    pub fn external_port(&self) -> &ExternalPort {
        &self.external_port
    }

    pub fn set_external_port_attachment(
        &mut self,
        attachment_kind: crate::external_port::ExternalPortAttachmentKind,
    ) {
        self.external_port.set_attachment_kind(attachment_kind);
        self.sync_serial_peer_from_external_port();
    }

    pub fn set_serial_peer(&mut self, peer: SerialPeer) {
        let attachment_kind = match peer {
            SerialPeer::Disconnected => crate::external_port::ExternalPortAttachmentKind::None,
            SerialPeer::Loopback => crate::external_port::ExternalPortAttachmentKind::Loopback,
            SerialPeer::StagedIncomingByte { .. } => {
                crate::external_port::ExternalPortAttachmentKind::None
            }
        };
        self.set_external_port_attachment(attachment_kind);
    }

    pub fn queue_external_serial_clock(&mut self) {
        self.pending_external_events.queue_external_serial_clock();
    }

    pub fn take_serial_output_bytes(&mut self) -> Vec<u8> {
        self.serial.take_completed_output_bytes()
    }

    pub fn take_printed_pages(&mut self) -> Vec<crate::external_port::PrintedPage> {
        self.external_port.take_printed_pages()
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
        self.pending_external_events
            .set_joypad_button_pressed(button, pressed);
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
            external_port: self.external_port,
            boot: self.boot,
            interrupts: self.interrupts,
            joypad: self.joypad,
            cartridge: self.cartridge,
        }
    }

    pub(super) fn sync_serial_peer_from_external_port(&mut self) {
        self.serial.set_peer(self.external_port.serial_peer());
    }
}

#[cfg(test)]
mod tests;
