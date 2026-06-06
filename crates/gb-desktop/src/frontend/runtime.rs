use crate::audio::{AudioSubmitTelemetry, DesktopAudioOutput};
use crate::{player_slots, screenshot_output};
#[cfg(test)]
use crate::linked_session;
use crate::audio_recording::{
    DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ, DesktopAudioRecorder, DesktopAudioRecordingOptions,
    resolve_next_audio_recording_output_path,
};
use crate::bootrom::{load_boot_rom_assets, missing_boot_rom_asset, resolve_path};
use crate::cli::{CliAction, DesktopRunOptions, help_text, parse_cli_arguments_with_base_config};
use gb_benchmark::{
    BenchmarkCase, BenchmarkMode, BenchmarkModel, BenchmarkPalette, BenchmarkStartup,
    BenchmarkStats, BenchmarkStimulusRuntime, GB_DESKTOP_FRONTEND, encode_stats_toml,
    frontend_screenshot_path, frontend_stats_path, load_benchmark_cases,
    target_frames_for_duration, target_tcycles_for_duration,
};
use gb_core::{
    ApuCh4DebugSnapshot, ApuCh4Nr43LiveWriteTrace, ApuCh4Nr43PassTrace, ApuRecordedChannel,
    ApuRecordedChannelMask, ApuRegisterWriteObservation, ApuRegisterWriteState, ApuSnapshot,
    CartridgeDiagnostic, CartridgeDiagnosticSeverity, CartridgeMappedRomSource,
    CartridgeMappedRomWindow, CgbInfraredStatus, CgbSpeedMode, CpuAddressEvent,
    CpuAddressEventKind, CpuAddressUpdateDirection, CpuBusAccessKind, CpuBusActivitySnapshot,
    CpuExecutionState, CpuRegisters, CpuSnapshot,
    DEFAULT_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES, DMG_T_CYCLES_PER_FRAME,
    DMG_T_CYCLES_PER_SECOND, DebugWramAddressSample, ExecutionMode, HardwareRevision,
    InterruptControllerSnapshot, JoypadButton, JoypadSnapshot,
    MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES, MIN_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES,
    Machine, MachineConfig, MachineRewindBuffer, MachineRewindFrameBoundaryTracker,
    MachineStepObserver, MachineStepRegion, PersistentCartState, PocketCameraFrame,
    PokemonMysteryGiftCode, PokemonMysteryGiftKind, PokemonPikachuColorGift,
    PokemonPikachuColorRegion, PpuAccessMode, PpuFramebufferLayerSource, PpuSnapshot,
    PpuStepRegion, SGB_FRAME_HEIGHT, SGB_FRAME_WIDTH, SerialTickTelemetry, SgbClockRate,
    SgbVideoStandard, StartupMode, TraceSummaryBuffer,
};
use gb_desktop::{
    BootRomVerificationMode, DesktopConfig, DesktopConsoleModel, DesktopDisplayPalette,
    DesktopExternalPortSelection, DesktopFrameBlendingMode, DesktopKey, DesktopSaveFlushPolicy,
    FAST_FORWARD_SPEED_MULTIPLIER_OPTIONS, FastForwardOptions, GamepadActionBindings,
    GamepadButtonBinding, GamepadButtonBindings, GamepadDirectionalSource, GamepadGyroMode,
    GamepadMenuBindings, GamepadRumbleMode, HotkeyBindings, JoypadKeyboardBindings,
    KeyboardBindings, MenuKeyboardBindings, PreferredGamepadIdentity, RewindOptions,
    SaveDirectoryPolicy, VideoOptions,
};
use gb_persistence::{
    CartridgeSaveFileExtension, CartridgeSaveKey, CartridgeSaveTimeSource,
    EXTERNAL_SAVE_FILE_EXTENSION, ExternalSaveError, ExternalSaveExportFormat,
    FilesystemCartridgeSaveStore, FixedCartridgeSaveTimeSource, MACHINE_SAVE_STATE_FILE_EXTENSION,
    MachineSaveStateEnvelope, SystemCartridgeSaveTimeSource, decode_machine_save_state_envelope,
    encode_external_cartridge_save, encode_machine_save_state_envelope,
    import_external_cartridge_save, uses_battery_backed_hardware_persistence,
};
use crate::input::{
    FrontendInputState, FrontendJoypadTarget, GamepadManager, gamepad_button_binding_from_sdl_axis,
    gamepad_button_binding_from_sdl_button, gamepad_trigger_axis_is_pressed,
    gamepad_trigger_axis_next_pressed,
};
use crate::linked_session::DesktopEmulationSession;
use crate::menu::{
    CgbInfraredHudSnapshot, CgbInfraredParticipantHudSnapshot, CompactMenuLabel,
    CompactRecentRomLabel, GamepadActionBindingTarget, GamepadBindingTarget,
    GamepadMenuBindingTarget, KeyboardBindingTarget, KeyboardMenuBindingTarget, MenuAction,
    MenuInput, MenuPresentation, OverlayMenuState, PerformanceHudSnapshot,
    RECENT_ROM_MENU_CAPACITY, RewindHudSnapshot, render_cgb_ir_indicator,
    render_fast_forward_indicator, render_performance_hud, render_rewind_indicator,
};
use crate::player_slots::{
    DesktopDmg07PlayerCount, DesktopPlayerSessionKind, PLAYER_SLOT_COUNT, PlayerInputStates,
    PlayerKeyboardProfile, PlayerSlot, audio_source_slot, host_policy_for_slot,
    linked_dmg04_p2_button_for_scancode, linked_dmg07_p3_button_for_scancode,
    linked_dmg07_p4_button_for_scancode, view_layout_for_session,
};
use png::{ColorType, Decoder, Transformations};
use crate::pocket_camera_live::PocketCameraLiveInput;
use crate::printer_output::PrinterOutputState;
use crate::save_session::DesktopSaveSession;
use sdl3::dialog::{
    DialogError, DialogFileFilter, show_open_file_dialog, show_open_folder_dialog,
    show_save_file_dialog,
};
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button};
use sdl3::hint;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::messagebox::{MessageBoxFlag, show_simple_message_box};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::{Canvas, ScaleMode, TextureCreator};
use sdl3::sys;
use sdl3::video::{FullscreenType, Window, WindowContext};
use crate::settings::DesktopSettingsStore;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsStr;
use std::fmt::Display;
use std::fs;
use std::io::{BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
#[cfg(test)]
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

pub(crate) const FRAMEBUFFER_WIDTH: u32 = 160;
pub(crate) const FRAMEBUFFER_HEIGHT: u32 = 144;
pub(crate) const SGB_HOST_FRAMEBUFFER_WIDTH: u32 = SGB_FRAME_WIDTH as u32;
pub(crate) const SGB_HOST_FRAMEBUFFER_HEIGHT: u32 = SGB_FRAME_HEIGHT as u32;
#[cfg(test)]
const FRAMEBUFFER_PITCH_BYTES: usize = FRAMEBUFFER_WIDTH as usize * 3;
const AUDIO_QUEUE_TARGET_MS: f64 = 96.0;
const AUDIO_QUEUE_DEADBAND_MS: f64 = 24.0;
const AUDIO_QUEUE_PACING_GAIN: f64 = 0.10;
const AUDIO_QUEUE_MAX_CORRECTION_MS: f64 = 4.0;
const PERFORMANCE_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const EXPECTED_SCANLINE_T_CYCLES: usize = 456;
const INPUT_POLL_SLICE_T_CYCLES: usize = 256;
const DEFAULT_TRACE_CAPTURE_T_CYCLES: usize = 8_192;
const DEFAULT_WATCH_TRACE_EVENTS: usize = 4_096;
const DEFAULT_PC_WATCH_TRACE_EVENTS: usize = 4_096;
const DEFAULT_EDGE_TRACE_EVENTS: usize = 4_096;
const DEFAULT_CGB_IR_TRACE_EVENTS: usize = 16_384;
const MACHINE_STATE_SLOT_COUNT: u8 = 4;
const DEFAULT_MACHINE_STATE_SLOT: u8 = 1;
const REWIND_HISTORY_SECONDS_OPTIONS: [u16; 5] = [5, 10, 20, 30, 60];
const REWIND_SUBFRAMES_PER_FRAME_OPTIONS: [u8; 4] = [0, 1, 2, 4];
const REWIND_SPEED_MULTIPLIER_OPTIONS: [u8; 3] = [1, 2, 4];
const REWIND_MAX_MEMORY_MIB_OPTIONS: [u16; 4] = [64, 128, 256, 512];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FramebufferDimensions {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct FramebufferPanelInput<'a> {
    pub(crate) dimensions: FramebufferDimensions,
    pub(crate) framebuffer: &'a [u8],
    pub(crate) framebuffer_layer_sources: &'a [PpuFramebufferLayerSource],
    pub(crate) bgwin_framebuffer: &'a [u8],
    pub(crate) backdrop_framebuffer: &'a [u8],
    pub(crate) bgwin_framebuffer_layer_sources: &'a [PpuFramebufferLayerSource],
    pub(crate) display_palette: DisplayPalette,
    pub(crate) cgb_framebuffer_rgb555: Option<&'a [u16]>,
    pub(crate) sgb_framebuffer_rgb555: Option<Vec<u16>>,
}

#[derive(Debug, Clone)]
pub(crate) struct FramebufferRenderInput<'a> {
    pub(crate) dimensions: FramebufferDimensions,
    pub(crate) panels: [Option<FramebufferPanelInput<'a>>; PLAYER_SLOT_COUNT],
}

#[derive(Debug, Clone, Copy)]
struct FramebufferPresentationSource<'a> {
    machine: &'a DesktopEmulationSession,
    video_options: &'a VideoOptions,
    session_has_loaded_rom: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct RenderHudInput {
    performance: Option<PerformanceHudSnapshot>,
    cgb_ir: Option<CgbInfraredHudSnapshot>,
    rewind_indicator: bool,
    fast_forward_indicator: bool,
}

#[derive(Default)]
struct RenderPresentationInput<'a> {
    frame_blending_state: Option<&'a mut FrameBlendingState>,
    menu_state: Option<(&'a OverlayMenuState, MenuPresentation)>,
    hud: RenderHudInput,
}

#[derive(Debug, Default)]
struct FrameBlendingState {
    mode: DesktopFrameBlendingMode,
    dimensions: Option<FramebufferDimensions>,
    previous_rgb_frame: Vec<u8>,
    current_rgb_frame: Vec<u8>,
    has_previous_frame: bool,
}

impl FrameBlendingState {
    fn reset(&mut self) {
        self.mode = DesktopFrameBlendingMode::Off;
        self.dimensions = None;
        self.previous_rgb_frame.clear();
        self.current_rgb_frame.clear();
        self.has_previous_frame = false;
    }

    fn apply(
        &mut self,
        rgb_frame: &mut [u8],
        dimensions: FramebufferDimensions,
        mode: DesktopFrameBlendingMode,
    ) {
        if mode == DesktopFrameBlendingMode::Off {
            if self.mode != DesktopFrameBlendingMode::Off
                || self.dimensions.is_some()
                || self.has_previous_frame
            {
                self.reset();
            }
            return;
        }

        if self.mode != mode
            || self.dimensions != Some(dimensions)
            || self.previous_rgb_frame.len() != rgb_frame.len()
        {
            self.mode = mode;
            self.dimensions = Some(dimensions);
            self.previous_rgb_frame.resize(rgb_frame.len(), 0);
            self.current_rgb_frame.clear();
            self.has_previous_frame = false;
        }

        if !self.has_previous_frame {
            self.previous_rgb_frame.copy_from_slice(rgb_frame);
            self.has_previous_frame = true;
            return;
        }

        self.current_rgb_frame.clear();
        self.current_rgb_frame.extend_from_slice(rgb_frame);
        blend_rgb24_frames(
            rgb_frame,
            &self.current_rgb_frame,
            &self.previous_rgb_frame,
            dimensions,
            mode,
        );
        self.previous_rgb_frame
            .copy_from_slice(&self.current_rgb_frame);
    }
}

const DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES: u32 = 15;
pub(crate) const DMG_GRAYSCALE_SHADES: [u8; 4] = [255, 170, 85, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DisplayPalette {
    shades: [[u8; 3]; 4],
}

impl DisplayPalette {
    pub(crate) const fn shade_rgb(self, shade: u8) -> [u8; 3] {
        match shade {
            0..=3 => self.shades[shade as usize],
            _ => self.shades[3],
        }
    }
}

pub(crate) const DMG_DISPLAY_PALETTE: DisplayPalette = DisplayPalette {
    shades: [
        [0xC6, 0xDE, 0x8C],
        [0x84, 0xA5, 0x63],
        [0x39, 0x61, 0x39],
        [0x08, 0x18, 0x10],
    ],
};
const MGB_DISPLAY_PALETTE: DisplayPalette = DisplayPalette {
    shades: [
        [0xC2, 0xCE, 0x93],
        [0x81, 0x8D, 0x66],
        [0x3A, 0x4C, 0x3A],
        [0x07, 0x10, 0x0E],
    ],
};
const GBL_DISPLAY_PALETTE: DisplayPalette = DisplayPalette {
    shades: [
        [0x7F, 0xE2, 0xC3],
        [0x56, 0xB4, 0x95],
        [0x35, 0x78, 0x62],
        [0x0A, 0x1C, 0x15],
    ],
};
const DMG_GREY_DISPLAY_PALETTE: DisplayPalette = DisplayPalette {
    shades: [
        [DMG_GRAYSCALE_SHADES[0]; 3],
        [DMG_GRAYSCALE_SHADES[1]; 3],
        [DMG_GRAYSCALE_SHADES[2]; 3],
        [DMG_GRAYSCALE_SHADES[3]; 3],
    ],
};
const DESKTOP_AUDIO_DISABLE_PACING_CORRECTION_ENV_VAR: &str =
    "GB_CYCLE_DESKTOP_AUDIO_DISABLE_PACING_CORRECTION";
const DESKTOP_EMU_PROFILE_ENV_VAR: &str = "GB_CYCLE_DESKTOP_EMU_PROFILE";
const DESKTOP_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_TRACE_PATH";
const DESKTOP_TRACE_T_CYCLES_ENV_VAR: &str = "GB_CYCLE_DESKTOP_TRACE_T_CYCLES";
const DESKTOP_WATCH_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_WATCH_TRACE_PATH";
const DESKTOP_WATCH_TRACE_ADDRESSES_ENV_VAR: &str = "GB_CYCLE_DESKTOP_WATCH_TRACE_ADDRESSES";
const DESKTOP_WATCH_TRACE_EVENTS_ENV_VAR: &str = "GB_CYCLE_DESKTOP_WATCH_TRACE_EVENTS";
const DESKTOP_PC_WATCH_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_PC_WATCH_TRACE_PATH";
const DESKTOP_PC_WATCH_TRACE_RANGES_ENV_VAR: &str = "GB_CYCLE_DESKTOP_PC_WATCH_TRACE_RANGES";
const DESKTOP_PC_WATCH_TRACE_EVENTS_ENV_VAR: &str = "GB_CYCLE_DESKTOP_PC_WATCH_TRACE_EVENTS";
const DESKTOP_EDGE_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_EDGE_TRACE_PATH";
const DESKTOP_EDGE_TRACE_ADDRESSES_ENV_VAR: &str = "GB_CYCLE_DESKTOP_EDGE_TRACE_ADDRESSES";
const DESKTOP_EDGE_TRACE_PC_RANGES_ENV_VAR: &str = "GB_CYCLE_DESKTOP_EDGE_TRACE_PC_RANGES";
const DESKTOP_EDGE_TRACE_EVENTS_ENV_VAR: &str = "GB_CYCLE_DESKTOP_EDGE_TRACE_EVENTS";
const DESKTOP_CGB_IR_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_CGB_IR_TRACE_PATH";
const DESKTOP_CGB_IR_TRACE_WATCH_ADDRESSES_ENV_VAR: &str =
    "GB_CYCLE_DESKTOP_CGB_IR_TRACE_WATCH_ADDRESSES";
const DESKTOP_CGB_IR_TRACE_TRIGGER_ADDRESSES_ENV_VAR: &str =
    "GB_CYCLE_DESKTOP_CGB_IR_TRACE_TRIGGER_ADDRESSES";
const DESKTOP_CGB_IR_TRACE_EVENTS_ENV_VAR: &str = "GB_CYCLE_DESKTOP_CGB_IR_TRACE_EVENTS";
const DESKTOP_CGB_IR_OPTICAL_DELAY_T_CYCLES_ENV_VAR: &str =
    "GB_CYCLE_DESKTOP_CGB_IR_OPTICAL_DELAY_T_CYCLES";
const DESKTOP_CH4_NR43_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_CH4_NR43_TRACE_PATH";
const DESKTOP_CH4_STARTUP_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_CH4_STARTUP_TRACE_PATH";
const DESKTOP_CPU_WINDOW_TRACE_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_CPU_WINDOW_TRACE_PATH";
const CPU_WINDOW_TRACE_START_PC: u16 = 0x4136;
const CPU_WINDOW_TRACE_END_PC: u16 = 0x7A8F;
const CGB_RP_ADDRESS: u16 = 0xFF56;
const MASTER_NR52_ADDRESS: u16 = 0xFF26;
const CH4_NR42_ADDRESS: u16 = 0xFF21;
const CH4_NR43_ADDRESS: u16 = 0xFF22;
const CH4_NR44_ADDRESS: u16 = 0xFF23;
const ROM_FILE_DIALOG_FILTERS: [DialogFileFilter<'static>; 1] = [DialogFileFilter {
    name: "Game Boy ROMs",
    pattern: "gb;gbc",
}];
const CAMERA_IMAGE_FILE_DIALOG_FILTERS: [DialogFileFilter<'static>; 2] = [
    DialogFileFilter {
        name: "PNG images",
        pattern: "png",
    },
    DialogFileFilter {
        name: "All files",
        pattern: "*",
    },
];
const EXTERNAL_SAVE_FILE_DIALOG_FILTERS: [DialogFileFilter<'static>; 2] = [
    DialogFileFilter {
        name: "Game Boy saves",
        pattern: "sav;sa1;sa2;sa3;sa4",
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
pub(crate) fn lock_sdl_test() -> std::sync::MutexGuard<'static, ()> {
    match sdl_test_lock().lock() {
        Ok(guard) => guard,
        Err(poison) => poison.into_inner(),
    }
}

#[cfg(test)]
pub(crate) fn configure_headless_sdl() {
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
    LinkedCgbInfraredTwoPlayer,
    LinkedDmg07,
}

impl EmulationProfileSessionKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::LinkedDmg04TwoPlayer => "linked-dmg04-2p",
            Self::LinkedCgbInfraredTwoPlayer => "linked-cgb-ir-2p",
            Self::LinkedDmg07 => "linked-dmg07",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HotkeyAction {
    None,
    ManualSave,
    SaveState,
    LoadState,
    SelectStateSlot(u8),
    Reset,
    Rewind,
    FastForward,
    ToggleFullscreen,
    TogglePerformanceHud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GamepadActionEvent {
    action: HotkeyAction,
    pressed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct GamepadTriggerState {
    left: bool,
    right: bool,
}

impl GamepadTriggerState {
    fn pressed_mut(&mut self, binding: GamepadButtonBinding) -> Option<&mut bool> {
        match binding {
            GamepadButtonBinding::LeftTrigger => Some(&mut self.left),
            GamepadButtonBinding::RightTrigger => Some(&mut self.right),
            GamepadButtonBinding::South
            | GamepadButtonBinding::East
            | GamepadButtonBinding::West
            | GamepadButtonBinding::North
            | GamepadButtonBinding::Back
            | GamepadButtonBinding::Start
            | GamepadButtonBinding::Guide
            | GamepadButtonBinding::LeftShoulder
            | GamepadButtonBinding::RightShoulder
            | GamepadButtonBinding::LeftStickClick
            | GamepadButtonBinding::RightStickClick
            | GamepadButtonBinding::DPadUp
            | GamepadButtonBinding::DPadDown
            | GamepadButtonBinding::DPadLeft
            | GamepadButtonBinding::DPadRight
            | GamepadButtonBinding::Misc1 => None,
        }
    }
}

struct FrontendRuntime {
    paused: bool,
    menu_state: OverlayMenuState,
    player_inputs: PlayerInputStates,
    keyboard_bindings: KeyboardBindings,
    video_options: VideoOptions,
    frame_blending_state: FrameBlendingState,
    audio_volume_percent: u8,
    audio_channel_mask: ApuRecordedChannelMask,
    audio_output: Option<DesktopAudioOutput>,
    audio_recording_mode: DesktopAudioRecordingMode,
    audio_recorder: Option<DesktopAudioRecorder>,
    gamepad_manager: Option<GamepadManager>,
    save_sessions: [Option<DesktopSaveSession>; PLAYER_SLOT_COUNT],
    machine_state_slot: u8,
    rewind_buffer: MachineRewindBuffer,
    rewind_frame_tracker: MachineRewindFrameBoundaryTracker,
    rewind_hotkey_active: bool,
    rewind_gamepad_active: bool,
    fast_forward_hotkey_active: bool,
    fast_forward_gamepad_active: bool,
    gamepad_trigger_state: GamepadTriggerState,
    fast_forward_audio_suppressed: bool,
    fast_forward_vsync_suppressed: bool,
    rtc_sync: HostRtcSync,
    open_rom_dialog: PathSelectionDialog,
    open_rom_dialog_mode: OpenRomDialogMode,
    camera_image_dialog: PathSelectionDialog,
    pocket_camera_live: PocketCameraLiveInput,
    boot_rom_directory_dialog: PathSelectionDialog,
    save_directory_dialog: PathSelectionDialog,
    external_save_export_dialog: PathSelectionDialog,
    external_save_import_dialog: PathSelectionDialog,
    trace_capture: DesktopTraceCapture,
    watch_trace: DesktopWatchTraceCapture,
    pc_watch_trace: DesktopPcWatchTraceCapture,
    edge_trace: DesktopEdgeTraceCapture,
    cgb_ir_trace: DesktopCgbIrTraceCapture,
    ch4_nr43_trace: DesktopCh4Nr43TraceCapture,
    ch4_startup_trace: DesktopCh4StartupTraceCapture,
    cpu_window_trace: DesktopCpuWindowTraceCapture,
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
    test_runner: bool,
    benchmark: Option<DesktopBenchmarkRun>,
    current_dir: PathBuf,
    loaded_rom: Option<LoadedRom>,
    linked_secondary_rom: Option<LoadedRom>,
    dmg07_player_count: Option<DesktopDmg07PlayerCount>,
    cgb_infrared_link_active: bool,
    pokemon_pikachu_color_active: bool,
    pokemon_pikachu_color_gift: PokemonPikachuColorGift,
    pokemon_mystery_gift_active: bool,
    pokemon_mystery_gift_kind: PokemonMysteryGiftKind,
    pokemon_mystery_gift_code: PokemonMysteryGiftCode,
    last_open_directory: Option<PathBuf>,
    recent_roms: Vec<PathBuf>,
    pocket_camera_frame: Option<PocketCameraFrame>,
    external_port_selection: DesktopExternalPortSelection,
}

#[derive(Clone)]
struct DesktopBenchmarkRun {
    case: BenchmarkCase,
    stimuli: BenchmarkStimulusRuntime,
    started_at: Instant,
    started_t_cycle: u64,
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

    fn cgb_infrared_link_active(&self) -> bool {
        self.cgb_infrared_link_active
    }

    fn pokemon_pikachu_color_active(&self) -> bool {
        self.pokemon_pikachu_color_active
    }

    fn pokemon_mystery_gift_active(&self) -> bool {
        self.pokemon_mystery_gift_active
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

fn cgb_infrared_same_game_active(session: &DesktopSession) -> bool {
    if !session.cgb_infrared_link_active {
        return false;
    }

    match (session.rom_path(), session.linked_secondary_rom_path()) {
        (Some(primary_path), Some(secondary_path)) => primary_path == secondary_path,
        _ => false,
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

struct DesktopWatchTraceCapture {
    output_path: Option<PathBuf>,
    watched_addresses: BTreeSet<u16>,
    max_records: usize,
    records: VecDeque<DesktopWatchTraceRecord>,
}

struct DesktopPcWatchTraceCapture {
    output_path: Option<PathBuf>,
    watched_ranges: Vec<PcWatchRange>,
    max_records: usize,
    records: VecDeque<DesktopPcWatchTraceRecord>,
}

struct DesktopEdgeTraceCapture {
    output_path: Option<PathBuf>,
    watched_addresses: BTreeSet<u16>,
    watched_pc_ranges: Vec<PcWatchRange>,
    active_pc_ranges: BTreeSet<PcWatchRange>,
    last_observed_values: BTreeMap<u16, u8>,
    max_records: usize,
    records: VecDeque<DesktopEdgeTraceRecord>,
}

struct DesktopCgbIrTraceCapture {
    output_path: Option<PathBuf>,
    watched_addresses: BTreeSet<u16>,
    watched_trigger_addresses: BTreeSet<u16>,
    max_records: usize,
    records: VecDeque<DesktopCgbIrTraceRecord>,
    last_p1_status: Option<CgbInfraredStatus>,
    last_p2_status: Option<CgbInfraredStatus>,
    last_p1_pressed_mask: Option<u8>,
    last_p2_pressed_mask: Option<u8>,
}

struct DesktopCh4Nr43TraceCapture {
    output_path: Option<PathBuf>,
    records: Vec<DesktopCh4Nr43TraceRecord>,
}

struct DesktopCh4StartupTraceCapture {
    output_path: Option<PathBuf>,
    records: Vec<DesktopCh4StartupTraceRecord>,
    last_ch4: Option<ApuCh4DebugSnapshot>,
}

struct DesktopCpuWindowTraceCapture {
    output_path: Option<PathBuf>,
    records: Vec<DesktopCpuWindowTraceRecord>,
    active: bool,
    finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesktopTraceServiceFlags {
    trace_capture: bool,
    watch_trace: bool,
    pc_watch_trace: bool,
    edge_trace: bool,
    cgb_ir_trace: bool,
    ch4_nr43_trace: bool,
    ch4_startup_trace: bool,
    cpu_window_trace: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesktopTcycleHostServices {
    capture_audio: bool,
    sync_gamepad_rumble: bool,
    record_rewind: bool,
    drain_printer: bool,
    traces: DesktopTraceServiceFlags,
}

#[derive(Debug, Clone)]
struct DesktopTraceRecord {
    t_cycle: u64,
    cpu: CpuSnapshot,
    apu: ApuSnapshot,
    interrupts: InterruptControllerSnapshot,
    joypad: JoypadSnapshot,
    cartridge_trace: String,
}

#[derive(Debug, Clone)]
struct DesktopWatchTraceRecord {
    t_cycle: u64,
    matched_addresses: Vec<u16>,
    cpu: CpuSnapshot,
    interrupts: InterruptControllerSnapshot,
    joypad: JoypadSnapshot,
    ppu_mode: PpuAccessMode,
    ppu_ly: u8,
    ppu_line_dot: u16,
    cartridge_trace: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PcWatchRange {
    start: u16,
    end: u16,
}

#[derive(Debug, Clone)]
struct DesktopPcWatchTraceRecord {
    t_cycle: u64,
    matched_ranges: Vec<PcWatchRange>,
    cpu: CpuSnapshot,
    interrupts: InterruptControllerSnapshot,
    joypad: JoypadSnapshot,
    ppu_mode: PpuAccessMode,
    ppu_ly: u8,
    ppu_line_dot: u16,
    cartridge_trace: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DesktopEdgeTraceTrigger {
    EnteredPcRange(PcWatchRange),
    AddressValueObserved {
        kind: CpuBusAccessKind,
        address: u16,
        previous: Option<u8>,
        current: u8,
    },
}

#[derive(Debug, Clone)]
struct DesktopEdgeTraceRecord {
    t_cycle: u64,
    current_pc_ranges: Vec<PcWatchRange>,
    triggers: Vec<DesktopEdgeTraceTrigger>,
    cpu: CpuSnapshot,
    interrupts: InterruptControllerSnapshot,
    joypad: JoypadSnapshot,
    ppu_mode: PpuAccessMode,
    ppu_ly: u8,
    ppu_line_dot: u16,
    cartridge_trace: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DesktopCgbIrTraceTrigger {
    StatusChanged {
        slot: PlayerSlot,
        previous: Option<CgbInfraredStatus>,
        current: CgbInfraredStatus,
    },
    RpBusActivity {
        slot: PlayerSlot,
        activity: CpuBusActivitySnapshot,
    },
    WatchedBusActivity {
        slot: PlayerSlot,
        activity: CpuBusActivitySnapshot,
    },
    JoypadPressedMaskChanged {
        slot: PlayerSlot,
        previous: Option<u8>,
        current: u8,
    },
}

#[derive(Debug, Clone)]
struct DesktopCgbIrTraceParticipantRecord {
    status: CgbInfraredStatus,
    cpu: CpuSnapshot,
    joypad: JoypadSnapshot,
    rom_window: Option<CartridgeMappedRomWindow>,
    watched_values: Vec<DesktopCgbIrTraceWatchedValue>,
}

#[derive(Debug, Clone)]
struct DesktopCgbIrTraceRecord {
    t_cycle: u64,
    triggers: Vec<DesktopCgbIrTraceTrigger>,
    p1: DesktopCgbIrTraceParticipantRecord,
    p2: DesktopCgbIrTraceParticipantRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesktopCgbIrTraceStatusKey {
    rp_latch: u8,
    emitter_on: bool,
    read_enabled: bool,
    external_optical_input: bool,
    optical_input_active: bool,
    sensor_warmed: bool,
    effective_signal_detected: bool,
    signal_visible_to_rp: bool,
    receive_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopCgbIrTraceWatchedValue {
    Wram(DebugWramAddressSample),
    Hram { address: u16, offset: u8, value: u8 },
    Unsupported { address: u16 },
}

#[derive(Debug, Clone)]
struct DesktopCh4Nr43TraceRecord {
    t_cycle: u64,
    cpu: CpuSnapshot,
    apu_write: ApuRegisterWriteObservation,
    ch4: ApuCh4DebugSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopCh4StartupTraceEventKind {
    RegisterWrite,
    DelayedStartFired,
}

#[derive(Debug, Clone)]
struct DesktopCh4StartupTraceRecord {
    event: DesktopCh4StartupTraceEventKind,
    t_cycle: u64,
    cpu: CpuSnapshot,
    apu_write: Option<ApuRegisterWriteObservation>,
    ch4: ApuCh4DebugSnapshot,
}

#[derive(Debug, Clone)]
struct DesktopCpuWindowTraceRecord {
    t_cycle: u64,
    cpu: CpuSnapshot,
    interrupts: InterruptControllerSnapshot,
    ppu: PpuSnapshot,
    ppu_ly_read: u8,
    ppu_stat_read: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum EmulationProfileDetail {
    #[default]
    Full,
    CoreOnly,
    Overhead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum EmulationProfileMode {
    #[default]
    Disabled,
    SampledSummary {
        sample_every_frames: u32,
        detail: EmulationProfileDetail,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct EmulationBreakdownSample {
    core_external_events_duration: Duration,
    core_timer_duration: Duration,
    core_apu_duration: Duration,
    core_dma_duration: Duration,
    core_ppu_duration: Duration,
    core_ppu_bus_sync_duration: Duration,
    core_ppu_bus_state_duration: Duration,
    core_ppu_bus_view_duration: Duration,
    core_ppu_bus_snapshot_duration: Duration,
    core_ppu_published_access_duration: Duration,
    core_ppu_tick_duration: Duration,
    core_ppu_misc_duration: Duration,
    core_ppu_mode_timing_duration: Duration,
    core_ppu_raster_advance_duration: Duration,
    core_ppu_raster_publication_duration: Duration,
    core_ppu_stat_irq_duration: Duration,
    core_ppu_visible_prep_duration: Duration,
    core_ppu_mode0_1_duration: Duration,
    core_ppu_mode2_duration: Duration,
    core_ppu_mode3_control_duration: Duration,
    core_ppu_mode3_startup_duration: Duration,
    core_ppu_bg_fetch_duration: Duration,
    core_ppu_bg_edge_duration: Duration,
    core_ppu_window_fetch_duration: Duration,
    core_ppu_window_edge_duration: Duration,
    core_ppu_push_duration: Duration,
    core_ppu_obj_edge_duration: Duration,
    core_ppu_obj_fetch_duration: Duration,
    core_ppu_pixel_transfer_duration: Duration,
    core_serial_duration: Duration,
    serial_active_t_cycles: u64,
    serial_internal_ticks: u64,
    serial_external_ticks: u64,
    serial_external_wait_ticks: u64,
    serial_shift_edges: u64,
    serial_completed_bytes: u64,
    serial_external_port_ticks: u64,
    core_cpu_duration: Duration,
    core_interrupts_duration: Duration,
    host_event_poll_duration: Duration,
    host_audio_submit_duration: Duration,
    host_save_flush_duration: Duration,
    profile_base_duration: Duration,
    profile_core_duration: Duration,
    profile_full_duration: Duration,
    profile_core_overhead_duration: Duration,
    profile_ppu_observer_overhead_duration: Duration,
}

#[derive(Debug)]
struct StepUntilNextFrameResult {
    signal: LoopSignal,
    emulation_profile_request: Option<EmulationProfileRequest>,
    frame_loop_telemetry: FrameLoopTelemetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FrameLoopTelemetry {
    speed_mode: Option<CgbSpeedMode>,
    start_ly: u8,
    start_dot: u16,
    end_ly: u8,
    end_dot: u16,
    stepped_t_cycles: usize,
    video_dots: usize,
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
    detail: EmulationProfileDetail,
    breakdown: EmulationBreakdownSample,
}

#[derive(Debug)]
struct EmulationProfileWorkItem {
    machine: DesktopEmulationSession,
    detail: EmulationProfileDetail,
    emulation_duration: Duration,
    breakdown: EmulationBreakdownSample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompletedEmulationProfileSample {
    emulation_duration: Duration,
    breakdown: EmulationBreakdownSample,
}

#[derive(Debug)]
struct ReplayFrameCoreProfiler {
    sample: EmulationBreakdownSample,
    records_ppu_regions: bool,
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
    CgbInfraredSecondary,
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
        let default_location = sdl_dialog_default_location(default_location);
        map_display_result(
            show_open_file_dialog(
                filters,
                Some(&default_location),
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

    fn show_save_file(
        &mut self,
        filters: &[DialogFileFilter<'static>],
        window: &Window,
        default_location: &Path,
    ) -> Result<(), String> {
        if self.pending {
            return Ok(());
        }

        let sender = self.sender.clone();
        let default_location = sdl_dialog_default_location(default_location);
        map_display_result(
            show_save_file_dialog(
                filters,
                Some(&default_location),
                window,
                Box::new(move |result, _| {
                    let _ = sender.send(map_path_dialog_result(result));
                }),
            ),
            "failed to show SDL3 save file dialog",
        )?;
        self.pending = true;
        Ok(())
    }

    fn show_folder(&mut self, window: &Window, default_location: &Path) {
        if self.pending {
            return;
        }

        let sender = self.sender.clone();
        let default_location = sdl_dialog_default_location(default_location);
        show_open_folder_dialog(
            Some(&default_location),
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

fn sdl_dialog_default_location(default_location: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if default_location.is_dir() {
            let mut directory = default_location.to_path_buf();
            directory.push("");
            return directory;
        }
    }

    default_location.to_path_buf()
}

impl FrontendRuntime {
    fn any_dialog_pending(&self) -> bool {
        self.open_rom_dialog.is_pending()
            || self.camera_image_dialog.is_pending()
            || self.boot_rom_directory_dialog.is_pending()
            || self.save_directory_dialog.is_pending()
            || self.external_save_export_dialog.is_pending()
            || self.external_save_import_dialog.is_pending()
    }
}

fn emulation_profile_session_kind(
    machine: &DesktopEmulationSession,
) -> EmulationProfileSessionKind {
    if machine.is_linked_dmg07() {
        EmulationProfileSessionKind::LinkedDmg07
    } else if machine.is_linked_cgb_infrared_two_player() {
        EmulationProfileSessionKind::LinkedCgbInfraredTwoPlayer
    } else if machine.is_linked_dmg04_two_player() {
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

fn should_exit_after_benchmark_tcycles(
    benchmark: Option<&DesktopBenchmarkRun>,
    machine: &DesktopEmulationSession,
) -> bool {
    benchmark.is_some_and(|benchmark| {
        machine
            .primary_machine()
            .next_t_cycle()
            .get()
            .saturating_sub(benchmark.started_t_cycle)
            >= target_tcycles_for_duration(benchmark.case.duration_seconds)
    })
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

fn gamepad_rumble_sync_needed(
    runtime: &FrontendRuntime,
    machine: &Machine<TraceSummaryBuffer>,
) -> bool {
    let Some(gamepad_manager) = runtime.gamepad_manager.as_ref() else {
        return false;
    };

    if machine.cartridge().has_rumble() {
        return gamepad_manager.can_deliver_rumble() || gamepad_manager.has_active_rumble_effect();
    }

    gamepad_manager.has_active_rumble_effect()
}

pub(crate) fn main() -> ExitCode {
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
            let persist_startup_fallback = !options.test_runner && options.config == base_config;
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
    settings_store: DesktopSettingsStore,
    persist_startup_fallback: bool,
) -> Result<(), String> {
    let current_dir =
        map_display_result(env::current_dir(), "failed to determine current directory")?;
    if options.benchmark_path.is_some() {
        return run_desktop_benchmark_suite(options, settings_store, current_dir);
    }

    run_desktop_prepared(
        options,
        settings_store,
        persist_startup_fallback,
        None,
        current_dir,
    )
}

fn run_desktop_prepared(
    options: DesktopRunOptions,
    mut settings_store: DesktopSettingsStore,
    persist_startup_fallback: bool,
    benchmark_case: Option<BenchmarkCase>,
    current_dir: PathBuf,
) -> Result<(), String> {
    let original_config = options.config.clone();
    let exit_after_frames = options.exit_after_frames;
    let test_runner = options.test_runner;
    let startup_links_peer = options.linked_peer_rom_path.is_some();
    let loaded_rom = load_initial_rom(&options, &current_dir)?;
    let linked_secondary_rom = load_initial_linked_secondary_rom(&options, &current_dir)?;
    let last_open_directory = match loaded_rom.as_ref() {
        Some(rom) => rom.path.parent().map(Path::to_path_buf),
        None => settings_store.last_open_directory().map(Path::to_path_buf),
    };
    let startup_external_port_selection =
        if startup_links_peer && options.config.launch.console_model.allows_ext_port_menu() {
            DesktopExternalPortSelection::GameLink
        } else {
            DesktopExternalPortSelection::None
        };
    let mut session = DesktopSession {
        config: options.config,
        test_runner,
        benchmark: benchmark_case.map(|case| DesktopBenchmarkRun {
            stimuli: BenchmarkStimulusRuntime::new(case.stimuli.clone()),
            case,
            started_at: Instant::now(),
            started_t_cycle: 0,
        }),
        current_dir,
        loaded_rom,
        linked_secondary_rom,
        dmg07_player_count: None,
        cgb_infrared_link_active: false,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: PokemonPikachuColorGift::default(),
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: PokemonMysteryGiftKind::default(),
        pokemon_mystery_gift_code: PokemonMysteryGiftCode::default(),
        last_open_directory,
        recent_roms: settings_store.recent_roms().to_vec(),
        pocket_camera_frame: None,
        external_port_selection: startup_external_port_selection,
    };

    let (mut machine, diagnostics) = load_initial_emulation_session(&mut session)?;
    if persist_startup_fallback && !session.test_runner && session.config != original_config {
        settings_store.persist_machine_preferences(&session.config)?;
    }
    write_cartridge_diagnostics(&diagnostics);
    if !session.test_runner
        && let Some(rom_path) = session.rom_path()
    {
        settings_store.remember_loaded_rom(rom_path)?;
        session.recent_roms = settings_store.recent_roms().to_vec();
    }
    let save_sessions = open_save_sessions_for_session(&session, &mut machine)?;

    if session.config.video.vsync {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "1", &hint::Hint::Default);
    } else {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "0", &hint::Hint::Default);
    }

    let sdl = map_display_result(sdl3::init(), "failed to initialize SDL3")?;
    let mut player_inputs = PlayerInputStates::new();
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
            player_inputs.input_mut(PlayerSlot::P1),
            machine
                .machine_for_player_slot_mut(PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        )?)
    } else {
        None
    };
    let video = map_display_result(sdl.video(), "failed to initialize SDL3 video subsystem")?;

    let framebuffer_dimensions = framebuffer_dimensions_for_session(
        &machine,
        &session.config.video,
        session.has_loaded_rom(),
    );
    let window_width = framebuffer_dimensions
        .width
        .checked_mul(u32::from(session.config.video.window_scale))
        .ok_or_else(|| overflow_error("window width overflowed"))?;
    let window_height = framebuffer_dimensions
        .height
        .checked_mul(u32::from(session.config.video.window_scale))
        .ok_or_else(|| overflow_error("window height overflowed"))?;

    let base_window_title = window_title(&session, &session.config);
    let mut frame_pacer = FramePacer::new(
        session.config.video.vsync,
        frame_duration_for_config(&session.config),
    );
    let mut performance_counter = if session.test_runner {
        PerformanceCounter::new_with_emulation_profile_mode(
            base_window_title.clone(),
            EmulationProfileMode::Disabled,
        )
    } else {
        PerformanceCounter::new(base_window_title.clone())
    };
    performance_counter.set_target_frame_rate_hz(target_frame_rate_hz_for_config(&session.config));
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
        player_inputs,
        keyboard_bindings: session.config.input.keyboard,
        video_options: session.config.video.clone(),
        frame_blending_state: FrameBlendingState::default(),
        audio_volume_percent: session.config.audio.volume_percent,
        audio_channel_mask,
        audio_output,
        audio_recording_mode,
        audio_recorder,
        gamepad_manager,
        save_sessions,
        machine_state_slot: DEFAULT_MACHINE_STATE_SLOT,
        rewind_buffer: MachineRewindBuffer::new(session.config.rewind.machine_rewind_config()),
        rewind_frame_tracker: MachineRewindFrameBoundaryTracker::new(),
        rewind_hotkey_active: false,
        rewind_gamepad_active: false,
        fast_forward_hotkey_active: false,
        fast_forward_gamepad_active: false,
        gamepad_trigger_state: GamepadTriggerState::default(),
        fast_forward_audio_suppressed: false,
        fast_forward_vsync_suppressed: false,
        rtc_sync: HostRtcSync::from_host_clock(),
        open_rom_dialog: PathSelectionDialog::new(),
        open_rom_dialog_mode: OpenRomDialogMode::Primary,
        camera_image_dialog: PathSelectionDialog::new(),
        pocket_camera_live: PocketCameraLiveInput::new(sdl.camera().map_err(|error| {
            format_display_error(
                "failed to initialize SDL3 camera subsystem",
                &error.to_string(),
            )
        })),
        boot_rom_directory_dialog: PathSelectionDialog::new(),
        save_directory_dialog: PathSelectionDialog::new(),
        external_save_export_dialog: PathSelectionDialog::new(),
        external_save_import_dialog: PathSelectionDialog::new(),
        trace_capture: DesktopTraceCapture::from_env()?,
        watch_trace: DesktopWatchTraceCapture::from_env()?,
        pc_watch_trace: DesktopPcWatchTraceCapture::from_env()?,
        edge_trace: DesktopEdgeTraceCapture::from_env()?,
        cgb_ir_trace: DesktopCgbIrTraceCapture::from_env()?,
        ch4_nr43_trace: DesktopCh4Nr43TraceCapture::from_env()?,
        ch4_startup_trace: DesktopCh4StartupTraceCapture::from_env()?,
        cpu_window_trace: DesktopCpuWindowTraceCapture::from_env(),
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
        FramebufferPresentationSource {
            machine: &machine,
            video_options: &runtime.video_options,
            session_has_loaded_rom: session.has_loaded_rom(),
        },
    )?;
    let _ = render_frame(
        &mut canvas,
        &mut texture,
        &mut rgb_frame,
        framebuffer_render_input_for_session(
            &machine,
            current_framebuffer_dimensions,
            &runtime.video_options,
            session.has_loaded_rom(),
        ),
        &runtime.video_options,
        RenderPresentationInput {
            frame_blending_state: Some(&mut runtime.frame_blending_state),
            menu_state: initial_menu_presentation,
            hud: RenderHudInput::default(),
        },
    )?;

    if let Some(benchmark) = &mut session.benchmark {
        benchmark.started_at = Instant::now();
        benchmark.started_t_cycle = machine.primary_machine().next_t_cycle().get();
    }

    'running: loop {
        if !session.test_runner {
            let mut context = FrontendActionContext {
                session: &mut session,
                machine: &mut machine,
                runtime: &mut runtime,
                performance_counter: &mut performance_counter,
                frame_pacer: &mut frame_pacer,
                settings_store: &mut settings_store,
            };
            process_pending_open_rom_dialog(&event_pump, &mut canvas, &mut context)?;
            process_pending_camera_image_dialog(&mut canvas, &mut context)?;
            process_pocket_camera_live_frame(&mut canvas, &mut context);
            process_pending_boot_rom_directory_dialog(&mut canvas, &mut context)?;
            process_pending_save_directory_dialog(&mut canvas, &mut context)?;
            process_pending_external_save_export_dialog(&mut canvas, &mut context)?;
            process_pending_external_save_import_dialog(&mut canvas, &mut context)?;
        }

        runtime.rtc_sync.sync_host_elapsed_to_machine(&mut machine);

        match {
            let mut context = FrontendActionContext {
                session: &mut session,
                machine: &mut machine,
                runtime: &mut runtime,
                performance_counter: &mut performance_counter,
                frame_pacer: &mut frame_pacer,
                settings_store: &mut settings_store,
            };
            if context.session.test_runner {
                process_test_runner_events(&mut event_pump, &mut context)
            } else {
                process_events(&mut event_pump, &mut canvas, &mut context)
            }
        }? {
            LoopSignal::Continue => {}
            LoopSignal::Quit => break 'running,
        }

        if emulation_paused(&machine, &runtime) {
            runtime.rtc_sync.apply_to_machine(&mut machine);
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
                    FramebufferPresentationSource {
                        machine: &machine,
                        video_options: &runtime.video_options,
                        session_has_loaded_rom: session.has_loaded_rom(),
                    },
                )?;
                let _ = render_frame(
                    &mut canvas,
                    &mut texture,
                    &mut rgb_frame,
                    framebuffer_render_input_for_session(
                        &machine,
                        current_framebuffer_dimensions,
                        &runtime.video_options,
                        session.has_loaded_rom(),
                    ),
                    &runtime.video_options,
                    RenderPresentationInput {
                        frame_blending_state: Some(&mut runtime.frame_blending_state),
                        menu_state: menu_presentation,
                        hud: RenderHudInput::default(),
                    },
                )?;
            }
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        let emulation_started_at = Instant::now();
        let mut rewound_this_frame = false;
        let mut fast_forwarded_this_frame = false;
        let step_result =
            if rewind_hold_active(&runtime) && rewind_actions_available(&session, &machine) {
                let rewind_steps =
                    rewind_restore_steps_for_speed(session.config.rewind.speed_multiplier);
                let rewind_result = {
                    let context = FrontendActionContext {
                        session: &mut session,
                        machine: &mut machine,
                        runtime: &mut runtime,
                        performance_counter: &mut performance_counter,
                        frame_pacer: &mut frame_pacer,
                        settings_store: &mut settings_store,
                    };
                    rewind_desktop_session_steps(
                        context.session,
                        context.machine,
                        context.runtime,
                        context.frame_pacer,
                        rewind_steps,
                    )
                };
                match rewind_result {
                    Ok(true) => {
                        rewound_this_frame = true;
                        sync_audio_playback_state(&machine, &runtime)?;
                        StepUntilNextFrameResult {
                            signal: LoopSignal::Continue,
                            emulation_profile_request: None,
                            frame_loop_telemetry: FrameLoopTelemetry::default(),
                        }
                    }
                    Ok(false) => StepUntilNextFrameResult {
                        signal: LoopSignal::Continue,
                        emulation_profile_request: None,
                        frame_loop_telemetry: FrameLoopTelemetry::default(),
                    },
                    Err(error) => {
                        show_warning_message(Some(canvas.window()), "Rewind", &error);
                        eprintln!("warning: {error}");
                        runtime.rewind_hotkey_active = false;
                        runtime.rewind_gamepad_active = false;
                        StepUntilNextFrameResult {
                            signal: LoopSignal::Continue,
                            emulation_profile_request: None,
                            frame_loop_telemetry: FrameLoopTelemetry::default(),
                        }
                    }
                }
            } else if fast_forward_active(&runtime, &session, &machine) {
                let mut context = FrontendActionContext {
                    session: &mut session,
                    machine: &mut machine,
                    runtime: &mut runtime,
                    performance_counter: &mut performance_counter,
                    frame_pacer: &mut frame_pacer,
                    settings_store: &mut settings_store,
                };
                let (step_result, fast_forwarded_frames) =
                    step_fast_forward_frames(&mut event_pump, &mut canvas, &mut context)?;
                fast_forwarded_this_frame = fast_forwarded_frames > 0;
                step_result
            } else {
                sync_fast_forward_audio_state(&mut runtime, false)?;
                let mut context = FrontendActionContext {
                    session: &mut session,
                    machine: &mut machine,
                    runtime: &mut runtime,
                    performance_counter: &mut performance_counter,
                    frame_pacer: &mut frame_pacer,
                    settings_store: &mut settings_store,
                };
                step_until_next_frame(&mut event_pump, &mut canvas, &mut context)?
            };
        match step_result.signal {
            LoopSignal::Continue => {}
            LoopSignal::Quit => break 'running,
        }
        let emulation_duration = emulation_started_at.elapsed();
        let fast_forward_still_active = fast_forward_active(&runtime, &session, &machine);
        sync_fast_forward_audio_state(&mut runtime, fast_forward_still_active)?;
        sync_fast_forward_host_pacing_state(
            &mut canvas,
            &mut frame_pacer,
            &mut runtime,
            fast_forward_still_active,
        )?;
        let audio_submit_telemetry = (!rewound_this_frame && !fast_forwarded_this_frame)
            .then(|| {
                runtime
                    .audio_output
                    .as_mut()
                    .and_then(DesktopAudioOutput::take_last_submit_telemetry)
            })
            .flatten();

        if emulation_paused(&machine, &runtime) {
            runtime.rtc_sync.apply_to_machine(&mut machine);
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
                    FramebufferPresentationSource {
                        machine: &machine,
                        video_options: &runtime.video_options,
                        session_has_loaded_rom: session.has_loaded_rom(),
                    },
                )?;
                let _ = render_frame(
                    &mut canvas,
                    &mut texture,
                    &mut rgb_frame,
                    framebuffer_render_input_for_session(
                        &machine,
                        current_framebuffer_dimensions,
                        &runtime.video_options,
                        session.has_loaded_rom(),
                    ),
                    &runtime.video_options,
                    RenderPresentationInput {
                        frame_blending_state: Some(&mut runtime.frame_blending_state),
                        menu_state: menu_presentation,
                        hud: RenderHudInput::default(),
                    },
                )?;
            }
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        let render_started_at = Instant::now();
        let render_hud = if session.test_runner {
            RenderHudInput::default()
        } else {
            let rewind_indicator =
                rewind_indicator_visible(&runtime, &session, &machine, rewound_this_frame);
            RenderHudInput {
                performance: current_performance_hud_snapshot(
                    performance_counter.hud_snapshot(),
                    &runtime,
                    &session,
                    &machine,
                ),
                cgb_ir: runtime
                    .video_options
                    .show_cgb_infrared_helper
                    .then(|| current_cgb_ir_hud_snapshot(&machine))
                    .flatten(),
                rewind_indicator,
                fast_forward_indicator: fast_forward_indicator_visible(
                    &runtime,
                    &session,
                    &machine,
                    fast_forwarded_this_frame,
                ) && !rewind_indicator,
            }
        };
        sync_framebuffer_presentation_resources(
            &mut canvas,
            &texture_creator,
            &mut texture,
            &mut rgb_frame,
            &mut current_framebuffer_dimensions,
            FramebufferPresentationSource {
                machine: &machine,
                video_options: &runtime.video_options,
                session_has_loaded_rom: session.has_loaded_rom(),
            },
        )?;
        let present_duration = render_frame(
            &mut canvas,
            &mut texture,
            &mut rgb_frame,
            framebuffer_render_input_for_session(
                &machine,
                current_framebuffer_dimensions,
                &runtime.video_options,
                session.has_loaded_rom(),
            ),
            &runtime.video_options,
            RenderPresentationInput {
                frame_blending_state: Some(&mut runtime.frame_blending_state),
                menu_state: None,
                hud: render_hud,
            },
        )?;
        let render_duration = render_started_at.elapsed();
        let audio_queue_ms_before_pacing = runtime
            .audio_output
            .as_ref()
            .and_then(DesktopAudioOutput::queued_duration_ms);
        frame_pacer.set_frame_duration(frame_duration_for_config(&session.config));
        performance_counter
            .set_target_frame_rate_hz(target_frame_rate_hz_for_config(&session.config));
        let pacing = if should_skip_host_frame_pacing(
            session.test_runner,
            fast_forward_still_active,
            fast_forwarded_this_frame,
        ) {
            frame_pacer.reset_host_pacing();
            FramePacingSample::default()
        } else {
            frame_pacer.wait_until_next_frame(audio_queue_ms_before_pacing)
        };
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
        if session.test_runner {
            performance_counter.presented_frames_total =
                performance_counter.presented_frames_total.saturating_add(1);
        } else {
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
                    speed_mode: frame_loop_telemetry.speed_mode,
                    frame_step_t_cycles: Some(frame_loop_telemetry.stepped_t_cycles),
                    frame_video_dots: Some(frame_loop_telemetry.video_dots),
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
                    ly_0_self_wrap_blank_frame: Some(
                        frame_loop_telemetry.ly_0_self_wrap_blank_frame,
                    ),
                    ly_0_to_1_transitions: Some(frame_loop_telemetry.ly_0_to_1_transitions),
                    ly_0_scanline_t_cycles: Some(frame_loop_telemetry.ly_0_scanline_t_cycles),
                    ly_0_max_mode0_start_dot: Some(frame_loop_telemetry.ly_0_max_mode0_start_dot),
                    ly_0_stall_t_cycles: Some(frame_loop_telemetry.ly_0_stall_t_cycles),
                    ly_0_stall_hblank_t_cycles: Some(
                        frame_loop_telemetry.ly_0_stall_hblank_t_cycles,
                    ),
                    ly_0_stall_oam_t_cycles: Some(frame_loop_telemetry.ly_0_stall_oam_t_cycles),
                    ly_0_stall_drawing_t_cycles: Some(
                        frame_loop_telemetry.ly_0_stall_drawing_t_cycles,
                    ),
                    ly_0_stall_startup_mode0_t_cycles: Some(
                        frame_loop_telemetry.ly_0_stall_startup_mode0_t_cycles,
                    ),
                    ly_0_stall_blank_frame_t_cycles: Some(
                        frame_loop_telemetry.ly_0_stall_blank_frame_t_cycles,
                    ),
                    ly_0_stall_runs: Some(frame_loop_telemetry.ly_0_stall_runs),
                    ly_0_max_stall_run_t_cycles: Some(
                        frame_loop_telemetry.ly_0_max_stall_run_t_cycles,
                    ),
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
                    ly_0_lcd_disabled_t_cycles: Some(
                        frame_loop_telemetry.ly_0_lcd_disabled_t_cycles,
                    ),
                    ly_0_stall_lcd_disabled_t_cycles: Some(
                        frame_loop_telemetry.ly_0_stall_lcd_disabled_t_cycles,
                    ),
                },
            )?;
        }
        if should_exit_after_presented_frames(
            exit_after_frames,
            performance_counter.presented_frames_total,
        ) || should_exit_after_benchmark_tcycles(session.benchmark.as_ref(), &machine)
        {
            break 'running;
        }
    }

    if !session.test_runner {
        settings_store.set_fullscreen(canvas.window().fullscreen_state() != FullscreenType::Off)?;
    }
    flush_pending_printer_output(canvas.window(), &session, &mut runtime);
    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager.update_rumble(false, Instant::now())?;
    }
    write_benchmark_artifacts_for_session(
        &session,
        &machine,
        &runtime.video_options,
        &performance_counter,
    )?;

    close_runtime_save_sessions(&mut runtime, &machine)?;
    if !session.test_runner
        && let Some(rom_path) = session.rom_path()
    {
        settings_store.remember_loaded_rom(rom_path)?;
    }
    if let Some(audio_output) = &runtime.audio_output {
        audio_output.flush()?;
    }
    if let Some(audio_recorder) = &mut runtime.audio_recorder {
        audio_recorder.finish()?;
    }
    runtime.trace_capture.write_artifact()?;
    runtime.watch_trace.write_artifact()?;
    runtime.pc_watch_trace.write_artifact()?;
    runtime.edge_trace.write_artifact()?;
    runtime.cgb_ir_trace.write_artifact()?;
    runtime.ch4_nr43_trace.write_artifact()?;
    runtime.ch4_startup_trace.write_artifact()?;
    runtime.cpu_window_trace.write_artifact()?;

    Ok(())
}
