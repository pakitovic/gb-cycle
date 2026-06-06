use super::*;

#[test]
fn parse_supports_revision_overrides() {
    let action = parse_cli_arguments(["demo.gb", "--model", "CGB", "--revision", "cpu-cgb-e"])
        .expect("CGB revision override should parse");

    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };

    assert_eq!(
        options.config.launch.console_model,
        DesktopConsoleModel::GameBoyColor
    );
    assert_eq!(options.config.launch.revision, HardwareRevision::CpuCgbE);
}

#[test]
fn parse_accepts_sgb_profiles_as_dmg_core_models() {
    let action =
        parse_cli_arguments(["demo.gb", "--model", "SGB"]).expect("SGB model should parse");
    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.config.launch.console_model,
        DesktopConsoleModel::SuperGameBoy
    );
    assert_eq!(options.config.launch.revision, HardwareRevision::DmgCpuC);
    assert_eq!(
        options.config.launch.console_model.sgb_profile(),
        Some(gb_core::SgbHostProfile::SgbNtsc)
    );
    assert_eq!(
        options.config.launch.sgb_video_standard,
        SgbVideoStandard::Ntsc
    );

    let action = parse_cli_arguments(["demo.gb", "--model", "SGB", "--sgb-standard", "pal"])
        .expect("SGB PAL should parse");
    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.config.launch.console_model,
        DesktopConsoleModel::SuperGameBoy
    );
    assert_eq!(
        options.config.launch.sgb_video_standard,
        SgbVideoStandard::Pal
    );
    assert_eq!(
        options
            .config
            .launch
            .machine_config_without_boot_rom_assets()
            .sgb_profile,
        Some(gb_core::SgbHostProfile::SgbPal)
    );

    let action =
        parse_cli_arguments(["demo.gb", "--model", "SGB2"]).expect("SGB2 model should parse");
    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.config.launch.console_model,
        DesktopConsoleModel::SuperGameBoy2
    );
    assert_eq!(options.config.launch.revision, HardwareRevision::DmgCpuC);
    assert_eq!(
        options.config.launch.console_model.sgb_profile(),
        Some(gb_core::SgbHostProfile::Sgb2Ntsc)
    );

    let sgb2_standard_error =
        parse_cli_arguments(["demo.gb", "--model", "SGB2", "--sgb-standard", "ntsc"])
            .expect_err("SGB2 should not accept an explicit SGB standard");
    assert_eq!(sgb2_standard_error, "--sgb-standard requires --model SGB");
}

#[test]
fn parse_applies_grey_palette_only_for_the_final_dmg_model() {
    let action = parse_cli_arguments(["demo.gb", "--model", "DMG", "--palette", "grey"])
        .expect("DMG grey palette override should parse");
    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.config.video.display_palette,
        DesktopDisplayPalette::Grey
    );

    let action = parse_cli_arguments(["demo.gb", "--palette", "grey", "--model", "DMG"])
        .expect("palette override should be order-independent");
    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.config.video.display_palette,
        DesktopDisplayPalette::Grey
    );

    let action = parse_cli_arguments(["demo.gb", "--model", "MGB", "--palette", "grey"])
        .expect("non-DMG palette override should be ignored");
    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.config.video.display_palette,
        DesktopConfig::default().video.display_palette
    );
}

#[test]
fn parse_supports_frame_blending_override() {
    let action = parse_cli_arguments(["demo.gb", "--frame-blend", "on"])
        .expect("frame blend override should parse");
    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.config.video.frame_blending,
        DesktopFrameBlendingMode::On
    );

    assert!(parse_cli_arguments(["demo.gb", "--frame-blend", "simple"]).is_err());
    assert!(parse_cli_arguments(["demo.gb", "--frame-blend", "lcd"]).is_err());
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
    assert_eq!(parse_console_model("DMG"), Ok(DesktopConsoleModel::GameBoy));
    assert_eq!(
        parse_console_model("MGB"),
        Ok(DesktopConsoleModel::GameBoyPocket)
    );
    assert_eq!(
        parse_console_model("LGB"),
        Ok(DesktopConsoleModel::GameBoyLight)
    );
    assert_eq!(
        parse_console_model("CGB"),
        Ok(DesktopConsoleModel::GameBoyColor)
    );
    assert_eq!(
        parse_console_model("AGB"),
        Ok(DesktopConsoleModel::GameBoyAdvance)
    );
    assert_eq!(
        parse_console_model("SGB"),
        Ok(DesktopConsoleModel::SuperGameBoy)
    );
    assert_eq!(
        parse_console_model("SGB2"),
        Ok(DesktopConsoleModel::SuperGameBoy2)
    );
    for previous in [
        "game-boy", "pocket", "light", "color", "dmg0", "dmg", "mgb", "cgb",
    ] {
        let error = parse_console_model(previous).expect_err("previous models should fail");
        assert!(error.contains("unsupported --model value"));
        assert!(error.contains("DMG, MGB, LGB, CGB, AGB, SGB, SGB2"));
        assert!(!error.contains("game-boy, pocket, light, color"));
    }
    assert!(parse_console_model("sgb").is_err());

    assert_eq!(parse_revision("dmg-cpu-c"), Ok(HardwareRevision::DmgCpuC));
    assert_eq!(parse_revision("cpu-mgb"), Ok(HardwareRevision::CpuMgb));
    assert_eq!(parse_revision("cpu-cgb-c"), Ok(HardwareRevision::CpuCgbC));
    assert_eq!(parse_revision("cpu-cgb-d"), Ok(HardwareRevision::CpuCgbD));
    assert_eq!(parse_revision("cpu-cgb-e"), Ok(HardwareRevision::CpuCgbE));
    assert_eq!(parse_revision("cpu-agb-a"), Ok(HardwareRevision::CpuAgbA));
    assert!(parse_revision("cpu-cgb-b").is_err());
    assert_eq!(
        revision_argument_name(HardwareRevision::CpuCgbE),
        "cpu-cgb-e"
    );
    assert_eq!(
        revision_argument_name(HardwareRevision::CpuAgbA),
        "cpu-agb-a"
    );
    assert_eq!(
        supported_revision_names(gb_core::ConsoleModel::GameBoyColor),
        "cpu-cgb-c, cpu-cgb-d, cpu-cgb-e"
    );
    assert_eq!(
        supported_revision_names(gb_core::ConsoleModel::GameBoyAdvance),
        "cpu-agb-a"
    );

    assert_eq!(parse_startup_mode("skip-boot"), Ok(StartupMode::SkipBoot));
    assert_eq!(
        parse_startup_mode("custom-boot"),
        Ok(StartupMode::CustomBoot)
    );
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
