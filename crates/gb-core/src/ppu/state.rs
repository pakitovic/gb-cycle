#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PpuPaletteRegister {
    Bgp,
    Obp0,
    Obp1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferDotKind {
    NotServed,
    ServedPreVisibleTransfer,
    ServedHiddenTransfer,
    ServedVisiblePixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3TransferDot {
    kind: Mode3TransferDotKind,
    consumed_scx_discard: bool,
}

impl Mode3TransferDot {
    const fn not_served() -> Self {
        Self {
            kind: Mode3TransferDotKind::NotServed,
            consumed_scx_discard: false,
        }
    }

    const fn served(kind: Mode3TransferDotKind, consumed_scx_discard: bool) -> Self {
        Self {
            kind,
            consumed_scx_discard,
        }
    }

    fn is_served(self) -> bool {
        !matches!(self.kind, Mode3TransferDotKind::NotServed)
    }

    fn can_start_window_after_x0_service(self) -> bool {
        matches!(
            self.kind,
            Mode3TransferDotKind::ServedHiddenTransfer | Mode3TransferDotKind::ServedVisiblePixel
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode3TransferPhase {
    #[default]
    Priming,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferLane {
    PreVisible,
    Hidden,
    Visible,
}

impl Mode3TransferLane {
    const fn dot_kind(self) -> Mode3TransferDotKind {
        match self {
            Self::PreVisible => Mode3TransferDotKind::ServedPreVisibleTransfer,
            Self::Hidden => Mode3TransferDotKind::ServedHiddenTransfer,
            Self::Visible => Mode3TransferDotKind::ServedVisiblePixel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferSourceWindow {
    AbstractStartup,
    FifoBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3TransferContext {
    lane: Mode3TransferLane,
    source_window: Mode3TransferSourceWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3TransferServicePlan {
    result_kind: Mode3TransferDotKind,
    execution: Mode3TransferServiceExecution,
    backing: Mode3TransferBacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3CurrentTransfer {
    context: Mode3TransferContext,
    readiness: Mode3TransferReadiness,
}

impl Mode3CurrentTransfer {
    const fn service_plan(self) -> Mode3TransferServicePlan {
        match self.readiness {
            Mode3TransferReadiness::WaitingForFifo(plan) | Mode3TransferReadiness::Ready(plan) => {
                plan
            }
        }
    }

    const fn can_start_obj_fetch_from_fifo_backed_transfer(
        self,
        real_bg_fifo_pixel_ready: bool,
    ) -> bool {
        self.readiness
            .can_start_obj_fetch_from_fifo_backed_transfer(real_bg_fifo_pixel_ready)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferReadiness {
    WaitingForFifo(Mode3TransferServicePlan),
    Ready(Mode3TransferServicePlan),
}

impl Mode3TransferReadiness {
    const fn can_start_obj_fetch_from_fifo_backed_transfer(
        self,
        real_bg_fifo_pixel_ready: bool,
    ) -> bool {
        match self {
            Self::Ready(plan) => {
                plan.can_start_obj_fetch_from_fifo_backed_transfer(real_bg_fifo_pixel_ready)
            }
            Self::WaitingForFifo(_) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferServiceExecution {
    ConsumeScxDiscard,
    AdvancePreVisibleWithBgPop,
    AdvanceHiddenWithBgAndObjPop,
    EmitVisiblePixel,
}

impl Mode3TransferServiceExecution {
    const fn can_start_obj_fetch_from_fifo_backed_transfer(self) -> bool {
        matches!(
            self,
            Self::AdvanceHiddenWithBgAndObjPop | Self::EmitVisiblePixel
        )
    }

    const fn requires_effective_bg_fifo_pixel(self) -> bool {
        matches!(
            self,
            Self::ConsumeScxDiscard
                | Self::AdvancePreVisibleWithBgPop
                | Self::AdvanceHiddenWithBgAndObjPop
        )
    }

    const fn requires_real_bg_fifo_pixel(self) -> bool {
        matches!(self, Self::EmitVisiblePixel)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferBacking {
    Abstract,
    FifoBacked,
}

impl Mode3TransferServicePlan {
    const fn requires_effective_bg_fifo_pixel(self) -> bool {
        self.execution.requires_effective_bg_fifo_pixel() && !self.requires_real_bg_fifo_pixel()
    }

    const fn requires_real_bg_fifo_pixel(self) -> bool {
        self.execution.requires_real_bg_fifo_pixel()
            || (matches!(self.backing, Mode3TransferBacking::FifoBacked)
                && matches!(
                    self.execution,
                    Mode3TransferServiceExecution::ConsumeScxDiscard
                        | Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop
                ))
    }

    const fn can_start_obj_fetch_from_fifo_backed_transfer(
        self,
        real_bg_fifo_pixel_ready: bool,
    ) -> bool {
        matches!(self.backing, Mode3TransferBacking::FifoBacked)
            && real_bg_fifo_pixel_ready
            && self
                .execution
                .can_start_obj_fetch_from_fifo_backed_transfer()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3StartupSourceState {
    EntryDelay { remaining: u8 },
    Abstract { remaining: u8 },
    FifoBacked,
}

const fn register_affects_pixel(register: PpuPaletteRegister, pixel: MixedPixel) -> bool {
    matches!(
        (register, pixel.source),
        (PpuPaletteRegister::Bgp, MixedPixelSource::Background)
            | (
                PpuPaletteRegister::Obp0,
                MixedPixelSource::Object {
                    palette_obp1: false,
                },
            )
            | (
                PpuPaletteRegister::Obp1,
                MixedPixelSource::Object { palette_obp1: true },
            )
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OamCorruptionEventKind {
    Read,
    Write,
    ReadWithIncDec,
    WriteWithIncDec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct OamCorruptionController;

impl OamCorruptionController {
    fn apply(
        self,
        console_model: ConsoleModel,
        current_row: u8,
        event: OamCorruptionEventKind,
        oam_bytes: &mut [u8],
    ) -> bool {
        if !console_model.is_dmg_family()
            || current_row >= OAM_CORRUPTION_ROW_COUNT
            || oam_bytes.len() < OAM_CORRUPTION_ROW_COUNT as usize * OAM_CORRUPTION_ROW_BYTES
        {
            return false;
        }

        match event {
            OamCorruptionEventKind::Read => self.apply_read_corruption(current_row, oam_bytes),
            OamCorruptionEventKind::Write | OamCorruptionEventKind::WriteWithIncDec => {
                self.apply_write_corruption(current_row, oam_bytes)
            }
            OamCorruptionEventKind::ReadWithIncDec => {
                self.apply_read_with_incdec_corruption(current_row, oam_bytes)
            }
        }

        true
    }

    fn apply_write_corruption(self, current_row: u8, oam_bytes: &mut [u8]) {
        if current_row == 0 {
            return;
        }

        let current_first = read_oam_word(oam_bytes, current_row, 0);
        let previous_first = read_oam_word(oam_bytes, current_row - 1, 0);
        let previous_third = read_oam_word(oam_bytes, current_row - 1, 2);
        let corrupted_first =
            ((current_first ^ previous_third) & (previous_first ^ previous_third)) ^ previous_third;
        write_oam_word(oam_bytes, current_row, 0, corrupted_first);
        copy_previous_row_tail(oam_bytes, current_row);
    }

    fn apply_read_corruption(self, current_row: u8, oam_bytes: &mut [u8]) {
        if current_row == 0 {
            return;
        }

        let current_first = read_oam_word(oam_bytes, current_row, 0);
        let previous_first = read_oam_word(oam_bytes, current_row - 1, 0);
        let previous_third = read_oam_word(oam_bytes, current_row - 1, 2);
        let corrupted_first = previous_first | (current_first & previous_third);
        write_oam_word(oam_bytes, current_row, 0, corrupted_first);
        copy_previous_row_tail(oam_bytes, current_row);
    }

    fn apply_read_with_incdec_corruption(self, current_row: u8, oam_bytes: &mut [u8]) {
        if (4..(OAM_CORRUPTION_ROW_COUNT - 1)).contains(&current_row) {
            let row_minus_two = current_row - 2;
            let previous_row = current_row - 1;
            let a = read_oam_word(oam_bytes, row_minus_two, 0);
            let b = read_oam_word(oam_bytes, previous_row, 0);
            let c = read_oam_word(oam_bytes, current_row, 0);
            let d = read_oam_word(oam_bytes, previous_row, 2);
            let corrupted_previous_first = (b & (a | c | d)) | (a & c & d);
            write_oam_word(oam_bytes, previous_row, 0, corrupted_previous_first);

            let previous_row_bytes = read_oam_row(oam_bytes, previous_row);
            write_oam_row(oam_bytes, current_row, previous_row_bytes);
            write_oam_row(oam_bytes, row_minus_two, previous_row_bytes);
        }

        self.apply_read_corruption(current_row, oam_bytes);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BgPipelineState {
    fetcher: BgFetcherState,
    push: BgPushState,
    fill: BgFifoFillState,
    fifo: VecDeque<u8>,
    fifo_cached_pixels: VecDeque<Option<BgFifoPixelCached>>,
    startup_fetch_seam: BgStartupFetchSeamState,
    startup_fifo_placeholders: u8,
    mode3_started: bool,
    mode0_start_dot: u16,
    initial_scx_discard: u8,
    scx_discard_remaining: u8,
    startup_source_state: Mode3StartupSourceState,
    startup_pre_visible_transfer_dots_remaining: u8,
    transfer_phase: Mode3TransferPhase,
    current_transfer_x: u8,
    visible_pixels_output: u8,
    saw_right_edge_visible_same_x_cluster_this_line: bool,
    window_wy_latch: bool,
    window_force_x0_this_line: bool,
    window_started_this_line: bool,
    wx0_scx_shortening_applied: bool,
    wx166_armed_this_line: bool,
}

impl BgPipelineState {
    fn reset(&mut self) {
        self.fetcher.reset();
        self.push.reset();
        self.fill.reset();
        self.fifo.clear();
        self.fifo_cached_pixels.clear();
        self.startup_fetch_seam = BgStartupFetchSeamState::Inactive;
        self.startup_fifo_placeholders = 0;
        self.mode3_started = false;
        self.mode0_start_dot = MODE0_START_DOT;
        self.initial_scx_discard = 0;
        self.scx_discard_remaining = 0;
        self.startup_source_state = Mode3StartupSourceState::FifoBacked;
        self.startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
        self.transfer_phase = Mode3TransferPhase::Priming;
        self.current_transfer_x = 0;
        self.visible_pixels_output = 0;
        self.saw_right_edge_visible_same_x_cluster_this_line = false;
        self.window_wy_latch = false;
        self.window_force_x0_this_line = false;
        self.window_started_this_line = false;
        self.wx0_scx_shortening_applied = false;
        self.wx166_armed_this_line = false;
    }

    fn start_line(&mut self, scx: u8) {
        self.mode3_started = true;
        self.initial_scx_discard = scx & 0x07;
        self.mode0_start_dot = MODE0_START_DOT + u16::from(self.initial_scx_discard);
        self.scx_discard_remaining = self.initial_scx_discard;
        self.fifo.clear();
        self.fifo_cached_pixels.clear();
        self.startup_fetch_seam = BgStartupFetchSeamState::AlignmentSeedPending;
        self.startup_fifo_placeholders = MODE3_ABSTRACT_SOURCE_WINDOW_DOTS;
        self.startup_source_state = Mode3StartupSourceState::EntryDelay {
            remaining: MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT as u8,
        };
        self.startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
        self.transfer_phase = Mode3TransferPhase::Priming;
        self.current_transfer_x = 0;
        self.saw_right_edge_visible_same_x_cluster_this_line = false;
        self.push.reset();
        self.fill.reset();
        self.fetcher.start_background();
    }

    fn prepare_window_line(&mut self, wy_latch: bool, force_x0_this_line: bool) {
        self.window_wy_latch = wy_latch;
        self.window_force_x0_this_line = force_x0_this_line;
        self.window_started_this_line = false;
        self.wx0_scx_shortening_applied = false;
        self.wx166_armed_this_line = false;
    }

    fn extend_mode3_by_one_dot(&mut self) {
        self.mode0_start_dot += 1;
    }

    fn startup_transfer_window_open(&self, mode3_dot: u16) -> bool {
        if !self.mode3_started {
            return mode3_dot >= MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
        }

        !matches!(
            self.startup_source_state,
            Mode3StartupSourceState::EntryDelay { .. }
        )
    }

    fn consume_startup_transfer_entry_delay_dot(&mut self) -> bool {
        if !self.mode3_started {
            return false;
        }

        match self.startup_source_state {
            Mode3StartupSourceState::EntryDelay { remaining } => {
                debug_assert!(
                    remaining > 0,
                    "entry delay state must keep a positive countdown"
                );
                if remaining == 1 {
                    self.startup_source_state = Mode3StartupSourceState::Abstract {
                        remaining: MODE3_ABSTRACT_SOURCE_WINDOW_DOTS,
                    };
                } else {
                    self.startup_source_state = Mode3StartupSourceState::EntryDelay {
                        remaining: remaining - 1,
                    };
                }
                true
            }
            Mode3StartupSourceState::Abstract { .. } | Mode3StartupSourceState::FifoBacked => false,
        }
    }

    fn current_startup_source_window(&self, mode3_dot: u16) -> Mode3TransferSourceWindow {
        if !self.mode3_started {
            if mode3_dot < MODE3_BG_FETCH_PRIMING_DOTS {
                return Mode3TransferSourceWindow::AbstractStartup;
            }

            return Mode3TransferSourceWindow::FifoBacked;
        }

        match self.startup_source_state {
            Mode3StartupSourceState::EntryDelay { .. }
            | Mode3StartupSourceState::Abstract { .. } => {
                Mode3TransferSourceWindow::AbstractStartup
            }
            Mode3StartupSourceState::FifoBacked => Mode3TransferSourceWindow::FifoBacked,
        }
    }

    fn current_startup_transfer_lane(&self) -> Mode3TransferLane {
        if self.startup_pre_visible_transfer_dots_remaining > 0 {
            Mode3TransferLane::PreVisible
        } else {
            Mode3TransferLane::Hidden
        }
    }

    fn consume_startup_source_window_dot(&mut self) {
        if !self.mode3_started {
            return;
        }

        match self.startup_source_state {
            Mode3StartupSourceState::Abstract { remaining } => {
                debug_assert!(
                    remaining > 0,
                    "abstract startup state must keep a positive countdown"
                );
                if remaining == 1 {
                    self.startup_source_state = Mode3StartupSourceState::FifoBacked;
                } else {
                    self.startup_source_state = Mode3StartupSourceState::Abstract {
                        remaining: remaining - 1,
                    };
                }
            }
            Mode3StartupSourceState::EntryDelay { .. } | Mode3StartupSourceState::FifoBacked => {}
        }
    }

    fn consume_startup_pre_visible_transfer_dot(&mut self) {
        if self.startup_pre_visible_transfer_dots_remaining > 0 {
            self.startup_pre_visible_transfer_dots_remaining -= 1;
        }
    }

    fn effective_fifo_is_empty(&self) -> bool {
        self.startup_fifo_placeholders == 0 && self.fifo.is_empty()
    }

    fn fifo_contains_real_pixels(&self) -> bool {
        self.fifo.len() > self.startup_fifo_placeholders as usize
    }

    fn consume_effective_fifo_pixel(&mut self) -> Option<u8> {
        if self.startup_fifo_placeholders > 0 {
            self.startup_fifo_placeholders -= 1;
            self.pop_fifo_pixel().map(BgFifoPixel::color).or(Some(0))
        } else {
            self.pop_real_fifo_pixel()
        }
    }

    fn pop_real_fifo_pixel(&mut self) -> Option<u8> {
        self.pop_fifo_pixel().map(BgFifoPixel::color)
    }

    fn pop_fifo_pixel(&mut self) -> Option<BgFifoPixel> {
        let color = self.fifo.pop_front()?;
        debug_assert!(self.fifo_cached_pixels.len() <= self.fifo.len() + 1);
        let cached = self.fifo_cached_pixels.pop_front().unwrap_or(None);
        Some(BgFifoPixel { color, cached })
    }

    fn pop_visible_fifo_pixel(&mut self) -> Option<BgFifoPixel> {
        self.pop_fifo_pixel()
    }

    fn mark_live_lcdc3_write_while_fifo_visible(&mut self, previous_lcdc: u8, lcdc: u8) {
        for cached in self.fifo_cached_pixels.iter_mut().flatten() {
            cached
                .cached
                .mark_live_lcdc3_write_while_fifo_visible(previous_lcdc, lcdc);
        }
    }

    fn push_dummy_fifo_pixels(&mut self, count: u8) {
        self.fifo.extend(std::iter::repeat_n(0, count as usize));
        self.fifo_cached_pixels
            .extend(std::iter::repeat_n(None, count as usize));
    }

    fn push_cached_slice_fifo_pixels(&mut self, cached: BgCachedSlice) {
        for pixel_index in 0..BG_TILE_WIDTH {
            self.fifo.push_back(bg_tile_pixel_value(
                cached.tile_low,
                cached.tile_high,
                pixel_index,
            ));
            self.fifo_cached_pixels
                .push_back(Some(BgFifoPixelCached::new(cached, pixel_index)));
        }
    }

    fn apply_wx0_scx_shortening(&mut self) {
        if self.wx0_scx_shortening_applied || self.mode0_start_dot == 0 {
            return;
        }

        self.wx0_scx_shortening_applied = true;
        self.mode0_start_dot -= 1;
    }

    fn peek_startup_background_fetch_origin(&self) -> BgCachedSliceOrigin {
        match self.startup_fetch_seam {
            BgStartupFetchSeamState::PostAlignment {
                next_startup_continuation_slice,
                startup_continuation_visible_tiles_remaining,
                ..
            } if startup_continuation_visible_tiles_remaining > 0 => {
                BgCachedSliceOrigin::from_startup_continuation_slice(
                    next_startup_continuation_slice,
                )
            }
            BgStartupFetchSeamState::Inactive
            | BgStartupFetchSeamState::AlignmentSeedPending
            | BgStartupFetchSeamState::PostAlignment { .. } => BgCachedSliceOrigin::Ordinary,
        }
    }

    fn startup_alignment_seed_pending(&self) -> bool {
        matches!(
            self.startup_fetch_seam,
            BgStartupFetchSeamState::AlignmentSeedPending
        )
    }

    fn startup_background_tilemap_uses_pipeline_snapshot(&self) -> bool {
        match self.startup_fetch_seam {
            BgStartupFetchSeamState::Inactive => false,
            BgStartupFetchSeamState::AlignmentSeedPending => true,
            BgStartupFetchSeamState::PostAlignment {
                delayed_background_tilemap_tiles_remaining,
                ..
            } => delayed_background_tilemap_tiles_remaining > 0,
        }
    }

    fn startup_background_tiledata_uses_pipeline_snapshot(&self) -> bool {
        match self.startup_fetch_seam {
            BgStartupFetchSeamState::Inactive => false,
            BgStartupFetchSeamState::AlignmentSeedPending => true,
            BgStartupFetchSeamState::PostAlignment {
                delayed_background_tileindex_read_tiles_remaining: _,
                delayed_background_tiledata_tiles_remaining,
                ..
            } => delayed_background_tiledata_tiles_remaining > 0,
        }
    }

    fn startup_background_tileindex_reads_on_stage_one(&self) -> bool {
        match self.startup_fetch_seam {
            BgStartupFetchSeamState::Inactive | BgStartupFetchSeamState::AlignmentSeedPending => {
                false
            }
            BgStartupFetchSeamState::PostAlignment {
                delayed_background_tileindex_read_tiles_remaining,
                ..
            } => delayed_background_tileindex_read_tiles_remaining > 0,
        }
    }

    fn begin_post_alignment_followup(&mut self) {
        self.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: true,
            next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        };
    }

    fn take_startup_first_real_push_skip_entry_delay(&mut self) -> bool {
        let skip_entry_delay = match &mut self.startup_fetch_seam {
            BgStartupFetchSeamState::PostAlignment {
                first_real_push_skips_entry_delay,
                ..
            } => {
                let skip = *first_real_push_skips_entry_delay;
                *first_real_push_skips_entry_delay = false;
                skip
            }
            BgStartupFetchSeamState::Inactive | BgStartupFetchSeamState::AlignmentSeedPending => {
                false
            }
        };
        self.maybe_finish_startup_fetch_seam();
        skip_entry_delay
    }

    fn advance_startup_background_fetch_tile(&mut self) {
        if let BgStartupFetchSeamState::PostAlignment {
            next_startup_continuation_slice,
            startup_continuation_visible_tiles_remaining,
            delayed_background_tileindex_read_tiles_remaining,
            delayed_background_tilemap_tiles_remaining,
            delayed_background_tiledata_tiles_remaining,
            ..
        } = &mut self.startup_fetch_seam
        {
            if *startup_continuation_visible_tiles_remaining > 0 {
                *next_startup_continuation_slice = next_startup_continuation_slice.next();
                *startup_continuation_visible_tiles_remaining -= 1;
            }
            if *delayed_background_tileindex_read_tiles_remaining > 0 {
                *delayed_background_tileindex_read_tiles_remaining -= 1;
            }
            if *delayed_background_tilemap_tiles_remaining > 0 {
                *delayed_background_tilemap_tiles_remaining -= 1;
            }
            if *delayed_background_tiledata_tiles_remaining > 0 {
                *delayed_background_tiledata_tiles_remaining -= 1;
            }
        }
        self.maybe_finish_startup_fetch_seam();
    }

    fn maybe_finish_startup_fetch_seam(&mut self) {
        if let BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: false,
            next_startup_continuation_slice: _,
            startup_continuation_visible_tiles_remaining: 0,
            delayed_background_tileindex_read_tiles_remaining: 0,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 0,
        } = self.startup_fetch_seam
        {
            self.startup_fetch_seam = BgStartupFetchSeamState::Inactive;
        }
    }
}

impl Default for BgPipelineState {
    fn default() -> Self {
        Self {
            fetcher: BgFetcherState::default(),
            push: BgPushState::default(),
            fill: BgFifoFillState::default(),
            fifo: VecDeque::default(),
            fifo_cached_pixels: VecDeque::default(),
            startup_fetch_seam: BgStartupFetchSeamState::Inactive,
            startup_fifo_placeholders: 0,
            mode3_started: false,
            mode0_start_dot: MODE0_START_DOT,
            initial_scx_discard: 0,
            scx_discard_remaining: 0,
            startup_source_state: Mode3StartupSourceState::FifoBacked,
            startup_pre_visible_transfer_dots_remaining: MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS,
            transfer_phase: Mode3TransferPhase::Priming,
            current_transfer_x: 0,
            visible_pixels_output: 0,
            saw_right_edge_visible_same_x_cluster_this_line: false,
            window_wy_latch: false,
            window_force_x0_this_line: false,
            window_started_this_line: false,
            wx0_scx_shortening_applied: false,
            wx166_armed_this_line: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BgStartupFetchSeamState {
    #[default]
    Inactive,
    AlignmentSeedPending,
    PostAlignment {
        first_real_push_skips_entry_delay: bool,
        next_startup_continuation_slice: BgStartupContinuationSlice,
        startup_continuation_visible_tiles_remaining: u8,
        delayed_background_tileindex_read_tiles_remaining: u8,
        delayed_background_tilemap_tiles_remaining: u8,
        delayed_background_tiledata_tiles_remaining: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BgStartupContinuationSlice {
    #[default]
    None,
    VisibleTile2,
    VisibleTile3,
}

impl BgStartupContinuationSlice {
    const fn next(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::VisibleTile2 => Self::VisibleTile3,
            Self::VisibleTile3 => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct BgFetcherState {
    source: PpuBgFetcherSource,
    stage: PpuBgFetcherStage,
    stage_dot: u8,
    cached_origin: BgCachedSliceOrigin,
    needs_live_tilemap_refetch_on_push: bool,
    fetch_x: u16,
    next_fetch_pixel: u16,
    post_alignment_fetch_restart_delay_dots: u8,
    window_tilemap_x: u8,
    bg_resume_fetch_pixel: u16,
    rewind_bg_resume_after_first_tile_index_dot: bool,
    first_window_tile_after_activation: bool,
    tile_map_address: u16,
    tile_data_address: u16,
    tile_index: u8,
    tile_low: u8,
    tile_high: u8,
}

impl BgFetcherState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn start_background(&mut self) {
        self.source = PpuBgFetcherSource::Background;
        self.start_common(0);
    }

    fn start_window(&mut self, bg_resume_fetch_pixel: u16) {
        self.source = PpuBgFetcherSource::Window;
        self.stage = PpuBgFetcherStage::WindowActivating;
        self.stage_dot = 0;
        self.cached_origin = BgCachedSliceOrigin::Ordinary;
        self.needs_live_tilemap_refetch_on_push = false;
        self.fetch_x = 0;
        self.next_fetch_pixel = 0;
        self.post_alignment_fetch_restart_delay_dots = 0;
        self.window_tilemap_x = 0;
        self.bg_resume_fetch_pixel = bg_resume_fetch_pixel;
        self.rewind_bg_resume_after_first_tile_index_dot = true;
        self.first_window_tile_after_activation = true;
        self.tile_map_address = 0;
        self.tile_data_address = 0;
        self.tile_index = 0;
        self.tile_low = 0;
        self.tile_high = 0;
    }

    fn start_common(&mut self, bg_resume_fetch_pixel: u16) {
        self.stage = PpuBgFetcherStage::TileIndex;
        self.stage_dot = 0;
        self.cached_origin = BgCachedSliceOrigin::Ordinary;
        self.needs_live_tilemap_refetch_on_push = false;
        self.fetch_x = 0;
        self.next_fetch_pixel = 0;
        self.post_alignment_fetch_restart_delay_dots = 0;
        self.window_tilemap_x = 0;
        self.bg_resume_fetch_pixel = bg_resume_fetch_pixel;
        self.rewind_bg_resume_after_first_tile_index_dot = false;
        self.first_window_tile_after_activation = false;
        self.tile_map_address = 0;
        self.tile_data_address = 0;
        self.tile_index = 0;
        self.tile_low = 0;
        self.tile_high = 0;
    }

    fn abort_window_to_background(&mut self) {
        if self.source != PpuBgFetcherSource::Window {
            return;
        }

        self.source = PpuBgFetcherSource::Background;
        self.cached_origin = BgCachedSliceOrigin::Ordinary;
        self.needs_live_tilemap_refetch_on_push = false;
        self.fetch_x = self.bg_resume_fetch_pixel;
        self.next_fetch_pixel = self.bg_resume_fetch_pixel;
        self.post_alignment_fetch_restart_delay_dots = 0;
        self.window_tilemap_x = 0;
        self.first_window_tile_after_activation = false;
    }

    fn mark_live_lcdc3_write_for_current_background_fetch(&mut self, previous_lcdc: u8, lcdc: u8) {
        if self.source != PpuBgFetcherSource::Background
            || (previous_lcdc ^ lcdc) & LCDC_BG_TILE_MAP_BIT == 0
            || !matches!(
                self.cached_origin,
                BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
            )
            || !matches!(
                self.stage,
                PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
            )
        {
            return;
        }

        self.needs_live_tilemap_refetch_on_push = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct BgPushState {
    pending: bool,
    disposition: BgPushDisposition,
    entry_delay_remaining: u8,
    terminal_placeholder_tail_extra_hold_remaining: u8,
    just_activated_window_tile: bool,
    next_fetch_pixel: u16,
    cached: BgCachedSlice,
}

impl BgPushState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn queue_from_fetcher(&mut self, fetcher: BgFetcherState) {
        self.pending = true;
        self.disposition = BgPushDisposition::Ready;
        self.terminal_placeholder_tail_extra_hold_remaining = 0;
        self.just_activated_window_tile = fetcher.first_window_tile_after_activation;
        self.entry_delay_remaining = if self.just_activated_window_tile {
            0
        } else {
            1
        };
        self.next_fetch_pixel = fetcher.fetch_x.wrapping_add(BG_TILE_WIDTH as u16);
        self.cached = BgCachedSlice::from_fetcher(fetcher);
    }

    fn queue_startup_alignment_seed_from_fetcher(&mut self, fetcher: BgFetcherState) {
        self.pending = true;
        self.disposition = BgPushDisposition::Ready;
        self.terminal_placeholder_tail_extra_hold_remaining = 0;
        self.just_activated_window_tile = fetcher.first_window_tile_after_activation;
        self.entry_delay_remaining = 0;
        self.next_fetch_pixel = fetcher.fetch_x.wrapping_add(BG_TILE_WIDTH as u16);
        self.cached = BgCachedSlice::from_fetcher(fetcher)
            .with_origin(BgCachedSliceOrigin::StartupAlignmentSeed);
    }

    fn interrupt_for_object_fetch(&mut self) {
        if !self.pending {
            return;
        }

        self.disposition = BgPushDisposition::InterruptedByObjectFetch;
    }

    fn resume_after_object_fetch(&mut self) {
        if self.pending && self.disposition == BgPushDisposition::InterruptedByObjectFetch {
            self.disposition = BgPushDisposition::Ready;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct BgFifoFillState {
    pending: bool,
    startup_dummy_pixels: u8,
    includes_real_tile_pixels: bool,
    cached: BgCachedSlice,
}

impl BgFifoFillState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn queue_from_push(&mut self, push: BgPushState) {
        self.pending = true;
        self.startup_dummy_pixels = 0;
        self.includes_real_tile_pixels = true;
        self.cached = push.cached;
    }

    fn queue_startup_alignment_from_push(&mut self, push: BgPushState, startup_dummy_pixels: u8) {
        self.pending = true;
        self.startup_dummy_pixels = startup_dummy_pixels;
        self.includes_real_tile_pixels = true;
        self.cached = push.cached.with_origin(push.cached.queued_fill_origin());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BgCachedSliceOrigin {
    #[default]
    Ordinary,
    StartupAlignmentSeed,
    StartupAlignmentFill,
    StartupContinuation(BgStartupContinuationSlice),
}

impl BgCachedSliceOrigin {
    const fn from_startup_continuation_slice(slice: BgStartupContinuationSlice) -> Self {
        match slice {
            BgStartupContinuationSlice::None => Self::Ordinary,
            slice => Self::StartupContinuation(slice),
        }
    }

    const fn startup_continuation_slice(self) -> BgStartupContinuationSlice {
        match self {
            Self::StartupContinuation(slice) => slice,
            Self::Ordinary | Self::StartupAlignmentSeed | Self::StartupAlignmentFill => {
                BgStartupContinuationSlice::None
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct BgCachedSlice {
    source: PpuBgFetcherSource,
    origin: BgCachedSliceOrigin,
    fetch_x: u16,
    same_cycle_live_tilemap_refetch_window_open: bool,
    needs_live_tilemap_refetch: bool,
    needs_live_tile_data_refetch: bool,
    needs_live_tile_data_current_row_refetch: bool,
    needs_live_tile_data_unsigned_reuse: bool,
    tile_map_address: u16,
    tile_data_address: u16,
    tile_index: u8,
    tile_low: u8,
    tile_high: u8,
}

impl BgCachedSlice {
    fn from_fetcher(fetcher: BgFetcherState) -> Self {
        Self {
            source: fetcher.source,
            origin: fetcher.cached_origin,
            fetch_x: fetcher.fetch_x,
            same_cycle_live_tilemap_refetch_window_open: false,
            needs_live_tilemap_refetch: fetcher.needs_live_tilemap_refetch_on_push,
            needs_live_tile_data_refetch: false,
            needs_live_tile_data_current_row_refetch: false,
            needs_live_tile_data_unsigned_reuse: false,
            tile_map_address: fetcher.tile_map_address,
            tile_data_address: fetcher.tile_data_address,
            tile_index: fetcher.tile_index,
            tile_low: fetcher.tile_low,
            tile_high: fetcher.tile_high,
        }
    }

    fn with_origin(mut self, origin: BgCachedSliceOrigin) -> Self {
        self.origin = origin;
        self
    }

    const fn is_background(self) -> bool {
        matches!(self.source, PpuBgFetcherSource::Background)
    }

    const fn is_startup_alignment_seed(self) -> bool {
        matches!(self.origin, BgCachedSliceOrigin::StartupAlignmentSeed)
    }

    const fn queued_fill_origin(self) -> BgCachedSliceOrigin {
        match self.origin {
            BgCachedSliceOrigin::StartupAlignmentSeed => BgCachedSliceOrigin::StartupAlignmentFill,
            origin => origin,
        }
    }

    const fn startup_continuation_slice(self) -> BgStartupContinuationSlice {
        self.origin.startup_continuation_slice()
    }

    const fn is_second_or_third_visible_post_startup_push(self) -> bool {
        matches!(
            (self.startup_continuation_slice(), self.fetch_x),
            (BgStartupContinuationSlice::VisibleTile2, x) if x == BG_TILE_WIDTH as u16
        ) || matches!(
            (self.startup_continuation_slice(), self.fetch_x),
            (BgStartupContinuationSlice::VisibleTile3, x) if x == BG_TILE_WIDTH as u16 * 2
        )
    }

    fn mark_live_register_write_while_push_pending(
        &mut self,
        address: u16,
        previous_lcdc: u8,
        lcdc: u8,
        entry_delay_active: bool,
    ) {
        if !self.is_background() || self.is_startup_alignment_seed() {
            return;
        }

        let tile_data_selector_changed = (previous_lcdc ^ lcdc) & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;
        let needs_tilemap_refetch = address == 0xFF40
            && (previous_lcdc ^ lcdc) & LCDC_BG_TILE_MAP_BIT != 0
            && (entry_delay_active
                || self.same_cycle_live_tilemap_refetch_window_open
                || self.is_second_or_third_visible_post_startup_push());
        let needs_tile_data_refetch = address == 0xFF40 && tile_data_selector_changed
            || address == 0xFF42;

        self.needs_live_tilemap_refetch |= needs_tilemap_refetch;
        self.needs_live_tile_data_refetch |= needs_tile_data_refetch;
        self.needs_live_tile_data_current_row_refetch |= address == 0xFF42;
    }

    fn mark_live_register_write_while_fill_pending(
        &mut self,
        address: u16,
        previous_lcdc: u8,
        lcdc: u8,
        includes_real_tile_pixels: bool,
        startup_dummy_pixels: u8,
    ) {
        if !self.is_background() || !includes_real_tile_pixels {
            return;
        }

        if address == 0xFF40
            && (previous_lcdc ^ lcdc) & LCDC_BG_TILE_MAP_BIT != 0
            && startup_dummy_pixels == 0
            && (self.same_cycle_live_tilemap_refetch_window_open
                || self.is_second_or_third_visible_post_startup_push())
        {
            self.needs_live_tilemap_refetch = true;
        }

        let tile_data_selector_changed = (previous_lcdc ^ lcdc) & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;
        let needs_tile_data_refetch = address == 0xFF40 && tile_data_selector_changed
            || address == 0xFF42;

        self.needs_live_tile_data_refetch |= needs_tile_data_refetch;
        self.needs_live_tile_data_current_row_refetch |= address == 0xFF42;
    }

    fn mark_live_lcdc3_write_while_fifo_visible(&mut self, previous_lcdc: u8, lcdc: u8) {
        if !self.is_background()
            || (previous_lcdc ^ lcdc) & LCDC_BG_TILE_MAP_BIT == 0
            || !self.is_second_or_third_visible_post_startup_push()
        {
            return;
        }

        self.needs_live_tilemap_refetch = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BgFifoPixelCached {
    cached: BgCachedSlice,
    pixel_index: u8,
}

impl BgFifoPixelCached {
    const fn new(cached: BgCachedSlice, pixel_index: u8) -> Self {
        Self {
            cached,
            pixel_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BgFifoPixel {
    color: u8,
    cached: Option<BgFifoPixelCached>,
}

impl BgFifoPixel {
    const fn color(self) -> u8 {
        self.color
    }
}

fn recompute_live_background_cached_slice(
    mut cached: BgCachedSlice,
    vram: &VramBusView<'_>,
    lcdc: u8,
    scy: u8,
    ly: u8,
    last_unsigned_tile_data_low_fetch: u8,
    last_unsigned_tile_data_high_fetch: u8,
) -> Option<BgCachedSlice> {
    if cached.source != PpuBgFetcherSource::Background
        || (!cached.needs_live_tilemap_refetch
            && !cached.needs_live_tile_data_refetch
            && !cached.needs_live_tile_data_current_row_refetch
            && !cached.needs_live_tile_data_unsigned_reuse)
    {
        return None;
    }

    let mut tile_map_address = cached.tile_map_address;
    let mut tile_index = cached.tile_index;
    if cached.needs_live_tilemap_refetch {
        let tile_map_offset = cached.tile_map_address & 0x03FF;
        let tile_map_base = if lcdc & LCDC_BG_TILE_MAP_BIT != 0 {
            0x1C00
        } else {
            0x1800
        };
        tile_map_address = tile_map_base | tile_map_offset;
        tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
    }

    let tile_data_row = if cached.needs_live_tile_data_refetch {
        if cached.needs_live_tile_data_current_row_refetch {
            u16::from(scy.wrapping_add(ly) % BG_TILE_WIDTH)
        } else {
            (cached.tile_data_address.saturating_sub(1) & (TILE_BYTES - 1)) / TILE_ROW_BYTES
        }
    } else {
        (cached.tile_data_address.saturating_sub(1) & (TILE_BYTES - 1)) / TILE_ROW_BYTES
    };
    let tile_low_address = bg_tile_data_base(lcdc, tile_index) + tile_data_row * TILE_ROW_BYTES;
    let tile_high_address = tile_low_address + 1;
    let (tile_low, tile_high) =
        if cached.needs_live_tile_data_unsigned_reuse && !cached.needs_live_tilemap_refetch {
            (
                last_unsigned_tile_data_low_fetch,
                last_unsigned_tile_data_high_fetch,
            )
        } else {
            (
                vram.read(tile_low_address as usize).unwrap_or(0),
                vram.read(tile_high_address as usize).unwrap_or(0),
            )
        };

    cached.tile_map_address = tile_map_address;
    cached.tile_data_address = tile_high_address;
    cached.tile_index = tile_index;
    cached.tile_low = tile_low;
    cached.tile_high = tile_high;
    cached.needs_live_tilemap_refetch = false;
    cached.needs_live_tile_data_refetch = false;
    cached.needs_live_tile_data_current_row_refetch = false;
    cached.needs_live_tile_data_unsigned_reuse = false;
    Some(cached)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BgPushDisposition {
    #[default]
    Ready,
    InterruptedByObjectFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BgPushDotResult {
    NotReady,
    EntryDelay,
    WaitingForEmptyFifo,
    HandedOffToObjectFetch,
    QueuedFillAndHandedOffToObjectFetch,
    QueuedFill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BgPushDotOwnership {
    NotReady,
    EntryDelay,
    WaitingForEmptyFifo,
    FifoBackedTransferObjectFetch,
    QueueFill,
    QueueFillThenObjectFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3DotArbitration {
    bg_transfer_can_advance: bool,
    obj_fetch_can_start_from_fifo_backed_transfer: bool,
    obj_fetch_can_start_from_queued_bg_fill: bool,
}

impl Mode3DotArbitration {
    const fn can_serve_bg_transfer(self) -> bool {
        self.bg_transfer_can_advance
    }

    const fn can_start_obj_fetch(self, start_source: ObjFetchStartSource) -> bool {
        match start_source {
            ObjFetchStartSource::FifoBackedTransfer => {
                self.obj_fetch_can_start_from_fifo_backed_transfer
            }
            ObjFetchStartSource::QueuedBgFill | ObjFetchStartSource::PushCachedBgFetch => {
                self.obj_fetch_can_start_from_queued_bg_fill
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjFetchStartSource {
    FifoBackedTransfer,
    PushCachedBgFetch,
    QueuedBgFill,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct WindowState {
    wy_triggered: bool,
    pending_wx166_next_line: bool,
    window_line_counter: u8,
}

impl WindowState {
    fn reset(&mut self) {
        self.wy_triggered = false;
        self.pending_wx166_next_line = false;
        self.window_line_counter = 0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct StatState {
    irq_line: bool,
    lcd_disabled_lyc_coincidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ObjPipelineState {
    fifo: VecDeque<ObjPixel>,
    fetched_sprite_slots: [bool; MAX_SELECTED_SPRITES_PER_LINE],
    pending_sprite_slots: VecDeque<u8>,
    pending_match_x: Option<u8>,
    late_metadata_word: Option<(u8, u8)>,
    fetch: ObjFetchState,
}

impl ObjPipelineState {
    fn reset(&mut self) {
        self.fifo.clear();
        self.fetched_sprite_slots.fill(false);
        self.pending_sprite_slots.clear();
        self.pending_match_x = None;
        self.late_metadata_word = None;
        self.fetch = ObjFetchState::default();
    }

    fn start_fetch(&mut self, sprite_slot: u8, sprite: PpuSelectedSprite) {
        self.fetch.stage = PpuObjFetcherStage::Startup;
        self.fetch.stage_dot = 0;
        self.fetch.sprite_slot = sprite_slot;
        self.fetch.sprite = Some(sprite);
        self.fetch.resolved_sprite = None;
        self.fetch.cancelled = false;
        self.fetch.count_terminal_push_dot = false;
        self.fetch.tile_low = 0;
        self.fetch.tile_high = 0;
    }

    fn mark_fetched(&mut self, sprite_slot: u8) {
        self.fetched_sprite_slots[sprite_slot as usize] = true;
    }

    fn has_fetched(&self, sprite_slot: u8) -> bool {
        self.fetched_sprite_slots[sprite_slot as usize]
    }

    fn queue_fetch_hit(&mut self, sprite_slot: u8, owner: ObjHitOwnership) {
        if self.has_fetched(sprite_slot)
            || self
                .pending_sprite_slots
                .iter()
                .any(|queued_slot| *queued_slot == sprite_slot)
            || (self.fetch.stage != PpuObjFetcherStage::Idle
                && self.fetch.sprite_slot == sprite_slot)
        {
            return;
        }

        if self.pending_sprite_slots.is_empty() {
            self.pending_match_x = Some(owner.match_x);
        } else {
            debug_assert_eq!(self.pending_match_x, Some(owner.match_x));
        }
        self.pending_sprite_slots.push_back(sprite_slot);
    }

    fn pop_pending_fetch_hit(&mut self) -> Option<u8> {
        let sprite_slot = self.pending_sprite_slots.pop_front();
        if self.pending_sprite_slots.is_empty() {
            self.pending_match_x = None;
        }
        sprite_slot
    }

    fn pending_hits_own_current_dot(&self, current_owner: ObjHitOwnership) -> bool {
        self.pending_match_x == Some(current_owner.match_x) && !self.pending_sprite_slots.is_empty()
    }

    fn clear_pending_fetch_hits(&mut self) {
        self.pending_sprite_slots.clear();
        self.pending_match_x = None;
    }

    fn clear_pending_fetch_hits_if_stale(&mut self, current_owner: ObjHitOwnership) {
        if self.fetch.stage != PpuObjFetcherStage::Idle {
            return;
        }

        if self.pending_match_x.is_some() && self.pending_match_x != Some(current_owner.match_x) {
            self.clear_pending_fetch_hits();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjHitOwnership {
    match_x: u8,
    phase: ObjHitPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjHitPhase {
    PreVisible,
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ObjFetchState {
    stage: PpuObjFetcherStage,
    stage_dot: u8,
    sprite_slot: u8,
    sprite: Option<PpuSelectedSprite>,
    resolved_sprite: Option<PpuSelectedSprite>,
    cancelled: bool,
    count_terminal_push_dot: bool,
    tile_low: u8,
    tile_high: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjPixel {
    color: u8,
    palette_obp1: bool,
    bg_over_obj: bool,
    sprite_x: u8,
    oam_index: u8,
}

impl ObjPixel {
    const fn transparent() -> Self {
        Self {
            color: 0,
            palette_obp1: false,
            bg_over_obj: false,
            sprite_x: u8::MAX,
            oam_index: u8::MAX,
        }
    }

    const fn is_transparent(self) -> bool {
        self.color == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MixedPixel {
    color: u8,
    source: MixedPixelSource,
}

impl MixedPixel {
    const fn background(color: u8) -> Self {
        Self {
            color,
            source: MixedPixelSource::Background,
        }
    }

    const fn object(color: u8, palette_obp1: bool) -> Self {
        Self {
            color,
            source: MixedPixelSource::Object { palette_obp1 },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MixedPixelSource {
    Background,
    Object { palette_obp1: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mode2ScanState {
    scanned_entries: u8,
    selected_sprite_count: u8,
    selected_sprites: [Option<PpuSelectedSprite>; MAX_SELECTED_SPRITES_PER_LINE],
    latched_mode2_yx_word: Option<(u8, u8)>,
}

impl Mode2ScanState {
    fn reset_scanline(&mut self) {
        self.scanned_entries = 0;
        self.selected_sprite_count = 0;
        self.selected_sprites.fill(None);
    }

    fn reset(&mut self) {
        self.reset_scanline();
        self.latched_mode2_yx_word = None;
    }

    fn scanned_entries(&self) -> u8 {
        self.scanned_entries
    }

    fn increment_scanned_entries(&mut self) {
        self.scanned_entries += 1;
    }

    fn latch_mode2_yx_word(&mut self, y: u8, x: u8) {
        self.latched_mode2_yx_word = Some((y, x));
    }

    fn latched_mode2_yx_word(&self) -> Option<(u8, u8)> {
        self.latched_mode2_yx_word
    }

    fn selected_sprite_count(&self) -> u8 {
        self.selected_sprite_count
    }

    fn is_full(&self) -> bool {
        self.selected_sprite_count as usize == MAX_SELECTED_SPRITES_PER_LINE
    }

    fn push(&mut self, sprite: PpuSelectedSprite) {
        if self.is_full() {
            return;
        }

        let slot = self.selected_sprite_count as usize;
        self.selected_sprites[slot] = Some(sprite);
        self.selected_sprite_count += 1;
    }

    fn selected_sprites_snapshot(&self) -> Vec<PpuSelectedSprite> {
        self.selected_sprites
            .iter()
            .take(self.selected_sprite_count as usize)
            .flatten()
            .copied()
            .collect()
    }

    fn selected_sprite(&self, slot: u8) -> Option<PpuSelectedSprite> {
        self.selected_sprites
            .get(slot as usize)
            .and_then(|sprite| *sprite)
    }
}

impl Default for Mode2ScanState {
    fn default() -> Self {
        Self {
            scanned_entries: 0,
            selected_sprite_count: 0,
            selected_sprites: [None; MAX_SELECTED_SPRITES_PER_LINE],
            latched_mode2_yx_word: None,
        }
    }
}
