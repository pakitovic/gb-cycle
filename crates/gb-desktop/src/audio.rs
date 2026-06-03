use gb_core::{
    APU_HOST_MAX_ABS_SAMPLE, Apu, ApuHostHpf, ApuHostSample, ApuRecordedChannelMask,
    ApuSampleCapture, ApuSampleCaptureError, ConsoleModel,
};
use gb_desktop::AudioOptions;
use sdl3::AudioSubsystem;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use std::cell::Cell;
use std::env;
use std::ffi::OsStr;
use std::fmt::Display;
use std::mem::size_of;

const AUDIO_CHANNEL_COUNT: i32 = 2;
const BYTES_PER_F32_SAMPLE: i32 = size_of::<f32>() as i32;
const OVERSIZED_QUEUE_CLEAR_BUFFER_MULTIPLIER: i32 = 192;
const OVERSIZED_QUEUE_CLEAR_STREAK: u8 = 3;
const LATENCY_QUEUE_CLEAR_MS: f64 = 500.0;
const LATENCY_QUEUE_CLEAR_STREAK: u8 = 3;
pub(crate) const DESKTOP_AUDIO_LOG_ENV_VAR: &str = "GB_CYCLE_DESKTOP_AUDIO_LOG";
pub(crate) const DESKTOP_AUDIO_DISABLE_AUTO_CLEAR_ENV_VAR: &str =
    "GB_CYCLE_DESKTOP_AUDIO_DISABLE_AUTO_CLEAR";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AudioTelemetryMode {
    #[default]
    Disabled,
    Events,
    Verbose,
}

#[derive(Debug, Default)]
struct AudioTelemetry {
    mode: AudioTelemetryMode,
    next_sequence: Cell<u64>,
    queue_clear_count: Cell<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct AudioSubmitTelemetry {
    pub(crate) sample_count: usize,
    pub(crate) captured_t_cycles: usize,
    pub(crate) queued_ms_before: Option<f64>,
    pub(crate) enqueued_ms: Option<f64>,
    pub(crate) queued_ms_after: Option<f64>,
}

pub struct DesktopAudioOutput {
    capture: ApuSampleCapture,
    captured_samples: Vec<ApuHostSample>,
    stream: AudioStreamOwner,
    interleaved_buffer: Vec<f32>,
    output_sample_rate_hz: u32,
    volume_percent: u8,
    volume_scale: f32,
    muted: bool,
    auto_queue_clear_enabled: bool,
    max_queued_bytes: i32,
    oversized_queue_streak: u8,
    latency_queue_streak: u8,
    captured_t_cycles_since_submit: usize,
    telemetry: AudioTelemetry,
    last_submit_telemetry: Option<AudioSubmitTelemetry>,
    console_model: ConsoleModel,
    channel_mask: ApuRecordedChannelMask,
    masked_mix_hpf: Option<ApuHostHpf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AutoQueueClearPolicy {
    #[default]
    Enabled,
    Disabled,
}

impl AudioTelemetryMode {
    fn from_env() -> Self {
        Self::from_env_value(env::var_os(DESKTOP_AUDIO_LOG_ENV_VAR).as_deref())
    }

    fn from_env_value(value: Option<&OsStr>) -> Self {
        let Some(value) = value else {
            return Self::Disabled;
        };

        let value = value.to_string_lossy();
        if value.is_empty()
            || value == "0"
            || value.eq_ignore_ascii_case("false")
            || value.eq_ignore_ascii_case("off")
            || value.eq_ignore_ascii_case("no")
        {
            Self::Disabled
        } else if value.eq_ignore_ascii_case("verbose")
            || value.eq_ignore_ascii_case("debug")
            || value.eq_ignore_ascii_case("all")
        {
            Self::Verbose
        } else {
            Self::Events
        }
    }

    fn enabled(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    fn log_submit_batches(self) -> bool {
        matches!(self, Self::Verbose)
    }
}

impl AutoQueueClearPolicy {
    fn from_env() -> Self {
        Self::from_env_value(env::var_os(DESKTOP_AUDIO_DISABLE_AUTO_CLEAR_ENV_VAR).as_deref())
    }

    fn from_env_value(value: Option<&OsStr>) -> Self {
        let Some(value) = value else {
            return Self::Enabled;
        };

        let value = value.to_string_lossy();
        if value.is_empty()
            || value.eq_ignore_ascii_case("1")
            || value.eq_ignore_ascii_case("true")
            || value.eq_ignore_ascii_case("on")
            || value.eq_ignore_ascii_case("yes")
            || value.eq_ignore_ascii_case("disable")
            || value.eq_ignore_ascii_case("disabled")
        {
            Self::Disabled
        } else {
            Self::Enabled
        }
    }

    fn auto_queue_clear_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

impl AudioTelemetry {
    fn from_env() -> Self {
        Self {
            mode: AudioTelemetryMode::from_env(),
            next_sequence: Cell::new(0),
            queue_clear_count: Cell::new(0),
        }
    }

    fn enabled(&self) -> bool {
        self.mode.enabled()
    }

    fn next_sequence(&self) -> u64 {
        let sequence = self.next_sequence.get();
        self.next_sequence.set(sequence + 1);
        sequence
    }

    fn record_queue_clear(&self) -> u64 {
        let next = self.queue_clear_count.get() + 1;
        self.queue_clear_count.set(next);
        next
    }

    fn log_event(&self, event: &str, details: impl Display) {
        if !self.enabled() {
            return;
        }

        let sequence = self.next_sequence();
        match self.mode {
            AudioTelemetryMode::Disabled => {}
            AudioTelemetryMode::Events | AudioTelemetryMode::Verbose => {
                eprintln!("gb-desktop audio seq={sequence} event={event} {details}");
            }
        }
    }

    fn log_submit_batch(&self, event: &str, details: impl Display) {
        if self.mode.log_submit_batches() {
            self.log_event(event, details);
        }
    }
}

impl DesktopAudioOutput {
    pub fn new(
        audio: &AudioSubsystem,
        options: &AudioOptions,
        console_model: ConsoleModel,
    ) -> Result<Self, String> {
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

        let output = Self {
            capture: ApuSampleCapture::new(options.output_sample_rate_hz)
                .map_err(format_capture_error)?,
            captured_samples: Vec::new(),
            stream,
            interleaved_buffer: Vec::new(),
            output_sample_rate_hz: options.output_sample_rate_hz,
            volume_percent: options.volume_percent,
            volume_scale: f32::from(options.volume_percent) / 100.0,
            muted: false,
            auto_queue_clear_enabled: AutoQueueClearPolicy::from_env().auto_queue_clear_enabled(),
            max_queued_bytes: i32::from(options.buffer_frames)
                * AUDIO_CHANNEL_COUNT
                * BYTES_PER_F32_SAMPLE
                * OVERSIZED_QUEUE_CLEAR_BUFFER_MULTIPLIER,
            oversized_queue_streak: 0,
            latency_queue_streak: 0,
            captured_t_cycles_since_submit: 0,
            telemetry: AudioTelemetry::from_env(),
            last_submit_telemetry: None,
            console_model,
            channel_mask: ApuRecordedChannelMask::ALL,
            masked_mix_hpf: None,
        };
        output.telemetry.log_event(
            "init",
            format!(
                "sample_rate_hz={} volume_percent={} max_queued_bytes={} auto_queue_clear_enabled={}",
                output.output_sample_rate_hz,
                output.volume_percent,
                output.max_queued_bytes,
                output.auto_queue_clear_enabled,
            ),
        );

        Ok(output)
    }

    pub fn capture_t_cycle(&mut self, apu: &Apu) {
        self.captured_t_cycles_since_submit += 1;
        if self.channel_mask.is_all() {
            self.capture.record_t_cycle(apu);
        } else {
            let tap = apu.recorded_channel_mix_tap_pre_hpf(self.channel_mask);
            let filtered = self
                .masked_mix_hpf
                .as_mut()
                .expect("subset playback must own a masked-mix HPF")
                .filter_t_cycle(tap.sample, tap.any_output_connected);
            self.capture.record_output_t_cycle(filtered);
        }
    }

    pub fn submit_captured_samples(&mut self) -> Result<(), String> {
        self.last_submit_telemetry = None;
        self.capture.drain_samples_into(&mut self.captured_samples);
        if self.captured_samples.is_empty() {
            return Ok(());
        }

        let queued_bytes = map_audio_result(
            self.stream.queued_bytes(),
            "failed to query queued SDL3 audio bytes",
        )?;
        let queued_ms_before = self.queued_duration_ms_for_bytes(queued_bytes);
        let mut cleared_queue = false;
        if queued_bytes > self.max_queued_bytes {
            if self.oversized_queue_streak < OVERSIZED_QUEUE_CLEAR_STREAK {
                self.oversized_queue_streak += 1;
            }
            if self.auto_queue_clear_enabled
                && self.oversized_queue_streak >= OVERSIZED_QUEUE_CLEAR_STREAK
            {
                self.clear_stream("oversized-queue", Some(queued_bytes))?;
                self.oversized_queue_streak = 0;
                cleared_queue = true;
            } else if !self.auto_queue_clear_enabled
                && self.oversized_queue_streak == OVERSIZED_QUEUE_CLEAR_STREAK
            {
                self.telemetry.log_event(
                    "oversized-queue-observed",
                    format!(
                        "queued_before_bytes={} queued_before_ms={} muted={} volume_percent={} auto_queue_clear_enabled={}",
                        queued_bytes,
                        format_optional_ms(queued_ms_before),
                        self.muted,
                        self.volume_percent,
                        self.auto_queue_clear_enabled,
                    ),
                );
            }
        } else {
            self.oversized_queue_streak = 0;
        }
        if !cleared_queue {
            let latency_queue_oversized =
                queued_ms_before.is_some_and(|queued_ms| queued_ms > LATENCY_QUEUE_CLEAR_MS);
            if latency_queue_oversized {
                if self.latency_queue_streak < LATENCY_QUEUE_CLEAR_STREAK {
                    self.latency_queue_streak += 1;
                }
                if self.auto_queue_clear_enabled
                    && self.latency_queue_streak >= LATENCY_QUEUE_CLEAR_STREAK
                {
                    self.clear_stream("latency-queue", Some(queued_bytes))?;
                    self.latency_queue_streak = 0;
                    cleared_queue = true;
                } else if !self.auto_queue_clear_enabled
                    && self.latency_queue_streak == LATENCY_QUEUE_CLEAR_STREAK
                {
                    self.telemetry.log_event(
                        "latency-queue-observed",
                        format!(
                            "queued_before_bytes={} queued_before_ms={} muted={} volume_percent={} auto_queue_clear_enabled={}",
                            queued_bytes,
                            format_optional_ms(queued_ms_before),
                            self.muted,
                            self.volume_percent,
                            self.auto_queue_clear_enabled,
                        ),
                    );
                }
            } else {
                self.latency_queue_streak = 0;
            }
        } else {
            self.latency_queue_streak = 0;
        }
        let sample_count = self.captured_samples.len();
        let enqueued_ms = self.sample_frames_duration_ms(sample_count);

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
        )?;
        let queued_bytes_after = self.stream.queued_bytes().ok();
        let queued_ms_after = queued_bytes_after
            .and_then(|queued_bytes| self.queued_duration_ms_for_bytes(queued_bytes));
        self.last_submit_telemetry = Some(AudioSubmitTelemetry {
            sample_count,
            captured_t_cycles: self.captured_t_cycles_since_submit,
            queued_ms_before,
            enqueued_ms,
            queued_ms_after,
        });
        self.captured_t_cycles_since_submit = 0;

        self.telemetry.log_submit_batch(
            "submit",
            format!(
                "sample_count={} queued_before_bytes={} queued_before_ms={} queued_after_bytes={} queued_after_ms={} cleared_queue={} muted={} volume_percent={}",
                self.captured_samples.len(),
                queued_bytes,
                format_optional_ms(queued_ms_before),
                format_optional_i32(queued_bytes_after),
                format_optional_ms(queued_ms_after),
                cleared_queue,
                self.muted,
                self.volume_percent,
            ),
        );

        Ok(())
    }

    pub fn pause(&self) -> Result<(), String> {
        map_audio_result(self.stream.pause(), "failed to pause SDL3 audio stream")?;
        self.telemetry.log_event(
            "pause",
            format!(
                "queued_bytes={} queued_ms={} muted={} volume_percent={}",
                format_optional_i32(self.stream.queued_bytes().ok()),
                format_optional_ms(self.queued_duration_ms()),
                self.muted,
                self.volume_percent,
            ),
        );
        Ok(())
    }

    pub fn resume(&self) -> Result<(), String> {
        map_audio_result(self.stream.resume(), "failed to resume SDL3 audio stream")?;
        self.telemetry.log_event(
            "resume",
            format!(
                "queued_bytes={} queued_ms={} muted={} volume_percent={}",
                format_optional_i32(self.stream.queued_bytes().ok()),
                format_optional_ms(self.queued_duration_ms()),
                self.muted,
                self.volume_percent,
            ),
        );
        Ok(())
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn set_muted(&mut self, muted: bool) -> Result<(), String> {
        if self.muted == muted {
            return Ok(());
        }

        self.muted = muted;
        self.telemetry.log_event(
            "mute",
            format!("muted={muted} volume_percent={}", self.volume_percent),
        );
        self.oversized_queue_streak = 0;
        self.latency_queue_streak = 0;
        self.clear_stream("mute-toggle", None)
    }

    pub fn set_volume_percent(&mut self, volume_percent: u8) -> Result<(), String> {
        let volume_percent = volume_percent.min(100);
        if self.volume_percent == volume_percent {
            return Ok(());
        }

        self.volume_percent = volume_percent;
        self.volume_scale = f32::from(volume_percent) / 100.0;
        self.telemetry.log_event(
            "volume",
            format!(
                "volume_percent={} muted={}",
                self.volume_percent, self.muted
            ),
        );
        self.oversized_queue_streak = 0;
        self.latency_queue_streak = 0;
        self.clear_stream("volume-change", None)
    }

    pub fn clear_buffer(&mut self) -> Result<(), String> {
        self.capture =
            ApuSampleCapture::new(self.output_sample_rate_hz).map_err(format_capture_error)?;
        self.captured_samples.clear();
        self.interleaved_buffer.clear();
        if let Some(masked_mix_hpf) = &mut self.masked_mix_hpf {
            masked_mix_hpf.reset();
        }
        self.oversized_queue_streak = 0;
        self.latency_queue_streak = 0;
        self.captured_t_cycles_since_submit = 0;
        self.telemetry.log_event(
            "capture-reset",
            format!(
                "sample_rate_hz={} muted={} volume_percent={} channel_mask={:#04X}",
                self.output_sample_rate_hz,
                self.muted,
                self.volume_percent,
                self.channel_mask.bits(),
            ),
        );
        self.clear_stream("capture-reset", None)
    }

    pub fn set_channel_mask(&mut self, channel_mask: ApuRecordedChannelMask) -> Result<(), String> {
        if self.channel_mask == channel_mask {
            return Ok(());
        }

        self.channel_mask = channel_mask;
        self.reset_masked_mix_hpf();
        self.telemetry.log_event(
            "channel-mask",
            format!("channel_mask={:#04X}", self.channel_mask.bits()),
        );
        self.clear_buffer()
    }

    pub fn reset_for_session_swap(&mut self, console_model: ConsoleModel) -> Result<(), String> {
        if self.console_model != console_model {
            self.console_model = console_model;
            self.reset_masked_mix_hpf();
            self.telemetry
                .log_event("console-model", format!("console_model={console_model:?}"));
        } else {
            self.telemetry.log_event(
                "session-audio-reset",
                format!(
                    "sample_rate_hz={} muted={} volume_percent={} channel_mask={:#04X}",
                    self.output_sample_rate_hz,
                    self.muted,
                    self.volume_percent,
                    self.channel_mask.bits(),
                ),
            );
        }
        self.clear_buffer()
    }

    pub fn flush(&self) -> Result<(), String> {
        map_audio_result(self.stream.flush(), "failed to flush SDL3 audio stream")
    }

    pub fn queued_duration_ms(&self) -> Option<f64> {
        let queued_bytes = self.stream.queued_bytes().ok()?;
        self.queued_duration_ms_for_bytes(queued_bytes)
    }

    pub(crate) fn take_last_submit_telemetry(&mut self) -> Option<AudioSubmitTelemetry> {
        self.last_submit_telemetry.take()
    }

    fn queued_duration_ms_for_bytes(&self, queued_bytes: i32) -> Option<f64> {
        let bytes_per_second = f64::from(self.output_sample_rate_hz)
            * f64::from(AUDIO_CHANNEL_COUNT)
            * f64::from(BYTES_PER_F32_SAMPLE);
        if bytes_per_second == 0.0 {
            return None;
        }

        Some(f64::from(queued_bytes) * 1_000.0 / bytes_per_second)
    }

    fn sample_frames_duration_ms(&self, sample_frames: usize) -> Option<f64> {
        if self.output_sample_rate_hz == 0 {
            return None;
        }

        Some(sample_frames as f64 * 1_000.0 / f64::from(self.output_sample_rate_hz))
    }

    fn reset_masked_mix_hpf(&mut self) {
        self.masked_mix_hpf = if self.channel_mask.is_all() {
            None
        } else {
            Some(ApuHostHpf::new(self.console_model))
        };
    }

    fn clear_stream(&self, reason: &str, known_queued_bytes: Option<i32>) -> Result<(), String> {
        let queued_bytes_before = known_queued_bytes.or_else(|| self.stream.queued_bytes().ok());
        map_audio_result(
            self.stream.clear(),
            "failed to clear queued SDL3 audio bytes",
        )?;
        self.telemetry.log_event(
            "stream-clear",
            format!(
                "reason={reason} clear_count={} queued_before_bytes={} queued_before_ms={} muted={} volume_percent={} oversized_queue_streak={} latency_queue_streak={}",
                self.telemetry.record_queue_clear(),
                format_optional_i32(queued_bytes_before),
                format_optional_ms(
                    queued_bytes_before.and_then(|bytes| self.queued_duration_ms_for_bytes(bytes))
                ),
                self.muted,
                self.volume_percent,
                self.oversized_queue_streak,
                self.latency_queue_streak,
            ),
        );
        Ok(())
    }
}

fn normalize_sample(sample: i32) -> f32 {
    (sample as f32 / APU_HOST_MAX_ABS_SAMPLE as f32).clamp(-1.0, 1.0)
}

fn format_optional_i32(value: Option<i32>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "unknown".to_string(),
    }
}

fn format_optional_ms(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:.3}"),
        None => "unknown".to_string(),
    }
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
mod test;
