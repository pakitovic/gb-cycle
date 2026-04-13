use super::*;

#[test]
fn latching_object_hits_queues_all_matching_sprite_slots_once() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;

    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 8, 0, 0));
    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(1, 16, 8, 1, 0));

    ppu.latch_object_fetch_hits();
    ppu.latch_object_fetch_hits();

    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn bg_push_can_handoff_to_a_latched_object_fetch_without_losing_the_tile() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
    ppu.bg_pipeline_state.fifo.push_back(0);

    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 8, 0, 0));
    queue_current_obj_hit(&mut ppu, 0);

    assert_eq!(
        ppu.advance_bg_push(),
        BgPushDotResult::HandedOffToObjectFetch
    );
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.push.disposition,
        BgPushDisposition::InterruptedByObjectFetch
    );
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::Startup
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 1);
    assert!(!ppu.bg_pipeline_state.fill.pending);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
}

#[test]
fn bg_push_with_an_empty_fifo_can_queue_fill_and_start_object_fetch_on_the_same_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 8, 0, 0));
    queue_current_obj_hit(&mut ppu, 0);

    assert_eq!(
        ppu.advance_bg_push(),
        BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
    );
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::Startup
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn current_dot_arbitration_distinguishes_fifo_backed_and_queued_fill_obj_start() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 8, 0, 0));
    queue_current_obj_hit(&mut ppu, 0);

    let empty_fifo = ppu.current_dot_arbitration();
    assert!(!empty_fifo.can_serve_bg_transfer());
    assert!(!empty_fifo.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
    assert!(empty_fifo.can_start_obj_fetch(ObjFetchStartSource::QueuedBgFill));

    ppu.bg_pipeline_state.fifo.push_back(0);

    let fifo_backed = ppu.current_dot_arbitration();
    assert!(!fifo_backed.can_serve_bg_transfer());
    assert!(fifo_backed.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
    assert!(fifo_backed.can_start_obj_fetch(ObjFetchStartSource::QueuedBgFill));
}

#[test]
fn fifo_backed_obj_start_requires_a_fifo_backed_transfer_dot_not_just_a_nonempty_fifo() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
    ppu.bg_pipeline_state.fifo.push_back(0);
    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 15, 0, 0));
    queue_current_obj_hit(&mut ppu, 0);

    let arbitration = ppu.current_dot_arbitration();
    assert!(!arbitration.can_serve_bg_transfer());
    assert!(!arbitration.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
    assert!(arbitration.can_start_obj_fetch(ObjFetchStartSource::QueuedBgFill));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
}

#[test]
fn fifo_backed_obj_start_waits_until_bg_fetcher_leaves_tile_data_low() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 8, 0, 0));
    queue_current_obj_hit(&mut ppu, 0);

    let tile_index = ppu.current_dot_arbitration();
    assert!(!tile_index.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    let tile_data_low = ppu.current_dot_arbitration();
    assert!(!tile_data_low.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    let tile_data_high = ppu.current_dot_arbitration();
    assert!(tile_data_high.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
}

#[test]
fn abstract_previsible_obj_start_keeps_startup_placeholders_non_fifo_backed() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 8;
    for _ in 0..16 {
        ppu.bg_pipeline_state.fifo.push_back(0);
    }
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 0, 0, 0));
    queue_current_obj_hit(&mut ppu, 0);

    let arbitration = ppu.current_dot_arbitration();
    assert!(!arbitration.can_serve_bg_transfer());
    assert!(!arbitration.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
}

#[test]
fn abstract_startup_service_kind_tracks_served_progress_not_raw_mode3_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS - 2;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
            execution: Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop,
            backing: Mode3TransferBacking::Abstract,
        })
    );

    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::Abstract,
        })
    );
}

#[test]
fn obj_hit_ownership_tracks_served_startup_progress_not_raw_mode3_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.line_dot = MODE2_DOTS + MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS - 2;

    assert_eq!(
        ppu.current_obj_hit_ownership().phase,
        ObjHitPhase::PreVisible
    );

    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;

    assert_eq!(ppu.current_obj_hit_ownership().phase, ObjHitPhase::Hidden);
}

#[test]
fn pending_obj_hit_blocks_output_phase_and_stretches_mode3() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 20;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.visible_pixels_output = 12;
    ppu.bg_pipeline_state.fifo.push_back(3);
    queue_current_obj_hit(&mut ppu, 0);

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::NotServed);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 12);
    assert_eq!(
        ppu.bg_pipeline_state
            .fifo
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![3]
    );
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
}

#[test]
fn pending_obj_hit_stalls_pre_visible_match_x_until_fetch_service() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 5;
    queue_current_obj_hit(&mut ppu, 0);

    ppu.advance_mode3_output_phase();

    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
}

#[test]
fn hidden_startup_dot_advances_pre_visible_match_x_without_bg_fifo_pop() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.current_transfer_x = 5;

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedPreVisibleTransfer);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 6);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn current_obj_hit_ownership_tracks_x_and_dot_phase() {
    let mut ppu = PpuTestRig::dmg();

    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
    ppu.bg_pipeline_state.current_transfer_x = 6;
    assert_eq!(
        ppu.current_obj_hit_ownership(),
        ObjHitOwnership {
            match_x: 6,
            phase: ObjHitPhase::PreVisible,
        }
    );

    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;
    assert_eq!(
        ppu.current_obj_hit_ownership(),
        ObjHitOwnership {
            match_x: 0,
            phase: ObjHitPhase::Hidden,
        }
    );

    ppu.bg_pipeline_state.scx_discard_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 20;
    ppu.bg_pipeline_state.visible_pixels_output = 12;
    assert_eq!(
        ppu.current_obj_hit_ownership(),
        ObjHitOwnership {
            match_x: 20,
            phase: ObjHitPhase::Visible,
        }
    );
}

#[test]
fn stale_pending_obj_hit_is_cleared_once_current_x_moves_on() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.bg_pipeline_state.current_transfer_x = 13;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.visible_pixels_output = 5;
    ppu.obj_pipeline_state.queue_fetch_hit(
        0,
        ObjHitOwnership {
            match_x: 12,
            phase: ObjHitPhase::Visible,
        },
    );

    ppu.sync_pending_obj_hit_ownership();

    assert!(ppu.obj_pipeline_state.pending_sprite_slots.is_empty());
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, None);
}

#[test]
fn pending_obj_hit_survives_dot_phase_changes_while_current_x_is_still_the_same() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 6;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.obj_pipeline_state.queue_fetch_hit(
        0,
        ObjHitOwnership {
            match_x: 6,
            phase: ObjHitPhase::PreVisible,
        },
    );

    ppu.sync_pending_obj_hit_ownership();

    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(6));
}

#[test]
fn terminal_fifo_backed_obj_start_extends_mode3_immediately_to_keep_fetch_alive() {
    let mut ppu = PpuTestRig::dmg();
    ppu.write_oam_entry(0, 16, 167, 0);
    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);

    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = MODE0_START_DOT - 1;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 167, 0, 0));
    queue_current_obj_hit(&mut ppu, 0);

    assert!(ppu.advance_mode3_object_phase_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
}

#[test]
fn late_visible_x160_obj_start_can_still_begin_from_fifo_backed_transfer() {
    let mut ppu = PpuTestRig::dmg();
    ppu.write_oam_entry(0, 16, 160, 0);
    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);

    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 66;
    ppu.line_dot = MODE0_START_DOT - 1;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 160;
    ppu.bg_pipeline_state.visible_pixels_output = 152;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    fill_bg_fifo(&mut ppu, 8);
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 160, 0, 0));
    queue_current_obj_hit(&mut ppu, 0);

    let transfer = ppu
        .current_transfer()
        .expect("late visible x160 should still have a transfer");
    assert_eq!(transfer.context.lane, Mode3TransferLane::Visible);
    assert!(transfer.can_start_obj_fetch_from_fifo_backed_transfer(
        ppu.bg_pipeline_state.fifo_contains_real_pixels()
    ));

    let arbitration = ppu.current_dot_arbitration();
    assert!(arbitration.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));

    assert!(ppu.advance_mode3_object_phase_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
}
