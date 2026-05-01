use super::map::{
    BusAddressInfo, BusRegion, IoRegisterAccess, IoRegisterAvailability, IoRegisterImplementation,
    IoRegisterInfo, IoRegisterKind, IoRegisterOwner, UnusableAreaInfo, UnusableAreaReadProfile,
    UnusableAreaWriteProfile,
};
use super::{BLOCKED_READ_VALUE, BusAccessKind, BusArbitrationState, DMG_UNUSABLE_READ_VALUE};
use crate::model::ConsoleModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct AddressRouter;

const fn implemented_io(
    address: u16,
    owner: IoRegisterOwner,
    availability: IoRegisterAvailability,
    access: IoRegisterAccess,
    kind: IoRegisterKind,
) -> IoRegisterInfo {
    IoRegisterInfo::new(address, owner, availability, access, kind)
}

const fn non_functional_io(
    address: u16,
    owner: IoRegisterOwner,
    availability: IoRegisterAvailability,
    implementation: IoRegisterImplementation,
    access: IoRegisterAccess,
    kind: IoRegisterKind,
) -> IoRegisterInfo {
    IoRegisterInfo::new_with_implementation(
        address,
        owner,
        availability,
        implementation,
        access,
        kind,
    )
}

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
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Joyp,
            ),
            0xFF01 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Serial,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::SerialData,
            ),
            0xFF02 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Serial,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::SerialControl,
            ),
            0xFF03 => non_functional_io(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::Shared,
                IoRegisterImplementation::Unavailable,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Reserved,
            ),
            0xFF04 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Timer,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Div,
            ),
            0xFF05 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Timer,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Tima,
            ),
            0xFF06 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Timer,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Tma,
            ),
            0xFF07 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Timer,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Tac,
            ),
            0xFF08..=0xFF0E => non_functional_io(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::Shared,
                IoRegisterImplementation::Unavailable,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Reserved,
            ),
            0xFF0F => IoRegisterInfo::new(
                address,
                IoRegisterOwner::InterruptController,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::InterruptFlag,
            ),
            0xFF10 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr10,
            ),
            0xFF11 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr11,
            ),
            0xFF12 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Nr12,
            ),
            0xFF13 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::Nr13,
            ),
            0xFF14 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr14,
            ),
            0xFF15 => non_functional_io(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::Shared,
                IoRegisterImplementation::Unavailable,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Reserved,
            ),
            0xFF16 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr21,
            ),
            0xFF17 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Nr22,
            ),
            0xFF18 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::Nr23,
            ),
            0xFF19 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr24,
            ),
            0xFF1A => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr30,
            ),
            0xFF1B => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::Nr31,
            ),
            0xFF1C => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr32,
            ),
            0xFF1D => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::Nr33,
            ),
            0xFF1E => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr34,
            ),
            0xFF1F => non_functional_io(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::Shared,
                IoRegisterImplementation::Unavailable,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Reserved,
            ),
            0xFF20 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::Nr41,
            ),
            0xFF21 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Nr42,
            ),
            0xFF22 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Nr43,
            ),
            0xFF23 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr44,
            ),
            0xFF24 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Nr50,
            ),
            0xFF25 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Nr51,
            ),
            0xFF26 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Nr52,
            ),
            0xFF27..=0xFF2F => non_functional_io(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::Shared,
                IoRegisterImplementation::Unavailable,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Reserved,
            ),
            0xFF30..=0xFF3F => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::WaveRam,
            ),
            0xFF40 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Lcdc,
            ),
            0xFF41 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Stat,
            ),
            0xFF42 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Scy,
            ),
            0xFF43 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Scx,
            ),
            0xFF44 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadOnly,
                IoRegisterKind::Ly,
            ),
            0xFF45 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Lyc,
            ),
            0xFF46 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Dma,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::OamDma,
            ),
            0xFF47 => implemented_io(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::DmgCompatible,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Bgp,
            ),
            0xFF48 => implemented_io(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::DmgCompatible,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Obp0,
            ),
            0xFF49 => implemented_io(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::DmgCompatible,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Obp1,
            ),
            0xFF4A => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Wy,
            ),
            0xFF4B => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Wx,
            ),
            0xFF4C => implemented_io(
                address,
                IoRegisterOwner::CgbSystem,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Key0,
            ),
            0xFF4D => implemented_io(
                address,
                IoRegisterOwner::CgbSystem,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Key1,
            ),
            0xFF4F => implemented_io(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Vbk,
            ),
            0xFF51 => non_functional_io(
                address,
                IoRegisterOwner::Dma,
                IoRegisterAvailability::CgbOnly,
                IoRegisterImplementation::Stubbed,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::Hdma1,
            ),
            0xFF52 => non_functional_io(
                address,
                IoRegisterOwner::Dma,
                IoRegisterAvailability::CgbOnly,
                IoRegisterImplementation::Stubbed,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::Hdma2,
            ),
            0xFF53 => non_functional_io(
                address,
                IoRegisterOwner::Dma,
                IoRegisterAvailability::CgbOnly,
                IoRegisterImplementation::Stubbed,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::Hdma3,
            ),
            0xFF54 => non_functional_io(
                address,
                IoRegisterOwner::Dma,
                IoRegisterAvailability::CgbOnly,
                IoRegisterImplementation::Stubbed,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::Hdma4,
            ),
            0xFF55 => non_functional_io(
                address,
                IoRegisterOwner::Dma,
                IoRegisterAvailability::CgbOnly,
                IoRegisterImplementation::Stubbed,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Hdma5,
            ),
            0xFF56 => non_functional_io(
                address,
                IoRegisterOwner::Infrared,
                IoRegisterAvailability::CgbOnly,
                IoRegisterImplementation::Stubbed,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Rp,
            ),
            0xFF68 => implemented_io(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Bcps,
            ),
            0xFF69 => implemented_io(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Bcpd,
            ),
            0xFF6A => implemented_io(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Ocps,
            ),
            0xFF6B => implemented_io(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Ocpd,
            ),
            0xFF6C => implemented_io(
                address,
                IoRegisterOwner::Ppu,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::Opri,
            ),
            0xFF70 => implemented_io(
                address,
                IoRegisterOwner::MemoryController,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Svbk,
            ),
            0xFF72 => implemented_io(
                address,
                IoRegisterOwner::CgbSystem,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::CgbUndocumented72,
            ),
            0xFF73 => implemented_io(
                address,
                IoRegisterOwner::CgbSystem,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::CgbUndocumented73,
            ),
            0xFF74 => implemented_io(
                address,
                IoRegisterOwner::CgbSystem,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::CgbUndocumented74,
            ),
            0xFF75 => implemented_io(
                address,
                IoRegisterOwner::CgbSystem,
                IoRegisterAvailability::CgbOnly,
                IoRegisterAccess::Mixed,
                IoRegisterKind::CgbUndocumented75,
            ),
            0xFF76 => non_functional_io(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::CgbOnly,
                IoRegisterImplementation::Stubbed,
                IoRegisterAccess::ReadOnly,
                IoRegisterKind::Pcm12,
            ),
            0xFF77 => non_functional_io(
                address,
                IoRegisterOwner::Apu,
                IoRegisterAvailability::CgbOnly,
                IoRegisterImplementation::Stubbed,
                IoRegisterAccess::ReadOnly,
                IoRegisterKind::Pcm34,
            ),
            0xFF6D..=0xFF6F => non_functional_io(
                address,
                IoRegisterOwner::CgbSystem,
                IoRegisterAvailability::CgbOnly,
                IoRegisterImplementation::Stubbed,
                IoRegisterAccess::Mixed,
                IoRegisterKind::CgbUndocumented,
            ),
            0xFF4E | 0xFF57..=0xFF67 | 0xFF71 | 0xFF78..=0xFF7F => non_functional_io(
                address,
                IoRegisterOwner::Reserved,
                IoRegisterAvailability::Shared,
                IoRegisterImplementation::Unavailable,
                IoRegisterAccess::Mixed,
                IoRegisterKind::Reserved,
            ),
            0xFF50 => IoRegisterInfo::new(
                address,
                IoRegisterOwner::Boot,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::WriteOnly,
                IoRegisterKind::BootRomDisable,
            ),
            0xFFFF => IoRegisterInfo::new(
                address,
                IoRegisterOwner::InterruptController,
                IoRegisterAvailability::Shared,
                IoRegisterAccess::ReadWrite,
                IoRegisterKind::InterruptEnable,
            ),
            _ => return None,
        })
    }

    pub fn describe_unusable_area(
        &self,
        console_model: ConsoleModel,
        address: u16,
    ) -> Option<UnusableAreaInfo> {
        if !(0xFEA0..=0xFEFF).contains(&address) {
            return None;
        }

        Some(match console_model {
            ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => {
                UnusableAreaInfo::new(
                    address,
                    UnusableAreaReadProfile::DmgFamilyFixedZero,
                    UnusableAreaWriteProfile::Ignored,
                    DMG_UNUSABLE_READ_VALUE,
                    true,
                )
            }
            ConsoleModel::GameBoyColor => UnusableAreaInfo::new(
                address,
                UnusableAreaReadProfile::CgbRevisionDependent,
                UnusableAreaWriteProfile::CgbRevisionDependentRam,
                BLOCKED_READ_VALUE,
                true,
            ),
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
