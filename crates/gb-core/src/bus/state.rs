use crate::cartridge::CartridgeExternalAccessInfo;
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
    UnusableRegionDuringDmaVideoBusConflict,
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
    low_window_mapped: bool,
    cgb_upper_window_mapped: bool,
}

impl BootRomBusState {
    pub const fn unmapped() -> Self {
        Self {
            low_window_mapped: false,
            cgb_upper_window_mapped: false,
        }
    }

    pub const fn map_dmg_low_bytes() -> Self {
        Self {
            low_window_mapped: true,
            cgb_upper_window_mapped: false,
        }
    }

    pub const fn map_cgb_windows() -> Self {
        Self {
            low_window_mapped: true,
            cgb_upper_window_mapped: true,
        }
    }

    pub const fn maps_low_window(self) -> bool {
        self.low_window_mapped
    }

    pub const fn maps_dmg_low_bytes(self) -> bool {
        self.maps_low_window()
    }

    pub const fn maps_cgb_upper_window(self) -> bool {
        self.cgb_upper_window_mapped
    }

    pub const fn overlays_read(self, address: u16) -> bool {
        (self.low_window_mapped && address <= 0x00FF)
            || (self.cgb_upper_window_mapped && address >= 0x0200 && address <= 0x08FF)
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
    requested_address: u16,
    nominal_target: BusAddressInfo,
    target: BusAddressInfo,
    nominal_cartridge_external: Option<CartridgeExternalAccessInfo>,
    cartridge_external: Option<CartridgeExternalAccessInfo>,
    nominal_disposition: BusAccessDisposition,
    disposition: BusAccessDisposition,
}

impl BusAccessResolution {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        requester: BusRequester,
        kind: BusAccessKind,
        requested_address: u16,
        nominal_target: BusAddressInfo,
        target: BusAddressInfo,
        nominal_cartridge_external: Option<CartridgeExternalAccessInfo>,
        cartridge_external: Option<CartridgeExternalAccessInfo>,
        nominal_disposition: BusAccessDisposition,
        disposition: BusAccessDisposition,
    ) -> Self {
        Self {
            requester,
            kind,
            requested_address,
            nominal_target,
            target,
            nominal_cartridge_external,
            cartridge_external,
            nominal_disposition,
            disposition,
        }
    }

    pub const fn requester(self) -> BusRequester {
        self.requester
    }

    pub const fn kind(self) -> BusAccessKind {
        self.kind
    }

    pub const fn requested_address(self) -> u16 {
        self.requested_address
    }

    pub const fn nominal_target(self) -> BusAddressInfo {
        self.nominal_target
    }

    pub const fn target(self) -> BusAddressInfo {
        self.target
    }

    pub const fn effective_target(self) -> BusAddressInfo {
        self.target
    }

    pub const fn nominal_cartridge_external(self) -> Option<CartridgeExternalAccessInfo> {
        self.nominal_cartridge_external
    }

    pub const fn cartridge_external(self) -> Option<CartridgeExternalAccessInfo> {
        self.cartridge_external
    }

    pub const fn effective_cartridge_external(self) -> Option<CartridgeExternalAccessInfo> {
        self.cartridge_external
    }

    pub const fn nominal_disposition(self) -> BusAccessDisposition {
        self.nominal_disposition
    }

    pub const fn disposition(self) -> BusAccessDisposition {
        self.disposition
    }

    pub const fn is_redirected(self) -> bool {
        self.target.address() != self.requested_address
    }

    pub const fn redirected_source_address(self) -> Option<u16> {
        if self.is_redirected() {
            Some(self.target.address())
        } else {
            None
        }
    }
}
