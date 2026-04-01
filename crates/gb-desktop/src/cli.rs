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

    Ok(CliAction::Run(Box::new(DesktopRunOptions {
        rom_path,
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
}
