use super::*;

#[test]
fn audio_helpers_cover_normalization_duration_and_capture_errors() {
    assert_eq!(
        AudioTelemetryMode::from_env_value(None),
        AudioTelemetryMode::Disabled
    );
    assert_eq!(
        AudioTelemetryMode::from_env_value(Some(OsStr::new("0"))),
        AudioTelemetryMode::Disabled
    );
    assert_eq!(
        AudioTelemetryMode::from_env_value(Some(OsStr::new("false"))),
        AudioTelemetryMode::Disabled
    );
    assert_eq!(
        AudioTelemetryMode::from_env_value(Some(OsStr::new("off"))),
        AudioTelemetryMode::Disabled
    );
    assert_eq!(
        AudioTelemetryMode::from_env_value(Some(OsStr::new("1"))),
        AudioTelemetryMode::Events
    );
    assert_eq!(
        AudioTelemetryMode::from_env_value(Some(OsStr::new("debug"))),
        AudioTelemetryMode::Verbose
    );
    assert_eq!(
        AudioTelemetryMode::from_env_value(Some(OsStr::new("verbose"))),
        AudioTelemetryMode::Verbose
    );
    assert_eq!(
        AudioTelemetryMode::from_env_value(Some(OsStr::new("all"))),
        AudioTelemetryMode::Verbose
    );
    assert_eq!(
        AutoQueueClearPolicy::from_env_value(None),
        AutoQueueClearPolicy::Enabled
    );
    assert_eq!(
        AutoQueueClearPolicy::from_env_value(Some(OsStr::new("1"))),
        AutoQueueClearPolicy::Disabled
    );
    assert_eq!(
        AutoQueueClearPolicy::from_env_value(Some(OsStr::new("true"))),
        AutoQueueClearPolicy::Disabled
    );
    assert_eq!(
        AutoQueueClearPolicy::from_env_value(Some(OsStr::new("disabled"))),
        AutoQueueClearPolicy::Disabled
    );
    assert_eq!(
        AutoQueueClearPolicy::from_env_value(Some(OsStr::new("0"))),
        AutoQueueClearPolicy::Enabled
    );
    assert_eq!(normalize_sample(APU_HOST_MAX_ABS_SAMPLE / 4), 0.25);
    assert_eq!(normalize_sample(APU_HOST_MAX_ABS_SAMPLE * 2), 1.0);
    assert_eq!(normalize_sample(-APU_HOST_MAX_ABS_SAMPLE * 2), -1.0);
    assert_eq!(format_optional_i32(Some(12)), "12");
    assert_eq!(format_optional_i32(None), "unknown");
    assert_eq!(format_optional_ms(Some(1.25)), "1.250");
    assert_eq!(format_optional_ms(None), "unknown");
    assert_eq!(
        format_audio_error("failed to pause SDL3 audio stream", "paused"),
        "failed to pause SDL3 audio stream: paused"
    );
    assert_eq!(
        map_audio_result::<(), _>(Err("stream"), "stream op")
            .expect_err("error mapping should preserve context"),
        "stream op: stream"
    );
    assert_eq!(
        format_capture_error(ApuSampleCaptureError::OutputSampleRateZero),
        "audio output sample rate must be greater than zero"
    );

    let _guard = crate::lock_sdl_test();
    let audio = init_audio_subsystem();
    let mut output = DesktopAudioOutput::new(&audio, &test_audio_options(), ConsoleModel::GameBoy)
        .expect("audio output");
    output.output_sample_rate_hz = 0;
    assert_eq!(output.queued_duration_ms(), None);
    assert_eq!(output.sample_frames_duration_ms(1), None);
    assert_eq!(
        output
            .clear_buffer()
            .expect_err("clear_buffer should reject zero Hz"),
        "audio output sample rate must be greater than zero"
    );

    assert_eq!(AUDIO_CHANNEL_COUNT, 2);
    assert_eq!(BYTES_PER_F32_SAMPLE, std::mem::size_of::<f32>() as i32);
}

#[test]
fn audio_telemetry_logging_advances_sequence_numbers_when_enabled() {
    let telemetry = AudioTelemetry {
        mode: AudioTelemetryMode::Events,
        next_sequence: Cell::new(0),
        queue_clear_count: Cell::new(0),
    };

    telemetry.log_event("test", "first=true");
    telemetry.log_event("test", "second=true");

    assert_eq!(telemetry.next_sequence.get(), 2);
    assert_eq!(telemetry.queue_clear_count.get(), 0);

    let verbose_telemetry = AudioTelemetry {
        mode: AudioTelemetryMode::Verbose,
        next_sequence: Cell::new(0),
        queue_clear_count: Cell::new(0),
    };
    verbose_telemetry.log_submit_batch("submit", "batch=true");
    assert_eq!(verbose_telemetry.next_sequence.get(), 1);
}
