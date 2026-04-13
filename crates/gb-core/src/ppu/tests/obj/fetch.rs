use super::*;

#[test]
fn object_fetch_reads_tile_and_attributes_from_live_oam_metadata() {
    let sprite = PpuSelectedSprite {
        oam_index: 3,
        y: 16,
        x: 24,
        tile_index: 0x11,
        attributes: 0x22,
    };
    let mut oam_bytes = [0; 160];
    write_oam_entry_with_attributes(
        &mut oam_bytes,
        sprite.oam_index,
        sprite.y,
        sprite.x,
        0x44,
        0xA0,
    );

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let oam = OamBusView::new(BusMaster::Ppu, &mut oam);

    let (tile_index, attributes) = read_obj_fetch_sprite_metadata(&oam, sprite, None);

    assert_eq!(tile_index, 0x44);
    assert_eq!(attributes, 0xA0);
}

#[test]
fn object_fetch_uses_the_dma_conflict_word_address_for_late_oam_metadata_reads() {
    let sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 24,
        tile_index: 0x11,
        attributes: 0x22,
    };
    let mut oam_bytes = [0; 160];
    write_oam_entry_with_attributes(
        &mut oam_bytes,
        sprite.oam_index,
        sprite.y,
        sprite.x,
        0x44,
        0xA0,
    );
    write_oam_entry_with_attributes(&mut oam_bytes, 5, 32, 40, 0x99, 0x10);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let oam = OamBusView::new(BusMaster::Ppu, &mut oam);

    let (tile_index, attributes) =
        read_obj_fetch_sprite_metadata(&oam, sprite, Some(PpuDmaOamConflict::new(0xFE17, 0x77)));

    assert_eq!(tile_index, 0x99);
    assert_eq!(attributes, 0x77);
}

#[test]
fn object_fetch_uses_the_current_dma_byte_for_even_conflict_word_reads() {
    let sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 24,
        tile_index: 0x11,
        attributes: 0x22,
    };
    let mut oam_bytes = [0; 160];
    write_oam_entry_with_attributes(
        &mut oam_bytes,
        sprite.oam_index,
        sprite.y,
        sprite.x,
        0x44,
        0xA0,
    );
    write_oam_entry_with_attributes(&mut oam_bytes, 5, 32, 40, 0x99, 0x10);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let oam = OamBusView::new(BusMaster::Ppu, &mut oam);

    let (tile_index, attributes) =
        read_obj_fetch_sprite_metadata(&oam, sprite, Some(PpuDmaOamConflict::new(0xFE16, 0x66)));

    assert_eq!(tile_index, 0x66);
    assert_eq!(attributes, 0x10);
}

#[test]
fn smaller_raw_obj_x_values_start_fetch_earlier_during_mode3_startup() {
    fn fetch_start_line_dot(sprite_x: u8) -> u16 {
        let mut ppu = dmg_obj_render_rig(ObjRenderRigConfig { lcdc: 0x82, ly: 0 });
        ppu.write_oam_entry(0, 16, sprite_x, 0);
        ppu.write_bg_tile_row(0, 0, 0xFF, 0x00);

        for _ in 0..160 {
            ppu.tick();
            let snapshot = ppu.snapshot();
            if snapshot.obj_fetcher_stage != PpuObjFetcherStage::Idle {
                return snapshot.line_dot;
            }
        }

        panic!("sprite fetch did not begin during early Mode 3");
    }

    let left_edge = fetch_start_line_dot(1);
    let first_visible = fetch_start_line_dot(8);

    assert!(left_edge < first_visible);
    assert!(left_edge >= MODE2_DOTS);
    assert!(first_visible >= MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1);
}

#[test]
fn overlapped_obj_fetch_uses_explicit_one_dot_stage_progression() {
    let mut ppu = PpuTestRig::dmg();
    ppu.write_oam_entry(0, 16, 8, 0);
    ppu.write_bg_tile_row(0, 0, 0x55, 0x33);

    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    push_selected_sprite(&mut ppu, SelectedSpriteSpec::new(0, 16, 8, 0, 0));
    queue_current_obj_hit(&mut ppu, 0);

    assert!(
        ppu.try_start_object_fetch_from_current_dot(ObjFetchStartSource::FifoBackedTransfer, true,)
    );
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::Startup
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Push);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Push);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

    assert!(ppu.advance_object_fetch_with_ppu_video(None));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
}
