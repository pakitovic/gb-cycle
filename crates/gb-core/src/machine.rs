mod access;
mod step;

use crate::apu::Apu;
use crate::boot::BootController;
use crate::bus::Bus;
use crate::cartridge::{
    CartridgePersistentStateError, CartridgeSlot, Mbc7AccelerometerError, Mbc7AccelerometerInput,
    PersistentCartState, PocketCameraFrame, PocketCameraFrameError,
};
use crate::cpu::{CpuCore, CpuExecutionState};
use crate::debugger::{
    DebugControl, MachineSnapshot, TraceBuffer, TraceSink, TraceSnapshotProvider,
    TraceSummaryBuffer, Tracer,
};
use crate::dma::DmaController;
use crate::external_port::{ExternalPort, ExternalPortAttachmentKind, ExternalPortResetPolicy};
use crate::interrupts::InterruptController;
use crate::joypad::{Joypad, JoypadButton, button_mask};
use crate::link::Dmg07Port;
use crate::model::MachineConfig;
use crate::ppu::{Ppu, PpuStepObserver, PpuStepRegion};
use crate::save_state::{
    MachineBootSaveStateMetadata, MachineCartridgeSaveStateMetadata, MachineCoreSaveState,
    MachineRuntimeSaveState, MachineSaveState, MachineSaveStateMetadata,
    MachineSaveStateRestoreError, SchedulerSaveState,
};
use crate::scheduler::GlobalScheduler;
use crate::serial::{Serial, SerialClockMode, SerialTickTelemetry, SerialTransferState};
use crate::speed::SpeedController;
use crate::timer::Timer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PendingExternalEvents {
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

    fn has_pending_work(&self) -> bool {
        self.joypad_state_dirty || self.external_serial_clock_pulses_pending != 0
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct Dmg04EndpointState {
    pub attached: bool,
    pub active_transfer: bool,
    pub staged_outgoing_byte: u8,
    pub waiting_for_external_clock: bool,
    pub internal_clock_edge_pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct Dmg07EndpointState {
    pub attached: bool,
    pub port: Option<Dmg07Port>,
    pub active_transfer: bool,
    pub staged_outgoing_byte: u8,
    pub waiting_for_external_clock: bool,
    pub using_internal_clock: bool,
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
    speed: SpeedController,
    external_port: ExternalPort,
    boot: BootController,
    interrupts: InterruptController,
    joypad: Joypad,
    cartridge: CartridgeSlot,
    pending_external_events: PendingExternalEvents,
    pending_ppu_mmio_write: Option<step::PendingPpuMmioWrite>,
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
    pub speed: SpeedController,
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
    fn records_regions(&self) -> bool {
        true
    }

    fn records_ppu_regions(&self) -> bool {
        self.records_regions()
    }

    fn begin_region(&mut self, _region: MachineStepRegion) {}

    fn end_region(&mut self, _region: MachineStepRegion) {}

    fn begin_ppu_region(&mut self, _region: PpuStepRegion) {}

    fn end_ppu_region(&mut self, _region: PpuStepRegion) {}

    fn record_serial_tick(&mut self, _telemetry: SerialTickTelemetry) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NoopMachineStepObserver;

impl MachineStepObserver for NoopMachineStepObserver {
    fn records_regions(&self) -> bool {
        false
    }
}

impl<T> PpuStepObserver for T
where
    T: MachineStepObserver,
{
    fn records_ppu_regions(&self) -> bool {
        MachineStepObserver::records_ppu_regions(self)
    }

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
            speed: self.speed.snapshot(),
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
        let operating_mode = config.operating_mode;
        let startup_mode = config.startup_mode;
        let boot_rom_kind = config.boot_rom_kind;
        let boot_rom_assets = config.boot_rom_assets.clone();

        let mut machine = Self {
            config,
            scheduler: GlobalScheduler::new(),
            tracer,
            debug_controls: DebugControl::new(),
            cpu: CpuCore::new(console_model),
            bus: Bus::new_with_operating_mode(console_model, operating_mode),
            apu: Apu::new(console_model),
            ppu: Ppu::new(console_model),
            dma: DmaController::new(console_model),
            timer: Timer::new(console_model),
            serial: Serial::new_with_operating_mode(console_model, operating_mode),
            speed: SpeedController::new(console_model, operating_mode),
            external_port: ExternalPort::new(),
            boot: BootController::new(console_model, startup_mode, boot_rom_kind, boot_rom_assets),
            interrupts: InterruptController::new(console_model),
            joypad: Joypad::new(console_model),
            cartridge: CartridgeSlot::empty(),
            pending_external_events: PendingExternalEvents::default(),
            pending_ppu_mmio_write: None,
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

    /// Returns raw VRAM backing bytes for deterministic debug probes.
    ///
    /// This bypasses CPU bus visibility rules and must be used only by tooling that needs non-perturbing state comparison.
    pub fn debug_vram_bytes(&self) -> &[u8] {
        self.bus.debug_vram_bytes()
    }

    /// Returns raw OAM backing bytes for deterministic debug probes.
    ///
    /// This bypasses CPU bus visibility rules and must be used only by tooling that needs non-perturbing state comparison.
    pub fn debug_oam_bytes(&self) -> &[u8] {
        self.bus.debug_oam_bytes()
    }

    /// Returns raw WRAM backing bytes for deterministic debug probes.
    ///
    /// This bypasses CPU bus side effects and must be used only by tooling that needs direct storage state.
    pub fn debug_wram_bytes(&self) -> &[u8] {
        self.bus.debug_wram_bytes()
    }

    /// Returns raw HRAM backing bytes for deterministic debug probes.
    ///
    /// This excludes MMIO and IE; those live in subsystem state.
    pub fn debug_hram_bytes(&self) -> &[u8] {
        self.bus.debug_hram_bytes()
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

    pub fn speed(&self) -> &SpeedController {
        &self.speed
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

    pub fn set_external_port_reset_policy(&mut self, reset_policy: ExternalPortResetPolicy) {
        self.external_port.set_reset_policy(reset_policy);
    }

    pub fn queue_external_serial_clock(&mut self) {
        self.pending_external_events.queue_external_serial_clock();
    }

    pub fn take_serial_output_bytes(&mut self) -> Vec<u8> {
        self.serial.take_completed_output_bytes()
    }

    pub(crate) fn latest_completed_serial_output_byte(&self) -> Option<u8> {
        self.serial.latest_completed_output_byte()
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

    pub fn has_pocket_camera(&self) -> bool {
        self.cartridge.has_pocket_camera()
    }

    pub fn set_pocket_camera_frame(
        &mut self,
        frame: PocketCameraFrame,
    ) -> Result<(), PocketCameraFrameError> {
        self.cartridge.set_pocket_camera_frame(frame)
    }

    pub fn clear_pocket_camera_frame(&mut self) -> Result<(), PocketCameraFrameError> {
        self.cartridge.clear_pocket_camera_frame()
    }

    pub fn has_mbc7_accelerometer(&self) -> bool {
        self.cartridge.has_mbc7_accelerometer()
    }

    pub fn set_mbc7_accelerometer_input(
        &mut self,
        input: Mbc7AccelerometerInput,
    ) -> Result<(), Mbc7AccelerometerError> {
        self.cartridge.set_mbc7_accelerometer_input(input)
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

    pub fn advance_mbc3_cartridge_rtc_clock_ticks(&mut self, ticks: u64) {
        self.cartridge.advance_mbc3_rtc_clock_ticks(ticks);
    }

    pub fn capture_save_state(&self) -> MachineSaveState {
        MachineSaveState::new(
            self.save_state_metadata(),
            MachineCoreSaveState {
                scheduler: SchedulerSaveState {
                    next_t_cycle: self.scheduler.next_t_cycle(),
                },
                machine: MachineRuntimeSaveState {
                    joypad_pressed_mask: self.pending_external_events.joypad_pressed_mask,
                    joypad_state_dirty: self.pending_external_events.joypad_state_dirty,
                    external_serial_clock_pulses_pending: self
                        .pending_external_events
                        .external_serial_clock_pulses_pending,
                },
                cpu: self.cpu.capture_save_state(),
                bus: self.bus.capture_save_state(),
                apu: self.apu.capture_save_state(),
                ppu: self.ppu.capture_save_state(),
                dma: self.dma.capture_save_state(),
                timer: self.timer.capture_save_state(),
                serial: self.serial.capture_save_state(),
                speed: self.speed.capture_save_state(),
                external_port: self.external_port.capture_save_state(),
                boot: self.boot.capture_save_state(),
                interrupts: self.interrupts.capture_save_state(),
                joypad: self.joypad.capture_save_state(),
                cartridge: self.cartridge.capture_save_state(),
            },
        )
    }

    pub fn restore_save_state(
        &mut self,
        state: &MachineSaveState,
    ) -> Result<(), MachineSaveStateRestoreError> {
        self.validate_save_state_metadata(state.metadata())?;

        let core = state.core();
        self.cartridge.validate_save_state(&core.cartridge)?;
        self.scheduler.set_next_t_cycle(core.scheduler.next_t_cycle);
        self.cpu.restore_save_state(&core.cpu);
        self.bus.restore_save_state(&core.bus);
        self.apu.restore_save_state(&core.apu);
        self.ppu.restore_save_state(&core.ppu);
        self.dma.restore_save_state(&core.dma);
        self.timer.restore_save_state(&core.timer);
        self.serial.restore_save_state(&core.serial);
        self.speed.restore_save_state(&core.speed);
        self.external_port.restore_save_state(&core.external_port);
        self.boot.restore_save_state(&core.boot);
        self.interrupts.restore_save_state(&core.interrupts);
        self.joypad.restore_save_state(&core.joypad);
        self.cartridge.restore_save_state(&core.cartridge);
        self.pending_external_events = PendingExternalEvents {
            joypad_pressed_mask: core.machine.joypad_pressed_mask,
            joypad_state_dirty: core.machine.joypad_state_dirty,
            external_serial_clock_pulses_pending: core.machine.external_serial_clock_pulses_pending,
        };
        self.pending_ppu_mmio_write = None;
        self.sync_serial_peer_from_external_port();
        Ok(())
    }

    fn save_state_metadata(&self) -> MachineSaveStateMetadata {
        MachineSaveStateMetadata {
            console_model: self.config.console_model,
            operating_mode: self.config.operating_mode,
            host_platform: self.config.host_platform,
            startup_mode: self.config.startup_mode,
            compatibility: self.config.compatibility.clone(),
            next_t_cycle: self.scheduler.next_t_cycle(),
            cartridge: self.cartridge_save_state_metadata(),
            boot: self.boot_save_state_metadata(),
        }
    }

    fn cartridge_save_state_metadata(&self) -> MachineCartridgeSaveStateMetadata {
        MachineCartridgeSaveStateMetadata {
            state: self.cartridge.state(),
            rom_fingerprint: self.cartridge.rom_fingerprint(),
        }
    }

    fn boot_save_state_metadata(&self) -> MachineBootSaveStateMetadata {
        MachineBootSaveStateMetadata {
            startup_mode: self.boot.startup_mode(),
            boot_rom_kind: self.boot.boot_rom_kind(),
            boot_rom_mapped: self.boot.is_boot_rom_mapped(),
            boot_rom_fingerprint: self.boot.boot_rom_fingerprint(),
        }
    }

    fn validate_save_state_metadata(
        &self,
        metadata: &MachineSaveStateMetadata,
    ) -> Result<(), MachineSaveStateRestoreError> {
        if metadata.console_model != self.config.console_model {
            return Err(MachineSaveStateRestoreError::ConsoleModelMismatch {
                expected: metadata.console_model,
                actual: self.config.console_model,
            });
        }
        if metadata.operating_mode != self.config.operating_mode {
            return Err(MachineSaveStateRestoreError::OperatingModeMismatch {
                expected: metadata.operating_mode,
                actual: self.config.operating_mode,
            });
        }
        if metadata.host_platform != self.config.host_platform {
            return Err(MachineSaveStateRestoreError::HostPlatformMismatch {
                expected: metadata.host_platform,
                actual: self.config.host_platform,
            });
        }
        if metadata.startup_mode != self.config.startup_mode {
            return Err(MachineSaveStateRestoreError::StartupModeMismatch {
                expected: metadata.startup_mode,
                actual: self.config.startup_mode,
            });
        }
        if metadata.compatibility != self.config.compatibility {
            return Err(MachineSaveStateRestoreError::CompatibilityMismatch);
        }

        let actual_cartridge = self.cartridge_save_state_metadata();
        if metadata.cartridge != actual_cartridge {
            return Err(MachineSaveStateRestoreError::CartridgeMismatch {
                expected: metadata.cartridge.clone(),
                actual: actual_cartridge,
            });
        }

        let actual_boot = self.boot_save_state_metadata();
        if !boot_save_state_metadata_is_compatible(&metadata.boot, &actual_boot) {
            return Err(MachineSaveStateRestoreError::BootRomMismatch {
                expected: metadata.boot.clone(),
                actual: actual_boot,
            });
        }

        Ok(())
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
            speed: self.speed,
            external_port: self.external_port,
            boot: self.boot,
            interrupts: self.interrupts,
            joypad: self.joypad,
            cartridge: self.cartridge,
        }
    }

    pub(crate) fn dmg04_endpoint_state(&self) -> Dmg04EndpointState {
        let attached =
            self.external_port.attachment_kind() == ExternalPortAttachmentKind::GameLinkDmg04;
        let transfer_requested = matches!(
            self.serial.transfer_state(),
            SerialTransferState::TransferRequested { .. }
        );
        let active_transfer = attached && transfer_requested && !self.cpu_stop_active();

        Dmg04EndpointState {
            attached,
            active_transfer,
            staged_outgoing_byte: self.serial.endpoint_outgoing_byte(),
            waiting_for_external_clock: active_transfer
                && self.serial.clock_mode() == SerialClockMode::External,
            internal_clock_edge_pending: active_transfer
                && self.serial.clock_mode() == SerialClockMode::Internal
                && self
                    .serial
                    .internal_clock_edge_pending_this_t_cycle_for_speed(self.speed.current_speed()),
        }
    }

    pub(crate) fn set_dmg04_incoming_byte(&mut self, incoming_byte: Option<u8>) {
        self.external_port.set_dmg04_incoming_byte(incoming_byte);
        self.sync_serial_peer_from_external_port();
    }

    pub(crate) fn set_dmg07_attachment(&mut self, port: Dmg07Port) {
        self.external_port.set_dmg07_attachment(port);
        self.sync_serial_peer_from_external_port();
    }

    pub(crate) fn dmg07_endpoint_state(&self) -> Dmg07EndpointState {
        let port = self.external_port.dmg07_port();
        let attached = port.is_some();
        let transfer_requested = matches!(
            self.serial.transfer_state(),
            SerialTransferState::TransferRequested { .. }
        );
        let active_transfer = attached && transfer_requested && !self.cpu_stop_active();

        Dmg07EndpointState {
            attached,
            port,
            active_transfer,
            staged_outgoing_byte: self.serial.endpoint_outgoing_byte(),
            waiting_for_external_clock: active_transfer
                && self.serial.clock_mode() == SerialClockMode::External,
            using_internal_clock: active_transfer
                && self.serial.clock_mode() == SerialClockMode::Internal,
        }
    }

    pub(crate) fn set_dmg07_incoming_byte(&mut self, incoming_byte: Option<u8>) {
        self.external_port.set_dmg07_incoming_byte(incoming_byte);
        self.sync_serial_peer_from_external_port();
    }

    pub(super) fn sync_serial_peer_from_external_port(&mut self) {
        self.serial.set_peer(self.external_port.serial_peer());
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

fn boot_save_state_metadata_is_compatible(
    expected: &MachineBootSaveStateMetadata,
    actual: &MachineBootSaveStateMetadata,
) -> bool {
    expected.startup_mode == actual.startup_mode
        && expected.boot_rom_kind == actual.boot_rom_kind
        && expected.boot_rom_fingerprint == actual.boot_rom_fingerprint
}

#[cfg(test)]
mod tests;
