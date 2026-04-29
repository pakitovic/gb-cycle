use super::*;

#[test]
fn framebuffer_applies_bgp_without_changing_logical_scanline_colors() {
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);

    let mut ppu = PpuTestRig::dmg()
        .with_oam([0; 160])
        .with_vram(vram_bytes)
        .with_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x1B,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(
        &snapshot.current_scanline_pixels[..8],
        &[0, 1, 2, 3, 0, 1, 2, 3]
    );
    assert_eq!(&ppu.framebuffer()[..8], &[3, 2, 1, 0, 3, 2, 1, 0]);
}

#[test]
fn dmg_pixel_output_uses_or_of_current_and_previous_bgp_for_one_dot() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xB1,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.visible_registers.bgp = 0x1B;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.bgp = 0xE4;
    ppu.bg_pipeline_state.scx_discard_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[0], 1);
    assert_eq!(ppu.framebuffer()[0], 3);
}

#[test]
fn first_visible_pixel_uses_live_lcdc_instead_of_the_delayed_copy() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 2,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 8,
        oam_index: 0,
    });

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[0], 2);
}

#[test]
fn later_visible_pixels_use_the_delayed_lcdc_copy() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 1;
    ppu.bg_pipeline_state.current_transfer_x = 9;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 2,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 9,
        oam_index: 0,
    });

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[1], 1);
}

#[test]
fn later_visible_pixels_use_the_live_lcdc_bg_enable_bit() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x92;
    ppu.pipeline_registers.lcdc = 0x93;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 1;
    ppu.bg_pipeline_state.current_transfer_x = 9;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[1], 0);
}

#[test]
fn later_visible_pixels_keep_live_bg_output_when_only_the_delayed_copy_disables_bg() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x92;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 1;
    ppu.bg_pipeline_state.current_transfer_x = 9;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[1], 1);
}

#[test]
fn bg_disabled_background_pixels_force_white_panel_output_on_dmg() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x92;
    ppu.pipeline_registers.lcdc = 0x93;
    ppu.visible_registers.bgp = 0x1B;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[0], 0);
    assert_eq!(ppu.framebuffer()[0], 0);
}

#[test]
fn bg_disabled_background_pixels_keep_the_underlying_mixed_bg_color_on_dmg() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x92;
    ppu.pipeline_registers.lcdc = 0x93;
    ppu.visible_registers.bgp = 0xE4;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(2);

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(
        ppu.current_scanline_mixed_pixels[0],
        MixedPixel::background(2)
    );
    assert_eq!(ppu.snapshot().current_scanline_pixels[0], 0);
    assert_eq!(ppu.framebuffer()[0], 0);
}

#[test]
fn dmg_single_left_sprite_lcdc0_first_write_retroactively_forces_the_expected_left_edge_window() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.lcdc = 0x93;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x93;
    ppu.visible_registers.bgp = 0xE4;
    ppu.line_dot = 104;
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.bg_pipeline_state.current_transfer_x = 12;
    ppu.mode2_scan_state.selected_sprite_count = 1;
    ppu.mode2_scan_state.selected_sprites[0] = Some(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });

    for (x, color) in [1, 2, 3, 1].into_iter().enumerate() {
        let pixel = MixedPixel::background(color);
        ppu.current_scanline_mixed_pixels[x] = pixel;
        ppu.current_scanline_pixels[x] = color;
        ppu.current_scanline_dmg_bg_forced_white[x] = false;
        ppu.framebuffer[x] = ppu.map_mixed_pixel_to_panel_shade(pixel);
    }

    ppu.write_register(0xFF40, 0x92);

    assert_eq!(&ppu.current_scanline_pixels[..4], &[0, 0, 0, 0]);
    assert_eq!(&ppu.framebuffer()[..4], &[0, 0, 0, 0]);
    assert!(
        ppu.current_scanline_dmg_bg_forced_white[..4]
            .iter()
            .all(|&dot| dot)
    );
    assert_eq!(
        &ppu.current_scanline_mixed_pixels[..4],
        &[
            MixedPixel::background(1),
            MixedPixel::background(2),
            MixedPixel::background(3),
            MixedPixel::background(1),
        ]
    );
}

#[test]
fn dmg_single_left_sprite_lcdc0_second_write_restores_the_expected_retroactive_window() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.lcdc = 0x92;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x92;
    ppu.pipeline_registers.lcdc = 0x92;
    ppu.visible_registers.bgp = 0xE4;
    ppu.line_dot = 116;
    ppu.bg_pipeline_state.visible_pixels_output = 16;
    ppu.bg_pipeline_state.current_transfer_x = 24;
    ppu.mode2_scan_state.selected_sprite_count = 1;
    ppu.mode2_scan_state.selected_sprites[0] = Some(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });
    ppu.dmg_panel_live_write_state
        .lcdc0
        .current_line_bg_enable_write_count = 1;

    for x in 0..16 {
        let color = ((x % 3) + 1) as u8;
        let pixel = MixedPixel::background(color);
        ppu.current_scanline_mixed_pixels[x] = pixel;
        ppu.current_scanline_pixels[x] = 0;
        ppu.current_scanline_dmg_bg_forced_white[x] = true;
        ppu.framebuffer[x] = 0;
    }

    ppu.write_register(0xFF40, 0x93);

    assert_eq!(&ppu.current_scanline_pixels[..11], &[0; 11]);
    assert_eq!(&ppu.framebuffer()[..11], &[0; 11]);
    assert!(
        ppu.current_scanline_dmg_bg_forced_white[..11]
            .iter()
            .all(|&dot| dot)
    );

    let expected_restored_pixels = [3, 1, 2, 3, 1];
    assert_eq!(
        &ppu.current_scanline_pixels[11..16],
        &expected_restored_pixels
    );
    assert_eq!(&ppu.framebuffer()[11..16], &expected_restored_pixels);
    assert!(
        ppu.current_scanline_dmg_bg_forced_white[11..16]
            .iter()
            .all(|&dot| !dot)
    );
}

#[test]
fn dmg_lcdc0_historical_repaint_only_updates_background_dots() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.lcdc = 0x93;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x93;
    ppu.visible_registers.bgp = 0xE4;
    ppu.visible_registers.obp0 = Some(0x00);
    ppu.line_dot = 104;
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.bg_pipeline_state.current_transfer_x = 12;
    ppu.mode2_scan_state.selected_sprite_count = 1;
    ppu.mode2_scan_state.selected_sprites[0] = Some(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });

    let pixels = [
        MixedPixel::background(1),
        MixedPixel::object(2, false),
        MixedPixel::background(3),
        MixedPixel::background(1),
    ];
    ppu.current_scanline_mixed_pixels[..4].copy_from_slice(&pixels);
    ppu.current_scanline_pixels[..4].copy_from_slice(&[1, 2, 3, 1]);
    ppu.framebuffer[..4].copy_from_slice(&[1, 3, 3, 1]);

    for (visible_x, pixel) in pixels.into_iter().enumerate() {
        ppu.dmg_panel_live_write_state
            .recent_panel_dots
            .push_back(PpuRecentPanelDot {
                visible_x: visible_x as u8,
                pixel,
                dmg_bg_forced_white: false,
            });
    }

    ppu.write_register(0xFF40, 0x92);

    assert_eq!(&ppu.current_scanline_pixels[..4], &[0, 2, 0, 0]);
    assert_eq!(&ppu.framebuffer()[..4], &[0, 3, 0, 0]);
    assert_eq!(
        &ppu.current_scanline_dmg_bg_forced_white[..4],
        &[true, false, true, true]
    );

    let recent_dot_forced_white: Vec<_> = ppu
        .dmg_panel_live_write_state
        .recent_panel_dots
        .iter()
        .map(|dot| dot.dmg_bg_forced_white)
        .collect();
    assert_eq!(recent_dot_forced_white, vec![true, false, true, true]);
}

#[test]
fn dmg_lcdc0_historical_repaint_ignores_active_bgp_output_delay_override() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.lcdc = 0x92;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x92;
    ppu.visible_registers.bgp = 0xE4;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.line_dot = 116;
    ppu.bg_pipeline_state.visible_pixels_output = 16;
    ppu.bg_pipeline_state.current_transfer_x = 24;
    ppu.mode2_scan_state.selected_sprite_count = 1;
    ppu.mode2_scan_state.selected_sprites[0] = Some(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });
    ppu.dmg_panel_live_write_state
        .lcdc0
        .current_line_bg_enable_write_count = 1;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .output_palette_override = Some(0x00);
    for x in 0..16 {
        ppu.current_scanline_mixed_pixels[x] = MixedPixel::background(2);
        ppu.current_scanline_pixels[x] = 0;
        ppu.current_scanline_dmg_bg_forced_white[x] = true;
        ppu.framebuffer[x] = 0;
    }

    ppu.write_register(0xFF40, 0x93);

    assert_eq!(&ppu.current_scanline_pixels[..11], &[0; 11]);
    assert_eq!(&ppu.framebuffer()[..11], &[0; 11]);
    assert_eq!(&ppu.current_scanline_pixels[11..16], &[2; 5]);
    assert_eq!(&ppu.framebuffer()[11..16], &[2; 5]);
    assert!(
        ppu.current_scanline_dmg_bg_forced_white[..11]
            .iter()
            .all(|&dot| dot)
    );
    assert!(
        ppu.current_scanline_dmg_bg_forced_white[11..16]
            .iter()
            .all(|&dot| !dot)
    );
}

#[test]
fn dmg_single_left_sprite_lcdc0_first_write_waits_until_the_modeled_future_onset() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.lcdc = 0x93;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x93;
    ppu.line_dot = 104;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.mode2_scan_state.selected_sprite_count = 1;
    ppu.mode2_scan_state.selected_sprites[0] = Some(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 3,
        tile_index: 0,
        attributes: 0,
    });

    ppu.write_register(0xFF40, 0x92);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc0
            .bg_enable_visible_hold
            .override_value,
        Some(true)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc0
            .bg_enable_visible_hold
            .pixels_remaining,
        2
    );
    assert!(ppu.pixel_transfer_bg_enabled());

    ppu.consume_dmg_lcdc0_bg_enable_visible_hold();
    assert!(ppu.pixel_transfer_bg_enabled());

    ppu.consume_dmg_lcdc0_bg_enable_visible_hold();
    ppu.sync_visible_registers();
    assert!(!ppu.pixel_transfer_bg_enabled());
}

#[test]
fn dmg_single_left_sprite_lcdc0_second_write_waits_until_the_modeled_future_onset() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.lcdc = 0x92;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x92;
    ppu.pipeline_registers.lcdc = 0x92;
    ppu.line_dot = 116;
    ppu.bg_pipeline_state.visible_pixels_output = 12;
    ppu.bg_pipeline_state.current_transfer_x = 20;
    ppu.mode2_scan_state.selected_sprite_count = 1;
    ppu.mode2_scan_state.selected_sprites[0] = Some(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 3,
        tile_index: 0,
        attributes: 0,
    });
    ppu.dmg_panel_live_write_state
        .lcdc0
        .current_line_bg_enable_write_count = 1;

    ppu.write_register(0xFF40, 0x93);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc0
            .bg_enable_visible_hold
            .override_value,
        Some(false)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .lcdc0
            .bg_enable_visible_hold
            .pixels_remaining,
        2
    );
    assert!(!ppu.pixel_transfer_bg_enabled());

    ppu.consume_dmg_lcdc0_bg_enable_visible_hold();
    assert!(!ppu.pixel_transfer_bg_enabled());

    ppu.consume_dmg_lcdc0_bg_enable_visible_hold();
    ppu.sync_visible_registers();
    assert!(ppu.pixel_transfer_bg_enabled());
}

#[test]
fn bg_disabled_background_pixels_stay_white_during_retroactive_bgp_repaint() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x90,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x11,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    for _ in 0..4 {
        ppu.bg_pipeline_state.fifo.push_back(0);
        let _ = ppu.advance_mode3_output_phase();
    }

    assert_eq!(&ppu.framebuffer()[..4], &[0, 0, 0, 0]);

    ppu.write_register_with_source(0xFF47, 0x12, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[..4], &[0, 0, 0, 0]);
}

#[test]
fn bg_disabled_does_not_hide_obj_panel_output_on_dmg() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x92;
    ppu.pipeline_registers.lcdc = 0x93;
    ppu.visible_registers.bgp = 0x1B;
    ppu.visible_registers.obp0 = Some(0xE4);
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 2,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 8,
        oam_index: 0,
    });

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[0], 2);
    assert_eq!(ppu.framebuffer()[0], 2);
}

#[test]
fn first_visible_pixel_with_bg_disabled_still_consumes_the_bg_fifo() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x92;
    ppu.pipeline_registers.lcdc = 0x93;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);
    ppu.bg_pipeline_state
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                fetch_x: BG_TILE_WIDTH as u16,
                tile_low: 0xFF,
                tile_high: 0x00,
                ..BgCachedSlice::default()
            },
            0,
        )));
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 2,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 8,
        oam_index: 0,
    });

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[0], 2);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 9);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 1);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn later_visible_pixel_with_bg_disabled_still_consumes_the_bg_fifo() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x92;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 1;
    ppu.bg_pipeline_state.current_transfer_x = 9;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);
    ppu.bg_pipeline_state
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                fetch_x: BG_TILE_WIDTH as u16,
                tile_low: 0xFF,
                tile_high: 0x00,
                ..BgCachedSlice::default()
            },
            0,
        )));
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 2,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 9,
        oam_index: 0,
    });

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[1], 2);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 10);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 2);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn framebuffer_applies_obj_palette_selection_without_changing_logical_obj_colors() {
    let mut ppu = PpuTestRig::dmg();
    ppu.write_oam_entry_with_attributes(0, 16, 8, 0, 0x10);
    ppu.write_bg_tile_row(0, 0, 0xFF, 0x00);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x92,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.write_register(0xFF48, 0xE4);
    ppu.write_register(0xFF49, 0x0C);

    ppu.advance_until_hblank();

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[..8], &[1; 8]);
    assert_eq!(&ppu.framebuffer()[..8], &[3; 8]);
}

#[test]
fn dmg_bgp_write_during_mode3_recolors_recent_bg_pixels_with_transient_then_final_palette() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xB1,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = 200;
    ppu.bg_pipeline_state.visible_pixels_output = 8;
    ppu.current_scanline_mixed_pixels[4..8].fill(MixedPixel::background(0));
    ppu.framebuffer[4..8].fill(1);

    ppu.write_register(0xFF47, 0x00);

    assert_eq!(&ppu.framebuffer()[4..8], &[1, 0, 0, 0]);
}

#[test]
fn cpu_mmio_bgp_write_retroactively_recolors_recent_bg_color0_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xB1,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x11,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 25;
    ppu.bg_pipeline_state.visible_pixels_output = 25;
    ppu.bg_pipeline_state.current_transfer_x = 33;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    for _ in 0..5 {
        ppu.bg_pipeline_state.fifo.push_back(0);
    }
    ppu.current_scanline_mixed_pixels[21..25].fill(MixedPixel::background(0));
    ppu.framebuffer[21..25].fill(1);

    ppu.write_register_with_source(0xFF47, 0x12, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[21..25], &[3, 2, 2, 2]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 21,
            transient_palette: 0x13,
            repaint_visible_x: 29,
            transfer_lead_pixels: 0,
            value: 0x12,
        }]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x12)
    );

    ppu.sync_pipeline_registers();
    ppu.sync_visible_registers();
    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.framebuffer()[25], 2);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        0
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        None
    );
}

#[test]
fn cpu_mmio_bgp_write_uses_pipeline_delay_when_recent_pixels_are_not_all_bg_color0() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 25;
    ppu.bg_pipeline_state.visible_pixels_output = 25;
    ppu.bg_pipeline_state.current_transfer_x = 33;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    for _ in 0..5 {
        ppu.bg_pipeline_state.fifo.push_back(1);
    }
    ppu.current_scanline_mixed_pixels[21..25].fill(MixedPixel::background(1));
    ppu.framebuffer[21..25].fill(1);

    ppu.write_register_with_source(0xFF47, 0xAA, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[21..25], &[1, 1, 1, 1]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 21,
            transient_palette: 0xEE,
            repaint_visible_x: 29,
            transfer_lead_pixels: 0,
            value: 0xAA,
        }]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        4
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0xE4)
    );

    for _ in 0..5 {
        ppu.sync_pipeline_registers();
        ppu.sync_visible_registers();
        let _ = ppu.advance_mode3_output_phase();
    }

    assert_eq!(&ppu.framebuffer()[25..30], &[1, 1, 1, 1, 2]);
}

#[test]
fn cpu_mmio_bgp_write_ignores_recent_obj_pixels_when_bg_tail_is_color0() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x11,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 25;
    ppu.bg_pipeline_state.visible_pixels_output = 25;
    ppu.bg_pipeline_state.current_transfer_x = 33;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    for _ in 0..5 {
        ppu.bg_pipeline_state.fifo.push_back(0);
    }
    ppu.current_scanline_mixed_pixels[21] = MixedPixel::object(1, false);
    ppu.current_scanline_mixed_pixels[22..25].fill(MixedPixel::background(0));
    ppu.framebuffer[21] = 3;
    ppu.framebuffer[22..25].fill(1);

    ppu.write_register_with_source(0xFF47, 0x12, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[21..25], &[3, 3, 2, 2]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 21,
            transient_palette: 0x13,
            repaint_visible_x: 29,
            transfer_lead_pixels: 0,
            value: 0x12,
        }]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x12)
    );
}

#[test]
fn cpu_mmio_bgp_write_stays_delayed_when_recent_affected_bg_pixel_is_nonzero_even_with_obj_tail() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 25;
    ppu.bg_pipeline_state.visible_pixels_output = 25;
    ppu.bg_pipeline_state.current_transfer_x = 33;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    for _ in 0..5 {
        ppu.bg_pipeline_state.fifo.push_back(1);
    }
    ppu.current_scanline_mixed_pixels[21] = MixedPixel::object(1, false);
    ppu.current_scanline_mixed_pixels[22..25].fill(MixedPixel::background(1));
    ppu.framebuffer[21] = 3;
    ppu.framebuffer[22..25].fill(1);

    ppu.write_register_with_source(0xFF47, 0xAA, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[21..25], &[3, 1, 1, 1]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 21,
            transient_palette: 0xEE,
            repaint_visible_x: 29,
            transfer_lead_pixels: 0,
            value: 0xAA,
        }]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        4
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0xE4)
    );
}

#[test]
fn cpu_mmio_bgp_second_write_after_window_restart_backdates_the_recent_bg_tail() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x08,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 7;
    ppu.bg_pipeline_state.current_transfer_x = 15;
    ppu.bg_pipeline_state.visible_pixels_output = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.visible_registers.lcdc = 0xB1;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.wx = 0x08;
    ppu.pipeline_registers.wx = 0x08;
    ppu.visible_registers.bgp = 0x00;
    ppu.pipeline_registers.bgp = 0x00;
    ppu.bgp = 0x00;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(3);
    }
    ppu.current_scanline_mixed_pixels[..7].fill(MixedPixel::background(3));
    ppu.framebuffer[..7].fill(0);
    for visible_x in 3..7 {
        ppu.record_dmg_recent_panel_dot(visible_x, MixedPixel::background(3), false);
    }
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
        transient_visible_x: 0,
        transient_palette: 0x00,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x00,
    }];

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[..7], &[0, 0, 0, 3, 3, 3, 3]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![
            PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
                transient_visible_x: 0,
                transient_palette: 0x00,
                repaint_visible_x: 4,
                transfer_lead_pixels: 0,
                value: 0x00,
            },
            PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
                transient_visible_x: 3,
                transient_palette: 0xFF,
                repaint_visible_x: 4,
                transfer_lead_pixels: 0,
                value: 0xFF,
            },
        ]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0xFF)
    );

    ppu.sync_pipeline_registers();
    ppu.sync_visible_registers();
    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.framebuffer()[7], 3);
}

#[test]
fn cpu_mmio_bgp_first_write_after_window_restart_backdates_the_recent_bg_tail_to_the_final_palette()
{
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFF,
        wy: 0x00,
        wx: 0x08,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 1;
    ppu.bg_pipeline_state.current_transfer_x = 9;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.visible_registers.lcdc = 0xB1;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.wx = 0x08;
    ppu.pipeline_registers.wx = 0x08;
    ppu.visible_registers.bgp = 0xFF;
    ppu.pipeline_registers.bgp = 0xFF;
    ppu.bgp = 0xFF;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(3);
    }
    ppu.current_scanline_mixed_pixels[0] = MixedPixel::background(1);
    ppu.framebuffer[0] = 3;
    ppu.record_dmg_recent_panel_dot(0, MixedPixel::background(1), false);

    ppu.write_register_with_source(0xFF47, 0x00, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(ppu.framebuffer()[0], 0);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 0,
            transient_palette: 0x00,
            repaint_visible_x: 4,
            transfer_lead_pixels: 0,
            value: 0x00,
        }]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x00)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        2
    );
}

#[test]
fn cpu_mmio_bgp_second_write_after_window_restart_backdates_even_if_the_first_write_was_retroactive()
 {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x08,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 7;
    ppu.bg_pipeline_state.current_transfer_x = 15;
    ppu.bg_pipeline_state.visible_pixels_output = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.visible_registers.lcdc = 0xB1;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.bgp = 0x00;
    ppu.pipeline_registers.bgp = 0x00;
    ppu.bgp = 0x00;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(3);
    }
    ppu.current_scanline_mixed_pixels[0] = MixedPixel::background(0);
    ppu.current_scanline_mixed_pixels[1..7].fill(MixedPixel::background(3));
    ppu.framebuffer[0] = 3;
    ppu.framebuffer[1..7].fill(0);
    for visible_x in 3..7 {
        ppu.record_dmg_recent_panel_dot(visible_x, MixedPixel::background(3), false);
    }
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
        transient_visible_x: 0,
        transient_palette: 0x00,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x00,
    }];

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[..7], &[3, 0, 0, 3, 3, 3, 3]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes[1],
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 3,
            transient_palette: 0xFF,
            repaint_visible_x: 4,
            transfer_lead_pixels: 0,
            value: 0xFF,
        }
    );
}

#[test]
fn cpu_mmio_bgp_second_write_after_window_restart_uses_scanline_positions_when_panel_history_stalls()
 {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x12,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 11;
    ppu.bg_pipeline_state.current_transfer_x = 19;
    ppu.bg_pipeline_state.visible_pixels_output = 11;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.visible_registers.lcdc = 0xB1;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.bgp = 0x00;
    ppu.pipeline_registers.bgp = 0x00;
    ppu.visible_registers.wx = 0x12;
    ppu.pipeline_registers.wx = 0x12;
    ppu.bgp = 0x00;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(3);
    }
    ppu.current_scanline_mixed_pixels[..11].fill(MixedPixel::background(1));
    ppu.framebuffer[..11].fill(0);
    for &visible_x in &[7, 8, 10, 10] {
        ppu.record_dmg_recent_panel_dot(visible_x, MixedPixel::background(1), false);
    }
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
        transient_visible_x: 0,
        transient_palette: 0x00,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x00,
    }];

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[..11], &[0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes[1],
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 9,
            transient_palette: 0xFF,
            repaint_visible_x: 10,
            transfer_lead_pixels: 0,
            value: 0xFF,
        }
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0xFF)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
}

#[test]
fn cpu_mmio_bgp_first_write_before_window_restart_waits_for_the_clamped_window_onset() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFF,
        wy: 0x00,
        wx: 0x15,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 1;
    ppu.bg_pipeline_state.current_transfer_x = 9;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.visible_registers.lcdc = 0xB1;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.bgp = 0xFF;
    ppu.pipeline_registers.bgp = 0xFF;
    ppu.visible_registers.wx = 0x15;
    ppu.pipeline_registers.wx = 0x15;
    ppu.bgp = 0xFF;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(1);
    }
    ppu.current_scanline_mixed_pixels[0] = MixedPixel::background(1);
    ppu.framebuffer[0] = 0;
    ppu.record_dmg_recent_panel_dot(0, MixedPixel::background(1), false);

    ppu.write_register_with_source(0xFF47, 0x00, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 0,
            transient_palette: 0x00,
            repaint_visible_x: 10,
            transfer_lead_pixels: 0,
            value: 0x00,
        }]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        8
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x00)
    );
}

#[test]
fn cpu_mmio_bgp_second_write_before_window_restart_backdates_to_the_clamped_onset() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x15,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 13;
    ppu.bg_pipeline_state.current_transfer_x = 21;
    ppu.bg_pipeline_state.visible_pixels_output = 13;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.visible_registers.lcdc = 0xB1;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.bgp = 0x00;
    ppu.pipeline_registers.bgp = 0x00;
    ppu.visible_registers.wx = 0x15;
    ppu.pipeline_registers.wx = 0x15;
    ppu.bgp = 0x00;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(3);
    }
    ppu.current_scanline_mixed_pixels[..13].fill(MixedPixel::background(1));
    ppu.framebuffer[..13].fill(0);
    for visible_x in 9..13 {
        ppu.record_dmg_recent_panel_dot(visible_x, MixedPixel::background(1), false);
    }
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
        transient_visible_x: 0,
        transient_palette: 0x00,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x00,
    }];

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        &ppu.framebuffer()[..13],
        &[0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes[1],
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 9,
            transient_palette: 0xFF,
            repaint_visible_x: 10,
            transfer_lead_pixels: 0,
            value: 0xFF,
        }
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0xFF)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
}

#[test]
fn cpu_mmio_bgp_second_write_after_wx0_window_restart_uses_the_first_window_row_onset() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 15;
    ppu.bg_pipeline_state.current_transfer_x = 23;
    ppu.bg_pipeline_state.visible_pixels_output = 15;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.visible_registers.lcdc = 0xB1;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.bgp = 0x00;
    ppu.pipeline_registers.bgp = 0x00;
    ppu.visible_registers.wx = 0x00;
    ppu.pipeline_registers.wx = 0x00;
    ppu.bgp = 0x00;
    ppu.window_state.window_line_counter = 0;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(3);
    }
    ppu.current_scanline_mixed_pixels[..15].fill(MixedPixel::background(1));
    ppu.framebuffer[..15].fill(0);
    for visible_x in 10..15 {
        ppu.record_dmg_recent_panel_dot(visible_x, MixedPixel::background(1), false);
    }
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
        transient_visible_x: 0,
        transient_palette: 0xFF,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x00,
    }];

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        &ppu.framebuffer()[..15],
        &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes[1],
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 11,
            transient_palette: 0xFF,
            repaint_visible_x: 12,
            transfer_lead_pixels: 0,
            value: 0xFF,
        }
    );
}

#[test]
fn cpu_mmio_bgp_second_write_after_wx0_window_restart_caps_the_row_onset_by_the_arrival_dot() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 7;
    ppu.bg_pipeline_state.current_transfer_x = 15;
    ppu.bg_pipeline_state.visible_pixels_output = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.visible_registers.lcdc = 0xB1;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.bgp = 0x00;
    ppu.pipeline_registers.bgp = 0x00;
    ppu.visible_registers.wx = 0x00;
    ppu.pipeline_registers.wx = 0x00;
    ppu.bgp = 0x00;
    ppu.window_state.window_line_counter = 0;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(3);
    }
    ppu.current_scanline_mixed_pixels[..7].fill(MixedPixel::background(1));
    ppu.framebuffer[..7].fill(0);
    for visible_x in 3..7 {
        ppu.record_dmg_recent_panel_dot(visible_x, MixedPixel::background(1), false);
    }
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
        transient_visible_x: 0,
        transient_palette: 0x00,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x00,
    }];

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[..7], &[0, 0, 0, 3, 3, 3, 3]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes[1],
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 3,
            transient_palette: 0xFF,
            repaint_visible_x: 4,
            transfer_lead_pixels: 0,
            value: 0xFF,
        }
    );
}

#[test]
fn cpu_mmio_bgp_second_write_after_wx0_window_restart_uses_the_active_window_tile_row() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 20;
    ppu.bg_pipeline_state.current_transfer_x = 28;
    ppu.bg_pipeline_state.visible_pixels_output = 20;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_active_line_counter = 7;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.visible_registers.lcdc = 0xB1;
    ppu.pipeline_registers.lcdc = 0xB1;
    ppu.visible_registers.bgp = 0x00;
    ppu.pipeline_registers.bgp = 0x00;
    ppu.visible_registers.wx = 0x00;
    ppu.pipeline_registers.wx = 0x00;
    ppu.bgp = 0x00;
    ppu.window_state.window_line_counter = 0;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(3);
    }
    ppu.current_scanline_mixed_pixels[..20].fill(MixedPixel::background(1));
    ppu.framebuffer[..20].fill(0);
    for visible_x in 15..20 {
        ppu.record_dmg_recent_panel_dot(visible_x, MixedPixel::background(1), false);
    }
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
        transient_visible_x: 0,
        transient_palette: 0x00,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x00,
    }];

    assert_eq!(ppu.current_window_line_counter(), 7);

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        &ppu.framebuffer()[..20],
        &[0, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes[1],
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 3,
            transient_palette: 0xFF,
            repaint_visible_x: 4,
            transfer_lead_pixels: 0,
            value: 0xFF,
        }
    );
}

fn dmg_bgp_pipeline_delayed_burst_rig() -> Ppu {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 25;
    ppu.bg_pipeline_state.visible_pixels_output = 25;
    ppu.bg_pipeline_state.current_transfer_x = 33;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    for _ in 0..20 {
        ppu.bg_pipeline_state.fifo.push_back(1);
    }
    ppu.current_scanline_mixed_pixels[21..25].fill(MixedPixel::background(1));
    ppu.framebuffer[21..25].fill(1);
    ppu
}

#[test]
fn cpu_mmio_bgp_burst_without_visible_progress_reuses_the_same_pipeline_delayed_window() {
    let mut ppu = dmg_bgp_pipeline_delayed_burst_rig();

    ppu.write_register_with_source(0xFF47, 0xAA, PpuRegisterWriteSource::CpuMmioCommit);
    ppu.write_register_with_source(0xFF47, 0x03, PpuRegisterWriteSource::CpuMmioCommit);
    ppu.write_register_with_source(0xFF47, 0x12, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![
            PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
                transient_visible_x: 21,
                transient_palette: 0xEE,
                repaint_visible_x: 29,
                transfer_lead_pixels: 0,
                value: 0xAA,
            },
            PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
                transient_visible_x: 21,
                transient_palette: 0xE7,
                repaint_visible_x: 29,
                transfer_lead_pixels: 0,
                value: 0x03,
            },
            PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
                transient_visible_x: 21,
                transient_palette: 0xF6,
                repaint_visible_x: 29,
                transfer_lead_pixels: 0,
                value: 0x12,
            },
        ]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        4
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0xE4)
    );
    assert_eq!(&ppu.framebuffer()[21..25], &[1, 1, 1, 1]);
}

#[test]
fn cpu_mmio_bgp_burst_with_visible_progress_slides_the_pipeline_delayed_window_forward() {
    let mut ppu = dmg_bgp_pipeline_delayed_burst_rig();

    ppu.write_register_with_source(0xFF47, 0xAA, PpuRegisterWriteSource::CpuMmioCommit);
    for _ in 0..4 {
        ppu.sync_pipeline_registers();
        ppu.sync_visible_registers();
        let _ = ppu.advance_mode3_output_phase();
    }

    ppu.write_register_with_source(0xFF47, 0x03, PpuRegisterWriteSource::CpuMmioCommit);
    for _ in 0..4 {
        ppu.sync_pipeline_registers();
        ppu.sync_visible_registers();
        let _ = ppu.advance_mode3_output_phase();
    }

    ppu.write_register_with_source(0xFF47, 0xFC, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![
            PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
                transient_visible_x: 21,
                transient_palette: 0xEE,
                repaint_visible_x: 29,
                transfer_lead_pixels: 0,
                value: 0xAA,
            },
            PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
                transient_visible_x: 25,
                transient_palette: 0xAB,
                repaint_visible_x: 33,
                transfer_lead_pixels: 0,
                value: 0x03,
            },
            PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
                transient_visible_x: 29,
                transient_palette: 0xFF,
                repaint_visible_x: 37,
                transfer_lead_pixels: 0,
                value: 0xFC,
            },
        ]
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        4
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x03)
    );

    for _ in 0..4 {
        ppu.sync_pipeline_registers();
        ppu.sync_visible_registers();
        let _ = ppu.advance_mode3_output_phase();
    }
    ppu.sync_pipeline_registers();
    ppu.sync_visible_registers();
    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(
        &ppu.framebuffer()[25..38],
        &[1, 1, 1, 1, 2, 2, 2, 2, 0, 0, 0, 0, 3]
    );
}

#[test]
fn cpu_mmio_bgp_write_before_first_visible_bg_pixel_uses_the_visible_obj_prefix_as_delay() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 20;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.current_transfer_x = 6;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    for _ in 0..8 {
        ppu.bg_pipeline_state.fifo.push_back(0);
    }

    let sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 4,
        tile_index: 0,
        attributes: 0,
    };
    ppu.push_obj_pixels(sprite, 0x3C, 0x00, 0);

    ppu.write_register_with_source(0xFF47, 0x03, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        2
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0xFC)
    );
}

#[test]
fn cpu_mmio_bgp_write_after_four_leading_obj_pixels_uses_the_committed_palette_on_the_current_dot()
{
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x03,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 32;
    ppu.bg_pipeline_state.visible_pixels_output = 12;
    ppu.bg_pipeline_state.current_transfer_x = 20;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[..4].fill(MixedPixel::object(1, false));
    ppu.current_scanline_mixed_pixels[4..12].fill(MixedPixel::background(0));
    ppu.framebuffer[4..12].fill(3);
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
        transient_visible_x: 0,
        transient_palette: 0x03,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x03,
    }];

    ppu.write_register_with_source(0xFF47, 0x01, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x01)
    );

    ppu.sync_pipeline_registers();
    ppu.sync_visible_registers();
    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.framebuffer()[12], 1);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        0
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        None
    );
}

#[test]
fn cpu_mmio_bgp_write_after_the_first_selected_retroactive_write_keeps_the_recent_tail_and_flips_on_the_current_dot()
 {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 96;
    ppu.bg_pipeline_state.visible_pixels_output = 50;
    ppu.bg_pipeline_state.current_transfer_x = 58;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[..2].fill(MixedPixel::object(1, false));
    ppu.current_scanline_mixed_pixels[2..50].fill(MixedPixel::background(0));
    ppu.framebuffer[46..50].fill(1);
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 0,
            transient_palette: 0x03,
            repaint_visible_x: 4,
            transfer_lead_pixels: 0,
            value: 0x03,
        },
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 6,
            transient_palette: 0x03,
            repaint_visible_x: 14,
            transfer_lead_pixels: 0,
            value: 0x01,
        },
    ];

    ppu.write_register_with_source(0xFF47, 0x03, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[46..50], &[1, 1, 1, 1]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient,
            transient_visible_x: 50,
            transient_palette: 0x03,
            repaint_visible_x: 51,
            transfer_lead_pixels: 0,
            value: 0x03,
        })
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x03)
    );

    ppu.sync_pipeline_registers();
    ppu.sync_visible_registers();
    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(&ppu.framebuffer()[46..50], &[1, 1, 1, 1]);
    assert_eq!(ppu.framebuffer()[50], 3);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        0
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        None
    );
}

#[test]
fn cpu_mmio_bgp_write_with_a_single_left_sprite_can_backdate_the_first_write_onset() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 32;
    ppu.bg_pipeline_state.visible_pixels_output = 10;
    ppu.bg_pipeline_state.current_transfer_x = 18;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[..10].fill(MixedPixel::background(0));
    ppu.framebuffer[..10].fill(1);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });

    ppu.write_register_with_source(0xFF47, 0x00, PpuRegisterWriteSource::CpuMmioCommit);

    assert!(ppu.framebuffer()[..10].iter().all(|&shade| shade == 0));
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 0,
            transient_palette: 0x00,
            repaint_visible_x: 1,
            transfer_lead_pixels: 0,
            value: 0x00,
        }]
    );
}

#[test]
fn cpu_mmio_bgp_write_with_a_single_left_sprite_does_not_repaint_the_whole_prefix_late_in_the_line()
{
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 308;
    ppu.bg_pipeline_state.visible_pixels_output = 148;
    ppu.bg_pipeline_state.current_transfer_x = 156;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[..148].fill(MixedPixel::background(0));
    ppu.framebuffer[..148].fill(0);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 3,
        tile_index: 0,
        attributes: 0,
    });

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert!(ppu.framebuffer()[..24].iter().all(|&shade| shade == 0));
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 149,
            transient_palette: 0xFF,
            repaint_visible_x: 150,
            transfer_lead_pixels: 0,
            value: 0xFF,
        })
    );
}

#[test]
fn cpu_mmio_bgp_late_black_pulse_with_single_left_sprite_uses_the_variant_tail_onset_table() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 308;
    ppu.bg_pipeline_state.visible_pixels_output = 148;
    ppu.bg_pipeline_state.current_transfer_x = 156;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[..148].fill(MixedPixel::background(0));
    ppu.framebuffer[..148].fill(0);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 3,
        tile_index: 0,
        attributes: 0,
    });

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 149,
            transient_palette: 0xFF,
            repaint_visible_x: 150,
            transfer_lead_pixels: 0,
            value: 0xFF,
        })
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x00)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
}

#[test]
fn cpu_mmio_bgp_late_black_pulse_with_sprite_x0_tracks_the_late_visible_position() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 304;
    ppu.bg_pipeline_state.visible_pixels_output = 152;
    ppu.bg_pipeline_state.current_transfer_x = 160;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[..152].fill(MixedPixel::background(0));
    ppu.framebuffer[..152].fill(0);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 146,
            transient_palette: 0xFF,
            repaint_visible_x: 147,
            transfer_lead_pixels: 0,
            value: 0xFF,
        })
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0xFF)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
}

#[test]
fn cpu_mmio_bgp_late_black_pulse_with_sprite_x16_extends_the_tail_window() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 320;
    ppu.bg_pipeline_state.visible_pixels_output = 154;
    ppu.bg_pipeline_state.current_transfer_x = 162;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[..154].fill(MixedPixel::background(0));
    ppu.framebuffer[..154].fill(0);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 16,
        tile_index: 0,
        attributes: 0,
    });

    ppu.write_register_with_source(0xFF47, 0xFF, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 156,
            transient_palette: 0xFF,
            repaint_visible_x: 157,
            transfer_lead_pixels: 0,
            value: 0xFF,
        })
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        2
    );
}

#[test]
fn cpu_mmio_bgp_second_write_with_sprite_x15_recolors_the_left_edge_boundary_dot() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 48;
    ppu.bg_pipeline_state.visible_pixels_output = 13;
    ppu.bg_pipeline_state.current_transfer_x = 21;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[12] = MixedPixel::background(0);
    ppu.framebuffer[12] = 1;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 15,
        tile_index: 0,
        attributes: 0,
    });
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes
        .push(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 9,
            transient_palette: 0x01,
            repaint_visible_x: 10,
            transfer_lead_pixels: 0,
            value: 0x01,
        });

    ppu.write_register_with_source(0xFF47, 0x00, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(ppu.framebuffer()[12], 0);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 12,
            transient_palette: 0x01,
            repaint_visible_x: 12,
            transfer_lead_pixels: 0,
            value: 0x00,
        })
    );
}

#[test]
fn cpu_mmio_bgp_write_with_single_left_sprite_on_the_current_dot_uses_current_dot_transient() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 20;
    ppu.bg_pipeline_state.visible_pixels_output = 5;
    ppu.bg_pipeline_state.current_transfer_x = 13;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[..5].fill(MixedPixel::background(0));
    ppu.framebuffer[..5].fill(1);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 13,
        tile_index: 0,
        attributes: 0,
    });

    ppu.write_register_with_source(0xFF47, 0x00, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient,
            transient_visible_x: 5,
            transient_palette: 0x00,
            repaint_visible_x: 6,
            transfer_lead_pixels: 0,
            value: 0x00,
        })
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x00)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
    assert_eq!(&ppu.framebuffer()[..5], &[1, 1, 1, 1, 1]);
}

#[test]
fn cpu_mmio_bgp_write_with_single_left_sprite_can_keep_a_late_write_pipeline_delayed() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 84;
    ppu.bg_pipeline_state.visible_pixels_output = 45;
    ppu.bg_pipeline_state.current_transfer_x = 53;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[41..45].fill(MixedPixel::background(0));
    ppu.framebuffer[41..45].fill(3);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 9,
        tile_index: 0,
        attributes: 0,
    });
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 0,
            transient_palette: 0xFC,
            repaint_visible_x: 4,
            transfer_lead_pixels: 0,
            value: 0xFC,
        },
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 12,
            transient_palette: 0xFC,
            repaint_visible_x: 16,
            transfer_lead_pixels: 0,
            value: 0xFC,
        },
    ];

    ppu.write_register_with_source(0xFF47, 0x03, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 47,
            transient_palette: 0x03,
            repaint_visible_x: 48,
            transfer_lead_pixels: 0,
            value: 0x03,
        })
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0xFC)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        2
    );
    assert_eq!(&ppu.framebuffer()[41..45], &[3, 3, 3, 3]);
}

#[test]
fn cpu_mmio_bgp_second_write_with_single_left_sprite_covers_the_x14_clamp_and_x16_window() {
    let mut clamped = Ppu::new(ConsoleModel::GameBoy);
    clamped.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    clamped.visible_output = PpuVisibleOutputState::Driving;
    clamped.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 48;
    clamped.bg_pipeline_state.visible_pixels_output = 13;
    clamped.bg_pipeline_state.current_transfer_x = 21;
    clamped.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    clamped.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    clamped.bg_pipeline_state.fifo.push_back(0);
    clamped.current_scanline_mixed_pixels[12] = MixedPixel::background(0);
    clamped.framebuffer[12] = 1;
    clamped.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 14,
        tile_index: 0,
        attributes: 0,
    });
    clamped
        .dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes
        .push(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 9,
            transient_palette: 0x01,
            repaint_visible_x: 10,
            transfer_lead_pixels: 0,
            value: 0x01,
        });

    clamped.write_register_with_source(0xFF47, 0x00, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        clamped
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 12,
            transient_palette: 0x01,
            repaint_visible_x: 12,
            transfer_lead_pixels: 0,
            value: 0x00,
        })
    );
    assert_eq!(clamped.framebuffer()[12], 0);

    let mut windowed = Ppu::new(ConsoleModel::GameBoy);
    windowed.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    windowed.visible_output = PpuVisibleOutputState::Driving;
    windowed.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 52;
    windowed.bg_pipeline_state.visible_pixels_output = 13;
    windowed.bg_pipeline_state.current_transfer_x = 21;
    windowed.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    windowed.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    windowed.bg_pipeline_state.fifo.push_back(0);
    windowed.current_scanline_mixed_pixels[5..13].fill(MixedPixel::background(0));
    windowed.framebuffer[5..13].fill(0);
    windowed.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 16,
        tile_index: 0,
        attributes: 0,
    });
    windowed
        .dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes
        .push(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 4,
            transient_palette: 0x01,
            repaint_visible_x: 5,
            transfer_lead_pixels: 0,
            value: 0x01,
        });

    windowed.write_register_with_source(0xFF47, 0x00, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&windowed.framebuffer()[5..13], &[1, 1, 1, 0, 0, 0, 0, 0]);
    assert_eq!(
        windowed
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 5,
            transient_palette: 0x01,
            repaint_visible_x: 8,
            transfer_lead_pixels: 0,
            value: 0x00,
        })
    );
}

#[test]
fn cpu_mmio_bgp_second_write_with_single_left_sprite_waits_for_the_transient_window() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 16;
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.bg_pipeline_state.current_transfer_x = 12;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    for _ in 0..5 {
        ppu.bg_pipeline_state.fifo.push_back(0);
    }
    ppu.current_scanline_mixed_pixels[..4].fill(MixedPixel::background(0));
    ppu.framebuffer[..4].fill(1);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 16,
        tile_index: 0,
        attributes: 0,
    });
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes
        .push(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 4,
            transient_palette: 0x01,
            repaint_visible_x: 5,
            transfer_lead_pixels: 0,
            value: 0x01,
        });

    ppu.write_register_with_source(0xFF47, 0x00, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .last()
            .copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 5,
            transient_palette: 0x01,
            repaint_visible_x: 8,
            transfer_lead_pixels: 0,
            value: 0x00,
        })
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        Some(0x01)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        1
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_followup_palette_override,
        Some(0x01)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_followup_pixels_remaining,
        3
    );

    for _ in 0..5 {
        ppu.sync_pipeline_registers();
        ppu.sync_visible_registers();
        let _ = ppu.advance_mode3_output_phase();
    }

    assert_eq!(&ppu.framebuffer()[4..9], &[1, 1, 1, 1, 0]);
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        None
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        0
    );
}

#[test]
fn cpu_mmio_bgp_write_with_future_obj_pixels_starts_and_consumes_the_bg_visible_hold() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x11,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 12;
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.bg_pipeline_state.current_transfer_x = 12;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.current_scanline_mixed_pixels[..4].fill(MixedPixel::background(0));
    ppu.framebuffer[..4].fill(1);
    ppu.obj_pipeline_state
        .fifo
        .push_back(ObjPixel::transparent());
    ppu.obj_pipeline_state
        .fifo
        .push_back(ObjPixel::transparent());
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 1,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 12,
        oam_index: 0,
    });
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 2,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 13,
        oam_index: 0,
    });

    ppu.write_register_with_source(0xFF47, 0x12, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_palette_override,
        Some(0x12)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_bg_pixels_remaining,
        5
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_fallback_palette,
        Some(0x12)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        None
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        0
    );

    ppu.consume_dmg_bgp_cpu_commit_bg_visible_hold(MixedPixel::object(1, false));
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_bg_pixels_remaining,
        5
    );

    for remaining in (1..=5).rev() {
        ppu.consume_dmg_bgp_cpu_commit_bg_visible_hold(MixedPixel::background(0));
        assert_eq!(
            ppu.dmg_panel_live_write_state
                .bgp_cpu_commit
                .bg_visible_hold_bg_pixels_remaining,
            remaining - 1
        );
    }

    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_palette_override,
        Some(0x12)
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_fallback_palette,
        None
    );
}

#[test]
fn dmg_recent_panel_dot_history_drives_the_bgp_panel_path() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.bg_pipeline_state.visible_pixels_output = 7;
    ppu.bg_pipeline_state.current_transfer_x = 15;
    ppu.dmg_panel_live_write_state
        .recent_panel_dots
        .push_back(PpuRecentPanelDot {
            visible_x: 4,
            pixel: MixedPixel::background(0),
            dmg_bg_forced_white: false,
        });
    ppu.dmg_panel_live_write_state
        .recent_panel_dots
        .push_back(PpuRecentPanelDot {
            visible_x: 5,
            pixel: MixedPixel::object(1, false),
            dmg_bg_forced_white: false,
        });
    ppu.dmg_panel_live_write_state
        .recent_panel_dots
        .push_back(PpuRecentPanelDot {
            visible_x: 6,
            pixel: MixedPixel::background(0),
            dmg_bg_forced_white: false,
        });

    assert_eq!(
        ppu.dmg_bgp_cpu_commit_effect_kind(3),
        PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
    );

    ppu.dmg_panel_live_write_state.recent_panel_dots.pop_back();
    ppu.dmg_panel_live_write_state
        .recent_panel_dots
        .push_back(PpuRecentPanelDot {
            visible_x: 6,
            pixel: MixedPixel::background(1),
            dmg_bg_forced_white: false,
        });

    assert_eq!(
        ppu.dmg_bgp_cpu_commit_effect_kind(3),
        PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
    );

    ppu.dmg_panel_live_write_state.recent_panel_dots.pop_back();
    ppu.dmg_panel_live_write_state
        .recent_panel_dots
        .push_back(PpuRecentPanelDot {
            visible_x: 6,
            pixel: MixedPixel::background(0),
            dmg_bg_forced_white: false,
        });
    ppu.framebuffer[4] = 1;
    ppu.framebuffer[5] = 0;
    ppu.framebuffer[6] = 1;

    ppu.retroactively_recolor_recent_pixels(PpuPaletteRegister::Bgp, 0x03, 0x02, 3, false);

    assert_eq!(ppu.framebuffer()[4], 3);
    assert_eq!(ppu.framebuffer()[5], 0);
    assert_eq!(ppu.framebuffer()[6], 2);
}

#[test]
fn dmg_bgp_cpu_commit_uses_the_panel_path_at_visible_line_start_before_transfer_x_advances() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.current_transfer_x = 0;

    assert_eq!(
        ppu.dmg_bgp_cpu_commit_effect_kind(4),
        PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
    );
}

#[test]
fn dmg_bgp_cpu_commit_keeps_the_delayed_path_once_transfer_x_has_advanced_past_line_start() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.current_transfer_x = 1;

    assert_eq!(
        ppu.dmg_bgp_cpu_commit_effect_kind(4),
        PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
    );
}

#[test]
fn finalize_dmg_bgp_cpu_commit_scanline_recolors_the_previous_boundary_without_selected_sprites() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 8;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.framebuffer[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH].fill(2);
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
        transient_visible_x: 0,
        transient_palette: 0x11,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x11,
    }];

    ppu.finalize_dmg_bgp_cpu_commit_scanline();

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert!(row[..4].iter().all(|&shade| shade == 0));
    assert!(row[4..].iter().all(|&shade| shade == 1));
    assert_eq!(ppu.previous_scanline_ly, Some(8));
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .previous_line_writes,
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
    );
}

#[test]
fn finalize_dmg_bgp_cpu_commit_scanline_skips_previous_boundary_repaint_with_selected_sprites() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 8;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.framebuffer[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH].fill(2);
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
        transient_visible_x: 0,
        transient_palette: 0x11,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x11,
    }];
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });

    ppu.finalize_dmg_bgp_cpu_commit_scanline();

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert!(row.iter().all(|&shade| shade == 2));
    assert_eq!(ppu.previous_scanline_ly, Some(8));
}

#[test]
fn dmg_bgp_row_family_change_handles_selected_current_retroactive_and_current_dot_transient_writes()
{
    let ppu = Ppu::new(ConsoleModel::GameBoy);
    let retroactive_writes = [
        PpuDmgBgpBoundaryRepaintWrite {
            write: PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
                transient_visible_x: 0,
                transient_palette: 0x11,
                repaint_visible_x: 4,
                transfer_lead_pixels: 0,
                value: 0x11,
            },
            selected_current: true,
        },
        PpuDmgBgpBoundaryRepaintWrite {
            write: PpuDmgBgpCpuCommitWrite {
                effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
                transient_visible_x: 6,
                transient_palette: 0x13,
                repaint_visible_x: 14,
                transfer_lead_pixels: 0,
                value: 0x12,
            },
            selected_current: true,
        },
    ];
    assert_eq!(
        ppu.dmg_bgp_cpu_commit_palette_for_visible_x(
            0x10,
            &retroactive_writes,
            6,
            true,
            false,
            Some(4)
        ),
        0x13
    );
    assert_eq!(
        ppu.dmg_bgp_cpu_commit_palette_for_visible_x(
            0x10,
            &retroactive_writes,
            10,
            true,
            false,
            Some(4)
        ),
        0x11
    );
    assert_eq!(
        ppu.dmg_bgp_cpu_commit_palette_for_visible_x(
            0x10,
            &retroactive_writes,
            11,
            true,
            false,
            Some(4)
        ),
        0x12
    );

    let transient_writes = [PpuDmgBgpBoundaryRepaintWrite {
        write: PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient,
            transient_visible_x: 6,
            transient_palette: 0x13,
            repaint_visible_x: 8,
            transfer_lead_pixels: 0,
            value: 0x12,
        },
        selected_current: true,
    }];
    assert_eq!(
        ppu.dmg_bgp_cpu_commit_palette_for_visible_x(
            0x10,
            &transient_writes,
            5,
            false,
            false,
            None
        ),
        0x10
    );
    assert_eq!(
        ppu.dmg_bgp_cpu_commit_palette_for_visible_x(
            0x10,
            &transient_writes,
            6,
            false,
            false,
            None
        ),
        0x13
    );
    assert_eq!(
        ppu.dmg_bgp_cpu_commit_palette_for_visible_x(
            0x10,
            &transient_writes,
            7,
            false,
            false,
            None
        ),
        0x10
    );
    assert_eq!(
        ppu.dmg_bgp_cpu_commit_palette_for_visible_x(
            0x10,
            &transient_writes,
            8,
            false,
            false,
            None
        ),
        0x12
    );
}

#[test]
fn dmg_bgp_row_family_change_recolors_the_previous_boundary_scanline() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 1,
            transient_palette: 0x11,
            repaint_visible_x: 1,
            transfer_lead_pixels: 0,
            value: 0x11,
        },
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 13,
            transient_palette: 0x11,
            repaint_visible_x: 13,
            transfer_lead_pixels: 0,
            value: 0x10,
        },
    ];

    ppu.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(7, false);

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert_eq!(row[0], 0);
    assert!(row[1..13].iter().all(|&shade| shade == 1));
    assert!(row[13..].iter().all(|&shade| shade == 0));
}

#[test]
fn dmg_bgp_row_family_change_keeps_lcdc0_forced_white_previous_boundary_pixels_white() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.previous_scanline_dmg_bg_forced_white[1..4].fill(true);
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 1,
            transient_palette: 0x11,
            repaint_visible_x: 1,
            transfer_lead_pixels: 0,
            value: 0x11,
        },
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 13,
            transient_palette: 0x11,
            repaint_visible_x: 13,
            transfer_lead_pixels: 0,
            value: 0x10,
        },
    ];

    ppu.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(7, false);

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert_eq!(&row[..4], &[0, 0, 0, 0]);
    assert!(row[4..13].iter().all(|&shade| shade == 1));
    assert!(row[13..].iter().all(|&shade| shade == 0));
}

#[test]
fn dmg_bgp_row_family_change_can_optionally_consume_retroactive_panel_writes() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 1,
            transient_palette: 0x11,
            repaint_visible_x: 1,
            transfer_lead_pixels: 0,
            value: 0x11,
        },
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 13,
            transient_palette: 0x11,
            repaint_visible_x: 13,
            transfer_lead_pixels: 0,
            value: 0x10,
        },
    ];

    ppu.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(7, true);

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert_eq!(row[0], 0);
    assert!(row[1..13].iter().all(|&shade| shade == 1));
    assert_eq!(row[13], 1);
    assert!(row[14..].iter().all(|&shade| shade == 0));
}

#[test]
fn dmg_bgp_row_family_change_uses_transient_x_for_optional_retroactive_panel_repaint() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
        transient_visible_x: 2,
        transient_palette: 0x13,
        repaint_visible_x: 10,
        transfer_lead_pixels: 0,
        value: 0x12,
    }];

    ppu.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(7, true);

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert!(row[..2].iter().all(|&shade| shade == 0));
    assert_eq!(row[2], 3);
    assert!(row[3..].iter().all(|&shade| shade == 2));
}

#[test]
fn dmg_bgp_row_family_change_ignores_transfer_lead_for_optional_retroactive_repaint() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
        transient_visible_x: 2,
        transient_palette: 0x13,
        repaint_visible_x: 10,
        transfer_lead_pixels: 4,
        value: 0x12,
    }];

    ppu.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(7, true);

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert!(row[..2].iter().all(|&shade| shade == 0));
    assert_eq!(row[2], 3);
    assert!(row[3..].iter().all(|&shade| shade == 2));
}

#[test]
fn dmg_bgp_row_family_change_allows_zero_start_optional_retroactive_repaint_for_bg_prefix() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
        transient_visible_x: 0,
        transient_palette: 0x11,
        repaint_visible_x: 6,
        transfer_lead_pixels: 0,
        value: 0x11,
    }];

    ppu.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(7, true);

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert!(row.iter().all(|&shade| shade == 1));
}

#[test]
fn dmg_bgp_row_family_change_skips_zero_start_optional_retroactive_repaint_for_obj_prefix() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.previous_scanline_mixed_pixels[..4].fill(MixedPixel::object(1, false));
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
        transient_visible_x: 0,
        transient_palette: 0x11,
        repaint_visible_x: 6,
        transfer_lead_pixels: 0,
        value: 0x11,
    }];

    ppu.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(7, true);

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert!(row[4..].iter().all(|&shade| shade == 0));
}

#[test]
fn dmg_bgp_row_family_change_skips_optional_retroactive_repaint_that_precedes_delayed_boundary() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0x10;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 0,
            transient_palette: 0x11,
            repaint_visible_x: 4,
            transfer_lead_pixels: 0,
            value: 0x11,
        },
        PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 3,
            transient_palette: 0x13,
            repaint_visible_x: 11,
            transfer_lead_pixels: 0,
            value: 0x12,
        },
    ];

    ppu.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(7, true);

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert!(row[..4].iter().all(|&shade| shade == 0));
    assert!(row[4..].iter().all(|&shade| shade == 1));
}

#[test]
fn dmg_bgp_row_family_change_uses_previous_line_start_palette() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_start_palette = 0x01;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .previous_line_start_palette = 0xFC;
    ppu.dmg_panel_live_write_state
        .bgp_cpu_commit
        .current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
        transient_visible_x: 4,
        transient_palette: 0xFD,
        repaint_visible_x: 12,
        transfer_lead_pixels: 0,
        value: 0xFC,
    }];

    ppu.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(7, true);

    let row = &ppu.framebuffer()[7 * SCREEN_WIDTH..8 * SCREEN_WIDTH];
    assert!(row[..4].iter().all(|&shade| shade == 0));
    assert_eq!(row[4], 1);
    assert!(row[5..].iter().all(|&shade| shade == 0));
}

#[test]
fn dmg_bgp_write_in_early_hblank_recolors_only_last_three_visible_bg_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x80,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.current_scanline_mixed_pixels[156..160].fill(MixedPixel::background(0));
    ppu.framebuffer[156..160].fill(1);

    ppu.write_register(0xFF47, 0x00);

    assert_eq!(&ppu.framebuffer()[156..160], &[1, 1, 0, 0]);
}

#[test]
fn dmg_obp0_write_before_visible_x10_does_not_retroactively_recolor_obj_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = 200;
    ppu.bg_pipeline_state.visible_pixels_output = 8;
    ppu.current_scanline_mixed_pixels[3..8].fill(MixedPixel::object(1, false));
    ppu.framebuffer[3..8].fill(3);

    ppu.write_register(0xFF48, 0x04);

    assert_eq!(&ppu.framebuffer()[3..8], &[3, 3, 3, 3, 3]);
}

#[test]
fn dmg_obp0_write_during_mode3_skips_background_gaps_and_recolors_four_recent_obj_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = 200;
    ppu.bg_pipeline_state.visible_pixels_output = 10;
    ppu.obp0 = Some(0x00);
    ppu.visible_registers.obp0 = Some(0x00);
    ppu.current_scanline_mixed_pixels[2..6].fill(MixedPixel::object(1, false));
    ppu.current_scanline_mixed_pixels[6..10].fill(MixedPixel::background(0));
    ppu.framebuffer[2..6].fill(0);
    ppu.framebuffer[6..10].fill(0);

    ppu.write_register(0xFF48, 0x04);

    assert_eq!(&ppu.framebuffer()[2..10], &[1, 1, 1, 1, 0, 0, 0, 0]);
}

#[test]
fn dmg_obp0_write_at_visible_x10_skips_a_leading_isolated_obj_pixel() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = 200;
    ppu.bg_pipeline_state.visible_pixels_output = 10;
    ppu.obp0 = Some(0x00);
    ppu.visible_registers.obp0 = Some(0x00);
    ppu.current_scanline_mixed_pixels[5] = MixedPixel::object(1, false);
    ppu.current_scanline_mixed_pixels[7] = MixedPixel::object(1, false);
    ppu.current_scanline_mixed_pixels[..10]
        .iter_mut()
        .enumerate()
        .filter(|(x, _)| !matches!(x, 5 | 7))
        .for_each(|(_, pixel)| *pixel = MixedPixel::background(0));
    ppu.framebuffer[5] = 0;
    ppu.framebuffer[7] = 0;

    ppu.write_register(0xFF48, 0x04);

    assert_eq!(&ppu.framebuffer()[5..8], &[0, 0, 1]);
}

#[test]
fn dmg_obp0_write_during_mode3_recolors_recent_obj_pixels_with_the_committed_palette() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = 200;
    ppu.bg_pipeline_state.visible_pixels_output = 10;
    ppu.obp0 = Some(0x08);
    ppu.visible_registers.obp0 = Some(0x08);
    ppu.current_scanline_mixed_pixels[2..6].fill(MixedPixel::object(1, false));
    ppu.framebuffer[2..6].fill(2);

    ppu.write_register(0xFF48, 0x04);

    assert_eq!(ppu.obp0, Some(0x04));
    assert_eq!(&ppu.framebuffer()[2..6], &[1, 1, 1, 1]);
}

#[test]
fn cgb_bgp_cpu_commit_write_during_mode3_skips_dmg_palette_conflict_paths() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x11,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 25;
    ppu.visible_registers.bgp = 0x11;
    ppu.pipeline_registers.bgp = 0x11;
    ppu.bg_pipeline_state.visible_pixels_output = 25;
    ppu.bg_pipeline_state.current_transfer_x = 33;
    ppu.current_scanline_mixed_pixels[21..25].fill(MixedPixel::background(0));
    ppu.framebuffer[21..25].fill(1);

    ppu.write_register_with_source(0xFF47, 0x12, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(ppu.bgp, 0x12);
    assert_eq!(&ppu.framebuffer()[21..25], &[1, 1, 1, 1]);
    assert!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .is_empty()
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override,
        None
    );
    assert_eq!(
        ppu.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining,
        0
    );
}

#[test]
fn cgb_obp0_write_during_mode3_does_not_retroactively_recolor_recent_obj_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = 200;
    ppu.bg_pipeline_state.visible_pixels_output = 10;
    ppu.obp0 = Some(0x08);
    ppu.visible_registers.obp0 = Some(0x08);
    ppu.current_scanline_mixed_pixels[2..6].fill(MixedPixel::object(1, false));
    ppu.framebuffer[2..6].fill(2);

    ppu.write_register(0xFF48, 0x04);

    assert_eq!(ppu.obp0, Some(0x04));
    assert_eq!(&ppu.framebuffer()[2..6], &[2, 2, 2, 2]);
}

#[test]
fn mixed_pixel_override_keeps_background_palette_when_an_obj_register_is_overridden() {
    let visible = PpuVisibleRegisters {
        bgp: 0xE4,
        obp0: Some(0x39),
        ..PpuVisibleRegisters::default()
    };

    assert_eq!(
        visible.palette_for_mixed_pixel_with_override(
            MixedPixel::background(2),
            PpuPaletteRegister::Obp0,
            0x1B,
            0xE4,
            DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        ),
        0xE4
    );
}
