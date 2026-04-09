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
    IoRegisterImplementation, IoRegisterKind, IoRegisterOwner,
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
        policy.initialize_hram(&mut self.hram);
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

        if !io_register_is_available(info.availability(), console_model)
            || info.implementation() != IoRegisterImplementation::Implemented
        {
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
            IoRegisterKind::OamDma => io.dma.map_or(BLOCKED_READ_VALUE, DmaController::read_ff46),
            IoRegisterKind::BootRomDisable => io
                .boot
                .map_or(BLOCKED_READ_VALUE, BootController::read_ff50),
            IoRegisterKind::InterruptEnable => io
                .interrupts
                .map_or(BLOCKED_READ_VALUE, InterruptController::read_ie),
            _ => match info.owner() {
                IoRegisterOwner::Ppu => io
                    .ppu
                    .map_or(BLOCKED_READ_VALUE, |ppu| ppu.read_register(address)),
                IoRegisterOwner::Apu => io
                    .apu
                    .map_or(BLOCKED_READ_VALUE, |apu| apu.read_register(address)),
                IoRegisterOwner::MemoryController
                | IoRegisterOwner::Infrared
                | IoRegisterOwner::CgbSystem
                | IoRegisterOwner::Reserved => BLOCKED_READ_VALUE,
                _ => unreachable!("MMIO descriptor kind/owner mismatch for {address:#06X}"),
            },
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

        if !io_register_is_available(info.availability(), console_model)
            || info.implementation() != IoRegisterImplementation::Implemented
        {
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
                let BusIoWriteView { apu, timer, .. } = io;
                if let Some(timer) = timer {
                    let effects = timer.write_div_with_effects(value);
                    if effects.apu_frame_sequencer_edge
                        && let Some(apu) = apu
                    {
                        apu.on_div_apu_edge();
                    }
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
            _ => match info.owner() {
                IoRegisterOwner::Ppu => {
                    if let Some(ppu) = io.ppu {
                        ppu.write_register(address, value);
                    }
                }
                IoRegisterOwner::Apu => {
                    let BusIoWriteView { apu, .. } = io;
                    if let Some(apu) = apu {
                        apu.write_register(address, value);
                    }
                }
                IoRegisterOwner::MemoryController
                | IoRegisterOwner::Infrared
                | IoRegisterOwner::CgbSystem
                | IoRegisterOwner::Reserved => {}
                _ => unreachable!("MMIO descriptor kind/owner mismatch for {address:#06X}"),
            },
        }
    }
}

fn io_register_is_available(
    availability: IoRegisterAvailability,
    console_model: ConsoleModel,
) -> bool {
    match availability {
        IoRegisterAvailability::Shared | IoRegisterAvailability::DmgCompatible => true,
        IoRegisterAvailability::CgbOnly => console_model.is_cgb_family(),
    }
}
