use crate::apu::Apu;
use crate::boot::{BootController, StartupMemoryPolicy};
use crate::cartridge::CgbFlag;
use crate::dma::DmaController;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::model::{ConsoleModel, HeuristicPolicy, OperatingMode};
use crate::ppu::Ppu;
use crate::serial::Serial;
use crate::speed::{CgbSpeedMode, SpeedController};
use crate::timer::Timer;

use super::infrared::{CgbInfraredState, CgbInfraredStatus};
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
    pub speed: Option<&'a SpeedController>,
    pub ppu_cpu_visible_read: bool,
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
    pub speed: Option<&'a mut SpeedController>,
    pub boot_ff50_newly_unmapped: Option<&'a mut bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct IoHramDomain {
    #[serde(with = "serde_big_array::BigArray")]
    hram: [u8; HRAM_LEN],
    key0: CgbKey0State,
    cgb_misc: CgbMiscIoState,
    infrared: CgbInfraredState,
}

impl IoHramDomain {
    pub(crate) fn new() -> Self {
        Self {
            hram: [0; HRAM_LEN],
            key0: CgbKey0State::new(),
            cgb_misc: CgbMiscIoState::new(),
            infrared: CgbInfraredState::new(),
        }
    }

    pub(crate) fn apply_startup_memory_policy(&mut self, policy: StartupMemoryPolicy) {
        policy.initialize_hram(&mut self.hram);
    }

    pub(crate) fn hram_bytes(&self) -> &[u8] {
        &self.hram
    }

    pub(crate) fn reset_cgb_misc_io(&mut self) {
        self.cgb_misc = CgbMiscIoState::new();
    }

    pub(crate) fn apply_direct_boot_key0(
        &mut self,
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
        cgb_flag: CgbFlag,
    ) {
        self.key0 = CgbKey0State::direct_boot(console_model, operating_mode, cgb_flag);
    }

    pub(crate) fn reset_real_boot_key0(
        &mut self,
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
    ) {
        self.key0 = CgbKey0State::real_boot_entry(console_model, operating_mode);
    }

    #[cfg(test)]
    pub(crate) const fn key0_state(&self) -> CgbKey0State {
        self.key0
    }

    pub(crate) fn read_key0(&self) -> u8 {
        self.key0.read_runtime()
    }

    pub(crate) fn write_key0(&mut self, value: u8) {
        self.key0.write_runtime(value);
    }

    pub(crate) fn lock_cgb_real_boot_key0_at_handoff(
        &mut self,
        console_model: ConsoleModel,
        heuristic_policy: HeuristicPolicy,
    ) -> OperatingMode {
        self.key0
            .lock_real_boot_handoff(console_model, heuristic_policy)
    }

    pub(crate) fn read_cgb_misc_io(&self, address: u16) -> u8 {
        self.cgb_misc.read(address)
    }

    pub(crate) fn write_cgb_misc_io(&mut self, address: u16, value: u8) {
        self.cgb_misc.write(address, value);
    }

    pub(crate) fn read_rp(&self) -> u8 {
        self.infrared.read_rp()
    }

    pub(crate) fn write_rp(&mut self, value: u8) {
        self.infrared.write_rp(value);
    }

    pub(crate) fn tick_cgb_infrared_t_cycle(&mut self) {
        self.infrared.tick_t_cycle();
    }

    pub(crate) fn set_cgb_infrared_external_input(&mut self, active: bool) {
        self.infrared.set_external_optical_input(active);
    }

    pub(crate) fn cgb_infrared_emitter_on(&self) -> bool {
        self.infrared.emitter_on()
    }

    pub(crate) fn cgb_infrared_status(&self) -> CgbInfraredStatus {
        self.infrared.status()
    }

    #[cfg(test)]
    pub(crate) fn cgb_infrared_effective_signal_detected(&self) -> bool {
        self.infrared.effective_signal_detected()
    }

    pub(crate) fn read(
        &self,
        router: &AddressRouter,
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
        target: BusAddressInfo,
        io: BusIoReadView<'_>,
    ) -> u8 {
        match target.region() {
            BusRegion::Hram => self.hram[target.region_offset() as usize],
            BusRegion::Mmio | BusRegion::InterruptEnable => {
                self.read_io_target(router, console_model, operating_mode, target.address(), io)
            }
            _ => unreachable!("non-IoHram target routed to IoHramDomain"),
        }
    }

    pub(crate) fn write(
        &mut self,
        router: &AddressRouter,
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
        target: BusAddressInfo,
        value: u8,
        io: BusIoWriteView<'_>,
    ) {
        match target.region() {
            BusRegion::Hram => {
                self.hram[target.region_offset() as usize] = value;
            }
            BusRegion::Mmio | BusRegion::InterruptEnable => self.write_io_target(
                router,
                console_model,
                operating_mode,
                target.address(),
                value,
                io,
            ),
            _ => unreachable!("non-IoHram target routed to IoHramDomain"),
        }
    }

    fn read_io_target(
        &self,
        router: &AddressRouter,
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
        address: u16,
        io: BusIoReadView<'_>,
    ) -> u8 {
        let Some(info) = router.describe_io_register(address) else {
            return BLOCKED_READ_VALUE;
        };

        if !io_register_kind_is_available(
            info.kind(),
            info.availability(),
            console_model,
            operating_mode,
        ) || info.implementation() != IoRegisterImplementation::Implemented
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
            IoRegisterKind::Key1 => io
                .speed
                .map_or(BLOCKED_READ_VALUE, SpeedController::read_key1),
            IoRegisterKind::InterruptFlag => {
                io.interrupts.map_or(BLOCKED_READ_VALUE, |interrupts| {
                    interrupts.read_if_with_pending_requests(io.interrupt_flag_pending_mask)
                })
            }
            IoRegisterKind::Stat | IoRegisterKind::Ly => io.ppu.map_or(BLOCKED_READ_VALUE, |ppu| {
                let source = if io.ppu_cpu_visible_read {
                    crate::ppu::PpuRegisterReadSource::CpuBusOperation
                } else {
                    crate::ppu::PpuRegisterReadSource::Immediate
                };
                ppu.read_register_with_source(address, source)
            }),
            IoRegisterKind::OamDma => io.dma.map_or(BLOCKED_READ_VALUE, DmaController::read_ff46),
            IoRegisterKind::Hdma1
            | IoRegisterKind::Hdma2
            | IoRegisterKind::Hdma3
            | IoRegisterKind::Hdma4 => BLOCKED_READ_VALUE,
            IoRegisterKind::Hdma5 => io.dma.map_or(BLOCKED_READ_VALUE, DmaController::read_hdma5),
            IoRegisterKind::Rp => self.read_rp(),
            IoRegisterKind::BootRomDisable => io
                .boot
                .map_or(BLOCKED_READ_VALUE, BootController::read_ff50),
            IoRegisterKind::Pcm12 => io.apu.map_or(BLOCKED_READ_VALUE, Apu::read_pcm12),
            IoRegisterKind::Pcm34 => io.apu.map_or(BLOCKED_READ_VALUE, Apu::read_pcm34),
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
        operating_mode: OperatingMode,
        address: u16,
        value: u8,
        mut io: BusIoWriteView<'_>,
    ) {
        let Some(info) = router.describe_io_register(address) else {
            return;
        };

        if !io_register_kind_is_available(
            info.kind(),
            info.availability(),
            console_model,
            operating_mode,
        ) || info.implementation() != IoRegisterImplementation::Implemented
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
                let BusIoWriteView {
                    apu, timer, speed, ..
                } = io;
                if let Some(timer) = timer {
                    let speed_mode = speed.as_deref().map_or(
                        crate::speed::CgbSpeedMode::Normal,
                        SpeedController::current_speed,
                    );
                    let effects = timer.write_div_with_effects_for_speed(value, speed_mode);
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
            IoRegisterKind::Key1 => {
                if let Some(speed) = io.speed {
                    speed.write_key1(value);
                }
            }
            IoRegisterKind::InterruptFlag => {
                if let Some(interrupts) = io.interrupts {
                    interrupts.write_if(value);
                }
            }
            IoRegisterKind::OamDma => {
                let BusIoWriteView { dma, speed, .. } = io;
                let speed_mode = speed
                    .as_deref()
                    .map_or(CgbSpeedMode::Normal, SpeedController::current_speed);
                if let Some(dma) = dma {
                    dma.write_ff46_for_speed(value, speed_mode);
                }
            }
            IoRegisterKind::Hdma1 => {
                if let Some(dma) = io.dma {
                    dma.write_hdma1(value);
                }
            }
            IoRegisterKind::Hdma2 => {
                if let Some(dma) = io.dma {
                    dma.write_hdma2(value);
                }
            }
            IoRegisterKind::Hdma3 => {
                if let Some(dma) = io.dma {
                    dma.write_hdma3(value);
                }
            }
            IoRegisterKind::Hdma4 => {
                if let Some(dma) = io.dma {
                    dma.write_hdma4(value);
                }
            }
            IoRegisterKind::Hdma5 => {
                if let Some(dma) = io.dma {
                    dma.write_hdma5(value);
                }
            }
            IoRegisterKind::Rp => self.write_rp(value),
            IoRegisterKind::BootRomDisable => {
                if let Some(boot) = io.boot {
                    let newly_unmapped = boot.write_ff50(value);
                    if let Some(signal) = io.boot_ff50_newly_unmapped.as_deref_mut() {
                        *signal = newly_unmapped;
                    }
                }
            }
            IoRegisterKind::InterruptEnable => {
                if let Some(interrupts) = io.interrupts {
                    interrupts.write_ie(value);
                }
            }
            IoRegisterKind::Pcm12 | IoRegisterKind::Pcm34 => {}
            _ => match info.owner() {
                IoRegisterOwner::Ppu => {
                    if let Some(ppu) = io.ppu {
                        ppu.write_register(address, value);
                    }
                }
                IoRegisterOwner::Apu => {
                    let BusIoWriteView {
                        apu, timer, speed, ..
                    } = io;
                    let speed_mode = speed
                        .as_deref()
                        .map_or(CgbSpeedMode::Normal, SpeedController::current_speed);
                    let div_apu_signal_high = timer
                        .as_deref()
                        .is_some_and(|timer| timer.current_div_apu_signal_for_speed(speed_mode));
                    if let Some(apu) = apu {
                        apu.write_register_for_speed_with_div_apu_signal(
                            address,
                            value,
                            speed_mode,
                            div_apu_signal_high,
                        );
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CgbKey0State {
    value: u8,
    locked: bool,
}

impl CgbKey0State {
    const DMG_COMPATIBILITY_MODE_BIT: u8 = 0x04;
    const DMG_EXT_MODE_BIT: u8 = 0x08;

    const fn new() -> Self {
        Self {
            value: 0,
            locked: true,
        }
    }

    pub(crate) const fn direct_boot(
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
        cgb_flag: CgbFlag,
    ) -> Self {
        if !console_model.is_cgb_family() {
            return Self::new();
        }

        let value = if matches!(operating_mode, OperatingMode::GbCompatible) {
            match cgb_flag {
                CgbFlag::SupportedNonCanonical(value)
                    if value & Self::DMG_COMPATIBILITY_MODE_BIT != 0 =>
                {
                    value
                }
                _ => Self::DMG_COMPATIBILITY_MODE_BIT,
            }
        } else if matches!(operating_mode, OperatingMode::CgbDmgExt) {
            match cgb_flag {
                CgbFlag::SupportedNonCanonical(value) => value,
                _ => 0x80 | Self::DMG_EXT_MODE_BIT,
            }
        } else {
            match cgb_flag {
                CgbFlag::Supported => 0x80,
                CgbFlag::Only => 0xC0,
                CgbFlag::SupportedNonCanonical(value) => value,
                CgbFlag::None | CgbFlag::Unknown(_) => Self::DMG_COMPATIBILITY_MODE_BIT,
            }
        };

        Self {
            value,
            locked: true,
        }
    }

    pub(crate) const fn real_boot_entry(
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
    ) -> Self {
        if !console_model.is_cgb_family() {
            return Self::new();
        }

        Self {
            value: if matches!(operating_mode, OperatingMode::GbCompatible) {
                Self::DMG_COMPATIBILITY_MODE_BIT
            } else if matches!(operating_mode, OperatingMode::CgbDmgExt) {
                Self::DMG_EXT_MODE_BIT
            } else {
                0
            },
            locked: false,
        }
    }

    #[cfg(test)]
    pub(crate) const fn value(self) -> u8 {
        self.value
    }

    #[cfg(test)]
    pub(crate) const fn is_locked(self) -> bool {
        self.locked
    }

    pub(crate) const fn read_runtime(self) -> u8 {
        // Pan Docs treats KEY0 as boot-owned and effectively unavailable to ordinary software after handoff. Keep the internal state for boot/mode ownership, but expose the ordinary unavailable readback until Slice 6 validates RealBoot read effects.
        super::BLOCKED_READ_VALUE
    }

    pub(crate) fn write_runtime(&mut self, value: u8) {
        if !self.locked {
            self.value = value;
        }
    }

    pub(crate) fn lock_real_boot_handoff(
        &mut self,
        console_model: ConsoleModel,
        heuristic_policy: HeuristicPolicy,
    ) -> OperatingMode {
        if !console_model.is_cgb_family() {
            self.value = 0;
            self.locked = true;
            return OperatingMode::Dmg;
        }

        self.locked = true;
        self.boot_selected_operating_mode(console_model, heuristic_policy)
    }

    pub(crate) const fn boot_selected_operating_mode(
        self,
        console_model: ConsoleModel,
        heuristic_policy: HeuristicPolicy,
    ) -> OperatingMode {
        if !console_model.is_cgb_family() {
            return OperatingMode::Dmg;
        }

        if matches!(heuristic_policy, HeuristicPolicy::AllowExperimental)
            && self.value & Self::DMG_EXT_MODE_BIT != 0
        {
            OperatingMode::CgbDmgExt
        } else if self.value & Self::DMG_COMPATIBILITY_MODE_BIT != 0 {
            OperatingMode::GbCompatible
        } else {
            OperatingMode::Cgb
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct CgbMiscIoState {
    ff72: u8,
    ff73: u8,
    ff74: u8,
    ff75_bits_4_6: u8,
}

impl CgbMiscIoState {
    const FF75_FORCED_BITS: u8 = 0x8F;
    const FF75_WRITABLE_MASK: u8 = 0x70;

    const fn new() -> Self {
        Self {
            ff72: 0,
            ff73: 0,
            ff74: 0,
            ff75_bits_4_6: 0,
        }
    }

    fn read(self, address: u16) -> u8 {
        match address {
            0xFF72 => self.ff72,
            0xFF73 => self.ff73,
            0xFF74 => self.ff74,
            0xFF75 => Self::FF75_FORCED_BITS | self.ff75_bits_4_6,
            _ => super::BLOCKED_READ_VALUE,
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFF72 => self.ff72 = value,
            0xFF73 => self.ff73 = value,
            0xFF74 => self.ff74 = value,
            0xFF75 => self.ff75_bits_4_6 = value & Self::FF75_WRITABLE_MASK,
            _ => {}
        }
    }
}

pub(super) fn io_register_kind_is_available(
    kind: IoRegisterKind,
    availability: IoRegisterAvailability,
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
) -> bool {
    match availability {
        IoRegisterAvailability::Shared | IoRegisterAvailability::DmgCompatible => true,
        IoRegisterAvailability::CgbOnly => {
            cgb_io_register_is_available(kind, console_model, operating_mode)
        }
    }
}

fn cgb_io_register_is_available(
    kind: IoRegisterKind,
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
) -> bool {
    if !console_model.is_cgb_family() {
        return false;
    }

    if operating_mode.enables_cgb_extensions() {
        return true;
    }

    if matches!(operating_mode, OperatingMode::CgbDmgExt) {
        return matches!(
            kind,
            IoRegisterKind::Key1
                | IoRegisterKind::Vbk
                | IoRegisterKind::Rp
                | IoRegisterKind::Bcps
                | IoRegisterKind::Ocps
                | IoRegisterKind::Opri
                | IoRegisterKind::Svbk
                | IoRegisterKind::CgbUndocumented72
                | IoRegisterKind::CgbUndocumented73
                | IoRegisterKind::CgbUndocumented74
                | IoRegisterKind::CgbUndocumented75
                | IoRegisterKind::Pcm12
                | IoRegisterKind::Pcm34
        );
    }

    // CGB-family compatibility mode is not DMG silicon. Keep the small set of
    // boot-HWIO-visible CGB registers routed even while native-only functional
    // features such as banking writes, HDMA, infrared, and palette data remain
    // unavailable to monochrome software.
    matches!(
        kind,
        IoRegisterKind::Vbk
            | IoRegisterKind::Bcps
            | IoRegisterKind::Ocps
            | IoRegisterKind::CgbUndocumented72
            | IoRegisterKind::CgbUndocumented73
            | IoRegisterKind::CgbUndocumented75
            | IoRegisterKind::Pcm12
            | IoRegisterKind::Pcm34
    )
}
