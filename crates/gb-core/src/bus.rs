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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bus {
    console_model: ConsoleModel,
    status: BusStatus,
    router: AddressRouter,
    vram: VramDomain,
    wram: WramDomain,
    oam: OamDomain,
    iohram: IoHramDomain,
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
}

#[cfg(test)]
mod tests;
