use super::*;

#[test]
fn retained_same_scanline_previsible_wx_retarget_skips_a_single_leftover_window_pixel() {
    let mut ppu = dmg_window_startup(0x50);
    ppu.bg_pipeline_state.window_started_this_line = false;
    ppu.bg_pipeline_state.window_start_count_this_line = 1;
    ppu.bg_pipeline_state.window_active_line_counter = 96;
    ppu.bg_pipeline_state.visible_pixels_output = 93;
    ppu.bg_pipeline_state.current_transfer_x = 101;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 24;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new(93, 96, 95));
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retained_trigger_glitch_x = Some(93);

    assert!(
        ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedVisiblePixel,
            false,
        ))
    );
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(ppu.bg_pipeline_state.window_active_line_counter, 96);
    assert_eq!(ppu.bg_pipeline_state.fetcher.window_tilemap_x, 12);
    assert_eq!(
        ppu.bg_pipeline_state
            .fetcher
            .first_window_tile_leading_pixel_skip,
        0
    );
}

#[test]
fn retained_same_scanline_previsible_wx_retarget_keeps_nonterminal_partial_window_offsets() {
    let mut ppu = dmg_window_startup(0x50);
    ppu.bg_pipeline_state.window_started_this_line = false;
    ppu.bg_pipeline_state.window_start_count_this_line = 1;
    ppu.bg_pipeline_state.window_active_line_counter = 96;
    ppu.bg_pipeline_state.visible_pixels_output = 93;
    ppu.bg_pipeline_state.current_transfer_x = 101;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 24;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new(93, 96, 94));
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retained_trigger_glitch_x = Some(93);

    assert!(
        ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedVisiblePixel,
            false,
        ))
    );
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(ppu.bg_pipeline_state.window_active_line_counter, 96);
    assert_eq!(ppu.bg_pipeline_state.fetcher.window_tilemap_x, 11);
    assert_eq!(
        ppu.bg_pipeline_state
            .fetcher
            .first_window_tile_leading_pixel_skip,
        6
    );
}

#[test]
fn same_scanline_late_wx_write_does_not_cancel_one_hidden_prefix_resume_restarts() {
    let mut ppu = dmg_window_startup(0x63);
    ppu.line_dot = MODE2_DOTS + 108;
    ppu.visible_registers.wx = 0x50;
    ppu.pipeline_registers.wx = 0x63;
    ppu.bg_pipeline_state.window_started_this_line = false;
    ppu.bg_pipeline_state.visible_pixels_output = 91;
    ppu.bg_pipeline_state.current_transfer_x = 100;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
        92, 96, 8,
    ));

    ppu.maybe_arm_dmg_previsible_wx_retarget(0x63, 0x50);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
            92, 96, 8
        ))
    );
    assert!(
        !ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_uses_visible_wx_once
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_background_override_onset_x,
        None
    );
}

#[test]
fn same_scanline_late_wx_write_cancels_distant_one_hidden_prefix_resume_restarts() {
    let mut ppu = dmg_window_startup(0x66);
    ppu.line_dot = MODE2_DOTS + 108;
    ppu.visible_registers.wx = 0x50;
    ppu.pipeline_registers.wx = 0x66;
    ppu.bg_pipeline_state.window_started_this_line = false;
    ppu.bg_pipeline_state.visible_pixels_output = 91;
    ppu.bg_pipeline_state.current_transfer_x = 103;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
        95, 96, 8,
    ));

    ppu.maybe_arm_dmg_previsible_wx_retarget(0x66, 0x50);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
    );
    assert!(
        !ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_uses_visible_wx_once
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_background_override_onset_x,
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
fn same_scanline_previsible_wx_retarget_can_arm_a_retained_fifo_prefix_resume() {
    let mut ppu = arm_previsible_retarget_fixture(4, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;

    ppu.maybe_arm_dmg_previsible_wx_retarget(4, 7);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_retained_fifo_prefix_resume(
            0, 6, 0, true
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
fn same_scanline_previsible_wx_retarget_can_arm_a_retained_fifo_prefix_resume_without_advancing_tilemap()
 {
    let mut ppu = arm_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;

    ppu.maybe_arm_dmg_previsible_wx_retarget(5, 7);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_retained_fifo_prefix_resume(
            0, 6, 0, false
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
fn same_scanline_retained_fifo_prefix_resume_can_continue_from_the_next_tilemap() {
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
    ppu.bg_pipeline_state.fifo.push_back(3);
    ppu.bg_pipeline_state.fifo_cached_pixels.push_back(None);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new_retained_fifo_prefix_resume(
        0, 6, 0, true,
    ));

    assert!(
        ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedVisiblePixel,
            false,
        ))
    );
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(ppu.bg_pipeline_state.window_active_line_counter, 6);
    assert_eq!(ppu.bg_pipeline_state.window_start_count_this_line, 1);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.window_tilemap_x, 1);
    assert_eq!(
        ppu.bg_pipeline_state
            .fetcher
            .first_window_tile_leading_pixel_skip,
        0
    );
}

#[test]
fn same_scanline_retained_fifo_prefix_resume_can_preserve_fifo_with_a_nonzero_window_offset() {
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
    ppu.bg_pipeline_state.fifo.push_back(3);
    ppu.bg_pipeline_state.fifo_cached_pixels.push_back(None);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new_retained_fifo_prefix_resume(
        0, 6, 9, false,
    ));

    assert!(
        ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedVisiblePixel,
            false,
        ))
    );
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(ppu.bg_pipeline_state.window_active_line_counter, 6);
    assert_eq!(ppu.bg_pipeline_state.window_start_count_this_line, 1);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 1);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.window_tilemap_x, 1);
    assert_eq!(
        ppu.bg_pipeline_state
            .fetcher
            .first_window_tile_leading_pixel_skip,
        1
    );
}

#[test]
fn same_scanline_one_hidden_pixel_previsible_retarget_keeps_the_restart_at_window_origin() {
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;

    ppu.maybe_arm_dmg_previsible_wx_retarget(6, 8);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
            1, 6, 0
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
fn same_scanline_one_hidden_pixel_previsible_retarget_keeps_the_boundary_restart_at_window_origin()
{
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;

    ppu.maybe_arm_dmg_previsible_wx_retarget(6, 14);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
            7, 6, 0
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
fn same_scanline_one_hidden_pixel_previsible_retarget_resumes_on_the_second_tile_after_the_boundary()
 {
    let mut ppu = arm_previsible_retarget_fixture(6, MODE2_DOTS + 14, 6);
    ppu.bg_pipeline_state.window_start_count_this_line = 1;

    ppu.maybe_arm_dmg_previsible_wx_retarget(6, 15);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
            8, 6, 8
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
