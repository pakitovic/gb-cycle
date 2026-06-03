use super::*;

#[test]
fn parse_supports_benchmark_case_without_positional_rom() {
    let action = parse_cli_arguments(["--test-runner", "--benchmark", "test/dr-mario.toml"])
        .expect("benchmark CLI should parse");

    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };

    assert_eq!(options.rom_path, None);
    assert_eq!(
        options.benchmark_path,
        Some(PathBuf::from("test/dr-mario.toml"))
    );
    assert!(options.test_runner);
}

#[test]
fn parse_rejects_invalid_benchmark_argument_combinations() {
    assert_eq!(
        parse_cli_arguments(["--benchmark"]).expect_err("missing benchmark path should fail"),
        "--benchmark requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--benchmark", "one.toml", "--benchmark", "two.toml"])
            .expect_err("duplicate benchmark paths should fail"),
        "--benchmark can only be provided once"
    );
    assert_eq!(
        parse_cli_arguments(["demo.gb", "--benchmark", "case.toml"])
            .expect_err("benchmark cases supply their own ROM path"),
        "--benchmark supplies the ROM path from the case TOML; omit the positional ROM path"
    );
}

#[test]
fn parse_test_runner_keeps_explicit_host_overrides_order_independent() {
    let action = parse_cli_arguments([
        "demo.gb",
        "--scale",
        "3",
        "--fullscreen",
        "--save-dir",
        "saves",
        "--gamepad-direction",
        "left-stick",
        "--test-runner",
    ])
    .expect("test-runner CLI overrides should parse");

    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };

    assert!(options.test_runner);
    assert!(options.config.saves.enabled);
    assert_eq!(
        options.config.saves.directory_policy,
        SaveDirectoryPolicy::Custom(PathBuf::from("saves"))
    );
    assert_eq!(options.config.video.window_scale, 3);
    assert!(options.config.video.fullscreen);
    assert_eq!(
        options.config.input.gamepad.directional_source,
        GamepadDirectionalSource::LeftStickOnly
    );
    assert!(options.config.input.gamepad.enabled);
    assert!(!options.config.audio.enabled);
    assert!(!options.config.rewind.enabled);
    assert!(!options.config.video.vsync);

    let action = parse_cli_arguments(["demo.gb", "--test-runner", "--scale", "2"])
        .expect("test-runner after scale should keep explicit scale");
    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(options.config.video.window_scale, 2);
}
