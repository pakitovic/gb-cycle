use super::*;

#[test]
fn first_hidden_same_x_cluster_fetch_can_skip_obj_tile_data_low_byte_when_bg_fetcher_is_on_tile_data_high_1()
 {
    let mut ppu = PpuTestRig::dmg();
    ppu.write_oam_entry(0, 16, 12, 0);
    ppu.write_oam_entry(1, 16, 12, 1);
    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 8;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 4;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    fill_bg_fifo(&mut ppu, 12);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;

    let current_sprite = same_x_test_sprite(0, 12);
    let next_sprite = same_x_test_sprite(1, 12);
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(4);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.start_fetch(0, current_sprite);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Startup;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
}

#[test]
fn first_hidden_same_x_cluster_fetch_at_x_six_keeps_the_low_byte_half_step() {
    let mut ppu = PpuTestRig::dmg();
    ppu.write_oam_entry(0, 16, 14, 0);
    ppu.write_oam_entry(1, 16, 14, 1);
    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 12;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 6;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    fill_bg_fifo(&mut ppu, 10);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;

    let current_sprite = same_x_test_sprite(0, 14);
    let next_sprite = same_x_test_sprite(1, 14);
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(6);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.start_fetch(0, current_sprite);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Startup;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite, Some(current_sprite));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.resolved_sprite,
        Some(current_sprite)
    );
}

#[test]
fn first_hidden_same_x_cluster_fetch_at_x_seven_keeps_the_full_low_byte() {
    let mut ppu = PpuTestRig::dmg();
    ppu.write_oam_entry(0, 16, 15, 0);
    ppu.write_oam_entry(1, 16, 15, 1);
    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 14;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    fill_bg_fifo(&mut ppu, 9);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;

    let current_sprite = same_x_test_sprite(0, 15);
    let next_sprite = same_x_test_sprite(1, 15);
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(7);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.start_fetch(0, current_sprite);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Startup;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite, Some(current_sprite));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.resolved_sprite,
        Some(current_sprite)
    );
}

#[test]
fn same_x_cluster_at_x_mod_8_eq_2_waits_until_the_next_dot_for_startup() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 2;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 6;
    fill_bg_fifo(&mut ppu, 14);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    push_same_x_test_sprites(&mut ppu, 2, 2);
    ppu.obj_pipeline_state.pending_match_x = Some(2);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(0);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);

    assert!(!ppu.try_start_object_fetch_from_current_dot(
        ObjFetchStartSource::FifoBackedTransfer,
        true,
    ));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
}

#[test]
fn long_same_x_obj_chain_waits_one_dot_before_the_terminal_restart() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 8;
    fill_bg_fifo(&mut ppu, 16);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    push_same_x_test_sprites(&mut ppu, 8, 6);
    for sprite_slot in 0..6_u8 {
        if sprite_slot < 5 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = same_x_test_sprite(4, 8);
    ppu.obj_pipeline_state.pending_match_x = Some(0);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(5);
    arm_object_fetch_push_stage(&mut ppu, 4, current_sprite);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(
        ppu.bg_pipeline_state.mode0_start_dot,
        MODE0_START_DOT,
        "fetch={:?} pending={:?} match_x={:?}",
        ppu.obj_pipeline_state.fetch,
        ppu.obj_pipeline_state.pending_sprite_slots,
        ppu.obj_pipeline_state.pending_match_x
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(0));
    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![5]
    );
}

#[test]
fn visible_same_x_obj_chain_with_early_start_does_not_use_long_tail_restart() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 1;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    push_same_x_test_sprites(&mut ppu, 167, 10);
    for sprite_slot in 0..10_u8 {
        if sprite_slot < 9 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = same_x_test_sprite(8, 167);
    ppu.obj_pipeline_state.pending_match_x = Some(167);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(9);
    arm_object_fetch_push_stage(&mut ppu, 8, current_sprite);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(167));
    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![9]
    );
    assert!(!ppu.obj_pipeline_state.fetch.count_terminal_push_dot);
}

#[test]
fn x_mod_8_eq_2_same_x_obj_chain_restart_reuses_the_current_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 16;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 10;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 6;
    fill_bg_fifo(&mut ppu, 14);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    let current_sprite = same_x_test_sprite(0, 2);
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(same_x_test_sprite(1, 2));
    ppu.obj_pipeline_state.pending_match_x = Some(2);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    arm_object_fetch_push_stage(&mut ppu, 0, current_sprite);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 10);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite_slot, 1);
}

#[test]
fn x_mod_8_eq_3_same_x_obj_chain_restart_reuses_the_current_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 16;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 10;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 3;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 5;
    fill_bg_fifo(&mut ppu, 14);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    let current_sprite = same_x_test_sprite(0, 3);
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(same_x_test_sprite(1, 3));
    ppu.obj_pipeline_state.pending_match_x = Some(3);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    arm_object_fetch_push_stage(&mut ppu, 0, current_sprite);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 10);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite_slot, 1);
}

#[test]
fn terminal_previsible_x_mod_8_eq_2_same_x_chain_skips_startup_and_low_byte() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 20;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 20;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 6;
    fill_bg_fifo(&mut ppu, 14);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    push_same_x_test_sprites(&mut ppu, 2, 10);
    for sprite_slot in 0..10_u8 {
        if sprite_slot < 8 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = same_x_test_sprite(8, 2);
    ppu.obj_pipeline_state.pending_match_x = Some(2);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(9);
    arm_object_fetch_push_stage(&mut ppu, 8, current_sprite);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 20);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite_slot, 9);
}

#[test]
fn hidden_x_mod_8_eq_4_late_same_x_chain_skips_first_low_half_step() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 24;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 20;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 4;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    fill_bg_fifo(&mut ppu, 12);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;

    push_same_x_test_sprites(&mut ppu, 4, 10);
    for sprite_slot in 0..10_u8 {
        if sprite_slot < 5 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = same_x_test_sprite(5, 4);
    ppu.obj_pipeline_state.pending_match_x = Some(4);
    for sprite_slot in 6..10_u8 {
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .push_back(sprite_slot);
    }
    arm_object_fetch_push_stage(&mut ppu, 5, current_sprite);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 21);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite_slot, 6);
}

#[test]
#[ignore = "diagnostic count=3 same-x push1 restart for x mod 8 == 2"]
fn x_mod_8_eq_2_count3_same_x_chain_logs_post_push1_restart_state() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 20;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 20;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 6;
    fill_bg_fifo(&mut ppu, 14);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    push_same_x_test_sprites(&mut ppu, 2, 10);
    for sprite_slot in 0..10_u8 {
        if sprite_slot < 2 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = same_x_test_sprite(2, 2);
    ppu.obj_pipeline_state.pending_match_x = Some(2);
    for sprite_slot in 3..10_u8 {
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .push_back(sprite_slot);
    }
    arm_object_fetch_push_stage(&mut ppu, 2, current_sprite);

    let transfer = ppu.current_transfer().expect("transfer should exist");
    println!("count3_before_transfer={transfer:?}");
    println!(
        "count3_before_previsible_can_start={} count3_before_arbitration={:?}",
        ppu.previsible_same_x_chain_can_start_obj_fetch(transfer),
        ppu.current_dot_arbitration()
    );

    assert!(ppu.advance_object_fetch_with_ppu_video(None));

    println!(
        "count3_after_fetch_stage={:?} stage_dot={} pending_match_x={:?} pending_len={} mode0_start_dot={}",
        ppu.obj_pipeline_state.fetch.stage,
        ppu.obj_pipeline_state.fetch.stage_dot,
        ppu.obj_pipeline_state.pending_match_x,
        ppu.obj_pipeline_state.pending_sprite_slots.len(),
        ppu.bg_pipeline_state.mode0_start_dot
    );
}

#[test]
fn x_mod_8_eq_7_same_x_obj_chain_restart_waits_before_reusing_the_full_low_byte() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 20;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 10;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    fill_bg_fifo(&mut ppu, 9);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    let current_sprite = same_x_test_sprite(0, 15);
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(same_x_test_sprite(1, 15));
    ppu.obj_pipeline_state.pending_match_x = Some(7);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    arm_object_fetch_push_stage(&mut ppu, 0, current_sprite);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 10);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(7));
    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );
}

#[test]
fn terminal_same_x_obj_chain_with_single_pending_slot_does_not_restart_but_still_counts_the_terminal_push_dot()
 {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 1;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    let current_sprite = same_x_test_sprite(0, 167);
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(same_x_test_sprite(1, 167));
    ppu.obj_pipeline_state.pending_match_x = Some(167);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    arm_object_fetch_push_stage(&mut ppu, 0, current_sprite);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 2);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(167));
    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );
}
