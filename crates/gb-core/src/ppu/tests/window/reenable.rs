use super::*;

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
fn wx0_window_disable_prefix_override_can_repaint_the_full_wx1_prefix_span() {
    let mut ppu = PpuTestRig::dmg();

    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.current_scanline_bg_pixels[0] = 2;
    ppu.current_scanline_mixed_pixels[0] = MixedPixel::background(2);
    for visible_x in 1..10 {
        ppu.current_scanline_mixed_pixels[visible_x] = MixedPixel::background(0);
    }
    ppu.bg_pipeline_state.dmg_wx0_window_disable_prefix_state =
        Some(DmgWx0WindowDisablePrefixState::new(10));

    ppu.test_apply_dmg_wx0_window_disable_prefix_override(0, 2);
    for visible_x in 1..10 {
        ppu.test_apply_dmg_wx0_window_disable_prefix_override(visible_x, 1);
    }

    assert_eq!(&ppu.framebuffer[..10], &[2; 10]);
    assert_eq!(&ppu.current_scanline_bg_pixels[..10], &[2; 10]);
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
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume,
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
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_window_reenable_resume = Some(DmgPendingWindowReenableResume::new(
        37,
        21,
        10,
        PpuBgFetcherStage::TileDataHigh,
        1,
    ));

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(
        ppu.bg_pipeline_state.dmg_late_window_enable_override,
        Some(DmgLateWindowEnableOverride::new(37, 45, 21))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume,
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
fn cgb_dmg_software_late_lcdc5_enable_arms_the_hardware_observed_initial_segment() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut ppu = cgb_previsible_retarget_fixture(18, MODE2_DOTS + 32, 0, operating_mode);
        ppu.visible_registers.lcdc = CGB_WINDOW_TEST_LCDC;
        ppu.pipeline_registers.lcdc = CGB_WINDOW_DISABLED_LCDC;
        ppu.visible_registers.wx = 18;
        ppu.pipeline_registers.wx = 18;
        ppu.bg_pipeline_state.window_started_this_line = false;
        ppu.bg_pipeline_state.visible_pixels_output = 13;

        assert!(
            !ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()),
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state.dmg_late_window_enable_override,
            Some(DmgLateWindowEnableOverride::new(11, 35, 11)),
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state.window_start_count_this_line, 0,
            "{operating_mode:?}"
        );
    }
}

#[test]
fn cgb_dmg_software_lcdc5_reenable_resume_repaints_without_counting_a_restart() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut ppu = cgb_previsible_retarget_fixture(28, MODE2_DOTS + 52, 20, operating_mode);
        ppu.visible_registers.lcdc = CGB_WINDOW_TEST_LCDC;
        ppu.pipeline_registers.lcdc = CGB_WINDOW_DISABLED_LCDC;
        ppu.visible_registers.wx = 28;
        ppu.pipeline_registers.wx = 28;
        ppu.bg_pipeline_state.window_started_this_line = true;
        ppu.bg_pipeline_state.window_start_count_this_line = 1;
        ppu.bg_pipeline_state.visible_pixels_output = 34;
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume = Some(DmgPendingWindowReenableResume::new(
            29,
            21,
            8,
            PpuBgFetcherStage::TileDataHigh,
            1,
        ));

        assert!(
            !ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()),
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state.dmg_late_window_enable_override,
            Some(DmgLateWindowEnableOverride::new(29, 37, 21)),
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .dmg_window_restart
                .pending_window_reenable_resume,
            None,
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state.window_start_count_this_line, 1,
            "{operating_mode:?}"
        );
    }
}

#[test]
fn cgb_dmg_software_lcdc5_low_wx_reenable_arms_fixed_panel_repaint() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut ppu = cgb_previsible_retarget_fixture(4, MODE2_DOTS + 32, 0, operating_mode);
        ppu.visible_registers.lcdc = CGB_WINDOW_TEST_LCDC;
        ppu.pipeline_registers.lcdc = CGB_WINDOW_DISABLED_LCDC;
        ppu.visible_registers.wx = 4;
        ppu.pipeline_registers.wx = 4;
        ppu.bg_pipeline_state.visible_pixels_output = 8;

        assert!(
            !ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()),
            "{operating_mode:?}"
        );
        let repaint = ppu
            .bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint
            .expect("low-WX LCDC.5 repaint should arm");
        assert_eq!(repaint.start_x, 5, "{operating_mode:?}");
        assert_eq!(repaint.end_x, 15, "{operating_mode:?}");
        assert_eq!(repaint.pattern_len, 10, "{operating_mode:?}");
        assert_eq!(
            &repaint.pixels[..10],
            &[0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            "{operating_mode:?}"
        );
    }
}

#[test]
fn cgb_dmg_software_lcdc5_fixed_panel_repaint_table_covers_observed_wx_cases() {
    let cases = [
        (
            2,
            8,
            0,
            11,
            11,
            [3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0],
        ),
        (
            5,
            8,
            0,
            6,
            6,
            [1, 1, 1, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            6,
            8,
            0,
            15,
            15,
            [1, 1, 1, 1, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            7,
            8,
            7,
            8,
            1,
            [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            8,
            8,
            8,
            17,
            9,
            [3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            9,
            8,
            2,
            7,
            5,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            32,
            34,
            33,
            41,
            8,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            33,
            34,
            26,
            34,
            8,
            [1, 1, 1, 1, 1, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            34,
            34,
            27,
            43,
            16,
            [1, 1, 1, 1, 1, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            36,
            34,
            29,
            45,
            16,
            [1, 1, 1, 1, 1, 1, 1, 3, 3, 3, 3, 3, 3, 3, 3, 3],
        ),
    ];

    for (wx, visible_output, start_x, end_x, pattern_len, pixels) in cases {
        let mut ppu =
            cgb_previsible_retarget_fixture(wx, MODE2_DOTS + 52, 0, OperatingMode::GbCompatible);
        ppu.visible_registers.lcdc = CGB_WINDOW_TEST_LCDC;
        ppu.pipeline_registers.lcdc = CGB_WINDOW_DISABLED_LCDC;
        ppu.visible_registers.wx = wx;
        ppu.pipeline_registers.wx = wx;
        ppu.bg_pipeline_state.visible_pixels_output = visible_output;

        assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));

        let repaint = ppu
            .bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint
            .unwrap_or_else(|| panic!("WX={wx} fixed panel repaint should arm"));
        assert_eq!(repaint.start_x, start_x, "WX={wx}");
        assert_eq!(repaint.end_x, end_x, "WX={wx}");
        assert_eq!(repaint.pattern_len, pattern_len, "WX={wx}");
        assert_eq!(repaint.pixels, pixels, "WX={wx}");
    }
}

#[test]
fn cgb_dmg_software_lcdc5_second_enable_can_replace_resume_with_fixed_panel_repaint() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut ppu = cgb_previsible_retarget_fixture(35, MODE2_DOTS + 52, 0, operating_mode);
        ppu.visible_registers.lcdc = CGB_WINDOW_TEST_LCDC;
        ppu.pipeline_registers.lcdc = CGB_WINDOW_DISABLED_LCDC;
        ppu.visible_registers.wx = 35;
        ppu.pipeline_registers.wx = 35;
        ppu.bg_pipeline_state.window_start_count_this_line = 1;
        ppu.bg_pipeline_state.visible_pixels_output = 34;
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume = Some(DmgPendingWindowReenableResume::new(
            28,
            28,
            0,
            PpuBgFetcherStage::TileDataHigh,
            1,
        ));

        assert!(
            !ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()),
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .dmg_window_restart
                .pending_window_reenable_resume,
            None,
            "{operating_mode:?}"
        );
        let repaint = ppu
            .bg_pipeline_state
            .dmg_window_restart
            .pending_cgb_previsible_wx_phase_repaint
            .expect("second-enable LCDC.5 repaint should arm");
        assert_eq!(repaint.start_x, 28, "{operating_mode:?}");
        assert_eq!(repaint.end_x, 36, "{operating_mode:?}");
        assert_eq!(
            &repaint.pixels[..8],
            &[1, 1, 1, 1, 1, 1, 1, 3],
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state.window_start_count_this_line, 1,
            "{operating_mode:?}"
        );
    }
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
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_window_reenable_resume = Some(DmgPendingWindowReenableResume::new(
        37,
        21,
        10,
        PpuBgFetcherStage::TileDataHigh,
        1,
    ));
    ppu.bg_pipeline_state.dmg_late_window_enable_override =
        Some(DmgLateWindowEnableOverride::new(37, 45, 21));

    let transfer_dot = ppu.advance_mode3_output_phase();

    assert!(ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume,
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
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume,
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
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_window_reenable_resume = Some(DmgPendingWindowReenableResume::new(
        8,
        0,
        8,
        PpuBgFetcherStage::TileIndex,
        0,
    ));

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
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume,
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
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_window_reenable_resume = Some(DmgPendingWindowReenableResume::new(
        SCREEN_WIDTH as u8,
        21,
        8,
        PpuBgFetcherStage::TileIndex,
        0,
    ));

    assert!(!ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::not_served()));
    assert_eq!(ppu.bg_pipeline_state.dmg_late_window_enable_override, None);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_window_reenable_resume,
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
