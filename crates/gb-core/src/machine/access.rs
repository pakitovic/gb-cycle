use super::Machine;
use crate::bus::{BusArbitrationState, BusIoReadView, BusIoWriteView, BusRequester};
use crate::cartridge::{CartridgeDiagnostic, CartridgeLoadError};
use crate::cpu::{CpuAddressEvent, CpuAddressEventKind};
use crate::debugger::TraceSink;
use crate::scheduler::TCycle;

impl<S: TraceSink> Machine<S> {
    pub fn read_bus(&mut self, address: u16) -> u8 {
        let state = self.current_bus_arbitration_state();
        let value = self.bus.read_with_context(
            address,
            BusRequester::Cpu,
            &state,
            Some(&self.cartridge),
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

        self.bus.write_with_context(
            address,
            value,
            BusRequester::Cpu,
            &state,
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
        let report = crate::cartridge::CartridgeSlot::load(rom_bytes, &self.config.compatibility)?;
        let (cartridge, diagnostics) = report.into_parts();
        self.cartridge = cartridge;
        self.apply_startup_configuration();
        Ok(diagnostics)
    }

    pub fn next_t_cycle(&self) -> TCycle {
        self.scheduler.next_t_cycle()
    }

    fn current_bus_arbitration_state(&self) -> BusArbitrationState {
        BusArbitrationState::default()
            .with_boot_rom(self.boot.bus_state())
            .with_ppu(self.ppu.bus_state())
            .with_dma(self.dma.bus_state())
    }

    pub(super) fn apply_startup_configuration(&mut self) {
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
}
