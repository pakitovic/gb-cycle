use super::fixtures::*;
use super::*;

#[test]
fn lcdc2_live_write_tracks_only_the_16_to_8_shrink_boundary() {
    let mut ppu = PpuTestRig::dmg();
    ppu.bg_pipeline_state.visible_pixels_output = 9;

    ppu.apply_dmg_lcdc2_live_obj_size_write(lcdc_write_context(0x87, 0x83));
    assert_eq!(
        ppu.dmg_panel_live_write_state.lcdc2.active_write,
        Some(DmgLcdc2ActiveObjSizeWrite::new(0, 9)),
    );
    assert!(
        ppu.dmg_panel_live_write_state
            .lcdc2
            .active_write()
            .is_some_and(DmgLcdc2ActiveObjSizeWrite::observed_effects_pending)
    );

    let mut grow = PpuTestRig::dmg();
    grow.bg_pipeline_state.visible_pixels_output = 11;
    grow.apply_dmg_lcdc2_live_obj_size_write(lcdc_write_context(0x83, 0x87));
    assert_eq!(
        grow.dmg_panel_live_write_state
            .lcdc2
            .current_line_obj_size_write_count,
        1,
    );
    assert_eq!(grow.dmg_panel_live_write_state.lcdc2.active_write, None);
}

#[test]
fn lcdc2_live_write_retains_an_older_pending_shrink_when_a_new_one_arrives() {
    let mut state = DmgLcdc2ObjSizeLiveWriteState::default();

    state.begin_active_shrink(0, 4);
    state.begin_active_shrink(2, 24);

    assert_eq!(
        state.retained_pending_write,
        Some(DmgLcdc2ActiveObjSizeWrite::new(0, 4)),
    );
    assert_eq!(
        state.active_write,
        Some(DmgLcdc2ActiveObjSizeWrite::new(2, 24))
    );
}

#[test]
fn pending_lcdc2_effects_stay_pending_until_a_fetched_sprite_uses_them() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 8, 4));
    ppu.dmg_panel_live_write_state.lcdc2.active_write = Some(DmgLcdc2ActiveObjSizeWrite::new(0, 6));

    ppu.with_ppu_vram(|ppu, vram| ppu.apply_pending_dmg_lcdc2_observed_write_effects(vram));

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc2
            .active_write
            .expect("write stays pending when no fetched sprite can observe it")
            .observed_effect_state,
        DmgLcdc2ObservedEffectState::Pending,
    );
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
    ppu.dmg_panel_live_write_state.lcdc2.active_write = Some(DmgLcdc2ActiveObjSizeWrite::new(0, 0));
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
    line_start16.dmg_panel_live_write_state.lcdc2.active_write =
        Some(DmgLcdc2ActiveObjSizeWrite::new(0, 0));
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
    split_planes.dmg_panel_live_write_state.lcdc2.active_write =
        Some(DmgLcdc2ActiveObjSizeWrite::new(2, 25));
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
fn retained_pending_lcdc2_write_can_still_drive_push_bytes_after_a_newer_write_arrives() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 2, 4));
    ppu.dmg_panel_live_write_state.lcdc2.retained_pending_write =
        Some(DmgLcdc2ActiveObjSizeWrite::new(0, 12));
    ppu.dmg_panel_live_write_state.lcdc2.active_write =
        Some(DmgLcdc2ActiveObjSizeWrite::new(2, 24));
    ppu.write_bg_tile_row(0x10, 2, 0x11, 0x22);
    ppu.write_bg_tile_row(0x11, 2, 0xAA, 0x55);
    let sprite = obj_toggle_sprite(0, 16, 32, 0x11, 0x00);

    ppu.with_ppu_vram(|ppu, vram| {
        assert_eq!(
            ppu.dmg_lcdc2_live_obj_size_push_bytes(sprite, 0x11, 0x22, vram),
            (0xAA, 0x55),
        );
    });
}

#[test]
fn retained_pending_lcdc2_write_can_still_drive_output_override_after_a_newer_write_arrives() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 2, 4));
    ppu.mode2_scan_state
        .push(obj_toggle_sprite(0, 16, 32, 0x11, 0x00));
    ppu.dmg_panel_live_write_state.lcdc2.retained_pending_write =
        Some(DmgLcdc2ActiveObjSizeWrite::new(0, 12));
    ppu.dmg_panel_live_write_state.lcdc2.active_write =
        Some(DmgLcdc2ActiveObjSizeWrite::new(2, 24));
    ppu.write_bg_tile_row(0x10, 2, 0x00, 0x80);
    ppu.write_bg_tile_row(0x11, 2, 0x80, 0x00);

    let overridden = ppu.with_ppu_vram(|ppu, vram| {
        ppu.apply_dmg_lcdc2_live_obj_size_output_override(
            ObjPixel {
                color: 2,
                palette_obp1: false,
                bg_over_obj: false,
                sprite_x: 32,
                oam_index: 0,
            },
            24,
            vram,
        )
    });

    assert_eq!(overridden.color, 1);
}

#[test]
fn retained_pending_lcdc2_write_does_not_override_scx0_sprite32_upper_half() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 2, 0));
    ppu.dmg_panel_live_write_state.lcdc2.retained_pending_write =
        Some(DmgLcdc2ActiveObjSizeWrite::new(0, 12));
    ppu.dmg_panel_live_write_state.lcdc2.active_write =
        Some(DmgLcdc2ActiveObjSizeWrite::new(2, 24));
    ppu.write_bg_tile_row(0x10, 2, 0x11, 0x22);
    ppu.write_bg_tile_row(0x11, 2, 0xAA, 0x55);
    let sprite = obj_toggle_sprite(0, 16, 32, 0x11, 0x00);

    ppu.with_ppu_vram(|ppu, vram| {
        assert_eq!(
            ppu.dmg_lcdc2_live_obj_size_push_bytes(sprite, 0x11, 0x22, vram),
            (0x11, 0x22),
        );
    });

    ppu.mode2_scan_state.push(sprite);
    ppu.write_bg_tile_row(0x10, 2, 0x00, 0x80);
    ppu.write_bg_tile_row(0x11, 2, 0x80, 0x00);
    let overridden = ppu.with_ppu_vram(|ppu, vram| {
        ppu.apply_dmg_lcdc2_live_obj_size_output_override(
            ObjPixel {
                color: 2,
                palette_obp1: false,
                bg_over_obj: false,
                sprite_x: 32,
                oam_index: 0,
            },
            24,
            vram,
        )
    });

    assert_eq!(overridden.color, 2);
}

#[test]
fn applied_lcdc2_effects_keep_the_active_write_but_do_not_re_run_pending_work() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 8, 4));
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.current_scanline_bg_pixels[4] = 0;
    ppu.current_scanline_mixed_pixels[4] = MixedPixel::background(0);
    ppu.current_scanline_pixels[4] = 0;
    ppu.dmg_panel_live_write_state.lcdc2.active_write = Some(DmgLcdc2ActiveObjSizeWrite {
        write_index: 0,
        visible_x: 6,
        observed_effect_state: DmgLcdc2ObservedEffectState::Applied,
    });
    ppu.write_bg_tile_row(0x10, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(0x11, 0, 0xFF, 0x00);

    ppu.with_ppu_vram(|ppu, vram| ppu.apply_pending_dmg_lcdc2_observed_write_effects(vram));

    assert_eq!(
        ppu.current_scanline_mixed_pixels[4],
        MixedPixel::background(0)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc2
            .active_write
            .expect("active write stays latched")
            .observed_effect_state,
        DmgLcdc2ObservedEffectState::Applied,
    );
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
    ppu.dmg_panel_live_write_state.lcdc2.active_write = Some(DmgLcdc2ActiveObjSizeWrite::new(0, 6));
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
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc2
            .active_write
            .expect("active write remains after applying effects")
            .observed_effect_state,
        DmgLcdc2ObservedEffectState::Applied,
    );
}

#[test]
fn pending_lcdc2_effects_rewrite_the_late_tail_fifo_for_the_scx0_variant() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(dmg_mode3_startup_state(0x87, 4, 0));
    ppu.bg_pipeline_state.visible_pixels_output = 25;
    let sprite = obj_toggle_sprite(0, 16, 32, 0x11, 0x00);
    ppu.mode2_scan_state.push(sprite);
    ppu.obj_pipeline_state.mark_fetched(0);
    ppu.dmg_panel_live_write_state.lcdc2.active_write =
        Some(DmgLcdc2ActiveObjSizeWrite::new(2, 25));
    ppu.write_bg_tile_row(0x10, 4, 0x00, 0x00);
    ppu.write_bg_tile_row(0x11, 4, 0x00, 0xFF);

    ppu.with_ppu_vram(|ppu, vram| ppu.apply_pending_dmg_lcdc2_observed_write_effects(vram));

    assert_eq!(ppu.obj_pipeline_state.fifo[0].color, 2);
    assert_eq!(ppu.obj_pipeline_state.fifo[0].sprite_x, sprite.x);
    assert_eq!(ppu.obj_pipeline_state.fifo[0].oam_index, sprite.oam_index);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc2
            .active_write
            .expect("active write remains after applying effects")
            .observed_effect_state,
        DmgLcdc2ObservedEffectState::Applied,
    );
}
