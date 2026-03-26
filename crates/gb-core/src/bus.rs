mod router;
mod view;

use crate::apu::Apu;
use crate::boot::{BootController, StartupMemoryPolicy};
use crate::cartridge::CartridgeSlot;
use crate::cpu::{CpuAddressEvent, CpuAddressEventKind, CpuAddressUpdateDirection};
use crate::dma::DmaController;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::model::ConsoleModel;
use crate::ppu::{OamCorruptionEventKind, Ppu, PpuAccessMode, PpuBusState};
use crate::scheduler::CycleContext;
use crate::serial::Serial;
use crate::timer::Timer;
pub use router::AddressRouter;
pub(crate) use view::{OamBusView, VramBusView};

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
    CgbOnly,
    Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoRegisterAvailability {
    AllModels,
    CgbOnly,
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
    Sound,
    Lcd,
    OamDma,
    BootRomDisable,
    CgbSystem,
    Reserved,
    InterruptEnable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IoRegisterInfo {
    address: u16,
    owner: IoRegisterOwner,
    availability: IoRegisterAvailability,
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
        Self {
            address,
            owner,
            availability,
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

    pub const fn access(self) -> IoRegisterAccess {
        self.access
    }

    pub const fn kind(self) -> IoRegisterKind {
        self.kind
    }
}

#[derive(Default)]
pub(crate) struct BusIoReadView<'a> {
    pub apu: Option<&'a Apu>,
    pub timer: Option<&'a Timer>,
    pub serial: Option<&'a Serial>,
    pub dma: Option<&'a DmaController>,
    pub boot: Option<&'a BootController>,
    pub interrupts: Option<&'a InterruptController>,
    pub interrupt_flag_pending_mask: u8,
    pub joypad: Option<&'a Joypad>,
    pub ppu: Option<&'a Ppu>,
}

#[derive(Default)]
pub(crate) struct BusIoWriteView<'a> {
    pub apu: Option<&'a mut Apu>,
    pub timer: Option<&'a mut Timer>,
    pub serial: Option<&'a mut Serial>,
    pub dma: Option<&'a mut DmaController>,
    pub boot: Option<&'a mut BootController>,
    pub interrupts: Option<&'a mut InterruptController>,
    pub joypad: Option<&'a mut Joypad>,
    pub ppu: Option<&'a mut Ppu>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bus {
    console_model: ConsoleModel,
    status: BusStatus,
    router: AddressRouter,
    vram: [u8; VRAM_LEN],
    wram: [u8; WRAM_LEN],
    oam: [u8; OAM_LEN],
    hram: [u8; HRAM_LEN],
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
            vram: [0; VRAM_LEN],
            wram: [0; WRAM_LEN],
            oam: [0; OAM_LEN],
            hram: [0; HRAM_LEN],
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

        let _ = ppu.apply_oam_corruption_event(kind, &mut self.oam);
    }

    pub(crate) fn oam_view(&self, master: BusMaster) -> OamBusView<'_> {
        OamBusView::new(master, &self.oam)
    }

    pub(crate) fn vram_view(&self, master: BusMaster) -> VramBusView<'_> {
        VramBusView::new(master, &self.vram)
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

    fn evaluate_access_policy(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        target: BusAddressInfo,
        state: &BusArbitrationState,
    ) -> BusAccessDisposition {
        if let Some(disposition) = self.evaluate_dma_policy(requester, kind, target, state) {
            return disposition;
        }

        if let Some(disposition) = self.evaluate_ppu_policy(requester, kind, target, state) {
            return disposition;
        }

        if let Some(disposition) = self.evaluate_unusable_policy(requester, kind, target, state) {
            return disposition;
        }

        BusAccessDisposition::Allowed
    }

    fn evaluate_dma_policy(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        target: BusAddressInfo,
        state: &BusArbitrationState,
    ) -> Option<BusAccessDisposition> {
        if requester != BusRequester::Cpu {
            return None;
        }

        match state.dma.cpu_access_policy() {
            DmaCpuAccessPolicy::Unrestricted => None,
            DmaCpuAccessPolicy::ExternalBusBlocked => {
                if target.region() == BusRegion::Hram
                    || target.region() == BusRegion::Vram
                    || target.address() == 0xFF46
                {
                    return None;
                }

                Some(self.block_access(kind, BusBlockReason::DmaExternalBusConflict))
            }
            DmaCpuAccessPolicy::VideoBusBlocked => {
                if matches!(target.region(), BusRegion::Vram | BusRegion::Oam) {
                    Some(self.block_access(kind, BusBlockReason::DmaVideoBusConflict))
                } else {
                    None
                }
            }
        }
    }

    fn evaluate_ppu_policy(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        target: BusAddressInfo,
        state: &BusArbitrationState,
    ) -> Option<BusAccessDisposition> {
        if requester != BusRequester::Cpu || !state.ppu.is_lcd_enabled() {
            return None;
        }

        match (target.region(), state.ppu.mode()) {
            (BusRegion::Vram, PpuAccessMode::Drawing) => {
                Some(self.block_access(kind, BusBlockReason::PpuVramBlockedDuringMode3))
            }
            (BusRegion::Oam, PpuAccessMode::OamScan) => {
                Some(self.block_access(kind, BusBlockReason::PpuOamBlockedDuringMode2))
            }
            (BusRegion::Oam, PpuAccessMode::Drawing) => {
                Some(self.block_access(kind, BusBlockReason::PpuOamBlockedDuringMode3))
            }
            _ => None,
        }
    }

    fn evaluate_unusable_policy(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        target: BusAddressInfo,
        state: &BusArbitrationState,
    ) -> Option<BusAccessDisposition> {
        if target.region() != BusRegion::Unusable {
            return None;
        }

        if kind == BusAccessKind::Write {
            return Some(BusAccessDisposition::IgnoredWrite {
                reason: BusBlockReason::UnusableRegion,
            });
        }

        if requester == BusRequester::Cpu
            && state.ppu.is_lcd_enabled()
            && matches!(
                state.ppu.mode(),
                PpuAccessMode::OamScan | PpuAccessMode::Drawing
            )
        {
            return Some(BusAccessDisposition::BlockedRead {
                value: BLOCKED_READ_VALUE,
                reason: BusBlockReason::UnusableRegionDuringOamBlock,
            });
        }

        None
    }

    fn perform_allowed_read(
        &self,
        target: BusAddressInfo,
        cartridge: Option<&CartridgeSlot>,
        io: BusIoReadView<'_>,
    ) -> u8 {
        match target.region() {
            BusRegion::BootRom => self.read_boot_rom_placeholder(target.address(), io),
            BusRegion::CartridgeRomBank0
            | BusRegion::CartridgeRomBankN
            | BusRegion::CartridgeExternal => {
                self.read_cartridge_target(target.address(), target.region(), cartridge)
            }
            BusRegion::Vram => self.vram[target.region_offset() as usize],
            BusRegion::WramBank0 | BusRegion::WramBankN | BusRegion::EchoRam => {
                self.wram[self.wram_index(target.address())]
            }
            BusRegion::Oam => self.oam[target.region_offset() as usize],
            BusRegion::Unusable => self.read_unusable_placeholder(),
            BusRegion::Mmio | BusRegion::InterruptEnable => {
                self.read_io_target(target.address(), io)
            }
            BusRegion::Hram => self.hram[target.region_offset() as usize],
        }
    }

    fn perform_allowed_write(
        &mut self,
        target: BusAddressInfo,
        value: u8,
        cartridge: Option<&mut CartridgeSlot>,
        io: BusIoWriteView<'_>,
    ) {
        match target.region() {
            BusRegion::BootRom => unreachable!("boot ROM overlay must not own writes"),
            BusRegion::CartridgeRomBank0
            | BusRegion::CartridgeRomBankN
            | BusRegion::CartridgeExternal => {
                self.write_cartridge_target(target.address(), target.region(), value, cartridge)
            }
            BusRegion::Vram => {
                self.vram[target.region_offset() as usize] = value;
            }
            BusRegion::WramBank0 | BusRegion::WramBankN | BusRegion::EchoRam => {
                let index = self.wram_index(target.address());
                self.wram[index] = value;
            }
            BusRegion::Oam => {
                self.oam[target.region_offset() as usize] = value;
            }
            BusRegion::Unusable => {}
            BusRegion::Mmio | BusRegion::InterruptEnable => {
                self.write_io_target(target.address(), value, io)
            }
            BusRegion::Hram => {
                self.hram[target.region_offset() as usize] = value;
            }
        }
    }

    pub fn apply_startup_memory_policy(&mut self, policy: StartupMemoryPolicy) {
        match policy {
            StartupMemoryPolicy::DeterministicZeroed => {
                self.wram.fill(0);
                self.hram.fill(0);
            }
        }
    }

    fn read_boot_rom_placeholder(&self, address: u16, io: BusIoReadView<'_>) -> u8 {
        io.boot
            .map_or(BLOCKED_READ_VALUE, |boot| boot.read_boot_rom(address))
    }

    fn read_cartridge_target(
        &self,
        address: u16,
        region: BusRegion,
        cartridge: Option<&CartridgeSlot>,
    ) -> u8 {
        match cartridge {
            Some(cartridge) => match region {
                BusRegion::CartridgeRomBank0 | BusRegion::CartridgeRomBankN => {
                    cartridge.read_rom(address)
                }
                BusRegion::CartridgeExternal => cartridge.read_ram(address),
                _ => unreachable!("non-cartridge region routed to cartridge target"),
            },
            None => BLOCKED_READ_VALUE,
        }
    }

    fn write_cartridge_target(
        &mut self,
        address: u16,
        region: BusRegion,
        value: u8,
        cartridge: Option<&mut CartridgeSlot>,
    ) {
        if let Some(cartridge) = cartridge {
            match region {
                BusRegion::CartridgeRomBank0 | BusRegion::CartridgeRomBankN => {
                    cartridge.write_rom(address, value)
                }
                BusRegion::CartridgeExternal => cartridge.write_ram(address, value),
                _ => unreachable!("non-cartridge region routed to cartridge target"),
            }
        }
    }

    fn read_unusable_placeholder(&self) -> u8 {
        if self.console_model.is_dmg_family() {
            DMG_UNUSABLE_READ_VALUE
        } else {
            BLOCKED_READ_VALUE
        }
    }

    fn read_io_target(&self, address: u16, io: BusIoReadView<'_>) -> u8 {
        let Some(info) = self.describe_io_register(address) else {
            return BLOCKED_READ_VALUE;
        };

        if info.availability() == IoRegisterAvailability::CgbOnly
            && self.console_model.is_dmg_family()
        {
            return BLOCKED_READ_VALUE;
        }

        match info.kind() {
            IoRegisterKind::Joyp => io.joypad.map_or(BLOCKED_READ_VALUE, Joypad::read_p1),
            IoRegisterKind::SerialData => io.serial.map_or(BLOCKED_READ_VALUE, Serial::read_sb),
            IoRegisterKind::SerialControl => io.serial.map_or(BLOCKED_READ_VALUE, Serial::read_sc),
            IoRegisterKind::Div => io.timer.map_or(BLOCKED_READ_VALUE, Timer::read_div),
            IoRegisterKind::Tima => io.timer.map_or(BLOCKED_READ_VALUE, Timer::read_tima),
            IoRegisterKind::Tma => io.timer.map_or(BLOCKED_READ_VALUE, Timer::read_tma),
            IoRegisterKind::Tac => io.timer.map_or(BLOCKED_READ_VALUE, Timer::read_tac),
            IoRegisterKind::InterruptFlag => {
                io.interrupts.map_or(BLOCKED_READ_VALUE, |interrupts| {
                    interrupts.read_if_with_pending_requests(io.interrupt_flag_pending_mask)
                })
            }
            IoRegisterKind::Lcd => io
                .ppu
                .map_or(BLOCKED_READ_VALUE, |ppu| ppu.read_register(address)),
            IoRegisterKind::OamDma => io.dma.map_or(BLOCKED_READ_VALUE, DmaController::read_ff46),
            IoRegisterKind::BootRomDisable => io
                .boot
                .map_or(BLOCKED_READ_VALUE, BootController::read_ff50),
            IoRegisterKind::InterruptEnable => io
                .interrupts
                .map_or(BLOCKED_READ_VALUE, InterruptController::read_ie),
            IoRegisterKind::Sound => io
                .apu
                .map_or(BLOCKED_READ_VALUE, |apu| apu.read_register(address)),
            IoRegisterKind::CgbSystem | IoRegisterKind::Reserved => BLOCKED_READ_VALUE,
        }
    }

    fn write_io_target(&mut self, address: u16, value: u8, io: BusIoWriteView<'_>) {
        let Some(info) = self.describe_io_register(address) else {
            return;
        };

        if info.availability() == IoRegisterAvailability::CgbOnly
            && self.console_model.is_dmg_family()
        {
            return;
        }

        match info.kind() {
            IoRegisterKind::Joyp => {
                if let Some(joypad) = io.joypad {
                    joypad.write_p1(value);
                }
            }
            IoRegisterKind::SerialData => {
                if let Some(serial) = io.serial {
                    serial.write_sb(value);
                }
            }
            IoRegisterKind::SerialControl => {
                if let Some(serial) = io.serial {
                    serial.write_sc(value);
                }
            }
            IoRegisterKind::Div => {
                if let Some(timer) = io.timer {
                    timer.write_div(value);
                }
            }
            IoRegisterKind::Tima => {
                if let Some(timer) = io.timer {
                    timer.write_tima(value);
                }
            }
            IoRegisterKind::Tma => {
                if let Some(timer) = io.timer {
                    timer.write_tma(value);
                }
            }
            IoRegisterKind::Tac => {
                if let Some(timer) = io.timer {
                    timer.write_tac(value);
                }
            }
            IoRegisterKind::InterruptFlag => {
                if let Some(interrupts) = io.interrupts {
                    interrupts.write_if(value);
                }
            }
            IoRegisterKind::Lcd => {
                if let Some(ppu) = io.ppu {
                    ppu.write_register(address, value);
                }
            }
            IoRegisterKind::OamDma => {
                if let Some(dma) = io.dma {
                    dma.write_ff46(value);
                }
            }
            IoRegisterKind::BootRomDisable => {
                if let Some(boot) = io.boot {
                    boot.write_ff50(value);
                }
            }
            IoRegisterKind::InterruptEnable => {
                if let Some(interrupts) = io.interrupts {
                    interrupts.write_ie(value);
                }
            }
            IoRegisterKind::Sound => {
                if let Some(apu) = io.apu {
                    apu.write_register(address, value);
                }
            }
            IoRegisterKind::CgbSystem | IoRegisterKind::Reserved => {}
        }
    }

    fn block_access(&self, kind: BusAccessKind, reason: BusBlockReason) -> BusAccessDisposition {
        match kind {
            BusAccessKind::Read => BusAccessDisposition::BlockedRead {
                value: BLOCKED_READ_VALUE,
                reason,
            },
            BusAccessKind::Write => BusAccessDisposition::IgnoredWrite { reason },
        }
    }

    fn wram_index(&self, address: u16) -> usize {
        match address {
            0xC000..=0xDFFF => (address - 0xC000) as usize,
            0xE000..=0xFDFF => (address - 0xE000) as usize,
            _ => panic!("address {address:#06X} does not map to WRAM storage"),
        }
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

    const TEST_VRAM_BYTES: usize = 0x2000;

    fn tick_ppu(ppu: &mut Ppu, t_cycle: u64) {
        let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
        ppu.tick_t_cycle(
            &mut context,
            OamBusView::new(BusMaster::Ppu, &[0; 160]),
            VramBusView::new(BusMaster::Ppu, &[0; TEST_VRAM_BYTES]),
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
        seed_oam_corruption_rows(&mut bus.oam);

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
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 0), expected_first);
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 1), 0x2468);
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 2), 0xAAAA);
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 3), 0xBBBB);
    }

    #[test]
    fn route_cpu_address_event_uses_the_unusable_mode2_read_path_for_corruption() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 1);
        seed_oam_corruption_rows(&mut bus.oam);

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
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 0), expected_first);
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 1), 0x2468);
    }

    #[test]
    fn route_cpu_address_event_uses_the_unusable_mode2_write_path_for_corruption() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 1);
        seed_oam_corruption_rows(&mut bus.oam);

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
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 0), expected_first);
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 1), 0x2468);
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 2), 0xAAAA);
        assert_eq!(read_oam_word_bytes(&bus.oam, 1, 3), 0xBBBB);
    }

    #[test]
    fn route_cpu_address_event_uses_pure_idu_activity_in_fe_range() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 2);
        seed_oam_corruption_rows(&mut bus.oam);

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
        assert_eq!(read_oam_word_bytes(&bus.oam, 2, 0), expected_first);
        assert_eq!(read_oam_word_bytes(&bus.oam, 2, 1), 0x1111);
        assert_eq!(read_oam_word_bytes(&bus.oam, 2, 2), 0x2222);
        assert_eq!(read_oam_word_bytes(&bus.oam, 2, 3), 0x3333);
    }

    #[test]
    fn route_cpu_address_event_uses_write_with_incdec_when_the_idu_edge_reaches_oam() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 2);
        seed_oam_corruption_rows(&mut bus.oam);

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
        assert_eq!(read_oam_word_bytes(&bus.oam, 2, 0), expected_first);
        assert_eq!(read_oam_word_bytes(&bus.oam, 2, 1), 0x1111);
        assert_eq!(read_oam_word_bytes(&bus.oam, 2, 2), 0x2222);
        assert_eq!(read_oam_word_bytes(&bus.oam, 2, 3), 0x3333);
    }

    #[test]
    fn route_cpu_address_event_does_not_turn_mode3_oam_blocking_into_corruption() {
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut ppu = prepare_mode3_ppu(ConsoleModel::Dmg);
        seed_oam_corruption_rows(&mut bus.oam);
        let before = bus.oam;

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
