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
mod test;
