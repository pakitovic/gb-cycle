use super::*;

#[test]
fn desktop_audio_output_channel_masks_reset_host_capture_and_follow_console_model() {
    let _guard = crate::lock_sdl_test();
    let audio = init_audio_subsystem();
    let mut output = DesktopAudioOutput::new(&audio, &test_audio_options(), ConsoleModel::GameBoy)
        .expect("audio output");
    output.pause().expect("pause");

    let subset_mask = ApuRecordedChannelMask::NONE.with_channel(ApuRecordedChannel::Ch1, true);
    output
        .set_channel_mask(subset_mask)
        .expect("subset channel mask should update");
    assert_eq!(output.channel_mask, subset_mask);
    assert!(output.masked_mix_hpf.is_some());

    output
        .set_channel_mask(subset_mask)
        .expect("setting the same mask should be a no-op");
    assert_eq!(output.channel_mask, subset_mask);
    assert!(output.masked_mix_hpf.is_some());

    let mut apu = Apu::new(ConsoleModel::GameBoy);
    configure_constant_ch1_output(&mut apu);
    while output.capture.pending_sample_count() == 0 {
        output.capture_t_cycle(&apu);
    }
    output
        .submit_captured_samples()
        .expect("subset mix should submit captured samples");
    assert!(!output.interleaved_buffer.is_empty());

    output
        .reset_for_session_swap(ConsoleModel::GameBoyPocket)
        .expect("console model changes should reset the masked capture");
    assert_eq!(output.console_model, ConsoleModel::GameBoyPocket);
    assert!(output.masked_mix_hpf.is_some());
    assert!(output.captured_samples.is_empty());
    assert!(output.interleaved_buffer.is_empty());

    output
        .reset_for_session_swap(ConsoleModel::GameBoyPocket)
        .expect("same-model session swaps should still clear buffered audio");
    assert_eq!(output.console_model, ConsoleModel::GameBoyPocket);

    output
        .set_channel_mask(ApuRecordedChannelMask::ALL)
        .expect("restoring the full mask should drop the masked HPF");
    assert_eq!(output.channel_mask, ApuRecordedChannelMask::ALL);
    assert!(output.masked_mix_hpf.is_none());
}

#[test]
fn session_swaps_clear_audio_output_even_when_the_console_model_stays_the_same() {
    let _guard = crate::lock_sdl_test();
    let audio = init_audio_subsystem();
    let mut output = DesktopAudioOutput::new(&audio, &test_audio_options(), ConsoleModel::GameBoy)
        .expect("audio output");
    output.pause().expect("pause");

    let subset_mask = ApuRecordedChannelMask::NONE.with_channel(ApuRecordedChannel::Ch1, true);
    output
        .set_channel_mask(subset_mask)
        .expect("subset channel mask should update");

    let mut apu = Apu::new(ConsoleModel::GameBoy);
    configure_constant_ch1_output(&mut apu);
    while output.capture.pending_sample_count() == 0 {
        output.capture_t_cycle(&apu);
    }
    output
        .submit_captured_samples()
        .expect("captured samples should submit before the session swap");
    assert!(!output.interleaved_buffer.is_empty());

    output
        .reset_for_session_swap(ConsoleModel::GameBoy)
        .expect("session swaps should clear buffered audio even when the model is unchanged");
    assert_eq!(output.console_model, ConsoleModel::GameBoy);
    assert!(output.captured_samples.is_empty());
    assert!(output.interleaved_buffer.is_empty());
    assert!(output.masked_mix_hpf.is_some());
}
