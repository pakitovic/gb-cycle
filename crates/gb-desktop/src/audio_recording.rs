use gb_core::{
    APU_HOST_MAX_ABS_SAMPLE, Apu, ApuHostHpf, ApuHostSample, ApuRecordedChannel,
    ApuRecordedChannelMask, ApuSampleCapture, ApuSampleCaptureError, ConsoleModel,
};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ: u32 = 96_000;
const AUDIO_RECORDING_OUTPUT_SUBDIRECTORY: &str = "audios";
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
    channel_mask: ApuRecordedChannelMask,
    console_model: ConsoleModel,
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
    post_hpf_filter: Option<ApuHostHpf>,
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
                    Some(ApuHostHpf::new(console_model)),
                )?,
            });
        }

        Ok(Self {
            mixed_stream,
            stem_streams,
            channel_mask: ApuRecordedChannelMask::ALL,
            console_model,
        })
    }

    pub(crate) fn capture_t_cycle(&mut self, apu: &Apu) {
        if self.channel_mask.is_all() {
            self.mixed_stream.capture.record_t_cycle(apu);
        } else {
            let tap = apu.recorded_channel_mix_tap_pre_hpf(self.channel_mask);
            let filtered = self
                .mixed_stream
                .post_hpf_filter
                .as_mut()
                .expect("subset recording must own a masked-mix HPF")
                .filter_t_cycle(tap.sample, tap.any_output_connected);
            self.mixed_stream.capture.record_output_t_cycle(filtered);
        }

        for stem_stream in &mut self.stem_streams {
            let tap = apu.recorded_channel_tap_pre_hpf(stem_stream.channel);
            let filtered = stem_stream
                .stream
                .post_hpf_filter
                .as_mut()
                .expect("channel stems must own a post-HPF filter")
                .filter_t_cycle(tap.sample, tap.any_output_connected);
            stem_stream.stream.capture.record_output_t_cycle(filtered);
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

    #[cfg(test)]
    pub(crate) fn channel_mask(&self) -> ApuRecordedChannelMask {
        self.channel_mask
    }

    pub(crate) fn set_channel_mask(
        &mut self,
        channel_mask: ApuRecordedChannelMask,
    ) -> Result<(), String> {
        if self.channel_mask == channel_mask {
            return Ok(());
        }

        self.write_captured_samples()?;
        self.channel_mask = channel_mask;
        self.reset_mixed_stream_capture()?;
        Ok(())
    }

    pub(crate) fn reset_for_session_swap(
        &mut self,
        console_model: ConsoleModel,
    ) -> Result<(), String> {
        self.write_captured_samples()?;
        if self.console_model != console_model {
            self.console_model = console_model;
        }
        self.reset_all_capture_state()?;
        Ok(())
    }

    fn reset_mixed_stream_capture(&mut self) -> Result<(), String> {
        let post_hpf_filter = if self.channel_mask.is_all() {
            None
        } else {
            Some(ApuHostHpf::new(self.console_model))
        };
        self.mixed_stream
            .reset_capture(self.mixed_stream.writer.sample_rate_hz, post_hpf_filter)
    }

    fn reset_all_capture_state(&mut self) -> Result<(), String> {
        self.reset_mixed_stream_capture()?;
        for stem_stream in &mut self.stem_streams {
            stem_stream.stream.reset_capture(
                stem_stream.stream.writer.sample_rate_hz,
                Some(ApuHostHpf::new(self.console_model)),
            )?;
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
        post_hpf_filter: Option<ApuHostHpf>,
    ) -> Result<Self, String> {
        Ok(Self {
            capture: ApuSampleCapture::new(sample_rate_hz).map_err(format_capture_error)?,
            captured_samples: Vec::new(),
            encoded_bytes: Vec::new(),
            writer: AudioRecordingWriter::new(output_path, sample_rate_hz)?,
            post_hpf_filter,
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
            let left = encode_recorded_sample(sample.left);
            let right = encode_recorded_sample(sample.right);
            self.writer.format.push_i16(&mut self.encoded_bytes, left);
            self.writer.format.push_i16(&mut self.encoded_bytes, right);
        }

        self.writer
            .write_frame_bytes(&self.encoded_bytes, self.captured_samples.len() as u64)
    }

    fn finish(&mut self) -> Result<(), String> {
        self.writer.finish()
    }

    fn reset_capture(
        &mut self,
        sample_rate_hz: u32,
        post_hpf_filter: Option<ApuHostHpf>,
    ) -> Result<(), String> {
        self.capture = ApuSampleCapture::new(sample_rate_hz).map_err(format_capture_error)?;
        self.captured_samples.clear();
        self.encoded_bytes.clear();
        self.post_hpf_filter = post_hpf_filter;
        Ok(())
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

pub(crate) fn resolve_next_audio_recording_output_path(
    rom_path: Option<&Path>,
    current_dir: &Path,
) -> Result<PathBuf, String> {
    let output_dir = audio_recording_output_directory(rom_path, current_dir);
    fs::create_dir_all(&output_dir).map_err(|error| {
        crate::format_path_error(
            "failed to create audio recording output directory",
            &output_dir,
            &error.to_string(),
        )
    })?;

    let stem = audio_recording_output_stem(rom_path);
    for index in 0..=u16::MAX {
        let candidate = output_dir.join(format!("{stem}-{index}.wav"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(crate::format_path_error(
        "failed to allocate a free audio recording path in",
        &output_dir,
        "directory is full",
    ))
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

fn audio_recording_output_directory(rom_path: Option<&Path>, current_dir: &Path) -> PathBuf {
    let base_dir = match rom_path.and_then(Path::parent) {
        Some(parent) => parent.to_path_buf(),
        None => current_dir.to_path_buf(),
    };
    base_dir.join(AUDIO_RECORDING_OUTPUT_SUBDIRECTORY)
}

fn audio_recording_output_stem(rom_path: Option<&Path>) -> String {
    rom_path
        .and_then(Path::file_stem)
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("gb-cycle")
        .to_string()
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
    let frame_count_u32 = checked_recording_frame_count(frame_count)?;
    let data_bytes = checked_recording_data_bytes(frame_count_u32)?;
    let riff_size = checked_recording_chunk_size(WAV_HEADER_LEN - 8, data_bytes, frame_count)?;
    let byte_rate = sample_rate_hz
        .checked_mul(AUDIO_RECORDING_BYTES_PER_FRAME)
        .ok_or_else(|| recording_header_overflow_error(frame_count))?;
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
    let frame_count_u32 = checked_recording_frame_count(frame_count)?;
    let data_bytes = checked_recording_data_bytes(frame_count_u32)?;
    let form_size = checked_recording_chunk_size(AIFC_HEADER_LEN - 8, data_bytes, frame_count)?;
    let ssnd_chunk_size = checked_recording_chunk_size(8, data_bytes, frame_count)?;

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
    push_u32_be(&mut header, frame_count_u32);
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

fn checked_recording_frame_count(frame_count: u64) -> std::io::Result<u32> {
    u32::try_from(frame_count).map_err(|_| recording_header_overflow_error(frame_count))
}

fn checked_recording_data_bytes(frame_count: u32) -> std::io::Result<u32> {
    frame_count
        .checked_mul(AUDIO_RECORDING_BYTES_PER_FRAME)
        .ok_or_else(|| recording_header_overflow_error(u64::from(frame_count)))
}

fn checked_recording_chunk_size(
    base_size: u32,
    data_bytes: u32,
    frame_count: u64,
) -> std::io::Result<u32> {
    base_size
        .checked_add(data_bytes)
        .ok_or_else(|| recording_header_overflow_error(frame_count))
}

fn recording_header_overflow_error(frame_count: u64) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!(
            "audio recording is too large to fit the selected file format header (frame_count={frame_count})"
        ),
    )
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
        DesktopAudioRecordingOptions, WAV_HEADER_LEN, aifc_sample_rate_bytes,
        audio_recording_output_directory, audio_recording_output_stem, channel_stem_suffix,
        encode_recorded_sample, format_flush_error, format_seek_error,
        resolve_next_audio_recording_output_path, stem_output_path,
    };
    use gb_core::{
        APU_HOST_MAX_ABS_SAMPLE, Apu, ApuRecordedChannel, ApuRecordedChannelMask, ConsoleModel,
        DMG_FAMILY_APU_CAPTURE_CLOCK_HZ,
    };
    use std::fs::{self, File};
    use std::io::Write;
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::os::unix::net::UnixStream;
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

    fn configure_constant_ch1_output(apu: &mut Apu) {
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF12, 0x08);
        apu.write_register(0xFF24, 0x77);
        apu.write_register(0xFF25, 0x11);
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
    fn recorder_rejects_zero_sample_rate() {
        let error = DesktopAudioRecorder::new(
            &DesktopAudioRecordingOptions {
                output_path: PathBuf::from("recording.wav"),
                sample_rate_hz: 0,
                stem_channels: Vec::new(),
            },
            ConsoleModel::Dmg,
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
        assert!(
            stem_output_path(&PathBuf::from("/tmp/zelda"), ApuRecordedChannel::Ch1)
                .expect_err("paths without extensions should fail")
                .contains("supported extension")
        );
        assert!(
            stem_output_path(&PathBuf::from("/"), ApuRecordedChannel::Ch1)
                .expect_err("paths without filename stems should fail")
                .contains("filename stem")
        );
    }

    #[test]
    fn automatic_audio_recordings_use_an_audios_sidecar_directory() {
        let root = temp_recording_path("dir");
        fs::create_dir_all(&root).expect("root directory");
        let rom_path = root.join("zelda.gb");
        fs::write(&rom_path, b"rom").expect("rom file");

        let first = resolve_next_audio_recording_output_path(Some(&rom_path), &root)
            .expect("first automatic path");
        assert_eq!(first, root.join("audios/zelda-0.wav"));

        fs::create_dir_all(first.parent().expect("audio output parent")).expect("audio dir");
        fs::write(&first, b"existing").expect("existing recording");

        let second = resolve_next_audio_recording_output_path(Some(&rom_path), &root)
            .expect("second automatic path");
        assert_eq!(second, root.join("audios/zelda-1.wav"));

        let _ = fs::remove_file(first);
        let _ = fs::remove_file(rom_path);
        let _ = fs::remove_dir_all(root.join("audios"));
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn automatic_audio_recording_helpers_fall_back_without_a_loaded_rom() {
        let root = temp_recording_path("dir");
        fs::create_dir_all(&root).expect("root directory");

        assert_eq!(
            audio_recording_output_directory(None, &root),
            root.join("audios")
        );
        assert_eq!(audio_recording_output_stem(None), "gb-cycle");

        let path = resolve_next_audio_recording_output_path(None, &root)
            .expect("automatic recording path without a rom");
        assert_eq!(path, root.join("audios/gb-cycle-0.wav"));

        let _ = fs::remove_dir_all(root.join("audios"));
        let _ = fs::remove_dir(root);
    }

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
            ConsoleModel::Dmg,
        )
        .expect("recorder");
        let mut apu = Apu::new(ConsoleModel::Dmg);
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
            ConsoleModel::Dmg,
        )
        .expect("recorder");
        let mut apu = Apu::new(ConsoleModel::Dmg);
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
            ConsoleModel::Dmg,
        )
        .expect("recorder");
        let mut apu = Apu::new(ConsoleModel::Dmg);
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
            .reset_for_session_swap(ConsoleModel::Mgb)
            .expect("console-model changes should reset all capture state");
        assert_eq!(recorder.console_model, ConsoleModel::Mgb);
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
            .reset_for_session_swap(ConsoleModel::Mgb)
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
            ConsoleModel::Dmg,
        )
        .expect("recorder");
        let mut apu = Apu::new(ConsoleModel::Dmg);
        configure_constant_ch1_output(&mut apu);

        for _ in 0..8 {
            recorder.capture_t_cycle(&apu);
        }
        assert!(recorder.mixed_stream.capture.pending_sample_count() > 0);

        recorder
            .reset_for_session_swap(ConsoleModel::Dmg)
            .expect("session swaps should flush and reset recording capture");
        assert_eq!(recorder.console_model, ConsoleModel::Dmg);
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
}
