use super::*;

#[test]
fn observed_lcdc2_obj_size_plane_selection_matches_the_curated_residual_seams() {
    let cases = [
        (
            0,
            8,
            0,
            0,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            0,
            16,
            0,
            0,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High),
        ),
        (
            0,
            24,
            0,
            0,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (
            2,
            33,
            0,
            8,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High),
        ),
        (
            2,
            40,
            0,
            8,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (
            0,
            12,
            4,
            8,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (0, 12, 4, 2, None, None),
        (
            0,
            32,
            0,
            2,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (0, 32, 0, 10, None, None),
        (
            0,
            32,
            1,
            2,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (
            0,
            32,
            2,
            10,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (0, 32, 3, 2, None, None),
        (
            0,
            32,
            4,
            2,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (2, 32, 0, 2, None, None),
        (
            2,
            32,
            0,
            4,
            Some(25),
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16LowLive8High),
        ),
        (
            2,
            32,
            0,
            2,
            Some(23),
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (2, 12, 3, 8, None, None),
        (2, 12, 4, 8, None, None),
        (0, 17, 0, 0, None, None),
        (
            2,
            34,
            0,
            0,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            2,
            39,
            0,
            12,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (2, 34, 1, 0, None, None),
    ];

    for (write_index, sprite_x, scx, raw_row, active_write_visible_x, expected) in cases {
        assert_eq!(
            PpuMode3ObservedLcdc2ObjSizePhaseTable::new(sprite_x, scx, raw_row)
                .plane_selection(write_index, active_write_visible_x),
            expected,
            "write_index={write_index} sprite_x={sprite_x} scx={scx} raw_row={raw_row} active_write_visible_x={active_write_visible_x:?}",
        );
    }
}

#[test]
fn observed_cgb_dmg_software_lcdc2_obj_size_plane_selection_tracks_cgb_residual_seams() {
    let cases = [
        (
            0,
            8,
            0,
            0,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            0,
            16,
            0,
            0,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            0,
            24,
            0,
            0,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (
            2,
            33,
            0,
            8,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            2,
            40,
            0,
            8,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (
            0,
            12,
            4,
            8,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            0,
            32,
            0,
            1,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            0,
            32,
            0,
            2,
            Some(10),
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (
            0,
            32,
            0,
            7,
            Some(10),
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (
            0,
            32,
            0,
            2,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            0,
            32,
            0,
            7,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            0,
            32,
            4,
            4,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            0,
            32,
            4,
            4,
            Some(4),
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16LowLive8High),
        ),
        (
            0,
            32,
            4,
            4,
            Some(5),
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            0,
            32,
            5,
            4,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16LowLive8High),
        ),
        (
            0,
            32,
            7,
            3,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8),
        ),
        (
            2,
            32,
            0,
            4,
            Some(25),
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High),
        ),
        (
            2,
            32,
            0,
            4,
            Some(24),
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::Live8LowLineStart16High),
        ),
        (0, 32, 0, 10, None, None),
        (
            0,
            32,
            1,
            2,
            None,
            Some(PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16),
        ),
        (2, 12, 4, 8, None, None),
    ];

    for (write_index, sprite_x, scx, raw_row, active_write_visible_x, expected) in cases {
        assert_eq!(
            PpuMode3ObservedLcdc2ObjSizePhaseTable::cgb_dmg_software(sprite_x, scx, raw_row)
                .plane_selection(write_index, active_write_visible_x),
            expected,
            "write_index={write_index} sprite_x={sprite_x} scx={scx} raw_row={raw_row} active_write_visible_x={active_write_visible_x:?}",
        );
    }
}

#[test]
fn observed_lcdc2_obj_size_decision_reports_pending_effects_in_policy_space() {
    let repaint = PpuMode3ObservedLcdc2ObjSizePhaseTable::new(12, 4, 8)
        .decision(0, Some(6))
        .expect("write 0 scx-shifted overlap should produce a decision");
    assert_eq!(
        repaint.plane_selection,
        PpuMode3Lcdc2ObjSizePlaneSelection::Live8
    );
    assert_eq!(
        repaint.pending_effect,
        Some(PpuMode3Lcdc2ObjSizeObservedEffect::RetroactiveRepaint {
            background_only: false,
        })
    );

    let fifo_rewrite = PpuMode3ObservedLcdc2ObjSizePhaseTable::new(32, 0, 4)
        .decision(2, Some(25))
        .expect("write 2 late-tail seam should produce a decision");
    assert_eq!(
        fifo_rewrite.plane_selection,
        PpuMode3Lcdc2ObjSizePlaneSelection::LineStart16LowLive8High
    );
    assert_eq!(
        fifo_rewrite.pending_effect,
        Some(PpuMode3Lcdc2ObjSizeObservedEffect::FifoRewrite)
    );
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
fn observed_cgb_dmg_software_lcdc1_disable_onset_tracks_cgb_windows() {
    let cases = [
        (0, Some(0)),
        (1, Some(1)),
        (2, Some(2)),
        (3, Some(3)),
        (4, Some(4)),
        (5, Some(5)),
        (6, Some(5)),
        (7, Some(5)),
        (8, Some(4)),
        (13, Some(9)),
        (16, None),
    ];

    for (sprite_x, expected_onset) in cases {
        assert_eq!(
            PpuMode3SingleSpritePhasePolicy::new(sprite_x)
                .cgb_dmg_software_lcdc1_disable_onset_visible_x(),
            expected_onset,
            "sprite_x={sprite_x}",
        );
    }
}
