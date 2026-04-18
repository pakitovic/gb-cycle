use super::*;

#[test]
fn first_window_tile_skips_the_normal_push_entry_delay() {
    let mut ppu = PpuTestRig::dmg();
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; TEST_VRAM_BYTES]);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0xB1;
    ppu.bg_pipeline_state.fetcher.start_window(8);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x55;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x33;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(ppu.bg_pipeline_state.push.just_activated_window_tile);
    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .first_window_tile_after_activation
    );
}

#[test]
fn first_window_tile_push_ignores_pending_obj_fetch_start() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xA3;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.push.just_activated_window_tile = true;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    let ownership = ppu.current_obj_hit_ownership();
    let obj_height = ppu.current_obj_height();
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ownership, obj_height);

    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFill
    );
}

#[test]
fn wx_zero_previsible_window_start_requires_a_late_fifo_backed_served_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 0;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;

    let not_ready_dot = ppu.advance_mode3_output_phase();
    assert_eq!(not_ready_dot.kind, Mode3TransferDotKind::NotServed);
    assert!(!ppu.maybe_start_window_after_transfer_dot(not_ready_dot));

    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;

    let ready_dot = ppu.advance_mode3_output_phase();

    assert_eq!(ready_dot.kind, Mode3TransferDotKind::ServedHiddenTransfer);
    assert!(ppu.maybe_start_window_after_transfer_dot(ready_dot));
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
}

#[test]
fn wx_zero_starts_after_first_visible_scx_discard_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 0;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.initial_scx_discard = 1;
    ppu.bg_pipeline_state.scx_discard_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;

    let transfer_dot = ppu.advance_mode3_output_phase();
    ppu.maybe_apply_wx0_shortening_after_transfer_dot(transfer_dot);

    assert_eq!(transfer_dot.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 1);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 8);
    assert!(ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
}

#[test]
fn wx_zero_last_scx_discard_shortening_is_applied_from_the_served_transfer_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 0;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT + 2;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 3;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.initial_scx_discard = 3;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    let transfer_dot = ppu.advance_mode3_output_phase();
    ppu.maybe_apply_wx0_shortening_after_transfer_dot(transfer_dot);

    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 2);
    assert!(!ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn wx_seven_starts_window_from_the_first_served_x0_transfer_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 7;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;

    let transfer_dot = ppu.advance_mode3_output_phase();

    assert_eq!(
        transfer_dot.kind,
        Mode3TransferDotKind::ServedHiddenTransfer
    );
    assert!(ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert!(ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn dmg_window_trigger_uses_the_previous_dot_wx_snapshot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 8;
    ppu.pipeline_registers.lcdc = 0xF1;
    ppu.pipeline_registers.wx = 7;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;

    let transfer_dot = ppu.advance_mode3_output_phase();

    assert_eq!(
        transfer_dot.kind,
        Mode3TransferDotKind::ServedHiddenTransfer
    );
    assert!(ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert!(ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn pending_obj_hit_blocks_window_start_because_the_output_dot_is_not_served() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0x93;
    ppu.visible_registers.wx = 15;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.visible_pixels_output = 8;
    ppu.bg_pipeline_state.fifo.push_back(1);
    let ownership = ppu.current_obj_hit_ownership();
    let obj_height = ppu.current_obj_height();
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ownership, obj_height);

    let transfer_dot = ppu.advance_mode3_output_phase();

    assert_eq!(transfer_dot.kind, Mode3TransferDotKind::NotServed);
    assert!(!ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert!(!ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn window_fetcher_aborts_to_background_and_restores_bg_progress_when_win_enable_turns_off() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tilemap_entry(1, 0, 0x11);
    ppu.write_window_tilemap_entry(0, 0, 0x22);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 0;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
    ppu.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = 8;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Background
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 8);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x11);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1801);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);
}

#[test]
fn low_wx_window_disable_waits_for_the_current_window_tile_before_aborting() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tilemap_entry(1, 0, 0x11);
    ppu.write_bg_tile_row(0x11, 0, 0x12, 0x34);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.visible_registers.wx = 0x00;
    ppu.pipeline_registers.wx = 0x00;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 0;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
    ppu.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = 8;
    ppu.bg_pipeline_state.fetcher.tile_index = 0x22;
    ppu.bg_pipeline_state.fetcher.tile_low = 0xAA;
    ppu.bg_pipeline_state.fetcher.tile_high = 0xBB;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataLow
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);
}

#[test]
fn low_wx_window_abort_retargets_the_fetch_registers_to_background_bytes_at_the_boundary() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tilemap_entry(1, 0, 0x11);
    ppu.write_bg_tile_row(0x11, 0, 0x12, 0x34);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.visible_registers.wx = 0x00;
    ppu.pipeline_registers.wx = 0x00;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.fetch_x = 0;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
    ppu.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = 8;
    ppu.bg_pipeline_state.fetcher.tile_index = 0x22;
    ppu.bg_pipeline_state.fetcher.tile_low = 0xAA;
    ppu.bg_pipeline_state.fetcher.tile_high = 0xBB;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Background
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1801);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x11);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x34);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);
}

#[test]
fn wx0_window_disable_prefix_override_repaints_the_extended_prefix_tail() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.current_scanline_bg_pixels[8] = 2;
    ppu.current_scanline_mixed_pixels[8] = MixedPixel::background(2);
    ppu.current_scanline_mixed_pixels[9] = MixedPixel::background(0);
    ppu.bg_pipeline_state.dmg_wx0_window_disable_prefix_state =
        Some(DmgWx0WindowDisablePrefixState::new(10));

    ppu.test_apply_dmg_wx0_window_disable_prefix_override(8, 1);
    assert_eq!(
        ppu.bg_pipeline_state.dmg_wx0_window_disable_prefix_state,
        Some(DmgWx0WindowDisablePrefixState {
            desired_prefix_pixels: 10,
            prefix_bg_pixel: Some(2),
        })
    );

    ppu.test_apply_dmg_wx0_window_disable_prefix_override(9, 1);
    assert_eq!(ppu.framebuffer[9], 2);
    assert_eq!(ppu.current_scanline_bg_pixels[9], 2);
    assert_eq!(
        ppu.bg_pipeline_state.dmg_wx0_window_disable_prefix_state,
        None
    );
}

#[test]
fn wx0_window_disable_prefix_override_retroactively_shifts_the_short_prefix_case() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.current_scanline_mixed_pixels[3..8].fill(MixedPixel::background(0));
    ppu.bg_pipeline_state.dmg_wx0_window_disable_prefix_state =
        Some(DmgWx0WindowDisablePrefixState::new(3));

    for visible_x in 8..13 {
        ppu.test_apply_dmg_wx0_window_disable_prefix_override(visible_x, 3);
    }

    assert_eq!(&ppu.framebuffer[3..8], &[3; 5]);
    assert_eq!(&ppu.current_scanline_bg_pixels[3..8], &[3; 5]);
    assert_eq!(
        ppu.bg_pipeline_state.dmg_wx0_window_disable_prefix_state,
        None
    );
}

#[test]
fn window_disable_records_a_pending_reenable_resume_for_supported_dmg_wx_rows() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tilemap_entry(4, 0, 0x11);
    ppu.write_bg_tile_row(0x11, 0, 0x12, 0x34);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.wx = 28;
    ppu.pipeline_registers.wx = 28;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.visible_pixels_output = 10;
    for context in &mut ppu.current_scanline_bg_dot_contexts[..10] {
        *context = Some(PpuRecentBgDotContext {
            source: PpuBgFetcherSource::Window,
            fetch_x: 0,
            pixel_index: 0,
            tile_index: 0,
        });
    }
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.fetch_x = 32;
    ppu.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = 32;

    ppu.maybe_abort_window_fetcher_to_background(&VramBusView::new(BusMaster::Ppu, &mut vram));

    assert_eq!(
        ppu.bg_pipeline_state.dmg_pending_window_reenable_resume,
        Some(DmgPendingWindowReenableResume::new(
            37,
            21,
            10,
            PpuBgFetcherStage::TileDataHigh,
            1,
        ))
    );
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Background
    );
}

#[test]
fn pending_window_reenable_resume_arms_and_repaints_the_saved_segment() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_window_tilemap_entry(2, 0, 0x20);
    ppu.write_bg_tile_row(0x20, 0, 0xFF, 0xFF);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.visible_registers.wx = 28;
    ppu.pipeline_registers.wx = 28;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 45;
    ppu.window_state.window_line_counter = 0;
    ppu.current_scanline_mixed_pixels[..45].fill(MixedPixel::background(0));
    ppu.bg_pipeline_state.dmg_pending_window_reenable_resume = Some(
        DmgPendingWindowReenableResume::new(37, 21, 10, PpuBgFetcherStage::TileDataHigh, 1),
    );

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(
        ppu.bg_pipeline_state.dmg_late_window_enable_override,
        Some(DmgLateWindowEnableOverride::new(37, 45, 21))
    );
    assert_eq!(
        ppu.bg_pipeline_state.dmg_pending_window_reenable_resume,
        None
    );

    ppu.test_apply_dmg_late_window_enable_override_repaint_up_to(
        45,
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(&ppu.framebuffer[..37], &[0; 37]);
    assert_eq!(&ppu.framebuffer[37..45], &[3; 8]);
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn late_window_enable_for_wx16_arms_and_repaints_the_observed_segment() {
    let mut ppu = PpuTestRig::dmg();

    for tilemap_x in 0..4 {
        ppu.write_window_tilemap_entry(tilemap_x, 0, 0x20);
    }
    ppu.write_bg_tile_row(0x20, 0, 0xFF, 0xFF);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.visible_registers.wx = 16;
    ppu.pipeline_registers.wx = 16;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 14;
    ppu.window_state.window_line_counter = 0;
    ppu.current_scanline_mixed_pixels[..34].fill(MixedPixel::background(0));

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(
        ppu.bg_pipeline_state.dmg_late_window_enable_override,
        Some(DmgLateWindowEnableOverride::new(10, 34, 9))
    );

    ppu.test_apply_dmg_late_window_enable_override_repaint_up_to(
        34,
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(&ppu.framebuffer[..10], &[0; 10]);
    assert_eq!(&ppu.framebuffer[10..34], &[3; 24]);
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn wx15_late_window_enable_repaints_the_white_glitch_pixel() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.visible_registers.wx = 15;
    ppu.pipeline_registers.wx = 15;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 13;
    ppu.current_scanline_mixed_pixels[8] = MixedPixel::background(3);
    ppu.framebuffer[8] = 3;

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(ppu.framebuffer[8], 0);
    assert_eq!(ppu.current_scanline_bg_pixels[8], 0);
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn wx39_late_window_enable_repaints_the_white_glitch_pixel() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.visible_registers.wx = 39;
    ppu.pipeline_registers.wx = 39;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 33;
    ppu.current_scanline_mixed_pixels[32] = MixedPixel::background(3);
    ppu.framebuffer[32] = 3;

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(ppu.framebuffer[32], 0);
    assert_eq!(ppu.current_scanline_bg_pixels[32], 0);
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn real_window_restart_clears_the_pending_and_active_dmg_reenable_state() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 7;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
    ppu.bg_pipeline_state.dmg_pending_window_reenable_resume = Some(
        DmgPendingWindowReenableResume::new(37, 21, 10, PpuBgFetcherStage::TileDataHigh, 1),
    );
    ppu.bg_pipeline_state.dmg_late_window_enable_override =
        Some(DmgLateWindowEnableOverride::new(37, 45, 21));

    let transfer_dot = ppu.advance_mode3_output_phase();

    assert!(ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert_eq!(
        ppu.bg_pipeline_state.dmg_pending_window_reenable_resume,
        None
    );
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn pending_reenable_resume_uses_the_forced_x0_window_origin() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tilemap_entry(1, 0, 0x11);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.wx = 35;
    ppu.pipeline_registers.wx = 35;
    ppu.bg_pipeline_state.window_force_x0_this_line = true;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.visible_pixels_output = 8;
    for context in &mut ppu.current_scanline_bg_dot_contexts[..8] {
        *context = Some(PpuRecentBgDotContext {
            source: PpuBgFetcherSource::Window,
            fetch_x: 0,
            pixel_index: 0,
            tile_index: 0,
        });
    }
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = 8;

    ppu.maybe_abort_window_fetcher_to_background(&VramBusView::new(BusMaster::Ppu, &mut vram));

    assert_eq!(
        ppu.bg_pipeline_state.dmg_pending_window_reenable_resume,
        Some(DmgPendingWindowReenableResume::new(
            8,
            0,
            8,
            PpuBgFetcherStage::TileIndex,
            0,
        ))
    );
}

#[test]
fn late_window_enable_does_not_arm_after_window_pixels_have_already_been_emitted() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.wx = 16;
    ppu.pipeline_registers.wx = 16;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 14;
    ppu.current_scanline_bg_dot_contexts[0] = Some(PpuRecentBgDotContext {
        source: PpuBgFetcherSource::Window,
        fetch_x: 0,
        pixel_index: 0,
        tile_index: 0,
    });

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn wx44_late_window_enable_repaints_from_the_clamped_onset_to_line_end() {
    let mut ppu = PpuTestRig::dmg();

    for tilemap_x in 0..BG_TILE_MAP_WIDTH {
        ppu.write_window_tilemap_entry(tilemap_x, 0, 0x20);
    }
    ppu.write_bg_tile_row(0x20, 0, 0xFF, 0xFF);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.visible_registers.wx = 44;
    ppu.pipeline_registers.wx = 44;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 42;
    ppu.window_state.window_line_counter = 0;
    ppu.current_scanline_mixed_pixels[..SCREEN_WIDTH].fill(MixedPixel::background(0));

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(
        ppu.bg_pipeline_state.dmg_late_window_enable_override,
        Some(DmgLateWindowEnableOverride::new(38, SCREEN_WIDTH as u8, 37))
    );

    ppu.test_apply_dmg_late_window_enable_override_repaint_up_to(
        SCREEN_WIDTH,
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(&ppu.framebuffer[..38], &[0; 38]);
    assert!(
        ppu.framebuffer[38..SCREEN_WIDTH]
            .iter()
            .all(|&pixel| pixel == 3)
    );
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn late_window_enable_repaint_skips_pre_origin_and_object_owned_pixels() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_window_tilemap_entry(0, 0, 0x20);
    ppu.write_bg_tile_row(0x20, 0, 0xFF, 0xFF);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xF1;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.window_state.window_line_counter = 0;
    ppu.current_scanline_mixed_pixels[6] = MixedPixel::background(0);
    ppu.current_scanline_mixed_pixels[7] = MixedPixel::object(1, false);
    ppu.current_scanline_mixed_pixels[8] = MixedPixel::background(0);
    ppu.framebuffer[7] = 2;
    ppu.bg_pipeline_state.dmg_late_window_enable_override =
        Some(DmgLateWindowEnableOverride::new(6, 9, 7));

    ppu.test_apply_dmg_late_window_enable_override_repaint_up_to(
        9,
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(ppu.framebuffer[6], 0);
    assert_eq!(ppu.framebuffer[7], 2);
    assert_eq!(ppu.framebuffer[8], 3);
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn late_window_enable_partial_repaint_keeps_the_override_active_and_updates_panel_history() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_window_tilemap_entry(0, 0, 0x20);
    ppu.write_bg_tile_row(0x20, 0, 0xFF, 0xFF);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xF1;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.window_state.window_line_counter = 0;
    ppu.current_scanline_mixed_pixels[8..12].fill(MixedPixel::background(0));
    for visible_x in 8..12 {
        ppu.dmg_panel_live_write_state
            .recent_panel_dots
            .push_back(PpuRecentPanelDot {
                visible_x,
                pixel: MixedPixel::background(0),
                dmg_bg_forced_white: false,
            });
    }
    ppu.bg_pipeline_state.dmg_late_window_enable_override =
        Some(DmgLateWindowEnableOverride::new(8, 12, 8));

    ppu.test_apply_dmg_late_window_enable_override_repaint_up_to(
        10,
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(&ppu.framebuffer[8..10], &[3, 3]);
    assert_eq!(&ppu.framebuffer[10..12], &[0, 0]);
    assert_eq!(
        ppu.bg_pipeline_state.dmg_late_window_enable_override,
        Some(DmgLateWindowEnableOverride::new(8, 12, 8))
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state.recent_panel_dots[0],
        PpuRecentPanelDot {
            visible_x: 8,
            pixel: MixedPixel::background(3),
            dmg_bg_forced_white: false,
        }
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state.recent_panel_dots[1],
        PpuRecentPanelDot {
            visible_x: 9,
            pixel: MixedPixel::background(3),
            dmg_bg_forced_white: false,
        }
    );
}

#[test]
fn wx35_pending_reenable_resume_arms_the_documented_eight_pixel_segment() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_window_tilemap_entry(1, 0, 0x20);
    ppu.write_bg_tile_row(0x20, 0, 0xFF, 0xFF);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.visible_registers.wx = 35;
    ppu.pipeline_registers.wx = 35;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 16;
    ppu.window_state.window_line_counter = 0;
    ppu.current_scanline_mixed_pixels[..16].fill(MixedPixel::background(0));
    ppu.bg_pipeline_state.dmg_pending_window_reenable_resume = Some(
        DmgPendingWindowReenableResume::new(8, 0, 8, PpuBgFetcherStage::TileIndex, 0),
    );

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(
        ppu.bg_pipeline_state.dmg_late_window_enable_override,
        Some(DmgLateWindowEnableOverride::new(8, 16, 0))
    );

    ppu.test_apply_dmg_late_window_enable_override_repaint_up_to(
        16,
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(&ppu.framebuffer[..8], &[0; 8]);
    assert_eq!(&ppu.framebuffer[8..16], &[3; 8]);
}

#[test]
fn late_window_enable_with_out_of_range_wx_does_not_arm_an_override() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.wx = 200;
    ppu.pipeline_registers.wx = 200;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 42;

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn pending_reenable_resume_records_wx29_from_mixed_scanline_contexts() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tilemap_entry(4, 0, 0x11);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.wx = 29;
    ppu.pipeline_registers.wx = 29;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.current_scanline_bg_dot_contexts[0] = Some(PpuRecentBgDotContext {
        source: PpuBgFetcherSource::Window,
        fetch_x: 0,
        pixel_index: 0,
        tile_index: 0,
    });
    ppu.current_scanline_bg_dot_contexts[1] = Some(PpuRecentBgDotContext {
        source: PpuBgFetcherSource::Background,
        fetch_x: 8,
        pixel_index: 0,
        tile_index: 0,
    });
    ppu.current_scanline_bg_dot_contexts[3] = Some(PpuRecentBgDotContext {
        source: PpuBgFetcherSource::Window,
        fetch_x: 16,
        pixel_index: 0,
        tile_index: 0,
    });
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.fetch_x = 32;
    ppu.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = 32;

    ppu.maybe_abort_window_fetcher_to_background(&VramBusView::new(BusMaster::Ppu, &mut vram));

    assert_eq!(
        ppu.bg_pipeline_state.dmg_pending_window_reenable_resume,
        Some(DmgPendingWindowReenableResume::new(
            30,
            22,
            2,
            PpuBgFetcherStage::TileDataLow,
            1,
        ))
    );
}

#[test]
fn pending_reenable_resume_with_an_unsupported_wx_does_not_arm_an_override() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.wx = 40;
    ppu.pipeline_registers.wx = 40;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 20;
    ppu.bg_pipeline_state.dmg_pending_window_reenable_resume =
        Some(DmgPendingWindowReenableResume::new(
            SCREEN_WIDTH as u8,
            21,
            8,
            PpuBgFetcherStage::TileIndex,
            0,
        ));

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
    assert_eq!(
        ppu.bg_pipeline_state.dmg_pending_window_reenable_resume,
        None
    );
}

#[test]
fn supported_wx_without_a_matching_late_enable_class_leaves_no_override() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_registers.lcdc = 0xF1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.visible_registers.wx = 30;
    ppu.pipeline_registers.wx = 30;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 20;

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
}

#[test]
fn first_window_tile_index_dot_rewinds_bg_resume_progress_by_one_tile() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tilemap_entry(1, 0, 0x11);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.bg_pipeline_state.fetcher.start_window(8);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Background
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.bg_resume_fetch_pixel, 0);
}

#[test]
fn window_fetcher_advances_tilemap_x_on_tile_index_dot_zero() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_window_tilemap_entry(0, 0, 0x11);
    ppu.write_window_tilemap_entry(1, 0, 0x22);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0xE1;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.bg_pipeline_state.fetcher.start_window(8);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C00);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x11);
    assert_eq!(ppu.bg_pipeline_state.fetcher.window_tilemap_x, 1);

    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C01);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x22);
    assert_eq!(ppu.bg_pipeline_state.fetcher.window_tilemap_x, 2);
}

#[test]
fn window_fetcher_rereads_the_unsigned_tile_data_byte_when_tile_selector_flips_to_unsigned_on_high1()
 {
    let mut ppu = PpuTestRig::dmg();

    ppu.vram_bytes[0x1011] = 0x34;
    ppu.vram_bytes[0x0011] = 0x78;
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0xA1;
    ppu.pipeline_registers.lcdc = 0xA1;
    ppu.window_state.window_line_counter = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x34);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1011);

    ppu.pipeline_registers.lcdc = 0xA1;
    ppu.visible_registers.lcdc = 0xB1;
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x78);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0011);
    assert!(ppu.bg_pipeline_state.push.pending);
}

#[test]
fn window_fetcher_consumes_previous_tiledata_selector_latch_on_low0_after_lcdc4_flip() {
    let mut ppu = PpuTestRig::dmg();

    ppu.vram_bytes[0x0000] = 0xFF;
    ppu.vram_bytes[0x1000] = 0x00;
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xB3;
    ppu.window_state.window_line_counter = 8;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.window_tilemap_x = 1;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state
        .fetcher
        .dmg_lcdc4_previous_tiledata_select_on_next_low = Some(BgTileDataSelect::Unsigned8000);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xFF);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0000);
    assert_eq!(
        ppu.bg_pipeline_state
            .fetcher
            .dmg_lcdc4_previous_tiledata_select_on_next_low,
        None
    );
}

#[test]
fn window_fetcher_keeps_the_current_low_plane_after_unsigned_to_signed_flip_on_low1() {
    let mut ppu = PpuTestRig::dmg();

    ppu.vram_bytes[0x0000] = 0xFF;
    ppu.vram_bytes[0x1000] = 0x00;
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xB3;
    ppu.window_state.window_line_counter = 32;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.window_tilemap_x = 1;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0000;
    ppu.bg_pipeline_state.fetcher.tile_low_address = 0x0000;
    ppu.bg_pipeline_state.fetcher.tile_low = 0xFF;
    ppu.bg_pipeline_state
        .fetcher
        .dmg_lcdc4_skip_window_current_low_glitch = true;

    ppu.maybe_apply_bgwin_tile_data_selector_glitch(
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        PpuBgFetcherSource::Window,
        0,
    );

    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xFF);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0000);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low_address, 0x0000);
    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .dmg_lcdc4_skip_window_current_low_glitch
    );
}

#[test]
fn window_start_restarts_the_fetcher_and_switches_to_window_pixels_mid_scanline() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.write_bg_tile_row(1, 0, 0xCC, 0xF0);
    ppu.write_bg_tilemap_entry(0, 0, 0);
    ppu.write_window_tilemap_entry(0, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x0F,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert!(snapshot.window_wy_latch);
    assert!(snapshot.window_started_this_line);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 3, 3, 2, 2, 1, 1, 0, 0]
    );
}

#[test]
fn wy_latch_is_sampled_at_mode2_start_and_not_recomputed_mid_line() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.write_bg_tile_row(1, 0, 0xCC, 0xF0);
    ppu.write_bg_tilemap_entry(0, 0, 0);
    ppu.write_window_tilemap_entry(0, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x01,
        wx: 0x07,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.tick_n(100);

    ppu.write_register(0xFF4A, 0x00);

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert!(!snapshot.window_wy_latch);
    assert!(!snapshot.window_started_this_line);
    assert_eq!(snapshot.window_line_counter, 0);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3]
    );
}

#[test]
fn window_line_counter_advances_only_on_lines_where_window_actually_starts() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.write_bg_tile_row(1, 0, 0xCC, 0xF0);
    ppu.write_bg_tilemap_entry(0, 0, 0);
    ppu.write_window_tilemap_entry(0, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0xA7,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.advance_until_line_start(1);
    assert_eq!(ppu.snapshot().window_line_counter, 0);

    ppu.write_register(0xFF4B, 0x07);

    ppu.advance_until_line_start(2);
    let line_2_start = ppu.snapshot();
    assert_eq!(line_2_start.window_line_counter, 1);
}

#[test]
fn same_scanline_window_restart_advances_to_the_next_internal_row() {
    let mut ppu = PpuTestRig::dmg();

    ppu.window_state.window_line_counter = 6;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 24;
    ppu.start_window_fetcher_restart();
    assert_eq!(ppu.bg_pipeline_state.window_active_line_counter, 6);
    assert_eq!(ppu.bg_pipeline_state.window_start_count_this_line, 1);

    ppu.bg_pipeline_state.fetcher.abort_window_to_background();
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 24;
    ppu.start_window_fetcher_restart();
    assert_eq!(ppu.bg_pipeline_state.window_active_line_counter, 7);
    assert_eq!(ppu.bg_pipeline_state.window_start_count_this_line, 2);
    assert_eq!(
        ppu.window_state
            .window_line_counter
            .wrapping_add(ppu.bg_pipeline_state.window_start_count_this_line),
        8
    );
}

#[test]
fn wx_zero_with_scx_discard_shortens_window_start_timing_by_one_dot() {
    let mut wx_zero = PpuTestRig::dmg();
    wx_zero.write_bg_tile_row(0, 0, 0x55, 0x33);
    wx_zero.write_window_tilemap_entry(0, 0, 0);
    wx_zero.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x03,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let mut wx_seven = PpuTestRig::dmg();
    wx_seven.write_bg_tile_row(0, 0, 0x55, 0x33);
    wx_seven.write_window_tilemap_entry(0, 0, 0);
    wx_seven.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x03,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x07,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    wx_zero.advance_until_hblank();
    wx_seven.advance_until_hblank();

    assert_eq!(
        wx_zero.snapshot().mode0_start_dot + 1,
        wx_seven.snapshot().mode0_start_dot
    );
}

#[test]
fn wx_166_defers_window_start_to_the_following_scanline() {
    let mut ppu = PpuTestRig::dmg();

    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.write_bg_tile_row(1, 0, 0xCC, 0xF0);
    ppu.write_bg_tilemap_entry(0, 0, 0);
    ppu.write_window_tilemap_entry(0, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 166,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.advance_until_line_start(1);
    let first_line = ppu.snapshot();
    assert_eq!(first_line.window_line_counter, 0);

    ppu.advance_until_hblank();
    let second_line = ppu.snapshot();
    assert!(second_line.window_started_this_line);
    assert_eq!(
        &second_line.current_scanline_pixels[..8],
        &[3, 3, 2, 2, 1, 1, 0, 0]
    );
}
