use super::super::*;

#[test]
fn parse_run_arguments_keep_the_default_game_boy_model() {
    let action = parse_cli_arguments(["run", "demo.gb"]).expect("run arguments should parse");

    match action {
        CliAction::Run(options) => {
            assert_eq!(options.model, RunModel::GameBoy);
            assert_eq!(options.startup_mode, StartupMode::SkipBoot);
            assert_eq!(options.execution_mode, ExecutionMode::Strict);
            assert_eq!(
                options.default_run_budget,
                Some(DefaultRunBudget::SkipBootFrames {
                    frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
                })
            );
            assert_eq!(options.frame_limit, None);
            assert_eq!(options.tcycle_limit, None);
            assert!(!options.test_runner);
        }
        other => panic!("expected run action, got {other:?}"),
    }
}

#[test]
fn parse_run_arguments_use_the_real_boot_default_budget_profile_when_no_limit_is_provided() {
    let action = parse_cli_arguments(["run", "demo.gb", "--startup", "real-boot"])
        .expect("real-boot arguments should parse");

    match action {
        CliAction::Run(options) => {
            assert_eq!(options.startup_mode, StartupMode::RealBoot);
            assert_eq!(
                options.default_run_budget,
                Some(DefaultRunBudget::RealBootPostHandoff {
                    post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
                    safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
                })
            );
            assert_eq!(options.frame_limit, None);
            assert_eq!(options.tcycle_limit, None);
        }
        other => panic!("expected run action, got {other:?}"),
    }
}

#[test]
fn parse_run_arguments_accepts_the_full_option_matrix() {
    let action = parse_run_arguments([
        "demo.gb",
        "--model",
        "CGB",
        "--revision",
        "cpu-cgb-e",
        "--startup",
        "real-boot",
        "--mode",
        "permissive",
        "--boot-rom-dir",
        "boot-assets",
        "--boot-rom-verify",
        "warn",
        "--frames",
        "7",
        "--tcycles",
        "11",
        "--serial-stdout",
        "--serial-out",
        "serial.bin",
        "--framebuffer-out",
        "framebuffer.png",
        "--palette",
        "grey",
        "--trace-out",
        "trace.txt",
        "--state-in",
        "input.gbstate",
        "--state-out",
        "output.gbstate",
        "--save-dir",
        "saves",
        "--save-key",
        "demo_save",
        "--save-policy",
        "manual",
        "--test-runner",
    ])
    .expect("run arguments should parse");

    match action {
        CliAction::Run(options) => {
            assert_eq!(options.rom_path, PathBuf::from("demo.gb"));
            assert_eq!(options.model, RunModel::Color);
            assert_eq!(options.revision, HardwareRevision::CpuCgbE);
            assert_eq!(options.effective_revision(), HardwareRevision::CpuCgbE);
            assert_eq!(options.startup_mode, StartupMode::RealBoot);
            assert_eq!(options.execution_mode, ExecutionMode::Permissive);
            assert_eq!(options.boot_rom_dir, Some(PathBuf::from("boot-assets")));
            assert_eq!(options.boot_rom_verify, BootRomVerificationMode::Warn);
            assert_eq!(options.frame_limit, Some(7));
            assert_eq!(options.tcycle_limit, Some(11));
            assert!(options.serial_stdout);
            assert_eq!(options.serial_out, Some(PathBuf::from("serial.bin")));
            assert_eq!(
                options.framebuffer_out,
                Some(PathBuf::from("framebuffer.png"))
            );
            assert!(!options.show_sgb_border);
            assert_eq!(options.display_palette, None);
            assert_eq!(options.effective_display_palette(), None);
            assert_eq!(options.trace_out, Some(PathBuf::from("trace.txt")));
            assert_eq!(options.state_in, Some(PathBuf::from("input.gbstate")));
            assert_eq!(options.state_out, Some(PathBuf::from("output.gbstate")));
            assert_eq!(options.save_dir, Some(PathBuf::from("saves")));
            assert_eq!(options.save_key.as_deref(), Some("demo_save"));
            assert_eq!(options.save_policy, SavePolicy::Manual);
            assert_eq!(options.default_run_budget, None);
            assert!(options.test_runner);
        }
        other => panic!("expected run action, got {other:?}"),
    }
}

#[test]
fn parse_run_arguments_accepts_sgb_profiles_as_dmg_core_models() {
    let action =
        parse_run_arguments(["demo.gb", "--model", "SGB"]).expect("SGB model should parse");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.model, RunModel::SuperGameBoy);
    assert_eq!(options.model.console_model(), ConsoleModel::GameBoy);
    assert_eq!(options.model.sgb_profile(), Some(SgbHostProfile::SgbNtsc));
    assert_eq!(options.sgb_video_standard, SgbVideoStandard::Ntsc);
    assert_eq!(
        options
            .model
            .sgb_profile_for_standard(options.sgb_video_standard),
        Some(SgbHostProfile::SgbNtsc)
    );
    assert_eq!(options.revision, HardwareRevision::DmgCpuC);
    assert!(options.show_sgb_border);

    let action = parse_run_arguments(["demo.gb", "--model", "SGB", "--sgb-standard", "pal"])
        .expect("SGB PAL should parse");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.model, RunModel::SuperGameBoy);
    assert_eq!(options.sgb_video_standard, SgbVideoStandard::Pal);
    assert_eq!(
        options
            .model
            .sgb_profile_for_standard(options.sgb_video_standard),
        Some(SgbHostProfile::SgbPal)
    );

    let action = parse_run_arguments(["demo.gb", "--model", "SGB2", "--border-off"])
        .expect("SGB2 model should parse with border disabled");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.model, RunModel::SuperGameBoy2);
    assert_eq!(options.model.console_model(), ConsoleModel::GameBoy);
    assert_eq!(options.model.sgb_profile(), Some(SgbHostProfile::Sgb2Ntsc));
    assert_eq!(options.revision, HardwareRevision::DmgCpuC);
    assert!(!options.show_sgb_border);

    let action = parse_run_arguments(["demo.gb", "--border-off", "--model", "SGB"])
        .expect("SGB model should accept order-independent border disabling");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.model, RunModel::SuperGameBoy);
    assert!(!options.show_sgb_border);

    let action = parse_run_arguments(["demo.gb", "--border-off"])
        .expect("border disabling should be accepted and ignored outside SGB-family output");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.model, RunModel::GameBoy);
    assert!(!options.show_sgb_border);

    let action = parse_run_arguments(["demo.gb", "--model", "CGB", "--border-off"])
        .expect("border disabling should not reject non-SGB models");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.model, RunModel::Color);
    assert!(!options.show_sgb_border);

    let sgb2_standard_error =
        parse_run_arguments(["demo.gb", "--model", "SGB2", "--sgb-standard", "ntsc"])
            .expect_err("SGB2 should not accept an explicit SGB standard");
    assert_eq!(sgb2_standard_error, "--sgb-standard requires --model SGB");

    let non_sgb_standard_error = parse_run_arguments(["demo.gb", "--sgb-standard", "pal"])
        .expect_err("non-SGB models should reject SGB standard overrides");
    assert_eq!(
        non_sgb_standard_error,
        "--sgb-standard requires --model SGB"
    );
}

#[test]
fn parse_run_arguments_applies_test_runner_defaults_without_changing_emulated_limits() {
    let action =
        parse_run_arguments(["demo.gb", "--test-runner"]).expect("test-runner should parse");

    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };

    assert!(options.test_runner);
    assert_eq!(options.startup_mode, StartupMode::SkipBoot);
    assert_eq!(options.execution_mode, ExecutionMode::Permissive);
    assert!(!options.show_sgb_border);
    assert_eq!(options.display_palette, Some(RunDisplayPalette::Grey));
    assert_eq!(
        options.effective_display_palette(),
        Some(DMG_GREY_DISPLAY_PALETTE)
    );
    assert_eq!(options.save_dir, None);
    assert_eq!(
        options.default_run_budget,
        Some(DefaultRunBudget::SkipBootFrames {
            frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
        })
    );

    let action = parse_run_arguments(["demo.gb", "--test-runner", "--mode", "strict"])
        .expect("test-runner should force permissive mode");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert!(options.test_runner);
    assert_eq!(options.execution_mode, ExecutionMode::Permissive);

    let action = parse_run_arguments(["demo.gb", "--model", "CGB", "--test-runner"])
        .expect("test-runner should be accepted for non-DMG models");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert!(options.test_runner);
    assert_eq!(options.model, RunModel::Color);
    assert_eq!(options.execution_mode, ExecutionMode::Permissive);
    assert!(!options.show_sgb_border);
    assert_eq!(options.display_palette, None);
    assert_eq!(options.effective_display_palette(), None);
}

#[test]
fn parse_run_arguments_accepts_benchmark_case_without_positional_rom() {
    let action = parse_run_arguments(["--test-runner", "--benchmark", "test/dr-mario.toml"])
        .expect("benchmark case should parse");

    let CliAction::RunBenchmark(options) = action else {
        panic!("expected benchmark run action");
    };

    assert_eq!(options.benchmark_path, PathBuf::from("test/dr-mario.toml"));
    assert!(options.test_runner);
}

#[test]
fn parse_run_arguments_applies_grey_palette_only_for_the_final_dmg_model() {
    let action = parse_run_arguments(["demo.gb", "--model", "DMG", "--palette", "grey"])
        .expect("DMG grey palette override should parse");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.display_palette, Some(RunDisplayPalette::Grey));
    assert_eq!(
        options.effective_display_palette(),
        Some(DMG_GREY_DISPLAY_PALETTE)
    );

    let action = parse_run_arguments(["demo.gb", "--palette", "grey", "--model", "DMG"])
        .expect("palette override should be order-independent");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.display_palette, Some(RunDisplayPalette::Grey));
    assert_eq!(
        options.effective_display_palette(),
        Some(DMG_GREY_DISPLAY_PALETTE)
    );

    let action = parse_run_arguments(["demo.gb", "--model", "MGB", "--palette", "grey"])
        .expect("non-DMG palette override should be ignored");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.display_palette, None);
    assert_eq!(options.effective_display_palette(), None);
}

#[test]
fn parse_run_arguments_rejects_invalid_sequences_and_missing_values() {
    let missing_value_cases = [
        (vec!["demo.gb", "--model"], "--model requires a value"),
        (vec!["demo.gb", "--revision"], "--revision requires a value"),
        (
            vec!["demo.gb", "--sgb-standard"],
            "--sgb-standard requires a value",
        ),
        (vec!["demo.gb", "--startup"], "--startup requires a value"),
        (vec!["demo.gb", "--mode"], "--mode requires a value"),
        (
            vec!["demo.gb", "--boot-rom-dir"],
            "--boot-rom-dir requires a value",
        ),
        (
            vec!["demo.gb", "--boot-rom-verify"],
            "--boot-rom-verify requires a value",
        ),
        (vec!["demo.gb", "--frames"], "--frames requires a value"),
        (vec!["demo.gb", "--tcycles"], "--tcycles requires a value"),
        (
            vec!["demo.gb", "--serial-out"],
            "--serial-out requires a value",
        ),
        (
            vec!["demo.gb", "--framebuffer-out"],
            "--framebuffer-out requires a value",
        ),
        (vec!["demo.gb", "--palette"], "--palette requires a value"),
        (
            vec!["demo.gb", "--trace-out"],
            "--trace-out requires a value",
        ),
        (vec!["demo.gb", "--state-in"], "--state-in requires a value"),
        (
            vec!["demo.gb", "--state-out"],
            "--state-out requires a value",
        ),
        (vec!["demo.gb", "--save-dir"], "--save-dir requires a value"),
        (vec!["demo.gb", "--save-key"], "--save-key requires a value"),
        (
            vec!["demo.gb", "--save-policy"],
            "--save-policy requires a value",
        ),
        (vec!["--benchmark"], "--benchmark requires a value"),
    ];

    for (arguments, expected) in missing_value_cases {
        assert_eq!(
            parse_run_arguments(arguments).expect_err("missing values should fail"),
            expected
        );
    }

    assert_eq!(
        parse_run_arguments(["--model", "DMG"]).expect_err("ROM path must come first"),
        "the ROM path must be the first positional argument to `gb-cli run`"
    );
    assert_eq!(
        parse_run_arguments(std::iter::empty::<&str>()).expect_err("run requires a ROM path"),
        "missing required ROM path; run `gb-cli run --help` for usage"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--save-key", "demo"])
            .expect_err("save key requires save dir"),
        "--save-key requires --save-dir"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--save-policy", "manual"])
            .expect_err("save policy requires save dir"),
        "--save-policy requires --save-dir"
    );
    assert_eq!(
        parse_run_arguments(["--benchmark", "one.toml", "--benchmark", "two.toml"])
            .expect_err("duplicate benchmark paths should fail"),
        "--benchmark can only be provided once"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--benchmark", "case.toml"])
            .expect_err("benchmark cases supply their own ROM path"),
        "--benchmark supplies the ROM path from the case TOML; omit the positional ROM path"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--mystery"]).expect_err("unknown run options should fail"),
        "unknown run option \"--mystery\"; run `gb-cli run --help`"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--palette", "green"])
            .expect_err("unsupported palettes should fail"),
        "unsupported --palette value \"green\"; expected grey"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--revision", "cpu-cgb-e"])
            .expect_err("CGB-E hardware requires CGB model"),
        "--revision cpu-cgb-e is not supported by --model DMG; expected one of: dmg-cpu-c"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--cgb-revision", "cgb-e"])
            .expect_err("legacy CGB revision flags should fail"),
        "unknown run option \"--cgb-revision\"; run `gb-cli run --help`"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--boot-rom", "cgbE"])
            .expect_err("legacy boot ROM kind flags should fail"),
        "unknown run option \"--boot-rom\"; run `gb-cli run --help`"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "other.gb"])
            .expect_err("extra positional arguments should fail"),
        "unexpected extra positional argument \"other.gb\"; run `gb-cli run --help`"
    );
}
