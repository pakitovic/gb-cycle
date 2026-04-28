mod access;
mod corruption;
mod dispatch;
mod iohram;
mod map;
mod meta;
mod policy;
mod router;
mod state;
mod video;
mod view;
mod wram;

use crate::model::ConsoleModel;
pub(crate) use iohram::{BusIoReadView, BusIoWriteView, IoHramDomain};
pub use map::{
    BusAddressInfo, BusDomain, BusRegion, BusRegionOwner, IoRegisterAccess, IoRegisterAvailability,
    IoRegisterImplementation, IoRegisterInfo, IoRegisterKind, IoRegisterOwner, UnusableAreaInfo,
    UnusableAreaReadProfile, UnusableAreaWriteProfile,
};
pub use meta::BusSnapshot;
pub use router::AddressRouter;
pub use state::{
    BootRomBusState, BusAccessDisposition, BusAccessKind, BusAccessResolution, BusArbitrationState,
    BusBlockReason, BusMaster, BusRequester, BusStatus, DmaBusState, DmaCpuAccessPolicy,
    DmaMemoryRegionImpact,
};
pub(crate) use video::{OamDomain, VramDomain};
pub(crate) use view::{OamBusView, VramBusView};
pub(crate) use wram::WramDomain;

const VRAM_LEN: usize = 0x2000;
const WRAM_LEN: usize = 0x2000;
const OAM_LEN: usize = 0x00A0;
const HRAM_LEN: usize = 0x007F;

const BLOCKED_READ_VALUE: u8 = 0xFF;
const DMG_UNUSABLE_READ_VALUE: u8 = 0x00;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bus {
    console_model: ConsoleModel,
    status: BusStatus,
    router: AddressRouter,
    vram: VramDomain,
    wram: WramDomain,
    oam: OamDomain,
    iohram: IoHramDomain,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BusSaveState {
    console_model: ConsoleModel,
    status: BusStatus,
    router: AddressRouter,
    vram: VramDomain,
    wram: WramDomain,
    oam: OamDomain,
    iohram: IoHramDomain,
}

impl BusSaveState {
    pub(crate) const fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

impl Bus {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: BusStatus::Ready,
            router: AddressRouter::new(),
            vram: VramDomain::new(),
            wram: WramDomain::new(),
            oam: OamDomain::new(),
            iohram: IoHramDomain::new(),
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> BusStatus {
        self.status
    }

    pub(crate) fn capture_save_state(&self) -> BusSaveState {
        BusSaveState {
            console_model: self.console_model,
            status: self.status,
            router: self.router,
            vram: self.vram.clone(),
            wram: self.wram.clone(),
            oam: self.oam.clone(),
            iohram: self.iohram.clone(),
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &BusSaveState) {
        self.console_model = state.console_model;
        self.status = state.status;
        self.router = state.router;
        self.vram = state.vram.clone();
        self.wram = state.wram.clone();
        self.oam = state.oam.clone();
        self.iohram = state.iohram.clone();
    }

    /// Returns the static DMG memory-map classification for `address`.
    ///
    /// This is an address-only decode surface. It does not apply boot ROM
    /// overlay windows or any other live arbitration state.
    pub fn decode_address(&self, address: u16) -> BusAddressInfo {
        self.router.decode_address(address)
    }

    pub fn describe_io_register(&self, address: u16) -> Option<IoRegisterInfo> {
        self.router.describe_io_register(address)
    }

    pub fn describe_unusable_area(&self, address: u16) -> Option<UnusableAreaInfo> {
        self.router
            .describe_unusable_area(self.console_model, address)
    }

    /// Returns the raw VRAM backing bytes for deterministic debug probes.
    ///
    /// This is intentionally not a CPU bus read: it bypasses live PPU/DMA arbitration so external tooling can compare emulator state without perturbing the machine or conflating blocked CPU visibility with actual storage.
    pub fn debug_vram_bytes(&self) -> &[u8] {
        self.vram.bytes()
    }

    /// Returns the raw OAM backing bytes for deterministic debug probes.
    ///
    /// This is intentionally not a CPU bus read: it bypasses live PPU/DMA arbitration so external tooling can compare emulator state without perturbing the machine or conflating blocked CPU visibility with actual storage.
    pub fn debug_oam_bytes(&self) -> &[u8] {
        self.oam.bytes()
    }

    /// Returns the raw WRAM backing bytes for deterministic debug probes.
    ///
    /// This is intentionally not a CPU bus read: it bypasses echo routing and arbitration side effects so external tooling can compare storage state directly.
    pub fn debug_wram_bytes(&self) -> &[u8] {
        self.wram.bytes()
    }

    /// Returns the raw HRAM backing bytes for deterministic debug probes.
    ///
    /// This excludes MMIO registers and the interrupt-enable register; those are captured through subsystem snapshots or non-perturbing cloned reads by tooling that needs CPU-visible values.
    pub fn debug_hram_bytes(&self) -> &[u8] {
        self.iohram.hram_bytes()
    }
}

#[cfg(test)]
mod tests;
