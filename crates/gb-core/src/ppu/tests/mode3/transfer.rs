use super::super::*;

#[test]
fn bg_push_waits_for_fifo_space_without_losing_the_fetched_tile() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.fetch_x = 0;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
    ppu.bg_pipeline_state.fifo = (0..=8).collect();

    let result = ppu.advance_bg_push();

    assert_eq!(result, BgPushDotResult::WaitingForEmptyFifo);
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 0);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 9);

    for _ in 0..9 {
        let _ = ppu.bg_pipeline_state.fifo.pop_front();
    }
    let result = ppu.advance_bg_push();

    assert_eq!(result, BgPushDotResult::QueuedFill);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 8);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 0);

    ppu.flush_pending_bg_fifo_fill();

    assert!(!ppu.bg_pipeline_state.fill.pending);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 8);
    assert_eq!(
        ppu.bg_pipeline_state
            .fifo
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 0, 1, 2, 3]
    );
}

#[test]
fn current_bg_push_dot_ownership_distinguishes_fill_wait_and_obj_handoff_paths() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFill
    );

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    let current_obj_hit_ownership = ppu.current_obj_hit_ownership();
    let current_obj_height = ppu.current_obj_height();
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, current_obj_hit_ownership, current_obj_height);

    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFillThenObjectFetch
    );

    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.push_back(0);
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFillThenObjectFetch
    );

    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::FifoBackedTransferObjectFetch
    );

    ppu.obj_pipeline_state.clear_pending_fetch_hits();
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::WaitingForEmptyFifo
    );

    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::EntryDelay
    );

    ppu.bg_pipeline_state.push.pending = false;
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::NotReady
    );
}

#[test]
fn bg_push_stage_waits_one_dot_on_entry_then_retries_every_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.fetch_x = 0;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
    ppu.bg_pipeline_state.fifo = (0..=8).collect();

    assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::EntryDelay);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 9);

    assert_eq!(
        ppu.advance_bg_push_stage(),
        BgPushDotResult::WaitingForEmptyFifo
    );
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 9);

    for _ in 0..9 {
        let _ = ppu.bg_pipeline_state.fifo.pop_front();
    }
    assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::QueuedFill);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 8);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 0);

    ppu.flush_pending_bg_fifo_fill();

    assert!(!ppu.bg_pipeline_state.fill.pending);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 8);
}

#[test]
fn bg_push_queues_fifo_fill_before_the_fill_phase_materializes_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

    assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::QueuedFill);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());

    ppu.flush_pending_bg_fifo_fill();

    assert!(!ppu.bg_pipeline_state.fill.pending);
    assert_eq!(
        ppu.bg_pipeline_state
            .fifo
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 0, 1, 2, 3]
    );
}

#[test]
fn bg_push_stage_reports_not_ready_when_no_cached_slice_is_pending() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::NotReady);
}

#[test]
fn current_transfer_snapshot_keeps_context_and_readiness_together() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;

    let waiting = ppu
        .current_transfer()
        .expect("hidden startup dot must have transfer state");
    assert_eq!(
        waiting.context,
        Mode3TransferContext {
            lane: Mode3TransferLane::Hidden,
            source_window: Mode3TransferSourceWindow::FifoBacked,
        }
    );
    assert_eq!(
        waiting.readiness,
        Mode3TransferReadiness::WaitingForFifo(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );

    ppu.bg_pipeline_state.fifo.push_back(0);

    let ready = ppu
        .current_transfer()
        .expect("same hidden dot must stay describable");
    assert_eq!(ready.context, waiting.context);
    assert_eq!(ready.service_plan(), waiting.service_plan());
    assert_eq!(
        ready.readiness,
        Mode3TransferReadiness::Ready(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );
}

#[test]
fn transfer_service_plan_distinguishes_abstract_hidden_and_fifo_backed_visible_paths() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.fifo.push_back(0);

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::Abstract,
        })
    );

    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );

    ppu.bg_pipeline_state.current_transfer_x = 8;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedVisiblePixel,
            execution: Mode3TransferServiceExecution::EmitVisiblePixel,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );
}

#[test]
fn transfer_service_plan_rejects_non_visible_context_after_startup_tail() {
    let mut bg_pipeline_state = BgPipelineState {
        mode3_started: true,
        startup_source_state: Mode3StartupSourceState::FifoBacked,
        current_transfer_x: 8,
        ..BgPipelineState::default()
    };
    bg_pipeline_state.fifo.push_back(0);
    let policy = PpuMode3TransferPolicy::from_pipeline_state(
        &bg_pipeline_state,
        MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS,
    );

    assert_eq!(
        policy.transfer_service_plan(Mode3TransferContext {
            lane: Mode3TransferLane::Hidden,
            source_window: Mode3TransferSourceWindow::FifoBacked,
        }),
        None
    );
}

#[test]
fn bg_fifo_starvation_after_priming_does_not_advance_pre_visible_match_x() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 5;

    ppu.advance_mode3_output_phase();

    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
}

#[test]
fn abstract_previsible_scx_discard_keeps_lx_zero_until_hidden_transfer_begins() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedPreVisibleTransfer);
    assert!(result.consumed_scx_discard);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
    assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(
        ppu.bg_pipeline_state.transfer_phase,
        Mode3TransferPhase::Output
    );
}

#[test]
fn fifo_backed_hidden_service_moves_transfer_phase_to_output() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
    ppu.bg_pipeline_state.fifo.push_back(0);

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedHiddenTransfer);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 8);
    assert_eq!(
        ppu.bg_pipeline_state.transfer_phase,
        Mode3TransferPhase::Output
    );
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn bg_fifo_discard_after_priming_keeps_lx_zero_until_discard_finishes() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedHiddenTransfer);
    assert!(result.consumed_scx_discard);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
    assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
    assert_eq!(
        ppu.bg_pipeline_state.transfer_phase,
        Mode3TransferPhase::Output
    );
}

#[test]
fn visible_bg_pixel_output_reports_a_visible_pixel_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x91;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(2);

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 9);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 1);
}

#[test]
fn flushing_bg_fill_tracks_cached_slice_in_fifo_sideband() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fill.cached.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x33;

    ppu.flush_pending_bg_fifo_fill();

    assert_eq!(ppu.bg_pipeline_state.fifo.len(), BG_TILE_WIDTH as usize);
    assert_eq!(
        ppu.bg_pipeline_state.fifo.cached_len(),
        BG_TILE_WIDTH as usize
    );
    assert!(
        ppu.bg_pipeline_state
            .fifo
            .cached_slots()
            .enumerate()
            .all(|(pixel_index, cached)| {
                let Some(cached) = cached else {
                    return false;
                };
                cached.cached.origin
                    == BgCachedSliceOrigin::StartupContinuation(
                        BgStartupContinuationSlice::VisibleTile3,
                    )
                    && cached.cached.fetch_x == BG_TILE_WIDTH as u16 * 2
                    && cached.pixel_index == pixel_index as u8
            })
    );

    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    assert_eq!(ppu.bg_pipeline_state.fifo.cached_len(), 7);
}

#[test]
fn consuming_effective_fifo_pixel_keeps_the_visible_fifo_sideband_in_sync() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.push_back(2);
    ppu.bg_pipeline_state
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                tile_low: 0xAA,
                tile_high: 0x00,
                ..BgCachedSlice::default()
            },
            0,
        )));

    assert_eq!(
        ppu.bg_pipeline_state.consume_effective_fifo_pixel(),
        Some(2)
    );
    assert_eq!(ppu.bg_pipeline_state.startup_fifo_placeholders, 0);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn visible_fifo_pop_skips_residual_startup_placeholder_before_real_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fifo.push_back(3);
    ppu.bg_pipeline_state
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupAlignmentFill,
                fetch_x: 0,
                tile_low: 0x10,
                tile_high: 0x10,
                ..BgCachedSlice::default()
            },
            3,
        )));

    let pixel = ppu
        .bg_pipeline_state
        .pop_visible_fifo_pixel()
        .expect("visible BG pop should skip the placeholder and return the real pixel");

    assert_eq!(pixel.color(), 3);
    assert_eq!(ppu.bg_pipeline_state.startup_fifo_placeholders, 0);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn visible_fifo_pop_preserves_multi_placeholder_startup_tail_timing() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fifo.push_back(3);
    ppu.bg_pipeline_state
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupAlignmentFill,
                fetch_x: 0,
                tile_low: 0x10,
                tile_high: 0x10,
                ..BgCachedSlice::default()
            },
            3,
        )));

    let pixel = ppu
        .bg_pipeline_state
        .pop_visible_fifo_pixel()
        .expect("multi-placeholder startup tails remain timing-visible");

    assert_eq!(pixel.color(), 0);
    assert!(pixel.cached.is_none());
    assert_eq!(ppu.bg_pipeline_state.startup_fifo_placeholders, 2);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 2);
    assert_eq!(ppu.bg_pipeline_state.fifo.cached_len(), 2);
}
