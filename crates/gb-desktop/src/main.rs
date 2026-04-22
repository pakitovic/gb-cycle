mod audio;
mod audio_recording;
mod bootrom;
mod cli;
mod input;
mod linked_session;
mod menu;
mod printer_output;
mod save_session;
mod screenshot_output;
mod settings;

use audio::{AudioSubmitTelemetry, DesktopAudioOutput};
use audio_recording::{
    DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ, DesktopAudioRecorder, DesktopAudioRecordingOptions,
    resolve_next_audio_recording_output_path,
};
use bootrom::{load_boot_rom_assets, missing_boot_rom_asset_path, resolve_path};
use cli::{CliAction, DesktopRunOptions, help_text, parse_cli_arguments_with_base_config};
use gb_core::{
    ApuCh4DebugSnapshot, ApuCh4Nr43LiveWriteTrace, ApuRecordedChannel, ApuRecordedChannelMask,
    ApuRegisterWriteObservation, ApuRegisterWriteState, ApuSnapshot, CartridgeDiagnostic,
    CartridgeDiagnosticSeverity, CpuAddressEvent, CpuAddressEventKind, CpuAddressUpdateDirection,
    CpuBusAccessKind, CpuBusActivitySnapshot, CpuExecutionState, CpuSnapshot, ExecutionMode,
    InterruptControllerSnapshot, JoypadButton, JoypadSnapshot, Machine, MachineConfig,
    MachineStepObserver, MachineStepRegion, PpuAccessMode, PpuFramebufferLayerSource,
    PpuStepRegion, StartupMode, TraceSummaryBuffer,
};
use gb_desktop::{
    BootRomVerificationMode, DEFAULT_BOOT_ROM_DIR, DesktopConfig, DesktopConsoleModel,
    DesktopExternalPortSelection, DesktopKey, DesktopSaveFlushPolicy, GamepadButtonBinding,
    GamepadButtonBindings, GamepadDirectionalSource, GamepadMenuBindings, GamepadRumbleMode,
    HotkeyBindings, JoypadKeyboardBindings, KeyboardBindings, MenuKeyboardBindings,
    PreferredGamepadIdentity, SaveDirectoryPolicy, VideoOptions,
};
use gb_persistence::{
    CartridgeSaveTimeSource, SystemCartridgeSaveTimeSource,
    uses_battery_backed_hardware_persistence,
};
use input::{
    FrontendInputState, GamepadManager, gamepad_button_binding_from_sdl_button,
    sdl_button_for_binding,
};
use linked_session::DesktopEmulationSession;
use menu::{
    CompactMenuLabel, CompactRecentRomLabel, GamepadBindingTarget, GamepadMenuBindingTarget,
    KeyboardBindingTarget, KeyboardMenuBindingTarget, MenuAction, MenuInput, MenuPresentation,
    OverlayMenuState, PerformanceHudSnapshot, RECENT_ROM_MENU_CAPACITY, render_performance_hud,
};
use printer_output::PrinterOutputState;
use save_session::DesktopSaveSession;
use sdl3::dialog::{DialogError, DialogFileFilter, show_open_file_dialog, show_open_folder_dialog};
use sdl3::event::Event;
use sdl3::gamepad::Button;
use sdl3::hint;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::messagebox::{MessageBoxFlag, show_simple_message_box};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::{Canvas, ScaleMode, TextureCreator};
use sdl3::sys;
use sdl3::video::{FullscreenType, Window, WindowContext};
use settings::DesktopSettingsStore;
use std::collections::VecDeque;
use std::env;
use std::ffi::OsStr;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
#[cfg(test)]
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const FRAMEBUFFER_WIDTH: u32 = 160;
const FRAMEBUFFER_HEIGHT: u32 = 144;
#[cfg(test)]
const FRAMEBUFFER_PITCH_BYTES: usize = FRAMEBUFFER_WIDTH as usize * 3;
const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);
const AUDIO_QUEUE_TARGET_MS: f64 = 96.0;
const AUDIO_QUEUE_DEADBAND_MS: f64 = 24.0;
const AUDIO_QUEUE_PACING_GAIN: f64 = 0.10;
const AUDIO_QUEUE_MAX_CORRECTION_MS: f64 = 4.0;
const PERFORMANCE_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const EXPECTED_SCANLINE_T_CYCLES: usize = 456;
const INPUT_POLL_SLICE_T_CYCLES: usize = 256;
const DEFAULT_TRACE_CAPTURE_T_CYCLES: usize = 8_192;
const LINKED_SECONDARY_KEYBOARD_BINDINGS: [(JoypadButton, Scancode); 8] = [
    (JoypadButton::Up, Scancode::W),
    (JoypadButton::Down, Scancode::S),
    (JoypadButton::Left, Scancode::A),
    (JoypadButton::Right, Scancode::D),
    (JoypadButton::A, Scancode::V),
    (JoypadButton::B, Scancode::C),
    (JoypadButton::Select, Scancode::Q),
    (JoypadButton::Start, Scancode::E),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FramebufferDimensions {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
struct FramebufferPanelInput<'a> {
    framebuffer: &'a [u8],
    framebuffer_layer_sources: &'a [PpuFramebufferLayerSource],
    bgwin_framebuffer: &'a [u8],
    backdrop_framebuffer: &'a [u8],
    bgwin_framebuffer_layer_sources: &'a [PpuFramebufferLayerSource],
}

#[derive(Debug, Clone, Copy)]
struct FramebufferRenderInput<'a> {
    dimensions: FramebufferDimensions,
    primary: FramebufferPanelInput<'a>,
    secondary: Option<FramebufferPanelInput<'a>>,
}
const DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES: u32 = 15;
const DMG_GRAYSCALE_SHADES: [u8; 4] = [255, 170, 85, 0];
const DESKTOP_AUDIO_DISABLE_PACING_CORRECTION_ENV_VAR: &str =
    "GB_CYCLE_DESKTOP_AUDIO_DISABLE_PACING_CORRECTION";
const DESKTOP_EMU_PROFILE_ENV_VAR: &str = "GB_CYCLE_DESKTOP_EMU_PROFILE";
const DESKTOP_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_TRACE_PATH";
const DESKTOP_TRACE_T_CYCLES_ENV_VAR: &str = "GB_CYCLE_DESKTOP_TRACE_T_CYCLES";
const DESKTOP_CH4_NR43_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_CH4_NR43_TRACE_PATH";
const CH4_NR43_ADDRESS: u16 = 0xFF22;
const ROM_FILE_DIALOG_FILTERS: [DialogFileFilter<'static>; 2] = [
    DialogFileFilter {
        name: "Game Boy ROMs",
        pattern: "gb;gbc;bin",
    },
    DialogFileFilter {
        name: "All files",
        pattern: "*",
    },
];
const BOOT_ROM_FILE_DIALOG_FILTERS: [DialogFileFilter<'static>; 2] = [
    DialogFileFilter {
        name: "Boot ROM dumps",
        pattern: "bin;rom",
    },
    DialogFileFilter {
        name: "All files",
        pattern: "*",
    },
];

#[cfg(test)]
fn sdl_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
fn lock_sdl_test() -> std::sync::MutexGuard<'static, ()> {
    match sdl_test_lock().lock() {
        Ok(guard) => guard,
        Err(poison) => poison.into_inner(),
    }
}

#[cfg(test)]
fn configure_headless_sdl() {
    // SAFETY: SDL tests in this binary serialize environment mutation through `sdl_test_lock`.
    unsafe {
        env::set_var("SDL_VIDEODRIVER", "dummy");
        env::set_var("SDL_AUDIODRIVER", "dummy");
    }
    let _ = hint::set("SDL_VIDEO_DRIVER", "dummy");
    let _ = hint::set("SDL_AUDIO_DRIVER", "dummy");
    let _ = hint::set("SDL_AUDIO_DUMMY_TIMESCALE", "0");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopSignal {
    Continue,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmulationProfileSessionKind {
    Single,
    LinkedDmg04TwoPlayer,
}

impl EmulationProfileSessionKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::LinkedDmg04TwoPlayer => "linked-dmg04-2p",
        }
    }
}

enum HotkeyAction {
    None,
    ManualSave,
    Reset,
    ToggleFullscreen,
    TogglePerformanceHud,
}

struct FrontendRuntime {
    paused: bool,
    menu_state: OverlayMenuState,
    input_state: FrontendInputState,
    secondary_input_state: FrontendInputState,
    keyboard_bindings: KeyboardBindings,
    video_options: VideoOptions,
    audio_volume_percent: u8,
    audio_channel_mask: ApuRecordedChannelMask,
    audio_output: Option<DesktopAudioOutput>,
    audio_recording_mode: DesktopAudioRecordingMode,
    audio_recorder: Option<DesktopAudioRecorder>,
    gamepad_manager: Option<GamepadManager>,
    save_session: Option<DesktopSaveSession>,
    secondary_save_session: Option<DesktopSaveSession>,
    rtc_sync: HostRtcSync,
    open_rom_dialog: PathSelectionDialog,
    open_rom_dialog_mode: OpenRomDialogMode,
    boot_rom_file_dialog: PathSelectionDialog,
    boot_rom_directory_dialog: PathSelectionDialog,
    save_directory_dialog: PathSelectionDialog,
    trace_capture: DesktopTraceCapture,
    ch4_nr43_trace: DesktopCh4Nr43TraceCapture,
    printer_output: PrinterOutputState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DesktopAudioRecordingMode {
    Disabled,
    Automatic,
    Explicit(DesktopAudioRecordingOptions),
}

#[derive(Clone)]
struct DesktopSession {
    config: DesktopConfig,
    current_dir: PathBuf,
    loaded_rom: Option<LoadedRom>,
    linked_secondary_rom: Option<LoadedRom>,
    last_open_directory: Option<PathBuf>,
    recent_roms: Vec<PathBuf>,
    external_port_selection: DesktopExternalPortSelection,
}

#[derive(Clone)]
struct LoadedRom {
    path: PathBuf,
    bytes: Vec<u8>,
}

struct FrontendActionContext<'state> {
    session: &'state mut DesktopSession,
    machine: &'state mut DesktopEmulationSession,
    runtime: &'state mut FrontendRuntime,
    performance_counter: &'state mut PerformanceCounter,
    frame_pacer: &'state mut FramePacer,
    settings_store: &'state mut DesktopSettingsStore,
}

impl DesktopSession {
    fn has_loaded_rom(&self) -> bool {
        self.loaded_rom.is_some()
    }

    fn rom_path(&self) -> Option<&Path> {
        match self.loaded_rom.as_ref() {
            Some(rom) => Some(rom.path.as_path()),
            None => None,
        }
    }

    fn rom_bytes(&self) -> Option<&[u8]> {
        match self.loaded_rom.as_ref() {
            Some(rom) => Some(rom.bytes.as_slice()),
            None => None,
        }
    }

    fn linked_secondary_rom_path(&self) -> Option<&Path> {
        match self.linked_secondary_rom.as_ref() {
            Some(rom) => Some(rom.path.as_path()),
            None => None,
        }
    }

    fn linked_secondary_rom_bytes(&self) -> Option<&[u8]> {
        match self.linked_secondary_rom.as_ref() {
            Some(rom) => Some(rom.bytes.as_slice()),
            None => None,
        }
    }

    fn rom_directory_hint(&self) -> &Path {
        if let Some(rom_path) = self.rom_path()
            && let Some(parent) = rom_path.parent()
        {
            return parent;
        }
        if let Some(last_open_directory) = self.last_open_directory.as_deref() {
            return last_open_directory;
        }
        self.current_dir.as_path()
    }

    fn recent_roms(&self) -> &[PathBuf] {
        &self.recent_roms
    }
}

struct PathSelectionDialog {
    pending: bool,
    sender: Sender<PathDialogResult>,
    receiver: Receiver<PathDialogResult>,
}

struct DesktopTraceCapture {
    enabled: bool,
    output_path: Option<PathBuf>,
    max_t_cycles: usize,
    records: VecDeque<DesktopTraceRecord>,
}

struct DesktopCh4Nr43TraceCapture {
    output_path: Option<PathBuf>,
    records: Vec<DesktopCh4Nr43TraceRecord>,
}

#[derive(Debug, Clone)]
struct DesktopTraceRecord {
    t_cycle: u64,
    cpu: CpuSnapshot,
    apu: ApuSnapshot,
    interrupts: InterruptControllerSnapshot,
    joypad: JoypadSnapshot,
}

#[derive(Debug, Clone)]
struct DesktopCh4Nr43TraceRecord {
    t_cycle: u64,
    cpu: CpuSnapshot,
    apu_write: ApuRegisterWriteObservation,
    ch4: ApuCh4DebugSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum EmulationProfileMode {
    #[default]
    Disabled,
    SampledSummary {
        sample_every_frames: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct EmulationBreakdownSample {
    core_external_events_duration: Duration,
    core_timer_duration: Duration,
    core_apu_duration: Duration,
    core_dma_duration: Duration,
    core_ppu_duration: Duration,
    core_ppu_mode0_1_duration: Duration,
    core_ppu_mode2_duration: Duration,
    core_ppu_mode3_startup_duration: Duration,
    core_ppu_bg_fetch_duration: Duration,
    core_ppu_window_fetch_duration: Duration,
    core_ppu_push_duration: Duration,
    core_ppu_obj_fetch_duration: Duration,
    core_ppu_pixel_transfer_duration: Duration,
    core_serial_duration: Duration,
    core_cpu_duration: Duration,
    core_interrupts_duration: Duration,
    host_event_poll_duration: Duration,
    host_audio_submit_duration: Duration,
    host_save_flush_duration: Duration,
}

#[derive(Debug)]
struct StepUntilNextFrameResult {
    signal: LoopSignal,
    emulation_profile_request: Option<EmulationProfileRequest>,
    frame_loop_telemetry: FrameLoopTelemetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FrameLoopTelemetry {
    start_ly: u8,
    start_dot: u16,
    end_ly: u8,
    end_dot: u16,
    stepped_t_cycles: usize,
    frame_origin_crossings: u8,
    scanline_transitions: u16,
    scanlines_over_456: u16,
    max_scanline_t_cycles: usize,
    max_scanline_ly: u8,
    max_mode0_start_dot: u16,
    max_mode0_start_dot_ly: u8,
    ly_153_to_0_transitions: u8,
    ly_153_to_0_startup_mode0: u8,
    ly_153_to_0_blank_frame: u8,
    ly_0_self_wraps: u8,
    ly_0_self_wrap_startup_mode0: u8,
    ly_0_self_wrap_blank_frame: u8,
    ly_0_to_1_transitions: u8,
    ly_0_scanline_t_cycles: usize,
    ly_0_max_mode0_start_dot: u16,
    ly_0_stall_t_cycles: usize,
    ly_0_stall_hblank_t_cycles: usize,
    ly_0_stall_oam_t_cycles: usize,
    ly_0_stall_drawing_t_cycles: usize,
    ly_0_stall_startup_mode0_t_cycles: usize,
    ly_0_stall_blank_frame_t_cycles: usize,
    ly_0_stall_runs: u16,
    ly_0_max_stall_run_t_cycles: usize,
    ly_0_max_stall_dot: u16,
    ly_0_max_stall_mode_dot: u16,
    cpu_stop_t_cycles: usize,
    cpu_zombie_stop_t_cycles: usize,
    ly_0_cpu_stop_t_cycles: usize,
    ly_0_cpu_zombie_stop_t_cycles: usize,
    ly_0_stall_cpu_stop_t_cycles: usize,
    ly_0_stall_cpu_zombie_stop_t_cycles: usize,
    lcd_disabled_t_cycles: usize,
    lcd_disable_transitions: u8,
    lcd_enable_transitions: u8,
    ly_0_lcd_disabled_t_cycles: usize,
    ly_0_stall_lcd_disabled_t_cycles: usize,
}

#[derive(Debug)]
struct EmulationProfileRequest {
    machine: DesktopEmulationSession,
    breakdown: EmulationBreakdownSample,
}

#[derive(Debug)]
struct EmulationProfileWorkItem {
    machine: DesktopEmulationSession,
    emulation_duration: Duration,
    breakdown: EmulationBreakdownSample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompletedEmulationProfileSample {
    emulation_duration: Duration,
    breakdown: EmulationBreakdownSample,
}

#[derive(Debug, Default)]
struct ReplayFrameCoreProfiler {
    sample: EmulationBreakdownSample,
    active_region: Option<(MachineStepRegion, Instant)>,
    active_ppu_region: Option<(PpuStepRegion, Instant)>,
}

struct AsyncEmulationProfileWorker {
    request_sender: Option<SyncSender<EmulationProfileWorkItem>>,
    result_receiver: Receiver<CompletedEmulationProfileSample>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathDialogResult {
    Selected(PathBuf),
    Canceled,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum OpenRomDialogMode {
    #[default]
    Primary,
    LinkedSecondary,
}

impl PathSelectionDialog {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            pending: false,
            sender,
            receiver,
        }
    }

    fn is_pending(&self) -> bool {
        self.pending
    }

    fn show_file(
        &mut self,
        filters: &[DialogFileFilter<'static>],
        window: &Window,
        default_location: &Path,
    ) -> Result<(), String> {
        if self.pending {
            return Ok(());
        }

        let sender = self.sender.clone();
        map_display_result(
            show_open_file_dialog(
                filters,
                Some(default_location),
                false,
                window,
                Box::new(move |result, _| {
                    let _ = sender.send(map_path_dialog_result(result));
                }),
            ),
            "failed to show SDL3 open file dialog",
        )?;
        self.pending = true;
        Ok(())
    }

    fn show_folder(&mut self, window: &Window, default_location: &Path) {
        if self.pending {
            return;
        }

        let sender = self.sender.clone();
        show_open_folder_dialog(
            Some(default_location),
            false,
            window,
            Box::new(move |result, _| {
                let _ = sender.send(map_path_dialog_result(result));
            }),
        );
        self.pending = true;
    }

    fn take_result(&mut self) -> Option<PathDialogResult> {
        match self.receiver.try_recv() {
            Ok(result) => {
                self.pending = false;
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.pending = false;
                None
            }
        }
    }
}

impl FrontendRuntime {
    fn any_dialog_pending(&self) -> bool {
        self.open_rom_dialog.is_pending()
            || self.boot_rom_file_dialog.is_pending()
            || self.boot_rom_directory_dialog.is_pending()
            || self.save_directory_dialog.is_pending()
    }
}

impl EmulationProfileMode {
    fn from_env() -> Self {
        Self::from_env_value(env::var_os(DESKTOP_EMU_PROFILE_ENV_VAR).as_deref())
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
            || value.eq_ignore_ascii_case("disabled")
        {
            Self::Disabled
        } else {
            let normalized = value.trim().to_ascii_lowercase();
            let sample_every_frames = ["summary:", "sampled:", "every:", "stride:"]
                .iter()
                .find_map(|prefix| {
                    normalized.strip_prefix(prefix).and_then(|rest| {
                        rest.parse::<u32>()
                            .ok()
                            .filter(|sample_every| *sample_every > 0)
                    })
                })
                .unwrap_or(DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES);
            Self::SampledSummary {
                sample_every_frames,
            }
        }
    }

    fn enabled(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    fn sample_every_frames(self) -> Option<u32> {
        match self {
            Self::Disabled => None,
            Self::SampledSummary {
                sample_every_frames,
            } => Some(sample_every_frames),
        }
    }
}

impl EmulationBreakdownSample {
    fn add_core_region_duration(&mut self, region: MachineStepRegion, duration: Duration) {
        match region {
            MachineStepRegion::ExternalEvents => self.core_external_events_duration += duration,
            MachineStepRegion::Timer => self.core_timer_duration += duration,
            MachineStepRegion::Apu => self.core_apu_duration += duration,
            MachineStepRegion::Dma => self.core_dma_duration += duration,
            MachineStepRegion::Ppu => self.core_ppu_duration += duration,
            MachineStepRegion::Serial => self.core_serial_duration += duration,
            MachineStepRegion::Cpu => self.core_cpu_duration += duration,
            MachineStepRegion::Interrupts => self.core_interrupts_duration += duration,
        }
    }

    fn add_host_event_poll_duration(&mut self, duration: Duration) {
        self.host_event_poll_duration += duration;
    }

    fn add_host_audio_submit_duration(&mut self, duration: Duration) {
        self.host_audio_submit_duration += duration;
    }

    fn add_ppu_region_duration(&mut self, region: PpuStepRegion, duration: Duration) {
        match region {
            PpuStepRegion::Other => {}
            PpuStepRegion::Mode0Or1 => self.core_ppu_mode0_1_duration += duration,
            PpuStepRegion::Mode2Scan => self.core_ppu_mode2_duration += duration,
            PpuStepRegion::Mode3Startup => self.core_ppu_mode3_startup_duration += duration,
            PpuStepRegion::Mode3BgFetch => self.core_ppu_bg_fetch_duration += duration,
            PpuStepRegion::Mode3WindowFetch => self.core_ppu_window_fetch_duration += duration,
            PpuStepRegion::Mode3Push => self.core_ppu_push_duration += duration,
            PpuStepRegion::Mode3ObjFetch => self.core_ppu_obj_fetch_duration += duration,
            PpuStepRegion::Mode3PixelTransfer => {
                self.core_ppu_pixel_transfer_duration += duration;
            }
        }
    }

    fn add_host_save_flush_duration(&mut self, duration: Duration) {
        self.host_save_flush_duration += duration;
    }

    fn accumulate(&mut self, other: Self) {
        self.core_external_events_duration += other.core_external_events_duration;
        self.core_timer_duration += other.core_timer_duration;
        self.core_apu_duration += other.core_apu_duration;
        self.core_dma_duration += other.core_dma_duration;
        self.core_ppu_duration += other.core_ppu_duration;
        self.core_ppu_mode0_1_duration += other.core_ppu_mode0_1_duration;
        self.core_ppu_mode2_duration += other.core_ppu_mode2_duration;
        self.core_ppu_mode3_startup_duration += other.core_ppu_mode3_startup_duration;
        self.core_ppu_bg_fetch_duration += other.core_ppu_bg_fetch_duration;
        self.core_ppu_window_fetch_duration += other.core_ppu_window_fetch_duration;
        self.core_ppu_push_duration += other.core_ppu_push_duration;
        self.core_ppu_obj_fetch_duration += other.core_ppu_obj_fetch_duration;
        self.core_ppu_pixel_transfer_duration += other.core_ppu_pixel_transfer_duration;
        self.core_serial_duration += other.core_serial_duration;
        self.core_cpu_duration += other.core_cpu_duration;
        self.core_interrupts_duration += other.core_interrupts_duration;
        self.host_event_poll_duration += other.host_event_poll_duration;
        self.host_audio_submit_duration += other.host_audio_submit_duration;
        self.host_save_flush_duration += other.host_save_flush_duration;
    }

    fn core_duration(self) -> Duration {
        self.core_external_events_duration
            + self.core_timer_duration
            + self.core_apu_duration
            + self.core_dma_duration
            + self.core_ppu_duration
            + self.core_serial_duration
            + self.core_cpu_duration
            + self.core_interrupts_duration
    }

    fn host_duration(self) -> Duration {
        self.host_event_poll_duration
            + self.host_audio_submit_duration
            + self.host_save_flush_duration
    }

    fn core_other_duration(self) -> Duration {
        self.core_duration()
            .saturating_sub(self.core_ppu_duration + self.core_cpu_duration)
    }

    fn ppu_profiled_duration(self) -> Duration {
        self.core_ppu_mode0_1_duration
            + self.core_ppu_mode2_duration
            + self.core_ppu_mode3_startup_duration
            + self.core_ppu_bg_fetch_duration
            + self.core_ppu_window_fetch_duration
            + self.core_ppu_push_duration
            + self.core_ppu_obj_fetch_duration
            + self.core_ppu_pixel_transfer_duration
    }

    fn ppu_other_duration(self) -> Duration {
        self.core_ppu_duration
            .saturating_sub(self.ppu_profiled_duration())
    }
}

impl EmulationProfileRequest {
    fn new(machine: DesktopEmulationSession) -> Self {
        Self {
            machine,
            breakdown: EmulationBreakdownSample::default(),
        }
    }

    fn record_host_event_poll_duration(&mut self, duration: Duration) {
        self.breakdown.add_host_event_poll_duration(duration);
    }

    fn record_host_audio_submit_duration(&mut self, duration: Duration) {
        self.breakdown.add_host_audio_submit_duration(duration);
    }

    fn record_host_save_flush_duration(&mut self, duration: Duration) {
        self.breakdown.add_host_save_flush_duration(duration);
    }

    fn into_work_item(self, emulation_duration: Duration) -> EmulationProfileWorkItem {
        EmulationProfileWorkItem {
            machine: self.machine,
            emulation_duration,
            breakdown: self.breakdown,
        }
    }
}

impl ReplayFrameCoreProfiler {
    fn finish(self) -> EmulationBreakdownSample {
        debug_assert!(self.active_region.is_none());
        self.sample
    }
}

impl MachineStepObserver for ReplayFrameCoreProfiler {
    fn begin_region(&mut self, region: MachineStepRegion) {
        debug_assert!(self.active_region.is_none());
        self.active_region = Some((region, Instant::now()));
    }

    fn end_region(&mut self, region: MachineStepRegion) {
        let (active_region, started_at) = self
            .active_region
            .take()
            .expect("machine-step profiler region should have started before it ends");
        debug_assert_eq!(active_region, region);
        self.sample
            .add_core_region_duration(active_region, started_at.elapsed());
    }

    fn begin_ppu_region(&mut self, region: PpuStepRegion) {
        debug_assert!(self.active_ppu_region.is_none());
        self.active_ppu_region = Some((region, Instant::now()));
    }

    fn end_ppu_region(&mut self, region: PpuStepRegion) {
        let (active_region, started_at) = self
            .active_ppu_region
            .take()
            .expect("ppu-step profiler region should have started before it ends");
        debug_assert_eq!(active_region, region);
        self.sample
            .add_ppu_region_duration(active_region, started_at.elapsed());
    }
}

impl AsyncEmulationProfileWorker {
    fn new() -> Self {
        let (request_sender, request_receiver) = mpsc::sync_channel(1);
        let (result_sender, result_receiver) = mpsc::channel();
        let worker_handle = thread::spawn(move || {
            while let Ok(work_item) = request_receiver.recv() {
                let result = profile_emulation_work_item(work_item);
                if result_sender.send(result).is_err() {
                    break;
                }
            }
        });
        Self {
            request_sender: Some(request_sender),
            result_receiver,
            worker_handle: Some(worker_handle),
        }
    }

    fn try_submit(&self, work_item: EmulationProfileWorkItem) -> bool {
        self.request_sender
            .as_ref()
            .expect("emulation profile worker sender should exist while the worker is alive")
            .try_send(work_item)
            .is_ok()
    }

    fn collect_completed(&self, completed: &mut impl FnMut(CompletedEmulationProfileSample)) {
        while let Ok(result) = self.result_receiver.try_recv() {
            completed(result);
        }
    }
}

impl Drop for AsyncEmulationProfileWorker {
    fn drop(&mut self) {
        self.request_sender.take();
        if let Some(worker_handle) = self.worker_handle.take() {
            let _ = worker_handle.join();
        }
    }
}

fn profile_emulation_work_item(
    mut work_item: EmulationProfileWorkItem,
) -> CompletedEmulationProfileSample {
    let mut profiler = ReplayFrameCoreProfiler::default();
    let mut at_frame_origin =
        work_item.machine.ppu().ly() == 0 && work_item.machine.ppu().line_dot() == 0;

    loop {
        work_item.machine.step_t_cycle_with_observer(&mut profiler);
        let now_at_frame_origin =
            work_item.machine.ppu().ly() == 0 && work_item.machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            break;
        }
        at_frame_origin = now_at_frame_origin;
    }

    work_item.breakdown.accumulate(profiler.finish());
    CompletedEmulationProfileSample {
        emulation_duration: work_item.emulation_duration,
        breakdown: work_item.breakdown,
    }
}

impl DesktopTraceCapture {
    fn from_env() -> Result<Self, String> {
        let output_path = env::var_os(DESKTOP_TRACE_PATH_ENV_VAR).map(PathBuf::from);
        let max_t_cycles = if output_path.is_some() {
            parse_trace_capture_t_cycles(env::var_os(DESKTOP_TRACE_T_CYCLES_ENV_VAR).as_deref())?
        } else {
            DEFAULT_TRACE_CAPTURE_T_CYCLES
        };
        Ok(Self {
            enabled: output_path.is_some() && max_t_cycles > 0,
            output_path,
            max_t_cycles,
            records: VecDeque::new(),
        })
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn record_t_cycle(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        debug_assert!(self.enabled);

        if self.records.len() == self.max_t_cycles {
            self.records.pop_front();
        }
        self.records.push_back(DesktopTraceRecord {
            t_cycle: machine.next_t_cycle().get().saturating_sub(1),
            cpu: machine.cpu().snapshot(),
            apu: machine.apu().snapshot(),
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
        });
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create desktop trace artifact directory {parent:?}: {error}")
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered)
            .map_err(|error| format!("failed to write desktop trace artifact {path:?}: {error}"))
    }
}

impl DesktopCh4Nr43TraceCapture {
    fn from_env() -> Result<Self, String> {
        Ok(Self {
            output_path: env::var_os(DESKTOP_CH4_NR43_TRACE_PATH_ENV_VAR).map(PathBuf::from),
            records: Vec::new(),
        })
    }

    fn record_t_cycle(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        if self.output_path.is_none() {
            return;
        }

        let Some(apu_write) = machine
            .apu()
            .last_register_write()
            .filter(|observation| observation.address == CH4_NR43_ADDRESS)
            .cloned()
        else {
            return;
        };

        self.records.push(DesktopCh4Nr43TraceRecord {
            t_cycle: machine.next_t_cycle().get().saturating_sub(1),
            cpu: machine.cpu().snapshot(),
            apu_write,
            ch4: machine.apu().channel_4_debug_snapshot(),
        });
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create condensed CH4 NR43 trace artifact directory {parent:?}: {error}"
                )
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_ch4_nr43_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered).map_err(|error| {
            format!("failed to write condensed CH4 NR43 trace artifact {path:?}: {error}")
        })
    }
}

fn render_desktop_trace_record(record: &DesktopTraceRecord) -> String {
    format!(
        "t_cycle={} cpu.pc={:#06X} cpu.execution_state={:?} cpu.current_opcode={:?} cpu.ime={} cpu.delayed_ime_enable={} cpu.last_bus_activity={} cpu.last_address_event={} apu.powered={} apu.nr50={:#04X} apu.nr51={:#04X} apu.nr52={:#04X} apu.div_apu={} apu.active_mask={:#04X} apu.dac_mask={:#04X} apu.channel_outputs=[{:#04X},{:#04X},{:#04X},{:#04X}] apu.mixer=({}, {}) apu.hpf=({}, {}) irq.if={:#04X} irq.ie={:#04X} joypad.p1={:#04X} joypad.selection_bits={:#04X} joypad.pressed_mask={:#04X}{}",
        record.t_cycle,
        record.cpu.registers.pc,
        record.cpu.execution_state,
        record.cpu.current_opcode,
        record.cpu.ime,
        record.cpu.delayed_ime_enable,
        format_cpu_bus_activity(record.cpu.last_bus_activity),
        format_cpu_address_event(record.cpu.last_address_event),
        record.apu.powered,
        record.apu.nr50,
        record.apu.nr51,
        visible_nr52(record.apu.powered, record.apu.channel_active_mask),
        record.apu.div_apu,
        record.apu.channel_active_mask,
        record.apu.channel_dac_mask,
        record.apu.output.channel_digital_outputs[0],
        record.apu.output.channel_digital_outputs[1],
        record.apu.output.channel_digital_outputs[2],
        record.apu.output.channel_digital_outputs[3],
        record.apu.output.mixer_output.left,
        record.apu.output.mixer_output.right,
        record.apu.output.hpf_output.left,
        record.apu.output.hpf_output.right,
        record.interrupts.interrupt_flags,
        record.interrupts.interrupt_enable,
        0xC0 | record.joypad.selection_bits | visible_joypad_low_nibble(&record.joypad),
        record.joypad.selection_bits,
        record.joypad.pressed_mask,
        format_apu_last_register_write(record.apu.last_register_write.as_ref()),
    )
}

fn render_desktop_ch4_nr43_trace_record(record: &DesktopCh4Nr43TraceRecord) -> String {
    format!(
        "t_cycle={} cpu.pc={:#06X} cpu.execution_state={:?}{} {}",
        record.t_cycle,
        record.cpu.registers.pc,
        record.cpu.execution_state,
        format_apu_last_register_write(Some(&record.apu_write)),
        format_ch4_debug_snapshot(&record.ch4),
    )
}

fn format_cpu_bus_activity(activity: Option<CpuBusActivitySnapshot>) -> String {
    match activity {
        Some(activity) => format!(
            "{}@{:#06X}={:#04X}",
            match activity.kind {
                CpuBusAccessKind::OpcodeFetch => "opcode_fetch",
                CpuBusAccessKind::OperandRead => "operand_read",
                CpuBusAccessKind::DataRead => "data_read",
                CpuBusAccessKind::DataWrite => "data_write",
            },
            activity.address,
            activity.value,
        ),
        None => "none".to_string(),
    }
}

fn format_cpu_address_event(event: Option<CpuAddressEvent>) -> String {
    match event {
        Some(event) => match event.kind {
            CpuAddressEventKind::Read => match event.access_address {
                Some(address) => format!("read@{address:#06X}"),
                None => "read@missing".to_string(),
            },
            CpuAddressEventKind::Write => match event.access_address {
                Some(address) => format!("write@{address:#06X}"),
                None => "write@missing".to_string(),
            },
            CpuAddressEventKind::IncDec => match (event.idu_address, event.update_direction) {
                (Some(address), Some(direction)) => {
                    format!("{}@{address:#06X}", format_update_direction(direction))
                }
                _ => "incdec@missing".to_string(),
            },
            CpuAddressEventKind::ReadWithIncDec | CpuAddressEventKind::WriteWithIncDec => {
                match (
                    event.access_address,
                    event.idu_address,
                    event.update_direction,
                ) {
                    (Some(access), Some(idu), Some(direction)) => format!(
                        "{}+{}@{access:#06X}->{idu:#06X}",
                        match event.kind {
                            CpuAddressEventKind::ReadWithIncDec => "read",
                            CpuAddressEventKind::WriteWithIncDec => "write",
                            _ => unreachable!("combined event already constrained"),
                        },
                        format_update_direction(direction),
                    ),
                    _ => "combined@missing".to_string(),
                }
            }
        },
        None => "none".to_string(),
    }
}

fn format_update_direction(direction: CpuAddressUpdateDirection) -> &'static str {
    match direction {
        CpuAddressUpdateDirection::Increment => "inc",
        CpuAddressUpdateDirection::Decrement => "dec",
    }
}

fn format_apu_last_register_write(observation: Option<&ApuRegisterWriteObservation>) -> String {
    let Some(observation) = observation else {
        return String::new();
    };

    format!(
        " apu.last_write=write@{:#06X}={:#04X} before({}) after({})",
        observation.address,
        observation.value,
        format_apu_register_write_state(&observation.before),
        format_apu_register_write_state(&observation.after),
    )
}

fn format_apu_register_write_state(state: &ApuRegisterWriteState) -> String {
    format!(
        "nr52={:#04X} active={:#04X} dac={:#04X} outputs=[{:#04X},{:#04X},{:#04X},{:#04X}] mixer=({}, {}) hpf=({}, {})",
        state.nr52,
        state.channel_active_mask,
        state.channel_dac_mask,
        state.output.channel_digital_outputs[0],
        state.output.channel_digital_outputs[1],
        state.output.channel_digital_outputs[2],
        state.output.channel_digital_outputs[3],
        state.output.mixer_output.left,
        state.output.mixer_output.right,
        state.output.hpf_output.left,
        state.output.hpf_output.right,
    )
}

fn format_ch4_debug_snapshot(snapshot: &ApuCh4DebugSnapshot) -> String {
    format!(
        "ch4.nr43={:#04X} ch4.shift={} ch4.short_width={} ch4.divider={} ch4.alignment={} ch4.counter_timer={} ch4.noise_counter={:#06X} ch4.countdown_reloaded={} ch4.period_timer={} ch4.lfsr={:#06X} ch4.output={:#04X}{}",
        snapshot.nr43,
        snapshot.clock_shift,
        snapshot.short_width_mode,
        snapshot.clock_divider_code,
        snapshot.alignment,
        snapshot.counter_timer,
        snapshot.noise_counter,
        snapshot.countdown_reloaded,
        snapshot.period_timer,
        snapshot.lfsr_state,
        snapshot.current_digital_output,
        format_ch4_live_nr43_trace(snapshot.last_nr43_live_write.as_ref()),
    )
}

fn format_ch4_live_nr43_trace(trace: Option<&ApuCh4Nr43LiveWriteTrace>) -> String {
    let Some(trace) = trace else {
        return " ch4.last_nr43_live_write=none".to_string();
    };

    format!(
        " ch4.last_nr43_live_write=old({:#04X}/shift={}/bit={}) glitch1({:#04X}/shift={}/bit={}) glitch2({:#04X}/shift={}/bit={}) new({:#04X}/shift={}/bit={}) runtime_active={} same_shift_group={} effective_counter={:#06X} countdown_reloaded={} category={:?} action={:?} steps=[reload_seam:{},old_to_ff:{},old_to_ff_short:{},ff_to_new:{},ff_to_new_short:{},low_shift_extra:{},feedback_corruption:{}] lfsr={:#06X}->{:#06X}",
        trace.old_nr43,
        trace.old_shift,
        trace.old_bit,
        trace.glitch_value,
        trace.glitch_shift,
        trace.glitch_bit,
        trace.second_glitch_value,
        trace.second_glitch_shift,
        trace.second_glitch_bit,
        trace.new_nr43,
        trace.new_shift,
        trace.new_bit,
        trace.runtime_active,
        trace.same_shift_group,
        trace.effective_counter,
        trace.countdown_reloaded,
        trace.decision_category,
        trace.lfsr_action,
        trace.reload_seam_step,
        trace.old_to_ff_step,
        trace.old_to_ff_forced_short_width,
        trace.ff_to_new_step,
        trace.ff_to_new_forced_short_width,
        trace.low_shift_extra_step,
        trace.feedback_corruption,
        trace.lfsr_before,
        trace.lfsr_after,
    )
}

fn visible_nr52(powered: bool, active_mask: u8) -> u8 {
    0x70 | if powered {
        0x80 | (active_mask & 0x0F)
    } else {
        0
    }
}

fn visible_joypad_low_nibble(snapshot: &JoypadSnapshot) -> u8 {
    let dpad_selected = snapshot.selection_bits & 0x10 == 0;
    let buttons_selected = snapshot.selection_bits & 0x20 == 0;
    let mut low = 0x0F;
    if dpad_selected {
        if snapshot.pressed_mask & 0x01 != 0 {
            low &= !0x01;
        }
        if snapshot.pressed_mask & 0x02 != 0 {
            low &= !0x02;
        }
        if snapshot.pressed_mask & 0x04 != 0 {
            low &= !0x04;
        }
        if snapshot.pressed_mask & 0x08 != 0 {
            low &= !0x08;
        }
    }
    if buttons_selected {
        if snapshot.pressed_mask & 0x10 != 0 {
            low &= !0x01;
        }
        if snapshot.pressed_mask & 0x20 != 0 {
            low &= !0x02;
        }
        if snapshot.pressed_mask & 0x40 != 0 {
            low &= !0x04;
        }
        if snapshot.pressed_mask & 0x80 != 0 {
            low &= !0x08;
        }
    }
    low
}

fn parse_trace_capture_t_cycles(value: Option<&std::ffi::OsStr>) -> Result<usize, String> {
    let Some(value) = value else {
        return Ok(DEFAULT_TRACE_CAPTURE_T_CYCLES);
    };

    let text = value.to_string_lossy();
    let parsed = text.parse::<usize>().map_err(|error| {
        format!(
            "{DESKTOP_TRACE_T_CYCLES_ENV_VAR} must be a positive integer T-cycle count: {error}"
        )
    })?;
    if parsed == 0 {
        return Err(format!(
            "{DESKTOP_TRACE_T_CYCLES_ENV_VAR} must be greater than zero"
        ));
    }
    Ok(parsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AudioQueuePacingCorrectionPolicy {
    #[default]
    Enabled,
    Disabled,
}

impl AudioQueuePacingCorrectionPolicy {
    fn from_env() -> Self {
        Self::from_env_value(
            env::var_os(DESKTOP_AUDIO_DISABLE_PACING_CORRECTION_ENV_VAR).as_deref(),
        )
    }

    fn from_env_value(value: Option<&OsStr>) -> Self {
        let Some(value) = value else {
            return Self::Enabled;
        };

        let value = value.to_string_lossy();
        if value.is_empty()
            || value == "1"
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

    fn correction_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

struct FramePacer {
    next_frame_start: Instant,
    audio_queue_pacing_correction_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FramePacingSample {
    pacing_duration: Duration,
    sleep_target_duration: Duration,
    audio_correction_duration: Duration,
    late_duration: Duration,
    oversleep_duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostRtcSync {
    last_host_unix_seconds: u64,
}

impl HostRtcSync {
    fn new(last_host_unix_seconds: u64) -> Self {
        Self {
            last_host_unix_seconds,
        }
    }

    fn from_host_clock() -> Self {
        Self::new(Self::current_unix_seconds())
    }

    fn resync_to_host_clock(&mut self) {
        self.last_host_unix_seconds = Self::current_unix_seconds();
    }

    fn apply_to_machine(&mut self, machine: &mut Machine<TraceSummaryBuffer>) {
        self.apply_with_now(machine, Self::current_unix_seconds());
    }

    fn apply_with_now(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        current_host_unix_seconds: u64,
    ) {
        if current_host_unix_seconds <= self.last_host_unix_seconds {
            return;
        }

        machine
            .advance_cartridge_rtc_seconds(current_host_unix_seconds - self.last_host_unix_seconds);
        self.last_host_unix_seconds = current_host_unix_seconds;
    }

    fn current_unix_seconds() -> u64 {
        SystemCartridgeSaveTimeSource.now_unix_seconds()
    }
}

impl FramePacer {
    fn new(_vsync_enabled: bool) -> Self {
        Self {
            next_frame_start: Instant::now(),
            audio_queue_pacing_correction_enabled: AudioQueuePacingCorrectionPolicy::from_env()
                .correction_enabled(),
        }
    }

    fn wait_until_next_frame(&mut self, audio_queue_ms: Option<f64>) -> FramePacingSample {
        let audio_correction = audio_queue_pacing_correction_with_policy(
            audio_queue_ms,
            self.audio_queue_pacing_correction_enabled,
        );
        self.next_frame_start += FRAME_DURATION + audio_correction;
        let now = Instant::now();
        if now < self.next_frame_start {
            let sleep_target_duration = self.next_frame_start - now;
            thread::sleep(sleep_target_duration);
            let oversleep_duration =
                Instant::now().saturating_duration_since(self.next_frame_start);
            FramePacingSample {
                pacing_duration: sleep_target_duration,
                sleep_target_duration,
                audio_correction_duration: audio_correction,
                late_duration: Duration::ZERO,
                oversleep_duration,
            }
        } else {
            let late_duration = now - self.next_frame_start;
            self.next_frame_start = now;
            FramePacingSample {
                pacing_duration: Duration::ZERO,
                sleep_target_duration: Duration::ZERO,
                audio_correction_duration: audio_correction,
                late_duration,
                oversleep_duration: Duration::ZERO,
            }
        }
    }

    fn set_vsync_enabled(&mut self, _vsync_enabled: bool) {
        self.next_frame_start = Instant::now();
    }
}

fn audio_queue_pacing_correction_with_policy(
    audio_queue_ms: Option<f64>,
    correction_enabled: bool,
) -> Duration {
    if !correction_enabled {
        return Duration::ZERO;
    }

    let Some(audio_queue_ms) = audio_queue_ms else {
        return Duration::ZERO;
    };

    let excess_ms = audio_queue_ms - (AUDIO_QUEUE_TARGET_MS + AUDIO_QUEUE_DEADBAND_MS);
    if excess_ms <= 0.0 {
        return Duration::ZERO;
    }

    let correction_ms = (excess_ms * AUDIO_QUEUE_PACING_GAIN).min(AUDIO_QUEUE_MAX_CORRECTION_MS);
    Duration::from_secs_f64(correction_ms / 1_000.0)
}

#[derive(Debug)]
struct FramePerformanceSample {
    session_kind: EmulationProfileSessionKind,
    emulation_duration: Duration,
    emulation_profile_request: Option<EmulationProfileRequest>,
    render_duration: Duration,
    present_duration: Duration,
    pacing_duration: Duration,
    pacing_sleep_target_duration: Duration,
    pacing_audio_correction_duration: Duration,
    pacing_late_duration: Duration,
    pacing_oversleep_duration: Duration,
    audio_submit_sample_count: Option<usize>,
    audio_submit_t_cycles: Option<usize>,
    audio_submit_queue_before_ms: Option<f64>,
    audio_submit_enqueued_ms: Option<f64>,
    audio_submit_queue_after_ms: Option<f64>,
    audio_queue_before_pacing_ms: Option<f64>,
    audio_queue_after_pacing_ms: Option<f64>,
    frame_step_t_cycles: Option<usize>,
    frame_start_ly: Option<u8>,
    frame_start_dot: Option<u16>,
    frame_end_ly: Option<u8>,
    frame_end_dot: Option<u16>,
    frame_origin_crossings: Option<u8>,
    scanline_transitions: Option<u16>,
    scanlines_over_456: Option<u16>,
    max_scanline_t_cycles: Option<usize>,
    max_scanline_ly: Option<u8>,
    max_mode0_start_dot: Option<u16>,
    max_mode0_start_dot_ly: Option<u8>,
    ly_153_to_0_transitions: Option<u8>,
    ly_153_to_0_startup_mode0: Option<u8>,
    ly_153_to_0_blank_frame: Option<u8>,
    ly_0_self_wraps: Option<u8>,
    ly_0_self_wrap_startup_mode0: Option<u8>,
    ly_0_self_wrap_blank_frame: Option<u8>,
    ly_0_to_1_transitions: Option<u8>,
    ly_0_scanline_t_cycles: Option<usize>,
    ly_0_max_mode0_start_dot: Option<u16>,
    ly_0_stall_t_cycles: Option<usize>,
    ly_0_stall_hblank_t_cycles: Option<usize>,
    ly_0_stall_oam_t_cycles: Option<usize>,
    ly_0_stall_drawing_t_cycles: Option<usize>,
    ly_0_stall_startup_mode0_t_cycles: Option<usize>,
    ly_0_stall_blank_frame_t_cycles: Option<usize>,
    ly_0_stall_runs: Option<u16>,
    ly_0_max_stall_run_t_cycles: Option<usize>,
    ly_0_max_stall_dot: Option<u16>,
    ly_0_max_stall_mode_dot: Option<u16>,
    cpu_stop_t_cycles: Option<usize>,
    cpu_zombie_stop_t_cycles: Option<usize>,
    ly_0_cpu_stop_t_cycles: Option<usize>,
    ly_0_cpu_zombie_stop_t_cycles: Option<usize>,
    ly_0_stall_cpu_stop_t_cycles: Option<usize>,
    ly_0_stall_cpu_zombie_stop_t_cycles: Option<usize>,
    lcd_disabled_t_cycles: Option<usize>,
    lcd_disable_transitions: Option<u8>,
    lcd_enable_transitions: Option<u8>,
    ly_0_lcd_disabled_t_cycles: Option<usize>,
    ly_0_stall_lcd_disabled_t_cycles: Option<usize>,
}

struct PerformanceCounter {
    base_title: String,
    emulation_profile_mode: EmulationProfileMode,
    emulation_profile_worker: Option<AsyncEmulationProfileWorker>,
    emulation_profile_request_in_flight: bool,
    sample_session_kind: EmulationProfileSessionKind,
    presented_frames_total: u64,
    sample_started_at: Instant,
    frames_in_sample: u32,
    sample_emulation_duration: Duration,
    sample_profiled_frames: u32,
    sample_profiled_emulation_duration: Duration,
    sample_profiled_emulation_breakdown: EmulationBreakdownSample,
    sample_render_duration: Duration,
    sample_present_duration: Duration,
    sample_pacing_duration: Duration,
    sample_pacing_sleep_target_duration: Duration,
    sample_pacing_audio_correction_duration: Duration,
    sample_pacing_late_duration: Duration,
    sample_pacing_oversleep_duration: Duration,
    sample_audio_submit_sample_count: u64,
    sample_audio_submit_sample_count_observations: u32,
    sample_audio_submit_t_cycles: u64,
    sample_audio_submit_t_cycles_observations: u32,
    sample_audio_submit_queue_before_ms: f64,
    sample_audio_submit_queue_before_observations: u32,
    sample_audio_submit_enqueued_ms: f64,
    sample_audio_submit_enqueued_observations: u32,
    sample_audio_submit_queue_after_ms: f64,
    sample_audio_submit_queue_after_observations: u32,
    sample_audio_queue_before_pacing_ms: f64,
    sample_audio_queue_before_pacing_observations: u32,
    sample_audio_queue_after_pacing_ms: f64,
    sample_audio_queue_after_pacing_observations: u32,
    sample_frame_step_t_cycles: u64,
    sample_frame_step_t_cycles_observations: u32,
    sample_frame_start_ly: u64,
    sample_frame_start_ly_observations: u32,
    sample_frame_start_dot: u64,
    sample_frame_start_dot_observations: u32,
    sample_frame_end_ly: u64,
    sample_frame_end_ly_observations: u32,
    sample_frame_end_dot: u64,
    sample_frame_end_dot_observations: u32,
    sample_frame_origin_crossings: u64,
    sample_frame_origin_crossings_observations: u32,
    sample_scanline_transitions: u64,
    sample_scanline_transitions_observations: u32,
    sample_scanlines_over_456: u64,
    sample_scanlines_over_456_observations: u32,
    sample_max_scanline_t_cycles: u64,
    sample_max_scanline_t_cycles_observations: u32,
    sample_max_scanline_ly: u64,
    sample_max_scanline_ly_observations: u32,
    sample_max_mode0_start_dot: u64,
    sample_max_mode0_start_dot_observations: u32,
    sample_max_mode0_start_dot_ly: u64,
    sample_max_mode0_start_dot_ly_observations: u32,
    sample_ly_153_to_0_transitions: u64,
    sample_ly_153_to_0_transitions_observations: u32,
    sample_ly_153_to_0_startup_mode0: u64,
    sample_ly_153_to_0_startup_mode0_observations: u32,
    sample_ly_153_to_0_blank_frame: u64,
    sample_ly_153_to_0_blank_frame_observations: u32,
    sample_ly_0_self_wraps: u64,
    sample_ly_0_self_wraps_observations: u32,
    sample_ly_0_self_wrap_startup_mode0: u64,
    sample_ly_0_self_wrap_startup_mode0_observations: u32,
    sample_ly_0_self_wrap_blank_frame: u64,
    sample_ly_0_self_wrap_blank_frame_observations: u32,
    sample_ly_0_to_1_transitions: u64,
    sample_ly_0_to_1_transitions_observations: u32,
    sample_ly_0_scanline_t_cycles: u64,
    sample_ly_0_scanline_t_cycles_observations: u32,
    sample_ly_0_max_mode0_start_dot: u64,
    sample_ly_0_max_mode0_start_dot_observations: u32,
    sample_ly_0_stall_t_cycles: u64,
    sample_ly_0_stall_t_cycles_observations: u32,
    sample_ly_0_stall_hblank_t_cycles: u64,
    sample_ly_0_stall_hblank_t_cycles_observations: u32,
    sample_ly_0_stall_oam_t_cycles: u64,
    sample_ly_0_stall_oam_t_cycles_observations: u32,
    sample_ly_0_stall_drawing_t_cycles: u64,
    sample_ly_0_stall_drawing_t_cycles_observations: u32,
    sample_ly_0_stall_startup_mode0_t_cycles: u64,
    sample_ly_0_stall_startup_mode0_t_cycles_observations: u32,
    sample_ly_0_stall_blank_frame_t_cycles: u64,
    sample_ly_0_stall_blank_frame_t_cycles_observations: u32,
    sample_ly_0_stall_runs: u64,
    sample_ly_0_stall_runs_observations: u32,
    sample_ly_0_max_stall_run_t_cycles: u64,
    sample_ly_0_max_stall_run_t_cycles_observations: u32,
    sample_ly_0_max_stall_dot: u64,
    sample_ly_0_max_stall_dot_observations: u32,
    sample_ly_0_max_stall_mode_dot: u64,
    sample_ly_0_max_stall_mode_dot_observations: u32,
    sample_cpu_stop_t_cycles: u64,
    sample_cpu_stop_t_cycles_observations: u32,
    sample_cpu_zombie_stop_t_cycles: u64,
    sample_cpu_zombie_stop_t_cycles_observations: u32,
    sample_ly_0_cpu_stop_t_cycles: u64,
    sample_ly_0_cpu_stop_t_cycles_observations: u32,
    sample_ly_0_cpu_zombie_stop_t_cycles: u64,
    sample_ly_0_cpu_zombie_stop_t_cycles_observations: u32,
    sample_ly_0_stall_cpu_stop_t_cycles: u64,
    sample_ly_0_stall_cpu_stop_t_cycles_observations: u32,
    sample_ly_0_stall_cpu_zombie_stop_t_cycles: u64,
    sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations: u32,
    sample_lcd_disabled_t_cycles: u64,
    sample_lcd_disabled_t_cycles_observations: u32,
    sample_lcd_disable_transitions: u64,
    sample_lcd_disable_transitions_observations: u32,
    sample_lcd_enable_transitions: u64,
    sample_lcd_enable_transitions_observations: u32,
    sample_ly_0_lcd_disabled_t_cycles: u64,
    sample_ly_0_lcd_disabled_t_cycles_observations: u32,
    sample_ly_0_stall_lcd_disabled_t_cycles: u64,
    sample_ly_0_stall_lcd_disabled_t_cycles_observations: u32,
    hud_snapshot: Option<PerformanceHudSnapshot>,
}

impl PerformanceCounter {
    fn new(base_title: String) -> Self {
        Self::new_with_emulation_profile_mode(base_title, EmulationProfileMode::from_env())
    }

    fn new_with_emulation_profile_mode(
        base_title: String,
        emulation_profile_mode: EmulationProfileMode,
    ) -> Self {
        Self {
            base_title,
            emulation_profile_mode,
            emulation_profile_worker: emulation_profile_mode
                .enabled()
                .then(AsyncEmulationProfileWorker::new),
            emulation_profile_request_in_flight: false,
            sample_session_kind: EmulationProfileSessionKind::Single,
            presented_frames_total: 0,
            sample_started_at: Instant::now(),
            frames_in_sample: 0,
            sample_emulation_duration: Duration::ZERO,
            sample_profiled_frames: 0,
            sample_profiled_emulation_duration: Duration::ZERO,
            sample_profiled_emulation_breakdown: EmulationBreakdownSample::default(),
            sample_render_duration: Duration::ZERO,
            sample_present_duration: Duration::ZERO,
            sample_pacing_duration: Duration::ZERO,
            sample_pacing_sleep_target_duration: Duration::ZERO,
            sample_pacing_audio_correction_duration: Duration::ZERO,
            sample_pacing_late_duration: Duration::ZERO,
            sample_pacing_oversleep_duration: Duration::ZERO,
            sample_audio_submit_sample_count: 0,
            sample_audio_submit_sample_count_observations: 0,
            sample_audio_submit_t_cycles: 0,
            sample_audio_submit_t_cycles_observations: 0,
            sample_audio_submit_queue_before_ms: 0.0,
            sample_audio_submit_queue_before_observations: 0,
            sample_audio_submit_enqueued_ms: 0.0,
            sample_audio_submit_enqueued_observations: 0,
            sample_audio_submit_queue_after_ms: 0.0,
            sample_audio_submit_queue_after_observations: 0,
            sample_audio_queue_before_pacing_ms: 0.0,
            sample_audio_queue_before_pacing_observations: 0,
            sample_audio_queue_after_pacing_ms: 0.0,
            sample_audio_queue_after_pacing_observations: 0,
            sample_frame_step_t_cycles: 0,
            sample_frame_step_t_cycles_observations: 0,
            sample_frame_start_ly: 0,
            sample_frame_start_ly_observations: 0,
            sample_frame_start_dot: 0,
            sample_frame_start_dot_observations: 0,
            sample_frame_end_ly: 0,
            sample_frame_end_ly_observations: 0,
            sample_frame_end_dot: 0,
            sample_frame_end_dot_observations: 0,
            sample_frame_origin_crossings: 0,
            sample_frame_origin_crossings_observations: 0,
            sample_scanline_transitions: 0,
            sample_scanline_transitions_observations: 0,
            sample_scanlines_over_456: 0,
            sample_scanlines_over_456_observations: 0,
            sample_max_scanline_t_cycles: 0,
            sample_max_scanline_t_cycles_observations: 0,
            sample_max_scanline_ly: 0,
            sample_max_scanline_ly_observations: 0,
            sample_max_mode0_start_dot: 0,
            sample_max_mode0_start_dot_observations: 0,
            sample_max_mode0_start_dot_ly: 0,
            sample_max_mode0_start_dot_ly_observations: 0,
            sample_ly_153_to_0_transitions: 0,
            sample_ly_153_to_0_transitions_observations: 0,
            sample_ly_153_to_0_startup_mode0: 0,
            sample_ly_153_to_0_startup_mode0_observations: 0,
            sample_ly_153_to_0_blank_frame: 0,
            sample_ly_153_to_0_blank_frame_observations: 0,
            sample_ly_0_self_wraps: 0,
            sample_ly_0_self_wraps_observations: 0,
            sample_ly_0_self_wrap_startup_mode0: 0,
            sample_ly_0_self_wrap_startup_mode0_observations: 0,
            sample_ly_0_self_wrap_blank_frame: 0,
            sample_ly_0_self_wrap_blank_frame_observations: 0,
            sample_ly_0_to_1_transitions: 0,
            sample_ly_0_to_1_transitions_observations: 0,
            sample_ly_0_scanline_t_cycles: 0,
            sample_ly_0_scanline_t_cycles_observations: 0,
            sample_ly_0_max_mode0_start_dot: 0,
            sample_ly_0_max_mode0_start_dot_observations: 0,
            sample_ly_0_stall_t_cycles: 0,
            sample_ly_0_stall_t_cycles_observations: 0,
            sample_ly_0_stall_hblank_t_cycles: 0,
            sample_ly_0_stall_hblank_t_cycles_observations: 0,
            sample_ly_0_stall_oam_t_cycles: 0,
            sample_ly_0_stall_oam_t_cycles_observations: 0,
            sample_ly_0_stall_drawing_t_cycles: 0,
            sample_ly_0_stall_drawing_t_cycles_observations: 0,
            sample_ly_0_stall_startup_mode0_t_cycles: 0,
            sample_ly_0_stall_startup_mode0_t_cycles_observations: 0,
            sample_ly_0_stall_blank_frame_t_cycles: 0,
            sample_ly_0_stall_blank_frame_t_cycles_observations: 0,
            sample_ly_0_stall_runs: 0,
            sample_ly_0_stall_runs_observations: 0,
            sample_ly_0_max_stall_run_t_cycles: 0,
            sample_ly_0_max_stall_run_t_cycles_observations: 0,
            sample_ly_0_max_stall_dot: 0,
            sample_ly_0_max_stall_dot_observations: 0,
            sample_ly_0_max_stall_mode_dot: 0,
            sample_ly_0_max_stall_mode_dot_observations: 0,
            sample_cpu_stop_t_cycles: 0,
            sample_cpu_stop_t_cycles_observations: 0,
            sample_cpu_zombie_stop_t_cycles: 0,
            sample_cpu_zombie_stop_t_cycles_observations: 0,
            sample_ly_0_cpu_stop_t_cycles: 0,
            sample_ly_0_cpu_stop_t_cycles_observations: 0,
            sample_ly_0_cpu_zombie_stop_t_cycles: 0,
            sample_ly_0_cpu_zombie_stop_t_cycles_observations: 0,
            sample_ly_0_stall_cpu_stop_t_cycles: 0,
            sample_ly_0_stall_cpu_stop_t_cycles_observations: 0,
            sample_ly_0_stall_cpu_zombie_stop_t_cycles: 0,
            sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations: 0,
            sample_lcd_disabled_t_cycles: 0,
            sample_lcd_disabled_t_cycles_observations: 0,
            sample_lcd_disable_transitions: 0,
            sample_lcd_disable_transitions_observations: 0,
            sample_lcd_enable_transitions: 0,
            sample_lcd_enable_transitions_observations: 0,
            sample_ly_0_lcd_disabled_t_cycles: 0,
            sample_ly_0_lcd_disabled_t_cycles_observations: 0,
            sample_ly_0_stall_lcd_disabled_t_cycles: 0,
            sample_ly_0_stall_lcd_disabled_t_cycles_observations: 0,
            hud_snapshot: None,
        }
    }

    fn record_presented_frame(
        &mut self,
        window: &mut Window,
        sample: FramePerformanceSample,
    ) -> Result<(), String> {
        self.presented_frames_total = self.presented_frames_total.saturating_add(1);
        self.collect_emulation_profile_results();
        self.submit_emulation_profile_request(
            sample.emulation_profile_request,
            sample.emulation_duration,
        );

        self.sample_session_kind = sample.session_kind;
        self.frames_in_sample += 1;
        self.sample_emulation_duration += sample.emulation_duration;
        self.sample_render_duration += sample.render_duration;
        self.sample_present_duration += sample.present_duration;
        self.sample_pacing_duration += sample.pacing_duration;
        self.sample_pacing_sleep_target_duration += sample.pacing_sleep_target_duration;
        self.sample_pacing_audio_correction_duration += sample.pacing_audio_correction_duration;
        self.sample_pacing_late_duration += sample.pacing_late_duration;
        self.sample_pacing_oversleep_duration += sample.pacing_oversleep_duration;
        if let Some(sample_count) = sample.audio_submit_sample_count {
            self.sample_audio_submit_sample_count += sample_count as u64;
            self.sample_audio_submit_sample_count_observations += 1;
        }
        if let Some(t_cycles) = sample.audio_submit_t_cycles {
            self.sample_audio_submit_t_cycles += t_cycles as u64;
            self.sample_audio_submit_t_cycles_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_submit_queue_before_ms {
            self.sample_audio_submit_queue_before_ms += audio_queue_ms;
            self.sample_audio_submit_queue_before_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_submit_enqueued_ms {
            self.sample_audio_submit_enqueued_ms += audio_queue_ms;
            self.sample_audio_submit_enqueued_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_submit_queue_after_ms {
            self.sample_audio_submit_queue_after_ms += audio_queue_ms;
            self.sample_audio_submit_queue_after_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_queue_before_pacing_ms {
            self.sample_audio_queue_before_pacing_ms += audio_queue_ms;
            self.sample_audio_queue_before_pacing_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_queue_after_pacing_ms {
            self.sample_audio_queue_after_pacing_ms += audio_queue_ms;
            self.sample_audio_queue_after_pacing_observations += 1;
        }
        if let Some(t_cycles) = sample.frame_step_t_cycles {
            self.sample_frame_step_t_cycles += t_cycles as u64;
            self.sample_frame_step_t_cycles_observations += 1;
        }
        if let Some(start_ly) = sample.frame_start_ly {
            self.sample_frame_start_ly += u64::from(start_ly);
            self.sample_frame_start_ly_observations += 1;
        }
        if let Some(start_dot) = sample.frame_start_dot {
            self.sample_frame_start_dot += u64::from(start_dot);
            self.sample_frame_start_dot_observations += 1;
        }
        if let Some(end_ly) = sample.frame_end_ly {
            self.sample_frame_end_ly += u64::from(end_ly);
            self.sample_frame_end_ly_observations += 1;
        }
        if let Some(end_dot) = sample.frame_end_dot {
            self.sample_frame_end_dot += u64::from(end_dot);
            self.sample_frame_end_dot_observations += 1;
        }
        if let Some(frame_origin_crossings) = sample.frame_origin_crossings {
            self.sample_frame_origin_crossings += u64::from(frame_origin_crossings);
            self.sample_frame_origin_crossings_observations += 1;
        }
        if let Some(scanline_transitions) = sample.scanline_transitions {
            self.sample_scanline_transitions += u64::from(scanline_transitions);
            self.sample_scanline_transitions_observations += 1;
        }
        if let Some(scanlines_over_456) = sample.scanlines_over_456 {
            self.sample_scanlines_over_456 += u64::from(scanlines_over_456);
            self.sample_scanlines_over_456_observations += 1;
        }
        if let Some(max_scanline_t_cycles) = sample.max_scanline_t_cycles {
            self.sample_max_scanline_t_cycles += max_scanline_t_cycles as u64;
            self.sample_max_scanline_t_cycles_observations += 1;
        }
        if let Some(max_scanline_ly) = sample.max_scanline_ly {
            self.sample_max_scanline_ly += u64::from(max_scanline_ly);
            self.sample_max_scanline_ly_observations += 1;
        }
        if let Some(max_mode0_start_dot) = sample.max_mode0_start_dot {
            self.sample_max_mode0_start_dot += u64::from(max_mode0_start_dot);
            self.sample_max_mode0_start_dot_observations += 1;
        }
        if let Some(max_mode0_start_dot_ly) = sample.max_mode0_start_dot_ly {
            self.sample_max_mode0_start_dot_ly += u64::from(max_mode0_start_dot_ly);
            self.sample_max_mode0_start_dot_ly_observations += 1;
        }
        if let Some(transitions) = sample.ly_153_to_0_transitions {
            self.sample_ly_153_to_0_transitions += u64::from(transitions);
            self.sample_ly_153_to_0_transitions_observations += 1;
        }
        if let Some(transitions) = sample.ly_153_to_0_startup_mode0 {
            self.sample_ly_153_to_0_startup_mode0 += u64::from(transitions);
            self.sample_ly_153_to_0_startup_mode0_observations += 1;
        }
        if let Some(transitions) = sample.ly_153_to_0_blank_frame {
            self.sample_ly_153_to_0_blank_frame += u64::from(transitions);
            self.sample_ly_153_to_0_blank_frame_observations += 1;
        }
        if let Some(wraps) = sample.ly_0_self_wraps {
            self.sample_ly_0_self_wraps += u64::from(wraps);
            self.sample_ly_0_self_wraps_observations += 1;
        }
        if let Some(wraps) = sample.ly_0_self_wrap_startup_mode0 {
            self.sample_ly_0_self_wrap_startup_mode0 += u64::from(wraps);
            self.sample_ly_0_self_wrap_startup_mode0_observations += 1;
        }
        if let Some(wraps) = sample.ly_0_self_wrap_blank_frame {
            self.sample_ly_0_self_wrap_blank_frame += u64::from(wraps);
            self.sample_ly_0_self_wrap_blank_frame_observations += 1;
        }
        if let Some(transitions) = sample.ly_0_to_1_transitions {
            self.sample_ly_0_to_1_transitions += u64::from(transitions);
            self.sample_ly_0_to_1_transitions_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_scanline_t_cycles {
            self.sample_ly_0_scanline_t_cycles += t_cycles as u64;
            self.sample_ly_0_scanline_t_cycles_observations += 1;
        }
        if let Some(mode0_start_dot) = sample.ly_0_max_mode0_start_dot {
            self.sample_ly_0_max_mode0_start_dot += u64::from(mode0_start_dot);
            self.sample_ly_0_max_mode0_start_dot_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_t_cycles {
            self.sample_ly_0_stall_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_hblank_t_cycles {
            self.sample_ly_0_stall_hblank_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_hblank_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_oam_t_cycles {
            self.sample_ly_0_stall_oam_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_oam_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_drawing_t_cycles {
            self.sample_ly_0_stall_drawing_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_drawing_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_startup_mode0_t_cycles {
            self.sample_ly_0_stall_startup_mode0_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_startup_mode0_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_blank_frame_t_cycles {
            self.sample_ly_0_stall_blank_frame_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_blank_frame_t_cycles_observations += 1;
        }
        if let Some(stall_runs) = sample.ly_0_stall_runs {
            self.sample_ly_0_stall_runs += u64::from(stall_runs);
            self.sample_ly_0_stall_runs_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_max_stall_run_t_cycles {
            self.sample_ly_0_max_stall_run_t_cycles += t_cycles as u64;
            self.sample_ly_0_max_stall_run_t_cycles_observations += 1;
        }
        if let Some(dot) = sample.ly_0_max_stall_dot {
            self.sample_ly_0_max_stall_dot += u64::from(dot);
            self.sample_ly_0_max_stall_dot_observations += 1;
        }
        if let Some(mode_dot) = sample.ly_0_max_stall_mode_dot {
            self.sample_ly_0_max_stall_mode_dot += u64::from(mode_dot);
            self.sample_ly_0_max_stall_mode_dot_observations += 1;
        }
        if let Some(t_cycles) = sample.cpu_stop_t_cycles {
            self.sample_cpu_stop_t_cycles += t_cycles as u64;
            self.sample_cpu_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.cpu_zombie_stop_t_cycles {
            self.sample_cpu_zombie_stop_t_cycles += t_cycles as u64;
            self.sample_cpu_zombie_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_cpu_stop_t_cycles {
            self.sample_ly_0_cpu_stop_t_cycles += t_cycles as u64;
            self.sample_ly_0_cpu_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_cpu_zombie_stop_t_cycles {
            self.sample_ly_0_cpu_zombie_stop_t_cycles += t_cycles as u64;
            self.sample_ly_0_cpu_zombie_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_cpu_stop_t_cycles {
            self.sample_ly_0_stall_cpu_stop_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_cpu_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_cpu_zombie_stop_t_cycles {
            self.sample_ly_0_stall_cpu_zombie_stop_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.lcd_disabled_t_cycles {
            self.sample_lcd_disabled_t_cycles += t_cycles as u64;
            self.sample_lcd_disabled_t_cycles_observations += 1;
        }
        if let Some(transitions) = sample.lcd_disable_transitions {
            self.sample_lcd_disable_transitions += u64::from(transitions);
            self.sample_lcd_disable_transitions_observations += 1;
        }
        if let Some(transitions) = sample.lcd_enable_transitions {
            self.sample_lcd_enable_transitions += u64::from(transitions);
            self.sample_lcd_enable_transitions_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_lcd_disabled_t_cycles {
            self.sample_ly_0_lcd_disabled_t_cycles += t_cycles as u64;
            self.sample_ly_0_lcd_disabled_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_lcd_disabled_t_cycles {
            self.sample_ly_0_stall_lcd_disabled_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_lcd_disabled_t_cycles_observations += 1;
        }

        let elapsed = self.sample_started_at.elapsed();
        self.hud_snapshot = Some(self.snapshot_from_elapsed(elapsed));
        if elapsed < PERFORMANCE_SAMPLE_INTERVAL {
            return Ok(());
        }

        let snapshot = self
            .hud_snapshot
            .expect("performance HUD snapshot should exist after at least one frame");
        map_display_result(
            window.set_title(&performance_window_title(&self.base_title, snapshot)),
            "failed to update SDL3 window title",
        )?;
        if let Some(summary) = self.emulation_profile_summary(elapsed, snapshot) {
            eprintln!("{summary}");
        }

        self.reset_sample();

        Ok(())
    }

    fn reset_base_title(&mut self, window: &mut Window, base_title: String) -> Result<(), String> {
        self.base_title = base_title;
        self.hud_snapshot = None;
        self.reset_sample();
        map_display_result(
            window.set_title(&self.base_title),
            "failed to update SDL3 window title",
        )
    }

    fn hud_snapshot(&self) -> Option<PerformanceHudSnapshot> {
        self.hud_snapshot
    }

    fn emulation_profile_enabled(&self) -> bool {
        self.emulation_profile_mode.enabled()
    }

    fn should_profile_next_frame(&mut self) -> bool {
        self.collect_emulation_profile_results();
        let Some(sample_every_frames) = self.emulation_profile_mode.sample_every_frames() else {
            return false;
        };
        !self.emulation_profile_request_in_flight
            && self.presented_frames_total > 0
            && (self.presented_frames_total + 1).is_multiple_of(u64::from(sample_every_frames))
    }

    fn snapshot_from_elapsed(&self, elapsed: Duration) -> PerformanceHudSnapshot {
        let frames = self.frames_in_sample.max(1);
        let frames_f64 = f64::from(frames);
        let elapsed_secs = elapsed.as_secs_f64().max(f64::EPSILON);
        let fps = frames_f64 / elapsed_secs;

        PerformanceHudSnapshot {
            fps,
            speed_percent: fps / target_frame_rate_hz() * 100.0,
            frame_time_ms: elapsed_secs * 1_000.0 / frames_f64,
            emulation_time_ms: self.sample_emulation_duration.as_secs_f64() * 1_000.0 / frames_f64,
            render_time_ms: self.sample_render_duration.as_secs_f64() * 1_000.0 / frames_f64,
            pacing_time_ms: self.sample_pacing_duration.as_secs_f64() * 1_000.0 / frames_f64,
            audio_queue_ms: (self.sample_audio_queue_after_pacing_observations > 0).then_some(
                self.sample_audio_queue_after_pacing_ms
                    / f64::from(self.sample_audio_queue_after_pacing_observations),
            ),
        }
    }

    fn emulation_profile_summary(
        &self,
        elapsed: Duration,
        snapshot: PerformanceHudSnapshot,
    ) -> Option<String> {
        if !self.emulation_profile_enabled() || self.frames_in_sample == 0 {
            return None;
        }
        if self.sample_profiled_frames == 0 {
            return None;
        }

        let profiled_frames_f64 = f64::from(self.sample_profiled_frames.max(1));
        let frames_f64 = f64::from(self.frames_in_sample.max(1));
        let breakdown = self.sample_profiled_emulation_breakdown;
        let sampled_emu_ms =
            average_duration_ms(self.sample_profiled_emulation_duration, profiled_frames_f64);
        let estimated_core_duration = self
            .sample_profiled_emulation_duration
            .saturating_sub(breakdown.host_duration());
        let core_ms = average_duration_ms(estimated_core_duration, profiled_frames_f64);
        let host_ms = average_duration_ms(breakdown.host_duration(), profiled_frames_f64);
        let sample_every_frames = self
            .emulation_profile_mode
            .sample_every_frames()
            .expect("sampled emulation profile mode should provide a frame stride");
        let audio_submit_samples = if self.sample_audio_submit_sample_count_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_submit_sample_count as f64
                    / f64::from(self.sample_audio_submit_sample_count_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_submit_t_cycles = if self.sample_audio_submit_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_submit_t_cycles as f64
                    / f64::from(self.sample_audio_submit_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_submit_queue_before_ms = if self.sample_audio_submit_queue_before_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_audio_submit_queue_before_ms
                    / f64::from(self.sample_audio_submit_queue_before_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_submit_enqueued_ms = if self.sample_audio_submit_enqueued_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_submit_enqueued_ms
                    / f64::from(self.sample_audio_submit_enqueued_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_submit_queue_after_ms = if self.sample_audio_submit_queue_after_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_submit_queue_after_ms
                    / f64::from(self.sample_audio_submit_queue_after_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_queue_before_pacing_ms = if self.sample_audio_queue_before_pacing_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_audio_queue_before_pacing_ms
                    / f64::from(self.sample_audio_queue_before_pacing_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_queue_after_pacing_ms = if self.sample_audio_queue_after_pacing_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_queue_after_pacing_ms
                    / f64::from(self.sample_audio_queue_after_pacing_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_step_t_cycles = if self.sample_frame_step_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_step_t_cycles as f64
                    / f64::from(self.sample_frame_step_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_start_ly = if self.sample_frame_start_ly_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_start_ly as f64
                    / f64::from(self.sample_frame_start_ly_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_start_dot = if self.sample_frame_start_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_start_dot as f64
                    / f64::from(self.sample_frame_start_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_end_ly = if self.sample_frame_end_ly_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_end_ly as f64 / f64::from(self.sample_frame_end_ly_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_end_dot = if self.sample_frame_end_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_end_dot as f64
                    / f64::from(self.sample_frame_end_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_origin_crossings = if self.sample_frame_origin_crossings_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_origin_crossings as f64
                    / f64::from(self.sample_frame_origin_crossings_observations)
            )
        } else {
            "off".to_string()
        };
        let scanline_transitions = if self.sample_scanline_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_scanline_transitions as f64
                    / f64::from(self.sample_scanline_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let scanlines_over_456 = if self.sample_scanlines_over_456_observations > 0 {
            format!(
                "{:.2}",
                self.sample_scanlines_over_456 as f64
                    / f64::from(self.sample_scanlines_over_456_observations)
            )
        } else {
            "off".to_string()
        };
        let max_scanline_t_cycles = if self.sample_max_scanline_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_max_scanline_t_cycles as f64
                    / f64::from(self.sample_max_scanline_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let max_scanline_ly = if self.sample_max_scanline_ly_observations > 0 {
            format!(
                "{:.2}",
                self.sample_max_scanline_ly as f64
                    / f64::from(self.sample_max_scanline_ly_observations)
            )
        } else {
            "off".to_string()
        };
        let max_mode0_start_dot = if self.sample_max_mode0_start_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_max_mode0_start_dot as f64
                    / f64::from(self.sample_max_mode0_start_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let max_mode0_start_dot_ly = if self.sample_max_mode0_start_dot_ly_observations > 0 {
            format!(
                "{:.2}",
                self.sample_max_mode0_start_dot_ly as f64
                    / f64::from(self.sample_max_mode0_start_dot_ly_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_153_to_0_transitions = if self.sample_ly_153_to_0_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_153_to_0_transitions as f64
                    / f64::from(self.sample_ly_153_to_0_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_153_to_0_startup_mode0 = if self.sample_ly_153_to_0_startup_mode0_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_153_to_0_startup_mode0 as f64
                    / f64::from(self.sample_ly_153_to_0_startup_mode0_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_153_to_0_blank_frame = if self.sample_ly_153_to_0_blank_frame_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_153_to_0_blank_frame as f64
                    / f64::from(self.sample_ly_153_to_0_blank_frame_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_self_wraps = if self.sample_ly_0_self_wraps_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_self_wraps as f64
                    / f64::from(self.sample_ly_0_self_wraps_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_self_wrap_startup_mode0 =
            if self.sample_ly_0_self_wrap_startup_mode0_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_self_wrap_startup_mode0 as f64
                        / f64::from(self.sample_ly_0_self_wrap_startup_mode0_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_self_wrap_blank_frame = if self.sample_ly_0_self_wrap_blank_frame_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_ly_0_self_wrap_blank_frame as f64
                    / f64::from(self.sample_ly_0_self_wrap_blank_frame_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_to_1_transitions = if self.sample_ly_0_to_1_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_to_1_transitions as f64
                    / f64::from(self.sample_ly_0_to_1_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_scanline_t_cycles = if self.sample_ly_0_scanline_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_scanline_t_cycles as f64
                    / f64::from(self.sample_ly_0_scanline_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_max_mode0_start_dot = if self.sample_ly_0_max_mode0_start_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_max_mode0_start_dot as f64
                    / f64::from(self.sample_ly_0_max_mode0_start_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_t_cycles = if self.sample_ly_0_stall_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_stall_t_cycles as f64
                    / f64::from(self.sample_ly_0_stall_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_hblank_t_cycles = if self.sample_ly_0_stall_hblank_t_cycles_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_ly_0_stall_hblank_t_cycles as f64
                    / f64::from(self.sample_ly_0_stall_hblank_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_oam_t_cycles = if self.sample_ly_0_stall_oam_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_stall_oam_t_cycles as f64
                    / f64::from(self.sample_ly_0_stall_oam_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_drawing_t_cycles =
            if self.sample_ly_0_stall_drawing_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_drawing_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_drawing_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_startup_mode0_t_cycles =
            if self.sample_ly_0_stall_startup_mode0_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_startup_mode0_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_startup_mode0_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_blank_frame_t_cycles =
            if self.sample_ly_0_stall_blank_frame_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_blank_frame_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_blank_frame_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_runs = if self.sample_ly_0_stall_runs_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_stall_runs as f64
                    / f64::from(self.sample_ly_0_stall_runs_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_max_stall_run_t_cycles =
            if self.sample_ly_0_max_stall_run_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_max_stall_run_t_cycles as f64
                        / f64::from(self.sample_ly_0_max_stall_run_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_max_stall_dot = if self.sample_ly_0_max_stall_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_max_stall_dot as f64
                    / f64::from(self.sample_ly_0_max_stall_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_max_stall_mode_dot = if self.sample_ly_0_max_stall_mode_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_max_stall_mode_dot as f64
                    / f64::from(self.sample_ly_0_max_stall_mode_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let cpu_stop_t_cycles = if self.sample_cpu_stop_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_cpu_stop_t_cycles as f64
                    / f64::from(self.sample_cpu_stop_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let cpu_zombie_stop_t_cycles = if self.sample_cpu_zombie_stop_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_cpu_zombie_stop_t_cycles as f64
                    / f64::from(self.sample_cpu_zombie_stop_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_cpu_stop_t_cycles = if self.sample_ly_0_cpu_stop_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_cpu_stop_t_cycles as f64
                    / f64::from(self.sample_ly_0_cpu_stop_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_cpu_zombie_stop_t_cycles =
            if self.sample_ly_0_cpu_zombie_stop_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_cpu_zombie_stop_t_cycles as f64
                        / f64::from(self.sample_ly_0_cpu_zombie_stop_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_cpu_stop_t_cycles =
            if self.sample_ly_0_stall_cpu_stop_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_cpu_stop_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_cpu_stop_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_cpu_zombie_stop_t_cycles =
            if self.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_cpu_zombie_stop_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let lcd_disabled_t_cycles = if self.sample_lcd_disabled_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_lcd_disabled_t_cycles as f64
                    / f64::from(self.sample_lcd_disabled_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let lcd_disable_transitions = if self.sample_lcd_disable_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_lcd_disable_transitions as f64
                    / f64::from(self.sample_lcd_disable_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let lcd_enable_transitions = if self.sample_lcd_enable_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_lcd_enable_transitions as f64
                    / f64::from(self.sample_lcd_enable_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_lcd_disabled_t_cycles = if self.sample_ly_0_lcd_disabled_t_cycles_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_ly_0_lcd_disabled_t_cycles as f64
                    / f64::from(self.sample_ly_0_lcd_disabled_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_lcd_disabled_t_cycles =
            if self.sample_ly_0_stall_lcd_disabled_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_lcd_disabled_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_lcd_disabled_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };

        Some(format!(
            "gb-desktop emu-profile session={} fps={:.1} speed={:.0}% frame_ms={:.2} emu_ms={:.2} sampled_frames={} sample_every={} sampled_emu_ms={sampled_emu_ms:.2} core_est_ms={core_ms:.2} ppu_ms={:.2} cpu_ms={:.2} core_other_ms={:.2} ext_ms={:.2} timer_ms={:.2} apu_ms={:.2} dma_ms={:.2} serial_ms={:.2} irq_ms={:.2} ppu_mode0_1_ms={:.2} ppu_mode2_ms={:.2} ppu_mode3_startup_ms={:.2} ppu_bg_ms={:.2} ppu_win_ms={:.2} ppu_push_ms={:.2} ppu_obj_ms={:.2} ppu_px_ms={:.2} ppu_other_ms={:.2} host_ms={host_ms:.2} poll_ms={:.2} audsubmit_ms={:.2} save_ms={:.2} frame_tcycles={frame_step_t_cycles} frame_start_ly={frame_start_ly} frame_start_dot={frame_start_dot} frame_end_ly={frame_end_ly} frame_end_dot={frame_end_dot} frame_crossings={frame_origin_crossings} scanline_transitions={scanline_transitions} scanlines_over_456={scanlines_over_456} max_scanline_tcycles={max_scanline_t_cycles} max_scanline_ly={max_scanline_ly} max_mode0_start_dot={max_mode0_start_dot} max_mode0_start_dot_ly={max_mode0_start_dot_ly} ly153_to0={ly_153_to_0_transitions} ly153_to0_startup={ly_153_to_0_startup_mode0} ly153_to0_blank={ly_153_to_0_blank_frame} ly0_self_wraps={ly_0_self_wraps} ly0_self_wrap_startup={ly_0_self_wrap_startup_mode0} ly0_self_wrap_blank={ly_0_self_wrap_blank_frame} ly0_to1={ly_0_to_1_transitions} ly0_tcycles={ly_0_scanline_t_cycles} ly0_max_mode0_start_dot={ly_0_max_mode0_start_dot} ly0_stall_tcycles={ly_0_stall_t_cycles} ly0_stall_hb_tcycles={ly_0_stall_hblank_t_cycles} ly0_stall_oam_tcycles={ly_0_stall_oam_t_cycles} ly0_stall_draw_tcycles={ly_0_stall_drawing_t_cycles} ly0_stall_startup_tcycles={ly_0_stall_startup_mode0_t_cycles} ly0_stall_blank_tcycles={ly_0_stall_blank_frame_t_cycles} ly0_stall_runs={ly_0_stall_runs} ly0_max_stall_tcycles={ly_0_max_stall_run_t_cycles} ly0_max_stall_dot={ly_0_max_stall_dot} ly0_max_stall_mode_dot={ly_0_max_stall_mode_dot} cpu_stop_tcycles={cpu_stop_t_cycles} cpu_zstop_tcycles={cpu_zombie_stop_t_cycles} ly0_stop_tcycles={ly_0_cpu_stop_t_cycles} ly0_zstop_tcycles={ly_0_cpu_zombie_stop_t_cycles} ly0_stall_stop_tcycles={ly_0_stall_cpu_stop_t_cycles} ly0_stall_zstop_tcycles={ly_0_stall_cpu_zombie_stop_t_cycles} lcdoff_tcycles={lcd_disabled_t_cycles} lcdoff_transitions={lcd_disable_transitions} lcdon_transitions={lcd_enable_transitions} ly0_lcdoff_tcycles={ly_0_lcd_disabled_t_cycles} ly0_stall_lcdoff_tcycles={ly_0_stall_lcd_disabled_t_cycles} submit_samples={audio_submit_samples} submit_tcycles={audio_submit_t_cycles} submit_queue_before_ms={audio_submit_queue_before_ms} submit_enqueued_ms={audio_submit_enqueued_ms} submit_queue_after_ms={audio_submit_queue_after_ms} audio_queue_before_ms={audio_queue_before_pacing_ms} audio_queue_after_ms={audio_queue_after_pacing_ms} present_ms={:.2} pac_ms={:.2} sleep_target_ms={:.2} audio_corr_ms={:.2} late_ms={:.2} oversleep_ms={:.2} sample_secs={:.2}",
            self.sample_session_kind.label(),
            snapshot.fps,
            snapshot.speed_percent,
            snapshot.frame_time_ms,
            snapshot.emulation_time_ms,
            self.sample_profiled_frames,
            sample_every_frames,
            scaled_average_duration_ms(
                breakdown.core_ppu_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_cpu_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_other_duration(),
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_external_events_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_timer_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_apu_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_dma_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_serial_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_interrupts_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_mode0_1_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_mode2_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_mode3_startup_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_bg_fetch_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_window_fetch_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_push_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_obj_fetch_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_pixel_transfer_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.ppu_other_duration(),
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            average_duration_ms(breakdown.host_event_poll_duration, profiled_frames_f64),
            average_duration_ms(breakdown.host_audio_submit_duration, profiled_frames_f64),
            average_duration_ms(breakdown.host_save_flush_duration, profiled_frames_f64),
            average_duration_ms(self.sample_present_duration, frames_f64),
            average_duration_ms(self.sample_pacing_duration, frames_f64),
            average_duration_ms(self.sample_pacing_sleep_target_duration, frames_f64),
            average_duration_ms(self.sample_pacing_audio_correction_duration, frames_f64),
            average_duration_ms(self.sample_pacing_late_duration, frames_f64),
            average_duration_ms(self.sample_pacing_oversleep_duration, frames_f64),
            elapsed.as_secs_f64(),
        ))
    }

    fn reset_sample(&mut self) {
        self.sample_started_at = Instant::now();
        self.frames_in_sample = 0;
        self.sample_emulation_duration = Duration::ZERO;
        self.sample_profiled_frames = 0;
        self.sample_profiled_emulation_duration = Duration::ZERO;
        self.sample_profiled_emulation_breakdown = EmulationBreakdownSample::default();
        self.sample_render_duration = Duration::ZERO;
        self.sample_present_duration = Duration::ZERO;
        self.sample_pacing_duration = Duration::ZERO;
        self.sample_pacing_sleep_target_duration = Duration::ZERO;
        self.sample_pacing_audio_correction_duration = Duration::ZERO;
        self.sample_pacing_late_duration = Duration::ZERO;
        self.sample_pacing_oversleep_duration = Duration::ZERO;
        self.sample_audio_submit_sample_count = 0;
        self.sample_audio_submit_sample_count_observations = 0;
        self.sample_audio_submit_t_cycles = 0;
        self.sample_audio_submit_t_cycles_observations = 0;
        self.sample_audio_submit_queue_before_ms = 0.0;
        self.sample_audio_submit_queue_before_observations = 0;
        self.sample_audio_submit_enqueued_ms = 0.0;
        self.sample_audio_submit_enqueued_observations = 0;
        self.sample_audio_submit_queue_after_ms = 0.0;
        self.sample_audio_submit_queue_after_observations = 0;
        self.sample_audio_queue_before_pacing_ms = 0.0;
        self.sample_audio_queue_before_pacing_observations = 0;
        self.sample_audio_queue_after_pacing_ms = 0.0;
        self.sample_audio_queue_after_pacing_observations = 0;
        self.sample_frame_step_t_cycles = 0;
        self.sample_frame_step_t_cycles_observations = 0;
        self.sample_frame_start_ly = 0;
        self.sample_frame_start_ly_observations = 0;
        self.sample_frame_start_dot = 0;
        self.sample_frame_start_dot_observations = 0;
        self.sample_frame_end_ly = 0;
        self.sample_frame_end_ly_observations = 0;
        self.sample_frame_end_dot = 0;
        self.sample_frame_end_dot_observations = 0;
        self.sample_frame_origin_crossings = 0;
        self.sample_frame_origin_crossings_observations = 0;
        self.sample_scanline_transitions = 0;
        self.sample_scanline_transitions_observations = 0;
        self.sample_scanlines_over_456 = 0;
        self.sample_scanlines_over_456_observations = 0;
        self.sample_max_scanline_t_cycles = 0;
        self.sample_max_scanline_t_cycles_observations = 0;
        self.sample_max_scanline_ly = 0;
        self.sample_max_scanline_ly_observations = 0;
        self.sample_max_mode0_start_dot = 0;
        self.sample_max_mode0_start_dot_observations = 0;
        self.sample_max_mode0_start_dot_ly = 0;
        self.sample_max_mode0_start_dot_ly_observations = 0;
        self.sample_ly_153_to_0_transitions = 0;
        self.sample_ly_153_to_0_transitions_observations = 0;
        self.sample_ly_153_to_0_startup_mode0 = 0;
        self.sample_ly_153_to_0_startup_mode0_observations = 0;
        self.sample_ly_153_to_0_blank_frame = 0;
        self.sample_ly_153_to_0_blank_frame_observations = 0;
        self.sample_ly_0_self_wraps = 0;
        self.sample_ly_0_self_wraps_observations = 0;
        self.sample_ly_0_self_wrap_startup_mode0 = 0;
        self.sample_ly_0_self_wrap_startup_mode0_observations = 0;
        self.sample_ly_0_self_wrap_blank_frame = 0;
        self.sample_ly_0_self_wrap_blank_frame_observations = 0;
        self.sample_ly_0_to_1_transitions = 0;
        self.sample_ly_0_to_1_transitions_observations = 0;
        self.sample_ly_0_scanline_t_cycles = 0;
        self.sample_ly_0_scanline_t_cycles_observations = 0;
        self.sample_ly_0_max_mode0_start_dot = 0;
        self.sample_ly_0_max_mode0_start_dot_observations = 0;
        self.sample_ly_0_stall_t_cycles = 0;
        self.sample_ly_0_stall_t_cycles_observations = 0;
        self.sample_ly_0_stall_hblank_t_cycles = 0;
        self.sample_ly_0_stall_hblank_t_cycles_observations = 0;
        self.sample_ly_0_stall_oam_t_cycles = 0;
        self.sample_ly_0_stall_oam_t_cycles_observations = 0;
        self.sample_ly_0_stall_drawing_t_cycles = 0;
        self.sample_ly_0_stall_drawing_t_cycles_observations = 0;
        self.sample_ly_0_stall_startup_mode0_t_cycles = 0;
        self.sample_ly_0_stall_startup_mode0_t_cycles_observations = 0;
        self.sample_ly_0_stall_blank_frame_t_cycles = 0;
        self.sample_ly_0_stall_blank_frame_t_cycles_observations = 0;
        self.sample_ly_0_stall_runs = 0;
        self.sample_ly_0_stall_runs_observations = 0;
        self.sample_ly_0_max_stall_run_t_cycles = 0;
        self.sample_ly_0_max_stall_run_t_cycles_observations = 0;
        self.sample_ly_0_max_stall_dot = 0;
        self.sample_ly_0_max_stall_dot_observations = 0;
        self.sample_ly_0_max_stall_mode_dot = 0;
        self.sample_ly_0_max_stall_mode_dot_observations = 0;
        self.sample_cpu_stop_t_cycles = 0;
        self.sample_cpu_stop_t_cycles_observations = 0;
        self.sample_cpu_zombie_stop_t_cycles = 0;
        self.sample_cpu_zombie_stop_t_cycles_observations = 0;
        self.sample_ly_0_cpu_stop_t_cycles = 0;
        self.sample_ly_0_cpu_stop_t_cycles_observations = 0;
        self.sample_ly_0_cpu_zombie_stop_t_cycles = 0;
        self.sample_ly_0_cpu_zombie_stop_t_cycles_observations = 0;
        self.sample_ly_0_stall_cpu_stop_t_cycles = 0;
        self.sample_ly_0_stall_cpu_stop_t_cycles_observations = 0;
        self.sample_ly_0_stall_cpu_zombie_stop_t_cycles = 0;
        self.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations = 0;
        self.sample_lcd_disabled_t_cycles = 0;
        self.sample_lcd_disabled_t_cycles_observations = 0;
        self.sample_lcd_disable_transitions = 0;
        self.sample_lcd_disable_transitions_observations = 0;
        self.sample_lcd_enable_transitions = 0;
        self.sample_lcd_enable_transitions_observations = 0;
        self.sample_ly_0_lcd_disabled_t_cycles = 0;
        self.sample_ly_0_lcd_disabled_t_cycles_observations = 0;
        self.sample_ly_0_stall_lcd_disabled_t_cycles = 0;
        self.sample_ly_0_stall_lcd_disabled_t_cycles_observations = 0;
    }

    fn collect_emulation_profile_results(&mut self) {
        let Some(worker) = self.emulation_profile_worker.as_ref() else {
            return;
        };
        worker.collect_completed(&mut |result| {
            self.emulation_profile_request_in_flight = false;
            self.sample_profiled_frames += 1;
            self.sample_profiled_emulation_duration += result.emulation_duration;
            self.sample_profiled_emulation_breakdown
                .accumulate(result.breakdown);
        });
    }

    fn submit_emulation_profile_request(
        &mut self,
        request: Option<EmulationProfileRequest>,
        emulation_duration: Duration,
    ) {
        let Some(request) = request else {
            return;
        };
        let Some(worker) = self.emulation_profile_worker.as_ref() else {
            return;
        };
        self.emulation_profile_request_in_flight =
            worker.try_submit(request.into_work_item(emulation_duration));
    }
}

fn average_duration_ms(duration: Duration, frames_f64: f64) -> f64 {
    duration.as_secs_f64() * 1_000.0 / frames_f64.max(f64::EPSILON)
}

fn scaled_average_duration_ms(
    observed_duration: Duration,
    observed_total: Duration,
    scaled_total: Duration,
    frames_f64: f64,
) -> f64 {
    let observed_total_secs = observed_total.as_secs_f64();
    if observed_total_secs <= f64::EPSILON {
        return 0.0;
    }

    average_duration_ms(observed_duration, frames_f64)
        * (scaled_total.as_secs_f64() / observed_total_secs)
}

fn map_path_dialog_result(result: Result<Vec<PathBuf>, DialogError>) -> PathDialogResult {
    match result {
        Ok(paths) => paths
            .into_iter()
            .next()
            .map(PathDialogResult::Selected)
            .unwrap_or(PathDialogResult::Canceled),
        Err(DialogError::Canceled) => PathDialogResult::Canceled,
        Err(error) => PathDialogResult::Failed(error.to_string()),
    }
}

fn show_error_message(window: Option<&Window>, title: &str, message: &str) {
    show_message_box(window, MessageBoxFlag::ERROR, title, message);
}

fn show_warning_message(window: Option<&Window>, title: &str, message: &str) {
    show_message_box(window, MessageBoxFlag::WARNING, title, message);
}

fn show_message_box(window: Option<&Window>, flags: MessageBoxFlag, title: &str, message: &str) {
    if let Err(error) = show_simple_message_box(flags, title, message, window) {
        eprintln!("warning: failed to show SDL3 message box '{title}': {error}");
    }
}

fn map_display_result<T, E>(result: Result<T, E>, context: &str) -> Result<T, String>
where
    E: Display,
{
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(format_display_error(context, &error.to_string())),
    }
}

fn format_display_error(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

fn format_debug_error(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

fn format_path_error(context: &str, path: &Path, error: &str) -> String {
    format!("{context} {}: {error}", path.display())
}

fn overflow_error(message: &str) -> String {
    message.to_string()
}

fn emulation_paused(machine: &Machine<TraceSummaryBuffer>, runtime: &FrontendRuntime) -> bool {
    machine.cartridge().is_empty() || runtime.paused || runtime.menu_state.is_open()
}

fn audio_source_machine(machine: &DesktopEmulationSession) -> &Machine<TraceSummaryBuffer> {
    machine.primary_machine()
}

fn build_automatic_audio_recording_options(
    session: &DesktopSession,
) -> Result<DesktopAudioRecordingOptions, String> {
    Ok(DesktopAudioRecordingOptions {
        output_path: resolve_next_audio_recording_output_path(
            session.rom_path(),
            session.current_dir.as_path(),
        )?,
        sample_rate_hz: DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ,
        stem_channels: Vec::new(),
    })
}

fn create_audio_recorder(
    mode: &DesktopAudioRecordingMode,
    channel_mask: ApuRecordedChannelMask,
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
) -> Result<Option<DesktopAudioRecorder>, String> {
    let options = match mode {
        DesktopAudioRecordingMode::Disabled => return Ok(None),
        DesktopAudioRecordingMode::Automatic => build_automatic_audio_recording_options(session)?,
        DesktopAudioRecordingMode::Explicit(options) => options.clone(),
    };
    let mut recorder = DesktopAudioRecorder::new(
        &options,
        audio_source_machine(machine).apu().console_model(),
    )?;
    recorder.set_channel_mask(channel_mask)?;
    Ok(Some(recorder))
}

fn finish_audio_recorder(recorder: &mut Option<DesktopAudioRecorder>) -> Result<(), String> {
    if let Some(mut active_recorder) = recorder.take() {
        active_recorder.finish()?;
    }
    Ok(())
}

fn restart_automatic_audio_recorder(
    runtime: &mut FrontendRuntime,
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
) -> Result<(), String> {
    finish_audio_recorder(&mut runtime.audio_recorder)?;
    runtime.audio_recorder = create_audio_recorder(
        &runtime.audio_recording_mode,
        runtime.audio_channel_mask,
        session,
        machine,
    )?;
    Ok(())
}

fn emulation_profile_session_kind(
    machine: &DesktopEmulationSession,
) -> EmulationProfileSessionKind {
    if machine.is_linked_dmg04_two_player() {
        EmulationProfileSessionKind::LinkedDmg04TwoPlayer
    } else {
        EmulationProfileSessionKind::Single
    }
}

fn should_exit_after_presented_frames(
    exit_after_frames: Option<u64>,
    presented_frames_total: u64,
) -> bool {
    exit_after_frames.is_some_and(|limit| presented_frames_total >= limit)
}

fn sync_gamepad_rumble(
    runtime: &mut FrontendRuntime,
    machine: &Machine<TraceSummaryBuffer>,
    now: Instant,
) -> Result<(), String> {
    let paused = emulation_paused(machine, runtime);
    let Some(gamepad_manager) = &mut runtime.gamepad_manager else {
        return Ok(());
    };

    if !machine.cartridge().has_rumble() {
        if !gamepad_manager.has_active_rumble_effect() {
            return Ok(());
        }
        gamepad_manager.update_rumble(false, now)?;
        return Ok(());
    }

    if !gamepad_manager.can_deliver_rumble() && !gamepad_manager.has_active_rumble_effect() {
        return Ok(());
    }

    let rumble_requested = !paused && machine.cartridge().rumble_on();
    gamepad_manager.update_rumble(rumble_requested, now)?;

    Ok(())
}

fn main() -> ExitCode {
    match run_from_cli(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            show_error_message(None, "gb-desktop error", &error);
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_from_cli<I, S>(arguments: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let arguments = arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_string())
        .collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
    {
        print!("{}", help_text());
        return Ok(());
    }

    let settings_store = DesktopSettingsStore::load()?;
    let base_config = settings_store.base_config();
    match parse_cli_arguments_with_base_config(
        arguments.iter().map(String::as_str),
        base_config.clone(),
    )? {
        CliAction::ShowHelp => {
            print!("{}", help_text());
            Ok(())
        }
        CliAction::Run(options) => {
            let persist_startup_fallback = options.config == base_config;
            if persist_startup_fallback {
                run_desktop_with_startup_fallback_persistence(*options, settings_store, true)
            } else {
                run_desktop(*options, settings_store)
            }
        }
    }
}

fn run_desktop(
    options: DesktopRunOptions,
    settings_store: DesktopSettingsStore,
) -> Result<(), String> {
    run_desktop_with_startup_fallback_persistence(options, settings_store, false)
}

fn run_desktop_with_startup_fallback_persistence(
    options: DesktopRunOptions,
    mut settings_store: DesktopSettingsStore,
    persist_startup_fallback: bool,
) -> Result<(), String> {
    let original_config = options.config.clone();
    let exit_after_frames = options.exit_after_frames;
    let startup_links_peer = options.linked_peer_rom_path.is_some();
    let current_dir =
        map_display_result(env::current_dir(), "failed to determine current directory")?;
    let loaded_rom = load_initial_rom(&options, &current_dir)?;
    let linked_secondary_rom = load_initial_linked_secondary_rom(&options, &current_dir)?;
    let last_open_directory = match loaded_rom.as_ref() {
        Some(rom) => rom.path.parent().map(Path::to_path_buf),
        None => settings_store.last_open_directory().map(Path::to_path_buf),
    };
    let mut session = DesktopSession {
        config: options.config,
        current_dir,
        loaded_rom,
        linked_secondary_rom,
        last_open_directory,
        recent_roms: settings_store.recent_roms().to_vec(),
        external_port_selection: if startup_links_peer {
            DesktopExternalPortSelection::GameLink
        } else {
            DesktopExternalPortSelection::None
        },
    };

    let (mut machine, diagnostics) = load_initial_emulation_session(&mut session)?;
    if persist_startup_fallback && session.config != original_config {
        settings_store.persist_machine_preferences(&session.config)?;
    }
    write_cartridge_diagnostics(&diagnostics);
    if let Some(rom_path) = session.rom_path() {
        settings_store.remember_loaded_rom(rom_path)?;
        session.recent_roms = settings_store.recent_roms().to_vec();
    }
    let save_session = open_save_session_for_session(&session, machine.primary_machine_mut())?;
    let secondary_save_session = if let Some(secondary_machine) = machine.secondary_machine_mut() {
        open_secondary_save_session_for_session(&session, secondary_machine)?
    } else {
        None
    };

    if session.config.video.vsync {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "1", &hint::Hint::Default);
    } else {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "0", &hint::Hint::Default);
    }

    let sdl = map_display_result(sdl3::init(), "failed to initialize SDL3")?;
    let mut input_state = FrontendInputState::new();
    let audio_channel_mask = ApuRecordedChannelMask::ALL;
    let audio_output = if session.config.audio.enabled {
        let mut audio_output = DesktopAudioOutput::new(
            &map_display_result(sdl.audio(), "failed to initialize SDL3 audio subsystem")?,
            &session.config.audio,
            audio_source_machine(&machine).apu().console_model(),
        )?;
        if settings_store.audio_muted() {
            audio_output.set_muted(true)?;
        }
        Some(audio_output)
    } else {
        None
    };
    let audio_recording_mode = match options.audio_recording.clone() {
        Some(audio_recording) => DesktopAudioRecordingMode::Explicit(audio_recording),
        None => DesktopAudioRecordingMode::Disabled,
    };
    let audio_recorder = create_audio_recorder(
        &audio_recording_mode,
        audio_channel_mask,
        &session,
        &machine,
    )?;
    let gamepad_manager = if session.config.input.gamepad.enabled {
        Some(GamepadManager::new(
            &map_display_result(sdl.gamepad(), "failed to initialize SDL3 gamepad subsystem")?,
            session.config.input.gamepad.clone(),
            &mut input_state,
            &mut machine,
        )?)
    } else {
        None
    };
    let video = map_display_result(sdl.video(), "failed to initialize SDL3 video subsystem")?;

    let framebuffer_dimensions = framebuffer_dimensions_for_session(&machine);
    let window_width = framebuffer_dimensions
        .width
        .checked_mul(u32::from(session.config.video.window_scale))
        .ok_or_else(|| overflow_error("window width overflowed"))?;
    let window_height = framebuffer_dimensions
        .height
        .checked_mul(u32::from(session.config.video.window_scale))
        .ok_or_else(|| overflow_error("window height overflowed"))?;

    let base_window_title = window_title(&session, &session.config);
    let mut frame_pacer = FramePacer::new(session.config.video.vsync);
    let mut performance_counter = PerformanceCounter::new(base_window_title.clone());
    let mut window_builder = video.window(&base_window_title, window_width, window_height);
    window_builder.position_centered();
    if session.config.video.fullscreen {
        window_builder.fullscreen();
    }
    let window = map_display_result(window_builder.build(), "failed to create SDL3 window")?;
    let mut canvas = window.into_canvas();
    apply_renderer_vsync(&mut canvas, &mut frame_pacer, session.config.video.vsync)?;
    let texture_creator = canvas.texture_creator();
    let mut texture = create_framebuffer_texture(&texture_creator, framebuffer_dimensions)?;
    let mut event_pump = map_display_result(sdl.event_pump(), "failed to create SDL3 event pump")?;
    let mut rgb_frame = vec![
        0_u8;
        framebuffer_dimensions.height as usize
            * framebuffer_pitch_bytes_for_dimensions(framebuffer_dimensions)
    ];
    let mut current_framebuffer_dimensions = framebuffer_dimensions;
    let mut runtime = FrontendRuntime {
        paused: !session.has_loaded_rom(),
        menu_state: OverlayMenuState::default(),
        input_state,
        secondary_input_state: FrontendInputState::new(),
        keyboard_bindings: session.config.input.keyboard,
        video_options: session.config.video.clone(),
        audio_volume_percent: session.config.audio.volume_percent,
        audio_channel_mask,
        audio_output,
        audio_recording_mode,
        audio_recorder,
        gamepad_manager,
        save_session,
        secondary_save_session,
        rtc_sync: HostRtcSync::from_host_clock(),
        open_rom_dialog: PathSelectionDialog::new(),
        open_rom_dialog_mode: OpenRomDialogMode::Primary,
        boot_rom_file_dialog: PathSelectionDialog::new(),
        boot_rom_directory_dialog: PathSelectionDialog::new(),
        save_directory_dialog: PathSelectionDialog::new(),
        trace_capture: DesktopTraceCapture::from_env()?,
        ch4_nr43_trace: DesktopCh4Nr43TraceCapture::from_env()?,
        printer_output: PrinterOutputState::default(),
    };
    apply_canvas_video_options_for_dimensions(
        &mut canvas,
        &runtime.video_options,
        current_framebuffer_dimensions,
    )?;
    if !session.has_loaded_rom() {
        runtime.menu_state.open(current_menu_presentation(
            canvas.window(),
            &runtime,
            &machine,
            &session,
        ));
    }

    let initial_menu_presentation = runtime.menu_state.is_open().then_some((
        &runtime.menu_state,
        current_menu_presentation(canvas.window(), &runtime, &machine, &session),
    ));
    sync_framebuffer_presentation_resources(
        &mut canvas,
        &texture_creator,
        &mut texture,
        &mut rgb_frame,
        &mut current_framebuffer_dimensions,
        &machine,
        &runtime.video_options,
    )?;
    let _ = render_frame(
        &mut canvas,
        &mut texture,
        &mut rgb_frame,
        FramebufferRenderInput {
            dimensions: current_framebuffer_dimensions,
            primary: FramebufferPanelInput {
                framebuffer: machine.primary_machine().ppu().framebuffer(),
                framebuffer_layer_sources: machine
                    .primary_machine()
                    .ppu()
                    .framebuffer_layer_sources(),
                bgwin_framebuffer: machine
                    .primary_machine()
                    .ppu()
                    .framebuffer_bgwin_panel_shades(),
                backdrop_framebuffer: machine
                    .primary_machine()
                    .ppu()
                    .framebuffer_backdrop_panel_shades(),
                bgwin_framebuffer_layer_sources: machine
                    .primary_machine()
                    .ppu()
                    .framebuffer_bgwin_layer_sources(),
            },
            secondary: machine
                .secondary_machine()
                .map(|secondary| FramebufferPanelInput {
                    framebuffer: secondary.ppu().framebuffer(),
                    framebuffer_layer_sources: secondary.ppu().framebuffer_layer_sources(),
                    bgwin_framebuffer: secondary.ppu().framebuffer_bgwin_panel_shades(),
                    backdrop_framebuffer: secondary.ppu().framebuffer_backdrop_panel_shades(),
                    bgwin_framebuffer_layer_sources: secondary
                        .ppu()
                        .framebuffer_bgwin_layer_sources(),
                }),
        },
        &runtime.video_options,
        initial_menu_presentation,
        None,
    )?;

    'running: loop {
        {
            let mut context = FrontendActionContext {
                session: &mut session,
                machine: &mut machine,
                runtime: &mut runtime,
                performance_counter: &mut performance_counter,
                frame_pacer: &mut frame_pacer,
                settings_store: &mut settings_store,
            };
            process_pending_open_rom_dialog(&event_pump, &mut canvas, &mut context)?;
            process_pending_boot_rom_file_dialog(&mut canvas, &mut context)?;
            process_pending_boot_rom_directory_dialog(&mut canvas, &mut context)?;
            process_pending_save_directory_dialog(&mut canvas, &mut context)?;
        }

        runtime.rtc_sync.apply_to_machine(&mut machine);

        match {
            let mut context = FrontendActionContext {
                session: &mut session,
                machine: &mut machine,
                runtime: &mut runtime,
                performance_counter: &mut performance_counter,
                frame_pacer: &mut frame_pacer,
                settings_store: &mut settings_store,
            };
            process_events(&mut event_pump, &mut canvas, &mut context)
        }? {
            LoopSignal::Continue => {}
            LoopSignal::Quit => break 'running,
        }

        if emulation_paused(&machine, &runtime) {
            if runtime.menu_state.is_open() {
                let menu_presentation = Some((
                    &runtime.menu_state,
                    current_menu_presentation(canvas.window(), &runtime, &machine, &session),
                ));
                sync_framebuffer_presentation_resources(
                    &mut canvas,
                    &texture_creator,
                    &mut texture,
                    &mut rgb_frame,
                    &mut current_framebuffer_dimensions,
                    &machine,
                    &runtime.video_options,
                )?;
                let _ = render_frame(
                    &mut canvas,
                    &mut texture,
                    &mut rgb_frame,
                    FramebufferRenderInput {
                        dimensions: current_framebuffer_dimensions,
                        primary: FramebufferPanelInput {
                            framebuffer: machine.primary_machine().ppu().framebuffer(),
                            framebuffer_layer_sources: machine
                                .primary_machine()
                                .ppu()
                                .framebuffer_layer_sources(),
                            bgwin_framebuffer: machine
                                .primary_machine()
                                .ppu()
                                .framebuffer_bgwin_panel_shades(),
                            backdrop_framebuffer: machine
                                .primary_machine()
                                .ppu()
                                .framebuffer_backdrop_panel_shades(),
                            bgwin_framebuffer_layer_sources: machine
                                .primary_machine()
                                .ppu()
                                .framebuffer_bgwin_layer_sources(),
                        },
                        secondary: machine.secondary_machine().map(|secondary| {
                            FramebufferPanelInput {
                                framebuffer: secondary.ppu().framebuffer(),
                                framebuffer_layer_sources: secondary
                                    .ppu()
                                    .framebuffer_layer_sources(),
                                bgwin_framebuffer: secondary.ppu().framebuffer_bgwin_panel_shades(),
                                backdrop_framebuffer: secondary
                                    .ppu()
                                    .framebuffer_backdrop_panel_shades(),
                                bgwin_framebuffer_layer_sources: secondary
                                    .ppu()
                                    .framebuffer_bgwin_layer_sources(),
                            }
                        }),
                    },
                    &runtime.video_options,
                    menu_presentation,
                    None,
                )?;
            }
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        let emulation_started_at = Instant::now();
        let step_result = {
            let mut context = FrontendActionContext {
                session: &mut session,
                machine: &mut machine,
                runtime: &mut runtime,
                performance_counter: &mut performance_counter,
                frame_pacer: &mut frame_pacer,
                settings_store: &mut settings_store,
            };
            step_until_next_frame(&mut event_pump, &mut canvas, &mut context)
        }?;
        match step_result.signal {
            LoopSignal::Continue => {}
            LoopSignal::Quit => break 'running,
        }
        let emulation_duration = emulation_started_at.elapsed();
        let audio_submit_telemetry = runtime
            .audio_output
            .as_mut()
            .and_then(DesktopAudioOutput::take_last_submit_telemetry);

        if emulation_paused(&machine, &runtime) {
            if runtime.menu_state.is_open() {
                let menu_presentation = Some((
                    &runtime.menu_state,
                    current_menu_presentation(canvas.window(), &runtime, &machine, &session),
                ));
                sync_framebuffer_presentation_resources(
                    &mut canvas,
                    &texture_creator,
                    &mut texture,
                    &mut rgb_frame,
                    &mut current_framebuffer_dimensions,
                    &machine,
                    &runtime.video_options,
                )?;
                let _ = render_frame(
                    &mut canvas,
                    &mut texture,
                    &mut rgb_frame,
                    FramebufferRenderInput {
                        dimensions: current_framebuffer_dimensions,
                        primary: FramebufferPanelInput {
                            framebuffer: machine.primary_machine().ppu().framebuffer(),
                            framebuffer_layer_sources: machine
                                .primary_machine()
                                .ppu()
                                .framebuffer_layer_sources(),
                            bgwin_framebuffer: machine
                                .primary_machine()
                                .ppu()
                                .framebuffer_bgwin_panel_shades(),
                            backdrop_framebuffer: machine
                                .primary_machine()
                                .ppu()
                                .framebuffer_backdrop_panel_shades(),
                            bgwin_framebuffer_layer_sources: machine
                                .primary_machine()
                                .ppu()
                                .framebuffer_bgwin_layer_sources(),
                        },
                        secondary: machine.secondary_machine().map(|secondary| {
                            FramebufferPanelInput {
                                framebuffer: secondary.ppu().framebuffer(),
                                framebuffer_layer_sources: secondary
                                    .ppu()
                                    .framebuffer_layer_sources(),
                                bgwin_framebuffer: secondary.ppu().framebuffer_bgwin_panel_shades(),
                                backdrop_framebuffer: secondary
                                    .ppu()
                                    .framebuffer_backdrop_panel_shades(),
                                bgwin_framebuffer_layer_sources: secondary
                                    .ppu()
                                    .framebuffer_bgwin_layer_sources(),
                            }
                        }),
                    },
                    &runtime.video_options,
                    menu_presentation,
                    None,
                )?;
            }
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        let render_started_at = Instant::now();
        sync_framebuffer_presentation_resources(
            &mut canvas,
            &texture_creator,
            &mut texture,
            &mut rgb_frame,
            &mut current_framebuffer_dimensions,
            &machine,
            &runtime.video_options,
        )?;
        let present_duration = render_frame(
            &mut canvas,
            &mut texture,
            &mut rgb_frame,
            FramebufferRenderInput {
                dimensions: current_framebuffer_dimensions,
                primary: FramebufferPanelInput {
                    framebuffer: machine.primary_machine().ppu().framebuffer(),
                    framebuffer_layer_sources: machine
                        .primary_machine()
                        .ppu()
                        .framebuffer_layer_sources(),
                    bgwin_framebuffer: machine
                        .primary_machine()
                        .ppu()
                        .framebuffer_bgwin_panel_shades(),
                    backdrop_framebuffer: machine
                        .primary_machine()
                        .ppu()
                        .framebuffer_backdrop_panel_shades(),
                    bgwin_framebuffer_layer_sources: machine
                        .primary_machine()
                        .ppu()
                        .framebuffer_bgwin_layer_sources(),
                },
                secondary: machine
                    .secondary_machine()
                    .map(|secondary| FramebufferPanelInput {
                        framebuffer: secondary.ppu().framebuffer(),
                        framebuffer_layer_sources: secondary.ppu().framebuffer_layer_sources(),
                        bgwin_framebuffer: secondary.ppu().framebuffer_bgwin_panel_shades(),
                        backdrop_framebuffer: secondary.ppu().framebuffer_backdrop_panel_shades(),
                        bgwin_framebuffer_layer_sources: secondary
                            .ppu()
                            .framebuffer_bgwin_layer_sources(),
                    }),
            },
            &runtime.video_options,
            None,
            performance_counter.hud_snapshot(),
        )?;
        let render_duration = render_started_at.elapsed();
        let audio_queue_ms_before_pacing = runtime
            .audio_output
            .as_ref()
            .and_then(DesktopAudioOutput::queued_duration_ms);
        let pacing = frame_pacer.wait_until_next_frame(audio_queue_ms_before_pacing);
        let audio_queue_ms_after_pacing = runtime
            .audio_output
            .as_ref()
            .and_then(DesktopAudioOutput::queued_duration_ms);
        let AudioSubmitTelemetry {
            sample_count: audio_submit_sample_count,
            captured_t_cycles: audio_submit_t_cycles,
            queued_ms_before: audio_submit_queue_before_ms,
            enqueued_ms: audio_submit_enqueued_ms,
            queued_ms_after: audio_submit_queue_after_ms,
        } = audio_submit_telemetry.unwrap_or_default();
        let frame_loop_telemetry = step_result.frame_loop_telemetry;
        performance_counter.record_presented_frame(
            canvas.window_mut(),
            FramePerformanceSample {
                session_kind: emulation_profile_session_kind(&machine),
                emulation_duration,
                emulation_profile_request: step_result.emulation_profile_request,
                render_duration,
                present_duration,
                pacing_duration: pacing.pacing_duration,
                pacing_sleep_target_duration: pacing.sleep_target_duration,
                pacing_audio_correction_duration: pacing.audio_correction_duration,
                pacing_late_duration: pacing.late_duration,
                pacing_oversleep_duration: pacing.oversleep_duration,
                audio_submit_sample_count: Some(audio_submit_sample_count),
                audio_submit_t_cycles: Some(audio_submit_t_cycles),
                audio_submit_queue_before_ms,
                audio_submit_enqueued_ms,
                audio_submit_queue_after_ms,
                audio_queue_before_pacing_ms: audio_queue_ms_before_pacing,
                audio_queue_after_pacing_ms: audio_queue_ms_after_pacing,
                frame_step_t_cycles: Some(frame_loop_telemetry.stepped_t_cycles),
                frame_start_ly: Some(frame_loop_telemetry.start_ly),
                frame_start_dot: Some(frame_loop_telemetry.start_dot),
                frame_end_ly: Some(frame_loop_telemetry.end_ly),
                frame_end_dot: Some(frame_loop_telemetry.end_dot),
                frame_origin_crossings: Some(frame_loop_telemetry.frame_origin_crossings),
                scanline_transitions: Some(frame_loop_telemetry.scanline_transitions),
                scanlines_over_456: Some(frame_loop_telemetry.scanlines_over_456),
                max_scanline_t_cycles: Some(frame_loop_telemetry.max_scanline_t_cycles),
                max_scanline_ly: Some(frame_loop_telemetry.max_scanline_ly),
                max_mode0_start_dot: Some(frame_loop_telemetry.max_mode0_start_dot),
                max_mode0_start_dot_ly: Some(frame_loop_telemetry.max_mode0_start_dot_ly),
                ly_153_to_0_transitions: Some(frame_loop_telemetry.ly_153_to_0_transitions),
                ly_153_to_0_startup_mode0: Some(frame_loop_telemetry.ly_153_to_0_startup_mode0),
                ly_153_to_0_blank_frame: Some(frame_loop_telemetry.ly_153_to_0_blank_frame),
                ly_0_self_wraps: Some(frame_loop_telemetry.ly_0_self_wraps),
                ly_0_self_wrap_startup_mode0: Some(
                    frame_loop_telemetry.ly_0_self_wrap_startup_mode0,
                ),
                ly_0_self_wrap_blank_frame: Some(frame_loop_telemetry.ly_0_self_wrap_blank_frame),
                ly_0_to_1_transitions: Some(frame_loop_telemetry.ly_0_to_1_transitions),
                ly_0_scanline_t_cycles: Some(frame_loop_telemetry.ly_0_scanline_t_cycles),
                ly_0_max_mode0_start_dot: Some(frame_loop_telemetry.ly_0_max_mode0_start_dot),
                ly_0_stall_t_cycles: Some(frame_loop_telemetry.ly_0_stall_t_cycles),
                ly_0_stall_hblank_t_cycles: Some(frame_loop_telemetry.ly_0_stall_hblank_t_cycles),
                ly_0_stall_oam_t_cycles: Some(frame_loop_telemetry.ly_0_stall_oam_t_cycles),
                ly_0_stall_drawing_t_cycles: Some(frame_loop_telemetry.ly_0_stall_drawing_t_cycles),
                ly_0_stall_startup_mode0_t_cycles: Some(
                    frame_loop_telemetry.ly_0_stall_startup_mode0_t_cycles,
                ),
                ly_0_stall_blank_frame_t_cycles: Some(
                    frame_loop_telemetry.ly_0_stall_blank_frame_t_cycles,
                ),
                ly_0_stall_runs: Some(frame_loop_telemetry.ly_0_stall_runs),
                ly_0_max_stall_run_t_cycles: Some(frame_loop_telemetry.ly_0_max_stall_run_t_cycles),
                ly_0_max_stall_dot: Some(frame_loop_telemetry.ly_0_max_stall_dot),
                ly_0_max_stall_mode_dot: Some(frame_loop_telemetry.ly_0_max_stall_mode_dot),
                cpu_stop_t_cycles: Some(frame_loop_telemetry.cpu_stop_t_cycles),
                cpu_zombie_stop_t_cycles: Some(frame_loop_telemetry.cpu_zombie_stop_t_cycles),
                ly_0_cpu_stop_t_cycles: Some(frame_loop_telemetry.ly_0_cpu_stop_t_cycles),
                ly_0_cpu_zombie_stop_t_cycles: Some(
                    frame_loop_telemetry.ly_0_cpu_zombie_stop_t_cycles,
                ),
                ly_0_stall_cpu_stop_t_cycles: Some(
                    frame_loop_telemetry.ly_0_stall_cpu_stop_t_cycles,
                ),
                ly_0_stall_cpu_zombie_stop_t_cycles: Some(
                    frame_loop_telemetry.ly_0_stall_cpu_zombie_stop_t_cycles,
                ),
                lcd_disabled_t_cycles: Some(frame_loop_telemetry.lcd_disabled_t_cycles),
                lcd_disable_transitions: Some(frame_loop_telemetry.lcd_disable_transitions),
                lcd_enable_transitions: Some(frame_loop_telemetry.lcd_enable_transitions),
                ly_0_lcd_disabled_t_cycles: Some(frame_loop_telemetry.ly_0_lcd_disabled_t_cycles),
                ly_0_stall_lcd_disabled_t_cycles: Some(
                    frame_loop_telemetry.ly_0_stall_lcd_disabled_t_cycles,
                ),
            },
        )?;
        if should_exit_after_presented_frames(
            exit_after_frames,
            performance_counter.presented_frames_total,
        ) {
            break 'running;
        }
    }

    settings_store.set_fullscreen(canvas.window().fullscreen_state() != FullscreenType::Off)?;
    flush_pending_printer_output(canvas.window(), &session, &mut runtime);
    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager.update_rumble(false, Instant::now())?;
    }

    close_runtime_save_sessions(&mut runtime, &machine)?;
    if let Some(rom_path) = session.rom_path() {
        settings_store.remember_loaded_rom(rom_path)?;
    }
    if let Some(audio_output) = &runtime.audio_output {
        audio_output.flush()?;
    }
    if let Some(audio_recorder) = &mut runtime.audio_recorder {
        audio_recorder.finish()?;
    }
    runtime.trace_capture.write_artifact()?;
    runtime.ch4_nr43_trace.write_artifact()?;

    Ok(())
}

fn load_initial_emulation_session(
    session: &mut DesktopSession,
) -> Result<(DesktopEmulationSession, Vec<CartridgeDiagnostic>), String> {
    match (
        session.rom_bytes(),
        session.linked_secondary_rom_bytes(),
        session.external_port_selection,
    ) {
        (
            Some(primary_rom_bytes),
            Some(secondary_rom_bytes),
            DesktopExternalPortSelection::GameLink,
        ) => {
            let primary_loaded =
                load_machine_for_rom(&session.config, &session.current_dir, primary_rom_bytes)?;
            let secondary_loaded =
                load_machine_for_rom(&session.config, &session.current_dir, secondary_rom_bytes)?;
            if primary_loaded.effective_config != secondary_loaded.effective_config {
                return Err(
                    "linked desktop startup produced divergent effective configs between the primary and secondary machines"
                        .to_string(),
                );
            }

            log_boot_rom_fallback_warning(primary_loaded.boot_rom_fallback_warning.as_deref());
            log_boot_rom_fallback_warning(secondary_loaded.boot_rom_fallback_warning.as_deref());
            session.config = primary_loaded.effective_config;
            let machine = DesktopEmulationSession::new_linked_dmg04_two_player(
                primary_loaded.machine,
                secondary_loaded.machine,
            )?;
            let mut diagnostics = primary_loaded.diagnostics;
            diagnostics.extend(secondary_loaded.diagnostics);
            Ok((machine, diagnostics))
        }
        (Some(rom_bytes), _, _) => {
            let loaded = load_machine_for_rom(&session.config, &session.current_dir, rom_bytes)?;
            log_boot_rom_fallback_warning(loaded.boot_rom_fallback_warning.as_deref());
            session.config = loaded.effective_config;
            let mut machine = DesktopEmulationSession::new_single(loaded.machine);
            apply_external_port_selection_to_machine(&mut machine, session.external_port_selection);
            Ok((machine, loaded.diagnostics))
        }
        (None, _, _) => {
            let prepared = prepare_machine_config(&session.config, &session.current_dir)?;
            log_boot_rom_fallback_warning(prepared.boot_rom_fallback_warning.as_deref());
            session.config = prepared.effective_config;
            let mut machine =
                DesktopEmulationSession::new_single(Machine::new_summary(prepared.machine_config));
            apply_external_port_selection_to_machine(&mut machine, session.external_port_selection);
            Ok((machine, Vec::new()))
        }
    }
}

#[derive(Debug)]
struct PreparedMachineConfig {
    effective_config: DesktopConfig,
    machine_config: MachineConfig,
    boot_rom_fallback_warning: Option<String>,
}

#[derive(Debug)]
struct LoadedMachine {
    effective_config: DesktopConfig,
    machine: Machine<TraceSummaryBuffer>,
    diagnostics: Vec<CartridgeDiagnostic>,
    boot_rom_fallback_warning: Option<String>,
}

type RebuildMachineResult = (
    DesktopConfig,
    Vec<String>,
    DesktopEmulationSession,
    Option<DesktopSaveSession>,
    Option<DesktopSaveSession>,
);

fn prepare_machine_config(
    config: &DesktopConfig,
    current_dir: &Path,
) -> Result<PreparedMachineConfig, String> {
    let mut effective_config = config.clone();
    let boot_rom_fallback_warning =
        maybe_apply_missing_boot_rom_fallback(&mut effective_config, current_dir)?;
    let boot_rom_assets = load_boot_rom_assets(
        effective_config.boot_rom.search_path.as_deref(),
        effective_config.boot_rom.verification,
        effective_config.launch.console_model,
        effective_config.launch.startup_mode,
        current_dir,
    )?;

    Ok(PreparedMachineConfig {
        machine_config: MachineConfig::new(effective_config.launch.console_model.console_model())
            .with_startup_mode(effective_config.launch.startup_mode)
            .with_execution_mode(effective_config.launch.execution_mode)
            .with_boot_rom_assets(boot_rom_assets),
        effective_config,
        boot_rom_fallback_warning,
    })
}

fn maybe_apply_missing_boot_rom_fallback(
    config: &mut DesktopConfig,
    current_dir: &Path,
) -> Result<Option<String>, String> {
    if config.launch.startup_mode != StartupMode::RealBoot {
        return Ok(None);
    }

    let Some(missing_path) = missing_boot_rom_asset_path(
        config.boot_rom.search_path.as_deref(),
        config.launch.console_model,
        current_dir,
    )?
    else {
        return Ok(None);
    };

    config.launch.startup_mode = StartupMode::SkipBoot;
    Ok(Some(format!(
        "boot ROM asset missing at {}; falling back to skip-boot",
        missing_path.display()
    )))
}

fn log_boot_rom_fallback_warning(warning: Option<&str>) {
    if let Some(warning) = warning {
        eprintln!("warning: {warning}");
    }
}

fn load_machine_for_rom(
    config: &DesktopConfig,
    current_dir: &Path,
    rom_bytes: &[u8],
) -> Result<LoadedMachine, String> {
    let prepared = prepare_machine_config(config, current_dir)?;
    let mut machine = Machine::new_summary(prepared.machine_config);
    let diagnostics = match machine.load_cartridge(rom_bytes.to_vec()) {
        Ok(diagnostics) => diagnostics,
        Err(error) => {
            return Err(format_debug_error(
                "failed to load cartridge",
                &format!("{error:?}"),
            ));
        }
    };
    Ok(LoadedMachine {
        effective_config: prepared.effective_config,
        machine,
        diagnostics,
        boot_rom_fallback_warning: prepared.boot_rom_fallback_warning,
    })
}

fn apply_external_port_selection_to_machine(
    machine: &mut Machine<TraceSummaryBuffer>,
    selection: DesktopExternalPortSelection,
) {
    machine.set_external_port_attachment(selection.core_attachment_kind());
}

fn drain_printed_pages_into_printer_output(
    main_window: &Window,
    session: &DesktopSession,
    runtime: &mut FrontendRuntime,
    machine: &mut Machine<TraceSummaryBuffer>,
) {
    let printed_pages = machine.take_printed_pages();
    if printed_pages.is_empty() {
        return;
    }

    for printed_page in printed_pages {
        if let Err(error) = runtime.printer_output.handle_printed_page(
            main_window,
            session.rom_path(),
            session.current_dir.as_path(),
            &printed_page,
        ) {
            eprintln!("printer output failed: {error}");
        }
    }
}

fn flush_pending_printer_output(
    main_window: &Window,
    session: &DesktopSession,
    runtime: &mut FrontendRuntime,
) {
    if let Err(error) = runtime.printer_output.flush_pending_document(
        main_window,
        session.rom_path(),
        session.current_dir.as_path(),
    ) {
        eprintln!("printer output failed: {error}");
    }
}

fn load_initial_rom(
    options: &DesktopRunOptions,
    current_dir: &Path,
) -> Result<Option<LoadedRom>, String> {
    let Some(rom_path) = options.rom_path.as_ref() else {
        return Ok(None);
    };
    load_rom_from_cli_path(current_dir, rom_path, "failed to read ROM").map(Some)
}

fn load_initial_linked_secondary_rom(
    options: &DesktopRunOptions,
    current_dir: &Path,
) -> Result<Option<LoadedRom>, String> {
    let Some(rom_path) = options.linked_peer_rom_path.as_ref() else {
        return Ok(None);
    };
    load_rom_from_cli_path(current_dir, rom_path, "failed to read linked peer ROM").map(Some)
}

fn load_rom_from_cli_path(
    current_dir: &Path,
    rom_path: &Path,
    read_error_label: &str,
) -> Result<LoadedRom, String> {
    let rom_path = resolve_path(current_dir, rom_path);
    let rom_bytes = match fs::read(&rom_path) {
        Ok(rom_bytes) => rom_bytes,
        Err(error) => {
            return Err(format_path_error(
                read_error_label,
                &rom_path,
                &error.to_string(),
            ));
        }
    };
    Ok(LoadedRom {
        path: rom_path,
        bytes: rom_bytes,
    })
}

fn open_save_session_for_session(
    session: &DesktopSession,
    machine: &mut Machine<TraceSummaryBuffer>,
) -> Result<Option<DesktopSaveSession>, String> {
    open_save_session_for_loaded_rom(session, session.loaded_rom.as_ref(), machine)
}

fn open_secondary_save_session_for_session(
    session: &DesktopSession,
    machine: &mut Machine<TraceSummaryBuffer>,
) -> Result<Option<DesktopSaveSession>, String> {
    open_save_session_for_loaded_rom(session, session.linked_secondary_rom.as_ref(), machine)
}

fn open_save_session_for_loaded_rom(
    session: &DesktopSession,
    loaded_rom: Option<&LoadedRom>,
    machine: &mut Machine<TraceSummaryBuffer>,
) -> Result<Option<DesktopSaveSession>, String> {
    let Some(rom_path) = loaded_rom.map(|rom| rom.path.as_path()) else {
        return Ok(None);
    };

    let save_root = session
        .config
        .saves
        .resolve_directory(rom_path)
        .map(|path| resolve_path(&session.current_dir, &path));
    let save_key = session.config.saves.resolve_key(rom_path);
    let save_key = match save_key {
        Ok(save_key) => save_key,
        Err(error) => return Err(error.to_string()),
    };
    DesktopSaveSession::open(
        save_root.as_deref(),
        session.config.saves.flush_policy,
        save_key,
        machine,
    )
}

fn window_title(session: &DesktopSession, config: &DesktopConfig) -> String {
    let rom_name = match (session.rom_path(), session.linked_secondary_rom_path()) {
        (Some(primary_path), Some(secondary_path)) => format!(
            "{} + {}",
            primary_path
                .file_name()
                .unwrap_or(primary_path.as_os_str())
                .to_string_lossy(),
            secondary_path
                .file_name()
                .unwrap_or(secondary_path.as_os_str())
                .to_string_lossy(),
        ),
        (Some(rom_path), None) => rom_path
            .file_name()
            .unwrap_or(rom_path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        (None, _) => "no ROM loaded".to_string(),
    };
    format!(
        "gb-desktop | {} | {} | {} | {}",
        rom_name,
        config.launch.console_model.name(),
        startup_mode_name(config.launch.startup_mode),
        execution_mode_name(config.launch.execution_mode),
    )
}

fn close_runtime_save_sessions(
    runtime: &mut FrontendRuntime,
    machine: &DesktopEmulationSession,
) -> Result<(), String> {
    if let Some(save_session) = &mut runtime.save_session {
        save_session.close(machine.primary_machine())?;
    }
    if let Some(save_session) = &mut runtime.secondary_save_session
        && let Some(secondary_machine) = machine.secondary_machine()
    {
        save_session.close(secondary_machine)?;
    }
    Ok(())
}

fn flush_runtime_save_sessions_if_changed(
    runtime: &mut FrontendRuntime,
    machine: &DesktopEmulationSession,
    reason: &str,
) -> Result<(), String> {
    if let Some(save_session) = &mut runtime.save_session {
        let _ = save_session.flush_if_changed(machine.primary_machine(), reason)?;
    }
    if let Some(save_session) = &mut runtime.secondary_save_session
        && let Some(secondary_machine) = machine.secondary_machine()
    {
        let _ = save_session.flush_if_changed(secondary_machine, reason)?;
    }
    Ok(())
}

fn maybe_flush_runtime_save_sessions_at_frame_boundary(
    runtime: &mut FrontendRuntime,
    machine: &DesktopEmulationSession,
    now: Instant,
) -> Result<(), String> {
    if let Some(save_session) = &mut runtime.save_session {
        let _ = save_session.maybe_flush_at_frame_boundary(machine.primary_machine(), now)?;
    }
    if let Some(save_session) = &mut runtime.secondary_save_session
        && let Some(secondary_machine) = machine.secondary_machine()
    {
        let _ = save_session.maybe_flush_at_frame_boundary(secondary_machine, now)?;
    }
    Ok(())
}

fn performance_window_title(base_title: &str, snapshot: PerformanceHudSnapshot) -> String {
    let audio = match snapshot.audio_queue_ms {
        Some(audio_queue_ms) => format!("{audio_queue_ms:.1} ms"),
        None => "off".to_string(),
    };
    format!(
        "{base_title} | {:.1} FPS | {:.2} ms | {:.0}% speed | emu {:.2} | render {:.2} | pacing {:.2} | audio {audio}",
        snapshot.fps,
        snapshot.frame_time_ms,
        snapshot.speed_percent,
        snapshot.emulation_time_ms,
        snapshot.render_time_ms,
        snapshot.pacing_time_ms,
    )
}

fn target_frame_rate_hz() -> f64 {
    1.0 / FRAME_DURATION.as_secs_f64()
}

fn process_events(
    event_pump: &mut sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<LoopSignal, String> {
    let session = &mut *context.session;
    let machine = &mut *context.machine;
    let runtime = &mut *context.runtime;
    let performance_counter = &mut *context.performance_counter;
    let frame_pacer = &mut *context.frame_pacer;
    let settings_store = &mut *context.settings_store;
    let events = event_pump.poll_iter().collect::<Vec<_>>();
    for event in events {
        if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
            gamepad_manager.handle_event(
                &event,
                &mut runtime.input_state,
                machine.primary_machine_mut(),
            )?;
            if let Event::ControllerButtonDown { which, .. } = &event {
                gamepad_manager.activate_gamepad_from_input(
                    gamepad_event_joystick_id(*which),
                    &mut runtime.input_state,
                    machine.primary_machine_mut(),
                );
            }
        }

        if runtime.printer_output.handle_event(&event)? {
            continue;
        }

        if runtime.menu_state.is_open() && runtime.menu_state.is_capturing_binding() {
            match &event {
                Event::Quit { .. } => return Ok(LoopSignal::Quit),
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    repeat: false,
                    ..
                } => {
                    runtime.menu_state.cancel_binding_capture();
                    continue;
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    if let Some(target) = runtime.menu_state.pending_keyboard_binding_target() {
                        if let Some(key) =
                            assignable_key_for_binding_target_from_keycode(*keycode, target)
                            && let Some(action) =
                                runtime.menu_state.handle_keyboard_binding_capture(key)
                        {
                            let mut context = FrontendActionContext {
                                session,
                                machine,
                                runtime,
                                performance_counter,
                                frame_pacer,
                                settings_store,
                            };
                            let _ = execute_menu_action(action, event_pump, canvas, &mut context)?;
                        }
                    } else if let Some(target) =
                        runtime.menu_state.pending_keyboard_menu_binding_target()
                        && let Some(key) =
                            assignable_menu_key_for_binding_target_from_keycode(*keycode, target)
                        && let Some(action) =
                            runtime.menu_state.handle_keyboard_binding_capture(key)
                    {
                        let mut context = FrontendActionContext {
                            session,
                            machine,
                            runtime,
                            performance_counter,
                            frame_pacer,
                            settings_store,
                        };
                        let _ = execute_menu_action(action, event_pump, canvas, &mut context)?;
                    }
                    continue;
                }
                Event::ControllerButtonDown { which, button, .. }
                    if runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                        manager.is_active_gamepad(gamepad_event_joystick_id(*which))
                    }) =>
                {
                    if runtime
                        .menu_state
                        .pending_gamepad_binding_target()
                        .is_some()
                        && let Some(binding) = gamepad_button_binding_from_sdl_button(*button)
                        && let Some(action) =
                            runtime.menu_state.handle_gamepad_binding_capture(binding)
                    {
                        let mut context = FrontendActionContext {
                            session,
                            machine,
                            runtime,
                            performance_counter,
                            frame_pacer,
                            settings_store,
                        };
                        let _ = execute_menu_action(action, event_pump, canvas, &mut context)?;
                    }
                    continue;
                }
                _ => continue,
            }
        }

        match &event {
            Event::Quit { .. } => return Ok(LoopSignal::Quit),
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                repeat: false,
                ..
            } if !runtime.menu_state.is_open() => {
                toggle_menu(event_pump, canvas.window(), session, machine, runtime)?;
                continue;
            }
            Event::ControllerButtonDown { which, button, .. }
                if *button == Button::Guide
                    && runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                        manager.is_active_gamepad(gamepad_event_joystick_id(*which))
                    }) =>
            {
                if runtime.menu_state.is_open() {
                    let presentation =
                        current_menu_presentation(canvas.window(), runtime, machine, session);
                    if let Some(action) = runtime
                        .menu_state
                        .handle_input(MenuInput::Cancel, presentation)
                    {
                        let mut context = FrontendActionContext {
                            session,
                            machine,
                            runtime,
                            performance_counter,
                            frame_pacer,
                            settings_store,
                        };
                        let _ = execute_menu_action(action, event_pump, canvas, &mut context)?;
                    }
                } else {
                    toggle_menu(event_pump, canvas.window(), session, machine, runtime)?;
                }
                continue;
            }
            _ => {}
        }

        if runtime.menu_state.is_open() {
            let presentation =
                current_menu_presentation(canvas.window(), runtime, machine, session);
            let menu_action = match &event {
                Event::KeyDown {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => menu_input_for_key(runtime.keyboard_bindings.menu, *keycode)
                    .and_then(|input| runtime.menu_state.handle_input(input, presentation)),
                Event::ControllerButtonDown { which, button, .. }
                    if runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                        manager.is_active_gamepad(gamepad_event_joystick_id(*which))
                    }) =>
                {
                    runtime
                        .gamepad_manager
                        .as_ref()
                        .and_then(|manager| {
                            menu_input_for_gamepad_button(manager.menu_bindings(), *button)
                        })
                        .and_then(|input| runtime.menu_state.handle_input(input, presentation))
                }
                _ => None,
            };

            if let Some(action) = menu_action {
                let mut context = FrontendActionContext {
                    session,
                    machine,
                    runtime,
                    performance_counter,
                    frame_pacer,
                    settings_store,
                };
                if let Some(signal) = execute_menu_action(action, event_pump, canvas, &mut context)?
                {
                    return Ok(signal);
                }
            }
            continue;
        }

        match event {
            Event::KeyDown {
                keycode: Some(keycode),
                scancode,
                repeat,
                ..
            } => {
                if !repeat {
                    match hotkey_action(&runtime.keyboard_bindings, keycode) {
                        HotkeyAction::None => {}
                        HotkeyAction::ManualSave => {
                            flush_runtime_save_sessions_if_changed(
                                runtime,
                                machine,
                                "manual-hotkey",
                            )?;
                        }
                        HotkeyAction::Reset => {
                            reset_machine(
                                canvas.window(),
                                session,
                                machine,
                                runtime,
                                settings_store,
                            )?;
                            let keyboard_bindings = runtime.keyboard_bindings;
                            sync_live_input_state(event_pump, &keyboard_bindings, machine, runtime);
                        }
                        HotkeyAction::ToggleFullscreen => {
                            toggle_fullscreen(canvas.window_mut())?;
                            runtime.video_options.fullscreen =
                                canvas.window().fullscreen_state() != FullscreenType::Off;
                            settings_store.set_fullscreen(runtime.video_options.fullscreen)?;
                        }
                        HotkeyAction::TogglePerformanceHud => {
                            runtime.video_options.show_performance_hud =
                                !runtime.video_options.show_performance_hud;
                            settings_store.set_show_performance_hud(
                                runtime.video_options.show_performance_hud,
                            )?;
                        }
                    }

                    if key_matches(runtime.keyboard_bindings.hotkeys.pause, keycode) {
                        runtime.paused = !runtime.paused;
                        sync_audio_playback_state(machine, runtime)?;
                    }
                }
                if let Some(button) =
                    joypad_button_for_key(runtime.keyboard_bindings.joypad, keycode)
                {
                    runtime.input_state.set_keyboard_button(
                        machine.primary_machine_mut(),
                        button,
                        true,
                    );
                }
                if let Some(scancode) = scancode
                    && let Some(button) = linked_secondary_joypad_button_for_scancode(scancode)
                    && let Some(secondary_machine) = machine.secondary_machine_mut()
                {
                    runtime.secondary_input_state.set_keyboard_button(
                        secondary_machine,
                        button,
                        true,
                    );
                }
            }
            Event::KeyUp {
                keycode: Some(keycode),
                scancode,
                repeat,
                ..
            } => {
                if repeat {
                    continue;
                }
                if let Some(button) =
                    joypad_button_for_key(runtime.keyboard_bindings.joypad, keycode)
                {
                    runtime.input_state.set_keyboard_button(
                        machine.primary_machine_mut(),
                        button,
                        false,
                    );
                }
                if let Some(scancode) = scancode
                    && let Some(button) = linked_secondary_joypad_button_for_scancode(scancode)
                    && let Some(secondary_machine) = machine.secondary_machine_mut()
                {
                    runtime.secondary_input_state.set_keyboard_button(
                        secondary_machine,
                        button,
                        false,
                    );
                }
            }
            _ => {}
        }
    }

    if runtime.menu_state.is_open() {
        sync_gamepad_rumble(runtime, machine, Instant::now())?;
        return Ok(LoopSignal::Continue);
    }

    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager
            .poll_active_gamepad_state(&mut runtime.input_state, machine.primary_machine_mut());
    }
    sync_gamepad_rumble(runtime, machine, Instant::now())?;

    Ok(LoopSignal::Continue)
}

fn step_until_next_frame(
    event_pump: &mut sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<StepUntilNextFrameResult, String> {
    let collect_frame_telemetry = context.performance_counter.emulation_profile_enabled();
    let frame_start_ly = context.machine.ppu().ly();
    let frame_start_dot = context.machine.ppu().line_dot();
    let mut current_scanline_ly = frame_start_ly;
    let mut current_scanline_t_cycles = 0usize;
    let mut at_frame_origin = frame_start_ly == 0 && frame_start_dot == 0;
    let mut previous_ly = frame_start_ly;
    let mut previous_dot = frame_start_dot;
    let profile_this_frame = context.performance_counter.should_profile_next_frame();
    let mut profile_request = None::<EmulationProfileRequest>;
    let mut pending_event_poll_duration = Duration::ZERO;
    let mut stepped_t_cycles = 0usize;
    let mut frame_origin_crossings = 0u8;
    let mut scanline_transitions = 0u16;
    let mut scanlines_over_456 = 0u16;
    let mut max_scanline_t_cycles = 0usize;
    let mut max_scanline_ly = frame_start_ly;
    let mut max_mode0_start_dot = context.machine.ppu().mode0_start_dot();
    let mut max_mode0_start_dot_ly = frame_start_ly;
    let mut ly_153_to_0_transitions = 0u8;
    let mut ly_153_to_0_startup_mode0 = 0u8;
    let mut ly_153_to_0_blank_frame = 0u8;
    let mut ly_0_self_wraps = 0u8;
    let mut ly_0_self_wrap_startup_mode0 = 0u8;
    let mut ly_0_self_wrap_blank_frame = 0u8;
    let mut ly_0_to_1_transitions = 0u8;
    let mut ly_0_scanline_t_cycles = 0usize;
    let mut ly_0_max_mode0_start_dot = if frame_start_ly == 0 {
        max_mode0_start_dot
    } else {
        0
    };
    let mut ly_0_stall_t_cycles = 0usize;
    let mut ly_0_stall_hblank_t_cycles = 0usize;
    let mut ly_0_stall_oam_t_cycles = 0usize;
    let mut ly_0_stall_drawing_t_cycles = 0usize;
    let mut ly_0_stall_startup_mode0_t_cycles = 0usize;
    let mut ly_0_stall_blank_frame_t_cycles = 0usize;
    let mut ly_0_stall_runs = 0u16;
    let mut ly_0_current_stall_run_t_cycles = 0usize;
    let mut ly_0_max_stall_run_t_cycles = 0usize;
    let mut ly_0_max_stall_dot = 0u16;
    let mut ly_0_max_stall_mode_dot = 0u16;
    let mut cpu_stop_t_cycles = 0usize;
    let mut cpu_zombie_stop_t_cycles = 0usize;
    let mut ly_0_cpu_stop_t_cycles = 0usize;
    let mut ly_0_cpu_zombie_stop_t_cycles = 0usize;
    let mut ly_0_stall_cpu_stop_t_cycles = 0usize;
    let mut ly_0_stall_cpu_zombie_stop_t_cycles = 0usize;
    let mut lcd_disabled_t_cycles = 0usize;
    let mut lcd_disable_transitions = 0u8;
    let mut lcd_enable_transitions = 0u8;
    let mut ly_0_lcd_disabled_t_cycles = 0usize;
    let mut ly_0_stall_lcd_disabled_t_cycles = 0usize;
    let mut previous_lcd_enabled = context.machine.ppu().lcd_state().is_enabled();

    loop {
        let process_events_started_at = profile_this_frame.then(Instant::now);
        let loop_signal = process_events(event_pump, canvas, context)?;
        if let Some(process_events_started_at) = process_events_started_at {
            let duration = process_events_started_at.elapsed();
            if let Some(profile_request) = &mut profile_request {
                profile_request.record_host_event_poll_duration(duration);
            } else {
                pending_event_poll_duration += duration;
            }
        }
        match loop_signal {
            LoopSignal::Continue => {}
            LoopSignal::Quit => {
                return Ok(StepUntilNextFrameResult {
                    signal: LoopSignal::Quit,
                    emulation_profile_request: None,
                    frame_loop_telemetry: FrameLoopTelemetry::default(),
                });
            }
        }
        if emulation_paused(context.machine, context.runtime) {
            return Ok(StepUntilNextFrameResult {
                signal: LoopSignal::Continue,
                emulation_profile_request: None,
                frame_loop_telemetry: FrameLoopTelemetry::default(),
            });
        }
        if profile_this_frame && profile_request.is_none() {
            let mut request = EmulationProfileRequest::new(context.machine.clone());
            request.record_host_event_poll_duration(pending_event_poll_duration);
            profile_request = Some(request);
            pending_event_poll_duration = Duration::ZERO;
        }

        for _ in 0..INPUT_POLL_SLICE_T_CYCLES {
            context.machine.step_t_cycle();
            stepped_t_cycles += 1;
            drain_printed_pages_into_printer_output(
                canvas.window(),
                context.session,
                context.runtime,
                context.machine,
            );
            context.runtime.trace_capture.is_enabled().then(|| {
                context
                    .runtime
                    .trace_capture
                    .record_t_cycle(audio_source_machine(context.machine))
            });
            context
                .runtime
                .ch4_nr43_trace
                .record_t_cycle(audio_source_machine(context.machine));

            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.capture_t_cycle(audio_source_machine(context.machine).apu());
            }
            if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
                audio_recorder.capture_t_cycle(audio_source_machine(context.machine).apu());
            }

            let rumble_result =
                sync_gamepad_rumble(context.runtime, context.machine, Instant::now());
            rumble_result?;

            let current_ly = context.machine.ppu().ly();
            let current_dot = context.machine.ppu().line_dot();
            if collect_frame_telemetry {
                let current_mode0_start_dot = context.machine.ppu().mode0_start_dot();
                let current_access_mode = context.machine.ppu().access_mode();
                let current_mode_dot = context.machine.ppu().mode_dot();
                let startup_mode0_active = context.machine.ppu().is_startup_mode0_window_active();
                let blank_frame_active = context.machine.ppu().is_blank_frame_active();
                let current_lcd_enabled = context.machine.ppu().lcd_state().is_enabled();
                let current_cpu_execution_state = context.machine.cpu().execution_state();
                current_scanline_t_cycles += 1;
                if !current_lcd_enabled {
                    lcd_disabled_t_cycles = lcd_disabled_t_cycles.saturating_add(1);
                    if current_ly == 0 {
                        ly_0_lcd_disabled_t_cycles = ly_0_lcd_disabled_t_cycles.saturating_add(1);
                    }
                }
                match (previous_lcd_enabled, current_lcd_enabled) {
                    (true, false) => {
                        lcd_disable_transitions = lcd_disable_transitions.saturating_add(1);
                    }
                    (false, true) => {
                        lcd_enable_transitions = lcd_enable_transitions.saturating_add(1);
                    }
                    _ => {}
                }
                match current_cpu_execution_state {
                    CpuExecutionState::Stopped => {
                        cpu_stop_t_cycles = cpu_stop_t_cycles.saturating_add(1);
                        if current_ly == 0 {
                            ly_0_cpu_stop_t_cycles = ly_0_cpu_stop_t_cycles.saturating_add(1);
                        }
                    }
                    CpuExecutionState::ZombieStopped => {
                        cpu_zombie_stop_t_cycles = cpu_zombie_stop_t_cycles.saturating_add(1);
                        if current_ly == 0 {
                            ly_0_cpu_zombie_stop_t_cycles =
                                ly_0_cpu_zombie_stop_t_cycles.saturating_add(1);
                        }
                    }
                    _ => {}
                }
                if current_mode0_start_dot > max_mode0_start_dot {
                    max_mode0_start_dot = current_mode0_start_dot;
                    max_mode0_start_dot_ly = current_ly;
                }
                if current_ly == 0 {
                    ly_0_max_mode0_start_dot =
                        ly_0_max_mode0_start_dot.max(current_mode0_start_dot);
                }
                if current_ly == 0 && current_ly == previous_ly && current_dot == previous_dot {
                    ly_0_stall_t_cycles = ly_0_stall_t_cycles.saturating_add(1);
                    match current_access_mode {
                        PpuAccessMode::HBlank => {
                            ly_0_stall_hblank_t_cycles =
                                ly_0_stall_hblank_t_cycles.saturating_add(1);
                        }
                        PpuAccessMode::OamScan => {
                            ly_0_stall_oam_t_cycles = ly_0_stall_oam_t_cycles.saturating_add(1);
                        }
                        PpuAccessMode::Drawing => {
                            ly_0_stall_drawing_t_cycles =
                                ly_0_stall_drawing_t_cycles.saturating_add(1);
                        }
                        PpuAccessMode::VBlank => {}
                    }
                    if startup_mode0_active {
                        ly_0_stall_startup_mode0_t_cycles =
                            ly_0_stall_startup_mode0_t_cycles.saturating_add(1);
                    }
                    if blank_frame_active {
                        ly_0_stall_blank_frame_t_cycles =
                            ly_0_stall_blank_frame_t_cycles.saturating_add(1);
                    }
                    if !current_lcd_enabled {
                        ly_0_stall_lcd_disabled_t_cycles =
                            ly_0_stall_lcd_disabled_t_cycles.saturating_add(1);
                    }
                    match current_cpu_execution_state {
                        CpuExecutionState::Stopped => {
                            ly_0_stall_cpu_stop_t_cycles =
                                ly_0_stall_cpu_stop_t_cycles.saturating_add(1);
                        }
                        CpuExecutionState::ZombieStopped => {
                            ly_0_stall_cpu_zombie_stop_t_cycles =
                                ly_0_stall_cpu_zombie_stop_t_cycles.saturating_add(1);
                        }
                        _ => {}
                    }
                    if ly_0_current_stall_run_t_cycles == 0 {
                        ly_0_stall_runs = ly_0_stall_runs.saturating_add(1);
                    }
                    ly_0_current_stall_run_t_cycles =
                        ly_0_current_stall_run_t_cycles.saturating_add(1);
                    if ly_0_current_stall_run_t_cycles > ly_0_max_stall_run_t_cycles {
                        ly_0_max_stall_run_t_cycles = ly_0_current_stall_run_t_cycles;
                        ly_0_max_stall_dot = current_dot;
                        ly_0_max_stall_mode_dot = current_mode_dot;
                    }
                } else {
                    ly_0_current_stall_run_t_cycles = 0;
                }
                if current_dot == 0 && previous_dot != 0 {
                    match (previous_ly, current_ly) {
                        (153, 0) => {
                            ly_153_to_0_transitions = ly_153_to_0_transitions.saturating_add(1);
                            if startup_mode0_active {
                                ly_153_to_0_startup_mode0 =
                                    ly_153_to_0_startup_mode0.saturating_add(1);
                            }
                            if blank_frame_active {
                                ly_153_to_0_blank_frame = ly_153_to_0_blank_frame.saturating_add(1);
                            }
                        }
                        (0, 0) => {
                            ly_0_self_wraps = ly_0_self_wraps.saturating_add(1);
                            if startup_mode0_active {
                                ly_0_self_wrap_startup_mode0 =
                                    ly_0_self_wrap_startup_mode0.saturating_add(1);
                            }
                            if blank_frame_active {
                                ly_0_self_wrap_blank_frame =
                                    ly_0_self_wrap_blank_frame.saturating_add(1);
                            }
                        }
                        (0, 1) => {
                            ly_0_to_1_transitions = ly_0_to_1_transitions.saturating_add(1);
                            ly_0_scanline_t_cycles = current_scanline_t_cycles;
                        }
                        _ => {}
                    }
                }
                if current_dot == 0 && current_ly != current_scanline_ly {
                    scanline_transitions = scanline_transitions.saturating_add(1);
                    if current_scanline_t_cycles > max_scanline_t_cycles {
                        max_scanline_t_cycles = current_scanline_t_cycles;
                        max_scanline_ly = current_scanline_ly;
                    }
                    if current_scanline_t_cycles > EXPECTED_SCANLINE_T_CYCLES {
                        scanlines_over_456 = scanlines_over_456.saturating_add(1);
                    }
                    current_scanline_ly = current_ly;
                    current_scanline_t_cycles = 0;
                }
                previous_ly = current_ly;
                previous_dot = current_dot;
                previous_lcd_enabled = current_lcd_enabled;
            }

            let now_at_frame_origin = current_ly == 0 && current_dot == 0;
            if now_at_frame_origin && !at_frame_origin {
                if collect_frame_telemetry {
                    frame_origin_crossings = frame_origin_crossings.saturating_add(1);
                }
                if context.runtime.audio_output.is_some()
                    || context.runtime.audio_recorder.is_some()
                {
                    let audio_submit_started_at = profile_request.as_ref().map(|_| Instant::now());
                    if let Some(audio_output) = &mut context.runtime.audio_output {
                        audio_output.submit_captured_samples()?;
                    }
                    if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
                        audio_recorder.write_captured_samples()?;
                    }
                    if let Some(audio_submit_started_at) = audio_submit_started_at
                        && let Some(profile_request) = &mut profile_request
                    {
                        profile_request
                            .record_host_audio_submit_duration(audio_submit_started_at.elapsed());
                    }
                }
                let save_flush_started_at = profile_request.as_ref().map(|_| Instant::now());
                maybe_flush_runtime_save_sessions_at_frame_boundary(
                    context.runtime,
                    context.machine,
                    Instant::now(),
                )?;
                if let Some(save_flush_started_at) = save_flush_started_at
                    && let Some(profile_request) = &mut profile_request
                {
                    profile_request
                        .record_host_save_flush_duration(save_flush_started_at.elapsed());
                }
                return Ok(StepUntilNextFrameResult {
                    signal: LoopSignal::Continue,
                    emulation_profile_request: profile_request,
                    frame_loop_telemetry: if collect_frame_telemetry {
                        FrameLoopTelemetry {
                            start_ly: frame_start_ly,
                            start_dot: frame_start_dot,
                            end_ly: current_ly,
                            end_dot: current_dot,
                            stepped_t_cycles,
                            frame_origin_crossings,
                            scanline_transitions,
                            scanlines_over_456,
                            max_scanline_t_cycles,
                            max_scanline_ly,
                            max_mode0_start_dot,
                            max_mode0_start_dot_ly,
                            ly_153_to_0_transitions,
                            ly_153_to_0_startup_mode0,
                            ly_153_to_0_blank_frame,
                            ly_0_self_wraps,
                            ly_0_self_wrap_startup_mode0,
                            ly_0_self_wrap_blank_frame,
                            ly_0_to_1_transitions,
                            ly_0_scanline_t_cycles,
                            ly_0_max_mode0_start_dot,
                            ly_0_stall_t_cycles,
                            ly_0_stall_hblank_t_cycles,
                            ly_0_stall_oam_t_cycles,
                            ly_0_stall_drawing_t_cycles,
                            ly_0_stall_startup_mode0_t_cycles,
                            ly_0_stall_blank_frame_t_cycles,
                            ly_0_stall_runs,
                            ly_0_max_stall_run_t_cycles,
                            ly_0_max_stall_dot,
                            ly_0_max_stall_mode_dot,
                            cpu_stop_t_cycles,
                            cpu_zombie_stop_t_cycles,
                            ly_0_cpu_stop_t_cycles,
                            ly_0_cpu_zombie_stop_t_cycles,
                            ly_0_stall_cpu_stop_t_cycles,
                            ly_0_stall_cpu_zombie_stop_t_cycles,
                            lcd_disabled_t_cycles,
                            lcd_disable_transitions,
                            lcd_enable_transitions,
                            ly_0_lcd_disabled_t_cycles,
                            ly_0_stall_lcd_disabled_t_cycles,
                        }
                    } else {
                        FrameLoopTelemetry::default()
                    },
                });
            }
            at_frame_origin = now_at_frame_origin;
        }
    }
}

fn toggle_menu(
    event_pump: &sdl3::EventPump,
    window: &Window,
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
) -> Result<(), String> {
    if runtime.menu_state.is_open() {
        close_menu(event_pump, machine, runtime)
    } else {
        open_menu(window, machine, session, runtime)
    }
}

fn process_pending_open_rom_dialog(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.open_rom_dialog.take_result() else {
        return Ok(());
    };
    let open_rom_dialog_mode = context.runtime.open_rom_dialog_mode;
    context.runtime.open_rom_dialog_mode = OpenRomDialogMode::Primary;

    match result {
        PathDialogResult::Selected(path) => {
            let open_result = match open_rom_dialog_mode {
                OpenRomDialogMode::Primary => open_selected_rom(event_pump, canvas, path, context),
                OpenRomDialogMode::LinkedSecondary => {
                    open_selected_linked_secondary_rom(event_pump, canvas, path, context)
                }
            };
            if let Err(error) = open_result {
                show_error_message(Some(canvas.window()), "Open ROM failed", &error);
                eprintln!("warning: {error}");
            }
        }
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Open ROM failed",
                &format!("failed to complete SDL3 open ROM dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 open ROM dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn process_pending_boot_rom_file_dialog(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.boot_rom_file_dialog.take_result() else {
        return Ok(());
    };

    match result {
        PathDialogResult::Selected(path) => {
            apply_machine_settings_change(canvas, context, "Boot ROM file", |config| {
                config.boot_rom.search_path = Some(path);
            })?;
        }
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Boot ROM file",
                &format!("failed to complete SDL3 boot ROM file dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 boot ROM file dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn process_pending_boot_rom_directory_dialog(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.boot_rom_directory_dialog.take_result() else {
        return Ok(());
    };

    match result {
        PathDialogResult::Selected(path) => {
            apply_machine_settings_change(canvas, context, "Boot ROM directory", |config| {
                config.boot_rom.search_path = Some(path);
            })?;
        }
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Boot ROM directory",
                &format!("failed to complete SDL3 boot ROM directory dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 boot ROM directory dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn process_pending_save_directory_dialog(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.save_directory_dialog.take_result() else {
        return Ok(());
    };

    match result {
        PathDialogResult::Selected(path) => {
            apply_machine_settings_change(canvas, context, "Save directory", |config| {
                config.saves.directory_policy = SaveDirectoryPolicy::Custom(path);
            })?;
        }
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Save directory",
                &format!("failed to complete SDL3 save directory dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 save directory dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn restore_window_after_native_dialog(canvas: &mut Canvas<Window>) {
    let _ = canvas.window_mut().raise();
}

fn apply_machine_settings_change(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
    title: &str,
    update: impl FnOnce(&mut DesktopConfig),
) -> Result<(), String> {
    let previous_config = context.session.config.clone();
    let mut next_config = previous_config.clone();
    update(&mut next_config);
    if next_config == previous_config {
        return Ok(());
    }

    let effective_config = match rebuild_machine_for_config(canvas, context, &next_config) {
        Ok(effective_config) => effective_config,
        Err(error) => {
            show_warning_message(Some(canvas.window()), title, &error);
            eprintln!("warning: {error}");
            return Ok(());
        }
    };

    context.session.config = effective_config;
    context
        .settings_store
        .persist_machine_preferences(&context.session.config)?;
    Ok(())
}

fn rebuild_machine_for_config(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
    next_config: &DesktopConfig,
) -> Result<DesktopConfig, String> {
    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let primary_battery_backed_state = uses_battery_backed_hardware_persistence(
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistence_metadata(),
    )
    .then(|| {
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistent_state()
    });
    let secondary_battery_backed_state = context.machine.secondary_machine().and_then(|machine| {
        uses_battery_backed_hardware_persistence(machine.cartridge().persistence_metadata())
            .then(|| machine.cartridge().persistent_state())
    });

    let mut previous_save_session = context.runtime.save_session.take();
    let mut previous_secondary_save_session = context.runtime.secondary_save_session.take();
    if let Some(save_session) = previous_save_session.as_mut()
        && let Err(error) = save_session.close(context.machine.primary_machine())
    {
        context.runtime.save_session = previous_save_session;
        context.runtime.secondary_save_session = previous_secondary_save_session;
        return Err(error);
    }
    if let Some(save_session) = previous_secondary_save_session.as_mut()
        && let Some(secondary_machine) = context.machine.secondary_machine()
        && let Err(error) = save_session.close(secondary_machine)
    {
        context.runtime.save_session = previous_save_session;
        context.runtime.secondary_save_session = previous_secondary_save_session;
        return Err(error);
    }

    let rebuild_result: Result<RebuildMachineResult, String> = (|| {
        let mut boot_rom_fallback_warnings = Vec::new();

        let next_session = DesktopSession {
            config: next_config.clone(),
            current_dir: context.session.current_dir.clone(),
            loaded_rom: context.session.loaded_rom.clone(),
            linked_secondary_rom: context.session.linked_secondary_rom.clone(),
            last_open_directory: context.session.last_open_directory.clone(),
            recent_roms: context.session.recent_roms.clone(),
            external_port_selection: context.session.external_port_selection,
        };

        match (
            next_session.rom_bytes(),
            next_session.linked_secondary_rom_bytes(),
            next_session.external_port_selection,
        ) {
            (
                Some(primary_rom_bytes),
                Some(secondary_rom_bytes),
                DesktopExternalPortSelection::GameLink,
            ) => {
                let primary_loaded = load_machine_for_rom(
                    next_config,
                    &context.session.current_dir,
                    primary_rom_bytes,
                )?;
                let secondary_loaded = load_machine_for_rom(
                    next_config,
                    &context.session.current_dir,
                    secondary_rom_bytes,
                )?;
                if primary_loaded.effective_config != secondary_loaded.effective_config {
                    return Err(
                        "reconfiguring a linked DMG-04 session produced divergent effective configs between primary and secondary machines"
                            .to_string(),
                    );
                }

                write_cartridge_diagnostics(&primary_loaded.diagnostics);
                write_cartridge_diagnostics(&secondary_loaded.diagnostics);
                if let Some(warning) = primary_loaded.boot_rom_fallback_warning {
                    boot_rom_fallback_warnings.push(warning);
                }
                if let Some(warning) = secondary_loaded.boot_rom_fallback_warning {
                    boot_rom_fallback_warnings.push(warning);
                }

                let mut next_machine = DesktopEmulationSession::new_single(primary_loaded.machine);
                if let Some(persistent_state) = primary_battery_backed_state
                    && let Err(error) = next_machine
                        .primary_machine_mut()
                        .restore_cartridge_persistent_state(&persistent_state)
                {
                    return Err(format!(
                        "failed to restore battery-backed persistence after reconfigure: {error:?}"
                    ));
                }

                next_machine.attach_secondary_dmg04(secondary_loaded.machine)?;
                if let Some(persistent_state) = secondary_battery_backed_state
                    && let Some(secondary_machine) = next_machine.secondary_machine_mut()
                    && let Err(error) =
                        secondary_machine.restore_cartridge_persistent_state(&persistent_state)
                {
                    return Err(format!(
                        "failed to restore linked battery-backed persistence after reconfigure: {error:?}"
                    ));
                }

                let effective_config = primary_loaded.effective_config;
                let next_primary_save_session = open_save_session_for_session(
                    &DesktopSession {
                        config: effective_config.clone(),
                        ..next_session.clone()
                    },
                    next_machine.primary_machine_mut(),
                )?;
                let next_secondary_save_session = open_secondary_save_session_for_session(
                    &DesktopSession {
                        config: effective_config.clone(),
                        ..next_session
                    },
                    next_machine
                        .secondary_machine_mut()
                        .expect("linked desktop session should expose a secondary machine"),
                )?;
                Ok((
                    effective_config,
                    boot_rom_fallback_warnings,
                    next_machine,
                    next_primary_save_session,
                    next_secondary_save_session,
                ))
            }
            (Some(rom_bytes), _, _) => {
                let loaded =
                    load_machine_for_rom(next_config, &context.session.current_dir, rom_bytes)?;
                write_cartridge_diagnostics(&loaded.diagnostics);
                if let Some(warning) = loaded.boot_rom_fallback_warning {
                    boot_rom_fallback_warnings.push(warning);
                }
                let mut next_machine = DesktopEmulationSession::new_single(loaded.machine);
                apply_external_port_selection_to_machine(
                    next_machine.primary_machine_mut(),
                    next_session.external_port_selection,
                );
                if let Some(persistent_state) = primary_battery_backed_state
                    && let Err(error) = next_machine
                        .primary_machine_mut()
                        .restore_cartridge_persistent_state(&persistent_state)
                {
                    return Err(format!(
                        "failed to restore battery-backed persistence after reconfigure: {error:?}"
                    ));
                }

                let effective_config = loaded.effective_config;
                let next_primary_save_session = open_save_session_for_session(
                    &DesktopSession {
                        config: effective_config.clone(),
                        linked_secondary_rom: None,
                        external_port_selection: next_session.external_port_selection,
                        ..next_session
                    },
                    next_machine.primary_machine_mut(),
                )?;
                Ok((
                    effective_config,
                    boot_rom_fallback_warnings,
                    next_machine,
                    next_primary_save_session,
                    None,
                ))
            }
            (None, _, _) => {
                let prepared = prepare_machine_config(next_config, &context.session.current_dir)?;
                if let Some(warning) = prepared.boot_rom_fallback_warning {
                    boot_rom_fallback_warnings.push(warning);
                }

                let mut next_machine = DesktopEmulationSession::new_single(Machine::new_summary(
                    prepared.machine_config,
                ));
                apply_external_port_selection_to_machine(
                    next_machine.primary_machine_mut(),
                    next_session.external_port_selection,
                );
                Ok((
                    prepared.effective_config,
                    boot_rom_fallback_warnings,
                    next_machine,
                    None,
                    None,
                ))
            }
        }
    })();

    let (
        effective_config,
        boot_rom_fallback_warnings,
        next_machine,
        next_save_session,
        next_secondary_save_session,
    ) = match rebuild_result {
        Ok(value) => value,
        Err(error) => {
            context.runtime.save_session = previous_save_session;
            context.runtime.secondary_save_session = previous_secondary_save_session;
            return Err(error);
        }
    };

    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.clear_buffer()?;
    }

    for warning in &boot_rom_fallback_warnings {
        log_boot_rom_fallback_warning(Some(warning));
    }
    clear_live_input_state(context.machine, context.runtime);
    *context.machine = next_machine;
    context.runtime.save_session = next_save_session;
    context.runtime.secondary_save_session = next_secondary_save_session;
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &effective_config),
    )?;
    Ok(effective_config)
}

fn boot_rom_dialog_default_location(session: &DesktopSession) -> PathBuf {
    let configured_source = session
        .config
        .boot_rom
        .search_path
        .as_deref()
        .map(|path| resolve_path(&session.current_dir, path));
    match configured_source {
        Some(path) if path.is_dir() => path,
        Some(path) => path
            .parent()
            .unwrap_or(session.current_dir.as_path())
            .to_path_buf(),
        None => session.current_dir.join(DEFAULT_BOOT_ROM_DIR),
    }
}

fn save_directory_dialog_default_location(session: &DesktopSession) -> PathBuf {
    match &session.config.saves.directory_policy {
        SaveDirectoryPolicy::Custom(path) => {
            let path = resolve_path(&session.current_dir, path);
            if path.is_dir() {
                path
            } else {
                path.parent()
                    .unwrap_or(session.current_dir.as_path())
                    .to_path_buf()
            }
        }
        SaveDirectoryPolicy::RomFolderSavesSubdir => session.rom_directory_hint().to_path_buf(),
    }
}

fn load_selected_rom(
    selected_path: PathBuf,
    session: &DesktopSession,
) -> Result<LoadedRom, String> {
    let rom_path = if selected_path.is_absolute() {
        selected_path
    } else {
        resolve_path(&session.current_dir, &selected_path)
    };
    let rom_bytes = match fs::read(&rom_path) {
        Ok(rom_bytes) => rom_bytes,
        Err(error) => {
            return Err(format_path_error(
                "failed to read ROM",
                &rom_path,
                &error.to_string(),
            ));
        }
    };

    Ok(LoadedRom {
        path: rom_path,
        bytes: rom_bytes,
    })
}

fn next_single_external_port_selection(
    current_selection: DesktopExternalPortSelection,
) -> DesktopExternalPortSelection {
    match current_selection {
        DesktopExternalPortSelection::None | DesktopExternalPortSelection::Printer => {
            current_selection
        }
        DesktopExternalPortSelection::GameLink
        | DesktopExternalPortSelection::FourPlayerAdapter => DesktopExternalPortSelection::None,
    }
}

fn open_selected_rom(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    selected_path: PathBuf,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let next_loaded_rom = load_selected_rom(selected_path, context.session)?;
    let loaded = load_machine_for_rom(
        &context.session.config,
        &context.session.current_dir,
        &next_loaded_rom.bytes,
    )?;
    log_boot_rom_fallback_warning(loaded.boot_rom_fallback_warning.as_deref());
    write_cartridge_diagnostics(&loaded.diagnostics);
    let effective_config = loaded.effective_config;
    let config_fell_back = effective_config != context.session.config;
    let mut next_machine = loaded.machine;
    let next_external_port_selection =
        next_single_external_port_selection(context.session.external_port_selection);
    apply_external_port_selection_to_machine(&mut next_machine, next_external_port_selection);
    let next_session = DesktopSession {
        config: effective_config.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: Some(next_loaded_rom),
        linked_secondary_rom: None,
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
        external_port_selection: next_external_port_selection,
    };
    let next_save_session = open_save_session_for_session(&next_session, &mut next_machine)?;

    close_runtime_save_sessions(context.runtime, context.machine)?;
    let next_console_model = next_machine.apu().console_model();

    context.session.config = effective_config;
    context.session.loaded_rom = next_session.loaded_rom;
    context.session.linked_secondary_rom = None;
    context.session.last_open_directory = context
        .session
        .loaded_rom
        .as_ref()
        .and_then(|rom| rom.path.parent().map(Path::to_path_buf));
    context.session.external_port_selection = next_external_port_selection;
    if config_fell_back {
        context
            .settings_store
            .persist_machine_preferences(&context.session.config)?;
    }
    if let Some(rom_path) = context.session.rom_path() {
        context.settings_store.remember_loaded_rom(rom_path)?;
        context.session.recent_roms = context.settings_store.recent_roms().to_vec();
    }
    clear_live_input_state(context.machine, context.runtime);
    *context.machine = DesktopEmulationSession::new_single(next_machine);
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.set_console_model(next_console_model)?;
    }
    match context.runtime.audio_recording_mode {
        DesktopAudioRecordingMode::Disabled => {
            finish_audio_recorder(&mut context.runtime.audio_recorder)?;
        }
        DesktopAudioRecordingMode::Automatic => {
            restart_automatic_audio_recorder(context.runtime, context.session, context.machine)?;
        }
        DesktopAudioRecordingMode::Explicit(_) => {
            if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
                audio_recorder.set_console_model(next_console_model)?;
            }
        }
    }
    context.runtime.save_session = next_save_session;
    context.runtime.secondary_save_session = None;
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;
    context.runtime.paused = false;

    if context.runtime.menu_state.is_open() {
        close_menu(event_pump, context.machine, context.runtime)?;
    }

    Ok(())
}

fn open_selected_linked_secondary_rom(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    selected_path: PathBuf,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    if !context.session.has_loaded_rom() {
        return Err(
            "GAME LINK requires a primary ROM before selecting a second cartridge".to_string(),
        );
    }

    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let Some(primary_rom_bytes) = context.session.rom_bytes() else {
        return Err(
            "GAME LINK requires a primary ROM before selecting a second cartridge".to_string(),
        );
    };
    let primary_battery_backed_state = uses_battery_backed_hardware_persistence(
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistence_metadata(),
    )
    .then(|| {
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistent_state()
    });

    let next_secondary_rom = load_selected_rom(selected_path, context.session)?;
    let loaded_primary = load_machine_for_rom(
        &context.session.config,
        &context.session.current_dir,
        primary_rom_bytes,
    )?;
    let loaded_secondary = load_machine_for_rom(
        &context.session.config,
        &context.session.current_dir,
        &next_secondary_rom.bytes,
    )?;
    if loaded_primary.effective_config != loaded_secondary.effective_config {
        return Err(
            "activating GAME LINK produced divergent effective configs between primary and secondary machines"
                .to_string(),
        );
    }

    log_boot_rom_fallback_warning(loaded_primary.boot_rom_fallback_warning.as_deref());
    write_cartridge_diagnostics(&loaded_primary.diagnostics);
    log_boot_rom_fallback_warning(loaded_secondary.boot_rom_fallback_warning.as_deref());
    write_cartridge_diagnostics(&loaded_secondary.diagnostics);

    let effective_config = loaded_primary.effective_config;
    let config_fell_back = effective_config != context.session.config;
    let mut next_machine = DesktopEmulationSession::new_single(loaded_primary.machine);
    if let Some(persistent_state) = primary_battery_backed_state
        && let Err(error) = next_machine
            .primary_machine_mut()
            .restore_cartridge_persistent_state(&persistent_state)
    {
        return Err(format!(
            "failed to restore battery-backed persistence while activating GAME LINK: {error:?}"
        ));
    }
    next_machine.attach_secondary_dmg04(loaded_secondary.machine)?;

    let next_session = DesktopSession {
        config: effective_config.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: context.session.loaded_rom.clone(),
        linked_secondary_rom: Some(next_secondary_rom),
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
        external_port_selection: DesktopExternalPortSelection::GameLink,
    };
    let next_primary_save_session =
        open_save_session_for_session(&next_session, next_machine.primary_machine_mut())?;
    let next_secondary_save_session = open_secondary_save_session_for_session(
        &next_session,
        next_machine
            .secondary_machine_mut()
            .expect("linked desktop session should expose a secondary machine"),
    )?;

    close_runtime_save_sessions(context.runtime, context.machine)?;
    let next_console_model = next_machine.primary_machine().apu().console_model();

    context.session.config = effective_config;
    context.session.linked_secondary_rom = next_session.linked_secondary_rom;
    context.session.last_open_directory = context
        .session
        .linked_secondary_rom
        .as_ref()
        .and_then(|rom| rom.path.parent().map(Path::to_path_buf));
    context.session.external_port_selection = DesktopExternalPortSelection::GameLink;
    if config_fell_back {
        context
            .settings_store
            .persist_machine_preferences(&context.session.config)?;
    }
    clear_live_input_state(context.machine, context.runtime);
    *context.machine = next_machine;
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.set_console_model(next_console_model)?;
    }
    if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
        audio_recorder.set_console_model(next_console_model)?;
    }
    context.runtime.save_session = next_primary_save_session;
    context.runtime.secondary_save_session = next_secondary_save_session;
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;

    if context.runtime.menu_state.is_open() {
        close_menu(event_pump, context.machine, context.runtime)?;
    }

    Ok(())
}

fn open_menu(
    window: &Window,
    machine: &mut DesktopEmulationSession,
    session: &DesktopSession,
    runtime: &mut FrontendRuntime,
) -> Result<(), String> {
    runtime
        .menu_state
        .open(current_menu_presentation(window, runtime, machine, session));
    clear_live_input_state(machine, runtime);
    sync_audio_playback_state(machine, runtime)
}

fn close_menu(
    event_pump: &sdl3::EventPump,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
) -> Result<(), String> {
    runtime.menu_state.close();
    let keyboard_bindings = runtime.keyboard_bindings;
    sync_live_input_state(event_pump, &keyboard_bindings, machine, runtime);
    sync_audio_playback_state(machine, runtime)
}

fn execute_menu_action(
    action: MenuAction,
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<Option<LoopSignal>, String> {
    match action {
        MenuAction::Resume => {
            context.runtime.paused = false;
            close_menu(event_pump, context.machine, context.runtime)?;
            Ok(None)
        }
        MenuAction::OpenRom => {
            let default_location = context.session.rom_directory_hint();
            context.runtime.open_rom_dialog_mode = OpenRomDialogMode::Primary;
            if let Err(error) = context.runtime.open_rom_dialog.show_file(
                &ROM_FILE_DIALOG_FILTERS,
                canvas.window(),
                default_location,
            ) {
                show_warning_message(Some(canvas.window()), "Open ROM", &error);
                eprintln!("warning: {error}");
            }
            Ok(None)
        }
        MenuAction::CycleConsoleModel => {
            apply_machine_settings_change(canvas, context, "Console model", |config| {
                config.launch.console_model = next_console_model(config.launch.console_model);
            })?;
            Ok(None)
        }
        MenuAction::CycleStartupMode => {
            apply_machine_settings_change(canvas, context, "Startup mode", |config| {
                config.launch.startup_mode = next_startup_mode(config.launch.startup_mode);
            })?;
            Ok(None)
        }
        MenuAction::CycleExecutionMode => {
            apply_machine_settings_change(canvas, context, "Execution mode", |config| {
                config.launch.execution_mode = next_execution_mode(config.launch.execution_mode);
            })?;
            Ok(None)
        }
        MenuAction::ClearBootRomPath => {
            apply_machine_settings_change(canvas, context, "Boot ROM path", |config| {
                config.boot_rom.search_path = None;
            })?;
            Ok(None)
        }
        MenuAction::SelectBootRomFilePath => {
            let default_location = boot_rom_dialog_default_location(context.session);
            if let Err(error) = context.runtime.boot_rom_file_dialog.show_file(
                &BOOT_ROM_FILE_DIALOG_FILTERS,
                canvas.window(),
                &default_location,
            ) {
                show_warning_message(Some(canvas.window()), "Boot ROM file", &error);
                eprintln!("warning: {error}");
            }
            Ok(None)
        }
        MenuAction::SelectBootRomDirectoryPath => {
            let default_location = boot_rom_dialog_default_location(context.session);
            context
                .runtime
                .boot_rom_directory_dialog
                .show_folder(canvas.window(), &default_location);
            Ok(None)
        }
        MenuAction::CycleBootRomVerify => {
            apply_machine_settings_change(canvas, context, "Boot ROM verification", |config| {
                config.boot_rom.verification =
                    next_boot_rom_verification_mode(config.boot_rom.verification);
            })?;
            Ok(None)
        }
        MenuAction::ToggleSavesEnabled => {
            apply_machine_settings_change(canvas, context, "Save support", |config| {
                config.saves.enabled = !config.saves.enabled;
            })?;
            Ok(None)
        }
        MenuAction::CycleSavePolicy => {
            apply_machine_settings_change(canvas, context, "Save policy", |config| {
                config.saves.flush_policy = next_save_flush_policy(config.saves.flush_policy);
            })?;
            Ok(None)
        }
        MenuAction::ClearSaveDirectoryPath => {
            apply_machine_settings_change(canvas, context, "Save directory", |config| {
                config.saves.directory_policy = SaveDirectoryPolicy::RomFolderSavesSubdir;
            })?;
            Ok(None)
        }
        MenuAction::SelectSaveDirectoryPath => {
            let default_location = save_directory_dialog_default_location(context.session);
            context
                .runtime
                .save_directory_dialog
                .show_folder(canvas.window(), &default_location);
            Ok(None)
        }
        MenuAction::OpenRecentRom(index) => {
            let Some(rom_path) = context.session.recent_roms().get(index).cloned() else {
                return Ok(None);
            };
            if !rom_path.exists() {
                context.settings_store.remove_recent_rom(&rom_path)?;
                context.session.recent_roms = context.settings_store.recent_roms().to_vec();
                let error = format!("recent ROM no longer exists: {}", rom_path.display());
                show_warning_message(Some(canvas.window()), "Open Recent", &error);
                eprintln!("warning: {error}");
                return Ok(None);
            }

            if let Err(error) = open_selected_rom(event_pump, canvas, rom_path, context) {
                show_warning_message(Some(canvas.window()), "Open Recent", &error);
                eprintln!("warning: {error}");
            }
            Ok(None)
        }
        MenuAction::ClearRecentList => {
            context.settings_store.clear_recent_roms()?;
            context.session.recent_roms = context.settings_store.recent_roms().to_vec();
            context.runtime.menu_state.open(current_menu_presentation(
                canvas.window(),
                context.runtime,
                context.machine,
                context.session,
            ));
            Ok(None)
        }
        MenuAction::SaveBattery => {
            flush_runtime_save_sessions_if_changed(context.runtime, context.machine, "menu")?;
            Ok(None)
        }
        MenuAction::SaveScreenshot => {
            match save_screenshot_for_session(
                context.session,
                context.machine,
                &context.runtime.video_options,
            ) {
                Ok(path) => {
                    eprintln!("info: screenshot saved to {}", path.display());
                }
                Err(error) => {
                    show_warning_message(Some(canvas.window()), "Screenshot", &error);
                    eprintln!("warning: {error}");
                }
            }
            Ok(None)
        }
        MenuAction::ToggleFullscreen => {
            toggle_fullscreen(canvas.window_mut())?;
            context.runtime.video_options.fullscreen =
                canvas.window().fullscreen_state() != FullscreenType::Off;
            if canvas.window().fullscreen_state() == FullscreenType::Off {
                apply_window_scale_for_dimensions(
                    canvas.window_mut(),
                    context.runtime.video_options.window_scale,
                    framebuffer_dimensions_for_session(context.machine),
                )?;
            }
            context
                .settings_store
                .set_fullscreen(context.runtime.video_options.fullscreen)?;
            Ok(None)
        }
        MenuAction::ToggleVsync => {
            context.runtime.video_options.vsync = !context.runtime.video_options.vsync;
            apply_renderer_vsync(
                canvas,
                context.frame_pacer,
                context.runtime.video_options.vsync,
            )?;
            context
                .settings_store
                .set_vsync(context.runtime.video_options.vsync)?;
            Ok(None)
        }
        MenuAction::CycleWindowScale => {
            context.runtime.video_options.window_scale =
                next_window_scale(context.runtime.video_options.window_scale);
            if canvas.window().fullscreen_state() == FullscreenType::Off {
                apply_window_scale_for_dimensions(
                    canvas.window_mut(),
                    context.runtime.video_options.window_scale,
                    framebuffer_dimensions_for_session(context.machine),
                )?;
            }
            context
                .settings_store
                .set_window_scale(context.runtime.video_options.window_scale)?;
            Ok(None)
        }
        MenuAction::ToggleIntegerScale => {
            context.runtime.video_options.integer_scale =
                !context.runtime.video_options.integer_scale;
            context
                .settings_store
                .set_integer_scale(context.runtime.video_options.integer_scale)?;
            Ok(None)
        }
        MenuAction::TogglePresentationFilter => {
            context.runtime.video_options.presentation_filter =
                !context.runtime.video_options.presentation_filter;
            context
                .settings_store
                .set_presentation_filter(context.runtime.video_options.presentation_filter)?;
            Ok(None)
        }
        MenuAction::ToggleBackgroundLayer => {
            context.runtime.video_options.show_background =
                !context.runtime.video_options.show_background;
            context
                .settings_store
                .set_show_background(context.runtime.video_options.show_background)?;
            Ok(None)
        }
        MenuAction::ToggleWindowLayer => {
            context.runtime.video_options.show_window = !context.runtime.video_options.show_window;
            context
                .settings_store
                .set_show_window(context.runtime.video_options.show_window)?;
            Ok(None)
        }
        MenuAction::ToggleObjectLayer => {
            context.runtime.video_options.show_objects =
                !context.runtime.video_options.show_objects;
            context
                .settings_store
                .set_show_objects(context.runtime.video_options.show_objects)?;
            Ok(None)
        }
        MenuAction::TogglePerformanceHud => {
            context.runtime.video_options.show_performance_hud =
                !context.runtime.video_options.show_performance_hud;
            context
                .settings_store
                .set_show_performance_hud(context.runtime.video_options.show_performance_hud)?;
            Ok(None)
        }
        MenuAction::ResetVideoDefaults => {
            let defaults = VideoOptions::default();
            context.runtime.video_options = defaults.clone();
            apply_renderer_vsync(canvas, context.frame_pacer, defaults.vsync)?;
            set_fullscreen_state(canvas.window_mut(), defaults.fullscreen)?;
            if canvas.window().fullscreen_state() == FullscreenType::Off {
                apply_window_scale_for_dimensions(
                    canvas.window_mut(),
                    defaults.window_scale,
                    framebuffer_dimensions_for_session(context.machine),
                )?;
            }
            context.settings_store.reset_video_defaults()?;
            Ok(None)
        }
        MenuAction::ToggleMute => {
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.set_muted(!audio_output.is_muted())?;
                context
                    .settings_store
                    .set_audio_muted(audio_output.is_muted())?;
            }
            Ok(None)
        }
        MenuAction::CycleAudioVolume => {
            context.runtime.audio_volume_percent =
                next_audio_volume_percent(context.runtime.audio_volume_percent);
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.set_volume_percent(context.runtime.audio_volume_percent)?;
            }
            context
                .settings_store
                .set_audio_volume_percent(context.runtime.audio_volume_percent)?;
            Ok(None)
        }
        MenuAction::ToggleAudioRecording => {
            if matches!(
                context.runtime.audio_recording_mode,
                DesktopAudioRecordingMode::Disabled
            ) {
                let recording_mode = DesktopAudioRecordingMode::Automatic;
                context.runtime.audio_recorder = create_audio_recorder(
                    &recording_mode,
                    context.runtime.audio_channel_mask,
                    context.session,
                    context.machine,
                )?;
                context.runtime.audio_recording_mode = recording_mode;
            } else {
                finish_audio_recorder(&mut context.runtime.audio_recorder)?;
                context.runtime.audio_recording_mode = DesktopAudioRecordingMode::Disabled;
            }
            Ok(None)
        }
        MenuAction::ToggleAudioChannel(channel) => {
            context.runtime.audio_channel_mask =
                context.runtime.audio_channel_mask.toggled(channel);
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.set_channel_mask(context.runtime.audio_channel_mask)?;
            }
            if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
                audio_recorder.set_channel_mask(context.runtime.audio_channel_mask)?;
            }
            Ok(None)
        }
        MenuAction::ResetAudioDefaults => {
            let defaults = gb_desktop::AudioOptions::default();
            context.runtime.audio_volume_percent = defaults.volume_percent;
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.set_muted(false)?;
                audio_output.set_volume_percent(defaults.volume_percent)?;
                audio_output.set_channel_mask(ApuRecordedChannelMask::ALL)?;
            }
            finish_audio_recorder(&mut context.runtime.audio_recorder)?;
            context.runtime.audio_recording_mode = DesktopAudioRecordingMode::Disabled;
            context.runtime.audio_channel_mask = ApuRecordedChannelMask::ALL;
            context.settings_store.reset_audio_defaults()?;
            Ok(None)
        }
        MenuAction::SetExternalPort(selection) => {
            drain_printed_pages_into_printer_output(
                canvas.window(),
                context.session,
                context.runtime,
                context.machine,
            );
            flush_pending_printer_output(canvas.window(), context.session, context.runtime);
            match selection {
                DesktopExternalPortSelection::GameLink => {
                    if !context.session.has_loaded_rom() {
                        return Ok(None);
                    }

                    context.runtime.open_rom_dialog_mode = OpenRomDialogMode::LinkedSecondary;
                    let default_location = context.session.rom_directory_hint();
                    if let Err(error) = context.runtime.open_rom_dialog.show_file(
                        &ROM_FILE_DIALOG_FILTERS,
                        canvas.window(),
                        default_location,
                    ) {
                        context.runtime.open_rom_dialog_mode = OpenRomDialogMode::Primary;
                        show_warning_message(Some(canvas.window()), "GAME LINK", &error);
                        eprintln!("warning: {error}");
                    }
                }
                DesktopExternalPortSelection::None | DesktopExternalPortSelection::Printer => {
                    if context.machine.is_linked_dmg04_two_player() {
                        if let Some(save_session) = &mut context.runtime.secondary_save_session
                            && let Some(secondary_machine) = context.machine.secondary_machine()
                        {
                            save_session.close(secondary_machine)?;
                        }
                        context.machine.detach_to_single_primary();
                    }

                    context.runtime.secondary_save_session = None;
                    context.session.linked_secondary_rom = None;
                    context.session.external_port_selection = selection;
                    apply_external_port_selection_to_machine(
                        context.machine.primary_machine_mut(),
                        selection,
                    );
                    context.performance_counter.reset_base_title(
                        canvas.window_mut(),
                        window_title(context.session, &context.session.config),
                    )?;
                    context.runtime.rtc_sync.resync_to_host_clock();
                }
                DesktopExternalPortSelection::FourPlayerAdapter => {}
            }
            Ok(None)
        }
        MenuAction::CycleGamepadDirectionalSource => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let next_directional_source =
                    next_gamepad_directional_source(gamepad_manager.directional_source());
                gamepad_manager.set_directional_source(
                    next_directional_source,
                    &mut context.runtime.input_state,
                    context.machine,
                );
                clear_live_input_state(context.machine, context.runtime);
                context
                    .settings_store
                    .set_gamepad_directional_source(next_directional_source)?;
            }
            Ok(None)
        }
        MenuAction::CycleGamepadRumbleMode => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let next_rumble_mode = next_gamepad_rumble_mode(gamepad_manager.rumble_mode());
                gamepad_manager.set_rumble_mode(next_rumble_mode);
                context
                    .settings_store
                    .set_gamepad_rumble_mode(next_rumble_mode)?;
                sync_gamepad_rumble(context.runtime, context.machine, Instant::now())?;
            }
            Ok(None)
        }
        MenuAction::TogglePreferredGamepad => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let preferred_device = toggled_preferred_gamepad_device(gamepad_manager);
                gamepad_manager.set_preferred_device(
                    preferred_device.clone(),
                    &mut context.runtime.input_state,
                    context.machine,
                );
                context
                    .settings_store
                    .set_preferred_gamepad_device(preferred_device)?;
            }
            Ok(None)
        }
        MenuAction::SetKeyboardBinding(target, key) => {
            assign_keyboard_binding(&mut context.runtime.keyboard_bindings, target, key);
            match target {
                KeyboardBindingTarget::Up
                | KeyboardBindingTarget::Down
                | KeyboardBindingTarget::Left
                | KeyboardBindingTarget::Right
                | KeyboardBindingTarget::A
                | KeyboardBindingTarget::B
                | KeyboardBindingTarget::Select
                | KeyboardBindingTarget::Start => {
                    context
                        .settings_store
                        .set_keyboard_joypad_bindings(context.runtime.keyboard_bindings.joypad)?;
                }
                KeyboardBindingTarget::Pause
                | KeyboardBindingTarget::Reset
                | KeyboardBindingTarget::ToggleFullscreen
                | KeyboardBindingTarget::TogglePerformanceHud
                | KeyboardBindingTarget::SaveBattery => {
                    context
                        .settings_store
                        .set_keyboard_hotkey_bindings(context.runtime.keyboard_bindings.hotkeys)?;
                }
            }
            Ok(None)
        }
        MenuAction::ResetInputDefaults => {
            let defaults = gb_desktop::InputOptions::default();
            context.runtime.keyboard_bindings = defaults.keyboard;
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                gamepad_manager.set_button_bindings(
                    defaults.gamepad.bindings,
                    &mut context.runtime.input_state,
                    context.machine,
                );
                gamepad_manager.set_menu_bindings(defaults.gamepad.menu);
                gamepad_manager.set_directional_source(
                    defaults.gamepad.directional_source,
                    &mut context.runtime.input_state,
                    context.machine,
                );
                gamepad_manager.set_rumble_mode(defaults.gamepad.rumble_mode);
                gamepad_manager.set_preferred_device(
                    defaults.gamepad.preferred_device,
                    &mut context.runtime.input_state,
                    context.machine,
                );
            }
            context.settings_store.reset_input_defaults()?;
            sync_gamepad_rumble(context.runtime, context.machine, Instant::now())?;
            Ok(None)
        }
        MenuAction::SetKeyboardMenuBinding(target, key) => {
            assign_keyboard_menu_binding(&mut context.runtime.keyboard_bindings.menu, target, key);
            context
                .settings_store
                .set_keyboard_menu_bindings(context.runtime.keyboard_bindings.menu)?;
            Ok(None)
        }
        MenuAction::SetGamepadBinding(target, binding) => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let mut bindings = gamepad_manager.button_bindings();
                assign_gamepad_binding(&mut bindings, target, binding);
                gamepad_manager.set_button_bindings(
                    bindings,
                    &mut context.runtime.input_state,
                    context.machine,
                );
                context.settings_store.set_gamepad_bindings(bindings)?;
            }
            Ok(None)
        }
        MenuAction::SetGamepadMenuBinding(target, binding) => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let mut bindings = gamepad_manager.menu_bindings();
                assign_gamepad_menu_binding(&mut bindings, target, binding);
                gamepad_manager.set_menu_bindings(bindings);
                context.settings_store.set_gamepad_menu_bindings(bindings)?;
            }
            Ok(None)
        }
        MenuAction::Reset => {
            reset_machine(
                canvas.window(),
                context.session,
                context.machine,
                context.runtime,
                context.settings_store,
            )?;
            close_menu(event_pump, context.machine, context.runtime)?;
            Ok(None)
        }
        MenuAction::Quit => {
            flush_pending_printer_output(canvas.window(), context.session, context.runtime);
            Ok(Some(LoopSignal::Quit))
        }
    }
}

fn current_menu_presentation(
    window: &Window,
    runtime: &FrontendRuntime,
    machine: &Machine<TraceSummaryBuffer>,
    session: &DesktopSession,
) -> MenuPresentation {
    let gamepad_available = runtime.gamepad_manager.is_some();
    let active_gamepad_label = runtime
        .gamepad_manager
        .as_ref()
        .and_then(GamepadManager::active_gamepad_name)
        .map(CompactMenuLabel::from_gamepad_name)
        .unwrap_or_default();
    let cartridge_rumble_supported = machine.cartridge().has_rumble();
    let preferred_gamepad_configured = runtime
        .gamepad_manager
        .as_ref()
        .is_some_and(|manager| manager.preferred_device().is_configured());
    let preferred_gamepad_label = runtime
        .gamepad_manager
        .as_ref()
        .and_then(GamepadManager::preferred_device_name)
        .map(CompactMenuLabel::from_gamepad_name)
        .unwrap_or(if preferred_gamepad_configured {
            CompactMenuLabel::from_text("SAVED")
        } else {
            CompactMenuLabel::default()
        });
    let mut recent_rom_labels = [CompactRecentRomLabel::default(); RECENT_ROM_MENU_CAPACITY];
    for (slot, rom_path) in session
        .recent_roms()
        .iter()
        .take(RECENT_ROM_MENU_CAPACITY)
        .enumerate()
    {
        recent_rom_labels[slot] = compact_recent_rom_label(rom_path);
    }

    MenuPresentation {
        rom_loaded: !machine.cartridge().is_empty(),
        recent_rom_count: session.recent_roms().len().min(RECENT_ROM_MENU_CAPACITY) as u8,
        recent_rom_labels,
        console_model: session.config.launch.console_model,
        startup_mode: session.config.launch.startup_mode,
        execution_mode: session.config.launch.execution_mode,
        external_port_selection: session.external_port_selection,
        boot_rom_uses_default_path: session.config.boot_rom.search_path.is_none(),
        boot_rom_verification: session.config.boot_rom.verification,
        saves_enabled: session.config.saves.enabled,
        save_flush_policy: session.config.saves.flush_policy,
        save_directory_uses_default_path: match &session.config.saves.directory_policy {
            SaveDirectoryPolicy::RomFolderSavesSubdir => true,
            SaveDirectoryPolicy::Custom(_) => false,
        },
        fullscreen: window.fullscreen_state() != FullscreenType::Off,
        vsync: runtime.video_options.vsync,
        window_scale: runtime.video_options.window_scale.max(1),
        integer_scale: runtime.video_options.integer_scale,
        presentation_filter: runtime.video_options.presentation_filter,
        show_background: runtime.video_options.show_background,
        show_window: runtime.video_options.show_window,
        show_objects: runtime.video_options.show_objects,
        show_performance_hud: runtime.video_options.show_performance_hud,
        muted: runtime
            .audio_output
            .as_ref()
            .is_some_and(DesktopAudioOutput::is_muted),
        audio_available: runtime.audio_output.is_some(),
        audio_volume_percent: runtime.audio_volume_percent.min(100),
        audio_recording_enabled: !matches!(
            runtime.audio_recording_mode,
            DesktopAudioRecordingMode::Disabled
        ),
        ch1_enabled: runtime.audio_channel_mask.contains(ApuRecordedChannel::Ch1),
        ch2_enabled: runtime.audio_channel_mask.contains(ApuRecordedChannel::Ch2),
        ch3_enabled: runtime.audio_channel_mask.contains(ApuRecordedChannel::Ch3),
        ch4_enabled: runtime.audio_channel_mask.contains(ApuRecordedChannel::Ch4),
        manual_save_available: runtime
            .save_session
            .as_ref()
            .is_some_and(|session| session.flush_policy() == DesktopSaveFlushPolicy::Manual)
            || runtime
                .secondary_save_session
                .as_ref()
                .is_some_and(|session| session.flush_policy() == DesktopSaveFlushPolicy::Manual),
        any_dialog_pending: runtime.any_dialog_pending(),
        gamepad_available,
        gamepad_directional_source: runtime.gamepad_manager.as_ref().map_or(
            GamepadDirectionalSource::default(),
            GamepadManager::directional_source,
        ),
        gamepad_rumble_mode: runtime
            .gamepad_manager
            .as_ref()
            .map_or(GamepadRumbleMode::default(), GamepadManager::rumble_mode),
        gamepad_bindings: runtime.gamepad_manager.as_ref().map_or(
            GamepadButtonBindings::default(),
            GamepadManager::button_bindings,
        ),
        gamepad_menu_bindings: runtime.gamepad_manager.as_ref().map_or(
            GamepadMenuBindings::default(),
            GamepadManager::menu_bindings,
        ),
        active_gamepad_connected: runtime
            .gamepad_manager
            .as_ref()
            .is_some_and(GamepadManager::has_connected_gamepad),
        cartridge_rumble_supported,
        active_gamepad_rumble_supported: runtime
            .gamepad_manager
            .as_ref()
            .is_some_and(GamepadManager::active_gamepad_has_rumble),
        active_gamepad_label,
        preferred_gamepad_configured,
        preferred_gamepad_label,
        keyboard_bindings: runtime.keyboard_bindings.joypad,
        keyboard_menu_bindings: runtime.keyboard_bindings.menu,
        hotkey_bindings: runtime.keyboard_bindings.hotkeys,
    }
}

fn next_console_model(console_model: DesktopConsoleModel) -> DesktopConsoleModel {
    match console_model {
        DesktopConsoleModel::Dmg0 => DesktopConsoleModel::Dmg,
        DesktopConsoleModel::Dmg => DesktopConsoleModel::Mgb,
        DesktopConsoleModel::Mgb => DesktopConsoleModel::Dmg0,
    }
}

fn next_startup_mode(startup_mode: StartupMode) -> StartupMode {
    match startup_mode {
        StartupMode::SkipBoot => StartupMode::RealBoot,
        StartupMode::RealBoot => StartupMode::SkipBoot,
    }
}

fn next_execution_mode(execution_mode: ExecutionMode) -> ExecutionMode {
    match execution_mode {
        ExecutionMode::Strict => ExecutionMode::Permissive,
        ExecutionMode::Permissive => ExecutionMode::Experimental,
        ExecutionMode::Experimental => ExecutionMode::Strict,
    }
}

fn next_boot_rom_verification_mode(
    verification_mode: BootRomVerificationMode,
) -> BootRomVerificationMode {
    match verification_mode {
        BootRomVerificationMode::Strict => BootRomVerificationMode::Warn,
        BootRomVerificationMode::Warn => BootRomVerificationMode::Off,
        BootRomVerificationMode::Off => BootRomVerificationMode::Strict,
    }
}

fn next_save_flush_policy(flush_policy: DesktopSaveFlushPolicy) -> DesktopSaveFlushPolicy {
    match flush_policy {
        DesktopSaveFlushPolicy::Manual => DesktopSaveFlushPolicy::OnClose,
        DesktopSaveFlushPolicy::OnClose => DesktopSaveFlushPolicy::OnWrite,
        DesktopSaveFlushPolicy::OnWrite => DesktopSaveFlushPolicy::Debounced,
        DesktopSaveFlushPolicy::Debounced => DesktopSaveFlushPolicy::Manual,
    }
}

fn next_gamepad_directional_source(
    directional_source: GamepadDirectionalSource,
) -> GamepadDirectionalSource {
    match directional_source {
        GamepadDirectionalSource::DpadOnly => GamepadDirectionalSource::LeftStickOnly,
        GamepadDirectionalSource::LeftStickOnly => GamepadDirectionalSource::DpadAndLeftStick,
        GamepadDirectionalSource::DpadAndLeftStick => GamepadDirectionalSource::DpadOnly,
    }
}

fn next_gamepad_rumble_mode(rumble_mode: GamepadRumbleMode) -> GamepadRumbleMode {
    match rumble_mode {
        GamepadRumbleMode::Off => GamepadRumbleMode::Strong,
        GamepadRumbleMode::Strong => GamepadRumbleMode::Weak,
        GamepadRumbleMode::Weak => GamepadRumbleMode::Off,
    }
}

fn next_window_scale(current_scale: u8) -> u8 {
    match current_scale {
        1..=7 => current_scale + 1,
        _ => 1,
    }
}

fn next_audio_volume_percent(current_volume_percent: u8) -> u8 {
    match current_volume_percent {
        0..=24 => 25,
        25..=49 => 50,
        50..=74 => 75,
        75..=99 => 100,
        _ => 25,
    }
}

fn compact_recent_rom_label(path: &Path) -> CompactRecentRomLabel {
    let stem = path
        .file_stem()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let trimmed = stem
        .split(['(', '['])
        .next()
        .unwrap_or(stem.as_ref())
        .trim();
    let mut compact = String::new();
    let mut pending_space = false;
    for character in trimmed.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_space && !compact.is_empty() {
                compact.push(' ');
            }
            compact.push(character.to_ascii_uppercase());
            pending_space = false;
        } else if !compact.is_empty() {
            pending_space = true;
        }
    }

    if compact.is_empty() {
        CompactRecentRomLabel::from_text("ROM")
    } else {
        CompactRecentRomLabel::from_text(&compact)
    }
}

fn toggled_preferred_gamepad_device(gamepad_manager: &GamepadManager) -> PreferredGamepadIdentity {
    if gamepad_manager.preferred_device().is_configured()
        && !gamepad_manager.has_connected_gamepad()
    {
        return PreferredGamepadIdentity::default();
    }

    if gamepad_manager.active_matches_preferred() {
        return PreferredGamepadIdentity::default();
    }

    gamepad_manager
        .active_gamepad_identity()
        .unwrap_or_default()
}

fn assign_keyboard_binding(
    bindings: &mut KeyboardBindings,
    target: KeyboardBindingTarget,
    key: DesktopKey,
) {
    let previous_key = keyboard_binding_value(*bindings, target);
    if previous_key == key {
        return;
    }

    let other_target = match target {
        KeyboardBindingTarget::Up
        | KeyboardBindingTarget::Down
        | KeyboardBindingTarget::Left
        | KeyboardBindingTarget::Right
        | KeyboardBindingTarget::A
        | KeyboardBindingTarget::B
        | KeyboardBindingTarget::Select
        | KeyboardBindingTarget::Start => joypad_binding_target_for_key(bindings.joypad, key),
        KeyboardBindingTarget::Pause
        | KeyboardBindingTarget::Reset
        | KeyboardBindingTarget::ToggleFullscreen
        | KeyboardBindingTarget::TogglePerformanceHud
        | KeyboardBindingTarget::SaveBattery => {
            hotkey_binding_target_for_key(bindings.hotkeys, key)
        }
    };

    if let Some(other_target) = other_target {
        set_keyboard_binding_value(bindings, other_target, previous_key);
    }
    set_keyboard_binding_value(bindings, target, key);
}

fn assign_keyboard_menu_binding(
    bindings: &mut MenuKeyboardBindings,
    target: KeyboardMenuBindingTarget,
    key: DesktopKey,
) {
    let previous_key = keyboard_menu_binding_value(*bindings, target);
    if previous_key == key {
        return;
    }

    if let Some(other_target) = keyboard_menu_binding_target_for_key(*bindings, key) {
        set_keyboard_menu_binding_value(bindings, other_target, previous_key);
    }
    set_keyboard_menu_binding_value(bindings, target, key);
}

fn assign_gamepad_binding(
    bindings: &mut GamepadButtonBindings,
    target: GamepadBindingTarget,
    binding: GamepadButtonBinding,
) {
    let previous_binding = gamepad_binding_value(*bindings, target);
    if previous_binding == binding {
        return;
    }

    if let Some(other_target) = gamepad_binding_target_for_binding(*bindings, binding) {
        set_gamepad_binding_value(bindings, other_target, previous_binding);
    }
    set_gamepad_binding_value(bindings, target, binding);
}

fn assign_gamepad_menu_binding(
    bindings: &mut GamepadMenuBindings,
    target: GamepadMenuBindingTarget,
    binding: GamepadButtonBinding,
) {
    let previous_binding = gamepad_menu_binding_value(*bindings, target);
    if previous_binding == binding {
        return;
    }

    if let Some(other_target) = gamepad_menu_binding_target_for_binding(*bindings, binding) {
        set_gamepad_menu_binding_value(bindings, other_target, previous_binding);
    }
    set_gamepad_menu_binding_value(bindings, target, binding);
}

fn gamepad_binding_target_for_binding(
    bindings: GamepadButtonBindings,
    binding: GamepadButtonBinding,
) -> Option<GamepadBindingTarget> {
    [
        GamepadBindingTarget::Up,
        GamepadBindingTarget::Down,
        GamepadBindingTarget::Left,
        GamepadBindingTarget::Right,
        GamepadBindingTarget::A,
        GamepadBindingTarget::B,
        GamepadBindingTarget::Select,
        GamepadBindingTarget::Start,
    ]
    .into_iter()
    .find(|target| gamepad_binding_value(bindings, *target) == binding)
}

fn gamepad_menu_binding_target_for_binding(
    bindings: GamepadMenuBindings,
    binding: GamepadButtonBinding,
) -> Option<GamepadMenuBindingTarget> {
    [
        GamepadMenuBindingTarget::Up,
        GamepadMenuBindingTarget::Down,
        GamepadMenuBindingTarget::Confirm,
        GamepadMenuBindingTarget::Cancel,
    ]
    .into_iter()
    .find(|target| gamepad_menu_binding_value(bindings, *target) == binding)
}

fn gamepad_binding_value(
    bindings: GamepadButtonBindings,
    target: GamepadBindingTarget,
) -> GamepadButtonBinding {
    match target {
        GamepadBindingTarget::Up => bindings.up,
        GamepadBindingTarget::Down => bindings.down,
        GamepadBindingTarget::Left => bindings.left,
        GamepadBindingTarget::Right => bindings.right,
        GamepadBindingTarget::A => bindings.a,
        GamepadBindingTarget::B => bindings.b,
        GamepadBindingTarget::Select => bindings.select,
        GamepadBindingTarget::Start => bindings.start,
    }
}

fn gamepad_menu_binding_value(
    bindings: GamepadMenuBindings,
    target: GamepadMenuBindingTarget,
) -> GamepadButtonBinding {
    match target {
        GamepadMenuBindingTarget::Up => bindings.up,
        GamepadMenuBindingTarget::Down => bindings.down,
        GamepadMenuBindingTarget::Confirm => bindings.confirm,
        GamepadMenuBindingTarget::Cancel => bindings.cancel,
    }
}

fn set_gamepad_binding_value(
    bindings: &mut GamepadButtonBindings,
    target: GamepadBindingTarget,
    binding: GamepadButtonBinding,
) {
    match target {
        GamepadBindingTarget::Up => bindings.up = binding,
        GamepadBindingTarget::Down => bindings.down = binding,
        GamepadBindingTarget::Left => bindings.left = binding,
        GamepadBindingTarget::Right => bindings.right = binding,
        GamepadBindingTarget::A => bindings.a = binding,
        GamepadBindingTarget::B => bindings.b = binding,
        GamepadBindingTarget::Select => bindings.select = binding,
        GamepadBindingTarget::Start => bindings.start = binding,
    }
}

fn set_gamepad_menu_binding_value(
    bindings: &mut GamepadMenuBindings,
    target: GamepadMenuBindingTarget,
    binding: GamepadButtonBinding,
) {
    match target {
        GamepadMenuBindingTarget::Up => bindings.up = binding,
        GamepadMenuBindingTarget::Down => bindings.down = binding,
        GamepadMenuBindingTarget::Confirm => bindings.confirm = binding,
        GamepadMenuBindingTarget::Cancel => bindings.cancel = binding,
    }
}

fn joypad_binding_target_for_key(
    bindings: JoypadKeyboardBindings,
    key: DesktopKey,
) -> Option<KeyboardBindingTarget> {
    [
        KeyboardBindingTarget::Up,
        KeyboardBindingTarget::Down,
        KeyboardBindingTarget::Left,
        KeyboardBindingTarget::Right,
        KeyboardBindingTarget::A,
        KeyboardBindingTarget::B,
        KeyboardBindingTarget::Select,
        KeyboardBindingTarget::Start,
    ]
    .into_iter()
    .find(|target| {
        keyboard_binding_value(
            KeyboardBindings {
                joypad: bindings,
                ..KeyboardBindings::default()
            },
            *target,
        ) == key
    })
}

fn keyboard_menu_binding_target_for_key(
    bindings: MenuKeyboardBindings,
    key: DesktopKey,
) -> Option<KeyboardMenuBindingTarget> {
    [
        KeyboardMenuBindingTarget::Up,
        KeyboardMenuBindingTarget::Down,
        KeyboardMenuBindingTarget::Confirm,
        KeyboardMenuBindingTarget::Cancel,
    ]
    .into_iter()
    .find(|target| keyboard_menu_binding_value(bindings, *target) == key)
}

fn hotkey_binding_target_for_key(
    bindings: HotkeyBindings,
    key: DesktopKey,
) -> Option<KeyboardBindingTarget> {
    [
        KeyboardBindingTarget::Pause,
        KeyboardBindingTarget::Reset,
        KeyboardBindingTarget::ToggleFullscreen,
        KeyboardBindingTarget::TogglePerformanceHud,
        KeyboardBindingTarget::SaveBattery,
    ]
    .into_iter()
    .find(|target| {
        keyboard_binding_value(
            KeyboardBindings {
                hotkeys: bindings,
                ..KeyboardBindings::default()
            },
            *target,
        ) == key
    })
}

fn keyboard_menu_binding_value(
    bindings: MenuKeyboardBindings,
    target: KeyboardMenuBindingTarget,
) -> DesktopKey {
    match target {
        KeyboardMenuBindingTarget::Up => bindings.up,
        KeyboardMenuBindingTarget::Down => bindings.down,
        KeyboardMenuBindingTarget::Confirm => bindings.confirm,
        KeyboardMenuBindingTarget::Cancel => bindings.cancel,
    }
}

fn keyboard_binding_value(bindings: KeyboardBindings, target: KeyboardBindingTarget) -> DesktopKey {
    match target {
        KeyboardBindingTarget::Up => bindings.joypad.up,
        KeyboardBindingTarget::Down => bindings.joypad.down,
        KeyboardBindingTarget::Left => bindings.joypad.left,
        KeyboardBindingTarget::Right => bindings.joypad.right,
        KeyboardBindingTarget::A => bindings.joypad.a,
        KeyboardBindingTarget::B => bindings.joypad.b,
        KeyboardBindingTarget::Select => bindings.joypad.select,
        KeyboardBindingTarget::Start => bindings.joypad.start,
        KeyboardBindingTarget::Pause => bindings.hotkeys.pause,
        KeyboardBindingTarget::Reset => bindings.hotkeys.reset,
        KeyboardBindingTarget::ToggleFullscreen => bindings.hotkeys.toggle_fullscreen,
        KeyboardBindingTarget::TogglePerformanceHud => bindings.hotkeys.toggle_performance_hud,
        KeyboardBindingTarget::SaveBattery => bindings.hotkeys.save_battery,
    }
}

fn set_keyboard_menu_binding_value(
    bindings: &mut MenuKeyboardBindings,
    target: KeyboardMenuBindingTarget,
    key: DesktopKey,
) {
    match target {
        KeyboardMenuBindingTarget::Up => bindings.up = key,
        KeyboardMenuBindingTarget::Down => bindings.down = key,
        KeyboardMenuBindingTarget::Confirm => bindings.confirm = key,
        KeyboardMenuBindingTarget::Cancel => bindings.cancel = key,
    }
}

fn set_keyboard_binding_value(
    bindings: &mut KeyboardBindings,
    target: KeyboardBindingTarget,
    key: DesktopKey,
) {
    match target {
        KeyboardBindingTarget::Up => bindings.joypad.up = key,
        KeyboardBindingTarget::Down => bindings.joypad.down = key,
        KeyboardBindingTarget::Left => bindings.joypad.left = key,
        KeyboardBindingTarget::Right => bindings.joypad.right = key,
        KeyboardBindingTarget::A => bindings.joypad.a = key,
        KeyboardBindingTarget::B => bindings.joypad.b = key,
        KeyboardBindingTarget::Select => bindings.joypad.select = key,
        KeyboardBindingTarget::Start => bindings.joypad.start = key,
        KeyboardBindingTarget::Pause => bindings.hotkeys.pause = key,
        KeyboardBindingTarget::Reset => bindings.hotkeys.reset = key,
        KeyboardBindingTarget::ToggleFullscreen => bindings.hotkeys.toggle_fullscreen = key,
        KeyboardBindingTarget::TogglePerformanceHud => {
            bindings.hotkeys.toggle_performance_hud = key;
        }
        KeyboardBindingTarget::SaveBattery => bindings.hotkeys.save_battery = key,
    }
}

fn sync_live_input_state(
    event_pump: &sdl3::EventPump,
    keyboard_bindings: &KeyboardBindings,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
) {
    clear_live_input_state(machine, runtime);
    sync_keyboard_state(
        event_pump,
        keyboard_bindings,
        &mut runtime.input_state,
        machine.primary_machine_mut(),
    );
    sync_linked_secondary_keyboard_state(
        event_pump,
        &mut runtime.secondary_input_state,
        machine.secondary_machine_mut(),
    );
    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager
            .sync_active_gamepad_state(&mut runtime.input_state, machine.primary_machine_mut());
    }
}

fn clear_live_input_state(machine: &mut DesktopEmulationSession, runtime: &mut FrontendRuntime) {
    runtime.input_state.clear_all(machine.primary_machine_mut());
    if let Some(secondary_machine) = machine.secondary_machine_mut() {
        runtime.secondary_input_state.clear_all(secondary_machine);
    } else {
        runtime.secondary_input_state.reset();
    }
}

fn sync_keyboard_state(
    event_pump: &sdl3::EventPump,
    keyboard_bindings: &KeyboardBindings,
    input_state: &mut FrontendInputState,
    machine: &mut Machine<TraceSummaryBuffer>,
) {
    let keyboard_state = event_pump.keyboard_state();
    let joypad = keyboard_bindings.joypad;
    let bindings = [
        (JoypadButton::Up, joypad.up),
        (JoypadButton::Down, joypad.down),
        (JoypadButton::Left, joypad.left),
        (JoypadButton::Right, joypad.right),
        (JoypadButton::A, joypad.a),
        (JoypadButton::B, joypad.b),
        (JoypadButton::Select, joypad.select),
        (JoypadButton::Start, joypad.start),
    ];

    for (joypad_button, desktop_key) in bindings {
        input_state.set_keyboard_button(
            machine,
            joypad_button,
            keyboard_state.is_scancode_pressed(desktop_key_scancode(desktop_key)),
        );
    }
}

fn sync_linked_secondary_keyboard_state(
    event_pump: &sdl3::EventPump,
    input_state: &mut FrontendInputState,
    machine: Option<&mut Machine<TraceSummaryBuffer>>,
) {
    let Some(machine) = machine else {
        input_state.reset();
        return;
    };

    let keyboard_state = event_pump.keyboard_state();
    for (joypad_button, scancode) in LINKED_SECONDARY_KEYBOARD_BINDINGS {
        input_state.set_keyboard_button(
            machine,
            joypad_button,
            keyboard_state.is_scancode_pressed(scancode),
        );
    }
}

fn linked_secondary_joypad_button_for_scancode(scancode: Scancode) -> Option<JoypadButton> {
    LINKED_SECONDARY_KEYBOARD_BINDINGS
        .into_iter()
        .find_map(|(button, binding)| (binding == scancode).then_some(button))
}

fn desktop_key_scancode(binding: DesktopKey) -> Scancode {
    match binding {
        DesktopKey::Escape => Scancode::Escape,
        DesktopKey::ArrowUp => Scancode::Up,
        DesktopKey::ArrowDown => Scancode::Down,
        DesktopKey::ArrowLeft => Scancode::Left,
        DesktopKey::ArrowRight => Scancode::Right,
        DesktopKey::Backspace => Scancode::Backspace,
        DesktopKey::Return => Scancode::Return,
        DesktopKey::Space => Scancode::Space,
        DesktopKey::R => Scancode::R,
        DesktopKey::X => Scancode::X,
        DesktopKey::Z => Scancode::Z,
        DesktopKey::F5 => Scancode::F5,
        DesktopKey::F10 => Scancode::F10,
        DesktopKey::F11 => Scancode::F11,
    }
}

fn gamepad_event_joystick_id(which: u32) -> sdl3::joystick::JoystickId {
    sdl3::sys::joystick::SDL_JoystickID(which)
}

fn menu_input_for_key(bindings: MenuKeyboardBindings, keycode: Keycode) -> Option<MenuInput> {
    if keycode == Keycode::Escape {
        return Some(MenuInput::Cancel);
    }

    if key_matches(bindings.up, keycode) {
        Some(MenuInput::Up)
    } else if key_matches(bindings.down, keycode) {
        Some(MenuInput::Down)
    } else if key_matches(bindings.confirm, keycode) {
        Some(MenuInput::Confirm)
    } else if key_matches(bindings.cancel, keycode) {
        Some(MenuInput::Cancel)
    } else {
        None
    }
}

fn menu_input_for_gamepad_button(
    bindings: GamepadMenuBindings,
    button: Button,
) -> Option<MenuInput> {
    if button == sdl_button_for_binding(bindings.up) {
        Some(MenuInput::Up)
    } else if button == sdl_button_for_binding(bindings.down) {
        Some(MenuInput::Down)
    } else if button == sdl_button_for_binding(bindings.confirm) {
        Some(MenuInput::Confirm)
    } else if button == sdl_button_for_binding(bindings.cancel) {
        Some(MenuInput::Cancel)
    } else {
        None
    }
}

fn reset_machine(
    main_window: &Window,
    session: &mut DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    settings_store: &mut DesktopSettingsStore,
) -> Result<(), String> {
    let Some(rom_bytes) = session.rom_bytes() else {
        return Ok(());
    };
    drain_printed_pages_into_printer_output(main_window, session, runtime, machine);
    flush_pending_printer_output(main_window, session, runtime);
    runtime.rtc_sync.apply_to_machine(machine);
    let primary_battery_backed_state = uses_battery_backed_hardware_persistence(
        machine.primary_machine().cartridge().persistence_metadata(),
    )
    .then(|| machine.primary_machine().cartridge().persistent_state());
    let secondary_battery_backed_state =
        machine.secondary_machine().and_then(|secondary_machine| {
            uses_battery_backed_hardware_persistence(
                secondary_machine.cartridge().persistence_metadata(),
            )
            .then(|| secondary_machine.cartridge().persistent_state())
        });

    close_runtime_save_sessions(runtime, machine)?;

    let (
        effective_config,
        boot_rom_fallback_warnings,
        reset_machine,
        next_save_session,
        next_secondary_save_session,
    ) = match (
        session.linked_secondary_rom_bytes(),
        session.external_port_selection,
    ) {
        (Some(secondary_rom_bytes), DesktopExternalPortSelection::GameLink) => {
            let primary_loaded =
                match load_machine_for_rom(&session.config, &session.current_dir, rom_bytes) {
                    Ok(result) => result,
                    Err(error) => {
                        return Err(format_display_error(
                            "failed to reload primary cartridge during linked reset",
                            &error,
                        ));
                    }
                };
            let secondary_loaded = match load_machine_for_rom(
                &session.config,
                &session.current_dir,
                secondary_rom_bytes,
            ) {
                Ok(result) => result,
                Err(error) => {
                    return Err(format_display_error(
                        "failed to reload secondary cartridge during linked reset",
                        &error,
                    ));
                }
            };
            if primary_loaded.effective_config != secondary_loaded.effective_config {
                return Err(
                    "linked reset produced divergent effective configs between the primary and secondary machines"
                        .to_string(),
                );
            }

            let mut boot_rom_fallback_warnings = Vec::new();
            if let Some(warning) = primary_loaded.boot_rom_fallback_warning {
                boot_rom_fallback_warnings.push(warning);
            }
            if let Some(warning) = secondary_loaded.boot_rom_fallback_warning {
                boot_rom_fallback_warnings.push(warning);
            }
            write_cartridge_diagnostics(&primary_loaded.diagnostics);
            write_cartridge_diagnostics(&secondary_loaded.diagnostics);

            let mut reset_machine = DesktopEmulationSession::new_single(primary_loaded.machine);
            if let Some(persistent_state) = primary_battery_backed_state
                && let Err(error) = reset_machine
                    .primary_machine_mut()
                    .restore_cartridge_persistent_state(&persistent_state)
            {
                return Err(format!(
                    "failed to restore battery-backed persistence after reset: {error:?}"
                ));
            }
            reset_machine.attach_secondary_dmg04(secondary_loaded.machine)?;
            if let Some(persistent_state) = secondary_battery_backed_state
                && let Some(secondary_machine) = reset_machine.secondary_machine_mut()
                && let Err(error) =
                    secondary_machine.restore_cartridge_persistent_state(&persistent_state)
            {
                return Err(format!(
                    "failed to restore linked battery-backed persistence after reset: {error:?}"
                ));
            }

            let effective_config = primary_loaded.effective_config;
            let next_save_session = open_save_session_for_session(
                &DesktopSession {
                    config: effective_config.clone(),
                    ..session.clone()
                },
                reset_machine.primary_machine_mut(),
            )?;
            let next_secondary_save_session = open_secondary_save_session_for_session(
                &DesktopSession {
                    config: effective_config.clone(),
                    ..session.clone()
                },
                reset_machine
                    .secondary_machine_mut()
                    .expect("linked desktop session should expose a secondary machine"),
            )?;
            (
                effective_config,
                boot_rom_fallback_warnings,
                reset_machine,
                next_save_session,
                next_secondary_save_session,
            )
        }
        _ => {
            let loaded =
                match load_machine_for_rom(&session.config, &session.current_dir, rom_bytes) {
                    Ok(result) => result,
                    Err(error) => {
                        return Err(format_display_error(
                            "failed to reload cartridge during reset",
                            &error,
                        ));
                    }
                };
            let mut boot_rom_fallback_warnings = Vec::new();
            if let Some(warning) = loaded.boot_rom_fallback_warning {
                boot_rom_fallback_warnings.push(warning);
            }
            write_cartridge_diagnostics(&loaded.diagnostics);
            let mut reset_machine = DesktopEmulationSession::new_single(loaded.machine);
            if let Some(persistent_state) = primary_battery_backed_state
                && let Err(error) = reset_machine
                    .primary_machine_mut()
                    .restore_cartridge_persistent_state(&persistent_state)
            {
                return Err(format!(
                    "failed to restore battery-backed persistence after reset: {error:?}"
                ));
            }
            apply_external_port_selection_to_machine(
                reset_machine.primary_machine_mut(),
                session.external_port_selection,
            );

            let effective_config = loaded.effective_config;
            let next_save_session = open_save_session_for_session(
                &DesktopSession {
                    config: effective_config.clone(),
                    linked_secondary_rom: None,
                    ..session.clone()
                },
                reset_machine.primary_machine_mut(),
            )?;
            (
                effective_config,
                boot_rom_fallback_warnings,
                reset_machine,
                next_save_session,
                None,
            )
        }
    };

    for warning in &boot_rom_fallback_warnings {
        log_boot_rom_fallback_warning(Some(warning));
    }
    let config_fell_back = effective_config != session.config;
    session.config = effective_config;
    if config_fell_back {
        settings_store.persist_machine_preferences(&session.config)?;
    }
    let reset_console_model = reset_machine.primary_machine().apu().console_model();

    clear_live_input_state(machine, runtime);
    *machine = reset_machine;
    if let Some(audio_output) = &mut runtime.audio_output {
        audio_output.set_console_model(reset_console_model)?;
    }
    if let Some(audio_recorder) = &mut runtime.audio_recorder {
        audio_recorder.set_console_model(reset_console_model)?;
    }
    runtime.save_session = next_save_session;
    runtime.secondary_save_session = next_secondary_save_session;
    runtime.rtc_sync.resync_to_host_clock();
    Ok(())
}

fn save_screenshot_for_session(
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
    video_options: &VideoOptions,
) -> Result<PathBuf, String> {
    let rendered = screenshot_output::render_screenshot(
        FramebufferPanelInput {
            framebuffer: machine.ppu().framebuffer(),
            framebuffer_layer_sources: machine.ppu().framebuffer_layer_sources(),
            bgwin_framebuffer: machine.ppu().framebuffer_bgwin_panel_shades(),
            backdrop_framebuffer: machine.ppu().framebuffer_backdrop_panel_shades(),
            bgwin_framebuffer_layer_sources: machine.ppu().framebuffer_bgwin_layer_sources(),
        },
        machine
            .secondary_machine()
            .map(|secondary| FramebufferPanelInput {
                framebuffer: secondary.ppu().framebuffer(),
                framebuffer_layer_sources: secondary.ppu().framebuffer_layer_sources(),
                bgwin_framebuffer: secondary.ppu().framebuffer_bgwin_panel_shades(),
                backdrop_framebuffer: secondary.ppu().framebuffer_backdrop_panel_shades(),
                bgwin_framebuffer_layer_sources: secondary.ppu().framebuffer_bgwin_layer_sources(),
            }),
        video_options,
    );
    let output_path = screenshot_output::resolve_next_screenshot_output_path(
        session.rom_path(),
        session.current_dir.as_path(),
    )?;
    screenshot_output::save_rendered_screenshot_png(&rendered, &output_path)?;
    Ok(output_path)
}

fn toggle_fullscreen(window: &mut Window) -> Result<(), String> {
    let target_state = window.fullscreen_state() == FullscreenType::Off;
    map_display_result(
        window.set_fullscreen(target_state),
        "failed to toggle SDL3 fullscreen state",
    )
}

fn set_fullscreen_state(window: &mut Window, enabled: bool) -> Result<(), String> {
    if (window.fullscreen_state() != FullscreenType::Off) == enabled {
        return Ok(());
    }

    map_display_result(
        window.set_fullscreen(enabled),
        "failed to set SDL3 fullscreen state",
    )
}

fn apply_renderer_vsync(
    canvas: &mut Canvas<Window>,
    frame_pacer: &mut FramePacer,
    vsync_enabled: bool,
) -> Result<(), String> {
    let interval = if vsync_enabled {
        1
    } else {
        sys::render::SDL_RENDERER_VSYNC_DISABLED
    };
    // SDL3 exposes render-vsync control on the renderer, not on the window.
    let success = unsafe { sys::render::SDL_SetRenderVSync(canvas.raw(), interval) };
    if !success {
        return Err(format!(
            "failed to configure SDL3 renderer vsync: {}",
            sdl3::get_error()
        ));
    }

    frame_pacer.set_vsync_enabled(vsync_enabled);
    Ok(())
}

#[cfg(test)]
fn apply_window_scale(window: &mut Window, scale: u8) -> Result<(), String> {
    apply_window_scale_for_dimensions(
        window,
        scale,
        FramebufferDimensions {
            width: FRAMEBUFFER_WIDTH,
            height: FRAMEBUFFER_HEIGHT,
        },
    )
}

fn apply_window_scale_for_dimensions(
    window: &mut Window,
    scale: u8,
    dimensions: FramebufferDimensions,
) -> Result<(), String> {
    let scale = u32::from(scale.max(1));
    let width = dimensions
        .width
        .checked_mul(scale)
        .ok_or_else(|| overflow_error("window width overflowed while applying window scale"))?;
    let height = dimensions
        .height
        .checked_mul(scale)
        .ok_or_else(|| overflow_error("window height overflowed while applying window scale"))?;
    map_display_result(
        window.set_size(width, height),
        "failed to resize SDL3 window",
    )
}

fn apply_canvas_video_options_for_dimensions(
    canvas: &mut Canvas<Window>,
    video_options: &VideoOptions,
    dimensions: FramebufferDimensions,
) -> Result<(), String> {
    let presentation_mode = if video_options.integer_scale {
        sys::render::SDL_LOGICAL_PRESENTATION_INTEGER_SCALE
    } else {
        sys::render::SDL_LOGICAL_PRESENTATION_LETTERBOX
    };
    map_display_result(
        canvas.set_logical_size(dimensions.width, dimensions.height, presentation_mode),
        "failed to configure SDL3 logical presentation",
    )
}

fn sync_audio_playback_state(
    machine: &DesktopEmulationSession,
    runtime: &FrontendRuntime,
) -> Result<(), String> {
    let Some(audio_output) = runtime.audio_output.as_ref() else {
        return Ok(());
    };

    if emulation_paused(audio_source_machine(machine), runtime) {
        audio_output.pause()
    } else {
        audio_output.resume()
    }
}

fn framebuffer_dimensions_for_session(machine: &DesktopEmulationSession) -> FramebufferDimensions {
    FramebufferDimensions {
        width: if machine.is_linked_dmg04_two_player() {
            FRAMEBUFFER_WIDTH * 2
        } else {
            FRAMEBUFFER_WIDTH
        },
        height: FRAMEBUFFER_HEIGHT,
    }
}

fn framebuffer_pitch_bytes_for_dimensions(dimensions: FramebufferDimensions) -> usize {
    dimensions.width as usize * 3
}

fn bgwin_layer_source_visible(
    video_options: &VideoOptions,
    source: PpuFramebufferLayerSource,
) -> bool {
    match source {
        PpuFramebufferLayerSource::Backdrop => false,
        PpuFramebufferLayerSource::Background => video_options.show_background,
        PpuFramebufferLayerSource::Window => video_options.show_window,
        PpuFramebufferLayerSource::Object => false,
    }
}

fn composite_framebuffer_panel_shade(
    final_shade: u8,
    final_source: PpuFramebufferLayerSource,
    bgwin_shade: u8,
    bgwin_source: PpuFramebufferLayerSource,
    backdrop_shade: u8,
    video_options: &VideoOptions,
) -> u8 {
    let bgwin_panel_shade = if bgwin_layer_source_visible(video_options, bgwin_source) {
        bgwin_shade
    } else {
        backdrop_shade
    };

    if video_options.show_objects && final_source == PpuFramebufferLayerSource::Object {
        final_shade
    } else {
        bgwin_panel_shade
    }
}

fn framebuffer_texture_scale_mode(video_options: &VideoOptions) -> ScaleMode {
    if video_options.presentation_filter {
        ScaleMode::Linear
    } else {
        ScaleMode::Nearest
    }
}

fn sync_framebuffer_texture_video_options(
    texture: &mut sdl3::render::Texture<'_>,
    video_options: &VideoOptions,
) {
    let expected_scale_mode = framebuffer_texture_scale_mode(video_options);
    if texture.scale_mode() != expected_scale_mode {
        texture.set_scale_mode(expected_scale_mode);
    }
}

fn create_framebuffer_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    dimensions: FramebufferDimensions,
) -> Result<sdl3::render::Texture<'a>, String> {
    map_display_result(
        texture_creator.create_texture_streaming(
            PixelFormat::RGB24,
            dimensions.width,
            dimensions.height,
        ),
        "failed to create framebuffer texture",
    )
}

fn sync_framebuffer_presentation_resources<'a>(
    canvas: &mut Canvas<Window>,
    texture_creator: &'a TextureCreator<WindowContext>,
    texture: &mut sdl3::render::Texture<'a>,
    rgb_frame: &mut Vec<u8>,
    current_dimensions: &mut FramebufferDimensions,
    machine: &DesktopEmulationSession,
    video_options: &VideoOptions,
) -> Result<(), String> {
    let next_dimensions = framebuffer_dimensions_for_session(machine);
    if next_dimensions == *current_dimensions {
        return Ok(());
    }

    *texture = create_framebuffer_texture(texture_creator, next_dimensions)?;
    rgb_frame.resize(
        next_dimensions.height as usize * framebuffer_pitch_bytes_for_dimensions(next_dimensions),
        0,
    );
    if canvas.window().fullscreen_state() == FullscreenType::Off {
        apply_window_scale_for_dimensions(
            canvas.window_mut(),
            video_options.window_scale,
            next_dimensions,
        )?;
    }
    apply_canvas_video_options_for_dimensions(canvas, video_options, next_dimensions)?;
    *current_dimensions = next_dimensions;
    Ok(())
}

fn write_monochrome_framebuffer_region(
    target_rgb_frame: &mut [u8],
    target_dimensions: FramebufferDimensions,
    target_origin_x: usize,
    source_panel: FramebufferPanelInput<'_>,
    video_options: &VideoOptions,
) {
    let target_pitch_bytes = framebuffer_pitch_bytes_for_dimensions(target_dimensions);
    let target_width = target_dimensions.width as usize;
    let target_height = target_dimensions.height as usize;
    for y in 0..target_height.min(FRAMEBUFFER_HEIGHT as usize) {
        for x in 0..(FRAMEBUFFER_WIDTH as usize) {
            if target_origin_x + x >= target_width {
                break;
            }

            let source_index = y * FRAMEBUFFER_WIDTH as usize + x;
            let target_pixel_index = y * target_pitch_bytes + ((target_origin_x + x) * 3);
            let panel_shade = composite_framebuffer_panel_shade(
                source_panel.framebuffer[source_index],
                source_panel.framebuffer_layer_sources[source_index],
                source_panel.bgwin_framebuffer[source_index],
                source_panel.bgwin_framebuffer_layer_sources[source_index],
                source_panel.backdrop_framebuffer[source_index],
                video_options,
            );
            let shade = framebuffer_pixel_to_grayscale(panel_shade);
            target_rgb_frame[target_pixel_index] = shade;
            target_rgb_frame[target_pixel_index + 1] = shade;
            target_rgb_frame[target_pixel_index + 2] = shade;
        }
    }
}

fn render_frame(
    canvas: &mut Canvas<Window>,
    texture: &mut sdl3::render::Texture<'_>,
    rgb_frame: &mut [u8],
    framebuffer: FramebufferRenderInput<'_>,
    video_options: &VideoOptions,
    menu_state: Option<(&OverlayMenuState, MenuPresentation)>,
    performance_hud: Option<PerformanceHudSnapshot>,
) -> Result<Duration, String> {
    apply_canvas_video_options_for_dimensions(canvas, video_options, framebuffer.dimensions)?;
    sync_framebuffer_texture_video_options(texture, video_options);
    rgb_frame.fill(0);
    write_monochrome_framebuffer_region(
        rgb_frame,
        framebuffer.dimensions,
        0,
        framebuffer.primary,
        video_options,
    );
    if let Some(secondary_panel) = framebuffer.secondary {
        write_monochrome_framebuffer_region(
            rgb_frame,
            framebuffer.dimensions,
            FRAMEBUFFER_WIDTH as usize,
            secondary_panel,
            video_options,
        );
    }
    if let Some((menu_state, menu_presentation)) = menu_state {
        menu_state.render_overlay(
            rgb_frame,
            framebuffer.dimensions.width as usize,
            framebuffer.dimensions.height as usize,
            menu_presentation,
        );
    }
    if menu_state.is_none()
        && video_options.show_performance_hud
        && let Some(snapshot) = performance_hud
    {
        render_performance_hud(
            rgb_frame,
            framebuffer.dimensions.width as usize,
            framebuffer.dimensions.height as usize,
            snapshot,
        );
    }

    map_display_result(
        texture.update(
            None,
            rgb_frame,
            framebuffer_pitch_bytes_for_dimensions(framebuffer.dimensions),
        ),
        "failed to update framebuffer texture",
    )?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    map_display_result(
        canvas.copy(texture, None, None),
        "failed to present framebuffer texture",
    )?;
    let present_started_at = Instant::now();
    canvas.present();
    Ok(present_started_at.elapsed())
}

fn write_cartridge_diagnostics(diagnostics: &[CartridgeDiagnostic]) {
    for diagnostic in diagnostics {
        eprintln!(
            "{}: {}",
            diagnostic_severity_name(diagnostic.severity),
            diagnostic.message
        );
    }
}

fn diagnostic_severity_name(severity: CartridgeDiagnosticSeverity) -> &'static str {
    match severity {
        CartridgeDiagnosticSeverity::Warning => "warning",
        CartridgeDiagnosticSeverity::Error => "error",
    }
}

fn joypad_button_for_key(
    bindings: JoypadKeyboardBindings,
    keycode: Keycode,
) -> Option<JoypadButton> {
    if key_matches(bindings.up, keycode) {
        Some(JoypadButton::Up)
    } else if key_matches(bindings.down, keycode) {
        Some(JoypadButton::Down)
    } else if key_matches(bindings.left, keycode) {
        Some(JoypadButton::Left)
    } else if key_matches(bindings.right, keycode) {
        Some(JoypadButton::Right)
    } else if key_matches(bindings.a, keycode) {
        Some(JoypadButton::A)
    } else if key_matches(bindings.b, keycode) {
        Some(JoypadButton::B)
    } else if key_matches(bindings.select, keycode) {
        Some(JoypadButton::Select)
    } else if key_matches(bindings.start, keycode) {
        Some(JoypadButton::Start)
    } else {
        None
    }
}

fn hotkey_action(keyboard_bindings: &KeyboardBindings, keycode: Keycode) -> HotkeyAction {
    if key_matches(keyboard_bindings.hotkeys.save_battery, keycode) {
        HotkeyAction::ManualSave
    } else if key_matches(keyboard_bindings.hotkeys.reset, keycode) {
        HotkeyAction::Reset
    } else if key_matches(keyboard_bindings.hotkeys.toggle_fullscreen, keycode) {
        HotkeyAction::ToggleFullscreen
    } else if key_matches(keyboard_bindings.hotkeys.toggle_performance_hud, keycode) {
        HotkeyAction::TogglePerformanceHud
    } else {
        HotkeyAction::None
    }
}

fn desktop_key_from_keycode(keycode: Keycode) -> Option<DesktopKey> {
    match keycode {
        Keycode::Escape => Some(DesktopKey::Escape),
        Keycode::Up => Some(DesktopKey::ArrowUp),
        Keycode::Down => Some(DesktopKey::ArrowDown),
        Keycode::Left => Some(DesktopKey::ArrowLeft),
        Keycode::Right => Some(DesktopKey::ArrowRight),
        Keycode::Backspace => Some(DesktopKey::Backspace),
        Keycode::Return => Some(DesktopKey::Return),
        Keycode::Space => Some(DesktopKey::Space),
        Keycode::R => Some(DesktopKey::R),
        Keycode::X => Some(DesktopKey::X),
        Keycode::Z => Some(DesktopKey::Z),
        Keycode::F5 => Some(DesktopKey::F5),
        Keycode::F10 => Some(DesktopKey::F10),
        Keycode::F11 => Some(DesktopKey::F11),
        _ => None,
    }
}

fn assignable_key_for_binding_target_from_keycode(
    keycode: Keycode,
    target: KeyboardBindingTarget,
) -> Option<DesktopKey> {
    match target {
        KeyboardBindingTarget::Pause
        | KeyboardBindingTarget::Reset
        | KeyboardBindingTarget::ToggleFullscreen
        | KeyboardBindingTarget::TogglePerformanceHud
        | KeyboardBindingTarget::SaveBattery => match keycode {
            Keycode::Up
            | Keycode::Down
            | Keycode::Left
            | Keycode::Right
            | Keycode::Backspace
            | Keycode::Return
            | Keycode::Space
            | Keycode::R
            | Keycode::X
            | Keycode::Z
            | Keycode::F5
            | Keycode::F10
            | Keycode::F11 => desktop_key_from_keycode(keycode),
            _ => None,
        },
        KeyboardBindingTarget::Up
        | KeyboardBindingTarget::Down
        | KeyboardBindingTarget::Left
        | KeyboardBindingTarget::Right
        | KeyboardBindingTarget::A
        | KeyboardBindingTarget::B
        | KeyboardBindingTarget::Select
        | KeyboardBindingTarget::Start => match keycode {
            Keycode::Up
            | Keycode::Down
            | Keycode::Left
            | Keycode::Right
            | Keycode::Backspace
            | Keycode::Return
            | Keycode::Space
            | Keycode::R
            | Keycode::X
            | Keycode::Z => desktop_key_from_keycode(keycode),
            _ => None,
        },
    }
}

fn assignable_menu_key_for_binding_target_from_keycode(
    keycode: Keycode,
    target: KeyboardMenuBindingTarget,
) -> Option<DesktopKey> {
    match target {
        KeyboardMenuBindingTarget::Cancel => match keycode {
            Keycode::Escape
            | Keycode::Up
            | Keycode::Down
            | Keycode::Backspace
            | Keycode::Return
            | Keycode::Space
            | Keycode::R
            | Keycode::X
            | Keycode::Z
            | Keycode::F5
            | Keycode::F10
            | Keycode::F11 => desktop_key_from_keycode(keycode),
            _ => None,
        },
        KeyboardMenuBindingTarget::Up
        | KeyboardMenuBindingTarget::Down
        | KeyboardMenuBindingTarget::Confirm => match keycode {
            Keycode::Up
            | Keycode::Down
            | Keycode::Backspace
            | Keycode::Return
            | Keycode::Space
            | Keycode::R
            | Keycode::X
            | Keycode::Z
            | Keycode::F5
            | Keycode::F10
            | Keycode::F11 => desktop_key_from_keycode(keycode),
            _ => None,
        },
    }
}

fn key_matches(binding: DesktopKey, keycode: Keycode) -> bool {
    match binding {
        DesktopKey::Escape => keycode == Keycode::Escape,
        DesktopKey::ArrowUp => keycode == Keycode::Up,
        DesktopKey::ArrowDown => keycode == Keycode::Down,
        DesktopKey::ArrowLeft => keycode == Keycode::Left,
        DesktopKey::ArrowRight => keycode == Keycode::Right,
        DesktopKey::Backspace => keycode == Keycode::Backspace,
        DesktopKey::Return => keycode == Keycode::Return,
        DesktopKey::Space => keycode == Keycode::Space,
        DesktopKey::R => keycode == Keycode::R,
        DesktopKey::X => keycode == Keycode::X,
        DesktopKey::Z => keycode == Keycode::Z,
        DesktopKey::F5 => keycode == Keycode::F5,
        DesktopKey::F10 => keycode == Keycode::F10,
        DesktopKey::F11 => keycode == Keycode::F11,
    }
}

fn startup_mode_name(startup_mode: StartupMode) -> &'static str {
    match startup_mode {
        StartupMode::SkipBoot => "skip-boot",
        StartupMode::RealBoot => "real-boot",
    }
}

fn execution_mode_name(execution_mode: ExecutionMode) -> &'static str {
    match execution_mode {
        ExecutionMode::Strict => "strict",
        ExecutionMode::Permissive => "permissive",
        ExecutionMode::Experimental => "experimental",
    }
}

fn framebuffer_pixel_to_grayscale(pixel: u8) -> u8 {
    match pixel {
        0..=3 => DMG_GRAYSCALE_SHADES[usize::from(pixel)],
        _ => DMG_GRAYSCALE_SHADES[3],
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BOOT_ROM_FILE_DIALOG_FILTERS, DEFAULT_BOOT_ROM_DIR, DesktopRunOptions,
        DesktopSettingsStore, GamepadBindingTarget, GamepadMenuBindingTarget, HostRtcSync,
        KeyboardBindingTarget, KeyboardMenuBindingTarget, PathDialogResult, PerformanceHudSnapshot,
        ROM_FILE_DIALOG_FILTERS, assign_gamepad_binding, assign_gamepad_menu_binding,
        assign_keyboard_binding, assign_keyboard_menu_binding,
        assignable_key_for_binding_target_from_keycode,
        assignable_menu_key_for_binding_target_from_keycode, compact_recent_rom_label,
        desktop_key_from_keycode, desktop_key_scancode, gamepad_binding_target_for_binding,
        gamepad_menu_binding_target_for_binding, hotkey_binding_target_for_key,
        joypad_binding_target_for_key, keyboard_menu_binding_target_for_key,
        map_path_dialog_result, menu_input_for_gamepad_button, menu_input_for_key,
        next_audio_volume_percent, next_boot_rom_verification_mode, next_console_model,
        next_execution_mode, next_gamepad_directional_source, next_gamepad_rumble_mode,
        next_save_flush_policy, next_startup_mode, next_window_scale, parse_trace_capture_t_cycles,
        performance_window_title, render_desktop_trace_record, run_desktop,
    };
    use crate::audio_recording::DesktopAudioRecordingOptions;
    use gb_core::apu::{ApuOutputSnapshot, ApuStereoOutputSnapshot};
    use gb_core::{
        Apu, ApuRecordedChannel, ApuRecordedChannelMask, ApuRegisterWriteObservation,
        ApuRegisterWriteState, CartridgeDiagnostic, CartridgeDiagnosticSeverity, ConsoleModel,
        CpuAddressEvent, CpuAddressEventKind, CpuAddressUpdateDirection, CpuBusAccessKind,
        CpuBusActivitySnapshot, ExecutionMode, ExternalPortAttachmentKind, JoypadSnapshot,
        JoypadStatus, LinkedTopologyKind, Machine, MachineConfig, MachineStepRegion,
        PersistentCartState, PpuFramebufferLayerSource, PpuStepRegion, PrinterCommand, StartupMode,
        TraceSummaryBuffer,
    };
    use gb_desktop::{
        BootRomVerificationMode, DesktopConfig, DesktopConsoleModel, DesktopExternalPortSelection,
        DesktopKey, DesktopSaveFlushPolicy, GamepadButtonBinding, GamepadDirectionalSource,
        GamepadMenuBindings, GamepadRumbleMode, MenuKeyboardBindings,
    };
    use sdl3::dialog::DialogError;
    use sdl3::event::Event;
    use sdl3::gamepad::Button;
    use sdl3::joystick::JoystickId;
    use sdl3::keyboard::{Keycode, Mod, Scancode};
    use sdl3::render::Canvas;
    use sdl3::video::Window;
    use std::collections::VecDeque;
    use std::ffi::CString;
    use std::ffi::OsStr;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
    const ENTRY_POINT_START: usize = 0x0100;
    const LOGO_START: usize = 0x0104;
    const TITLE_START: usize = 0x0134;
    const CGB_FLAG_ADDRESS: usize = 0x0143;
    const SGB_FLAG_ADDRESS: usize = 0x0146;
    const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
    const ROM_SIZE_ADDRESS: usize = 0x0148;
    const RAM_SIZE_ADDRESS: usize = 0x0149;
    const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;

    fn temp_test_root(prefix: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "gb-cycle-desktop-main-tests-{prefix}-{}-{id}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temp root should be removable");
        }
        fs::create_dir_all(&root).expect("temp root should be creatable");
        root
    }

    fn build_test_rom(
        len: usize,
        cartridge_type: u8,
        rom_size_code: u8,
        ram_size_code: u8,
    ) -> Vec<u8> {
        let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
        rom[0x0000] = 0x12;
        rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
        rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
        rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"DESKTOP!");
        rom[CGB_FLAG_ADDRESS] = 0x80;
        rom[SGB_FLAG_ADDRESS] = 0x03;
        rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
        rom[ROM_SIZE_ADDRESS] = rom_size_code;
        rom[RAM_SIZE_ADDRESS] = ram_size_code;
        rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
        rom
    }

    fn write_test_rom(root: &Path, name: &str) -> PathBuf {
        let rom_path = root.join(name);
        fs::write(&rom_path, build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
            .expect("test ROM should be writable");
        rom_path
    }

    fn serial_transfer_byte(machine: &mut Machine<TraceSummaryBuffer>, outgoing_byte: u8) -> u8 {
        machine.write_bus(0xFF01, outgoing_byte);
        machine.write_bus(0xFF02, 0x81);

        while !matches!(
            machine.serial().transfer_state(),
            gb_core::SerialTransferState::Idle
        ) {
            machine.step_t_cycle();
        }

        machine.read_bus(0xFF01)
    }

    fn printer_packet(command: PrinterCommand, data: &[u8]) -> Vec<u8> {
        let mut packet = vec![
            0x88,
            0x33,
            command as u8,
            0x00,
            (data.len() & 0xFF) as u8,
            ((data.len() >> 8) & 0xFF) as u8,
        ];
        packet.extend_from_slice(data);
        let checksum = packet[2..]
            .iter()
            .fold(0u16, |sum, &byte| sum.wrapping_add(byte as u16));
        packet.push((checksum & 0xFF) as u8);
        packet.push((checksum >> 8) as u8);
        packet
    }

    fn send_printer_packet(
        machine: &mut Machine<TraceSummaryBuffer>,
        command: PrinterCommand,
        data: &[u8],
    ) -> Vec<u8> {
        printer_packet(command, data)
            .into_iter()
            .map(|byte| serial_transfer_byte(machine, byte))
            .collect()
    }

    fn run_print_sequence(machine: &mut Machine<TraceSummaryBuffer>) {
        let tile_row = vec![0xFF; 320];

        send_printer_packet(machine, PrinterCommand::Data, &tile_row);
        assert_eq!(serial_transfer_byte(machine, 0x00), 0x81);
        assert_eq!(serial_transfer_byte(machine, 0x00), 0x08);

        send_printer_packet(machine, PrinterCommand::Data, &[]);
        serial_transfer_byte(machine, 0x00);
        serial_transfer_byte(machine, 0x00);

        send_printer_packet(machine, PrinterCommand::Print, &[0x01, 0x13, 0xE4, 0x40]);
        assert_eq!(serial_transfer_byte(machine, 0x00), 0x81);
        assert_eq!(serial_transfer_byte(machine, 0x00), 0x08);

        send_printer_packet(machine, PrinterCommand::Status, &[]);
        serial_transfer_byte(machine, 0x00);
        assert_eq!(serial_transfer_byte(machine, 0x00), 0x06);

        send_printer_packet(machine, PrinterCommand::Status, &[]);
        serial_transfer_byte(machine, 0x00);
        assert_eq!(serial_transfer_byte(machine, 0x00), 0x04);
    }

    fn dmg_skip_boot_summary_machine() -> Machine<TraceSummaryBuffer> {
        Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        )
    }

    fn schedule_quit_event() -> thread::JoinHandle<()> {
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(75));
            crate::configure_headless_sdl();
            let sdl = sdl3::init().expect("SDL should initialize for quit-event helper");
            let events = sdl
                .event()
                .expect("SDL event subsystem should initialize for quit-event helper");
            events
                .push_event(Event::Quit { timestamp: 0 })
                .expect("quit event should be pushable");
        })
    }

    #[test]
    fn desktop_emulation_session_can_wrap_a_two_console_dmg04_runtime() {
        let primary = dmg_skip_boot_summary_machine();
        let secondary = dmg_skip_boot_summary_machine();

        let linked = super::linked_session::DesktopEmulationSession::new_linked_dmg04_two_player(
            primary, secondary,
        )
        .expect("desktop linked session should build from two aligned machines");

        assert_eq!(
            linked.kind(),
            super::linked_session::DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
        );
        assert_eq!(linked.linked_topology_kind(), LinkedTopologyKind::Dmg04);
        assert_eq!(
            linked.external_port().attachment_kind(),
            ExternalPortAttachmentKind::GameLinkDmg04
        );
        assert_eq!(
            linked
                .secondary_machine()
                .expect("secondary machine should exist")
                .external_port()
                .attachment_kind(),
            ExternalPortAttachmentKind::GameLinkDmg04
        );
    }

    #[test]
    fn desktop_emulation_session_can_return_to_a_single_primary_machine() {
        let primary = dmg_skip_boot_summary_machine();
        let secondary = dmg_skip_boot_summary_machine();

        let mut linked =
            super::linked_session::DesktopEmulationSession::new_linked_dmg04_two_player(
                primary, secondary,
            )
            .expect("desktop linked session should build from two aligned machines");

        linked.step_t_cycle();
        let primary_wram_before = linked.read_bus(0xC000);
        linked
            .secondary_machine_mut()
            .expect("secondary machine should exist")
            .write_bus(0xC000, 0x3C);

        let mut primary = linked.into_primary_machine();

        assert_eq!(primary.next_t_cycle(), gb_core::TCycle::new(1));
        assert_eq!(
            primary.external_port().attachment_kind(),
            ExternalPortAttachmentKind::None
        );
        assert_eq!(primary.read_bus(0xC000), primary_wram_before);
    }

    #[test]
    fn game_link_menu_action_loads_a_secondary_rom_into_a_linked_runtime() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("game-link-activate", true, false, false);
        let secondary_rom_path = harness.root.join("linked-secondary.gb");
        fs::write(
            &secondary_rom_path,
            build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        )
        .expect("secondary link ROM should be writable");

        harness.runtime.open_rom_dialog_mode = super::OpenRomDialogMode::LinkedSecondary;

        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Selected(secondary_rom_path.clone()))
            .expect("secondary ROM selection should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("secondary ROM selection should activate GAME LINK");

        assert_eq!(
            harness.session.external_port_selection,
            DesktopExternalPortSelection::GameLink
        );
        assert_eq!(
            harness.session.linked_secondary_rom_path(),
            Some(secondary_rom_path.as_path())
        );
        assert_eq!(
            harness.machine.kind(),
            super::linked_session::DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
        );
        assert_eq!(
            harness.machine.linked_topology_kind(),
            LinkedTopologyKind::Dmg04
        );
        assert_eq!(
            harness.machine.external_port().attachment_kind(),
            ExternalPortAttachmentKind::GameLinkDmg04
        );
        assert_eq!(
            harness
                .machine
                .secondary_machine()
                .expect("secondary linked machine should exist")
                .external_port()
                .attachment_kind(),
            ExternalPortAttachmentKind::GameLinkDmg04
        );
    }

    #[test]
    fn selecting_none_after_game_link_returns_to_a_single_primary_runtime() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("game-link-detach", true, false, false);
        let secondary_rom_path = harness.root.join("linked-secondary.gb");
        fs::write(
            &secondary_rom_path,
            build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        )
        .expect("secondary link ROM should be writable");

        harness.runtime.open_rom_dialog_mode = super::OpenRomDialogMode::LinkedSecondary;
        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Selected(secondary_rom_path))
            .expect("secondary ROM selection should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("secondary ROM selection should activate GAME LINK");
        harness.machine.write_bus(0xC000, 0x5A);
        harness
            .machine
            .secondary_machine_mut()
            .expect("secondary linked machine should exist")
            .write_bus(0xC000, 0x99);

        assert!(
            harness
                .execute_action(super::MenuAction::SetExternalPort(
                    DesktopExternalPortSelection::None,
                ))
                .expect("returning to NONE should tear down GAME LINK")
                .is_none()
        );

        assert_eq!(
            harness.session.external_port_selection,
            DesktopExternalPortSelection::None
        );
        assert!(harness.session.linked_secondary_rom.is_none());
        assert_eq!(
            harness.machine.kind(),
            super::linked_session::DesktopEmulationSessionKind::Single
        );
        assert_eq!(
            harness.machine.linked_topology_kind(),
            LinkedTopologyKind::None
        );
        assert_eq!(
            harness.machine.external_port().attachment_kind(),
            ExternalPortAttachmentKind::None
        );
        assert_eq!(harness.machine.read_bus(0xC000), 0x5A);
    }

    #[test]
    fn game_link_activation_rebuilds_an_advanced_primary_into_a_fresh_linked_session() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("game-link-fresh-sync", true, false, false);
        let secondary_rom_path = harness.root.join("linked-secondary.gb");
        fs::write(
            &secondary_rom_path,
            build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        )
        .expect("secondary link ROM should be writable");

        for _ in 0..256 {
            harness.machine.step_t_cycle();
        }
        assert_ne!(harness.machine.next_t_cycle(), gb_core::TCycle::ZERO);

        harness.runtime.open_rom_dialog_mode = super::OpenRomDialogMode::LinkedSecondary;
        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Selected(secondary_rom_path))
            .expect("secondary ROM selection should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("advanced primary should still activate GAME LINK");

        assert_eq!(
            harness.machine.kind(),
            super::linked_session::DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
        );
        assert_eq!(
            harness.machine.primary_machine().next_t_cycle(),
            gb_core::TCycle::ZERO
        );
        assert_eq!(
            harness
                .machine
                .secondary_machine()
                .expect("secondary linked machine should exist")
                .next_t_cycle(),
            gb_core::TCycle::ZERO
        );
    }

    #[test]
    fn reset_keeps_the_linked_runtime_active_for_game_link_sessions() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("game-link-reset", true, false, false);
        let secondary_rom_path = harness.root.join("linked-secondary.gb");
        fs::write(
            &secondary_rom_path,
            build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        )
        .expect("secondary link ROM should be writable");

        harness.runtime.open_rom_dialog_mode = super::OpenRomDialogMode::LinkedSecondary;
        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Selected(secondary_rom_path))
            .expect("secondary ROM selection should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("secondary ROM selection should activate GAME LINK");

        let primary_reset_baseline = harness.machine.read_bus(0xC000);
        let secondary_reset_baseline = harness
            .machine
            .secondary_machine_mut()
            .expect("secondary linked machine should exist")
            .read_bus(0xC000);
        harness.machine.write_bus(0xC000, 0xA5);
        harness
            .machine
            .secondary_machine_mut()
            .expect("secondary linked machine should exist")
            .write_bus(0xC000, 0x3C);

        super::reset_machine(
            harness.canvas.window(),
            &mut harness.session,
            &mut harness.machine,
            &mut harness.runtime,
            &mut harness.settings_store,
        )
        .expect("linked reset should succeed");

        assert_eq!(
            harness.session.external_port_selection,
            DesktopExternalPortSelection::GameLink
        );
        assert!(harness.session.linked_secondary_rom.is_some());
        assert_eq!(
            harness.machine.kind(),
            super::linked_session::DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
        );
        assert_eq!(harness.machine.read_bus(0xC000), primary_reset_baseline);
        assert_eq!(
            harness
                .machine
                .secondary_machine_mut()
                .expect("secondary linked machine should exist")
                .read_bus(0xC000),
            secondary_reset_baseline
        );
    }

    #[test]
    fn game_link_menu_action_switches_the_open_rom_dialog_into_secondary_mode() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("game-link-action", true, false, false);
        harness.runtime.open_rom_dialog.pending = true;

        assert!(
            harness
                .execute_action(super::MenuAction::SetExternalPort(
                    DesktopExternalPortSelection::GameLink,
                ))
                .expect("GAME LINK action should not fail when the open dialog is already pending")
                .is_none()
        );
        assert_eq!(
            harness.runtime.open_rom_dialog_mode,
            super::OpenRomDialogMode::LinkedSecondary
        );
    }

    fn push_key_event(events: &sdl3::EventSubsystem, keycode: Keycode, down: bool) {
        let scancode = desktop_key_from_keycode(keycode)
            .map(desktop_key_scancode)
            .unwrap_or_else(|| keycode_to_test_scancode(keycode));
        push_key_event_with_scancode(events, keycode, scancode, down);
    }

    fn push_key_event_with_scancode(
        events: &sdl3::EventSubsystem,
        keycode: Keycode,
        scancode: Scancode,
        down: bool,
    ) {
        let event = if down {
            Event::KeyDown {
                timestamp: 0,
                window_id: 0,
                keycode: Some(keycode),
                scancode: Some(scancode),
                keymod: Mod::NOMOD,
                repeat: false,
                which: 0,
                raw: 0,
            }
        } else {
            Event::KeyUp {
                timestamp: 0,
                window_id: 0,
                keycode: Some(keycode),
                scancode: Some(scancode),
                keymod: Mod::NOMOD,
                repeat: false,
                which: 0,
                raw: 0,
            }
        };

        events
            .push_event(event)
            .expect("keyboard event should be pushable");
    }

    fn keycode_to_test_scancode(keycode: Keycode) -> Scancode {
        match keycode {
            Keycode::A => Scancode::A,
            Keycode::C => Scancode::C,
            Keycode::D => Scancode::D,
            Keycode::E => Scancode::E,
            Keycode::Q => Scancode::Q,
            Keycode::S => Scancode::S,
            Keycode::V => Scancode::V,
            Keycode::W => Scancode::W,
            _ => panic!("test keycode should map to a desktop key"),
        }
    }

    fn schedule_key_sequence(sequence: Vec<(Keycode, bool)>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(75));
            crate::configure_headless_sdl();
            let sdl = sdl3::init().expect("SDL should initialize for key-sequence helper");
            let events = sdl
                .event()
                .expect("SDL event subsystem should initialize for key-sequence helper");
            for (keycode, down) in sequence {
                push_key_event(&events, keycode, down);
                thread::sleep(Duration::from_millis(8));
            }
            events
                .push_event(Event::Quit { timestamp: 0 })
                .expect("quit event should be pushable");
        })
    }

    fn wait_for_profiled_counter_sample(counter: &mut super::PerformanceCounter) {
        for _ in 0..200 {
            counter.collect_emulation_profile_results();
            if counter.sample_profiled_frames > 0 {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("timed out waiting for an async emulation profile sample");
    }

    struct VirtualGamepad {
        joystick_id: JoystickId,
        raw: *mut sdl3::sys::joystick::SDL_Joystick,
        _name: CString,
    }

    impl VirtualGamepad {
        fn attach(name: &str) -> Self {
            let name = CString::new(name).expect("virtual gamepad name");
            let mut descriptor = sdl3::sys::joystick::SDL_VirtualJoystickDesc::new();
            descriptor.r#type = sdl3::sys::joystick::SDL_JOYSTICK_TYPE_GAMEPAD.0 as u16;
            descriptor.naxes = 0;
            descriptor.nbuttons = 16;
            descriptor.button_mask = (1 << Button::Guide as u32)
                | (1 << Button::South as u32)
                | (1 << Button::East as u32)
                | (1 << Button::DPadDown as u32)
                | (1 << Button::North as u32);
            descriptor.name = name.as_ptr();

            let joystick_id =
                unsafe { sdl3::sys::joystick::SDL_AttachVirtualJoystick(&descriptor) };
            assert_ne!(joystick_id.0, 0, "failed to attach a virtual SDL gamepad");
            let raw = unsafe { sdl3::sys::joystick::SDL_OpenJoystick(joystick_id) };
            assert!(!raw.is_null(), "failed to open the virtual SDL gamepad");

            Self {
                joystick_id,
                raw,
                _name: name,
            }
        }
    }

    impl Drop for VirtualGamepad {
        fn drop(&mut self) {
            unsafe {
                sdl3::sys::joystick::SDL_CloseJoystick(self.raw);
                let _ = sdl3::sys::joystick::SDL_DetachVirtualJoystick(self.joystick_id);
            }
        }
    }

    #[allow(dead_code)]
    struct FrontendHarness {
        root: PathBuf,
        settings_path: PathBuf,
        sdl: sdl3::Sdl,
        canvas: Canvas<Window>,
        event_pump: sdl3::EventPump,
        session: super::DesktopSession,
        machine: super::DesktopEmulationSession,
        runtime: super::FrontendRuntime,
        settings_store: DesktopSettingsStore,
        performance_counter: super::PerformanceCounter,
        frame_pacer: super::FramePacer,
        _gamepad_subsystem: Option<sdl3::GamepadSubsystem>,
    }

    #[allow(dead_code)]
    impl FrontendHarness {
        fn new(name: &str, with_rom: bool, with_audio: bool, with_gamepad: bool) -> Self {
            crate::configure_headless_sdl();
            let root = temp_test_root(name);
            let settings_path = root.join("desktop-settings.toml");
            let mut config = DesktopConfig::default();
            config.boot_rom.verification = BootRomVerificationMode::Off;
            config.audio.enabled = with_audio;
            config.input.gamepad.enabled = with_gamepad;
            if with_gamepad {
                config.input.gamepad.preferred_device.name = Some("Saved Pad".to_string());
            }
            let boot_rom_root = root.join(DEFAULT_BOOT_ROM_DIR);
            fs::create_dir_all(&boot_rom_root).expect("frontend harness boot ROM root");
            for file_name in ["dmg0_boot.bin", "dmg_boot.bin", "mgb_boot.bin"] {
                fs::write(boot_rom_root.join(file_name), vec![0_u8; 0x0100])
                    .expect("frontend harness boot ROM image");
            }

            let rom_path = root.join(format!("{name}.gb"));
            let rom_bytes = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
            fs::write(&rom_path, &rom_bytes).expect("frontend harness ROM should be writable");
            let loaded_rom = with_rom.then_some(super::LoadedRom {
                path: rom_path,
                bytes: rom_bytes.clone(),
            });
            let current_dir = root.clone();
            let mut machine = if with_rom {
                super::DesktopEmulationSession::new_single(
                    super::load_machine_for_rom(&config, &current_dir, &rom_bytes)
                        .expect("frontend harness machine should load")
                        .machine,
                )
            } else {
                super::DesktopEmulationSession::new_single(Machine::new_summary(
                    MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
                ))
            };

            let session = super::DesktopSession {
                config: config.clone(),
                current_dir,
                loaded_rom,
                linked_secondary_rom: None,
                last_open_directory: Some(root.clone()),
                recent_roms: Vec::new(),
                external_port_selection: DesktopExternalPortSelection::None,
            };

            let sdl = sdl3::init().expect("frontend harness SDL should initialize");
            let mut input_state = super::FrontendInputState::new();
            let audio_output = if with_audio {
                Some(
                    super::DesktopAudioOutput::new(
                        &sdl.audio().expect("frontend harness audio subsystem"),
                        &config.audio,
                        super::audio_source_machine(&machine).apu().console_model(),
                    )
                    .expect("frontend harness audio output"),
                )
            } else {
                None
            };
            let gamepad_subsystem = if with_gamepad {
                Some(sdl.gamepad().expect("frontend harness gamepad subsystem"))
            } else {
                None
            };
            let gamepad_manager = gamepad_subsystem.as_ref().map(|subsystem| {
                super::GamepadManager::new(
                    subsystem,
                    config.input.gamepad.clone(),
                    &mut input_state,
                    &mut machine,
                )
                .expect("frontend harness gamepad manager")
            });

            let video = sdl.video().expect("frontend harness video subsystem");
            let window = video
                .window("frontend-harness", 160 * 4, 144 * 4)
                .build()
                .expect("frontend harness window");
            let mut canvas = window.into_canvas();
            let mut frame_pacer = super::FramePacer::new(config.video.vsync);
            super::apply_renderer_vsync(&mut canvas, &mut frame_pacer, config.video.vsync)
                .expect("frontend harness vsync");
            let event_pump = sdl.event_pump().expect("frontend harness event pump");
            let settings_store = DesktopSettingsStore::new_for_tests(settings_path.clone());
            let performance_counter = super::PerformanceCounter::new_with_emulation_profile_mode(
                super::window_title(&session, &config),
                super::EmulationProfileMode::Disabled,
            );
            let save_session = super::open_save_session_for_session(&session, &mut machine)
                .expect("frontend harness save session");
            let runtime = super::FrontendRuntime {
                paused: !with_rom,
                menu_state: super::OverlayMenuState::default(),
                input_state,
                secondary_input_state: super::FrontendInputState::new(),
                keyboard_bindings: config.input.keyboard,
                video_options: config.video.clone(),
                audio_volume_percent: config.audio.volume_percent,
                audio_channel_mask: super::ApuRecordedChannelMask::ALL,
                audio_output,
                audio_recording_mode: super::DesktopAudioRecordingMode::Disabled,
                audio_recorder: None,
                gamepad_manager,
                save_session,
                secondary_save_session: None,
                rtc_sync: super::HostRtcSync::from_host_clock(),
                open_rom_dialog: super::PathSelectionDialog::new(),
                open_rom_dialog_mode: super::OpenRomDialogMode::Primary,
                boot_rom_file_dialog: super::PathSelectionDialog::new(),
                boot_rom_directory_dialog: super::PathSelectionDialog::new(),
                save_directory_dialog: super::PathSelectionDialog::new(),
                trace_capture: super::DesktopTraceCapture {
                    enabled: false,
                    output_path: None,
                    max_t_cycles: super::DEFAULT_TRACE_CAPTURE_T_CYCLES,
                    records: VecDeque::new(),
                },
                ch4_nr43_trace: super::DesktopCh4Nr43TraceCapture {
                    output_path: None,
                    records: Vec::new(),
                },
                printer_output: super::PrinterOutputState::default(),
            };

            Self {
                root,
                settings_path,
                sdl,
                canvas,
                event_pump,
                session,
                machine,
                runtime,
                settings_store,
                performance_counter,
                frame_pacer,
                _gamepad_subsystem: gamepad_subsystem,
            }
        }

        fn execute_action(
            &mut self,
            action: super::MenuAction,
        ) -> Result<Option<super::LoopSignal>, String> {
            let mut context = super::FrontendActionContext {
                session: &mut self.session,
                machine: &mut self.machine,
                runtime: &mut self.runtime,
                performance_counter: &mut self.performance_counter,
                frame_pacer: &mut self.frame_pacer,
                settings_store: &mut self.settings_store,
            };
            super::execute_menu_action(action, &self.event_pump, &mut self.canvas, &mut context)
        }

        fn push_key(&self, keycode: Keycode, down: bool) {
            let events = self
                .sdl
                .event()
                .expect("frontend harness event subsystem should initialize");
            push_key_event(&events, keycode, down);
        }

        fn push_key_with_scancode(&self, keycode: Keycode, scancode: Scancode, down: bool) {
            let events = self
                .sdl
                .event()
                .expect("frontend harness event subsystem should initialize");
            push_key_event_with_scancode(&events, keycode, scancode, down);
        }

        fn process_events(&mut self) -> Result<super::LoopSignal, String> {
            let mut context = super::FrontendActionContext {
                session: &mut self.session,
                machine: &mut self.machine,
                runtime: &mut self.runtime,
                performance_counter: &mut self.performance_counter,
                frame_pacer: &mut self.frame_pacer,
                settings_store: &mut self.settings_store,
            };
            super::process_events(&mut self.event_pump, &mut self.canvas, &mut context)
        }

        fn step_until_next_frame(&mut self) -> Result<super::LoopSignal, String> {
            let mut context = super::FrontendActionContext {
                session: &mut self.session,
                machine: &mut self.machine,
                runtime: &mut self.runtime,
                performance_counter: &mut self.performance_counter,
                frame_pacer: &mut self.frame_pacer,
                settings_store: &mut self.settings_store,
            };
            super::step_until_next_frame(&mut self.event_pump, &mut self.canvas, &mut context)
                .map(|result| result.signal)
        }

        fn process_pending_open_rom_dialog(&mut self) -> Result<(), String> {
            let mut context = super::FrontendActionContext {
                session: &mut self.session,
                machine: &mut self.machine,
                runtime: &mut self.runtime,
                performance_counter: &mut self.performance_counter,
                frame_pacer: &mut self.frame_pacer,
                settings_store: &mut self.settings_store,
            };
            super::process_pending_open_rom_dialog(&self.event_pump, &mut self.canvas, &mut context)
        }

        fn process_pending_boot_rom_file_dialog(&mut self) -> Result<(), String> {
            let mut context = super::FrontendActionContext {
                session: &mut self.session,
                machine: &mut self.machine,
                runtime: &mut self.runtime,
                performance_counter: &mut self.performance_counter,
                frame_pacer: &mut self.frame_pacer,
                settings_store: &mut self.settings_store,
            };
            super::process_pending_boot_rom_file_dialog(&mut self.canvas, &mut context)
        }

        fn process_pending_boot_rom_directory_dialog(&mut self) -> Result<(), String> {
            let mut context = super::FrontendActionContext {
                session: &mut self.session,
                machine: &mut self.machine,
                runtime: &mut self.runtime,
                performance_counter: &mut self.performance_counter,
                frame_pacer: &mut self.frame_pacer,
                settings_store: &mut self.settings_store,
            };
            super::process_pending_boot_rom_directory_dialog(&mut self.canvas, &mut context)
        }

        fn process_pending_save_directory_dialog(&mut self) -> Result<(), String> {
            let mut context = super::FrontendActionContext {
                session: &mut self.session,
                machine: &mut self.machine,
                runtime: &mut self.runtime,
                performance_counter: &mut self.performance_counter,
                frame_pacer: &mut self.frame_pacer,
                settings_store: &mut self.settings_store,
            };
            super::process_pending_save_directory_dialog(&mut self.canvas, &mut context)
        }
    }

    #[test]
    fn performance_window_title_formats_the_runtime_metrics() {
        assert_eq!(
            performance_window_title(
                "gb-desktop | drmario.gb | dmg | real-boot | strict",
                PerformanceHudSnapshot {
                    fps: 14.8,
                    speed_percent: 25.0,
                    frame_time_ms: 67.5,
                    emulation_time_ms: 54.2,
                    render_time_ms: 4.1,
                    pacing_time_ms: 9.2,
                    audio_queue_ms: Some(18.4),
                }
            ),
            "gb-desktop | drmario.gb | dmg | real-boot | strict | 14.8 FPS | 67.50 ms | 25% speed | emu 54.20 | render 4.10 | pacing 9.20 | audio 18.4 ms"
        );
    }

    #[test]
    fn audio_queue_pacing_correction_ignores_nominal_latency_and_caps_large_backlogs() {
        assert_eq!(
            super::audio_queue_pacing_correction_with_policy(None, true),
            Duration::ZERO
        );
        assert_eq!(
            super::audio_queue_pacing_correction_with_policy(
                Some(super::AUDIO_QUEUE_TARGET_MS + super::AUDIO_QUEUE_DEADBAND_MS,),
                true,
            ),
            Duration::ZERO
        );

        let modest_correction = super::audio_queue_pacing_correction_with_policy(
            Some(super::AUDIO_QUEUE_TARGET_MS + super::AUDIO_QUEUE_DEADBAND_MS + 20.0),
            true,
        );
        assert!(modest_correction > Duration::ZERO);
        assert_eq!(modest_correction, Duration::from_millis(2));

        assert_eq!(
            super::audio_queue_pacing_correction_with_policy(Some(2_000.0), true),
            Duration::from_secs_f64(super::AUDIO_QUEUE_MAX_CORRECTION_MS / 1_000.0)
        );
        assert_eq!(
            super::audio_queue_pacing_correction_with_policy(Some(2_000.0), false),
            Duration::ZERO
        );
    }

    #[test]
    fn audio_queue_pacing_correction_policy_from_env_value_accepts_disable_tokens() {
        assert_eq!(
            super::AudioQueuePacingCorrectionPolicy::from_env_value(None),
            super::AudioQueuePacingCorrectionPolicy::Enabled
        );
        assert_eq!(
            super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new(""))),
            super::AudioQueuePacingCorrectionPolicy::Disabled
        );
        assert_eq!(
            super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new("1"))),
            super::AudioQueuePacingCorrectionPolicy::Disabled
        );
        assert_eq!(
            super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new("true"))),
            super::AudioQueuePacingCorrectionPolicy::Disabled
        );
        assert_eq!(
            super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new("disabled"))),
            super::AudioQueuePacingCorrectionPolicy::Disabled
        );
        assert_eq!(
            super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new("0"))),
            super::AudioQueuePacingCorrectionPolicy::Enabled
        );
        assert_eq!(
            super::AudioQueuePacingCorrectionPolicy::from_env_value(Some(OsStr::new("off"))),
            super::AudioQueuePacingCorrectionPolicy::Enabled
        );
    }

    #[test]
    fn emulation_profile_mode_from_env_value_accepts_common_toggle_tokens() {
        assert_eq!(
            super::EmulationProfileMode::from_env_value(None),
            super::EmulationProfileMode::Disabled
        );
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("0"))),
            super::EmulationProfileMode::Disabled
        );
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("off"))),
            super::EmulationProfileMode::Disabled
        );
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("disabled"))),
            super::EmulationProfileMode::Disabled
        );
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("1"))),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            }
        );
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary"))),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            }
        );
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary:8"))),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: 8,
            }
        );
    }

    #[test]
    fn emulation_profile_summary_reports_core_frontend_and_other_buckets() {
        let mut counter = super::PerformanceCounter::new_with_emulation_profile_mode(
            "gb-desktop | no rom".to_string(),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            },
        );
        counter.frames_in_sample = 2;
        counter.sample_emulation_duration = Duration::from_millis(22);
        counter.sample_present_duration = Duration::from_millis(2);
        counter.sample_pacing_duration = Duration::from_millis(4);
        counter.sample_pacing_sleep_target_duration = Duration::from_millis(4);
        counter.sample_pacing_audio_correction_duration = Duration::from_millis(1);
        counter.sample_pacing_late_duration = Duration::from_millis(2);
        counter.sample_pacing_oversleep_duration = Duration::from_millis(1);
        counter.sample_audio_submit_sample_count = 1_608;
        counter.sample_audio_submit_sample_count_observations = 2;
        counter.sample_audio_submit_t_cycles = 140_448;
        counter.sample_audio_submit_t_cycles_observations = 2;
        counter.sample_audio_submit_queue_before_ms = 48.0;
        counter.sample_audio_submit_queue_before_observations = 2;
        counter.sample_audio_submit_enqueued_ms = 8.0;
        counter.sample_audio_submit_enqueued_observations = 2;
        counter.sample_audio_submit_queue_after_ms = 56.0;
        counter.sample_audio_submit_queue_after_observations = 2;
        counter.sample_audio_queue_before_pacing_ms = 40.0;
        counter.sample_audio_queue_before_pacing_observations = 2;
        counter.sample_audio_queue_after_pacing_ms = 36.0;
        counter.sample_audio_queue_after_pacing_observations = 2;
        counter.sample_frame_step_t_cycles = 140_448;
        counter.sample_frame_step_t_cycles_observations = 2;
        counter.sample_frame_start_ly = 0;
        counter.sample_frame_start_ly_observations = 2;
        counter.sample_frame_start_dot = 0;
        counter.sample_frame_start_dot_observations = 2;
        counter.sample_frame_end_ly = 0;
        counter.sample_frame_end_ly_observations = 2;
        counter.sample_frame_end_dot = 0;
        counter.sample_frame_end_dot_observations = 2;
        counter.sample_frame_origin_crossings = 2;
        counter.sample_frame_origin_crossings_observations = 2;
        counter.sample_scanline_transitions = 308;
        counter.sample_scanline_transitions_observations = 2;
        counter.sample_scanlines_over_456 = 0;
        counter.sample_scanlines_over_456_observations = 2;
        counter.sample_max_scanline_t_cycles = 912;
        counter.sample_max_scanline_t_cycles_observations = 2;
        counter.sample_max_scanline_ly = 306;
        counter.sample_max_scanline_ly_observations = 2;
        counter.sample_max_mode0_start_dot = 504;
        counter.sample_max_mode0_start_dot_observations = 2;
        counter.sample_max_mode0_start_dot_ly = 10;
        counter.sample_max_mode0_start_dot_ly_observations = 2;
        counter.sample_ly_153_to_0_transitions = 2;
        counter.sample_ly_153_to_0_transitions_observations = 2;
        counter.sample_ly_153_to_0_startup_mode0 = 0;
        counter.sample_ly_153_to_0_startup_mode0_observations = 2;
        counter.sample_ly_153_to_0_blank_frame = 0;
        counter.sample_ly_153_to_0_blank_frame_observations = 2;
        counter.sample_ly_0_self_wraps = 0;
        counter.sample_ly_0_self_wraps_observations = 2;
        counter.sample_ly_0_self_wrap_startup_mode0 = 0;
        counter.sample_ly_0_self_wrap_startup_mode0_observations = 2;
        counter.sample_ly_0_self_wrap_blank_frame = 0;
        counter.sample_ly_0_self_wrap_blank_frame_observations = 2;
        counter.sample_ly_0_to_1_transitions = 2;
        counter.sample_ly_0_to_1_transitions_observations = 2;
        counter.sample_ly_0_scanline_t_cycles = 912;
        counter.sample_ly_0_scanline_t_cycles_observations = 2;
        counter.sample_ly_0_max_mode0_start_dot = 508;
        counter.sample_ly_0_max_mode0_start_dot_observations = 2;
        counter.sample_ly_0_stall_t_cycles = 24;
        counter.sample_ly_0_stall_t_cycles_observations = 2;
        counter.sample_ly_0_stall_hblank_t_cycles = 16;
        counter.sample_ly_0_stall_hblank_t_cycles_observations = 2;
        counter.sample_ly_0_stall_oam_t_cycles = 6;
        counter.sample_ly_0_stall_oam_t_cycles_observations = 2;
        counter.sample_ly_0_stall_drawing_t_cycles = 2;
        counter.sample_ly_0_stall_drawing_t_cycles_observations = 2;
        counter.sample_ly_0_stall_startup_mode0_t_cycles = 4;
        counter.sample_ly_0_stall_startup_mode0_t_cycles_observations = 2;
        counter.sample_ly_0_stall_blank_frame_t_cycles = 0;
        counter.sample_ly_0_stall_blank_frame_t_cycles_observations = 2;
        counter.sample_ly_0_stall_runs = 2;
        counter.sample_ly_0_stall_runs_observations = 2;
        counter.sample_ly_0_max_stall_run_t_cycles = 18;
        counter.sample_ly_0_max_stall_run_t_cycles_observations = 2;
        counter.sample_ly_0_max_stall_dot = 224;
        counter.sample_ly_0_max_stall_dot_observations = 2;
        counter.sample_ly_0_max_stall_mode_dot = 42;
        counter.sample_ly_0_max_stall_mode_dot_observations = 2;
        counter.sample_cpu_stop_t_cycles = 10;
        counter.sample_cpu_stop_t_cycles_observations = 2;
        counter.sample_cpu_zombie_stop_t_cycles = 4;
        counter.sample_cpu_zombie_stop_t_cycles_observations = 2;
        counter.sample_ly_0_cpu_stop_t_cycles = 8;
        counter.sample_ly_0_cpu_stop_t_cycles_observations = 2;
        counter.sample_ly_0_cpu_zombie_stop_t_cycles = 2;
        counter.sample_ly_0_cpu_zombie_stop_t_cycles_observations = 2;
        counter.sample_ly_0_stall_cpu_stop_t_cycles = 6;
        counter.sample_ly_0_stall_cpu_stop_t_cycles_observations = 2;
        counter.sample_ly_0_stall_cpu_zombie_stop_t_cycles = 2;
        counter.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations = 2;
        counter.sample_lcd_disabled_t_cycles = 14;
        counter.sample_lcd_disabled_t_cycles_observations = 2;
        counter.sample_lcd_disable_transitions = 2;
        counter.sample_lcd_disable_transitions_observations = 2;
        counter.sample_lcd_enable_transitions = 2;
        counter.sample_lcd_enable_transitions_observations = 2;
        counter.sample_ly_0_lcd_disabled_t_cycles = 12;
        counter.sample_ly_0_lcd_disabled_t_cycles_observations = 2;
        counter.sample_ly_0_stall_lcd_disabled_t_cycles = 10;
        counter.sample_ly_0_stall_lcd_disabled_t_cycles_observations = 2;
        counter.sample_profiled_frames = 2;
        counter.sample_profiled_emulation_duration = Duration::from_millis(24);
        counter.sample_profiled_emulation_breakdown = super::EmulationBreakdownSample {
            core_external_events_duration: Duration::from_millis(1),
            core_timer_duration: Duration::from_millis(1),
            core_apu_duration: Duration::from_millis(1),
            core_dma_duration: Duration::from_millis(1),
            core_ppu_duration: Duration::from_millis(10),
            core_ppu_mode0_1_duration: Duration::from_millis(2),
            core_ppu_mode2_duration: Duration::from_millis(1),
            core_ppu_mode3_startup_duration: Duration::from_millis(1),
            core_ppu_bg_fetch_duration: Duration::from_millis(2),
            core_ppu_window_fetch_duration: Duration::from_millis(1),
            core_ppu_push_duration: Duration::from_millis(1),
            core_ppu_obj_fetch_duration: Duration::from_millis(1),
            core_ppu_pixel_transfer_duration: Duration::from_millis(0),
            core_cpu_duration: Duration::from_millis(4),
            core_serial_duration: Duration::from_millis(1),
            core_interrupts_duration: Duration::from_millis(1),
            host_event_poll_duration: Duration::from_millis(2),
            host_audio_submit_duration: Duration::from_millis(1),
            host_save_flush_duration: Duration::from_millis(1),
        };
        let elapsed = Duration::from_millis(34);
        let snapshot = counter.snapshot_from_elapsed(elapsed);
        let summary = counter
            .emulation_profile_summary(elapsed, snapshot)
            .expect("summary mode should render a profile line");

        assert!(summary.contains("session=single"));
        assert!(summary.contains("emu_ms=11.00"));
        assert!(summary.contains("sampled_frames=2"));
        assert!(summary.contains(&format!(
            "sample_every={}",
            super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES
        )));
        assert!(summary.contains("sampled_emu_ms=12.00"));
        assert!(summary.contains("core_est_ms=10.00"));
        assert!(summary.contains("ppu_ms=5.00"));
        assert!(summary.contains("cpu_ms=2.00"));
        assert!(summary.contains("core_other_ms=3.00"));
        assert!(summary.contains("ext_ms=0.50"));
        assert!(summary.contains("timer_ms=0.50"));
        assert!(summary.contains("apu_ms=0.50"));
        assert!(summary.contains("dma_ms=0.50"));
        assert!(summary.contains("serial_ms=0.50"));
        assert!(summary.contains("irq_ms=0.50"));
        assert!(summary.contains("ppu_mode0_1_ms=1.00"));
        assert!(summary.contains("ppu_mode2_ms=0.50"));
        assert!(summary.contains("ppu_mode3_startup_ms=0.50"));
        assert!(summary.contains("ppu_bg_ms=1.00"));
        assert!(summary.contains("ppu_win_ms=0.50"));
        assert!(summary.contains("ppu_push_ms=0.50"));
        assert!(summary.contains("ppu_obj_ms=0.50"));
        assert!(summary.contains("ppu_px_ms=0.00"));
        assert!(summary.contains("ppu_other_ms=0.50"));
        assert!(summary.contains("host_ms=2.00"));
        assert!(summary.contains("poll_ms=1.00"));
        assert!(summary.contains("audsubmit_ms=0.50"));
        assert!(summary.contains("save_ms=0.50"));
        assert!(summary.contains("frame_tcycles=70224.00"));
        assert!(summary.contains("frame_start_ly=0.00"));
        assert!(summary.contains("frame_start_dot=0.00"));
        assert!(summary.contains("frame_end_ly=0.00"));
        assert!(summary.contains("frame_end_dot=0.00"));
        assert!(summary.contains("frame_crossings=1.00"));
        assert!(summary.contains("scanline_transitions=154.00"));
        assert!(summary.contains("scanlines_over_456=0.00"));
        assert!(summary.contains("max_scanline_tcycles=456.00"));
        assert!(summary.contains("max_scanline_ly=153.00"));
        assert!(summary.contains("max_mode0_start_dot=252.00"));
        assert!(summary.contains("max_mode0_start_dot_ly=5.00"));
        assert!(summary.contains("ly153_to0=1.00"));
        assert!(summary.contains("ly153_to0_startup=0.00"));
        assert!(summary.contains("ly153_to0_blank=0.00"));
        assert!(summary.contains("ly0_self_wraps=0.00"));
        assert!(summary.contains("ly0_self_wrap_startup=0.00"));
        assert!(summary.contains("ly0_self_wrap_blank=0.00"));
        assert!(summary.contains("ly0_to1=1.00"));
        assert!(summary.contains("ly0_tcycles=456.00"));
        assert!(summary.contains("ly0_max_mode0_start_dot=254.00"));
        assert!(summary.contains("ly0_stall_tcycles=12.00"));
        assert!(summary.contains("ly0_stall_hb_tcycles=8.00"));
        assert!(summary.contains("ly0_stall_oam_tcycles=3.00"));
        assert!(summary.contains("ly0_stall_draw_tcycles=1.00"));
        assert!(summary.contains("ly0_stall_startup_tcycles=2.00"));
        assert!(summary.contains("ly0_stall_blank_tcycles=0.00"));
        assert!(summary.contains("ly0_stall_runs=1.00"));
        assert!(summary.contains("ly0_max_stall_tcycles=9.00"));
        assert!(summary.contains("ly0_max_stall_dot=112.00"));
        assert!(summary.contains("ly0_max_stall_mode_dot=21.00"));
        assert!(summary.contains("cpu_stop_tcycles=5.00"));
        assert!(summary.contains("cpu_zstop_tcycles=2.00"));
        assert!(summary.contains("ly0_stop_tcycles=4.00"));
        assert!(summary.contains("ly0_zstop_tcycles=1.00"));
        assert!(summary.contains("ly0_stall_stop_tcycles=3.00"));
        assert!(summary.contains("ly0_stall_zstop_tcycles=1.00"));
        assert!(summary.contains("lcdoff_tcycles=7.00"));
        assert!(summary.contains("lcdoff_transitions=1.00"));
        assert!(summary.contains("lcdon_transitions=1.00"));
        assert!(summary.contains("ly0_lcdoff_tcycles=6.00"));
        assert!(summary.contains("ly0_stall_lcdoff_tcycles=5.00"));
        assert!(summary.contains("submit_samples=804.00"));
        assert!(summary.contains("submit_tcycles=70224.00"));
        assert!(summary.contains("submit_queue_before_ms=24.00"));
        assert!(summary.contains("submit_enqueued_ms=4.00"));
        assert!(summary.contains("submit_queue_after_ms=28.00"));
        assert!(summary.contains("audio_queue_before_ms=20.00"));
        assert!(summary.contains("audio_queue_after_ms=18.00"));
        assert!(summary.contains("present_ms=1.00"));
        assert!(summary.contains("pac_ms=2.00"));
        assert!(summary.contains("sleep_target_ms=2.00"));
        assert!(summary.contains("audio_corr_ms=0.50"));
        assert!(summary.contains("late_ms=1.00"));
        assert!(summary.contains("oversleep_ms=0.50"));
        let summary_without_audio = counter
            .emulation_profile_summary(
                elapsed,
                super::PerformanceHudSnapshot {
                    fps: 60.0,
                    speed_percent: 100.0,
                    frame_time_ms: 16.7,
                    emulation_time_ms: 10.0,
                    render_time_ms: 1.0,
                    pacing_time_ms: 5.0,
                    audio_queue_ms: None,
                },
            )
            .expect("summary mode should render a profile line without audio");
        assert!(summary_without_audio.contains("audio_queue_before_ms=20.00"));
        assert!(summary_without_audio.contains("audio_queue_after_ms=18.00"));

        let disabled = super::PerformanceCounter::new_with_emulation_profile_mode(
            "gb-desktop | no rom".to_string(),
            super::EmulationProfileMode::Disabled,
        );
        assert!(
            disabled
                .emulation_profile_summary(
                    elapsed,
                    super::PerformanceHudSnapshot {
                        fps: 60.0,
                        speed_percent: 100.0,
                        frame_time_ms: 16.7,
                        emulation_time_ms: 10.0,
                        render_time_ms: 1.0,
                        pacing_time_ms: 5.0,
                        audio_queue_ms: Some(18.0),
                    },
                )
                .is_none()
        );
    }

    #[test]
    fn emulation_profile_mode_and_breakdown_helpers_cover_all_sampling_buckets() {
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("sampled:7"))),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: 7,
            }
        );
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("every:9"))),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: 9,
            }
        );
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("stride:11"))),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: 11,
            }
        );
        assert_eq!(
            super::EmulationProfileMode::from_env_value(Some(OsStr::new("summary:0"))),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: super::DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES,
            }
        );

        let disabled = super::EmulationProfileMode::Disabled;
        assert!(!disabled.enabled());
        assert_eq!(disabled.sample_every_frames(), None);

        let sampled = super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 7,
        };
        assert!(sampled.enabled());
        assert_eq!(sampled.sample_every_frames(), Some(7));

        let mut breakdown = super::EmulationBreakdownSample::default();
        for (region, millis) in [
            (MachineStepRegion::ExternalEvents, 1),
            (MachineStepRegion::Timer, 2),
            (MachineStepRegion::Apu, 3),
            (MachineStepRegion::Dma, 4),
            (MachineStepRegion::Ppu, 8),
            (MachineStepRegion::Serial, 6),
            (MachineStepRegion::Cpu, 7),
            (MachineStepRegion::Interrupts, 8),
        ] {
            breakdown.add_core_region_duration(region, Duration::from_millis(millis));
        }
        breakdown.add_host_event_poll_duration(Duration::from_millis(9));
        breakdown.add_host_audio_submit_duration(Duration::from_millis(10));
        breakdown.add_host_save_flush_duration(Duration::from_millis(11));
        for (region, millis) in [
            (PpuStepRegion::Mode0Or1, 1),
            (PpuStepRegion::Mode2Scan, 1),
            (PpuStepRegion::Mode3Startup, 1),
            (PpuStepRegion::Mode3BgFetch, 1),
            (PpuStepRegion::Mode3WindowFetch, 1),
            (PpuStepRegion::Mode3Push, 1),
            (PpuStepRegion::Mode3ObjFetch, 1),
            (PpuStepRegion::Mode3PixelTransfer, 1),
        ] {
            breakdown.add_ppu_region_duration(region, Duration::from_millis(millis));
        }

        assert_eq!(breakdown.core_duration(), Duration::from_millis(39));
        assert_eq!(breakdown.host_duration(), Duration::from_millis(30));
        assert_eq!(breakdown.core_other_duration(), Duration::from_millis(24));
        assert_eq!(breakdown.ppu_profiled_duration(), Duration::from_millis(8));
        assert_eq!(breakdown.ppu_other_duration(), Duration::ZERO);

        breakdown.accumulate(super::EmulationBreakdownSample {
            core_ppu_duration: Duration::from_millis(2),
            core_cpu_duration: Duration::from_millis(1),
            core_ppu_bg_fetch_duration: Duration::from_millis(1),
            host_event_poll_duration: Duration::from_millis(3),
            ..Default::default()
        });
        assert_eq!(breakdown.core_ppu_duration, Duration::from_millis(10));
        assert_eq!(breakdown.core_cpu_duration, Duration::from_millis(8));
        assert_eq!(
            breakdown.core_ppu_bg_fetch_duration,
            Duration::from_millis(2)
        );
        assert_eq!(
            breakdown.host_event_poll_duration,
            Duration::from_millis(12)
        );
        assert_eq!(breakdown.core_duration(), Duration::from_millis(42));
        assert_eq!(breakdown.host_duration(), Duration::from_millis(33));
        assert_eq!(breakdown.core_other_duration(), Duration::from_millis(24));
        assert_eq!(breakdown.ppu_other_duration(), Duration::from_millis(1));
    }

    #[test]
    fn emulation_profile_request_and_replay_preserve_host_and_core_timing() {
        let machine = Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        let mut request = super::EmulationProfileRequest::new(
            super::DesktopEmulationSession::new_single(machine),
        );
        request.record_host_event_poll_duration(Duration::from_millis(2));
        request.record_host_audio_submit_duration(Duration::from_millis(3));
        request.record_host_save_flush_duration(Duration::from_millis(4));

        let work_item = request.into_work_item(Duration::from_millis(9));
        assert_eq!(work_item.emulation_duration, Duration::from_millis(9));
        assert_eq!(
            work_item.breakdown.host_duration(),
            Duration::from_millis(9)
        );

        let completed = super::profile_emulation_work_item(work_item);
        assert_eq!(completed.emulation_duration, Duration::from_millis(9));
        assert!(completed.breakdown.core_duration() > Duration::ZERO);
        assert_eq!(
            completed.breakdown.host_duration(),
            Duration::from_millis(9)
        );
    }

    #[test]
    fn linked_emulation_profile_request_replays_core_regions() {
        let primary = Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        let secondary = Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        let linked =
            super::DesktopEmulationSession::new_linked_dmg04_two_player(primary, secondary)
                .expect("matching machines should create a linked desktop session");
        let request = super::EmulationProfileRequest::new(linked);

        let completed =
            super::profile_emulation_work_item(request.into_work_item(Duration::from_millis(11)));

        assert_eq!(completed.emulation_duration, Duration::from_millis(11));
        assert!(completed.breakdown.core_duration() > Duration::ZERO);
        assert!(completed.breakdown.core_ppu_duration > Duration::ZERO);
    }

    #[test]
    fn async_emulation_profile_worker_and_counter_collect_samples() {
        let machine = Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        let worker = super::AsyncEmulationProfileWorker::new();
        let mut completed = Vec::new();
        worker.collect_completed(&mut |sample| completed.push(sample));
        assert!(completed.is_empty());
        assert!(
            worker.try_submit(
                super::EmulationProfileRequest::new(super::DesktopEmulationSession::new_single(
                    machine.clone(),
                ))
                .into_work_item(Duration::from_millis(7))
            )
        );
        for _ in 0..200 {
            worker.collect_completed(&mut |sample| completed.push(sample));
            if !completed.is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].emulation_duration, Duration::from_millis(7));
        assert!(completed[0].breakdown.core_duration() > Duration::ZERO);

        let mut disabled = super::PerformanceCounter::new_with_emulation_profile_mode(
            "gb-desktop | disabled".to_string(),
            super::EmulationProfileMode::Disabled,
        );
        assert!(!disabled.emulation_profile_enabled());
        assert!(!disabled.should_profile_next_frame());
        disabled.collect_emulation_profile_results();
        disabled.submit_emulation_profile_request(
            Some(super::EmulationProfileRequest::new(
                super::DesktopEmulationSession::new_single(machine.clone()),
            )),
            Duration::from_millis(5),
        );
        assert!(!disabled.emulation_profile_request_in_flight);

        let mut counter = super::PerformanceCounter::new_with_emulation_profile_mode(
            "gb-desktop | sampled".to_string(),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: 2,
            },
        );
        assert!(counter.emulation_profile_enabled());
        assert!(!counter.should_profile_next_frame());
        counter.presented_frames_total = 1;
        assert!(counter.should_profile_next_frame());
        counter.emulation_profile_request_in_flight = true;
        assert!(!counter.should_profile_next_frame());
        counter.emulation_profile_request_in_flight = false;
        counter.submit_emulation_profile_request(
            Some(super::EmulationProfileRequest::new(
                super::DesktopEmulationSession::new_single(machine),
            )),
            Duration::from_millis(6),
        );
        assert!(counter.emulation_profile_request_in_flight);
        wait_for_profiled_counter_sample(&mut counter);
        assert!(!counter.emulation_profile_request_in_flight);
        assert_eq!(counter.sample_profiled_frames, 1);
        assert_eq!(
            counter.sample_profiled_emulation_duration,
            Duration::from_millis(6)
        );
        assert!(counter.sample_profiled_emulation_breakdown.core_duration() > Duration::ZERO);
    }

    #[test]
    fn performance_counter_record_presented_frame_reports_and_resets_sampled_state() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("profile-summary", true, false, false);
        let mut counter = super::PerformanceCounter::new_with_emulation_profile_mode(
            "gb-desktop | profile-summary".to_string(),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: 4,
            },
        );
        counter.sample_started_at = Instant::now() - Duration::from_secs(2);
        counter.sample_profiled_frames = 1;
        counter.sample_profiled_emulation_duration = Duration::from_millis(12);
        counter.sample_profiled_emulation_breakdown = super::EmulationBreakdownSample {
            core_cpu_duration: Duration::from_millis(2),
            core_ppu_duration: Duration::from_millis(6),
            host_event_poll_duration: Duration::from_millis(1),
            host_audio_submit_duration: Duration::from_millis(1),
            ..Default::default()
        };
        counter
            .record_presented_frame(
                harness.canvas.window_mut(),
                super::FramePerformanceSample {
                    session_kind: super::EmulationProfileSessionKind::Single,
                    emulation_duration: Duration::from_millis(12),
                    emulation_profile_request: None,
                    render_duration: Duration::from_millis(2),
                    present_duration: Duration::from_millis(1),
                    pacing_duration: Duration::from_millis(4),
                    pacing_sleep_target_duration: Duration::from_millis(4),
                    pacing_audio_correction_duration: Duration::from_millis(1),
                    pacing_late_duration: Duration::from_millis(2),
                    pacing_oversleep_duration: Duration::from_millis(1),
                    audio_submit_sample_count: Some(804),
                    audio_submit_t_cycles: Some(70_224),
                    audio_submit_queue_before_ms: Some(24.0),
                    audio_submit_enqueued_ms: Some(4.0),
                    audio_submit_queue_after_ms: Some(28.0),
                    audio_queue_before_pacing_ms: Some(20.0),
                    audio_queue_after_pacing_ms: Some(18.0),
                    frame_step_t_cycles: Some(70_224),
                    frame_start_ly: Some(0),
                    frame_start_dot: Some(0),
                    frame_end_ly: Some(0),
                    frame_end_dot: Some(0),
                    frame_origin_crossings: Some(1),
                    scanline_transitions: Some(154),
                    scanlines_over_456: Some(0),
                    max_scanline_t_cycles: Some(456),
                    max_scanline_ly: Some(153),
                    max_mode0_start_dot: Some(252),
                    max_mode0_start_dot_ly: Some(5),
                    ly_153_to_0_transitions: Some(1),
                    ly_153_to_0_startup_mode0: Some(0),
                    ly_153_to_0_blank_frame: Some(0),
                    ly_0_self_wraps: Some(0),
                    ly_0_self_wrap_startup_mode0: Some(0),
                    ly_0_self_wrap_blank_frame: Some(0),
                    ly_0_to_1_transitions: Some(1),
                    ly_0_scanline_t_cycles: Some(456),
                    ly_0_max_mode0_start_dot: Some(254),
                    ly_0_stall_t_cycles: Some(0),
                    ly_0_stall_hblank_t_cycles: Some(0),
                    ly_0_stall_oam_t_cycles: Some(0),
                    ly_0_stall_drawing_t_cycles: Some(0),
                    ly_0_stall_startup_mode0_t_cycles: Some(0),
                    ly_0_stall_blank_frame_t_cycles: Some(0),
                    ly_0_stall_runs: Some(0),
                    ly_0_max_stall_run_t_cycles: Some(0),
                    ly_0_max_stall_dot: Some(0),
                    ly_0_max_stall_mode_dot: Some(0),
                    cpu_stop_t_cycles: Some(0),
                    cpu_zombie_stop_t_cycles: Some(0),
                    ly_0_cpu_stop_t_cycles: Some(0),
                    ly_0_cpu_zombie_stop_t_cycles: Some(0),
                    ly_0_stall_cpu_stop_t_cycles: Some(0),
                    ly_0_stall_cpu_zombie_stop_t_cycles: Some(0),
                    lcd_disabled_t_cycles: Some(0),
                    lcd_disable_transitions: Some(0),
                    lcd_enable_transitions: Some(0),
                    ly_0_lcd_disabled_t_cycles: Some(0),
                    ly_0_stall_lcd_disabled_t_cycles: Some(0),
                },
            )
            .expect("recording a sampled frame should succeed");
        assert_eq!(counter.frames_in_sample, 0);
        assert_eq!(counter.sample_profiled_frames, 0);
        assert_eq!(counter.sample_emulation_duration, Duration::ZERO);
        assert_eq!(counter.sample_present_duration, Duration::ZERO);
        assert_eq!(counter.sample_pacing_sleep_target_duration, Duration::ZERO);
        assert_eq!(
            counter.sample_pacing_audio_correction_duration,
            Duration::ZERO
        );
        assert_eq!(counter.sample_pacing_late_duration, Duration::ZERO);
        assert_eq!(counter.sample_pacing_oversleep_duration, Duration::ZERO);
        assert_eq!(counter.sample_audio_submit_sample_count, 0);
        assert_eq!(counter.sample_audio_submit_sample_count_observations, 0);
        assert_eq!(counter.sample_audio_submit_t_cycles, 0);
        assert_eq!(counter.sample_audio_submit_t_cycles_observations, 0);
        assert_eq!(counter.sample_audio_submit_queue_before_ms, 0.0);
        assert_eq!(counter.sample_audio_submit_queue_before_observations, 0);
        assert_eq!(counter.sample_audio_submit_enqueued_ms, 0.0);
        assert_eq!(counter.sample_audio_submit_enqueued_observations, 0);
        assert_eq!(counter.sample_audio_submit_queue_after_ms, 0.0);
        assert_eq!(counter.sample_audio_submit_queue_after_observations, 0);
        assert_eq!(counter.sample_audio_queue_before_pacing_ms, 0.0);
        assert_eq!(counter.sample_audio_queue_before_pacing_observations, 0);
        assert_eq!(counter.sample_audio_queue_after_pacing_ms, 0.0);
        assert_eq!(counter.sample_audio_queue_after_pacing_observations, 0);
        assert_eq!(counter.sample_frame_step_t_cycles, 0);
        assert_eq!(counter.sample_frame_step_t_cycles_observations, 0);
        assert_eq!(counter.sample_frame_start_ly, 0);
        assert_eq!(counter.sample_frame_start_ly_observations, 0);
        assert_eq!(counter.sample_frame_start_dot, 0);
        assert_eq!(counter.sample_frame_start_dot_observations, 0);
        assert_eq!(counter.sample_frame_end_ly, 0);
        assert_eq!(counter.sample_frame_end_ly_observations, 0);
        assert_eq!(counter.sample_frame_end_dot, 0);
        assert_eq!(counter.sample_frame_end_dot_observations, 0);
        assert_eq!(counter.sample_frame_origin_crossings, 0);
        assert_eq!(counter.sample_frame_origin_crossings_observations, 0);
        assert_eq!(counter.sample_scanline_transitions, 0);
        assert_eq!(counter.sample_scanline_transitions_observations, 0);
        assert_eq!(counter.sample_scanlines_over_456, 0);
        assert_eq!(counter.sample_scanlines_over_456_observations, 0);
        assert_eq!(counter.sample_max_scanline_t_cycles, 0);
        assert_eq!(counter.sample_max_scanline_t_cycles_observations, 0);
        assert_eq!(counter.sample_max_scanline_ly, 0);
        assert_eq!(counter.sample_max_scanline_ly_observations, 0);
        assert_eq!(counter.sample_max_mode0_start_dot, 0);
        assert_eq!(counter.sample_max_mode0_start_dot_observations, 0);
        assert_eq!(counter.sample_max_mode0_start_dot_ly, 0);
        assert_eq!(counter.sample_max_mode0_start_dot_ly_observations, 0);
        assert!(counter.hud_snapshot().is_some());

        counter.frames_in_sample = 1;
        assert!(
            counter
                .emulation_profile_summary(
                    Duration::from_millis(20),
                    super::PerformanceHudSnapshot {
                        fps: 60.0,
                        speed_percent: 100.0,
                        frame_time_ms: 16.7,
                        emulation_time_ms: 9.0,
                        render_time_ms: 1.0,
                        pacing_time_ms: 2.0,
                        audio_queue_ms: None,
                    },
                )
                .is_none()
        );
    }

    #[test]
    fn step_until_next_frame_returns_quit_when_process_events_requests_exit() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("step-quit", true, false, false);
        harness
            .sdl
            .event()
            .expect("quit-path event subsystem")
            .push_event(Event::Quit { timestamp: 0 })
            .expect("quit event should be pushable");
        let FrontendHarness {
            event_pump,
            canvas,
            session,
            machine,
            runtime,
            settings_store,
            performance_counter,
            frame_pacer,
            ..
        } = &mut harness;
        let mut context = super::FrontendActionContext {
            session,
            machine,
            runtime,
            performance_counter,
            frame_pacer,
            settings_store,
        };
        let result = super::step_until_next_frame(event_pump, canvas, &mut context)
            .expect("quit-path stepping should succeed");
        assert_eq!(result.signal, super::LoopSignal::Quit);
        assert!(result.emulation_profile_request.is_none());
        assert_eq!(
            result.frame_loop_telemetry,
            super::FrameLoopTelemetry::default()
        );
    }

    #[test]
    fn step_until_next_frame_returns_continue_without_profile_when_paused() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("step-paused", true, false, false);
        harness.runtime.paused = true;
        let FrontendHarness {
            event_pump,
            canvas,
            session,
            machine,
            runtime,
            settings_store,
            performance_counter,
            frame_pacer,
            ..
        } = &mut harness;
        let mut context = super::FrontendActionContext {
            session,
            machine,
            runtime,
            performance_counter,
            frame_pacer,
            settings_store,
        };
        let result = super::step_until_next_frame(event_pump, canvas, &mut context)
            .expect("paused stepping should succeed");
        assert_eq!(result.signal, super::LoopSignal::Continue);
        assert!(result.emulation_profile_request.is_none());
        assert_eq!(
            result.frame_loop_telemetry,
            super::FrameLoopTelemetry::default()
        );
    }

    #[test]
    fn step_until_next_frame_skips_detailed_frame_telemetry_when_emulation_profiling_is_disabled() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("step-no-telemetry", true, true, false);
        harness.performance_counter = super::PerformanceCounter::new_with_emulation_profile_mode(
            "gb-desktop | step-no-telemetry".to_string(),
            super::EmulationProfileMode::Disabled,
        );
        let FrontendHarness {
            event_pump,
            canvas,
            session,
            machine,
            runtime,
            settings_store,
            performance_counter,
            frame_pacer,
            ..
        } = &mut harness;
        let mut context = super::FrontendActionContext {
            session,
            machine,
            runtime,
            performance_counter,
            frame_pacer,
            settings_store,
        };
        let result = super::step_until_next_frame(event_pump, canvas, &mut context)
            .expect("stepping should still succeed without emulation profiling");
        assert_eq!(result.signal, super::LoopSignal::Continue);
        assert!(result.emulation_profile_request.is_none());
        assert_eq!(
            result.frame_loop_telemetry,
            super::FrameLoopTelemetry::default()
        );
    }

    #[test]
    fn step_until_next_frame_returns_profile_requests_for_sampled_frames() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("step-profile", true, true, false);
        harness.performance_counter = super::PerformanceCounter::new_with_emulation_profile_mode(
            "gb-desktop | step-profile".to_string(),
            super::EmulationProfileMode::SampledSummary {
                sample_every_frames: 2,
            },
        );
        harness.performance_counter.presented_frames_total = 1;
        let FrontendHarness {
            event_pump,
            canvas,
            session,
            machine,
            runtime,
            settings_store,
            performance_counter,
            frame_pacer,
            ..
        } = &mut harness;
        let mut context = super::FrontendActionContext {
            session,
            machine,
            runtime,
            performance_counter,
            frame_pacer,
            settings_store,
        };
        let result = super::step_until_next_frame(event_pump, canvas, &mut context)
            .expect("sampled stepping should succeed");
        assert_eq!(result.signal, super::LoopSignal::Continue);
        let request = result
            .emulation_profile_request
            .expect("sampled frames should snapshot a profile request");
        assert_eq!(result.frame_loop_telemetry.start_ly, 0);
        assert_eq!(result.frame_loop_telemetry.start_dot, 0);
        assert_eq!(result.frame_loop_telemetry.end_ly, 0);
        assert_eq!(result.frame_loop_telemetry.end_dot, 0);
        assert!(result.frame_loop_telemetry.stepped_t_cycles > 0);
        assert_eq!(result.frame_loop_telemetry.frame_origin_crossings, 1);
        assert!(request.breakdown.host_event_poll_duration <= Duration::from_millis(50));
        assert!(request.breakdown.host_audio_submit_duration <= Duration::from_millis(50));
    }

    #[test]
    fn trace_capture_t_cycles_parser_uses_default_and_rejects_zero() {
        assert_eq!(parse_trace_capture_t_cycles(None), Ok(8_192));
        assert_eq!(
            parse_trace_capture_t_cycles(Some(OsStr::new("4096"))),
            Ok(4_096)
        );
        assert!(
            parse_trace_capture_t_cycles(Some(OsStr::new("0")))
                .expect_err("zero trace window should be rejected")
                .contains("must be greater than zero")
        );
    }

    #[test]
    fn desktop_trace_renderer_includes_apu_last_write_when_present() {
        let machine = Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF1A, 0x80);
        apu.write_register(0xFF1E, 0x80);
        apu.write_register(0xFF1A, 0x00);

        let rendered = render_desktop_trace_record(&super::DesktopTraceRecord {
            t_cycle: 123,
            cpu: machine.cpu().snapshot(),
            apu: apu.snapshot(),
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
        });

        assert!(rendered.contains("apu.last_write=write@0xFF1A=0x00"));
        assert!(rendered.contains("before("));
        assert!(rendered.contains("after("));
    }

    #[test]
    fn desktop_trace_capture_from_env_keeps_a_ring_buffer_and_writes_the_artifact() {
        let _guard = crate::lock_sdl_test();
        let root = temp_test_root("trace-capture");
        let output_path = root.join("artifacts").join("desktop-trace.txt");
        unsafe {
            std::env::set_var(super::DESKTOP_TRACE_PATH_ENV_VAR, &output_path);
            std::env::set_var(super::DESKTOP_TRACE_T_CYCLES_ENV_VAR, "2");
        }
        let mut capture = super::DesktopTraceCapture::from_env().expect("trace capture from env");
        unsafe {
            std::env::remove_var(super::DESKTOP_TRACE_PATH_ENV_VAR);
            std::env::remove_var(super::DESKTOP_TRACE_T_CYCLES_ENV_VAR);
        }

        assert_eq!(capture.output_path.as_deref(), Some(output_path.as_path()));
        assert!(capture.is_enabled());
        assert_eq!(capture.max_t_cycles, 2);

        let mut machine = Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        for _ in 0..3 {
            machine.step_t_cycle();
            capture.record_t_cycle(&machine);
        }

        assert_eq!(capture.records.len(), 2);
        capture
            .write_artifact()
            .expect("trace artifact should be writable");
        let rendered = fs::read_to_string(&output_path).expect("trace artifact should exist");
        assert_eq!(rendered.lines().count(), 2);
        assert!(rendered.contains("cpu.pc=0x0100"));
        assert!(rendered.contains("apu.nr50=0x77"));

        super::DesktopTraceCapture {
            enabled: false,
            output_path: None,
            max_t_cycles: 2,
            records: std::collections::VecDeque::new(),
        }
        .write_artifact()
        .expect("disabled trace capture should be a no-op");
    }

    #[test]
    fn desktop_trace_helpers_cover_bus_address_joypad_and_apu_formatting() {
        assert_eq!(super::format_cpu_bus_activity(None), "none");
        assert_eq!(
            super::format_cpu_bus_activity(Some(CpuBusActivitySnapshot {
                kind: CpuBusAccessKind::OpcodeFetch,
                address: 0x0100,
                value: 0x31,
            })),
            "opcode_fetch@0x0100=0x31"
        );
        assert_eq!(
            super::format_cpu_bus_activity(Some(CpuBusActivitySnapshot {
                kind: CpuBusAccessKind::OperandRead,
                address: 0x0101,
                value: 0xFE,
            })),
            "operand_read@0x0101=0xFE"
        );
        assert_eq!(
            super::format_cpu_bus_activity(Some(CpuBusActivitySnapshot {
                kind: CpuBusAccessKind::DataRead,
                address: 0xC123,
                value: 0x45,
            })),
            "data_read@0xC123=0x45"
        );
        assert_eq!(
            super::format_cpu_bus_activity(Some(CpuBusActivitySnapshot {
                kind: CpuBusAccessKind::DataWrite,
                address: 0xFF40,
                value: 0x91,
            })),
            "data_write@0xFF40=0x91"
        );

        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::Read,
                access_address: Some(0xC000),
                idu_address: None,
                update_direction: None,
            })),
            "read@0xC000"
        );
        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::Read,
                access_address: None,
                idu_address: None,
                update_direction: None,
            })),
            "read@missing"
        );
        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::Write,
                access_address: Some(0xC001),
                idu_address: None,
                update_direction: None,
            })),
            "write@0xC001"
        );
        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::Write,
                access_address: None,
                idu_address: None,
                update_direction: None,
            })),
            "write@missing"
        );
        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::IncDec,
                access_address: None,
                idu_address: Some(0xC002),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            })),
            "inc@0xC002"
        );
        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::IncDec,
                access_address: None,
                idu_address: None,
                update_direction: None,
            })),
            "incdec@missing"
        );
        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::ReadWithIncDec,
                access_address: Some(0xC003),
                idu_address: Some(0xC004),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
            })),
            "read+dec@0xC003->0xC004"
        );
        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::ReadWithIncDec,
                access_address: None,
                idu_address: None,
                update_direction: None,
            })),
            "combined@missing"
        );
        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::WriteWithIncDec,
                access_address: Some(0xC005),
                idu_address: Some(0xC006),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            })),
            "write+inc@0xC005->0xC006"
        );
        assert_eq!(
            super::format_cpu_address_event(Some(CpuAddressEvent {
                kind: CpuAddressEventKind::WriteWithIncDec,
                access_address: None,
                idu_address: None,
                update_direction: None,
            })),
            "combined@missing"
        );
        assert_eq!(
            super::format_update_direction(CpuAddressUpdateDirection::Increment),
            "inc"
        );
        assert_eq!(
            super::format_update_direction(CpuAddressUpdateDirection::Decrement),
            "dec"
        );
        assert_eq!(super::visible_nr52(true, 0x0B), 0xFB);
        assert_eq!(super::visible_nr52(false, 0x0B), 0x70);
        assert_eq!(
            super::visible_joypad_low_nibble(&JoypadSnapshot {
                console_model: ConsoleModel::Dmg,
                status: JoypadStatus::Ready,
                selection_bits: 0x00,
                pressed_mask: 0xFF,
            }),
            0x00
        );
        assert_eq!(
            super::visible_joypad_low_nibble(&JoypadSnapshot {
                console_model: ConsoleModel::Dmg,
                status: JoypadStatus::Ready,
                selection_bits: 0x30,
                pressed_mask: 0xFF,
            }),
            0x0F
        );

        let base_state = ApuRegisterWriteState {
            powered: true,
            nr50: 0x77,
            nr51: 0xFF,
            nr52: 0xFB,
            channel_active_mask: 0x0B,
            channel_dac_mask: 0x0F,
            output: ApuOutputSnapshot {
                channel_digital_outputs: [0x01, 0x02, 0x03, 0x04],
                channel_dac_outputs: [0; 4],
                vin_analog_output: ApuStereoOutputSnapshot::default(),
                mixer_output: ApuStereoOutputSnapshot { left: 5, right: 6 },
                master_output: ApuStereoOutputSnapshot::default(),
                hpf_output: ApuStereoOutputSnapshot { left: 7, right: 8 },
                hpf_capacitor: Default::default(),
            },
        };
        assert_eq!(super::format_apu_last_register_write(None), "");
        let rendered = super::format_apu_last_register_write(Some(&ApuRegisterWriteObservation {
            address: 0xFF1A,
            value: 0x00,
            before: base_state,
            after: ApuRegisterWriteState {
                nr52: 0xF7,
                channel_active_mask: 0x07,
                ..base_state
            },
        }));
        assert!(rendered.contains("apu.last_write=write@0xFF1A=0x00"));
        assert!(rendered.contains("before("));
        assert!(rendered.contains("after("));
    }

    #[test]
    fn frame_pacer_and_performance_counter_cover_idle_paths() {
        let mut frame_pacer = super::FramePacer::new(true);
        frame_pacer.next_frame_start = Instant::now() - Duration::from_secs(1);
        let pacing = frame_pacer.wait_until_next_frame(None);
        assert_eq!(pacing.pacing_duration, Duration::ZERO);
        assert_eq!(pacing.sleep_target_duration, Duration::ZERO);
        assert!(pacing.late_duration > Duration::ZERO);
        assert_eq!(pacing.audio_correction_duration, Duration::ZERO);
        assert_eq!(pacing.oversleep_duration, Duration::ZERO);
        frame_pacer.set_vsync_enabled(true);
        assert!(frame_pacer.next_frame_start <= Instant::now());

        let counter = super::PerformanceCounter::new_with_emulation_profile_mode(
            "gb-desktop | no rom".to_string(),
            super::EmulationProfileMode::Disabled,
        );
        let snapshot = counter.snapshot_from_elapsed(Duration::ZERO);
        assert!(snapshot.fps.is_finite());
        assert_eq!(snapshot.audio_queue_ms, None);
    }

    #[test]
    fn host_rtc_sync_advances_live_mbc3_sessions_from_wall_clock_elapsed_seconds() {
        let mut machine = Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_test_rom(32 * 1024, 0x0F, 0x00, 0x00))
            .expect("MBC3 RTC cartridge should load");

        let mut rtc_sync = HostRtcSync::new(1_000);
        rtc_sync.apply_with_now(&mut machine, 1_005);

        assert_eq!(
            machine.cartridge().persistent_state(),
            PersistentCartState::Mbc3Rtc {
                rtc: gb_core::Mbc3RtcPersistentState {
                    seconds: 5,
                    minutes: 0,
                    hours: 0,
                    day_counter: 0,
                    halt: false,
                    carry: false,
                },
            },
        );
    }

    #[test]
    fn host_rtc_sync_ignores_backward_host_clock_steps() {
        let mut machine = Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_test_rom(32 * 1024, 0x0F, 0x00, 0x00))
            .expect("MBC3 RTC cartridge should load");

        let mut rtc_sync = HostRtcSync::new(1_000);
        rtc_sync.apply_with_now(&mut machine, 1_010);
        rtc_sync.apply_with_now(&mut machine, 1_005);

        assert_eq!(
            machine.cartridge().persistent_state(),
            PersistentCartState::Mbc3Rtc {
                rtc: gb_core::Mbc3RtcPersistentState {
                    seconds: 10,
                    minutes: 0,
                    hours: 0,
                    day_counter: 0,
                    halt: false,
                    carry: false,
                },
            },
        );
    }

    #[test]
    fn open_rom_dialog_result_uses_the_first_selected_path() {
        assert_eq!(
            map_path_dialog_result(Ok(vec![
                PathBuf::from("/tmp/tetris.gb"),
                PathBuf::from("/tmp/other.gb"),
            ])),
            PathDialogResult::Selected(PathBuf::from("/tmp/tetris.gb"))
        );
    }

    #[test]
    fn open_rom_dialog_result_preserves_cancel_as_a_non_selection() {
        assert_eq!(
            map_path_dialog_result(Err(DialogError::Canceled)),
            PathDialogResult::Canceled
        );
    }

    #[test]
    fn open_rom_dialog_filters_include_supported_game_boy_extensions() {
        assert_eq!(ROM_FILE_DIALOG_FILTERS[0].name, "Game Boy ROMs");
        assert_eq!(ROM_FILE_DIALOG_FILTERS[0].pattern, "gb;gbc;bin");
    }

    #[test]
    fn boot_rom_file_dialog_filters_include_common_dump_extensions() {
        assert_eq!(BOOT_ROM_FILE_DIALOG_FILTERS[0].name, "Boot ROM dumps");
        assert_eq!(BOOT_ROM_FILE_DIALOG_FILTERS[0].pattern, "bin;rom");
    }

    #[test]
    fn system_option_cycle_helpers_wrap_in_the_expected_order() {
        assert_eq!(
            next_console_model(DesktopConsoleModel::Dmg0),
            DesktopConsoleModel::Dmg
        );
        assert_eq!(
            next_console_model(DesktopConsoleModel::Dmg),
            DesktopConsoleModel::Mgb
        );
        assert_eq!(
            next_console_model(DesktopConsoleModel::Mgb),
            DesktopConsoleModel::Dmg0
        );
        assert_eq!(
            next_startup_mode(StartupMode::SkipBoot),
            StartupMode::RealBoot
        );
        assert_eq!(
            next_startup_mode(StartupMode::RealBoot),
            StartupMode::SkipBoot
        );
        assert_eq!(
            next_execution_mode(ExecutionMode::Strict),
            ExecutionMode::Permissive
        );
        assert_eq!(
            next_execution_mode(ExecutionMode::Permissive),
            ExecutionMode::Experimental
        );
        assert_eq!(
            next_execution_mode(ExecutionMode::Experimental),
            ExecutionMode::Strict
        );
        assert_eq!(
            next_boot_rom_verification_mode(BootRomVerificationMode::Strict),
            BootRomVerificationMode::Warn
        );
        assert_eq!(
            next_boot_rom_verification_mode(BootRomVerificationMode::Warn),
            BootRomVerificationMode::Off
        );
        assert_eq!(
            next_boot_rom_verification_mode(BootRomVerificationMode::Off),
            BootRomVerificationMode::Strict
        );
        assert_eq!(
            next_save_flush_policy(DesktopSaveFlushPolicy::Manual),
            DesktopSaveFlushPolicy::OnClose
        );
        assert_eq!(
            next_save_flush_policy(DesktopSaveFlushPolicy::OnClose),
            DesktopSaveFlushPolicy::OnWrite
        );
        assert_eq!(
            next_save_flush_policy(DesktopSaveFlushPolicy::OnWrite),
            DesktopSaveFlushPolicy::Debounced
        );
        assert_eq!(
            next_save_flush_policy(DesktopSaveFlushPolicy::Debounced),
            DesktopSaveFlushPolicy::Manual
        );
        assert_eq!(
            next_gamepad_rumble_mode(GamepadRumbleMode::Off),
            GamepadRumbleMode::Strong
        );
        assert_eq!(
            next_gamepad_rumble_mode(GamepadRumbleMode::Strong),
            GamepadRumbleMode::Weak
        );
        assert_eq!(
            next_gamepad_rumble_mode(GamepadRumbleMode::Weak),
            GamepadRumbleMode::Off
        );
    }

    #[test]
    fn recent_rom_labels_compact_the_stem_for_the_overlay_width() {
        assert_eq!(
            compact_recent_rom_label(Path::new(
                "/tmp/roms/Super Mario Land 2 - 6 Golden Coins (USA, Europe) (Rev 2).gb"
            ))
            .as_str(),
            "SUPER MARIO LAND 2 6 GOLDEN COINS"
        );
    }

    #[test]
    fn menu_keyboard_input_uses_dedicated_menu_bindings() {
        let config = DesktopConfig::default();

        assert_eq!(
            menu_input_for_key(config.input.keyboard.menu, Keycode::Up),
            Some(super::MenuInput::Up)
        );
        assert_eq!(
            menu_input_for_key(config.input.keyboard.menu, Keycode::Return),
            Some(super::MenuInput::Confirm)
        );
        assert_eq!(
            menu_input_for_key(config.input.keyboard.menu, Keycode::Escape),
            Some(super::MenuInput::Cancel)
        );
        assert_eq!(
            menu_input_for_key(config.input.keyboard.menu, Keycode::X),
            None
        );
        assert_eq!(
            menu_input_for_key(config.input.keyboard.menu, Keycode::Backspace),
            None
        );
    }

    #[test]
    fn menu_keyboard_input_tracks_remapped_menu_bindings() {
        let bindings = MenuKeyboardBindings {
            confirm: DesktopKey::Space,
            cancel: DesktopKey::Return,
            ..MenuKeyboardBindings::default()
        };

        assert_eq!(
            menu_input_for_key(bindings, Keycode::Space),
            Some(super::MenuInput::Confirm)
        );
        assert_eq!(
            menu_input_for_key(bindings, Keycode::Return),
            Some(super::MenuInput::Cancel)
        );
        assert_eq!(
            menu_input_for_key(bindings, Keycode::Escape),
            Some(super::MenuInput::Cancel)
        );
    }

    #[test]
    fn menu_gamepad_input_tracks_remapped_menu_bindings() {
        let bindings = GamepadMenuBindings {
            confirm: GamepadButtonBinding::North,
            cancel: GamepadButtonBinding::West,
            ..GamepadMenuBindings::default()
        };

        assert_eq!(
            menu_input_for_gamepad_button(bindings, Button::North),
            Some(super::MenuInput::Confirm)
        );
        assert_eq!(
            menu_input_for_gamepad_button(bindings, Button::West),
            Some(super::MenuInput::Cancel)
        );
        assert_eq!(menu_input_for_gamepad_button(bindings, Button::East), None);
    }

    #[test]
    fn keyboard_binding_assignment_swaps_existing_keys_instead_of_creating_duplicates() {
        let mut bindings = DesktopConfig::default().input.keyboard;
        assign_keyboard_binding(&mut bindings, KeyboardBindingTarget::A, DesktopKey::Z);

        assert_eq!(bindings.joypad.a, DesktopKey::Z);
        assert_eq!(bindings.joypad.b, DesktopKey::X);
        assert_eq!(
            joypad_binding_target_for_key(bindings.joypad, DesktopKey::Z),
            Some(KeyboardBindingTarget::A)
        );
        assert_eq!(
            joypad_binding_target_for_key(bindings.joypad, DesktopKey::X),
            Some(KeyboardBindingTarget::B)
        );
    }

    #[test]
    fn hotkey_binding_assignment_swaps_existing_keys_without_touching_joypad_bindings() {
        let mut bindings = DesktopConfig::default().input.keyboard;
        let original_a = bindings.joypad.a;

        assign_keyboard_binding(&mut bindings, KeyboardBindingTarget::Pause, DesktopKey::R);

        assert_eq!(bindings.hotkeys.pause, DesktopKey::R);
        assert_eq!(bindings.hotkeys.reset, DesktopKey::Space);
        assert_eq!(bindings.joypad.a, original_a);
        assert_eq!(
            hotkey_binding_target_for_key(bindings.hotkeys, DesktopKey::R),
            Some(KeyboardBindingTarget::Pause)
        );
        assert_eq!(
            hotkey_binding_target_for_key(bindings.hotkeys, DesktopKey::Space),
            Some(KeyboardBindingTarget::Reset)
        );
    }

    #[test]
    fn gamepad_binding_assignment_swaps_existing_buttons_instead_of_creating_duplicates() {
        let mut bindings = DesktopConfig::default().input.gamepad.bindings;
        assign_gamepad_binding(
            &mut bindings,
            GamepadBindingTarget::A,
            GamepadButtonBinding::South,
        );

        assert_eq!(bindings.a, GamepadButtonBinding::South);
        assert_eq!(bindings.b, GamepadButtonBinding::East);
        assert_eq!(
            gamepad_binding_target_for_binding(bindings, GamepadButtonBinding::South),
            Some(GamepadBindingTarget::A)
        );
        assert_eq!(
            gamepad_binding_target_for_binding(bindings, GamepadButtonBinding::East),
            Some(GamepadBindingTarget::B)
        );
    }

    #[test]
    fn keyboard_menu_binding_assignment_swaps_existing_keys_instead_of_creating_duplicates() {
        let mut bindings = MenuKeyboardBindings::default();
        assign_keyboard_menu_binding(
            &mut bindings,
            KeyboardMenuBindingTarget::Confirm,
            DesktopKey::Escape,
        );

        assert_eq!(bindings.confirm, DesktopKey::Escape);
        assert_eq!(bindings.cancel, DesktopKey::Return);
        assert_eq!(
            keyboard_menu_binding_target_for_key(bindings, DesktopKey::Escape),
            Some(KeyboardMenuBindingTarget::Confirm)
        );
        assert_eq!(
            keyboard_menu_binding_target_for_key(bindings, DesktopKey::Return),
            Some(KeyboardMenuBindingTarget::Cancel)
        );
    }

    #[test]
    fn gamepad_menu_binding_assignment_swaps_existing_buttons_instead_of_creating_duplicates() {
        let mut bindings = GamepadMenuBindings::default();
        assign_gamepad_menu_binding(
            &mut bindings,
            GamepadMenuBindingTarget::Confirm,
            GamepadButtonBinding::East,
        );

        assert_eq!(bindings.confirm, GamepadButtonBinding::East);
        assert_eq!(bindings.cancel, GamepadButtonBinding::South);
        assert_eq!(
            gamepad_menu_binding_target_for_binding(bindings, GamepadButtonBinding::East),
            Some(GamepadMenuBindingTarget::Confirm)
        );
        assert_eq!(
            gamepad_menu_binding_target_for_binding(bindings, GamepadButtonBinding::South),
            Some(GamepadMenuBindingTarget::Cancel)
        );
    }

    #[test]
    fn joypad_key_capture_rejects_hotkey_only_function_keys() {
        assert_eq!(
            assignable_key_for_binding_target_from_keycode(Keycode::F5, KeyboardBindingTarget::A),
            None
        );
        assert_eq!(
            assignable_key_for_binding_target_from_keycode(
                Keycode::F11,
                KeyboardBindingTarget::Start
            ),
            None
        );
        assert_eq!(
            assignable_key_for_binding_target_from_keycode(
                Keycode::Space,
                KeyboardBindingTarget::B
            ),
            Some(DesktopKey::Space)
        );
    }

    #[test]
    fn hotkey_key_capture_accepts_function_keys() {
        assert_eq!(
            assignable_key_for_binding_target_from_keycode(
                Keycode::F5,
                KeyboardBindingTarget::SaveBattery
            ),
            Some(DesktopKey::F5)
        );
        assert_eq!(
            assignable_key_for_binding_target_from_keycode(
                Keycode::F11,
                KeyboardBindingTarget::ToggleFullscreen
            ),
            Some(DesktopKey::F11)
        );
    }

    #[test]
    fn menu_key_capture_restricts_escape_to_cancel_bindings() {
        assert_eq!(
            assignable_menu_key_for_binding_target_from_keycode(
                Keycode::Escape,
                KeyboardMenuBindingTarget::Confirm
            ),
            None
        );
        assert_eq!(
            assignable_menu_key_for_binding_target_from_keycode(
                Keycode::Escape,
                KeyboardMenuBindingTarget::Cancel
            ),
            Some(DesktopKey::Escape)
        );
        assert_eq!(
            assignable_menu_key_for_binding_target_from_keycode(
                Keycode::Space,
                KeyboardMenuBindingTarget::Confirm
            ),
            Some(DesktopKey::Space)
        );
    }

    #[test]
    fn gamepad_directional_source_cycles_through_the_three_supported_modes() {
        assert_eq!(
            next_gamepad_directional_source(GamepadDirectionalSource::DpadOnly),
            GamepadDirectionalSource::LeftStickOnly
        );
        assert_eq!(
            next_gamepad_directional_source(GamepadDirectionalSource::LeftStickOnly),
            GamepadDirectionalSource::DpadAndLeftStick
        );
        assert_eq!(
            next_gamepad_directional_source(GamepadDirectionalSource::DpadAndLeftStick),
            GamepadDirectionalSource::DpadOnly
        );
    }

    #[test]
    fn window_scale_cycles_through_the_supported_overlay_values() {
        assert_eq!(next_window_scale(0), 1);
        assert_eq!(next_window_scale(1), 2);
        assert_eq!(next_window_scale(7), 8);
        assert_eq!(next_window_scale(8), 1);
    }

    #[test]
    fn audio_volume_cycles_in_quarter_steps() {
        assert_eq!(next_audio_volume_percent(0), 25);
        assert_eq!(next_audio_volume_percent(25), 50);
        assert_eq!(next_audio_volume_percent(50), 75);
        assert_eq!(next_audio_volume_percent(75), 100);
        assert_eq!(next_audio_volume_percent(100), 25);
    }

    #[test]
    fn binding_value_helpers_cover_all_frontend_targets() {
        let mut keyboard = gb_desktop::KeyboardBindings::default();
        let keyboard_targets = [
            (KeyboardBindingTarget::Up, DesktopKey::Escape),
            (KeyboardBindingTarget::Down, DesktopKey::ArrowUp),
            (KeyboardBindingTarget::Left, DesktopKey::ArrowDown),
            (KeyboardBindingTarget::Right, DesktopKey::ArrowLeft),
            (KeyboardBindingTarget::A, DesktopKey::ArrowRight),
            (KeyboardBindingTarget::B, DesktopKey::Backspace),
            (KeyboardBindingTarget::Select, DesktopKey::Return),
            (KeyboardBindingTarget::Start, DesktopKey::Space),
            (KeyboardBindingTarget::Pause, DesktopKey::R),
            (KeyboardBindingTarget::Reset, DesktopKey::X),
            (KeyboardBindingTarget::ToggleFullscreen, DesktopKey::Z),
            (KeyboardBindingTarget::TogglePerformanceHud, DesktopKey::F5),
            (KeyboardBindingTarget::SaveBattery, DesktopKey::F10),
        ];
        for (target, key) in keyboard_targets {
            super::set_keyboard_binding_value(&mut keyboard, target, key);
            assert_eq!(super::keyboard_binding_value(keyboard, target), key);
        }
        let keyboard_before = keyboard;
        assign_keyboard_binding(
            &mut keyboard,
            KeyboardBindingTarget::SaveBattery,
            keyboard_before.hotkeys.save_battery,
        );
        assert_eq!(keyboard, keyboard_before);

        let mut keyboard_menu = MenuKeyboardBindings::default();
        let keyboard_menu_targets = [
            (KeyboardMenuBindingTarget::Up, DesktopKey::Backspace),
            (KeyboardMenuBindingTarget::Down, DesktopKey::Return),
            (KeyboardMenuBindingTarget::Confirm, DesktopKey::Space),
            (KeyboardMenuBindingTarget::Cancel, DesktopKey::Escape),
        ];
        for (target, key) in keyboard_menu_targets {
            super::set_keyboard_menu_binding_value(&mut keyboard_menu, target, key);
            assert_eq!(
                super::keyboard_menu_binding_value(keyboard_menu, target),
                key
            );
        }
        let keyboard_menu_before = keyboard_menu;
        assign_keyboard_menu_binding(
            &mut keyboard_menu,
            KeyboardMenuBindingTarget::Cancel,
            keyboard_menu_before.cancel,
        );
        assert_eq!(keyboard_menu, keyboard_menu_before);

        let mut gamepad = gb_desktop::GamepadButtonBindings::default();
        let gamepad_targets = [
            (GamepadBindingTarget::Up, GamepadButtonBinding::South),
            (GamepadBindingTarget::Down, GamepadButtonBinding::East),
            (GamepadBindingTarget::Left, GamepadButtonBinding::West),
            (GamepadBindingTarget::Right, GamepadButtonBinding::North),
            (GamepadBindingTarget::A, GamepadButtonBinding::Back),
            (GamepadBindingTarget::B, GamepadButtonBinding::Start),
            (GamepadBindingTarget::Select, GamepadButtonBinding::Guide),
            (
                GamepadBindingTarget::Start,
                GamepadButtonBinding::LeftShoulder,
            ),
        ];
        for (target, binding) in gamepad_targets {
            super::set_gamepad_binding_value(&mut gamepad, target, binding);
            assert_eq!(super::gamepad_binding_value(gamepad, target), binding);
            assert_eq!(
                gamepad_binding_target_for_binding(gamepad, binding),
                Some(target)
            );
        }
        let gamepad_before = gamepad;
        assign_gamepad_binding(
            &mut gamepad,
            GamepadBindingTarget::Start,
            gamepad_before.start,
        );
        assert_eq!(gamepad, gamepad_before);

        let mut gamepad_menu = GamepadMenuBindings::default();
        let gamepad_menu_targets = [
            (GamepadMenuBindingTarget::Up, GamepadButtonBinding::DPadUp),
            (
                GamepadMenuBindingTarget::Down,
                GamepadButtonBinding::DPadDown,
            ),
            (
                GamepadMenuBindingTarget::Confirm,
                GamepadButtonBinding::DPadLeft,
            ),
            (
                GamepadMenuBindingTarget::Cancel,
                GamepadButtonBinding::DPadRight,
            ),
        ];
        for (target, binding) in gamepad_menu_targets {
            super::set_gamepad_menu_binding_value(&mut gamepad_menu, target, binding);
            assert_eq!(
                super::gamepad_menu_binding_value(gamepad_menu, target),
                binding
            );
            assert_eq!(
                gamepad_menu_binding_target_for_binding(gamepad_menu, binding),
                Some(target)
            );
        }
        let gamepad_menu_before = gamepad_menu;
        assign_gamepad_menu_binding(
            &mut gamepad_menu,
            GamepadMenuBindingTarget::Cancel,
            gamepad_menu_before.cancel,
        );
        assert_eq!(gamepad_menu, gamepad_menu_before);
    }

    #[test]
    fn key_and_button_mapping_helpers_cover_all_variants_and_fallbacks() {
        let key_pairs = [
            (
                DesktopKey::Escape,
                Keycode::Escape,
                sdl3::keyboard::Scancode::Escape,
            ),
            (
                DesktopKey::ArrowUp,
                Keycode::Up,
                sdl3::keyboard::Scancode::Up,
            ),
            (
                DesktopKey::ArrowDown,
                Keycode::Down,
                sdl3::keyboard::Scancode::Down,
            ),
            (
                DesktopKey::ArrowLeft,
                Keycode::Left,
                sdl3::keyboard::Scancode::Left,
            ),
            (
                DesktopKey::ArrowRight,
                Keycode::Right,
                sdl3::keyboard::Scancode::Right,
            ),
            (
                DesktopKey::Backspace,
                Keycode::Backspace,
                sdl3::keyboard::Scancode::Backspace,
            ),
            (
                DesktopKey::Return,
                Keycode::Return,
                sdl3::keyboard::Scancode::Return,
            ),
            (
                DesktopKey::Space,
                Keycode::Space,
                sdl3::keyboard::Scancode::Space,
            ),
            (DesktopKey::R, Keycode::R, sdl3::keyboard::Scancode::R),
            (DesktopKey::X, Keycode::X, sdl3::keyboard::Scancode::X),
            (DesktopKey::Z, Keycode::Z, sdl3::keyboard::Scancode::Z),
            (DesktopKey::F5, Keycode::F5, sdl3::keyboard::Scancode::F5),
            (DesktopKey::F10, Keycode::F10, sdl3::keyboard::Scancode::F10),
            (DesktopKey::F11, Keycode::F11, sdl3::keyboard::Scancode::F11),
        ];
        for (desktop_key, keycode, scancode) in key_pairs {
            assert_eq!(desktop_key_scancode(desktop_key), scancode);
            assert_eq!(desktop_key_from_keycode(keycode), Some(desktop_key));
            assert!(super::key_matches(desktop_key, keycode));
        }
        assert_eq!(desktop_key_from_keycode(Keycode::A), None);

        let joypad = gb_desktop::JoypadKeyboardBindings::default();
        assert_eq!(
            super::joypad_button_for_key(joypad, Keycode::Up),
            Some(gb_core::JoypadButton::Up)
        );
        assert_eq!(
            super::joypad_button_for_key(joypad, Keycode::Down),
            Some(gb_core::JoypadButton::Down)
        );
        assert_eq!(
            super::joypad_button_for_key(joypad, Keycode::Left),
            Some(gb_core::JoypadButton::Left)
        );
        assert_eq!(
            super::joypad_button_for_key(joypad, Keycode::Right),
            Some(gb_core::JoypadButton::Right)
        );
        assert_eq!(
            super::joypad_button_for_key(joypad, Keycode::Z),
            Some(gb_core::JoypadButton::B)
        );
        assert_eq!(
            super::joypad_button_for_key(joypad, Keycode::X),
            Some(gb_core::JoypadButton::A)
        );
        assert_eq!(
            super::joypad_button_for_key(joypad, Keycode::Backspace),
            Some(gb_core::JoypadButton::Select)
        );
        assert_eq!(
            super::joypad_button_for_key(joypad, Keycode::Return),
            Some(gb_core::JoypadButton::Start)
        );
        assert_eq!(super::joypad_button_for_key(joypad, Keycode::F5), None);

        let keyboard_bindings = gb_desktop::KeyboardBindings::default();
        assert!(matches!(
            super::hotkey_action(&keyboard_bindings, Keycode::F5),
            super::HotkeyAction::ManualSave
        ));
        assert!(matches!(
            super::hotkey_action(&keyboard_bindings, Keycode::R),
            super::HotkeyAction::Reset
        ));
        assert!(matches!(
            super::hotkey_action(&keyboard_bindings, Keycode::F11),
            super::HotkeyAction::ToggleFullscreen
        ));
        assert!(matches!(
            super::hotkey_action(&keyboard_bindings, Keycode::F10),
            super::HotkeyAction::TogglePerformanceHud
        ));
        assert!(matches!(
            super::hotkey_action(&keyboard_bindings, Keycode::Space),
            super::HotkeyAction::None
        ));

        let menu_bindings = GamepadMenuBindings::default();
        assert_eq!(
            menu_input_for_gamepad_button(menu_bindings, Button::DPadUp),
            Some(super::MenuInput::Up)
        );
        assert_eq!(
            menu_input_for_gamepad_button(menu_bindings, Button::DPadDown),
            Some(super::MenuInput::Down)
        );

        assert_eq!(
            super::compact_recent_rom_label(Path::new("/tmp/(([])).gb")).as_str(),
            "ROM"
        );
        assert_eq!(
            map_path_dialog_result(Ok(Vec::new())),
            PathDialogResult::Canceled
        );
        assert!(matches!(
            map_path_dialog_result(Err(DialogError::SdlError(sdl3::get_error()))),
            PathDialogResult::Failed(_)
        ));
        assert_eq!(
            super::diagnostic_severity_name(CartridgeDiagnosticSeverity::Error),
            "error"
        );
        assert_eq!(
            super::execution_mode_name(ExecutionMode::Experimental),
            "experimental"
        );
        assert_eq!(
            super::framebuffer_pixel_to_grayscale(7),
            super::DMG_GRAYSCALE_SHADES[3]
        );
    }

    #[test]
    fn run_desktop_supports_headless_startup_with_and_without_an_initial_rom() {
        let _guard = crate::lock_sdl_test();
        crate::configure_headless_sdl();

        let launcher_root = temp_test_root("headless-launcher");
        let mut launcher_config = DesktopConfig::default();
        launcher_config.input.gamepad.enabled = false;
        let launcher_store =
            DesktopSettingsStore::new_for_tests(launcher_root.join("desktop-settings.toml"));
        let launcher_quit = schedule_quit_event();
        run_desktop(
            DesktopRunOptions {
                rom_path: None,
                linked_peer_rom_path: None,
                exit_after_frames: None,
                config: launcher_config,
                audio_recording: None,
            },
            launcher_store,
        )
        .expect("launcher should start and stop cleanly under headless SDL");
        launcher_quit
            .join()
            .expect("launcher quit-event helper should finish");

        let rom_root = temp_test_root("headless-rom");
        let rom_path = write_test_rom(&rom_root, "headless.gb");
        crate::configure_headless_sdl();
        let mut rom_config = DesktopConfig::default();
        rom_config.input.gamepad.enabled = false;
        let rom_store = DesktopSettingsStore::new_for_tests(rom_root.join("desktop-settings.toml"));
        let rom_quit = schedule_quit_event();
        run_desktop(
            DesktopRunOptions {
                rom_path: Some(rom_path),
                linked_peer_rom_path: None,
                exit_after_frames: None,
                config: rom_config,
                audio_recording: None,
            },
            rom_store,
        )
        .expect("ROM startup should run and stop cleanly under headless SDL");
        rom_quit
            .join()
            .expect("ROM quit-event helper should finish");
    }

    #[test]
    fn run_desktop_writes_audio_recordings_and_stems() {
        let _guard = crate::lock_sdl_test();
        crate::configure_headless_sdl();

        let root = temp_test_root("headless-audio-recording");
        let rom_path = write_test_rom(&root, "audio-recording.gb");
        let output_path = root.join("audio-recording.wav");
        let stem_ch1_path = root.join("audio-recording.ch1.wav");
        let stem_ch4_path = root.join("audio-recording.ch4.wav");

        let mut config = DesktopConfig::default();
        config.boot_rom.verification = BootRomVerificationMode::Off;
        config.input.gamepad.enabled = false;
        config.audio.enabled = false;
        let quit = schedule_quit_event();

        run_desktop(
            DesktopRunOptions {
                rom_path: Some(rom_path),
                linked_peer_rom_path: None,
                exit_after_frames: None,
                config,
                audio_recording: Some(DesktopAudioRecordingOptions {
                    output_path: output_path.clone(),
                    sample_rate_hz: 96_000,
                    stem_channels: vec![ApuRecordedChannel::Ch1, ApuRecordedChannel::Ch4],
                }),
            },
            DesktopSettingsStore::new_for_tests(root.join("desktop-settings.toml")),
        )
        .expect("audio-recording run should complete");
        quit.join()
            .expect("audio-recording quit-event helper should finish");

        let mix_len = fs::metadata(&output_path)
            .expect("mixed recording should exist")
            .len();
        let ch1_len = fs::metadata(&stem_ch1_path)
            .expect("ch1 stem should exist")
            .len();
        let ch4_len = fs::metadata(&stem_ch4_path)
            .expect("ch4 stem should exist")
            .len();
        assert!(mix_len > 44);
        assert!(ch1_len > 44);
        assert!(ch4_len > 44);
    }

    #[test]
    fn load_initial_emulation_session_supports_direct_linked_startup() {
        let root = temp_test_root("direct-linked-startup");
        let primary_rom_path = write_test_rom(&root, "primary.gb");
        let secondary_rom_path = write_test_rom(&root, "secondary.gb");
        let primary_bytes = fs::read(&primary_rom_path).expect("primary ROM should exist");
        let secondary_bytes = fs::read(&secondary_rom_path).expect("secondary ROM should exist");
        let mut session = super::DesktopSession {
            config: DesktopConfig::default(),
            current_dir: root.clone(),
            loaded_rom: Some(super::LoadedRom {
                path: primary_rom_path,
                bytes: primary_bytes,
            }),
            linked_secondary_rom: Some(super::LoadedRom {
                path: secondary_rom_path,
                bytes: secondary_bytes,
            }),
            last_open_directory: Some(root.clone()),
            recent_roms: Vec::new(),
            external_port_selection: super::DesktopExternalPortSelection::GameLink,
        };

        let (machine, diagnostics) = super::load_initial_emulation_session(&mut session)
            .expect("linked desktop startup helper should build a DMG-04 session");

        assert!(diagnostics.is_empty());
        assert!(machine.is_linked_dmg04_two_player());
        assert!(machine.secondary_machine().is_some());
    }

    #[test]
    fn prepare_machine_config_falls_back_to_skip_boot_when_the_selected_boot_rom_is_missing() {
        let root = temp_test_root("missing-bootrom-fallback");
        let mut config = DesktopConfig::default();
        config.launch.startup_mode = StartupMode::RealBoot;
        config.boot_rom.verification = BootRomVerificationMode::Strict;
        config.boot_rom.search_path = Some(root.join("missing-dmg.bin"));

        let prepared = super::prepare_machine_config(&config, &root)
            .expect("missing boot ROM paths should degrade to skip-boot");

        assert_eq!(
            prepared.effective_config.launch.startup_mode,
            StartupMode::SkipBoot
        );
        assert_eq!(prepared.machine_config.startup_mode, StartupMode::SkipBoot);
        assert!(prepared.machine_config.boot_rom_assets.is_empty());
        assert!(
            prepared
                .boot_rom_fallback_warning
                .as_deref()
                .is_some_and(|warning| warning.contains("falling back to skip-boot"))
        );
    }

    #[test]
    fn prepare_machine_config_keeps_strict_real_boot_errors_for_existing_invalid_images() {
        let root = temp_test_root("invalid-bootrom-strict");
        let image_path = root.join("dmg_boot.bin");
        fs::write(&image_path, vec![0x99; 0x100]).expect("synthetic boot ROM image should exist");

        let mut config = DesktopConfig::default();
        config.launch.startup_mode = StartupMode::RealBoot;
        config.boot_rom.verification = BootRomVerificationMode::Strict;
        config.boot_rom.search_path = Some(image_path);

        let error = super::prepare_machine_config(&config, &root)
            .expect_err("strict real-boot should still reject invalid existing images");
        assert!(error.contains("unexpected sha256"));
    }

    #[test]
    fn run_desktop_persists_skip_boot_after_missing_boot_rom_startup_fallback() {
        let _guard = crate::lock_sdl_test();
        crate::configure_headless_sdl();

        let root = temp_test_root("startup-fallback-persist");
        let settings_path = root.join("desktop-settings.toml");
        let mut settings_store = DesktopSettingsStore::new_for_tests(settings_path.clone());
        let mut config = DesktopConfig::default();
        config.launch.startup_mode = StartupMode::RealBoot;
        config.boot_rom.verification = BootRomVerificationMode::Strict;
        config.boot_rom.search_path = Some(root.join("missing-boot.bin"));
        config.input.gamepad.enabled = false;
        settings_store
            .persist_machine_preferences(&config)
            .expect("stale real-boot settings should persist");

        let quit = schedule_quit_event();
        super::run_desktop_with_startup_fallback_persistence(
            DesktopRunOptions {
                rom_path: None,
                linked_peer_rom_path: None,
                exit_after_frames: None,
                config,
                audio_recording: None,
            },
            settings_store,
            true,
        )
        .expect("desktop should start after degrading the missing boot ROM");
        quit.join()
            .expect("startup fallback quit-event helper should finish");

        let persisted =
            fs::read_to_string(&settings_path).expect("desktop settings should persist");
        assert!(persisted.contains("startup_mode = \"skip-boot\""));
        assert!(!persisted.contains("startup_mode = \"real-boot\""));
    }

    #[test]
    fn run_desktop_processes_hotkeys_plus_video_and_audio_menu_actions() {
        let _guard = crate::lock_sdl_test();
        crate::configure_headless_sdl();

        let root = temp_test_root("video-audio-actions");
        let rom_path = write_test_rom(&root, "video-audio.gb");
        let settings_path = root.join("desktop-settings.toml");
        let mut config = DesktopConfig::default();
        config.input.gamepad.enabled = false;
        let settings_store = DesktopSettingsStore::new_for_tests(settings_path.clone());
        let sequence = schedule_key_sequence(vec![
            (Keycode::F11, true),
            (Keycode::Z, true),
            (Keycode::Z, false),
            (Keycode::Escape, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Escape, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Return, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
        ]);

        run_desktop(
            DesktopRunOptions {
                rom_path: Some(rom_path.clone()),
                linked_peer_rom_path: None,
                exit_after_frames: None,
                config,
                audio_recording: None,
            },
            settings_store,
        )
        .expect("desktop frontend should process hotkeys and video/audio menu actions");
        sequence
            .join()
            .expect("video/audio key sequence helper should finish");

        let persisted = fs::read_to_string(&settings_path)
            .expect("desktop settings should persist after menu-driven changes");
        assert!(persisted.contains("fullscreen = true"));
        assert!(persisted.contains(&rom_path.display().to_string()));
    }

    #[test]
    fn run_desktop_processes_input_and_system_menu_actions() {
        let _guard = crate::lock_sdl_test();
        crate::configure_headless_sdl();

        let root = temp_test_root("input-system-actions");
        let rom_path = write_test_rom(&root, "input-system.gb");
        let settings_path = root.join("desktop-settings.toml");
        let mut config = DesktopConfig::default();
        config.input.gamepad.enabled = false;
        let settings_store = DesktopSettingsStore::new_for_tests(settings_path.clone());
        let sequence = schedule_key_sequence(vec![
            (Keycode::Escape, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Escape, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Return, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
            (Keycode::Down, true),
            (Keycode::Return, true),
        ]);

        run_desktop(
            DesktopRunOptions {
                rom_path: Some(rom_path.clone()),
                linked_peer_rom_path: None,
                exit_after_frames: None,
                config,
                audio_recording: None,
            },
            settings_store,
        )
        .expect("desktop frontend should process input and system menu actions");
        sequence
            .join()
            .expect("input/system key sequence helper should finish");

        let persisted = fs::read_to_string(&settings_path)
            .expect("desktop settings should persist after input/system changes");
        assert!(persisted.contains("version = 1"));
        assert!(persisted.contains(&rom_path.display().to_string()));
    }

    #[test]
    fn frontend_helpers_cover_runtime_dialog_and_title_utilities() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("helpers", true, false, false);

        assert!(harness.session.has_loaded_rom());
        assert!(harness.session.rom_path().is_some());
        assert!(harness.session.rom_bytes().is_some());
        assert_eq!(harness.session.rom_directory_hint(), harness.root.as_path());
        assert!(harness.session.recent_roms().is_empty());
        assert!(!harness.runtime.any_dialog_pending());

        let mut dialog = super::PathSelectionDialog::new();
        assert!(!dialog.is_pending());
        dialog.pending = true;
        dialog
            .show_file(
                &ROM_FILE_DIALOG_FILTERS,
                harness.canvas.window(),
                harness.root.as_path(),
            )
            .expect("pending file dialogs should be a no-op");
        dialog.show_folder(harness.canvas.window(), harness.root.as_path());
        dialog.pending = false;
        dialog
            .sender
            .send(super::PathDialogResult::Selected(
                harness.root.join("picked.gb"),
            ))
            .expect("dialog result should send");
        assert!(matches!(
            dialog.take_result(),
            Some(super::PathDialogResult::Selected(_))
        ));

        harness.performance_counter.sample_started_at = Instant::now() - Duration::from_secs(2);
        harness
            .performance_counter
            .record_presented_frame(
                harness.canvas.window_mut(),
                super::FramePerformanceSample {
                    session_kind: super::EmulationProfileSessionKind::Single,
                    emulation_duration: Duration::from_millis(10),
                    emulation_profile_request: None,
                    render_duration: Duration::from_millis(2),
                    present_duration: Duration::from_millis(1),
                    pacing_duration: Duration::from_millis(4),
                    pacing_sleep_target_duration: Duration::from_millis(4),
                    pacing_audio_correction_duration: Duration::from_millis(1),
                    pacing_late_duration: Duration::from_millis(2),
                    pacing_oversleep_duration: Duration::from_millis(1),
                    audio_submit_sample_count: Some(804),
                    audio_submit_t_cycles: Some(70_224),
                    audio_submit_queue_before_ms: Some(24.0),
                    audio_submit_enqueued_ms: Some(4.0),
                    audio_submit_queue_after_ms: Some(28.0),
                    audio_queue_before_pacing_ms: Some(20.0),
                    audio_queue_after_pacing_ms: Some(18.0),
                    frame_step_t_cycles: Some(70_224),
                    frame_start_ly: Some(0),
                    frame_start_dot: Some(0),
                    frame_end_ly: Some(0),
                    frame_end_dot: Some(0),
                    frame_origin_crossings: Some(1),
                    scanline_transitions: Some(154),
                    scanlines_over_456: Some(0),
                    max_scanline_t_cycles: Some(456),
                    max_scanline_ly: Some(153),
                    max_mode0_start_dot: Some(252),
                    max_mode0_start_dot_ly: Some(5),
                    ly_153_to_0_transitions: Some(1),
                    ly_153_to_0_startup_mode0: Some(0),
                    ly_153_to_0_blank_frame: Some(0),
                    ly_0_self_wraps: Some(0),
                    ly_0_self_wrap_startup_mode0: Some(0),
                    ly_0_self_wrap_blank_frame: Some(0),
                    ly_0_to_1_transitions: Some(1),
                    ly_0_scanline_t_cycles: Some(456),
                    ly_0_max_mode0_start_dot: Some(254),
                    ly_0_stall_t_cycles: Some(0),
                    ly_0_stall_hblank_t_cycles: Some(0),
                    ly_0_stall_oam_t_cycles: Some(0),
                    ly_0_stall_drawing_t_cycles: Some(0),
                    ly_0_stall_startup_mode0_t_cycles: Some(0),
                    ly_0_stall_blank_frame_t_cycles: Some(0),
                    ly_0_stall_runs: Some(0),
                    ly_0_max_stall_run_t_cycles: Some(0),
                    ly_0_max_stall_dot: Some(0),
                    ly_0_max_stall_mode_dot: Some(0),
                    cpu_stop_t_cycles: Some(0),
                    cpu_zombie_stop_t_cycles: Some(0),
                    ly_0_cpu_stop_t_cycles: Some(0),
                    ly_0_cpu_zombie_stop_t_cycles: Some(0),
                    ly_0_stall_cpu_stop_t_cycles: Some(0),
                    ly_0_stall_cpu_zombie_stop_t_cycles: Some(0),
                    lcd_disabled_t_cycles: Some(0),
                    lcd_disable_transitions: Some(0),
                    lcd_enable_transitions: Some(0),
                    ly_0_lcd_disabled_t_cycles: Some(0),
                    ly_0_stall_lcd_disabled_t_cycles: Some(0),
                },
            )
            .expect("performance counter should record a frame");
        assert!(harness.performance_counter.hud_snapshot().is_some());
        harness
            .performance_counter
            .reset_base_title(
                harness.canvas.window_mut(),
                "gb-desktop | reset".to_string(),
            )
            .expect("resetting the window title should succeed");

        super::show_message_box(
            None,
            sdl3::messagebox::MessageBoxFlag::WARNING,
            "warn",
            "msg",
        );
        super::show_warning_message(None, "warn", "msg");
        super::show_error_message(None, "error", "msg");
        assert_eq!(
            super::diagnostic_severity_name(CartridgeDiagnosticSeverity::Warning),
            "warning"
        );
        super::write_cartridge_diagnostics(&[CartridgeDiagnostic {
            severity: CartridgeDiagnosticSeverity::Warning,
            message: "test warning".to_string(),
        }]);
        assert!(super::target_frame_rate_hz() > 0.0);
        assert_eq!(super::gamepad_event_joystick_id(7).0, 7);
        assert_eq!(
            super::boot_rom_dialog_default_location(&harness.session),
            harness.root.join(DEFAULT_BOOT_ROM_DIR)
        );
        harness.session.config.boot_rom.search_path = Some(PathBuf::from("custom/boot.bin"));
        assert_eq!(
            super::boot_rom_dialog_default_location(&harness.session),
            harness.root.join("custom")
        );
        assert_eq!(
            super::save_directory_dialog_default_location(&harness.session),
            harness.root
        );
        harness.session.config.saves.directory_policy =
            gb_desktop::SaveDirectoryPolicy::Custom(PathBuf::from("custom/saves/state.sav"));
        assert_eq!(
            super::save_directory_dialog_default_location(&harness.session),
            harness.root.join("custom/saves")
        );

        let (replacement_sender, _) = std::sync::mpsc::channel();
        let (disconnected_sender, disconnected_receiver) = std::sync::mpsc::channel();
        drop(disconnected_sender);
        dialog.sender = replacement_sender;
        dialog.receiver = disconnected_receiver;
        dialog.pending = true;
        assert_eq!(dialog.take_result(), None);
        assert!(!dialog.pending);
    }

    #[test]
    fn frontend_harness_processes_dialog_results_and_recent_rom_paths() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("dialogs", false, false, false);
        let relative_rom_name = "picked.gb";
        let relative_rom_path = harness.root.join(relative_rom_name);
        fs::write(
            &relative_rom_path,
            build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        )
        .expect("dialog test ROM should be writable");
        let boot_file = harness.root.join("custom-boot.bin");
        let boot_dir = harness.root.join("boot-assets");
        let save_dir = harness.root.join("save-root");
        fs::write(&boot_file, vec![0_u8; 0x0100]).expect("boot file should be writable");
        fs::create_dir_all(&boot_dir).expect("boot directory should be creatable");
        fs::create_dir_all(&save_dir).expect("save directory should be creatable");

        assert!(!harness.session.has_loaded_rom());
        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                &harness.machine,
                &harness.session,
            ));
        assert!(harness.runtime.menu_state.is_open());

        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Selected(PathBuf::from(relative_rom_name)))
            .expect("open ROM selection should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("selected ROM should load");
        assert_eq!(
            harness.session.rom_path(),
            Some(relative_rom_path.as_path())
        );
        assert_eq!(
            harness.session.last_open_directory.as_deref(),
            Some(harness.root.as_path())
        );
        assert!(!harness.runtime.paused);
        assert!(!harness.runtime.menu_state.is_open());
        assert_eq!(
            harness.session.recent_roms().first(),
            Some(&relative_rom_path)
        );

        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Canceled)
            .expect("open ROM cancel should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("canceled ROM dialog should be ignored");
        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Failed("open failed".to_string()))
            .expect("open ROM failure should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("failed ROM dialog should be reported");

        harness
            .runtime
            .boot_rom_file_dialog
            .sender
            .send(PathDialogResult::Selected(boot_file.clone()))
            .expect("boot ROM file selection should send");
        harness
            .process_pending_boot_rom_file_dialog()
            .expect("selected boot ROM file should update the config");
        assert_eq!(
            harness.session.config.boot_rom.search_path.as_deref(),
            Some(boot_file.as_path())
        );

        harness
            .runtime
            .boot_rom_file_dialog
            .sender
            .send(PathDialogResult::Failed("boot file failed".to_string()))
            .expect("boot ROM file failure should send");
        harness
            .process_pending_boot_rom_file_dialog()
            .expect("failed boot ROM file dialog should be reported");
        harness
            .runtime
            .boot_rom_file_dialog
            .sender
            .send(PathDialogResult::Canceled)
            .expect("boot ROM file cancel should send");
        harness
            .process_pending_boot_rom_file_dialog()
            .expect("canceled boot ROM file dialog should be ignored");

        harness
            .runtime
            .boot_rom_directory_dialog
            .sender
            .send(PathDialogResult::Selected(boot_dir.clone()))
            .expect("boot ROM directory selection should send");
        harness
            .process_pending_boot_rom_directory_dialog()
            .expect("selected boot ROM directory should update the config");
        assert_eq!(
            harness.session.config.boot_rom.search_path.as_deref(),
            Some(boot_dir.as_path())
        );

        harness
            .runtime
            .boot_rom_directory_dialog
            .sender
            .send(PathDialogResult::Failed("boot dir failed".to_string()))
            .expect("boot ROM directory failure should send");
        harness
            .process_pending_boot_rom_directory_dialog()
            .expect("failed boot ROM directory dialog should be reported");
        harness
            .runtime
            .boot_rom_directory_dialog
            .sender
            .send(PathDialogResult::Canceled)
            .expect("boot ROM directory cancel should send");
        harness
            .process_pending_boot_rom_directory_dialog()
            .expect("canceled boot ROM directory dialog should be ignored");

        harness
            .runtime
            .save_directory_dialog
            .sender
            .send(PathDialogResult::Selected(save_dir.clone()))
            .expect("save directory selection should send");
        harness
            .process_pending_save_directory_dialog()
            .expect("selected save directory should update the config");
        assert_eq!(
            harness.session.config.saves.directory_policy,
            gb_desktop::SaveDirectoryPolicy::Custom(save_dir.clone())
        );

        harness
            .runtime
            .save_directory_dialog
            .sender
            .send(PathDialogResult::Failed("save dir failed".to_string()))
            .expect("save directory failure should send");
        harness
            .process_pending_save_directory_dialog()
            .expect("failed save directory dialog should be reported");
        harness
            .runtime
            .save_directory_dialog
            .sender
            .send(PathDialogResult::Canceled)
            .expect("save directory cancel should send");
        harness
            .process_pending_save_directory_dialog()
            .expect("canceled save directory dialog should be ignored");

        let missing_recent = harness.root.join("missing.gb");
        harness.session.recent_roms = vec![missing_recent.clone()];
        assert!(
            harness
                .execute_action(super::MenuAction::OpenRecentRom(0))
                .expect("missing recent ROM should be handled")
                .is_none()
        );
        assert!(!harness.session.recent_roms().contains(&missing_recent));

        let persisted = fs::read_to_string(&harness.settings_path)
            .expect("dialog actions should persist settings");
        assert!(persisted.contains(&boot_dir.display().to_string()));
        assert!(persisted.contains(&save_dir.display().to_string()));
    }

    #[test]
    fn resume_action_clears_manual_pause_after_screenshot_for_dialog_loaded_rom() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("resume-after-screenshot", false, false, false);
        let rom_name = "picked.gb";
        let rom_path = harness.root.join(rom_name);
        fs::write(&rom_path, build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
            .expect("dialog test ROM should be writable");

        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Selected(PathBuf::from(rom_name)))
            .expect("open ROM selection should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("selected ROM should load");
        assert_eq!(harness.session.rom_path(), Some(rom_path.as_path()));

        harness.runtime.paused = true;
        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                harness.machine.primary_machine(),
                &harness.session,
            ));

        assert!(
            harness
                .execute_action(super::MenuAction::SaveScreenshot)
                .expect("screenshot should save while paused")
                .is_none()
        );

        let resume_action = harness
            .runtime
            .menu_state
            .handle_input(
                super::MenuInput::Confirm,
                super::current_menu_presentation(
                    harness.canvas.window(),
                    &harness.runtime,
                    harness.machine.primary_machine(),
                    &harness.session,
                ),
            )
            .expect("root RESUME should stay available after taking a screenshot");
        assert_eq!(resume_action, super::MenuAction::Resume);

        assert!(
            harness
                .execute_action(resume_action)
                .expect("resume action should succeed")
                .is_none()
        );
        assert!(harness.session.has_loaded_rom());
        assert!(!harness.runtime.paused);
        assert!(!harness.runtime.menu_state.is_open());
        assert!(!super::emulation_paused(
            harness.machine.primary_machine(),
            &harness.runtime,
        ));
    }

    #[test]
    fn escape_resumes_after_screenshot_when_the_session_was_manually_paused() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("escape-after-screenshot", false, false, false);
        let rom_name = "picked.gb";
        let rom_path = harness.root.join(rom_name);
        fs::write(&rom_path, build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
            .expect("dialog test ROM should be writable");

        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Selected(PathBuf::from(rom_name)))
            .expect("open ROM selection should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("selected ROM should load");
        assert_eq!(harness.session.rom_path(), Some(rom_path.as_path()));

        harness.runtime.paused = true;
        harness.push_key(Keycode::Escape, true);
        harness.process_events().expect("menu open should process");
        assert!(harness.runtime.menu_state.is_open());

        assert!(
            harness
                .execute_action(super::MenuAction::SaveScreenshot)
                .expect("screenshot should save while paused")
                .is_none()
        );

        harness.push_key(Keycode::Escape, true);
        harness.process_events().expect("menu close should process");

        assert!(harness.session.has_loaded_rom());
        assert!(!harness.runtime.paused);
        assert!(!harness.runtime.menu_state.is_open());
        assert!(!super::emulation_paused(
            harness.machine.primary_machine(),
            &harness.runtime,
        ));
    }

    #[test]
    fn opening_a_new_primary_rom_clears_manual_pause_state() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("open-rom-clears-pause", false, false, false);
        let first_rom_name = "first.gb";
        let first_rom_path = harness.root.join(first_rom_name);
        fs::write(&first_rom_path, build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
            .expect("first ROM should be writable");

        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Selected(PathBuf::from(first_rom_name)))
            .expect("first open ROM selection should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("first ROM should load");
        assert_eq!(harness.session.rom_path(), Some(first_rom_path.as_path()));

        harness.runtime.paused = true;
        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                harness.machine.primary_machine(),
                &harness.session,
            ));

        let second_rom_name = "second.gb";
        let second_rom_path = harness.root.join(second_rom_name);
        fs::write(
            &second_rom_path,
            build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        )
        .expect("second ROM should be writable");
        harness
            .runtime
            .open_rom_dialog
            .sender
            .send(PathDialogResult::Selected(PathBuf::from(second_rom_name)))
            .expect("second open ROM selection should send");
        harness
            .process_pending_open_rom_dialog()
            .expect("second ROM should load");

        assert_eq!(harness.session.rom_path(), Some(second_rom_path.as_path()));
        assert!(harness.session.has_loaded_rom());
        assert!(!harness.runtime.paused);
        assert!(!harness.runtime.menu_state.is_open());
        assert!(!super::emulation_paused(
            harness.machine.primary_machine(),
            &harness.runtime,
        ));
    }

    #[test]
    fn opening_a_recent_rom_clears_manual_pause_state() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("recent-rom-clears-pause", true, false, false);
        let recent_rom_path = harness.root.join("recent.gb");
        fs::write(
            &recent_rom_path,
            build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        )
        .expect("recent ROM should be writable");
        harness.session.recent_roms = vec![recent_rom_path.clone()];

        harness.runtime.paused = true;
        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                harness.machine.primary_machine(),
                &harness.session,
            ));

        assert!(
            harness
                .execute_action(super::MenuAction::OpenRecentRom(0))
                .expect("recent ROM should open")
                .is_none()
        );

        assert_eq!(harness.session.rom_path(), Some(recent_rom_path.as_path()));
        assert!(harness.session.has_loaded_rom());
        assert!(!harness.runtime.paused);
        assert!(!harness.runtime.menu_state.is_open());
        assert!(!super::emulation_paused(
            harness.machine.primary_machine(),
            &harness.runtime,
        ));
    }

    #[test]
    fn frontend_harness_covers_event_loop_frame_and_render_helpers() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("runtime", true, true, true);
        let relative_rom = PathBuf::from("runtime.gb");
        let relative_rom_path = harness.root.join(&relative_rom);
        fs::write(
            &relative_rom_path,
            build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        )
        .expect("runtime ROM should be writable");

        let loaded = super::load_initial_rom(
            &DesktopRunOptions {
                rom_path: Some(relative_rom.clone()),
                linked_peer_rom_path: None,
                exit_after_frames: None,
                config: DesktopConfig::default(),
                audio_recording: None,
            },
            &harness.root,
        )
        .expect("relative ROM path should load")
        .expect("relative ROM should exist");
        assert_eq!(loaded.path, relative_rom_path);
        assert!(
            super::load_initial_rom(
                &DesktopRunOptions {
                    rom_path: None,
                    linked_peer_rom_path: None,
                    exit_after_frames: None,
                    config: DesktopConfig::default(),
                    audio_recording: None,
                },
                &harness.root,
            )
            .expect("missing ROM path should be allowed")
            .is_none()
        );
        let linked_loaded = super::load_initial_linked_secondary_rom(
            &DesktopRunOptions {
                rom_path: Some(relative_rom.clone()),
                linked_peer_rom_path: Some(relative_rom.clone()),
                exit_after_frames: Some(8),
                config: DesktopConfig::default(),
                audio_recording: None,
            },
            &harness.root,
        )
        .expect("relative linked peer path should load")
        .expect("relative linked peer should exist");
        assert_eq!(linked_loaded.path, relative_rom_path);
        assert!(super::should_exit_after_presented_frames(Some(4), 4));
        assert!(!super::should_exit_after_presented_frames(Some(5), 4));
        assert!(!super::should_exit_after_presented_frames(None, 4));

        let mut reloaded_machine = super::load_machine_for_rom(
            &harness.session.config,
            &harness.session.current_dir,
            harness.session.rom_bytes().expect("loaded ROM bytes"),
        )
        .expect("machine should reload from ROM bytes")
        .machine;
        assert!(
            super::open_save_session_for_session(&harness.session, &mut reloaded_machine)
                .expect("save session should open for the loaded ROM")
                .is_none()
        );

        super::run_from_cli(["--help"]).expect("help path should succeed");
        let expected_toggled_device = harness
            .runtime
            .gamepad_manager
            .as_ref()
            .and_then(super::GamepadManager::active_gamepad_identity)
            .unwrap_or_default();
        assert_eq!(
            super::toggled_preferred_gamepad_device(
                harness
                    .runtime
                    .gamepad_manager
                    .as_ref()
                    .expect("runtime test should have a gamepad manager")
            ),
            expected_toggled_device
        );
        if !expected_toggled_device.is_configured() {
            assert_eq!(
                harness
                    .runtime
                    .gamepad_manager
                    .as_ref()
                    .expect("runtime test should have a gamepad manager")
                    .preferred_device(),
                &gb_desktop::PreferredGamepadIdentity {
                    path: None,
                    name: Some("Saved Pad".to_string()),
                }
            );
        } else {
            harness
                .runtime
                .gamepad_manager
                .as_mut()
                .expect("runtime test should have a gamepad manager")
                .set_preferred_device(
                    expected_toggled_device.clone(),
                    &mut harness.runtime.input_state,
                    &mut harness.machine,
                );
            assert_eq!(
                super::toggled_preferred_gamepad_device(
                    harness
                        .runtime
                        .gamepad_manager
                        .as_ref()
                        .expect("runtime test should have a gamepad manager")
                ),
                gb_desktop::PreferredGamepadIdentity::default()
            );
        }

        harness.push_key(Keycode::Z, true);
        assert!(matches!(
            harness
                .process_events()
                .expect("keyboard press should process"),
            super::LoopSignal::Continue
        ));
        harness.machine.step_t_cycle();
        assert_ne!(harness.machine.joypad().snapshot().pressed_mask, 0);
        harness.push_key(Keycode::Z, false);
        harness
            .process_events()
            .expect("keyboard release should process");
        harness.machine.step_t_cycle();
        assert_eq!(harness.machine.joypad().snapshot().pressed_mask, 0);

        harness.push_key(Keycode::Escape, true);
        harness.process_events().expect("menu open should process");
        assert!(harness.runtime.menu_state.is_open());
        harness.push_key(Keycode::Escape, true);
        harness.process_events().expect("menu close should process");
        assert!(!harness.runtime.menu_state.is_open());

        assert!(matches!(
            harness
                .step_until_next_frame()
                .expect("frame stepping should complete"),
            super::LoopSignal::Continue
        ));

        super::apply_window_scale(harness.canvas.window_mut(), 3)
            .expect("window scale should apply");
        super::set_fullscreen_state(harness.canvas.window_mut(), false)
            .expect("setting the existing fullscreen state should be a no-op");
        super::reset_machine(
            harness.canvas.window(),
            &mut harness.session,
            &mut harness.machine,
            &mut harness.runtime,
            &mut harness.settings_store,
        )
        .expect("machine reset should succeed");

        let texture_creator = harness.canvas.texture_creator();
        let mut texture = texture_creator
            .create_texture_streaming(
                sdl3::pixels::PixelFormat::RGB24,
                super::FRAMEBUFFER_WIDTH,
                super::FRAMEBUFFER_HEIGHT,
            )
            .expect("runtime texture should be creatable");
        let mut rgb_frame =
            vec![0_u8; super::FRAMEBUFFER_HEIGHT as usize * super::FRAMEBUFFER_PITCH_BYTES];
        let menu_presentation = super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        );
        harness.runtime.menu_state.open(menu_presentation);
        let open_menu_presentation = super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        );
        let _ = super::render_frame(
            &mut harness.canvas,
            &mut texture,
            &mut rgb_frame,
            super::FramebufferRenderInput {
                dimensions: super::FramebufferDimensions {
                    width: super::FRAMEBUFFER_WIDTH,
                    height: super::FRAMEBUFFER_HEIGHT,
                },
                primary: super::FramebufferPanelInput {
                    framebuffer: harness.machine.ppu().framebuffer(),
                    framebuffer_layer_sources: harness.machine.ppu().framebuffer_layer_sources(),
                    bgwin_framebuffer: harness.machine.ppu().framebuffer_bgwin_panel_shades(),
                    backdrop_framebuffer: harness.machine.ppu().framebuffer_backdrop_panel_shades(),
                    bgwin_framebuffer_layer_sources: harness
                        .machine
                        .ppu()
                        .framebuffer_bgwin_layer_sources(),
                },
                secondary: None,
            },
            &harness.runtime.video_options,
            Some((&harness.runtime.menu_state, open_menu_presentation)),
            None,
        )
        .expect("overlay frame should render");
        assert!(rgb_frame.iter().any(|byte| *byte != 0));

        harness.runtime.menu_state.close();
        harness.runtime.video_options.show_performance_hud = true;
        let _ = super::render_frame(
            &mut harness.canvas,
            &mut texture,
            &mut rgb_frame,
            super::FramebufferRenderInput {
                dimensions: super::FramebufferDimensions {
                    width: super::FRAMEBUFFER_WIDTH,
                    height: super::FRAMEBUFFER_HEIGHT,
                },
                primary: super::FramebufferPanelInput {
                    framebuffer: harness.machine.ppu().framebuffer(),
                    framebuffer_layer_sources: harness.machine.ppu().framebuffer_layer_sources(),
                    bgwin_framebuffer: harness.machine.ppu().framebuffer_bgwin_panel_shades(),
                    backdrop_framebuffer: harness.machine.ppu().framebuffer_backdrop_panel_shades(),
                    bgwin_framebuffer_layer_sources: harness
                        .machine
                        .ppu()
                        .framebuffer_bgwin_layer_sources(),
                },
                secondary: None,
            },
            &harness.runtime.video_options,
            None,
            Some(PerformanceHudSnapshot {
                fps: 59.7,
                speed_percent: 100.0,
                frame_time_ms: 16.7,
                emulation_time_ms: 10.0,
                render_time_ms: 2.0,
                pacing_time_ms: 4.0,
                audio_queue_ms: Some(12.5),
            }),
        )
        .expect("HUD frame should render");
        assert!(rgb_frame.iter().any(|byte| *byte != 0));
    }

    #[test]
    fn linked_runtime_routes_primary_and_secondary_keyboard_input_independently() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("linked-keyboard-routing", true, false, false);
        let secondary_machine = Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        harness
            .machine
            .attach_secondary_dmg04(secondary_machine)
            .expect("secondary machine should attach");

        harness.push_key(Keycode::Z, true);
        harness.push_key_with_scancode(Keycode::W, Scancode::W, true);
        harness
            .process_events()
            .expect("linked keyboard press should process");
        harness.machine.step_t_cycle();

        assert_eq!(
            harness
                .machine
                .primary_machine()
                .joypad()
                .snapshot()
                .pressed_mask,
            0x20
        );
        assert_eq!(
            harness
                .machine
                .secondary_machine()
                .expect("linked runtime should expose a secondary machine")
                .joypad()
                .snapshot()
                .pressed_mask,
            0x04
        );

        harness.push_key(Keycode::Z, false);
        harness.push_key_with_scancode(Keycode::W, Scancode::W, false);
        harness
            .process_events()
            .expect("linked keyboard release should process");
        harness.machine.step_t_cycle();

        assert_eq!(
            harness
                .machine
                .primary_machine()
                .joypad()
                .snapshot()
                .pressed_mask,
            0
        );
        assert_eq!(
            harness
                .machine
                .secondary_machine()
                .expect("linked runtime should expose a secondary machine")
                .joypad()
                .snapshot()
                .pressed_mask,
            0
        );
    }

    #[test]
    fn audio_source_machine_is_primary_for_single_and_linked_sessions() {
        let _guard = crate::lock_sdl_test();
        let single = FrontendHarness::new("audio-source-single", true, false, false).machine;
        assert_eq!(
            super::emulation_profile_session_kind(&single),
            super::EmulationProfileSessionKind::Single
        );
        assert!(std::ptr::eq(
            super::audio_source_machine(&single),
            single.primary_machine()
        ));

        let primary = dmg_skip_boot_summary_machine();
        let secondary = dmg_skip_boot_summary_machine();
        let linked = super::linked_session::DesktopEmulationSession::new_linked_dmg04_two_player(
            primary, secondary,
        )
        .expect("linked desktop session should build");

        assert_eq!(
            super::emulation_profile_session_kind(&linked),
            super::EmulationProfileSessionKind::LinkedDmg04TwoPlayer
        );
        assert!(std::ptr::eq(
            super::audio_source_machine(&linked),
            linked.primary_machine()
        ));
    }

    #[test]
    fn automatic_audio_recording_helpers_build_restart_and_finish_recorders() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("automatic-recorder", true, false, false);
        let channel_mask =
            super::ApuRecordedChannelMask::NONE.with_channel(super::ApuRecordedChannel::Ch4, true);
        let recorder = super::create_audio_recorder(
            &super::DesktopAudioRecordingMode::Automatic,
            channel_mask,
            &harness.session,
            &harness.machine,
        )
        .expect("automatic recorder creation should succeed")
        .expect("automatic mode should create a recorder");
        let first_path = harness.root.join("audios").join("automatic-recorder-0.wav");
        assert!(first_path.exists());
        assert_eq!(recorder.channel_mask(), channel_mask);

        let mut recorder_slot = Some(recorder);
        super::finish_audio_recorder(&mut recorder_slot).expect("finishing a live recorder");
        assert!(recorder_slot.is_none());

        harness.runtime.audio_recording_mode = super::DesktopAudioRecordingMode::Automatic;
        harness.runtime.audio_channel_mask = channel_mask;
        harness.runtime.audio_recorder = super::create_audio_recorder(
            &harness.runtime.audio_recording_mode,
            harness.runtime.audio_channel_mask,
            &harness.session,
            &harness.machine,
        )
        .expect("initial automatic recorder should build");
        super::restart_automatic_audio_recorder(
            &mut harness.runtime,
            &harness.session,
            &harness.machine,
        )
        .expect("restarting automatic recording should rotate to a new file");
        let second_path = harness.root.join("audios").join("automatic-recorder-1.wav");
        assert!(second_path.exists());
        assert!(harness.runtime.audio_recorder.is_some());

        super::finish_audio_recorder(&mut harness.runtime.audio_recorder)
            .expect("final recorder cleanup should succeed");
        assert!(harness.runtime.audio_recorder.is_none());
    }

    #[test]
    fn automatic_audio_recording_without_a_rom_falls_back_to_the_session_directory() {
        let _guard = crate::lock_sdl_test();
        let harness = FrontendHarness::new("automatic-recorder-no-rom", false, false, false);
        let recorder = super::create_audio_recorder(
            &super::DesktopAudioRecordingMode::Automatic,
            super::ApuRecordedChannelMask::ALL,
            &harness.session,
            &harness.machine,
        )
        .expect("automatic recorder creation without a rom should succeed");
        assert!(recorder.is_some());
        let fallback_path = harness.root.join("audios").join("gb-cycle-0.wav");
        assert!(fallback_path.exists());
    }

    #[test]
    fn render_frame_places_linked_secondary_output_in_the_right_panel() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("linked-render", true, false, false);
        let texture_creator = harness.canvas.texture_creator();
        let mut texture = texture_creator
            .create_texture_streaming(
                sdl3::pixels::PixelFormat::RGB24,
                super::FRAMEBUFFER_WIDTH * 2,
                super::FRAMEBUFFER_HEIGHT,
            )
            .expect("linked runtime texture should be creatable");
        let linked_dimensions = super::FramebufferDimensions {
            width: super::FRAMEBUFFER_WIDTH * 2,
            height: super::FRAMEBUFFER_HEIGHT,
        };
        let mut rgb_frame =
            vec![
                0_u8;
                linked_dimensions.height as usize
                    * super::framebuffer_pitch_bytes_for_dimensions(linked_dimensions)
            ];
        let primary_framebuffer =
            vec![0_u8; (super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT) as usize];
        let secondary_framebuffer =
            vec![3_u8; (super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT) as usize];
        let primary_sources =
            vec![PpuFramebufferLayerSource::Background; primary_framebuffer.len()];
        let secondary_sources =
            vec![PpuFramebufferLayerSource::Background; secondary_framebuffer.len()];

        let _ = super::render_frame(
            &mut harness.canvas,
            &mut texture,
            &mut rgb_frame,
            super::FramebufferRenderInput {
                dimensions: linked_dimensions,
                primary: super::FramebufferPanelInput {
                    framebuffer: &primary_framebuffer,
                    framebuffer_layer_sources: &primary_sources,
                    bgwin_framebuffer: &primary_framebuffer,
                    backdrop_framebuffer: &primary_framebuffer,
                    bgwin_framebuffer_layer_sources: &primary_sources,
                },
                secondary: Some(super::FramebufferPanelInput {
                    framebuffer: &secondary_framebuffer,
                    framebuffer_layer_sources: &secondary_sources,
                    bgwin_framebuffer: &secondary_framebuffer,
                    backdrop_framebuffer: &secondary_framebuffer,
                    bgwin_framebuffer_layer_sources: &secondary_sources,
                }),
            },
            &harness.runtime.video_options,
            None,
            None,
        )
        .expect("linked frame should render");

        let pitch = super::framebuffer_pitch_bytes_for_dimensions(linked_dimensions);
        let left_pixel = &rgb_frame[0..3];
        let right_pixel_index = super::FRAMEBUFFER_WIDTH as usize * 3;
        let right_pixel = &rgb_frame[right_pixel_index..right_pixel_index + 3];
        assert_eq!(left_pixel, &[super::framebuffer_pixel_to_grayscale(0); 3]);
        assert_eq!(right_pixel, &[super::framebuffer_pixel_to_grayscale(3); 3]);
        assert_eq!(rgb_frame.len(), linked_dimensions.height as usize * pitch);
    }

    #[test]
    fn render_frame_reveals_bgwin_pixels_when_objects_are_hidden() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("layer-mask-render", true, false, false);
        let texture_creator = harness.canvas.texture_creator();
        let mut texture = texture_creator
            .create_texture_streaming(
                sdl3::pixels::PixelFormat::RGB24,
                super::FRAMEBUFFER_WIDTH,
                super::FRAMEBUFFER_HEIGHT,
            )
            .expect("runtime texture should be creatable");
        let mut rgb_frame =
            vec![0_u8; super::FRAMEBUFFER_HEIGHT as usize * super::FRAMEBUFFER_PITCH_BYTES];
        let framebuffer =
            vec![3_u8; (super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT) as usize];
        let layer_sources = vec![PpuFramebufferLayerSource::Object; framebuffer.len()];
        let bgwin_framebuffer =
            vec![1_u8; (super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT) as usize];
        let bgwin_layer_sources = vec![PpuFramebufferLayerSource::Window; framebuffer.len()];
        let mut video_options = harness.runtime.video_options.clone();
        video_options.show_objects = false;

        super::render_frame(
            &mut harness.canvas,
            &mut texture,
            &mut rgb_frame,
            super::FramebufferRenderInput {
                dimensions: super::FramebufferDimensions {
                    width: super::FRAMEBUFFER_WIDTH,
                    height: super::FRAMEBUFFER_HEIGHT,
                },
                primary: super::FramebufferPanelInput {
                    framebuffer: &framebuffer,
                    framebuffer_layer_sources: &layer_sources,
                    bgwin_framebuffer: &bgwin_framebuffer,
                    backdrop_framebuffer: &bgwin_framebuffer,
                    bgwin_framebuffer_layer_sources: &bgwin_layer_sources,
                },
                secondary: None,
            },
            &video_options,
            None,
            None,
        )
        .expect("layer-masked frame should render");

        assert_eq!(&rgb_frame[..3], &[170, 170, 170]);
    }

    #[test]
    fn render_frame_uses_dynamic_backdrop_when_bgwin_layers_are_hidden() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("layer-mask-dynamic-backdrop", true, false, false);
        let texture_creator = harness.canvas.texture_creator();
        let mut texture = texture_creator
            .create_texture_streaming(
                sdl3::pixels::PixelFormat::RGB24,
                super::FRAMEBUFFER_WIDTH,
                super::FRAMEBUFFER_HEIGHT,
            )
            .expect("runtime texture should be creatable");
        let mut rgb_frame =
            vec![0_u8; super::FRAMEBUFFER_HEIGHT as usize * super::FRAMEBUFFER_PITCH_BYTES];
        let mut framebuffer =
            vec![0_u8; (super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT) as usize];
        let mut layer_sources = vec![PpuFramebufferLayerSource::Background; framebuffer.len()];
        let bgwin_framebuffer =
            vec![1_u8; (super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT) as usize];
        let bgwin_layer_sources = vec![PpuFramebufferLayerSource::Window; framebuffer.len()];
        let mut backdrop_framebuffer =
            vec![2_u8; (super::FRAMEBUFFER_WIDTH * super::FRAMEBUFFER_HEIGHT) as usize];
        let mut video_options = harness.runtime.video_options.clone();
        video_options.show_background = false;
        video_options.show_window = false;
        backdrop_framebuffer[0] = 1;
        framebuffer[1] = 3;
        layer_sources[1] = PpuFramebufferLayerSource::Object;

        super::render_frame(
            &mut harness.canvas,
            &mut texture,
            &mut rgb_frame,
            super::FramebufferRenderInput {
                dimensions: super::FramebufferDimensions {
                    width: super::FRAMEBUFFER_WIDTH,
                    height: super::FRAMEBUFFER_HEIGHT,
                },
                primary: super::FramebufferPanelInput {
                    framebuffer: &framebuffer,
                    framebuffer_layer_sources: &layer_sources,
                    bgwin_framebuffer: &bgwin_framebuffer,
                    backdrop_framebuffer: &backdrop_framebuffer,
                    bgwin_framebuffer_layer_sources: &bgwin_layer_sources,
                },
                secondary: None,
            },
            &video_options,
            None,
            None,
        )
        .expect("OBJ-only frame should render with a dynamic backdrop");

        assert_eq!(&rgb_frame[..3], &[170, 170, 170]);
        assert_eq!(&rgb_frame[3..6], &[0, 0, 0]);
        assert_eq!(&rgb_frame[6..9], &[85, 85, 85]);
    }

    #[test]
    fn render_frame_applies_the_selected_presentation_filter_to_the_texture() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("render-scale-mode", true, false, false);
        let texture_creator = harness.canvas.texture_creator();
        let mut texture = texture_creator
            .create_texture_streaming(
                sdl3::pixels::PixelFormat::RGB24,
                super::FRAMEBUFFER_WIDTH,
                super::FRAMEBUFFER_HEIGHT,
            )
            .expect("runtime texture should be creatable");
        let mut rgb_frame =
            vec![0_u8; super::FRAMEBUFFER_HEIGHT as usize * super::FRAMEBUFFER_PITCH_BYTES];
        let framebuffer = super::FramebufferRenderInput {
            dimensions: super::FramebufferDimensions {
                width: super::FRAMEBUFFER_WIDTH,
                height: super::FRAMEBUFFER_HEIGHT,
            },
            primary: super::FramebufferPanelInput {
                framebuffer: harness.machine.ppu().framebuffer(),
                framebuffer_layer_sources: harness.machine.ppu().framebuffer_layer_sources(),
                bgwin_framebuffer: harness.machine.ppu().framebuffer_bgwin_panel_shades(),
                backdrop_framebuffer: harness.machine.ppu().framebuffer_backdrop_panel_shades(),
                bgwin_framebuffer_layer_sources: harness
                    .machine
                    .ppu()
                    .framebuffer_bgwin_layer_sources(),
            },
            secondary: None,
        };
        let mut video_options = harness.runtime.video_options.clone();

        video_options.presentation_filter = false;
        super::render_frame(
            &mut harness.canvas,
            &mut texture,
            &mut rgb_frame,
            framebuffer,
            &video_options,
            None,
            None,
        )
        .expect("nearest-neighbor frame should render");
        assert_eq!(texture.scale_mode(), sdl3::render::ScaleMode::Nearest);

        video_options.presentation_filter = true;
        super::render_frame(
            &mut harness.canvas,
            &mut texture,
            &mut rgb_frame,
            framebuffer,
            &video_options,
            None,
            None,
        )
        .expect("filtered frame should render");
        assert_eq!(texture.scale_mode(), sdl3::render::ScaleMode::Linear);
    }

    #[test]
    fn frontend_harness_covers_gamepad_event_paths() {
        let _guard = crate::lock_sdl_test();
        let virtual_gamepad = VirtualGamepad::attach("Runtime Pad");
        let mut harness = FrontendHarness::new("gamepad-events", true, false, true);
        harness
            ._gamepad_subsystem
            .as_ref()
            .expect("gamepad subsystem")
            .update();
        harness
            .runtime
            .gamepad_manager
            .as_mut()
            .expect("gamepad manager")
            .set_preferred_device(
                gb_desktop::PreferredGamepadIdentity {
                    path: None,
                    name: Some("Runtime Pad".to_string()),
                },
                &mut harness.runtime.input_state,
                &mut harness.machine,
            );
        assert_eq!(
            harness
                .runtime
                .gamepad_manager
                .as_ref()
                .and_then(super::GamepadManager::active_gamepad_name),
            Some("Runtime Pad")
        );

        let events = harness
            .sdl
            .event()
            .expect("event subsystem should initialize for controller events");
        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                &harness.machine,
                &harness.session,
            ));
        harness
            .runtime
            .menu_state
            .begin_gamepad_binding_capture_for_tests(GamepadBindingTarget::A);
        events
            .push_event(Event::ControllerButtonDown {
                timestamp: 0,
                which: virtual_gamepad.joystick_id.0,
                button: Button::North,
            })
            .expect("gamepad binding event should be pushable");
        harness
            .process_events()
            .expect("gamepad binding capture should process");
        assert_eq!(
            harness
                .runtime
                .gamepad_manager
                .as_ref()
                .expect("gamepad manager")
                .button_bindings()
                .a,
            GamepadButtonBinding::North
        );

        events
            .push_event(Event::ControllerButtonDown {
                timestamp: 0,
                which: virtual_gamepad.joystick_id.0,
                button: Button::Guide,
            })
            .expect("guide event should be pushable");
        harness
            .process_events()
            .expect("guide button should close the menu");
        assert!(!harness.runtime.menu_state.is_open());

        events
            .push_event(Event::ControllerButtonDown {
                timestamp: 0,
                which: virtual_gamepad.joystick_id.0,
                button: Button::Guide,
            })
            .expect("second guide event should be pushable");
        harness
            .process_events()
            .expect("guide button should open the menu");
        assert!(harness.runtime.menu_state.is_open());

        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                &harness.machine,
                &harness.session,
            ));
        events
            .push_event(Event::ControllerButtonDown {
                timestamp: 0,
                which: virtual_gamepad.joystick_id.0,
                button: Button::South,
            })
            .expect("menu confirm event should be pushable");
        harness
            .process_events()
            .expect("gamepad menu navigation should process");
        assert!(!harness.runtime.menu_state.is_open());
    }

    #[test]
    fn guide_button_keeps_the_launcher_open_without_a_loaded_rom() {
        let _guard = crate::lock_sdl_test();
        let virtual_gamepad = VirtualGamepad::attach("Launcher Pad");
        let mut harness = FrontendHarness::new("launcher-guide", false, false, true);
        harness
            ._gamepad_subsystem
            .as_ref()
            .expect("gamepad subsystem")
            .update();
        harness
            .runtime
            .gamepad_manager
            .as_mut()
            .expect("gamepad manager")
            .set_preferred_device(
                gb_desktop::PreferredGamepadIdentity {
                    path: None,
                    name: Some("Launcher Pad".to_string()),
                },
                &mut harness.runtime.input_state,
                &mut harness.machine,
            );

        let events = harness
            .sdl
            .event()
            .expect("event subsystem should initialize for controller events");
        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                &harness.machine,
                &harness.session,
            ));
        assert!(harness.runtime.menu_state.is_open());

        events
            .push_event(Event::ControllerButtonDown {
                timestamp: 0,
                which: virtual_gamepad.joystick_id.0,
                button: Button::Guide,
            })
            .expect("guide event should be pushable");
        harness
            .process_events()
            .expect("guide button should leave the launcher open");
        assert!(harness.runtime.menu_state.is_open());
    }

    #[test]
    fn guide_button_matches_keyboard_cancel_behavior_inside_submenus() {
        let _guard = crate::lock_sdl_test();
        let virtual_gamepad = VirtualGamepad::attach("Overlay Pad");
        let mut harness = FrontendHarness::new("guide-cancel", true, false, true);
        harness
            ._gamepad_subsystem
            .as_ref()
            .expect("gamepad subsystem")
            .update();
        harness
            .runtime
            .gamepad_manager
            .as_mut()
            .expect("gamepad manager")
            .set_preferred_device(
                gb_desktop::PreferredGamepadIdentity {
                    path: None,
                    name: Some("Overlay Pad".to_string()),
                },
                &mut harness.runtime.input_state,
                &mut harness.machine,
            );

        let events = harness
            .sdl
            .event()
            .expect("event subsystem should initialize for controller events");
        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                &harness.machine,
                &harness.session,
            ));

        for button in [Button::DPadDown, Button::DPadDown, Button::South] {
            events
                .push_event(Event::ControllerButtonDown {
                    timestamp: 0,
                    which: virtual_gamepad.joystick_id.0,
                    button,
                })
                .expect("menu navigation event should be pushable");
            harness
                .process_events()
                .expect("menu navigation should process");
        }
        assert!(harness.runtime.menu_state.is_open());

        events
            .push_event(Event::ControllerButtonDown {
                timestamp: 0,
                which: virtual_gamepad.joystick_id.0,
                button: Button::Guide,
            })
            .expect("guide event should be pushable");
        harness
            .process_events()
            .expect("guide button should back out of the submenu");
        assert!(harness.runtime.menu_state.is_open());

        events
            .push_event(Event::ControllerButtonDown {
                timestamp: 0,
                which: virtual_gamepad.joystick_id.0,
                button: Button::East,
            })
            .expect("cancel event should be pushable");
        harness
            .process_events()
            .expect("cancel button should close the root menu");
        assert!(!harness.runtime.menu_state.is_open());
    }

    #[test]
    fn frontend_harness_covers_keyboard_binding_capture_paths() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("keyboard-capture", true, false, false);
        let events = harness
            .sdl
            .event()
            .expect("event subsystem should initialize for keyboard capture");

        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                &harness.machine,
                &harness.session,
            ));
        harness
            .runtime
            .menu_state
            .begin_keyboard_binding_capture_for_tests(KeyboardBindingTarget::A);
        push_key_event(&events, Keycode::Space, true);
        harness
            .process_events()
            .expect("joypad keyboard capture should process");
        assert_eq!(
            harness.runtime.keyboard_bindings.joypad.a,
            DesktopKey::Space
        );

        harness
            .runtime
            .menu_state
            .begin_keyboard_menu_binding_capture_for_tests(KeyboardMenuBindingTarget::Confirm);
        push_key_event(&events, Keycode::F5, true);
        harness
            .process_events()
            .expect("menu keyboard capture should process");
        assert_eq!(
            harness.runtime.keyboard_bindings.menu.confirm,
            DesktopKey::F5
        );

        harness
            .runtime
            .menu_state
            .begin_keyboard_binding_capture_for_tests(KeyboardBindingTarget::B);
        push_key_event(&events, Keycode::Escape, true);
        harness
            .process_events()
            .expect("escape should cancel the active keyboard capture");
        assert!(!harness.runtime.menu_state.is_capturing_binding());
        assert_eq!(harness.runtime.keyboard_bindings.joypad.b, DesktopKey::Z);

        harness
            .runtime
            .menu_state
            .begin_keyboard_binding_capture_for_tests(KeyboardBindingTarget::Start);
        events
            .push_event(Event::Quit { timestamp: 0 })
            .expect("quit event should be pushable during binding capture");
        assert!(matches!(
            harness
                .process_events()
                .expect("quit should short-circuit the binding capture loop"),
            super::LoopSignal::Quit
        ));
    }

    #[test]
    fn frontend_harness_covers_presentation_fallbacks_and_missing_subsystems() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("fallbacks", false, false, false);
        harness.session.config.saves.directory_policy =
            gb_desktop::SaveDirectoryPolicy::Custom(harness.root.join("custom-saves"));
        let presentation = super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        );
        assert!(!presentation.rom_loaded);
        assert_eq!(presentation.recent_rom_count, 0);
        assert!(!presentation.save_directory_uses_default_path);
        assert!(!presentation.audio_available);
        assert!(!presentation.manual_save_available);
        assert!(!presentation.gamepad_available);
        assert!(!presentation.active_gamepad_connected);
        assert!(presentation.active_gamepad_label.is_empty());
        assert!(!presentation.preferred_gamepad_configured);
        assert!(presentation.preferred_gamepad_label.is_empty());

        assert!(
            harness
                .execute_action(super::MenuAction::SaveBattery)
                .expect("save action should no-op without a session")
                .is_none()
        );
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleMute)
                .expect("mute should no-op without audio")
                .is_none()
        );
        assert!(
            harness
                .execute_action(super::MenuAction::CycleAudioVolume)
                .expect("volume cycling should still update the runtime setting")
                .is_none()
        );
        assert_eq!(harness.runtime.audio_volume_percent, 25);
        assert!(
            harness
                .execute_action(super::MenuAction::ResetAudioDefaults)
                .expect("audio reset should no-op without audio")
                .is_none()
        );
        assert_eq!(harness.runtime.audio_volume_percent, 100);
        assert!(
            harness
                .execute_action(super::MenuAction::CycleGamepadDirectionalSource)
                .expect("directional source should no-op without a gamepad manager")
                .is_none()
        );
        assert!(
            harness
                .execute_action(super::MenuAction::CycleGamepadRumbleMode)
                .expect("rumble mode should no-op without a gamepad manager")
                .is_none()
        );
        assert!(
            harness
                .execute_action(super::MenuAction::TogglePreferredGamepad)
                .expect("preferred gamepad should no-op without a gamepad manager")
                .is_none()
        );
        assert!(
            harness
                .execute_action(super::MenuAction::SetGamepadBinding(
                    GamepadBindingTarget::A,
                    GamepadButtonBinding::South,
                ))
                .expect("gamepad bindings should no-op without a gamepad manager")
                .is_none()
        );
        assert!(
            harness
                .execute_action(super::MenuAction::SetGamepadMenuBinding(
                    GamepadMenuBindingTarget::Confirm,
                    GamepadButtonBinding::North,
                ))
                .expect("gamepad menu bindings should no-op without a gamepad manager")
                .is_none()
        );
        harness.runtime.menu_state.open(presentation);
        assert!(
            harness
                .execute_action(super::MenuAction::Reset)
                .expect("reset should close the menu even without a loaded ROM")
                .is_none()
        );
        assert!(!harness.runtime.menu_state.is_open());

        drop(harness);

        let mut gamepad_harness = FrontendHarness::new("saved-preferred", true, false, true);
        let preferred_device = gb_desktop::PreferredGamepadIdentity {
            path: Some("saved-path".to_string()),
            name: None,
        };
        gamepad_harness
            .runtime
            .gamepad_manager
            .as_mut()
            .expect("gamepad harness should have a manager")
            .set_preferred_device(
                preferred_device,
                &mut gamepad_harness.runtime.input_state,
                &mut gamepad_harness.machine,
            );
        let gamepad_presentation = super::current_menu_presentation(
            gamepad_harness.canvas.window(),
            &gamepad_harness.runtime,
            &gamepad_harness.machine,
            &gamepad_harness.session,
        );
        assert!(gamepad_presentation.gamepad_available);
        assert!(gamepad_presentation.preferred_gamepad_configured);
        assert_eq!(
            gamepad_presentation.gamepad_rumble_mode,
            GamepadRumbleMode::Strong
        );
        assert_eq!(
            gamepad_presentation.preferred_gamepad_label.as_str(),
            "SAVED"
        );
        let manager = gamepad_harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad harness should have a manager");
        assert_eq!(
            gamepad_presentation.active_gamepad_connected,
            manager.has_connected_gamepad()
        );
        if manager.has_connected_gamepad() {
            assert!(!gamepad_presentation.active_gamepad_label.is_empty());
        } else {
            assert!(gamepad_presentation.active_gamepad_label.is_empty());
        }
    }

    #[test]
    fn drain_printed_pages_into_printer_output_saves_png_and_updates_the_window() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("printer-sink", true, false, false);
        harness.session.external_port_selection = DesktopExternalPortSelection::Printer;
        super::apply_external_port_selection_to_machine(
            &mut harness.machine,
            harness.session.external_port_selection,
        );

        run_print_sequence(&mut harness.machine);
        super::drain_printed_pages_into_printer_output(
            harness.canvas.window(),
            &harness.session,
            &mut harness.runtime,
            &mut harness.machine,
        );

        assert_eq!(harness.machine.take_printed_pages().len(), 0);
        assert!(harness.runtime.printer_output.has_window());
        assert_eq!(
            harness.runtime.printer_output.latest_page_dimensions(),
            Some((160, 8))
        );
        let saved_path = harness
            .runtime
            .printer_output
            .last_saved_path()
            .expect("printer output should remember the saved PNG path");
        assert!(saved_path.exists());
        assert!(saved_path.starts_with(harness.root.join("printer")));
    }

    #[test]
    fn reset_machine_persists_skip_boot_when_the_boot_rom_path_goes_missing() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("reset-missing-bootrom", true, false, false);
        harness.session.config.launch.startup_mode = StartupMode::RealBoot;
        harness.session.config.boot_rom.verification = BootRomVerificationMode::Strict;
        harness.session.config.boot_rom.search_path = Some(harness.root.join("missing.bin"));
        harness
            .settings_store
            .persist_machine_preferences(&harness.session.config)
            .expect("stale real-boot settings should persist before reset");

        super::reset_machine(
            harness.canvas.window(),
            &mut harness.session,
            &mut harness.machine,
            &mut harness.runtime,
            &mut harness.settings_store,
        )
        .expect("reset should degrade missing boot ROM settings instead of failing");

        assert_eq!(
            harness.session.config.launch.startup_mode,
            StartupMode::SkipBoot
        );
        let persisted = fs::read_to_string(&harness.settings_path)
            .expect("reset fallback should update persisted settings");
        assert!(persisted.contains("startup_mode = \"skip-boot\""));
    }

    #[test]
    fn execute_menu_actions_update_runtime_machine_and_persisted_settings() {
        let _guard = crate::lock_sdl_test();
        let mut harness = FrontendHarness::new("actions", true, true, true);

        assert!(
            harness
                .execute_action(super::MenuAction::CycleConsoleModel)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.session.config.launch.console_model,
            DesktopConsoleModel::Mgb
        );
        assert!(
            harness
                .execute_action(super::MenuAction::CycleStartupMode)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.session.config.launch.startup_mode,
            StartupMode::RealBoot
        );
        assert!(
            harness
                .execute_action(super::MenuAction::CycleExecutionMode)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.session.config.launch.execution_mode,
            ExecutionMode::Permissive
        );
        harness.session.config.boot_rom.search_path = Some(harness.root.join("boot.bin"));
        assert!(
            harness
                .execute_action(super::MenuAction::ClearBootRomPath)
                .unwrap()
                .is_none()
        );
        assert!(harness.session.config.boot_rom.search_path.is_none());
        assert!(
            harness
                .execute_action(super::MenuAction::CycleBootRomVerify)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.session.config.boot_rom.verification,
            BootRomVerificationMode::Off
        );
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleSavesEnabled)
                .unwrap()
                .is_none()
        );
        assert!(!harness.session.config.saves.enabled);
        assert!(
            harness
                .execute_action(super::MenuAction::CycleSavePolicy)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.session.config.saves.flush_policy,
            DesktopSaveFlushPolicy::Manual
        );
        harness.session.config.saves.directory_policy =
            gb_desktop::SaveDirectoryPolicy::Custom(harness.root.join("manual-saves"));
        assert!(
            harness
                .execute_action(super::MenuAction::ClearSaveDirectoryPath)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.session.config.saves.directory_policy,
            gb_desktop::SaveDirectoryPolicy::RomFolderSavesSubdir
        );
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleVsync)
                .unwrap()
                .is_none()
        );
        assert!(!harness.runtime.video_options.vsync);
        assert!(
            harness
                .execute_action(super::MenuAction::CycleWindowScale)
                .unwrap()
                .is_none()
        );
        assert_eq!(harness.runtime.video_options.window_scale, 5);
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleIntegerScale)
                .unwrap()
                .is_none()
        );
        assert!(!harness.runtime.video_options.integer_scale);
        assert!(
            harness
                .execute_action(super::MenuAction::TogglePresentationFilter)
                .unwrap()
                .is_none()
        );
        assert!(harness.runtime.video_options.presentation_filter);
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleBackgroundLayer)
                .unwrap()
                .is_none()
        );
        assert!(!harness.runtime.video_options.show_background);
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleWindowLayer)
                .unwrap()
                .is_none()
        );
        assert!(!harness.runtime.video_options.show_window);
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleObjectLayer)
                .unwrap()
                .is_none()
        );
        assert!(!harness.runtime.video_options.show_objects);
        assert!(
            harness
                .execute_action(super::MenuAction::SaveScreenshot)
                .unwrap()
                .is_none()
        );
        let screenshot_path = harness.root.join("screenshots").join("actions-0.png");
        let encoded = fs::read(&screenshot_path).expect("screenshot PNG should exist");
        let decoder = png::Decoder::new(std::io::Cursor::new(encoded));
        let mut reader = decoder.read_info().expect("PNG header should decode");
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut buffer)
            .expect("PNG payload should decode");
        assert_eq!(info.width, super::FRAMEBUFFER_WIDTH);
        assert_eq!(info.height, super::FRAMEBUFFER_HEIGHT);
        assert_eq!(info.color_type, png::ColorType::Rgb);
        assert!(
            harness
                .execute_action(super::MenuAction::TogglePerformanceHud)
                .unwrap()
                .is_none()
        );
        assert!(!harness.runtime.video_options.show_performance_hud);
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleMute)
                .unwrap()
                .is_none()
        );
        assert!(
            harness
                .runtime
                .audio_output
                .as_ref()
                .is_some_and(|audio| audio.is_muted())
        );
        assert!(
            harness
                .execute_action(super::MenuAction::CycleAudioVolume)
                .unwrap()
                .is_none()
        );
        assert_eq!(harness.runtime.audio_volume_percent, 25);
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleAudioChannel(
                    ApuRecordedChannel::Ch2
                ))
                .unwrap()
                .is_none()
        );
        assert!(
            !harness
                .runtime
                .audio_channel_mask
                .contains(ApuRecordedChannel::Ch2)
        );
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleAudioRecording)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            harness.runtime.audio_recording_mode,
            super::DesktopAudioRecordingMode::Automatic
        ));
        assert!(harness.runtime.audio_recorder.is_some());
        let automatic_recording_path = harness.root.join("audios").join("actions-0.wav");
        assert!(automatic_recording_path.exists());
        assert!(
            harness
                .execute_action(super::MenuAction::ToggleAudioRecording)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            harness.runtime.audio_recording_mode,
            super::DesktopAudioRecordingMode::Disabled
        ));
        assert!(harness.runtime.audio_recorder.is_none());
        assert!(
            harness
                .execute_action(super::MenuAction::SetExternalPort(
                    DesktopExternalPortSelection::Printer,
                ))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.session.external_port_selection,
            DesktopExternalPortSelection::Printer
        );
        assert_eq!(
            harness.machine.external_port().attachment_kind(),
            ExternalPortAttachmentKind::Printer
        );
        assert!(
            harness
                .execute_action(super::MenuAction::SetExternalPort(
                    DesktopExternalPortSelection::None,
                ))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.session.external_port_selection,
            DesktopExternalPortSelection::None
        );
        assert_eq!(
            harness.machine.external_port().attachment_kind(),
            ExternalPortAttachmentKind::None
        );
        assert!(
            harness
                .execute_action(super::MenuAction::CycleGamepadDirectionalSource)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness
                .runtime
                .gamepad_manager
                .as_ref()
                .expect("gamepad manager")
                .directional_source(),
            GamepadDirectionalSource::DpadOnly
        );
        assert!(
            harness
                .execute_action(super::MenuAction::CycleGamepadRumbleMode)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness
                .runtime
                .gamepad_manager
                .as_ref()
                .expect("gamepad manager")
                .rumble_mode(),
            GamepadRumbleMode::Weak
        );
        assert_eq!(
            harness
                .settings_store
                .base_config()
                .input
                .gamepad
                .rumble_mode,
            GamepadRumbleMode::Weak
        );
        assert!(
            harness
                .execute_action(super::MenuAction::OpenRecentRom(99))
                .unwrap()
                .is_none()
        );
        harness
            .settings_store
            .remember_loaded_rom(&harness.root.join("Tetris DX.gb"))
            .expect("recent ROM should persist for clear-list coverage");
        harness.session.recent_roms = harness.settings_store.recent_roms().to_vec();
        harness
            .runtime
            .menu_state
            .open(super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                &harness.machine,
                &harness.session,
            ));
        assert!(
            harness
                .execute_action(super::MenuAction::ClearRecentList)
                .unwrap()
                .is_none()
        );
        assert!(harness.session.recent_roms().is_empty());
        assert!(harness.settings_store.recent_roms().is_empty());
        assert!(harness.runtime.menu_state.is_open());
        assert_eq!(
            super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                &harness.machine,
                &harness.session,
            )
            .recent_rom_count,
            0
        );
        assert!(
            harness
                .execute_action(super::MenuAction::SetKeyboardBinding(
                    KeyboardBindingTarget::A,
                    DesktopKey::Space,
                ))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.runtime.keyboard_bindings.joypad.a,
            DesktopKey::Space
        );
        assert!(
            harness
                .execute_action(super::MenuAction::SetKeyboardMenuBinding(
                    KeyboardMenuBindingTarget::Confirm,
                    DesktopKey::X,
                ))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.runtime.keyboard_bindings.menu.confirm,
            DesktopKey::X
        );
        assert!(
            harness
                .execute_action(super::MenuAction::SetGamepadBinding(
                    GamepadBindingTarget::A,
                    GamepadButtonBinding::South,
                ))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness
                .runtime
                .gamepad_manager
                .as_ref()
                .expect("gamepad manager")
                .button_bindings()
                .a,
            GamepadButtonBinding::South
        );
        assert!(
            harness
                .execute_action(super::MenuAction::SetGamepadMenuBinding(
                    GamepadMenuBindingTarget::Confirm,
                    GamepadButtonBinding::North,
                ))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness
                .runtime
                .gamepad_manager
                .as_ref()
                .expect("gamepad manager")
                .menu_bindings()
                .confirm,
            GamepadButtonBinding::North
        );
        assert!(
            harness
                .execute_action(super::MenuAction::ResetAudioDefaults)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.runtime.audio_channel_mask,
            ApuRecordedChannelMask::ALL
        );
        assert!(matches!(
            harness.runtime.audio_recording_mode,
            super::DesktopAudioRecordingMode::Disabled
        ));
        assert_eq!(harness.runtime.audio_volume_percent, 100);
        assert!(
            harness
                .execute_action(super::MenuAction::ResetVideoDefaults)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.runtime.video_options,
            gb_desktop::VideoOptions::default()
        );
        assert!(
            harness
                .execute_action(super::MenuAction::ResetInputDefaults)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            harness.runtime.keyboard_bindings,
            gb_desktop::InputOptions::default().keyboard
        );
        assert_eq!(
            harness
                .runtime
                .gamepad_manager
                .as_ref()
                .expect("gamepad manager")
                .rumble_mode(),
            GamepadRumbleMode::Strong
        );
        assert!(
            harness
                .execute_action(super::MenuAction::Reset)
                .unwrap()
                .is_none()
        );
        assert!(!harness.machine.cartridge().is_empty());
        assert!(matches!(
            harness.execute_action(super::MenuAction::Quit).unwrap(),
            Some(super::LoopSignal::Quit)
        ));

        let persisted = fs::read_to_string(&harness.settings_path)
            .expect("actions test should persist settings");
        assert!(persisted.contains("console_model = \"mgb\""));
        assert!(persisted.contains("startup_mode = \"real-boot\""));
    }
}
