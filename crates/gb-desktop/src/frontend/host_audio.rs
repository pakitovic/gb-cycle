fn build_automatic_audio_recording_options(
    session: &DesktopSession,
) -> Result<DesktopAudioRecordingOptions, String> {
    Ok(DesktopAudioRecordingOptions {
        output_path: resolve_next_audio_recording_output_path(
            session.rom_path(),
            session.current_dir.as_path(),
        )?,
        sample_rate_hz: DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ,
        stem_channels: Vec::new(),
    })
}

fn create_audio_recorder(
    mode: &DesktopAudioRecordingMode,
    channel_mask: ApuRecordedChannelMask,
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
) -> Result<Option<DesktopAudioRecorder>, String> {
    let options = match mode {
        DesktopAudioRecordingMode::Disabled => return Ok(None),
        DesktopAudioRecordingMode::Automatic => build_automatic_audio_recording_options(session)?,
        DesktopAudioRecordingMode::Explicit(options) => options.clone(),
    };
    let mut recorder = DesktopAudioRecorder::new(
        &options,
        audio_source_machine(machine).apu().console_model(),
    )?;
    recorder.set_channel_mask(channel_mask)?;
    Ok(Some(recorder))
}

fn finish_audio_recorder(recorder: &mut Option<DesktopAudioRecorder>) -> Result<(), String> {
    if let Some(mut active_recorder) = recorder.take() {
        active_recorder.finish()?;
    }
    Ok(())
}

fn restart_automatic_audio_recorder(
    runtime: &mut FrontendRuntime,
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
) -> Result<(), String> {
    finish_audio_recorder(&mut runtime.audio_recorder)?;
    runtime.audio_recorder = create_audio_recorder(
        &runtime.audio_recording_mode,
        runtime.audio_channel_mask,
        session,
        machine,
    )?;
    Ok(())
}
