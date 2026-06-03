use super::*;

#[test]
fn format_inference_accepts_common_audio_extensions() {
    assert_eq!(
        AudioRecordingFormat::from_output_path(&PathBuf::from("test.wav")),
        Ok(AudioRecordingFormat::Wav)
    );
    assert_eq!(
        AudioRecordingFormat::from_output_path(&PathBuf::from("test.aiff")),
        Ok(AudioRecordingFormat::Aifc)
    );
    assert_eq!(
        AudioRecordingFormat::from_output_path(&PathBuf::from("test.aif")),
        Ok(AudioRecordingFormat::Aifc)
    );
    assert_eq!(
        AudioRecordingFormat::from_output_path(&PathBuf::from("test.aifc")),
        Ok(AudioRecordingFormat::Aifc)
    );
    assert!(AudioRecordingFormat::from_output_path(&PathBuf::from("test.flac")).is_err());
}

#[test]
fn sample_encoding_maps_the_full_host_range_into_i16() {
    assert_eq!(encode_recorded_sample(APU_HOST_MAX_ABS_SAMPLE), i16::MAX);
    assert_eq!(encode_recorded_sample(-APU_HOST_MAX_ABS_SAMPLE), i16::MIN);
    assert_eq!(encode_recorded_sample(0), 0);
}

#[test]
fn wav_recording_writer_finalizes_a_valid_header() {
    let output_path = temp_recording_path("wav");
    let mut writer = AudioRecordingWriter::new(&output_path, 96_000).expect("writer");
    writer
        .write_frame_bytes(&[0, 0, 1, 0, 0, 0, 255, 127], 2)
        .expect("sample frames should write");
    writer.finish().expect("writer should finish");

    let bytes = fs::read(&output_path).expect("recording should exist");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[12..16], b"fmt ");
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        96_000
    );
    assert_eq!(
        u32::from_le_bytes(bytes[40..44].try_into().unwrap()),
        2 * AUDIO_RECORDING_BYTES_PER_FRAME
    );
    assert_eq!(
        bytes.len(),
        (WAV_HEADER_LEN + 2 * AUDIO_RECORDING_BYTES_PER_FRAME) as usize
    );

    let _ = fs::remove_file(output_path);
}

#[test]
fn aifc_recording_writer_finalizes_a_valid_header() {
    let output_path = temp_recording_path("aifc");
    let mut writer =
        AudioRecordingWriter::new(&output_path, DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ)
            .expect("writer");
    writer
        .write_frame_bytes(&[0, 0, 1, 0], 1)
        .expect("sample frame should write");
    writer.finish().expect("writer should finish");

    let bytes = fs::read(&output_path).expect("recording should exist");
    assert_eq!(&bytes[0..4], b"FORM");
    assert_eq!(&bytes[8..12], b"AIFC");
    assert_eq!(&bytes[12..16], b"FVER");
    assert_eq!(&bytes[24..28], b"COMM");
    assert_eq!(&bytes[56..60], b"SSND");
    assert_eq!(u32::from_be_bytes(bytes[34..38].try_into().unwrap()), 1);
    assert_eq!(
        &bytes[40..50],
        &aifc_sample_rate_bytes(DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ)
    );
    #[cfg(target_endian = "big")]
    assert_eq!(&bytes[50..54], b"NONE");
    #[cfg(not(target_endian = "big"))]
    assert_eq!(&bytes[50..54], b"twos");
    assert_eq!(
        u32::from_be_bytes(bytes[60..64].try_into().unwrap()),
        AUDIO_RECORDING_BYTES_PER_FRAME + 8
    );
    assert_eq!(
        bytes.len(),
        (AIFC_HEADER_LEN + AUDIO_RECORDING_BYTES_PER_FRAME) as usize
    );

    let _ = fs::remove_file(output_path);
}
