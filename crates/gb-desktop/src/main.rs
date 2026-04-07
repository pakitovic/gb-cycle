mod audio;
mod bootrom;
mod cli;
mod input;
mod menu;
mod save_session;
mod settings;

use audio::DesktopAudioOutput;
use bootrom::{load_boot_rom_assets, missing_boot_rom_asset_path, resolve_path};
use cli::{CliAction, DesktopRunOptions, help_text, parse_cli_arguments_with_base_config};
use gb_core::{
    CartridgeDiagnostic, CartridgeDiagnosticSeverity, ExecutionMode, JoypadButton, Machine,
    MachineConfig, StartupMode, TraceSummaryBuffer,
};
use gb_desktop::{
    BootRomVerificationMode, DEFAULT_BOOT_ROM_DIR, DesktopConfig, DesktopConsoleModel, DesktopKey,
    DesktopSaveFlushPolicy, GamepadButtonBinding, GamepadButtonBindings, GamepadDirectionalSource,
    GamepadMenuBindings, GamepadRumbleMode, HotkeyBindings, JoypadKeyboardBindings,
    KeyboardBindings, MenuKeyboardBindings, PreferredGamepadIdentity, SaveDirectoryPolicy,
    VideoOptions,
};
use gb_persistence::{
    CartridgeSaveTimeSource, SystemCartridgeSaveTimeSource,
    uses_battery_backed_hardware_persistence,
};
use input::{
    FrontendInputState, GamepadManager, gamepad_button_binding_from_sdl_button,
    sdl_button_for_binding,
};
use menu::{
    CompactMenuLabel, CompactRecentRomLabel, GamepadBindingTarget, GamepadMenuBindingTarget,
    KeyboardBindingTarget, KeyboardMenuBindingTarget, MenuAction, MenuInput, MenuPresentation,
    OverlayMenuState, PerformanceHudSnapshot, RECENT_ROM_MENU_CAPACITY, render_performance_hud,
};
use save_session::DesktopSaveSession;
use sdl3::dialog::{DialogError, DialogFileFilter, show_open_file_dialog, show_open_folder_dialog};
use sdl3::event::Event;
use sdl3::gamepad::Button;
use sdl3::hint;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::messagebox::{MessageBoxFlag, show_simple_message_box};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::Canvas;
use sdl3::sys;
use sdl3::video::{FullscreenType, Window};
use settings::DesktopSettingsStore;
use std::env;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
#[cfg(test)]
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const FRAMEBUFFER_WIDTH: u32 = 160;
const FRAMEBUFFER_HEIGHT: u32 = 144;
const FRAMEBUFFER_PITCH_BYTES: usize = FRAMEBUFFER_WIDTH as usize * 3;
const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);
const PERFORMANCE_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const INPUT_POLL_SLICE_T_CYCLES: usize = 256;
const DMG_GRAYSCALE_SHADES: [u8; 4] = [255, 170, 85, 0];
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

enum LoopSignal {
    Continue,
    Quit,
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
    keyboard_bindings: KeyboardBindings,
    video_options: VideoOptions,
    audio_volume_percent: u8,
    audio_output: Option<DesktopAudioOutput>,
    gamepad_manager: Option<GamepadManager>,
    save_session: Option<DesktopSaveSession>,
    rtc_sync: HostRtcSync,
    open_rom_dialog: PathSelectionDialog,
    boot_rom_file_dialog: PathSelectionDialog,
    boot_rom_directory_dialog: PathSelectionDialog,
    save_directory_dialog: PathSelectionDialog,
}

struct DesktopSession {
    config: DesktopConfig,
    current_dir: PathBuf,
    loaded_rom: Option<LoadedRom>,
    last_open_directory: Option<PathBuf>,
    recent_roms: Vec<PathBuf>,
}

#[derive(Clone)]
struct LoadedRom {
    path: PathBuf,
    bytes: Vec<u8>,
}

struct FrontendActionContext<'state> {
    session: &'state mut DesktopSession,
    machine: &'state mut Machine<TraceSummaryBuffer>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathDialogResult {
    Selected(PathBuf),
    Canceled,
    Failed(String),
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

struct FramePacer {
    enabled: bool,
    next_frame_start: Instant,
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
    fn new(vsync_enabled: bool) -> Self {
        Self {
            enabled: !vsync_enabled,
            next_frame_start: Instant::now(),
        }
    }

    fn wait_until_next_frame(&mut self) -> Duration {
        if !self.enabled {
            return Duration::ZERO;
        }

        self.next_frame_start += FRAME_DURATION;
        let now = Instant::now();
        if now < self.next_frame_start {
            let wait_duration = self.next_frame_start - now;
            thread::sleep(wait_duration);
            wait_duration
        } else {
            self.next_frame_start = now;
            Duration::ZERO
        }
    }

    fn set_vsync_enabled(&mut self, vsync_enabled: bool) {
        self.enabled = !vsync_enabled;
        self.next_frame_start = Instant::now();
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FramePerformanceSample {
    emulation_duration: Duration,
    render_duration: Duration,
    pacing_duration: Duration,
    audio_queue_ms: Option<f64>,
}

struct PerformanceCounter {
    base_title: String,
    sample_started_at: Instant,
    frames_in_sample: u32,
    sample_emulation_duration: Duration,
    sample_render_duration: Duration,
    sample_pacing_duration: Duration,
    sample_audio_queue_ms: f64,
    sample_audio_queue_observations: u32,
    hud_snapshot: Option<PerformanceHudSnapshot>,
}

impl PerformanceCounter {
    fn new(base_title: String) -> Self {
        Self {
            base_title,
            sample_started_at: Instant::now(),
            frames_in_sample: 0,
            sample_emulation_duration: Duration::ZERO,
            sample_render_duration: Duration::ZERO,
            sample_pacing_duration: Duration::ZERO,
            sample_audio_queue_ms: 0.0,
            sample_audio_queue_observations: 0,
            hud_snapshot: None,
        }
    }

    fn record_presented_frame(
        &mut self,
        window: &mut Window,
        sample: FramePerformanceSample,
    ) -> Result<(), String> {
        self.frames_in_sample += 1;
        self.sample_emulation_duration += sample.emulation_duration;
        self.sample_render_duration += sample.render_duration;
        self.sample_pacing_duration += sample.pacing_duration;
        if let Some(audio_queue_ms) = sample.audio_queue_ms {
            self.sample_audio_queue_ms += audio_queue_ms;
            self.sample_audio_queue_observations += 1;
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
            audio_queue_ms: (self.sample_audio_queue_observations > 0).then_some(
                self.sample_audio_queue_ms / f64::from(self.sample_audio_queue_observations),
            ),
        }
    }

    fn reset_sample(&mut self) {
        self.sample_started_at = Instant::now();
        self.frames_in_sample = 0;
        self.sample_emulation_duration = Duration::ZERO;
        self.sample_render_duration = Duration::ZERO;
        self.sample_pacing_duration = Duration::ZERO;
        self.sample_audio_queue_ms = 0.0;
        self.sample_audio_queue_observations = 0;
    }
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

fn sync_gamepad_rumble(
    runtime: &mut FrontendRuntime,
    machine: &Machine<TraceSummaryBuffer>,
    now: Instant,
) -> Result<(), String> {
    let rumble_requested = !emulation_paused(machine, runtime)
        && machine.cartridge().has_rumble()
        && machine.cartridge().rumble_on();
    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager.update_rumble(rumble_requested, now)?;
    }

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
    let current_dir =
        map_display_result(env::current_dir(), "failed to determine current directory")?;
    let loaded_rom = load_initial_rom(&options, &current_dir)?;
    let last_open_directory = match loaded_rom.as_ref() {
        Some(rom) => rom.path.parent().map(Path::to_path_buf),
        None => settings_store.last_open_directory().map(Path::to_path_buf),
    };
    let mut session = DesktopSession {
        config: options.config,
        current_dir,
        loaded_rom,
        last_open_directory,
        recent_roms: settings_store.recent_roms().to_vec(),
    };

    let (mut machine, diagnostics) = match session.rom_bytes() {
        Some(rom_bytes) => {
            let loaded = load_machine_for_rom(&session.config, &session.current_dir, rom_bytes)?;
            log_boot_rom_fallback_warning(loaded.boot_rom_fallback_warning.as_deref());
            session.config = loaded.effective_config;
            (loaded.machine, loaded.diagnostics)
        }
        None => {
            let prepared = prepare_machine_config(&session.config, &session.current_dir)?;
            log_boot_rom_fallback_warning(prepared.boot_rom_fallback_warning.as_deref());
            session.config = prepared.effective_config;
            (Machine::new_summary(prepared.machine_config), Vec::new())
        }
    };
    if persist_startup_fallback && session.config != original_config {
        settings_store.persist_machine_preferences(&session.config)?;
    }
    write_cartridge_diagnostics(&diagnostics);
    if let Some(rom_path) = session.rom_path() {
        settings_store.remember_loaded_rom(rom_path)?;
        session.recent_roms = settings_store.recent_roms().to_vec();
    }
    let save_session = open_save_session_for_session(&session, &mut machine)?;

    if session.config.video.vsync {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "1", &hint::Hint::Default);
    } else {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "0", &hint::Hint::Default);
    }

    let sdl = map_display_result(sdl3::init(), "failed to initialize SDL3")?;
    let mut input_state = FrontendInputState::new();
    let audio_output = if session.config.audio.enabled {
        let mut audio_output = DesktopAudioOutput::new(
            &map_display_result(sdl.audio(), "failed to initialize SDL3 audio subsystem")?,
            &session.config.audio,
        )?;
        if settings_store.audio_muted() {
            audio_output.set_muted(true)?;
        }
        Some(audio_output)
    } else {
        None
    };
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

    let window_width = FRAMEBUFFER_WIDTH
        .checked_mul(u32::from(session.config.video.window_scale))
        .ok_or_else(|| overflow_error("window width overflowed"))?;
    let window_height = FRAMEBUFFER_HEIGHT
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
    let mut texture = map_display_result(
        texture_creator.create_texture_streaming(
            PixelFormat::RGB24,
            FRAMEBUFFER_WIDTH,
            FRAMEBUFFER_HEIGHT,
        ),
        "failed to create framebuffer texture",
    )?;
    let mut event_pump = map_display_result(sdl.event_pump(), "failed to create SDL3 event pump")?;
    let mut rgb_frame = vec![0_u8; FRAMEBUFFER_HEIGHT as usize * FRAMEBUFFER_PITCH_BYTES];
    let mut runtime = FrontendRuntime {
        paused: !session.has_loaded_rom(),
        menu_state: OverlayMenuState::default(),
        input_state,
        keyboard_bindings: session.config.input.keyboard,
        video_options: session.config.video.clone(),
        audio_volume_percent: session.config.audio.volume_percent,
        audio_output,
        gamepad_manager,
        save_session,
        rtc_sync: HostRtcSync::from_host_clock(),
        open_rom_dialog: PathSelectionDialog::new(),
        boot_rom_file_dialog: PathSelectionDialog::new(),
        boot_rom_directory_dialog: PathSelectionDialog::new(),
        save_directory_dialog: PathSelectionDialog::new(),
    };
    apply_canvas_video_options(&mut canvas, &runtime.video_options)?;
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
    render_frame(
        &mut canvas,
        &mut texture,
        &mut rgb_frame,
        machine.ppu().framebuffer(),
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
                render_frame(
                    &mut canvas,
                    &mut texture,
                    &mut rgb_frame,
                    machine.ppu().framebuffer(),
                    &runtime.video_options,
                    menu_presentation,
                    None,
                )?;
            }
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        let emulation_started_at = Instant::now();
        match {
            let mut context = FrontendActionContext {
                session: &mut session,
                machine: &mut machine,
                runtime: &mut runtime,
                performance_counter: &mut performance_counter,
                frame_pacer: &mut frame_pacer,
                settings_store: &mut settings_store,
            };
            step_until_next_frame(&mut event_pump, &mut canvas, &mut context)
        }? {
            LoopSignal::Continue => {}
            LoopSignal::Quit => break 'running,
        }
        let emulation_duration = emulation_started_at.elapsed();

        if emulation_paused(&machine, &runtime) {
            if runtime.menu_state.is_open() {
                let menu_presentation = Some((
                    &runtime.menu_state,
                    current_menu_presentation(canvas.window(), &runtime, &machine, &session),
                ));
                render_frame(
                    &mut canvas,
                    &mut texture,
                    &mut rgb_frame,
                    machine.ppu().framebuffer(),
                    &runtime.video_options,
                    menu_presentation,
                    None,
                )?;
            }
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        let render_started_at = Instant::now();
        render_frame(
            &mut canvas,
            &mut texture,
            &mut rgb_frame,
            machine.ppu().framebuffer(),
            &runtime.video_options,
            None,
            performance_counter.hud_snapshot(),
        )?;
        let render_duration = render_started_at.elapsed();
        let pacing_duration = frame_pacer.wait_until_next_frame();
        performance_counter.record_presented_frame(
            canvas.window_mut(),
            FramePerformanceSample {
                emulation_duration,
                render_duration,
                pacing_duration,
                audio_queue_ms: runtime
                    .audio_output
                    .as_ref()
                    .and_then(DesktopAudioOutput::queued_duration_ms),
            },
        )?;
    }

    settings_store.set_fullscreen(canvas.window().fullscreen_state() != FullscreenType::Off)?;
    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager.update_rumble(false, Instant::now())?;
    }

    if let Some(save_session) = &mut runtime.save_session {
        save_session.close(&machine)?;
    }
    if let Some(rom_path) = session.rom_path() {
        settings_store.remember_loaded_rom(rom_path)?;
    }
    if let Some(audio_output) = &runtime.audio_output {
        audio_output.flush()?;
    }

    Ok(())
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
    Option<String>,
    Machine<TraceSummaryBuffer>,
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

fn load_initial_rom(
    options: &DesktopRunOptions,
    current_dir: &Path,
) -> Result<Option<LoadedRom>, String> {
    let Some(rom_path) = options.rom_path.as_ref() else {
        return Ok(None);
    };
    let rom_path = resolve_path(current_dir, rom_path);
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
    Ok(Some(LoadedRom {
        path: rom_path,
        bytes: rom_bytes,
    }))
}

fn open_save_session_for_session(
    session: &DesktopSession,
    machine: &mut Machine<TraceSummaryBuffer>,
) -> Result<Option<DesktopSaveSession>, String> {
    let Some(rom_path) = session.rom_path() else {
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
    let rom_name = match session.rom_path() {
        Some(rom_path) => rom_path
            .file_name()
            .unwrap_or(rom_path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        None => "no ROM loaded".to_string(),
    };
    format!(
        "gb-desktop | {} | {} | {} | {}",
        rom_name,
        config.launch.console_model.name(),
        startup_mode_name(config.launch.startup_mode),
        execution_mode_name(config.launch.execution_mode),
    )
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
            gamepad_manager.handle_event(&event, &mut runtime.input_state, machine)?;
            if let Event::ControllerButtonDown { which, .. } = &event {
                gamepad_manager.activate_gamepad_from_input(
                    gamepad_event_joystick_id(*which),
                    &mut runtime.input_state,
                    machine,
                );
            }
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
                toggle_menu(event_pump, canvas.window(), session, machine, runtime)?;
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
                repeat,
                ..
            } => {
                if !repeat {
                    match hotkey_action(&runtime.keyboard_bindings, keycode) {
                        HotkeyAction::None => {}
                        HotkeyAction::ManualSave => {
                            if let Some(save_session) = &mut runtime.save_session {
                                let _ = save_session.flush_if_changed(machine, "manual-hotkey")?;
                            }
                        }
                        HotkeyAction::Reset => {
                            reset_machine(session, machine, runtime, settings_store)?;
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
                    runtime
                        .input_state
                        .set_keyboard_button(machine, button, true);
                }
            }
            Event::KeyUp {
                keycode: Some(keycode),
                repeat,
                ..
            } => {
                if repeat {
                    continue;
                }
                if let Some(button) =
                    joypad_button_for_key(runtime.keyboard_bindings.joypad, keycode)
                {
                    runtime
                        .input_state
                        .set_keyboard_button(machine, button, false);
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
        gamepad_manager.poll_active_gamepad_state(&mut runtime.input_state, machine);
    }
    sync_gamepad_rumble(runtime, machine, Instant::now())?;

    Ok(LoopSignal::Continue)
}

fn step_until_next_frame(
    event_pump: &mut sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<LoopSignal, String> {
    let mut at_frame_origin =
        context.machine.ppu().ly() == 0 && context.machine.ppu().line_dot() == 0;

    loop {
        match process_events(event_pump, canvas, context)? {
            LoopSignal::Continue => {}
            LoopSignal::Quit => return Ok(LoopSignal::Quit),
        }
        if emulation_paused(context.machine, context.runtime) {
            return Ok(LoopSignal::Continue);
        }

        for _ in 0..INPUT_POLL_SLICE_T_CYCLES {
            context.machine.step_t_cycle();
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.capture_t_cycle(context.machine.apu());
            }
            sync_gamepad_rumble(context.runtime, context.machine, Instant::now())?;
            let now_at_frame_origin =
                context.machine.ppu().ly() == 0 && context.machine.ppu().line_dot() == 0;
            if now_at_frame_origin && !at_frame_origin {
                if let Some(audio_output) = &mut context.runtime.audio_output {
                    audio_output.submit_captured_samples()?;
                }
                if let Some(save_session) = &mut context.runtime.save_session {
                    let _ = save_session
                        .maybe_flush_at_frame_boundary(context.machine, Instant::now())?;
                }
                return Ok(LoopSignal::Continue);
            }
            at_frame_origin = now_at_frame_origin;
        }
    }
}

fn toggle_menu(
    event_pump: &sdl3::EventPump,
    window: &Window,
    session: &DesktopSession,
    machine: &mut Machine<TraceSummaryBuffer>,
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

    match result {
        PathDialogResult::Selected(path) => {
            if let Err(error) = open_selected_rom(event_pump, canvas, path, context) {
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

    Ok(())
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
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let battery_backed_state = uses_battery_backed_hardware_persistence(
        context.machine.cartridge().persistence_metadata(),
    )
    .then(|| context.machine.cartridge().persistent_state());

    let mut previous_save_session = context.runtime.save_session.take();
    if let Some(save_session) = previous_save_session.as_mut()
        && let Err(error) = save_session.close(context.machine)
    {
        context.runtime.save_session = previous_save_session;
        return Err(error);
    }

    let rebuild_result: Result<RebuildMachineResult, String> = (|| {
        let (effective_config, boot_rom_fallback_warning, mut next_machine, diagnostics) =
            match context.session.rom_bytes() {
                Some(rom_bytes) => {
                    let loaded =
                        load_machine_for_rom(next_config, &context.session.current_dir, rom_bytes)?;
                    (
                        loaded.effective_config,
                        loaded.boot_rom_fallback_warning,
                        loaded.machine,
                        loaded.diagnostics,
                    )
                }
                None => {
                    let prepared =
                        prepare_machine_config(next_config, &context.session.current_dir)?;
                    (
                        prepared.effective_config,
                        prepared.boot_rom_fallback_warning,
                        Machine::new_summary(prepared.machine_config),
                        Vec::new(),
                    )
                }
            };
        write_cartridge_diagnostics(&diagnostics);
        if let Some(persistent_state) = battery_backed_state
            && let Err(error) = next_machine.restore_cartridge_persistent_state(&persistent_state)
        {
            return Err(format!(
                "failed to restore battery-backed persistence after reconfigure: {error:?}"
            ));
        }

        let next_session = DesktopSession {
            config: effective_config.clone(),
            current_dir: context.session.current_dir.clone(),
            loaded_rom: context.session.loaded_rom.clone(),
            last_open_directory: context.session.last_open_directory.clone(),
            recent_roms: context.session.recent_roms.clone(),
        };
        let next_save_session = open_save_session_for_session(&next_session, &mut next_machine)?;
        Ok((
            effective_config,
            boot_rom_fallback_warning,
            next_machine,
            next_save_session,
        ))
    })();

    let (effective_config, boot_rom_fallback_warning, next_machine, next_save_session) =
        match rebuild_result {
            Ok(value) => value,
            Err(error) => {
                context.runtime.save_session = previous_save_session;
                return Err(error);
            }
        };

    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.clear_buffer()?;
    }

    log_boot_rom_fallback_warning(boot_rom_fallback_warning.as_deref());
    context.runtime.input_state.clear_all(context.machine);
    *context.machine = next_machine;
    context.runtime.save_session = next_save_session;
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

fn open_selected_rom(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    selected_path: PathBuf,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let had_loaded_rom = context.session.has_loaded_rom();
    let rom_path = if selected_path.is_absolute() {
        selected_path
    } else {
        resolve_path(&context.session.current_dir, &selected_path)
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
    let loaded = load_machine_for_rom(
        &context.session.config,
        &context.session.current_dir,
        &rom_bytes,
    )?;
    log_boot_rom_fallback_warning(loaded.boot_rom_fallback_warning.as_deref());
    write_cartridge_diagnostics(&loaded.diagnostics);
    let next_loaded_rom = LoadedRom {
        path: rom_path,
        bytes: rom_bytes,
    };
    let effective_config = loaded.effective_config;
    let config_fell_back = effective_config != context.session.config;
    let mut next_machine = loaded.machine;
    let next_session = DesktopSession {
        config: effective_config.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: Some(next_loaded_rom),
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
    };
    let next_save_session = open_save_session_for_session(&next_session, &mut next_machine)?;

    if let Some(save_session) = &mut context.runtime.save_session {
        save_session.close(context.machine)?;
    }
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.clear_buffer()?;
    }

    context.session.config = effective_config;
    context.session.loaded_rom = next_session.loaded_rom;
    context.session.last_open_directory = context
        .session
        .loaded_rom
        .as_ref()
        .and_then(|rom| rom.path.parent().map(Path::to_path_buf));
    if config_fell_back {
        context
            .settings_store
            .persist_machine_preferences(&context.session.config)?;
    }
    if let Some(rom_path) = context.session.rom_path() {
        context.settings_store.remember_loaded_rom(rom_path)?;
        context.session.recent_roms = context.settings_store.recent_roms().to_vec();
    }
    context.runtime.input_state.clear_all(context.machine);
    *context.machine = next_machine;
    context.runtime.save_session = next_save_session;
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;
    if !had_loaded_rom {
        context.runtime.paused = false;
    }

    if context.runtime.menu_state.is_open() {
        close_menu(event_pump, context.machine, context.runtime)?;
    }

    Ok(())
}

fn open_menu(
    window: &Window,
    machine: &mut Machine<TraceSummaryBuffer>,
    session: &DesktopSession,
    runtime: &mut FrontendRuntime,
) -> Result<(), String> {
    runtime
        .menu_state
        .open(current_menu_presentation(window, runtime, machine, session));
    runtime.input_state.clear_all(machine);
    sync_audio_playback_state(machine, runtime)
}

fn close_menu(
    event_pump: &sdl3::EventPump,
    machine: &mut Machine<TraceSummaryBuffer>,
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
        MenuAction::Close => {
            close_menu(event_pump, context.machine, context.runtime)?;
            Ok(None)
        }
        MenuAction::OpenRom => {
            let default_location = context.session.rom_directory_hint();
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
        MenuAction::SaveBattery => {
            if let Some(save_session) = &mut context.runtime.save_session {
                let _ = save_session.flush_if_changed(context.machine, "menu")?;
            }
            Ok(None)
        }
        MenuAction::ToggleFullscreen => {
            toggle_fullscreen(canvas.window_mut())?;
            context.runtime.video_options.fullscreen =
                canvas.window().fullscreen_state() != FullscreenType::Off;
            if canvas.window().fullscreen_state() == FullscreenType::Off {
                apply_window_scale(
                    canvas.window_mut(),
                    context.runtime.video_options.window_scale,
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
                apply_window_scale(
                    canvas.window_mut(),
                    context.runtime.video_options.window_scale,
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
                apply_window_scale(canvas.window_mut(), defaults.window_scale)?;
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
        MenuAction::ResetAudioDefaults => {
            let defaults = gb_desktop::AudioOptions::default();
            context.runtime.audio_volume_percent = defaults.volume_percent;
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.set_muted(false)?;
                audio_output.set_volume_percent(defaults.volume_percent)?;
            }
            context.settings_store.reset_audio_defaults()?;
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
                context.runtime.input_state.clear_all(context.machine);
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
                context.session,
                context.machine,
                context.runtime,
                context.settings_store,
            )?;
            close_menu(event_pump, context.machine, context.runtime)?;
            Ok(None)
        }
        MenuAction::Quit => Ok(Some(LoopSignal::Quit)),
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
        show_performance_hud: runtime.video_options.show_performance_hud,
        muted: runtime
            .audio_output
            .as_ref()
            .is_some_and(DesktopAudioOutput::is_muted),
        audio_available: runtime.audio_output.is_some(),
        audio_volume_percent: runtime.audio_volume_percent.min(100),
        manual_save_available: runtime
            .save_session
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
    machine: &mut Machine<TraceSummaryBuffer>,
    runtime: &mut FrontendRuntime,
) {
    runtime.input_state.clear_all(machine);
    sync_keyboard_state(
        event_pump,
        keyboard_bindings,
        &mut runtime.input_state,
        machine,
    );
    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager.sync_active_gamepad_state(&mut runtime.input_state, machine);
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
    session: &mut DesktopSession,
    machine: &mut Machine<TraceSummaryBuffer>,
    runtime: &mut FrontendRuntime,
    settings_store: &mut DesktopSettingsStore,
) -> Result<(), String> {
    let Some(rom_bytes) = session.rom_bytes() else {
        return Ok(());
    };
    runtime.rtc_sync.apply_to_machine(machine);
    let battery_backed_state =
        uses_battery_backed_hardware_persistence(machine.cartridge().persistence_metadata())
            .then(|| machine.cartridge().persistent_state());

    let loaded = match load_machine_for_rom(&session.config, &session.current_dir, rom_bytes) {
        Ok(result) => result,
        Err(error) => {
            return Err(format_display_error(
                "failed to reload cartridge during reset",
                &error,
            ));
        }
    };
    log_boot_rom_fallback_warning(loaded.boot_rom_fallback_warning.as_deref());
    write_cartridge_diagnostics(&loaded.diagnostics);
    let config_fell_back = loaded.effective_config != session.config;
    session.config = loaded.effective_config;
    if config_fell_back {
        settings_store.persist_machine_preferences(&session.config)?;
    }
    let mut reset_machine = loaded.machine;
    if let Some(persistent_state) = battery_backed_state
        && let Err(error) = reset_machine.restore_cartridge_persistent_state(&persistent_state)
    {
        return Err(format!(
            "failed to restore battery-backed persistence after reset: {error:?}"
        ));
    }

    if let Some(audio_output) = &mut runtime.audio_output {
        audio_output.clear_buffer()?;
    }

    runtime.input_state.clear_all(machine);
    *machine = reset_machine;
    runtime.rtc_sync.resync_to_host_clock();
    Ok(())
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

fn apply_window_scale(window: &mut Window, scale: u8) -> Result<(), String> {
    let scale = u32::from(scale.max(1));
    let width = FRAMEBUFFER_WIDTH
        .checked_mul(scale)
        .ok_or_else(|| overflow_error("window width overflowed while applying window scale"))?;
    let height = FRAMEBUFFER_HEIGHT
        .checked_mul(scale)
        .ok_or_else(|| overflow_error("window height overflowed while applying window scale"))?;
    map_display_result(
        window.set_size(width, height),
        "failed to resize SDL3 window",
    )
}

fn apply_canvas_video_options(
    canvas: &mut Canvas<Window>,
    video_options: &VideoOptions,
) -> Result<(), String> {
    let presentation_mode = if video_options.integer_scale {
        sys::render::SDL_LOGICAL_PRESENTATION_INTEGER_SCALE
    } else {
        sys::render::SDL_LOGICAL_PRESENTATION_LETTERBOX
    };
    map_display_result(
        canvas.set_logical_size(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, presentation_mode),
        "failed to configure SDL3 logical presentation",
    )
}

fn sync_audio_playback_state(
    machine: &Machine<TraceSummaryBuffer>,
    runtime: &FrontendRuntime,
) -> Result<(), String> {
    let Some(audio_output) = runtime.audio_output.as_ref() else {
        return Ok(());
    };

    if emulation_paused(machine, runtime) {
        audio_output.pause()
    } else {
        audio_output.resume()
    }
}

fn render_frame(
    canvas: &mut Canvas<Window>,
    texture: &mut sdl3::render::Texture<'_>,
    rgb_frame: &mut [u8],
    framebuffer: &[u8],
    video_options: &VideoOptions,
    menu_state: Option<(&OverlayMenuState, MenuPresentation)>,
    performance_hud: Option<PerformanceHudSnapshot>,
) -> Result<(), String> {
    apply_canvas_video_options(canvas, video_options)?;
    for (source, target) in framebuffer.iter().zip(rgb_frame.chunks_exact_mut(3)) {
        let shade = framebuffer_pixel_to_grayscale(*source);
        target[0] = shade;
        target[1] = shade;
        target[2] = shade;
    }
    if let Some((menu_state, menu_presentation)) = menu_state {
        menu_state.render_overlay(
            rgb_frame,
            FRAMEBUFFER_WIDTH as usize,
            FRAMEBUFFER_HEIGHT as usize,
            menu_presentation,
        );
    }
    if menu_state.is_none()
        && video_options.show_performance_hud
        && let Some(snapshot) = performance_hud
    {
        render_performance_hud(
            rgb_frame,
            FRAMEBUFFER_WIDTH as usize,
            FRAMEBUFFER_HEIGHT as usize,
            snapshot,
        );
    }

    map_display_result(
        texture.update(None, rgb_frame, FRAMEBUFFER_PITCH_BYTES),
        "failed to update framebuffer texture",
    )?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    map_display_result(
        canvas.copy(texture, None, None),
        "failed to present framebuffer texture",
    )?;
    canvas.present();
    Ok(())
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
        next_save_flush_policy, next_startup_mode, next_window_scale, performance_window_title,
        run_desktop,
    };
    use gb_core::{
        CartridgeDiagnostic, CartridgeDiagnosticSeverity, ConsoleModel, ExecutionMode, Machine,
        MachineConfig, PersistentCartState, StartupMode, TraceSummaryBuffer,
    };
    use gb_desktop::{
        BootRomVerificationMode, DesktopConfig, DesktopConsoleModel, DesktopKey,
        DesktopSaveFlushPolicy, GamepadButtonBinding, GamepadDirectionalSource,
        GamepadMenuBindings, GamepadRumbleMode, MenuKeyboardBindings,
    };
    use sdl3::dialog::DialogError;
    use sdl3::event::Event;
    use sdl3::gamepad::Button;
    use sdl3::joystick::JoystickId;
    use sdl3::keyboard::{Keycode, Mod};
    use sdl3::render::Canvas;
    use sdl3::video::Window;
    use std::ffi::CString;
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

    fn push_key_event(events: &sdl3::EventSubsystem, keycode: Keycode, down: bool) {
        let desktop_key =
            desktop_key_from_keycode(keycode).expect("test keycode should map to a desktop key");
        let scancode = desktop_key_scancode(desktop_key);
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
                | (1 << Button::East as u32)
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
        machine: Machine<TraceSummaryBuffer>,
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
                super::load_machine_for_rom(&config, &current_dir, &rom_bytes)
                    .expect("frontend harness machine should load")
                    .machine
            } else {
                Machine::new_summary(
                    MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
                )
            };

            let session = super::DesktopSession {
                config: config.clone(),
                current_dir,
                loaded_rom,
                last_open_directory: Some(root.clone()),
                recent_roms: Vec::new(),
            };

            let sdl = sdl3::init().expect("frontend harness SDL should initialize");
            let mut input_state = super::FrontendInputState::new();
            let audio_output = if with_audio {
                Some(
                    super::DesktopAudioOutput::new(
                        &sdl.audio().expect("frontend harness audio subsystem"),
                        &config.audio,
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
            let performance_counter =
                super::PerformanceCounter::new(super::window_title(&session, &config));
            let save_session = super::open_save_session_for_session(&session, &mut machine)
                .expect("frontend harness save session");
            let runtime = super::FrontendRuntime {
                paused: !with_rom,
                menu_state: super::OverlayMenuState::default(),
                input_state,
                keyboard_bindings: config.input.keyboard,
                video_options: config.video.clone(),
                audio_volume_percent: config.audio.volume_percent,
                audio_output,
                gamepad_manager,
                save_session,
                rtc_sync: super::HostRtcSync::from_host_clock(),
                open_rom_dialog: super::PathSelectionDialog::new(),
                boot_rom_file_dialog: super::PathSelectionDialog::new(),
                boot_rom_directory_dialog: super::PathSelectionDialog::new(),
                save_directory_dialog: super::PathSelectionDialog::new(),
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
            "SUPER MARIO L"
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
                config: launcher_config,
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
                config: rom_config,
            },
            rom_store,
        )
        .expect("ROM startup should run and stop cleanly under headless SDL");
        rom_quit
            .join()
            .expect("ROM quit-event helper should finish");
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
                config,
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
                config,
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
                config,
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
                    emulation_duration: Duration::from_millis(10),
                    render_duration: Duration::from_millis(2),
                    pacing_duration: Duration::from_millis(4),
                    audio_queue_ms: Some(18.0),
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
                config: DesktopConfig::default(),
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
                    config: DesktopConfig::default(),
                },
                &harness.root,
            )
            .expect("missing ROM path should be allowed")
            .is_none()
        );

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
        assert_ne!(harness.machine.joypad().snapshot().pressed_mask, 0);
        harness.push_key(Keycode::Z, false);
        harness
            .process_events()
            .expect("keyboard release should process");
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
        super::render_frame(
            &mut harness.canvas,
            &mut texture,
            &mut rgb_frame,
            harness.machine.ppu().framebuffer(),
            &harness.runtime.video_options,
            Some((&harness.runtime.menu_state, open_menu_presentation)),
            None,
        )
        .expect("overlay frame should render");
        assert!(rgb_frame.iter().any(|byte| *byte != 0));

        harness.runtime.menu_state.close();
        harness.runtime.video_options.show_performance_hud = true;
        super::render_frame(
            &mut harness.canvas,
            &mut texture,
            &mut rgb_frame,
            harness.machine.ppu().framebuffer(),
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
