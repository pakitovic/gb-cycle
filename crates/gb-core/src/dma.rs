use crate::bus::{DmaBusState, DmaMemoryRegionImpact};
use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

const OAM_DMA_DESTINATION_START: u16 = 0xFE00;
const OAM_DMA_TRANSFER_BYTES: u16 = 160;
const OAM_DMA_FIRST_BYTE_DELAY_T_CYCLES: u8 = 2;
const OAM_DMA_T_CYCLES_PER_BYTE: u8 = 4;
const OAM_DMA_TOTAL_T_CYCLES: u16 = 640;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DmaTransferKind {
    Oam,
    Gdma,
    Hdma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DmaCpuImpactPolicy {
    NoCpuStallButBusRestriction,
    CpuFullyStalledUntilDone,
    CpuStalledPerBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DmaTransferFamily {
    FullBurst,
    BlockWindowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DmaAdvanceCondition {
    EveryTCycle,
    HBlank,
    ExternalGate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DmaTransfer {
    kind: DmaTransferKind,
    source_start: u16,
    destination_start: u16,
    total_bytes: u16,
    block_size: u16,
    family: DmaTransferFamily,
    timing: DmaTransferTiming,
    cpu_impact_policy: DmaCpuImpactPolicy,
    memory_region_impact: DmaMemoryRegionImpact,
    advance_condition: DmaAdvanceCondition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DmaTransferSpec {
    kind: DmaTransferKind,
    source_start: u16,
    destination_start: u16,
    total_bytes: u16,
    block_size: u16,
    family: DmaTransferFamily,
    timing: DmaTransferTiming,
    cpu_impact_policy: DmaCpuImpactPolicy,
    memory_region_impact: DmaMemoryRegionImpact,
    advance_condition: DmaAdvanceCondition,
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
            cpu_impact_policy: spec.cpu_impact_policy,
            memory_region_impact: spec.memory_region_impact,
            advance_condition: spec.advance_condition,
        }
    }

    const fn oam(source_page: u8) -> Self {
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
                cpu_bus_restriction_delay_t_cycles: OAM_DMA_FIRST_BYTE_DELAY_T_CYCLES,
                t_cycles_per_byte: OAM_DMA_T_CYCLES_PER_BYTE,
            },
            cpu_impact_policy: DmaCpuImpactPolicy::NoCpuStallButBusRestriction,
            memory_region_impact: DmaMemoryRegionImpact::Oam,
            advance_condition: DmaAdvanceCondition::EveryTCycle,
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
        self.source_start + byte_index
    }

    pub const fn destination_address_for_byte(self, byte_index: u16) -> u16 {
        self.destination_start + byte_index
    }

    pub const fn timing(self) -> DmaTransferTiming {
        self.timing
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DmaTransferState {
    #[default]
    Idle,
    Starting(DmaTransferProgress),
    Active(DmaTransferProgress),
    Completed(DmaTransferProgress),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    pub(crate) const fn destination_address(self) -> u16 {
        self.transfer.destination_address_for_byte(self.byte_index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DmaStartupState {
    pub source_page_latch: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaController {
    console_model: ConsoleModel,
    status: DmaStatus,
    source_page_latch: u8,
    transfer_state: DmaTransferState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaSnapshot {
    pub console_model: ConsoleModel,
    pub status: DmaStatus,
    pub source_page_latch: u8,
    pub transfer_state: DmaTransferState,
}

impl DmaController {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: DmaStatus::Ready,
            source_page_latch: 0,
            transfer_state: DmaTransferState::Idle,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> DmaStatus {
        self.status
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

    pub fn bus_state(&self) -> DmaBusState {
        match self.transfer_state {
            DmaTransferState::Idle | DmaTransferState::Completed(_) => DmaBusState::unrestricted(),
            DmaTransferState::Starting(progress) | DmaTransferState::Active(progress)
                if !progress.is_cpu_bus_restriction_active() =>
            {
                DmaBusState::unrestricted()
            }
            DmaTransferState::Starting(progress) | DmaTransferState::Active(progress) => {
                DmaBusState::cpu_hram_only(Some(progress.transfer().memory_region_impact()))
            }
        }
    }

    pub fn read_ff46(&self) -> u8 {
        self.source_page_latch
    }

    pub fn write_ff46(&mut self, value: u8) {
        self.source_page_latch = value;
        self.transfer_state =
            DmaTransferState::Starting(DmaTransferProgress::new(DmaTransfer::oam(value)));
    }

    pub fn apply_startup_state(&mut self, startup_state: DmaStartupState) {
        self.source_page_latch = startup_state.source_page_latch;
        self.transfer_state = DmaTransferState::Idle;
    }

    pub fn snapshot(&self) -> DmaSnapshot {
        DmaSnapshot {
            console_model: self.console_model,
            status: self.status,
            source_page_latch: self.source_page_latch,
            transfer_state: self.transfer_state,
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
                "t_cycle={} phase={} console_model={:?} status={:?} transfer_state={} transfer_kind={:?} transfer_family={:?} block_size={} advance_condition={:?} first_byte_delay_t_cycles={} first_byte_delay_remaining_t_cycles={} cpu_bus_restriction_delay_t_cycles={} cpu_bus_restriction_delay_remaining_t_cycles={} cpu_bus_restriction_active={} elapsed_t_cycles={} completed_bytes={} remaining_bytes={} completed_blocks={} remaining_blocks={} byte_phase_t_cycles={} total_t_cycles={} cpu_access_policy={:?} active_region={:?}",
                context.t_cycle().get(),
                context.phase(),
                self.console_model,
                self.status,
                self.transfer_state.label(),
                progress.transfer().kind(),
                progress.transfer().family(),
                progress.transfer().block_size(),
                progress.transfer().advance_condition(),
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

    pub(crate) fn tick_t_cycle(&mut self, _context: &mut CycleContext) -> Option<DmaTransferWork> {
        let previous_state = self.transfer_state;

        self.transfer_state = match self.transfer_state {
            DmaTransferState::Idle => DmaTransferState::Idle,
            DmaTransferState::Completed(progress) => DmaTransferState::Completed(progress),
            DmaTransferState::Starting(progress) => {
                let advanced_progress = progress.advance_one_t_cycle();

                if advanced_progress.completed_bytes() == 0 {
                    DmaTransferState::Starting(advanced_progress)
                } else {
                    DmaTransferState::Active(advanced_progress)
                }
            }
            DmaTransferState::Active(progress) if progress.is_complete() => {
                DmaTransferState::Completed(progress)
            }
            DmaTransferState::Active(progress) => {
                DmaTransferState::Active(progress.advance_one_t_cycle())
            }
        };

        Self::transfer_work_for_current_t_cycle(previous_state, self.transfer_state)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oam_transfer_normalizes_the_source_range_destination_and_dmg_metadata() {
        let transfer = DmaTransfer::oam(0x12);

        assert_eq!(transfer.kind(), DmaTransferKind::Oam);
        assert_eq!(transfer.source_start(), 0x1200);
        assert_eq!(transfer.source_end_inclusive(), 0x129F);
        assert_eq!(transfer.destination_start(), 0xFE00);
        assert_eq!(transfer.destination_end_inclusive(), 0xFE9F);
        assert_eq!(transfer.total_bytes(), OAM_DMA_TRANSFER_BYTES);
        assert_eq!(transfer.timing().total_t_cycles(), OAM_DMA_TOTAL_T_CYCLES);
        assert_eq!(
            transfer.timing().first_byte_delay_t_cycles(),
            OAM_DMA_FIRST_BYTE_DELAY_T_CYCLES
        );
        assert_eq!(
            transfer.timing().cpu_bus_restriction_delay_t_cycles(),
            OAM_DMA_FIRST_BYTE_DELAY_T_CYCLES
        );
        assert_eq!(
            transfer.timing().t_cycles_per_byte(),
            OAM_DMA_T_CYCLES_PER_BYTE
        );
        assert_eq!(
            transfer.cpu_impact_policy(),
            DmaCpuImpactPolicy::NoCpuStallButBusRestriction
        );
        assert_eq!(transfer.memory_region_impact(), DmaMemoryRegionImpact::Oam);
    }

    #[test]
    fn ff46_latches_the_source_page_and_builds_a_starting_oam_transfer_immediately() {
        let mut dma = DmaController::new(ConsoleModel::Dmg);

        dma.write_ff46(0x12);

        assert_eq!(dma.read_ff46(), 0x12);
        assert_eq!(
            dma.transfer_state(),
            DmaTransferState::Starting(DmaTransferProgress::new(DmaTransfer::oam(0x12)))
        );
        assert_eq!(dma.current_transfer(), Some(DmaTransfer::oam(0x12)));
        assert_eq!(
            dma.transfer_progress(),
            Some(DmaTransferProgress::new(DmaTransfer::oam(0x12)))
        );
        assert_eq!(dma.bus_state(), DmaBusState::unrestricted());
    }

    #[test]
    fn dma_tick_advances_starting_active_and_completed_lifecycle_over_t_cycles() {
        let mut dma = DmaController::new(ConsoleModel::Dmg);
        let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

        dma.write_ff46(0x12);
        let transfer_work = dma.tick_t_cycle(&mut context);

        let starting_progress = match dma.transfer_state() {
            DmaTransferState::Starting(progress) => progress,
            state => panic!("expected starting progress after first tick, got {state:?}"),
        };
        assert_eq!(starting_progress.elapsed_t_cycles(), 1);
        assert_eq!(starting_progress.completed_bytes(), 0);
        assert!(!starting_progress.is_cpu_bus_restriction_active());
        assert_eq!(dma.bus_state(), DmaBusState::unrestricted());

        assert_eq!(transfer_work, None);

        let first_byte_work = dma
            .tick_t_cycle(&mut context)
            .expect("expected first DMA byte after the startup seam");
        let active_progress = match dma.transfer_state() {
            DmaTransferState::Active(progress) => progress,
            state => panic!("expected active progress after second tick, got {state:?}"),
        };
        assert_eq!(active_progress.elapsed_t_cycles(), 2);
        assert_eq!(active_progress.completed_bytes(), 1);
        assert_eq!(active_progress.first_byte_delay_remaining_t_cycles(), 0);
        assert!(active_progress.is_cpu_bus_restriction_active());
        assert_eq!(first_byte_work.byte_index(), 0);

        for _ in 0..638 {
            dma.tick_t_cycle(&mut context);
        }

        let final_active_progress = match dma.transfer_state() {
            DmaTransferState::Active(progress) => progress,
            state => panic!("expected final active progress before completion, got {state:?}"),
        };
        assert_eq!(final_active_progress.elapsed_t_cycles(), 640);
        assert_eq!(final_active_progress.completed_bytes(), 160);
        assert_eq!(final_active_progress.remaining_bytes(), 0);

        dma.tick_t_cycle(&mut context);

        let completed_progress = match dma.transfer_state() {
            DmaTransferState::Completed(progress) => progress,
            state => panic!("expected completed transfer after final tick, got {state:?}"),
        };
        assert_eq!(completed_progress.elapsed_t_cycles(), 640);
        assert_eq!(dma.bus_state(), DmaBusState::unrestricted());
    }

    #[test]
    fn dma_tick_emits_the_first_oam_byte_after_two_t_cycles_and_then_every_four_t_cycles() {
        let mut dma = DmaController::new(ConsoleModel::Dmg);
        let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

        dma.write_ff46(0x12);

        assert_eq!(dma.tick_t_cycle(&mut context), None);

        let first_work = dma
            .tick_t_cycle(&mut context)
            .expect("expected first DMA byte after two T-cycles");
        assert_eq!(first_work.transfer(), DmaTransfer::oam(0x12));
        assert_eq!(first_work.byte_index(), 0);
        assert_eq!(first_work.source_address(), 0x1200);
        assert_eq!(first_work.destination_address(), 0xFE00);

        for _ in 0..3 {
            assert_eq!(dma.tick_t_cycle(&mut context), None);
        }

        let second_work = dma
            .tick_t_cycle(&mut context)
            .expect("expected second DMA byte four T-cycles later");
        assert_eq!(second_work.byte_index(), 1);
        assert_eq!(second_work.source_address(), 0x1201);
        assert_eq!(second_work.destination_address(), 0xFE01);
    }

    #[test]
    fn transfer_progress_reports_the_oam_startup_seam_and_tail_without_losing_total_duration() {
        let transfer = DmaTransfer::oam(0x12);
        let warm_up_progress = DmaTransferProgress {
            transfer,
            elapsed_t_cycles: 1,
        };
        let first_byte_progress = DmaTransferProgress {
            transfer,
            elapsed_t_cycles: 2,
        };
        let completed_progress = DmaTransferProgress {
            transfer,
            elapsed_t_cycles: 640,
        };

        assert_eq!(warm_up_progress.first_byte_delay_remaining_t_cycles(), 1);
        assert_eq!(
            warm_up_progress.cpu_bus_restriction_delay_remaining_t_cycles(),
            1
        );
        assert_eq!(warm_up_progress.completed_bytes(), 0);
        assert_eq!(warm_up_progress.byte_phase_t_cycles(), 1);
        assert!(!warm_up_progress.is_cpu_bus_restriction_active());

        assert_eq!(first_byte_progress.first_byte_delay_remaining_t_cycles(), 0);
        assert_eq!(
            first_byte_progress.cpu_bus_restriction_delay_remaining_t_cycles(),
            0
        );
        assert_eq!(first_byte_progress.completed_bytes(), 1);
        assert_eq!(first_byte_progress.remaining_bytes(), 159);
        assert_eq!(first_byte_progress.byte_phase_t_cycles(), 0);
        assert!(first_byte_progress.is_cpu_bus_restriction_active());

        assert_eq!(completed_progress.completed_bytes(), 160);
        assert_eq!(completed_progress.remaining_bytes(), 0);
        assert_eq!(completed_progress.byte_phase_t_cycles(), 2);
    }

    #[test]
    fn startup_state_preserves_idle_dma_while_setting_visible_ff46() {
        let mut dma = DmaController::new(ConsoleModel::Dmg);

        dma.apply_startup_state(DmaStartupState {
            source_page_latch: 0xFF,
        });

        assert_eq!(dma.read_ff46(), 0xFF);
        assert_eq!(dma.transfer_state(), DmaTransferState::Idle);
        assert_eq!(dma.current_transfer(), None);
    }

    #[test]
    fn dma_transfer_contract_can_model_a_future_hblank_block_transfer_shape() {
        let transfer = DmaTransfer::from_spec(DmaTransferSpec {
            kind: DmaTransferKind::Hdma,
            source_start: 0x1230,
            destination_start: 0x8010,
            total_bytes: 0x40,
            block_size: 0x10,
            family: DmaTransferFamily::BlockWindowed,
            timing: DmaTransferTiming {
                total_t_cycles: 0x40,
                first_byte_delay_t_cycles: 1,
                cpu_bus_restriction_delay_t_cycles: 1,
                t_cycles_per_byte: 1,
            },
            cpu_impact_policy: DmaCpuImpactPolicy::CpuStalledPerBlock,
            memory_region_impact: DmaMemoryRegionImpact::Vram,
            advance_condition: DmaAdvanceCondition::HBlank,
        });
        let progress = DmaTransferProgress {
            transfer,
            elapsed_t_cycles: 0x20,
        };

        assert_eq!(transfer.kind(), DmaTransferKind::Hdma);
        assert_eq!(transfer.family(), DmaTransferFamily::BlockWindowed);
        assert_eq!(transfer.block_size(), 0x10);
        assert_eq!(transfer.total_blocks(), 4);
        assert_eq!(transfer.advance_condition(), DmaAdvanceCondition::HBlank);
        assert_eq!(
            transfer.cpu_impact_policy(),
            DmaCpuImpactPolicy::CpuStalledPerBlock
        );
        assert_eq!(transfer.memory_region_impact(), DmaMemoryRegionImpact::Vram);
        assert_eq!(progress.completed_bytes(), 0x20);
        assert_eq!(progress.completed_blocks(), 2);
        assert_eq!(progress.remaining_blocks(), 2);
    }
}
