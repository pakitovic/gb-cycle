use super::super::*;

#[test]
fn helper_parsers_names_and_formatters_cover_supported_variants() {
    assert_eq!(RunModel::GameBoy.console_model(), ConsoleModel::GameBoy);
    assert_eq!(
        RunModel::Pocket.console_model(),
        ConsoleModel::GameBoyPocket
    );
    assert_eq!(RunModel::Light.console_model(), ConsoleModel::GameBoyLight);
    assert_eq!(RunModel::Color.console_model(), ConsoleModel::GameBoyColor);
    assert_eq!(
        RunModel::Advance.console_model(),
        ConsoleModel::GameBoyAdvance
    );
    assert_eq!(
        RunModel::SuperGameBoy.console_model(),
        ConsoleModel::GameBoy
    );
    assert_eq!(
        RunModel::SuperGameBoy2.console_model(),
        ConsoleModel::GameBoy
    );
    assert_eq!(
        RunModel::SuperGameBoy.sgb_profile(),
        Some(SgbHostProfile::SgbNtsc)
    );
    assert_eq!(
        RunModel::SuperGameBoy.sgb_profile_for_standard(SgbVideoStandard::Ntsc),
        Some(SgbHostProfile::SgbNtsc)
    );
    assert_eq!(
        RunModel::SuperGameBoy.sgb_profile_for_standard(SgbVideoStandard::Pal),
        Some(SgbHostProfile::SgbPal)
    );
    assert_eq!(
        RunModel::SuperGameBoy2.sgb_profile(),
        Some(SgbHostProfile::Sgb2Ntsc)
    );
    assert_eq!(
        RunModel::SuperGameBoy2.sgb_profile_for_standard(SgbVideoStandard::Pal),
        Some(SgbHostProfile::Sgb2Ntsc)
    );
    assert_eq!(RunModel::GameBoy.name(), "DMG");
    assert_eq!(
        RunModel::GameBoy.console_model().default_revision(),
        HardwareRevision::DmgCpuC
    );
    assert_eq!(RunModel::Pocket.name(), "MGB");
    assert_eq!(
        RunModel::Pocket.console_model().default_revision(),
        HardwareRevision::CpuMgb
    );
    assert_eq!(RunModel::Light.name(), "LGB");
    assert_eq!(
        RunModel::Light.console_model().default_revision(),
        HardwareRevision::CpuMgb
    );
    assert_eq!(RunModel::Color.name(), "CGB");
    assert_eq!(
        RunModel::Color.console_model().default_revision(),
        HardwareRevision::CpuCgbE
    );
    assert_eq!(RunModel::Advance.name(), "AGB");
    assert_eq!(
        RunModel::Advance.console_model().default_revision(),
        HardwareRevision::CpuAgbA
    );
    assert_eq!(RunModel::SuperGameBoy.name(), "SGB");
    assert_eq!(RunModel::SuperGameBoy2.name(), "SGB2");
    assert_eq!(SavePolicy::Manual.name(), "manual");
    assert_eq!(SavePolicy::OnClose.name(), "on-close");
    assert_eq!(SavePolicy::OnWrite.name(), "on-write");
    assert_eq!(
        DefaultRunBudget::for_startup_mode(StartupMode::SkipBoot),
        DefaultRunBudget::SkipBootFrames {
            frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
        }
    );
    assert_eq!(
        DefaultRunBudget::for_startup_mode(StartupMode::CustomBoot),
        DefaultRunBudget::SkipBootFrames {
            frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
        }
    );
    assert_eq!(
        DefaultRunBudget::for_startup_mode(StartupMode::RealBoot),
        DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }
    );

    assert_eq!(
        compatibility_for_execution_mode(ExecutionMode::Strict),
        CompatibilityPolicy::strict()
    );
    assert_eq!(
        compatibility_for_execution_mode(ExecutionMode::Permissive),
        CompatibilityPolicy::permissive()
    );
    assert_eq!(
        compatibility_for_execution_mode(ExecutionMode::Experimental),
        CompatibilityPolicy::experimental()
    );

    assert_eq!(
        run_model_from_benchmark(BenchmarkModel::Dmg),
        RunModel::GameBoy
    );
    assert_eq!(
        run_model_from_benchmark(BenchmarkModel::Mgb),
        RunModel::Pocket
    );
    assert_eq!(
        run_model_from_benchmark(BenchmarkModel::Lgb),
        RunModel::Light
    );
    assert_eq!(
        run_model_from_benchmark(BenchmarkModel::Cgb),
        RunModel::Color
    );
    assert_eq!(
        run_model_from_benchmark(BenchmarkModel::Agb),
        RunModel::Advance
    );
    assert_eq!(
        startup_mode_from_benchmark(BenchmarkStartup::SkipBoot),
        StartupMode::SkipBoot
    );
    assert_eq!(
        startup_mode_from_benchmark(BenchmarkStartup::CustomBoot),
        StartupMode::CustomBoot
    );
    assert_eq!(
        startup_mode_from_benchmark(BenchmarkStartup::RealBoot),
        StartupMode::RealBoot
    );
    assert_eq!(
        execution_mode_from_benchmark(BenchmarkMode::Strict),
        ExecutionMode::Strict
    );
    assert_eq!(
        execution_mode_from_benchmark(BenchmarkMode::Permissive),
        ExecutionMode::Permissive
    );
    assert_eq!(
        execution_mode_from_benchmark(BenchmarkMode::Experimental),
        ExecutionMode::Experimental
    );
    assert_eq!(
        display_palette_from_benchmark(BenchmarkPalette::Grey),
        RunDisplayPalette::Grey
    );

    assert_eq!(parse_run_model("DMG"), Ok(RunModel::GameBoy));
    assert_eq!(parse_run_model("MGB"), Ok(RunModel::Pocket));
    assert_eq!(parse_run_model("LGB"), Ok(RunModel::Light));
    assert_eq!(parse_run_model("CGB"), Ok(RunModel::Color));
    assert_eq!(parse_run_model("AGB"), Ok(RunModel::Advance));
    assert_eq!(parse_run_model("SGB"), Ok(RunModel::SuperGameBoy));
    assert_eq!(parse_run_model("SGB2"), Ok(RunModel::SuperGameBoy2));
    assert_eq!(parse_sgb_video_standard("ntsc"), Ok(SgbVideoStandard::Ntsc));
    assert_eq!(parse_sgb_video_standard("pal"), Ok(SgbVideoStandard::Pal));
    assert!(
        parse_sgb_video_standard("secam")
            .expect_err("unsupported SGB standards should fail")
            .contains("unsupported --sgb-standard value")
    );
    for previous in [
        "game-boy", "pocket", "light", "color", "dmg0", "dmg", "mgb", "cgb",
    ] {
        let error = parse_run_model(previous).expect_err("previous models should fail");
        assert!(error.contains("unsupported --model value"));
        assert!(error.contains("DMG, MGB, LGB, CGB, AGB, SGB, SGB2"));
        assert!(!error.contains("game-boy, pocket, light, color"));
    }
    assert!(
        parse_run_model("sgb")
            .expect_err("unsupported models should fail")
            .contains("unsupported --model value")
    );
    assert_eq!(parse_revision("dmg-cpu-c"), Ok(HardwareRevision::DmgCpuC));
    assert_eq!(parse_revision("cpu-mgb"), Ok(HardwareRevision::CpuMgb));
    assert_eq!(parse_revision("cpu-cgb-c"), Ok(HardwareRevision::CpuCgbC));
    assert_eq!(parse_revision("cpu-cgb-d"), Ok(HardwareRevision::CpuCgbD));
    assert_eq!(parse_revision("cpu-cgb-e"), Ok(HardwareRevision::CpuCgbE));
    assert_eq!(parse_revision("cpu-agb-a"), Ok(HardwareRevision::CpuAgbA));
    assert!(
        parse_revision("cpu-cgb-b")
            .expect_err("inactive revisions should fail")
            .contains("unsupported --revision value")
    );
    assert_eq!(revision_argument_name(HardwareRevision::DmgCpu), "dmg-cpu");
    assert_eq!(
        revision_argument_name(HardwareRevision::DmgCpuA),
        "dmg-cpu-a"
    );
    assert_eq!(
        revision_argument_name(HardwareRevision::DmgCpuB),
        "dmg-cpu-b"
    );
    assert_eq!(
        revision_argument_name(HardwareRevision::DmgCpuC),
        "dmg-cpu-c"
    );
    assert_eq!(revision_argument_name(HardwareRevision::CpuMgb), "cpu-mgb");
    assert_eq!(revision_argument_name(HardwareRevision::CpuCgb), "cpu-cgb");
    assert_eq!(
        revision_argument_name(HardwareRevision::CpuCgbA),
        "cpu-cgb-a"
    );
    assert_eq!(
        revision_argument_name(HardwareRevision::CpuCgbB),
        "cpu-cgb-b"
    );
    assert_eq!(
        revision_argument_name(HardwareRevision::CpuCgbC),
        "cpu-cgb-c"
    );
    assert_eq!(
        revision_argument_name(HardwareRevision::CpuCgbD),
        "cpu-cgb-d"
    );
    assert_eq!(
        revision_argument_name(HardwareRevision::CpuCgbE),
        "cpu-cgb-e"
    );
    assert_eq!(
        revision_argument_name(HardwareRevision::CpuAgbA),
        "cpu-agb-a"
    );
    assert_eq!(
        supported_revision_names(ConsoleModel::GameBoyColor),
        "cpu-cgb-c, cpu-cgb-d, cpu-cgb-e"
    );
    assert_eq!(
        supported_revision_names(ConsoleModel::GameBoyAdvance),
        "cpu-agb-a"
    );
    assert_eq!(parse_display_palette("grey"), Ok(RunDisplayPalette::Grey));
    assert_eq!(
        parse_display_palette("green").expect_err("unsupported palettes should fail"),
        "unsupported --palette value \"green\"; expected grey"
    );
    assert_eq!(
        RunDisplayPalette::Grey.display_palette().shade_rgb(0),
        [255; 3]
    );
    assert_eq!(
        RunDisplayPalette::Grey.display_palette().shade_rgb(9),
        [0; 3]
    );
    assert_eq!(RunDisplayPalette::Grey.display_palette().shade_luma(2), 85);

    assert_eq!(parse_startup_mode("skip-boot"), Ok(StartupMode::SkipBoot));
    assert_eq!(
        parse_startup_mode("custom-boot"),
        Ok(StartupMode::CustomBoot)
    );
    assert_eq!(parse_startup_mode("real-boot"), Ok(StartupMode::RealBoot));
    assert!(
        parse_startup_mode("boot")
            .expect_err("unsupported startup modes should fail")
            .contains("unsupported --startup value")
    );

    assert_eq!(parse_execution_mode("strict"), Ok(ExecutionMode::Strict));
    assert_eq!(
        parse_execution_mode("permissive"),
        Ok(ExecutionMode::Permissive)
    );
    assert_eq!(
        parse_execution_mode("experimental"),
        Ok(ExecutionMode::Experimental)
    );
    assert!(
        parse_execution_mode("oracle")
            .expect_err("unsupported execution modes should fail")
            .contains("unsupported --mode value")
    );

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
    assert!(
        parse_boot_rom_verification_mode("auto")
            .expect_err("unsupported verification modes should fail")
            .contains("unsupported --boot-rom-verify value")
    );

    assert_eq!(parse_save_policy("manual"), Ok(SavePolicy::Manual));
    assert_eq!(parse_save_policy("on-close"), Ok(SavePolicy::OnClose));
    assert_eq!(parse_save_policy("on-write"), Ok(SavePolicy::OnWrite));
    assert!(
        parse_save_policy("always")
            .expect_err("unsupported save policies should fail")
            .contains("unsupported --save-policy value")
    );

    assert_eq!(parse_positive_u32("--frames", "5"), Ok(5));
    assert_eq!(
        parse_positive_u32("--frames", "0"),
        Err("--frames must be greater than zero".to_string())
    );
    assert!(
        parse_positive_u32("--frames", "abc")
            .expect_err("invalid u32 values should fail")
            .contains("invalid --frames value")
    );

    assert_eq!(parse_positive_u64("--tcycles", "9"), Ok(9));
    assert_eq!(
        parse_positive_u64("--tcycles", "0"),
        Err("--tcycles must be greater than zero".to_string())
    );
    assert!(
        parse_positive_u64("--tcycles", "abc")
            .expect_err("invalid u64 values should fail")
            .contains("invalid --tcycles value")
    );

    assert!(run_limit_reached(Some(2), None, 2, 0));
    assert!(run_limit_reached(None, Some(3), 0, 3));
    assert!(!run_limit_reached(None, None, 0, 0));

    assert_eq!(startup_mode_name(StartupMode::CustomBoot), "custom-boot");
    assert_eq!(startup_mode_name(StartupMode::RealBoot), "real-boot");
    assert_eq!(execution_mode_name(ExecutionMode::Strict), "strict");
    assert_eq!(execution_mode_name(ExecutionMode::Permissive), "permissive");
    assert_eq!(
        execution_mode_name(ExecutionMode::Experimental),
        "experimental"
    );
    assert_eq!(
        diagnostic_severity_name(CartridgeDiagnosticSeverity::Warning),
        "warning"
    );
    assert_eq!(
        diagnostic_severity_name(CartridgeDiagnosticSeverity::Error),
        "error"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Supported(
            SupportedCartridgeFamily::NoMbc
        )),
        "supported"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::PlannedVariant
        )),
        "unsupported-planned-variant"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::DocumentedButUnsupported
        )),
        "unsupported-documented"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::ExperimentalHeuristic
        )),
        "unsupported-experimental-heuristic"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::AccessorySpecialCase
        )),
        "unsupported-accessory"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::UnknownCode
        )),
        "unsupported-unknown"
    );
    assert_eq!(cgb_flag_name(CgbFlag::None), "none");
    assert_eq!(cgb_flag_name(CgbFlag::Supported), "supported");
    assert_eq!(cgb_flag_name(CgbFlag::Only), "only");
    assert_eq!(
        cgb_flag_name(CgbFlag::SupportedNonCanonical(0xA0)),
        "supported-noncanonical(0xA0)"
    );
    assert_eq!(cgb_flag_name(CgbFlag::Unknown(0x42)), "unknown(0x42)");
    assert_eq!(sgb_flag_name(SgbFlag::None), "none");
    assert_eq!(sgb_flag_name(SgbFlag::Supported), "supported");
    assert_eq!(sgb_flag_name(SgbFlag::Unknown(0x03)), "unknown(0x03)");
    assert_eq!(optional_usize_name(Some(8)), "8");
    assert_eq!(optional_usize_name(None), "unknown");
    for revision in [
        HardwareRevision::DmgCpu,
        HardwareRevision::DmgCpuA,
        HardwareRevision::DmgCpuB,
        HardwareRevision::DmgCpuC,
        HardwareRevision::CpuMgb,
        HardwareRevision::CpuCgb,
        HardwareRevision::CpuCgbA,
        HardwareRevision::CpuCgbB,
        HardwareRevision::CpuCgbC,
        HardwareRevision::CpuCgbD,
        HardwareRevision::CpuCgbE,
        HardwareRevision::CpuAgbA,
    ] {
        assert_eq!(revision.boot_rom_expected_sha256().len(), 64);
    }
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
