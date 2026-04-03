use gb_core::{
    APU_HOST_MAX_ABS_SAMPLE, Apu, ApuHostSample, ApuSampleCapture, ApuSampleCaptureError,
};
use gb_desktop::AudioOptions;
use sdl3::AudioSubsystem;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use std::fmt::Display;
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
        let stream = map_audio_result(
            audio
                .default_playback_device()
                .open_device_stream(Some(&app_spec)),
            "failed to open SDL3 audio playback stream",
        )?;
        map_audio_result(
            stream.resume(),
            "failed to start SDL3 audio playback stream",
        )?;

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

        let queued_bytes = map_audio_result(
            self.stream.queued_bytes(),
            "failed to query queued SDL3 audio bytes",
        )?;
        if queued_bytes > self.max_queued_bytes {
            map_audio_result(
                self.stream.clear(),
                "failed to clear queued SDL3 audio bytes",
            )?;
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

        map_audio_result(
            self.stream.put_data_f32(&self.interleaved_buffer),
            "failed to queue SDL3 audio samples",
        )
    }

    pub fn pause(&self) -> Result<(), String> {
        map_audio_result(self.stream.pause(), "failed to pause SDL3 audio stream")
    }

    pub fn resume(&self) -> Result<(), String> {
        map_audio_result(self.stream.resume(), "failed to resume SDL3 audio stream")
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn set_muted(&mut self, muted: bool) -> Result<(), String> {
        if self.muted == muted {
            return Ok(());
        }

        self.muted = muted;
        map_audio_result(
            self.stream.clear(),
            "failed to clear queued SDL3 audio bytes",
        )
    }

    pub fn set_volume_percent(&mut self, volume_percent: u8) -> Result<(), String> {
        let volume_percent = volume_percent.min(100);
        if self.volume_percent == volume_percent {
            return Ok(());
        }

        self.volume_percent = volume_percent;
        self.volume_scale = f32::from(volume_percent) / 100.0;
        map_audio_result(
            self.stream.clear(),
            "failed to clear queued SDL3 audio bytes",
        )
    }

    pub fn clear_buffer(&mut self) -> Result<(), String> {
        self.capture =
            ApuSampleCapture::new(self.output_sample_rate_hz).map_err(format_capture_error)?;
        self.captured_samples.clear();
        self.interleaved_buffer.clear();
        map_audio_result(
            self.stream.clear(),
            "failed to clear queued SDL3 audio bytes",
        )
    }

    pub fn flush(&self) -> Result<(), String> {
        map_audio_result(self.stream.flush(), "failed to flush SDL3 audio stream")
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

fn map_audio_result<T, E>(result: Result<T, E>, context: &str) -> Result<T, String>
where
    E: Display,
{
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(format_audio_error(context, &error.to_string())),
    }
}

fn format_audio_error(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

fn format_capture_error(error: ApuSampleCaptureError) -> String {
    match error {
        ApuSampleCaptureError::OutputSampleRateZero => {
            "audio output sample rate must be greater than zero".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AUDIO_CHANNEL_COUNT, BYTES_PER_F32_SAMPLE, DesktopAudioOutput, format_audio_error,
        format_capture_error, map_audio_result, normalize_sample,
    };
    use gb_core::{
        APU_HOST_MAX_ABS_SAMPLE, Apu, ApuHostSample, ApuSampleCaptureError, ConsoleModel,
    };
    use gb_desktop::AudioOptions;
    use sdl3::{AudioSubsystem, hint};

    fn init_audio_subsystem() -> AudioSubsystem {
        crate::configure_headless_sdl();
        let _ = hint::set("SDL_AUDIO_DRIVER", "dummy");
        let _ = hint::set("SDL_AUDIO_DUMMY_TIMESCALE", "0");
        let sdl = sdl3::init().expect("failed to initialize SDL");
        sdl.audio()
            .expect("failed to initialize the SDL audio subsystem")
    }

    fn test_audio_options() -> AudioOptions {
        AudioOptions {
            output_sample_rate_hz: 48_000,
            buffer_frames: 16,
            ..AudioOptions::default()
        }
    }

    fn push_captured_sample(output: &mut DesktopAudioOutput, sample: ApuHostSample) {
        let pending_before = output.capture.pending_sample_count();
        while output.capture.pending_sample_count() == pending_before {
            output.capture.record_output_t_cycle(sample);
        }
    }

    #[test]
    fn desktop_audio_output_scales_queues_and_clears_captured_samples() {
        let _guard = crate::lock_sdl_test();
        let audio = init_audio_subsystem();
        let mut output =
            DesktopAudioOutput::new(&audio, &test_audio_options()).expect("audio output");
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
        assert!(
            output
                .queued_duration_ms()
                .expect("queued duration should exist")
                >= 0.0
        );

        output.max_queued_bytes = -1;
        push_captured_sample(
            &mut output,
            ApuHostSample {
                left: APU_HOST_MAX_ABS_SAMPLE,
                right: APU_HOST_MAX_ABS_SAMPLE,
            },
        );
        output
            .submit_captured_samples()
            .expect("submit_captured_samples should clear oversized queues");
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
    fn desktop_audio_output_controls_pause_volume_and_buffer_reset() {
        let _guard = crate::lock_sdl_test();
        let audio = init_audio_subsystem();
        let mut output =
            DesktopAudioOutput::new(&audio, &test_audio_options()).expect("audio output");

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

        output.capture_t_cycle(&Apu::new(ConsoleModel::Dmg));
        output
            .submit_captured_samples()
            .expect("empty submit_captured_samples");
        assert!(output.captured_samples.is_empty());

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

    #[test]
    fn audio_helpers_cover_normalization_duration_and_capture_errors() {
        assert_eq!(normalize_sample(APU_HOST_MAX_ABS_SAMPLE / 4), 0.25);
        assert_eq!(normalize_sample(APU_HOST_MAX_ABS_SAMPLE * 2), 1.0);
        assert_eq!(normalize_sample(-APU_HOST_MAX_ABS_SAMPLE * 2), -1.0);
        assert_eq!(
            format_audio_error("failed to pause SDL3 audio stream", "paused"),
            "failed to pause SDL3 audio stream: paused"
        );
        assert_eq!(
            map_audio_result::<(), _>(Err("stream"), "stream op")
                .expect_err("error mapping should preserve context"),
            "stream op: stream"
        );
        assert_eq!(
            format_capture_error(ApuSampleCaptureError::OutputSampleRateZero),
            "audio output sample rate must be greater than zero"
        );

        let _guard = crate::lock_sdl_test();
        let audio = init_audio_subsystem();
        let mut output =
            DesktopAudioOutput::new(&audio, &test_audio_options()).expect("audio output");
        output.output_sample_rate_hz = 0;
        assert_eq!(output.queued_duration_ms(), None);
        assert_eq!(
            output
                .clear_buffer()
                .expect_err("clear_buffer should reject zero Hz"),
            "audio output sample rate must be greater than zero"
        );

        assert_eq!(AUDIO_CHANNEL_COUNT, 2);
        assert_eq!(BYTES_PER_F32_SAMPLE, std::mem::size_of::<f32>() as i32);
    }
}
