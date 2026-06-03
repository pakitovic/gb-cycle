use super::*;

#[test]
fn recorder_reports_unsupported_extensions() {
    let error = DesktopAudioRecorder::new(
        &DesktopAudioRecordingOptions {
            output_path: PathBuf::from("recording.txt"),
            sample_rate_hz: 48_000,
            stem_channels: Vec::new(),
        },
        ConsoleModel::GameBoy,
    )
    .expect_err("unsupported extensions should fail");
    assert!(error.contains("unsupported audio recording extension"));
}

#[test]
fn recorder_rejects_zero_sample_rate() {
    let error = DesktopAudioRecorder::new(
        &DesktopAudioRecordingOptions {
            output_path: PathBuf::from("recording.wav"),
            sample_rate_hz: 0,
            stem_channels: Vec::new(),
        },
        ConsoleModel::GameBoy,
    )
    .expect_err("zero sample rate should fail");
    assert_eq!(
        error,
        "audio recording sample rate must be greater than zero"
    );
}

#[test]
fn writer_finish_is_idempotent() {
    let output_path = temp_recording_path("wav");
    let mut writer = AudioRecordingWriter::new(&output_path, 96_000).expect("writer");
    writer.finish().expect("first finish should succeed");
    writer.finish().expect("second finish should also succeed");

    let bytes = fs::read(&output_path).expect("recording should exist");
    assert_eq!(bytes.len(), WAV_HEADER_LEN as usize);

    let _ = fs::remove_file(output_path);
}

#[test]
fn writer_rejects_recordings_that_overflow_container_header_sizes() {
    let wav_path = temp_recording_path("wav");
    let mut wav_writer = AudioRecordingWriter::new(&wav_path, 96_000).expect("wav writer");
    wav_writer.frame_count = u64::from(u32::MAX / AUDIO_RECORDING_BYTES_PER_FRAME) + 1;
    assert!(
        wav_writer
            .finish()
            .expect_err("oversized wav recordings should fail header finalization")
            .contains("too large to fit the selected file format header")
    );

    let aifc_path = temp_recording_path("aifc");
    let mut aifc_writer =
        AudioRecordingWriter::new(&aifc_path, DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ)
            .expect("aifc writer");
    aifc_writer.frame_count = u64::from(u32::MAX / AUDIO_RECORDING_BYTES_PER_FRAME) + 1;
    assert!(
        aifc_writer
            .finish()
            .expect_err("oversized aifc recordings should fail header finalization")
            .contains("too large to fit the selected file format header")
    );

    let _ = fs::remove_file(wav_path);
    let _ = fs::remove_file(aifc_path);
}

#[test]
fn low_level_audio_recording_helpers_cover_remaining_error_and_format_paths() {
    assert_eq!(
        AudioRecordingWriter::new(&PathBuf::from("recording.wav"), 0)
            .expect_err("zero sample rate should fail"),
        "audio recording sample rate must be greater than zero"
    );
    assert!(
        AudioRecordingFormat::from_output_path(&PathBuf::from("recording"))
            .expect_err("missing extensions should fail")
            .contains("unsupported audio recording path")
    );

    let mut aifc_bytes = Vec::new();
    AudioRecordingFormat::Aifc.push_i16(&mut aifc_bytes, 0x1234);
    assert_eq!(aifc_bytes, i16::to_le_bytes(0x1234).to_vec());
    assert_eq!(channel_stem_suffix(ApuRecordedChannel::Ch3), "ch3");
    assert_eq!(
        format_seek_error(&PathBuf::from("/tmp/recording.wav"), "boom"),
        "failed to seek while finalizing audio recording at /tmp/recording.wav: boom"
    );
    assert_eq!(
        format_flush_error(&PathBuf::from("/tmp/recording.wav"), "boom"),
        "failed to flush audio recording at /tmp/recording.wav: boom"
    );
}

#[test]
fn writer_surfaces_create_write_and_seek_failures() {
    let create_error_path = temp_recording_path("wav");
    fs::create_dir(&create_error_path).expect("directory-backed error path");
    let create_error = AudioRecordingWriter::new(&create_error_path, 96_000)
        .expect_err("directory outputs should fail to open as files");
    assert!(create_error.contains("failed to create audio recording"));
    fs::remove_dir(&create_error_path).expect("directory-backed error path should clean up");

    let output_path = temp_recording_path("wav");
    let read_only_file = File::options()
        .read(true)
        .open(&{
            let mut file = File::create(&output_path).expect("backing file");
            file.write_all(b"seed").expect("seed bytes");
            output_path.clone()
        })
        .expect("read-only file");
    let mut write_error_writer = AudioRecordingWriter {
        output_path: output_path.clone(),
        file: read_only_file,
        format: AudioRecordingFormat::Wav,
        sample_rate_hz: 96_000,
        frame_count: 0,
        finished: false,
    };
    assert!(
        write_error_writer
            .write_frame_bytes(&[0, 0, 0, 0], 1)
            .expect_err("read-only files should reject sample writes")
            .contains("failed to write audio recording samples")
    );

    let read_only_header_file = File::options()
        .read(true)
        .open(&output_path)
        .expect("read-only header file");
    let mut finalize_error_writer = AudioRecordingWriter {
        output_path: output_path.clone(),
        file: read_only_header_file,
        format: AudioRecordingFormat::Wav,
        sample_rate_hz: 96_000,
        frame_count: 1,
        finished: false,
    };
    assert!(
        finalize_error_writer
            .finish()
            .expect_err("read-only files should reject header finalization")
            .contains("failed to finalize audio recording header")
    );

    let (stream_a, _stream_b) = UnixStream::pair().expect("unix stream pair");
    let mut seek_error_writer = AudioRecordingWriter {
        output_path: output_path.clone(),
        file: unsafe { File::from_raw_fd(stream_a.into_raw_fd()) },
        format: AudioRecordingFormat::Wav,
        sample_rate_hz: 96_000,
        frame_count: 0,
        finished: false,
    };
    assert!(
        seek_error_writer
            .finish()
            .expect_err("non-seekable files should fail during finalization")
            .contains("failed to seek while finalizing audio recording")
    );

    let _ = fs::remove_file(output_path);
}
