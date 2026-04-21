use gb_core::{
    APU_HOST_MAX_ABS_SAMPLE, Apu, ApuHostDcBlocker, ApuHostSample, ApuRecordedChannel,
    ApuSampleCapture, ApuSampleCaptureError, ConsoleModel,
};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ: u32 = 96_000;
const AUDIO_RECORDING_CHANNEL_COUNT: u16 = 2;
const AUDIO_RECORDING_BYTES_PER_SAMPLE: u16 = 2;
const AUDIO_RECORDING_BYTES_PER_FRAME: u32 =
    AUDIO_RECORDING_CHANNEL_COUNT as u32 * AUDIO_RECORDING_BYTES_PER_SAMPLE as u32;
const WAV_HEADER_LEN: u32 = 44;
const AIFC_HEADER_LEN: u32 = 72;
const AIFC_COMM_CHUNK_SIZE: u32 = 0x18;
const AIFC_FVER_TIMESTAMP: u32 = 0xA280_5140;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopAudioRecordingOptions {
    pub(crate) output_path: PathBuf,
    pub(crate) sample_rate_hz: u32,
    pub(crate) stem_channels: Vec<ApuRecordedChannel>,
}

#[derive(Debug)]
pub(crate) struct DesktopAudioRecorder {
    mixed_stream: AudioRecordingStream,
    stem_streams: Vec<ChannelAudioRecordingStream>,
}

#[derive(Debug)]
struct ChannelAudioRecordingStream {
    channel: ApuRecordedChannel,
    stream: AudioRecordingStream,
}

#[derive(Debug)]
struct AudioRecordingStream {
    capture: ApuSampleCapture,
    captured_samples: Vec<ApuHostSample>,
    encoded_bytes: Vec<u8>,
    writer: AudioRecordingWriter,
    dc_blocker: Option<ApuHostDcBlocker>,
}

#[derive(Debug)]
struct AudioRecordingWriter {
    output_path: PathBuf,
    file: File,
    format: AudioRecordingFormat,
    sample_rate_hz: u32,
    frame_count: u64,
    finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioRecordingFormat {
    Wav,
    Aifc,
}

impl DesktopAudioRecorder {
    pub(crate) fn new(
        options: &DesktopAudioRecordingOptions,
        console_model: ConsoleModel,
    ) -> Result<Self, String> {
        let mixed_stream =
            AudioRecordingStream::new(&options.output_path, options.sample_rate_hz, None)?;
        let mut stem_streams = Vec::with_capacity(options.stem_channels.len());

        for channel in options.stem_channels.iter().copied() {
            stem_streams.push(ChannelAudioRecordingStream {
                channel,
                stream: AudioRecordingStream::new(
                    &stem_output_path(&options.output_path, channel)?,
                    options.sample_rate_hz,
                    Some(ApuHostDcBlocker::new(console_model, options.sample_rate_hz)),
                )?,
            });
        }

        Ok(Self {
            mixed_stream,
            stem_streams,
        })
    }

    pub(crate) fn capture_t_cycle(&mut self, apu: &Apu) {
        self.mixed_stream.capture.record_t_cycle(apu);

        for stem_stream in &mut self.stem_streams {
            stem_stream
                .stream
                .capture
                .record_output_t_cycle(apu.recorded_channel_sample_pre_hpf(stem_stream.channel));
        }
    }

    pub(crate) fn write_captured_samples(&mut self) -> Result<(), String> {
        self.mixed_stream.write_captured_samples()?;
        for stem_stream in &mut self.stem_streams {
            stem_stream.stream.write_captured_samples()?;
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<(), String> {
        self.write_captured_samples()?;
        self.mixed_stream.finish()?;
        for stem_stream in &mut self.stem_streams {
            stem_stream.stream.finish()?;
        }
        Ok(())
    }
}

impl Drop for DesktopAudioRecorder {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

impl AudioRecordingWriter {
    fn new(output_path: &Path, sample_rate_hz: u32) -> Result<Self, String> {
        if sample_rate_hz == 0 {
            return Err("audio recording sample rate must be greater than zero".to_string());
        }

        let format = AudioRecordingFormat::from_output_path(output_path)?;
        let mut file = File::create(output_path).map_err(|error| {
            format!(
                "failed to create audio recording at {}: {}",
                output_path.display(),
                error
            )
        })?;
        match format {
            AudioRecordingFormat::Wav => write_wav_header(&mut file, sample_rate_hz, 0),
            AudioRecordingFormat::Aifc => write_aifc_header(&mut file, sample_rate_hz, 0),
        }
        .map_err(|error| {
            format!(
                "failed to initialize audio recording header at {}: {}",
                output_path.display(),
                error
            )
        })?;

        Ok(Self {
            output_path: output_path.to_path_buf(),
            file,
            format,
            sample_rate_hz,
            frame_count: 0,
            finished: false,
        })
    }

    fn write_frame_bytes(&mut self, bytes: &[u8], frame_count: u64) -> Result<(), String> {
        self.file.write_all(bytes).map_err(|error| {
            format!(
                "failed to write audio recording samples to {}: {}",
                self.output_path.display(),
                error
            )
        })?;
        self.frame_count += frame_count;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }

        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| format_seek_error(&self.output_path, &error.to_string()))?;
        match self.format {
            AudioRecordingFormat::Wav => {
                write_wav_header(&mut self.file, self.sample_rate_hz, self.frame_count)
            }
            AudioRecordingFormat::Aifc => {
                write_aifc_header(&mut self.file, self.sample_rate_hz, self.frame_count)
            }
        }
        .map_err(|error| {
            format!(
                "failed to finalize audio recording header at {}: {}",
                self.output_path.display(),
                error
            )
        })?;
        self.file
            .flush()
            .map_err(|error| format_flush_error(&self.output_path, &error.to_string()))?;
        self.finished = true;
        Ok(())
    }
}

impl AudioRecordingStream {
    fn new(
        output_path: &Path,
        sample_rate_hz: u32,
        dc_blocker: Option<ApuHostDcBlocker>,
    ) -> Result<Self, String> {
        Ok(Self {
            capture: ApuSampleCapture::new(sample_rate_hz).map_err(format_capture_error)?,
            captured_samples: Vec::new(),
            encoded_bytes: Vec::new(),
            writer: AudioRecordingWriter::new(output_path, sample_rate_hz)?,
            dc_blocker,
        })
    }

    fn write_captured_samples(&mut self) -> Result<(), String> {
        self.capture.drain_samples_into(&mut self.captured_samples);
        if self.captured_samples.is_empty() {
            return Ok(());
        }

        self.encoded_bytes.clear();
        self.encoded_bytes
            .reserve(self.captured_samples.len() * AUDIO_RECORDING_BYTES_PER_FRAME as usize);

        for sample in self.captured_samples.iter().copied() {
            let filtered = match &mut self.dc_blocker {
                Some(dc_blocker) => dc_blocker.filter_sample(sample),
                None => sample,
            };
            let left = encode_recorded_sample(filtered.left);
            let right = encode_recorded_sample(filtered.right);
            self.writer.format.push_i16(&mut self.encoded_bytes, left);
            self.writer.format.push_i16(&mut self.encoded_bytes, right);
        }

        self.writer
            .write_frame_bytes(&self.encoded_bytes, self.captured_samples.len() as u64)
    }

    fn finish(&mut self) -> Result<(), String> {
        self.writer.finish()
    }
}

impl AudioRecordingFormat {
    fn from_output_path(output_path: &Path) -> Result<Self, String> {
        let Some(extension) = output_path.extension().and_then(OsStr::to_str) else {
            return Err(format!(
                "unsupported audio recording path {}; use a .wav, .aiff, .aif, or .aifc extension",
                output_path.display()
            ));
        };

        if extension.eq_ignore_ascii_case("wav") {
            Ok(Self::Wav)
        } else if extension.eq_ignore_ascii_case("aiff")
            || extension.eq_ignore_ascii_case("aif")
            || extension.eq_ignore_ascii_case("aifc")
        {
            Ok(Self::Aifc)
        } else {
            Err(format!(
                "unsupported audio recording extension .{}; use .wav, .aiff, .aif, or .aifc",
                extension
            ))
        }
    }

    fn push_i16(self, bytes: &mut Vec<u8>, sample: i16) {
        match self {
            Self::Wav => bytes.extend_from_slice(&sample.to_le_bytes()),
            Self::Aifc => {
                #[cfg(target_endian = "big")]
                bytes.extend_from_slice(&sample.to_be_bytes());
                #[cfg(not(target_endian = "big"))]
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
        }
    }
}

fn encode_recorded_sample(sample: i32) -> i16 {
    let normalized = (sample as f64 / APU_HOST_MAX_ABS_SAMPLE as f64).clamp(-1.0, 1.0);
    if normalized <= -1.0 {
        i16::MIN
    } else if normalized >= 1.0 {
        i16::MAX
    } else {
        (normalized * f64::from(i16::MAX)).round() as i16
    }
}

fn stem_output_path(output_path: &Path, channel: ApuRecordedChannel) -> Result<PathBuf, String> {
    let Some(file_stem) = output_path.file_stem() else {
        return Err(format!(
            "audio recording path {} must include a filename stem",
            output_path.display()
        ));
    };
    let Some(extension) = output_path.extension() else {
        return Err(format!(
            "audio recording path {} must include a supported extension",
            output_path.display()
        ));
    };

    let mut stem_name = file_stem.to_os_string();
    stem_name.push(".");
    stem_name.push(channel_stem_suffix(channel));
    stem_name.push(".");
    stem_name.push(extension);

    Ok(output_path.with_file_name(stem_name))
}

const fn channel_stem_suffix(channel: ApuRecordedChannel) -> &'static str {
    match channel {
        ApuRecordedChannel::Ch1 => "ch1",
        ApuRecordedChannel::Ch2 => "ch2",
        ApuRecordedChannel::Ch3 => "ch3",
        ApuRecordedChannel::Ch4 => "ch4",
    }
}

fn write_wav_header(file: &mut File, sample_rate_hz: u32, frame_count: u64) -> std::io::Result<()> {
    let data_bytes = frame_count as u32 * AUDIO_RECORDING_BYTES_PER_FRAME;
    let riff_size = WAV_HEADER_LEN - 8 + data_bytes;
    let byte_rate = sample_rate_hz * AUDIO_RECORDING_BYTES_PER_FRAME;
    let block_align = AUDIO_RECORDING_BYTES_PER_FRAME as u16;

    let mut header = Vec::with_capacity(WAV_HEADER_LEN as usize);
    header.extend_from_slice(b"RIFF");
    push_u32_le(&mut header, riff_size);
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    push_u32_le(&mut header, 16);
    push_u16_le(&mut header, 1);
    push_u16_le(&mut header, AUDIO_RECORDING_CHANNEL_COUNT);
    push_u32_le(&mut header, sample_rate_hz);
    push_u32_le(&mut header, byte_rate);
    push_u16_le(&mut header, block_align);
    push_u16_le(&mut header, AUDIO_RECORDING_BYTES_PER_SAMPLE * 8);
    header.extend_from_slice(b"data");
    push_u32_le(&mut header, data_bytes);
    file.write_all(&header)
}

fn write_aifc_header(
    file: &mut File,
    sample_rate_hz: u32,
    frame_count: u64,
) -> std::io::Result<()> {
    let data_bytes = frame_count as u32 * AUDIO_RECORDING_BYTES_PER_FRAME;
    let form_size = AIFC_HEADER_LEN - 8 + data_bytes;
    let ssnd_chunk_size = data_bytes + 8;

    let mut header = Vec::with_capacity(AIFC_HEADER_LEN as usize);
    header.extend_from_slice(b"FORM");
    push_u32_be(&mut header, form_size);
    header.extend_from_slice(b"AIFC");
    header.extend_from_slice(b"FVER");
    push_u32_be(&mut header, 4);
    push_u32_be(&mut header, AIFC_FVER_TIMESTAMP);
    header.extend_from_slice(b"COMM");
    push_u32_be(&mut header, AIFC_COMM_CHUNK_SIZE);
    push_u16_be(&mut header, AUDIO_RECORDING_CHANNEL_COUNT);
    push_u32_be(&mut header, frame_count as u32);
    push_u16_be(&mut header, AUDIO_RECORDING_BYTES_PER_SAMPLE * 8);
    header.extend_from_slice(&aifc_sample_rate_bytes(sample_rate_hz));
    #[cfg(target_endian = "big")]
    header.extend_from_slice(b"NONE");
    #[cfg(not(target_endian = "big"))]
    header.extend_from_slice(b"twos");
    push_u16_be(&mut header, 0);
    header.extend_from_slice(b"SSND");
    push_u32_be(&mut header, ssnd_chunk_size);
    push_u32_be(&mut header, 0);
    push_u32_be(&mut header, 0);
    file.write_all(&header)
}

fn aifc_sample_rate_bytes(sample_rate_hz: u32) -> [u8; 10] {
    let mut bytes = [0_u8; 10];
    let mut significand = u64::from(sample_rate_hz);
    let mut exponent = 0x403E_u16;

    while (significand as i64) > 0 {
        significand <<= 1;
        exponent -= 1;
    }

    bytes[..2].copy_from_slice(&exponent.to_be_bytes());
    bytes[2..].copy_from_slice(&significand.to_be_bytes());
    bytes
}

fn push_u16_le(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32_le(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u16_be(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn push_u32_be(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn format_capture_error(error: ApuSampleCaptureError) -> String {
    match error {
        ApuSampleCaptureError::OutputSampleRateZero => {
            "audio recording sample rate must be greater than zero".to_string()
        }
    }
}

fn format_seek_error(output_path: &Path, error: &str) -> String {
    format!(
        "failed to seek while finalizing audio recording at {}: {}",
        output_path.display(),
        error
    )
}

fn format_flush_error(output_path: &Path, error: &str) -> String {
    format!(
        "failed to flush audio recording at {}: {}",
        output_path.display(),
        error
    )
}

#[cfg(test)]
mod tests {
    use super::{
        AIFC_HEADER_LEN, AUDIO_RECORDING_BYTES_PER_FRAME, AudioRecordingFormat,
        AudioRecordingWriter, DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ, DesktopAudioRecorder,
        DesktopAudioRecordingOptions, WAV_HEADER_LEN, aifc_sample_rate_bytes, channel_stem_suffix,
        encode_recorded_sample, stem_output_path,
    };
    use gb_core::{APU_HOST_MAX_ABS_SAMPLE, ApuRecordedChannel, ConsoleModel};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_recording_path(extension: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("gb-cycle-audio-recording-{unique}.{extension}"))
    }

    #[test]
    fn format_inference_accepts_sameboy_style_extensions() {
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
    fn aifc_recording_writer_finalizes_a_sameboy_style_header() {
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

    #[test]
    fn recorder_reports_unsupported_extensions() {
        let error = DesktopAudioRecorder::new(
            &DesktopAudioRecordingOptions {
                output_path: PathBuf::from("recording.txt"),
                sample_rate_hz: 48_000,
                stem_channels: Vec::new(),
            },
            ConsoleModel::Dmg,
        )
        .expect_err("unsupported extensions should fail");
        assert!(error.contains("unsupported audio recording extension"));
    }

    #[test]
    fn stem_output_paths_use_sidecar_channel_suffixes() {
        assert_eq!(channel_stem_suffix(ApuRecordedChannel::Ch1), "ch1");
        assert_eq!(channel_stem_suffix(ApuRecordedChannel::Ch4), "ch4");
        assert_eq!(
            stem_output_path(&PathBuf::from("/tmp/zelda.wav"), ApuRecordedChannel::Ch4,)
                .expect("stem output path"),
            PathBuf::from("/tmp/zelda.ch4.wav")
        );
        assert_eq!(
            stem_output_path(&PathBuf::from("/tmp/zelda.aifc"), ApuRecordedChannel::Ch2,)
                .expect("stem output path"),
            PathBuf::from("/tmp/zelda.ch2.aifc")
        );
    }
}
