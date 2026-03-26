mod access;
mod iohram;
mod map;
mod policy;
mod router;
mod video;
mod view;
mod wram;

use crate::cartridge::CartridgeSlot;
use crate::cpu::{CpuAddressEvent, CpuAddressEventKind, CpuAddressUpdateDirection};
use crate::model::ConsoleModel;
use crate::ppu::{OamCorruptionEventKind, Ppu, PpuAccessMode, PpuBusState};
use crate::scheduler::CycleContext;
pub(crate) use iohram::{BusIoReadView, BusIoWriteView, IoHramDomain};
pub use map::{
    BusAddressInfo, BusDomain, BusRegion, BusRegionOwner, IoRegisterAccess, IoRegisterAvailability,
    IoRegisterInfo, IoRegisterKind, IoRegisterOwner,
};
pub use router::AddressRouter;
pub(crate) use video::{OamDomain, VramDomain};
pub(crate) use view::{OamBusView, VramBusView};
pub(crate) use wram::WramDomain;

const VRAM_LEN: usize = 0x2000;
const WRAM_LEN: usize = 0x2000;
const OAM_LEN: usize = 0x00A0;
const HRAM_LEN: usize = 0x007F;

const BLOCKED_READ_VALUE: u8 = 0xFF;
const DMG_UNUSABLE_READ_VALUE: u8 = 0x00;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusSnapshot {
    pub console_model: ConsoleModel,
    pub status: BusStatus,
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

    pub fn decode_address(&self, address: u16) -> BusAddressInfo {
        self.router.decode_address(address)
    }

    pub fn describe_io_register(&self, address: u16) -> Option<IoRegisterInfo> {
        self.router.describe_io_register(address)
    }

    pub fn resolve_access(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        address: u16,
        state: &BusArbitrationState,
    ) -> BusAccessResolution {
        let target = self.resolve_nominal_target(kind, address, state);
        let disposition = self.evaluate_access_policy(requester, kind, target, state);

        BusAccessResolution::new(requester, kind, target, disposition)
    }

    pub fn read(&mut self, address: u16) -> u8 {
        self.read_with(address, BusRequester::Cpu, &BusArbitrationState::default())
    }

    pub fn read_with(
        &mut self,
        address: u16,
        requester: BusRequester,
        state: &BusArbitrationState,
    ) -> u8 {
        self.read_with_context(address, requester, state, None, BusIoReadView::default())
    }

    pub fn read_with_cartridge(
        &mut self,
        address: u16,
        requester: BusRequester,
        state: &BusArbitrationState,
        cartridge: Option<&CartridgeSlot>,
    ) -> u8 {
        self.read_with_context(
            address,
            requester,
            state,
            cartridge,
            BusIoReadView::default(),
        )
    }

    pub(crate) fn read_with_context(
        &mut self,
        address: u16,
        requester: BusRequester,
        state: &BusArbitrationState,
        cartridge: Option<&CartridgeSlot>,
        io: BusIoReadView<'_>,
    ) -> u8 {
        if let Some(conflict_source_address) =
            self.cpu_dma_conflict_source_address(requester, address, state)
        {
            let target =
                self.resolve_nominal_target(BusAccessKind::Read, conflict_source_address, state);
            return self.perform_allowed_read(target, cartridge, io);
        }

        let resolution = self.resolve_access(requester, BusAccessKind::Read, address, state);

        match resolution.disposition() {
            BusAccessDisposition::Allowed => {
                self.perform_allowed_read(resolution.target(), cartridge, io)
            }
            BusAccessDisposition::BlockedRead { value, .. } => value,
            BusAccessDisposition::IgnoredWrite { .. } => {
                panic!("read path received write-only access disposition")
            }
        }
    }

    pub fn write(&mut self, address: u16, value: u8) {
        self.write_with(
            address,
            value,
            BusRequester::Cpu,
            &BusArbitrationState::default(),
        );
    }

    pub fn write_with(
        &mut self,
        address: u16,
        value: u8,
        requester: BusRequester,
        state: &BusArbitrationState,
    ) {
        self.write_with_context(
            address,
            value,
            requester,
            state,
            None,
            BusIoWriteView::default(),
        );
    }

    pub fn write_with_cartridge(
        &mut self,
        address: u16,
        value: u8,
        requester: BusRequester,
        state: &BusArbitrationState,
        cartridge: Option<&mut CartridgeSlot>,
    ) {
        self.write_with_context(
            address,
            value,
            requester,
            state,
            cartridge,
            BusIoWriteView::default(),
        );
    }

    pub(crate) fn write_with_context(
        &mut self,
        address: u16,
        value: u8,
        requester: BusRequester,
        state: &BusArbitrationState,
        cartridge: Option<&mut CartridgeSlot>,
        io: BusIoWriteView<'_>,
    ) {
        if let Some(conflict_source_address) =
            self.cpu_dma_conflict_source_address(requester, address, state)
        {
            let target =
                self.resolve_nominal_target(BusAccessKind::Write, conflict_source_address, state);
            self.perform_allowed_write(target, value, cartridge, io);
            return;
        }

        let resolution = self.resolve_access(requester, BusAccessKind::Write, address, state);

        match resolution.disposition() {
            BusAccessDisposition::Allowed => {
                self.perform_allowed_write(resolution.target(), value, cartridge, io)
            }
            BusAccessDisposition::IgnoredWrite { .. } => {}
            BusAccessDisposition::BlockedRead { .. } => {
                panic!("write path received read-only access disposition")
            }
        }
    }

    pub fn snapshot(&self) -> BusSnapshot {
        BusSnapshot {
            console_model: self.console_model,
            status: self.status,
        }
    }

    pub(crate) fn route_cpu_address_event(
        &mut self,
        event: CpuAddressEvent,
        state: &BusArbitrationState,
        ppu: &mut Ppu,
    ) {
        let Some(kind) = self.classify_oam_corruption_event(event, state) else {
            return;
        };

        let _ = ppu.apply_oam_corruption_event(kind, self.oam.bytes_mut());
    }

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

    pub fn scheduler_trace_message(
        &self,
        context: &CycleContext,
        state: &BusArbitrationState,
    ) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} ppu_lcd_enabled={} ppu_mode={:?} dma_cpu_access_policy={:?} dma_active_region={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            state.ppu.is_lcd_enabled(),
            state.ppu.mode(),
            state.dma.cpu_access_policy(),
            state.dma.active_region(),
        )
    }

    fn cpu_dma_conflict_source_address(
        &self,
        requester: BusRequester,
        address: u16,
        state: &BusArbitrationState,
    ) -> Option<u16> {
        if requester != BusRequester::Cpu
            || state.dma.cpu_access_policy() != DmaCpuAccessPolicy::ExternalBusBlocked
            || address >= 0xFE00
        {
            return None;
        }

        let target = self.decode_address(address);
        if !matches!(
            target.region(),
            BusRegion::CartridgeRomBank0
                | BusRegion::CartridgeRomBankN
                | BusRegion::CartridgeExternal
                | BusRegion::WramBank0
                | BusRegion::WramBankN
                | BusRegion::EchoRam
        ) {
            return None;
        }

        state.dma.cpu_conflict_source_address()
    }

    fn resolve_nominal_target(
        &self,
        kind: BusAccessKind,
        address: u16,
        state: &BusArbitrationState,
    ) -> BusAddressInfo {
        self.router.resolve_nominal_target(kind, address, state)
    }

    fn classify_oam_corruption_event(
        &self,
        event: CpuAddressEvent,
        state: &BusArbitrationState,
    ) -> Option<OamCorruptionEventKind> {
        match event.kind {
            CpuAddressEventKind::IncDec => {
                let glitched_address = idu_glitched_address(event)?;
                if self.idu_event_reaches_oam(glitched_address, state) {
                    Some(OamCorruptionEventKind::Write)
                } else {
                    None
                }
            }
            CpuAddressEventKind::Read
            | CpuAddressEventKind::Write
            | CpuAddressEventKind::ReadWithIncDec
            | CpuAddressEventKind::WriteWithIncDec => {
                let access_address = event.access_address?;
                let access_kind = access_kind_for_cpu_address_event(event.kind);
                let resolution =
                    self.resolve_access(BusRequester::Cpu, access_kind, access_address, state);

                let access_hits_corruption = match resolution.target().region() {
                    BusRegion::Oam
                        if resolution.disposition().blocked_reason()
                            == Some(BusBlockReason::PpuOamBlockedDuringMode2) =>
                    {
                        true
                    }
                    BusRegion::Unusable
                        if access_kind == BusAccessKind::Write
                            && state.ppu.is_lcd_enabled()
                            && state.ppu.mode() == PpuAccessMode::OamScan =>
                    {
                        true
                    }
                    BusRegion::Unusable
                        if access_kind == BusAccessKind::Read
                            && resolution.disposition().blocked_reason()
                                == Some(BusBlockReason::UnusableRegionDuringOamBlock) =>
                    {
                        true
                    }
                    _ => false,
                };
                let idu_hits_corruption = matches!(
                    event.kind,
                    CpuAddressEventKind::ReadWithIncDec | CpuAddressEventKind::WriteWithIncDec
                ) && idu_glitched_address(event)
                    .is_some_and(|address| self.idu_event_reaches_oam(address, state));

                if access_hits_corruption || idu_hits_corruption {
                    Some(oam_corruption_event_kind(event.kind))
                } else {
                    None
                }
            }
        }
    }

    fn idu_event_reaches_oam(&self, address: u16, state: &BusArbitrationState) -> bool {
        self.console_model.is_dmg_family()
            && state.ppu.is_lcd_enabled()
            && state.ppu.mode() == PpuAccessMode::OamScan
            && (0xFE00..=0xFEFF).contains(&address)
    }
}

fn access_kind_for_cpu_address_event(kind: CpuAddressEventKind) -> BusAccessKind {
    match kind {
        CpuAddressEventKind::Read | CpuAddressEventKind::ReadWithIncDec => BusAccessKind::Read,
        CpuAddressEventKind::Write | CpuAddressEventKind::WriteWithIncDec => BusAccessKind::Write,
        CpuAddressEventKind::IncDec => {
            unreachable!("pure IDU events do not have an ordinary bus access kind")
        }
    }
}

fn oam_corruption_event_kind(kind: CpuAddressEventKind) -> OamCorruptionEventKind {
    match kind {
        CpuAddressEventKind::Read => OamCorruptionEventKind::Read,
        CpuAddressEventKind::Write => OamCorruptionEventKind::Write,
        CpuAddressEventKind::IncDec => OamCorruptionEventKind::Write,
        CpuAddressEventKind::ReadWithIncDec => OamCorruptionEventKind::ReadWithIncDec,
        CpuAddressEventKind::WriteWithIncDec => OamCorruptionEventKind::WriteWithIncDec,
    }
}

fn idu_glitched_address(event: CpuAddressEvent) -> Option<u16> {
    let driven_address = event.idu_address?;
    match event.update_direction? {
        CpuAddressUpdateDirection::Increment => Some(driven_address.wrapping_sub(1)),
        CpuAddressUpdateDirection::Decrement => Some(driven_address.wrapping_add(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ppu::{DmgObjPaletteReadPolicy, PpuStartupState};
    use crate::scheduler::{CycleContext, TCycle};

    fn sync_test_video_ownership(ppu: &Ppu, oam: &mut OamDomain, vram: &mut VramDomain) {
        let bus_state = ppu.bus_state();
        let ppu_vram = bus_state.is_lcd_enabled() && bus_state.mode() == PpuAccessMode::Drawing;
        let ppu_oam = bus_state.is_lcd_enabled()
            && matches!(
                bus_state.mode(),
                PpuAccessMode::OamScan | PpuAccessMode::Drawing
            );

        oam.set_acquired(BusMaster::Ppu, ppu_oam);
        vram.set_acquired(BusMaster::Ppu, ppu_vram);
        oam.set_acquired(BusMaster::Dma, false);
        vram.set_acquired(BusMaster::Dma, false);
    }

    fn tick_ppu(ppu: &mut Ppu, t_cycle: u64) {
        let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
        let mut oam = OamDomain::new();
        let mut vram = VramDomain::new();
        sync_test_video_ownership(ppu, &mut oam, &mut vram);
        ppu.tick_t_cycle(
            &mut context,
            OamBusView::new(BusMaster::Ppu, &mut oam),
            VramBusView::new(BusMaster::Ppu, &mut vram),
            false,
            None,
        );
    }

    fn prepare_mode2_ppu_at_row(console_model: ConsoleModel, row: u8) -> Ppu {
        let mut ppu = Ppu::new(console_model);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let ticks = if row == 0 { 0 } else { u64::from(row) * 4 + 1 };

        for t_cycle in 0..ticks {
            tick_ppu(&mut ppu, t_cycle);
        }

        assert_eq!(ppu.snapshot().current_oam_scan_row, Some(row));
        ppu
    }

    fn prepare_mode3_ppu(console_model: ConsoleModel) -> Ppu {
        let mut ppu = prepare_mode2_ppu_at_row(console_model, 0);
        for t_cycle in 0..80 {
            tick_ppu(&mut ppu, t_cycle);
        }
        assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
        ppu
    }

    fn write_oam_word_bytes(oam_bytes: &mut [u8], row: u8, word_index: usize, value: u16) {
        let word_start = row as usize * 8 + word_index * 2;
        let [low, high] = value.to_le_bytes();
        oam_bytes[word_start] = low;
        oam_bytes[word_start + 1] = high;
    }

    fn read_oam_word_bytes(oam_bytes: &[u8], row: u8, word_index: usize) -> u16 {
        let word_start = row as usize * 8 + word_index * 2;
        u16::from_le_bytes([oam_bytes[word_start], oam_bytes[word_start + 1]])
    }

    fn seed_oam_corruption_rows(oam_bytes: &mut [u8]) {
        write_oam_word_bytes(oam_bytes, 0, 0, 0x1357);
        write_oam_word_bytes(oam_bytes, 0, 1, 0x2468);
        write_oam_word_bytes(oam_bytes, 0, 2, 0xAAAA);
        write_oam_word_bytes(oam_bytes, 0, 3, 0xBBBB);
        write_oam_word_bytes(oam_bytes, 1, 0, 0x0F0F);
        write_oam_word_bytes(oam_bytes, 1, 1, 0x1111);
        write_oam_word_bytes(oam_bytes, 1, 2, 0x2222);
        write_oam_word_bytes(oam_bytes, 1, 3, 0x3333);
        write_oam_word_bytes(oam_bytes, 2, 0, 0x5555);
        write_oam_word_bytes(oam_bytes, 2, 1, 0x6666);
        write_oam_word_bytes(oam_bytes, 2, 2, 0x7777);
        write_oam_word_bytes(oam_bytes, 2, 3, 0x8888);
    }

    #[test]
    fn decode_address_covers_each_dmg_region_boundary() {
        let bus = Bus::new(ConsoleModel::Dmg);
        let cases = [
            (
                0x0000,
                BusRegion::CartridgeRomBank0,
                BusRegionOwner::Cartridge,
                0x0000,
            ),
            (
                0x3FFF,
                BusRegion::CartridgeRomBank0,
                BusRegionOwner::Cartridge,
                0x3FFF,
            ),
            (
                0x4000,
                BusRegion::CartridgeRomBankN,
                BusRegionOwner::Cartridge,
                0x0000,
            ),
            (
                0x7FFF,
                BusRegion::CartridgeRomBankN,
                BusRegionOwner::Cartridge,
                0x3FFF,
            ),
            (0x8000, BusRegion::Vram, BusRegionOwner::Ppu, 0x0000),
            (0x9FFF, BusRegion::Vram, BusRegionOwner::Ppu, 0x1FFF),
            (
                0xA000,
                BusRegion::CartridgeExternal,
                BusRegionOwner::Cartridge,
                0x0000,
            ),
            (
                0xBFFF,
                BusRegion::CartridgeExternal,
                BusRegionOwner::Cartridge,
                0x1FFF,
            ),
            (0xC000, BusRegion::WramBank0, BusRegionOwner::Bus, 0x0000),
            (0xCFFF, BusRegion::WramBank0, BusRegionOwner::Bus, 0x0FFF),
            (0xD000, BusRegion::WramBankN, BusRegionOwner::Bus, 0x0000),
            (0xDFFF, BusRegion::WramBankN, BusRegionOwner::Bus, 0x0FFF),
            (0xE000, BusRegion::EchoRam, BusRegionOwner::Bus, 0x0000),
            (0xFDFF, BusRegion::EchoRam, BusRegionOwner::Bus, 0x1DFF),
            (0xFE00, BusRegion::Oam, BusRegionOwner::Ppu, 0x0000),
            (0xFE9F, BusRegion::Oam, BusRegionOwner::Ppu, 0x009F),
            (0xFEA0, BusRegion::Unusable, BusRegionOwner::Bus, 0x0000),
            (0xFEFF, BusRegion::Unusable, BusRegionOwner::Bus, 0x005F),
            (0xFF00, BusRegion::Mmio, BusRegionOwner::Mmio, 0x0000),
            (0xFF7F, BusRegion::Mmio, BusRegionOwner::Mmio, 0x007F),
            (0xFF80, BusRegion::Hram, BusRegionOwner::Bus, 0x0000),
            (0xFFFE, BusRegion::Hram, BusRegionOwner::Bus, 0x007E),
            (
                0xFFFF,
                BusRegion::InterruptEnable,
                BusRegionOwner::InterruptController,
                0x0000,
            ),
        ];

        for (address, region, owner, region_offset) in cases {
            let decoded = bus.decode_address(address);
            assert_eq!(decoded.address(), address);
            assert_eq!(decoded.region(), region);
            assert_eq!(decoded.owner(), owner);
            assert_eq!(decoded.region_offset(), region_offset);
        }
    }

    #[test]
    fn resolve_access_uses_boot_overlay_for_reads_but_not_for_writes() {
        let bus = Bus::new(ConsoleModel::Dmg);
        let state =
            BusArbitrationState::default().with_boot_rom(BootRomBusState::map_dmg_low_bytes());

        let read = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x0000, &state);
        let write = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0x0000, &state);

        assert_eq!(read.target().region(), BusRegion::BootRom);
        assert_eq!(read.target().owner(), BusRegionOwner::Boot);
        assert!(read.disposition().is_allowed());
        assert_eq!(write.target().region(), BusRegion::CartridgeRomBank0);
        assert_eq!(write.target().owner(), BusRegionOwner::Cartridge);
        assert!(write.disposition().is_allowed());
    }

    #[test]
    fn resolve_access_keeps_nominal_target_and_policy_separate() {
        let bus = Bus::new(ConsoleModel::Dmg);
        let state = BusArbitrationState::default()
            .with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::OamScan));

        let resolution = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFE00, &state);

        assert_eq!(resolution.target().region(), BusRegion::Oam);
        assert_eq!(resolution.target().owner(), BusRegionOwner::Ppu);
        assert_eq!(
            resolution.disposition(),
            BusAccessDisposition::BlockedRead {
                value: BLOCKED_READ_VALUE,
                reason: BusBlockReason::PpuOamBlockedDuringMode2,
            }
        );
    }

    #[test]
    fn echo_ram_aliases_shared_wram_storage() {
        let mut bus = Bus::new(ConsoleModel::Dmg);

        bus.write(0xC123, 0x42);
        assert_eq!(bus.read(0xE123), 0x42);

        bus.write(0xFDFF, 0x7E);
        assert_eq!(bus.read(0xDDFF), 0x7E);
    }

    #[test]
    fn cartridge_mmio_and_unusable_placeholders_do_not_behave_like_storage() {
        let mut bus = Bus::new(ConsoleModel::Dmg);

        bus.write(0x0000, 0x12);
        bus.write(0x4000, 0x23);
        bus.write(0xA000, 0x34);
        bus.write(0xFF10, 0x45);
        bus.write(0xFEA0, 0x56);

        assert_eq!(bus.read(0x0000), BLOCKED_READ_VALUE);
        assert_eq!(bus.read(0x4000), BLOCKED_READ_VALUE);
        assert_eq!(bus.read(0xA000), BLOCKED_READ_VALUE);
        assert_eq!(bus.read(0xFF10), BLOCKED_READ_VALUE);
        assert_eq!(bus.read(0xFEA0), DMG_UNUSABLE_READ_VALUE);
    }

    #[test]
    fn io_contract_table_covers_ff00_ff7f_and_ie() {
        let bus = Bus::new(ConsoleModel::Dmg);

        for address in 0xFF00..=0xFF7F {
            assert!(
                bus.describe_io_register(address).is_some(),
                "missing IO contract for {address:#06X}"
            );
        }

        let ff46 = bus.describe_io_register(0xFF46).unwrap();
        let ff50 = bus.describe_io_register(0xFF50).unwrap();
        let ie = bus.describe_io_register(0xFFFF).unwrap();

        assert_eq!(ff46.owner(), IoRegisterOwner::Dma);
        assert_eq!(ff46.kind(), IoRegisterKind::OamDma);
        assert_eq!(ff50.owner(), IoRegisterOwner::Boot);
        assert_eq!(ff50.access(), IoRegisterAccess::WriteOnly);
        assert_eq!(ie.kind(), IoRegisterKind::InterruptEnable);
    }

    #[test]
    fn dmg_cgb_only_io_fallback_reads_as_ff() {
        let bus = Bus::new(ConsoleModel::Dmg);

        assert_eq!(bus.read_io_target(0xFF4D, BusIoReadView::default()), 0xFF);
        assert_eq!(bus.read_io_target(0xFF70, BusIoReadView::default()), 0xFF);
    }

    #[test]
    fn video_bus_dma_policy_has_precedence_over_ppu_region_rules() {
        let bus = Bus::new(ConsoleModel::Dmg);
        let state = BusArbitrationState::default()
            .with_dma(DmaBusState::video_bus_blocked(Some(
                DmaMemoryRegionImpact::Oam,
            )))
            .with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::Drawing));

        let resolution = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x8000, &state);

        assert_eq!(resolution.target().region(), BusRegion::Vram);
        assert_eq!(
            resolution.disposition().blocked_reason(),
            Some(BusBlockReason::DmaVideoBusConflict)
        );
    }

    #[test]
    fn external_bus_dma_policy_keeps_ff46_readable_and_writable_during_active_dma() {
        let bus = Bus::new(ConsoleModel::Dmg);
        let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(
            Some(DmaMemoryRegionImpact::Oam),
        ));

        let read_resolution =
            bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFF46, &state);
        assert_eq!(read_resolution.target().region(), BusRegion::Mmio);
        assert!(read_resolution.disposition().is_allowed());

        let write_resolution =
            bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0xFF46, &state);
        assert_eq!(write_resolution.target().region(), BusRegion::Mmio);
        assert!(write_resolution.disposition().is_allowed());
    }

    #[test]
    fn route_cpu_address_event_turns_mode2_oam_reads_into_corruption_events() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 1);
        seed_oam_corruption_rows(bus.oam.bytes_mut());

        let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
        bus.route_cpu_address_event(
            CpuAddressEvent {
                kind: CpuAddressEventKind::Read,
                access_address: Some(0xFE20),
                idu_address: None,
                update_direction: None,
            },
            &state,
            &mut ppu,
        );

        let expected_first = 0x1357_u16 | (0x0F0F & 0xAAAA);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 0), expected_first);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 1), 0x2468);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 2), 0xAAAA);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 3), 0xBBBB);
    }

    #[test]
    fn route_cpu_address_event_uses_the_unusable_mode2_read_path_for_corruption() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 1);
        seed_oam_corruption_rows(bus.oam.bytes_mut());

        let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
        bus.route_cpu_address_event(
            CpuAddressEvent {
                kind: CpuAddressEventKind::Read,
                access_address: Some(0xFEA0),
                idu_address: None,
                update_direction: None,
            },
            &state,
            &mut ppu,
        );

        let expected_first = 0x1357_u16 | (0x0F0F & 0xAAAA);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 0), expected_first);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 1), 0x2468);
    }

    #[test]
    fn route_cpu_address_event_uses_the_unusable_mode2_write_path_for_corruption() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 1);
        seed_oam_corruption_rows(bus.oam.bytes_mut());

        let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
        bus.route_cpu_address_event(
            CpuAddressEvent {
                kind: CpuAddressEventKind::Write,
                access_address: Some(0xFEA0),
                idu_address: None,
                update_direction: None,
            },
            &state,
            &mut ppu,
        );

        let expected_first = ((0x0F0F_u16 ^ 0xAAAA) & (0x1357 ^ 0xAAAA)) ^ 0xAAAA;
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 0), expected_first);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 1), 0x2468);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 2), 0xAAAA);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 3), 0xBBBB);
    }

    #[test]
    fn route_cpu_address_event_uses_pure_idu_activity_in_fe_range() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 2);
        seed_oam_corruption_rows(bus.oam.bytes_mut());

        let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
        bus.route_cpu_address_event(
            CpuAddressEvent {
                kind: CpuAddressEventKind::IncDec,
                access_address: None,
                idu_address: Some(0xFE11),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            },
            &state,
            &mut ppu,
        );

        let expected_first = ((0x5555_u16 ^ 0x2222) & (0x0F0F ^ 0x2222)) ^ 0x2222;
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 0), expected_first);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 1), 0x1111);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 2), 0x2222);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 3), 0x3333);
    }

    #[test]
    fn route_cpu_address_event_uses_write_with_incdec_when_the_idu_edge_reaches_oam() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 2);
        seed_oam_corruption_rows(bus.oam.bytes_mut());

        let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
        bus.route_cpu_address_event(
            CpuAddressEvent {
                kind: CpuAddressEventKind::WriteWithIncDec,
                access_address: Some(0xFDFF),
                idu_address: Some(0xFDFF),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
            },
            &state,
            &mut ppu,
        );

        let expected_first = ((0x5555_u16 ^ 0x2222) & (0x0F0F ^ 0x2222)) ^ 0x2222;
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 0), expected_first);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 1), 0x1111);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 2), 0x2222);
        assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 3), 0x3333);
    }

    #[test]
    fn route_cpu_address_event_does_not_turn_mode3_oam_blocking_into_corruption() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode3_ppu(ConsoleModel::Dmg);
        seed_oam_corruption_rows(bus.oam.bytes_mut());
        let before = bus.oam.clone();

        let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
        bus.route_cpu_address_event(
            CpuAddressEvent {
                kind: CpuAddressEventKind::Read,
                access_address: Some(0xFE20),
                idu_address: None,
                update_direction: None,
            },
            &state,
            &mut ppu,
        );

        assert_eq!(bus.oam, before);
    }
}
