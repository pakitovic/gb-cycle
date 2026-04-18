use gb_core::{ExecutionMode, StartupMode};
use gb_desktop::{
    AudioOptions, BootRomVerificationMode, DesktopConfig, DesktopConsoleModel,
    DesktopSaveFlushPolicy, GamepadButtonBinding, GamepadButtonBindings, GamepadDirectionalSource,
    GamepadFaceLayout, SaveDirectoryPolicy, SaveKeyPolicy,
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
    pub exit_after_frames: Option<u64>,
    pub config: DesktopConfig,
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
    let mut exit_after_frames = None;

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(CliAction::ShowHelp),
            "--model" => {
                let Some(value) = arguments.next() else {
                    return Err("--model requires a value".to_string());
                };
                config.launch.console_model = parse_console_model(value.as_ref())?;
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
                config.saves.directory_policy =
                    SaveDirectoryPolicy::Custom(PathBuf::from(value.as_ref()));
            }
            "--save-key" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-key requires a value".to_string());
                };
                config.saves.key_policy = SaveKeyPolicy::Explicit(parse_save_key(value.as_ref())?);
            }
            "--save-policy" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-policy requires a value".to_string());
                };
                config.saves.flush_policy = parse_save_policy(value.as_ref())?;
            }
            "--no-saves" => {
                config.saves.enabled = false;
            }
            "--fullscreen" => {
                config.video.fullscreen = true;
            }
            "--no-vsync" => {
                config.video.vsync = false;
            }
            "--scale" => {
                let Some(value) = arguments.next() else {
                    return Err("--scale requires a value".to_string());
                };
                config.video.window_scale = parse_positive_u8("--scale", value.as_ref())?;
            }
            "--mute" => {
                config.audio = AudioOptions {
                    enabled: false,
                    ..config.audio
                };
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
                config.input.gamepad.enabled = false;
            }
            "--gamepad-direction" => {
                let Some(value) = arguments.next() else {
                    return Err("--gamepad-direction requires a value".to_string());
                };
                config.input.gamepad.directional_source =
                    parse_gamepad_directional_source(value.as_ref())?;
            }
            "--gamepad-face-layout" => {
                let Some(value) = arguments.next() else {
                    return Err("--gamepad-face-layout requires a value".to_string());
                };
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
                config.input.gamepad.preferred_device.name = Some(parse_non_empty_text(
                    "--gamepad-preferred-name",
                    value.as_ref(),
                )?);
            }
            "--gamepad-preferred-path" => {
                let Some(value) = arguments.next() else {
                    return Err("--gamepad-preferred-path requires a value".to_string());
                };
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

    Ok(CliAction::Run(Box::new(DesktopRunOptions {
        rom_path,
        linked_peer_rom_path,
        exit_after_frames,
        config,
    })))
}

pub fn help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  gb-desktop [rom] [options]\n",
        "\n",
        "Options:\n",
        "  --model <dmg0|dmg|mgb>                 Select the DMG-family startup model (default: dmg)\n",
        "  --startup <skip-boot|real-boot>        Choose startup path (default: skip-boot)\n",
        "  --mode <strict|permissive|experimental> Set the compatibility policy (default: strict)\n",
        "  --boot-rom-dir <dir>                   Override the boot ROM directory root\n",
        "  --boot-rom-verify <off|warn|strict>    Control DMG boot ROM SHA-256 verification (default: strict)\n",
        "  --save-dir <dir>                       Override the battery-save directory\n",
        "  --save-key <key>                       Override the derived save key (ASCII alnum, '_' or '-')\n",
        "  --save-policy <manual|on-close|on-write|debounced>\n",
        "                                         Select battery-save flushing (default: debounced)\n",
        "  --no-saves                             Disable battery-save load/save\n",
        "  --scale <n>                            Set the initial window scale (default: 4)\n",
        "  --fullscreen                           Start in fullscreen mode\n",
        "  --no-vsync                             Disable presentation vsync hint\n",
        "  --mute                                 Start with audio disabled\n",
        "  --link-rom <path>                      Start a local linked DMG-04 session with this secondary ROM\n",
        "  --exit-after-frames <n>                Exit automatically after presenting n emulated frames\n",
        "  --no-gamepad                           Disable SDL gamepad input\n",
        "  --gamepad-direction <dpad-only|left-stick|both>\n",
        "                                         Choose which controller directions drive the joypad (default: both)\n",
        "  --gamepad-face-layout <east-a|south-a>\n",
        "                                         Apply a face-button preset for A/B (default: east-a)\n",
        "  --gamepad-bind-<up|down|left|right|a|b|select|start> <button>\n",
        "                                         Remap any GB gamepad button to a standard SDL button name\n",
        "                                         Button names: south, east, west, north, back, start, guide, left-shoulder,\n",
        "                                         right-shoulder, left-stick-click, right-stick-click, dpad-up, dpad-down,\n",
        "                                         dpad-left, dpad-right, misc1\n",
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
        "\n",
        "If no ROM is provided, gb-desktop opens without a cartridge and starts in the in-window menu.\n",
    )
}

fn parse_console_model(value: &str) -> Result<DesktopConsoleModel, String> {
    match value {
        "dmg0" => Ok(DesktopConsoleModel::Dmg0),
        "dmg" => Ok(DesktopConsoleModel::Dmg),
        "mgb" => Ok(DesktopConsoleModel::Mgb),
        _ => Err(format!(
            "unsupported --model value {value:?}; expected one of: dmg0, dmg, mgb"
        )),
    }
}

fn parse_startup_mode(value: &str) -> Result<StartupMode, String> {
    match value {
        "skip-boot" => Ok(StartupMode::SkipBoot),
        "real-boot" => Ok(StartupMode::RealBoot),
        _ => Err(format!(
            "unsupported --startup value {value:?}; expected skip-boot or real-boot"
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
        "left-stick-click" => Ok(GamepadButtonBinding::LeftStickClick),
        "right-stick-click" => Ok(GamepadButtonBinding::RightStickClick),
        "dpad-up" => Ok(GamepadButtonBinding::DPadUp),
        "dpad-down" => Ok(GamepadButtonBinding::DPadDown),
        "dpad-left" => Ok(GamepadButtonBinding::DPadLeft),
        "dpad-right" => Ok(GamepadButtonBinding::DPadRight),
        "misc1" => Ok(GamepadButtonBinding::Misc1),
        _ => Err(format!(
            "unsupported gamepad binding value {value:?}; expected a standard SDL button name"
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

fn parse_positive_u64(flag: &str, value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|error| format!("invalid {flag} value {value:?}: {error}"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be greater than zero"));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gb_desktop::PreferredGamepadIdentity;

    #[test]
    fn parse_defaults_to_the_expected_dmg_desktop_baseline() {
        let action = parse_cli_arguments(["roms/tetris.gb"]).expect("default CLI should parse");

        let CliAction::Run(options) = action else {
            panic!("expected run action");
        };
        assert_eq!(options.rom_path, Some(PathBuf::from("roms/tetris.gb")));
        assert_eq!(options.linked_peer_rom_path, None);
        assert_eq!(options.exit_after_frames, None);
        assert_eq!(
            options.config.launch.console_model,
            DesktopConsoleModel::Dmg
        );
        assert_eq!(options.config.launch.startup_mode, StartupMode::SkipBoot);
        assert_eq!(options.config.launch.execution_mode, ExecutionMode::Strict);
        assert!(options.config.saves.enabled);
        assert_eq!(
            options.config.saves.flush_policy,
            DesktopSaveFlushPolicy::Debounced
        );
        assert_eq!(
            options.config.video.window_scale,
            DesktopConfig::default().video.window_scale
        );
    }

    #[test]
    fn parse_supports_running_without_a_rom_path() {
        let action =
            parse_cli_arguments(["--startup", "real-boot"]).expect("CLI should allow no ROM");

        let CliAction::Run(options) = action else {
            panic!("expected run action");
        };
        assert_eq!(options.rom_path, None);
        assert_eq!(options.linked_peer_rom_path, None);
        assert_eq!(options.exit_after_frames, None);
        assert_eq!(options.config.launch.startup_mode, StartupMode::RealBoot);
    }

    #[test]
    fn parse_supports_disabling_saves_and_overriding_the_scale() {
        let action = parse_cli_arguments(["demo.gb", "--no-saves", "--scale", "6"])
            .expect("CLI overrides should parse");

        let CliAction::Run(options) = action else {
            panic!("expected run action");
        };
        assert!(!options.config.saves.enabled);
        assert_eq!(options.config.video.window_scale, 6);
    }

    #[test]
    fn parse_supports_debounced_save_policy_overrides() {
        let action = parse_cli_arguments(["demo.gb", "--save-policy", "on-close"])
            .expect("save policy CLI overrides should parse");

        let CliAction::Run(options) = action else {
            panic!("expected run action");
        };
        assert_eq!(
            options.config.saves.flush_policy,
            DesktopSaveFlushPolicy::OnClose
        );

        let action = parse_cli_arguments(["demo.gb", "--save-policy", "debounced"])
            .expect("debounced save policy should parse");
        let CliAction::Run(options) = action else {
            panic!("expected run action");
        };
        assert_eq!(
            options.config.saves.flush_policy,
            DesktopSaveFlushPolicy::Debounced
        );
    }

    #[test]
    fn parse_supports_gamepad_overrides() {
        let action = parse_cli_arguments([
            "demo.gb",
            "--no-gamepad",
            "--gamepad-direction",
            "left-stick",
            "--gamepad-face-layout",
            "south-a",
        ])
        .expect("gamepad CLI overrides should parse");

        let CliAction::Run(options) = action else {
            panic!("expected run action");
        };
        assert!(!options.config.input.gamepad.enabled);
        assert_eq!(
            options.config.input.gamepad.directional_source,
            GamepadDirectionalSource::LeftStickOnly
        );
        assert_eq!(
            options.config.input.gamepad.bindings.a,
            GamepadButtonBinding::South
        );
        assert_eq!(
            options.config.input.gamepad.bindings.b,
            GamepadButtonBinding::East
        );
    }

    #[test]
    fn parse_supports_explicit_gamepad_binding_overrides() {
        let action = parse_cli_arguments([
            "demo.gb",
            "--gamepad-bind-a",
            "north",
            "--gamepad-bind-b",
            "west",
            "--gamepad-bind-select",
            "left-shoulder",
            "--gamepad-bind-start",
            "right-shoulder",
        ])
        .expect("explicit gamepad binding overrides should parse");

        let CliAction::Run(options) = action else {
            panic!("expected run action");
        };
        assert_eq!(
            options.config.input.gamepad.bindings.a,
            GamepadButtonBinding::North
        );
        assert_eq!(
            options.config.input.gamepad.bindings.b,
            GamepadButtonBinding::West
        );
        assert_eq!(
            options.config.input.gamepad.bindings.select,
            GamepadButtonBinding::LeftShoulder
        );
        assert_eq!(
            options.config.input.gamepad.bindings.start,
            GamepadButtonBinding::RightShoulder
        );
    }

    #[test]
    fn parse_supports_preferred_gamepad_identity_overrides() {
        let action = parse_cli_arguments([
            "demo.gb",
            "--gamepad-preferred-name",
            "Nintendo Switch Pro Controller",
            "--gamepad-preferred-path",
            "bluetooth:vendor=057e,product=2009",
        ])
        .expect("preferred gamepad CLI overrides should parse");

        let CliAction::Run(options) = action else {
            panic!("expected a run action");
        };

        assert_eq!(
            options.config.input.gamepad.preferred_device,
            PreferredGamepadIdentity {
                name: Some("Nintendo Switch Pro Controller".to_string()),
                path: Some("bluetooth:vendor=057e,product=2009".to_string()),
            }
        );
    }

    #[test]
    fn parse_uses_the_provided_base_config_before_cli_overrides() {
        let mut base_config = DesktopConfig::default();
        base_config.video.window_scale = 6;
        base_config.input.gamepad.enabled = false;

        let action = parse_cli_arguments_with_base_config(["demo.gb"], base_config)
            .expect("base-config parsing should succeed");

        let CliAction::Run(options) = action else {
            panic!("expected a run action");
        };

        assert_eq!(options.config.video.window_scale, 6);
        assert!(!options.config.input.gamepad.enabled);
    }

    #[test]
    fn parse_cli_overrides_still_win_over_the_base_config() {
        let mut base_config = DesktopConfig::default();
        base_config.video.window_scale = 6;

        let action = parse_cli_arguments_with_base_config(["demo.gb", "--scale", "3"], base_config)
            .expect("CLI override over base config should succeed");

        let CliAction::Run(options) = action else {
            panic!("expected a run action");
        };

        assert_eq!(options.config.video.window_scale, 3);
    }

    #[test]
    fn help_text_lists_host_boot_audio_and_input_overrides() {
        let text = help_text();

        assert!(text.contains("Usage:"));
        assert!(text.contains("--boot-rom-dir <dir>"));
        assert!(text.contains("--save-key <key>"));
        assert!(text.contains("--fullscreen"));
        assert!(text.contains("--mute"));
        assert!(text.contains("--link-rom <path>"));
        assert!(text.contains("--exit-after-frames <n>"));
        assert!(text.contains("--gamepad-preferred-path <path>"));
        assert!(text.contains("GB_CYCLE_DESKTOP_SETTINGS_PATH"));
        assert!(text.contains("GB_CYCLE_DESKTOP_AUDIO_LOG"));
        assert!(text.contains("GB_CYCLE_DESKTOP_AUDIO_DISABLE_AUTO_CLEAR"));
        assert!(text.contains("GB_CYCLE_DESKTOP_EMU_PROFILE"));
        assert!(text.contains("GB_CYCLE_DESKTOP_TRACE_PATH"));
        assert!(text.contains("GB_CYCLE_DESKTOP_TRACE_T_CYCLES"));
    }

    #[test]
    fn parse_supports_model_boot_save_and_video_overrides() {
        let action = parse_cli_arguments([
            "demo.gb",
            "--model",
            "dmg0",
            "--mode",
            "experimental",
            "--boot-rom-dir",
            "firmware",
            "--boot-rom-verify",
            "warn",
            "--save-dir",
            "saves",
            "--save-key",
            "slot_1",
            "--fullscreen",
            "--no-vsync",
            "--mute",
            "--link-rom",
            "linked.gb",
            "--exit-after-frames",
            "120",
        ])
        .expect("host overrides should parse");

        let CliAction::Run(options) = action else {
            panic!("expected a run action");
        };

        assert_eq!(options.rom_path, Some(PathBuf::from("demo.gb")));
        assert_eq!(
            options.linked_peer_rom_path,
            Some(PathBuf::from("linked.gb"))
        );
        assert_eq!(options.exit_after_frames, Some(120));
        assert_eq!(
            options.config.launch.console_model,
            DesktopConsoleModel::Dmg0
        );
        assert_eq!(
            options.config.launch.execution_mode,
            ExecutionMode::Experimental
        );
        assert_eq!(
            options.config.boot_rom.search_path,
            Some(PathBuf::from("firmware"))
        );
        assert_eq!(
            options.config.boot_rom.verification,
            BootRomVerificationMode::Warn
        );
        assert_eq!(
            options.config.saves.directory_policy,
            SaveDirectoryPolicy::Custom(PathBuf::from("saves"))
        );
        let SaveKeyPolicy::Explicit(save_key) = &options.config.saves.key_policy else {
            panic!("expected an explicit save key override");
        };
        assert_eq!(save_key.as_str(), "slot_1");
        assert!(options.config.video.fullscreen);
        assert!(!options.config.video.vsync);
        assert_eq!(
            options.config.audio,
            AudioOptions {
                enabled: false,
                ..DesktopConfig::default().audio
            }
        );
    }

    #[test]
    fn parse_supports_help_and_remaining_directional_gamepad_bindings() {
        assert_eq!(parse_cli_arguments(["--help"]), Ok(CliAction::ShowHelp));
        assert_eq!(parse_cli_arguments(["-h"]), Ok(CliAction::ShowHelp));

        let action = parse_cli_arguments([
            "demo.gb",
            "--gamepad-bind-up",
            "south",
            "--gamepad-bind-down",
            "dpad-down",
            "--gamepad-bind-left",
            "dpad-left",
            "--gamepad-bind-right",
            "back",
        ])
        .expect("remaining directional bindings should parse");

        let CliAction::Run(options) = action else {
            panic!("expected a run action");
        };

        assert_eq!(
            options.config.input.gamepad.bindings.up,
            GamepadButtonBinding::South
        );
        assert_eq!(
            options.config.input.gamepad.bindings.down,
            GamepadButtonBinding::DPadDown
        );
        assert_eq!(
            options.config.input.gamepad.bindings.left,
            GamepadButtonBinding::DPadLeft
        );
        assert_eq!(
            options.config.input.gamepad.bindings.right,
            GamepadButtonBinding::Back
        );
    }

    #[test]
    fn parse_rejects_a_linked_peer_without_a_primary_rom() {
        assert_eq!(
            parse_cli_arguments(["--link-rom", "linked.gb"]),
            Err("--link-rom requires a primary ROM positional argument".to_string())
        );
    }

    #[test]
    fn parser_helpers_accept_supported_values_and_reject_unknown_ones() {
        assert_eq!(parse_console_model("dmg0"), Ok(DesktopConsoleModel::Dmg0));
        assert_eq!(parse_console_model("dmg"), Ok(DesktopConsoleModel::Dmg));
        assert_eq!(parse_console_model("mgb"), Ok(DesktopConsoleModel::Mgb));
        assert!(parse_console_model("cgb").is_err());

        assert_eq!(parse_startup_mode("skip-boot"), Ok(StartupMode::SkipBoot));
        assert_eq!(parse_startup_mode("real-boot"), Ok(StartupMode::RealBoot));
        assert!(parse_startup_mode("warm-boot").is_err());

        assert_eq!(parse_execution_mode("strict"), Ok(ExecutionMode::Strict));
        assert_eq!(
            parse_execution_mode("permissive"),
            Ok(ExecutionMode::Permissive)
        );
        assert_eq!(
            parse_execution_mode("experimental"),
            Ok(ExecutionMode::Experimental)
        );
        assert!(parse_execution_mode("fast").is_err());

        assert_eq!(
            parse_boot_rom_verification_mode("off"),
            Ok(BootRomVerificationMode::Off)
        );
        assert_eq!(
            parse_boot_rom_verification_mode("warn"),
            Ok(BootRomVerificationMode::Warn)
        );
        assert_eq!(
            parse_boot_rom_verification_mode("strict"),
            Ok(BootRomVerificationMode::Strict)
        );
        assert!(parse_boot_rom_verification_mode("lenient").is_err());
    }

    #[test]
    fn cli_value_parsers_cover_save_policy_direction_source_and_face_layout() {
        assert_eq!(
            parse_save_policy("manual"),
            Ok(DesktopSaveFlushPolicy::Manual)
        );
        assert_eq!(
            parse_save_policy("on-close"),
            Ok(DesktopSaveFlushPolicy::OnClose)
        );
        assert_eq!(
            parse_save_policy("on-write"),
            Ok(DesktopSaveFlushPolicy::OnWrite)
        );
        assert_eq!(
            parse_save_policy("debounced"),
            Ok(DesktopSaveFlushPolicy::Debounced)
        );
        assert!(parse_save_policy("later").is_err());

        assert_eq!(
            parse_gamepad_directional_source("dpad-only"),
            Ok(GamepadDirectionalSource::DpadOnly)
        );
        assert_eq!(
            parse_gamepad_directional_source("left-stick"),
            Ok(GamepadDirectionalSource::LeftStickOnly)
        );
        assert_eq!(
            parse_gamepad_directional_source("both"),
            Ok(GamepadDirectionalSource::DpadAndLeftStick)
        );
        assert!(parse_gamepad_directional_source("stick-only").is_err());

        assert_eq!(
            parse_gamepad_face_layout("east-a"),
            Ok(GamepadFaceLayout::EastASouthB)
        );
        assert_eq!(
            parse_gamepad_face_layout("south-a"),
            Ok(GamepadFaceLayout::SouthAEastB)
        );
        assert!(parse_gamepad_face_layout("north-a").is_err());
    }

    #[test]
    fn gamepad_binding_parsers_cover_supported_buttons_and_slots() {
        assert_eq!(
            parse_gamepad_button_binding("south"),
            Ok(GamepadButtonBinding::South)
        );
        assert_eq!(
            parse_gamepad_button_binding("east"),
            Ok(GamepadButtonBinding::East)
        );
        assert_eq!(
            parse_gamepad_button_binding("back"),
            Ok(GamepadButtonBinding::Back)
        );
        assert_eq!(
            parse_gamepad_button_binding("start"),
            Ok(GamepadButtonBinding::Start)
        );
        assert_eq!(
            parse_gamepad_button_binding("guide"),
            Ok(GamepadButtonBinding::Guide)
        );
        assert_eq!(
            parse_gamepad_button_binding("dpad-up"),
            Ok(GamepadButtonBinding::DPadUp)
        );
        assert_eq!(
            parse_gamepad_button_binding("dpad-down"),
            Ok(GamepadButtonBinding::DPadDown)
        );
        assert_eq!(
            parse_gamepad_button_binding("dpad-left"),
            Ok(GamepadButtonBinding::DPadLeft)
        );
        assert_eq!(
            parse_gamepad_button_binding("left-stick-click"),
            Ok(GamepadButtonBinding::LeftStickClick)
        );
        assert_eq!(
            parse_gamepad_button_binding("right-stick-click"),
            Ok(GamepadButtonBinding::RightStickClick)
        );
        assert_eq!(
            parse_gamepad_button_binding("dpad-right"),
            Ok(GamepadButtonBinding::DPadRight)
        );
        assert_eq!(
            parse_gamepad_button_binding("misc1"),
            Ok(GamepadButtonBinding::Misc1)
        );
        assert!(parse_gamepad_button_binding("touchpad").is_err());

        let mut bindings = GamepadButtonBindings::default();
        apply_gamepad_binding_override(&mut bindings, "up", GamepadButtonBinding::North)
            .expect("known slots should update");
        apply_gamepad_binding_override(&mut bindings, "down", GamepadButtonBinding::South)
            .expect("known slots should update");
        apply_gamepad_binding_override(&mut bindings, "left", GamepadButtonBinding::Back)
            .expect("known slots should update");
        apply_gamepad_binding_override(&mut bindings, "right", GamepadButtonBinding::Guide)
            .expect("known slots should update");
        apply_gamepad_binding_override(&mut bindings, "start", GamepadButtonBinding::Guide)
            .expect("known slots should update");
        assert_eq!(bindings.up, GamepadButtonBinding::North);
        assert_eq!(bindings.down, GamepadButtonBinding::South);
        assert_eq!(bindings.left, GamepadButtonBinding::Back);
        assert_eq!(bindings.right, GamepadButtonBinding::Guide);
        assert_eq!(bindings.start, GamepadButtonBinding::Guide);
        assert!(
            apply_gamepad_binding_override(&mut bindings, "shoulder", GamepadButtonBinding::West)
                .is_err()
        );
    }

    #[test]
    fn text_and_numeric_parsers_trim_and_validate_user_input() {
        assert_eq!(
            parse_non_empty_text("--gamepad-preferred-name", "  Switch Pro  "),
            Ok("Switch Pro".to_string())
        );
        assert!(parse_non_empty_text("--save-key", "   ").is_err());

        assert_eq!(parse_positive_u8("--scale", "6"), Ok(6));
        assert!(parse_positive_u8("--scale", "0").is_err());
        assert!(parse_positive_u8("--scale", "wide").is_err());
        assert_eq!(parse_positive_u64("--exit-after-frames", "6"), Ok(6));
        assert!(parse_positive_u64("--exit-after-frames", "0").is_err());
        assert!(parse_positive_u64("--exit-after-frames", "wide").is_err());

        assert_eq!(
            parse_save_key("SAVE_SLOT_1")
                .expect("valid cartridge save keys should parse")
                .as_str(),
            "SAVE_SLOT_1"
        );
        assert!(parse_save_key("contains spaces").is_err());
    }

    #[test]
    fn cli_reports_missing_values_unknown_options_and_extra_positional_arguments() {
        assert_eq!(
            parse_cli_arguments(["--model"]).expect_err("missing values should fail"),
            "--model requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--mode"]).expect_err("missing mode values should fail"),
            "--mode requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--boot-rom-dir"])
                .expect_err("missing boot ROM directory values should fail"),
            "--boot-rom-dir requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--save-key"]).expect_err("missing save-key values should fail"),
            "--save-key requires a value"
        );
        assert!(parse_cli_arguments(["--mystery"]).is_err());
        assert!(parse_cli_arguments(["first.gb", "second.gb"]).is_err());
    }

    #[test]
    fn cli_reports_remaining_missing_values_and_invalid_flag_inputs() {
        assert_eq!(
            parse_cli_arguments(["--startup"]).expect_err("missing startup values should fail"),
            "--startup requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--boot-rom-verify"])
                .expect_err("missing boot ROM verification values should fail"),
            "--boot-rom-verify requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--save-dir"]).expect_err("missing save-dir values should fail"),
            "--save-dir requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--save-policy"])
                .expect_err("missing save-policy values should fail"),
            "--save-policy requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--scale"]).expect_err("missing scale values should fail"),
            "--scale requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--link-rom"])
                .expect_err("missing linked peer values should fail"),
            "--link-rom requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--exit-after-frames"])
                .expect_err("missing exit-after-frames values should fail"),
            "--exit-after-frames requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--gamepad-direction"])
                .expect_err("missing gamepad direction values should fail"),
            "--gamepad-direction requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--gamepad-face-layout"])
                .expect_err("missing face-layout values should fail"),
            "--gamepad-face-layout requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--gamepad-preferred-name"])
                .expect_err("missing preferred-name values should fail"),
            "--gamepad-preferred-name requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--gamepad-preferred-path"])
                .expect_err("missing preferred-path values should fail"),
            "--gamepad-preferred-path requires a value"
        );
        assert_eq!(
            parse_cli_arguments(["--gamepad-bind-a"])
                .expect_err("missing gamepad binding values should fail"),
            "--gamepad-bind-a requires a value"
        );

        assert!(parse_cli_arguments(["--model", "cgb"]).is_err());
        assert!(parse_cli_arguments(["--startup", "warm-boot"]).is_err());
        assert!(parse_cli_arguments(["--mode", "fast"]).is_err());
        assert!(parse_cli_arguments(["--boot-rom-verify", "lenient"]).is_err());
        assert!(parse_cli_arguments(["--save-key", "contains spaces"]).is_err());
        assert!(parse_cli_arguments(["--save-policy", "later"]).is_err());
        assert!(parse_cli_arguments(["--scale", "0"]).is_err());
        assert!(parse_cli_arguments(["--exit-after-frames", "0"]).is_err());
        assert!(parse_cli_arguments(["--gamepad-direction", "stick-only"]).is_err());
        assert!(parse_cli_arguments(["--gamepad-face-layout", "north-a"]).is_err());
        assert!(parse_cli_arguments(["--gamepad-preferred-name", "   "]).is_err());
        assert!(parse_cli_arguments(["--gamepad-preferred-path", "   "]).is_err());
        assert!(parse_cli_arguments(["--gamepad-bind-a", "touchpad"]).is_err());
    }
}
