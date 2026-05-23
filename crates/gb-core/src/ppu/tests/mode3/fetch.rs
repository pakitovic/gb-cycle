use super::super::*;

fn dmg_fetch_rig() -> PpuTestRig {
    PpuTestRig::dmg()
}

fn dmg_fetch_startup_rig(lcdc: u8) -> PpuTestRig {
    PpuTestRig::dmg().with_startup_state(PpuStartupState {
        lcdc,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    })
}

fn cgb_fetch_startup_rig(operating_mode: crate::model::OperatingMode, lcdc: u8) -> PpuTestRig {
    let mut ppu =
        PpuTestRig::with_model(ConsoleModel::GameBoyColor).with_startup_state(PpuStartupState {
            lcdc,
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
    ppu.apply_operating_mode_state(operating_mode);
    ppu
}

fn push_selected_sprite_x(ppu: &mut PpuTestRig, x: u8) {
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x,
        tile_index: 0,
        attributes: 0,
    });
}

fn observed_scy_obj_phase_table_for_sprite_x(sprite_x: u8) -> PpuMode3ObservedScyObjPhaseTable {
    PpuMode3ObservedScyObjPhaseTable::new(sprite_x)
}

fn observed_single_sprite_phase_policy(sprite_x: u8) -> PpuMode3SingleSpritePhasePolicy {
    PpuMode3SingleSpritePhasePolicy::new(sprite_x)
}

fn with_test_vram_view<T>(bytes: [u8; 0x2000], f: impl FnOnce(&VramBusView<'_>) -> T) -> T {
    let mut vram = crate::bus::VramDomain::from_bytes(&bytes);
    vram.set_acquired(BusMaster::Ppu, true);
    f(&VramBusView::new(BusMaster::Ppu, &mut vram))
}

fn dmg_live_window_restart_write_rig(lcdc: u8) -> PpuTestRig {
    let mut ppu = dmg_fetch_startup_rig(lcdc);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 16;
    ppu.bg_pipeline_state.window_started_this_line = true;
    ppu.bg_pipeline_state.window_active_line_counter = 32;
    ppu.window_state.window_line_counter = 0;
    ppu
}

#[test]
fn observed_scy_obj_phase_table_classifies_pending_refetch_by_obj_fetch_phase() {
    for sprite_x in 0..16 {
        let table = observed_scy_obj_phase_table_for_sprite_x(sprite_x);
        let phase = sprite_x & (BG_TILE_WIDTH - 1);

        assert_eq!(table.obj_match_tile_phase(), phase);
        assert_eq!(
            table.pending_refetch_prefers_high_plane_only(),
            matches!(phase, 5..=7),
            "sprite_x={sprite_x} phase={phase}"
        );
        assert_eq!(
            table.pending_refetch_prefers_tilemap_row(),
            matches!(phase, 0..=2),
            "sprite_x={sprite_x} phase={phase}"
        );
        assert_eq!(
            table.startup_visible_tile2_refetch_prefers_tilemap_row(),
            matches!(phase, 4..=7),
            "sprite_x={sprite_x} phase={phase}"
        );
    }
}

#[test]
fn observed_scy_obj_phase_table_tracks_cgb_dmg_software_route_classes() {
    let cases = [
        (0, true, false, true, true),
        (1, false, false, true, true),
        (2, true, false, false, false),
        (3, false, false, false, false),
        (8, true, false, false, true),
        (16, false, false, true, true),
        (17, false, false, false, false),
    ];

    for (
        sprite_x,
        pending_cached_slices,
        startup_alignment_fifo,
        current_fetch,
        pending_tilemap_row_refetch,
    ) in cases
    {
        let route = observed_scy_obj_phase_table_for_sprite_x(sprite_x)
            .cgb_dmg_software_live_scy_write_route();

        assert_eq!(
            route.pending_cached_slices(),
            pending_cached_slices,
            "sprite_x={sprite_x}"
        );
        assert_eq!(
            route.startup_alignment_fifo(),
            startup_alignment_fifo,
            "sprite_x={sprite_x}"
        );
        assert_eq!(route.current_fetch(), current_fetch, "sprite_x={sprite_x}");
        assert_eq!(
            route.scy_routing().pending_tilemap_row_refetch,
            pending_tilemap_row_refetch,
            "sprite_x={sprite_x}"
        );
        assert!(!route.scy_routing().pending_high_plane_only);
        assert!(
            !route
                .scy_routing()
                .startup_visible_tile2_tilemap_row_refetch
        );
        assert!(
            !route
                .scy_routing()
                .startup_visible_tile2_phase6_tilemap_row_refetch
        );
    }
}

#[test]
fn observed_scy_obj_phase_table_tracks_cgb_dmg_software_startup_tile_retargets() {
    assert_eq!(
        observed_scy_obj_phase_table_for_sprite_x(2)
            .cgb_dmg_software_startup_visible_tile2_tilemap_retarget(0, 0),
        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta: 0,
            tiledata_row_delta: -1,
        })
    );
    assert_eq!(
        observed_scy_obj_phase_table_for_sprite_x(2)
            .cgb_dmg_software_startup_visible_tile2_tilemap_retarget(6, 7),
        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta: 0,
            tiledata_row_delta: 0,
        })
    );
    assert_eq!(
        observed_scy_obj_phase_table_for_sprite_x(3)
            .cgb_dmg_software_startup_visible_tile2_tilemap_retarget(0, 0),
        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta: 0,
            tiledata_row_delta: 2,
        })
    );
    assert_eq!(
        observed_scy_obj_phase_table_for_sprite_x(8)
            .cgb_dmg_software_startup_visible_tile2_tilemap_retarget(5, 6),
        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta: 0,
            tiledata_row_delta: -1,
        })
    );
    assert_eq!(
        observed_scy_obj_phase_table_for_sprite_x(9)
            .cgb_dmg_software_startup_visible_tile2_tilemap_retarget(7, 4),
        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta: 0,
            tiledata_row_delta: 0,
        })
    );
    assert_eq!(
        observed_scy_obj_phase_table_for_sprite_x(16)
            .cgb_dmg_software_startup_visible_tile3_tilemap_retarget(0, 0),
        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta: 0,
            tiledata_row_delta: -2,
        })
    );
    assert_eq!(
        observed_scy_obj_phase_table_for_sprite_x(17)
            .cgb_dmg_software_startup_visible_tile2_tilemap_retarget(7, 0),
        Some(PpuMode3ScyTilemapRetarget {
            tilemap_row_delta: 0,
            tiledata_row_delta: 2,
        })
    );
    assert_eq!(
        observed_scy_obj_phase_table_for_sprite_x(4)
            .cgb_dmg_software_startup_visible_tile2_tilemap_retarget(0, 0),
        None
    );
}

#[test]
fn scy_obj_phase_policy_uses_current_transfer_x_for_owned_pending_obj_hits() {
    let policy = PpuMode3ScyObjPhasePolicy::new(PpuMode3ScyObjPhaseContext {
        phase_owner: PpuMode3ScyObjPhaseOwner::PendingHit { match_x: 8 },
        current_transfer_x: 8,
        current_transfer: None,
        bg_fetcher_stage: PpuBgFetcherStage::TileDataHigh,
        bg_fetcher_stage_dot: 1,
        bg_fifo_len: 8,
        startup_fifo_placeholders: 0,
        obj_fetcher_stage: PpuObjFetcherStage::Idle,
        obj_fetcher_stage_dot: 0,
    });

    assert_eq!(
        policy.phase_owner(),
        PpuMode3ScyObjPhaseOwner::PendingHit { match_x: 8 }
    );
    assert_eq!(
        policy.observed_phase_table(),
        PpuMode3ObservedScyObjPhaseTable::new(8)
    );
    assert_eq!(policy.observed_phase_table().obj_match_x(), 8);
    assert_eq!(policy.observed_phase_table().obj_match_tile_phase(), 0);
    assert!(!policy.pending_refetch_prefers_high_plane_only());
    assert!(policy.pending_refetch_prefers_tilemap_row());
}

#[test]
fn observed_lcdc0_onset_table_tracks_the_curated_single_sprite_write_windows() {
    let cases = [
        (0, Some(0), Some(11), Some(19), Some(27)),
        (5, Some(4), Some(16), Some(24), Some(32)),
        (8, Some(0), Some(11), Some(19), Some(27)),
        (16, Some(8), Some(11), Some(19), Some(27)),
        (18, None, None, None, None),
    ];

    for (sprite_x, write0, write1, write2, write3) in cases {
        let table = observed_single_sprite_phase_policy(sprite_x).observed_lcdc0_onset_table();
        assert_eq!(
            table.onset_visible_x(0),
            write0,
            "sprite_x={sprite_x} write=0"
        );
        assert_eq!(
            table.onset_visible_x(1),
            write1,
            "sprite_x={sprite_x} write=1"
        );
        assert_eq!(
            table.onset_visible_x(2),
            write2,
            "sprite_x={sprite_x} write=2"
        );
        assert_eq!(
            table.onset_visible_x(3),
            write3,
            "sprite_x={sprite_x} write=3"
        );
        assert_eq!(
            table.onset_visible_x(4),
            None,
            "sprite_x={sprite_x} write=4"
        );
    }
}

#[test]
fn observed_cgb_dmg_software_lcdc0_onset_table_tracks_cgb_write_windows() {
    let cases = [
        (0, Some(0), Some(12), Some(20), Some(28)),
        (1, Some(1), Some(13), Some(21), Some(29)),
        (2, Some(2), Some(14), Some(22), Some(30)),
        (5, Some(5), Some(17), Some(25), Some(33)),
        (8, Some(0), Some(12), Some(20), Some(28)),
        (17, Some(9), Some(13), Some(21), Some(29)),
        (18, None, None, None, None),
    ];

    for (sprite_x, write0, write1, write2, write3) in cases {
        let table = observed_single_sprite_phase_policy(sprite_x).observed_lcdc0_onset_table();
        assert_eq!(
            table.cgb_dmg_software_onset_visible_x(0),
            write0,
            "sprite_x={sprite_x} write=0"
        );
        assert_eq!(
            table.cgb_dmg_software_onset_visible_x(1),
            write1,
            "sprite_x={sprite_x} write=1"
        );
        assert_eq!(
            table.cgb_dmg_software_onset_visible_x(2),
            write2,
            "sprite_x={sprite_x} write=2"
        );
        assert_eq!(
            table.cgb_dmg_software_onset_visible_x(3),
            write3,
            "sprite_x={sprite_x} write=3"
        );
        assert_eq!(
            table.cgb_dmg_software_onset_visible_x(4),
            None,
            "sprite_x={sprite_x} write=4"
        );
    }
}

#[test]
fn observed_lcdc4_phase_table_returns_typed_startup_overrides() {
    let cases = [
        (
            BgTileDataSelect::Unsigned8000,
            3,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile2,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            }),
        ),
        (
            BgTileDataSelect::Unsigned8000,
            6,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile2,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            }),
        ),
        (
            BgTileDataSelect::Signed8800,
            13,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile3,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Signed8800),
                ),
            }),
        ),
        (
            BgTileDataSelect::Signed8800,
            16,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile3,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            }),
        ),
        (BgTileDataSelect::Unsigned8000, 2, None),
        (BgTileDataSelect::Signed8800, 18, None),
    ];

    for (target_select, sprite_x, expected) in cases {
        assert_eq!(
            observed_single_sprite_phase_policy(sprite_x)
                .observed_lcdc4_phase_table()
                .startup_override_for_target_select(target_select),
            expected,
            "target_select={target_select:?} sprite_x={sprite_x}",
        );
    }
}

#[test]
fn observed_cgb_dmg_software_lcdc4_phase_table_returns_typed_startup_overrides() {
    let cases = [
        (BgTileDataSelect::Unsigned8000, 2, 0, None),
        (
            BgTileDataSelect::Unsigned8000,
            3,
            0,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile2,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            }),
        ),
        (
            BgTileDataSelect::Unsigned8000,
            8,
            0,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile2,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Signed8800),
                ),
            }),
        ),
        (
            BgTileDataSelect::Signed8800,
            3,
            0,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile3,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Signed8800),
                ),
            }),
        ),
        (
            BgTileDataSelect::Signed8800,
            12,
            0,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile3,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Signed8800),
                ),
            }),
        ),
        (
            BgTileDataSelect::Signed8800,
            16,
            0,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile3,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Unsigned8000),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            }),
        ),
        (
            BgTileDataSelect::Signed8800,
            16,
            1,
            Some(PpuMode3Lcdc4StartupOverride {
                slice: BgVisibleStartupSlice::VisibleTile3,
                override_select: PerPlane::new(
                    Some(BgTileDataSelect::Signed8800),
                    Some(BgTileDataSelect::Unsigned8000),
                ),
            }),
        ),
        (BgTileDataSelect::Signed8800, 18, 0, None),
    ];

    for (target_select, sprite_x, ly, expected) in cases {
        assert_eq!(
            observed_single_sprite_phase_policy(sprite_x)
                .observed_lcdc4_phase_table()
                .cgb_dmg_software_startup_override_for_target_select(target_select, ly),
            expected,
            "target_select={target_select:?} sprite_x={sprite_x} ly={ly}",
        );
    }
}

#[test]
fn observed_lcdc3_phase_table_returns_declarative_live_write_decisions() {
    let cases = [
        (
            5,
            0,
            true,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: true,
                tilemap_override: Some(PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: false,
                    applies_to_visible_tile3: true,
                }),
            }),
        ),
        (
            2,
            0,
            true,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: false,
                tilemap_override: Some(PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: true,
                    applies_to_visible_tile3: false,
                }),
            }),
        ),
        (
            2,
            1,
            false,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: false,
                tilemap_override: Some(PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: true,
                    applies_to_visible_tile3: false,
                }),
            }),
        ),
        (
            16,
            1,
            false,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: true,
                tilemap_override: None,
            }),
        ),
        (
            20,
            0,
            true,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: false,
                tilemap_override: Some(PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: false,
                    applies_to_visible_tile3: true,
                }),
            }),
        ),
        (18, 0, false, None),
    ];

    for (sprite_x, write_index, current_bg_tilemap_select, expected) in cases {
        assert_eq!(
            observed_single_sprite_phase_policy(sprite_x)
                .observed_lcdc3_phase_table()
                .live_write_decision(write_index, current_bg_tilemap_select),
            expected,
            "sprite_x={sprite_x} write_index={write_index} current_bg_tilemap_select={current_bg_tilemap_select}",
        );
    }
}

#[test]
fn observed_cgb_dmg_software_lcdc3_phase_table_returns_declarative_live_write_decisions() {
    let cases = [
        (
            0,
            0,
            true,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: false,
                tilemap_override: Some(PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: true,
                    applies_to_visible_tile3: false,
                }),
            }),
        ),
        (
            1,
            0,
            true,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: true,
                tilemap_override: Some(PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: false,
                    applies_to_visible_tile3: true,
                }),
            }),
        ),
        (
            2,
            1,
            false,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: true,
                tilemap_override: Some(PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: false,
                    applies_to_visible_tile3: true,
                }),
            }),
        ),
        (
            8,
            0,
            true,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: true,
                tilemap_override: None,
            }),
        ),
        (
            9,
            0,
            true,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: true,
                tilemap_override: Some(PpuMode3Lcdc3StartupTilemapOverride {
                    tilemap_select: true,
                    applies_to_visible_tile2: false,
                    applies_to_visible_tile3: true,
                }),
            }),
        ),
        (
            16,
            0,
            true,
            Some(PpuMode3Lcdc3LiveWriteDecision {
                clear_visible_tile2_live_refetch: true,
                tilemap_override: None,
            }),
        ),
        (18, 0, true, None),
    ];

    for (sprite_x, write_index, current_bg_tilemap_select, expected) in cases {
        assert_eq!(
            observed_single_sprite_phase_policy(sprite_x)
                .observed_lcdc3_phase_table()
                .cgb_dmg_software_live_write_decision(write_index, current_bg_tilemap_select),
            expected,
            "sprite_x={sprite_x} write_index={write_index} current_bg_tilemap_select={current_bg_tilemap_select}",
        );
    }
}

#[test]
fn scy_obj_phase_policy_uses_current_owner_for_startup_windows() {
    let mut ppu = dmg_fetch_startup_rig(0x93);
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers = ppu.visible_registers;
    push_selected_sprite_x(&mut ppu, 8);
    push_selected_sprite_x(&mut ppu, 16);
    ppu.bg_pipeline_state.current_transfer_x = 16;
    ppu.obj_pipeline_state.pending_match_x = Some(16);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);

    let policy = ppu
        .scy_obj_phase_policy()
        .expect("pending OBJ hit should produce an SCY/OBJ phase policy");

    assert_eq!(
        policy.phase_owner(),
        PpuMode3ScyObjPhaseOwner::PendingHit { match_x: 16 }
    );
    assert_eq!(policy.observed_phase_table().obj_match_x(), 16);
    assert!(!policy.startup_visible_tile2_uses_previous_tiledata_row());
    assert!(policy.startup_visible_tile3_uses_previous_tiledata_row());
}

#[test]
fn scy_obj_phase_policy_uses_active_fetch_owner_before_line_order() {
    let mut ppu = dmg_fetch_startup_rig(0x93);
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers = ppu.visible_registers;
    push_selected_sprite_x(&mut ppu, 8);
    let active_sprite = PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 16,
        tile_index: 0,
        attributes: 0,
    };
    ppu.mode2_scan_state.push(active_sprite);
    let obj_height = ppu.current_obj_height();
    ppu.obj_pipeline_state
        .start_fetch(1, active_sprite, obj_height, obj_height);

    let policy = ppu
        .scy_obj_phase_policy()
        .expect("active OBJ fetch should produce an SCY/OBJ phase policy");

    assert_eq!(
        policy.phase_owner(),
        PpuMode3ScyObjPhaseOwner::ActiveFetch { sprite_x: 16 }
    );
    assert_eq!(policy.observed_phase_table().obj_match_x(), 16);
}

#[test]
fn scy_obj_phase_policy_has_no_line_lead_owner_before_transfer_window() {
    let mut ppu = dmg_fetch_startup_rig(0x93);
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers = ppu.visible_registers;
    push_selected_sprite_x(&mut ppu, 8);
    ppu.bg_pipeline_state.current_transfer_x = 16;

    assert_eq!(ppu.scy_obj_phase_policy(), None);
}

#[test]
fn scy_obj_phase_policy_names_startup_line_lead_owner_inside_transfer_window() {
    let mut ppu = dmg_fetch_startup_rig(0x93);
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers = ppu.visible_registers;
    push_selected_sprite_x(&mut ppu, 8);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 16;

    let policy = ppu
        .scy_obj_phase_policy()
        .expect("open Mode 3 transfer window should expose the startup line-lead owner");

    assert_eq!(
        policy.phase_owner(),
        PpuMode3ScyObjPhaseOwner::StartupLineLead { sprite_x: 8 }
    );
    assert_eq!(policy.observed_phase_table().obj_match_x(), 8);
}

#[test]
fn scy_obj_phase_policy_names_startup_line_lead_owner_during_startup_seam() {
    let mut ppu = dmg_fetch_startup_rig(0x93);
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers = ppu.visible_registers;
    push_selected_sprite_x(&mut ppu, 8);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::AlignmentSeedPending;
    ppu.bg_pipeline_state.current_transfer_x = 16;

    let policy = ppu
        .scy_obj_phase_policy()
        .expect("startup seam should expose the startup line-lead owner");

    assert_eq!(
        policy.phase_owner(),
        PpuMode3ScyObjPhaseOwner::StartupLineLead { sprite_x: 8 }
    );
    assert_eq!(policy.observed_phase_table().obj_match_x(), 8);
}

#[test]
fn observed_scy_obj_phase_table_keeps_startup_previous_row_windows_explicit() {
    for sprite_x in 0..24 {
        let table = observed_scy_obj_phase_table_for_sprite_x(sprite_x);

        assert_eq!(
            table.startup_visible_tile2_phase6_refetch_prefers_tilemap_row(),
            matches!(sprite_x, 4..=7),
            "sprite_x={sprite_x}"
        );
        assert_eq!(
            table.startup_visible_tile2_uses_previous_tiledata_row(),
            matches!(sprite_x, 8..=15),
            "sprite_x={sprite_x}"
        );
        assert_eq!(
            table.startup_visible_tile3_uses_previous_tiledata_row(),
            matches!(sprite_x, 16..=17),
            "sprite_x={sprite_x}"
        );
    }
}

#[test]
fn observed_scy_obj_phase_table_names_startup_tile2_retarget_cases() {
    let cases = [
        (9, 6, 4, Some((1, 0))),
        (9, 6, 5, Some((1, 0))),
        (9, 6, 6, Some((1, -1))),
        (9, 6, 7, Some((1, 0))),
        (10, 6, 5, Some((-1, 0))),
        (10, 6, 6, Some((-1, 0))),
        (10, 6, 7, Some((-1, 0))),
        (11, 7, 5, Some((0, 0))),
        (16, 6, 0, Some((1, 0))),
        (16, 6, 7, Some((0, -1))),
        (9, 5, 4, None),
        (10, 6, 4, None),
        (16, 5, 0, None),
    ];

    for (sprite_x, ly, pixel_index, expected) in cases {
        let retarget = observed_scy_obj_phase_table_for_sprite_x(sprite_x)
            .startup_visible_tile2_tilemap_retarget(ly, pixel_index)
            .map(|retarget| (retarget.tilemap_row_delta, retarget.tiledata_row_delta));

        assert_eq!(
            retarget, expected,
            "sprite_x={sprite_x} ly={ly} pixel_index={pixel_index}"
        );
    }
}

#[test]
fn observed_scy_obj_phase_table_names_startup_placeholder_cases() {
    let positive_cases = [(16, 5, 16), (17, 6, 8)];
    for (sprite_x, ly, visible_x) in positive_cases {
        assert!(
            observed_scy_obj_phase_table_for_sprite_x(sprite_x)
                .startup_visible_tile2_placeholder_uses_previous_tilemap_row(ly, visible_x),
            "sprite_x={sprite_x} ly={ly} visible_x={visible_x}"
        );
    }

    let negative_cases = [(15, 5, 16), (16, 5, 8), (16, 6, 16), (17, 6, 16)];
    for (sprite_x, ly, visible_x) in negative_cases {
        assert!(
            !observed_scy_obj_phase_table_for_sprite_x(sprite_x)
                .startup_visible_tile2_placeholder_uses_previous_tilemap_row(ly, visible_x),
            "sprite_x={sprite_x} ly={ly} visible_x={visible_x}"
        );
    }
}

#[test]
fn bg_fetcher_stage_dot_is_an_explicit_one_dot_automaton() {
    let mut ppu = dmg_fetch_rig();
    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);
    ppu.write_bg_tilemap_entry(0, 0, 0);

    ppu.visible_registers.lcdc = 0x91;
    ppu.bg_pipeline_state.fetcher.start_background();

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataLow
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataLow
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);
    assert!(ppu.bg_pipeline_state.push.pending);
    assert!(!ppu.bg_pipeline_state.fill.pending);
}

#[test]
fn bg_fetcher_records_the_tilemap_address_for_the_current_phase() {
    let mut ppu = dmg_fetch_rig();
    ppu.vram_bytes[0x1C64] = 0x66;

    ppu.visible_registers.lcdc = 0x99;
    ppu.visible_registers.scx = 24;
    ppu.visible_registers.scy = 16;
    ppu.ly = 8;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 8;

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C64);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x66);
}

#[test]
fn bg_fetcher_recomputes_scy_for_each_tile_data_plane_read_on_dmg() {
    let mut ppu = dmg_fetch_rig();
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(0, 1, 0x56, 0x78);

    ppu.visible_registers.lcdc = 0x91;
    ppu.visible_registers.scy = 0;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0000);

    ppu.visible_registers.scy = 1;
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x78);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0003);
}

#[test]
fn bg_fetcher_recomputes_tile_data_address_when_tile_selector_changes_between_planes() {
    let mut ppu = dmg_fetch_rig();
    ppu.write_bg_tile_row(1, 0, 0x12, 0x34);
    ppu.vram_bytes[0x1011] = 0xAB;

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.visible_registers.scy = 0;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0010);

    ppu.visible_registers.lcdc = 0x81;
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1011);
}

#[test]
fn cached_background_push_recomputes_tilemap_and_tiledata_on_push_dot_zero_map_change() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_window_tilemap_entry(1, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C01);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 1);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0011);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0xCD);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_high, 0xCD);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
}

#[test]
fn cached_background_push_accepts_same_tcycle_tilemap_refetch_after_entry_delay_dot() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_window_tilemap_entry(1, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::EntryDelay);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );

    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0xCD);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(
        !ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
}

#[test]
fn cached_background_push_recomputes_tilemap_when_scx_tile_column_changes() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_bg_tilemap_entry(2, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.fetch_x = 8;
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::EntryDelay);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
    ppu.write_register(0xFF43, 0x08);
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0xCD);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1802);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 1);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0011);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0xCD);
}

#[test]
fn cached_background_push_ignores_scx_low_bit_only_changes() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_bg_tilemap_entry(2, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.fetch_x = 8;
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::EntryDelay);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
    ppu.write_register(0xFF43, 0x02);
    assert!(!ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0x34);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1801);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0001);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x34);
}

#[test]
fn late_second_startup_continuation_push_marks_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;

    ppu.write_register(0xFF40, 0x99);

    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
}

#[test]
fn third_startup_continuation_fetcher_carries_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };

    ppu.write_register(0xFF40, 0x99);
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());

    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.push.cached.origin,
        ppu.bg_pipeline_state.fetcher.cached_origin
    );
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
}

#[test]
fn third_startup_continuation_fetcher_carries_full_tilemap_refetch_on_scx_tile_column_change() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };

    ppu.write_register(0xFF43, 0x08);
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());

    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.push.cached.origin,
        ppu.bg_pipeline_state.fetcher.cached_origin
    );
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tilemap_full_refetch
    );
}

#[test]
fn startup_visible_tile3_scx_boundary_full_refetch_stays_narrow_to_the_late_high0_window() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(2, 0, 0);
    ppu.write_bg_tilemap_entry(5, 0, 1);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + 112;
    ppu.ly = 0;
    ppu.bg_pipeline_state.visible_pixels_output = 7;
    ppu.bg_pipeline_state.current_transfer_x = 15;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };

    ppu.write_register(0xFF43, 0x18);

    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .startup_visible_tile3_scx_boundary_full_refetch_next_tile
    );
    assert!(
        ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_full_refetch_on_push
    );

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert!(ppu.bg_pipeline_state.push.pending);

    ppu.with_ppu_vram(|ppu, vram| ppu.maybe_recompute_pending_background_push(vram));

    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_map_address, 0x1805);
}

fn configure_startup_visible_tile3_current_fetch_boundary(ppu: &mut PpuTestRig) {
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + 112;
    ppu.ly = 0;
    ppu.bg_pipeline_state.current_transfer_x = 16;
    ppu.bg_pipeline_state.visible_pixels_output = 8;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state
        .fetcher
        .needs_live_tilemap_refetch_on_push = true;
    ppu.bg_pipeline_state
        .fetcher
        .needs_live_tilemap_full_refetch_on_push = true;
    ppu.bg_pipeline_state
        .fetcher
        .startup_visible_tile3_scx_boundary_full_refetch_next_tile = true;
    ppu.bg_pipeline_state
        .fetcher
        .startup_visible_tile3_scx_boundary_old_tail_start_pixel = 3;
    ppu.bg_pipeline_state
        .startup_visible_tile3_scx_boundary_next_slice_previous_scx = Some(0x88);
    ppu.bg_pipeline_state
        .startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels = 3;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
}

#[test]
fn startup_visible_tile3_scx_boundary_write_clears_current_fetch_full_refetch_state() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    configure_startup_visible_tile3_current_fetch_boundary(&mut ppu);

    ppu.write_register(0xFF43, 0x18);

    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_refetch_on_push
    );
    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_full_refetch_on_push
    );
    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .startup_visible_tile3_scx_boundary_full_refetch_next_tile
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .fetcher
            .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
        BG_TILE_WIDTH
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .startup_visible_tile3_scx_boundary_next_slice_previous_scx,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels,
        0
    );
}

fn configure_startup_visible_tile3_push_boundary(
    ppu: &mut PpuTestRig,
    current_transfer_x: u8,
    visible_pixels_output: u8,
) {
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + 112;
    ppu.ly = 0;
    ppu.bg_pipeline_state.current_transfer_x = current_transfer_x;
    ppu.bg_pipeline_state.visible_pixels_output = visible_pixels_output;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::Inactive;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        needs_live_tilemap_refetch: true,
        needs_live_tilemap_full_refetch: true,
        startup_visible_tile3_scx_boundary_previous_scx: Some(0x44),
        startup_visible_tile3_scx_boundary_old_tail_start_pixel: BG_TILE_WIDTH,
        startup_visible_tile3_scx_boundary_old_prefix_pixels: 0,
        ..BgCachedSlice::default()
    };
    let visible_tile2 = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_low: 0xFF,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels_with_skip(
            visible_tile2,
            current_transfer_x.saturating_sub(16),
        );
}

#[test]
fn startup_visible_tile3_scx_low_band_old_pixel_window_clears_broad_refetch_state() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    configure_startup_visible_tile3_push_boundary(&mut ppu, 21, 13);
    ppu.bg_pipeline_state
        .push
        .cached
        .startup_visible_tile3_scx_boundary_old_tail_start_pixel = 2;
    ppu.bg_pipeline_state
        .push
        .cached
        .startup_visible_tile3_scx_boundary_old_prefix_pixels = 3;

    ppu.write_register(0xFF43, 0x0B);

    assert!(!ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
    assert!(
        !ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tilemap_full_refetch
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_previous_scx,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
        BG_TILE_WIDTH
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_old_prefix_pixels,
        0
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx,
        Some(0)
    );
}

#[test]
fn startup_visible_tile3_scx_next_tile_output_retarget_covers_low_band_classes() {
    for (scx, expected_prefix, expected_tail) in [
        (0x58, 1, BG_TILE_WIDTH),
        (0x61, 2, BG_TILE_WIDTH),
        (0x63, 5, 4),
        (0x65, 5, 4),
        (0x76, 1, 3),
        (0x78, 1, 7),
        (0x7A, 0, 5),
    ] {
        let mut ppu = dmg_fetch_startup_rig(0x91);
        configure_startup_visible_tile3_push_boundary(&mut ppu, 22, 14);

        ppu.write_register(0xFF43, scx);

        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx,
            Some(scx),
            "SCX {scx:#04X} should retarget the carried tile"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_old_prefix_pixels,
            expected_prefix,
            "SCX {scx:#04X} old-prefix span"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
            expected_tail,
            "SCX {scx:#04X} old-tail start"
        );
    }
}

#[test]
fn cgb_dmg_software_scx_high_write_preserves_visible_tile3_push_old_tile() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = cgb_fetch_startup_rig(operating_mode, 0x91);
        configure_startup_visible_tile3_push_boundary(&mut ppu, 21, 13);
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_previous_scx = None;
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_old_tail_start_pixel = BG_TILE_WIDTH;

        ppu.write_register(0xFF43, 0x08);

        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_previous_scx,
            Some(0x00),
            "{operating_mode:?} should keep the old startup tile source"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
            0,
            "{operating_mode:?} should preserve the whole carried tile"
        );
    }
}

#[test]
fn cgb_dmg_software_scx_high_write_preserves_visible_tile3_current_fetch_old_tile() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = cgb_fetch_startup_rig(operating_mode, 0x91);
        ppu.bg_pipeline_state.mode3_started = true;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.line_dot = MODE2_DOTS + 112;
        ppu.bg_pipeline_state.current_transfer_x = 15;
        ppu.bg_pipeline_state.visible_pixels_output = 7;
        ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
        ppu.bg_pipeline_state.fetcher.cached_origin =
            BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
        ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
        ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
        ppu.bg_pipeline_state.fetcher.stage_dot = 1;

        ppu.write_register(0xFF43, 0x10);

        assert_eq!(
            ppu.bg_pipeline_state
                .fetcher
                .startup_visible_tile3_scx_boundary_previous_scx,
            Some(0x00),
            "{operating_mode:?} should keep the old startup tile source"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .fetcher
                .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
            0,
            "{operating_mode:?} should preserve the whole in-flight tile"
        );
    }
}

#[test]
fn native_cgb_scx_high_write_skips_dmg_software_visible_tile3_old_tile_seam() {
    let mut ppu = cgb_fetch_startup_rig(crate::model::OperatingMode::Cgb, 0x91);
    configure_startup_visible_tile3_push_boundary(&mut ppu, 21, 13);
    ppu.bg_pipeline_state
        .push
        .cached
        .startup_visible_tile3_scx_boundary_previous_scx = None;
    ppu.bg_pipeline_state
        .push
        .cached
        .startup_visible_tile3_scx_boundary_old_tail_start_pixel = BG_TILE_WIDTH;

    ppu.write_register(0xFF43, 0x08);

    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_previous_scx,
        None
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .cached
            .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
        BG_TILE_WIDTH
    );
}

#[test]
fn visible_tile3_scx_boundary_old_tail_window_preserves_old_pixels_on_output() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.visible_registers.scx = 0x1B;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.write_bg_tilemap_entry(2, 0, 0);
    ppu.write_bg_tilemap_entry(5, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(1, 0, 0xFF, 0x00);

    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        tile_map_address: 0x1805,
        tile_data_address: 0x0011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };
    cached.arm_startup_visible_tile3_scx_boundary_old_tail(0x00, 0x1B);
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let mut pixels = Vec::new();
    for _ in 0..BG_TILE_WIDTH {
        let pixel = ppu
            .with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
            .expect("queued slice should still expose a visible pixel");
        pixels.push(pixel.color);
    }

    assert_eq!(pixels, vec![1, 1, 1, 1, 1, 1, 0, 0]);
}

#[test]
fn ordinary_slice_after_visible_tile3_scx_boundary_preserves_old_prefix_pixel_on_output() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.visible_registers.scx = 0x1B;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.write_bg_tilemap_entry(3, 0, 0);
    ppu.write_bg_tilemap_entry(6, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x00, 0x00);
    ppu.write_bg_tile_row(1, 0, 0xFF, 0x00);

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::Ordinary,
        fetch_x: BG_TILE_WIDTH as u16 * 3,
        tile_map_address: 0x1806,
        tile_data_address: 0x0011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0x00,
        startup_visible_tile3_scx_boundary_previous_scx: Some(0x00),
        startup_visible_tile3_scx_boundary_old_tail_start_pixel: BG_TILE_WIDTH,
        startup_visible_tile3_scx_boundary_old_prefix_pixels: 1,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let mut pixels = Vec::new();
    for _ in 0..BG_TILE_WIDTH {
        let pixel = ppu
            .with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
            .expect("queued slice should still expose a visible pixel");
        pixels.push(pixel.color);
    }

    assert_eq!(pixels, vec![0, 1, 1, 1, 1, 1, 1, 1]);
}

#[test]
fn first_window_activation_pixel_can_use_the_previous_tilemap_on_window_line_25() {
    let mut ppu = dmg_fetch_rig();
    ppu.visible_registers.lcdc = 0xE3;
    ppu.pipeline_registers.lcdc = 0xA3;
    ppu.window_state.window_line_counter = 25;
    ppu.write_bg_tilemap_entry(0, 3, 0);
    ppu.write_window_tilemap_entry(0, 3, 1);
    ppu.vram_bytes[0x1002] = 0x00;
    ppu.vram_bytes[0x1003] = 0x00;
    ppu.vram_bytes[0x1012] = 0xFF;
    ppu.vram_bytes[0x1013] = 0xFF;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: 0,
        window_activation_first_pixel_previous_tilemap_select: Some(false),
        tile_map_address: 0x1C60,
        tile_data_address: 0x1013,
        tile_low_address: 0x1012,
        tile_high_address: 0x1013,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0xFF,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let pixel = ppu
        .with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
        .expect("queued window slice should expose a visible pixel");

    assert_eq!(pixel.color, 0);
}

#[test]
fn first_window_activation_first_tile_uses_the_oracle_mask_on_window_line_32() {
    let mut ppu = dmg_fetch_rig();
    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xA3;
    ppu.window_state.window_line_counter = 32;
    ppu.write_bg_tilemap_entry(0, 4, 0);
    ppu.write_window_tilemap_entry(0, 4, 1);
    ppu.vram_bytes[0x1000] = 0x00;
    ppu.vram_bytes[0x1001] = 0x00;
    ppu.vram_bytes[0x1010] = 0xFF;
    ppu.vram_bytes[0x1011] = 0xFF;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: 0,
        window_activation_first_pixel_previous_tilemap_select: Some(false),
        tile_map_address: 0x1C80,
        tile_data_address: 0x1011,
        tile_low_address: 0x1010,
        tile_high_address: 0x1011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0xFF,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let pixels = (0..BG_TILE_WIDTH)
        .map(|_| {
            ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                .expect("queued window slice should expose a visible pixel")
                .color
        })
        .collect::<Vec<_>>();

    assert_eq!(pixels, vec![3, 3, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn first_window_activation_first_tile_uses_the_oracle_mask_on_window_line_56() {
    let mut ppu = dmg_fetch_rig();
    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xA3;
    ppu.window_state.window_line_counter = 56;
    ppu.write_bg_tilemap_entry(0, 7, 0);
    ppu.write_window_tilemap_entry(0, 7, 1);
    ppu.vram_bytes[0x1000] = 0x00;
    ppu.vram_bytes[0x1001] = 0x00;
    ppu.vram_bytes[0x1010] = 0xFF;
    ppu.vram_bytes[0x1011] = 0xFF;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: 0,
        window_activation_first_pixel_previous_tilemap_select: Some(false),
        tile_map_address: 0x1CE0,
        tile_data_address: 0x1011,
        tile_low_address: 0x1010,
        tile_high_address: 0x1011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0xFF,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let pixels = (0..BG_TILE_WIDTH)
        .map(|_| {
            ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                .expect("queued window slice should expose a visible pixel")
                .color
        })
        .collect::<Vec<_>>();

    assert_eq!(pixels, vec![0, 3, 3, 3, 3, 0, 0, 0]);
}

#[test]
fn second_window_tile_uses_the_oracle_mask_on_window_line_32() {
    let mut ppu = dmg_fetch_rig();
    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xA3;
    ppu.window_state.window_line_counter = 32;
    ppu.write_bg_tilemap_entry(1, 4, 0);
    ppu.write_window_tilemap_entry(1, 4, 1);
    ppu.vram_bytes[0x1000] = 0x00;
    ppu.vram_bytes[0x1001] = 0x00;
    ppu.vram_bytes[0x1010] = 0xFF;
    ppu.vram_bytes[0x1011] = 0xFF;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: BG_TILE_WIDTH as u16,
        window_activation_first_pixel_previous_tilemap_select: Some(false),
        tile_map_address: 0x1C81,
        tile_data_address: 0x1011,
        tile_low_address: 0x1010,
        tile_high_address: 0x1011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0xFF,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let pixels = (0..BG_TILE_WIDTH)
        .map(|_| {
            ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                .expect("queued window slice should expose a visible pixel")
                .color
        })
        .collect::<Vec<_>>();

    assert_eq!(pixels, vec![3, 3, 3, 3, 3, 3, 3, 3]);
}

#[test]
fn cgb_dmg_software_second_window_tile_uses_current_map_during_lead_in_lines() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = cgb_fetch_startup_rig(operating_mode, 0xA3);
        ppu.visible_registers.lcdc = 0xA3;
        ppu.pipeline_registers.lcdc = 0xA3;
        ppu.window_state.window_line_counter = 0;
        ppu.write_bg_tilemap_entry(1, 0, 0);
        ppu.write_window_tilemap_entry(1, 0, 1);
        for row in 0..BG_TILE_WIDTH as usize {
            let tile0_row = 0x1000 + row * 2;
            let tile1_row = 0x1010 + row * 2;
            ppu.vram_bytes[tile0_row] = 0x00;
            ppu.vram_bytes[tile0_row + 1] = 0x00;
            ppu.vram_bytes[tile1_row] = 0xFF;
            ppu.vram_bytes[tile1_row + 1] = 0xFF;
        }

        let cached = BgCachedSlice {
            source: PpuBgFetcherSource::Window,
            fetch_x: BG_TILE_WIDTH as u16,
            window_activation_first_pixel_previous_tilemap_select: Some(false),
            tile_map_address: 0x1801,
            tile_data_address: 0x1001,
            tile_low_address: 0x1000,
            tile_high_address: 0x1001,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        };
        ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

        let pixels = (0..BG_TILE_WIDTH)
            .map(|_| {
                ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                    .expect("queued window slice should expose a visible pixel")
                    .color
            })
            .collect::<Vec<_>>();

        assert_eq!(pixels, vec![3, 3, 3, 3, 3, 3, 3, 3]);
    }
}

#[test]
fn cgb_dmg_software_first_window_tile_uses_previous_map_after_first_lead_in_tile() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = cgb_fetch_startup_rig(operating_mode, 0xA3);
        ppu.visible_registers.lcdc = 0xA3;
        ppu.pipeline_registers.lcdc = 0xA3;
        ppu.window_state.window_line_counter = 8;
        ppu.write_bg_tilemap_entry(0, 1, 0);
        ppu.write_window_tilemap_entry(0, 1, 1);
        ppu.vram_bytes[0x1000] = 0x00;
        ppu.vram_bytes[0x1001] = 0x00;
        ppu.vram_bytes[0x1010] = 0xFF;
        ppu.vram_bytes[0x1011] = 0xFF;

        let cached = BgCachedSlice {
            source: PpuBgFetcherSource::Window,
            fetch_x: 0,
            window_activation_first_pixel_previous_tilemap_select: Some(false),
            tile_map_address: 0x1C20,
            tile_data_address: 0x1011,
            tile_low_address: 0x1010,
            tile_high_address: 0x1011,
            tile_index: 1,
            tile_low: 0xFF,
            tile_high: 0xFF,
            ..BgCachedSlice::default()
        };
        ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

        let pixels = (0..BG_TILE_WIDTH)
            .map(|_| {
                ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                    .expect("queued window slice should expose a visible pixel")
                    .color
            })
            .collect::<Vec<_>>();

        assert_eq!(pixels, vec![0, 0, 0, 0, 0, 0, 0, 0]);
    }
}

#[test]
fn cgb_dmg_software_first_window_tile_uses_sparse_transient_map_masks_in_lead_in_lines() {
    let cases = [
        (10, false, vec![3, 0, 0, 0, 0, 0, 0, 0]),
        (18, true, vec![0, 3, 0, 0, 0, 0, 0, 0]),
    ];

    for (window_line_counter, previous_tilemap_select, expected) in cases {
        let mut ppu = cgb_fetch_startup_rig(crate::model::OperatingMode::GbCompatible, 0xA3);
        ppu.visible_registers.lcdc = 0xA3;
        ppu.pipeline_registers.lcdc = 0xA3;
        ppu.window_state.window_line_counter = window_line_counter;
        let tilemap_y = window_line_counter / BG_TILE_WIDTH;
        ppu.write_bg_tilemap_entry(0, tilemap_y, 0);
        ppu.write_window_tilemap_entry(0, tilemap_y, 1);
        for row in 0..BG_TILE_WIDTH as usize {
            let tile0_row = 0x1000 + row * 2;
            let tile1_row = 0x1010 + row * 2;
            ppu.vram_bytes[tile0_row] = 0x00;
            ppu.vram_bytes[tile0_row + 1] = 0x00;
            ppu.vram_bytes[tile1_row] = 0xFF;
            ppu.vram_bytes[tile1_row + 1] = 0xFF;
        }

        let cached = BgCachedSlice {
            source: PpuBgFetcherSource::Window,
            fetch_x: 0,
            window_activation_first_pixel_previous_tilemap_select: Some(previous_tilemap_select),
            tile_map_address: 0x1C00 | (u16::from(tilemap_y) * BG_TILE_MAP_WIDTH as u16),
            tile_data_address: 0x1011,
            tile_low_address: 0x1010,
            tile_high_address: 0x1011,
            tile_index: 1,
            tile_low: 0xFF,
            tile_high: 0xFF,
            ..BgCachedSlice::default()
        };
        ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

        let pixels = (0..BG_TILE_WIDTH)
            .map(|_| {
                ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                    .expect("queued window slice should expose a visible pixel")
                    .color
            })
            .collect::<Vec<_>>();

        assert_eq!(pixels, expected);
    }
}

#[test]
fn native_cgb_skips_dmg_software_window_map_lead_in_profile() {
    let mut ppu = cgb_fetch_startup_rig(crate::model::OperatingMode::Cgb, 0xA3);
    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xA3;
    ppu.window_state.window_line_counter = 8;
    ppu.write_bg_tilemap_entry(0, 1, 0);
    ppu.write_window_tilemap_entry(0, 1, 1);
    ppu.vram_bytes[0x1000] = 0x00;
    ppu.vram_bytes[0x1001] = 0x00;
    ppu.vram_bytes[0x1010] = 0xFF;
    ppu.vram_bytes[0x1011] = 0xFF;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: 0,
        window_activation_first_pixel_previous_tilemap_select: Some(false),
        tile_map_address: 0x1C20,
        tile_data_address: 0x1011,
        tile_low_address: 0x1010,
        tile_high_address: 0x1011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0xFF,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let pixels = (0..BG_TILE_WIDTH)
        .map(|_| {
            ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                .expect("queued window slice should expose a visible pixel")
                .color
        })
        .collect::<Vec<_>>();

    assert_eq!(pixels, vec![3, 3, 3, 3, 3, 3, 3, 3]);
}

#[test]
fn second_window_tile_uses_the_oracle_mask_on_window_line_64() {
    let mut ppu = dmg_fetch_rig();
    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xA3;
    ppu.window_state.window_line_counter = 64;
    ppu.write_bg_tilemap_entry(1, 8, 0);
    ppu.write_window_tilemap_entry(1, 8, 1);
    ppu.vram_bytes[0x1000] = 0x00;
    ppu.vram_bytes[0x1001] = 0x00;
    ppu.vram_bytes[0x1010] = 0xFF;
    ppu.vram_bytes[0x1011] = 0xFF;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: BG_TILE_WIDTH as u16,
        window_activation_first_pixel_previous_tilemap_select: Some(false),
        tile_map_address: 0x1D01,
        tile_data_address: 0x1011,
        tile_low_address: 0x1010,
        tile_high_address: 0x1011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0xFF,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let pixels = (0..BG_TILE_WIDTH)
        .map(|_| {
            ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                .expect("queued window slice should expose a visible pixel")
                .color
        })
        .collect::<Vec<_>>();

    assert_eq!(pixels, vec![0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn third_window_tile_uses_the_oracle_mask_on_window_line_112() {
    let mut ppu = dmg_fetch_rig();
    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xA3;
    ppu.window_state.window_line_counter = 112;
    ppu.write_bg_tilemap_entry(2, 14, 0);
    ppu.write_window_tilemap_entry(2, 14, 1);
    ppu.vram_bytes[0x1000] = 0x00;
    ppu.vram_bytes[0x1001] = 0x00;
    ppu.vram_bytes[0x1010] = 0xFF;
    ppu.vram_bytes[0x1011] = 0xFF;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        window_activation_first_pixel_previous_tilemap_select: Some(false),
        tile_map_address: 0x1DC2,
        tile_data_address: 0x1011,
        tile_low_address: 0x1010,
        tile_high_address: 0x1011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0xFF,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let pixels = (0..BG_TILE_WIDTH)
        .map(|_| {
            ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                .expect("queued window slice should expose a visible pixel")
                .color
        })
        .collect::<Vec<_>>();

    assert_eq!(pixels, vec![0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn third_window_tile_uses_the_oracle_mask_on_window_line_128() {
    let mut ppu = dmg_fetch_rig();
    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xA3;
    ppu.window_state.window_line_counter = 128;
    ppu.write_bg_tilemap_entry(2, 16, 0);
    ppu.write_window_tilemap_entry(2, 16, 1);
    ppu.vram_bytes[0x1000] = 0x00;
    ppu.vram_bytes[0x1001] = 0x00;
    ppu.vram_bytes[0x1010] = 0xFF;
    ppu.vram_bytes[0x1011] = 0xFF;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        window_activation_first_pixel_previous_tilemap_select: Some(false),
        tile_map_address: 0x1E02,
        tile_data_address: 0x1011,
        tile_low_address: 0x1010,
        tile_high_address: 0x1011,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0xFF,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let pixels = (0..BG_TILE_WIDTH)
        .map(|_| {
            ppu.with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
                .expect("queued window slice should expose a visible pixel")
                .color
        })
        .collect::<Vec<_>>();

    assert_eq!(pixels, vec![3, 3, 3, 3, 3, 3, 3, 3]);
}

#[test]
fn first_window_activation_pixel_keeps_the_current_tilemap_on_window_line_27() {
    let mut ppu = dmg_fetch_rig();
    ppu.visible_registers.lcdc = 0xE3;
    ppu.pipeline_registers.lcdc = 0xA3;
    ppu.window_state.window_line_counter = 27;
    ppu.write_bg_tilemap_entry(0, 3, 0);
    ppu.write_window_tilemap_entry(0, 3, 1);
    ppu.vram_bytes[0x1006] = 0x00;
    ppu.vram_bytes[0x1007] = 0x00;
    ppu.vram_bytes[0x1016] = 0xFF;
    ppu.vram_bytes[0x1017] = 0xFF;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: 0,
        window_activation_first_pixel_previous_tilemap_select: Some(false),
        tile_map_address: 0x1C60,
        tile_data_address: 0x1017,
        tile_low_address: 0x1016,
        tile_high_address: 0x1017,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0xFF,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    let pixel = ppu
        .with_ppu_vram(|ppu, vram| ppu.pop_visible_bg_fifo_pixel(vram))
        .expect("queued window slice should expose a visible pixel");

    assert_eq!(pixel.color, 3);
}

#[test]
fn startup_scy_visible_tile2_previous_row_override_requires_a_live_scy_latch() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    push_selected_sprite_x(&mut ppu, 12);
    ppu.bg_pipeline_state.current_transfer_x = 12;
    ppu.write_bg_tile_row(2, 2, 0x08, 0x00);

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_index: 2,
        tile_high_address: 0x20 + 3 * TILE_ROW_BYTES + 1,
        ..BgCachedSlice::default()
    };

    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile2_previous_row_pixel(cached, 4, vram)
        }),
        None
    );

    ppu.bg_pipeline_state.startup_scy_tiledata_latch =
        Some(BgStartupScyTiledataLatch::new(0x91, 2));

    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile2_previous_row_pixel(cached, 4, vram)
        }),
        Some(1)
    );
}

#[test]
fn startup_scy_visible_tile3_previous_row_override_uses_the_latched_scy_class() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    push_selected_sprite_x(&mut ppu, 16);
    ppu.bg_pipeline_state.current_transfer_x = 16;
    ppu.bg_pipeline_state.startup_scy_tiledata_latch =
        Some(BgStartupScyTiledataLatch::new(0x91, 3));
    ppu.write_bg_tile_row(3, 3, 0x04, 0x00);

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        tile_index: 3,
        tile_high_address: 0x30 + 4 * TILE_ROW_BYTES + 1,
        ..BgCachedSlice::default()
    };

    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile3_previous_row_pixel(cached, 5, vram)
        }),
        Some(1)
    );
}

#[test]
fn startup_scy_visible_tile2_placeholder_reads_the_previous_tilemap_row() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 5;
    ppu.scy = 3;
    push_selected_sprite_x(&mut ppu, 16);
    ppu.bg_pipeline_state.current_transfer_x = 16;
    ppu.bg_pipeline_state.startup_scy_tiledata_latch =
        Some(BgStartupScyTiledataLatch::new(0x91, 7));
    ppu.write_bg_tilemap_entry(2, 0, 4);
    ppu.write_bg_tile_row(4, 7, 0x80, 0x00);

    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile2_scy_placeholder_pixel(16, vram)
        }),
        Some(1)
    );
}

#[test]
fn startup_scy_visible_tile2_placeholder_preserves_obj_mixing_priority() {
    let mut ppu = dmg_fetch_startup_rig(0x93);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 16;
    ppu.ly = 5;
    ppu.scy = 3;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 24;
    ppu.bg_pipeline_state.visible_pixels_output = 16;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.startup_scy_tiledata_latch =
        Some(BgStartupScyTiledataLatch::new(0x91, 7));
    push_selected_sprite_x(&mut ppu, 16);
    ppu.write_bg_tilemap_entry(2, 0, 4);
    ppu.write_bg_tile_row(4, 7, 0x80, 0x00);
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 2,
        palette_obp1: false,
        bg_over_obj: false,
        cgb_obj_attrs: None,
        sprite_x: 16,
        oam_index: 0,
    });

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(
        ppu.current_scanline_mixed_pixels[16],
        MixedPixel::object(2, false)
    );
    assert_eq!(ppu.current_scanline_pixels[16], 2);
}

#[test]
fn startup_scy_visible_tile2_tilemap_retarget_can_read_a_neighbor_row() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 6;
    push_selected_sprite_x(&mut ppu, 9);
    ppu.bg_pipeline_state.current_transfer_x = 9;
    ppu.bg_pipeline_state.startup_scy_tiledata_latch =
        Some(BgStartupScyTiledataLatch::new(0x91, 2));
    ppu.write_bg_tilemap_entry(0, 1, 1);
    ppu.write_bg_tile_row(1, 2, 0x08, 0x00);

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_map_address: 0x1800,
        tile_index: 0,
        tile_high_address: 2 * TILE_ROW_BYTES + 1,
        ..BgCachedSlice::default()
    };

    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile2_scy_tilemap_retarget_pixel(cached, 4, vram)
        }),
        Some(1)
    );
}

#[test]
fn cgb_dmg_software_startup_scy_tilemap_retarget_requires_a_live_scy_write_marker() {
    let mut ppu = cgb_fetch_startup_rig(crate::model::OperatingMode::GbCompatible, 0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    push_selected_sprite_x(&mut ppu, 2);
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.write_bg_tile_row(0, 1, 0x08, 0x00);

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_index: 0,
        tile_high_address: 2 * TILE_ROW_BYTES + 1,
        ..BgCachedSlice::default()
    };

    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile2_scy_tilemap_retarget_pixel(cached, 4, vram)
        }),
        None
    );

    ppu.bg_pipeline_state.cgb_dmg_scy_startup_retarget_active = true;

    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile2_scy_tilemap_retarget_pixel(cached, 4, vram)
        }),
        Some(1)
    );
}

#[test]
fn cgb_dmg_software_startup_scy_retarget_marker_requires_effective_row_change() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = cgb_fetch_startup_rig(operating_mode, 0x91);
        ppu.scy = 5;
        ppu.visible_registers.scy = 5;
        ppu.pipeline_registers = ppu.visible_registers;
        ppu.lcd_state = PpuLcdState::Enabled;
        ppu.bg_pipeline_state.mode3_started = true;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        push_selected_sprite_x(&mut ppu, 2);

        assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
        ppu.write_register(0xFF42, 5);
        assert!(
            !ppu.bg_pipeline_state.cgb_dmg_scy_startup_retarget_active,
            "{operating_mode:?} should ignore an SCY write that keeps the effective row"
        );

        ppu.write_register(0xFF42, 6);
        assert!(
            ppu.bg_pipeline_state.cgb_dmg_scy_startup_retarget_active,
            "{operating_mode:?} should arm the retarget marker once SCY changes the effective row"
        );
    }
}

#[test]
fn visible_tile3_scx_boundary_next_tile_retarget_reads_the_following_bg_tile() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.visible_registers.scx = 0;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.scx = 0;
    ppu.write_bg_tilemap_entry(3, 0, 5);
    ppu.write_bg_tile_row(5, 0, 0x02, 0x00);

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx: Some(0),
        ..BgCachedSlice::default()
    };

    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile3_scx_boundary_next_tile_output_retarget_pixel(
                cached, 6, vram,
            )
        }),
        Some(1)
    );
}

#[test]
fn visible_tile3_scx_low_band_shift_can_use_cached_or_next_tile_pixels() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.write_bg_tilemap_entry(3, 0, 6);
    ppu.write_bg_tile_row(6, 0, 0x02, 0x00);

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        tile_low: 0x40,
        tile_high: 0x00,
        startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx: Some(0),
        ..BgCachedSlice::default()
    };

    ppu.scx = 0x08;
    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile3_scx_low_band_shifted_pixel(cached, 0, vram)
        }),
        Some(1)
    );

    ppu.scx = 0x0B;
    assert_eq!(
        ppu.with_ppu_vram(|ppu, vram| {
            ppu.compute_startup_visible_tile3_scx_low_band_shifted_pixel(cached, 5, vram)
        }),
        Some(1)
    );
}

#[test]
fn ordinary_background_fetcher_carries_full_tilemap_refetch_on_scx_tile_column_change() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin = BgCachedSliceOrigin::Ordinary;
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 3;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;

    ppu.write_register(0xFF43, 0x08);
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());

    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.push.cached.origin,
        BgCachedSliceOrigin::Ordinary
    );
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tilemap_full_refetch
    );
}

fn live_write_registers(lcdc: u8, scx: u8) -> PpuVisibleRegisters {
    PpuVisibleRegisters {
        lcdc,
        scx,
        ..PpuVisibleRegisters::default()
    }
}

fn live_write_registers_with_scy(lcdc: u8, scx: u8, scy: u8) -> PpuVisibleRegisters {
    PpuVisibleRegisters {
        lcdc,
        scx,
        scy,
        ..PpuVisibleRegisters::default()
    }
}

#[test]
fn live_background_write_effects_ignore_dummy_fill_slices() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x99, 0x08),
    );

    let mut dummy_fill = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        same_cycle_live_tilemap_refetch_window_open: true,
        ..BgCachedSlice::default()
    };
    PpuMode3LiveBackgroundWriteEffects::for_fill_pending_slice(
        dummy_fill,
        PpuMode3LiveBackgroundRegister::Scx,
        write_context,
        false,
        0,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    )
    .apply_to_cached_slice(&mut dummy_fill);
    assert!(!dummy_fill.needs_live_tilemap_refetch);
    assert!(!dummy_fill.needs_live_tilemap_full_refetch);
}

#[test]
fn live_background_write_effects_keep_window_push_on_the_old_tilemap_after_lcdc6_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0xD1, 0x00),
    );

    let mut window_push = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        ..BgCachedSlice::default()
    };
    PpuMode3LiveBackgroundWriteEffects::for_push_pending_slice(
        window_push,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
        false,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    )
    .apply_to_cached_slice(&mut window_push);

    assert!(!window_push.needs_live_tilemap_refetch);
    assert!(!window_push.needs_live_tilemap_full_refetch);
}

#[test]
fn live_background_write_effects_keep_window_push_on_the_old_tiledata_after_lcdc4_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0xB3, 0x00),
        live_write_registers(0xA3, 0x00),
    );

    let mut window_push = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        ..BgCachedSlice::default()
    };
    PpuMode3LiveBackgroundWriteEffects::for_push_pending_slice(
        window_push,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
        false,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    )
    .apply_to_cached_slice(&mut window_push);

    assert!(!window_push.needs_live_tile_data_refetch);
}

#[test]
fn live_background_write_effects_retarget_next_window_push_on_lcdc4_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0xB3, 0x00),
        live_write_registers(0xA3, 0x00),
    );

    let mut window_push = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: 8,
        ..BgCachedSlice::default()
    };
    PpuMode3LiveBackgroundWriteEffects::for_push_pending_slice(
        window_push,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
        false,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    )
    .apply_to_cached_slice(&mut window_push);

    assert!(window_push.needs_live_tile_data_refetch);
}

#[test]
fn window_visible_fifo_marks_current_tail_before_window_push_on_lcdc6_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0xD1, 0x00),
    );

    let mut pipeline = BgPipelineState::default();
    for pixel_index in [5, 6, 7, 0, 1] {
        pipeline
            .fifo
            .push_back_cached_slot(Some(BgFifoPixelCached::new(
                BgCachedSlice {
                    source: PpuBgFetcherSource::Window,
                    tile_map_address: if pixel_index >= 5 { 0x1800 } else { 0x1801 },
                    ..BgCachedSlice::default()
                },
                pixel_index,
            )));
    }

    pipeline.mark_live_lcdc3_write_while_fifo_visible(
        write_context,
        BgFetcherState {
            source: PpuBgFetcherSource::Window,
            stage: PpuBgFetcherStage::TileIndex,
            ..BgFetcherState::default()
        },
        24,
        false,
    );

    let cached = pipeline.fifo.cached_pixels().collect::<Vec<_>>();
    assert!(
        cached
            .iter()
            .all(|cached| cached.cached.needs_live_tilemap_refetch)
    );
}

#[test]
fn window_visible_fifo_latches_the_previous_tilemap_for_the_first_window_tile() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0xA3, 0x00),
        live_write_registers(0xE3, 0x00),
    );

    let mut pipeline = BgPipelineState::default();
    for pixel_index in 0..BG_TILE_WIDTH {
        pipeline
            .fifo
            .push_back_cached_slot(Some(BgFifoPixelCached::new(
                BgCachedSlice {
                    source: PpuBgFetcherSource::Window,
                    fetch_x: 0,
                    tile_map_address: 0x1D00,
                    ..BgCachedSlice::default()
                },
                pixel_index,
            )));
    }

    pipeline.mark_live_lcdc3_write_while_fifo_visible(
        write_context,
        BgFetcherState {
            source: PpuBgFetcherSource::Window,
            stage: PpuBgFetcherStage::Push,
            ..BgFetcherState::default()
        },
        64,
        false,
    );

    let cached = pipeline.fifo.cached_pixels().collect::<Vec<_>>();
    assert!(cached.iter().all(|cached| {
        cached
            .cached
            .window_activation_first_pixel_previous_tilemap_select
            == Some(false)
    }));
}

#[test]
fn window_visible_fifo_marks_the_current_tail_during_window_push_on_lcdc6_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0xD1, 0x00),
    );

    let mut pipeline = BgPipelineState::default();
    for pixel_index in [5, 6, 7, 0, 1] {
        pipeline
            .fifo
            .push_back_cached_slot(Some(BgFifoPixelCached::new(
                BgCachedSlice {
                    source: PpuBgFetcherSource::Window,
                    tile_map_address: if pixel_index >= 5 { 0x1800 } else { 0x1801 },
                    ..BgCachedSlice::default()
                },
                pixel_index,
            )));
    }

    pipeline.mark_live_lcdc3_write_while_fifo_visible(
        write_context,
        BgFetcherState {
            source: PpuBgFetcherSource::Window,
            stage: PpuBgFetcherStage::Push,
            ..BgFetcherState::default()
        },
        24,
        false,
    );

    let cached = pipeline.fifo.cached_pixels().collect::<Vec<_>>();
    assert!(
        cached
            .iter()
            .all(|cached| cached.cached.needs_live_tilemap_refetch)
    );
}

#[test]
fn window_visible_fifo_preserves_row1_current_tail_pixel_on_second_lcdc6_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0xE3, 0x00),
        live_write_registers(0xA3, 0x00),
    );

    let mut pipeline = BgPipelineState::default();
    for pixel_index in 1..=7 {
        pipeline
            .fifo
            .push_back_cached_slot(Some(BgFifoPixelCached::new(
                BgCachedSlice {
                    source: PpuBgFetcherSource::Window,
                    tile_map_address: 0x1C60,
                    ..BgCachedSlice::default()
                },
                pixel_index,
            )));
    }

    pipeline.mark_live_lcdc3_write_while_fifo_visible(
        write_context,
        BgFetcherState {
            source: PpuBgFetcherSource::Window,
            stage: PpuBgFetcherStage::TileIndex,
            window_tilemap_x: 1,
            ..BgFetcherState::default()
        },
        1,
        false,
    );

    let cached = pipeline.fifo.cached_pixels().collect::<Vec<_>>();
    assert!(!cached[0].cached.needs_live_tilemap_refetch);
    assert!(
        cached[1..]
            .iter()
            .all(|cached| cached.cached.needs_live_tilemap_refetch)
    );
}

#[test]
fn window_visible_fifo_keeps_the_current_window_tiledata_after_lcdc4_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0xB3, 0x00),
        live_write_registers(0xA3, 0x00),
    );

    let mut pipeline = BgPipelineState::default();
    for pixel_index in 1..=7 {
        pipeline
            .fifo
            .push_back_cached_slot(Some(BgFifoPixelCached::new(
                BgCachedSlice {
                    source: PpuBgFetcherSource::Window,
                    fetch_x: 0,
                    tile_data_address: 0x0001,
                    ..BgCachedSlice::default()
                },
                pixel_index,
            )));
    }

    pipeline.mark_live_lcdc3_write_while_fifo_visible(
        write_context,
        BgFetcherState {
            source: PpuBgFetcherSource::Window,
            stage: PpuBgFetcherStage::TileIndex,
            fetch_x: 8,
            window_tilemap_x: 1,
            ..BgFetcherState::default()
        },
        0,
        false,
    );

    let cached = pipeline.fifo.cached_pixels().collect::<Vec<_>>();
    assert!(
        cached
            .iter()
            .all(|cached| !cached.cached.needs_live_tile_data_refetch)
    );
}

#[test]
fn window_visible_fifo_retargets_the_next_window_tiledata_after_lcdc4_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0xB3, 0x00),
        live_write_registers(0xA3, 0x00),
    );

    let mut pipeline = BgPipelineState::default();
    for pixel_index in [5, 6, 7, 0, 1] {
        pipeline
            .fifo
            .push_back_cached_slot(Some(BgFifoPixelCached::new(
                BgCachedSlice {
                    source: PpuBgFetcherSource::Window,
                    fetch_x: if pixel_index >= 5 { 0 } else { 8 },
                    tile_data_address: 0x0001,
                    ..BgCachedSlice::default()
                },
                pixel_index,
            )));
    }

    pipeline.mark_live_lcdc3_write_while_fifo_visible(
        write_context,
        BgFetcherState {
            source: PpuBgFetcherSource::Window,
            stage: PpuBgFetcherStage::TileIndex,
            fetch_x: 16,
            window_tilemap_x: 2,
            ..BgFetcherState::default()
        },
        0,
        false,
    );

    let cached = pipeline.fifo.cached_pixels().collect::<Vec<_>>();
    assert!(
        cached[..3]
            .iter()
            .all(|cached| !cached.cached.needs_live_tile_data_refetch)
    );
    assert!(
        cached[3..]
            .iter()
            .all(|cached| cached.cached.needs_live_tile_data_refetch)
    );
}

#[test]
fn live_background_write_effects_do_not_arm_window_push_refetch_for_unsigned_to_signed_lcdc4_flip()
{
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0xB3, 0x00),
        live_write_registers(0xA3, 0x00),
    );
    let mut fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Window,
        stage: PpuBgFetcherStage::TileDataLow,
        stage_dot: 1,
        fetch_x: 8,
        window_tilemap_x: 1,
        tile_index: 0,
        ..BgFetcherState::default()
    };

    fetcher.mark_live_register_write_for_current_background_fetch(
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
        32,
        32,
        PpuMode3LiveScyWriteRouting::default(),
    );

    assert!(!fetcher.needs_live_tile_data_refetch_on_push);
    assert!(fetcher.dmg_lcdc4_skip_window_current_low_glitch);
}

#[test]
fn live_lcdc4_write_uses_the_active_window_row_after_a_same_scanline_restart() {
    let mut ppu = dmg_live_window_restart_write_rig(0xB3);
    ppu.bg_pipeline_state.fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Window,
        stage: PpuBgFetcherStage::TileDataLow,
        stage_dot: 1,
        fetch_x: 8,
        window_tilemap_x: 1,
        tile_index: 0,
        ..BgFetcherState::default()
    };

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);

    ppu.write_register(0xFF40, 0xA3);

    assert!(
        ppu.bg_pipeline_state
            .fetcher
            .dmg_lcdc4_skip_window_current_low_glitch
    );
}

#[test]
fn live_lcdc3_fifo_write_uses_the_active_window_row_after_a_same_scanline_restart() {
    let mut ppu = dmg_live_window_restart_write_rig(0x91);
    ppu.bg_pipeline_state.fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Window,
        stage: PpuBgFetcherStage::Push,
        ..BgFetcherState::default()
    };
    for pixel_index in [5, 6, 7, 0, 1] {
        ppu.bg_pipeline_state
            .fifo
            .push_back_cached_slot(Some(BgFifoPixelCached::new(
                BgCachedSlice {
                    source: PpuBgFetcherSource::Window,
                    tile_map_address: if pixel_index >= 5 { 0x1800 } else { 0x1801 },
                    ..BgCachedSlice::default()
                },
                pixel_index,
            )));
    }

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);

    ppu.write_register(0xFF40, 0xD1);

    assert!(
        ppu.bg_pipeline_state
            .fifo
            .cached_pixels()
            .all(|cached| cached.cached.needs_live_tilemap_refetch)
    );
}

#[test]
fn window_lcdc4_unsigned_to_signed_previous_plane_masks_match_observed_row_blocks() {
    assert_eq!(
        crate::ppu::mode3::window_lcdc4_unsigned_to_signed_previous_plane_masks(0, 24),
        Some(PerPlane::new(0xFF, 0xFF))
    );
    assert_eq!(
        crate::ppu::mode3::window_lcdc4_unsigned_to_signed_previous_plane_masks(8, 24),
        Some(PerPlane::new(0xFF, 0x00))
    );
    assert_eq!(
        crate::ppu::mode3::window_lcdc4_unsigned_to_signed_previous_plane_masks(0, 40),
        Some(PerPlane::new(0xE0, 0xFF))
    );
    assert_eq!(
        crate::ppu::mode3::window_lcdc4_unsigned_to_signed_previous_plane_masks(8, 40),
        Some(PerPlane::new(0xFF, 0xFF))
    );
    assert_eq!(
        crate::ppu::mode3::window_lcdc4_unsigned_to_signed_previous_plane_masks(0, 64),
        Some(PerPlane::new(0x3C, 0x3C))
    );
    assert_eq!(
        crate::ppu::mode3::window_lcdc4_unsigned_to_signed_previous_plane_masks(8, 64),
        Some(PerPlane::new(0x00, 0xFF))
    );
    assert_eq!(
        crate::ppu::mode3::window_lcdc4_unsigned_to_signed_previous_plane_masks(16, 128),
        Some(PerPlane::new(0xFF, 0x00))
    );
    assert_eq!(
        crate::ppu::mode3::window_lcdc4_unsigned_to_signed_previous_plane_masks(16, 136),
        Some(PerPlane::new(0xFF, 0x00))
    );
}

#[test]
fn cgb_dmg_software_window_lcdc4_previous_plane_masks_match_paired_write_phases() {
    assert_eq!(
        crate::ppu::mode3::cgb_dmg_software_window_lcdc4_signed_to_unsigned_previous_plane_masks(
            0, 24,
        ),
        Some(PerPlane::new(0x7F, 0x00))
    );
    assert_eq!(
        crate::ppu::mode3::cgb_dmg_software_window_lcdc4_signed_to_unsigned_previous_plane_masks(
            8, 64,
        ),
        Some(PerPlane::new(0xFF, 0x00))
    );
    assert_eq!(
        crate::ppu::mode3::cgb_dmg_software_window_lcdc4_unsigned_to_signed_previous_plane_masks(
            8, 16,
        ),
        Some(PerPlane::new(0xFF, 0x00))
    );
    assert_eq!(
        crate::ppu::mode3::cgb_dmg_software_window_lcdc4_unsigned_to_signed_previous_plane_masks(
            16, 128,
        ),
        Some(PerPlane::new(0xFF, 0x00))
    );
    assert_eq!(
        crate::ppu::mode3::cgb_dmg_software_window_lcdc4_unsigned_to_signed_previous_plane_masks(
            0, 24,
        ),
        None
    );
}

#[test]
fn dmg_lcdc4_output_override_marks_window_seam_slices() {
    let mut pipeline = BgPipelineState {
        fetcher: BgFetcherState {
            source: PpuBgFetcherSource::Window,
            fetch_x: 8,
            ..BgFetcherState::default()
        },
        push: BgPushState {
            pending: true,
            cached: BgCachedSlice {
                source: PpuBgFetcherSource::Window,
                fetch_x: 16,
                ..BgCachedSlice::default()
            },
            ..BgPushState::default()
        },
        fill: BgFifoFillState {
            pending: true,
            cached: BgCachedSlice {
                source: PpuBgFetcherSource::Window,
                fetch_x: 24,
                ..BgCachedSlice::default()
            },
            ..BgFifoFillState::default()
        },
        ..BgPipelineState::default()
    };
    pipeline
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Window,
                fetch_x: 0,
                ..BgCachedSlice::default()
            },
            4,
        )));

    pipeline.apply_dmg_lcdc4_output_override_to_window_seam_slices(BgTileDataSelect::Unsigned8000);

    assert_eq!(
        pipeline
            .fetcher
            .dmg_lcdc4_previous_tiledata_select_for_output_override,
        Some(BgTileDataSelect::Unsigned8000)
    );
    assert_eq!(
        pipeline
            .push
            .cached
            .dmg_lcdc4_previous_tiledata_select_for_output_override,
        Some(BgTileDataSelect::Unsigned8000)
    );
    assert_eq!(
        pipeline
            .fill
            .cached
            .dmg_lcdc4_previous_tiledata_select_for_output_override,
        None
    );
    assert_eq!(
        pipeline
            .fifo
            .cached_front()
            .flatten()
            .and_then(|cached| cached
                .cached
                .dmg_lcdc4_previous_tiledata_select_for_output_override),
        Some(BgTileDataSelect::Unsigned8000)
    );
}

#[test]
fn dmg_lcdc4_output_override_up_to_leaves_later_window_seam_slices_unmarked() {
    let mut pipeline = BgPipelineState {
        fetcher: BgFetcherState {
            source: PpuBgFetcherSource::Window,
            fetch_x: 0,
            ..BgFetcherState::default()
        },
        push: BgPushState {
            pending: true,
            cached: BgCachedSlice {
                source: PpuBgFetcherSource::Window,
                fetch_x: 8,
                ..BgCachedSlice::default()
            },
            ..BgPushState::default()
        },
        ..BgPipelineState::default()
    };
    pipeline
        .fifo
        .push_back_cached_slot(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Window,
                fetch_x: 0,
                ..BgCachedSlice::default()
            },
            4,
        )));

    pipeline.apply_dmg_lcdc4_output_override_to_window_seam_slices_up_to(
        BgTileDataSelect::Signed8800,
        0,
    );

    assert_eq!(
        pipeline
            .fetcher
            .dmg_lcdc4_previous_tiledata_select_for_output_override,
        Some(BgTileDataSelect::Signed8800)
    );
    assert_eq!(
        pipeline
            .push
            .cached
            .dmg_lcdc4_previous_tiledata_select_for_output_override,
        None
    );
    assert_eq!(
        pipeline
            .fifo
            .cached_front()
            .flatten()
            .and_then(|cached| cached
                .cached
                .dmg_lcdc4_previous_tiledata_select_for_output_override),
        Some(BgTileDataSelect::Signed8800)
    );
}

#[test]
fn cgb_dmg_software_signed_lcdc4_output_override_is_one_shot_after_window_push_queue() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = PpuTestRig::with_model(ConsoleModel::GameBoyColor);
        ppu.apply_operating_mode_state(operating_mode);
        ppu.visible_registers.lcdc = 0xB1;
        ppu.pipeline_registers = ppu.visible_registers;
        ppu.bg_pipeline_state.window_lcdc5_latch = true;
        ppu.bg_pipeline_state.fetcher = BgFetcherState {
            source: PpuBgFetcherSource::Window,
            stage: PpuBgFetcherStage::TileDataHigh,
            stage_dot: 1,
            fetch_x: 0,
            dmg_lcdc4_previous_tiledata_select_for_output_override: Some(
                BgTileDataSelect::Signed8800,
            ),
            ..BgFetcherState::default()
        };
        assert_eq!(
            ppu.bg_pipeline_state
                .fetcher
                .dmg_lcdc4_previous_tiledata_select_for_output_override,
            Some(BgTileDataSelect::Signed8800)
        );

        assert!(!ppu.advance_bg_fetcher_with_ppu_vram());

        assert!(
            ppu.bg_pipeline_state.push.pending,
            "{operating_mode:?} should queue a push"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .dmg_lcdc4_previous_tiledata_select_for_output_override,
            Some(BgTileDataSelect::Signed8800),
            "{operating_mode:?} should keep the override on the queued slice"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .fetcher
                .dmg_lcdc4_previous_tiledata_select_for_output_override,
            None,
            "{operating_mode:?} should not let the signed->unsigned override leak into the next tile"
        );
    }
}

#[test]
fn cgb_dmg_software_lcdc4_signed_to_unsigned_write_marks_only_current_window_fetch() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = cgb_fetch_startup_rig(operating_mode, 0xA3);
        ppu.window_state.window_line_counter = 56;
        ppu.bg_pipeline_state.fetcher = BgFetcherState {
            source: PpuBgFetcherSource::Window,
            fetch_x: 0,
            ..BgFetcherState::default()
        };
        ppu.bg_pipeline_state.push = BgPushState {
            pending: true,
            cached: BgCachedSlice {
                source: PpuBgFetcherSource::Window,
                fetch_x: 8,
                ..BgCachedSlice::default()
            },
            ..BgPushState::default()
        };

        ppu.apply_dmg_lcdc4_live_bg_tiledata_write(PpuMode3LiveRegisterWriteContext::new(
            live_write_registers(0xA3, 0x00),
            live_write_registers(0xB3, 0x00),
        ));

        assert_eq!(
            ppu.bg_pipeline_state
                .fetcher
                .dmg_lcdc4_previous_tiledata_select_for_output_override,
            Some(BgTileDataSelect::Signed8800),
            "{operating_mode:?} should tag the current fetch"
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .push
                .cached
                .dmg_lcdc4_previous_tiledata_select_for_output_override,
            None,
            "{operating_mode:?} should leave later fetch_x slices available for the paired write"
        );
        assert_eq!(
            ppu.pending_dmg_window_lcdc4_output_repaint,
            Some(BgTileDataSelect::Signed8800)
        );
    }
}

#[test]
fn cgb_dmg_software_lcdc4_unsigned_to_signed_write_marks_window_from_row16() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = cgb_fetch_startup_rig(operating_mode, 0xB3);
        ppu.window_state.window_line_counter = 16;
        ppu.bg_pipeline_state.fetcher = BgFetcherState {
            source: PpuBgFetcherSource::Window,
            fetch_x: 8,
            ..BgFetcherState::default()
        };

        ppu.apply_dmg_lcdc4_live_bg_tiledata_write(PpuMode3LiveRegisterWriteContext::new(
            live_write_registers(0xB3, 0x00),
            live_write_registers(0xA3, 0x00),
        ));

        assert_eq!(
            ppu.bg_pipeline_state
                .fetcher
                .dmg_lcdc4_previous_tiledata_select_for_output_override,
            Some(BgTileDataSelect::Unsigned8000),
            "{operating_mode:?} should tag the CGB lead-in row"
        );
        assert_eq!(
            ppu.pending_dmg_window_lcdc4_output_repaint,
            Some(BgTileDataSelect::Unsigned8000)
        );
    }
}

#[test]
fn native_cgb_lcdc4_window_writes_skip_dmg_software_output_override() {
    let mut ppu = cgb_fetch_startup_rig(crate::model::OperatingMode::Cgb, 0xA3);
    ppu.window_state.window_line_counter = 56;
    ppu.bg_pipeline_state.fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Window,
        fetch_x: 0,
        ..BgFetcherState::default()
    };

    ppu.apply_dmg_lcdc4_live_bg_tiledata_write(PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0xA3, 0x00),
        live_write_registers(0xB3, 0x00),
    ));

    assert_eq!(
        ppu.bg_pipeline_state
            .fetcher
            .dmg_lcdc4_previous_tiledata_select_for_output_override,
        None
    );
    assert_eq!(ppu.pending_dmg_window_lcdc4_output_repaint, None);
}

#[test]
fn window_visible_fifo_preserves_row2_current_tail_pixel_on_second_lcdc6_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0xE3, 0x00),
        live_write_registers(0xA3, 0x00),
    );

    let mut pipeline = BgPipelineState::default();
    for pixel_index in 1..=7 {
        pipeline
            .fifo
            .push_back_cached_slot(Some(BgFifoPixelCached::new(
                BgCachedSlice {
                    source: PpuBgFetcherSource::Window,
                    tile_map_address: 0x1C60,
                    ..BgCachedSlice::default()
                },
                pixel_index,
            )));
    }

    pipeline.mark_live_lcdc3_write_while_fifo_visible(
        write_context,
        BgFetcherState {
            source: PpuBgFetcherSource::Window,
            stage: PpuBgFetcherStage::TileIndex,
            window_tilemap_x: 1,
            ..BgFetcherState::default()
        },
        2,
        false,
    );

    let cached = pipeline.fifo.cached_pixels().collect::<Vec<_>>();
    assert!(cached[0].cached.needs_live_tilemap_refetch);
    assert!(!cached[1].cached.needs_live_tilemap_refetch);
    assert!(
        cached[2..]
            .iter()
            .all(|cached| cached.cached.needs_live_tilemap_refetch)
    );
}

#[test]
fn live_background_write_effects_mark_window_current_fetch_tilemap_refetch_on_lcdc6_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0xD1, 0x00),
    );
    let fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Window,
        stage: PpuBgFetcherStage::TileDataLow,
        ..BgFetcherState::default()
    };
    let effects = PpuMode3LiveBackgroundWriteEffects::for_current_background_fetch(
        fetcher,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
        0,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    );

    let mut cached = BgCachedSlice::default();
    effects.apply_to_cached_slice(&mut cached);
    assert!(cached.needs_live_tilemap_refetch);

    let mut fetcher = fetcher;
    effects.apply_to_fetcher(&mut fetcher);
    assert!(fetcher.needs_live_tilemap_refetch_on_push);
}

#[test]
fn live_background_write_effects_keep_window_push_stage_on_the_old_tilemap_after_lcdc6_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0xD1, 0x00),
    );
    let fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Window,
        stage: PpuBgFetcherStage::Push,
        ..BgFetcherState::default()
    };
    let effects = PpuMode3LiveBackgroundWriteEffects::for_current_background_fetch(
        fetcher,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
        0,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    );

    let mut cached = BgCachedSlice::default();
    effects.apply_to_cached_slice(&mut cached);
    assert!(!cached.needs_live_tilemap_refetch);

    let mut fetcher = fetcher;
    effects.apply_to_fetcher(&mut fetcher);
    assert!(!fetcher.needs_live_tilemap_refetch_on_push);
}

#[test]
fn dmg_window_tileindex_glitch_keeps_the_current_tilemap() {
    let mut ppu = dmg_fetch_rig();
    ppu.visible_registers.lcdc = 0xA3;
    ppu.pipeline_registers.lcdc = 0xE3;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1C41;
    ppu.bg_pipeline_state.fetcher.tile_index = 7;

    ppu.with_ppu_vram(|ppu, vram| {
        ppu.maybe_apply_bgwin_tilemap_selector_glitch(vram, PpuBgFetcherSource::Window);
    });

    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C41);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 7);
}

#[test]
fn live_background_tilemap_refetch_recomputes_window_slice_on_lcdc6_change() {
    let mut ppu = dmg_fetch_rig();
    ppu.vram_bytes[0x1801] = 0;
    ppu.vram_bytes[0x0000] = 0x12;
    ppu.vram_bytes[0x0001] = 0x34;
    ppu.vram_bytes[0x1C01] = 1;
    ppu.vram_bytes[0x0010] = 0xAB;
    ppu.vram_bytes[0x0011] = 0xCD;
    ppu.lcdc = 0xD1;
    ppu.visible_registers.lcdc = 0xD1;
    ppu.pipeline_registers.lcdc = 0xD1;
    ppu.window_state.window_line_counter = 0;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: BG_TILE_WIDTH as u16,
        needs_live_tilemap_refetch: true,
        tile_map_address: 0x1801,
        tile_data_address: 0x0001,
        tile_low_address: 0x0000,
        tile_high_address: 0x0001,
        tile_index: 0,
        tile_low: 0x12,
        tile_high: 0x34,
        ..BgCachedSlice::default()
    };

    let recomputed = ppu
        .with_ppu_vram(|ppu, vram| {
            recompute_live_background_cached_slice(
                cached,
                vram,
                ppu.current_mode3_live_background_refetch_context(),
            )
        })
        .expect("LCDC6 live write should recompute the cached window slice");

    assert_eq!(recomputed.tile_map_address, 0x1C01);
    assert_eq!(recomputed.tile_index, 1);
    assert_eq!(recomputed.tile_low, 0xAB);
    assert_eq!(recomputed.tile_high, 0xCD);
}

#[test]
fn live_scy_write_routing_tracks_selected_sprite_phase_classes() {
    let mut high_plane = dmg_fetch_startup_rig(0x91);
    push_selected_sprite_x(&mut high_plane, 13);
    high_plane.bg_pipeline_state.current_transfer_x = 13;
    let routing = high_plane.live_scy_write_routing(PpuMode3LiveBackgroundRegister::Scy);
    assert!(routing.pending_high_plane_only);
    assert!(!routing.pending_tilemap_row_refetch);
    assert!(routing.startup_visible_tile2_tilemap_row_refetch);
    assert!(!routing.startup_visible_tile2_phase6_tilemap_row_refetch);

    let mut phase6 = dmg_fetch_startup_rig(0x91);
    push_selected_sprite_x(&mut phase6, 4);
    phase6.bg_pipeline_state.current_transfer_x = 4;
    let routing = phase6.live_scy_write_routing(PpuMode3LiveBackgroundRegister::Scy);
    assert!(!routing.pending_high_plane_only);
    assert!(!routing.pending_tilemap_row_refetch);
    assert!(routing.startup_visible_tile2_tilemap_row_refetch);
    assert!(routing.startup_visible_tile2_phase6_tilemap_row_refetch);

    let lcdc_routing = phase6.live_scy_write_routing(PpuMode3LiveBackgroundRegister::Lcdc);
    assert_eq!(lcdc_routing, PpuMode3LiveScyWriteRouting::default());
}

#[test]
fn live_background_write_effects_mark_pending_push_scy_tile_row_refetch_in_live_window() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers_with_scy(0x91, 0x00, 0),
        live_write_registers_with_scy(0x91, 0x00, 1),
    );
    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        same_cycle_live_tilemap_refetch_window_open: true,
        ..BgCachedSlice::default()
    };

    PpuMode3LiveBackgroundWriteEffects::for_push_pending_slice(
        cached,
        PpuMode3LiveBackgroundRegister::Scy,
        write_context,
        false,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    )
    .apply_to_cached_slice(&mut cached);

    assert!(!cached.needs_live_tilemap_refetch);
    assert!(!cached.needs_live_tilemap_full_refetch);
    assert!(cached.needs_live_tile_data_refetch);
    assert!(cached.needs_live_tile_data_current_row_refetch);
}

#[test]
fn live_background_write_effects_can_limit_scy_refetch_to_high_plane() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers_with_scy(0x91, 0x00, 0),
        live_write_registers_with_scy(0x91, 0x00, 1),
    );
    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        same_cycle_live_tilemap_refetch_window_open: true,
        ..BgCachedSlice::default()
    };

    PpuMode3LiveBackgroundWriteEffects::for_push_pending_slice(
        cached,
        PpuMode3LiveBackgroundRegister::Scy,
        write_context,
        false,
        0,
        PpuMode3LiveScyWriteRouting {
            pending_high_plane_only: true,
            ..PpuMode3LiveScyWriteRouting::default()
        },
    )
    .apply_to_cached_slice(&mut cached);

    assert!(cached.needs_live_tile_data_refetch);
    assert!(!cached.needs_live_tile_data_current_row_refetch);
    assert!(!cached.needs_live_tile_low_current_row_refetch);
    assert!(cached.needs_live_tile_high_current_row_refetch);
}

#[test]
fn live_background_write_effects_mark_pending_push_scy_tilemap_row_refetch() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers_with_scy(0x91, 0x00, 0),
        live_write_registers_with_scy(0x91, 0x00, 1),
    );
    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        same_cycle_live_tilemap_refetch_window_open: true,
        ..BgCachedSlice::default()
    };

    PpuMode3LiveBackgroundWriteEffects::for_push_pending_slice(
        cached,
        PpuMode3LiveBackgroundRegister::Scy,
        write_context,
        false,
        7,
        PpuMode3LiveScyWriteRouting {
            pending_tilemap_row_refetch: true,
            ..PpuMode3LiveScyWriteRouting::default()
        },
    )
    .apply_to_cached_slice(&mut cached);

    assert!(cached.needs_live_tilemap_refetch);
    assert!(cached.needs_live_tilemap_full_refetch);
    assert!(cached.needs_live_tile_data_refetch);
    assert!(cached.needs_live_tile_data_current_row_refetch);
}

#[test]
fn live_background_write_effects_mark_startup_visible_tile2_scy_tilemap_phase6_refetch() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers_with_scy(0x91, 0x00, 0),
        live_write_registers_with_scy(0x91, 0x00, 8),
    );
    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        ..BgCachedSlice::default()
    };

    PpuMode3LiveBackgroundWriteEffects::for_push_pending_slice(
        cached,
        PpuMode3LiveBackgroundRegister::Scy,
        write_context,
        false,
        6,
        PpuMode3LiveScyWriteRouting {
            startup_visible_tile2_tilemap_row_refetch: true,
            startup_visible_tile2_phase6_tilemap_row_refetch: true,
            ..PpuMode3LiveScyWriteRouting::default()
        },
    )
    .apply_to_cached_slice(&mut cached);

    assert!(cached.needs_live_tilemap_refetch);
    assert!(cached.needs_live_tilemap_full_refetch);
    assert!(!cached.needs_live_tile_data_refetch);
}

#[test]
fn live_background_write_effects_mark_fill_full_refetch_on_scx_tile_column_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x91, 0x08),
    );
    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        same_cycle_live_tilemap_refetch_window_open: true,
        ..BgCachedSlice::default()
    };

    PpuMode3LiveBackgroundWriteEffects::for_fill_pending_slice(
        cached,
        PpuMode3LiveBackgroundRegister::Scx,
        write_context,
        true,
        0,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    )
    .apply_to_cached_slice(&mut cached);

    assert!(cached.needs_live_tilemap_refetch);
    assert!(cached.needs_live_tilemap_full_refetch);
}

#[test]
fn live_background_write_effects_mark_startup_fill_full_refetch_on_scx_tile_column_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x91, 0x08),
    );
    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
        fetch_x: BG_TILE_WIDTH as u16 * 2,
        ..BgCachedSlice::default()
    };

    PpuMode3LiveBackgroundWriteEffects::for_fill_pending_slice(
        cached,
        PpuMode3LiveBackgroundRegister::Scx,
        write_context,
        true,
        0,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    )
    .apply_to_cached_slice(&mut cached);

    assert!(cached.needs_live_tilemap_refetch);
    assert!(cached.needs_live_tilemap_full_refetch);
}

#[test]
fn live_background_write_effects_mark_fill_scy_high_plane_only_refetch() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers_with_scy(0x91, 0x00, 0),
        live_write_registers_with_scy(0x91, 0x00, 1),
    );
    let mut cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        same_cycle_live_tilemap_refetch_window_open: true,
        ..BgCachedSlice::default()
    };

    PpuMode3LiveBackgroundWriteEffects::for_fill_pending_slice(
        cached,
        PpuMode3LiveBackgroundRegister::Scy,
        write_context,
        true,
        0,
        0,
        PpuMode3LiveScyWriteRouting {
            pending_high_plane_only: true,
            ..PpuMode3LiveScyWriteRouting::default()
        },
    )
    .apply_to_cached_slice(&mut cached);

    assert!(cached.needs_live_tile_data_refetch);
    assert!(!cached.needs_live_tile_data_current_row_refetch);
    assert!(cached.needs_live_tile_high_current_row_refetch);
}

#[test]
fn live_background_write_effects_mark_current_fetcher_scy_tiledata_on_push() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers_with_scy(0x91, 0x00, 0),
        live_write_registers_with_scy(0x91, 0x00, 1),
    );
    let fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Background,
        stage: PpuBgFetcherStage::TileDataLow,
        ..BgFetcherState::default()
    };
    let effects = PpuMode3LiveBackgroundWriteEffects::for_current_background_fetch(
        fetcher,
        PpuMode3LiveBackgroundRegister::Scy,
        write_context,
        0,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    );

    let mut fetcher = fetcher;
    effects.apply_to_fetcher(&mut fetcher);
    assert!(!fetcher.needs_live_tilemap_refetch_on_push);
    assert!(!fetcher.needs_live_tilemap_full_refetch_on_push);
    assert!(fetcher.needs_live_tile_data_refetch_on_push);
    assert!(fetcher.needs_live_tile_data_current_row_refetch_on_push);
    assert!(!fetcher.needs_live_tile_low_current_row_refetch_on_push);
    assert!(!fetcher.needs_live_tile_high_current_row_refetch_on_push);
}

#[test]
fn live_background_write_effects_mark_current_fetcher_startup_visible_tile2_scy_tilemap_on_push() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers_with_scy(0x91, 0x00, 0),
        live_write_registers_with_scy(0x91, 0x00, 8),
    );
    let fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Background,
        stage: PpuBgFetcherStage::TileDataHigh,
        cached_origin: BgCachedSliceOrigin::StartupContinuation(
            BgStartupContinuationSlice::VisibleTile2,
        ),
        ..BgFetcherState::default()
    };
    let effects = PpuMode3LiveBackgroundWriteEffects::for_current_background_fetch(
        fetcher,
        PpuMode3LiveBackgroundRegister::Scy,
        write_context,
        6,
        0,
        PpuMode3LiveScyWriteRouting {
            startup_visible_tile2_tilemap_row_refetch: true,
            startup_visible_tile2_phase6_tilemap_row_refetch: true,
            ..PpuMode3LiveScyWriteRouting::default()
        },
    );

    let mut fetcher = fetcher;
    effects.apply_to_fetcher(&mut fetcher);
    assert!(fetcher.needs_live_tilemap_refetch_on_push);
    assert!(fetcher.needs_live_tilemap_full_refetch_on_push);
}

#[test]
fn live_background_refetch_can_mix_tile_data_plane_rows_after_scy_write() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(0, 1, 0x56, 0x78);
    ppu.scy = 1;
    ppu.ly = 0;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        needs_live_tile_data_refetch: true,
        needs_live_tile_high_current_row_refetch: true,
        tile_map_address: 0x1800,
        tile_data_address: 0x0001,
        tile_low_address: 0x0000,
        tile_high_address: 0x0001,
        tile_index: 0,
        tile_low: 0x12,
        tile_high: 0x34,
        ..BgCachedSlice::default()
    };

    let recomputed = ppu
        .with_ppu_vram(|ppu, vram| {
            recompute_live_background_cached_slice(
                cached,
                vram,
                ppu.current_mode3_live_background_refetch_context(),
            )
        })
        .expect("SCY live write should recompute the cached BG slice");

    assert_eq!(recomputed.tile_low_address, 0x0000);
    assert_eq!(recomputed.tile_high_address, 0x0003);
    assert_eq!(recomputed.tile_low, 0x12);
    assert_eq!(recomputed.tile_high, 0x78);
}

#[test]
fn startup_scy_latch_marks_alignment_fifo_push_and_fill_cached_slices() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers_with_scy(0x91, 0x00, 0),
        live_write_registers_with_scy(0x91, 0x00, 1),
    );
    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupAlignmentFill,
        tile_index: 2,
        ..BgCachedSlice::default()
    };
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::AlignmentSeedPending;
    ppu.bg_pipeline_state.push.cached = cached;
    ppu.bg_pipeline_state.fill.cached = cached;
    ppu.bg_pipeline_state.push_cached_slice_fifo_pixels(cached);

    ppu.bg_pipeline_state
        .mark_live_scy_write_while_startup_alignment_fifo_visible(write_context, 0);

    let fifo_cached = ppu
        .bg_pipeline_state
        .fifo
        .cached_front()
        .flatten()
        .expect("startup alignment cached pixel should remain queued")
        .cached;
    assert!(fifo_cached.needs_live_tile_data_refetch);
    assert_eq!(fifo_cached.tile_low_address, 0x20 + TILE_ROW_BYTES);
    assert_eq!(fifo_cached.tile_high_address, 0x20 + TILE_ROW_BYTES + 1);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_refetch
    );
    assert!(
        ppu.bg_pipeline_state
            .fill
            .cached
            .needs_live_tile_data_refetch
    );
}

#[test]
fn startup_scy_latch_can_be_applied_to_a_later_fill_slice() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.startup_scy_tiledata_latch =
        Some(BgStartupScyTiledataLatch::new(0x91, 3));
    ppu.bg_pipeline_state.fill.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupAlignmentFill,
        tile_index: 1,
        ..BgCachedSlice::default()
    };

    ppu.bg_pipeline_state
        .apply_startup_scy_tiledata_latch_to_fill();

    assert!(
        ppu.bg_pipeline_state
            .fill
            .cached
            .needs_live_tile_data_refetch
    );
    assert_eq!(
        ppu.bg_pipeline_state.fill.cached.tile_low_address,
        0x10 + 3 * TILE_ROW_BYTES
    );
}

#[test]
fn live_background_write_effects_mark_visible_tile3_current_fetch_on_lcdc3_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x99, 0x00),
    );
    let fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Background,
        stage: PpuBgFetcherStage::TileDataLow,
        cached_origin: BgCachedSliceOrigin::StartupContinuation(
            BgStartupContinuationSlice::VisibleTile3,
        ),
        ..BgFetcherState::default()
    };
    let effects = PpuMode3LiveBackgroundWriteEffects::for_current_background_fetch(
        fetcher,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
        0,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    );

    let mut cached = BgCachedSlice::default();
    effects.apply_to_cached_slice(&mut cached);
    assert!(cached.needs_live_tilemap_refetch);

    let mut fetcher = fetcher;
    effects.apply_to_fetcher(&mut fetcher);
    assert!(fetcher.needs_live_tilemap_refetch_on_push);
}

#[test]
fn live_background_write_effects_mark_visible_tile3_high_byte_fetch_on_lcdc3_change() {
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x91, 0x00),
        live_write_registers(0x99, 0x00),
    );
    let fetcher = BgFetcherState {
        source: PpuBgFetcherSource::Background,
        stage: PpuBgFetcherStage::TileDataHigh,
        cached_origin: BgCachedSliceOrigin::StartupContinuation(
            BgStartupContinuationSlice::VisibleTile3,
        ),
        ..BgFetcherState::default()
    };
    let effects = PpuMode3LiveBackgroundWriteEffects::for_current_background_fetch(
        fetcher,
        PpuMode3LiveBackgroundRegister::Lcdc,
        write_context,
        0,
        0,
        PpuMode3LiveScyWriteRouting::default(),
    );

    let mut cached = BgCachedSlice::default();
    effects.apply_to_cached_slice(&mut cached);
    assert!(cached.needs_live_tilemap_refetch);

    let mut fetcher = fetcher;
    effects.apply_to_fetcher(&mut fetcher);
    assert!(fetcher.needs_live_tilemap_refetch_on_push);
}

#[test]
fn live_background_tilemap_refetch_uses_latched_lcdc3_override_instead_of_current_registers() {
    let mut ppu = dmg_fetch_rig();
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_window_tilemap_entry(1, 0, 1);
    ppu.vram_bytes[0x1000] = 0x12;
    ppu.vram_bytes[0x1001] = 0x34;
    ppu.vram_bytes[0x1010] = 0xAB;
    ppu.vram_bytes[0x1011] = 0xCD;
    ppu.lcdc = 0x83;
    ppu.visible_registers.lcdc = 0x83;
    ppu.pipeline_registers.lcdc = 0x83;

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        dmg_lcdc3_tilemap_select_override: Some(true),
        needs_live_tilemap_refetch: true,
        tile_map_address: 0x1801,
        tile_data_address: 0x0001,
        tile_low_address: 0x0000,
        tile_high_address: 0x0001,
        tile_index: 0,
        tile_low: 0x12,
        tile_high: 0x34,
        ..BgCachedSlice::default()
    };

    let recomputed = ppu
        .with_ppu_vram(|ppu, vram| {
            recompute_live_background_cached_slice(
                cached,
                vram,
                ppu.current_mode3_live_background_refetch_context(),
            )
        })
        .expect("latched LCDC3 override should recompute the cached BG slice");

    assert_eq!(recomputed.tile_map_address, 0x1C01);
    assert_eq!(recomputed.tile_index, 1);
    assert_eq!(recomputed.tile_low, 0xAB);
    assert_eq!(recomputed.tile_high, 0xCD);
    assert_eq!(recomputed.dmg_lcdc3_tilemap_select_override, None);
}

#[test]
fn pending_dmg_lcdc3_startup_override_latches_future_visible_tile3_push() {
    let mut pipeline = BgPipelineState::default();
    pipeline
        .dmg_mode3_live_lcdc_bg_state
        .startup_continuation_overrides
        .lcdc3_tilemap_select = StartupContinuationSliceOverrides {
        visible_tile2: None,
        visible_tile3: Some(true),
    };
    pipeline.push.pending = true;
    pipeline.push.cached.source = PpuBgFetcherSource::Background;
    pipeline.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    pipeline.push.cached.tile_map_address = 0x1802;
    pipeline.push.cached.tile_index = 0;

    pipeline.maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_push();

    assert_eq!(
        pipeline.push.cached.dmg_lcdc3_tilemap_select_override,
        Some(true)
    );
    assert!(pipeline.push.cached.needs_live_tilemap_refetch);
}

#[test]
fn pending_dmg_lcdc3_startup_override_latches_future_visible_tile2_fill() {
    let mut pipeline = BgPipelineState::default();
    pipeline
        .dmg_mode3_live_lcdc_bg_state
        .startup_continuation_overrides
        .lcdc3_tilemap_select = StartupContinuationSliceOverrides {
        visible_tile2: Some(true),
        visible_tile3: None,
    };
    pipeline.fill.pending = true;
    pipeline.fill.cached.source = PpuBgFetcherSource::Background;
    pipeline.fill.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    pipeline.fill.cached.tile_map_address = 0x1801;
    pipeline.fill.cached.tile_index = 0;

    pipeline.maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_fill();

    assert_eq!(
        pipeline.fill.cached.dmg_lcdc3_tilemap_select_override,
        Some(true)
    );
    assert!(pipeline.fill.cached.needs_live_tilemap_refetch);
}

#[test]
fn first_dmg_lcdc3_bg_map_write_clears_visible_tile2_live_refetch_for_single_sprite_lines() {
    let mut rig = dmg_fetch_rig();
    push_selected_sprite_x(&mut rig, 4);
    rig.ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        needs_live_tilemap_refetch: true,
        dmg_lcdc3_tilemap_select_override: Some(false),
        ..BgCachedSlice::default()
    };
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x83, 0x00),
        live_write_registers(0x8B, 0x00),
    );

    rig.ppu.apply_dmg_lcdc3_live_bg_tilemap_write(write_context);

    assert_eq!(
        rig.ppu
            .bg_pipeline_state
            .dmg_mode3_live_lcdc_bg_state
            .lcdc3_current_line_bg_tilemap_write_count,
        1
    );
    assert!(
        !rig.ppu
            .bg_pipeline_state
            .push
            .cached
            .needs_live_tilemap_refetch
    );
    assert_eq!(
        rig.ppu
            .bg_pipeline_state
            .push
            .cached
            .dmg_lcdc3_tilemap_select_override,
        None
    );
    assert_eq!(
        rig.ppu
            .bg_pipeline_state
            .dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides
            .lcdc3_tilemap_select
            .visible_tile2,
        None
    );
    assert_eq!(
        rig.ppu
            .bg_pipeline_state
            .dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides
            .lcdc3_tilemap_select
            .visible_tile3,
        Some(true)
    );
}

#[test]
fn second_dmg_lcdc3_bg_map_write_for_left_edge_sprite_retargets_visible_tile2_only() {
    let mut rig = dmg_fetch_rig();
    push_selected_sprite_x(&mut rig, 2);
    rig.ppu
        .bg_pipeline_state
        .dmg_mode3_live_lcdc_bg_state
        .lcdc3_current_line_bg_tilemap_write_count = 1;
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x83, 0x00),
        live_write_registers(0x8B, 0x00),
    );

    rig.ppu.apply_dmg_lcdc3_live_bg_tilemap_write(write_context);

    assert_eq!(
        rig.ppu
            .bg_pipeline_state
            .dmg_mode3_live_lcdc_bg_state
            .lcdc3_current_line_bg_tilemap_write_count,
        2
    );
    assert_eq!(
        rig.ppu
            .bg_pipeline_state
            .dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides
            .lcdc3_tilemap_select
            .visible_tile2,
        Some(true)
    );
    assert_eq!(
        rig.ppu
            .bg_pipeline_state
            .dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides
            .lcdc3_tilemap_select
            .visible_tile3,
        None
    );
}

#[test]
fn second_dmg_lcdc3_bg_map_write_for_x16_plus_clears_visible_tile2_live_refetch() {
    let mut rig = dmg_fetch_rig();
    push_selected_sprite_x(&mut rig, 16);
    rig.ppu
        .bg_pipeline_state
        .dmg_mode3_live_lcdc_bg_state
        .lcdc3_current_line_bg_tilemap_write_count = 1;
    rig.ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        needs_live_tilemap_refetch: true,
        dmg_lcdc3_tilemap_select_override: Some(true),
        ..BgCachedSlice::default()
    };
    let write_context = PpuMode3LiveRegisterWriteContext::new(
        live_write_registers(0x8B, 0x00),
        live_write_registers(0x83, 0x00),
    );

    rig.ppu.apply_dmg_lcdc3_live_bg_tilemap_write(write_context);

    assert_eq!(
        rig.ppu
            .bg_pipeline_state
            .dmg_mode3_live_lcdc_bg_state
            .lcdc3_current_line_bg_tilemap_write_count,
        2
    );
    assert!(
        !rig.ppu
            .bg_pipeline_state
            .push
            .cached
            .needs_live_tilemap_refetch
    );
    assert_eq!(
        rig.ppu
            .bg_pipeline_state
            .push
            .cached
            .dmg_lcdc3_tilemap_select_override,
        None
    );
}

#[test]
fn cgb_dmg_software_lcdc3_bg_map_write_retargets_visible_tile3_for_low_sprite_phases() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut rig = cgb_fetch_startup_rig(operating_mode, 0x83);
        push_selected_sprite_x(&mut rig, 1);
        rig.ppu.bg_pipeline_state.push.cached = BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupContinuation(
                BgStartupContinuationSlice::VisibleTile2,
            ),
            needs_live_tilemap_refetch: true,
            dmg_lcdc3_tilemap_select_override: Some(false),
            ..BgCachedSlice::default()
        };
        let write_context = PpuMode3LiveRegisterWriteContext::new(
            live_write_registers(0x83, 0x00),
            live_write_registers(0x8B, 0x00),
        );

        rig.ppu.apply_dmg_lcdc3_live_bg_tilemap_write(write_context);

        assert!(
            !rig.ppu
                .bg_pipeline_state
                .push
                .cached
                .needs_live_tilemap_refetch,
            "{operating_mode:?} should clear the visible-tile2 live refetch"
        );
        assert_eq!(
            rig.ppu
                .bg_pipeline_state
                .dmg_mode3_live_lcdc_bg_state
                .startup_continuation_overrides
                .lcdc3_tilemap_select
                .visible_tile2,
            None,
            "{operating_mode:?} should not retarget visible tile2"
        );
        assert_eq!(
            rig.ppu
                .bg_pipeline_state
                .dmg_mode3_live_lcdc_bg_state
                .startup_continuation_overrides
                .lcdc3_tilemap_select
                .visible_tile3,
            Some(true),
            "{operating_mode:?} should retarget visible tile3"
        );
    }
}

#[test]
fn cached_background_fill_recomputes_tilemap_before_the_next_flush_when_same_tcycle_window_is_open()
{
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_window_tilemap_entry(1, 0, 1);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(1, 0, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 0;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;
    ppu.bg_pipeline_state
        .fill
        .cached
        .same_cycle_live_tilemap_refetch_window_open = true;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.fill.cached.needs_live_tilemap_refetch);

    ppu.maybe_recompute_pending_background_fill_with_ppu_vram();
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_map_address, 0x1C01);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_index, 1);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_data_address, 0x0011);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0xCD);
}

#[test]
fn third_startup_continuation_fill_marks_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fill.cached.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;

    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.fill.cached.needs_live_tilemap_refetch);
}

#[test]
fn ordinary_cached_fill_ignores_lcdc3_write_without_the_narrow_live_window() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1803;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;

    ppu.write_register(0xFF40, 0x99);
    assert!(!ppu.bg_pipeline_state.fill.cached.needs_live_tilemap_refetch);
}

#[test]
fn queue_from_push_preserves_the_same_tcycle_tilemap_refetch_window() {
    let mut fill = BgFifoFillState::default();
    let mut push = BgPushState {
        pending: true,
        ..BgPushState::default()
    };
    push.cached.source = PpuBgFetcherSource::Background;
    push.cached.same_cycle_live_tilemap_refetch_window_open = true;

    fill.queue_from_push(push);

    assert!(fill.cached.same_cycle_live_tilemap_refetch_window_open);
}

#[test]
fn queued_fill_from_real_push_preserves_the_same_tcycle_tilemap_refetch_window() {
    let mut ppu = dmg_fetch_rig();

    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state
        .push
        .cached
        .same_cycle_live_tilemap_refetch_window_open = true;

    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::QueuedFill);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert!(
        ppu.bg_pipeline_state
            .fill
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
}

#[test]
fn cached_background_fill_keeps_fetched_tiledata_after_scy_write_before_flush() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(0, 1, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 0;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF42, 0x01);
    assert!(
        !ppu.bg_pipeline_state
            .fill
            .cached
            .needs_live_tile_data_refetch
    );
    assert!(
        !ppu.bg_pipeline_state
            .fill
            .cached
            .needs_live_tile_data_current_row_refetch
    );

    ppu.maybe_recompute_pending_background_fill_with_ppu_vram();
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_data_address, 0x0001);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0x34);
}

#[test]
fn cached_background_push_keeps_fetched_tiledata_after_scy_write_before_flush() {
    let mut ppu = dmg_fetch_startup_rig(0x91);
    ppu.write_bg_tilemap_entry(1, 0, 0);
    ppu.write_bg_tile_row(0, 0, 0x12, 0x34);
    ppu.write_bg_tile_row(0, 1, 0xAB, 0xCD);
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF42, 0x01);
    assert!(
        !ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_refetch
    );
    assert!(
        !ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_current_row_refetch
    );

    ppu.with_ppu_vram(|ppu, vram| ppu.maybe_recompute_pending_background_push(vram));
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_data_address, 0x0001);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_high, 0x34);
}

#[test]
fn bg_fetcher_rereads_the_unsigned_tile_data_byte_when_tile_selector_flips_to_unsigned_on_low1() {
    let mut ppu = dmg_fetch_rig();
    ppu.vram_bytes[0x1010] = 0x12;
    ppu.vram_bytes[0x0010] = 0x56;
    ppu.visible_registers.lcdc = 0x81;
    ppu.pipeline_registers.lcdc = 0x81;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;

    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1010);

    ppu.pipeline_registers.lcdc = 0x81;
    ppu.visible_registers.lcdc = 0x91;
    assert!(!ppu.advance_bg_fetcher_with_ppu_vram());
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x56);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0010);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
}

#[test]
fn window_lcdc4_output_override_uses_observed_previous_plane_masks_for_window_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.window_state.window_line_counter = 40;
    ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(PpuVisibleRegisters {
        lcdc: LCDC_BG_ENABLE_BIT,
        bgp: 0xE4,
        ..PpuVisibleRegisters::default()
    }));

    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: 8,
        tile_index: 0,
        dmg_lcdc4_previous_tiledata_select_for_output_override: Some(
            BgTileDataSelect::Unsigned8000,
        ),
        ..BgCachedSlice::default()
    };
    let mut vram_bytes = [0; 0x2000];
    vram_bytes[0x0000] = 0x80;
    vram_bytes[0x0001] = 0x80;
    vram_bytes[0x1000] = 0x00;
    vram_bytes[0x1001] = 0x00;

    with_test_vram_view(vram_bytes, |vram| {
        assert_eq!(
            ppu.test_compute_window_lcdc4_tiledata_selector_override(cached, 0, vram),
            Some(3)
        );
        assert_eq!(
            ppu.test_compute_window_lcdc4_tiledata_selector_override(
                BgCachedSlice {
                    source: PpuBgFetcherSource::Background,
                    ..cached
                },
                0,
                vram,
            ),
            None
        );
    });
}

#[test]
fn cgb_dmg_software_window_lcdc4_output_override_reconstructs_signed_to_unsigned_pixels() {
    for operating_mode in [
        crate::model::OperatingMode::GbCompatible,
        crate::model::OperatingMode::CgbDmgExt,
    ] {
        let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
        ppu.apply_operating_mode_state(operating_mode);
        ppu.window_state.window_line_counter = 24;
        ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(PpuVisibleRegisters {
            lcdc: LCDC_BG_ENABLE_BIT | LCDC_BG_WINDOW_TILE_DATA_BIT,
            bgp: 0xE4,
            ..PpuVisibleRegisters::default()
        }));

        let cached = BgCachedSlice {
            source: PpuBgFetcherSource::Window,
            fetch_x: 0,
            tile_index: 0,
            dmg_lcdc4_previous_tiledata_select_for_output_override: Some(
                BgTileDataSelect::Signed8800,
            ),
            ..BgCachedSlice::default()
        };
        let mut vram_bytes = [0; 0x2000];
        vram_bytes[0x0000] = 0x00;
        vram_bytes[0x0001] = 0x00;
        vram_bytes[0x1000] = 0xFF;
        vram_bytes[0x1001] = 0x00;

        with_test_vram_view(vram_bytes, |vram| {
            assert_eq!(
                ppu.test_compute_window_lcdc4_tiledata_selector_override(cached, 0, vram),
                Some(0),
                "{operating_mode:?} should use the current selector for the unmasked leading pixel"
            );
            assert_eq!(
                ppu.test_compute_window_lcdc4_tiledata_selector_override(cached, 1, vram),
                Some(1),
                "{operating_mode:?} should use the previous signed selector for masked low-plane pixels"
            );
        });
    }
}

#[test]
fn native_cgb_skips_dmg_software_window_lcdc4_signed_to_unsigned_override() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(crate::model::OperatingMode::Cgb);
    ppu.window_state.window_line_counter = 24;
    ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(PpuVisibleRegisters {
        lcdc: LCDC_BG_ENABLE_BIT | LCDC_BG_WINDOW_TILE_DATA_BIT,
        bgp: 0xE4,
        ..PpuVisibleRegisters::default()
    }));
    let cached = BgCachedSlice {
        source: PpuBgFetcherSource::Window,
        fetch_x: 0,
        tile_index: 0,
        dmg_lcdc4_previous_tiledata_select_for_output_override: Some(BgTileDataSelect::Signed8800),
        ..BgCachedSlice::default()
    };

    with_test_vram_view([0; 0x2000], |vram| {
        assert_eq!(
            ppu.test_compute_window_lcdc4_tiledata_selector_override(cached, 1, vram),
            None
        );
    });
}

#[test]
fn window_lcdc4_output_override_from_context_reconstructs_previous_window_dot_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.window_state.window_line_counter = 40;
    ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(PpuVisibleRegisters {
        lcdc: LCDC_BG_ENABLE_BIT,
        bgp: 0xE4,
        ..PpuVisibleRegisters::default()
    }));
    let context = PpuRecentBgDotContext {
        source: PpuBgFetcherSource::Window,
        fetch_x: 8,
        pixel_index: 0,
        tile_index: 0,
    };
    let mut vram_bytes = [0; 0x2000];
    vram_bytes[0x0000] = 0x80;
    vram_bytes[0x0001] = 0x80;
    vram_bytes[0x1000] = 0x00;
    vram_bytes[0x1001] = 0x00;

    with_test_vram_view(vram_bytes, |vram| {
        assert_eq!(
            ppu.test_compute_window_lcdc4_tiledata_selector_override_from_context(
                context,
                BgTileDataSelect::Unsigned8000,
                vram,
            ),
            Some(3)
        );
        assert_eq!(
            ppu.test_compute_window_lcdc4_tiledata_selector_override_from_context(
                PpuRecentBgDotContext {
                    source: PpuBgFetcherSource::Background,
                    ..context
                },
                BgTileDataSelect::Unsigned8000,
                vram,
            ),
            None
        );
    });
}

#[test]
fn pending_window_lcdc4_output_repaint_updates_background_owned_visible_pixels_and_panel_history() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.window_state.window_line_counter = 40;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.pending_dmg_window_lcdc4_output_repaint = Some(BgTileDataSelect::Unsigned8000);
    ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(PpuVisibleRegisters {
        lcdc: LCDC_BG_ENABLE_BIT,
        bgp: 0xE4,
        ..PpuVisibleRegisters::default()
    }));
    ppu.current_scanline_bg_dot_contexts[0] = Some(PpuRecentBgDotContext {
        source: PpuBgFetcherSource::Window,
        fetch_x: 8,
        pixel_index: 0,
        tile_index: 0,
    });
    ppu.current_scanline_bg_pixels[0] = 0;
    ppu.current_scanline_mixed_pixels[0] = MixedPixel::background(0);
    ppu.current_scanline_pixels[0] = 0;
    ppu.current_scanline_dmg_bg_forced_white[0] = false;
    ppu.framebuffer[0] = 0;
    ppu.dmg_panel_live_write_state
        .recent_panel_dots
        .push_back(PpuRecentPanelDot {
            visible_x: 0,
            pixel: MixedPixel::background(0),
            dmg_bg_forced_white: false,
        });
    let mut vram_bytes = [0; 0x2000];
    vram_bytes[0x0000] = 0x80;
    vram_bytes[0x0001] = 0x80;
    vram_bytes[0x1000] = 0x00;
    vram_bytes[0x1001] = 0x00;

    with_test_vram_view(vram_bytes, |vram| {
        ppu.test_apply_pending_dmg_window_lcdc4_output_repaint(vram)
    });

    assert_eq!(ppu.pending_dmg_window_lcdc4_output_repaint, None);
    assert_eq!(ppu.current_scanline_bg_pixels[0], 3);
    assert_eq!(
        ppu.current_scanline_mixed_pixels[0],
        MixedPixel::background(3)
    );
    assert!(!ppu.current_scanline_dmg_bg_forced_white[0]);
    assert_eq!(ppu.current_scanline_pixels[0], 3);
    assert_eq!(ppu.framebuffer[0], 3);
    assert_eq!(
        ppu.dmg_panel_live_write_state.recent_panel_dots[0],
        PpuRecentPanelDot {
            visible_x: 0,
            pixel: MixedPixel::background(3),
            dmg_bg_forced_white: false,
        }
    );
}

#[test]
fn pending_window_lcdc4_output_repaint_keeps_object_owned_pixels_mixed_while_refreshing_bg_history()
{
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.ly = 0;
    ppu.window_state.window_line_counter = 40;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.pending_dmg_window_lcdc4_output_repaint = Some(BgTileDataSelect::Unsigned8000);
    ppu.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(PpuVisibleRegisters {
        lcdc: LCDC_BG_ENABLE_BIT,
        bgp: 0xE4,
        obp0: Some(0xE4),
        ..PpuVisibleRegisters::default()
    }));
    ppu.current_scanline_bg_dot_contexts[0] = Some(PpuRecentBgDotContext {
        source: PpuBgFetcherSource::Window,
        fetch_x: 8,
        pixel_index: 0,
        tile_index: 0,
    });
    ppu.current_scanline_bg_pixels[0] = 0;
    ppu.current_scanline_mixed_pixels[0] = MixedPixel::object(1, false);
    ppu.current_scanline_pixels[0] = 1;
    ppu.current_scanline_dmg_bg_forced_white[0] = false;
    ppu.framebuffer[0] = 1;
    ppu.dmg_panel_live_write_state
        .recent_panel_dots
        .push_back(PpuRecentPanelDot {
            visible_x: 0,
            pixel: MixedPixel::object(1, false),
            dmg_bg_forced_white: false,
        });
    let mut vram_bytes = [0; 0x2000];
    vram_bytes[0x0000] = 0x80;
    vram_bytes[0x0001] = 0x80;
    vram_bytes[0x1000] = 0x00;
    vram_bytes[0x1001] = 0x00;

    with_test_vram_view(vram_bytes, |vram| {
        ppu.test_apply_pending_dmg_window_lcdc4_output_repaint(vram)
    });

    assert_eq!(ppu.current_scanline_bg_pixels[0], 3);
    assert_eq!(
        ppu.current_scanline_mixed_pixels[0],
        MixedPixel::object(1, false)
    );
    assert_eq!(ppu.current_scanline_pixels[0], 1);
    assert_eq!(ppu.framebuffer[0], 1);
    assert_eq!(
        ppu.dmg_panel_live_write_state.recent_panel_dots[0],
        PpuRecentPanelDot {
            visible_x: 0,
            pixel: MixedPixel::object(1, false),
            dmg_bg_forced_white: false,
        }
    );
}
