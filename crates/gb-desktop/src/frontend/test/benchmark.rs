use super::*;

#[test]
fn benchmark_helpers_apply_cases_and_write_artifacts() {
    let root = temp_test_root("benchmark-artifact-helpers");
    let rom_path = root.join("bench.gb");
    let rom_bytes = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
    fs::write(&rom_path, &rom_bytes).expect("benchmark ROM should be writable");

    let mut options = DesktopRunOptions {
        rom_path: None,
        linked_peer_rom_path: None,
        benchmark_path: None,
        exit_after_frames: None,
        config: DesktopConfig::default(),
        audio_recording: None,
        test_runner: true,
    };
    options.config.video.display_palette = DesktopDisplayPalette::Light;
    let dmg_case = BenchmarkCase {
        source_path: root.join("test/bench.toml"),
        id: "bench".to_string(),
        run_id: Some("dmg".to_string()),
        run_label: Some("DMG run".to_string()),
        artifact_id: "bench-dmg".to_string(),
        rom: rom_path.clone(),
        model: BenchmarkModel::Dmg,
        startup: BenchmarkStartup::CustomBoot,
        mode: BenchmarkMode::Permissive,
        palette: Some(BenchmarkPalette::Grey),
        duration_seconds: 2,
        screenshot: true,
        stats: true,
        stimuli: Vec::new(),
    };
    super::super::apply_benchmark_case_to_desktop_options(&mut options, &dmg_case);
    assert_eq!(options.rom_path, Some(rom_path.clone()));
    assert_eq!(options.exit_after_frames, Some(120));
    assert_eq!(
        options.config.launch.console_model,
        DesktopConsoleModel::GameBoy
    );
    assert_eq!(options.config.launch.revision, HardwareRevision::DmgCpuC);
    assert_eq!(options.config.launch.startup_mode, StartupMode::CustomBoot);
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

    assert_eq!(
        super::super::desktop_model_from_benchmark(BenchmarkModel::Dmg),
        DesktopConsoleModel::GameBoy
    );
    assert_eq!(
        super::super::desktop_model_from_benchmark(BenchmarkModel::Mgb),
        DesktopConsoleModel::GameBoyPocket
    );
    assert_eq!(
        super::super::desktop_model_from_benchmark(BenchmarkModel::Lgb),
        DesktopConsoleModel::GameBoyLight
    );
    assert_eq!(
        super::super::desktop_model_from_benchmark(BenchmarkModel::Cgb),
        DesktopConsoleModel::GameBoyColor
    );
    assert_eq!(
        super::super::startup_mode_from_benchmark(BenchmarkStartup::SkipBoot),
        StartupMode::SkipBoot
    );
    assert_eq!(
        super::super::startup_mode_from_benchmark(BenchmarkStartup::CustomBoot),
        StartupMode::CustomBoot
    );
    assert_eq!(
        super::super::startup_mode_from_benchmark(BenchmarkStartup::RealBoot),
        StartupMode::RealBoot
    );
    assert_eq!(
        super::super::execution_mode_from_benchmark(BenchmarkMode::Strict),
        ExecutionMode::Strict
    );
    assert_eq!(
        super::super::execution_mode_from_benchmark(BenchmarkMode::Permissive),
        ExecutionMode::Permissive
    );
    assert_eq!(
        super::super::execution_mode_from_benchmark(BenchmarkMode::Experimental),
        ExecutionMode::Experimental
    );
    assert_eq!(
        super::super::desktop_display_palette_from_benchmark(BenchmarkPalette::Grey),
        DesktopDisplayPalette::Grey
    );

    let mut cgb_options = options.clone();
    cgb_options.config.video.display_palette = DesktopDisplayPalette::Light;
    let mut cgb_case = dmg_case.clone();
    cgb_case.model = BenchmarkModel::Cgb;
    cgb_case.startup = BenchmarkStartup::RealBoot;
    cgb_case.mode = BenchmarkMode::Experimental;
    super::super::apply_benchmark_case_to_desktop_options(&mut cgb_options, &cgb_case);
    assert_eq!(
        cgb_options.config.launch.console_model,
        DesktopConsoleModel::GameBoyColor
    );
    assert_eq!(
        cgb_options.config.launch.revision,
        HardwareRevision::CpuCgbE
    );
    assert_eq!(
        cgb_options.config.video.display_palette,
        DesktopDisplayPalette::Light
    );
    assert_eq!(
        cgb_options.config.launch.execution_mode,
        ExecutionMode::Permissive
    );
    assert_eq!(
        cgb_options.config.video.sgb_border,
        SgbBorderPresentationMode::Off
    );

    let mut agb_options = options.clone();
    let mut agb_case = dmg_case.clone();
    agb_case.model = BenchmarkModel::Agb;
    super::super::apply_benchmark_case_to_desktop_options(&mut agb_options, &agb_case);
    assert_eq!(
        agb_options.config.launch.console_model,
        DesktopConsoleModel::GameBoyAdvance
    );
    assert_eq!(
        agb_options.config.launch.revision,
        HardwareRevision::CpuAgbA
    );

    let machine = super::super::DesktopEmulationSession::new_single(
        super::super::load_machine_for_rom(&options.config, &root, &rom_bytes)
            .expect("benchmark machine should load")
            .machine,
    );
    let session = super::super::DesktopSession {
        config: options.config.clone(),
        test_runner: true,
        benchmark: Some(super::super::DesktopBenchmarkRun {
            case: dmg_case.clone(),
            stimuli: BenchmarkStimulusRuntime::new(Vec::new()),
            started_at: Instant::now(),
            started_t_cycle: 0,
        }),
        current_dir: root.clone(),
        loaded_rom: Some(super::super::LoadedRom {
            path: rom_path,
            bytes: rom_bytes,
        }),
        linked_secondary_rom: None,
        dmg07_player_count: None,
        cgb_infrared_link_active: false,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: PokemonPikachuColorGift::default(),
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: PokemonMysteryGiftKind::default(),
        pokemon_mystery_gift_code: PokemonMysteryGiftCode::default(),
        last_open_directory: Some(root.clone()),
        recent_roms: Vec::new(),
        pocket_camera_frame: None,
        external_port_selection: DesktopExternalPortSelection::None,
    };
    let performance_counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "benchmark".to_string(),
        super::super::EmulationProfileMode::Disabled,
    );
    super::super::write_benchmark_artifacts_for_session(
        &session,
        &machine,
        &session.config.video,
        &performance_counter,
    )
    .expect("benchmark artifacts should be written");
    assert!(root.join("gb-desktop/bench-dmg.png").exists());
    let stats = fs::read_to_string(root.join("gb-desktop/bench-dmg-stats.toml"))
        .expect("benchmark stats should be written");
    assert!(stats.contains("artifact_id = \"bench-dmg\""));
    assert!(stats.contains("run_label = \"DMG run\""));

    let no_benchmark_session = super::super::DesktopSession {
        benchmark: None,
        ..session
    };
    super::super::write_benchmark_artifacts_for_session(
        &no_benchmark_session,
        &machine,
        &no_benchmark_session.config.video,
        &performance_counter,
    )
    .expect("missing benchmark context should be a no-op");

    let nested_text_path = root.join("nested/artifact.txt");
    super::super::write_text_file_with_parent(&nested_text_path, "ok")
        .expect("text artifacts should create parent directories");
    assert_eq!(
        fs::read_to_string(nested_text_path).expect("text artifact should be readable"),
        "ok"
    );

    let suite_error = super::super::run_desktop_with_startup_fallback_persistence(
        DesktopRunOptions {
            benchmark_path: Some(root.join("missing-benchmark.toml")),
            ..options
        },
        DesktopSettingsStore::new_for_tests(root.join("missing-settings.toml")),
        false,
    )
    .expect_err("missing benchmark suites should fail before SDL startup");
    assert!(suite_error.contains("missing-benchmark.toml"));
}
