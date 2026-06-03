use super::{
    AUDIO_CHANNEL_COUNT, AudioTelemetry, AudioTelemetryMode, AutoQueueClearPolicy,
    BYTES_PER_F32_SAMPLE, DesktopAudioOutput, OVERSIZED_QUEUE_CLEAR_STREAK, format_audio_error,
    format_capture_error, format_optional_i32, format_optional_ms, map_audio_result,
    normalize_sample,
};
use gb_core::{
    APU_HOST_MAX_ABS_SAMPLE, Apu, ApuHostSample, ApuRecordedChannel, ApuRecordedChannelMask,
    ApuSampleCaptureError, ConsoleModel,
};
use gb_desktop::AudioOptions;
use sdl3::{AudioSubsystem, hint};
use std::cell::Cell;
use std::ffi::OsStr;

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

fn queue_silence_ms(output: &DesktopAudioOutput, duration_ms: f64) {
    let sample_frames =
        (duration_ms * f64::from(output.output_sample_rate_hz) / 1_000.0).ceil() as usize;
    let interleaved_silence = vec![0.0; sample_frames * AUDIO_CHANNEL_COUNT as usize];
    output
        .stream
        .put_data_f32(&interleaved_silence)
        .expect("dummy stream should accept queued silence");
}

fn configure_constant_ch1_output(apu: &mut Apu) {
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF24, 0x77);
    apu.write_register(0xFF25, 0x11);
}

#[path = "test/capture.rs"]
mod capture;
#[path = "test/helpers.rs"]
mod helpers;
#[path = "test/queue.rs"]
mod queue;
