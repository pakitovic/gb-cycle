use super::*;

#[test]
fn performance_window_title_formats_the_runtime_metrics() {
    assert_eq!(
        performance_window_title(
            "gb-desktop | drmario.gb | dmg | real-boot | strict",
            PerformanceHudSnapshot {
                fps: 14.8,
                speed_percent: 25.0,
                frame_time_ms: 67.5,
                emulation_time_ms: 54.2,
                render_time_ms: 4.1,
                pacing_time_ms: 9.2,
                audio_queue_ms: Some(18.4),
                rewind: RewindHudSnapshot::default(),
            }
        ),
        "gb-desktop | drmario.gb | dmg | real-boot | strict | 14.8 FPS | 67.50 ms | 25% speed | emu 54.20 | render 4.10 | pacing 9.20 | audio 18.4 ms"
    );
}

#[test]
fn audio_queue_pacing_correction_ignores_nominal_latency_and_caps_large_backlogs() {
    assert_eq!(
        super::super::audio_queue_pacing_correction_with_policy(None, true),
        Duration::ZERO
    );
    assert_eq!(
        super::super::audio_queue_pacing_correction_with_policy(
            Some(super::super::AUDIO_QUEUE_TARGET_MS + super::super::AUDIO_QUEUE_DEADBAND_MS,),
            true,
        ),
        Duration::ZERO
    );

    let modest_correction = super::super::audio_queue_pacing_correction_with_policy(
        Some(super::super::AUDIO_QUEUE_TARGET_MS + super::super::AUDIO_QUEUE_DEADBAND_MS + 20.0),
        true,
    );
    assert!(modest_correction > Duration::ZERO);
    assert_eq!(modest_correction, Duration::from_millis(2));

    assert_eq!(
        super::super::audio_queue_pacing_correction_with_policy(Some(2_000.0), true),
        Duration::from_secs_f64(super::super::AUDIO_QUEUE_MAX_CORRECTION_MS / 1_000.0)
    );
    assert_eq!(
        super::super::audio_queue_pacing_correction_with_policy(Some(2_000.0), false),
        Duration::ZERO
    );
}

#[test]
fn host_audio_capture_uses_undoubled_cgb_apu_domain() {
    assert!(super::super::host_audio_capture_due_for_t_cycle(
        CgbSpeedMode::Normal,
        0,
        CpuExecutionState::FetchOpcode { t_cycle: 0 },
    ));
    assert!(super::super::host_audio_capture_due_for_t_cycle(
        CgbSpeedMode::Normal,
        1,
        CpuExecutionState::FetchOpcode { t_cycle: 0 },
    ));
    assert!(super::super::host_audio_capture_due_for_t_cycle(
        CgbSpeedMode::Double,
        0,
        CpuExecutionState::FetchOpcode { t_cycle: 0 },
    ));
    assert!(!super::super::host_audio_capture_due_for_t_cycle(
        CgbSpeedMode::Double,
        1,
        CpuExecutionState::FetchOpcode { t_cycle: 0 },
    ));
    assert!(!super::super::host_audio_capture_due_for_t_cycle(
        CgbSpeedMode::Double,
        0,
        CpuExecutionState::Stopped,
    ));
    assert!(!super::super::host_audio_capture_due_for_t_cycle(
        CgbSpeedMode::Double,
        0,
        CpuExecutionState::SpeedSwitchPause {
            remaining_t_cycles: 1,
        },
    ));
}

#[test]
fn cgb_double_speed_host_audio_captures_one_video_frame_of_samples() {
    fn captured_samples_for_scheduler_t_cycles(
        speed_mode: CgbSpeedMode,
        scheduler_t_cycles: u64,
    ) -> usize {
        let apu = Apu::new(ConsoleModel::GameBoyColor);
        let mut capture = ApuSampleCapture::new(48_000).expect("valid sample rate");
        for scheduler_t_cycle in 0..scheduler_t_cycles {
            if super::super::host_audio_capture_due_for_t_cycle(
                speed_mode,
                scheduler_t_cycle,
                CpuExecutionState::FetchOpcode { t_cycle: 0 },
            ) {
                capture.record_t_cycle(&apu);
            }
        }
        let mut samples = Vec::new();
        capture.drain_samples_into(&mut samples);
        samples.len()
    }

    let normal_speed_samples =
        captured_samples_for_scheduler_t_cycles(CgbSpeedMode::Normal, 70_224);
    let double_speed_samples =
        captured_samples_for_scheduler_t_cycles(CgbSpeedMode::Double, 140_448);
    let ungated_double_speed_samples =
        captured_samples_for_scheduler_t_cycles(CgbSpeedMode::Normal, 140_448);

    assert!(normal_speed_samples > 0);
    assert_eq!(double_speed_samples, normal_speed_samples);
    assert!(ungated_double_speed_samples >= normal_speed_samples * 2 - 1);
}

#[test]
fn audio_queue_pacing_correction_policy_from_env_value_accepts_disable_tokens() {
    assert_eq!(
        super::super::AudioQueuePacingCorrectionPolicy::from_env_value(None),
        super::super::AudioQueuePacingCorrectionPolicy::Enabled
    );
    assert_eq!(
        super::super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new(""))),
        super::super::AudioQueuePacingCorrectionPolicy::Disabled
    );
    assert_eq!(
        super::super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new("1"))),
        super::super::AudioQueuePacingCorrectionPolicy::Disabled
    );
    assert_eq!(
        super::super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new("true"))),
        super::super::AudioQueuePacingCorrectionPolicy::Disabled
    );
    assert_eq!(
        super::super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new(
            "disabled"
        ))),
        super::super::AudioQueuePacingCorrectionPolicy::Disabled
    );
    assert_eq!(
        super::super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new("0"))),
        super::super::AudioQueuePacingCorrectionPolicy::Enabled
    );
    assert_eq!(
        super::super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new("off"))),
        super::super::AudioQueuePacingCorrectionPolicy::Enabled
    );
}

#[test]
fn emulation_profile_mode_from_env_value_accepts_common_toggle_tokens() {
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(None),
        super::super::EmulationProfileMode::Disabled
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("0"))),
        super::super::EmulationProfileMode::Disabled
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("off"))),
        super::super::EmulationProfileMode::Disabled
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("disabled"))),
        super::super::EmulationProfileMode::Disabled
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("1"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: super::super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            detail: super::super::EmulationProfileDetail::Full,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: super::super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            detail: super::super::EmulationProfileDetail::Full,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary:8"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 8,
            detail: super::super::EmulationProfileDetail::Full,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary-lite"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: super::super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            detail: super::super::EmulationProfileDetail::CoreOnly,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary-lite:8"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 8,
            detail: super::super::EmulationProfileDetail::CoreOnly,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary-overhead"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: super::super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            detail: super::super::EmulationProfileDetail::Overhead,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary-overhead:8"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 8,
            detail: super::super::EmulationProfileDetail::Overhead,
        }
    );
}

#[test]
fn emulation_profile_summary_reports_core_frontend_and_other_buckets() {
    let mut counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | no rom".to_string(),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: super::super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            detail: super::super::EmulationProfileDetail::Full,
        },
    );
    counter.frames_in_sample = 2;
    counter.sample_emulation_duration = Duration::from_millis(22);
    counter.sample_present_duration = Duration::from_millis(2);
    counter.sample_pacing_duration = Duration::from_millis(4);
    counter.sample_pacing_sleep_target_duration = Duration::from_millis(4);
    counter.sample_pacing_audio_correction_duration = Duration::from_millis(1);
    counter.sample_pacing_late_duration = Duration::from_millis(2);
    counter.sample_pacing_oversleep_duration = Duration::from_millis(1);
    counter.sample_audio_submit_sample_count = 1_608;
    counter.sample_audio_submit_sample_count_observations = 2;
    counter.sample_audio_submit_t_cycles = 140_448;
    counter.sample_audio_submit_t_cycles_observations = 2;
    counter.sample_audio_submit_queue_before_ms = 48.0;
    counter.sample_audio_submit_queue_before_observations = 2;
    counter.sample_audio_submit_enqueued_ms = 8.0;
    counter.sample_audio_submit_enqueued_observations = 2;
    counter.sample_audio_submit_queue_after_ms = 56.0;
    counter.sample_audio_submit_queue_after_observations = 2;
    counter.sample_audio_queue_before_pacing_ms = 40.0;
    counter.sample_audio_queue_before_pacing_observations = 2;
    counter.sample_audio_queue_after_pacing_ms = 36.0;
    counter.sample_audio_queue_after_pacing_observations = 2;
    counter.sample_speed_mode_normal_frames = 2;
    counter.sample_frame_step_t_cycles = 140_448;
    counter.sample_frame_step_t_cycles_observations = 2;
    counter.sample_frame_video_dots = 140_448;
    counter.sample_frame_video_dots_observations = 2;
    counter.sample_frame_start_ly = 0;
    counter.sample_frame_start_ly_observations = 2;
    counter.sample_frame_start_dot = 0;
    counter.sample_frame_start_dot_observations = 2;
    counter.sample_frame_end_ly = 0;
    counter.sample_frame_end_ly_observations = 2;
    counter.sample_frame_end_dot = 0;
    counter.sample_frame_end_dot_observations = 2;
    counter.sample_frame_origin_crossings = 2;
    counter.sample_frame_origin_crossings_observations = 2;
    counter.sample_scanline_transitions = 308;
    counter.sample_scanline_transitions_observations = 2;
    counter.sample_scanlines_over_456 = 0;
    counter.sample_scanlines_over_456_observations = 2;
    counter.sample_max_scanline_t_cycles = 912;
    counter.sample_max_scanline_t_cycles_observations = 2;
    counter.sample_max_scanline_ly = 306;
    counter.sample_max_scanline_ly_observations = 2;
    counter.sample_max_mode0_start_dot = 504;
    counter.sample_max_mode0_start_dot_observations = 2;
    counter.sample_max_mode0_start_dot_ly = 10;
    counter.sample_max_mode0_start_dot_ly_observations = 2;
    counter.sample_ly_153_to_0_transitions = 2;
    counter.sample_ly_153_to_0_transitions_observations = 2;
    counter.sample_ly_153_to_0_startup_mode0 = 0;
    counter.sample_ly_153_to_0_startup_mode0_observations = 2;
    counter.sample_ly_153_to_0_blank_frame = 0;
    counter.sample_ly_153_to_0_blank_frame_observations = 2;
    counter.sample_ly_0_self_wraps = 0;
    counter.sample_ly_0_self_wraps_observations = 2;
    counter.sample_ly_0_self_wrap_startup_mode0 = 0;
    counter.sample_ly_0_self_wrap_startup_mode0_observations = 2;
    counter.sample_ly_0_self_wrap_blank_frame = 0;
    counter.sample_ly_0_self_wrap_blank_frame_observations = 2;
    counter.sample_ly_0_to_1_transitions = 2;
    counter.sample_ly_0_to_1_transitions_observations = 2;
    counter.sample_ly_0_scanline_t_cycles = 912;
    counter.sample_ly_0_scanline_t_cycles_observations = 2;
    counter.sample_ly_0_max_mode0_start_dot = 508;
    counter.sample_ly_0_max_mode0_start_dot_observations = 2;
    counter.sample_ly_0_stall_t_cycles = 24;
    counter.sample_ly_0_stall_t_cycles_observations = 2;
    counter.sample_ly_0_stall_hblank_t_cycles = 16;
    counter.sample_ly_0_stall_hblank_t_cycles_observations = 2;
    counter.sample_ly_0_stall_oam_t_cycles = 6;
    counter.sample_ly_0_stall_oam_t_cycles_observations = 2;
    counter.sample_ly_0_stall_drawing_t_cycles = 2;
    counter.sample_ly_0_stall_drawing_t_cycles_observations = 2;
    counter.sample_ly_0_stall_startup_mode0_t_cycles = 4;
    counter.sample_ly_0_stall_startup_mode0_t_cycles_observations = 2;
    counter.sample_ly_0_stall_blank_frame_t_cycles = 0;
    counter.sample_ly_0_stall_blank_frame_t_cycles_observations = 2;
    counter.sample_ly_0_stall_runs = 2;
    counter.sample_ly_0_stall_runs_observations = 2;
    counter.sample_ly_0_max_stall_run_t_cycles = 18;
    counter.sample_ly_0_max_stall_run_t_cycles_observations = 2;
    counter.sample_ly_0_max_stall_dot = 224;
    counter.sample_ly_0_max_stall_dot_observations = 2;
    counter.sample_ly_0_max_stall_mode_dot = 42;
    counter.sample_ly_0_max_stall_mode_dot_observations = 2;
    counter.sample_cpu_stop_t_cycles = 10;
    counter.sample_cpu_stop_t_cycles_observations = 2;
    counter.sample_cpu_zombie_stop_t_cycles = 4;
    counter.sample_cpu_zombie_stop_t_cycles_observations = 2;
    counter.sample_ly_0_cpu_stop_t_cycles = 8;
    counter.sample_ly_0_cpu_stop_t_cycles_observations = 2;
    counter.sample_ly_0_cpu_zombie_stop_t_cycles = 2;
    counter.sample_ly_0_cpu_zombie_stop_t_cycles_observations = 2;
    counter.sample_ly_0_stall_cpu_stop_t_cycles = 6;
    counter.sample_ly_0_stall_cpu_stop_t_cycles_observations = 2;
    counter.sample_ly_0_stall_cpu_zombie_stop_t_cycles = 2;
    counter.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations = 2;
    counter.sample_lcd_disabled_t_cycles = 14;
    counter.sample_lcd_disabled_t_cycles_observations = 2;
    counter.sample_lcd_disable_transitions = 2;
    counter.sample_lcd_disable_transitions_observations = 2;
    counter.sample_lcd_enable_transitions = 2;
    counter.sample_lcd_enable_transitions_observations = 2;
    counter.sample_ly_0_lcd_disabled_t_cycles = 12;
    counter.sample_ly_0_lcd_disabled_t_cycles_observations = 2;
    counter.sample_ly_0_stall_lcd_disabled_t_cycles = 10;
    counter.sample_ly_0_stall_lcd_disabled_t_cycles_observations = 2;
    counter.sample_profiled_frames = 2;
    counter.sample_profiled_emulation_duration = Duration::from_millis(24);
    counter.sample_profiled_emulation_breakdown = super::super::EmulationBreakdownSample {
        core_external_events_duration: Duration::from_millis(1),
        core_timer_duration: Duration::from_millis(1),
        core_apu_duration: Duration::from_millis(1),
        core_dma_duration: Duration::from_millis(1),
        core_ppu_duration: Duration::from_millis(10),
        core_ppu_bus_sync_duration: Duration::from_millis(0),
        core_ppu_bus_state_duration: Duration::from_millis(0),
        core_ppu_bus_view_duration: Duration::from_millis(0),
        core_ppu_bus_snapshot_duration: Duration::from_millis(0),
        core_ppu_published_access_duration: Duration::from_millis(0),
        core_ppu_tick_duration: Duration::from_millis(0),
        core_ppu_misc_duration: Duration::from_millis(0),
        core_ppu_mode_timing_duration: Duration::from_millis(0),
        core_ppu_raster_advance_duration: Duration::from_millis(0),
        core_ppu_raster_publication_duration: Duration::from_millis(0),
        core_ppu_stat_irq_duration: Duration::from_millis(0),
        core_ppu_visible_prep_duration: Duration::from_millis(0),
        core_ppu_mode0_1_duration: Duration::from_millis(2),
        core_ppu_mode2_duration: Duration::from_millis(1),
        core_ppu_mode3_control_duration: Duration::from_millis(0),
        core_ppu_mode3_startup_duration: Duration::from_millis(1),
        core_ppu_bg_fetch_duration: Duration::from_millis(2),
        core_ppu_bg_edge_duration: Duration::from_millis(0),
        core_ppu_window_fetch_duration: Duration::from_millis(1),
        core_ppu_window_edge_duration: Duration::from_millis(0),
        core_ppu_push_duration: Duration::from_millis(1),
        core_ppu_obj_edge_duration: Duration::from_millis(0),
        core_ppu_obj_fetch_duration: Duration::from_millis(1),
        core_ppu_pixel_transfer_duration: Duration::from_millis(0),
        core_cpu_duration: Duration::from_millis(4),
        core_serial_duration: Duration::from_millis(1),
        serial_active_t_cycles: 200,
        serial_internal_ticks: 160,
        serial_external_ticks: 40,
        serial_external_wait_ticks: 20,
        serial_shift_edges: 8,
        serial_completed_bytes: 2,
        serial_external_port_ticks: 6,
        core_interrupts_duration: Duration::from_millis(1),
        host_event_poll_duration: Duration::from_millis(2),
        host_audio_submit_duration: Duration::from_millis(1),
        host_save_flush_duration: Duration::from_millis(1),
        profile_base_duration: Duration::from_millis(0),
        profile_core_duration: Duration::from_millis(0),
        profile_full_duration: Duration::from_millis(0),
        profile_core_overhead_duration: Duration::from_millis(0),
        profile_ppu_observer_overhead_duration: Duration::from_millis(0),
    };
    let elapsed = Duration::from_millis(34);
    let snapshot = counter.snapshot_from_elapsed(elapsed);
    let summary = counter
        .emulation_profile_summary(elapsed, snapshot)
        .expect("summary mode should render a profile line");

    assert!(summary.contains("session=single"));
    assert!(summary.contains("emu_ms=11.00"));
    assert!(summary.contains("sampled_frames=2"));
    assert!(summary.contains(&format!(
        "sample_every={}",
        super::super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES
    )));
    assert!(summary.contains("profile_detail=full"));
    assert!(summary.contains("sampled_emu_ms=12.00"));
    assert!(summary.contains("core_est_ms=10.00"));
    assert!(summary.contains("profile_base_ms=0.00"));
    assert!(summary.contains("profile_core_ms=0.00"));
    assert!(summary.contains("profile_full_ms=0.00"));
    assert!(summary.contains("profile_core_overhead_ms=0.00"));
    assert!(summary.contains("profile_ppu_observer_overhead_ms=0.00"));
    assert!(summary.contains("ppu_ms=5.00"));
    assert!(summary.contains("cpu_ms=2.00"));
    assert!(summary.contains("core_other_ms=3.00"));
    assert!(summary.contains("ext_ms=0.50"));
    assert!(summary.contains("timer_ms=0.50"));
    assert!(summary.contains("apu_ms=0.50"));
    assert!(summary.contains("dma_ms=0.50"));
    assert!(summary.contains("serial_ms=0.50"));
    assert!(summary.contains("serial_active_tcycles=100.00"));
    assert!(summary.contains("serial_internal_ticks=80.00"));
    assert!(summary.contains("serial_external_ticks=20.00"));
    assert!(summary.contains("serial_wait_external_ticks=10.00"));
    assert!(summary.contains("serial_shift_edges=4.00"));
    assert!(summary.contains("serial_completed_bytes=1.00"));
    assert!(summary.contains("serial_ext_port_ticks=3.00"));
    assert!(summary.contains("irq_ms=0.50"));
    assert!(summary.contains("ppu_mode0_1_ms=1.00"));
    assert!(summary.contains("ppu_mode2_ms=0.50"));
    assert!(summary.contains("ppu_mode3_startup_ms=0.50"));
    assert!(summary.contains("ppu_bg_ms=1.00"));
    assert!(summary.contains("ppu_win_ms=0.50"));
    assert!(summary.contains("ppu_push_ms=0.50"));
    assert!(summary.contains("ppu_obj_ms=0.50"));
    assert!(summary.contains("ppu_px_ms=0.00"));
    assert!(summary.contains("ppu_bus_ms=0.00"));
    assert!(summary.contains("ppu_busstate_ms=0.00"));
    assert!(summary.contains("ppu_busview_ms=0.00"));
    assert!(summary.contains("ppu_snapshot_ms=0.00"));
    assert!(summary.contains("ppu_pub_ms=0.00"));
    assert!(summary.contains("ppu_tick_ms=0.00"));
    assert!(summary.contains("ppu_mode3_ctrl_ms=0.00"));
    assert!(summary.contains("ppu_bg_edge_ms=0.00"));
    assert!(summary.contains("ppu_win_edge_ms=0.00"));
    assert!(summary.contains("ppu_obj_edge_ms=0.00"));
    assert!(summary.contains("ppu_raster_pub_ms=0.00"));
    assert!(summary.contains("ppu_mode_ms=0.00"));
    assert!(summary.contains("ppu_raster_ms=0.00"));
    assert!(summary.contains("ppu_stat_ms=0.00"));
    assert!(summary.contains("ppu_visible_ms=0.00"));
    assert!(summary.contains("ppu_misc_ms=0.00"));
    assert!(summary.contains("ppu_other_ms=0.50"));
    assert!(summary.contains("ppu_unbucketed_ms=0.50"));
    assert!(summary.contains("ppu_profile_gap_ms=0.50"));
    assert!(summary.contains("host_ms=2.00"));
    assert!(summary.contains("poll_ms=1.00"));
    assert!(summary.contains("audsubmit_ms=0.50"));
    assert!(summary.contains("save_ms=0.50"));
    assert!(summary.contains("frame_tcycles=70224.00"));
    assert!(summary.contains("scheduler_tcycles=70224.00"));
    assert!(summary.contains("video_dots=70224.00"));
    assert!(summary.contains("speed_mode=normal"));
    assert!(summary.contains("frame_start_ly=0.00"));
    assert!(summary.contains("frame_start_dot=0.00"));
    assert!(summary.contains("frame_end_ly=0.00"));
    assert!(summary.contains("frame_end_dot=0.00"));
    assert!(summary.contains("frame_crossings=1.00"));
    assert!(summary.contains("scanline_transitions=154.00"));
    assert!(summary.contains("scanlines_over_456=0.00"));
    assert!(summary.contains("max_scanline_tcycles=456.00"));
    assert!(summary.contains("max_scanline_ly=153.00"));
    assert!(summary.contains("max_mode0_start_dot=252.00"));
    assert!(summary.contains("max_mode0_start_dot_ly=5.00"));
    assert!(summary.contains("ly153_to0=1.00"));
    assert!(summary.contains("ly153_to0_startup=0.00"));
    assert!(summary.contains("ly153_to0_blank=0.00"));
    assert!(summary.contains("ly0_self_wraps=0.00"));
    assert!(summary.contains("ly0_self_wrap_startup=0.00"));
    assert!(summary.contains("ly0_self_wrap_blank=0.00"));
    assert!(summary.contains("ly0_to1=1.00"));
    assert!(summary.contains("ly0_tcycles=456.00"));
    assert!(summary.contains("ly0_max_mode0_start_dot=254.00"));
    assert!(summary.contains("ly0_stall_tcycles=12.00"));
    assert!(summary.contains("ly0_stall_hb_tcycles=8.00"));
    assert!(summary.contains("ly0_stall_oam_tcycles=3.00"));
    assert!(summary.contains("ly0_stall_draw_tcycles=1.00"));
    assert!(summary.contains("ly0_stall_startup_tcycles=2.00"));
    assert!(summary.contains("ly0_stall_blank_tcycles=0.00"));
    assert!(summary.contains("ly0_stall_runs=1.00"));
    assert!(summary.contains("ly0_max_stall_tcycles=9.00"));
    assert!(summary.contains("ly0_max_stall_dot=112.00"));
    assert!(summary.contains("ly0_max_stall_mode_dot=21.00"));
    assert!(summary.contains("cpu_stop_tcycles=5.00"));
    assert!(summary.contains("cpu_zstop_tcycles=2.00"));
    assert!(summary.contains("ly0_stop_tcycles=4.00"));
    assert!(summary.contains("ly0_zstop_tcycles=1.00"));
    assert!(summary.contains("ly0_stall_stop_tcycles=3.00"));
    assert!(summary.contains("ly0_stall_zstop_tcycles=1.00"));
    assert!(summary.contains("lcdoff_tcycles=7.00"));
    assert!(summary.contains("lcdoff_transitions=1.00"));
    assert!(summary.contains("lcdon_transitions=1.00"));
    assert!(summary.contains("ly0_lcdoff_tcycles=6.00"));
    assert!(summary.contains("ly0_stall_lcdoff_tcycles=5.00"));
    assert!(summary.contains("submit_samples=804.00"));
    assert!(summary.contains("submit_tcycles=70224.00"));
    assert!(summary.contains("submit_queue_before_ms=24.00"));
    assert!(summary.contains("submit_enqueued_ms=4.00"));
    assert!(summary.contains("submit_queue_after_ms=28.00"));
    assert!(summary.contains("audio_queue_before_ms=20.00"));
    assert!(summary.contains("audio_queue_after_ms=18.00"));
    assert!(summary.contains("present_ms=1.00"));
    assert!(summary.contains("pac_ms=2.00"));
    assert!(summary.contains("sleep_target_ms=2.00"));
    assert!(summary.contains("audio_corr_ms=0.50"));
    assert!(summary.contains("late_ms=1.00"));
    assert!(summary.contains("oversleep_ms=0.50"));
    let summary_without_audio = counter
        .emulation_profile_summary(
            elapsed,
            super::super::PerformanceHudSnapshot {
                fps: 60.0,
                speed_percent: 100.0,
                frame_time_ms: 16.7,
                emulation_time_ms: 10.0,
                render_time_ms: 1.0,
                pacing_time_ms: 5.0,
                audio_queue_ms: None,
                rewind: RewindHudSnapshot::default(),
            },
        )
        .expect("summary mode should render a profile line without audio");
    assert!(summary_without_audio.contains("audio_queue_before_ms=20.00"));
    assert!(summary_without_audio.contains("audio_queue_after_ms=18.00"));

    let disabled = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | no rom".to_string(),
        super::super::EmulationProfileMode::Disabled,
    );
    assert!(
        disabled
            .emulation_profile_summary(
                elapsed,
                super::super::PerformanceHudSnapshot {
                    fps: 60.0,
                    speed_percent: 100.0,
                    frame_time_ms: 16.7,
                    emulation_time_ms: 10.0,
                    render_time_ms: 1.0,
                    pacing_time_ms: 5.0,
                    audio_queue_ms: Some(18.0),
                    rewind: RewindHudSnapshot::default(),
                },
            )
            .is_none()
    );
}

#[test]
fn emulation_profile_mode_and_breakdown_helpers_cover_all_sampling_buckets() {
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("sampled:7"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 7,
            detail: super::super::EmulationProfileDetail::Full,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("every:9"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 9,
            detail: super::super::EmulationProfileDetail::Full,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("stride:11"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 11,
            detail: super::super::EmulationProfileDetail::Full,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary:0"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: super::super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            detail: super::super::EmulationProfileDetail::Full,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("lite"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: super::super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            detail: super::super::EmulationProfileDetail::CoreOnly,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("lite:3"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 3,
            detail: super::super::EmulationProfileDetail::CoreOnly,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("overhead"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: super::super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            detail: super::super::EmulationProfileDetail::Overhead,
        }
    );
    assert_eq!(
        super::super::EmulationProfileMode::from_env_value(Some(OsStr::new("overhead:5"))),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 5,
            detail: super::super::EmulationProfileDetail::Overhead,
        }
    );

    let disabled = super::super::EmulationProfileMode::Disabled;
    assert!(!disabled.enabled());
    assert_eq!(disabled.sample_every_frames(), None);
    assert_eq!(disabled.detail(), None);

    let sampled = super::super::EmulationProfileMode::SampledSummary {
        sample_every_frames: 7,
        detail: super::super::EmulationProfileDetail::Full,
    };
    assert!(sampled.enabled());
    assert_eq!(sampled.sample_every_frames(), Some(7));
    assert_eq!(
        sampled.detail(),
        Some(super::super::EmulationProfileDetail::Full)
    );

    let mut breakdown = super::super::EmulationBreakdownSample::default();
    for (region, millis) in [
        (MachineStepRegion::ExternalEvents, 1),
        (MachineStepRegion::Timer, 2),
        (MachineStepRegion::Apu, 3),
        (MachineStepRegion::Dma, 4),
        (MachineStepRegion::Ppu, 24),
        (MachineStepRegion::Serial, 6),
        (MachineStepRegion::Cpu, 7),
        (MachineStepRegion::Interrupts, 8),
    ] {
        breakdown.add_core_region_duration(region, Duration::from_millis(millis));
    }
    breakdown.add_host_event_poll_duration(Duration::from_millis(9));
    breakdown.add_host_audio_submit_duration(Duration::from_millis(10));
    breakdown.add_host_save_flush_duration(Duration::from_millis(11));
    breakdown.add_serial_telemetry(SerialTickTelemetry {
        active_t_cycles: 12,
        internal_ticks: 8,
        external_ticks: 4,
        external_wait_ticks: 3,
        shift_edges: 2,
        completed_bytes: 1,
        external_port_ticks: 3,
    });
    for (region, millis) in [
        (PpuStepRegion::Other, 1),
        (PpuStepRegion::BusSync, 1),
        (PpuStepRegion::BusState, 1),
        (PpuStepRegion::BusView, 1),
        (PpuStepRegion::BusSnapshot, 1),
        (PpuStepRegion::PublishedAccess, 1),
        (PpuStepRegion::Tick, 1),
        (PpuStepRegion::ModeTiming, 1),
        (PpuStepRegion::RasterAdvance, 1),
        (PpuStepRegion::RasterPublication, 1),
        (PpuStepRegion::StatIrq, 1),
        (PpuStepRegion::VisiblePrep, 1),
        (PpuStepRegion::Mode0Or1, 1),
        (PpuStepRegion::Mode2Scan, 1),
        (PpuStepRegion::Mode3Control, 1),
        (PpuStepRegion::Mode3Startup, 1),
        (PpuStepRegion::Mode3BgFetch, 1),
        (PpuStepRegion::Mode3BgEdge, 1),
        (PpuStepRegion::Mode3WindowFetch, 1),
        (PpuStepRegion::Mode3WindowEdge, 1),
        (PpuStepRegion::Mode3Push, 1),
        (PpuStepRegion::Mode3ObjEdge, 1),
        (PpuStepRegion::Mode3ObjFetch, 1),
        (PpuStepRegion::Mode3PixelTransfer, 1),
    ] {
        breakdown.add_ppu_region_duration(region, Duration::from_millis(millis));
    }

    assert_eq!(breakdown.core_duration(), Duration::from_millis(55));
    assert_eq!(breakdown.host_duration(), Duration::from_millis(30));
    assert_eq!(breakdown.profile_base_duration, Duration::ZERO);
    assert_eq!(breakdown.profile_core_duration, Duration::ZERO);
    assert_eq!(breakdown.profile_full_duration, Duration::ZERO);
    assert_eq!(breakdown.serial_active_t_cycles, 12);
    assert_eq!(breakdown.serial_internal_ticks, 8);
    assert_eq!(breakdown.serial_external_ticks, 4);
    assert_eq!(breakdown.serial_external_wait_ticks, 3);
    assert_eq!(breakdown.serial_shift_edges, 2);
    assert_eq!(breakdown.serial_completed_bytes, 1);
    assert_eq!(breakdown.serial_external_port_ticks, 3);
    breakdown.add_profile_replay_durations(
        Duration::from_millis(13),
        Duration::from_millis(17),
        Duration::from_millis(23),
    );
    assert_eq!(breakdown.profile_base_duration, Duration::from_millis(13));
    assert_eq!(breakdown.profile_core_duration, Duration::from_millis(17));
    assert_eq!(breakdown.profile_full_duration, Duration::from_millis(23));
    assert_eq!(
        breakdown.profile_core_overhead_duration,
        Duration::from_millis(4)
    );
    assert_eq!(
        breakdown.profile_ppu_observer_overhead_duration,
        Duration::from_millis(6)
    );
    assert_eq!(breakdown.core_other_duration(), Duration::from_millis(24));
    assert_eq!(breakdown.ppu_profiled_duration(), Duration::from_millis(24));
    assert_eq!(breakdown.ppu_other_duration(), Duration::ZERO);

    breakdown.accumulate(super::super::EmulationBreakdownSample {
        core_ppu_duration: Duration::from_millis(2),
        core_cpu_duration: Duration::from_millis(1),
        core_ppu_bg_fetch_duration: Duration::from_millis(1),
        serial_active_t_cycles: 5,
        serial_external_wait_ticks: 2,
        serial_shift_edges: 1,
        host_event_poll_duration: Duration::from_millis(3),
        ..Default::default()
    });
    assert_eq!(breakdown.core_ppu_duration, Duration::from_millis(26));
    assert_eq!(breakdown.core_cpu_duration, Duration::from_millis(8));
    assert_eq!(
        breakdown.core_ppu_bg_fetch_duration,
        Duration::from_millis(2)
    );
    assert_eq!(
        breakdown.host_event_poll_duration,
        Duration::from_millis(12)
    );
    assert_eq!(breakdown.profile_base_duration, Duration::from_millis(13));
    assert_eq!(breakdown.profile_core_duration, Duration::from_millis(17));
    assert_eq!(breakdown.profile_full_duration, Duration::from_millis(23));
    assert_eq!(breakdown.serial_active_t_cycles, 17);
    assert_eq!(breakdown.serial_external_wait_ticks, 5);
    assert_eq!(breakdown.serial_shift_edges, 3);
    assert_eq!(breakdown.core_duration(), Duration::from_millis(58));
    assert_eq!(breakdown.host_duration(), Duration::from_millis(33));
    assert_eq!(breakdown.core_other_duration(), Duration::from_millis(24));
    assert_eq!(breakdown.ppu_other_duration(), Duration::from_millis(1));
}

#[test]
fn emulation_profile_summary_reports_detail_and_overhead_fields() {
    let elapsed = Duration::from_secs(1);
    let snapshot = super::super::PerformanceHudSnapshot {
        fps: 60.0,
        speed_percent: 100.0,
        frame_time_ms: 16.67,
        emulation_time_ms: 10.0,
        render_time_ms: 1.0,
        pacing_time_ms: 5.0,
        audio_queue_ms: None,
        rewind: RewindHudSnapshot::default(),
    };

    let mut lite_counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | profile-lite".to_string(),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 1,
            detail: super::super::EmulationProfileDetail::CoreOnly,
        },
    );
    lite_counter.frames_in_sample = 1;
    lite_counter.sample_profiled_frames = 1;
    lite_counter.sample_profiled_emulation_duration = Duration::from_millis(10);
    lite_counter.sample_profiled_emulation_breakdown = super::super::EmulationBreakdownSample {
        core_ppu_duration: Duration::from_millis(6),
        core_cpu_duration: Duration::from_millis(2),
        ..Default::default()
    };
    let lite_summary = lite_counter
        .emulation_profile_summary(elapsed, snapshot)
        .expect("summary-lite should render a profile line");
    assert!(lite_summary.contains("profile_detail=core"));
    assert!(lite_summary.contains("profile_base_ms=0.00"));
    assert!(lite_summary.contains("profile_core_ms=0.00"));
    assert!(lite_summary.contains("profile_full_ms=0.00"));
    assert!(lite_summary.contains("profile_core_overhead_ms=0.00"));
    assert!(lite_summary.contains("profile_ppu_observer_overhead_ms=0.00"));

    let mut overhead_counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | profile-overhead".to_string(),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 30,
            detail: super::super::EmulationProfileDetail::Overhead,
        },
    );
    overhead_counter.frames_in_sample = 1;
    overhead_counter.sample_profiled_frames = 1;
    overhead_counter.sample_profiled_emulation_duration = Duration::from_millis(15);
    overhead_counter.sample_profiled_emulation_breakdown = super::super::EmulationBreakdownSample {
        core_ppu_duration: Duration::from_millis(7),
        core_cpu_duration: Duration::from_millis(2),
        profile_base_duration: Duration::from_millis(8),
        profile_core_duration: Duration::from_millis(11),
        profile_full_duration: Duration::from_millis(17),
        profile_core_overhead_duration: Duration::from_millis(3),
        profile_ppu_observer_overhead_duration: Duration::from_millis(6),
        ..Default::default()
    };
    let overhead_summary = overhead_counter
        .emulation_profile_summary(elapsed, snapshot)
        .expect("summary-overhead should render a profile line");
    assert!(overhead_summary.contains("profile_detail=overhead"));
    assert!(overhead_summary.contains("sample_every=30"));
    assert!(overhead_summary.contains("profile_base_ms=8.00"));
    assert!(overhead_summary.contains("profile_core_ms=11.00"));
    assert!(overhead_summary.contains("profile_full_ms=17.00"));
    assert!(overhead_summary.contains("profile_core_overhead_ms=3.00"));
    assert!(overhead_summary.contains("profile_ppu_observer_overhead_ms=6.00"));
}

#[test]
fn emulation_profile_request_and_replay_preserve_host_and_core_timing() {
    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut request = super::super::EmulationProfileRequest::new(
        super::super::DesktopEmulationSession::new_single(machine),
    );
    request.record_host_event_poll_duration(Duration::from_millis(2));
    request.record_host_audio_submit_duration(Duration::from_millis(3));
    request.record_host_save_flush_duration(Duration::from_millis(4));

    let work_item = request.into_work_item(Duration::from_millis(9));
    assert_eq!(work_item.emulation_duration, Duration::from_millis(9));
    assert_eq!(
        work_item.breakdown.host_duration(),
        Duration::from_millis(9)
    );

    let completed = super::super::profile_emulation_work_item(work_item);
    assert_eq!(completed.emulation_duration, Duration::from_millis(9));
    assert!(completed.breakdown.core_duration() > Duration::ZERO);
    assert_eq!(
        completed.breakdown.host_duration(),
        Duration::from_millis(9)
    );
}

#[test]
fn emulation_profile_core_only_replay_keeps_core_regions_without_ppu_subregions() {
    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    let request = super::super::EmulationProfileRequest::new_with_detail(
        super::super::DesktopEmulationSession::new_single(machine),
        super::super::EmulationProfileDetail::CoreOnly,
    );

    let completed =
        super::super::profile_emulation_work_item(request.into_work_item(Duration::from_millis(9)));

    assert!(completed.breakdown.core_duration() > Duration::ZERO);
    assert!(completed.breakdown.core_ppu_duration > Duration::ZERO);
    assert_eq!(completed.breakdown.ppu_profiled_duration(), Duration::ZERO);
    assert_eq!(
        completed.breakdown.ppu_other_duration(),
        completed.breakdown.core_ppu_duration
    );
}

#[test]
fn emulation_profile_overhead_replay_reports_three_equivalent_paths() {
    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    let starting_session = super::super::DesktopEmulationSession::new_single(machine);
    let mut unobserved = starting_session.clone();
    let mut core_only = starting_session.clone();
    let mut full = starting_session.clone();
    let mut core_profiler = super::super::ReplayFrameCoreProfiler::new(false);
    let mut full_profiler = super::super::ReplayFrameCoreProfiler::new(true);

    super::super::step_profile_replay_frame_unobserved(&mut unobserved);
    super::super::step_profile_replay_frame_with_observer(&mut core_only, &mut core_profiler);
    super::super::step_profile_replay_frame_with_observer(&mut full, &mut full_profiler);

    assert_eq!(
        unobserved.primary_machine().snapshot(),
        core_only.primary_machine().snapshot()
    );
    assert_eq!(
        unobserved.primary_machine().snapshot(),
        full.primary_machine().snapshot()
    );
    let core_breakdown = core_profiler.finish();
    let full_breakdown = full_profiler.finish();
    assert!(core_breakdown.core_duration() > Duration::ZERO);
    assert_eq!(core_breakdown.ppu_profiled_duration(), Duration::ZERO);
    assert!(full_breakdown.core_duration() > Duration::ZERO);
    assert!(full_breakdown.ppu_profiled_duration() > Duration::ZERO);

    let request = super::super::EmulationProfileRequest::new_with_detail(
        starting_session,
        super::super::EmulationProfileDetail::Overhead,
    );
    let completed = super::super::profile_emulation_work_item(
        request.into_work_item(Duration::from_millis(11)),
    );

    assert_eq!(completed.emulation_duration, Duration::from_millis(11));
    assert!(completed.breakdown.core_duration() > Duration::ZERO);
    assert!(completed.breakdown.core_ppu_duration > Duration::ZERO);
    assert!(completed.breakdown.ppu_profiled_duration() > Duration::ZERO);
    assert!(completed.breakdown.profile_base_duration > Duration::ZERO);
    assert!(completed.breakdown.profile_core_duration > Duration::ZERO);
    assert!(completed.breakdown.profile_full_duration > Duration::ZERO);
}

#[test]
fn linked_emulation_profile_request_replays_core_regions() {
    let primary = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let secondary = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let linked =
        super::super::DesktopEmulationSession::new_linked_dmg04_two_player(primary, secondary)
            .expect("matching machines should create a linked desktop session");
    let request = super::super::EmulationProfileRequest::new(linked);

    let completed = super::super::profile_emulation_work_item(
        request.into_work_item(Duration::from_millis(11)),
    );

    assert_eq!(completed.emulation_duration, Duration::from_millis(11));
    assert!(completed.breakdown.core_duration() > Duration::ZERO);
    assert!(completed.breakdown.core_ppu_duration > Duration::ZERO);
}

#[test]
fn async_emulation_profile_worker_and_counter_collect_samples() {
    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let worker = super::super::AsyncEmulationProfileWorker::new();
    let mut completed = Vec::new();
    worker.collect_completed(&mut |sample| completed.push(sample));
    assert!(completed.is_empty());
    assert!(
        worker.try_submit(
            super::super::EmulationProfileRequest::new(
                super::super::DesktopEmulationSession::new_single(machine.clone(),)
            )
            .into_work_item(Duration::from_millis(7))
        )
    );
    for _ in 0..200 {
        worker.collect_completed(&mut |sample| completed.push(sample));
        if !completed.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].emulation_duration, Duration::from_millis(7));
    assert!(completed[0].breakdown.core_duration() > Duration::ZERO);

    let mut disabled = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | disabled".to_string(),
        super::super::EmulationProfileMode::Disabled,
    );
    assert!(!disabled.emulation_profile_enabled());
    assert!(!disabled.should_profile_next_frame());
    disabled.collect_emulation_profile_results();
    disabled.submit_emulation_profile_request(
        Some(super::super::EmulationProfileRequest::new(
            super::super::DesktopEmulationSession::new_single(machine.clone()),
        )),
        Duration::from_millis(5),
    );
    assert!(!disabled.emulation_profile_request_in_flight);

    let mut counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | sampled".to_string(),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 2,
            detail: super::super::EmulationProfileDetail::Full,
        },
    );
    assert!(counter.emulation_profile_enabled());
    assert!(!counter.should_profile_next_frame());
    counter.presented_frames_total = 1;
    assert!(counter.should_profile_next_frame());
    counter.emulation_profile_request_in_flight = true;
    assert!(!counter.should_profile_next_frame());
    counter.emulation_profile_request_in_flight = false;
    counter.submit_emulation_profile_request(
        Some(super::super::EmulationProfileRequest::new(
            super::super::DesktopEmulationSession::new_single(machine),
        )),
        Duration::from_millis(6),
    );
    assert!(counter.emulation_profile_request_in_flight);
    wait_for_profiled_counter_sample(&mut counter);
    assert!(!counter.emulation_profile_request_in_flight);
    assert_eq!(counter.sample_profiled_frames, 1);
    assert_eq!(
        counter.sample_profiled_emulation_duration,
        Duration::from_millis(6)
    );
    assert!(counter.sample_profiled_emulation_breakdown.core_duration() > Duration::ZERO);
}

#[test]
fn performance_counter_record_presented_frame_reports_and_resets_sampled_state() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("profile-summary", true, false, false);
    let mut counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | profile-summary".to_string(),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 4,
            detail: super::super::EmulationProfileDetail::Full,
        },
    );
    counter.sample_started_at = Instant::now() - Duration::from_secs(2);
    counter.sample_profiled_frames = 1;
    counter.sample_profiled_emulation_duration = Duration::from_millis(12);
    counter.sample_profiled_emulation_breakdown = super::super::EmulationBreakdownSample {
        core_cpu_duration: Duration::from_millis(2),
        core_ppu_duration: Duration::from_millis(6),
        host_event_poll_duration: Duration::from_millis(1),
        host_audio_submit_duration: Duration::from_millis(1),
        ..Default::default()
    };
    counter
        .record_presented_frame(
            harness.canvas.window_mut(),
            super::super::FramePerformanceSample {
                session_kind: super::super::EmulationProfileSessionKind::Single,
                emulation_duration: Duration::from_millis(12),
                emulation_profile_request: None,
                render_duration: Duration::from_millis(2),
                present_duration: Duration::from_millis(1),
                pacing_duration: Duration::from_millis(4),
                pacing_sleep_target_duration: Duration::from_millis(4),
                pacing_audio_correction_duration: Duration::from_millis(1),
                pacing_late_duration: Duration::from_millis(2),
                pacing_oversleep_duration: Duration::from_millis(1),
                audio_submit_sample_count: Some(804),
                audio_submit_t_cycles: Some(70_224),
                audio_submit_queue_before_ms: Some(24.0),
                audio_submit_enqueued_ms: Some(4.0),
                audio_submit_queue_after_ms: Some(28.0),
                audio_queue_before_pacing_ms: Some(20.0),
                audio_queue_after_pacing_ms: Some(18.0),
                speed_mode: Some(CgbSpeedMode::Normal),
                frame_step_t_cycles: Some(70_224),
                frame_video_dots: Some(70_224),
                frame_start_ly: Some(0),
                frame_start_dot: Some(0),
                frame_end_ly: Some(0),
                frame_end_dot: Some(0),
                frame_origin_crossings: Some(1),
                scanline_transitions: Some(154),
                scanlines_over_456: Some(0),
                max_scanline_t_cycles: Some(456),
                max_scanline_ly: Some(153),
                max_mode0_start_dot: Some(252),
                max_mode0_start_dot_ly: Some(5),
                ly_153_to_0_transitions: Some(1),
                ly_153_to_0_startup_mode0: Some(0),
                ly_153_to_0_blank_frame: Some(0),
                ly_0_self_wraps: Some(0),
                ly_0_self_wrap_startup_mode0: Some(0),
                ly_0_self_wrap_blank_frame: Some(0),
                ly_0_to_1_transitions: Some(1),
                ly_0_scanline_t_cycles: Some(456),
                ly_0_max_mode0_start_dot: Some(254),
                ly_0_stall_t_cycles: Some(0),
                ly_0_stall_hblank_t_cycles: Some(0),
                ly_0_stall_oam_t_cycles: Some(0),
                ly_0_stall_drawing_t_cycles: Some(0),
                ly_0_stall_startup_mode0_t_cycles: Some(0),
                ly_0_stall_blank_frame_t_cycles: Some(0),
                ly_0_stall_runs: Some(0),
                ly_0_max_stall_run_t_cycles: Some(0),
                ly_0_max_stall_dot: Some(0),
                ly_0_max_stall_mode_dot: Some(0),
                cpu_stop_t_cycles: Some(0),
                cpu_zombie_stop_t_cycles: Some(0),
                ly_0_cpu_stop_t_cycles: Some(0),
                ly_0_cpu_zombie_stop_t_cycles: Some(0),
                ly_0_stall_cpu_stop_t_cycles: Some(0),
                ly_0_stall_cpu_zombie_stop_t_cycles: Some(0),
                lcd_disabled_t_cycles: Some(0),
                lcd_disable_transitions: Some(0),
                lcd_enable_transitions: Some(0),
                ly_0_lcd_disabled_t_cycles: Some(0),
                ly_0_stall_lcd_disabled_t_cycles: Some(0),
            },
        )
        .expect("recording a sampled frame should succeed");
    assert_eq!(counter.frames_in_sample, 0);
    assert_eq!(counter.sample_profiled_frames, 0);
    assert_eq!(counter.sample_emulation_duration, Duration::ZERO);
    assert_eq!(counter.sample_present_duration, Duration::ZERO);
    assert_eq!(counter.sample_pacing_sleep_target_duration, Duration::ZERO);
    assert_eq!(
        counter.sample_pacing_audio_correction_duration,
        Duration::ZERO
    );
    assert_eq!(counter.sample_pacing_late_duration, Duration::ZERO);
    assert_eq!(counter.sample_pacing_oversleep_duration, Duration::ZERO);
    assert_eq!(counter.sample_audio_submit_sample_count, 0);
    assert_eq!(counter.sample_audio_submit_sample_count_observations, 0);
    assert_eq!(counter.sample_audio_submit_t_cycles, 0);
    assert_eq!(counter.sample_audio_submit_t_cycles_observations, 0);
    assert_eq!(counter.sample_audio_submit_queue_before_ms, 0.0);
    assert_eq!(counter.sample_audio_submit_queue_before_observations, 0);
    assert_eq!(counter.sample_audio_submit_enqueued_ms, 0.0);
    assert_eq!(counter.sample_audio_submit_enqueued_observations, 0);
    assert_eq!(counter.sample_audio_submit_queue_after_ms, 0.0);
    assert_eq!(counter.sample_audio_submit_queue_after_observations, 0);
    assert_eq!(counter.sample_audio_queue_before_pacing_ms, 0.0);
    assert_eq!(counter.sample_audio_queue_before_pacing_observations, 0);
    assert_eq!(counter.sample_audio_queue_after_pacing_ms, 0.0);
    assert_eq!(counter.sample_audio_queue_after_pacing_observations, 0);
    assert_eq!(counter.sample_speed_mode_normal_frames, 0);
    assert_eq!(counter.sample_speed_mode_double_frames, 0);
    assert_eq!(counter.sample_frame_step_t_cycles, 0);
    assert_eq!(counter.sample_frame_step_t_cycles_observations, 0);
    assert_eq!(counter.sample_frame_video_dots, 0);
    assert_eq!(counter.sample_frame_video_dots_observations, 0);
    assert_eq!(counter.sample_frame_start_ly, 0);
    assert_eq!(counter.sample_frame_start_ly_observations, 0);
    assert_eq!(counter.sample_frame_start_dot, 0);
    assert_eq!(counter.sample_frame_start_dot_observations, 0);
    assert_eq!(counter.sample_frame_end_ly, 0);
    assert_eq!(counter.sample_frame_end_ly_observations, 0);
    assert_eq!(counter.sample_frame_end_dot, 0);
    assert_eq!(counter.sample_frame_end_dot_observations, 0);
    assert_eq!(counter.sample_frame_origin_crossings, 0);
    assert_eq!(counter.sample_frame_origin_crossings_observations, 0);
    assert_eq!(counter.sample_scanline_transitions, 0);
    assert_eq!(counter.sample_scanline_transitions_observations, 0);
    assert_eq!(counter.sample_scanlines_over_456, 0);
    assert_eq!(counter.sample_scanlines_over_456_observations, 0);
    assert_eq!(counter.sample_max_scanline_t_cycles, 0);
    assert_eq!(counter.sample_max_scanline_t_cycles_observations, 0);
    assert_eq!(counter.sample_max_scanline_ly, 0);
    assert_eq!(counter.sample_max_scanline_ly_observations, 0);
    assert_eq!(counter.sample_max_mode0_start_dot, 0);
    assert_eq!(counter.sample_max_mode0_start_dot_observations, 0);
    assert_eq!(counter.sample_max_mode0_start_dot_ly, 0);
    assert_eq!(counter.sample_max_mode0_start_dot_ly_observations, 0);
    assert!(counter.hud_snapshot().is_some());

    counter.frames_in_sample = 1;
    assert!(
        counter
            .emulation_profile_summary(
                Duration::from_millis(20),
                super::super::PerformanceHudSnapshot {
                    fps: 60.0,
                    speed_percent: 100.0,
                    frame_time_ms: 16.7,
                    emulation_time_ms: 9.0,
                    render_time_ms: 1.0,
                    pacing_time_ms: 2.0,
                    audio_queue_ms: None,
                    rewind: RewindHudSnapshot::default(),
                },
            )
            .is_none()
    );
}
