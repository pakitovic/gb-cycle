use crate::ppu::{PpuAccessMode, PpuBusState};

use super::{
    BLOCKED_READ_VALUE, Bus, BusAccessDisposition, BusAccessKind, BusBlockReason, BusMaster,
    BusRequester, DmaBusState, DmaCpuAccessPolicy, DmaMemoryRegionImpact, OAM_LEN, OamBusView,
    VRAM_LEN, VramBusView,
};

type BusMasterMask = u16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OamDomain {
    bytes: [u8; OAM_LEN],
    acquired_by: BusMasterMask,
}

impl OamDomain {
    pub(crate) fn new() -> Self {
        Self {
            bytes: [0; OAM_LEN],
            acquired_by: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        let mut domain = Self::new();
        let copy_len = bytes.len().min(domain.bytes.len());
        domain.bytes[..copy_len].copy_from_slice(&bytes[..copy_len]);
        domain
    }

    pub(crate) fn read(&self, offset: usize) -> u8 {
        self.bytes[offset]
    }

    pub(crate) fn write(&mut self, offset: usize, value: u8) {
        self.bytes[offset] = value;
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub(crate) fn acquire(&mut self, master: BusMaster) {
        self.acquired_by |= master_mask(master);
    }

    pub(crate) fn release(&mut self, master: BusMaster) {
        self.acquired_by &= !master_mask(master);
    }

    pub(crate) fn set_acquired(&mut self, master: BusMaster, acquired: bool) {
        if acquired {
            self.acquire(master);
        } else {
            self.release(master);
        }
    }

    #[allow(dead_code)]
    #[allow(dead_code)]
    pub(crate) fn is_acquired(&self) -> bool {
        self.acquired_by != 0
    }

    pub(crate) fn is_acquired_by(&self, master: BusMaster) -> bool {
        self.acquired_by & master_mask(master) != 0
    }

    pub(crate) fn evaluate_access(
        requester: BusRequester,
        kind: BusAccessKind,
        ppu: PpuBusState,
        dma: DmaBusState,
    ) -> Option<BusAccessDisposition> {
        if requester != BusRequester::Cpu {
            return None;
        }

        if dma.cpu_access_policy() == DmaCpuAccessPolicy::VideoBusBlocked {
            return Some(block_access(kind, BusBlockReason::DmaVideoBusConflict));
        }

        if !ppu.is_lcd_enabled() {
            return None;
        }

        match ppu.mode() {
            PpuAccessMode::OamScan => {
                Some(block_access(kind, BusBlockReason::PpuOamBlockedDuringMode2))
            }
            PpuAccessMode::Drawing => {
                Some(block_access(kind, BusBlockReason::PpuOamBlockedDuringMode3))
            }
            PpuAccessMode::HBlank | PpuAccessMode::VBlank => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VramDomain {
    bytes: [u8; VRAM_LEN],
    acquired_by: BusMasterMask,
}

impl VramDomain {
    pub(crate) fn new() -> Self {
        Self {
            bytes: [0; VRAM_LEN],
            acquired_by: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        let mut domain = Self::new();
        let copy_len = bytes.len().min(domain.bytes.len());
        domain.bytes[..copy_len].copy_from_slice(&bytes[..copy_len]);
        domain
    }

    pub(crate) fn read(&self, offset: usize) -> u8 {
        self.bytes[offset]
    }

    pub(crate) fn write(&mut self, offset: usize, value: u8) {
        self.bytes[offset] = value;
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn acquire(&mut self, master: BusMaster) {
        self.acquired_by |= master_mask(master);
    }

    pub(crate) fn release(&mut self, master: BusMaster) {
        self.acquired_by &= !master_mask(master);
    }

    pub(crate) fn set_acquired(&mut self, master: BusMaster, acquired: bool) {
        if acquired {
            self.acquire(master);
        } else {
            self.release(master);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_acquired(&self) -> bool {
        self.acquired_by != 0
    }

    pub(crate) fn is_acquired_by(&self, master: BusMaster) -> bool {
        self.acquired_by & master_mask(master) != 0
    }

    pub(crate) fn evaluate_access(
        requester: BusRequester,
        kind: BusAccessKind,
        ppu: PpuBusState,
        dma: DmaBusState,
    ) -> Option<BusAccessDisposition> {
        if requester != BusRequester::Cpu {
            return None;
        }

        if dma.cpu_access_policy() == DmaCpuAccessPolicy::VideoBusBlocked {
            return Some(block_access(kind, BusBlockReason::DmaVideoBusConflict));
        }

        if ppu.is_lcd_enabled() && ppu.mode() == PpuAccessMode::Drawing {
            return Some(block_access(
                kind,
                BusBlockReason::PpuVramBlockedDuringMode3,
            ));
        }

        None
    }
}

impl Bus {
    pub(crate) fn video_views(&mut self, master: BusMaster) -> (OamBusView<'_>, VramBusView<'_>) {
        (
            OamBusView::new(master, &mut self.oam),
            VramBusView::new(master, &mut self.vram),
        )
    }

    pub(crate) fn sync_video_domain_ownership(&mut self, ppu: PpuBusState, dma: DmaBusState) {
        let ppu_vram = ppu.is_lcd_enabled() && ppu.mode() == PpuAccessMode::Drawing;
        let ppu_oam = ppu.is_lcd_enabled()
            && matches!(ppu.mode(), PpuAccessMode::OamScan | PpuAccessMode::Drawing);
        let dma_oam = dma.active_region() == Some(DmaMemoryRegionImpact::Oam);
        let dma_vram = dma.active_region() == Some(DmaMemoryRegionImpact::Vram);

        self.vram.set_acquired(BusMaster::Ppu, ppu_vram);
        self.oam.set_acquired(BusMaster::Ppu, ppu_oam);
        self.oam.set_acquired(BusMaster::Dma, dma_oam);
        self.vram.set_acquired(BusMaster::Dma, dma_vram);
    }
}

const fn master_mask(master: BusMaster) -> BusMasterMask {
    match master {
        BusMaster::Cpu => 1 << 0,
        BusMaster::Dma => 1 << 1,
        BusMaster::Ppu => 1 << 2,
        BusMaster::Apu => 1 << 3,
        BusMaster::Serial => 1 << 4,
        BusMaster::Boot => 1 << 5,
        BusMaster::Cartridge => 1 << 6,
    }
}

const fn block_access(kind: BusAccessKind, reason: BusBlockReason) -> BusAccessDisposition {
    match kind {
        BusAccessKind::Read => BusAccessDisposition::BlockedRead {
            value: BLOCKED_READ_VALUE,
            reason,
        },
        BusAccessKind::Write => BusAccessDisposition::IgnoredWrite { reason },
    }
}
