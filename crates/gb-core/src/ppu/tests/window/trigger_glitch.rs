use super::*;

#[test]
fn same_scanline_late_wx_write_keeps_a_previsible_restart_armed_until_the_trigger_dot() {
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
        wx: 0x64,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = MODE2_DOTS + 108;
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.visible_registers.wx = 0x50;
    ppu.pipeline_registers.wx = 0x64;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = false;
    ppu.bg_pipeline_state.visible_pixels_output = 91;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new(93, 96, 95));
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_carry = Some(DmgPendingPrevisibleWxCarry::new(92, 93, 96, 95));

    ppu.maybe_arm_dmg_previsible_wx_retarget(0x64, 0x50);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        Some(DmgPrevisibleWxRetarget::new(93, 96, 95))
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        Some(DmgPendingPrevisibleWxCarry::new(92, 93, 96, 95))
    );
    assert!(
        !ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_uses_visible_wx_once
    );
}

#[test]
fn previsible_wx_cancel_background_override_forces_white_fifo_output_at_its_onset() {
    let mut ppu = PpuTestRig::dmg();
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; TEST_VRAM_BYTES]);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.bgp = 0x1B;
    ppu.pipeline_registers.bgp = 0x1B;
    ppu.bg_pipeline_state.visible_pixels_output = 3;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_cancel_background_override_onset_x = Some(3);
    ppu.bg_pipeline_state.fifo.push_back(2);

    assert_eq!(
        ppu.pop_visible_bg_fifo_pixel(&VramBusView::new(BusMaster::Ppu, &mut vram))
            .map(|pixel| pixel.color),
        Some(3)
    );
    assert_eq!(ppu.current_scanline_bg_dot_contexts[3], None);
}

#[test]
fn previsible_wx_carry_noops_cleanly_when_no_carry_is_pending() {
    let mut ppu = PpuTestRig::dmg();
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.test_apply_pending_dmg_previsible_wx_carry(
        Mode3TransferDot::served(Mode3TransferDotKind::ServedVisiblePixel, false),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert!(ppu.bg_pipeline_state.fifo.is_empty());
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn previsible_wx_carry_ignores_non_visible_transfer_dots() {
    let mut ppu = PpuTestRig::dmg();
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_carry = Some(DmgPendingPrevisibleWxCarry::new(4, 6, 0, 0));
    ppu.bg_pipeline_state.visible_pixels_output = 4;

    ppu.test_apply_pending_dmg_previsible_wx_carry(
        Mode3TransferDot::served(Mode3TransferDotKind::ServedHiddenTransfer, false),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        Some(DmgPendingPrevisibleWxCarry::new(4, 6, 0, 0))
    );
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn previsible_wx_carry_pushes_window_pixels_and_expires_at_the_end_of_the_span() {
    let mut ppu = PpuTestRig::dmg();
    ppu.write_window_tilemap_entry(0, 0, 0x01);
    ppu.write_bg_tile_row(0x01, 0, 0xFF, 0x00);
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_carry = Some(DmgPendingPrevisibleWxCarry::new(4, 6, 0, 0));
    ppu.bg_pipeline_state.visible_pixels_output = 4;

    ppu.test_apply_pending_dmg_previsible_wx_carry(
        Mode3TransferDot::served(Mode3TransferDotKind::ServedVisiblePixel, false),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 1);
    assert_eq!(ppu.bg_pipeline_state.fifo.cached_front(), Some(None));
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        Some(DmgPendingPrevisibleWxCarry::new(5, 6, 0, 1))
    );

    ppu.bg_pipeline_state.visible_pixels_output = 5;
    ppu.test_apply_pending_dmg_previsible_wx_carry(
        Mode3TransferDot::served(Mode3TransferDotKind::ServedVisiblePixel, false),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn previsible_wx_carry_expires_once_visible_output_has_passed_the_trigger() {
    let mut ppu = PpuTestRig::dmg();
    let mut vram = crate::bus::VramDomain::from_bytes(&ppu.vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_carry = Some(DmgPendingPrevisibleWxCarry::new(4, 6, 0, 0));
    ppu.bg_pipeline_state.visible_pixels_output = 5;

    ppu.test_apply_pending_dmg_previsible_wx_carry(
        Mode3TransferDot::served(Mode3TransferDotKind::ServedVisiblePixel, false),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
    );

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry,
        None
    );
}

#[test]
fn previsible_wx_onset_glitch_repaint_waits_until_visible_output_has_passed_the_trigger() {
    let mut ppu = PpuTestRig::dmg();
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; TEST_VRAM_BYTES]);
    vram.set_acquired(BusMaster::Ppu, true);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.current_scanline_bg_pixels[3] = 1;
    ppu.current_scanline_mixed_pixels[3] = MixedPixel::background(1);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_onset_glitch = Some(3);
    ppu.bg_pipeline_state.visible_pixels_output = 3;

    ppu.test_apply_pending_dmg_previsible_wx_onset_glitch_repaint(&VramBusView::new(
        BusMaster::Ppu,
        &mut vram,
    ));
    assert_eq!(ppu.current_scanline_bg_pixels[3], 1);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        Some(3)
    );

    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.test_apply_pending_dmg_previsible_wx_onset_glitch_repaint(&VramBusView::new(
        BusMaster::Ppu,
        &mut vram,
    ));

    assert_eq!(ppu.current_scanline_bg_pixels[3], 0);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        None
    );
}

#[test]
fn previsible_wx_onset_glitch_repaint_can_reveal_a_behind_bg_object_pixel() {
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
        wx: 0,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.visible_registers.obp0 = Some(0xE4);
    ppu.pipeline_registers.obp0 = Some(0xE4);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.obj_pipeline_state.mode3_line_start_obj_height = 8;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 11,
        tile_index: 0,
        attributes: 0x80,
    });
    ppu.current_scanline_bg_pixels[3] = 3;
    ppu.current_scanline_mixed_pixels[3] = MixedPixel::background(3);
    ppu.current_scanline_pixels[3] = 3;
    ppu.framebuffer[3] = 3;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_onset_glitch = Some(3);
    ppu.bg_pipeline_state.visible_pixels_output = 4;

    let mut vram_bytes = [0; TEST_VRAM_BYTES];
    vram_bytes[1] = 0x80;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.test_apply_pending_dmg_previsible_wx_onset_glitch_repaint(&VramBusView::new(
        BusMaster::Ppu,
        &mut vram,
    ));

    assert_eq!(ppu.current_scanline_bg_pixels[3], 0);
    assert_eq!(
        ppu.current_scanline_mixed_pixels[3],
        MixedPixel::object(2, false)
    );
    assert_eq!(ppu.current_scanline_pixels[3], 2);
    assert_eq!(ppu.framebuffer[3], 2);
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch,
        None
    );
}

#[test]
fn previsible_wx_onset_glitch_repaint_updates_recent_panel_history_while_forced_blank() {
    let mut ppu = PpuTestRig::dmg();
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; TEST_VRAM_BYTES]);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.visible_output = PpuVisibleOutputState::ForcedBlank;
    ppu.current_scanline_bg_pixels[3] = 1;
    ppu.current_scanline_mixed_pixels[3] = MixedPixel::background(1);
    ppu.current_scanline_pixels[3] = 1;
    ppu.framebuffer[3] = 1;
    ppu.dmg_panel_live_write_state
        .recent_panel_dots
        .push_back(PpuRecentPanelDot {
            visible_x: 3,
            pixel: MixedPixel::background(1),
            dmg_bg_forced_white: true,
        });
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_onset_glitch = Some(3);
    ppu.bg_pipeline_state.visible_pixels_output = 4;

    ppu.test_apply_pending_dmg_previsible_wx_onset_glitch_repaint(&VramBusView::new(
        BusMaster::Ppu,
        &mut vram,
    ));

    assert_eq!(ppu.current_scanline_bg_pixels[3], 0);
    assert_eq!(
        ppu.current_scanline_mixed_pixels[3],
        MixedPixel::background(0)
    );
    assert_eq!(ppu.current_scanline_pixels[3], 0);
    assert_eq!(ppu.framebuffer[3], 0);
    assert_eq!(
        ppu.dmg_panel_live_write_state.recent_panel_dots[0],
        PpuRecentPanelDot {
            visible_x: 3,
            pixel: MixedPixel::background(0),
            dmg_bg_forced_white: false,
        }
    );
}

#[test]
fn previsible_wx_onset_glitch_repaint_uses_current_obj_height_and_keeps_front_priority() {
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
        wx: 0,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.visible_registers.obp0 = Some(0xE4);
    ppu.pipeline_registers.obp0 = Some(0xE4);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.obj_pipeline_state.mode3_line_start_obj_height = 0;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 40,
        x: 11,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 40,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 2,
        y: 16,
        x: 11,
        tile_index: 0,
        attributes: 0x20,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 3,
        y: 16,
        x: 11,
        tile_index: 1,
        attributes: 0x20,
    });
    ppu.current_scanline_bg_pixels[3] = 3;
    ppu.current_scanline_mixed_pixels[3] = MixedPixel::background(3);
    ppu.current_scanline_pixels[3] = 3;
    ppu.framebuffer[3] = 3;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_onset_glitch = Some(3);
    ppu.bg_pipeline_state.visible_pixels_output = 4;

    let mut vram_bytes = [0; TEST_VRAM_BYTES];
    vram_bytes[1] = 0x01;
    vram_bytes[16] = 0x01;
    vram_bytes[17] = 0x01;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.test_apply_pending_dmg_previsible_wx_onset_glitch_repaint(&VramBusView::new(
        BusMaster::Ppu,
        &mut vram,
    ));

    assert_eq!(ppu.current_scanline_bg_pixels[3], 0);
    assert_eq!(
        ppu.current_scanline_mixed_pixels[3],
        MixedPixel::object(2, false)
    );
    assert_eq!(ppu.current_scanline_pixels[3], 2);
    assert_eq!(ppu.framebuffer[3], 2);
}

#[test]
fn previsible_wx_retarget_expiry_clears_all_companion_state_once_the_trigger_is_past() {
    let mut ppu = PpuTestRig::dmg();
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new(3, 0, 0));
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_cancel_background_override_onset_x = Some(3);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retained_trigger_glitch_x = Some(3);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_onset_glitch = Some(3);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_carry = Some(DmgPendingPrevisibleWxCarry::new(3, 4, 0, 0));
    ppu.bg_pipeline_state.visible_pixels_output = 4;

    ppu.test_expire_dmg_previsible_wx_retarget();

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
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
            .previsible_wx_retained_trigger_glitch_x,
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
fn cancel_only_previsible_wx_retarget_expires_once_visible_output_moves_past_x0() {
    let mut ppu = PpuTestRig::dmg();
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retarget = Some(DmgPrevisibleWxRetarget::new_cancel_only(0, 0));
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_cancel_background_override_onset_x = Some(0);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .previsible_wx_retained_trigger_glitch_x = Some(0);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_onset_glitch = Some(0);
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_previsible_wx_carry = Some(DmgPendingPrevisibleWxCarry::new(0, 1, 0, 0));
    ppu.bg_pipeline_state.visible_pixels_output = 1;

    ppu.test_expire_dmg_previsible_wx_retarget();

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget,
        None
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
            .previsible_wx_retained_trigger_glitch_x,
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
fn same_scanline_live_wx_write_after_visible_output_waits_until_the_new_trigger() {
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
        wx: 7,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = MODE2_DOTS + 12;
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state.fifo.push_back(3);

    ppu.maybe_arm_dmg_live_wx_trigger_glitch(10);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        Some(DmgPendingLiveWxTriggerGlitch::new(3))
    );

    ppu.bg_pipeline_state.visible_pixels_output = 2;
    ppu.maybe_apply_pending_dmg_live_wx_trigger_glitch(Mode3TransferDot::served(
        Mode3TransferDotKind::ServedVisiblePixel,
        false,
    ));
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 1);

    ppu.bg_pipeline_state.visible_pixels_output = 3;
    ppu.maybe_apply_pending_dmg_live_wx_trigger_glitch(Mode3TransferDot::served(
        Mode3TransferDotKind::ServedVisiblePixel,
        false,
    ));

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 2);
    assert_eq!(ppu.bg_pipeline_state.fifo.back(), Some(&0));
}

#[test]
fn wx_cpu_commit_after_visible_output_routes_through_live_trigger_glitch_logic() {
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
        wx: 7,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = MODE2_DOTS + 12;
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state.fifo.push_back(3);

    ppu.write_register_with_source(0xFF4B, 10, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(ppu.wx, 10);
    assert_eq!(ppu.visible_registers.wx, 7);
    assert_eq!(ppu.pipeline_registers.wx, 7);
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
        Some(DmgPendingLiveWxTriggerGlitch::new(3))
    );
}

#[test]
fn pending_live_wx_glitch_ignores_non_visible_transfer_dots() {
    let mut ppu = PpuTestRig::dmg();
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_live_wx_trigger_glitch = Some(DmgPendingLiveWxTriggerGlitch::new(3));
    ppu.bg_pipeline_state.visible_pixels_output = 3;

    ppu.maybe_apply_pending_dmg_live_wx_trigger_glitch(Mode3TransferDot::served(
        Mode3TransferDotKind::ServedHiddenTransfer,
        false,
    ));

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        Some(DmgPendingLiveWxTriggerGlitch::new(3))
    );
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn same_scanline_live_wx_write_clears_invalid_glitch_triggers() {
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
        wx: 7,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = MODE2_DOTS + 12;
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_live_wx_trigger_glitch = Some(DmgPendingLiveWxTriggerGlitch::new(12));

    ppu.maybe_arm_dmg_live_wx_trigger_glitch(6);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
}

#[test]
fn same_scanline_live_wx_write_clears_glitches_that_are_already_behind_the_visible_dot() {
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
        wx: 7,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = MODE2_DOTS + 12;
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_live_wx_trigger_glitch = Some(DmgPendingLiveWxTriggerGlitch::new(12));

    ppu.maybe_arm_dmg_live_wx_trigger_glitch(10);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
}

#[test]
fn same_scanline_live_wx_write_can_push_the_glitch_pixel_immediately() {
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
        wx: 7,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = MODE2_DOTS + 12;
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.visible_pixels_output = 3;
    ppu.bg_pipeline_state.fifo.push_back(2);

    ppu.maybe_arm_dmg_live_wx_trigger_glitch(10);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 2);
    assert_eq!(ppu.bg_pipeline_state.fifo.back(), Some(&0));
}

#[test]
fn pending_live_wx_glitch_expires_when_the_visible_dot_has_already_passed() {
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
        wx: 7,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = MODE2_DOTS + 12;
    ppu.visible_registers.lcdc = 0xF3;
    ppu.pipeline_registers.lcdc = 0xF3;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_live_wx_trigger_glitch = Some(DmgPendingLiveWxTriggerGlitch::new(3));

    ppu.maybe_apply_pending_dmg_live_wx_trigger_glitch(Mode3TransferDot::served(
        Mode3TransferDotKind::ServedVisiblePixel,
        false,
    ));

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
}

#[test]
fn pending_live_wx_glitch_expiry_helper_waits_until_the_trigger_is_behind() {
    let mut ppu = PpuTestRig::dmg();
    ppu.bg_pipeline_state
        .dmg_window_restart
        .pending_live_wx_trigger_glitch = Some(DmgPendingLiveWxTriggerGlitch::new(3));
    ppu.bg_pipeline_state.visible_pixels_output = 3;

    ppu.test_expire_pending_dmg_live_wx_trigger_glitch();
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        Some(DmgPendingLiveWxTriggerGlitch::new(3))
    );

    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.test_expire_pending_dmg_live_wx_trigger_glitch();
    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
}

#[test]
fn cgb_dmg_software_previsible_live_wx_write_arms_only_on_window_tile_index_phase() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut aligned = cgb_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6, operating_mode);
        aligned.bg_pipeline_state.current_transfer_x = 6;
        aligned.maybe_arm_dmg_live_wx_trigger_glitch(13);

        assert_eq!(
            aligned
                .bg_pipeline_state
                .dmg_window_restart
                .pending_live_wx_trigger_glitch,
            Some(DmgPendingLiveWxTriggerGlitch::new(6)),
            "{operating_mode:?}"
        );

        let mut unaligned = cgb_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6, operating_mode);
        unaligned.bg_pipeline_state.current_transfer_x = 6;
        unaligned.maybe_arm_dmg_live_wx_trigger_glitch(14);

        assert_eq!(
            unaligned
                .bg_pipeline_state
                .dmg_window_restart
                .pending_live_wx_trigger_glitch,
            None,
            "{operating_mode:?}"
        );
    }
}

#[test]
fn cgb_dmg_software_previsible_live_wx_trigger_inserts_a_raw_zero_pixel() {
    for operating_mode in [OperatingMode::GbCompatible, OperatingMode::CgbDmgExt] {
        let mut ppu = cgb_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6, operating_mode);
        ppu.bg_pipeline_state.current_transfer_x = 6;

        ppu.maybe_arm_dmg_live_wx_trigger_glitch(13);
        ppu.bg_pipeline_state.visible_pixels_output = 6;
        ppu.maybe_apply_pending_dmg_live_wx_trigger_glitch(Mode3TransferDot::served(
            Mode3TransferDotKind::ServedVisiblePixel,
            false,
        ));

        assert_eq!(
            ppu.bg_pipeline_state
                .dmg_window_restart
                .pending_live_wx_trigger_glitch,
            None,
            "{operating_mode:?}"
        );
        assert_eq!(
            ppu.bg_pipeline_state.fifo.back(),
            Some(&0),
            "{operating_mode:?}"
        );
    }
}

#[test]
fn native_cgb_ignores_dmg_software_previsible_live_wx_trigger_glitches() {
    let mut ppu = cgb_previsible_retarget_fixture(5, MODE2_DOTS + 14, 6, OperatingMode::Cgb);
    ppu.bg_pipeline_state.current_transfer_x = 6;

    ppu.maybe_arm_dmg_live_wx_trigger_glitch(13);

    assert_eq!(
        ppu.bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch,
        None
    );
}
