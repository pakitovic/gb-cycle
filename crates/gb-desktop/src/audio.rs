use gb_core::{
    APU_HOST_MAX_ABS_SAMPLE, Apu, ApuHostSample, ApuSampleCapture, ApuSampleCaptureError,
};
use gb_desktop::AudioOptions;
use sdl3::AudioSubsystem;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use std::mem::size_of;

const AUDIO_CHANNEL_COUNT: i32 = 2;
const BYTES_PER_F32_SAMPLE: i32 = size_of::<f32>() as i32;

pub struct DesktopAudioOutput {
    capture: ApuSampleCapture,
    captured_samples: Vec<ApuHostSample>,
    stream: AudioStreamOwner,
    interleaved_buffer: Vec<f32>,
    output_sample_rate_hz: u32,
    volume_percent: u8,
    volume_scale: f32,
    muted: bool,
    max_queued_bytes: i32,
}

impl DesktopAudioOutput {
    pub fn new(audio: &AudioSubsystem, options: &AudioOptions) -> Result<Self, String> {
        let app_spec = AudioSpec::new(
            Some(options.output_sample_rate_hz as i32),
            Some(AUDIO_CHANNEL_COUNT),
            Some(AudioFormat::f32_sys()),
        );
        let stream = audio
            .default_playback_device()
            .open_device_stream(Some(&app_spec))
            .map_err(|error| format!("failed to open SDL3 audio playback stream: {error}"))?;
        stream
            .resume()
            .map_err(|error| format!("failed to start SDL3 audio playback stream: {error}"))?;

        Ok(Self {
            capture: ApuSampleCapture::new(options.output_sample_rate_hz)
                .map_err(format_capture_error)?,
            captured_samples: Vec::new(),
            stream,
            interleaved_buffer: Vec::new(),
            output_sample_rate_hz: options.output_sample_rate_hz,
            volume_percent: options.volume_percent,
            volume_scale: f32::from(options.volume_percent) / 100.0,
            muted: false,
            max_queued_bytes: i32::from(options.buffer_frames)
                * AUDIO_CHANNEL_COUNT
                * BYTES_PER_F32_SAMPLE
                * 4,
        })
    }

    pub fn capture_t_cycle(&mut self, apu: &Apu) {
        self.capture.record_t_cycle(apu);
    }

    pub fn submit_captured_samples(&mut self) -> Result<(), String> {
        self.capture.drain_samples_into(&mut self.captured_samples);
        if self.captured_samples.is_empty() {
            return Ok(());
        }

        let queued_bytes = self
            .stream
            .queued_bytes()
            .map_err(|error| format!("failed to query queued SDL3 audio bytes: {error}"))?;
        if queued_bytes > self.max_queued_bytes {
            self.stream
                .clear()
                .map_err(|error| format!("failed to clear queued SDL3 audio bytes: {error}"))?;
        }

        let sample_scale = if self.muted { 0.0 } else { self.volume_scale };
        self.interleaved_buffer.clear();
        self.interleaved_buffer
            .reserve(self.captured_samples.len() * 2);
        for sample in self.captured_samples.iter().copied() {
            self.interleaved_buffer
                .push(normalize_sample(sample.left) * sample_scale);
            self.interleaved_buffer
                .push(normalize_sample(sample.right) * sample_scale);
        }

        self.stream
            .put_data_f32(&self.interleaved_buffer)
            .map_err(|error| format!("failed to queue SDL3 audio samples: {error}"))
    }

    pub fn pause(&self) -> Result<(), String> {
        self.stream
            .pause()
            .map_err(|error| format!("failed to pause SDL3 audio stream: {error}"))
    }

    pub fn resume(&self) -> Result<(), String> {
        self.stream
            .resume()
            .map_err(|error| format!("failed to resume SDL3 audio stream: {error}"))
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn set_muted(&mut self, muted: bool) -> Result<(), String> {
        if self.muted == muted {
            return Ok(());
        }

        self.muted = muted;
        self.stream
            .clear()
            .map_err(|error| format!("failed to clear queued SDL3 audio bytes: {error}"))
    }

    pub fn set_volume_percent(&mut self, volume_percent: u8) -> Result<(), String> {
        let volume_percent = volume_percent.min(100);
        if self.volume_percent == volume_percent {
            return Ok(());
        }

        self.volume_percent = volume_percent;
        self.volume_scale = f32::from(volume_percent) / 100.0;
        self.stream
            .clear()
            .map_err(|error| format!("failed to clear queued SDL3 audio bytes: {error}"))
    }

    pub fn clear_buffer(&mut self) -> Result<(), String> {
        self.capture =
            ApuSampleCapture::new(self.output_sample_rate_hz).map_err(format_capture_error)?;
        self.captured_samples.clear();
        self.interleaved_buffer.clear();
        self.stream
            .clear()
            .map_err(|error| format!("failed to clear queued SDL3 audio bytes: {error}"))
    }

    pub fn flush(&self) -> Result<(), String> {
        self.stream
            .flush()
            .map_err(|error| format!("failed to flush SDL3 audio stream: {error}"))
    }

    pub fn queued_duration_ms(&self) -> Option<f64> {
        let queued_bytes = self.stream.queued_bytes().ok()?;
        let bytes_per_second = f64::from(self.output_sample_rate_hz)
            * f64::from(AUDIO_CHANNEL_COUNT)
            * f64::from(BYTES_PER_F32_SAMPLE);
        if bytes_per_second == 0.0 {
            return None;
        }

        Some(f64::from(queued_bytes) * 1_000.0 / bytes_per_second)
    }
}

fn normalize_sample(sample: i32) -> f32 {
    (sample as f32 / APU_HOST_MAX_ABS_SAMPLE as f32).clamp(-1.0, 1.0)
}

fn format_capture_error(error: ApuSampleCaptureError) -> String {
    match error {
        ApuSampleCaptureError::OutputSampleRateZero => {
            "audio output sample rate must be greater than zero".to_string()
        }
    }
}
