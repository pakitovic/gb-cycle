use crate::audio_recording::{
    DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ, DesktopAudioRecordingOptions,
};
use gb_core::{ApuRecordedChannel, ExecutionMode, HardwareRevision, SgbVideoStandard, StartupMode};
use gb_desktop::{
    AudioOptions, BootRomVerificationMode, DesktopConfig, DesktopConsoleModel,
    DesktopDisplayPalette, DesktopFrameBlendingMode, DesktopSaveFlushPolicy, GamepadButtonBinding,
    GamepadButtonBindings, GamepadDirectionalSource, GamepadFaceLayout, SaveDirectoryPolicy,
    SaveKeyPolicy, SgbBorderPresentationMode,
};
use gb_persistence::CartridgeSaveKey;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    Run(Box<DesktopRunOptions>),
    ShowHelp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopRunOptions {
    pub rom_path: Option<PathBuf>,
    pub linked_peer_rom_path: Option<PathBuf>,
    pub benchmark_path: Option<PathBuf>,
    pub exit_after_frames: Option<u64>,
    pub config: DesktopConfig,
    pub audio_recording: Option<DesktopAudioRecordingOptions>,
    pub test_runner: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TestRunnerExplicitOverrides {
    saves: bool,
    saves_disabled: bool,
    rewind: bool,
    fullscreen: bool,
    vsync: bool,
    scale: bool,
    audio: bool,
    gamepad: bool,
}

#[cfg(test)]
pub fn parse_cli_arguments<I, S>(arguments: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    parse_cli_arguments_with_base_config(arguments, DesktopConfig::default())
}

pub fn parse_cli_arguments_with_base_config<I, S>(
    arguments: I,
    mut config: DesktopConfig,
) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let mut rom_path = None;
    let mut linked_peer_rom_path = None;
    let mut benchmark_path = None;
    let mut exit_after_frames = None;
    let mut audio_recording_path = None;
    let mut audio_recording_sample_rate_hz = DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ;
    let mut audio_recording_sample_rate_overridden = false;
    let mut audio_recording_stem_channels = Vec::new();
    let mut requested_display_palette = None;
    let mut explicit_revision = None;
    let mut explicit_sgb_video_standard = false;
    let mut test_runner = false;
    let mut test_runner_overrides = TestRunnerExplicitOverrides::default();

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(CliAction::ShowHelp),
            "--test-runner" => {
                test_runner = true;
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
            "--model" => {
                let Some(value) = arguments.next() else {
                    return Err("--model requires a value".to_string());
                };
                config.launch.console_model = parse_console_model(value.as_ref())?;
                if explicit_revision.is_none() {
                    config.launch.normalize_revision_for_model();
                }
            }
            "--revision" => {
                let Some(value) = arguments.next() else {
                    return Err("--revision requires a value".to_string());
                };
                let revision = parse_revision(value.as_ref())?;
                explicit_revision = Some(revision);
                config.launch.revision = revision;
            }
            "--sgb-standard" => {
                let Some(value) = arguments.next() else {
                    return Err("--sgb-standard requires a value".to_string());
                };
                config.launch.sgb_video_standard = parse_sgb_video_standard(value.as_ref())?;
                explicit_sgb_video_standard = true;
            }
            "--startup" => {
                let Some(value) = arguments.next() else {
                    return Err("--startup requires a value".to_string());
                };
                config.launch.startup_mode = parse_startup_mode(value.as_ref())?;
            }
            "--mode" => {
                let Some(value) = arguments.next() else {
                    return Err("--mode requires a value".to_string());
                };
                config.launch.execution_mode = parse_execution_mode(value.as_ref())?;
            }
            "--boot-rom-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--boot-rom-dir requires a value".to_string());
                };
                config.boot_rom.search_path = Some(PathBuf::from(value.as_ref()));
            }
            "--boot-rom-verify" => {
                let Some(value) = arguments.next() else {
                    return Err("--boot-rom-verify requires a value".to_string());
                };
                config.boot_rom.verification = parse_boot_rom_verification_mode(value.as_ref())?;
            }
            "--save-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-dir requires a value".to_string());
                };
                test_runner_overrides.saves = true;
                config.saves.directory_policy =
                    SaveDirectoryPolicy::Custom(PathBuf::from(value.as_ref()));
            }
            "--save-key" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-key requires a value".to_string());
                };
                test_runner_overrides.saves = true;
                config.saves.key_policy = SaveKeyPolicy::Explicit(parse_save_key(value.as_ref())?);
            }
            "--save-policy" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-policy requires a value".to_string());
                };
                test_runner_overrides.saves = true;
                config.saves.flush_policy = parse_save_policy(value.as_ref())?;
            }
            "--no-saves" => {
                test_runner_overrides.saves = true;
                test_runner_overrides.saves_disabled = true;
                config.saves.enabled = false;
            }
            "--no-rewind" => {
                test_runner_overrides.rewind = true;
                config.rewind.enabled = false;
            }
            "--fullscreen" => {
                test_runner_overrides.fullscreen = true;
                config.video.fullscreen = true;
            }
            "--no-vsync" => {
                test_runner_overrides.vsync = true;
                config.video.vsync = false;
            }
            "--palette" => {
                let Some(value) = arguments.next() else {
                    return Err("--palette requires a value".to_string());
                };
                requested_display_palette = Some(parse_display_palette(value.as_ref())?);
            }
            "--frame-blend" => {
                let Some(value) = arguments.next() else {
                    return Err("--frame-blend requires a value".to_string());
                };
                config.video.frame_blending = parse_frame_blending_mode(value.as_ref())?;
            }
            "--scale" => {
                let Some(value) = arguments.next() else {
                    return Err("--scale requires a value".to_string());
                };
                test_runner_overrides.scale = true;
                config.video.window_scale = parse_positive_u8("--scale", value.as_ref())?;
            }
            "--mute" => {
                test_runner_overrides.audio = true;
                config.audio = AudioOptions {
                    enabled: false,
                    ..config.audio
                };
            }
            "--audio-record" => {
                let Some(value) = arguments.next() else {
                    return Err("--audio-record requires a value".to_string());
                };
                audio_recording_path = Some(PathBuf::from(value.as_ref()));
            }
            "--audio-record-rate" => {
                let Some(value) = arguments.next() else {
                    return Err("--audio-record-rate requires a value".to_string());
                };
                audio_recording_sample_rate_overridden = true;
                audio_recording_sample_rate_hz =
                    parse_positive_u32("--audio-record-rate", value.as_ref())?;
            }
            "--audio-record-stems" => {
                let Some(value) = arguments.next() else {
                    return Err("--audio-record-stems requires a value".to_string());
                };
                audio_recording_stem_channels = parse_audio_recording_stems(value.as_ref())?;
            }
            "--link-rom" => {
                let Some(value) = arguments.next() else {
                    return Err("--link-rom requires a value".to_string());
                };
                linked_peer_rom_path = Some(PathBuf::from(value.as_ref()));
            }
            "--exit-after-frames" => {
                let Some(value) = arguments.next() else {
                    return Err("--exit-after-frames requires a value".to_string());
                };
                exit_after_frames =
                    Some(parse_positive_u64("--exit-after-frames", value.as_ref())?);
            }
            "--no-gamepad" => {
                test_runner_overrides.gamepad = true;
                config.input.gamepad.enabled = false;
            }
            "--gamepad-direction" => {
                let Some(value) = arguments.next() else {
                    return Err("--gamepad-direction requires a value".to_string());
                };
                test_runner_overrides.gamepad = true;
                config.input.gamepad.directional_source =
                    parse_gamepad_directional_source(value.as_ref())?;
            }
            "--gamepad-face-layout" => {
                let Some(value) = arguments.next() else {
                    return Err("--gamepad-face-layout requires a value".to_string());
                };
                test_runner_overrides.gamepad = true;
                config
                    .input
                    .gamepad
                    .bindings
                    .apply_face_layout(parse_gamepad_face_layout(value.as_ref())?);
            }
            "--gamepad-preferred-name" => {
                let Some(value) = arguments.next() else {
                    return Err("--gamepad-preferred-name requires a value".to_string());
                };
                test_runner_overrides.gamepad = true;
                config.input.gamepad.preferred_device.name = Some(parse_non_empty_text(
                    "--gamepad-preferred-name",
                    value.as_ref(),
                )?);
            }
            "--gamepad-preferred-path" => {
                let Some(value) = arguments.next() else {
                    return Err("--gamepad-preferred-path requires a value".to_string());
                };
                test_runner_overrides.gamepad = true;
                config.input.gamepad.preferred_device.path = Some(parse_non_empty_text(
                    "--gamepad-preferred-path",
                    value.as_ref(),
                )?);
            }
            value if value.starts_with("--gamepad-bind-") => {
                let Some(binding_slot) = value.strip_prefix("--gamepad-bind-") else {
                    unreachable!("checked prefix above");
                };
                let Some(binding_value) = arguments.next() else {
                    return Err(format!("{value} requires a value"));
                };
                test_runner_overrides.gamepad = true;
                apply_gamepad_binding_override(
                    &mut config.input.gamepad.bindings,
                    binding_slot,
                    parse_gamepad_button_binding(binding_value.as_ref())?,
                )?;
            }
            value if value.starts_with("--") => {
                return Err(format!("unknown option {value:?}; run `gb-desktop --help`"));
            }
            value => {
                if rom_path.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument {value:?}; run `gb-desktop --help`"
                    ));
                }
                rom_path = Some(PathBuf::from(value));
            }
        }
    }

    if linked_peer_rom_path.is_some() && rom_path.is_none() {
        return Err("--link-rom requires a primary ROM positional argument".to_string());
    }
    if benchmark_path.is_some() && rom_path.is_some() {
        return Err(
            "--benchmark supplies the ROM path from the case TOML; omit the positional ROM path"
                .to_string(),
        );
    }
    validate_model_axes(&config, explicit_sgb_video_standard)?;
    config.launch.normalize_revision_for_model();

    if config.launch.console_model == DesktopConsoleModel::GameBoy
        && let Some(display_palette) = requested_display_palette
    {
        config.video.display_palette = display_palette;
    }
    if test_runner {
        apply_test_runner_defaults(&mut config, test_runner_overrides);
    }

    let audio_recording = match audio_recording_path {
        Some(output_path) => Some(DesktopAudioRecordingOptions {
            output_path,
            sample_rate_hz: audio_recording_sample_rate_hz,
            stem_channels: audio_recording_stem_channels,
        }),
        None => {
            if audio_recording_sample_rate_overridden {
                return Err(
                    "--audio-record-rate requires --audio-record <path> to enable recording"
                        .to_string(),
                );
            }
            if !audio_recording_stem_channels.is_empty() {
                return Err(
                    "--audio-record-stems requires --audio-record <path> to enable recording"
                        .to_string(),
                );
            }
            None
        }
    };

    Ok(CliAction::Run(Box::new(DesktopRunOptions {
        rom_path,
        linked_peer_rom_path,
        benchmark_path,
        exit_after_frames,
        config,
        audio_recording,
        test_runner,
    })))
}

fn validate_model_axes(
    config: &DesktopConfig,
    explicit_sgb_video_standard: bool,
) -> Result<(), String> {
    let console_model = config.launch.console_model.console_model();
    let host_platform = config.launch.console_model.host_platform();
    if !console_model.supports_revision_on_host(host_platform, config.launch.revision) {
        return Err(format!(
            "--revision {} is not supported by --model {}; expected one of: {}",
            revision_argument_name(config.launch.revision),
            config.launch.console_model.name(),
            supported_revision_names(config.launch.console_model)
        ));
    }
    if explicit_sgb_video_standard
        && config.launch.console_model != DesktopConsoleModel::SuperGameBoy
    {
        return Err("--sgb-standard requires --model SGB".to_string());
    }

    Ok(())
}

fn apply_test_runner_defaults(config: &mut DesktopConfig, explicit: TestRunnerExplicitOverrides) {
    config.launch.execution_mode = ExecutionMode::Permissive;
    if config.launch.console_model == DesktopConsoleModel::GameBoy {
        config.video.display_palette = DesktopDisplayPalette::Grey;
    }
    config.video.sgb_border = SgbBorderPresentationMode::Off;
    if !explicit.saves {
        config.saves.enabled = false;
    } else if !explicit.saves_disabled {
        config.saves.enabled = true;
    }
    if !explicit.rewind {
        config.rewind.enabled = false;
    }
    if !explicit.fullscreen {
        config.video.fullscreen = false;
    }
    if !explicit.vsync {
        config.video.vsync = false;
    }
    if !explicit.scale {
        config.video.window_scale = 1;
    }
    if !explicit.audio {
        config.audio = AudioOptions {
            enabled: false,
            ..config.audio
        };
    }
    if !explicit.gamepad {
        config.input.gamepad.enabled = false;
    }
    config.video.show_performance_hud = false;
}

pub fn help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  gb-desktop [rom] [options]\n",
        "\n",
        "Options:\n",
        "  --model <DMG|MGB|LGB|CGB|AGB|SGB|SGB2> Select the console model/profile (default: DMG)\n",
        "  --revision <dmg-cpu-0|dmg-cpu-c|cpu-mgb|cpu-cgb-0|cpu-cgb-c|cpu-cgb-d|cpu-cgb-e|cpu-agb-a>\n",
        "                                         Select the active hardware revision for --model\n",
        "  --sgb-standard <ntsc|pal>             Select the original SGB video standard (requires --model SGB)\n",
        "  --startup <skip-boot|custom-boot|real-boot> Choose startup path (default: skip-boot)\n",
        "  --mode <strict|permissive|experimental> Set the compatibility policy (default: strict)\n",
        "  --boot-rom-dir <dir>                   Override the boot ROM directory root\n",
        "  --boot-rom-verify <off|warn|strict>    Control boot ROM SHA-256 verification (default: strict)\n",
        "  --test-runner                          Use host-light runner defaults: permissive mode, DMG grey palette, and BORDER OFF\n",
        "  --benchmark <path>                     Run one portable benchmark case TOML\n",
        "  --save-dir <dir>                       Override the battery-save directory\n",
        "  --save-key <key>                       Override the derived save key (default: ROM stem)\n",
        "  --save-policy <manual|on-close|on-write|debounced>\n",
        "                                         Select battery-save flushing (default: debounced)\n",
        "  --no-saves                             Disable battery-save load/save\n",
        "  --no-rewind                            Disable desktop rewind capture for this run\n",
        "  --scale <n>                            Set the initial window scale (default: 4)\n",
        "  --fullscreen                           Start in fullscreen mode\n",
        "  --no-vsync                             Disable presentation vsync hint\n",
        "  --palette <grey>                       Use the DMG grey display palette when --model DMG is active\n",
        "  --frame-blend <off|on>                 Simulate LCD frame persistence in desktop presentation\n",
        "  --mute                                 Start with audio disabled\n",
        "  --audio-record <path.wav|path.aifc>    Record direct stereo APU output to WAV/AIFC (pre-mute/pre-volume)\n",
        "  --audio-record-rate <hz>               Override the recording sample rate (default: 96000)\n",
        "  --audio-record-stems <all|ch1,ch2,ch3,ch4>\n",
        "                                         Record isolated channel sidecars with solo DC-blocking after NR51/NR50\n",
        "  --link-rom <path>                      Start a local linked DMG-04 session with this secondary ROM\n",
        "  --exit-after-frames <n>                Exit automatically after presenting n emulated frames\n",
        "  --no-gamepad                           Disable SDL gamepad input\n",
        "  --gamepad-direction <dpad-only|left-stick|both>\n",
        "                                         Choose which controller directions drive the joypad (default: both)\n",
        "  --gamepad-face-layout <east-a|south-a>\n",
        "                                         Apply a face-button preset for A/B (default: east-a)\n",
        "  --gamepad-bind-<up|down|left|right|a|b|select|start> <button>\n",
        "                                         Remap any GB gamepad button to a standard SDL button or trigger name\n",
        "                                         Button names: south, east, west, north, back, start, guide, left-shoulder,\n",
        "                                         right-shoulder, left-trigger, right-trigger, left-stick-click,\n",
        "                                         right-stick-click, dpad-up, dpad-down, dpad-left, dpad-right, misc1\n",
        "  --gamepad-preferred-name <name>\n",
        "                                         Prefer a gamepad with the exact SDL device name when choosing the active pad\n",
        "  --gamepad-preferred-path <path>\n",
        "                                         Prefer a gamepad with the exact SDL device path; falls back to name if both are set\n",
        "\n",
        "Environment:\n",
        "  GB_CYCLE_DESKTOP_SETTINGS_PATH         Override the persisted desktop settings file location\n",
        "  GB_CYCLE_DESKTOP_AUDIO_LOG             Emit opt-in SDL audio telemetry to stderr (use 1 for events, verbose for per-submit logs)\n",
        "  GB_CYCLE_DESKTOP_AUDIO_DISABLE_AUTO_CLEAR Disable automatic oversized SDL audio queue clears for investigation\n",
        "  GB_CYCLE_DESKTOP_EMU_PROFILE           Emit opt-in sampled emulation breakdowns to stderr (use 1/summary or summary:<frames>)\n",
        "  GB_CYCLE_DESKTOP_TRACE_PATH            Write a rolling per-T-cycle CPU/APU debug trace to this path on exit\n",
        "  GB_CYCLE_DESKTOP_TRACE_T_CYCLES        Override the rolling trace window length in T-cycles (default: 8192)\n",
        "  GB_CYCLE_DESKTOP_CGB_IR_TRACE_PATH     Write a CGB IR RP/status transition trace to this path on exit\n",
        "  GB_CYCLE_DESKTOP_CGB_IR_TRACE_WATCH_ADDRESSES Add watched CPU addresses to the CGB IR trace (comma-separated hex)\n",
        "  GB_CYCLE_DESKTOP_CGB_IR_TRACE_TRIGGER_ADDRESSES Limit watched-address triggers while still rendering all watched values\n",
        "  GB_CYCLE_DESKTOP_CGB_IR_TRACE_EVENTS   Override the CGB IR trace event window length (default: 16384)\n",
        "  GB_CYCLE_DESKTOP_CGB_IR_OPTICAL_DELAY_T_CYCLES Override the provisional CGB IR optical edge delay for investigation\n",
        "  GB_CYCLE_DESKTOP_CH4_NR43_TRACE_PATH   Write a condensed CH4/NR43 live-write trace to this path on exit\n",
        "\n",
        "If no ROM is provided, gb-desktop opens without a cartridge and starts in the in-window menu.\n",
    )
}

fn parse_console_model(value: &str) -> Result<DesktopConsoleModel, String> {
    match value {
        "DMG" => Ok(DesktopConsoleModel::GameBoy),
        "MGB" => Ok(DesktopConsoleModel::GameBoyPocket),
        "LGB" => Ok(DesktopConsoleModel::GameBoyLight),
        "CGB" => Ok(DesktopConsoleModel::GameBoyColor),
        "AGB" => Ok(DesktopConsoleModel::GameBoyAdvance),
        "SGB" => Ok(DesktopConsoleModel::SuperGameBoy),
        "SGB2" => Ok(DesktopConsoleModel::SuperGameBoy2),
        _ => Err(format!(
            "unsupported --model value {value:?}; expected one of: DMG, MGB, LGB, CGB, AGB, SGB, SGB2"
        )),
    }
}

fn parse_revision(value: &str) -> Result<HardwareRevision, String> {
    match value {
        "dmg-cpu-0" => Ok(HardwareRevision::DmgCpu0),
        "dmg-cpu-c" => Ok(HardwareRevision::DmgCpuC),
        "cpu-mgb" => Ok(HardwareRevision::CpuMgb),
        "cpu-cgb-0" => Ok(HardwareRevision::CpuCgb0),
        "cpu-cgb-c" => Ok(HardwareRevision::CpuCgbC),
        "cpu-cgb-d" => Ok(HardwareRevision::CpuCgbD),
        "cpu-cgb-e" => Ok(HardwareRevision::CpuCgbE),
        "cpu-agb-a" => Ok(HardwareRevision::CpuAgbA),
        _ => Err(format!(
            "unsupported --revision value {value:?}; expected dmg-cpu-0, dmg-cpu-c, cpu-mgb, cpu-cgb-0, cpu-cgb-c, cpu-cgb-d, cpu-cgb-e, or cpu-agb-a"
        )),
    }
}

fn parse_sgb_video_standard(value: &str) -> Result<SgbVideoStandard, String> {
    match value {
        "ntsc" => Ok(SgbVideoStandard::Ntsc),
        "pal" => Ok(SgbVideoStandard::Pal),
        _ => Err(format!(
            "unsupported --sgb-standard value {value:?}; expected ntsc or pal"
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

fn revision_argument_name(revision: HardwareRevision) -> &'static str {
    match revision {
        HardwareRevision::DmgCpu0 => "dmg-cpu-0",
        HardwareRevision::DmgCpuA => "dmg-cpu-a",
        HardwareRevision::DmgCpuB => "dmg-cpu-b",
        HardwareRevision::DmgCpuC => "dmg-cpu-c",
        HardwareRevision::CpuMgb => "cpu-mgb",
        HardwareRevision::CpuCgb0 => "cpu-cgb-0",
        HardwareRevision::CpuCgbA => "cpu-cgb-a",
        HardwareRevision::CpuCgbB => "cpu-cgb-b",
        HardwareRevision::CpuCgbC => "cpu-cgb-c",
        HardwareRevision::CpuCgbD => "cpu-cgb-d",
        HardwareRevision::CpuCgbE => "cpu-cgb-e",
        HardwareRevision::CpuAgbA => "cpu-agb-a",
    }
}

fn supported_revision_names(console_model: DesktopConsoleModel) -> String {
    console_model
        .active_revisions()
        .iter()
        .map(|revision| revision_argument_name(*revision))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_display_palette(value: &str) -> Result<DesktopDisplayPalette, String> {
    match value {
        "grey" => Ok(DesktopDisplayPalette::Grey),
        _ => Err(format!(
            "unsupported --palette value {value:?}; expected grey"
        )),
    }
}

fn parse_frame_blending_mode(value: &str) -> Result<DesktopFrameBlendingMode, String> {
    match value {
        "off" => Ok(DesktopFrameBlendingMode::Off),
        "on" => Ok(DesktopFrameBlendingMode::On),
        _ => Err(format!(
            "unsupported --frame-blend value {value:?}; expected off or on"
        )),
    }
}

fn parse_save_policy(value: &str) -> Result<DesktopSaveFlushPolicy, String> {
    match value {
        "manual" => Ok(DesktopSaveFlushPolicy::Manual),
        "on-close" => Ok(DesktopSaveFlushPolicy::OnClose),
        "on-write" => Ok(DesktopSaveFlushPolicy::OnWrite),
        "debounced" => Ok(DesktopSaveFlushPolicy::Debounced),
        _ => Err(format!(
            "unsupported --save-policy value {value:?}; expected manual, on-close, on-write, or debounced"
        )),
    }
}

fn parse_gamepad_directional_source(value: &str) -> Result<GamepadDirectionalSource, String> {
    match value {
        "dpad-only" => Ok(GamepadDirectionalSource::DpadOnly),
        "left-stick" => Ok(GamepadDirectionalSource::LeftStickOnly),
        "both" => Ok(GamepadDirectionalSource::DpadAndLeftStick),
        _ => Err(format!(
            "unsupported --gamepad-direction value {value:?}; expected dpad-only, left-stick, or both"
        )),
    }
}

fn parse_gamepad_face_layout(value: &str) -> Result<GamepadFaceLayout, String> {
    match value {
        "east-a" => Ok(GamepadFaceLayout::EastASouthB),
        "south-a" => Ok(GamepadFaceLayout::SouthAEastB),
        _ => Err(format!(
            "unsupported --gamepad-face-layout value {value:?}; expected east-a or south-a"
        )),
    }
}

fn parse_gamepad_button_binding(value: &str) -> Result<GamepadButtonBinding, String> {
    match value {
        "south" => Ok(GamepadButtonBinding::South),
        "east" => Ok(GamepadButtonBinding::East),
        "west" => Ok(GamepadButtonBinding::West),
        "north" => Ok(GamepadButtonBinding::North),
        "back" => Ok(GamepadButtonBinding::Back),
        "start" => Ok(GamepadButtonBinding::Start),
        "guide" => Ok(GamepadButtonBinding::Guide),
        "left-shoulder" => Ok(GamepadButtonBinding::LeftShoulder),
        "right-shoulder" => Ok(GamepadButtonBinding::RightShoulder),
        "left-trigger" => Ok(GamepadButtonBinding::LeftTrigger),
        "right-trigger" => Ok(GamepadButtonBinding::RightTrigger),
        "left-stick-click" => Ok(GamepadButtonBinding::LeftStickClick),
        "right-stick-click" => Ok(GamepadButtonBinding::RightStickClick),
        "dpad-up" => Ok(GamepadButtonBinding::DPadUp),
        "dpad-down" => Ok(GamepadButtonBinding::DPadDown),
        "dpad-left" => Ok(GamepadButtonBinding::DPadLeft),
        "dpad-right" => Ok(GamepadButtonBinding::DPadRight),
        "misc1" => Ok(GamepadButtonBinding::Misc1),
        _ => Err(format!(
            "unsupported gamepad binding value {value:?}; expected a standard SDL button or trigger name"
        )),
    }
}

fn apply_gamepad_binding_override(
    bindings: &mut GamepadButtonBindings,
    slot: &str,
    binding: GamepadButtonBinding,
) -> Result<(), String> {
    match slot {
        "up" => bindings.up = binding,
        "down" => bindings.down = binding,
        "left" => bindings.left = binding,
        "right" => bindings.right = binding,
        "a" => bindings.a = binding,
        "b" => bindings.b = binding,
        "select" => bindings.select = binding,
        "start" => bindings.start = binding,
        _ => {
            return Err(format!(
                "unsupported gamepad binding slot {slot:?}; expected up, down, left, right, a, b, select, or start"
            ));
        }
    }

    Ok(())
}

fn parse_non_empty_text(flag_name: &str, value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{flag_name} requires a non-empty value"));
    }

    Ok(trimmed.to_string())
}

fn parse_save_key(value: &str) -> Result<CartridgeSaveKey, String> {
    CartridgeSaveKey::new(value.to_string()).map_err(|error| error.to_string())
}

fn parse_positive_u8(flag: &str, value: &str) -> Result<u8, String> {
    let parsed = value
        .parse::<u8>()
        .map_err(|error| format!("invalid {flag} value {value:?}: {error}"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be greater than zero"));
    }
    Ok(parsed)
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

fn parse_audio_recording_stems(value: &str) -> Result<Vec<ApuRecordedChannel>, String> {
    if value == "all" {
        return Ok(ApuRecordedChannel::ALL.to_vec());
    }

    let mut channels = Vec::new();
    for token in value.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(
                "invalid --audio-record-stems value; expected all or a comma-separated list of ch1,ch2,ch3,ch4"
                    .to_string(),
            );
        }

        let channel = match trimmed {
            "ch1" => ApuRecordedChannel::Ch1,
            "ch2" => ApuRecordedChannel::Ch2,
            "ch3" => ApuRecordedChannel::Ch3,
            "ch4" => ApuRecordedChannel::Ch4,
            _ => {
                return Err(format!(
                    "unsupported --audio-record-stems entry {trimmed:?}; expected all or a comma-separated list of ch1,ch2,ch3,ch4"
                ));
            }
        };

        if channels.contains(&channel) {
            return Err(format!(
                "duplicate --audio-record-stems entry {trimmed:?}; expected each channel at most once"
            ));
        }

        channels.push(channel);
    }

    if channels.is_empty() {
        return Err(
            "invalid --audio-record-stems value; expected all or a comma-separated list of ch1,ch2,ch3,ch4"
                .to_string(),
        );
    }

    Ok(channels)
}

#[cfg(test)]
mod test;
