use crate::apu::Apu;
use crate::boot::{BootController, StartupMemoryPolicy};
use crate::dma::DmaController;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::model::ConsoleModel;
use crate::ppu::Ppu;
use crate::serial::Serial;
use crate::timer::Timer;

use super::{
    AddressRouter, BLOCKED_READ_VALUE, BusAddressInfo, BusRegion, HRAM_LEN, IoRegisterAvailability,
    IoRegisterKind,
};

#[derive(Default)]
pub(crate) struct BusIoReadView<'a> {
    pub apu: Option<&'a Apu>,
    pub timer: Option<&'a Timer>,
    pub serial: Option<&'a Serial>,
    pub dma: Option<&'a DmaController>,
    pub boot: Option<&'a BootController>,
    pub interrupts: Option<&'a InterruptController>,
    pub interrupt_flag_pending_mask: u8,
    pub joypad: Option<&'a Joypad>,
    pub ppu: Option<&'a Ppu>,
}

#[derive(Default)]
pub(crate) struct BusIoWriteView<'a> {
    pub apu: Option<&'a mut Apu>,
    pub timer: Option<&'a mut Timer>,
    pub serial: Option<&'a mut Serial>,
    pub dma: Option<&'a mut DmaController>,
    pub boot: Option<&'a mut BootController>,
    pub interrupts: Option<&'a mut InterruptController>,
    pub joypad: Option<&'a mut Joypad>,
    pub ppu: Option<&'a mut Ppu>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IoHramDomain {
    hram: [u8; HRAM_LEN],
}

impl IoHramDomain {
    pub(crate) fn new() -> Self {
        Self {
            hram: [0; HRAM_LEN],
        }
    }

    pub(crate) fn apply_startup_memory_policy(&mut self, policy: StartupMemoryPolicy) {
        match policy {
            StartupMemoryPolicy::DeterministicZeroed => self.hram.fill(0),
        }
    }

    pub(crate) fn read(
        &self,
        router: &AddressRouter,
        console_model: ConsoleModel,
        target: BusAddressInfo,
        io: BusIoReadView<'_>,
    ) -> u8 {
        match target.region() {
            BusRegion::Hram => self.hram[target.region_offset() as usize],
            BusRegion::Mmio | BusRegion::InterruptEnable => {
                self.read_io_target(router, console_model, target.address(), io)
            }
            _ => unreachable!("non-IoHram target routed to IoHramDomain"),
        }
    }

    pub(crate) fn write(
        &mut self,
        router: &AddressRouter,
        console_model: ConsoleModel,
        target: BusAddressInfo,
        value: u8,
        io: BusIoWriteView<'_>,
    ) {
        match target.region() {
            BusRegion::Hram => {
                self.hram[target.region_offset() as usize] = value;
            }
            BusRegion::Mmio | BusRegion::InterruptEnable => {
                self.write_io_target(router, console_model, target.address(), value, io)
            }
            _ => unreachable!("non-IoHram target routed to IoHramDomain"),
        }
    }

    fn read_io_target(
        &self,
        router: &AddressRouter,
        console_model: ConsoleModel,
        address: u16,
        io: BusIoReadView<'_>,
    ) -> u8 {
        let Some(info) = router.describe_io_register(address) else {
            return BLOCKED_READ_VALUE;
        };

        if info.availability() == IoRegisterAvailability::CgbOnly && console_model.is_dmg_family() {
            return BLOCKED_READ_VALUE;
        }

        match info.kind() {
            IoRegisterKind::Joyp => io.joypad.map_or(BLOCKED_READ_VALUE, Joypad::read_p1),
            IoRegisterKind::SerialData => io.serial.map_or(BLOCKED_READ_VALUE, Serial::read_sb),
            IoRegisterKind::SerialControl => io.serial.map_or(BLOCKED_READ_VALUE, Serial::read_sc),
            IoRegisterKind::Div => io.timer.map_or(BLOCKED_READ_VALUE, Timer::read_div),
            IoRegisterKind::Tima => io.timer.map_or(BLOCKED_READ_VALUE, Timer::read_tima),
            IoRegisterKind::Tma => io.timer.map_or(BLOCKED_READ_VALUE, Timer::read_tma),
            IoRegisterKind::Tac => io.timer.map_or(BLOCKED_READ_VALUE, Timer::read_tac),
            IoRegisterKind::InterruptFlag => {
                io.interrupts.map_or(BLOCKED_READ_VALUE, |interrupts| {
                    interrupts.read_if_with_pending_requests(io.interrupt_flag_pending_mask)
                })
            }
            IoRegisterKind::Lcd => io
                .ppu
                .map_or(BLOCKED_READ_VALUE, |ppu| ppu.read_register(address)),
            IoRegisterKind::OamDma => io.dma.map_or(BLOCKED_READ_VALUE, DmaController::read_ff46),
            IoRegisterKind::BootRomDisable => io
                .boot
                .map_or(BLOCKED_READ_VALUE, BootController::read_ff50),
            IoRegisterKind::InterruptEnable => io
                .interrupts
                .map_or(BLOCKED_READ_VALUE, InterruptController::read_ie),
            IoRegisterKind::Sound => io
                .apu
                .map_or(BLOCKED_READ_VALUE, |apu| apu.read_register(address)),
            IoRegisterKind::CgbSystem | IoRegisterKind::Reserved => BLOCKED_READ_VALUE,
        }
    }

    fn write_io_target(
        &mut self,
        router: &AddressRouter,
        console_model: ConsoleModel,
        address: u16,
        value: u8,
        io: BusIoWriteView<'_>,
    ) {
        let Some(info) = router.describe_io_register(address) else {
            return;
        };

        if info.availability() == IoRegisterAvailability::CgbOnly && console_model.is_dmg_family() {
            return;
        }

        match info.kind() {
            IoRegisterKind::Joyp => {
                if let Some(joypad) = io.joypad {
                    joypad.write_p1(value);
                }
            }
            IoRegisterKind::SerialData => {
                if let Some(serial) = io.serial {
                    serial.write_sb(value);
                }
            }
            IoRegisterKind::SerialControl => {
                if let Some(serial) = io.serial {
                    serial.write_sc(value);
                }
            }
            IoRegisterKind::Div => {
                if let Some(timer) = io.timer {
                    timer.write_div(value);
                }
            }
            IoRegisterKind::Tima => {
                if let Some(timer) = io.timer {
                    timer.write_tima(value);
                }
            }
            IoRegisterKind::Tma => {
                if let Some(timer) = io.timer {
                    timer.write_tma(value);
                }
            }
            IoRegisterKind::Tac => {
                if let Some(timer) = io.timer {
                    timer.write_tac(value);
                }
            }
            IoRegisterKind::InterruptFlag => {
                if let Some(interrupts) = io.interrupts {
                    interrupts.write_if(value);
                }
            }
            IoRegisterKind::Lcd => {
                if let Some(ppu) = io.ppu {
                    ppu.write_register(address, value);
                }
            }
            IoRegisterKind::OamDma => {
                if let Some(dma) = io.dma {
                    dma.write_ff46(value);
                }
            }
            IoRegisterKind::BootRomDisable => {
                if let Some(boot) = io.boot {
                    boot.write_ff50(value);
                }
            }
            IoRegisterKind::InterruptEnable => {
                if let Some(interrupts) = io.interrupts {
                    interrupts.write_ie(value);
                }
            }
            IoRegisterKind::Sound => {
                if let Some(apu) = io.apu {
                    apu.write_register(address, value);
                }
            }
            IoRegisterKind::CgbSystem | IoRegisterKind::Reserved => {}
        }
    }
}
