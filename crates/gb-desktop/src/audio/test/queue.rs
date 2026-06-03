use super::*;

#[test]
fn desktop_audio_output_scales_queues_and_clears_captured_samples() {
    let _guard = crate::lock_sdl_test();
    let audio = init_audio_subsystem();
    let mut output = DesktopAudioOutput::new(&audio, &test_audio_options(), ConsoleModel::GameBoy)
        .expect("audio output");
    output.pause().expect("pause");

    push_captured_sample(
        &mut output,
        ApuHostSample {
            left: APU_HOST_MAX_ABS_SAMPLE / 2,
            right: -APU_HOST_MAX_ABS_SAMPLE,
        },
    );
    push_captured_sample(
        &mut output,
        ApuHostSample {
            left: APU_HOST_MAX_ABS_SAMPLE,
            right: 0,
        },
    );
    output
        .submit_captured_samples()
        .expect("submit_captured_samples");

    assert_eq!(output.captured_samples.len(), 2);
    assert_eq!(output.interleaved_buffer, vec![0.5, -1.0, 1.0, 0.0]);
    let submit_telemetry = output
        .take_last_submit_telemetry()
        .expect("submit should record queue telemetry");
    assert_eq!(submit_telemetry.sample_count, 2);
    assert_eq!(submit_telemetry.captured_t_cycles, 0);
    assert_eq!(submit_telemetry.queued_ms_before, Some(0.0));
    assert!(
        submit_telemetry
            .enqueued_ms
            .expect("submit should report enqueued duration")
            > 0.0
    );
    assert!(
        submit_telemetry
            .queued_ms_after
            .expect("submit should report queued duration after enqueue")
            > 0.0
    );
    assert_eq!(output.take_last_submit_telemetry(), None);
    assert!(
        output
            .queued_duration_ms()
            .expect("queued duration should exist")
            >= 0.0
    );

    output.max_queued_bytes = -1;
    for streak in 1..=OVERSIZED_QUEUE_CLEAR_STREAK {
        push_captured_sample(
            &mut output,
            ApuHostSample {
                left: APU_HOST_MAX_ABS_SAMPLE,
                right: APU_HOST_MAX_ABS_SAMPLE,
            },
        );
        output
            .submit_captured_samples()
            .expect("submit_captured_samples should tolerate temporary oversized queues");
        let expected_streak = if streak == OVERSIZED_QUEUE_CLEAR_STREAK {
            0
        } else {
            streak
        };
        assert_eq!(output.oversized_queue_streak, expected_streak);
    }
    assert_eq!(output.captured_samples.len(), 1);
    assert_eq!(output.interleaved_buffer, vec![1.0, 1.0]);

    output.set_muted(true).expect("set_muted");
    push_captured_sample(
        &mut output,
        ApuHostSample {
            left: APU_HOST_MAX_ABS_SAMPLE,
            right: -APU_HOST_MAX_ABS_SAMPLE,
        },
    );
    output
        .submit_captured_samples()
        .expect("muted submit_captured_samples");
    assert_eq!(output.interleaved_buffer, vec![0.0, -0.0]);
}

#[test]
fn desktop_audio_output_can_disable_automatic_oversized_queue_clears() {
    let _guard = crate::lock_sdl_test();
    let audio = init_audio_subsystem();
    let mut output = DesktopAudioOutput::new(&audio, &test_audio_options(), ConsoleModel::GameBoy)
        .expect("audio output");
    output.pause().expect("pause");
    output.auto_queue_clear_enabled = false;
    output.max_queued_bytes = -1;

    for _ in 0..=OVERSIZED_QUEUE_CLEAR_STREAK {
        push_captured_sample(
            &mut output,
            ApuHostSample {
                left: APU_HOST_MAX_ABS_SAMPLE,
                right: APU_HOST_MAX_ABS_SAMPLE,
            },
        );
        output
            .submit_captured_samples()
            .expect("submit_captured_samples should keep the backlog without auto clear");
    }

    assert_eq!(output.telemetry.queue_clear_count.get(), 0);
    assert_eq!(output.oversized_queue_streak, OVERSIZED_QUEUE_CLEAR_STREAK);
}

#[test]
fn desktop_audio_output_clears_high_latency_queue_after_streak() {
    let _guard = crate::lock_sdl_test();
    let audio = init_audio_subsystem();
    let mut output = DesktopAudioOutput::new(&audio, &test_audio_options(), ConsoleModel::GameBoy)
        .expect("audio output");
    output.pause().expect("pause");
    output.max_queued_bytes = i32::MAX;

    for streak in 1..=super::super::LATENCY_QUEUE_CLEAR_STREAK {
        queue_silence_ms(&output, super::super::LATENCY_QUEUE_CLEAR_MS + 50.0);
        push_captured_sample(
            &mut output,
            ApuHostSample {
                left: APU_HOST_MAX_ABS_SAMPLE,
                right: APU_HOST_MAX_ABS_SAMPLE,
            },
        );
        output
            .submit_captured_samples()
            .expect("submit_captured_samples should recover high-latency queues");
        let expected_streak = if streak == super::super::LATENCY_QUEUE_CLEAR_STREAK {
            0
        } else {
            streak
        };
        assert_eq!(output.latency_queue_streak, expected_streak);
        assert_eq!(output.oversized_queue_streak, 0);
    }

    assert_eq!(output.telemetry.queue_clear_count.get(), 1);
}

#[test]
fn desktop_audio_output_controls_pause_volume_and_buffer_reset() {
    let _guard = crate::lock_sdl_test();
    let audio = init_audio_subsystem();
    let mut output = DesktopAudioOutput::new(&audio, &test_audio_options(), ConsoleModel::GameBoy)
        .expect("audio output");

    assert!(!output.is_muted());
    assert!(
        !output.stream.device_paused().expect("device_paused"),
        "new streams resume playback during initialization"
    );

    output.pause().expect("pause");
    assert!(output.stream.device_paused().expect("device_paused"));
    output.resume().expect("resume");
    assert!(!output.stream.device_paused().expect("device_paused"));
    output.pause().expect("pause");

    output.capture_t_cycle(&Apu::new(ConsoleModel::GameBoy));
    output
        .submit_captured_samples()
        .expect("empty submit_captured_samples");
    assert!(output.captured_samples.is_empty());

    let silent_apu = Apu::new(ConsoleModel::GameBoy);
    while output.capture.pending_sample_count() == 0 {
        output.capture_t_cycle(&silent_apu);
    }
    output
        .submit_captured_samples()
        .expect("submit_captured_samples after capture_t_cycle");
    let submit_telemetry = output
        .take_last_submit_telemetry()
        .expect("capture_t_cycle submits should record telemetry");
    assert!(submit_telemetry.sample_count > 0);
    assert!(submit_telemetry.captured_t_cycles > 0);

    push_captured_sample(
        &mut output,
        ApuHostSample {
            left: APU_HOST_MAX_ABS_SAMPLE,
            right: 0,
        },
    );
    output
        .submit_captured_samples()
        .expect("submit_captured_samples");
    assert!(!output.interleaved_buffer.is_empty());

    output.set_muted(true).expect("set_muted");
    assert!(output.is_muted());
    output
        .set_muted(true)
        .expect("setting the same mute state is a no-op");

    output.set_volume_percent(250).expect("set_volume_percent");
    assert_eq!(output.volume_percent, 100);
    assert_eq!(output.volume_scale, 1.0);
    output
        .set_volume_percent(100)
        .expect("setting the same volume is a no-op");

    push_captured_sample(&mut output, ApuHostSample { left: 7, right: -7 });
    output
        .submit_captured_samples()
        .expect("submit_captured_samples");
    assert!(!output.captured_samples.is_empty());
    assert!(!output.interleaved_buffer.is_empty());

    output.clear_buffer().expect("clear_buffer");
    assert!(output.captured_samples.is_empty());
    assert!(output.interleaved_buffer.is_empty());

    output.flush().expect("flush");
}
