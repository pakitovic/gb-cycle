use super::*;

#[test]
fn obj_priority_prefers_lower_x_before_oam_order() {
    let mut ppu = dmg_obj_render_rig(ObjRenderRigConfig { lcdc: 0x82, ly: 0 });
    ppu.write_oam_entry(0, 16, 20, 0);
    ppu.write_oam_entry(1, 16, 18, 1);
    ppu.write_bg_tile_row(0, 0, 0xFF, 0x00);
    ppu.write_bg_tile_row(1, 0, 0x00, 0xFF);

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(
        &snapshot.current_scanline_pixels[10..20],
        &[2, 2, 2, 2, 2, 2, 2, 2, 1, 1]
    );
}

#[test]
fn previsible_left_edge_obj_push_keeps_negative_screen_pixels_until_hidden_dots_consume_them() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.bg_pipeline_state.current_transfer_x = 6;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;

    let sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 6,
        tile_index: 0,
        attributes: 0,
    };

    ppu.push_obj_pixels(sprite, 0xC0, 0x00, 0);

    assert_eq!(
        ppu.obj_pipeline_state
            .fifo
            .iter()
            .take(8)
            .map(|pixel| pixel.color)
            .collect::<Vec<_>>(),
        vec![1, 1, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn obj_priority_uses_oam_order_when_x_matches() {
    let mut ppu = dmg_obj_render_rig(ObjRenderRigConfig { lcdc: 0x82, ly: 0 });
    ppu.write_oam_entry(0, 16, 20, 0);
    ppu.write_oam_entry(1, 16, 20, 1);
    ppu.write_bg_tile_row(0, 0, 0xFF, 0x00);
    ppu.write_bg_tile_row(1, 0, 0x00, 0xFF);

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[12..20], &[1; 8]);
}

#[test]
fn transparent_obj_pixels_do_not_hide_lower_priority_obj_pixels() {
    let mut ppu = dmg_obj_render_rig(ObjRenderRigConfig { lcdc: 0x82, ly: 0 });
    ppu.write_oam_entry(0, 16, 20, 0);
    ppu.write_oam_entry(1, 16, 20, 1);
    ppu.write_bg_tile_row(0, 0, 0xAA, 0x00);
    ppu.write_bg_tile_row(1, 0, 0x00, 0xFF);

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(
        &snapshot.current_scanline_pixels[12..20],
        &[1, 2, 1, 2, 1, 2, 1, 2]
    );
}

#[test]
fn bg_over_obj_priority_blocks_only_nonzero_bg_pixels() {
    let mut ppu = dmg_obj_render_rig(ObjRenderRigConfig { lcdc: 0x93, ly: 0 });
    write_oam_entry_with_attributes(&mut ppu.oam_bytes, 0, 16, 8, 0, 0x80);
    ppu.write_bg_tile_row(0, 0, 0x00, 0xFF);
    ppu.write_bg_tile_row(1, 0, 0xAA, 0x00);
    ppu.write_bg_tilemap_entry(0, 0, 1);
    ppu.write_bg_tilemap_entry(1, 0, 1);

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(
        &snapshot.current_scanline_pixels[..8],
        &[1, 2, 1, 2, 1, 2, 1, 2]
    );
}

#[test]
fn obj_8x16_uses_even_aligned_tile_pairs_for_lower_half_rows() {
    let mut ppu = dmg_obj_render_rig(ObjRenderRigConfig { lcdc: 0x86, ly: 0 });
    ppu.write_oam_entry(0, 8, 8, 0x11);
    ppu.write_bg_tile_row(0x10, 0, 0xFF, 0x00);
    ppu.write_bg_tile_row(0x11, 0, 0x00, 0xFF);

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[..8], &[2; 8]);
}

#[test]
fn partially_visible_top_clipped_8x16_sprite_uses_the_correct_row() {
    let mut ppu = dmg_obj_render_rig(ObjRenderRigConfig { lcdc: 0x86, ly: 0 });
    ppu.write_oam_entry(0, 2, 8, 0x10);
    ppu.write_bg_tile_row(0x11, 6, 0x00, 0xFF);

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[..8], &[2; 8]);
}

#[test]
fn partially_visible_bottom_clipped_sprite_uses_the_correct_final_rows() {
    let mut ppu = dmg_obj_render_rig(ObjRenderRigConfig {
        lcdc: 0x82,
        ly: 143,
    });
    ppu.write_oam_entry(0, 154, 8, 0x12);
    ppu.write_bg_tile_row(0x12, 5, 0xFF, 0xFF);

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[..8], &[3; 8]);
}

#[test]
fn live_obj_size_shrink_drops_out_of_range_y_flipped_rows_without_panicking() {
    let mut ppu = PpuTestRig::dmg();
    ppu.lcdc = 0x82;
    ppu.ly = 0;

    let sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 2,
        x: 8,
        tile_index: 0x10,
        attributes: 0x40,
    };

    assert_eq!(ppu.obj_tile_index_and_row(sprite), None);
}

#[test]
fn mode3_live_obj_size_shrink_wraps_lower_half_rows_onto_the_base_tile() {
    let mut ppu = PpuTestRig::dmg();
    ppu.lcdc = 0x82;
    ppu.ly = 24;

    let sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 32,
        x: 16,
        tile_index: 0x4C,
        attributes: 0x00,
    };

    assert_eq!(
        ppu.obj_tile_index_and_row_for_mode3_fetch(sprite, 16, 8),
        Some((0x4C, 0))
    );
}

#[test]
fn mode3_live_obj_size_shrink_wraps_y_flipped_lower_half_rows_onto_the_base_tile() {
    let mut ppu = PpuTestRig::dmg();
    ppu.lcdc = 0x82;
    ppu.ly = 24;

    let sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 32,
        x: 16,
        tile_index: 0x4C,
        attributes: 0x40,
    };

    assert_eq!(
        ppu.obj_tile_index_and_row_for_mode3_fetch(sprite, 16, 8),
        Some((0x4C, 7))
    );
}

#[test]
fn turning_off_lcdc1_during_object_fetch_cancels_sprite_pixels_but_keeps_timing_cost() {
    fn run_case(disable_obj_during_fetch: bool) -> PpuSnapshot {
        let mut ppu = dmg_obj_render_rig(ObjRenderRigConfig { lcdc: 0x82, ly: 0 });
        ppu.write_oam_entry(0, 16, 8, 0);
        ppu.write_bg_tile_row(0, 0, 0xFF, 0x00);

        loop {
            ppu.tick();

            let fetching = ppu.snapshot();
            if fetching.obj_fetcher_stage != PpuObjFetcherStage::Idle {
                assert!(
                    ppu.current_access_mode() == PpuAccessMode::Drawing,
                    "left-edge OBJ fetch must still begin during Mode 3"
                );
                assert!(
                    fetching.visible_pixels_output <= 1,
                    "left-edge OBJ fetch should still begin around the left edge"
                );
                break;
            }

            assert!(
                ppu.t_cycle < 160,
                "sprite fetch should begin during early Mode 3"
            );
        }

        if disable_obj_during_fetch {
            ppu.write_register(0xFF40, 0x80);
        }

        ppu.advance_until_hblank();
        ppu.snapshot()
    }

    let enabled = run_case(false);
    let disabled = run_case(true);

    assert_eq!(disabled.mode0_start_dot, enabled.mode0_start_dot);
    assert_ne!(enabled.current_scanline_pixels[0], 0);
    assert_eq!(&disabled.current_scanline_pixels[..8], &[0; 8]);
}
