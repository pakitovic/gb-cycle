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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
        ppu.dmg_bgp_cpu_commit_current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 21,
            transient_palette: 0x13,
            repaint_visible_x: 29,
            transfer_lead_pixels: 0,
            value: 0x12,
        }]
    );
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 1);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, Some(0x12));

    ppu.sync_pipeline_registers();
    ppu.sync_visible_registers();
    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.framebuffer()[25], 2);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 0);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, None);
}

#[test]
fn cpu_mmio_bgp_write_uses_pipeline_delay_when_recent_pixels_are_not_all_bg_color0() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
        ppu.dmg_bgp_cpu_commit_current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 21,
            transient_palette: 0xEE,
            repaint_visible_x: 29,
            transfer_lead_pixels: 0,
            value: 0xAA,
        }]
    );
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 4);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, Some(0xE4));

    for _ in 0..5 {
        ppu.sync_pipeline_registers();
        ppu.sync_visible_registers();
        let _ = ppu.advance_mode3_output_phase();
    }

    assert_eq!(&ppu.framebuffer()[25..30], &[1, 1, 1, 1, 2]);
}

#[test]
fn cpu_mmio_bgp_write_ignores_recent_obj_pixels_when_bg_tail_is_color0() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
        ppu.dmg_bgp_cpu_commit_current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x: 21,
            transient_palette: 0x13,
            repaint_visible_x: 29,
            transfer_lead_pixels: 0,
            value: 0x12,
        }]
    );
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 1);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, Some(0x12));
}

#[test]
fn cpu_mmio_bgp_write_stays_delayed_when_recent_affected_bg_pixel_is_nonzero_even_with_obj_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
        ppu.dmg_bgp_cpu_commit_current_line_writes,
        vec![PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
            transient_visible_x: 21,
            transient_palette: 0xEE,
            repaint_visible_x: 29,
            transfer_lead_pixels: 0,
            value: 0xAA,
        }]
    );
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 4);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, Some(0xE4));
}

#[test]
fn cpu_mmio_bgp_write_before_first_visible_bg_pixel_uses_the_visible_obj_prefix_as_delay() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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

    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 2);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, Some(0xFC));
}

#[test]
fn cpu_mmio_bgp_write_after_four_leading_obj_pixels_uses_the_committed_palette_on_the_current_dot()
{
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
        effect_kind: PpuDmgBgpCpuCommitEffectKind::PipelineDelayed,
        transient_visible_x: 0,
        transient_palette: 0x03,
        repaint_visible_x: 4,
        transfer_lead_pixels: 0,
        value: 0x03,
    }];

    ppu.write_register_with_source(0xFF47, 0x01, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 1);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, Some(0x01));

    ppu.sync_pipeline_registers();
    ppu.sync_visible_registers();
    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.framebuffer()[12], 1);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 0);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, None);
}

#[test]
fn cpu_mmio_bgp_write_after_the_first_selected_retroactive_write_keeps_the_recent_tail_and_flips_on_the_current_dot()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![
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
        ppu.dmg_bgp_cpu_commit_current_line_writes.last().copied(),
        Some(PpuDmgBgpCpuCommitWrite {
            effect_kind: PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient,
            transient_visible_x: 50,
            transient_palette: 0x03,
            repaint_visible_x: 51,
            transfer_lead_pixels: 0,
            value: 0x03,
        })
    );
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 1);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, Some(0x03));

    ppu.sync_pipeline_registers();
    ppu.sync_visible_registers();
    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(&ppu.framebuffer()[46..50], &[1, 1, 1, 1]);
    assert_eq!(ppu.framebuffer()[50], 3);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_delay_pixels_remaining, 0);
    assert_eq!(ppu.dmg_bgp_cpu_commit_output_palette_override, None);
}

#[test]
fn dmg_bgp_row_family_change_recolors_the_previous_boundary_scanline() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_bgp_cpu_commit_current_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_previous_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![
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
fn dmg_bgp_row_family_change_can_optionally_consume_retroactive_panel_writes() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_bgp_cpu_commit_current_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_previous_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_bgp_cpu_commit_current_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_previous_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_bgp_cpu_commit_current_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_previous_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_bgp_cpu_commit_current_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_previous_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.previous_scanline_mixed_pixels[..4].fill(MixedPixel::object(1, false));
    ppu.dmg_bgp_cpu_commit_current_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_previous_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_bgp_cpu_commit_current_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_previous_line_start_palette = 0x10;
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.previous_scanline_ly = Some(7);
    ppu.previous_scanline_mixed_pixels
        .fill(MixedPixel::background(0));
    ppu.dmg_bgp_cpu_commit_current_line_start_palette = 0x01;
    ppu.dmg_bgp_cpu_commit_previous_line_start_palette = 0xFC;
    ppu.dmg_bgp_cpu_commit_current_line_writes = vec![PpuDmgBgpCpuCommitWrite {
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
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
