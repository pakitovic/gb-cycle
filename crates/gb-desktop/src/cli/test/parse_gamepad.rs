use super::*;

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
fn parse_test_runner_applies_host_light_defaults_without_changing_hardware_axes() {
    let mut base_config = DesktopConfig::default();
    base_config.video.window_scale = 6;
    base_config.video.vsync = true;
    base_config.video.fullscreen = true;
    base_config.video.show_performance_hud = true;
    base_config.audio.enabled = true;
    base_config.input.gamepad.enabled = true;
    base_config.launch.startup_mode = StartupMode::RealBoot;
    base_config.launch.execution_mode = ExecutionMode::Experimental;

    let action = parse_cli_arguments_with_base_config(["demo.gb", "--test-runner"], base_config)
        .expect("test-runner CLI should parse");

    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };

    assert!(options.test_runner);
    assert!(!options.config.saves.enabled);
    assert!(!options.config.rewind.enabled);
    assert!(!options.config.audio.enabled);
    assert!(!options.config.input.gamepad.enabled);
    assert!(!options.config.video.vsync);
    assert!(!options.config.video.fullscreen);
    assert!(!options.config.video.show_performance_hud);
    assert_eq!(options.config.video.window_scale, 1);
    assert_eq!(options.config.launch.startup_mode, StartupMode::RealBoot);
    assert_eq!(
        options.config.launch.execution_mode,
        ExecutionMode::Permissive
    );
    assert_eq!(
        options.config.video.display_palette,
        DesktopDisplayPalette::Grey
    );
    assert_eq!(
        options.config.video.sgb_border,
        SgbBorderPresentationMode::Off
    );

    let action = parse_cli_arguments(["demo.gb", "--model", "MGB", "--test-runner"])
        .expect("test-runner should not force the DMG grey palette on non-DMG desktop models");
    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.config.video.display_palette,
        DesktopConfig::default().video.display_palette
    );
    assert_eq!(
        options.config.launch.execution_mode,
        ExecutionMode::Permissive
    );
    assert_eq!(
        options.config.video.sgb_border,
        SgbBorderPresentationMode::Off
    );
}
