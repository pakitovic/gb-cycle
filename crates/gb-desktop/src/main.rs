mod audio;
mod bootrom;
mod cli;
mod input;
mod menu;
mod save_session;
mod settings;

use audio::DesktopAudioOutput;
use bootrom::{load_boot_rom_assets, resolve_path};
use cli::{CliAction, DesktopRunOptions, help_text, parse_cli_arguments_with_base_config};
use gb_core::{
    CartridgeDiagnostic, CartridgeDiagnosticSeverity, ExecutionMode, JoypadButton, Machine,
    MachineConfig, StartupMode, TraceSummaryBuffer,
};
use gb_desktop::{
    DesktopConfig, DesktopKey, DesktopSaveFlushPolicy, GamepadButtonBinding, GamepadButtonBindings,
    GamepadDirectionalSource, GamepadMenuBindings, HotkeyBindings, JoypadKeyboardBindings,
    KeyboardBindings, MenuKeyboardBindings, PreferredGamepadIdentity, VideoOptions,
};
use gb_persistence::uses_battery_backed_hardware_persistence;
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
use sdl3::dialog::{DialogError, DialogFileFilter, show_open_file_dialog};
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
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
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
    open_rom_dialog: OpenRomDialog,
}

struct DesktopSession<'a> {
    config: &'a DesktopConfig,
    current_dir: PathBuf,
    loaded_rom: Option<LoadedRom>,
    last_open_directory: Option<PathBuf>,
    recent_roms: Vec<PathBuf>,
}

struct LoadedRom {
    path: PathBuf,
    bytes: Vec<u8>,
}

struct FrontendActionContext<'state, 'config> {
    session: &'state mut DesktopSession<'config>,
    machine: &'state mut Machine<TraceSummaryBuffer>,
    runtime: &'state mut FrontendRuntime,
    performance_counter: &'state mut PerformanceCounter,
    settings_store: &'state mut DesktopSettingsStore,
}

impl DesktopSession<'_> {
    fn has_loaded_rom(&self) -> bool {
        self.loaded_rom.is_some()
    }

    fn rom_path(&self) -> Option<&Path> {
        self.loaded_rom.as_ref().map(|rom| rom.path.as_path())
    }

    fn rom_bytes(&self) -> Option<&[u8]> {
        self.loaded_rom.as_ref().map(|rom| rom.bytes.as_slice())
    }

    fn rom_directory_hint(&self) -> &Path {
        self.rom_path()
            .and_then(Path::parent)
            .or(self.last_open_directory.as_deref())
            .unwrap_or(self.current_dir.as_path())
    }

    fn recent_roms(&self) -> &[PathBuf] {
        &self.recent_roms
    }
}

struct OpenRomDialog {
    pending: bool,
    sender: Sender<OpenRomDialogResult>,
    receiver: Receiver<OpenRomDialogResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OpenRomDialogResult {
    Selected(PathBuf),
    Canceled,
    Failed(String),
}

impl OpenRomDialog {
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

    fn show(&mut self, window: &Window, default_location: &Path) -> Result<(), String> {
        if self.pending {
            return Ok(());
        }

        let sender = self.sender.clone();
        show_open_file_dialog(
            &ROM_FILE_DIALOG_FILTERS,
            Some(default_location),
            false,
            window,
            Box::new(move |result, _| {
                let _ = sender.send(map_open_rom_dialog_result(result));
            }),
        )
        .map_err(|error| format!("failed to show SDL3 open ROM dialog: {error}"))?;
        self.pending = true;
        Ok(())
    }

    fn take_result(&mut self) -> Option<OpenRomDialogResult> {
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

struct FramePacer {
    enabled: bool,
    next_frame_start: Instant,
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
        window
            .set_title(&performance_window_title(&self.base_title, snapshot))
            .map_err(|error| format!("failed to update SDL3 window title: {error}"))?;

        self.reset_sample();

        Ok(())
    }

    fn reset_base_title(&mut self, window: &mut Window, base_title: String) -> Result<(), String> {
        self.base_title = base_title;
        self.hud_snapshot = None;
        self.reset_sample();
        window
            .set_title(&self.base_title)
            .map_err(|error| format!("failed to update SDL3 window title: {error}"))
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

fn map_open_rom_dialog_result(result: Result<Vec<PathBuf>, DialogError>) -> OpenRomDialogResult {
    match result {
        Ok(paths) => paths
            .into_iter()
            .next()
            .map(OpenRomDialogResult::Selected)
            .unwrap_or(OpenRomDialogResult::Canceled),
        Err(DialogError::Canceled) => OpenRomDialogResult::Canceled,
        Err(error) => OpenRomDialogResult::Failed(error.to_string()),
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

fn emulation_paused(machine: &Machine<TraceSummaryBuffer>, runtime: &FrontendRuntime) -> bool {
    machine.cartridge().is_empty() || runtime.paused || runtime.menu_state.is_open()
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
    match parse_cli_arguments_with_base_config(arguments.iter().map(String::as_str), base_config)? {
        CliAction::ShowHelp => {
            print!("{}", help_text());
            Ok(())
        }
        CliAction::Run(options) => run_desktop(*options, settings_store),
    }
}

fn run_desktop(
    options: DesktopRunOptions,
    mut settings_store: DesktopSettingsStore,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let loaded_rom = load_initial_rom(&options, &current_dir)?;
    let last_open_directory = loaded_rom
        .as_ref()
        .and_then(|rom| rom.path.parent().map(Path::to_path_buf))
        .or_else(|| settings_store.last_open_directory().map(Path::to_path_buf));
    let mut session = DesktopSession {
        config: &options.config,
        current_dir,
        loaded_rom,
        last_open_directory,
        recent_roms: settings_store.recent_roms().to_vec(),
    };

    let (mut machine, diagnostics) = match session.rom_bytes() {
        Some(rom_bytes) => load_machine_for_rom(session.config, &session.current_dir, rom_bytes)?,
        None => (
            Machine::new_summary(build_machine_config(session.config, &session.current_dir)?),
            Vec::new(),
        ),
    };
    write_cartridge_diagnostics(&diagnostics);
    if let Some(rom_path) = session.rom_path() {
        settings_store.remember_loaded_rom(rom_path)?;
        session.recent_roms = settings_store.recent_roms().to_vec();
    }
    let save_session = open_save_session_for_session(&session, &mut machine)?;

    if options.config.video.vsync {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "1", &hint::Hint::Default);
    } else {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "0", &hint::Hint::Default);
    }

    let sdl = sdl3::init().map_err(|error| format!("failed to initialize SDL3: {error}"))?;
    let mut input_state = FrontendInputState::new();
    let audio_output = if options.config.audio.enabled {
        let mut audio_output = DesktopAudioOutput::new(
            &sdl.audio()
                .map_err(|error| format!("failed to initialize SDL3 audio subsystem: {error}"))?,
            &options.config.audio,
        )?;
        if settings_store.audio_muted() {
            audio_output.set_muted(true)?;
        }
        Some(audio_output)
    } else {
        None
    };
    let gamepad_manager = if options.config.input.gamepad.enabled {
        Some(GamepadManager::new(
            &sdl.gamepad()
                .map_err(|error| format!("failed to initialize SDL3 gamepad subsystem: {error}"))?,
            options.config.input.gamepad.clone(),
            &mut input_state,
            &mut machine,
        )?)
    } else {
        None
    };
    let video = sdl
        .video()
        .map_err(|error| format!("failed to initialize SDL3 video subsystem: {error}"))?;

    let window_width = FRAMEBUFFER_WIDTH
        .checked_mul(u32::from(options.config.video.window_scale))
        .ok_or_else(|| "window width overflowed".to_string())?;
    let window_height = FRAMEBUFFER_HEIGHT
        .checked_mul(u32::from(options.config.video.window_scale))
        .ok_or_else(|| "window height overflowed".to_string())?;

    let base_window_title = window_title(&session, session.config);
    let mut frame_pacer = FramePacer::new(session.config.video.vsync);
    let mut performance_counter = PerformanceCounter::new(base_window_title.clone());
    let mut window_builder = video.window(&base_window_title, window_width, window_height);
    window_builder.position_centered();
    if session.config.video.fullscreen {
        window_builder.fullscreen();
    }
    let window = window_builder
        .build()
        .map_err(|error| format!("failed to create SDL3 window: {error}"))?;
    let mut canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormat::RGB24, FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT)
        .map_err(|error| format!("failed to create framebuffer texture: {error}"))?;
    let mut event_pump = sdl
        .event_pump()
        .map_err(|error| format!("failed to create SDL3 event pump: {error}"))?;
    let mut rgb_frame = vec![0_u8; FRAMEBUFFER_HEIGHT as usize * FRAMEBUFFER_PITCH_BYTES];
    let mut runtime = FrontendRuntime {
        paused: !session.has_loaded_rom(),
        menu_state: OverlayMenuState::default(),
        input_state,
        keyboard_bindings: options.config.input.keyboard,
        video_options: options.config.video.clone(),
        audio_volume_percent: options.config.audio.volume_percent,
        audio_output,
        gamepad_manager,
        save_session,
        open_rom_dialog: OpenRomDialog::new(),
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
        process_pending_open_rom_dialog(
            &event_pump,
            canvas.window_mut(),
            &mut performance_counter,
            &mut session,
            &mut machine,
            &mut runtime,
            &mut settings_store,
        )?;

        match process_events(
            &mut event_pump,
            canvas.window_mut(),
            &mut session,
            &mut machine,
            &mut runtime,
            &mut performance_counter,
            &mut settings_store,
        )? {
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
        match step_until_next_frame(
            &mut event_pump,
            canvas.window_mut(),
            &mut session,
            &mut machine,
            &mut runtime,
            &mut performance_counter,
            &mut settings_store,
        )? {
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

fn build_machine_config(
    config: &DesktopConfig,
    current_dir: &Path,
) -> Result<MachineConfig, String> {
    let boot_rom_assets = load_boot_rom_assets(
        config.boot_rom.search_path.as_deref(),
        config.boot_rom.verification,
        config.launch.console_model,
        config.launch.startup_mode,
        current_dir,
    )?;

    Ok(
        MachineConfig::new(config.launch.console_model.console_model())
            .with_startup_mode(config.launch.startup_mode)
            .with_execution_mode(config.launch.execution_mode)
            .with_boot_rom_assets(boot_rom_assets),
    )
}

fn load_machine_for_rom(
    config: &DesktopConfig,
    current_dir: &Path,
    rom_bytes: &[u8],
) -> Result<(Machine<TraceSummaryBuffer>, Vec<CartridgeDiagnostic>), String> {
    let machine_config = build_machine_config(config, current_dir)?;
    let mut machine = Machine::new_summary(machine_config);
    let diagnostics = machine
        .load_cartridge(rom_bytes.to_vec())
        .map_err(|error| format!("failed to load cartridge: {error:?}"))?;
    Ok((machine, diagnostics))
}

fn load_initial_rom(
    options: &DesktopRunOptions,
    current_dir: &Path,
) -> Result<Option<LoadedRom>, String> {
    let Some(rom_path) = options.rom_path.as_ref() else {
        return Ok(None);
    };
    let rom_path = resolve_path(current_dir, rom_path);
    let rom_bytes = fs::read(&rom_path)
        .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;
    Ok(Some(LoadedRom {
        path: rom_path,
        bytes: rom_bytes,
    }))
}

fn open_save_session_for_session(
    session: &DesktopSession<'_>,
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
    let save_key = session
        .config
        .saves
        .resolve_key(rom_path)
        .map_err(|error| error.to_string())?;
    DesktopSaveSession::open(
        save_root.as_deref(),
        session.config.saves.flush_policy,
        save_key,
        machine,
    )
}

fn window_title(session: &DesktopSession<'_>, config: &DesktopConfig) -> String {
    let rom_name = session
        .rom_path()
        .map(|rom_path| {
            rom_path
                .file_name()
                .unwrap_or(rom_path.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| "no ROM loaded".to_string());
    format!(
        "gb-desktop | {} | {} | {} | {}",
        rom_name,
        config.launch.console_model.name(),
        startup_mode_name(config.launch.startup_mode),
        execution_mode_name(config.launch.execution_mode),
    )
}

fn performance_window_title(base_title: &str, snapshot: PerformanceHudSnapshot) -> String {
    let audio = snapshot
        .audio_queue_ms
        .map(|audio_queue_ms| format!("{audio_queue_ms:.1} ms"))
        .unwrap_or_else(|| "off".to_string());
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
    window: &mut Window,
    session: &mut DesktopSession<'_>,
    machine: &mut Machine<TraceSummaryBuffer>,
    runtime: &mut FrontendRuntime,
    performance_counter: &mut PerformanceCounter,
    settings_store: &mut DesktopSettingsStore,
) -> Result<LoopSignal, String> {
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
                                settings_store,
                            };
                            let _ = execute_menu_action(action, event_pump, window, &mut context)?;
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
                            settings_store,
                        };
                        let _ = execute_menu_action(action, event_pump, window, &mut context)?;
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
                            settings_store,
                        };
                        let _ = execute_menu_action(action, event_pump, window, &mut context)?;
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
                toggle_menu(event_pump, window, session, machine, runtime)?;
                continue;
            }
            Event::ControllerButtonDown { which, button, .. }
                if *button == Button::Guide
                    && runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                        manager.is_active_gamepad(gamepad_event_joystick_id(*which))
                    }) =>
            {
                toggle_menu(event_pump, window, session, machine, runtime)?;
                continue;
            }
            _ => {}
        }

        if runtime.menu_state.is_open() {
            let presentation = current_menu_presentation(window, runtime, machine, session);
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
                    settings_store,
                };
                if let Some(signal) = execute_menu_action(action, event_pump, window, &mut context)?
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
                            reset_machine(session, machine, runtime)?;
                            let keyboard_bindings = runtime.keyboard_bindings;
                            sync_live_input_state(event_pump, &keyboard_bindings, machine, runtime);
                        }
                        HotkeyAction::ToggleFullscreen => {
                            toggle_fullscreen(window)?;
                            settings_store
                                .set_fullscreen(window.fullscreen_state() != FullscreenType::Off)?;
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
        return Ok(LoopSignal::Continue);
    }

    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager.poll_active_gamepad_state(&mut runtime.input_state, machine);
    }

    Ok(LoopSignal::Continue)
}

fn step_until_next_frame(
    event_pump: &mut sdl3::EventPump,
    window: &mut Window,
    session: &mut DesktopSession<'_>,
    machine: &mut Machine<TraceSummaryBuffer>,
    runtime: &mut FrontendRuntime,
    performance_counter: &mut PerformanceCounter,
    settings_store: &mut DesktopSettingsStore,
) -> Result<LoopSignal, String> {
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    loop {
        match process_events(
            event_pump,
            window,
            session,
            machine,
            runtime,
            performance_counter,
            settings_store,
        )? {
            LoopSignal::Continue => {}
            LoopSignal::Quit => return Ok(LoopSignal::Quit),
        }
        if emulation_paused(machine, runtime) {
            return Ok(LoopSignal::Continue);
        }

        for _ in 0..INPUT_POLL_SLICE_T_CYCLES {
            machine.step_t_cycle();
            if let Some(audio_output) = &mut runtime.audio_output {
                audio_output.capture_t_cycle(machine.apu());
            }
            let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
            if now_at_frame_origin && !at_frame_origin {
                if let Some(audio_output) = &mut runtime.audio_output {
                    audio_output.submit_captured_samples()?;
                }
                if let Some(save_session) = &mut runtime.save_session {
                    let _ = save_session.maybe_flush_at_frame_boundary(machine, Instant::now())?;
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
    session: &DesktopSession<'_>,
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
    window: &mut Window,
    performance_counter: &mut PerformanceCounter,
    session: &mut DesktopSession<'_>,
    machine: &mut Machine<TraceSummaryBuffer>,
    runtime: &mut FrontendRuntime,
    settings_store: &mut DesktopSettingsStore,
) -> Result<(), String> {
    let Some(result) = runtime.open_rom_dialog.take_result() else {
        return Ok(());
    };

    match result {
        OpenRomDialogResult::Selected(path) => {
            let mut context = FrontendActionContext {
                session,
                machine,
                runtime,
                performance_counter,
                settings_store,
            };
            if let Err(error) = open_selected_rom(event_pump, window, path, &mut context) {
                show_error_message(Some(window), "Open ROM failed", &error);
                eprintln!("warning: {error}");
            }
        }
        OpenRomDialogResult::Canceled => {}
        OpenRomDialogResult::Failed(error) => {
            show_error_message(
                Some(window),
                "Open ROM failed",
                &format!("failed to complete SDL3 open ROM dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 open ROM dialog: {error}");
        }
    }

    Ok(())
}

fn open_selected_rom(
    event_pump: &sdl3::EventPump,
    window: &mut Window,
    selected_path: PathBuf,
    context: &mut FrontendActionContext<'_, '_>,
) -> Result<(), String> {
    let had_loaded_rom = context.session.has_loaded_rom();
    let rom_path = if selected_path.is_absolute() {
        selected_path
    } else {
        resolve_path(&context.session.current_dir, &selected_path)
    };
    let rom_bytes = fs::read(&rom_path)
        .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;
    let (mut next_machine, diagnostics) = load_machine_for_rom(
        context.session.config,
        &context.session.current_dir,
        &rom_bytes,
    )?;
    write_cartridge_diagnostics(&diagnostics);
    let next_loaded_rom = LoadedRom {
        path: rom_path,
        bytes: rom_bytes,
    };
    let next_session = DesktopSession {
        config: context.session.config,
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

    context.session.loaded_rom = next_session.loaded_rom;
    context.session.last_open_directory = context
        .session
        .loaded_rom
        .as_ref()
        .and_then(|rom| rom.path.parent().map(Path::to_path_buf));
    if let Some(rom_path) = context.session.rom_path() {
        context.settings_store.remember_loaded_rom(rom_path)?;
        context.session.recent_roms = context.settings_store.recent_roms().to_vec();
    }
    context.runtime.input_state.clear_all(context.machine);
    *context.machine = next_machine;
    context.runtime.save_session = next_save_session;
    context.performance_counter.reset_base_title(
        window,
        window_title(context.session, context.session.config),
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
    session: &DesktopSession<'_>,
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
    window: &mut Window,
    context: &mut FrontendActionContext<'_, '_>,
) -> Result<Option<LoopSignal>, String> {
    match action {
        MenuAction::Close => {
            close_menu(event_pump, context.machine, context.runtime)?;
            Ok(None)
        }
        MenuAction::OpenRom => {
            let default_location = context.session.rom_directory_hint();
            if let Err(error) = context
                .runtime
                .open_rom_dialog
                .show(window, default_location)
            {
                show_warning_message(Some(window), "Open ROM", &error);
                eprintln!("warning: {error}");
            }
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
                show_warning_message(Some(window), "Open Recent", &error);
                eprintln!("warning: {error}");
                return Ok(None);
            }

            if let Err(error) = open_selected_rom(event_pump, window, rom_path, context) {
                show_warning_message(Some(window), "Open Recent", &error);
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
            toggle_fullscreen(window)?;
            if window.fullscreen_state() == FullscreenType::Off {
                apply_window_scale(window, context.runtime.video_options.window_scale)?;
            }
            context
                .settings_store
                .set_fullscreen(window.fullscreen_state() != FullscreenType::Off)?;
            Ok(None)
        }
        MenuAction::CycleWindowScale => {
            context.runtime.video_options.window_scale =
                next_window_scale(context.runtime.video_options.window_scale);
            if window.fullscreen_state() == FullscreenType::Off {
                apply_window_scale(window, context.runtime.video_options.window_scale)?;
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
            reset_machine(context.session, context.machine, context.runtime)?;
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
    session: &DesktopSession<'_>,
) -> MenuPresentation {
    let gamepad_available = runtime.gamepad_manager.is_some();
    let active_gamepad_label = runtime
        .gamepad_manager
        .as_ref()
        .and_then(GamepadManager::active_gamepad_name)
        .map(CompactMenuLabel::from_gamepad_name)
        .unwrap_or_default();
    let preferred_gamepad_configured = runtime
        .gamepad_manager
        .as_ref()
        .is_some_and(|manager| manager.preferred_device().is_configured());
    let preferred_gamepad_label = runtime
        .gamepad_manager
        .as_ref()
        .and_then(GamepadManager::preferred_device_name)
        .map(CompactMenuLabel::from_gamepad_name)
        .unwrap_or_else(|| {
            if preferred_gamepad_configured {
                CompactMenuLabel::from_text("SAVED")
            } else {
                CompactMenuLabel::default()
            }
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
        fullscreen: window.fullscreen_state() != FullscreenType::Off,
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
        rom_dialog_pending: runtime.open_rom_dialog.is_pending(),
        gamepad_available,
        gamepad_directional_source: runtime.gamepad_manager.as_ref().map_or(
            GamepadDirectionalSource::default(),
            GamepadManager::directional_source,
        ),
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
        active_gamepad_label,
        preferred_gamepad_configured,
        preferred_gamepad_label,
        keyboard_bindings: runtime.keyboard_bindings.joypad,
        keyboard_menu_bindings: runtime.keyboard_bindings.menu,
        hotkey_bindings: runtime.keyboard_bindings.hotkeys,
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
    session: &DesktopSession<'_>,
    machine: &mut Machine<TraceSummaryBuffer>,
    runtime: &mut FrontendRuntime,
) -> Result<(), String> {
    let Some(rom_bytes) = session.rom_bytes() else {
        return Ok(());
    };
    let battery_backed_state =
        uses_battery_backed_hardware_persistence(machine.cartridge().persistence_metadata())
            .then(|| machine.cartridge().persistent_state());

    let (mut reset_machine, diagnostics) =
        load_machine_for_rom(session.config, &session.current_dir, rom_bytes)
            .map_err(|error| format!("failed to reload cartridge during reset: {error}"))?;
    write_cartridge_diagnostics(&diagnostics);
    if let Some(persistent_state) = battery_backed_state {
        reset_machine
            .restore_cartridge_persistent_state(&persistent_state)
            .map_err(|error| {
                format!("failed to restore battery-backed persistence after reset: {error:?}")
            })?;
    }

    if let Some(audio_output) = &mut runtime.audio_output {
        audio_output.clear_buffer()?;
    }

    runtime.input_state.clear_all(machine);
    *machine = reset_machine;
    Ok(())
}

fn toggle_fullscreen(window: &mut Window) -> Result<(), String> {
    let target_state = window.fullscreen_state() == FullscreenType::Off;
    window
        .set_fullscreen(target_state)
        .map_err(|error| format!("failed to toggle SDL3 fullscreen state: {error}"))
}

fn apply_window_scale(window: &mut Window, scale: u8) -> Result<(), String> {
    let scale = u32::from(scale.max(1));
    let width = FRAMEBUFFER_WIDTH
        .checked_mul(scale)
        .ok_or_else(|| "window width overflowed while applying window scale".to_string())?;
    let height = FRAMEBUFFER_HEIGHT
        .checked_mul(scale)
        .ok_or_else(|| "window height overflowed while applying window scale".to_string())?;
    window
        .set_size(width, height)
        .map_err(|error| format!("failed to resize SDL3 window: {error}"))
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
    canvas
        .set_logical_size(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, presentation_mode)
        .map_err(|error| format!("failed to configure SDL3 logical presentation: {error}"))
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

    texture
        .update(None, rgb_frame, FRAMEBUFFER_PITCH_BYTES)
        .map_err(|error| format!("failed to update framebuffer texture: {error}"))?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas
        .copy(texture, None, None)
        .map_err(|error| format!("failed to present framebuffer texture: {error}"))?;
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
        GamepadBindingTarget, GamepadMenuBindingTarget, KeyboardBindingTarget,
        KeyboardMenuBindingTarget, OpenRomDialogResult, PerformanceHudSnapshot,
        ROM_FILE_DIALOG_FILTERS, assign_gamepad_binding, assign_gamepad_menu_binding,
        assign_keyboard_binding, assign_keyboard_menu_binding,
        assignable_key_for_binding_target_from_keycode,
        assignable_menu_key_for_binding_target_from_keycode, compact_recent_rom_label,
        gamepad_binding_target_for_binding, gamepad_menu_binding_target_for_binding,
        hotkey_binding_target_for_key, joypad_binding_target_for_key,
        keyboard_menu_binding_target_for_key, map_open_rom_dialog_result,
        menu_input_for_gamepad_button, menu_input_for_key, next_audio_volume_percent,
        next_gamepad_directional_source, next_window_scale, performance_window_title,
    };
    use gb_desktop::{
        DesktopConfig, DesktopKey, GamepadButtonBinding, GamepadDirectionalSource,
        GamepadMenuBindings, MenuKeyboardBindings,
    };
    use sdl3::dialog::DialogError;
    use sdl3::gamepad::Button;
    use sdl3::keyboard::Keycode;
    use std::path::{Path, PathBuf};

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
    fn open_rom_dialog_result_uses_the_first_selected_path() {
        assert_eq!(
            map_open_rom_dialog_result(Ok(vec![
                PathBuf::from("/tmp/tetris.gb"),
                PathBuf::from("/tmp/other.gb"),
            ])),
            OpenRomDialogResult::Selected(PathBuf::from("/tmp/tetris.gb"))
        );
    }

    #[test]
    fn open_rom_dialog_result_preserves_cancel_as_a_non_selection() {
        assert_eq!(
            map_open_rom_dialog_result(Err(DialogError::Canceled)),
            OpenRomDialogResult::Canceled
        );
    }

    #[test]
    fn open_rom_dialog_filters_include_supported_game_boy_extensions() {
        assert_eq!(ROM_FILE_DIALOG_FILTERS[0].name, "Game Boy ROMs");
        assert_eq!(ROM_FILE_DIALOG_FILTERS[0].pattern, "gb;gbc;bin");
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
}
