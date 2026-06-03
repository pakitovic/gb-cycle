use super::*;

#[test]
fn recorder_writes_mixed_and_stem_recordings() {
    let output_path = temp_recording_path("wav");
    let stem_ch1_path =
        stem_output_path(&output_path, ApuRecordedChannel::Ch1).expect("ch1 stem path");
    let stem_ch4_path =
        stem_output_path(&output_path, ApuRecordedChannel::Ch4).expect("ch4 stem path");
    let mut recorder = DesktopAudioRecorder::new(
        &DesktopAudioRecordingOptions {
            output_path: output_path.clone(),
            sample_rate_hz: DMG_FAMILY_APU_CAPTURE_CLOCK_HZ,
            stem_channels: vec![ApuRecordedChannel::Ch1, ApuRecordedChannel::Ch4],
        },
        ConsoleModel::GameBoy,
    )
    .expect("recorder");
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    configure_constant_ch1_output(&mut apu);

    for _ in 0..8 {
        recorder.capture_t_cycle(&apu);
    }
    recorder
        .write_captured_samples()
        .expect("captured samples should flush");
    recorder.finish().expect("recorder should finish");
    recorder
        .finish()
        .expect("finishing twice should be harmless");

    let mixed_bytes = fs::read(&output_path).expect("mixed recording");
    let stem_ch1_bytes = fs::read(&stem_ch1_path).expect("ch1 stem");
    let stem_ch4_bytes = fs::read(&stem_ch4_path).expect("ch4 stem");
    assert!(mixed_bytes.len() > WAV_HEADER_LEN as usize);
    assert!(stem_ch1_bytes.len() > WAV_HEADER_LEN as usize);
    assert!(stem_ch4_bytes.len() > WAV_HEADER_LEN as usize);

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(stem_ch1_path);
    let _ = fs::remove_file(stem_ch4_path);
}

#[test]
fn recorder_channel_mask_controls_the_mixed_recording_only() {
    let output_path = temp_recording_path("wav");
    let mut recorder = DesktopAudioRecorder::new(
        &DesktopAudioRecordingOptions {
            output_path: output_path.clone(),
            sample_rate_hz: DMG_FAMILY_APU_CAPTURE_CLOCK_HZ,
            stem_channels: Vec::new(),
        },
        ConsoleModel::GameBoy,
    )
    .expect("recorder");
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    configure_constant_ch1_output(&mut apu);

    recorder
        .set_channel_mask(
            ApuRecordedChannelMask::NONE
                .with_channel(ApuRecordedChannel::Ch1, true)
                .with_channel(ApuRecordedChannel::Ch4, true),
        )
        .expect("set channel mask");
    assert_eq!(
        recorder.channel_mask(),
        ApuRecordedChannelMask::NONE
            .with_channel(ApuRecordedChannel::Ch1, true)
            .with_channel(ApuRecordedChannel::Ch4, true)
    );

    for _ in 0..8 {
        recorder.capture_t_cycle(&apu);
    }
    recorder.finish().expect("recorder should finish");

    let mixed_bytes = fs::read(&output_path).expect("mixed recording");
    assert!(mixed_bytes.len() > WAV_HEADER_LEN as usize);

    let _ = fs::remove_file(output_path);
}

#[test]
fn recorder_updates_capture_state_when_masks_or_console_model_change() {
    let output_path = temp_recording_path("wav");
    let stem_ch3_path =
        stem_output_path(&output_path, ApuRecordedChannel::Ch3).expect("ch3 stem path");
    let mut recorder = DesktopAudioRecorder::new(
        &DesktopAudioRecordingOptions {
            output_path: output_path.clone(),
            sample_rate_hz: DMG_FAMILY_APU_CAPTURE_CLOCK_HZ,
            stem_channels: vec![ApuRecordedChannel::Ch3],
        },
        ConsoleModel::GameBoy,
    )
    .expect("recorder");
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    configure_constant_ch1_output(&mut apu);

    recorder
        .set_channel_mask(ApuRecordedChannelMask::ALL)
        .expect("setting the existing full mask should be a no-op");
    assert!(recorder.mixed_stream.post_hpf_filter.is_none());

    let subset_mask = ApuRecordedChannelMask::NONE.with_channel(ApuRecordedChannel::Ch1, true);
    recorder
        .set_channel_mask(subset_mask)
        .expect("subset mask should reset the mixed stream capture");
    assert_eq!(recorder.channel_mask(), subset_mask);
    assert!(recorder.mixed_stream.post_hpf_filter.is_some());

    for _ in 0..8 {
        recorder.capture_t_cycle(&apu);
    }
    recorder
        .write_captured_samples()
        .expect("captured samples should flush before console-model updates");

    recorder
        .reset_for_session_swap(ConsoleModel::GameBoyPocket)
        .expect("console-model changes should reset all capture state");
    assert_eq!(recorder.console_model, ConsoleModel::GameBoyPocket);
    assert_eq!(recorder.mixed_stream.capture.pending_sample_count(), 0);
    assert_eq!(
        recorder.stem_streams[0]
            .stream
            .capture
            .pending_sample_count(),
        0
    );
    assert!(recorder.mixed_stream.post_hpf_filter.is_some());
    assert!(recorder.stem_streams[0].stream.post_hpf_filter.is_some());

    recorder
        .reset_for_session_swap(ConsoleModel::GameBoyPocket)
        .expect("same-model session swaps should still reset capture state");

    recorder
        .set_channel_mask(ApuRecordedChannelMask::ALL)
        .expect("restoring the full mask should drop the mixed-stream post-HPF filter");
    assert_eq!(recorder.channel_mask(), ApuRecordedChannelMask::ALL);
    assert!(recorder.mixed_stream.post_hpf_filter.is_none());

    recorder.finish().expect("recorder should finish");

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(stem_ch3_path);
}

#[test]
fn session_swaps_reset_recording_capture_even_when_the_console_model_is_unchanged() {
    let output_path = temp_recording_path("wav");
    let stem_ch4_path =
        stem_output_path(&output_path, ApuRecordedChannel::Ch4).expect("ch4 stem path");
    let mut recorder = DesktopAudioRecorder::new(
        &DesktopAudioRecordingOptions {
            output_path: output_path.clone(),
            sample_rate_hz: DMG_FAMILY_APU_CAPTURE_CLOCK_HZ,
            stem_channels: vec![ApuRecordedChannel::Ch4],
        },
        ConsoleModel::GameBoy,
    )
    .expect("recorder");
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    configure_constant_ch1_output(&mut apu);

    for _ in 0..8 {
        recorder.capture_t_cycle(&apu);
    }
    assert!(recorder.mixed_stream.capture.pending_sample_count() > 0);

    recorder
        .reset_for_session_swap(ConsoleModel::GameBoy)
        .expect("session swaps should flush and reset recording capture");
    assert_eq!(recorder.console_model, ConsoleModel::GameBoy);
    assert_eq!(recorder.mixed_stream.capture.pending_sample_count(), 0);
    assert_eq!(
        recorder.stem_streams[0]
            .stream
            .capture
            .pending_sample_count(),
        0
    );

    recorder.finish().expect("recorder should finish");

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(stem_ch4_path);
}
