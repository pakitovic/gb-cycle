use super::*;

mod low_wx;
mod reenable;
mod retained_prefix;
mod trigger_glitch;

const DMG_WINDOW_TEST_LCDC: u8 = 0xF3;
const DMG_WINDOW_TEST_STAT: u8 = 0x83;
const DMG_WINDOW_TEST_BGP: u8 = 0xE4;
const CGB_WINDOW_TEST_LCDC: u8 = LCDC_ENABLE_BIT
    | LCDC_WINDOW_ENABLE_BIT
    | LCDC_WINDOW_TILE_MAP_BIT
    | LCDC_BG_WINDOW_TILE_DATA_BIT
    | LCDC_BG_ENABLE_BIT;
const CGB_WINDOW_DISABLED_LCDC: u8 =
    LCDC_ENABLE_BIT | LCDC_WINDOW_TILE_MAP_BIT | LCDC_BG_WINDOW_TILE_DATA_BIT | LCDC_BG_ENABLE_BIT;

fn dmg_window_startup(wx: u8) -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: DMG_WINDOW_TEST_LCDC,
        stat: DMG_WINDOW_TEST_STAT,
        scy: 0,
        scx: 0,
        ly: 0,
        lyc: 0,
        bgp: DMG_WINDOW_TEST_BGP,
        wy: 0,
        wx,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_registers.lcdc = DMG_WINDOW_TEST_LCDC;
    ppu.pipeline_registers.lcdc = DMG_WINDOW_TEST_LCDC;
    ppu.visible_registers.bgp = DMG_WINDOW_TEST_BGP;
    ppu.pipeline_registers.bgp = DMG_WINDOW_TEST_BGP;
    ppu.visible_registers.wx = wx;
    ppu.pipeline_registers.wx = wx;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu
}

fn make_window_fetcher_state(
    stage: PpuBgFetcherStage,
    stage_dot: u8,
    fetch_x: u16,
) -> BgFetcherState {
    BgFetcherState {
        source: PpuBgFetcherSource::Window,
        stage,
        stage_dot,
        fetch_x,
        next_fetch_pixel: fetch_x,
        bg_resume_fetch_pixel: fetch_x,
        ..BgFetcherState::default()
    }
}

fn arm_previsible_retarget_fixture(wx: u8, line_dot: u16, active_line_counter: u8) -> PpuTestRig {
    let mut ppu = dmg_window_startup(wx);
    ppu.line_dot = line_dot;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_active_line_counter = active_line_counter;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.fetcher = make_window_fetcher_state(PpuBgFetcherStage::TileIndex, 0, 0);
    ppu
}

fn cgb_previsible_retarget_fixture(
    wx: u8,
    line_dot: u16,
    active_line_counter: u8,
    operating_mode: OperatingMode,
) -> PpuTestRig {
    let mut ppu = PpuTestRig::with_model(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(operating_mode);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: CGB_WINDOW_TEST_LCDC,
        stat: 0x82,
        scy: 0,
        scx: 0,
        ly: 0,
        lyc: 0,
        bgp: DMG_WINDOW_TEST_BGP,
        wy: 0,
        wx,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_registers.lcdc = CGB_WINDOW_TEST_LCDC;
    ppu.pipeline_registers.lcdc = CGB_WINDOW_TEST_LCDC;
    ppu.visible_registers.bgp = DMG_WINDOW_TEST_BGP;
    ppu.pipeline_registers.bgp = DMG_WINDOW_TEST_BGP;
    ppu.visible_registers.wx = wx;
    ppu.pipeline_registers.wx = wx;
    ppu.line_dot = line_dot;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_active_line_counter = active_line_counter;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.fetcher = make_window_fetcher_state(PpuBgFetcherStage::TileIndex, 0, 0);
    ppu
}

fn cgb_window_activation_startup(lcdc: u8) -> PpuTestRig {
    let mut ppu = PpuTestRig::with_model(ConsoleModel::GameBoyColor);
    ppu.write_bg_tile_row(0, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(1, 0, 0xFF, 0xFF);
    ppu.write_bg_tilemap_entry(0, 0, 0);
    ppu.write_window_tilemap_entry(0, 0, 1);
    ppu.apply_startup_state(PpuStartupState {
        lcdc,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: DMG_WINDOW_TEST_BGP,
        wy: 0x00,
        wx: 0x07,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu
}

#[test]
fn cgb_mode2_lcdc5_disable_does_not_cancel_current_line_window_activation() {
    let mut ppu = cgb_window_activation_startup(CGB_WINDOW_TEST_LCDC);

    ppu.tick();
    assert_eq!(ppu.snapshot().line_dot, 1);
    ppu.write_register(0xFF40, CGB_WINDOW_DISABLED_LCDC);

    ppu.advance_until_hblank();
    let snapshot = ppu.snapshot();
    assert!(snapshot.window_started_this_line);
    assert_eq!(&snapshot.current_scanline_pixels[..8], &[3; 8]);
}

#[test]
fn cgb_mode2_lcdc5_enable_waits_until_the_next_line_window_activation() {
    let mut ppu = cgb_window_activation_startup(CGB_WINDOW_DISABLED_LCDC);

    ppu.tick();
    assert_eq!(ppu.snapshot().line_dot, 1);
    ppu.write_register(0xFF40, CGB_WINDOW_TEST_LCDC);

    ppu.advance_until_hblank();
    let first_line = ppu.snapshot();
    assert!(!first_line.window_started_this_line);
    assert_eq!(&first_line.current_scanline_pixels[..8], &[0; 8]);

    ppu.advance_until_line_start(1);
    ppu.advance_until_hblank();
    let second_line = ppu.snapshot();
    assert!(second_line.window_started_this_line);
    assert_eq!(&second_line.current_scanline_pixels[..8], &[3; 8]);
}

#[test]
fn cgb_mode3_lcdc5_writes_update_the_current_line_window_latch() {
    let mut ppu = cgb_window_activation_startup(CGB_WINDOW_DISABLED_LCDC);

    while ppu.snapshot().mode != PpuAccessMode::Drawing {
        ppu.tick();
    }
    assert!(!ppu.bg_pipeline_state.window_lcdc5_latch);

    ppu.write_register(0xFF40, CGB_WINDOW_TEST_LCDC);
    assert!(ppu.bg_pipeline_state.window_lcdc5_latch);

    ppu.write_register(0xFF40, CGB_WINDOW_DISABLED_LCDC);
    assert!(!ppu.bg_pipeline_state.window_lcdc5_latch);
}

#[test]
fn cgb_dmg_software_same_line_startnow_suppresses_wx_retrigger_but_allows_lcdc5_reenable() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut wx_retrigger =
            cgb_previsible_retarget_fixture(15, MODE2_DOTS + 80, 6, operating_mode);
        wx_retrigger.bg_pipeline_state.visible_pixels_output = 8;
        wx_retrigger.bg_pipeline_state.current_transfer_x = 16;
        wx_retrigger.bg_pipeline_state.window_start_count_this_line = 1;
        wx_retrigger.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
        wx_retrigger.bg_pipeline_state.window_lcdc5_latch = true;

        assert!(
            !wx_retrigger.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
                Mode3TransferDotKind::ServedVisiblePixel,
                false,
            )),
            "{operating_mode:?}"
        );
        assert_eq!(
            wx_retrigger.bg_pipeline_state.window_start_count_this_line, 1,
            "{operating_mode:?}"
        );

        let mut lcdc5_reenable =
            cgb_previsible_retarget_fixture(15, MODE2_DOTS + 80, 6, operating_mode);
        lcdc5_reenable.bg_pipeline_state.visible_pixels_output = 8;
        lcdc5_reenable.bg_pipeline_state.current_transfer_x = 16;
        lcdc5_reenable
            .bg_pipeline_state
            .window_start_count_this_line = 1;
        lcdc5_reenable
            .bg_pipeline_state
            .fetcher
            .abort_window_to_background();
        lcdc5_reenable.bg_pipeline_state.window_lcdc5_latch = true;

        assert!(
            lcdc5_reenable.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
                Mode3TransferDotKind::ServedVisiblePixel,
                false,
            )),
            "{operating_mode:?}"
        );
        assert_eq!(
            lcdc5_reenable.bg_pipeline_state.fetcher.source,
            PpuBgFetcherSource::Window,
            "{operating_mode:?}"
        );
        assert_eq!(
            lcdc5_reenable.bg_pipeline_state.window_active_line_counter, 1,
            "{operating_mode:?}"
        );
        assert_eq!(
            lcdc5_reenable
                .bg_pipeline_state
                .window_start_count_this_line,
            2,
            "{operating_mode:?}"
        );
    }
}

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

    assert_eq!(
        transfer_dot.kind,
        Mode3TransferDotKind::ServedHiddenTransfer
    );
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
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
    ppu.bg_pipeline_state.push_dummy_fifo_pixels(1);
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
fn wx_four_starts_window_from_the_hidden_transfer_dot_that_matches_the_raw_coordinate() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 4;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 4;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 4;
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
fn wx_two_starts_window_from_the_previsible_transfer_dot_that_matches_the_low_wx_trigger() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 2;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 1;
    ppu.bg_pipeline_state.current_transfer_x = 3;

    let transfer_dot =
        Mode3TransferDot::served(Mode3TransferDotKind::ServedPreVisibleTransfer, false);

    assert!(ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
}

#[test]
fn low_wx_previsible_window_start_uses_the_current_visible_wx_write_before_visible_output() {
    let mut ppu = PpuTestRig::dmg();
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 2;
    ppu.pipeline_registers.lcdc = 0xF1;
    ppu.pipeline_registers.wx = 1;
    ppu.ly = 0;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.current_transfer_x = 2;

    assert!(
        !ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedPreVisibleTransfer,
            false,
        ))
    );
    assert!(!ppu.bg_pipeline_state.window_started_this_line);

    ppu.bg_pipeline_state.current_transfer_x = 3;

    assert!(
        ppu.maybe_start_window_after_transfer_dot(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedPreVisibleTransfer,
            false,
        ))
    );
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
