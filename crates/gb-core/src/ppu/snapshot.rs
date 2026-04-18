use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpuSnapshot {
    pub console_model: ConsoleModel,
    pub status: PpuStatus,
    pub lcdc: u8,
    pub stat_interrupt_enable: u8,
    pub lyc_coincidence: bool,
    pub stat_irq_line: bool,
    pub blank_frame_active: bool,
    pub lcd_state: PpuLcdState,
    pub visible_output: PpuVisibleOutputState,
    pub mode: PpuAccessMode,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub line_dot: u16,
    pub mode_dot: u16,
    pub mode0_start_dot: u16,
    pub current_oam_scan_row: Option<u8>,
    pub mode2_scanned_entries: u8,
    pub selected_sprites: Vec<PpuSelectedSprite>,
    pub bg_fetcher_source: PpuBgFetcherSource,
    pub bg_fetcher_stage: PpuBgFetcherStage,
    pub bg_fetcher_stage_dot: u8,
    pub bg_fetcher_tile_map_address: u16,
    pub bg_fetcher_tile_data_address: u16,
    pub bg_fetcher_tile_index: u8,
    pub bg_fetcher_tile_low: u8,
    pub bg_fetcher_tile_high: u8,
    pub last_unsigned_tile_data_low_fetch: u8,
    pub last_unsigned_tile_data_high_fetch: u8,
    pub bg_push_pending: bool,
    pub bg_push_cached: PpuBgCachedSliceSnapshot,
    pub bg_push_disposition: PpuBgPushDispositionSnapshot,
    pub bg_fill_pending: bool,
    pub bg_fill_cached: PpuBgCachedSliceSnapshot,
    pub bg_fifo_pixels: Vec<u8>,
    pub bg_fifo_cached_pixels: Vec<Option<PpuBgFifoCachedPixelSnapshot>>,
    pub bg_startup_source_state: PpuMode3StartupSourceStateSnapshot,
    pub bg_startup_fetch_seam: PpuBgStartupFetchSeamSnapshot,
    pub bg_startup_fifo_placeholders: u8,
    pub bg_push_entry_delay_remaining: u8,
    pub bg_fill_startup_dummy_pixels: u8,
    pub bg_fetcher_post_alignment_restart_delay_dots: u8,
    pub bg_transfer_phase: PpuMode3TransferPhaseSnapshot,
    pub bg_current_transfer_x: u8,
    pub bg_current_transfer_lane: Option<PpuMode3TransferLaneSnapshot>,
    pub bg_current_transfer_source_window: Option<PpuMode3TransferSourceWindowSnapshot>,
    pub bg_current_transfer_backing: Option<PpuMode3TransferBackingSnapshot>,
    pub bg_current_transfer_readiness: Option<PpuMode3TransferReadinessSnapshot>,
    pub bg_current_transfer_kind: Option<PpuMode3TransferDotKindSnapshot>,
    pub obj_fetcher_stage: PpuObjFetcherStage,
    pub obj_fetcher_stage_dot: u8,
    pub obj_fetcher_requested_sprite: Option<PpuSelectedSprite>,
    pub obj_fetcher_resolved_sprite: Option<PpuSelectedSprite>,
    pub obj_fetcher_selected_obj_height: u8,
    pub obj_fetcher_latched_obj_height: u8,
    pub obj_fetcher_resolved_tile_index: Option<u8>,
    pub obj_fetcher_resolved_tile_row: Option<u8>,
    pub obj_fetcher_tile_low_address: Option<u16>,
    pub obj_fetcher_tile_high_address: Option<u16>,
    pub obj_fetcher_tile_low: u8,
    pub obj_fetcher_tile_high: u8,
    pub obj_mode3_line_start_obj_height: u8,
    pub obj_pending_hit_match_x: Option<u8>,
    pub obj_pending_hit_len: usize,
    pub obj_pending_hit_front_sprite_slot: Option<u8>,
    pub obj_fetched_same_x_active_count: usize,
    pub obj_fetched_same_x_pending_count: usize,
    pub obj_fifo_pixels: Vec<Option<u8>>,
    pub scx_discard_remaining: u8,
    pub visible_pixels_output: u8,
    pub window_wy_latch: bool,
    pub window_started_this_line: bool,
    pub window_line_counter: u8,
    pub dmg_previsible_wx_retarget_trigger_x: Option<u8>,
    pub dmg_previsible_wx_retarget_window_pixel_offset: Option<u16>,
    pub dmg_pending_previsible_wx_carry_next_trigger_x: Option<u8>,
    pub dmg_pending_previsible_wx_carry_end_trigger_x: Option<u8>,
    pub dmg_pending_previsible_wx_carry_next_window_pixel_offset: Option<u16>,
    pub current_scanline_mixed_colors: Vec<u8>,
    pub current_scanline_pixels: Vec<u8>,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: Option<u8>,
    pub obp1: Option<u8>,
    pub wy: u8,
    pub wx: u8,
    pub visible_lcdc: u8,
    pub visible_scy: u8,
    pub visible_scx: u8,
    pub visible_bgp: u8,
    pub visible_obp0: Option<u8>,
    pub visible_obp1: Option<u8>,
    pub visible_wy: u8,
    pub visible_wx: u8,
    pub pipeline_lcdc: u8,
    pub pipeline_scy: u8,
    pub pipeline_scx: u8,
    pub pipeline_bgp: u8,
    pub pipeline_obp0: Option<u8>,
    pub pipeline_obp1: Option<u8>,
    pub pipeline_wy: u8,
    pub pipeline_wx: u8,
    pub dmg_bgp_cpu_commit_output_palette_override: Option<u8>,
    pub dmg_bgp_cpu_commit_output_delay_pixels_remaining: u8,
    pub obj_palette_read_policy: DmgObjPaletteReadPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuBgCachedSliceOriginSnapshot {
    Ordinary,
    StartupAlignmentSeed,
    StartupAlignmentFill,
    StartupContinuationVisibleTile2,
    StartupContinuationVisibleTile3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PpuBgFifoCachedPixelSnapshot {
    pub source: PpuBgFetcherSource,
    pub origin: PpuBgCachedSliceOriginSnapshot,
    pub fetch_x: u16,
    pub pixel_index: u8,
    pub same_cycle_live_tilemap_refetch_window_open: bool,
    pub needs_live_tilemap_refetch: bool,
    pub needs_live_tile_data_refetch: bool,
    pub needs_live_tile_data_unsigned_reuse: bool,
    pub tile_map_address: u16,
    pub tile_data_address: u16,
    pub tile_index: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PpuBgCachedSliceSnapshot {
    pub source: PpuBgFetcherSource,
    pub origin: PpuBgCachedSliceOriginSnapshot,
    pub fetch_x: u16,
    pub tile_map_address: u16,
    pub tile_data_address: u16,
    pub tile_index: u8,
    pub tile_low: u8,
    pub tile_high: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuBgPushDispositionSnapshot {
    Ready,
    InterruptedByObjectFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferPhaseSnapshot {
    Priming,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferLaneSnapshot {
    PreVisible,
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferSourceWindowSnapshot {
    AbstractStartup,
    FifoBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferBackingSnapshot {
    Abstract,
    FifoBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferReadinessSnapshot {
    WaitingForFifo,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferDotKindSnapshot {
    NotServed,
    ServedPreVisibleTransfer,
    ServedHiddenTransfer,
    ServedVisiblePixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3StartupSourceStateSnapshot {
    EntryDelay { remaining: u8 },
    Abstract { remaining: u8 },
    FifoBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuBgStartupContinuationSliceSnapshot {
    None,
    VisibleTile2,
    VisibleTile3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuBgStartupFetchSeamSnapshot {
    Inactive,
    AlignmentSeedPending,
    PostAlignment {
        first_real_push_skips_entry_delay: bool,
        next_startup_continuation_slice: PpuBgStartupContinuationSliceSnapshot,
        startup_continuation_visible_tiles_remaining: u8,
        delayed_background_tileindex_read_tiles_remaining: u8,
        delayed_background_tilemap_tiles_remaining: u8,
        delayed_background_tiledata_tiles_remaining: u8,
    },
}

pub(super) const fn snapshot_bg_fifo_cached_origin(
    origin: BgCachedSliceOrigin,
) -> PpuBgCachedSliceOriginSnapshot {
    match origin {
        BgCachedSliceOrigin::Ordinary => PpuBgCachedSliceOriginSnapshot::Ordinary,
        BgCachedSliceOrigin::StartupAlignmentSeed => {
            PpuBgCachedSliceOriginSnapshot::StartupAlignmentSeed
        }
        BgCachedSliceOrigin::StartupAlignmentFill => {
            PpuBgCachedSliceOriginSnapshot::StartupAlignmentFill
        }
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2) => {
            PpuBgCachedSliceOriginSnapshot::StartupContinuationVisibleTile2
        }
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3) => {
            PpuBgCachedSliceOriginSnapshot::StartupContinuationVisibleTile3
        }
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::None) => {
            PpuBgCachedSliceOriginSnapshot::Ordinary
        }
    }
}

pub(super) fn snapshot_bg_fifo_cached_pixel(
    cached: Option<BgFifoPixelCached>,
) -> Option<PpuBgFifoCachedPixelSnapshot> {
    let cached = cached?;
    Some(PpuBgFifoCachedPixelSnapshot {
        source: cached.cached.source,
        origin: snapshot_bg_fifo_cached_origin(cached.cached.origin),
        fetch_x: cached.cached.fetch_x,
        pixel_index: cached.pixel_index,
        same_cycle_live_tilemap_refetch_window_open: cached
            .cached
            .same_cycle_live_tilemap_refetch_window_open,
        needs_live_tilemap_refetch: cached.cached.needs_live_tilemap_refetch,
        needs_live_tile_data_refetch: cached.cached.needs_live_tile_data_refetch,
        needs_live_tile_data_unsigned_reuse: cached.cached.needs_live_tile_data_unsigned_reuse,
        tile_map_address: cached.cached.tile_map_address,
        tile_data_address: cached.cached.tile_data_address,
        tile_index: cached.cached.tile_index,
    })
}

pub(super) const fn snapshot_bg_cached_slice(cached: BgCachedSlice) -> PpuBgCachedSliceSnapshot {
    PpuBgCachedSliceSnapshot {
        source: cached.source,
        origin: snapshot_bg_fifo_cached_origin(cached.origin),
        fetch_x: cached.fetch_x,
        tile_map_address: cached.tile_map_address,
        tile_data_address: cached.tile_data_address,
        tile_index: cached.tile_index,
        tile_low: cached.tile_low,
        tile_high: cached.tile_high,
    }
}

pub(super) const fn snapshot_bg_transfer_phase(
    phase: Mode3TransferPhase,
) -> PpuMode3TransferPhaseSnapshot {
    match phase {
        Mode3TransferPhase::Priming => PpuMode3TransferPhaseSnapshot::Priming,
        Mode3TransferPhase::Output => PpuMode3TransferPhaseSnapshot::Output,
    }
}

pub(super) const fn snapshot_bg_transfer_lane(
    lane: Mode3TransferLane,
) -> PpuMode3TransferLaneSnapshot {
    match lane {
        Mode3TransferLane::PreVisible => PpuMode3TransferLaneSnapshot::PreVisible,
        Mode3TransferLane::Hidden => PpuMode3TransferLaneSnapshot::Hidden,
        Mode3TransferLane::Visible => PpuMode3TransferLaneSnapshot::Visible,
    }
}

pub(super) const fn snapshot_bg_transfer_source_window(
    source_window: Mode3TransferSourceWindow,
) -> PpuMode3TransferSourceWindowSnapshot {
    match source_window {
        Mode3TransferSourceWindow::AbstractStartup => {
            PpuMode3TransferSourceWindowSnapshot::AbstractStartup
        }
        Mode3TransferSourceWindow::FifoBacked => PpuMode3TransferSourceWindowSnapshot::FifoBacked,
    }
}

pub(super) const fn snapshot_bg_transfer_backing(
    backing: Mode3TransferBacking,
) -> PpuMode3TransferBackingSnapshot {
    match backing {
        Mode3TransferBacking::Abstract => PpuMode3TransferBackingSnapshot::Abstract,
        Mode3TransferBacking::FifoBacked => PpuMode3TransferBackingSnapshot::FifoBacked,
    }
}

pub(super) const fn snapshot_bg_transfer_readiness(
    readiness: Mode3TransferReadiness,
) -> PpuMode3TransferReadinessSnapshot {
    match readiness {
        Mode3TransferReadiness::WaitingForFifo(_) => {
            PpuMode3TransferReadinessSnapshot::WaitingForFifo
        }
        Mode3TransferReadiness::Ready(_) => PpuMode3TransferReadinessSnapshot::Ready,
    }
}

pub(super) const fn snapshot_bg_push_disposition(
    disposition: BgPushDisposition,
) -> PpuBgPushDispositionSnapshot {
    match disposition {
        BgPushDisposition::Ready => PpuBgPushDispositionSnapshot::Ready,
        BgPushDisposition::InterruptedByObjectFetch => {
            PpuBgPushDispositionSnapshot::InterruptedByObjectFetch
        }
    }
}

pub(super) const fn snapshot_bg_transfer_kind(
    kind: Mode3TransferDotKind,
) -> PpuMode3TransferDotKindSnapshot {
    match kind {
        Mode3TransferDotKind::NotServed => PpuMode3TransferDotKindSnapshot::NotServed,
        Mode3TransferDotKind::ServedPreVisibleTransfer => {
            PpuMode3TransferDotKindSnapshot::ServedPreVisibleTransfer
        }
        Mode3TransferDotKind::ServedHiddenTransfer => {
            PpuMode3TransferDotKindSnapshot::ServedHiddenTransfer
        }
        Mode3TransferDotKind::ServedVisiblePixel => {
            PpuMode3TransferDotKindSnapshot::ServedVisiblePixel
        }
    }
}

pub(super) const fn snapshot_bg_startup_source_state(
    state: Mode3StartupSourceState,
) -> PpuMode3StartupSourceStateSnapshot {
    match state {
        Mode3StartupSourceState::EntryDelay { remaining } => {
            PpuMode3StartupSourceStateSnapshot::EntryDelay { remaining }
        }
        Mode3StartupSourceState::Abstract { remaining } => {
            PpuMode3StartupSourceStateSnapshot::Abstract { remaining }
        }
        Mode3StartupSourceState::FifoBacked => PpuMode3StartupSourceStateSnapshot::FifoBacked,
    }
}

pub(super) const fn snapshot_bg_startup_continuation_slice(
    slice: BgStartupContinuationSlice,
) -> PpuBgStartupContinuationSliceSnapshot {
    match slice {
        BgStartupContinuationSlice::None => PpuBgStartupContinuationSliceSnapshot::None,
        BgStartupContinuationSlice::VisibleTile2 => {
            PpuBgStartupContinuationSliceSnapshot::VisibleTile2
        }
        BgStartupContinuationSlice::VisibleTile3 => {
            PpuBgStartupContinuationSliceSnapshot::VisibleTile3
        }
    }
}

pub(super) const fn snapshot_bg_startup_fetch_seam(
    seam: BgStartupFetchSeamState,
) -> PpuBgStartupFetchSeamSnapshot {
    match seam {
        BgStartupFetchSeamState::Inactive => PpuBgStartupFetchSeamSnapshot::Inactive,
        BgStartupFetchSeamState::AlignmentSeedPending => {
            PpuBgStartupFetchSeamSnapshot::AlignmentSeedPending
        }
        BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay,
            next_startup_continuation_slice,
            startup_continuation_visible_tiles_remaining,
            delayed_background_tileindex_read_tiles_remaining,
            delayed_background_tilemap_tiles_remaining,
            delayed_background_tiledata_tiles_remaining,
        } => PpuBgStartupFetchSeamSnapshot::PostAlignment {
            first_real_push_skips_entry_delay,
            next_startup_continuation_slice: snapshot_bg_startup_continuation_slice(
                next_startup_continuation_slice,
            ),
            startup_continuation_visible_tiles_remaining,
            delayed_background_tileindex_read_tiles_remaining,
            delayed_background_tilemap_tiles_remaining,
            delayed_background_tiledata_tiles_remaining,
        },
    }
}
