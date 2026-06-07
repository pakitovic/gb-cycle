use super::*;

#[test]
fn drain_printed_pages_into_printer_output_saves_png_and_updates_the_window() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("printer-sink", true, false, false);
    harness.session.external_port_selection = DesktopExternalPortSelection::Printer;
    super::super::apply_external_port_selection_to_machine(
        &mut harness.machine,
        harness.session.external_port_selection,
    );

    run_print_sequence(&mut harness.machine);
    super::super::drain_printed_pages_into_printer_output(
        harness.canvas.window(),
        &harness.session,
        &mut harness.runtime,
        &mut harness.machine,
    );

    assert_eq!(harness.machine.take_printed_pages().len(), 0);
    assert!(harness.runtime.printer_output.has_window());
    assert_eq!(
        harness.runtime.printer_output.latest_page_dimensions(),
        Some((160, 8))
    );
    let saved_path = harness
        .runtime
        .printer_output
        .last_saved_path()
        .expect("printer output should remember the saved PNG path");
    assert!(saved_path.exists());
    assert!(saved_path.starts_with(harness.root.join("printer")));
}

#[test]
fn reset_machine_persists_skip_boot_when_the_boot_rom_path_goes_missing() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("reset-missing-bootrom", true, false, false);
    harness.session.config.launch.startup_mode = StartupMode::RealBoot;
    harness.session.config.boot_rom.verification = BootRomVerificationMode::Strict;
    harness.session.config.boot_rom.search_path = Some(harness.root.join("missing.bin"));
    harness
        .settings_store
        .persist_machine_preferences(&harness.session.config)
        .expect("stale real-boot settings should persist before reset");

    super::super::reset_machine(
        harness.canvas.window(),
        &mut harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.settings_store,
    )
    .expect("reset should degrade missing boot ROM settings instead of failing");

    assert_eq!(
        harness.session.config.launch.startup_mode,
        StartupMode::SkipBoot
    );
    let persisted = fs::read_to_string(&harness.settings_path)
        .expect("reset fallback should update persisted settings");
    assert!(persisted.contains("startup_mode = \"skip-boot\""));
}

#[test]
fn cycling_console_model_resets_the_runtime_display_palette_to_the_model_default() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("model-palette-defaults", true, false, false);

    assert_eq!(
        harness.runtime.video_options.display_palette,
        DesktopDisplayPalette::GameBoy
    );
    for (expected_model, expected_palette) in [
        (
            DesktopConsoleModel::GameBoyPocket,
            DesktopDisplayPalette::Pocket,
        ),
        (
            DesktopConsoleModel::GameBoyLight,
            DesktopDisplayPalette::Light,
        ),
        (
            DesktopConsoleModel::GameBoyColor,
            DesktopDisplayPalette::Grey,
        ),
        (
            DesktopConsoleModel::GameBoyAdvance,
            DesktopDisplayPalette::Grey,
        ),
        (
            DesktopConsoleModel::SuperGameBoy,
            DesktopDisplayPalette::Grey,
        ),
        (
            DesktopConsoleModel::SuperGameBoy2,
            DesktopDisplayPalette::Grey,
        ),
        (DesktopConsoleModel::GameBoy, DesktopDisplayPalette::GameBoy),
    ] {
        harness.runtime.video_options.display_palette = DesktopDisplayPalette::Grey;
        assert!(
            harness
                .execute_action(super::super::MenuAction::CycleConsoleModel)
                .expect("model cycling should rebuild successfully")
                .is_none()
        );
        assert_eq!(harness.session.config.launch.console_model, expected_model);
        assert_eq!(
            harness.runtime.video_options.display_palette,
            expected_palette
        );
        assert_eq!(
            harness.settings_store.base_config().video.display_palette,
            expected_palette
        );
    }
}

#[test]
fn execution_mode_cycle_skips_modes_that_reject_the_loaded_rom() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("execution-mode-skip-unloadable", false, false, false);
    let legacy_mbc1_ram_header = build_test_rom(32 * 1024, 0x02, 0x00, 0x00);
    let rom_path = harness.root.join("halt_bug_like.gb");
    fs::write(&rom_path, &legacy_mbc1_ram_header).expect("legacy test ROM should be writable");

    harness.session.config.launch.execution_mode = ExecutionMode::Experimental;
    harness.session.loaded_rom = Some(super::super::LoadedRom {
        path: rom_path,
        bytes: legacy_mbc1_ram_header.clone(),
    });
    harness.machine = super::super::DesktopEmulationSession::new_single(
        super::super::load_machine_for_rom(
            &harness.session.config,
            &harness.session.current_dir,
            &legacy_mbc1_ram_header,
        )
        .expect("experimental mode should load the legacy MBC1+RAM header")
        .machine,
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleExecutionMode)
            .expect("execution mode action should complete")
            .is_none()
    );

    assert_eq!(
        harness.session.config.launch.execution_mode,
        ExecutionMode::Permissive
    );
}

#[test]
fn rebuild_preflight_covers_launcher_linked_and_adapter_sessions() {
    let root = temp_test_root("rebuild-preflight-session-kinds");
    let config = DesktopConfig::default();
    let primary_rom = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
    let secondary_rom = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
    let primary_loaded = super::super::LoadedRom {
        path: root.join("primary.gb"),
        bytes: primary_rom,
    };
    let secondary_loaded = super::super::LoadedRom {
        path: root.join("secondary.gb"),
        bytes: secondary_rom,
    };

    let launcher_session = super::super::DesktopSession {
        config: config.clone(),
        test_runner: false,
        benchmark: None,
        current_dir: root.clone(),
        loaded_rom: None,
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
    super::super::check_current_session_rebuilds_with_config(&launcher_session, &config)
        .expect("launcher preflight should prepare an empty machine");

    let linked_session = super::super::DesktopSession {
        loaded_rom: Some(primary_loaded.clone()),
        linked_secondary_rom: Some(secondary_loaded.clone()),
        external_port_selection: DesktopExternalPortSelection::GameLink,
        ..launcher_session.clone()
    };
    super::super::check_current_session_rebuilds_with_config(&linked_session, &config)
        .expect("DMG-04 preflight should load both cartridges");

    let mut cgb_config = config.clone();
    cgb_config.launch.console_model = DesktopConsoleModel::GameBoyColor;
    let cgb_primary_loaded = super::super::LoadedRom {
        path: root.join("gold.gbc"),
        bytes: build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    };
    let cgb_secondary_loaded = super::super::LoadedRom {
        path: root.join("silver.gbc"),
        bytes: build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    };
    let cgb_ir_session = super::super::DesktopSession {
        config: cgb_config.clone(),
        loaded_rom: Some(cgb_primary_loaded),
        linked_secondary_rom: Some(cgb_secondary_loaded),
        cgb_infrared_link_active: true,
        external_port_selection: DesktopExternalPortSelection::None,
        ..launcher_session.clone()
    };
    super::super::check_current_session_rebuilds_with_config(&cgb_ir_session, &cgb_config)
        .expect("CGB IR preflight should load both native CGB cartridges");

    let adapter_session = super::super::DesktopSession {
        loaded_rom: Some(primary_loaded),
        external_port_selection: DesktopExternalPortSelection::FourPlayerAdapter,
        dmg07_player_count: Some(super::super::DesktopDmg07PlayerCount::Two),
        cgb_infrared_link_active: false,
        ..launcher_session
    };
    super::super::check_current_session_rebuilds_with_config(&adapter_session, &config)
        .expect("DMG-07 preflight should load cloned cartridges");
}

#[test]
fn execute_menu_actions_update_runtime_machine_and_persisted_settings() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("actions", true, true, true);

    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleConsoleModel)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.config.launch.console_model,
        DesktopConsoleModel::GameBoyPocket
    );
    assert_eq!(
        harness.session.config.launch.revision,
        HardwareRevision::CpuMgb
    );
    assert_eq!(
        harness.runtime.video_options.display_palette,
        DesktopDisplayPalette::Pocket
    );
    harness.session.config.boot_rom.search_path = Some(harness.root.join("bootroms"));
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleStartupMode)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.config.launch.startup_mode,
        StartupMode::CustomBoot
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleStartupMode)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.config.launch.startup_mode,
        StartupMode::RealBoot
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleExecutionMode)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.config.launch.execution_mode,
        ExecutionMode::Permissive
    );
    harness.session.config.launch.startup_mode = StartupMode::SkipBoot;
    harness.session.config.boot_rom.verification = BootRomVerificationMode::Off;
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleBootRomVerify)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.config.boot_rom.verification,
        BootRomVerificationMode::Strict
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleSavesEnabled)
            .unwrap()
            .is_none()
    );
    assert!(!harness.session.config.saves.enabled);
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleSavePolicy)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.config.saves.flush_policy,
        DesktopSaveFlushPolicy::Manual
    );
    assert_eq!(harness.session.config.machine_state.autoload_slot, None);
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleStateAutoloadSlot)
            .unwrap()
            .is_none()
    );
    assert_eq!(harness.session.config.machine_state.autoload_slot, Some(1));
    assert_eq!(
        harness
            .settings_store
            .base_config()
            .machine_state
            .autoload_slot,
        Some(1)
    );
    assert!(
        harness
            .runtime
            .rewind_buffer
            .record_frame_boundary(harness.machine.primary_machine())
    );
    assert!(!harness.runtime.rewind_buffer.is_empty());
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleRewindEnabled)
            .unwrap()
            .is_none()
    );
    assert!(!harness.session.config.rewind.enabled);
    assert!(harness.runtime.rewind_buffer.is_empty());
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleRewindHistory)
            .unwrap()
            .is_none()
    );
    assert_eq!(harness.session.config.rewind.history_seconds, 20);
    assert_eq!(
        harness
            .runtime
            .rewind_buffer
            .config()
            .target_history_t_cycles,
        20 * super::super::DMG_T_CYCLES_PER_SECOND
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleRewindSubframes)
            .unwrap()
            .is_none()
    );
    assert_eq!(harness.session.config.rewind.subframes_per_frame, 2);
    assert!(
        harness
            .runtime
            .rewind_buffer
            .record_frame_boundary(harness.machine.primary_machine())
    );
    assert!(!harness.runtime.rewind_buffer.is_empty());
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleRewindSpeed)
            .unwrap()
            .is_none()
    );
    assert_eq!(harness.session.config.rewind.speed_multiplier, 4);
    assert!(
        !harness.runtime.rewind_buffer.is_empty(),
        "playback speed changes must not discard existing rewind history"
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleRewindMemory)
            .unwrap()
            .is_none()
    );
    assert_eq!(harness.session.config.rewind.max_memory_mib, 512);
    assert!(
        harness
            .execute_action(super::super::MenuAction::ResetRewindDefaults)
            .unwrap()
            .is_none()
    );
    assert_eq!(harness.session.config.rewind, RewindOptions::default());
    harness.session.config.saves.directory_policy =
        gb_desktop::SaveDirectoryPolicy::Custom(harness.root.join("manual-saves"));
    assert!(
        harness
            .execute_action(super::super::MenuAction::ClearSaveDirectoryPath)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.config.saves.directory_policy,
        gb_desktop::SaveDirectoryPolicy::RomFolderSavesSubdir
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleVsync)
            .unwrap()
            .is_none()
    );
    assert!(!harness.runtime.video_options.vsync);
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleWindowScale)
            .unwrap()
            .is_none()
    );
    assert_eq!(harness.runtime.video_options.window_scale, 5);
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleIntegerScale)
            .unwrap()
            .is_none()
    );
    assert!(!harness.runtime.video_options.integer_scale);
    assert!(
        harness
            .execute_action(super::super::MenuAction::TogglePresentationFilter)
            .unwrap()
            .is_none()
    );
    assert!(harness.runtime.video_options.presentation_filter);
    harness.runtime.frame_blending_state.mode = DesktopFrameBlendingMode::On;
    harness.runtime.frame_blending_state.dimensions = Some(super::super::FramebufferDimensions {
        width: super::super::FRAMEBUFFER_WIDTH,
        height: super::super::FRAMEBUFFER_HEIGHT,
    });
    harness.runtime.frame_blending_state.previous_rgb_frame = vec![1, 2, 3];
    harness.runtime.frame_blending_state.has_previous_frame = true;
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleFrameBlending)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.video_options.frame_blending,
        DesktopFrameBlendingMode::On
    );
    assert_eq!(
        harness.settings_store.base_config().video.frame_blending,
        DesktopFrameBlendingMode::On
    );
    assert_eq!(
        harness.runtime.frame_blending_state.mode,
        DesktopFrameBlendingMode::Off
    );
    assert!(
        harness
            .runtime
            .frame_blending_state
            .previous_rgb_frame
            .is_empty()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleFrameBlending)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.video_options.frame_blending,
        DesktopFrameBlendingMode::Off
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleDisplayPalette)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.video_options.display_palette,
        DesktopDisplayPalette::Light
    );
    assert_eq!(
        harness.settings_store.base_config().video.display_palette,
        DesktopDisplayPalette::Light
    );
    harness.session.config.launch.console_model = DesktopConsoleModel::SuperGameBoy;
    harness.runtime.frame_blending_state.previous_rgb_frame = vec![4, 5, 6];
    harness.runtime.frame_blending_state.has_previous_frame = true;
    assert_eq!(
        harness.runtime.video_options.sgb_border,
        SgbBorderPresentationMode::Auto
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleSgbBorder)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.video_options.sgb_border,
        SgbBorderPresentationMode::Off
    );
    assert_eq!(
        harness.settings_store.base_config().video.sgb_border,
        SgbBorderPresentationMode::Off
    );
    assert!(
        harness
            .runtime
            .frame_blending_state
            .previous_rgb_frame
            .is_empty()
    );
    harness.session.config.launch.console_model = DesktopConsoleModel::GameBoyColor;
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleDisplayPalette)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.video_options.display_palette,
        DesktopDisplayPalette::Light
    );
    assert_eq!(
        harness.settings_store.base_config().video.display_palette,
        DesktopDisplayPalette::Light
    );
    harness.session.config.launch.console_model = DesktopConsoleModel::GameBoyPocket;
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleBackgroundLayer)
            .unwrap()
            .is_none()
    );
    assert!(!harness.runtime.video_options.show_background);
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleWindowLayer)
            .unwrap()
            .is_none()
    );
    assert!(!harness.runtime.video_options.show_window);
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleObjectLayer)
            .unwrap()
            .is_none()
    );
    assert!(!harness.runtime.video_options.show_objects);
    assert!(
        harness
            .execute_action(super::super::MenuAction::SaveScreenshot)
            .unwrap()
            .is_none()
    );
    let screenshot_path = harness.root.join("screenshots").join("actions-0.png");
    let encoded = fs::read(&screenshot_path).expect("screenshot PNG should exist");
    let decoder = png::Decoder::new(std::io::Cursor::new(encoded));
    let mut reader = decoder.read_info().expect("PNG header should decode");
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .expect("PNG output buffer size should fit in memory")
    ];
    let info = reader
        .next_frame(&mut buffer)
        .expect("PNG payload should decode");
    assert_eq!(info.width, super::super::FRAMEBUFFER_WIDTH);
    assert_eq!(info.height, super::super::FRAMEBUFFER_HEIGHT);
    assert_eq!(info.color_type, png::ColorType::Rgb);
    assert_eq!(
        &buffer[..3],
        &super::super::GBL_DISPLAY_PALETTE.shade_rgb(0)
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::TogglePerformanceHud)
            .unwrap()
            .is_none()
    );
    assert!(harness.runtime.video_options.show_performance_hud);
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleMute)
            .unwrap()
            .is_none()
    );
    assert!(
        harness
            .runtime
            .audio_output
            .as_ref()
            .is_some_and(|audio| audio.is_muted())
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleAudioVolume)
            .unwrap()
            .is_none()
    );
    assert_eq!(harness.runtime.audio_volume_percent, 25);
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleAudioChannel(
                ApuRecordedChannel::Ch2
            ))
            .unwrap()
            .is_none()
    );
    assert!(
        !harness
            .runtime
            .audio_channel_mask
            .contains(ApuRecordedChannel::Ch2)
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleAudioRecording)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        harness.runtime.audio_recording_mode,
        super::super::DesktopAudioRecordingMode::Automatic
    ));
    assert!(harness.runtime.audio_recorder.is_some());
    let automatic_recording_path = harness.root.join("audios").join("actions-0.wav");
    assert!(automatic_recording_path.exists());
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleAudioRecording)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        harness.runtime.audio_recording_mode,
        super::super::DesktopAudioRecordingMode::Disabled
    ));
    assert!(harness.runtime.audio_recorder.is_none());
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetExternalPort(
                DesktopExternalPortSelection::Printer,
            ))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::Printer
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::Printer
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetExternalPort(
                DesktopExternalPortSelection::None,
            ))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    harness.session.config.launch.console_model = DesktopConsoleModel::SuperGameBoy;
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetExternalPort(
                DesktopExternalPortSelection::Printer,
            ))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    harness.session.config.launch.console_model = DesktopConsoleModel::GameBoyPocket;
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleGamepadDirectionalSource)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad manager")
            .directional_source(),
        GamepadDirectionalSource::DpadOnly
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleGamepadRumbleMode)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad manager")
            .rumble_mode(),
        GamepadRumbleMode::Weak
    );
    assert_eq!(
        harness
            .settings_store
            .base_config()
            .input
            .gamepad
            .rumble_mode,
        GamepadRumbleMode::Weak
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleGamepadGyroMode)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad manager")
            .gyro_mode(),
        GamepadGyroMode::PadGyro
    );
    assert_eq!(
        harness.settings_store.base_config().input.gamepad.gyro_mode,
        GamepadGyroMode::PadGyro
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::OpenRecentRom(99))
            .unwrap()
            .is_none()
    );
    harness
        .settings_store
        .remember_loaded_rom(&harness.root.join("Tetris DX.gb"))
        .expect("recent ROM should persist for clear-list coverage");
    harness.session.recent_roms = harness.settings_store.recent_roms().to_vec();
    harness
        .runtime
        .menu_state
        .open(super::super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        ));
    assert!(
        harness
            .execute_action(super::super::MenuAction::ClearRecentList)
            .unwrap()
            .is_none()
    );
    assert!(harness.session.recent_roms().is_empty());
    assert!(harness.settings_store.recent_roms().is_empty());
    assert!(harness.runtime.menu_state.is_open());
    assert_eq!(
        super::super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        )
        .recent_rom_count,
        0
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetKeyboardBinding(
                KeyboardBindingTarget::A,
                DesktopKey::Space,
            ))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.keyboard_bindings.joypad.a,
        DesktopKey::Space
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetKeyboardMenuBinding(
                KeyboardMenuBindingTarget::Confirm,
                DesktopKey::X,
            ))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.keyboard_bindings.menu.confirm,
        DesktopKey::X
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetGamepadBinding(
                GamepadBindingTarget::A,
                GamepadButtonBinding::South,
            ))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad manager")
            .button_bindings()
            .a,
        GamepadButtonBinding::South
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetGamepadMenuBinding(
                GamepadMenuBindingTarget::Confirm,
                GamepadButtonBinding::North,
            ))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad manager")
            .menu_bindings()
            .confirm,
        GamepadButtonBinding::North
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::ResetAudioDefaults)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.audio_channel_mask,
        ApuRecordedChannelMask::ALL
    );
    assert!(matches!(
        harness.runtime.audio_recording_mode,
        super::super::DesktopAudioRecordingMode::Disabled
    ));
    assert_eq!(harness.runtime.audio_volume_percent, 100);
    harness.runtime.video_options.frame_blending = DesktopFrameBlendingMode::On;
    harness.runtime.frame_blending_state.mode = DesktopFrameBlendingMode::On;
    harness.runtime.frame_blending_state.dimensions = Some(super::super::FramebufferDimensions {
        width: super::super::FRAMEBUFFER_WIDTH,
        height: super::super::FRAMEBUFFER_HEIGHT,
    });
    harness.runtime.frame_blending_state.previous_rgb_frame = vec![1, 2, 3];
    harness.runtime.frame_blending_state.has_previous_frame = true;
    assert!(
        harness
            .execute_action(super::super::MenuAction::ResetVideoDefaults)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.video_options,
        gb_desktop::VideoOptions::default_for_console_model(
            harness.session.config.launch.console_model
        )
    );
    assert_eq!(
        harness.runtime.video_options.display_palette,
        DesktopDisplayPalette::Pocket
    );
    assert_eq!(
        harness.runtime.video_options.frame_blending,
        DesktopFrameBlendingMode::Off
    );
    assert!(
        harness
            .runtime
            .frame_blending_state
            .previous_rgb_frame
            .is_empty()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::ResetInputDefaults)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        harness.runtime.keyboard_bindings,
        gb_desktop::InputOptions::default().keyboard
    );
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad manager")
            .rumble_mode(),
        GamepadRumbleMode::Strong
    );
    assert_eq!(
        harness
            .runtime
            .gamepad_manager
            .as_ref()
            .expect("gamepad manager")
            .gyro_mode(),
        GamepadGyroMode::Off
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::Reset)
            .unwrap()
            .is_none()
    );
    assert!(!harness.machine.cartridge().is_empty());
    assert!(matches!(
        harness
            .execute_action(super::super::MenuAction::Quit)
            .unwrap(),
        Some(super::super::LoopSignal::Quit)
    ));

    let persisted =
        fs::read_to_string(&harness.settings_path).expect("actions test should persist settings");
    assert!(persisted.contains("model = \"MGB\""));
    assert!(persisted.contains("revision = \"cpu-mgb\""));
    assert!(persisted.contains("startup_mode = \"skip-boot\""));
}
