use super::*;

fn seed_lcdc0_trace_signature(
    visible_pixels_output: u8,
    current_transfer_x: u8,
    startup_fifo_placeholders: u8,
    startup_alignment_fill_pixel_index: u8,
) -> PpuTestRig {
    let mut ppu = dmg_observability_rig(ObservabilityRigConfig::new(0x93, 67, 0x96));
    ppu.line_dot = 104;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 263;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = startup_fifo_placeholders;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = current_transfer_x;
    ppu.bg_pipeline_state.visible_pixels_output = visible_pixels_output;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupAlignmentFill,
            fetch_x: 0,
            tile_map_address: 0x18A0,
            tile_data_address: 0x1001,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    for _ in 0..startup_alignment_fill_pixel_index {
        let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    }
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x18A1;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x1003;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x00;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x00;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_map_address: 0x18A1,
        tile_data_address: 0x1003,
        tile_index: 0,
        tile_low: 0x00,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };
    ppu
}

fn assert_front_signature(
    ppu: &PpuTestRig,
    line_dot: u16,
    visible_pixels_output: u8,
    current_transfer_x: u8,
    origin: BgCachedSliceOrigin,
    pixel_index: u8,
) {
    let front_cached = ppu
        .bg_pipeline_state
        .fifo
        .cached_slot(0)
        .expect("BG FIFO cached slot must exist")
        .expect("the traced startup slice should still front the FIFO");
    assert_eq!(ppu.line_dot, line_dot);
    assert_eq!(
        ppu.bg_pipeline_state.visible_pixels_output,
        visible_pixels_output
    );
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, current_transfer_x);
    assert_eq!(front_cached.cached.origin, origin);
    assert_eq!(front_cached.pixel_index, pixel_index);
}

fn advance_traced_visible_dots(ppu: &mut PpuTestRig, count: usize) {
    for _ in 0..count {
        let result = advance_visible_output_step(ppu);
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        let _ = ppu.advance_bg_fetcher_with_ppu_vram();
        ppu.line_dot += 1;
    }
}

fn resolve_test_rom_path(relative: &str) -> std::path::PathBuf {
    if let Some(root) = std::env::var_os("GB_CYCLE_TEST_ROM_ROOT") {
        return std::path::PathBuf::from(root).join(relative);
    }

    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test")
        .join(relative)
}

fn load_mealybug_m3_lcdc_bg_en_change_machine() -> Machine<TraceSummaryBuffer> {
    let rom_path = resolve_test_rom_path("mealybug-tearoom-tests/ppu/m3_lcdc_bg_en_change.gb");
    let rom =
        std::fs::read(&rom_path).expect("mealybug m3_lcdc_bg_en_change ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

#[test]
fn traced_lcdc0_low_mismatch_signature_progresses_from_fill3_to_tile2_7_tile3_7_and_ordinary_7() {
    let mut ppu = seed_lcdc0_trace_signature(3, 11, 3, 3);

    assert_front_signature(
        &ppu,
        104,
        3,
        11,
        BgCachedSliceOrigin::StartupAlignmentFill,
        3,
    );

    ppu.write_register(0xFF40, 0x92);
    advance_traced_visible_dots(&mut ppu, 12);

    assert_front_signature(
        &ppu,
        116,
        15,
        23,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        7,
    );

    ppu.write_register(0xFF40, 0x93);
    advance_traced_visible_dots(&mut ppu, 8);

    assert_front_signature(
        &ppu,
        124,
        23,
        31,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        7,
    );

    ppu.write_register(0xFF40, 0x92);
    advance_traced_visible_dots(&mut ppu, 8);

    assert_front_signature(&ppu, 132, 31, 39, BgCachedSliceOrigin::Ordinary, 7);
}

#[test]
fn traced_lcdc0_worst_band_signature_progresses_from_fill6_to_tile3_2_and_ordinary_2() {
    let mut ppu = seed_lcdc0_trace_signature(6, 14, 0, 6);

    assert_front_signature(
        &ppu,
        104,
        6,
        14,
        BgCachedSliceOrigin::StartupAlignmentFill,
        6,
    );

    ppu.write_register(0xFF40, 0x92);
    advance_traced_visible_dots(&mut ppu, 12);

    assert_front_signature(
        &ppu,
        116,
        18,
        26,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        2,
    );

    ppu.write_register(0xFF40, 0x93);
    advance_traced_visible_dots(&mut ppu, 8);

    assert_front_signature(&ppu, 124, 26, 34, BgCachedSliceOrigin::Ordinary, 2);

    ppu.write_register(0xFF40, 0x92);
    advance_traced_visible_dots(&mut ppu, 8);

    assert_front_signature(&ppu, 132, 34, 42, BgCachedSliceOrigin::Ordinary, 2);
}

fn log_mealybug_m3_lcdc_bg_en_change_internal_row(target_ly: u8) {
    let mut machine = load_mealybug_m3_lcdc_bg_en_change_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu();
        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps < 9 || ppu.ly != target_ly || ppu.current_access_mode() != PpuAccessMode::HBlank {
            continue;
        }

        let selected = ppu.mode2_scan_state.selected_sprites_snapshot();
        let mixed_colors = ppu.current_scanline_mixed_pixels[..40]
            .iter()
            .map(|pixel| pixel.color)
            .collect::<Vec<_>>();
        let mixed_sources = ppu.current_scanline_mixed_pixels[..40]
            .iter()
            .map(|pixel| match pixel.source {
                MixedPixelSource::Background => 'B',
                MixedPixelSource::Object { .. } => 'O',
            })
            .collect::<String>();
        println!(
            "ly={} sprite_xs={:?} scanline={:?} forced_white={:?} mixed_colors={:?} mixed_sources={}",
            ppu.ly,
            selected.iter().map(|sprite| sprite.x).collect::<Vec<_>>(),
            &ppu.current_scanline_pixels[..40],
            &ppu.current_scanline_dmg_bg_forced_white[..40],
            mixed_colors,
            mixed_sources,
        );
        return;
    }

    panic!("timed out before sampling the target HBlank row");
}

#[test]
#[ignore = "diag: internal mealybug m3_lcdc_bg_en_change ly0"]
fn real_mealybug_m3_lcdc_bg_en_change_logs_internal_ly0() {
    log_mealybug_m3_lcdc_bg_en_change_internal_row(0);
}

#[test]
#[ignore = "diag: internal mealybug m3_lcdc_bg_en_change ly16"]
fn real_mealybug_m3_lcdc_bg_en_change_logs_internal_ly16() {
    log_mealybug_m3_lcdc_bg_en_change_internal_row(16);
}
