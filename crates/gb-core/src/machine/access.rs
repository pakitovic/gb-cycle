use super::Machine;
use super::step::{
    PendingPpuMmioWrite, RealBootHandoffParts, commit_pending_ppu_mmio_write,
    cpu_write_targets_ppu_mmio, finalize_cgb_real_boot_handoff_if_needed,
};
use crate::apu::Apu;
use crate::boot::BootController;
use crate::bus::{BusArbitrationState, BusIoReadView, BusIoWriteView, BusRequester};
use crate::cartridge::{CartridgeDiagnostic, CartridgeLoadError};
use crate::cpu::{CpuAddressEvent, CpuAddressEventKind, CpuCore};
use crate::debugger::TraceSink;
use crate::dma::DmaController;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::model::OperatingMode;
use crate::ppu::Ppu;
use crate::scheduler::{CycleContext, SchedulerPhase, TCycle};
use crate::serial::Serial;
use crate::sgb::SgbHost;
use crate::speed::SpeedController;
use crate::timer::{Timer, TimerStartupState};

impl<S: TraceSink> Machine<S> {
    pub fn read_bus(&mut self, address: u16) -> u8 {
        let state = self.current_bus_arbitration_state();
        let value = self.bus.read_with_t_cycle_context(
            address,
            BusRequester::Cpu,
            &state,
            self.next_t_cycle(),
            Some(&mut self.cartridge),
            BusIoReadView {
                apu: Some(&self.apu),
                timer: Some(&self.timer),
                serial: Some(&self.serial),
                dma: Some(&self.dma),
                boot: Some(&self.boot),
                interrupts: Some(&self.interrupts),
                interrupt_flag_pending_mask: 0,
                joypad: Some(&self.joypad),
                ppu: Some(&self.ppu),
                speed: Some(&self.speed),
                ppu_cpu_visible_read: false,
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

        if address == 0xFF00 {
            self.sgb_host.observe_joyp_write(value);
        }

        if cpu_write_targets_ppu_mmio(&self.bus, address) {
            let mut pending = Some(PendingPpuMmioWrite { address, value });

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
            let _ = commit_pending_ppu_mmio_write(&mut self.ppu, &mut pending);
            return;
        }

        let mut boot_rom_newly_unmapped = false;
        self.bus.write_with_t_cycle_context(
            address,
            value,
            BusRequester::Cpu,
            &state,
            self.next_t_cycle(),
            Some(&mut self.cartridge),
            BusIoWriteView {
                apu: Some(&mut self.apu),
                timer: Some(&mut self.timer),
                serial: Some(&mut self.serial),
                dma: Some(&mut self.dma),
                boot: Some(&mut self.boot),
                interrupts: Some(&mut self.interrupts),
                joypad: Some(&mut self.joypad),
                ppu: Some(&mut self.ppu),
                speed: Some(&mut self.speed),
                boot_ff50_newly_unmapped: Some(&mut boot_rom_newly_unmapped),
            },
        );
        finalize_cgb_real_boot_handoff_if_needed(
            RealBootHandoffParts {
                config: &mut self.config,
                bus: &mut self.bus,
                ppu: &mut self.ppu,
                serial: &mut self.serial,
                speed: &mut self.speed,
                timer: &mut self.timer,
                cartridge: &self.cartridge,
                boot: &self.boot,
            },
            boot_rom_newly_unmapped,
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

    /// Applies an explicit timer startup profile after machine construction or cartridge-load reset.
    ///
    /// This is intentionally narrower than a full machine startup state and is meant for deterministic ROM-runner profiles that need to isolate a specific test-ROM assumption without changing the core `SkipBoot` contract.
    pub fn apply_timer_startup_state(&mut self, startup_state: TimerStartupState) {
        self.timer.apply_startup_state(startup_state);
        self.apu
            .apply_div_apu_startup_phase_from_system_counter(startup_state.system_counter);
    }

    pub fn load_cartridge(
        &mut self,
        rom_bytes: Vec<u8>,
    ) -> Result<Vec<CartridgeDiagnostic>, CartridgeLoadError> {
        let report = crate::cartridge::CartridgeSlot::load(rom_bytes, &self.config.compatibility)?;
        let (cartridge, diagnostics) = report.into_parts();
        let external_port = self.external_port.clone();
        let host_joypad_pressed_mask = self.pending_external_events.joypad_pressed_mask();
        self.cartridge = cartridge;
        self.config
            .apply_direct_boot_cartridge_header(self.cartridge.header());
        self.restart_runtime_after_cartridge_load(host_joypad_pressed_mask, external_port);
        Ok(diagnostics)
    }

    pub fn next_t_cycle(&self) -> TCycle {
        self.scheduler.next_t_cycle()
    }

    pub fn post_step_debug_trace_line(&self) -> String {
        let completed_t_cycle = self.scheduler.next_t_cycle().get().saturating_sub(1);
        let mut context = CycleContext::for_cycle(TCycle::new(completed_t_cycle));
        context.enter_phase(SchedulerPhase::CpuWakeInterruptEvaluation);
        let arbitration_state = self.current_bus_arbitration_state();

        format!(
            "cpu: {} | apu: {} | interrupts: {} | joypad: {} | bus: {}",
            self.cpu.scheduler_trace_message(&context),
            self.apu.scheduler_trace_message(&context),
            self.interrupts.scheduler_trace_message(&context),
            self.joypad.scheduler_trace_message(&context),
            self.bus
                .scheduler_trace_message(&context, &arbitration_state),
        )
    }

    pub(super) fn current_bus_arbitration_state(&self) -> BusArbitrationState {
        BusArbitrationState::default()
            .with_boot_rom(self.boot.bus_state())
            .with_ppu(self.ppu.bus_state())
            .with_dma(self.dma.bus_state())
    }

    fn restart_runtime_after_cartridge_load(
        &mut self,
        host_joypad_pressed_mask: u8,
        external_port: crate::external_port::ExternalPort,
    ) {
        let console_model = self.config.console_model;
        let operating_mode = self.config.operating_mode;
        let revision = self.config.revision;
        let startup_mode = self.config.startup_mode;
        let host_platform = self.config.host_platform;
        let boot_rom_assets = self.config.boot_rom_assets.clone();

        self.scheduler.reset();
        self.tracer.reset();
        self.cpu = CpuCore::new(console_model);
        self.bus = crate::bus::Bus::new_with_operating_mode(console_model, operating_mode);
        self.apu = Apu::new_with_revision(console_model, revision);
        self.ppu = Ppu::new(console_model);
        self.dma = DmaController::new(console_model);
        self.timer = Timer::new(console_model);
        self.serial = Serial::new_with_operating_mode(console_model, operating_mode);
        self.sgb_host = SgbHost::new_with_startup(host_platform, startup_mode);
        self.speed = SpeedController::new(console_model, operating_mode);
        self.external_port = external_port;
        self.boot = BootController::new(console_model, revision, startup_mode, boot_rom_assets);
        self.interrupts = InterruptController::new(console_model);
        self.joypad = Joypad::new(console_model);
        self.pending_ppu_mmio_write = None;

        self.apply_startup_configuration(host_joypad_pressed_mask);
    }

    pub(super) fn apply_startup_configuration(&mut self, host_joypad_pressed_mask: u8) {
        if let Some(mut startup_state) = self.boot.machine_skip_boot_state(Some(&self.cartridge)) {
            if self.config.operating_mode == OperatingMode::CgbDmgExt {
                // DocBoy's CGB DMG-ext direct-boot profile starts with the boot-logo pulse residue inactive while keeping the ordinary powered APU register image. Keep this narrow to the experimental mode so promoted CGB startup audio state stays unchanged.
                startup_state.apu.channel_active_mask = 0;
            }
            self.cpu.apply_startup_state(startup_state.cpu);
            self.apu.apply_startup_state(startup_state.apu);
            self.ppu.apply_startup_state(startup_state.ppu);
            if self.config.startup_mode == crate::model::StartupMode::CustomBoot {
                // DMG-family CustomBoot exposes reset-facing PPU publication only through CPU-bus reads; ordinary SkipBoot keeps the plain cartridge-entry HWIO snapshot used by Mooneye boot_hwio.
                self.ppu.apply_dmg_skip_boot_stat_irq_startup_phase();
            }
            if self.config.console_model.is_cgb_family()
                && self.config.startup_mode == crate::model::StartupMode::CustomBoot
            {
                self.ppu.apply_cgb_custom_boot_entry_phase();
            }
            self.timer.apply_startup_state(startup_state.timer);
            self.serial.apply_startup_state(startup_state.serial);
            self.dma.apply_startup_state(startup_state.dma);
            self.interrupts
                .apply_startup_state(startup_state.interrupts);
            self.joypad.apply_startup_state(startup_state.joypad);
            self.bus
                .apply_startup_memory_policy(startup_state.startup_memory_policy);
        }
        if let Some(startup_state) = self.boot.real_boot_power_on_state() {
            self.ppu.apply_dmg_real_boot_power_on_lcd_enable_phase();
            self.timer.apply_startup_state(startup_state.timer);
            self.serial.apply_startup_state(startup_state.serial);
            self.dma.apply_startup_state(startup_state.dma);
            self.joypad.apply_startup_state(startup_state.joypad);
        }
        self.ppu
            .apply_operating_mode_state(self.config.operating_mode);
        self.serial
            .apply_operating_mode_state(self.config.operating_mode);
        self.ppu.apply_cgb_compatibility_palette_startup_state(
            self.config.startup_mode,
            self.config.operating_mode,
            self.cartridge.header(),
            host_joypad_pressed_mask,
        );
        self.ppu.apply_cgb_native_palette_startup_state(
            self.config.startup_mode,
            self.config.operating_mode,
        );
        self.bus
            .apply_cgb_startup_state(self.config.startup_mode, self.cartridge.header());
        self.sgb_host
            .apply_cartridge_header(self.cartridge.header());
        self.external_port.apply_startup_reset();
        self.sync_serial_peer_from_external_port();
        self.pending_external_events
            .reset_for_startup(host_joypad_pressed_mask, self.joypad.pressed_mask());
    }
}
