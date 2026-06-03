use super::*;

#[test]
fn root_cancel_clears_manual_pause_after_screenshot_for_dialog_loaded_rom() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cancel-after-screenshot", false, false, false);
    let rom_name = "picked.gb";
    let rom_path = harness.root.join(rom_name);
    fs::write(&rom_path, build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
        .expect("dialog test ROM should be writable");

    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(rom_name)))
        .expect("open ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("selected ROM should load");
    assert_eq!(harness.session.rom_path(), Some(rom_path.as_path()));

    harness.runtime.paused = true;
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
            .execute_action(super::super::MenuAction::SaveScreenshot)
            .expect("screenshot should save while paused")
            .is_none()
    );

    let cancel_action = harness
        .runtime
        .menu_state
        .handle_input(
            super::super::MenuInput::Cancel,
            super::super::current_menu_presentation(
                harness.canvas.window(),
                &harness.runtime,
                &harness.machine,
                &harness.session,
            ),
        )
        .expect("root cancel should resume after taking a screenshot");
    assert_eq!(cancel_action, super::super::MenuAction::Resume);

    assert!(
        harness
            .execute_action(cancel_action)
            .expect("resume action should succeed")
            .is_none()
    );
    assert!(harness.session.has_loaded_rom());
    assert!(!harness.runtime.paused);
    assert!(!harness.runtime.menu_state.is_open());
    assert!(!super::super::emulation_paused(
        harness.machine.primary_machine(),
        &harness.runtime,
    ));
}

#[test]
fn current_menu_presentation_only_exposes_camera_actions_for_camera_cartridges() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("camera-menu-presentation", false, false, false);

    let no_rom_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(!no_rom_presentation.cartridge_pocket_camera_supported);
    assert!(!no_rom_presentation.pocket_camera_live_enabled);

    let camera_rom_name = "camera.gb";
    let camera_rom_path = write_test_camera_rom(&harness.root, camera_rom_name);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(camera_rom_name)))
        .expect("Pocket Camera ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("Pocket Camera ROM should load");
    assert_eq!(harness.session.rom_path(), Some(camera_rom_path.as_path()));

    let camera_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(camera_presentation.cartridge_pocket_camera_supported);
    assert!(!camera_presentation.pocket_camera_live_enabled);
}

#[test]
fn camera_image_dialog_updates_the_session_and_reset_reapplies_it_until_cam_reset() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("camera-image-dialog", false, false, false);
    let camera_rom_name = "camera.gb";
    write_test_camera_rom(&harness.root, camera_rom_name);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(camera_rom_name)))
        .expect("Pocket Camera ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("Pocket Camera ROM should load");
    assert!(harness.machine.primary_machine().has_pocket_camera());

    let png_path = write_grayscale_png(&harness.root, "camera.png", 1, 1, &[0x00]);
    harness
        .runtime
        .camera_image_dialog
        .sender
        .send(PathDialogResult::Selected(png_path))
        .expect("Pocket Camera image selection should send");
    harness
        .process_pending_camera_image_dialog()
        .expect("Pocket Camera image dialog should complete");
    assert_eq!(
        harness.session.pocket_camera_frame,
        Some(PocketCameraFrame {
            width: 1,
            height: 1,
            grayscale_pixels: vec![0x00],
        })
    );

    let expected_black_tiles = [
        0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00,
        0xFF,
    ];
    let initial_tiles = capture_camera_tile_bytes(harness.machine.primary_machine_mut());
    assert_eq!(initial_tiles, expected_black_tiles);

    assert!(
        harness
            .execute_action(super::super::MenuAction::Reset)
            .expect("reset should succeed")
            .is_none()
    );
    let after_reset_tiles = capture_camera_tile_bytes(harness.machine.primary_machine_mut());
    assert_eq!(after_reset_tiles, expected_black_tiles);

    assert!(
        harness
            .execute_action(super::super::MenuAction::ResetCameraImage)
            .expect("CAM RESET should succeed")
            .is_none()
    );
    assert_eq!(harness.session.pocket_camera_frame, None);
    let placeholder_tiles = capture_camera_tile_bytes(harness.machine.primary_machine_mut());
    assert_ne!(placeholder_tiles, expected_black_tiles);
}

#[test]
fn camera_image_loader_converts_supported_png_color_types() {
    let _guard = crate::lock_sdl_test();
    let harness = FrontendHarness::new("camera-image-loader-colors", false, false, false);

    write_png(
        &harness.root,
        "gray-alpha.png",
        1,
        1,
        png::ColorType::GrayscaleAlpha,
        &[0x44, 0x99],
    );
    write_png(
        &harness.root,
        "rgb.png",
        1,
        1,
        png::ColorType::Rgb,
        &[255, 0, 0],
    );
    write_png(
        &harness.root,
        "rgba.png",
        1,
        1,
        png::ColorType::Rgba,
        &[0, 255, 0, 0x80],
    );

    assert_eq!(
        super::super::load_selected_camera_image(PathBuf::from("gray-alpha.png"), &harness.session)
            .expect("grayscale alpha PNG should load"),
        PocketCameraFrame {
            width: 1,
            height: 1,
            grayscale_pixels: vec![0x44],
        }
    );
    assert_eq!(
        super::super::load_selected_camera_image(PathBuf::from("rgb.png"), &harness.session)
            .expect("RGB PNG should load"),
        PocketCameraFrame {
            width: 1,
            height: 1,
            grayscale_pixels: vec![76],
        }
    );
    assert_eq!(
        super::super::load_selected_camera_image(PathBuf::from("rgba.png"), &harness.session)
            .expect("RGBA PNG should load"),
        PocketCameraFrame {
            width: 1,
            height: 1,
            grayscale_pixels: vec![150],
        }
    );

    let error =
        super::super::load_selected_camera_image(PathBuf::from("missing.png"), &harness.session)
            .expect_err("missing image should report a file error");
    assert!(error.contains("failed to read Pocket Camera image"));
    assert!(error.contains("missing.png"));
}

#[test]
fn camera_image_dialog_reports_failure_and_cancel_edges() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("camera-image-dialog-edges", false, false, false);
    fs::write(harness.root.join("not-a-png.png"), b"not a png")
        .expect("invalid PNG fixture should be writable");

    harness
        .runtime
        .camera_image_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from("not-a-png.png")))
        .expect("invalid Pocket Camera image selection should send");
    harness
        .process_pending_camera_image_dialog()
        .expect("invalid Pocket Camera image selection should be reported");

    harness
        .runtime
        .camera_image_dialog
        .sender
        .send(PathDialogResult::Canceled)
        .expect("Pocket Camera image cancel should send");
    harness
        .process_pending_camera_image_dialog()
        .expect("canceled Pocket Camera image dialog should be ignored");

    harness
        .runtime
        .camera_image_dialog
        .sender
        .send(PathDialogResult::Failed("camera image failed".to_string()))
        .expect("Pocket Camera image failure should send");
    harness
        .process_pending_camera_image_dialog()
        .expect("failed Pocket Camera image dialog should be reported");
}

#[test]
fn camera_image_loader_reports_png_decode_errors() {
    let _guard = crate::lock_sdl_test();
    let harness = FrontendHarness::new("camera-image-loader-errors", false, false, false);
    fs::write(harness.root.join("not-a-png.png"), b"not a png")
        .expect("invalid PNG fixture should be writable");
    fs::write(
        harness.root.join("truncated.png"),
        [
            0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n', 0, 0, 0, 0x0D, b'I', b'H', b'D',
            b'R',
        ],
    )
    .expect("truncated PNG fixture should be writable");

    let metadata_error =
        super::super::load_selected_camera_image(PathBuf::from("not-a-png.png"), &harness.session)
            .expect_err("non-PNG image should fail metadata decoding");
    assert!(metadata_error.contains("failed to decode PNG metadata"));

    let image_error =
        super::super::load_selected_camera_image(PathBuf::from("truncated.png"), &harness.session)
            .expect_err("truncated PNG image should fail image decoding");
    assert!(
        image_error.contains("failed to decode PNG metadata")
            || image_error.contains("failed to decode PNG image")
    );
}

#[test]
fn desktop_rom_and_external_port_helpers_cover_error_edges() {
    let _guard = crate::lock_sdl_test();
    let harness = FrontendHarness::new("desktop-rom-helper-edges", false, false, false);

    let missing_rom_error =
        match super::super::load_selected_rom(PathBuf::from("missing.gb"), &harness.session) {
            Ok(_) => panic!("missing ROM should report a file read error"),
            Err(error) => error,
        };
    assert!(missing_rom_error.contains("failed to read ROM"));
    assert_eq!(
        super::super::next_single_external_port_selection(DesktopExternalPortSelection::GameLink),
        DesktopExternalPortSelection::None
    );
    assert_eq!(
        super::super::next_single_external_port_selection(
            DesktopExternalPortSelection::FourPlayerAdapter,
        ),
        DesktopExternalPortSelection::None
    );
    assert_eq!(
        super::super::next_single_external_port_selection(DesktopExternalPortSelection::Printer),
        DesktopExternalPortSelection::Printer
    );
}

#[test]
fn pocket_camera_live_menu_actions_cover_unavailable_and_no_rom_edges() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("camera-live-menu-error", false, false, false);

    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleCameraLive)
            .expect("unavailable live backend should be reported without failing the menu")
            .is_none()
    );
    assert!(!harness.runtime.pocket_camera_live.is_enabled());
    assert!(
        harness
            .execute_action(super::super::MenuAction::ResetCameraImage)
            .expect("CAM RESET should be a no-op without a Camera ROM")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetExternalPort(
                DesktopExternalPortSelection::GameLink,
            ))
            .expect("GAME LINK should be ignored without a primary ROM")
            .is_none()
    );
    assert_eq!(
        harness.runtime.open_rom_dialog_mode,
        super::super::OpenRomDialogMode::Primary
    );
}

#[test]
fn camera_live_start_failure_preserves_the_static_session_frame() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("camera-live-start-failure", false, false, false);
    let camera_rom_name = "camera.gb";
    write_test_camera_rom(&harness.root, camera_rom_name);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(camera_rom_name)))
        .expect("Pocket Camera ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("Pocket Camera ROM should load");
    assert!(super::super::session_has_pocket_camera(&harness.machine));

    let static_frame = PocketCameraFrame {
        width: 1,
        height: 1,
        grayscale_pixels: vec![0x44],
    };
    harness.session.pocket_camera_frame = Some(static_frame.clone());

    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleCameraLive)
            .expect("unavailable live backend should be reported without failing the menu")
            .is_none()
    );

    assert!(!harness.runtime.pocket_camera_live.is_enabled());
    assert_eq!(harness.session.pocket_camera_frame, Some(static_frame));
}

#[test]
fn desktop_format_grayscale_and_key_helpers_cover_camera_paths() {
    assert_eq!(
        super::super::format_display_error("context", "error"),
        "context: error"
    );
    assert_eq!(
        super::super::format_debug_error("context", "error"),
        "context: error"
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
        super::super::EmulationProfileSessionKind::Single.label(),
        "single"
    );
    assert_eq!(
        super::super::EmulationProfileSessionKind::LinkedDmg04TwoPlayer.label(),
        "linked-dmg04-2p"
    );
    assert!(super::super::key_matches(
        DesktopKey::Escape,
        Keycode::Escape
    ));
    assert!(!super::super::key_matches(DesktopKey::Escape, Keycode::A));
    assert_eq!(super::super::grayscale_from_rgb(255, 0, 0), 76);
    assert_eq!(super::super::grayscale_from_rgb(0, 255, 0), 150);
    assert_eq!(super::super::grayscale_from_rgb(0, 0, 255), 29);
    assert_eq!(keycode_to_test_scancode(Keycode::A), Scancode::A);
    assert_eq!(keycode_to_test_scancode(Keycode::C), Scancode::C);
    assert_eq!(keycode_to_test_scancode(Keycode::D), Scancode::D);
    assert_eq!(keycode_to_test_scancode(Keycode::E), Scancode::E);
    assert_eq!(keycode_to_test_scancode(Keycode::Q), Scancode::Q);
    assert_eq!(keycode_to_test_scancode(Keycode::S), Scancode::S);
    assert_eq!(keycode_to_test_scancode(Keycode::V), Scancode::V);
    assert_eq!(keycode_to_test_scancode(Keycode::W), Scancode::W);
}

#[test]
fn pocket_camera_frame_helpers_apply_only_to_camera_machines() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("camera-frame-helpers", false, false, false);
    let live_frame = PocketCameraFrame {
        width: 1,
        height: 1,
        grayscale_pixels: vec![0x00],
    };

    assert!(!super::super::session_has_pocket_camera(&harness.machine));
    assert!(
        super::super::apply_pocket_camera_live_frame_to_desktop_session(
            &live_frame,
            &mut harness.machine,
        )
        .is_ok()
    );

    let camera_rom_name = "camera.gb";
    write_test_camera_rom(&harness.root, camera_rom_name);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(camera_rom_name)))
        .expect("Pocket Camera ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("Pocket Camera ROM should load");
    assert!(super::super::session_has_pocket_camera(&harness.machine));

    harness.session.pocket_camera_frame = Some(live_frame.clone());
    super::super::apply_session_pocket_camera_frame_to_desktop_session(
        &harness.session,
        &mut harness.machine,
    )
    .expect("session image should apply to the camera machine");
    let expected_black_tiles = [
        0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00,
        0xFF,
    ];
    assert_eq!(
        capture_camera_tile_bytes(harness.machine.primary_machine_mut()),
        expected_black_tiles
    );

    let white_live_frame = PocketCameraFrame {
        width: 1,
        height: 1,
        grayscale_pixels: vec![0xFF],
    };
    super::super::apply_pocket_camera_live_frame_to_desktop_session(
        &white_live_frame,
        &mut harness.machine,
    )
    .expect("live image should apply to the camera machine");
    assert_ne!(
        capture_camera_tile_bytes(harness.machine.primary_machine_mut()),
        expected_black_tiles
    );
}

#[test]
fn disabled_pocket_camera_live_processing_is_a_noop() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("camera-live-disabled", false, false, false);

    assert!(!harness.runtime.pocket_camera_live.is_enabled());
    harness.process_pocket_camera_live_frame();
    assert!(!harness.runtime.pocket_camera_live.is_enabled());
}

#[test]
fn pocket_camera_live_processing_stops_missing_camera_sessions() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("camera-live-missing-camera", false, false, false);

    harness.runtime.pocket_camera_live =
        super::super::PocketCameraLiveInput::enabled_without_camera_for_tests();
    harness.process_pocket_camera_live_frame();
    assert!(!harness.runtime.pocket_camera_live.is_enabled());

    let camera_rom_name = "camera.gb";
    write_test_camera_rom(&harness.root, camera_rom_name);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(camera_rom_name)))
        .expect("Pocket Camera ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("Pocket Camera ROM should load");
    assert!(super::super::session_has_pocket_camera(&harness.machine));

    harness.runtime.pocket_camera_live =
        super::super::PocketCameraLiveInput::enabled_without_camera_for_tests();
    harness.process_pocket_camera_live_frame();
    assert!(!harness.runtime.pocket_camera_live.is_enabled());
}

#[test]
fn pocket_camera_live_processing_stops_poll_error_sessions() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("camera-live-poll-error", false, false, false);
    let camera_rom_name = "camera.gb";
    write_test_camera_rom(&harness.root, camera_rom_name);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(camera_rom_name)))
        .expect("Pocket Camera ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("Pocket Camera ROM should load");
    assert!(super::super::session_has_pocket_camera(&harness.machine));

    harness.runtime.pocket_camera_live =
        super::super::PocketCameraLiveInput::enabled_with_poll_error_for_tests("bad camera frame");
    harness.process_pocket_camera_live_frame();

    assert!(!harness.runtime.pocket_camera_live.is_enabled());
}

#[test]
fn escape_resumes_after_screenshot_when_the_session_was_manually_paused() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("escape-after-screenshot", false, false, false);
    let rom_name = "picked.gb";
    let rom_path = harness.root.join(rom_name);
    fs::write(&rom_path, build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
        .expect("dialog test ROM should be writable");

    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(rom_name)))
        .expect("open ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("selected ROM should load");
    assert_eq!(harness.session.rom_path(), Some(rom_path.as_path()));

    harness.runtime.paused = true;
    harness.push_key(Keycode::Escape, true);
    harness.process_events().expect("menu open should process");
    assert!(harness.runtime.menu_state.is_open());

    assert!(
        harness
            .execute_action(super::super::MenuAction::SaveScreenshot)
            .expect("screenshot should save while paused")
            .is_none()
    );

    harness.push_key(Keycode::Escape, true);
    harness.process_events().expect("menu close should process");

    assert!(harness.session.has_loaded_rom());
    assert!(!harness.runtime.paused);
    assert!(!harness.runtime.menu_state.is_open());
    assert!(!super::super::emulation_paused(
        harness.machine.primary_machine(),
        &harness.runtime,
    ));
}

#[test]
fn opening_a_new_primary_rom_clears_manual_pause_state() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("open-rom-clears-pause", false, false, false);
    let first_rom_name = "first.gb";
    let first_rom_path = harness.root.join(first_rom_name);
    fs::write(&first_rom_path, build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
        .expect("first ROM should be writable");

    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(first_rom_name)))
        .expect("first open ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("first ROM should load");
    assert_eq!(harness.session.rom_path(), Some(first_rom_path.as_path()));

    harness.runtime.paused = true;
    harness
        .runtime
        .menu_state
        .open(super::super::current_menu_presentation(
            harness.canvas.window(),
            &harness.runtime,
            &harness.machine,
            &harness.session,
        ));

    let second_rom_name = "second.gb";
    let second_rom_path = harness.root.join(second_rom_name);
    fs::write(
        &second_rom_path,
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    )
    .expect("second ROM should be writable");
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(second_rom_name)))
        .expect("second open ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("second ROM should load");

    assert_eq!(harness.session.rom_path(), Some(second_rom_path.as_path()));
    assert!(harness.session.has_loaded_rom());
    assert!(!harness.runtime.paused);
    assert!(!harness.runtime.menu_state.is_open());
    assert!(!super::super::emulation_paused(
        harness.machine.primary_machine(),
        &harness.runtime,
    ));
}

#[test]
fn opening_a_new_primary_rom_clears_dmg07_linked_state() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("open-rom-clears-dmg07", true, false, false);
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetFourPlayerAdapter(
                super::super::DesktopDmg07PlayerCount::Three,
            ))
            .expect("4 PLAYER ADAPTER action should activate")
            .is_none()
    );
    assert!(harness.machine.is_linked_dmg07());

    let next_rom_name = "next.gb";
    let next_rom_path = harness.root.join(next_rom_name);
    fs::write(&next_rom_path, build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
        .expect("next ROM should be writable");
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(next_rom_name)))
        .expect("next open ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("next ROM should load");

    assert_eq!(harness.session.rom_path(), Some(next_rom_path.as_path()));
    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert_eq!(harness.session.dmg07_player_count, None);
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
}

#[test]
fn opening_a_new_primary_rom_clears_cgb_ir_linked_state() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("open-rom-clears-cgb-ir", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    write_cgb_test_rom(&harness.root, "silver.gbc", 0x00, 0x00);
    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::CgbInfraredSecondary;
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from("silver.gbc")))
        .expect("CGB IR secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("secondary ROM selection should activate CGB IR");
    assert!(harness.machine.is_linked_cgb_infrared_two_player());

    let next_rom_path = write_cgb_test_rom(&harness.root, "crystal.gbc", 0x00, 0x00);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from("crystal.gbc")))
        .expect("next open ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("next ROM should load");

    assert_eq!(harness.session.rom_path(), Some(next_rom_path.as_path()));
    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
}

#[test]
fn opening_a_recent_rom_clears_manual_pause_state() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("recent-rom-clears-pause", true, false, false);
    let recent_rom_path = harness.root.join("recent.gb");
    fs::write(
        &recent_rom_path,
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    )
    .expect("recent ROM should be writable");
    harness.session.recent_roms = vec![recent_rom_path.clone()];

    harness.runtime.paused = true;
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
            .execute_action(super::super::MenuAction::OpenRecentRom(0))
            .expect("recent ROM should open")
            .is_none()
    );

    assert_eq!(harness.session.rom_path(), Some(recent_rom_path.as_path()));
    assert!(harness.session.has_loaded_rom());
    assert!(!harness.runtime.paused);
    assert!(!harness.runtime.menu_state.is_open());
    assert!(!super::super::emulation_paused(
        harness.machine.primary_machine(),
        &harness.runtime,
    ));
}

#[test]
fn frontend_harness_covers_event_loop_frame_and_render_helpers() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("runtime", true, true, true);
    let relative_rom = PathBuf::from("runtime.gb");
    let relative_rom_path = harness.root.join(&relative_rom);
    fs::write(
        &relative_rom_path,
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    )
    .expect("runtime ROM should be writable");

    let loaded = super::super::load_initial_rom(
        &DesktopRunOptions {
            rom_path: Some(relative_rom.clone()),
            linked_peer_rom_path: None,
            benchmark_path: None,
            exit_after_frames: None,
            config: DesktopConfig::default(),
            audio_recording: None,
            test_runner: false,
        },
        &harness.root,
    )
    .expect("relative ROM path should load")
    .expect("relative ROM should exist");
    assert_eq!(loaded.path, relative_rom_path);
    assert!(
        super::super::load_initial_rom(
            &DesktopRunOptions {
                rom_path: None,
                linked_peer_rom_path: None,
                benchmark_path: None,
                exit_after_frames: None,
                config: DesktopConfig::default(),
                audio_recording: None,
                test_runner: false,
            },
            &harness.root,
        )
        .expect("missing ROM path should be allowed")
        .is_none()
    );
    let linked_loaded = super::super::load_initial_linked_secondary_rom(
        &DesktopRunOptions {
            rom_path: Some(relative_rom.clone()),
            linked_peer_rom_path: Some(relative_rom.clone()),
            benchmark_path: None,
            exit_after_frames: Some(8),
            config: DesktopConfig::default(),
            audio_recording: None,
            test_runner: false,
        },
        &harness.root,
    )
    .expect("relative linked peer path should load")
    .expect("relative linked peer should exist");
    assert_eq!(linked_loaded.path, relative_rom_path);
    assert!(super::super::should_exit_after_presented_frames(Some(4), 4));
    assert!(!super::super::should_exit_after_presented_frames(
        Some(5),
        4
    ));
    assert!(!super::super::should_exit_after_presented_frames(None, 4));

    let mut reloaded_machine = super::super::load_machine_for_rom(
        &harness.session.config,
        &harness.session.current_dir,
        harness.session.rom_bytes().expect("loaded ROM bytes"),
    )
    .expect("machine should reload from ROM bytes")
    .machine;
    assert!(
        super::super::open_save_session_for_session(&harness.session, &mut reloaded_machine)
            .expect("save session should open for the loaded ROM")
            .is_none()
    );

    super::super::run_from_cli(["--help"]).expect("help path should succeed");
    let expected_toggled_device = harness
        .runtime
        .gamepad_manager
        .as_ref()
        .and_then(super::super::GamepadManager::active_gamepad_identity)
        .unwrap_or_default();
    assert_eq!(
        super::super::toggled_preferred_gamepad_device(
            harness
                .runtime
                .gamepad_manager
                .as_ref()
                .expect("runtime test should have a gamepad manager")
        ),
        expected_toggled_device
    );
    if !expected_toggled_device.is_configured() {
        assert_eq!(
            harness
                .runtime
                .gamepad_manager
                .as_ref()
                .expect("runtime test should have a gamepad manager")
                .preferred_device(),
            &gb_desktop::PreferredGamepadIdentity {
                path: None,
                name: Some("Saved Pad".to_string()),
            }
        );
    } else {
        harness
            .runtime
            .gamepad_manager
            .as_mut()
            .expect("runtime test should have a gamepad manager")
            .set_preferred_device(
                expected_toggled_device.clone(),
                harness
                    .runtime
                    .player_inputs
                    .input_mut(super::super::PlayerSlot::P1),
                harness
                    .machine
                    .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
                    .expect("P1 should always map to an active desktop machine"),
            );
        assert_eq!(
            super::super::toggled_preferred_gamepad_device(
                harness
                    .runtime
                    .gamepad_manager
                    .as_ref()
                    .expect("runtime test should have a gamepad manager")
            ),
            gb_desktop::PreferredGamepadIdentity::default()
        );
    }

    harness.push_key(Keycode::LAlt, true);
    assert!(matches!(
        harness
            .process_events()
            .expect("keyboard press should process"),
        super::super::LoopSignal::Continue
    ));
    harness.machine.step_t_cycle();
    assert_ne!(harness.machine.joypad().snapshot().pressed_mask, 0);
    harness.push_key(Keycode::LAlt, false);
    harness
        .process_events()
        .expect("keyboard release should process");
    harness.machine.step_t_cycle();
    assert_eq!(harness.machine.joypad().snapshot().pressed_mask, 0);

    harness.push_key(Keycode::Escape, true);
    harness.process_events().expect("menu open should process");
    assert!(harness.runtime.menu_state.is_open());
    harness.push_key(Keycode::Escape, true);
    harness.process_events().expect("menu close should process");
    assert!(!harness.runtime.menu_state.is_open());

    assert!(matches!(
        harness
            .step_until_next_frame()
            .expect("frame stepping should complete"),
        super::super::LoopSignal::Continue
    ));

    super::super::apply_window_scale(harness.canvas.window_mut(), 3)
        .expect("window scale should apply");
    super::super::set_fullscreen_state(harness.canvas.window_mut(), false)
        .expect("setting the existing fullscreen state should be a no-op");
    super::super::reset_machine(
        harness.canvas.window(),
        &mut harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.settings_store,
    )
    .expect("machine reset should succeed");

    let texture_creator = harness.canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            sdl3::pixels::PixelFormat::RGB24,
            super::super::FRAMEBUFFER_WIDTH,
            super::super::FRAMEBUFFER_HEIGHT,
        )
        .expect("runtime texture should be creatable");
    let mut rgb_frame = vec![
        0_u8;
        super::super::FRAMEBUFFER_HEIGHT as usize
            * super::super::FRAMEBUFFER_PITCH_BYTES
    ];
    let menu_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    harness.runtime.menu_state.open(menu_presentation);
    let open_menu_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    let _ = super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut rgb_frame,
        super::super::FramebufferRenderInput {
            dimensions: super::super::FramebufferDimensions {
                width: super::super::FRAMEBUFFER_WIDTH,
                height: super::super::FRAMEBUFFER_HEIGHT,
            },
            panels: [
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: harness.machine.ppu().framebuffer(),
                    framebuffer_layer_sources: harness.machine.ppu().framebuffer_layer_sources(),
                    bgwin_framebuffer: harness.machine.ppu().framebuffer_bgwin_panel_shades(),
                    backdrop_framebuffer: harness.machine.ppu().framebuffer_backdrop_panel_shades(),
                    bgwin_framebuffer_layer_sources: harness
                        .machine
                        .ppu()
                        .framebuffer_bgwin_layer_sources(),
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                }),
                None,
                None,
                None,
            ],
        },
        &harness.runtime.video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: None,
            menu_state: Some((&harness.runtime.menu_state, open_menu_presentation)),
            hud: super::super::RenderHudInput::default(),
        },
    )
    .expect("overlay frame should render");
    assert!(rgb_frame.iter().any(|byte| *byte != 0));

    harness.runtime.menu_state.close();
    harness.runtime.video_options.show_performance_hud = true;
    let _ = super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut rgb_frame,
        super::super::FramebufferRenderInput {
            dimensions: super::super::FramebufferDimensions {
                width: super::super::FRAMEBUFFER_WIDTH,
                height: super::super::FRAMEBUFFER_HEIGHT,
            },
            panels: [
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: harness.machine.ppu().framebuffer(),
                    framebuffer_layer_sources: harness.machine.ppu().framebuffer_layer_sources(),
                    bgwin_framebuffer: harness.machine.ppu().framebuffer_bgwin_panel_shades(),
                    backdrop_framebuffer: harness.machine.ppu().framebuffer_backdrop_panel_shades(),
                    bgwin_framebuffer_layer_sources: harness
                        .machine
                        .ppu()
                        .framebuffer_bgwin_layer_sources(),
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                }),
                None,
                None,
                None,
            ],
        },
        &harness.runtime.video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: None,
            menu_state: None,
            hud: super::super::RenderHudInput {
                performance: Some(PerformanceHudSnapshot {
                    fps: 59.7,
                    speed_percent: 100.0,
                    frame_time_ms: 16.7,
                    emulation_time_ms: 10.0,
                    render_time_ms: 2.0,
                    pacing_time_ms: 4.0,
                    audio_queue_ms: Some(12.5),
                    rewind: RewindHudSnapshot::default(),
                }),
                cgb_ir: None,
                rewind_indicator: false,
                fast_forward_indicator: false,
            },
        },
    )
    .expect("HUD frame should render");
    assert!(rgb_frame.iter().any(|byte| *byte != 0));
}
