use super::{
    AIFC_HEADER_LEN, AUDIO_RECORDING_BYTES_PER_FRAME, AudioRecordingFormat, AudioRecordingWriter,
    DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ, DesktopAudioRecorder, DesktopAudioRecordingOptions,
    WAV_HEADER_LEN, aifc_sample_rate_bytes, audio_recording_output_directory,
    audio_recording_output_stem, channel_stem_suffix, encode_recorded_sample, format_flush_error,
    format_seek_error, resolve_next_audio_recording_output_path, stem_output_path,
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
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_RECORDING_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_recording_path(extension: &str) -> PathBuf {
    let unique = TEMP_RECORDING_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("gb-cycle-audio-recording-{unique}.{extension}"))
}

fn configure_constant_ch1_output(apu: &mut Apu) {
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF24, 0x77);
    apu.write_register(0xFF25, 0x11);
}

#[path = "test/format.rs"]
mod format;
#[path = "test/paths.rs"]
mod paths;
#[path = "test/recorder.rs"]
mod recorder;
#[path = "test/writer.rs"]
mod writer;
