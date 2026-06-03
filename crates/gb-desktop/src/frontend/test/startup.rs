use super::*;

#[test]
fn desktop_mode_labels_cover_all_public_variants() {
    assert_eq!(
        super::super::EmulationProfileSessionKind::Single.label(),
        "single"
    );
    assert_eq!(
        super::super::EmulationProfileSessionKind::LinkedDmg04TwoPlayer.label(),
        "linked-dmg04-2p"
    );
    assert_eq!(
        super::super::EmulationProfileSessionKind::LinkedCgbInfraredTwoPlayer.label(),
        "linked-cgb-ir-2p"
    );
    assert_eq!(
        super::super::EmulationProfileSessionKind::LinkedDmg07.label(),
        "linked-dmg07"
    );
    assert_eq!(
        super::super::startup_mode_name(StartupMode::SkipBoot),
        "skip-boot"
    );
    assert_eq!(
        super::super::startup_mode_name(StartupMode::CustomBoot),
        "custom-boot"
    );
    assert_eq!(
        super::super::startup_mode_name(StartupMode::RealBoot),
        "real-boot"
    );
    assert_eq!(
        super::super::execution_mode_name(ExecutionMode::Strict),
        "strict"
    );
    assert_eq!(
        super::super::execution_mode_name(ExecutionMode::Permissive),
        "permissive"
    );
    assert_eq!(
        super::super::execution_mode_name(ExecutionMode::Experimental),
        "experimental"
    );
    assert_eq!(
        super::super::diagnostic_severity_name(CartridgeDiagnosticSeverity::Warning),
        "warning"
    );
    assert_eq!(
        super::super::diagnostic_severity_name(CartridgeDiagnosticSeverity::Error),
        "error"
    );
}

#[test]
fn run_desktop_supports_headless_startup_with_and_without_an_initial_rom() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();

    let launcher_root = temp_test_root("headless-launcher");
    let mut launcher_config = DesktopConfig::default();
    launcher_config.input.gamepad.enabled = false;
    let launcher_store =
        DesktopSettingsStore::new_for_tests(launcher_root.join("desktop-settings.toml"));
    let launcher_quit = schedule_quit_event();
    run_desktop(
        DesktopRunOptions {
            rom_path: None,
            linked_peer_rom_path: None,
            benchmark_path: None,
            exit_after_frames: None,
            config: launcher_config,
            audio_recording: None,
            test_runner: false,
        },
        launcher_store,
    )
    .expect("launcher should start and stop cleanly under headless SDL");
    launcher_quit
        .join()
        .expect("launcher quit-event helper should finish");

    let rom_root = temp_test_root("headless-rom");
    let rom_path = write_test_rom(&rom_root, "headless.gb");
    crate::configure_headless_sdl();
    let mut rom_config = DesktopConfig::default();
    rom_config.input.gamepad.enabled = false;
    let rom_store = DesktopSettingsStore::new_for_tests(rom_root.join("desktop-settings.toml"));
    let rom_quit = schedule_quit_event();
    run_desktop(
        DesktopRunOptions {
            rom_path: Some(rom_path),
            linked_peer_rom_path: None,
            benchmark_path: None,
            exit_after_frames: None,
            config: rom_config,
            audio_recording: None,
            test_runner: false,
        },
        rom_store,
    )
    .expect("ROM startup should run and stop cleanly under headless SDL");
    rom_quit
        .join()
        .expect("ROM quit-event helper should finish");
}

#[test]
fn run_desktop_test_runner_exits_after_frames_without_settings_writes() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();

    let root = temp_test_root("headless-test-runner");
    let rom_path = root.join("test-runner.gb");
    let mut rom = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 2].copy_from_slice(&[0x18, 0xFE]);
    fs::write(&rom_path, rom).expect("test-runner ROM should be writable");
    let settings_path = root.join("desktop-settings.toml");
    let mut config = DesktopConfig::default();
    config.input.gamepad.enabled = false;
    config.audio.enabled = false;
    config.video.vsync = false;

    run_desktop(
        DesktopRunOptions {
            rom_path: Some(rom_path),
            linked_peer_rom_path: None,
            benchmark_path: None,
            exit_after_frames: Some(2),
            config,
            audio_recording: None,
            test_runner: true,
        },
        DesktopSettingsStore::new_for_tests(settings_path.clone()),
    )
    .expect("test-runner mode should exit through presented-frame counting");

    assert!(
        !settings_path.exists(),
        "test-runner mode should not persist settings or recent ROMs"
    );
}

#[test]
fn run_desktop_writes_audio_recordings_and_stems() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();

    let root = temp_test_root("headless-audio-recording");
    let rom_path = write_test_rom(&root, "audio-recording.gb");
    let output_path = root.join("audio-recording.wav");
    let stem_ch1_path = root.join("audio-recording.ch1.wav");
    let stem_ch4_path = root.join("audio-recording.ch4.wav");

    let mut config = DesktopConfig::default();
    config.boot_rom.verification = BootRomVerificationMode::Off;
    config.input.gamepad.enabled = false;
    config.audio.enabled = false;
    let quit = schedule_quit_event();

    run_desktop(
        DesktopRunOptions {
            rom_path: Some(rom_path),
            linked_peer_rom_path: None,
            benchmark_path: None,
            exit_after_frames: None,
            config,
            audio_recording: Some(DesktopAudioRecordingOptions {
                output_path: output_path.clone(),
                sample_rate_hz: 96_000,
                stem_channels: vec![ApuRecordedChannel::Ch1, ApuRecordedChannel::Ch4],
            }),
            test_runner: false,
        },
        DesktopSettingsStore::new_for_tests(root.join("desktop-settings.toml")),
    )
    .expect("audio-recording run should complete");
    quit.join()
        .expect("audio-recording quit-event helper should finish");

    let mix_len = fs::metadata(&output_path)
        .expect("mixed recording should exist")
        .len();
    let ch1_len = fs::metadata(&stem_ch1_path)
        .expect("ch1 stem should exist")
        .len();
    let ch4_len = fs::metadata(&stem_ch4_path)
        .expect("ch4 stem should exist")
        .len();
    assert!(mix_len > 44);
    assert!(ch1_len > 44);
    assert!(ch4_len > 44);
}

#[test]
fn load_initial_emulation_session_supports_direct_linked_startup() {
    let root = temp_test_root("direct-linked-startup");
    let primary_rom_path = write_test_rom(&root, "primary.gb");
    let secondary_rom_path = write_test_rom(&root, "secondary.gb");
    let primary_bytes = fs::read(&primary_rom_path).expect("primary ROM should exist");
    let secondary_bytes = fs::read(&secondary_rom_path).expect("secondary ROM should exist");
    let mut session = super::super::DesktopSession {
        config: DesktopConfig::default(),
        test_runner: false,
        benchmark: None,
        current_dir: root.clone(),
        loaded_rom: Some(super::super::LoadedRom {
            path: primary_rom_path,
            bytes: primary_bytes,
        }),
        linked_secondary_rom: Some(super::super::LoadedRom {
            path: secondary_rom_path,
            bytes: secondary_bytes,
        }),
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
        external_port_selection: super::super::DesktopExternalPortSelection::GameLink,
    };

    let (machine, diagnostics) = super::super::load_initial_emulation_session(&mut session)
        .expect("linked desktop startup helper should build a DMG-04 session");

    assert!(diagnostics.is_empty());
    assert!(machine.is_linked_dmg04_two_player());
    assert!(machine.secondary_machine().is_some());
}

#[test]
fn load_initial_emulation_session_supports_direct_cgb_ir_startup() {
    run_with_large_test_stack(
        "direct-cgb-ir-startup",
        load_initial_emulation_session_supports_direct_cgb_ir_startup_inner,
    );
}

#[test]
fn prepare_machine_config_falls_back_to_skip_boot_when_the_selected_boot_rom_is_missing() {
    let root = temp_test_root("missing-bootrom-fallback");
    let mut config = DesktopConfig::default();
    config.launch.startup_mode = StartupMode::RealBoot;
    config.boot_rom.verification = BootRomVerificationMode::Strict;
    config.boot_rom.search_path = Some(root.join("missing-dmg.bin"));

    let prepared = super::super::prepare_machine_config(&config, &root)
        .expect("missing boot ROM paths should degrade to skip-boot");

    assert_eq!(
        prepared.effective_config.launch.startup_mode,
        StartupMode::SkipBoot
    );
    assert_eq!(prepared.machine_config.startup_mode, StartupMode::SkipBoot);
    assert!(prepared.machine_config.boot_rom_assets.is_empty());
    assert!(
        prepared
            .boot_rom_fallback_warning
            .as_deref()
            .is_some_and(|warning| warning.contains("falling back to skip-boot"))
    );
}

#[test]
fn prepare_machine_config_falls_back_to_skip_boot_when_boot_rom_path_is_unconfigured() {
    let root = temp_test_root("unconfigured-bootrom-fallback");
    let mut config = DesktopConfig::default();
    config.launch.startup_mode = StartupMode::RealBoot;
    config.boot_rom.verification = BootRomVerificationMode::Strict;

    let prepared = super::super::prepare_machine_config(&config, &root)
        .expect("unconfigured boot ROM paths should degrade to skip-boot");

    assert_eq!(
        prepared.effective_config.launch.startup_mode,
        StartupMode::SkipBoot
    );
    assert_eq!(prepared.machine_config.startup_mode, StartupMode::SkipBoot);
    assert!(prepared.machine_config.boot_rom_assets.is_empty());
    assert_eq!(
        prepared.boot_rom_fallback_warning.as_deref(),
        Some(
            "boot ROM root is not configured; choose a boot ROM directory; falling back to skip-boot"
        )
    );
}

#[test]
fn prepare_machine_config_uses_sgb_boot_asset_identity_before_real_boot_fallback() {
    let root = temp_test_root("missing-sgb2-bootrom-fallback");
    let mut config = DesktopConfig::default();
    config.launch.console_model = DesktopConsoleModel::SuperGameBoy2;
    config.launch.startup_mode = StartupMode::RealBoot;
    config.boot_rom.verification = BootRomVerificationMode::Strict;
    config.boot_rom.search_path = Some(root.clone());

    let prepared = super::super::prepare_machine_config(&config, &root)
        .expect("missing SGB2 boot ROM paths should degrade to skip-boot");

    assert_eq!(
        prepared.effective_config.launch.startup_mode,
        StartupMode::SkipBoot
    );
    assert_eq!(
        prepared.machine_config.sgb_profile,
        Some(SgbHostProfile::Sgb2Ntsc)
    );
    assert_eq!(
        prepared.machine_config.boot_rom_asset_kind(),
        BootRomAssetKind::Sgb2
    );
    assert!(
        prepared
            .boot_rom_fallback_warning
            .as_deref()
            .is_some_and(|warning| warning.contains("sgb2_boot.bin"))
    );
}

#[test]
fn prepare_machine_config_keeps_strict_real_boot_errors_for_existing_invalid_images() {
    let root = temp_test_root("invalid-bootrom-strict");
    let image_path = root.join("dmg_boot.bin");
    fs::write(&image_path, vec![0x99; 0x100]).expect("synthetic boot ROM image should exist");

    let mut config = DesktopConfig::default();
    config.launch.startup_mode = StartupMode::RealBoot;
    config.boot_rom.verification = BootRomVerificationMode::Strict;
    config.boot_rom.search_path = Some(image_path);

    let error = super::super::prepare_machine_config(&config, &root)
        .expect_err("strict real-boot should still reject invalid existing images");
    assert!(error.contains("unexpected sha256"));
}

#[test]
fn prepare_machine_config_applies_full_execution_mode_policy() {
    let root = temp_test_root("desktop-permissive-policy");
    let mut config = DesktopConfig::default();
    config.launch.execution_mode = ExecutionMode::Permissive;

    let prepared = super::super::prepare_machine_config(&config, &root)
        .expect("skip-boot permissive machine config should prepare");
    assert_eq!(
        prepared.machine_config.compatibility,
        config.launch.compatibility_policy()
    );

    let legacy_mbc1_ram_header = build_test_rom(32 * 1024, 0x02, 0x00, 0x00);
    let loaded = super::super::load_machine_for_rom(&config, &root, &legacy_mbc1_ram_header)
        .expect("permissive desktop loading should warn instead of rejecting");

    assert!(loaded.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == CartridgeDiagnosticSeverity::Warning
            && diagnostic
                .message
                .contains("contradicts the current MBC1+RAM Standard wiring baseline")
    }));
}

#[test]
fn run_desktop_persists_skip_boot_after_missing_boot_rom_startup_fallback() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();

    let root = temp_test_root("startup-fallback-persist");
    let settings_path = root.join("desktop-settings.toml");
    let mut settings_store = DesktopSettingsStore::new_for_tests(settings_path.clone());
    let mut config = DesktopConfig::default();
    config.launch.startup_mode = StartupMode::RealBoot;
    config.boot_rom.verification = BootRomVerificationMode::Strict;
    config.boot_rom.search_path = Some(root.join("missing-boot.bin"));
    config.input.gamepad.enabled = false;
    settings_store
        .persist_machine_preferences(&config)
        .expect("stale real-boot settings should persist");

    let quit = schedule_quit_event();
    super::super::run_desktop_with_startup_fallback_persistence(
        DesktopRunOptions {
            rom_path: None,
            linked_peer_rom_path: None,
            benchmark_path: None,
            exit_after_frames: None,
            config,
            audio_recording: None,
            test_runner: false,
        },
        settings_store,
        true,
    )
    .expect("desktop should start after degrading the missing boot ROM");
    quit.join()
        .expect("startup fallback quit-event helper should finish");

    let persisted = fs::read_to_string(&settings_path).expect("desktop settings should persist");
    assert!(persisted.contains("startup_mode = \"skip-boot\""));
    assert!(!persisted.contains("startup_mode = \"real-boot\""));
}

#[test]
fn run_desktop_processes_hotkeys_plus_video_and_audio_menu_actions() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();

    let root = temp_test_root("video-audio-actions");
    let rom_path = write_test_rom(&root, "video-audio.gb");
    let settings_path = root.join("desktop-settings.toml");
    let mut config = DesktopConfig::default();
    config.input.gamepad.enabled = false;
    let settings_store = DesktopSettingsStore::new_for_tests(settings_path.clone());
    let sequence = schedule_key_sequence(vec![
        (Keycode::F11, true),
        (Keycode::Z, true),
        (Keycode::Z, false),
        (Keycode::Escape, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Escape, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Return, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
    ]);

    run_desktop(
        DesktopRunOptions {
            rom_path: Some(rom_path.clone()),
            linked_peer_rom_path: None,
            benchmark_path: None,
            exit_after_frames: None,
            config,
            audio_recording: None,
            test_runner: false,
        },
        settings_store,
    )
    .expect("desktop frontend should process hotkeys and video/audio menu actions");
    sequence
        .join()
        .expect("video/audio key sequence helper should finish");

    let persisted = fs::read_to_string(&settings_path)
        .expect("desktop settings should persist after menu-driven changes");
    assert!(persisted.contains("fullscreen = true"));
    assert!(persisted.contains(&rom_path.display().to_string()));
}

#[test]
fn run_desktop_processes_input_and_system_menu_actions() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();

    let root = temp_test_root("input-system-actions");
    let rom_path = write_test_rom(&root, "input-system.gb");
    let settings_path = root.join("desktop-settings.toml");
    let mut config = DesktopConfig::default();
    config.input.gamepad.enabled = false;
    let settings_store = DesktopSettingsStore::new_for_tests(settings_path.clone());
    let sequence = schedule_key_sequence(vec![
        (Keycode::Escape, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Escape, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Return, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
        (Keycode::Down, true),
        (Keycode::Return, true),
    ]);

    run_desktop(
        DesktopRunOptions {
            rom_path: Some(rom_path.clone()),
            linked_peer_rom_path: None,
            benchmark_path: None,
            exit_after_frames: None,
            config,
            audio_recording: None,
            test_runner: false,
        },
        settings_store,
    )
    .expect("desktop frontend should process input and system menu actions");
    sequence
        .join()
        .expect("input/system key sequence helper should finish");

    let persisted = fs::read_to_string(&settings_path)
        .expect("desktop settings should persist after input/system changes");
    assert!(persisted.contains("version = 1"));
    assert!(persisted.contains(&rom_path.display().to_string()));
}

#[test]
fn frontend_helpers_cover_runtime_dialog_and_title_utilities() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("helpers", true, false, false);

    assert!(harness.session.has_loaded_rom());
    assert!(harness.session.rom_path().is_some());
    assert!(harness.session.rom_bytes().is_some());
    assert_eq!(harness.session.rom_directory_hint(), harness.root.as_path());
    assert!(harness.session.recent_roms().is_empty());
    assert!(!harness.runtime.any_dialog_pending());
    assert_eq!(
        super::super::EXTERNAL_SAVE_FILE_DIALOG_FILTERS[0].pattern,
        "sav;sa1;sa2;sa3;sa4"
    );
    harness.runtime.external_save_export_dialog.pending = true;
    assert!(harness.runtime.any_dialog_pending());
    harness.runtime.external_save_export_dialog.pending = false;
    harness.runtime.external_save_import_dialog.pending = true;
    assert!(harness.runtime.any_dialog_pending());
    harness.runtime.external_save_import_dialog.pending = false;

    let mut dialog = super::super::PathSelectionDialog::new();
    assert!(!dialog.is_pending());
    dialog.pending = true;
    dialog
        .show_file(
            &ROM_FILE_DIALOG_FILTERS,
            harness.canvas.window(),
            harness.root.as_path(),
        )
        .expect("pending file dialogs should be a no-op");
    dialog
        .show_save_file(
            &super::super::EXTERNAL_SAVE_FILE_DIALOG_FILTERS,
            harness.canvas.window(),
            harness.root.as_path(),
        )
        .expect("pending save file dialogs should be a no-op");
    dialog.show_folder(harness.canvas.window(), harness.root.as_path());
    dialog.pending = false;
    dialog
        .sender
        .send(super::super::PathDialogResult::Selected(
            harness.root.join("picked.gb"),
        ))
        .expect("dialog result should send");
    assert!(matches!(
        dialog.take_result(),
        Some(super::super::PathDialogResult::Selected(_))
    ));

    harness.performance_counter.sample_started_at = Instant::now() - Duration::from_secs(2);
    harness
        .performance_counter
        .record_presented_frame(
            harness.canvas.window_mut(),
            super::super::FramePerformanceSample {
                session_kind: super::super::EmulationProfileSessionKind::Single,
                emulation_duration: Duration::from_millis(10),
                emulation_profile_request: None,
                render_duration: Duration::from_millis(2),
                present_duration: Duration::from_millis(1),
                pacing_duration: Duration::from_millis(4),
                pacing_sleep_target_duration: Duration::from_millis(4),
                pacing_audio_correction_duration: Duration::from_millis(1),
                pacing_late_duration: Duration::from_millis(2),
                pacing_oversleep_duration: Duration::from_millis(1),
                audio_submit_sample_count: Some(804),
                audio_submit_t_cycles: Some(70_224),
                audio_submit_queue_before_ms: Some(24.0),
                audio_submit_enqueued_ms: Some(4.0),
                audio_submit_queue_after_ms: Some(28.0),
                audio_queue_before_pacing_ms: Some(20.0),
                audio_queue_after_pacing_ms: Some(18.0),
                speed_mode: Some(CgbSpeedMode::Normal),
                frame_step_t_cycles: Some(70_224),
                frame_video_dots: Some(70_224),
                frame_start_ly: Some(0),
                frame_start_dot: Some(0),
                frame_end_ly: Some(0),
                frame_end_dot: Some(0),
                frame_origin_crossings: Some(1),
                scanline_transitions: Some(154),
                scanlines_over_456: Some(0),
                max_scanline_t_cycles: Some(456),
                max_scanline_ly: Some(153),
                max_mode0_start_dot: Some(252),
                max_mode0_start_dot_ly: Some(5),
                ly_153_to_0_transitions: Some(1),
                ly_153_to_0_startup_mode0: Some(0),
                ly_153_to_0_blank_frame: Some(0),
                ly_0_self_wraps: Some(0),
                ly_0_self_wrap_startup_mode0: Some(0),
                ly_0_self_wrap_blank_frame: Some(0),
                ly_0_to_1_transitions: Some(1),
                ly_0_scanline_t_cycles: Some(456),
                ly_0_max_mode0_start_dot: Some(254),
                ly_0_stall_t_cycles: Some(0),
                ly_0_stall_hblank_t_cycles: Some(0),
                ly_0_stall_oam_t_cycles: Some(0),
                ly_0_stall_drawing_t_cycles: Some(0),
                ly_0_stall_startup_mode0_t_cycles: Some(0),
                ly_0_stall_blank_frame_t_cycles: Some(0),
                ly_0_stall_runs: Some(0),
                ly_0_max_stall_run_t_cycles: Some(0),
                ly_0_max_stall_dot: Some(0),
                ly_0_max_stall_mode_dot: Some(0),
                cpu_stop_t_cycles: Some(0),
                cpu_zombie_stop_t_cycles: Some(0),
                ly_0_cpu_stop_t_cycles: Some(0),
                ly_0_cpu_zombie_stop_t_cycles: Some(0),
                ly_0_stall_cpu_stop_t_cycles: Some(0),
                ly_0_stall_cpu_zombie_stop_t_cycles: Some(0),
                lcd_disabled_t_cycles: Some(0),
                lcd_disable_transitions: Some(0),
                lcd_enable_transitions: Some(0),
                ly_0_lcd_disabled_t_cycles: Some(0),
                ly_0_stall_lcd_disabled_t_cycles: Some(0),
            },
        )
        .expect("performance counter should record a frame");
    assert!(harness.performance_counter.hud_snapshot().is_some());
    harness
        .performance_counter
        .reset_base_title(
            harness.canvas.window_mut(),
            "gb-desktop | reset".to_string(),
        )
        .expect("resetting the window title should succeed");

    super::super::show_message_box(
        None,
        sdl3::messagebox::MessageBoxFlag::WARNING,
        "warn",
        "msg",
    );
    super::super::show_warning_message(None, "warn", "msg");
    super::super::show_error_message(None, "error", "msg");
    assert_eq!(
        super::super::diagnostic_severity_name(CartridgeDiagnosticSeverity::Warning),
        "warning"
    );
    super::super::write_cartridge_diagnostics(&[CartridgeDiagnostic {
        severity: CartridgeDiagnosticSeverity::Warning,
        message: "test warning".to_string(),
    }]);
    assert!(super::super::target_frame_rate_hz() > 0.0);
    assert_eq!(super::super::gamepad_event_joystick_id(7).0, 7);
    assert_eq!(
        super::super::boot_rom_dialog_default_location(&harness.session),
        harness.root
    );
    harness.session.config.boot_rom.search_path = Some(PathBuf::from("custom/boot.bin"));
    assert_eq!(
        super::super::boot_rom_dialog_default_location(&harness.session),
        harness.root.join("custom")
    );
    assert_eq!(
        super::super::save_directory_dialog_default_location(&harness.session),
        harness.root
    );
    assert_eq!(
        super::super::external_save_export_dialog_default_location(&harness.session),
        harness.root.join("saves/export/helpers.sav")
    );
    assert_eq!(
        super::super::external_save_import_dialog_default_location(&harness.session),
        harness.root.join("saves/import/helpers.sav")
    );
    harness.session.config.saves.directory_policy =
        gb_desktop::SaveDirectoryPolicy::Custom(PathBuf::from("custom/saves/state.sav"));
    assert_eq!(
        super::super::save_directory_dialog_default_location(&harness.session),
        harness.root.join("custom/saves")
    );
    assert_eq!(
        super::super::external_save_export_dialog_default_location(&harness.session),
        harness
            .root
            .join("custom/saves/state.sav/export/helpers.sav")
    );

    let (replacement_sender, _) = std::sync::mpsc::channel();
    let (disconnected_sender, disconnected_receiver) = std::sync::mpsc::channel();
    drop(disconnected_sender);
    dialog.sender = replacement_sender;
    dialog.receiver = disconnected_receiver;
    dialog.pending = true;
    assert_eq!(dialog.take_result(), None);
    assert!(!dialog.pending);
}
