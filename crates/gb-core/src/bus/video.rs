use crate::boot::StartupMemoryPolicy;
use crate::model::ConsoleModel;
use crate::ppu::{PpuAccessMode, PpuBusState};

#[cfg(any(debug_assertions, test))]
use super::DmaMemoryRegionImpact;
use super::{
    BLOCKED_READ_VALUE, Bus, BusAccessDisposition, BusAccessKind, BusBlockReason, BusMaster,
    BusRequester, CGB_VRAM_LEN, DMG_VRAM_LEN, DmaBusState, DmaCpuAccessPolicy, OAM_LEN, OamBusView,
    VramBusView,
};

type BusMasterMask = u16;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct OamDomain {
    #[serde(with = "serde_big_array::BigArray")]
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

    #[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
    pub(crate) fn acquire(&mut self, master: BusMaster) {
        self.acquired_by |= master_mask(master);
    }

    #[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
    pub(crate) fn release(&mut self, master: BusMaster) {
        self.acquired_by &= !master_mask(master);
    }

    #[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct VramDomain {
    console_model: ConsoleModel,
    selected_bank: u8,
    #[serde(with = "serde_big_array::BigArray")]
    bytes: [u8; CGB_VRAM_LEN],
    acquired_by: BusMasterMask,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct VramSaveState {
    console_model: ConsoleModel,
    selected_bank: u8,
    bytes: Vec<u8>,
    acquired_by: BusMasterMask,
}

impl VramSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.bytes.len()
    }
}

impl VramDomain {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::new_for_model(ConsoleModel::GameBoy)
    }

    pub(crate) fn new_for_model(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            selected_bank: 0,
            bytes: [0; CGB_VRAM_LEN],
            acquired_by: 0,
        }
    }

    pub(crate) fn apply_startup_memory_policy(&mut self, policy: StartupMemoryPolicy) {
        policy.initialize_vram(self.debug_bytes_mut());
    }

    #[cfg(test)]
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_bytes_for_model(ConsoleModel::GameBoy, bytes)
    }

    #[cfg(test)]
    pub(crate) fn from_bytes_for_model(console_model: ConsoleModel, bytes: &[u8]) -> Self {
        let mut domain = Self::new_for_model(console_model);
        let copy_len = bytes.len().min(domain.debug_bytes().len());
        domain.bytes[..copy_len].copy_from_slice(&bytes[..copy_len]);
        domain
    }

    pub(crate) fn read(&self, offset: usize) -> u8 {
        self.bytes[self.storage_index(offset)]
    }

    pub(crate) fn read_bank(&self, bank: u8, offset: usize) -> u8 {
        self.bytes[self.storage_index_for_bank(bank, offset)]
    }

    pub(crate) fn write(&mut self, offset: usize, value: u8) {
        let index = self.storage_index(offset);
        self.bytes[index] = value;
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes[..DMG_VRAM_LEN]
    }

    pub(crate) fn debug_bytes(&self) -> &[u8] {
        let len = if self.console_model.is_cgb_family() {
            CGB_VRAM_LEN
        } else {
            DMG_VRAM_LEN
        };
        &self.bytes[..len]
    }

    fn debug_bytes_mut(&mut self) -> &mut [u8] {
        let len = if self.console_model.is_cgb_family() {
            CGB_VRAM_LEN
        } else {
            DMG_VRAM_LEN
        };
        &mut self.bytes[..len]
    }

    pub(crate) fn capture_save_state(&self) -> VramSaveState {
        VramSaveState {
            console_model: self.console_model,
            selected_bank: self.selected_bank,
            bytes: self.debug_bytes().to_vec(),
            acquired_by: self.acquired_by,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &VramSaveState) {
        self.console_model = state.console_model;
        self.selected_bank = state.selected_bank & 0x01;
        self.bytes.fill(0);
        let copy_len = state.bytes.len().min(self.bytes.len());
        self.bytes[..copy_len].copy_from_slice(&state.bytes[..copy_len]);
        self.acquired_by = state.acquired_by;
    }

    pub(crate) fn read_vbk(&self) -> u8 {
        0xFE | self.selected_bank
    }

    pub(crate) fn write_vbk(&mut self, value: u8) {
        if self.console_model.is_cgb_family() {
            self.selected_bank = value & 0x01;
        }
    }

    pub(crate) fn reset_bank_select(&mut self) {
        self.selected_bank = 0;
    }

    fn storage_index(&self, offset: usize) -> usize {
        self.storage_index_for_bank(self.selected_bank, offset)
    }

    fn storage_index_for_bank(&self, bank: u8, offset: usize) -> usize {
        debug_assert!(offset < DMG_VRAM_LEN);
        let bank = if self.console_model.is_cgb_family() {
            usize::from(bank & 0x01)
        } else {
            0
        };
        bank * DMG_VRAM_LEN + offset
    }

    #[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
    pub(crate) fn acquire(&mut self, master: BusMaster) {
        self.acquired_by |= master_mask(master);
    }

    #[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
    pub(crate) fn release(&mut self, master: BusMaster) {
        self.acquired_by &= !master_mask(master);
    }

    #[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
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

    #[cfg(not(any(debug_assertions, test)))]
    pub(crate) fn sync_video_domain_ownership(&mut self, _ppu: PpuBusState, _dma: DmaBusState) {}

    #[cfg(any(debug_assertions, test))]
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
