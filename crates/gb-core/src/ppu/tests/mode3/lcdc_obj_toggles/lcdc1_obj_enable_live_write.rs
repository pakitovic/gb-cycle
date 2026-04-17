use super::fixtures::*;
use super::*;

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
            .obj_enable_visible_hold
            .override_value,
        Some(true),
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold
            .pixels_remaining,
        2,
    );
    assert!(ppu.pixel_transfer_obj_enabled());
    ppu.consume_dmg_lcdc1_obj_enable_visible_hold();
    assert!(ppu.pixel_transfer_obj_enabled());
    ppu.consume_dmg_lcdc1_obj_enable_visible_hold();
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold
            .override_value,
        None,
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold
            .pixels_remaining,
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
            .obj_enable_visible_hold
            .override_value,
        Some(false),
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold
            .pixels_remaining,
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
        .obj_enable_visible_hold
        .override_value = Some(true);
    ppu.dmg_panel_live_write_state
        .lcdc1
        .obj_enable_visible_hold
        .pixels_remaining = 3;

    ppu.apply_dmg_lcdc1_live_obj_enable_write(lcdc_write_context(0x83, 0x81));

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold
            .override_value,
        None,
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold
            .pixels_remaining,
        0,
    );
}

#[test]
fn consuming_an_empty_lcdc1_hold_clears_any_stale_override() {
    let mut ppu = PpuTestRig::dmg();
    ppu.dmg_panel_live_write_state
        .lcdc1
        .obj_enable_visible_hold
        .override_value = Some(true);
    ppu.dmg_panel_live_write_state
        .lcdc1
        .obj_enable_visible_hold
        .pixels_remaining = 0;

    ppu.consume_dmg_lcdc1_obj_enable_visible_hold();

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc1
            .obj_enable_visible_hold
            .override_value,
        None,
    );
}
