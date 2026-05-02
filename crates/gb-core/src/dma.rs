use crate::bus::{DmaBusState, DmaMemoryRegionImpact};
use crate::model::ConsoleModel;
use crate::ppu::{PpuAccessMode, PpuBusState};
use crate::scheduler::CycleContext;
use crate::speed::CgbSpeedMode;

const OAM_DMA_DESTINATION_START: u16 = 0xFE00;
const OAM_DMA_TRANSFER_BYTES: u16 = 160;
// Mooneye's OAM-DMA start/timing ROMs require one full post-write M-cycle where
// CPU OAM fetches still work before the DMA-owned OAM blocking window begins.
const OAM_DMA_FIRST_BYTE_DELAY_T_CYCLES: u8 = 8;
const OAM_DMA_CPU_BUS_RESTRICTION_DELAY_T_CYCLES: u8 = 5;
const OAM_DMA_T_CYCLES_PER_BYTE: u8 = 4;
const OAM_DMA_TOTAL_T_CYCLES: u16 = 648;
const DMG_OAM_DMA_ECHO_ALIAS_OFFSET: u16 = 0x2000;
const VRAM_DMA_BLOCK_BYTES: u16 = 0x10;
const VRAM_DMA_FIRST_BYTE_DELAY_T_CYCLES: u8 = 2;
const VRAM_DMA_CPU_BUS_RESTRICTION_DELAY_T_CYCLES: u8 = 0;
const VRAM_DMA_T_CYCLES_PER_BYTE: u8 = 2;
const VRAM_DMA_DESTINATION_END: u16 = 0x9FFF;
const VRAM_DMA_INVALID_SOURCE_READ_VALUE: u8 = 0xFF;
const HDMA5_TRANSFER_LENGTH_MASK: u8 = 0x7F;
const HDMA5_MODE_BIT: u8 = 0x80;
const HDMA5_INACTIVE_READ_BIT: u8 = 0x80;
const HDMA5_COMPLETED_READ: u8 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DmaStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DmaTransferKind {
    Oam,
    Gdma,
    Hdma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DmaCpuImpactPolicy {
    NoCpuStallButBusRestriction,
    CpuFullyStalledUntilDone,
    CpuStalledPerBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DmaTransferFamily {
    FullBurst,
    BlockWindowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DmaAdvanceCondition {
    EveryTCycle,
    HBlank,
    ExternalGate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DmaTransferTiming {
    total_t_cycles: u16,
    first_byte_delay_t_cycles: u8,
    cpu_bus_restriction_delay_t_cycles: u8,
    t_cycles_per_byte: u8,
}

impl DmaTransferTiming {
    pub const fn total_t_cycles(self) -> u16 {
        self.total_t_cycles
    }

    pub const fn first_byte_delay_t_cycles(self) -> u8 {
        self.first_byte_delay_t_cycles
    }

    pub const fn cpu_bus_restriction_delay_t_cycles(self) -> u8 {
        self.cpu_bus_restriction_delay_t_cycles
    }

    pub const fn t_cycles_per_byte(self) -> u8 {
        self.t_cycles_per_byte
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DmaTransfer {
    kind: DmaTransferKind,
    source_start: u16,
    destination_start: u16,
    total_bytes: u16,
    block_size: u16,
    family: DmaTransferFamily,
    timing: DmaTransferTiming,
    #[serde(default = "default_cgb_speed_mode")]
    oam_speed_mode: CgbSpeedMode,
    cpu_impact_policy: DmaCpuImpactPolicy,
    memory_region_impact: DmaMemoryRegionImpact,
    advance_condition: DmaAdvanceCondition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct DmaTransferSpec {
    kind: DmaTransferKind,
    source_start: u16,
    destination_start: u16,
    total_bytes: u16,
    block_size: u16,
    family: DmaTransferFamily,
    timing: DmaTransferTiming,
    oam_speed_mode: CgbSpeedMode,
    cpu_impact_policy: DmaCpuImpactPolicy,
    memory_region_impact: DmaMemoryRegionImpact,
    advance_condition: DmaAdvanceCondition,
}

const fn default_cgb_speed_mode() -> CgbSpeedMode {
    CgbSpeedMode::Normal
}

impl DmaTransfer {
    const fn from_spec(spec: DmaTransferSpec) -> Self {
        Self {
            kind: spec.kind,
            source_start: spec.source_start,
            destination_start: spec.destination_start,
            total_bytes: spec.total_bytes,
            block_size: spec.block_size,
            family: spec.family,
            timing: spec.timing,
            oam_speed_mode: spec.oam_speed_mode,
            cpu_impact_policy: spec.cpu_impact_policy,
            memory_region_impact: spec.memory_region_impact,
            advance_condition: spec.advance_condition,
        }
    }

    #[cfg(test)]
    const fn oam(source_page: u8) -> Self {
        Self::oam_for_speed(source_page, CgbSpeedMode::Normal)
    }

    const fn oam_for_speed(source_page: u8, speed_mode: CgbSpeedMode) -> Self {
        Self::from_spec(DmaTransferSpec {
            kind: DmaTransferKind::Oam,
            source_start: (source_page as u16) << 8,
            destination_start: OAM_DMA_DESTINATION_START,
            total_bytes: OAM_DMA_TRANSFER_BYTES,
            block_size: 1,
            family: DmaTransferFamily::FullBurst,
            timing: DmaTransferTiming {
                total_t_cycles: OAM_DMA_TOTAL_T_CYCLES,
                first_byte_delay_t_cycles: OAM_DMA_FIRST_BYTE_DELAY_T_CYCLES,
                cpu_bus_restriction_delay_t_cycles: OAM_DMA_CPU_BUS_RESTRICTION_DELAY_T_CYCLES,
                t_cycles_per_byte: OAM_DMA_T_CYCLES_PER_BYTE,
            },
            oam_speed_mode: speed_mode,
            cpu_impact_policy: DmaCpuImpactPolicy::NoCpuStallButBusRestriction,
            memory_region_impact: DmaMemoryRegionImpact::Oam,
            advance_condition: DmaAdvanceCondition::EveryTCycle,
        })
    }

    const fn gdma(transfer: VramDmaTransfer) -> Self {
        let total_bytes = transfer.remaining_bytes();
        Self::from_spec(DmaTransferSpec {
            kind: DmaTransferKind::Gdma,
            source_start: transfer.source_start(),
            destination_start: transfer.destination_start(),
            total_bytes,
            block_size: VRAM_DMA_BLOCK_BYTES,
            family: DmaTransferFamily::FullBurst,
            timing: vram_dma_timing(total_bytes),
            oam_speed_mode: CgbSpeedMode::Normal,
            cpu_impact_policy: DmaCpuImpactPolicy::CpuFullyStalledUntilDone,
            memory_region_impact: DmaMemoryRegionImpact::Vram,
            advance_condition: DmaAdvanceCondition::EveryTCycle,
        })
    }

    const fn hdma_block(transfer: VramDmaTransfer) -> Self {
        Self::from_spec(DmaTransferSpec {
            kind: DmaTransferKind::Hdma,
            source_start: transfer.source_start(),
            destination_start: transfer.destination_start(),
            total_bytes: VRAM_DMA_BLOCK_BYTES,
            block_size: VRAM_DMA_BLOCK_BYTES,
            family: DmaTransferFamily::BlockWindowed,
            timing: vram_dma_timing(VRAM_DMA_BLOCK_BYTES),
            oam_speed_mode: CgbSpeedMode::Normal,
            cpu_impact_policy: DmaCpuImpactPolicy::CpuStalledPerBlock,
            memory_region_impact: DmaMemoryRegionImpact::Vram,
            advance_condition: DmaAdvanceCondition::HBlank,
        })
    }

    pub const fn kind(self) -> DmaTransferKind {
        self.kind
    }

    pub const fn source_start(self) -> u16 {
        self.source_start
    }

    pub const fn source_end_inclusive(self) -> u16 {
        self.source_start + self.total_bytes - 1
    }

    pub const fn destination_start(self) -> u16 {
        self.destination_start
    }

    pub const fn destination_end_inclusive(self) -> u16 {
        self.destination_start + self.total_bytes - 1
    }

    pub const fn total_bytes(self) -> u16 {
        self.total_bytes
    }

    pub const fn block_size(self) -> u16 {
        self.block_size
    }

    pub const fn total_blocks(self) -> u16 {
        self.total_bytes.div_ceil(self.block_size)
    }

    pub const fn family(self) -> DmaTransferFamily {
        self.family
    }

    pub const fn source_address_for_byte(self, byte_index: u16) -> u16 {
        match self.kind {
            DmaTransferKind::Oam => {
                normalize_dmg_oam_source_address(self.source_start + byte_index)
            }
            DmaTransferKind::Gdma | DmaTransferKind::Hdma => self.source_start + byte_index,
        }
    }

    pub const fn destination_address_for_byte(self, byte_index: u16) -> u16 {
        self.destination_start + byte_index
    }

    pub const fn timing(self) -> DmaTransferTiming {
        self.timing
    }

    pub const fn oam_speed_mode(self) -> CgbSpeedMode {
        self.oam_speed_mode
    }

    pub const fn lcd_domain_duration_dots(self) -> u16 {
        match (self.kind, self.oam_speed_mode) {
            (DmaTransferKind::Oam, CgbSpeedMode::Double) => {
                self.timing.total_t_cycles().div_ceil(2)
            }
            _ => self.timing.total_t_cycles(),
        }
    }

    const fn source_bus(self) -> DmaSourceBus {
        match self.source_address_for_byte(0) {
            0x8000..=0x9FFF => DmaSourceBus::VideoRam,
            0xC000..=0xFDFF => DmaSourceBus::WorkRam,
            _ => DmaSourceBus::External,
        }
    }

    pub const fn cpu_impact_policy(self) -> DmaCpuImpactPolicy {
        self.cpu_impact_policy
    }

    pub const fn memory_region_impact(self) -> DmaMemoryRegionImpact {
        self.memory_region_impact
    }

    pub const fn advance_condition(self) -> DmaAdvanceCondition {
        self.advance_condition
    }
}

const fn vram_dma_timing(total_bytes: u16) -> DmaTransferTiming {
    DmaTransferTiming {
        total_t_cycles: total_bytes * VRAM_DMA_T_CYCLES_PER_BYTE as u16,
        first_byte_delay_t_cycles: VRAM_DMA_FIRST_BYTE_DELAY_T_CYCLES,
        cpu_bus_restriction_delay_t_cycles: VRAM_DMA_CPU_BUS_RESTRICTION_DELAY_T_CYCLES,
        t_cycles_per_byte: VRAM_DMA_T_CYCLES_PER_BYTE,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DmaTransferProgress {
    transfer: DmaTransfer,
    elapsed_t_cycles: u16,
}

impl DmaTransferProgress {
    const fn new(transfer: DmaTransfer) -> Self {
        Self {
            transfer,
            elapsed_t_cycles: 0,
        }
    }

    pub const fn transfer(self) -> DmaTransfer {
        self.transfer
    }

    pub const fn elapsed_t_cycles(self) -> u16 {
        self.elapsed_t_cycles
    }

    pub const fn first_byte_delay_remaining_t_cycles(self) -> u8 {
        let first_byte_delay_t_cycles = self.transfer.timing().first_byte_delay_t_cycles() as u16;

        if self.elapsed_t_cycles >= first_byte_delay_t_cycles {
            0
        } else {
            (first_byte_delay_t_cycles - self.elapsed_t_cycles) as u8
        }
    }

    pub const fn cpu_bus_restriction_delay_remaining_t_cycles(self) -> u8 {
        let cpu_bus_restriction_delay_t_cycles =
            self.transfer.timing().cpu_bus_restriction_delay_t_cycles() as u16;

        if self.elapsed_t_cycles >= cpu_bus_restriction_delay_t_cycles {
            0
        } else {
            (cpu_bus_restriction_delay_t_cycles - self.elapsed_t_cycles) as u8
        }
    }

    pub const fn is_cpu_bus_restriction_active(self) -> bool {
        self.elapsed_t_cycles >= self.transfer.timing().cpu_bus_restriction_delay_t_cycles() as u16
    }

    pub const fn completed_bytes(self) -> u16 {
        let first_byte_delay_t_cycles = self.transfer.timing().first_byte_delay_t_cycles() as u16;

        if self.elapsed_t_cycles < first_byte_delay_t_cycles {
            0
        } else {
            let completed_bytes = 1
                + (self.elapsed_t_cycles - first_byte_delay_t_cycles)
                    / self.transfer.timing().t_cycles_per_byte as u16;
            if completed_bytes > self.transfer.total_bytes() {
                self.transfer.total_bytes()
            } else {
                completed_bytes
            }
        }
    }

    pub const fn remaining_bytes(self) -> u16 {
        self.transfer.total_bytes() - self.completed_bytes()
    }

    pub const fn completed_blocks(self) -> u16 {
        self.completed_bytes() / self.transfer.block_size()
    }

    pub const fn remaining_blocks(self) -> u16 {
        self.transfer.total_blocks() - self.completed_blocks()
    }

    pub const fn byte_phase_t_cycles(self) -> u8 {
        let first_byte_delay_t_cycles = self.transfer.timing().first_byte_delay_t_cycles() as u16;

        if self.elapsed_t_cycles < first_byte_delay_t_cycles {
            self.elapsed_t_cycles as u8
        } else {
            ((self.elapsed_t_cycles - first_byte_delay_t_cycles)
                % self.transfer.timing().t_cycles_per_byte as u16) as u8
        }
    }

    pub const fn is_complete(self) -> bool {
        self.elapsed_t_cycles >= self.transfer.timing().total_t_cycles()
    }

    fn advance_one_t_cycle(self) -> Self {
        Self {
            transfer: self.transfer,
            elapsed_t_cycles: self.elapsed_t_cycles.saturating_add(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
enum DmaSourceBus {
    External,
    VideoRam,
    WorkRam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum VramDmaMode {
    GeneralPurpose,
    HBlank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VramDmaRegisters {
    source_high: u8,
    source_low: u8,
    destination_high: u8,
    destination_low: u8,
}

impl Default for VramDmaRegisters {
    fn default() -> Self {
        Self {
            source_high: 0xFF,
            source_low: 0xF0,
            destination_high: 0x1F,
            destination_low: 0xF0,
        }
    }
}

impl VramDmaRegisters {
    pub const fn source_high(self) -> u8 {
        self.source_high
    }

    pub const fn source_low(self) -> u8 {
        self.source_low
    }

    pub const fn destination_high(self) -> u8 {
        self.destination_high
    }

    pub const fn destination_low(self) -> u8 {
        self.destination_low
    }

    pub const fn source_start(self) -> u16 {
        ((self.source_high as u16) << 8) | self.source_low as u16
    }

    pub const fn destination_start(self) -> u16 {
        0x8000 | ((self.destination_high as u16) << 8) | self.destination_low as u16
    }

    fn write_hdma1(&mut self, value: u8) {
        self.source_high = value;
    }

    fn write_hdma2(&mut self, value: u8) {
        self.source_low = value & 0xF0;
    }

    fn write_hdma3(&mut self, value: u8) {
        self.destination_high = value & 0x1F;
    }

    fn write_hdma4(&mut self, value: u8) {
        self.destination_low = value & 0xF0;
    }

    fn set_endpoints(&mut self, source: u16, destination: u16) {
        self.source_high = (source >> 8) as u8;
        self.source_low = (source as u8) & 0xF0;
        self.destination_high = ((destination >> 8) as u8) & 0x1F;
        self.destination_low = (destination as u8) & 0xF0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VramDmaTransfer {
    mode: VramDmaMode,
    source_start: u16,
    destination_start: u16,
    total_blocks: u16,
    remaining_blocks: u16,
}

impl VramDmaTransfer {
    const fn new(mode: VramDmaMode, registers: VramDmaRegisters, blocks: u16) -> Self {
        let total_blocks =
            clip_vram_dma_blocks_for_destination(registers.destination_start(), blocks);
        Self {
            mode,
            source_start: registers.source_start(),
            destination_start: registers.destination_start(),
            total_blocks,
            remaining_blocks: total_blocks,
        }
    }

    pub const fn mode(self) -> VramDmaMode {
        self.mode
    }

    pub const fn source_start(self) -> u16 {
        self.source_start
    }

    pub const fn destination_start(self) -> u16 {
        self.destination_start
    }

    pub const fn total_blocks(self) -> u16 {
        self.total_blocks
    }

    pub const fn remaining_blocks(self) -> u16 {
        self.remaining_blocks
    }

    pub const fn remaining_bytes(self) -> u16 {
        self.remaining_blocks * VRAM_DMA_BLOCK_BYTES
    }

    pub const fn total_bytes(self) -> u16 {
        self.total_blocks * VRAM_DMA_BLOCK_BYTES
    }

    pub const fn remaining_blocks_minus_one(self) -> u8 {
        self.remaining_blocks.saturating_sub(1) as u8
    }

    const fn advance_completed_blocks(self, completed_blocks: u16) -> Self {
        let completed_blocks = if completed_blocks > self.remaining_blocks {
            self.remaining_blocks
        } else {
            completed_blocks
        };
        let completed_bytes = completed_blocks * VRAM_DMA_BLOCK_BYTES;
        Self {
            mode: self.mode,
            source_start: self.source_start.wrapping_add(completed_bytes),
            destination_start: self.destination_start.wrapping_add(completed_bytes),
            total_blocks: self.total_blocks,
            remaining_blocks: self.remaining_blocks - completed_blocks,
        }
    }
}

const fn clip_vram_dma_blocks_for_destination(
    destination_start: u16,
    requested_blocks: u16,
) -> u16 {
    let bytes_until_overflow = VRAM_DMA_DESTINATION_END + 1 - destination_start;
    let blocks_until_overflow = bytes_until_overflow / VRAM_DMA_BLOCK_BYTES;
    if requested_blocks > blocks_until_overflow {
        blocks_until_overflow
    } else {
        requested_blocks
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum VramDmaState {
    Inactive { hdma5_read_low: u8 },
    GeneralPurposeActive(VramDmaTransfer),
    GeneralPurposeComplete(VramDmaTransfer),
    HBlankActive(VramDmaTransfer),
}

impl Default for VramDmaState {
    fn default() -> Self {
        Self::Inactive {
            hdma5_read_low: HDMA5_TRANSFER_LENGTH_MASK,
        }
    }
}

impl VramDmaState {
    pub const fn is_active(self) -> bool {
        matches!(self, Self::GeneralPurposeActive(_) | Self::HBlankActive(_))
    }

    pub const fn active_transfer(self) -> Option<VramDmaTransfer> {
        match self {
            Self::GeneralPurposeActive(transfer) | Self::HBlankActive(transfer) => Some(transfer),
            Self::Inactive { .. } | Self::GeneralPurposeComplete(_) => None,
        }
    }

    pub const fn hblank_active_transfer(self) -> Option<VramDmaTransfer> {
        match self {
            Self::HBlankActive(transfer) => Some(transfer),
            Self::Inactive { .. }
            | Self::GeneralPurposeActive(_)
            | Self::GeneralPurposeComplete(_) => None,
        }
    }

    const fn read_hdma5(self) -> u8 {
        match self {
            Self::Inactive { hdma5_read_low } => {
                HDMA5_INACTIVE_READ_BIT | (hdma5_read_low & HDMA5_TRANSFER_LENGTH_MASK)
            }
            Self::GeneralPurposeActive(transfer) => transfer.remaining_blocks_minus_one(),
            Self::GeneralPurposeComplete(_) => HDMA5_COMPLETED_READ,
            Self::HBlankActive(transfer) => transfer.remaining_blocks_minus_one(),
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(crate) enum VramDmaHBlankWindow {
    #[default]
    None,
    LcdDisabled,
    VisibleHBlank {
        ly: u8,
    },
}

impl VramDmaHBlankWindow {
    const fn from_ppu_bus_state(ppu: PpuBusState, ly: u8) -> Self {
        if !ppu.is_lcd_enabled() {
            return Self::LcdDisabled;
        }

        if ly < 144 && matches!(ppu.mode(), PpuAccessMode::HBlank) {
            Self::VisibleHBlank { ly }
        } else {
            Self::None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct VramDmaRuntimeContext {
    ppu_bus_state: PpuBusState,
    ppu_ly: u8,
    cpu_halted: bool,
}

impl VramDmaRuntimeContext {
    pub(crate) const fn new(ppu_bus_state: PpuBusState, ppu_ly: u8, cpu_halted: bool) -> Self {
        Self {
            ppu_bus_state,
            ppu_ly,
            cpu_halted,
        }
    }

    const fn hblank_window(self) -> VramDmaHBlankWindow {
        VramDmaHBlankWindow::from_ppu_bus_state(self.ppu_bus_state, self.ppu_ly)
    }

    const fn cpu_halted(self) -> bool {
        self.cpu_halted
    }
}

impl Default for VramDmaRuntimeContext {
    fn default() -> Self {
        Self::new(PpuBusState::lcd_disabled(), 0, false)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum DmaTransferState {
    #[default]
    Idle,
    Starting(DmaTransferProgress),
    Active(DmaTransferProgress),
    Completed(DmaTransferProgress),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum DmaTransferLifecycle {
    #[default]
    Idle,
    Starting,
    Active,
    Completed,
    Cancelled,
}

impl DmaTransferState {
    pub const fn current_transfer(self) -> Option<DmaTransfer> {
        match self.progress() {
            Some(progress) => Some(progress.transfer()),
            None => None,
        }
    }

    pub const fn progress(self) -> Option<DmaTransferProgress> {
        match self {
            Self::Idle => None,
            Self::Starting(progress) | Self::Active(progress) | Self::Completed(progress) => {
                Some(progress)
            }
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Starting(_) => "Starting",
            Self::Active(_) => "Active",
            Self::Completed(_) => "Completed",
        }
    }

    pub const fn lifecycle(self) -> DmaTransferLifecycle {
        match self {
            Self::Idle => DmaTransferLifecycle::Idle,
            Self::Starting(_) => DmaTransferLifecycle::Starting,
            Self::Active(_) => DmaTransferLifecycle::Active,
            Self::Completed(_) => DmaTransferLifecycle::Completed,
        }
    }

    pub const fn is_in_flight(self) -> bool {
        matches!(self, Self::Starting(_) | Self::Active(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DmaTransferStatusView {
    lifecycle: DmaTransferLifecycle,
    transfer: Option<DmaTransfer>,
    progress: Option<DmaTransferProgress>,
    bus_state: DmaBusState,
}

impl DmaTransferStatusView {
    const fn new(
        lifecycle: DmaTransferLifecycle,
        transfer: Option<DmaTransfer>,
        progress: Option<DmaTransferProgress>,
        bus_state: DmaBusState,
    ) -> Self {
        Self {
            lifecycle,
            transfer,
            progress,
            bus_state,
        }
    }

    pub const fn lifecycle(self) -> DmaTransferLifecycle {
        self.lifecycle
    }

    pub const fn transfer(self) -> Option<DmaTransfer> {
        self.transfer
    }

    pub const fn progress(self) -> Option<DmaTransferProgress> {
        self.progress
    }

    pub const fn bus_state(self) -> DmaBusState {
        self.bus_state
    }

    pub const fn is_in_flight(self) -> bool {
        matches!(
            self.lifecycle,
            DmaTransferLifecycle::Starting | DmaTransferLifecycle::Active
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct DmaTransferWork {
    transfer: DmaTransfer,
    byte_index: u16,
}

impl DmaTransferWork {
    const fn new(transfer: DmaTransfer, byte_index: u16) -> Self {
        Self {
            transfer,
            byte_index,
        }
    }

    #[cfg(test)]
    pub(crate) const fn transfer(self) -> DmaTransfer {
        self.transfer
    }

    #[cfg(test)]
    pub(crate) const fn byte_index(self) -> u16 {
        self.byte_index
    }

    pub(crate) const fn source_address(self) -> u16 {
        self.transfer.source_address_for_byte(self.byte_index)
    }

    pub(crate) const fn source_read_value_override(self) -> Option<u8> {
        if matches!(
            self.transfer.kind(),
            DmaTransferKind::Gdma | DmaTransferKind::Hdma
        ) && !vram_dma_source_address_is_supported(self.source_address())
        {
            Some(VRAM_DMA_INVALID_SOURCE_READ_VALUE)
        } else {
            None
        }
    }

    pub(crate) const fn destination_address(self) -> u16 {
        self.transfer.destination_address_for_byte(self.byte_index)
    }
}

const fn vram_dma_source_address_is_supported(address: u16) -> bool {
    matches!(address, 0x0000..=0x7FFF | 0xA000..=0xDFFF)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DmaStartupState {
    pub source_page_latch: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DmaController {
    console_model: ConsoleModel,
    status: DmaStatus,
    source_page_latch: u8,
    transfer_state: DmaTransferState,
    pending_restart: Option<DmaTransferProgress>,
    #[serde(default)]
    vram_dma_registers: VramDmaRegisters,
    #[serde(default)]
    vram_dma_state: VramDmaState,
    #[serde(default)]
    vram_dma_last_served_window: VramDmaHBlankWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DmaSaveState {
    console_model: ConsoleModel,
    status: DmaStatus,
    source_page_latch: u8,
    transfer_state: DmaTransferState,
    pending_restart: Option<DmaTransferProgress>,
    #[serde(default)]
    vram_dma_registers: VramDmaRegisters,
    #[serde(default)]
    vram_dma_state: VramDmaState,
    #[serde(default)]
    vram_dma_last_served_window: VramDmaHBlankWindow,
}

impl DmaSaveState {
    pub(crate) const fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DmaSnapshot {
    pub console_model: ConsoleModel,
    pub status: DmaStatus,
    pub source_page_latch: u8,
    pub transfer_state: DmaTransferState,
    pub pending_restart: Option<DmaTransferProgress>,
    #[serde(default)]
    pub vram_dma_registers: VramDmaRegisters,
    #[serde(default)]
    pub vram_dma_state: VramDmaState,
    #[serde(default)]
    pub(crate) vram_dma_last_served_window: VramDmaHBlankWindow,
}

impl DmaController {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: DmaStatus::Ready,
            source_page_latch: 0,
            transfer_state: DmaTransferState::Idle,
            pending_restart: None,
            vram_dma_registers: VramDmaRegisters::default(),
            vram_dma_state: VramDmaState::default(),
            vram_dma_last_served_window: VramDmaHBlankWindow::default(),
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> DmaStatus {
        self.status
    }

    pub(crate) fn capture_save_state(&self) -> DmaSaveState {
        DmaSaveState {
            console_model: self.console_model,
            status: self.status,
            source_page_latch: self.source_page_latch,
            transfer_state: self.transfer_state,
            pending_restart: self.pending_restart,
            vram_dma_registers: self.vram_dma_registers,
            vram_dma_state: self.vram_dma_state,
            vram_dma_last_served_window: self.vram_dma_last_served_window,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &DmaSaveState) {
        self.console_model = state.console_model;
        self.status = state.status;
        self.source_page_latch = state.source_page_latch;
        self.transfer_state = state.transfer_state;
        self.pending_restart = state.pending_restart;
        self.vram_dma_registers = state.vram_dma_registers;
        self.vram_dma_state = state.vram_dma_state;
        self.vram_dma_last_served_window = state.vram_dma_last_served_window;
    }

    pub fn source_page_latch(&self) -> u8 {
        self.source_page_latch
    }

    pub fn transfer_state(&self) -> DmaTransferState {
        self.transfer_state
    }

    pub fn current_transfer(&self) -> Option<DmaTransfer> {
        self.transfer_state.current_transfer()
    }

    pub fn transfer_progress(&self) -> Option<DmaTransferProgress> {
        self.transfer_state.progress()
    }

    pub fn transfer_lifecycle(&self) -> DmaTransferLifecycle {
        self.transfer_state.lifecycle()
    }

    pub fn has_in_flight_transfer(&self) -> bool {
        self.transfer_state.is_in_flight()
    }

    pub fn transfer_status(&self) -> DmaTransferStatusView {
        DmaTransferStatusView::new(
            self.transfer_lifecycle(),
            self.current_transfer(),
            self.transfer_progress(),
            self.bus_state(),
        )
    }

    pub fn vram_dma_registers(&self) -> VramDmaRegisters {
        self.vram_dma_registers
    }

    pub fn vram_dma_state(&self) -> VramDmaState {
        self.vram_dma_state
    }

    pub fn bus_state(&self) -> DmaBusState {
        match self.transfer_state {
            DmaTransferState::Idle | DmaTransferState::Completed(_) => DmaBusState::unrestricted(),
            DmaTransferState::Starting(progress) | DmaTransferState::Active(progress)
                if !progress.is_cpu_bus_restriction_active() =>
            {
                DmaBusState::unrestricted()
            }
            DmaTransferState::Starting(progress) | DmaTransferState::Active(progress) => {
                if matches!(
                    progress.transfer().kind(),
                    DmaTransferKind::Gdma | DmaTransferKind::Hdma
                ) {
                    return DmaBusState::video_bus_blocked(Some(
                        progress.transfer().memory_region_impact(),
                    ));
                }

                let cpu_conflict_source_address = (progress.completed_bytes() > 0).then(|| {
                    progress
                        .transfer()
                        .source_address_for_byte(progress.completed_bytes() - 1)
                });

                match progress.transfer().source_bus() {
                    DmaSourceBus::External if self.console_model.is_cgb_family() => {
                        DmaBusState::external_bus_only_blocked(Some(
                            progress.transfer().memory_region_impact(),
                        ))
                        .with_cpu_conflict_source_address(cpu_conflict_source_address)
                    }
                    DmaSourceBus::External => DmaBusState::external_bus_blocked(Some(
                        progress.transfer().memory_region_impact(),
                    ))
                    .with_cpu_conflict_source_address(cpu_conflict_source_address),
                    DmaSourceBus::VideoRam => DmaBusState::video_bus_blocked(Some(
                        progress.transfer().memory_region_impact(),
                    ))
                    .with_cpu_conflict_source_address(cpu_conflict_source_address),
                    DmaSourceBus::WorkRam if self.console_model.is_cgb_family() => {
                        DmaBusState::wram_bus_blocked(Some(
                            progress.transfer().memory_region_impact(),
                        ))
                        .with_cpu_conflict_source_address(cpu_conflict_source_address)
                    }
                    DmaSourceBus::WorkRam => DmaBusState::external_bus_blocked(Some(
                        progress.transfer().memory_region_impact(),
                    ))
                    .with_cpu_conflict_source_address(cpu_conflict_source_address),
                }
            }
        }
    }

    pub(crate) fn cpu_stall_active(&self) -> bool {
        if !self.transfer_state.is_in_flight() {
            return false;
        }

        let Some(progress) = self.transfer_state.progress() else {
            return false;
        };

        if !progress.is_cpu_bus_restriction_active() {
            return false;
        }

        matches!(
            progress.transfer().cpu_impact_policy(),
            DmaCpuImpactPolicy::CpuFullyStalledUntilDone | DmaCpuImpactPolicy::CpuStalledPerBlock
        )
    }

    pub fn read_ff46(&self) -> u8 {
        self.source_page_latch
    }

    pub fn write_ff46(&mut self, value: u8) {
        self.write_ff46_for_speed(value, CgbSpeedMode::Normal);
    }

    pub(crate) fn write_ff46_for_speed(&mut self, value: u8, speed_mode: CgbSpeedMode) {
        self.source_page_latch = value;
        let speed_mode = if self.console_model.is_cgb_family() {
            speed_mode
        } else {
            CgbSpeedMode::Normal
        };
        let restarted_transfer =
            DmaTransferProgress::new(DmaTransfer::oam_for_speed(value, speed_mode));

        if self.transfer_state.is_in_flight() {
            self.pending_restart = Some(restarted_transfer);
        } else {
            self.transfer_state = DmaTransferState::Starting(restarted_transfer);
            self.pending_restart = None;
        }
    }

    pub fn read_hdma5(&self) -> u8 {
        self.vram_dma_state.read_hdma5()
    }

    pub fn write_hdma1(&mut self, value: u8) {
        self.vram_dma_registers.write_hdma1(value);
    }

    pub fn write_hdma2(&mut self, value: u8) {
        self.vram_dma_registers.write_hdma2(value);
    }

    pub fn write_hdma3(&mut self, value: u8) {
        self.vram_dma_registers.write_hdma3(value);
    }

    pub fn write_hdma4(&mut self, value: u8) {
        self.vram_dma_registers.write_hdma4(value);
    }

    pub fn write_hdma5(&mut self, value: u8) {
        if let VramDmaState::HBlankActive(active_transfer) = self.vram_dma_state {
            if value & HDMA5_MODE_BIT == 0 {
                self.vram_dma_state = VramDmaState::Inactive {
                    hdma5_read_low: active_transfer.remaining_blocks_minus_one(),
                };
                if matches!(
                    self.transfer_state
                        .current_transfer()
                        .map(|transfer| transfer.kind()),
                    Some(DmaTransferKind::Hdma)
                ) {
                    self.transfer_state = DmaTransferState::Idle;
                }
            }
            return;
        }

        let blocks = u16::from(value & HDMA5_TRANSFER_LENGTH_MASK) + 1;
        if value & HDMA5_MODE_BIT == 0 {
            let transfer =
                VramDmaTransfer::new(VramDmaMode::GeneralPurpose, self.vram_dma_registers, blocks);
            self.vram_dma_state = VramDmaState::GeneralPurposeActive(transfer);
            self.vram_dma_last_served_window = VramDmaHBlankWindow::default();
            self.transfer_state =
                DmaTransferState::Starting(DmaTransferProgress::new(DmaTransfer::gdma(transfer)));
            self.pending_restart = None;
        } else {
            self.vram_dma_state = VramDmaState::HBlankActive(VramDmaTransfer::new(
                VramDmaMode::HBlank,
                self.vram_dma_registers,
                blocks,
            ));
            self.vram_dma_last_served_window = VramDmaHBlankWindow::default();
        }
    }

    pub fn apply_startup_state(&mut self, startup_state: DmaStartupState) {
        self.source_page_latch = startup_state.source_page_latch;
        self.transfer_state = DmaTransferState::Idle;
        self.pending_restart = None;
        self.vram_dma_registers = VramDmaRegisters::default();
        self.vram_dma_state = VramDmaState::default();
        self.vram_dma_last_served_window = VramDmaHBlankWindow::default();
    }

    pub fn snapshot(&self) -> DmaSnapshot {
        DmaSnapshot {
            console_model: self.console_model,
            status: self.status,
            source_page_latch: self.source_page_latch,
            transfer_state: self.transfer_state,
            pending_restart: self.pending_restart,
            vram_dma_registers: self.vram_dma_registers,
            vram_dma_state: self.vram_dma_state,
            vram_dma_last_served_window: self.vram_dma_last_served_window,
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        let published_bus_state = self.bus_state();

        match self.transfer_state {
            DmaTransferState::Idle => format!(
                "t_cycle={} phase={} console_model={:?} status={:?} transfer_state={} cpu_access_policy={:?} active_region={:?}",
                context.t_cycle().get(),
                context.phase(),
                self.console_model,
                self.status,
                self.transfer_state.label(),
                published_bus_state.cpu_access_policy(),
                published_bus_state.active_region(),
            ),
            DmaTransferState::Starting(progress)
            | DmaTransferState::Active(progress)
            | DmaTransferState::Completed(progress) => format!(
                "t_cycle={} phase={} console_model={:?} status={:?} transfer_state={} transfer_kind={:?} transfer_family={:?} block_size={} advance_condition={:?} oam_speed_mode={:?} lcd_domain_duration_dots={} first_byte_delay_t_cycles={} first_byte_delay_remaining_t_cycles={} cpu_bus_restriction_delay_t_cycles={} cpu_bus_restriction_delay_remaining_t_cycles={} cpu_bus_restriction_active={} elapsed_t_cycles={} completed_bytes={} remaining_bytes={} completed_blocks={} remaining_blocks={} byte_phase_t_cycles={} total_t_cycles={} cpu_access_policy={:?} active_region={:?}",
                context.t_cycle().get(),
                context.phase(),
                self.console_model,
                self.status,
                self.transfer_state.label(),
                progress.transfer().kind(),
                progress.transfer().family(),
                progress.transfer().block_size(),
                progress.transfer().advance_condition(),
                progress.transfer().oam_speed_mode(),
                progress.transfer().lcd_domain_duration_dots(),
                progress.transfer().timing().first_byte_delay_t_cycles(),
                progress.first_byte_delay_remaining_t_cycles(),
                progress
                    .transfer()
                    .timing()
                    .cpu_bus_restriction_delay_t_cycles(),
                progress.cpu_bus_restriction_delay_remaining_t_cycles(),
                progress.is_cpu_bus_restriction_active(),
                progress.elapsed_t_cycles(),
                progress.completed_bytes(),
                progress.remaining_bytes(),
                progress.completed_blocks(),
                progress.remaining_blocks(),
                progress.byte_phase_t_cycles(),
                progress.transfer().timing().total_t_cycles(),
                published_bus_state.cpu_access_policy(),
                published_bus_state.active_region(),
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn tick_t_cycle(&mut self, context: &mut CycleContext) -> Option<DmaTransferWork> {
        self.tick_t_cycle_with_vram_dma_context(context, VramDmaRuntimeContext::default())
    }

    pub(crate) fn tick_t_cycle_with_vram_dma_context(
        &mut self,
        _context: &mut CycleContext,
        vram_dma_context: VramDmaRuntimeContext,
    ) -> Option<DmaTransferWork> {
        self.start_hdma_block_if_eligible(vram_dma_context);

        if let Some(pending_restart) = self.pending_restart {
            let advanced_restart = pending_restart.advance_one_t_cycle();

            if !advanced_restart.is_cpu_bus_restriction_active() {
                self.pending_restart = Some(advanced_restart);
            } else {
                self.pending_restart = None;
                self.transfer_state = DmaTransferState::Active(advanced_restart);
                return None;
            }
        }

        let previous_state = self.transfer_state;

        self.transfer_state = match self.transfer_state {
            DmaTransferState::Idle => DmaTransferState::Idle,
            DmaTransferState::Completed(progress) => DmaTransferState::Completed(progress),
            DmaTransferState::Starting(progress) => {
                let advanced_progress = progress.advance_one_t_cycle();

                if advanced_progress.is_cpu_bus_restriction_active() {
                    DmaTransferState::Active(advanced_progress)
                } else {
                    DmaTransferState::Starting(advanced_progress)
                }
            }
            DmaTransferState::Active(progress) => {
                let advanced_progress = progress.advance_one_t_cycle();

                if advanced_progress.is_complete() {
                    DmaTransferState::Completed(advanced_progress)
                } else {
                    DmaTransferState::Active(advanced_progress)
                }
            }
        };

        let transfer_work =
            Self::transfer_work_for_current_t_cycle(previous_state, self.transfer_state);
        self.apply_vram_dma_progress_side_effects(previous_state, self.transfer_state);
        transfer_work
    }

    fn start_hdma_block_if_eligible(&mut self, vram_dma_context: VramDmaRuntimeContext) {
        if self.transfer_state.is_in_flight() || vram_dma_context.cpu_halted() {
            return;
        }

        let VramDmaState::HBlankActive(transfer) = self.vram_dma_state else {
            return;
        };

        let window = vram_dma_context.hblank_window();
        if matches!(window, VramDmaHBlankWindow::None) || self.vram_dma_last_served_window == window
        {
            return;
        }

        self.vram_dma_last_served_window = window;
        self.transfer_state =
            DmaTransferState::Starting(DmaTransferProgress::new(DmaTransfer::hdma_block(transfer)));
    }

    fn apply_vram_dma_progress_side_effects(
        &mut self,
        previous_state: DmaTransferState,
        current_state: DmaTransferState,
    ) {
        let previous_progress = previous_state.progress();
        let Some(current_progress) = current_state.progress() else {
            return;
        };
        let transfer = current_progress.transfer();
        if !matches!(
            transfer.kind(),
            DmaTransferKind::Gdma | DmaTransferKind::Hdma
        ) {
            return;
        }

        let completed_blocks = current_progress.completed_blocks()
            - previous_progress.map_or(0, DmaTransferProgress::completed_blocks);
        if completed_blocks == 0 {
            return;
        }

        match self.vram_dma_state {
            VramDmaState::GeneralPurposeActive(active_transfer)
                if transfer.kind() == DmaTransferKind::Gdma =>
            {
                let updated_transfer = active_transfer.advance_completed_blocks(completed_blocks);
                self.vram_dma_registers.set_endpoints(
                    updated_transfer.source_start(),
                    updated_transfer.destination_start(),
                );
                self.vram_dma_state = if updated_transfer.remaining_blocks() == 0 {
                    VramDmaState::GeneralPurposeComplete(updated_transfer)
                } else {
                    VramDmaState::GeneralPurposeActive(updated_transfer)
                };
            }
            VramDmaState::HBlankActive(active_transfer)
                if transfer.kind() == DmaTransferKind::Hdma =>
            {
                let updated_transfer = active_transfer.advance_completed_blocks(completed_blocks);
                self.vram_dma_registers.set_endpoints(
                    updated_transfer.source_start(),
                    updated_transfer.destination_start(),
                );
                self.vram_dma_state = if updated_transfer.remaining_blocks() == 0 {
                    VramDmaState::Inactive {
                        hdma5_read_low: HDMA5_TRANSFER_LENGTH_MASK,
                    }
                } else {
                    VramDmaState::HBlankActive(updated_transfer)
                };
            }
            _ => {}
        }
    }

    fn transfer_work_for_current_t_cycle(
        previous_state: DmaTransferState,
        current_state: DmaTransferState,
    ) -> Option<DmaTransferWork> {
        let previous_progress = previous_state.progress()?;
        let current_progress = current_state.progress()?;
        let previous_completed_bytes = previous_progress.completed_bytes();
        let current_completed_bytes = current_progress.completed_bytes();

        if current_completed_bytes <= previous_completed_bytes {
            return None;
        }

        Some(DmaTransferWork::new(
            current_progress.transfer(),
            current_completed_bytes - 1,
        ))
    }
}

const fn normalize_dmg_oam_source_address(address: u16) -> u16 {
    if address >= 0xE000 {
        address - DMG_OAM_DMA_ECHO_ALIAS_OFFSET
    } else {
        address
    }
}

#[cfg(test)]
mod tests;
