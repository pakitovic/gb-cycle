use super::super::*;

struct TileSelPhaseCase {
    sprite_x: u8,
    expected: [u8; 24],
}

fn new_dmg_lcdc_tile_sel_replay_ppu(sprite_x: u8) -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
    ppu.write_oam_entry(0, 26, sprite_x, 0);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x00,
        stat: 0xA4,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.write_register(0xFF40, 0x83);
    ppu
}

#[test]
fn sprite_coupled_line10_tile_sel_replay_matches_trace_signature() {
    let mut ppu = new_dmg_lcdc_tile_sel_replay_ppu(1);

    ppu.advance_until_tile_sel_replay_position(10, 85);

    let startup = ppu.snapshot();
    assert_eq!(startup.visible_lcdc, 0x83);
    assert_eq!(startup.pipeline_lcdc, 0x83);
    assert_eq!(startup.visible_pixels_output, 0);
    assert_eq!(startup.bg_current_transfer_x, 1);
    assert!(startup.bg_fill_pending);
    assert_eq!(startup.bg_fill_startup_dummy_pixels, 7);
    assert_eq!(startup.bg_startup_fifo_placeholders, 7);
    assert_eq!(startup.selected_sprites.len(), 1);
    assert_eq!(
        startup.bg_startup_fetch_seam,
        PpuBgStartupFetchSeamSnapshot::PostAlignment {
            first_real_push_skips_entry_delay: true,
            next_startup_continuation_slice: PpuBgStartupContinuationSliceSnapshot::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        }
    );
}

#[test]
fn sprite_coupled_line10_startup_tail_renders_correctly_once_panel_blank_is_lifted() {
    let mut ppu = new_dmg_lcdc_tile_sel_replay_ppu(1);

    for row in 0..BG_TILE_WIDTH {
        ppu.write_bg_tile_row(0, row, 0x00, 0x00);
        let signed_tile_row = 0x1000 + row as usize * TILE_ROW_BYTES as usize;
        ppu.vram_bytes[signed_tile_row] = 0xFF;
        ppu.vram_bytes[signed_tile_row + 1] = 0xFF;
    }

    ppu.advance_until_tile_sel_replay_position(10, 85);
    ppu.blank_frame_active = false;
    ppu.refresh_visible_output();
    assert_eq!(ppu.visible_output, PpuVisibleOutputState::Driving);
    ppu.advance_until_tile_sel_replay_position(10, 101);
    let front_cached = ppu.bg_pipeline_state.fifo.cached_slot(0).expect("BG FIFO cached slot must exist").expect(
        "the first visible startup-tail pixel should already be materialized before line_dot 102",
    );
    assert_eq!(
        front_cached.cached.origin,
        BgCachedSliceOrigin::StartupAlignmentFill
    );
    assert_eq!(ppu.bg_pipeline_state.fifo[0], 3);
    assert!(!front_cached.cached.needs_live_tilemap_refetch);
    assert!(!front_cached.cached.needs_live_tile_data_refetch);
    assert!(!front_cached.cached.needs_live_tile_data_current_row_refetch);
    assert!(!front_cached.cached.needs_live_tile_data_unsigned_reuse);
    assert_eq!(
        front_cached.cached.tile_low, 0xFF,
        "tile_high={:#04X} tile_data_address={:#06X}",
        front_cached.cached.tile_high, front_cached.cached.tile_data_address,
    );
    assert_eq!(front_cached.cached.tile_high, 0xFF);
    while !(ppu.snapshot().ly == 10 && ppu.snapshot().visible_pixels_output == 1) {
        apply_tile_sel_line_write_replay(&mut ppu);
        assert!(ppu.t_cycle < 11_000);
        ppu.tick();
    }

    let first_visible = ppu.snapshot();
    assert_eq!(
        first_visible.current_scanline_pixels[0],
        3,
        "line_dot={} visible_lcdc={:#04X} pipeline_lcdc={:#04X} visible_output={:?} current_transfer_x={}",
        first_visible.line_dot,
        first_visible.visible_lcdc,
        first_visible.pipeline_lcdc,
        first_visible.visible_output,
        first_visible.bg_current_transfer_x,
    );
}

fn dmg_tile_sel_replay_background_row(sprite_x: u8) -> [u8; 24] {
    let mut ppu = new_dmg_lcdc_tile_sel_replay_ppu(sprite_x);

    for row in 0..BG_TILE_WIDTH {
        ppu.write_bg_tile_row(0, row, 0xFF, 0xFF);
        let signed_tile_row = 0x1000 + row as usize * TILE_ROW_BYTES as usize;
        ppu.vram_bytes[signed_tile_row] = 0x00;
        ppu.vram_bytes[signed_tile_row + 1] = 0x00;
    }

    ppu.advance_until_tile_sel_replay_position(10, 85);
    ppu.blank_frame_active = false;
    ppu.refresh_visible_output();
    while !(ppu.snapshot().ly == 10 && ppu.snapshot().visible_pixels_output == 24) {
        apply_tile_sel_line_write_replay(&mut ppu);
        assert!(ppu.t_cycle < 12_000);
        ppu.tick();
    }

    let row = ppu.snapshot().current_scanline_pixels;
    let bg_start = sprite_x as usize;
    let mut sample = [0; 24];
    sample.copy_from_slice(&row[bg_start..bg_start + 24]);
    sample
}

#[test]
fn sprite_coupled_tile_sel_replay_matches_curated_background_windows() {
    let cases = [
        TileSelPhaseCase {
            sprite_x: 5,
            expected: [
                0, 0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0,
            ],
        },
        TileSelPhaseCase {
            sprite_x: 8,
            expected: [0; 24],
        },
        TileSelPhaseCase {
            sprite_x: 13,
            expected: [
                0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        },
        TileSelPhaseCase {
            sprite_x: 16,
            expected: [
                3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        },
    ];

    for case in cases {
        assert_eq!(
            dmg_tile_sel_replay_background_row(case.sprite_x),
            case.expected,
            "sprite_x={}",
            case.sprite_x,
        );
    }
}
