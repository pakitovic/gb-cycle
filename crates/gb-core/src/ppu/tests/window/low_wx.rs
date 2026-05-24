use super::*;

#[test]
fn same_scanline_live_wx_write_before_visible_output_arms_a_previsible_retarget() {
    let mut ppu = arm_previsible_retarget_fixture(4, MODE2_DOTS + 12, 3);

    ppu.maybe_arm_dmg_previsible_wx_retarget(4, 9);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new(2, 3, 5))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
}

#[test]
fn wx_cpu_commit_during_drawing_routes_through_previsible_retarget_logic() {
    let mut ppu = arm_previsible_retarget_fixture(4, MODE2_DOTS + 12, 3);

    ppu.write_register_with_source(0xFF4B, 9, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(ppu.wx, 9);
    assert_eq!(ppu.visible_registers.wx, 4);
    assert_eq!(ppu.pipeline_registers.wx, 4);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new(2, 3, 5))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
}

#[test]
fn same_scanline_low_wx_previsible_retarget_keeps_the_tile_boundary_carry_pixel() {
    let mut ppu = arm_previsible_retarget_fixture(4, MODE2_DOTS + 12, 3);

    ppu.maybe_arm_dmg_previsible_wx_retarget(4, 12);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new(5, 3, 7))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        Some(5)
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn same_scanline_low_wx_previsible_retarget_arms_a_pretrigger_carry_for_later_onsets() {
    let mut ppu = arm_previsible_retarget_fixture(4, MODE2_DOTS + 12, 3);

    ppu.maybe_arm_dmg_previsible_wx_retarget(4, 14);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new(7, 3, 10))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        Some(DmgPendingPrevisibleWxCarry::new(5, 7, 3, 8))
    );
}

#[test]
fn same_scanline_low_wx_boundary_retarget_keeps_boundary_restart_and_carry_span() {
    let mut ppu = arm_previsible_retarget_fixture(4, MODE2_DOTS + 12, 3);

    ppu.maybe_arm_dmg_previsible_wx_retarget(4, 20);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new(13, 3, 15))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        Some(13)
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        Some(DmgPendingPrevisibleWxCarry::new(5, 13, 3, 8))
    );
}

#[test]
fn same_scanline_previsible_wx_retarget_restarts_on_the_existing_window_row() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.visible_registers.wx = 9;
    ppu.pipeline_registers.wx = 9;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = false;
    ppu.bg_pipeline_state.window_start_count_this_line = 1;
    ppu.bg_pipeline_state.window_active_line_counter = 3;
    ppu.bg_pipeline_state.visible_pixels_output = 2;
    ppu.bg_pipeline_state.current_transfer_x = 10;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 24;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new(2, 3, 5));

    assert!(
        ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedVisiblePixel,
            false,
        ))
    );
    assert_eq!(ppu.bg_pipeline_state.window_active_line_counter, 3);
    assert_eq!(ppu.bg_pipeline_state.window_start_count_this_line, 1);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
}

#[test]
fn same_scanline_previsible_wx_retarget_uses_its_own_trigger_even_after_a_later_wx_write() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.visible_registers.wx = 0x50;
    ppu.pipeline_registers.wx = 0x50;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = false;
    ppu.bg_pipeline_state.window_start_count_this_line = 1;
    ppu.bg_pipeline_state.window_active_line_counter = 96;
    ppu.bg_pipeline_state.visible_pixels_output = 93;
    ppu.bg_pipeline_state.current_transfer_x = 101;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 24;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new(93, 96, 95));

    assert!(
        ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedVisiblePixel,
            false,
        ))
    );
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(ppu.bg_pipeline_state.window_active_line_counter, 96);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
}

#[test]
fn same_scanline_startnow_can_consume_a_pending_previsible_retarget_even_before_its_trigger_dot() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.visible_registers.wx = 7;
    ppu.pipeline_registers.wx = 7;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = false;
    ppu.bg_pipeline_state.window_start_count_this_line = 1;
    ppu.bg_pipeline_state.window_active_line_counter = 6;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 24;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_live_wx_trigger_glitch = Some(DmgPendingLiveWxTriggerGlitch::new(7));
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new(3, 5, 2));

    assert!(
        ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedVisiblePixel,
            false,
        ))
    );
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(ppu.bg_pipeline_state.window_active_line_counter, 5);
    assert_eq!(ppu.bg_pipeline_state.window_start_count_this_line, 1);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
}

#[test]
fn same_scanline_late_wx_write_cancels_the_next_pending_previsible_start() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xF3,
        stat: 0x83,
        scy: 0,
        scx: 0,
        ly: 0,
        lyc: 0,
        bgp: 0xE4,
        wy: 0,
        wx: 0x63,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = MODE2_DOTS + 108;
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.visible_registers.wx = 0x50;
    ppu.pipeline_registers.wx = 0x63;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = false;
    ppu.bg_pipeline_state.visible_pixels_output = 91;
    ppu.bg_pipeline_state.current_transfer_x = 100;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new(92, 95, 95));

    ppu.maybe_arm_dmg_previsible_wx_retarget(0x63, 0x50);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
    assert!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_uses_visible_wx_once
    );
    assert!(
        !ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedVisiblePixel,
            false,
        ))
    );
    assert!(!ppu.bg_pipeline_state.window_started_this_line);
    assert!(
        !ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_uses_visible_wx_once
    );
}

#[test]
fn same_scanline_previsible_wx_retarget_invalid_wx_clears_pending_gap_artifacts() {
    let mut ppu = arm_previsible_retarget_fixture(4, MODE2_DOTS + 12, 0);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_onset_glitch = Some(5);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_carry = Some(DmgPendingPrevisibleWxCarry::new(1, 2, 0, 3));

    ppu.maybe_arm_dmg_previsible_wx_retarget(4, 2);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn same_scanline_low_wx_previsible_retarget_can_cancel_the_hidden_window_before_x0() {
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    ppu.write_bg_tilemap_entry(0, 0, 0x42);
    ppu.write_bg_tile_row(0x42, 0, 0xFF, 0x99);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;
    ppu.bg_pipeline_state.fetcher.tile_index = 0x57;
    ppu.bg_pipeline_state.fetcher.tile_low = 0xFF;
    ppu.bg_pipeline_state.fetcher.tile_high = 0xFF;

    ppu.maybe_arm_dmg_previsible_wx_retarget(6, 4);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_cancel_only(6, 0))
    );

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Background
    );
    assert!(!ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(ppu.bg_pipeline_state.window_start_count_this_line, 0);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x42);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xFF);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x99);
}

#[test]
fn same_scanline_low_wx_cancel_only_retarget_can_arm_before_the_window_fetcher_is_visible() {
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;
    ppu.bg_pipeline_state.fetcher = make_window_fetcher_state(PpuBgFetcherStage::TileDataLow, 0, 0);
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;

    ppu.maybe_arm_dmg_previsible_wx_retarget(6, 4);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_cancel_only(6, 0))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn same_scanline_low_wx_previsible_retarget_does_not_restart_the_hidden_prefix_at_x0() {
    let mut ppu = arm_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;

    ppu.maybe_arm_dmg_previsible_wx_retarget(5, 4);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn same_scanline_low_wx_previsible_retarget_does_not_shift_the_hidden_prefix_later() {
    let mut ppu = arm_previsible_retarget_fixture(4, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;

    ppu.maybe_arm_dmg_previsible_wx_retarget(4, 5);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn same_scanline_low_wx_previsible_retarget_ignores_same_wx_writes() {
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;

    ppu.maybe_arm_dmg_previsible_wx_retarget(6, 6);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
    assert_eq!(ppu.bg_pipeline_state.window_start_count_this_line, 1);
}

#[test]
fn same_scanline_low_wx_previsible_retarget_to_x0_drops_the_hidden_prefix_offset() {
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;

    ppu.maybe_arm_dmg_previsible_wx_retarget(6, 7);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
            0, 6, 0
        ))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn same_scanline_low_wx_previsible_retarget_restores_background_fifo_before_a_later_trigger() {
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    ppu.write_bg_tilemap_entry(0, 0, 0x42);
    ppu.write_bg_tile_row(0x42, 0, 0xFF, 0x99);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;
    ppu.bg_pipeline_state.fetcher.tile_index = 0x57;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x7E;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x7E;

    ppu.maybe_arm_dmg_previsible_wx_retarget(6, 10);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
            3, 6, 0
        ))
    );

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Background
    );
    assert!(!ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(ppu.bg_pipeline_state.window_start_count_this_line, 1);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
            3, 6, 0
        ))
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x42);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xFF);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x99);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 8);
    assert!(ppu.bg_pipeline_state.fifo.cached_slots().all(|pixel| {
        pixel.is_some_and(|pixel| pixel.cached.source == PpuBgFetcherSource::Background)
    }));
    assert_eq!(
        ppu.bg_pipeline_state
            .fifo
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![3, 1, 1, 3, 3, 1, 1, 3]
    );
}

#[test]
fn same_scanline_previsible_wx_retarget_waits_until_the_window_fetcher_reaches_an_abortable_stage()
{
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
        3, 6, 0,
    ));
    ppu.bg_pipeline_state.fetcher = make_window_fetcher_state(PpuBgFetcherStage::TileDataLow, 1, 0);

    ppu.test_apply_dmg_previsible_wx_retarget(&VramBusView::new(BusMaster::Ppu, &mut vram));

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
            3, 6, 0
        ))
    );
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert!(ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn same_scanline_previsible_wx_retarget_ignores_non_window_fetcher_sources() {
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new_cancel_only(6, 0));
    ppu.bg_pipeline_state.fetcher = make_window_fetcher_state(PpuBgFetcherStage::TileIndex, 0, 0);
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;

    ppu.test_apply_dmg_previsible_wx_retarget(&VramBusView::new(BusMaster::Ppu, &mut vram));

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_cancel_only(6, 0))
    );
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Background
    );
    assert!(ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn same_scanline_previsible_wx_retarget_without_a_hidden_skip_clears_gap_artifacts() {
    let mut ppu = arm_previsible_retarget_fixture(7, MODE2_DOTS + 12, 6);
    ppu.bg_pipeline_state.window_active_line_counter = 6;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_onset_glitch = Some(5);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_carry = Some(DmgPendingPrevisibleWxCarry::new(1, 2, 0, 3));

    ppu.maybe_arm_dmg_previsible_wx_retarget(7, 9);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new(2, 6, 2))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn cgb_dmg_software_low_wx_cancel_only_preserves_the_line_start_count() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut ppu = cgb_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6, operating_mode);
        let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
        vram.set_acquired(BusMaster::Ppu, true);
        ppu.bg_pipeline_state.window_start_count_this_line = 1;
        ppu.bg_pipeline_state.fetcher.tile_index = 0x57;
        ppu.bg_pipeline_state.fetcher.tile_low = 0xFF;
        ppu.bg_pipeline_state.fetcher.tile_high = 0xFF;

        ppu.maybe_arm_dmg_previsible_wx_retarget(6, 4);

        assert_eq!(
            ppu.bg_pipeline_state
                .dmg_window_restart
                .previsible_wx_retarget,
            Some(DmgPrevisibleWxRetarget::new_cancel_only(6, 0)),
            "{operating_mode:?}"
        );

        ppu.test_apply_dmg_previsible_wx_retarget(&VramBusView::new(BusMaster::Ppu, &mut vram));

        assert_eq!(
            ppu.bg_pipeline_state.fetcher.source,
            PpuBgFetcherSource::Background,
            "{operating_mode:?}"
        );
        assert!(!ppu.bg_pipeline_state.window_started_this_line);
        assert_eq!(
            ppu.bg_pipeline_state.window_start_count_this_line, 1,
            "{operating_mode:?}"
        );
    }
}

#[test]
fn cgb_dmg_software_suppresses_non_cancel_low_wx_dmg_retarget_plans() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut ppu = cgb_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6, operating_mode);

        ppu.maybe_arm_dmg_previsible_wx_retarget(5, 9);

        assert_eq!(
            ppu.bg_pipeline_state
                .dmg_window_restart
                .previsible_wx_retarget,
            None,
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .dmg_window_restart
                .pending_previsible_wx_onset_glitch,
            None,
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .dmg_window_restart
                .pending_previsible_wx_carry,
            None,
            "{operating_mode:?}"
        );
    }
}

#[test]
fn native_cgb_does_not_use_dmg_software_low_wx_retarget_or_phase_repaint_paths() {
    let mut ppu = cgb_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6, OperatingMode::Cgb);

    ppu.maybe_arm_dmg_previsible_wx_retarget(5, 8);
    ppu.maybe_arm_dmg_previsible_wx_retarget(4, 7);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint,
        None
    );
}

#[test]
fn cgb_dmg_software_wx4_phase_repaint_arms_for_the_observed_tile_phases() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut wx5 = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 14, 6, operating_mode);
        wx5.maybe_arm_dmg_previsible_wx_retarget(4, 5);
        let wx5_repaint = wx5
            .bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint
            .expect("WX=4->5 fixed prefix repaint should arm");
        assert_eq!(wx5_repaint.cancel_guard_x, 0, "{operating_mode:?}");
        assert_eq!(wx5_repaint.start_x, 0, "{operating_mode:?}");
        assert_eq!(wx5_repaint.end_x, 16, "{operating_mode:?}");
        assert_eq!(
            wx5_repaint.mode,
            CgbPrevisibleWxPhaseRepaintMode::FixedPattern,
            "{operating_mode:?}"
        );
        assert_eq!(
            wx5_repaint.pixels,
            [3, 3, 3, 3, 3, 0, 3, 3, 1, 1, 3, 3, 0, 3, 3, 1],
            "{operating_mode:?}"
        );

        let mut phase0 = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 14, 6, operating_mode);
        phase0.maybe_arm_dmg_previsible_wx_retarget(4, 7);
        let phase0_repaint = phase0
            .bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint
            .expect("WX=4 phase-0 plane repaint should arm");
        assert_eq!(phase0_repaint.cancel_guard_x, 0, "{operating_mode:?}");
        assert_eq!(phase0_repaint.start_x, 5, "{operating_mode:?}");
        assert_eq!(phase0_repaint.end_x, 13, "{operating_mode:?}");
        assert_eq!(
            phase0_repaint.mode,
            CgbPrevisibleWxPhaseRepaintMode::CurrentHighPlaneWithWindowHighPlaneAsLowPlane {
                source_start_x: 0,
                window_line_counter: 6,
            },
            "{operating_mode:?}"
        );

        let mut phase2 = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 14, 6, operating_mode);
        phase2.maybe_arm_dmg_previsible_wx_retarget(4, 9);
        let phase2_repaint = phase2
            .bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint
            .expect("WX=4 phase-2 low-to-high repaint should arm");
        assert_eq!(phase2_repaint.cancel_guard_x, 6, "{operating_mode:?}");
        assert_eq!(phase2_repaint.start_x, 6, "{operating_mode:?}");
        assert_eq!(phase2_repaint.end_x, 14, "{operating_mode:?}");
        assert_eq!(
            phase2_repaint.mode,
            CgbPrevisibleWxPhaseRepaintMode::CurrentLowPlaneIntoHighPlane,
            "{operating_mode:?}"
        );

        let mut phase6 = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 14, 6, operating_mode);
        phase6.maybe_arm_dmg_previsible_wx_retarget(4, 13);
        let phase6_repaint = phase6
            .bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint
            .expect("WX=4 phase-6 delayed-window repaint should arm");
        assert_eq!(phase6_repaint.cancel_guard_x, 6, "{operating_mode:?}");
        assert_eq!(phase6_repaint.start_x, 13, "{operating_mode:?}");
        assert_eq!(phase6_repaint.end_x, 21, "{operating_mode:?}");
        assert_eq!(
            phase6_repaint.mode,
            CgbPrevisibleWxPhaseRepaintMode::WindowPixels {
                source_start_x: 8,
                window_line_counter: 6,
            },
            "{operating_mode:?}"
        );
    }
}

#[test]
fn cgb_dmg_software_wx4_phase_repaint_applies_plane_sources() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut phase0 = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 14, 0, operating_mode);
        phase0.write_window_tilemap_entry(0, 0, 0x01);
        phase0.write_bg_tile_row(0x01, 0, 0x00, 0b1010_1100);
        phase0.current_scanline_bg_pixels[5..13].fill(2);
        phase0.current_scanline_mixed_pixels[5..13].fill(MixedPixel::background(2));
        phase0.visible_output = PpuVisibleOutputState::Driving;
        phase0.maybe_arm_dmg_previsible_wx_retarget(4, 7);
        phase0.bg_pipeline_state.visible_pixels_output = 13;
        let mut vram = crate::bus::VramDomain::from_bytes(&phase0.vram_bytes);
        vram.set_acquired(BusMaster::Ppu, true);

        phase0.test_apply_pending_cgb_previsible_wx_phase_repaint(&VramBusView::new(
            BusMaster::Ppu,
            &mut vram,
        ));

        assert_eq!(
            &phase0.current_scanline_bg_pixels[5..13],
            &[3, 2, 3, 2, 3, 3, 2, 2],
            "{operating_mode:?}"
        );

        let mut phase2 = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 14, 0, operating_mode);
        phase2.current_scanline_bg_pixels[6..14].copy_from_slice(&[0, 1, 2, 3, 1, 0, 3, 2]);
        phase2.current_scanline_mixed_pixels[6..14].fill(MixedPixel::background(0));
        phase2.visible_output = PpuVisibleOutputState::Driving;
        phase2.maybe_arm_dmg_previsible_wx_retarget(4, 9);
        phase2.bg_pipeline_state.visible_pixels_output = 14;
        let mut vram = crate::bus::VramDomain::from_bytes(&phase2.vram_bytes);
        vram.set_acquired(BusMaster::Ppu, true);

        phase2.test_apply_pending_cgb_previsible_wx_phase_repaint(&VramBusView::new(
            BusMaster::Ppu,
            &mut vram,
        ));

        assert_eq!(
            &phase2.current_scanline_bg_pixels[6..14],
            &[0, 3, 0, 3, 3, 0, 3, 0],
            "{operating_mode:?}"
        );

        let mut phase6 = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 14, 0, operating_mode);
        phase6.write_window_tilemap_entry(1, 0, 0x02);
        phase6.write_bg_tile_row(0x02, 0, 0b1100_0011, 0b1010_0101);
        phase6.visible_output = PpuVisibleOutputState::Driving;
        phase6.maybe_arm_dmg_previsible_wx_retarget(4, 13);
        phase6.bg_pipeline_state.visible_pixels_output = 21;
        let mut vram = crate::bus::VramDomain::from_bytes(&phase6.vram_bytes);
        vram.set_acquired(BusMaster::Ppu, true);

        phase6.test_apply_pending_cgb_previsible_wx_phase_repaint(&VramBusView::new(
            BusMaster::Ppu,
            &mut vram,
        ));

        assert_eq!(
            &phase6.current_scanline_bg_pixels[13..21],
            &[3, 1, 2, 0, 0, 2, 1, 3],
            "{operating_mode:?}"
        );
    }
}

#[test]
fn cgb_dmg_software_low_wx_phase_repaint_arms_for_the_wx5_and_wx6_tile_phases() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut wx5 = cgb_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6, operating_mode);
        wx5.maybe_arm_dmg_previsible_wx_retarget(5, 8);
        let wx5_repaint = wx5
            .bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint
            .expect("WX=5 phase repaint should arm");
        assert_eq!(wx5_repaint.cancel_guard_x, 1, "{operating_mode:?}");
        assert_eq!(wx5_repaint.start_x, 7, "{operating_mode:?}");
        assert_eq!(wx5_repaint.end_x, 15, "{operating_mode:?}");
        assert_eq!(
            &wx5_repaint.pixels[..8],
            &[0, 0, 3, 0, 3, 0, 3, 3],
            "{operating_mode:?}"
        );

        let mut wx6 = cgb_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6, operating_mode);
        wx6.maybe_arm_dmg_previsible_wx_retarget(6, 9);
        let wx6_repaint = wx6
            .bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint
            .expect("WX=6 phase repaint should arm");
        assert_eq!(wx6_repaint.cancel_guard_x, 8, "{operating_mode:?}");
        assert_eq!(wx6_repaint.start_x, 8, "{operating_mode:?}");
        assert_eq!(wx6_repaint.end_x, 16, "{operating_mode:?}");
        assert_eq!(
            &wx6_repaint.pixels[..8],
            &[3, 0, 0, 0, 0, 3, 3, 3],
            "{operating_mode:?}"
        );
    }
}

#[test]
fn cgb_dmg_software_phase_repaint_cancellation_uses_the_observed_wx_phase_guard() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut wx4_phase0 = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 14, 6, operating_mode);
        wx4_phase0.maybe_arm_dmg_previsible_wx_retarget(4, 95);
        wx4_phase0.bg_pipeline_state.visible_pixels_output = 91;
        wx4_phase0.maybe_arm_dmg_previsible_wx_retarget(95, 80);
        assert!(
            wx4_phase0
                .bg_pipeline_state
                .dmg_window_restart
                .pending_cgb_previsible_wx_phase_repaint
                .is_some(),
            "{operating_mode:?}"
        );

        let mut wx4_phase2 = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 14, 6, operating_mode);
        wx4_phase2.maybe_arm_dmg_previsible_wx_retarget(4, 97);
        wx4_phase2.bg_pipeline_state.visible_pixels_output = 91;
        wx4_phase2.maybe_arm_dmg_previsible_wx_retarget(97, 80);
        assert_eq!(
            wx4_phase2
                .bg_pipeline_state
                .dmg_window_restart
                .pending_cgb_previsible_wx_phase_repaint,
            None,
            "{operating_mode:?}"
        );

        let mut wx5 = cgb_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6, operating_mode);
        wx5.maybe_arm_dmg_previsible_wx_retarget(5, 96);
        wx5.bg_pipeline_state.visible_pixels_output = 91;
        wx5.maybe_arm_dmg_previsible_wx_retarget(96, 80);
        assert!(
            wx5.bg_pipeline_state
                .dmg_window_restart
                .pending_cgb_previsible_wx_phase_repaint
                .is_some(),
            "{operating_mode:?}"
        );

        let mut wx6 = cgb_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6, operating_mode);
        wx6.maybe_arm_dmg_previsible_wx_retarget(6, 97);
        wx6.bg_pipeline_state.visible_pixels_output = 91;
        wx6.maybe_arm_dmg_previsible_wx_retarget(97, 80);
        assert_eq!(
            wx6.bg_pipeline_state
                .dmg_window_restart
                .pending_cgb_previsible_wx_phase_repaint,
            None,
            "{operating_mode:?}"
        );
    }
}
