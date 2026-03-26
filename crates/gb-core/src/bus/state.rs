use crate::ppu::PpuBusState;

use super::map::BusAddressInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusRequester {
    Cpu,
    Dma,
    Ppu,
    Apu,
    Serial,
    Boot,
    Cartridge,
}

pub type BusMaster = BusRequester;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusAccessKind {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusBlockReason {
    DmaExternalBusConflict,
    DmaVideoBusConflict,
    PpuVramBlockedDuringMode3,
    PpuOamBlockedDuringMode2,
    PpuOamBlockedDuringMode3,
    UnusableRegion,
    UnusableRegionDuringOamBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusAccessDisposition {
    Allowed,
    BlockedRead { value: u8, reason: BusBlockReason },
    IgnoredWrite { reason: BusBlockReason },
}

impl BusAccessDisposition {
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub const fn blocked_reason(self) -> Option<BusBlockReason> {
        match self {
            Self::Allowed => None,
            Self::BlockedRead { reason, .. } | Self::IgnoredWrite { reason } => Some(reason),
        }
    }

    pub const fn blocked_read_value(self) -> Option<u8> {
        match self {
            Self::BlockedRead { value, .. } => Some(value),
            Self::Allowed | Self::IgnoredWrite { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DmaCpuAccessPolicy {
    #[default]
    Unrestricted,
    ExternalBusBlocked,
    VideoBusBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DmaMemoryRegionImpact {
    Oam,
    Vram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DmaBusState {
    cpu_access_policy: DmaCpuAccessPolicy,
    active_region: Option<DmaMemoryRegionImpact>,
    cpu_conflict_source_address: Option<u16>,
}

impl DmaBusState {
    pub const fn unrestricted() -> Self {
        Self {
            cpu_access_policy: DmaCpuAccessPolicy::Unrestricted,
            active_region: None,
            cpu_conflict_source_address: None,
        }
    }

    pub const fn external_bus_blocked(active_region: Option<DmaMemoryRegionImpact>) -> Self {
        Self {
            cpu_access_policy: DmaCpuAccessPolicy::ExternalBusBlocked,
            active_region,
            cpu_conflict_source_address: None,
        }
    }

    pub const fn video_bus_blocked(active_region: Option<DmaMemoryRegionImpact>) -> Self {
        Self {
            cpu_access_policy: DmaCpuAccessPolicy::VideoBusBlocked,
            active_region,
            cpu_conflict_source_address: None,
        }
    }

    pub const fn with_cpu_conflict_source_address(mut self, address: Option<u16>) -> Self {
        self.cpu_conflict_source_address = address;
        self
    }

    pub const fn cpu_access_policy(self) -> DmaCpuAccessPolicy {
        self.cpu_access_policy
    }

    pub const fn active_region(self) -> Option<DmaMemoryRegionImpact> {
        self.active_region
    }

    pub const fn cpu_conflict_source_address(self) -> Option<u16> {
        self.cpu_conflict_source_address
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BootRomBusState {
    dmg_low_bytes_mapped: bool,
}

impl BootRomBusState {
    pub const fn unmapped() -> Self {
        Self {
            dmg_low_bytes_mapped: false,
        }
    }

    pub const fn map_dmg_low_bytes() -> Self {
        Self {
            dmg_low_bytes_mapped: true,
        }
    }

    pub const fn maps_dmg_low_bytes(self) -> bool {
        self.dmg_low_bytes_mapped
    }

    pub const fn overlays_read(self, address: u16) -> bool {
        self.dmg_low_bytes_mapped && address <= 0x00FF
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BusArbitrationState {
    pub boot_rom: BootRomBusState,
    pub ppu: PpuBusState,
    pub dma: DmaBusState,
}

impl BusArbitrationState {
    pub const fn with_boot_rom(mut self, boot_rom: BootRomBusState) -> Self {
        self.boot_rom = boot_rom;
        self
    }

    pub const fn with_ppu(mut self, ppu: PpuBusState) -> Self {
        self.ppu = ppu;
        self
    }

    pub const fn with_dma(mut self, dma: DmaBusState) -> Self {
        self.dma = dma;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BusAccessResolution {
    requester: BusRequester,
    kind: BusAccessKind,
    target: BusAddressInfo,
    disposition: BusAccessDisposition,
}

impl BusAccessResolution {
    pub const fn new(
        requester: BusRequester,
        kind: BusAccessKind,
        target: BusAddressInfo,
        disposition: BusAccessDisposition,
    ) -> Self {
        Self {
            requester,
            kind,
            target,
            disposition,
        }
    }

    pub const fn requester(self) -> BusRequester {
        self.requester
    }

    pub const fn kind(self) -> BusAccessKind {
        self.kind
    }

    pub const fn target(self) -> BusAddressInfo {
        self.target
    }

    pub const fn disposition(self) -> BusAccessDisposition {
        self.disposition
    }
}
