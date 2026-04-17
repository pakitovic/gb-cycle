use super::super::*;

fn dmg_mode3_startup_state(lcdc: u8, ly: u8, scx: u8) -> PpuStartupState {
    PpuStartupState {
        lcdc,
        stat: 0x82,
        scy: 0x00,
        scx,
        ly,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    }
}

fn lcdc_write_context(previous_lcdc: u8, current_lcdc: u8) -> PpuMode3LiveRegisterWriteContext {
    PpuMode3LiveRegisterWriteContext::new(
        PpuVisibleRegisters {
            lcdc: previous_lcdc,
            ..PpuVisibleRegisters::default()
        },
        PpuVisibleRegisters {
            lcdc: current_lcdc,
            ..PpuVisibleRegisters::default()
        },
    )
}

fn obj_toggle_sprite(
    oam_index: u8,
    y: u8,
    x: u8,
    tile_index: u8,
    attributes: u8,
) -> PpuSelectedSprite {
    PpuSelectedSprite {
        oam_index,
        y,
        x,
        tile_index,
        attributes,
    }
}

#[test]
fn observed_lcdc2_obj_size_plane_selection_matches_the_curated_residual_seams() {
    let cases = [
        (0, 8, 0, 0, Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8)),
        (
            0,
            16,
            0,
            0,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High),
        ),
        (
            0,
            24,
            0,
            0,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (
            2,
            33,
            0,
            8,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High),
        ),
        (
            2,
            40,
            0,
            8,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (0, 12, 4, 8, Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8)),
        (0, 12, 4, 2, None),
        (0, 32, 0, 2, Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8)),
        (0, 32, 0, 10, None),
        (0, 32, 1, 2, None),
        (0, 32, 4, 2, Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8)),
        (2, 32, 0, 2, None),
        (2, 12, 3, 8, None),
        (2, 12, 4, 8, None),
        (0, 17, 0, 0, None),
        (2, 34, 0, 0, None),
    ];

    for (write_index, sprite_x, scx, raw_row, expected) in cases {
        assert_eq!(
            PpuMode3ObservedLcdc2ObjSizePhaseTable::new(sprite_x, scx, raw_row)
                .plane_selection(write_index),
            expected,
            "write_index={write_index} sprite_x={sprite_x} scx={scx} raw_row={raw_row}",
        );
    }
}

#[test]
fn observed_lcdc1_disable_onset_matches_the_curated_single_sprite_windows() {
    let cases = [
        (1, Some(0)),
        (2, Some(0)),
        (3, Some(2)),
        (4, Some(3)),
        (5, Some(4)),
        (6, Some(4)),
        (7, Some(4)),
        (8, Some(3)),
        (13, Some(8)),
        (16, None),
    ];

    for (sprite_x, expected_onset) in cases {
        assert_eq!(
            PpuMode3SingleSpritePhasePolicy::new(sprite_x).observed_lcdc1_disable_onset_visible_x(),
            expected_onset,
            "sprite_x={sprite_x}",
        );
    }
}

#[test]
fn disabling_lcdc1_uses_the_observed_hold_window_for_the_single_selected_sprite() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x83,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 3,
        tile_index: 0,
        attributes: 0,
    });

    let write_context = PpuMode3LiveRegisterWriteContext::new(
        PpuVisibleRegisters {
            lcdc: 0x83,
            ..PpuVisibleRegisters::default()
        },
        PpuVisibleRegisters {
            lcdc: 0x81,
            ..PpuVisibleRegisters::default()
        },
    );

    ppu.apply_dmg_lcdc1_live_obj_enable_write(write_context);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold_override,
        Some(true),
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold_pixels_remaining,
        2,
    );
    assert!(ppu.pixel_transfer_obj_enabled());
    ppu.consume_dmg_lcdc1_obj_enable_visible_hold();
    assert!(ppu.pixel_transfer_obj_enabled());
    ppu.consume_dmg_lcdc1_obj_enable_visible_hold();
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold_override,
        None,
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold_pixels_remaining,
        0,
    );
}

#[test]
fn disabling_lcdc1_retroactively_repaints_object_dots_from_the_observed_onset() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x83,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.bg_pipeline_state.visible_pixels_output = 5;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 1,
        tile_index: 25,
        attributes: 0,
    });
    ppu.current_scanline_bg_pixels[0] = 0;
    ppu.current_scanline_mixed_pixels[0] = MixedPixel::object(1, false);
    ppu.current_scanline_pixels[0] = 1;

    let write_context = PpuMode3LiveRegisterWriteContext::new(
        PpuVisibleRegisters {
            lcdc: 0x83,
            ..PpuVisibleRegisters::default()
        },
        PpuVisibleRegisters {
            lcdc: 0x81,
            ..PpuVisibleRegisters::default()
        },
    );

    ppu.apply_dmg_lcdc1_live_obj_enable_write(write_context);

    assert_eq!(
        ppu.current_scanline_mixed_pixels[0],
        MixedPixel::background(0)
    );
    assert_eq!(ppu.current_scanline_pixels[0], 0);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold_override,
        Some(false),
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold_pixels_remaining,
        1,
    );
}

#[test]
fn disabling_lcdc1_at_the_first_visible_dot_keeps_the_queued_obj_prefix_pixels() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x83,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.write_oam_entry(0, 16, 3, 25);
    ppu.write_bg_tile_row(25, 0, 0x3C, 0x00);

    let start_t_cycle = ppu.t_cycle;
    while !(ppu.snapshot().mode == PpuAccessMode::Drawing
        && ppu.snapshot().visible_pixels_output == 0
        && ppu.snapshot().bg_current_transfer_x == 8)
    {
        ppu.tick();
        assert!(ppu.t_cycle - start_t_cycle < 200);
    }

    assert_eq!(ppu.snapshot().obj_fifo_pixels, vec![Some(1), None, None]);

    ppu.write_register(0xFF40, 0x81);
    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(
        &snapshot.current_scanline_pixels[..8],
        &[1, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn disabling_lcdc1_without_an_observed_single_sprite_window_clears_the_hold_override() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x83, 0, 0));
    ppu.mode2_scan_state
        .push(obj_toggle_sprite(0, 16, 16, 0, 0));
    ppu.dmg_panel_live_write_state
        .lcdc1
        .obj_enable_visible_hold_override = Some(true);
    ppu.dmg_panel_live_write_state
        .lcdc1
        .obj_enable_visible_hold_pixels_remaining = 3;

    ppu.apply_dmg_lcdc1_live_obj_enable_write(lcdc_write_context(0x83, 0x81));

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold_override,
        None,
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold_pixels_remaining,
        0,
    );
}

#[test]
fn consuming_an_empty_lcdc1_hold_clears_any_stale_override() {
    let mut ppu = PpuTestRig::dmg();
    ppu.dmg_panel_live_write_state
        .lcdc1
        .obj_enable_visible_hold_override = Some(true);
    ppu.dmg_panel_live_write_state
        .lcdc1
        .obj_enable_visible_hold_pixels_remaining = 0;

    ppu.consume_dmg_lcdc1_obj_enable_visible_hold();

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold_override,
        None,
    );
}

#[test]
fn lcdc2_live_write_tracks_only_the_16_to_8_shrink_boundary() {
    let mut ppu = PpuTestRig::dmg();
    ppu.bg_pipeline_state.visible_pixels_output = 9;

    ppu.apply_dmg_lcdc2_live_obj_size_write(lcdc_write_context(0x87, 0x83));
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc2
            .active_obj_size_write_index,
        Some(0),
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc2
            .active_obj_size_write_visible_x,
        Some(9),
    );
    assert!(ppu.dmg_panel_live_write_state.lcdc2.pending_effects);

    let mut grow = PpuTestRig::dmg();
    grow.bg_pipeline_state.visible_pixels_output = 11;
    grow.apply_dmg_lcdc2_live_obj_size_write(lcdc_write_context(0x83, 0x87));
    assert_eq!(
        grow.dmg_panel_live_write_state
            .lcdc2
            .current_line_obj_size_write_count,
        1,
    );
    assert_eq!(
        grow.dmg_panel_live_write_state
            .lcdc2
            .active_obj_size_write_index,
        None,
    );
    assert!(!grow.dmg_panel_live_write_state.lcdc2.pending_effects);
}

#[test]
fn obj_pipeline_state_tracks_queued_heights_and_resets_the_fetch_latches() {
    let sprite = obj_toggle_sprite(2, 16, 24, 0x11, 0x20);
    let owner = ObjHitOwnership {
        match_x: 24,
        phase: ObjHitPhase::Visible,
    };
    let mut state = ObjPipelineState::default();

    state.queue_fetch_hit(2, owner, 16);
    state.queue_fetch_hit(2, owner, 8);
    assert!(state.pending_hits_own_current_dot(owner));
    assert_eq!(state.pop_pending_fetch_hit(), Some((2, 16)));
    assert!(!state.pending_hits_own_current_dot(owner));

    state.start_fetch(2, sprite, 16, 8);
    assert_eq!(state.fetch.stage, PpuObjFetcherStage::Startup);
    assert_eq!(state.fetch.sprite_slot, 2);
    assert_eq!(state.fetch.sprite, Some(sprite));
    assert_eq!(state.fetch.selected_obj_height, 16);
    assert_eq!(state.fetch.latched_obj_height, 8);
    assert_eq!(state.fetch.resolved_tile_index, None);
    assert_eq!(state.fetch.resolved_tile_row, None);

    state.queue_fetch_hit(2, owner, 16);
    assert_eq!(state.pending_sprite_slots.len(), 0);

    state.mark_fetched(2);
    assert!(state.has_fetched(2));
    state.mode3_line_start_obj_height = 16;
    state.late_metadata_word = Some((0x11, 0x20));
    state.reset();
    assert!(!state.has_fetched(2));
    assert_eq!(state.pending_match_x, None);
    assert_eq!(state.mode3_line_start_obj_height, 8);
    assert_eq!(state.late_metadata_word, None);
}

#[test]
fn obj_tile_index_resolution_for_mode3_fetch_keeps_the_live_8x8_low_half_tile() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 12, 0));
    let sprite = obj_toggle_sprite(0, 16, 24, 0x11, 0x00);
    let y_flipped = obj_toggle_sprite(0, 16, 24, 0x11, 0x40);

    assert_eq!(
        ppu.obj_tile_index_and_row_for_mode3_fetch(sprite, 16, 8),
        Some((0x11, 4)),
    );
    assert_eq!(
        ppu.obj_tile_index_and_row_for_mode3_fetch(y_flipped, 16, 8),
        Some((0x11, 3)),
    );
    assert_eq!(
        ppu.obj_tile_index_and_row_for_mode3_fetch(sprite, 16, 16),
        Some((0x11, 4)),
    );
}

#[test]
fn rewriting_the_obj_fifo_can_clear_same_sprite_pixels_but_keeps_higher_priority_owners() {
    let mut ppu = PpuTestRig::dmg();
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    let sprite = obj_toggle_sprite(0, 16, 12, 0x11, 0x00);

    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 3,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: sprite.x,
        oam_index: sprite.oam_index,
    });
    ppu.rewrite_obj_fifo_pixels(sprite, 0x00, 0x00, 4);
    assert_eq!(ppu.obj_pipeline_state.fifo[0].color, 0);
    assert_eq!(ppu.obj_pipeline_state.fifo[0].sprite_x, sprite.x);

    ppu.obj_pipeline_state.fifo.clear();
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 3,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 10,
        oam_index: 0,
    });
    ppu.rewrite_obj_fifo_pixels(sprite, 0xFF, 0x00, 4);
    assert_eq!(
        ppu.obj_pipeline_state.fifo[0],
        ObjPixel {
            color: 3,
            palette_obp1: false,
            bg_over_obj: false,
            sprite_x: 10,
            oam_index: 0,
        }
    );
}

#[test]
fn lcdc2_push_bytes_cover_each_plane_selection_variant() {
    let sprite = obj_toggle_sprite(0, 16, 16, 0x11, 0x00);

    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 0, 0));
    ppu.dmg_panel_live_write_state
        .lcdc2
        .active_obj_size_write_index = Some(0);
    ppu.write_bg_tile_row(0x10, 0, 0x11, 0x22);
    ppu.write_bg_tile_row(0x11, 0, 0xAA, 0x55);
    ppu.with_ppu_vram(|ppu, vram| {
        assert_eq!(
            ppu.dmg_lcdc2_live_obj_size_push_bytes(sprite, 0x00, 0x00, vram),
            (0xAA, 0x22),
        );
    });

    ppu.mode2_scan_state.push(sprite);
    ppu.write_bg_tile_row(0x10, 0, 0x00, 0x80);
    ppu.write_bg_tile_row(0x11, 0, 0x80, 0x00);
    let overridden = ppu.with_ppu_vram(|ppu, vram| {
        ppu.apply_dmg_lcdc2_live_obj_size_output_override(
            ObjPixel {
                color: 1,
                palette_obp1: false,
                bg_over_obj: false,
                sprite_x: sprite.x,
                oam_index: sprite.oam_index,
            },
            8,
            vram,
        )
    });
    assert_eq!(overridden.color, 3);

    let mut line_start16 = PpuTestRig::dmg();
    line_start16.apply_startup_state(dmg_mode3_startup_state(0x87, 0, 0));
    line_start16
        .dmg_panel_live_write_state
        .lcdc2
        .active_obj_size_write_index = Some(0);
    line_start16.write_bg_tile_row(0x10, 0, 0x11, 0x22);
    line_start16.write_bg_tile_row(0x11, 0, 0xAA, 0x55);
    let sprite = obj_toggle_sprite(0, 16, 24, 0x11, 0x00);
    line_start16.with_ppu_vram(|ppu, vram| {
        assert_eq!(
            ppu.dmg_lcdc2_live_obj_size_push_bytes(sprite, 0x00, 0x00, vram),
            (0x11, 0x22),
        );
    });

    let mut split_planes = PpuTestRig::dmg();
    split_planes.apply_startup_state(dmg_mode3_startup_state(0x87, 4, 0));
    split_planes.bg_pipeline_state.visible_pixels_output = 25;
    split_planes
        .dmg_panel_live_write_state
        .lcdc2
        .active_obj_size_write_index = Some(2);
    split_planes
        .dmg_panel_live_write_state
        .lcdc2
        .active_obj_size_write_visible_x = Some(25);
    split_planes.write_bg_tile_row(0x10, 4, 0x11, 0x22);
    split_planes.write_bg_tile_row(0x11, 4, 0xAA, 0x55);
    let sprite = obj_toggle_sprite(0, 16, 32, 0x11, 0x00);
    split_planes.with_ppu_vram(|ppu, vram| {
        assert_eq!(
            ppu.dmg_lcdc2_live_obj_size_push_bytes(sprite, 0x00, 0x00, vram),
            (0x11, 0x55),
        );
    });
}

#[test]
fn pending_lcdc2_effects_clear_stale_flags_without_an_active_write_index() {
    let mut ppu = PpuTestRig::dmg();
    ppu.dmg_panel_live_write_state.lcdc2.pending_effects = true;

    ppu.with_ppu_vram(|ppu, vram| ppu.apply_pending_dmg_lcdc2_observed_write_effects(vram));

    assert!(!ppu.dmg_panel_live_write_state.lcdc2.pending_effects);
}

#[test]
fn pending_lcdc2_effects_repaint_the_shifted_left_scx_overlap() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 8, 4));
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.current_scanline_bg_pixels[4..6].fill(0);
    ppu.current_scanline_mixed_pixels[4..6].fill(MixedPixel::background(0));
    ppu.current_scanline_pixels[4..6].fill(0);
    ppu.dmg_panel_live_write_state
        .recent_panel_dots
        .push_back(PpuRecentPanelDot {
            visible_x: 4,
            pixel: MixedPixel::background(0),
            dmg_bg_forced_white: false,
        });
    let sprite = obj_toggle_sprite(0, 16, 12, 0x11, 0x00);
    ppu.mode2_scan_state.push(sprite);
    ppu.obj_pipeline_state.mark_fetched(0);
    ppu.dmg_panel_live_write_state
        .lcdc2
        .active_obj_size_write_index = Some(0);
    ppu.dmg_panel_live_write_state
        .lcdc2
        .active_obj_size_write_visible_x = Some(6);
    ppu.dmg_panel_live_write_state.lcdc2.pending_effects = true;
    ppu.write_bg_tile_row(0x10, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(0x11, 0, 0xFF, 0x00);

    ppu.with_ppu_vram(|ppu, vram| ppu.apply_pending_dmg_lcdc2_observed_write_effects(vram));

    assert_eq!(
        ppu.current_scanline_mixed_pixels[4],
        MixedPixel::object(1, false)
    );
    assert_eq!(ppu.current_scanline_pixels[4], 1);
    assert_eq!(
        ppu.dmg_panel_live_write_state.recent_panel_dots[0].pixel,
        MixedPixel::object(1, false),
    );
    assert!(!ppu.dmg_panel_live_write_state.lcdc2.pending_effects);
}

#[test]
fn pending_lcdc2_effects_rewrite_the_late_tail_fifo_for_the_scx0_variant() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 4, 0));
    ppu.bg_pipeline_state.visible_pixels_output = 25;
    let sprite = obj_toggle_sprite(0, 16, 32, 0x11, 0x00);
    ppu.mode2_scan_state.push(sprite);
    ppu.obj_pipeline_state.mark_fetched(0);
    ppu.dmg_panel_live_write_state
        .lcdc2
        .active_obj_size_write_index = Some(2);
    ppu.dmg_panel_live_write_state
        .lcdc2
        .active_obj_size_write_visible_x = Some(25);
    ppu.dmg_panel_live_write_state.lcdc2.pending_effects = true;
    ppu.write_bg_tile_row(0x10, 4, 0x00, 0x00);
    ppu.write_bg_tile_row(0x11, 4, 0x00, 0xFF);

    ppu.with_ppu_vram(|ppu, vram| ppu.apply_pending_dmg_lcdc2_observed_write_effects(vram));

    assert_eq!(ppu.obj_pipeline_state.fifo[0].color, 2);
    assert_eq!(ppu.obj_pipeline_state.fifo[0].sprite_x, sprite.x);
    assert_eq!(ppu.obj_pipeline_state.fifo[0].oam_index, sprite.oam_index);
    assert!(!ppu.dmg_panel_live_write_state.lcdc2.pending_effects);
}
