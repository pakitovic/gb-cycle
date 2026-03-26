use super::map::{
    BusAddressInfo, BusRegion, IoRegisterAccess, IoRegisterAvailability, IoRegisterInfo,
    IoRegisterKind, IoRegisterOwner,
};
use super::{BusAccessKind, BusArbitrationState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AddressRouter;

impl AddressRouter {
    pub const fn new() -> Self {
        Self
    }

    pub fn decode_address(&self, address: u16) -> BusAddressInfo {
        match address {
            0x0000..=0x3FFF => BusAddressInfo::new(address, BusRegion::CartridgeRomBank0, address),
            0x4000..=0x7FFF => {
                BusAddressInfo::new(address, BusRegion::CartridgeRomBankN, address - 0x4000)
            }
            0x8000..=0x9FFF => BusAddressInfo::new(address, BusRegion::Vram, address - 0x8000),
            0xA000..=0xBFFF => {
                BusAddressInfo::new(address, BusRegion::CartridgeExternal, address - 0xA000)
            }
            0xC000..=0xCFFF => BusAddressInfo::new(address, BusRegion::WramBank0, address - 0xC000),
            0xD000..=0xDFFF => BusAddressInfo::new(address, BusRegion::WramBankN, address - 0xD000),
            0xE000..=0xFDFF => BusAddressInfo::new(address, BusRegion::EchoRam, address - 0xE000),
            0xFE00..=0xFE9F => BusAddressInfo::new(address, BusRegion::Oam, address - 0xFE00),
            0xFEA0..=0xFEFF => BusAddressInfo::new(address, BusRegion::Unusable, address - 0xFEA0),
            0xFF00..=0xFF7F => BusAddressInfo::new(address, BusRegion::Mmio, address - 0xFF00),
            0xFF80..=0xFFFE => BusAddressInfo::new(address, BusRegion::Hram, address - 0xFF80),
            0xFFFF => BusAddressInfo::new(address, BusRegion::InterruptEnable, 0),
        }
    }

    pub fn describe_io_register(&self, address: u16) -> Option<IoRegisterInfo> {
        Some(match address {
            0xFF00 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Joypad,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Joyp,
            ),
            0xFF01 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Serial,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::SerialData,
            ),
            0xFF02 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Serial,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::Mixed,
                IoRegisterKind::SerialControl,
            ),
            0xFF03 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Reserved,
            ),
            0xFF04 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Timer,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Div,
            ),
            0xFF05 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Timer,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Tima,
            ),
            0xFF06 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Timer,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Tma,
            ),
            0xFF07 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Timer,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Tac,
            ),
            0xFF08..=0xFF0E => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Reserved,
            ),
            0xFF0F => IoRegisterInfo::new(
                address,
                IoRegisterOwner::InterruptController,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::Mixed,
                IoRegisterKind::InterruptFlag,
            ),
            0xFF10..=0xFF3F => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Sound,
            ),
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Lcd,
            ),
            0xFF46 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Dma,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::OamDma,
            ),
            0xFF4C => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Reserved,
            ),
            0xFF4D | 0xFF4F | 0xFF51..=0xFF56 | 0xFF68..=0xFF70 | 0xFF72..=0xFF77 => {
                IoRegisterInfo::new(
                    address,
                    IoRegisterOwner::CgbOnly,
                    IoRegisterAvailability::CgbOnly,
                    IoRegisterAccess::Mixed,
                    IoRegisterKind::CgbSystem,
                )
            }
            0xFF4E | 0xFF57..=0xFF67 | 0xFF71 | 0xFF78..=0xFF7F => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Reserved,
            ),
            0xFF50 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Boot,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::BootRomDisable,
            ),
            0xFFFF => IoRegisterInfo::new(
                address,
                IoRegisterOwner::InterruptController,
                IoRegisterAvailability::AllModels,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::InterruptEnable,
            ),
            _ => return None,
        })
    }

    pub fn resolve_nominal_target(
        &self,
        kind: BusAccessKind,
        address: u16,
        state: &BusArbitrationState,
    ) -> BusAddressInfo {
        if kind == BusAccessKind::Read && state.boot_rom.overlays_read(address) {
            return BusAddressInfo::new(address, BusRegion::BootRom, address);
        }

        self.decode_address(address)
    }
}
