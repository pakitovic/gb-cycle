#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusRegion {
    BootRom,
    CartridgeRomBank0,
    CartridgeRomBankN,
    Vram,
    CartridgeExternal,
    WramBank0,
    WramBankN,
    EchoRam,
    Oam,
    Unusable,
    Mmio,
    Hram,
    InterruptEnable,
}

impl BusRegion {
    pub const fn domain(self) -> BusDomain {
        match self {
            Self::BootRom => BusDomain::BootRom,
            Self::CartridgeRomBank0 | Self::CartridgeRomBankN | Self::CartridgeExternal => {
                BusDomain::Cartridge
            }
            Self::Vram => BusDomain::Vram,
            Self::WramBank0 | Self::WramBankN | Self::EchoRam => BusDomain::Wram,
            Self::Oam => BusDomain::Oam,
            Self::Unusable => BusDomain::Unusable,
            Self::Mmio | Self::Hram | Self::InterruptEnable => BusDomain::IoHram,
        }
    }

    pub const fn owner(self) -> BusRegionOwner {
        match self {
            Self::BootRom => BusRegionOwner::Boot,
            Self::CartridgeRomBank0 | Self::CartridgeRomBankN | Self::CartridgeExternal => {
                BusRegionOwner::Cartridge
            }
            Self::Vram | Self::Oam => BusRegionOwner::Ppu,
            Self::WramBank0 | Self::WramBankN | Self::EchoRam | Self::Unusable | Self::Hram => {
                BusRegionOwner::Bus
            }
            Self::Mmio => BusRegionOwner::Mmio,
            Self::InterruptEnable => BusRegionOwner::InterruptController,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusDomain {
    BootRom,
    Cartridge,
    Vram,
    Wram,
    Oam,
    Unusable,
    IoHram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusRegionOwner {
    Boot,
    Bus,
    Cartridge,
    Ppu,
    Mmio,
    InterruptController,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BusAddressInfo {
    address: u16,
    region: BusRegion,
    region_offset: u16,
}

impl BusAddressInfo {
    pub const fn new(address: u16, region: BusRegion, region_offset: u16) -> Self {
        Self {
            address,
            region,
            region_offset,
        }
    }

    pub const fn address(self) -> u16 {
        self.address
    }

    pub const fn region(self) -> BusRegion {
        self.region
    }

    pub const fn domain(self) -> BusDomain {
        self.region.domain()
    }

    pub const fn owner(self) -> BusRegionOwner {
        self.region.owner()
    }

    pub const fn region_offset(self) -> u16 {
        self.region_offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnusableAreaReadProfile {
    DmgFamilyFixedZero,
    CgbRevisionDependent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnusableAreaInfo {
    address: u16,
    read_profile: UnusableAreaReadProfile,
    runtime_fallback_read_value: u8,
}

impl UnusableAreaInfo {
    pub const fn new(
        address: u16,
        read_profile: UnusableAreaReadProfile,
        runtime_fallback_read_value: u8,
    ) -> Self {
        Self {
            address,
            read_profile,
            runtime_fallback_read_value,
        }
    }

    pub const fn address(self) -> u16 {
        self.address
    }

    pub const fn read_profile(self) -> UnusableAreaReadProfile {
        self.read_profile
    }

    pub const fn runtime_fallback_read_value(self) -> u8 {
        self.runtime_fallback_read_value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoRegisterOwner {
    Joypad,
    Serial,
    Timer,
    InterruptController,
    Apu,
    Ppu,
    Dma,
    Boot,
    MemoryController,
    Infrared,
    CgbSystem,
    Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoRegisterAvailability {
    Shared,
    DmgCompatible,
    CgbOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoRegisterImplementation {
    Implemented,
    Stubbed,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoRegisterAccess {
    ReadWrite,
    ReadOnly,
    WriteOnly,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoRegisterKind {
    Joyp,
    SerialData,
    SerialControl,
    Div,
    Tima,
    Tma,
    Tac,
    InterruptFlag,
    Nr10,
    Nr11,
    Nr12,
    Nr13,
    Nr14,
    Nr21,
    Nr22,
    Nr23,
    Nr24,
    Nr30,
    Nr31,
    Nr32,
    Nr33,
    Nr34,
    Nr41,
    Nr42,
    Nr43,
    Nr44,
    Nr50,
    Nr51,
    Nr52,
    WaveRam,
    Lcdc,
    Stat,
    Scy,
    Scx,
    Ly,
    Lyc,
    Bgp,
    Obp0,
    Obp1,
    Wy,
    Wx,
    OamDma,
    BootRomDisable,
    Key0,
    Key1,
    Vbk,
    Hdma1,
    Hdma2,
    Hdma3,
    Hdma4,
    Hdma5,
    Rp,
    Bcps,
    Bcpd,
    Ocps,
    Ocpd,
    Opri,
    Svbk,
    CgbUndocumented72,
    CgbUndocumented73,
    CgbUndocumented74,
    CgbUndocumented75,
    Pcm12,
    Pcm34,
    CgbUndocumented,
    Reserved,
    InterruptEnable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IoRegisterInfo {
    address: u16,
    owner: IoRegisterOwner,
    availability: IoRegisterAvailability,
    implementation: IoRegisterImplementation,
    access: IoRegisterAccess,
    kind: IoRegisterKind,
}

impl IoRegisterInfo {
    pub const fn new(
        address: u16,
        owner: IoRegisterOwner,
        availability: IoRegisterAvailability,
        access: IoRegisterAccess,
        kind: IoRegisterKind,
    ) -> Self {
        Self::new_with_implementation(
            address,
            owner,
            availability,
            IoRegisterImplementation::Implemented,
            access,
            kind,
        )
    }

    pub const fn new_with_implementation(
        address: u16,
        owner: IoRegisterOwner,
        availability: IoRegisterAvailability,
        implementation: IoRegisterImplementation,
        access: IoRegisterAccess,
        kind: IoRegisterKind,
    ) -> Self {
        Self {
            address,
            owner,
            availability,
            implementation,
            access,
            kind,
        }
    }

    pub const fn address(self) -> u16 {
        self.address
    }

    pub const fn owner(self) -> IoRegisterOwner {
        self.owner
    }

    pub const fn availability(self) -> IoRegisterAvailability {
        self.availability
    }

    pub const fn implementation(self) -> IoRegisterImplementation {
        self.implementation
    }

    pub const fn access(self) -> IoRegisterAccess {
        self.access
    }

    pub const fn kind(self) -> IoRegisterKind {
        self.kind
    }
}
