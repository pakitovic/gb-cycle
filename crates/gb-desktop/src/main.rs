mod audio;
mod bootrom;
mod cli;
mod input;
mod save_session;

use audio::DesktopAudioOutput;
use bootrom::{load_boot_rom_assets, resolve_path};
use cli::{CliAction, DesktopRunOptions, help_text, parse_cli_arguments};
use gb_core::{
    CartridgeDiagnostic, CartridgeDiagnosticSeverity, ExecutionMode, JoypadButton, Machine,
    MachineConfig, StartupMode, TraceSummaryBuffer,
};
use gb_desktop::{DesktopConfig, DesktopKey};
use input::{FrontendInputState, GamepadManager};
use save_session::DesktopSaveSession;
use sdl3::event::Event;
use sdl3::hint;
use sdl3::keyboard::Keycode;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::Canvas;
use sdl3::video::Window;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

const FRAMEBUFFER_WIDTH: u32 = 160;
const FRAMEBUFFER_HEIGHT: u32 = 144;
const FRAMEBUFFER_PITCH_BYTES: usize = FRAMEBUFFER_WIDTH as usize * 3;
const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);
const PERFORMANCE_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const INPUT_POLL_SLICE_T_CYCLES: usize = 256;
const DMG_GRAYSCALE_SHADES: [u8; 4] = [255, 170, 85, 0];

enum LoopSignal {
    Continue,
    Quit,
}

enum HotkeyAction {
    None,
    ManualSave,
}

struct FrontendRuntime {
    paused: bool,
    input_state: FrontendInputState,
    audio_output: Option<DesktopAudioOutput>,
    gamepad_manager: Option<GamepadManager>,
    save_session: Option<DesktopSaveSession>,
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

    fn wait_until_next_frame(&mut self) {
        if !self.enabled {
            return;
        }

        self.next_frame_start += FRAME_DURATION;
        let now = Instant::now();
        if now < self.next_frame_start {
            thread::sleep(self.next_frame_start - now);
        } else {
            self.next_frame_start = now;
        }
    }
}

struct PerformanceCounter {
    base_title: String,
    sample_started_at: Instant,
    frames_in_sample: u32,
}

impl PerformanceCounter {
    fn new(base_title: String) -> Self {
        Self {
            base_title,
            sample_started_at: Instant::now(),
            frames_in_sample: 0,
        }
    }

    fn record_presented_frame(&mut self, window: &mut Window) -> Result<(), String> {
        self.frames_in_sample += 1;

        let elapsed = self.sample_started_at.elapsed();
        if elapsed < PERFORMANCE_SAMPLE_INTERVAL {
            return Ok(());
        }

        let fps = f64::from(self.frames_in_sample) / elapsed.as_secs_f64();
        let frame_time_ms = elapsed.as_secs_f64() * 1_000.0 / f64::from(self.frames_in_sample);
        let speed_percent = fps / target_frame_rate_hz() * 100.0;
        window
            .set_title(&performance_window_title(
                &self.base_title,
                fps,
                frame_time_ms,
                speed_percent,
            ))
            .map_err(|error| format!("failed to update SDL3 window title: {error}"))?;

        self.sample_started_at = Instant::now();
        self.frames_in_sample = 0;

        Ok(())
    }
}

fn main() -> ExitCode {
    match run_from_cli(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
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
    match parse_cli_arguments(arguments)? {
        CliAction::ShowHelp => {
            print!("{}", help_text());
            Ok(())
        }
        CliAction::Run(options) => run_desktop(*options),
    }
}

fn run_desktop(options: DesktopRunOptions) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let rom_path = resolve_path(&current_dir, &options.rom_path);
    let rom_bytes = fs::read(&rom_path)
        .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;

    let machine_config = build_machine_config(&options.config, &current_dir)?;
    let mut machine = Machine::new_summary(machine_config);
    let diagnostics = machine
        .load_cartridge(rom_bytes)
        .map_err(|error| format!("failed to load cartridge: {error:?}"))?;
    write_cartridge_diagnostics(&diagnostics);

    let save_root = options
        .config
        .saves
        .resolve_directory(&rom_path)
        .map(|path| resolve_path(&current_dir, &path));
    let save_key = options
        .config
        .saves
        .resolve_key(&rom_path)
        .map_err(|error| error.to_string())?;
    let save_session = DesktopSaveSession::open(
        save_root.as_deref(),
        options.config.saves.flush_policy,
        save_key,
        &mut machine,
    )?;

    if options.config.video.vsync {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "1", &hint::Hint::Default);
    } else {
        let _ = hint::set_with_priority(hint::names::RENDER_VSYNC, "0", &hint::Hint::Default);
    }

    let sdl = sdl3::init().map_err(|error| format!("failed to initialize SDL3: {error}"))?;
    let mut input_state = FrontendInputState::new();
    let audio_output = if options.config.audio.enabled {
        Some(DesktopAudioOutput::new(
            &sdl.audio()
                .map_err(|error| format!("failed to initialize SDL3 audio subsystem: {error}"))?,
            &options.config.audio,
        )?)
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

    let base_window_title = window_title(&rom_path, &options.config);
    let mut frame_pacer = FramePacer::new(options.config.video.vsync);
    let mut performance_counter = PerformanceCounter::new(base_window_title.clone());
    let mut window_builder = video.window(&base_window_title, window_width, window_height);
    window_builder.position_centered();
    if options.config.video.fullscreen {
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
        paused: false,
        input_state,
        audio_output,
        gamepad_manager,
        save_session,
    };

    render_frame(
        &mut canvas,
        &mut texture,
        &mut rgb_frame,
        machine.ppu().framebuffer(),
    )?;

    'running: loop {
        match process_events(&mut event_pump, &options.config, &mut machine, &mut runtime)? {
            LoopSignal::Continue => {}
            LoopSignal::Quit => break 'running,
        }

        if runtime.paused {
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        match step_until_next_frame(&mut event_pump, &options.config, &mut machine, &mut runtime)? {
            LoopSignal::Continue => {}
            LoopSignal::Quit => break 'running,
        }

        render_frame(
            &mut canvas,
            &mut texture,
            &mut rgb_frame,
            machine.ppu().framebuffer(),
        )?;
        performance_counter.record_presented_frame(canvas.window_mut())?;
        frame_pacer.wait_until_next_frame();
    }

    if let Some(save_session) = &mut runtime.save_session {
        save_session.close(&machine)?;
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

fn window_title(rom_path: &Path, config: &DesktopConfig) -> String {
    let rom_name = rom_path
        .file_name()
        .unwrap_or(rom_path.as_os_str())
        .to_string_lossy();
    format!(
        "gb-desktop | {} | {} | {} | {}",
        rom_name,
        config.launch.console_model.name(),
        startup_mode_name(config.launch.startup_mode),
        execution_mode_name(config.launch.execution_mode),
    )
}

fn performance_window_title(
    base_title: &str,
    fps: f64,
    frame_time_ms: f64,
    speed_percent: f64,
) -> String {
    format!("{base_title} | {fps:.1} FPS | {frame_time_ms:.2} ms | {speed_percent:.0}% speed")
}

fn target_frame_rate_hz() -> f64 {
    1.0 / FRAME_DURATION.as_secs_f64()
}

fn process_events(
    event_pump: &mut sdl3::EventPump,
    config: &DesktopConfig,
    machine: &mut Machine<TraceSummaryBuffer>,
    runtime: &mut FrontendRuntime,
) -> Result<LoopSignal, String> {
    for event in event_pump.poll_iter() {
        if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
            gamepad_manager.handle_event(&event, &mut runtime.input_state, machine)?;
        }
        match event {
            Event::Quit { .. } => return Ok(LoopSignal::Quit),
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => return Ok(LoopSignal::Quit),
            Event::KeyDown {
                keycode: Some(keycode),
                repeat,
                ..
            } => {
                if !repeat {
                    match hotkey_action(config, keycode) {
                        HotkeyAction::None => {}
                        HotkeyAction::ManualSave => {
                            if let Some(save_session) = &mut runtime.save_session {
                                let _ = save_session.flush_if_changed(machine, "manual-hotkey")?;
                            }
                        }
                    }

                    if key_matches(config.input.keyboard.hotkeys.pause, keycode) {
                        runtime.paused = !runtime.paused;
                        if let Some(audio_output) = runtime.audio_output.as_ref() {
                            if runtime.paused {
                                audio_output.pause()?;
                            } else {
                                audio_output.resume()?;
                            }
                        }
                    }
                }
                if let Some(button) = joypad_button_for_key(config, keycode) {
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
                if let Some(button) = joypad_button_for_key(config, keycode) {
                    runtime
                        .input_state
                        .set_keyboard_button(machine, button, false);
                }
            }
            _ => {}
        }
    }

    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager.poll_active_gamepad_state(&mut runtime.input_state, machine);
    }

    Ok(LoopSignal::Continue)
}

fn step_until_next_frame(
    event_pump: &mut sdl3::EventPump,
    config: &DesktopConfig,
    machine: &mut Machine<TraceSummaryBuffer>,
    runtime: &mut FrontendRuntime,
) -> Result<LoopSignal, String> {
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    loop {
        match process_events(event_pump, config, machine, runtime)? {
            LoopSignal::Continue => {}
            LoopSignal::Quit => return Ok(LoopSignal::Quit),
        }
        if runtime.paused {
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
                if let Some(save_session) = &mut runtime.save_session
                    && matches!(
                        save_session.flush_policy(),
                        gb_persistence::HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite
                    )
                {
                    let _ = save_session.flush_if_changed(machine, "frame-boundary")?;
                }
                return Ok(LoopSignal::Continue);
            }
            at_frame_origin = now_at_frame_origin;
        }
    }
}

fn render_frame(
    canvas: &mut Canvas<Window>,
    texture: &mut sdl3::render::Texture<'_>,
    rgb_frame: &mut [u8],
    framebuffer: &[u8],
) -> Result<(), String> {
    for (source, target) in framebuffer.iter().zip(rgb_frame.chunks_exact_mut(3)) {
        let shade = framebuffer_pixel_to_grayscale(*source);
        target[0] = shade;
        target[1] = shade;
        target[2] = shade;
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

fn joypad_button_for_key(config: &DesktopConfig, keycode: Keycode) -> Option<JoypadButton> {
    let bindings = &config.input.keyboard.joypad;
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

fn hotkey_action(config: &DesktopConfig, keycode: Keycode) -> HotkeyAction {
    if key_matches(config.input.keyboard.hotkeys.save_battery, keycode) {
        HotkeyAction::ManualSave
    } else {
        HotkeyAction::None
    }
}

fn key_matches(binding: DesktopKey, keycode: Keycode) -> bool {
    match binding {
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
    use super::performance_window_title;

    #[test]
    fn performance_window_title_formats_the_runtime_metrics() {
        assert_eq!(
            performance_window_title(
                "gb-desktop | drmario.gb | dmg | real-boot | strict",
                14.8,
                67.5,
                25.0
            ),
            "gb-desktop | drmario.gb | dmg | real-boot | strict | 14.8 FPS | 67.50 ms | 25% speed"
        );
    }
}
