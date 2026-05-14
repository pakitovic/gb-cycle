use gb_benchmark::{
    BenchmarkCase, BenchmarkMode, BenchmarkModel, BenchmarkPalette, BenchmarkStartup,
    BenchmarkStats, BenchmarkStimulusRuntime, GB_CLI_FRONTEND, encode_stats_toml,
    frontend_screenshot_path, frontend_stats_path, load_benchmark_cases,
    target_frames_for_duration,
};
use gb_core::{
    BootRomAssetError, BootRomAssets, BootRomKind, CartridgeDiagnostic,
    CartridgeDiagnosticSeverity, CartridgeHeader, CartridgeHeaderParseError, CartridgeLoadError,
    CartridgePersistentStateError, CartridgeSelection, CartridgeSlot, CgbFlag, CompatibilityPolicy,
    ConsoleModel, ExecutionMode, JoypadButton, Machine, MachineConfig, MachineSaveState,
    MachineSaveStateRestoreError, PersistentCartState, SgbFlag, StartupMode, TraceBuffer,
    TraceSummaryBuffer, UnsupportedCartridgeCategory,
};
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveBackendError, CartridgeSaveEnvelope, CartridgeSaveKey,
    CartridgeSaveKeyError, CartridgeSaveTimeSource, EXTERNAL_SAVE_FILE_EXTENSION,
    ExternalSaveError, FilesystemCartridgeSaveBackend, FixedCartridgeSaveTimeSource,
    MACHINE_SAVE_STATE_FILE_EXTENSION, MachineSaveStateEnvelope, SystemCartridgeSaveTimeSource,
    decode_machine_save_state_envelope, encode_machine_save_state_envelope,
    export_external_cartridge_save, import_external_cartridge_save, legacy_sanitized_save_key,
    uses_battery_backed_hardware_persistence,
};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

const DEFAULT_SKIP_BOOT_FRAME_LIMIT: u32 = 120;
const DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT: u32 = 120;
const DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT: u32 = 480;
const DEFAULT_BOOT_ROM_ROOT_ENV_VAR: &str = "GB_CYCLE_BOOT_ROM_ROOT";
const FRAMEBUFFER_WIDTH: usize = 160;
const FRAMEBUFFER_HEIGHT: usize = 144;
const DMG_GRAYSCALE_SHADES: [u8; 4] = [255, 170, 85, 0];
const DMG_GREY_DISPLAY_PALETTE: DisplayPalette = DisplayPalette {
    shades: [
        [DMG_GRAYSCALE_SHADES[0]; 3],
        [DMG_GRAYSCALE_SHADES[1]; 3],
        [DMG_GRAYSCALE_SHADES[2]; 3],
        [DMG_GRAYSCALE_SHADES[3]; 3],
    ],
};
const RUN_HELP_TEXT: &str = concat!(
    "Usage:\n",
    "  gb-cli run <rom> [options]\n",
    "\n",
    "Options:\n",
    "  --model <DMG|MGB|LGB|CGB>             Select the console model (default: DMG)\n",
    "  --startup <skip-boot|custom-boot|real-boot> Choose startup path (default: skip-boot)\n",
    "  --mode <strict|permissive|experimental> Set the compatibility policy (default: strict)\n",
    "  --boot-rom-dir <dir>                   Override the boot ROM directory root\n",
    "  --boot-rom-verify <off|warn|strict>    Control DMG boot ROM SHA-256 verification (default: strict)\n",
    "  --test-runner                          Use host-light runner defaults without changing emulated timing\n",
    "  --benchmark <path>                     Run one portable benchmark case TOML\n",
    "  --frames <n>                           Stop after <n> completed frames\n",
    "  --tcycles <n>                          Stop after <n> T-cycles\n",
    "                                         If neither limit is provided, direct boot stops after 120 completed frames\n",
    "                                         and real-boot stops after boot-ROM handoff plus 120 completed frames\n",
    "                                         with a 480-frame safety cap if handoff never arrives\n",
    "  --serial-stdout                        Stream completed serial bytes to stdout as they arrive\n",
    "  --serial-out <path>                    Save completed serial bytes to a file at the end of the run\n",
    "  --framebuffer-out <path>               Save the final 160x144 framebuffer as PGM, or PNG when <path> ends in .png (CGB PNG uses RGB555)\n",
    "  --palette <grey>                       Use the DMG grey framebuffer palette when --model DMG is active\n",
    "  --trace-out <path>                     Save the scheduler trace text for the run\n",
    "  --state-in <path>                      Restore a full-machine .gbstate after loading the ROM\n",
    "  --state-out <path>                     Save a full-machine .gbstate at the end of the run\n",
    "  --save-dir <dir>                       Load/save battery-backed cartridge persistence under this directory\n",
    "  --save-key <key>                       Override the derived save key (default: ROM stem)\n",
    "  --save-policy <manual|on-close|on-write>\n",
    "                                         Select automatic persistence behavior (default: on-close)\n",
);
const INSPECT_HELP_TEXT: &str = concat!(
    "Usage:\n",
    "  gb-cli inspect-rom <rom> [--mode <strict|permissive|experimental>]\n",
    "\n",
    "Options:\n",
    "  --mode <strict|permissive|experimental> Evaluate loader compatibility under the selected mode\n",
);
const SAVES_HELP_TEXT: &str = concat!(
    "Usage:\n",
    "  gb-cli saves export <rom> <out.sav> --save-dir <dir> [--save-key <key>]\n",
    "  gb-cli saves import <rom> <in.sav> --save-dir <dir> [--save-key <key>]\n",
    "\n",
    "Options:\n",
    "  --save-dir <dir>                       Directory containing gb-cycle .gbsav files\n",
    "  --save-key <key>                       Override the derived save key (default: ROM stem)\n",
);

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliAction {
    ShowGeneralHelp,
    ShowRunHelp,
    ShowInspectHelp,
    ShowSavesHelp,
    Run(Box<RunOptions>),
    RunBenchmark(BenchmarkRunOptions),
    InspectRom(InspectRomOptions),
    Saves(SavesOptions),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BootRomVerificationMode {
    Off,
    Warn,
    #[default]
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum RunModel {
    #[default]
    GameBoy,
    Pocket,
    Light,
    Color,
}

impl RunModel {
    fn console_model(self) -> ConsoleModel {
        match self {
            Self::GameBoy => ConsoleModel::GameBoy,
            Self::Pocket => ConsoleModel::GameBoyPocket,
            Self::Light => ConsoleModel::GameBoyLight,
            Self::Color => ConsoleModel::GameBoyColor,
        }
    }

    fn boot_rom_kind(self) -> BootRomKind {
        match self {
            Self::GameBoy => BootRomKind::Dmg,
            Self::Pocket | Self::Light => BootRomKind::Mgb,
            Self::Color => BootRomKind::Cgb,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::GameBoy => "DMG",
            Self::Pocket => "MGB",
            Self::Light => "LGB",
            Self::Color => "CGB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SavePolicy {
    Manual,
    #[default]
    OnClose,
    OnWrite,
}

impl SavePolicy {
    fn name(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::OnClose => "on-close",
            Self::OnWrite => "on-write",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FramebufferOutputFormat {
    Pgm,
    Png,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunDisplayPalette {
    Grey,
}

impl RunDisplayPalette {
    const fn display_palette(self) -> DisplayPalette {
        match self {
            Self::Grey => DMG_GREY_DISPLAY_PALETTE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisplayPalette {
    shades: [[u8; 3]; 4],
}

impl DisplayPalette {
    const fn shade_rgb(self, shade: u8) -> [u8; 3] {
        match shade {
            0..=3 => self.shades[shade as usize],
            _ => self.shades[3],
        }
    }

    fn shade_luma(self, shade: u8) -> u8 {
        self.shade_rgb(shade)[0]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefaultRunBudget {
    SkipBootFrames {
        frame_limit: u32,
    },
    RealBootPostHandoff {
        post_handoff_frame_limit: u32,
        safety_frame_limit: u32,
    },
}

impl DefaultRunBudget {
    fn for_startup_mode(startup_mode: StartupMode) -> Self {
        match startup_mode {
            StartupMode::SkipBoot | StartupMode::CustomBoot => Self::SkipBootFrames {
                frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
            },
            StartupMode::RealBoot => Self::RealBootPostHandoff {
                post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
                safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunOptions {
    rom_path: PathBuf,
    model: RunModel,
    startup_mode: StartupMode,
    execution_mode: ExecutionMode,
    boot_rom_dir: Option<PathBuf>,
    boot_rom_verify: BootRomVerificationMode,
    frame_limit: Option<u32>,
    tcycle_limit: Option<u64>,
    default_run_budget: Option<DefaultRunBudget>,
    serial_stdout: bool,
    serial_out: Option<PathBuf>,
    framebuffer_out: Option<PathBuf>,
    display_palette: Option<RunDisplayPalette>,
    trace_out: Option<PathBuf>,
    state_in: Option<PathBuf>,
    state_out: Option<PathBuf>,
    save_dir: Option<PathBuf>,
    save_key: Option<String>,
    save_policy: SavePolicy,
    test_runner: bool,
    benchmark_case: Option<BenchmarkCase>,
}

impl RunOptions {
    fn default_with_rom(rom_path: PathBuf) -> Self {
        Self {
            rom_path,
            model: RunModel::default(),
            startup_mode: StartupMode::SkipBoot,
            execution_mode: ExecutionMode::Strict,
            boot_rom_dir: None,
            boot_rom_verify: BootRomVerificationMode::Strict,
            frame_limit: None,
            tcycle_limit: None,
            default_run_budget: None,
            serial_stdout: false,
            serial_out: None,
            framebuffer_out: None,
            display_palette: None,
            trace_out: None,
            state_in: None,
            state_out: None,
            save_dir: None,
            save_key: None,
            save_policy: SavePolicy::default(),
            test_runner: false,
            benchmark_case: None,
        }
    }

    fn effective_display_palette(&self) -> Option<DisplayPalette> {
        if self.model == RunModel::GameBoy {
            self.display_palette.map(RunDisplayPalette::display_palette)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BenchmarkRunOptions {
    benchmark_path: PathBuf,
    test_runner: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectRomOptions {
    rom_path: PathBuf,
    execution_mode: ExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SavesDirection {
    Export,
    Import,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SavesOptions {
    direction: SavesDirection,
    rom_path: PathBuf,
    external_save_path: PathBuf,
    save_dir: PathBuf,
    save_key: Option<String>,
}

enum CliMachine {
    Buffered(Machine<TraceBuffer>),
    Summary(Machine<TraceSummaryBuffer>),
}

impl CliMachine {
    fn new(config: MachineConfig, capture_trace: bool) -> Self {
        if capture_trace {
            Self::Buffered(Machine::new(config))
        } else {
            Self::Summary(Machine::new_summary(config))
        }
    }

    fn load_cartridge(
        &mut self,
        rom_bytes: Vec<u8>,
    ) -> Result<Vec<CartridgeDiagnostic>, CartridgeLoadError> {
        match self {
            Self::Buffered(machine) => machine.load_cartridge(rom_bytes),
            Self::Summary(machine) => machine.load_cartridge(rom_bytes),
        }
    }

    fn step_t_cycle(&mut self) {
        match self {
            Self::Buffered(machine) => {
                machine.step_t_cycle();
            }
            Self::Summary(machine) => {
                machine.step_t_cycle();
            }
        }
    }

    fn set_joypad_button_pressed(&mut self, button: JoypadButton, pressed: bool) {
        match self {
            Self::Buffered(machine) => {
                machine.set_joypad_button_pressed(button, pressed);
            }
            Self::Summary(machine) => {
                machine.set_joypad_button_pressed(button, pressed);
            }
        }
    }

    fn take_serial_output_bytes(&mut self) -> Vec<u8> {
        match self {
            Self::Buffered(machine) => machine.take_serial_output_bytes(),
            Self::Summary(machine) => machine.take_serial_output_bytes(),
        }
    }

    fn at_frame_origin(&self) -> bool {
        match self {
            Self::Buffered(machine) => machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
            Self::Summary(machine) => machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
        }
    }

    fn is_boot_rom_mapped(&self) -> bool {
        match self {
            Self::Buffered(machine) => machine.boot().is_boot_rom_mapped(),
            Self::Summary(machine) => machine.boot().is_boot_rom_mapped(),
        }
    }

    fn framebuffer(&self) -> &[u8] {
        match self {
            Self::Buffered(machine) => machine.ppu().framebuffer(),
            Self::Summary(machine) => machine.ppu().framebuffer(),
        }
    }

    fn cgb_framebuffer_rgb555(&self) -> Option<&[u16]> {
        match self {
            Self::Buffered(machine) => machine.ppu().cgb_framebuffer_rgb555(),
            Self::Summary(machine) => machine.ppu().cgb_framebuffer_rgb555(),
        }
    }

    fn cartridge(&self) -> &CartridgeSlot {
        match self {
            Self::Buffered(machine) => machine.cartridge(),
            Self::Summary(machine) => machine.cartridge(),
        }
    }

    fn restore_cartridge_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match self {
            Self::Buffered(machine) => machine.restore_cartridge_persistent_state(state),
            Self::Summary(machine) => machine.restore_cartridge_persistent_state(state),
        }
    }

    fn capture_save_state(&self) -> MachineSaveState {
        match self {
            Self::Buffered(machine) => machine.capture_save_state(),
            Self::Summary(machine) => machine.capture_save_state(),
        }
    }

    fn restore_save_state(
        &mut self,
        state: &MachineSaveState,
    ) -> Result<(), MachineSaveStateRestoreError> {
        match self {
            Self::Buffered(machine) => machine.restore_save_state(state),
            Self::Summary(machine) => machine.restore_save_state(state),
        }
    }

    fn trace_text(&self) -> Option<String> {
        match self {
            Self::Buffered(machine) => Some(machine.tracer().sink().render_text()),
            Self::Summary(_) => None,
        }
    }
}

#[derive(Debug)]
struct SaveSession {
    backend: FilesystemCartridgeSaveBackend,
    key: CartridgeSaveKey,
    last_saved_state: PersistentCartState,
    loaded_existing_save: bool,
    save_writes: usize,
}

impl SaveSession {
    fn save_path(&self) -> PathBuf {
        self.backend.path_for_key(&self.key)
    }
}

fn main() -> ExitCode {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    match run_cli_command(env::args().skip(1), &mut stdout, &mut stderr) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(stderr, "error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn general_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  gb-cli run <rom> [options]\n",
        "  gb-cli inspect-rom <rom> [--mode <strict|permissive|experimental>]\n",
        "  gb-cli saves <export|import> <rom> <save.sav> --save-dir <dir> [--save-key <key>]\n",
        "\n",
        "Commands:\n",
        "  run         Execute one ROM with the headless runner\n",
        "  inspect-rom Parse the cartridge header and report mapper compatibility\n",
        "  saves       Convert gb-cycle .gbsav cartridge persistence to or from external .sav files\n",
        "\n",
        "Run `gb-cli <command> --help` for command-specific options.\n",
    )
}

fn run_cli_command<I, S>(
    arguments: I,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match parse_cli_arguments(arguments)? {
        CliAction::ShowGeneralHelp => write_text(stdout, general_help_text()),
        CliAction::ShowRunHelp => write_text(stdout, RUN_HELP_TEXT),
        CliAction::ShowInspectHelp => write_text(stdout, INSPECT_HELP_TEXT),
        CliAction::ShowSavesHelp => write_text(stdout, SAVES_HELP_TEXT),
        CliAction::Run(options) => run_command(*options, stdout, stderr),
        CliAction::RunBenchmark(options) => run_benchmark_command(options, stdout, stderr),
        CliAction::InspectRom(options) => inspect_rom_command(options, stdout),
        CliAction::Saves(options) => saves_command(options, stdout, stderr),
    }
}

fn parse_cli_arguments<I, S>(arguments: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let Some(command) = arguments.next() else {
        return Ok(CliAction::ShowGeneralHelp);
    };

    match command.as_ref() {
        "--help" | "-h" => Ok(CliAction::ShowGeneralHelp),
        "run" => parse_run_arguments(arguments),
        "inspect-rom" => parse_inspect_rom_arguments(arguments),
        "saves" => parse_saves_arguments(arguments),
        other => Err(format!(
            "unknown subcommand {other:?}; run `gb-cli --help` for usage"
        )),
    }
}

fn parse_run_arguments<I, S>(arguments: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let mut rom_path = None;
    let mut options = None;
    let mut benchmark_path = None;
    let mut test_runner_requested = false;
    let mut save_policy_explicit = false;

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(CliAction::ShowRunHelp),
            "--model" => {
                let Some(value) = arguments.next() else {
                    return Err("--model requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().model = parse_run_model(value.as_ref())?;
            }
            "--startup" => {
                let Some(value) = arguments.next() else {
                    return Err("--startup requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().startup_mode = parse_startup_mode(value.as_ref())?;
            }
            "--mode" => {
                let Some(value) = arguments.next() else {
                    return Err("--mode requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().execution_mode = parse_execution_mode(value.as_ref())?;
            }
            "--boot-rom-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--boot-rom-dir requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().boot_rom_dir = Some(PathBuf::from(value.as_ref()));
            }
            "--boot-rom-verify" => {
                let Some(value) = arguments.next() else {
                    return Err("--boot-rom-verify requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().boot_rom_verify =
                    parse_boot_rom_verification_mode(value.as_ref())?;
            }
            "--test-runner" => {
                test_runner_requested = true;
                if let Some(options) = &mut options {
                    options.test_runner = true;
                }
            }
            "--benchmark" => {
                let Some(value) = arguments.next() else {
                    return Err("--benchmark requires a value".to_string());
                };
                if benchmark_path.is_some() {
                    return Err("--benchmark can only be provided once".to_string());
                }
                benchmark_path = Some(PathBuf::from(value.as_ref()));
            }
            "--frames" => {
                let Some(value) = arguments.next() else {
                    return Err("--frames requires a value".to_string());
                };
                let parsed = parse_positive_u32("--frames", value.as_ref())?;
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().frame_limit = Some(parsed);
            }
            "--tcycles" => {
                let Some(value) = arguments.next() else {
                    return Err("--tcycles requires a value".to_string());
                };
                let parsed = parse_positive_u64("--tcycles", value.as_ref())?;
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().tcycle_limit = Some(parsed);
            }
            "--serial-stdout" => {
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().serial_stdout = true;
            }
            "--serial-out" => {
                let Some(value) = arguments.next() else {
                    return Err("--serial-out requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().serial_out = Some(PathBuf::from(value.as_ref()));
            }
            "--framebuffer-out" => {
                let Some(value) = arguments.next() else {
                    return Err("--framebuffer-out requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().framebuffer_out = Some(PathBuf::from(value.as_ref()));
            }
            "--palette" => {
                let Some(value) = arguments.next() else {
                    return Err("--palette requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().display_palette =
                    Some(parse_display_palette(value.as_ref())?);
            }
            "--trace-out" => {
                let Some(value) = arguments.next() else {
                    return Err("--trace-out requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().trace_out = Some(PathBuf::from(value.as_ref()));
            }
            "--state-in" => {
                let Some(value) = arguments.next() else {
                    return Err("--state-in requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().state_in = Some(PathBuf::from(value.as_ref()));
            }
            "--state-out" => {
                let Some(value) = arguments.next() else {
                    return Err("--state-out requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().state_out = Some(PathBuf::from(value.as_ref()));
            }
            "--save-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-dir requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().save_dir = Some(PathBuf::from(value.as_ref()));
            }
            "--save-key" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-key requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().save_key = Some(value.as_ref().to_string());
            }
            "--save-policy" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-policy requires a value".to_string());
                };
                ensure_run_options_initialized(&mut options, &rom_path)?;
                options.as_mut().unwrap().save_policy = parse_save_policy(value.as_ref())?;
                save_policy_explicit = true;
            }
            value if value.starts_with("--") => {
                return Err(format!(
                    "unknown run option {value:?}; run `gb-cli run --help`"
                ));
            }
            value => {
                if rom_path.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument {value:?}; run `gb-cli run --help`"
                    ));
                }
                rom_path = Some(PathBuf::from(value));
                let mut next_options = RunOptions::default_with_rom(
                    rom_path.clone().expect("rom_path was just assigned"),
                );
                next_options.test_runner = test_runner_requested;
                options = Some(next_options);
            }
        }
    }

    if let Some(benchmark_path) = benchmark_path {
        if rom_path.is_some() {
            return Err("--benchmark supplies the ROM path from the case TOML; omit the positional ROM path".to_string());
        }
        if options.is_some() {
            return Err("--benchmark cannot be combined with normal run options; put benchmark parameters in the case TOML".to_string());
        }
        return Ok(CliAction::RunBenchmark(BenchmarkRunOptions {
            benchmark_path,
            test_runner: test_runner_requested,
        }));
    }

    let mut options = options.ok_or_else(|| {
        "missing required ROM path; run `gb-cli run --help` for usage".to_string()
    })?;
    options.test_runner |= test_runner_requested;
    if options.save_dir.is_none() {
        if options.save_key.is_some() {
            return Err("--save-key requires --save-dir".to_string());
        }
        if save_policy_explicit {
            return Err("--save-policy requires --save-dir".to_string());
        }
    }
    if options.frame_limit.is_none() && options.tcycle_limit.is_none() {
        options.default_run_budget = Some(DefaultRunBudget::for_startup_mode(options.startup_mode));
    }
    if options.model != RunModel::GameBoy {
        options.display_palette = None;
    }

    Ok(CliAction::Run(Box::new(options)))
}

fn parse_inspect_rom_arguments<I, S>(arguments: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let mut rom_path = None;
    let mut execution_mode = ExecutionMode::Strict;

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(CliAction::ShowInspectHelp),
            "--mode" => {
                let Some(value) = arguments.next() else {
                    return Err("--mode requires a value".to_string());
                };
                execution_mode = parse_execution_mode(value.as_ref())?;
            }
            value if value.starts_with("--") => {
                return Err(format!(
                    "unknown inspect-rom option {value:?}; run `gb-cli inspect-rom --help`"
                ));
            }
            value => {
                if rom_path.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument {value:?}; run `gb-cli inspect-rom --help`"
                    ));
                }
                rom_path = Some(PathBuf::from(value));
            }
        }
    }

    Ok(CliAction::InspectRom(InspectRomOptions {
        rom_path: rom_path.ok_or_else(|| {
            "missing required ROM path; run `gb-cli inspect-rom --help` for usage".to_string()
        })?,
        execution_mode,
    }))
}

fn parse_saves_arguments<I, S>(arguments: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let Some(direction) = arguments.next() else {
        return Err("missing saves action; run `gb-cli saves --help` for usage".to_string());
    };
    let direction = match direction.as_ref() {
        "--help" | "-h" => return Ok(CliAction::ShowSavesHelp),
        "export" => SavesDirection::Export,
        "import" => SavesDirection::Import,
        other => {
            return Err(format!(
                "unknown saves action {other:?}; expected export or import"
            ));
        }
    };

    let mut positional = Vec::new();
    let mut save_dir = None;
    let mut save_key = None;

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(CliAction::ShowSavesHelp),
            "--save-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-dir requires a value".to_string());
                };
                save_dir = Some(PathBuf::from(value.as_ref()));
            }
            "--save-key" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-key requires a value".to_string());
                };
                save_key = Some(value.as_ref().to_string());
            }
            value if value.starts_with("--") => {
                return Err(format!(
                    "unknown saves option {value:?}; run `gb-cli saves --help`"
                ));
            }
            value => {
                if positional.len() >= 2 {
                    return Err(format!(
                        "unexpected extra positional argument {value:?}; run `gb-cli saves --help`"
                    ));
                }
                positional.push(PathBuf::from(value));
            }
        }
    }

    if positional.len() != 2 {
        return Err(
            "missing required ROM path or .sav path; run `gb-cli saves --help` for usage"
                .to_string(),
        );
    }

    Ok(CliAction::Saves(SavesOptions {
        direction,
        rom_path: positional.remove(0),
        external_save_path: positional.remove(0),
        save_dir: save_dir.ok_or_else(|| "--save-dir is required".to_string())?,
        save_key,
    }))
}

fn ensure_run_options_initialized(
    options: &mut Option<RunOptions>,
    rom_path: &Option<PathBuf>,
) -> Result<(), String> {
    if options.is_none() {
        if let Some(rom_path) = rom_path {
            *options = Some(RunOptions::default_with_rom(rom_path.clone()));
        } else {
            return Err(
                "the ROM path must be the first positional argument to `gb-cli run`".to_string(),
            );
        }
    }
    Ok(())
}

fn run_benchmark_command(
    options: BenchmarkRunOptions,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let benchmark_path = resolve_path(&current_dir, &options.benchmark_path);
    let benchmark_cases =
        load_benchmark_cases(&benchmark_path).map_err(|error| error.to_string())?;

    for benchmark_case in benchmark_cases {
        run_benchmark_case(benchmark_case, options.test_runner, stdout, stderr)?;
    }

    Ok(())
}

fn run_benchmark_case(
    benchmark_case: BenchmarkCase,
    test_runner: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let framebuffer_out = benchmark_case
        .screenshot
        .then(|| frontend_screenshot_path(GB_CLI_FRONTEND, &benchmark_case.artifact_id));
    let run_options = RunOptions {
        rom_path: benchmark_case.rom.clone(),
        model: run_model_from_benchmark(benchmark_case.model),
        startup_mode: startup_mode_from_benchmark(benchmark_case.startup),
        execution_mode: execution_mode_from_benchmark(benchmark_case.mode),
        boot_rom_dir: None,
        boot_rom_verify: BootRomVerificationMode::Strict,
        frame_limit: Some(target_frames_for_duration(benchmark_case.duration_seconds)),
        tcycle_limit: None,
        default_run_budget: None,
        serial_stdout: false,
        serial_out: None,
        framebuffer_out,
        display_palette: benchmark_case.palette.map(display_palette_from_benchmark),
        trace_out: None,
        state_in: None,
        state_out: None,
        save_dir: None,
        save_key: None,
        save_policy: SavePolicy::Manual,
        test_runner,
        benchmark_case: Some(benchmark_case),
    };

    run_command(run_options, stdout, stderr)
}

fn run_command(
    options: RunOptions,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let rom_path = resolve_path(&current_dir, &options.rom_path);
    let rom_bytes = fs::read(&rom_path)
        .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;

    let boot_rom_assets = load_boot_rom_assets(&options, &current_dir, stderr)?;
    let config = MachineConfig::new(options.model.console_model())
        .with_startup_mode(options.startup_mode)
        .with_boot_rom_kind(options.model.boot_rom_kind())
        .with_compatibility(compatibility_for_execution_mode(options.execution_mode))
        .with_boot_rom_assets(boot_rom_assets);
    let mut machine = CliMachine::new(config, options.trace_out.is_some());
    let diagnostics = machine
        .load_cartridge(rom_bytes)
        .map_err(format_cartridge_load_error)?;
    write_cartridge_diagnostics(stderr, &diagnostics)?;

    let save_root = options
        .save_dir
        .as_ref()
        .map(|path| resolve_path(&current_dir, path));
    if let Some(save_root) = &save_root {
        validate_directory_input("--save-dir", save_root)?;
    }
    let state_in_path = options
        .state_in
        .as_ref()
        .map(|path| resolve_path(&current_dir, path));
    let state_out_path = options
        .state_out
        .as_ref()
        .map(|path| resolve_path(&current_dir, path));

    if let Some(state_in_path) = &state_in_path {
        restore_machine_save_state_from_path(&mut machine, state_in_path)?;
    }
    let mut save_session = open_save_session(
        save_root.as_deref(),
        &options,
        &rom_path,
        &mut machine,
        stderr,
        state_in_path.is_none(),
    )?;

    let frame_limit = options.frame_limit;
    let tcycle_limit = options.tcycle_limit;
    let default_run_budget = options.default_run_budget;
    let mut executed_tcycles = 0_u64;
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.at_frame_origin();
    let mut boot_rom_was_mapped = machine.is_boot_rom_mapped();
    let mut completed_frames_at_boot_handoff = None;
    let mut serial_byte_count = 0_usize;
    let mut serial_capture = options.serial_out.as_ref().map(|_| Vec::new());
    let mut benchmark_stimuli = options
        .benchmark_case
        .as_ref()
        .map(|case| BenchmarkStimulusRuntime::new(case.stimuli.clone()));
    let benchmark_started_at = options.benchmark_case.as_ref().map(|_| Instant::now());

    while !run_limit_reached(
        frame_limit,
        tcycle_limit,
        completed_frames,
        executed_tcycles,
    ) && !default_run_limit_reached(
        default_run_budget,
        completed_frames,
        completed_frames_at_boot_handoff,
    ) {
        if let Some(benchmark_stimuli) = &mut benchmark_stimuli {
            benchmark_stimuli.apply_due(
                executed_tcycles,
                u64::from(completed_frames),
                |button, pressed| {
                    machine.set_joypad_button_pressed(button, pressed);
                },
            );
        }
        machine.step_t_cycle();
        executed_tcycles += 1;

        let boot_rom_is_mapped = machine.is_boot_rom_mapped();
        if completed_frames_at_boot_handoff.is_none() && boot_rom_was_mapped && !boot_rom_is_mapped
        {
            completed_frames_at_boot_handoff = Some(completed_frames);
        }
        boot_rom_was_mapped = boot_rom_is_mapped;

        let serial_bytes = machine.take_serial_output_bytes();
        if !serial_bytes.is_empty() {
            serial_byte_count += serial_bytes.len();
            if options.serial_stdout {
                stdout
                    .write_all(&serial_bytes)
                    .map_err(|error| format!("failed to write serial stdout: {error}"))?;
                stdout
                    .flush()
                    .map_err(|error| format!("failed to flush serial stdout: {error}"))?;
            }
            if let Some(capture) = &mut serial_capture {
                capture.extend_from_slice(&serial_bytes);
            }
        }

        let now_at_frame_origin = machine.at_frame_origin();
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
            if matches!(options.save_policy, SavePolicy::OnWrite)
                && let Some(save_session) = &mut save_session
            {
                flush_save_if_changed(save_session, &machine, "frame-boundary")?;
            }
        }
        at_frame_origin = now_at_frame_origin;
    }
    let benchmark_elapsed = benchmark_started_at.map(|started_at| started_at.elapsed());

    if let Some(serial_out) = &options.serial_out {
        let serial_bytes = serial_capture.as_deref().unwrap_or_default();
        write_bytes_with_parent(serial_out, serial_bytes)?;
    }
    if let Some(framebuffer_out) = &options.framebuffer_out {
        let framebuffer_image = encode_framebuffer_artifact(
            framebuffer_out,
            machine.framebuffer(),
            machine.cgb_framebuffer_rgb555(),
            options.effective_display_palette(),
        )
        .map_err(|error| format_framebuffer_artifact_error(framebuffer_out, error))?;
        write_bytes_with_parent(framebuffer_out, &framebuffer_image)?;
    }
    if let Some(trace_out) = &options.trace_out {
        let Some(trace_text) = machine.trace_text() else {
            return Err("trace output requested without an in-memory trace buffer".to_string());
        };
        write_text_file_with_parent(trace_out, &trace_text)?;
    }
    if let Some(state_out_path) = &state_out_path {
        write_machine_save_state_to_path(&machine, state_out_path)?;
    }
    if let Some(benchmark_case) = options.benchmark_case.as_ref()
        && benchmark_case.stats
    {
        let stats_path = frontend_stats_path(GB_CLI_FRONTEND, &benchmark_case.artifact_id);
        let screenshot_path = benchmark_case
            .screenshot
            .then(|| frontend_screenshot_path(GB_CLI_FRONTEND, &benchmark_case.artifact_id));
        let stats = BenchmarkStats::new(
            GB_CLI_FRONTEND,
            benchmark_case,
            options.test_runner,
            u64::from(completed_frames),
            benchmark_elapsed.unwrap_or_default().as_secs_f64(),
            Some(executed_tcycles),
            screenshot_path.as_deref(),
        );
        let encoded_stats = encode_stats_toml(&stats)
            .map_err(|error| format!("failed to encode benchmark stats TOML: {error}"))?;
        write_text_file_with_parent(&stats_path, &encoded_stats)?;
        writeln_checked(
            stderr,
            &format!("benchmark_stats_out={}", stats_path.display()),
        )?;
    }

    if let Some(save_session) = &mut save_session {
        match options.save_policy {
            SavePolicy::Manual => {}
            SavePolicy::OnClose | SavePolicy::OnWrite => {
                flush_save_if_changed(save_session, &machine, "run-complete")?;
            }
        }
    }

    writeln_checked(stderr, &format!("rom={}", rom_path.display()))?;
    writeln_checked(stderr, &format!("model={}", options.model.name()))?;
    writeln_checked(
        stderr,
        &format!("startup={}", startup_mode_name(options.startup_mode)),
    )?;
    writeln_checked(
        stderr,
        &format!("mode={}", execution_mode_name(options.execution_mode)),
    )?;
    writeln_checked(stderr, &format!("executed_tcycles={executed_tcycles}"))?;
    writeln_checked(stderr, &format!("completed_frames={completed_frames}"))?;
    writeln_checked(stderr, &format!("serial_bytes={serial_byte_count}"))?;
    if let Some(framebuffer_out) = &options.framebuffer_out {
        writeln_checked(
            stderr,
            &format!("framebuffer_out={}", framebuffer_out.display()),
        )?;
    }
    if let Some(trace_out) = &options.trace_out {
        writeln_checked(stderr, &format!("trace_out={}", trace_out.display()))?;
    }
    if let Some(serial_out) = &options.serial_out {
        writeln_checked(stderr, &format!("serial_out={}", serial_out.display()))?;
    }
    if let Some(state_in_path) = &state_in_path {
        writeln_checked(stderr, &format!("state_in={}", state_in_path.display()))?;
    }
    if let Some(state_out_path) = &state_out_path {
        writeln_checked(stderr, &format!("state_out={}", state_out_path.display()))?;
    }
    if let Some(save_session) = &save_session {
        writeln_checked(stderr, &format!("save_key={}", save_session.key.as_str()))?;
        writeln_checked(
            stderr,
            &format!("save_file={}", save_session.save_path().display()),
        )?;
        writeln_checked(
            stderr,
            &format!("save_loaded_existing={}", save_session.loaded_existing_save),
        )?;
        writeln_checked(
            stderr,
            &format!("save_policy={}", options.save_policy.name()),
        )?;
        writeln_checked(stderr, &format!("save_writes={}", save_session.save_writes))?;
    }

    Ok(())
}

fn inspect_rom_command(options: InspectRomOptions, output: &mut dyn Write) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let rom_path = resolve_path(&current_dir, &options.rom_path);
    let rom_bytes = fs::read(&rom_path)
        .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;
    let header = CartridgeHeader::parse(&rom_bytes).map_err(format_header_parse_error)?;
    let compatibility = compatibility_for_execution_mode(options.execution_mode);

    let (load_status, classification, diagnostics, rejection_reason) =
        match CartridgeSlot::load(rom_bytes, &compatibility) {
            Ok(report) => {
                let classification = report
                    .cartridge()
                    .classification()
                    .expect("loaded cartridges should always expose a classification");
                (
                    "ok",
                    classification,
                    report.diagnostics().to_vec(),
                    None::<String>,
                )
            }
            Err(CartridgeLoadError::Rejected {
                classification,
                reason,
                diagnostics,
                ..
            }) => ("rejected", classification, diagnostics, Some(reason)),
            Err(CartridgeLoadError::HeaderParse(error)) => {
                return Err(format_header_parse_error(error));
            }
        };

    writeln_checked(output, &format!("rom={}", rom_path.display()))?;
    writeln_checked(output, &format!("title={}", header.title))?;
    writeln_checked(
        output,
        &format!(
            "execution_mode={}",
            execution_mode_name(options.execution_mode)
        ),
    )?;
    writeln_checked(output, &format!("load_status={load_status}"))?;
    writeln_checked(
        output,
        &format!("cartridge_type=0x{:02X}", header.cartridge_type),
    )?;
    writeln_checked(
        output,
        &format!("mapper_name={}", classification.detected_name()),
    )?;
    writeln_checked(
        output,
        &format!("selection={}", selection_name(classification.selection())),
    )?;
    writeln_checked(
        output,
        &format!("selection_reason={}", classification.reason()),
    )?;
    writeln_checked(
        output,
        &format!("cgb_flag={}", cgb_flag_name(header.cgb_flag)),
    )?;
    writeln_checked(
        output,
        &format!("sgb_flag={}", sgb_flag_name(header.sgb_flag)),
    )?;
    writeln_checked(
        output,
        &format!("rom_size_code=0x{:02X}", header.rom_size.raw_code),
    )?;
    writeln_checked(
        output,
        &format!(
            "rom_size_bytes={}",
            optional_usize_name(header.rom_size.decoded_bytes)
        ),
    )?;
    writeln_checked(
        output,
        &format!(
            "rom_bank_count={}",
            optional_usize_name(header.rom_size.bank_count)
        ),
    )?;
    writeln_checked(
        output,
        &format!("ram_size_code=0x{:02X}", header.ram_size.raw_code),
    )?;
    writeln_checked(
        output,
        &format!(
            "ram_size_bytes={}",
            optional_usize_name(header.ram_size.decoded_bytes)
        ),
    )?;
    writeln_checked(
        output,
        &format!(
            "ram_bank_count={}",
            optional_usize_name(header.ram_size.bank_count)
        ),
    )?;
    writeln_checked(output, &format!("diagnostic_count={}", diagnostics.len()))?;
    for diagnostic in diagnostics {
        writeln_checked(
            output,
            &format!(
                "diagnostic={} {}",
                diagnostic_severity_name(diagnostic.severity),
                diagnostic.message
            ),
        )?;
    }
    if let Some(reason) = rejection_reason {
        writeln_checked(output, &format!("rejection_reason={reason}"))?;
    }

    Ok(())
}

fn saves_command(
    options: SavesOptions,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    match options.direction {
        SavesDirection::Export => saves_export_command(options, stdout, stderr),
        SavesDirection::Import => saves_import_command(options, stdout, stderr),
    }
}

fn saves_export_command(
    options: SavesOptions,
    output: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let rom_path = resolve_path(&current_dir, &options.rom_path);
    let mut cartridge = load_cartridge_for_save_conversion(&rom_path, stderr)?;
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(format!(
            "ROM {} does not expose battery-backed cartridge persistence",
            rom_path.display()
        ));
    }

    let save_root = resolve_path(&current_dir, &options.save_dir);
    validate_directory_input("--save-dir", &save_root)?;
    let key = resolve_saves_key(options.save_key.as_deref(), &rom_path)?;
    let legacy_key = legacy_save_key_for_rom_path(options.save_key.as_deref(), &rom_path);
    let backend = FilesystemCartridgeSaveBackend::new(&save_root);
    let (envelope, source_save_path) =
        load_save_envelope_with_legacy_fallback(&backend, &key, legacy_key.as_ref())?.ok_or_else(
            || {
                format!(
                    "no gb-cycle save found at {}",
                    backend.path_for_key(&key).display()
                )
            },
        )?;
    cartridge
        .restore_persistent_state(&envelope.persistent_state)
        .map_err(|error| {
            format!(
                "save {} is not compatible with ROM {}: {error:?}",
                source_save_path.display(),
                rom_path.display()
            )
        })?;

    let external_bytes = export_external_cartridge_save(&envelope, backend.current_unix_seconds())
        .map_err(format_external_save_error)?;
    let external_path = resolve_path(&current_dir, &options.external_save_path);
    write_bytes_with_parent(&external_path, &external_bytes)?;

    writeln_checked(output, &format!("rom={}", rom_path.display()))?;
    writeln_checked(output, &format!("save_key={}", key.as_str()))?;
    writeln_checked(
        output,
        &format!("source_gbsav={}", source_save_path.display()),
    )?;
    writeln_checked(
        output,
        &format!("external_save={}", external_path.display()),
    )?;
    writeln_checked(output, &format!("external_bytes={}", external_bytes.len()))?;
    Ok(())
}

fn load_save_envelope_with_legacy_fallback(
    backend: &FilesystemCartridgeSaveBackend,
    key: &CartridgeSaveKey,
    legacy_key: Option<&CartridgeSaveKey>,
) -> Result<Option<(CartridgeSaveEnvelope, PathBuf)>, String> {
    let save_path = backend.path_for_key(key);
    if let Some(envelope) = backend
        .load(key)
        .map_err(|error| format_save_load_error(&save_path, error))?
    {
        return Ok(Some((envelope, save_path)));
    }

    let Some(legacy_key) = legacy_key.filter(|legacy_key| *legacy_key != key) else {
        return Ok(None);
    };
    let legacy_save_path = backend.path_for_key(legacy_key);
    backend
        .load(legacy_key)
        .map_err(|error| format_save_load_error(&legacy_save_path, error))
        .map(|envelope| envelope.map(|envelope| (envelope, legacy_save_path)))
}

fn saves_import_command(
    options: SavesOptions,
    output: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let rom_path = resolve_path(&current_dir, &options.rom_path);
    let mut cartridge = load_cartridge_for_save_conversion(&rom_path, stderr)?;
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(format!(
            "ROM {} does not expose battery-backed cartridge persistence",
            rom_path.display()
        ));
    }

    let external_path = resolve_path(&current_dir, &options.external_save_path);
    let external_bytes = fs::read(&external_path).map_err(|error| {
        format!(
            "failed to read external .{} save {}: {error}",
            EXTERNAL_SAVE_FILE_EXTENSION,
            external_path.display()
        )
    })?;
    let target_state = cartridge.persistent_state();
    let save_root = resolve_path(&current_dir, &options.save_dir);
    validate_directory_input("--save-dir", &save_root)?;
    let import_unix_seconds = SystemCartridgeSaveTimeSource.now_unix_seconds();
    let mut backend = FilesystemCartridgeSaveBackend::with_time_source(
        &save_root,
        FixedCartridgeSaveTimeSource::new(import_unix_seconds),
    );
    let imported_state = import_external_cartridge_save(
        metadata,
        &target_state,
        &external_bytes,
        import_unix_seconds,
    )
    .map_err(format_external_save_error)?;
    cartridge
        .restore_persistent_state(&imported_state)
        .map_err(|error| {
            format!(
                "external save {} is not compatible with ROM {}: {error:?}",
                external_path.display(),
                rom_path.display()
            )
        })?;

    let key = resolve_saves_key(options.save_key.as_deref(), &rom_path)?;
    let target_save_path = backend.path_for_key(&key);
    let envelope = backend
        .save(&key, metadata, &imported_state)
        .map_err(|error| format_save_flush_error(&target_save_path, "saves-import", error))?;

    writeln_checked(output, &format!("rom={}", rom_path.display()))?;
    writeln_checked(output, &format!("save_key={}", key.as_str()))?;
    writeln_checked(
        output,
        &format!("external_save={}", external_path.display()),
    )?;
    writeln_checked(
        output,
        &format!("target_gbsav={}", target_save_path.display()),
    )?;
    writeln_checked(
        output,
        &format!(
            "saved_at_unix_seconds={}",
            envelope.backend_metadata.saved_at_unix_seconds
        ),
    )?;
    Ok(())
}

fn load_cartridge_for_save_conversion(
    rom_path: &Path,
    stderr: &mut dyn Write,
) -> Result<CartridgeSlot, String> {
    let rom_bytes = fs::read(rom_path)
        .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;
    let report = CartridgeSlot::load(rom_bytes, &CompatibilityPolicy::strict())
        .map_err(format_cartridge_load_error)?;
    write_cartridge_diagnostics(stderr, report.diagnostics())?;
    Ok(report.cartridge().clone())
}

fn resolve_saves_key(
    explicit_key: Option<&str>,
    rom_path: &Path,
) -> Result<CartridgeSaveKey, String> {
    if let Some(key) = explicit_key {
        parse_save_key(key)
    } else {
        derive_save_key(rom_path)
    }
}

fn open_save_session(
    save_root: Option<&Path>,
    options: &RunOptions,
    rom_path: &Path,
    machine: &mut CliMachine,
    stderr: &mut dyn Write,
    load_existing_save: bool,
) -> Result<Option<SaveSession>, String> {
    let Some(save_root) = save_root else {
        return Ok(None);
    };

    let metadata = machine.cartridge().persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        writeln_checked(stderr, "save=skipped not_battery_backed=true")?;
        return Ok(None);
    }

    let key = if let Some(key) = &options.save_key {
        parse_save_key(key)?
    } else {
        derive_save_key(rom_path)?
    };
    let legacy_key = legacy_save_key_for_rom_path(options.save_key.as_deref(), rom_path);

    let backend = FilesystemCartridgeSaveBackend::new(save_root);
    let mut loaded_existing_save = false;
    let mut last_saved_state = machine.cartridge().persistent_state();

    if load_existing_save
        && let Some((envelope, save_path)) =
            load_save_envelope_with_legacy_fallback(&backend, &key, legacy_key.as_ref())?
    {
        let elapsed_seconds = backend
            .current_unix_seconds()
            .saturating_sub(envelope.backend_metadata.saved_at_unix_seconds);
        let mut restored_state = envelope.persistent_state;
        apply_elapsed_off_session_seconds(&mut restored_state, elapsed_seconds);
        machine
            .restore_cartridge_persistent_state(&restored_state)
            .map_err(|error| format!("failed to restore cartridge persistence: {error:?}"))?;
        last_saved_state = machine.cartridge().persistent_state();
        loaded_existing_save = true;
        writeln_checked(
            stderr,
            &format!(
                "save_loaded path={} elapsed_seconds={elapsed_seconds}",
                save_path.display()
            ),
        )?;
    }

    Ok(Some(SaveSession {
        backend,
        key,
        last_saved_state,
        loaded_existing_save,
        save_writes: 0,
    }))
}

fn flush_save_if_changed(
    save_session: &mut SaveSession,
    machine: &CliMachine,
    reason: &str,
) -> Result<bool, String> {
    let current_state = machine.cartridge().persistent_state();
    if current_state == save_session.last_saved_state {
        return Ok(false);
    }

    save_session
        .backend
        .save(
            &save_session.key,
            machine.cartridge().persistence_metadata(),
            &current_state,
        )
        .map_err(|error| format_save_flush_error(&save_session.save_path(), reason, error))?;
    save_session.last_saved_state = current_state;
    save_session.save_writes += 1;
    Ok(true)
}

fn restore_machine_save_state_from_path(
    machine: &mut CliMachine,
    path: &Path,
) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read .{} state {}: {error}",
            MACHINE_SAVE_STATE_FILE_EXTENSION,
            path.display()
        )
    })?;
    let envelope = decode_machine_save_state_envelope(&bytes)
        .map_err(|error| format_machine_save_state_io_error("decode", path, error))?;
    machine
        .restore_save_state(&envelope.state)
        .map_err(|error| format!("failed to restore state {}: {error}", path.display()))
}

fn write_machine_save_state_to_path(machine: &CliMachine, path: &Path) -> Result<(), String> {
    let envelope = MachineSaveStateEnvelope::new(machine.capture_save_state());
    let bytes = encode_machine_save_state_envelope(&envelope)
        .map_err(|error| format_machine_save_state_io_error("encode", path, error))?;
    write_bytes_with_parent(path, &bytes)
        .map_err(|error| format!("failed to write state {}: {error}", path.display()))
}

fn load_boot_rom_assets(
    options: &RunOptions,
    current_dir: &Path,
    stderr: &mut dyn Write,
) -> Result<BootRomAssets, String> {
    if options.startup_mode != StartupMode::RealBoot {
        return Ok(BootRomAssets::none());
    }

    let Some(root) = resolve_boot_rom_root(options.boot_rom_dir.as_deref(), current_dir) else {
        match options.boot_rom_verify {
            BootRomVerificationMode::Off => {}
            BootRomVerificationMode::Warn => {
                writeln_checked(
                    stderr,
                    &format!(
                        "warning: boot ROM root is not configured; use --boot-rom-dir or set {DEFAULT_BOOT_ROM_ROOT_ENV_VAR}"
                    ),
                )?;
            }
            BootRomVerificationMode::Strict => {
                return Err(format!(
                    "boot ROM root is not configured; use --boot-rom-dir or set {DEFAULT_BOOT_ROM_ROOT_ENV_VAR}"
                ));
            }
        }
        return Ok(BootRomAssets::none());
    };
    validate_explicit_directory_input("--boot-rom-dir", options.boot_rom_dir.as_deref(), &root)?;
    let image_path = root.join(BootRomAssets::filename(options.model.boot_rom_kind()));
    match options.boot_rom_verify {
        BootRomVerificationMode::Off => {}
        BootRomVerificationMode::Warn => {
            if let Err(error) = verify_boot_rom_file(&image_path, options.model.boot_rom_kind()) {
                writeln_checked(stderr, &format!("warning: {error}"))?;
            }
        }
        BootRomVerificationMode::Strict => {
            verify_boot_rom_file(&image_path, options.model.boot_rom_kind())?;
        }
    }

    if !root.is_dir() {
        return Ok(BootRomAssets::none());
    }

    match BootRomAssets::from_directory(&root) {
        Ok(assets) => Ok(assets),
        Err(error) => Err(format_boot_rom_asset_load_error(&root, error)),
    }
}

fn resolve_boot_rom_root(explicit_root: Option<&Path>, current_dir: &Path) -> Option<PathBuf> {
    if let Some(explicit_root) = explicit_root {
        return Some(resolve_path(current_dir, explicit_root));
    }
    if let Some(root) = env::var_os(DEFAULT_BOOT_ROM_ROOT_ENV_VAR) {
        return Some(PathBuf::from(root));
    }
    None
}

fn resolve_path(current_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_dir.join(path)
    }
}

fn validate_explicit_directory_input(
    flag: &str,
    explicit_path: Option<&Path>,
    resolved_path: &Path,
) -> Result<(), String> {
    if explicit_path.is_some() {
        validate_directory_input(flag, resolved_path)?;
    }
    Ok(())
}

fn validate_directory_input(flag: &str, path: &Path) -> Result<(), String> {
    if path.exists() && !path.is_dir() {
        return Err(format!(
            "{flag} expects a directory path: {}",
            path.display()
        ));
    }
    Ok(())
}

fn write_cartridge_diagnostics(
    stderr: &mut dyn Write,
    diagnostics: &[CartridgeDiagnostic],
) -> Result<(), String> {
    for diagnostic in diagnostics {
        writeln_checked(
            stderr,
            &format!(
                "{}: {}",
                diagnostic_severity_name(diagnostic.severity),
                diagnostic.message
            ),
        )?;
    }
    Ok(())
}

fn write_bytes_with_parent(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create directory {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn write_text_file_with_parent(path: &Path, text: &str) -> Result<(), String> {
    write_bytes_with_parent(path, text.as_bytes())
}

fn write_text(writer: &mut dyn Write, text: &str) -> Result<(), String> {
    if let Err(error) = writer.write_all(text.as_bytes()) {
        return Err(format!("failed to write output: {error}"));
    }
    Ok(())
}

fn writeln_checked(writer: &mut dyn Write, line: &str) -> Result<(), String> {
    if let Err(error) = writer.write_all(line.as_bytes()) {
        return Err(format!("failed to write output: {error}"));
    }
    if let Err(error) = writer.write_all(b"\n") {
        return Err(format!("failed to write output: {error}"));
    }
    Ok(())
}

fn run_limit_reached(
    frame_limit: Option<u32>,
    tcycle_limit: Option<u64>,
    completed_frames: u32,
    executed_tcycles: u64,
) -> bool {
    frame_limit.is_some_and(|limit| completed_frames >= limit)
        || tcycle_limit.is_some_and(|limit| executed_tcycles >= limit)
}

fn default_run_limit_reached(
    default_run_budget: Option<DefaultRunBudget>,
    completed_frames: u32,
    completed_frames_at_boot_handoff: Option<u32>,
) -> bool {
    match default_run_budget {
        Some(DefaultRunBudget::SkipBootFrames { frame_limit }) => completed_frames >= frame_limit,
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit,
            safety_frame_limit,
        }) => match completed_frames_at_boot_handoff {
            Some(frames_at_handoff) => {
                completed_frames >= frames_at_handoff.saturating_add(post_handoff_frame_limit)
            }
            None => completed_frames >= safety_frame_limit,
        },
        None => false,
    }
}

fn compatibility_for_execution_mode(execution_mode: ExecutionMode) -> CompatibilityPolicy {
    match execution_mode {
        ExecutionMode::Strict => CompatibilityPolicy::strict(),
        ExecutionMode::Permissive => CompatibilityPolicy::permissive(),
        ExecutionMode::Experimental => CompatibilityPolicy::experimental(),
    }
}

fn run_model_from_benchmark(model: BenchmarkModel) -> RunModel {
    match model {
        BenchmarkModel::Dmg => RunModel::GameBoy,
        BenchmarkModel::Mgb => RunModel::Pocket,
        BenchmarkModel::Lgb => RunModel::Light,
        BenchmarkModel::Cgb => RunModel::Color,
    }
}

fn startup_mode_from_benchmark(startup: BenchmarkStartup) -> StartupMode {
    match startup {
        BenchmarkStartup::SkipBoot => StartupMode::SkipBoot,
        BenchmarkStartup::CustomBoot => StartupMode::CustomBoot,
        BenchmarkStartup::RealBoot => StartupMode::RealBoot,
    }
}

fn execution_mode_from_benchmark(mode: BenchmarkMode) -> ExecutionMode {
    match mode {
        BenchmarkMode::Strict => ExecutionMode::Strict,
        BenchmarkMode::Permissive => ExecutionMode::Permissive,
        BenchmarkMode::Experimental => ExecutionMode::Experimental,
    }
}

fn display_palette_from_benchmark(palette: BenchmarkPalette) -> RunDisplayPalette {
    match palette {
        BenchmarkPalette::Grey => RunDisplayPalette::Grey,
    }
}

fn parse_run_model(value: &str) -> Result<RunModel, String> {
    match value {
        "DMG" => Ok(RunModel::GameBoy),
        "MGB" => Ok(RunModel::Pocket),
        "LGB" => Ok(RunModel::Light),
        "CGB" => Ok(RunModel::Color),
        _ => Err(format!(
            "unsupported --model value {value:?}; expected one of: DMG, MGB, LGB, CGB"
        )),
    }
}

fn parse_display_palette(value: &str) -> Result<RunDisplayPalette, String> {
    match value {
        "grey" => Ok(RunDisplayPalette::Grey),
        _ => Err(format!(
            "unsupported --palette value {value:?}; expected grey"
        )),
    }
}

fn parse_startup_mode(value: &str) -> Result<StartupMode, String> {
    match value {
        "skip-boot" => Ok(StartupMode::SkipBoot),
        "custom-boot" => Ok(StartupMode::CustomBoot),
        "real-boot" => Ok(StartupMode::RealBoot),
        _ => Err(format!(
            "unsupported --startup value {value:?}; expected skip-boot, custom-boot, or real-boot"
        )),
    }
}

fn parse_execution_mode(value: &str) -> Result<ExecutionMode, String> {
    match value {
        "strict" => Ok(ExecutionMode::Strict),
        "permissive" => Ok(ExecutionMode::Permissive),
        "experimental" => Ok(ExecutionMode::Experimental),
        _ => Err(format!(
            "unsupported --mode value {value:?}; expected strict, permissive, or experimental"
        )),
    }
}

fn parse_boot_rom_verification_mode(value: &str) -> Result<BootRomVerificationMode, String> {
    match value {
        "off" => Ok(BootRomVerificationMode::Off),
        "warn" => Ok(BootRomVerificationMode::Warn),
        "strict" => Ok(BootRomVerificationMode::Strict),
        _ => Err(format!(
            "unsupported --boot-rom-verify value {value:?}; expected off, warn, or strict"
        )),
    }
}

fn parse_save_policy(value: &str) -> Result<SavePolicy, String> {
    match value {
        "manual" => Ok(SavePolicy::Manual),
        "on-close" => Ok(SavePolicy::OnClose),
        "on-write" => Ok(SavePolicy::OnWrite),
        _ => Err(format!(
            "unsupported --save-policy value {value:?}; expected manual, on-close, or on-write"
        )),
    }
}

fn parse_positive_u32(flag: &str, value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|error| format!("invalid {flag} value {value:?}: {error}"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be greater than zero"));
    }
    Ok(parsed)
}

fn parse_positive_u64(flag: &str, value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|error| format!("invalid {flag} value {value:?}: {error}"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be greater than zero"));
    }
    Ok(parsed)
}

fn derive_save_key(rom_path: &Path) -> Result<CartridgeSaveKey, String> {
    let stem = rom_path
        .file_stem()
        .or_else(|| rom_path.file_name())
        .ok_or_else(|| {
            format!(
                "could not derive a save key from ROM path {}; use --save-key instead",
                rom_path.display()
            )
        })?
        .to_string_lossy()
        .into_owned();
    parse_save_key(&stem)
        .map_err(|error| format!("could not use ROM stem {stem:?} as save key: {error}"))
}

fn legacy_save_key_for_rom_path(
    explicit_key: Option<&str>,
    rom_path: &Path,
) -> Option<CartridgeSaveKey> {
    if explicit_key.is_some() {
        return None;
    }
    let stem = rom_path
        .file_stem()
        .or_else(|| rom_path.file_name())?
        .to_string_lossy();
    legacy_sanitized_save_key(&stem)
}

fn parse_save_key(key: &str) -> Result<CartridgeSaveKey, String> {
    CartridgeSaveKey::new(key).map_err(format_save_key_error)
}

fn format_save_key_error(error: CartridgeSaveKeyError) -> String {
    error.to_string()
}

fn apply_elapsed_off_session_seconds(state: &mut PersistentCartState, elapsed_seconds: u64) {
    match state {
        PersistentCartState::Mbc3Rtc { rtc } | PersistentCartState::Mbc3RamRtc { rtc, .. } => {
            rtc.apply_elapsed_seconds(elapsed_seconds);
        }
        PersistentCartState::Huc3 { rtc, .. } => rtc.apply_elapsed_seconds(elapsed_seconds),
        _ => {}
    }
}

fn framebuffer_output_format(path: &Path) -> FramebufferOutputFormat {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        FramebufferOutputFormat::Png
    } else {
        FramebufferOutputFormat::Pgm
    }
}

fn encode_framebuffer_artifact(
    path: &Path,
    framebuffer: &[u8],
    cgb_framebuffer_rgb555: Option<&[u16]>,
    display_palette: Option<DisplayPalette>,
) -> io::Result<Vec<u8>> {
    match framebuffer_output_format(path) {
        FramebufferOutputFormat::Pgm => {
            if let Some(display_palette) = display_palette {
                Ok(encode_framebuffer_palette_pgm(framebuffer, display_palette))
            } else {
                Ok(encode_framebuffer_pgm(framebuffer))
            }
        }
        FramebufferOutputFormat::Png => {
            if let Some(cgb_framebuffer_rgb555) = cgb_framebuffer_rgb555 {
                encode_rgb555_framebuffer_png(cgb_framebuffer_rgb555)
            } else if let Some(display_palette) = display_palette {
                encode_framebuffer_palette_png(framebuffer, display_palette)
            } else {
                encode_framebuffer_png(framebuffer)
            }
        }
    }
}

fn encode_framebuffer_pgm(framebuffer: &[u8]) -> Vec<u8> {
    let mut encoded = format!("P5\n{FRAMEBUFFER_WIDTH} {FRAMEBUFFER_HEIGHT}\n3\n").into_bytes();
    encoded.extend_from_slice(framebuffer);
    encoded
}

fn encode_framebuffer_palette_pgm(framebuffer: &[u8], display_palette: DisplayPalette) -> Vec<u8> {
    let mut encoded = format!("P5\n{FRAMEBUFFER_WIDTH} {FRAMEBUFFER_HEIGHT}\n255\n").into_bytes();
    encoded.extend(
        framebuffer
            .iter()
            .map(|pixel| display_palette.shade_luma(*pixel)),
    );
    encoded
}

fn encode_framebuffer_png(framebuffer: &[u8]) -> io::Result<Vec<u8>> {
    let pixels = framebuffer
        .iter()
        .map(|pixel| framebuffer_pixel_to_grayscale(*pixel))
        .collect::<Vec<_>>();
    encode_grayscale_png(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, &pixels)
}

fn encode_framebuffer_palette_png(
    framebuffer: &[u8],
    display_palette: DisplayPalette,
) -> io::Result<Vec<u8>> {
    let pixels = framebuffer
        .iter()
        .map(|pixel| display_palette.shade_rgb(*pixel))
        .collect::<Vec<_>>();
    encode_rgb_png(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, &pixels)
}

fn encode_rgb555_framebuffer_png(framebuffer: &[u16]) -> io::Result<Vec<u8>> {
    let pixels = framebuffer
        .iter()
        .copied()
        .map(rgb555_to_rgb888)
        .collect::<Vec<_>>();
    encode_rgb_png(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, &pixels)
}

fn rgb555_to_rgb888(color: u16) -> [u8; 3] {
    let red = (color & 0x001F) as u8;
    let green = ((color >> 5) & 0x001F) as u8;
    let blue = ((color >> 10) & 0x001F) as u8;
    [
        scale_5_bit_to_8_bit(red),
        scale_5_bit_to_8_bit(green),
        scale_5_bit_to_8_bit(blue),
    ]
}

fn scale_5_bit_to_8_bit(component: u8) -> u8 {
    (component << 3) | (component >> 2)
}

fn encode_grayscale_png(width: usize, height: usize, pixels: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width as u32, height as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(png_encoding_io_error)?;
        writer
            .write_image_data(pixels)
            .map_err(png_encoding_io_error)?;
    }
    Ok(encoded)
}

fn encode_rgb_png(width: usize, height: usize, pixels: &[[u8; 3]]) -> io::Result<Vec<u8>> {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(png_encoding_io_error)?;
        let mut bytes = Vec::with_capacity(pixels.len() * 3);
        for pixel in pixels {
            bytes.extend_from_slice(pixel);
        }
        writer
            .write_image_data(&bytes)
            .map_err(png_encoding_io_error)?;
    }
    Ok(encoded)
}

fn framebuffer_pixel_to_grayscale(pixel: u8) -> u8 {
    match pixel {
        0..=3 => DMG_GRAYSCALE_SHADES[usize::from(pixel)],
        _ => DMG_GRAYSCALE_SHADES[3],
    }
}

fn png_encoding_io_error(source: png::EncodingError) -> io::Error {
    io::Error::other(source.to_string())
}

fn format_framebuffer_artifact_error(path: &Path, error: io::Error) -> String {
    format!(
        "failed to encode framebuffer artifact {}: {error}",
        path.display()
    )
}

fn format_save_load_error(path: &Path, error: CartridgeSaveBackendError) -> String {
    format!("failed to load save {}: {error}", path.display())
}

fn format_save_flush_error(path: &Path, reason: &str, error: CartridgeSaveBackendError) -> String {
    format!(
        "failed to save cartridge persistence ({reason}) to {}: {error}",
        path.display()
    )
}

fn format_machine_save_state_io_error(
    operation: &str,
    path: &Path,
    error: CartridgeSaveBackendError,
) -> String {
    format!(
        "failed to {operation} .{} state {}: {error}",
        MACHINE_SAVE_STATE_FILE_EXTENSION,
        path.display()
    )
}

fn format_external_save_error(error: ExternalSaveError) -> String {
    format!("failed to convert external .{EXTERNAL_SAVE_FILE_EXTENSION} save: {error}")
}

fn format_boot_rom_asset_load_error(root: &Path, error: BootRomAssetError) -> String {
    format!(
        "failed to load boot ROM assets from {}: {error}",
        root.display()
    )
}

fn verify_boot_rom_file(path: &Path, kind: BootRomKind) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read boot ROM asset {:?} at {}: {}",
            kind,
            path.display(),
            error
        )
    })?;
    let actual_sha256 = sha256_hex(&bytes);
    let expected_sha256 = expected_boot_rom_sha256(kind);
    if actual_sha256 != expected_sha256 {
        return Err(format!(
            "boot ROM asset {:?} at {} has unexpected sha256: expected {}, got {}",
            kind,
            path.display(),
            expected_sha256,
            actual_sha256
        ));
    }
    Ok(())
}

fn expected_boot_rom_sha256(kind: BootRomKind) -> &'static str {
    match kind {
        BootRomKind::Dmg0 => "26e71cf01e301e5dc40e987cd2ecbf6d0276245890ac829db2a25323da86818e",
        BootRomKind::Dmg => "cf053eccb4ccafff9e67339d4e78e98dce7d1ed59be819d2a1ba2232c6fce1c7",
        BootRomKind::Mgb => "a8cb5f4f1f16f2573ed2ecd8daedb9c5d1dd2c30a481f9b179b5d725d95eafe2",
        BootRomKind::Cgb0 => "3a307a41689bee99a9a32ea021bf45136906c86b2e4f06c806738398e4f92e45",
        BootRomKind::Cgb => "b4f2e416a35eef52cba161b159c7c8523a92594facb924b3ede0d722867c50c7",
        BootRomKind::CgbE => "c56299bedd56debdbf36442238636bf5887a65c5173b33995682052353804da9",
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn format_cartridge_load_error(error: CartridgeLoadError) -> String {
    match error {
        CartridgeLoadError::HeaderParse(error) => format_header_parse_error(error),
        CartridgeLoadError::Rejected {
            classification,
            execution_mode,
            reason,
            diagnostics,
        } => {
            let mut message = format!(
                "cartridge rejected under {}: mapper={} selection={} reason={}",
                execution_mode_name(execution_mode),
                classification.detected_name(),
                selection_name(classification.selection()),
                reason,
            );
            if !diagnostics.is_empty() {
                let joined = diagnostics
                    .into_iter()
                    .map(|diagnostic| {
                        format!(
                            "{} {}",
                            diagnostic_severity_name(diagnostic.severity),
                            diagnostic.message
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                message.push_str(&format!(" diagnostics=[{joined}]"));
            }
            message
        }
    }
}

fn format_header_parse_error(error: CartridgeHeaderParseError) -> String {
    match error {
        CartridgeHeaderParseError::ImageTooSmall {
            actual_size,
            minimum_size,
        } => format!(
            "ROM image is too small to contain a cartridge header: expected at least {} bytes, got {}",
            minimum_size, actual_size
        ),
    }
}

fn startup_mode_name(startup_mode: StartupMode) -> &'static str {
    match startup_mode {
        StartupMode::SkipBoot => "skip-boot",
        StartupMode::CustomBoot => "custom-boot",
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

fn diagnostic_severity_name(severity: CartridgeDiagnosticSeverity) -> &'static str {
    match severity {
        CartridgeDiagnosticSeverity::Warning => "warning",
        CartridgeDiagnosticSeverity::Error => "error",
    }
}

fn selection_name(selection: CartridgeSelection) -> &'static str {
    match selection {
        CartridgeSelection::Supported(_) => "supported",
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::PlannedVariant) => {
            "unsupported-planned-variant"
        }
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::DocumentedButUnsupported) => {
            "unsupported-documented"
        }
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::ExperimentalHeuristic) => {
            "unsupported-experimental-heuristic"
        }
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::AccessorySpecialCase) => {
            "unsupported-accessory"
        }
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::UnknownCode) => {
            "unsupported-unknown"
        }
    }
}

fn cgb_flag_name(flag: CgbFlag) -> String {
    match flag {
        CgbFlag::None => "none".to_string(),
        CgbFlag::Supported => "supported".to_string(),
        CgbFlag::Only => "only".to_string(),
        CgbFlag::SupportedNonCanonical(value) => {
            format!("supported-noncanonical(0x{value:02X})")
        }
        CgbFlag::Unknown(value) => format!("unknown(0x{value:02X})"),
    }
}

fn sgb_flag_name(flag: SgbFlag) -> String {
    match flag {
        SgbFlag::None => "none".to_string(),
        SgbFlag::Supported => "supported".to_string(),
        SgbFlag::Unknown(value) => format!("unknown(0x{value:02X})"),
    }
}

fn optional_usize_name(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests;
