use super::*;

#[test]
fn performance_hud_renderer_draws_into_the_framebuffer() {
    let mut frame = vec![255_u8; 160 * 144 * 3];
    render_performance_hud(
        &mut frame,
        160,
        144,
        PerformanceHudSnapshot {
            fps: 59.8,
            speed_percent: 100.0,
            frame_time_ms: 16.7,
            emulation_time_ms: 11.0,
            render_time_ms: 2.0,
            pacing_time_ms: 3.0,
            audio_queue_ms: Some(18.0),
            rewind: RewindHudSnapshot::default(),
        },
    );

    assert!(
        frame.iter().any(|component| *component != 255),
        "HUD rendering should modify the destination framebuffer"
    );
}

#[test]
fn rewind_indicator_renderer_draws_into_the_top_right_corner() {
    let mut frame = vec![255_u8; 160 * 144 * 3];
    render_rewind_indicator(&mut frame, 160, 144);

    let top_right_region_start = 4 * 160 * 3 + 112 * 3;
    let top_right_region_end = 24 * 160 * 3;
    assert!(
        frame[top_right_region_start..top_right_region_end]
            .iter()
            .any(|component| *component != 255),
        "rewind indicator should modify the top-right framebuffer region"
    );
    assert_eq!(
        glyph_rows('<'),
        [
            0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
        ],
        "rewind indicator text requires the left chevron glyph"
    );
}

#[test]
fn fast_forward_indicator_renderer_draws_into_the_top_right_corner() {
    let mut frame = vec![255_u8; 160 * 144 * 3];
    render_fast_forward_indicator(&mut frame, 160, 144);

    let top_right_region_start = 4 * 160 * 3 + 112 * 3;
    let top_right_region_end = 24 * 160 * 3;
    assert!(
        frame[top_right_region_start..top_right_region_end]
            .iter()
            .any(|component| *component != 255),
        "fast-forward indicator should modify the top-right framebuffer region"
    );
    assert_eq!(
        glyph_rows('>'),
        [
            0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
        ],
        "fast-forward indicator text requires the right chevron glyph"
    );
    assert_ne!(glyph_rows(':'), [0; super::super::GLYPH_HEIGHT]);
    assert_ne!(glyph_rows('✓'), [0; super::super::GLYPH_HEIGHT]);
}

#[test]
fn cgb_ir_indicator_lines_report_one_player_init_readiness() {
    let ready = CgbInfraredParticipantHudSnapshot {
        read_enabled: true,
        sensor_warmed: true,
        ..CgbInfraredParticipantHudSnapshot::default()
    };

    assert_eq!(
        cgb_ir_indicator_lines(CgbInfraredHudSnapshot {
            p1: ready,
            p2: ready,
        }),
        ["IR READY".to_string(), "P1 RDY P2 RDY".to_string()]
    );
}

#[test]
fn cgb_ir_indicator_lines_prioritize_active_optical_state() {
    let transmitting = CgbInfraredParticipantHudSnapshot {
        emitter_on: true,
        read_enabled: true,
        sensor_warmed: true,
        optical_input_active: true,
        ..CgbInfraredParticipantHudSnapshot::default()
    };
    let receiving = CgbInfraredParticipantHudSnapshot {
        read_enabled: true,
        sensor_warmed: true,
        effective_signal_detected: true,
        optical_input_active: true,
        ..CgbInfraredParticipantHudSnapshot::default()
    };

    assert_eq!(
        cgb_ir_indicator_lines(CgbInfraredHudSnapshot {
            p1: transmitting,
            p2: receiving,
        }),
        ["IR ACTIVE".to_string(), "P1 TX P2 SIG".to_string()]
    );
}

#[test]
fn cgb_ir_indicator_renderer_draws_into_the_top_right_corner() {
    let ready = CgbInfraredParticipantHudSnapshot {
        read_enabled: true,
        sensor_warmed: true,
        ..CgbInfraredParticipantHudSnapshot::default()
    };
    let mut frame = vec![255_u8; 160 * 144 * 3];

    render_cgb_ir_indicator(
        &mut frame,
        160,
        144,
        CgbInfraredHudSnapshot {
            p1: ready,
            p2: ready,
        },
    );

    let top_right_region_start = 4 * 160 * 3 + 80 * 3;
    let top_right_region_end = 40 * 160 * 3;
    assert!(
        frame[top_right_region_start..top_right_region_end]
            .iter()
            .any(|component| *component != 255),
        "CGB IR indicator should modify the top-right framebuffer region"
    );
}
