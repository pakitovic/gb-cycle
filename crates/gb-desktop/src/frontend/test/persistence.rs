use super::*;

#[test]
fn state_slot_paths_use_rom_states_subdir_and_runtime_slot_selector() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("state-slot-paths", true, false, false);

    assert_eq!(next_machine_state_slot(1), 2);
    assert_eq!(next_machine_state_slot(2), 3);
    assert_eq!(next_machine_state_slot(3), 4);
    assert_eq!(next_machine_state_slot(4), 1);
    assert_eq!(next_machine_state_slot(0), 1);

    let default_path =
        machine_state_slot_path(&harness.session, 1).expect("default state path should resolve");
    assert_eq!(
        default_path,
        harness.root.join("states/state-slot-paths.slot1.gbstate")
    );
    assert!(!machine_state_slot_load_available(
        &harness.session,
        &harness.machine,
        1
    ));
    let presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(presentation.machine_state_available);
    assert!(!presentation.machine_state_load_available);

    harness.session.config.saves.enabled = false;
    harness.session.config.saves.key_policy = SaveKeyPolicy::Explicit(
        CartridgeSaveKey::new("manual-state-key").expect("explicit state key should be valid"),
    );
    let explicit_path =
        machine_state_slot_path(&harness.session, 4).expect("explicit state path should resolve");
    assert_eq!(
        explicit_path,
        harness.root.join("states/manual-state-key.slot4.gbstate")
    );

    assert_eq!(harness.runtime.machine_state_slot, 1);
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleStateSlot)
            .expect("slot selector should cycle")
            .is_none()
    );
    assert_eq!(harness.runtime.machine_state_slot, 2);
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleStateSlot)
            .expect("slot selector should cycle")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleStateSlot)
            .expect("slot selector should cycle")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleStateSlot)
            .expect("slot selector should wrap")
            .is_none()
    );
    assert_eq!(harness.runtime.machine_state_slot, 1);
}

#[test]
fn state_slot_menu_action_keeps_slot_selector_selected_while_cycling() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("state-slot-selection", true, false, false);
    save_machine_state_slot(&harness.session, &harness.machine, 2)
        .expect("slot 2 should save before cycling to it");

    let presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(!presentation.machine_state_load_available);
    harness.runtime.menu_state.open(presentation);
    assert_eq!(
        harness
            .runtime
            .menu_state
            .handle_input(super::super::MenuInput::Down, presentation),
        None
    );
    assert_eq!(
        harness
            .runtime
            .menu_state
            .handle_input(super::super::MenuInput::Down, presentation),
        None
    );
    let action = harness
        .runtime
        .menu_state
        .handle_input(super::super::MenuInput::Confirm, presentation)
        .expect("STATE SLOT should emit a cycle action");
    assert_eq!(action, super::super::MenuAction::CycleStateSlot);
    harness
        .execute_action(action)
        .expect("slot cycle action should execute");
    assert_eq!(harness.runtime.machine_state_slot, 2);

    let cycled_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(cycled_presentation.machine_state_load_available);
    let repeated_action = harness
        .runtime
        .menu_state
        .handle_input(super::super::MenuInput::Confirm, cycled_presentation)
        .expect("STATE SLOT should remain selected after cycling");
    assert_eq!(repeated_action, super::super::MenuAction::CycleStateSlot);
    harness
        .execute_action(repeated_action)
        .expect("second slot cycle action should execute");
    assert_eq!(harness.runtime.machine_state_slot, 3);
}

#[test]
fn state_slots_round_trip_machine_state_and_corrupt_loads_do_not_mutate() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("state-slot-roundtrip", true, false, false);

    harness.machine.write_bus(0xC000, 0x42);
    let saved_path = save_machine_state_slot(&harness.session, &harness.machine, 1)
        .expect("state slot should save");
    assert!(machine_state_slot_load_available(
        &harness.session,
        &harness.machine,
        1
    ));
    assert!(!machine_state_slot_load_available(
        &harness.session,
        &harness.machine,
        2
    ));
    let presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(presentation.machine_state_load_available);
    let decoded = decode_machine_save_state_envelope(
        &fs::read(&saved_path).expect("saved .gbstate should exist"),
    )
    .expect("saved .gbstate should decode");
    assert_eq!(decoded.state, harness.machine.capture_save_state());

    harness.machine.write_bus(0xC000, 0x99);
    assert!(
        harness
            .runtime
            .rewind_buffer
            .record_subframe(harness.machine.primary_machine())
    );
    assert_eq!(harness.machine.read_bus(0xC000), 0x99);
    let loaded_path = load_machine_state_slot(
        &harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.frame_pacer,
        1,
    )
    .expect("state slot should load");
    assert_eq!(loaded_path, saved_path);
    assert_eq!(harness.machine.read_bus(0xC000), 0x42);
    assert!(harness.runtime.rewind_buffer.is_empty());

    let before_missing = harness.machine.capture_save_state();
    let missing_error = load_machine_state_slot(
        &harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.frame_pacer,
        2,
    )
    .expect_err("missing state slot should fail");
    assert!(missing_error.contains("failed to read .gbstate state"));
    assert_eq!(harness.machine.capture_save_state(), before_missing);
    assert_eq!(
        super::super::load_machine_state_slot_if_present(
            &harness.session,
            &mut harness.machine,
            &mut harness.runtime,
            &mut harness.frame_pacer,
            2,
        )
        .expect("missing state slot should be ignored for hotkey-style loads"),
        None
    );
    assert_eq!(harness.machine.capture_save_state(), before_missing);

    let corrupt_path =
        machine_state_slot_path(&harness.session, 2).expect("corrupt path should resolve");
    fs::create_dir_all(
        corrupt_path
            .parent()
            .expect("corrupt path parent should exist"),
    )
    .expect("state dir should be creatable");
    fs::write(&corrupt_path, b"not-a-gbstate").expect("corrupt state should be writable");
    let before_corrupt = harness.machine.capture_save_state();
    let corrupt_error = load_machine_state_slot(
        &harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.frame_pacer,
        2,
    )
    .expect_err("corrupt state slot should fail");
    assert!(corrupt_error.contains("failed to decode .gbstate state"));
    assert_eq!(harness.machine.capture_save_state(), before_corrupt);
    let optional_corrupt_error = super::super::load_machine_state_slot_if_present(
        &harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.frame_pacer,
        2,
    )
    .expect_err("corrupt state slot should still fail for hotkey-style loads");
    assert!(optional_corrupt_error.contains("failed to decode .gbstate state"));
}

#[test]
fn state_slot_autoload_restores_existing_slot_and_ignores_missing_slot_on_rom_load() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("state-slot-autoload", true, false, false);
    let rom_path = harness
        .session
        .rom_path()
        .expect("harness should start with ROM")
        .to_path_buf();

    harness.machine.write_bus(0xC000, 0x42);
    let slot_path = save_machine_state_slot(&harness.session, &harness.machine, 2)
        .expect("autoload slot should save");
    assert!(slot_path.is_file());

    harness.session.config.machine_state.autoload_slot = Some(2);
    harness.machine.write_bus(0xC000, 0x99);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(rom_path.clone()))
        .expect("autoload ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("autoload ROM should load");
    assert_eq!(harness.machine.read_bus(0xC000), 0x42);
    assert!(harness.runtime.rewind_buffer.is_empty());

    harness.session.config.machine_state.autoload_slot = Some(3);
    harness.machine.write_bus(0xC000, 0x55);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(rom_path))
        .expect("missing autoload ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("missing autoload slot should not fail ROM load");
    assert_ne!(harness.machine.read_bus(0xC000), 0x55);
}

#[test]
fn state_slot_hotkeys_save_load_and_select_slots() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("state-slot-hotkeys", true, false, false);

    harness.machine.write_bus(0xC000, 0x42);
    harness.push_key(Keycode::F1, true);
    harness
        .process_events()
        .expect("save-state hotkey should process");
    let slot_one_path =
        machine_state_slot_path(&harness.session, 1).expect("slot path should resolve");
    assert!(slot_one_path.is_file());

    harness.push_key(Keycode::_3, true);
    harness
        .process_events()
        .expect("slot-3 hotkey should process");
    assert_eq!(harness.runtime.machine_state_slot, 3);
    harness.push_key(Keycode::_1, true);
    harness
        .process_events()
        .expect("slot-1 hotkey should process");
    assert_eq!(harness.runtime.machine_state_slot, 1);

    harness.machine.write_bus(0xC000, 0x99);
    assert_eq!(harness.machine.read_bus(0xC000), 0x99);
    harness.push_key(Keycode::F2, true);
    harness
        .process_events()
        .expect("load-state hotkey should process");
    assert_eq!(harness.machine.read_bus(0xC000), 0x42);
}

#[test]
fn state_slots_are_disabled_for_linked_desktop_sessions() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("state-slot-linked", true, false, false);
    let primary = harness.machine.primary_machine().clone();
    let secondary = primary.clone();
    harness.machine =
        super::super::DesktopEmulationSession::new_linked_dmg04_two_player(primary, secondary)
            .expect("matching machines should link");

    assert!(!machine_state_actions_available(
        &harness.session,
        &harness.machine
    ));
    let presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(!presentation.machine_state_available);
    let save_error = save_machine_state_slot(&harness.session, &harness.machine, 1)
        .expect_err("linked sessions should not save .gbstate slots");
    assert!(save_error.contains("single-machine sessions"));
}

#[test]
fn rewind_records_during_single_machine_stepping_and_is_disabled_when_linked() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("rewind-recording", true, false, false);

    let initial_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(!initial_presentation.rewind_available);
    assert!(initial_presentation.rewind_supported);
    assert!(harness.runtime.rewind_buffer.is_empty());
    let empty_hud = super::super::current_rewind_hud_snapshot(
        &harness.runtime,
        &harness.session,
        &harness.machine,
    );
    assert!(empty_hud.supported);
    assert!(empty_hud.enabled);
    assert_eq!(empty_hud.snapshot_count, 0);

    for _ in 0..16 {
        harness.machine.step_t_cycle();
        super::super::record_desktop_rewind_point(
            &harness.session,
            &harness.machine,
            &mut harness.runtime,
        );
    }
    assert!(!harness.runtime.rewind_buffer.is_empty());
    let recorded_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(recorded_presentation.rewind_available);
    let recorded_hud = super::super::current_rewind_hud_snapshot(
        &harness.runtime,
        &harness.session,
        &harness.machine,
    );
    assert!(recorded_hud.snapshot_count > 0);
    assert!(recorded_hud.accounted_bytes > 0);
    assert_eq!(
        recorded_hud.max_bytes,
        harness.runtime.rewind_buffer.config().max_estimated_bytes
    );

    let primary = harness.machine.primary_machine().clone();
    let secondary = primary.clone();
    harness.machine =
        super::super::DesktopEmulationSession::new_linked_dmg04_two_player(primary, secondary)
            .expect("matching machines should link");
    super::super::reset_frontend_timeline_state(&mut harness.runtime);
    for _ in 0..16 {
        harness.machine.step_t_cycle();
        super::super::record_desktop_rewind_point(
            &harness.session,
            &harness.machine,
            &mut harness.runtime,
        );
    }
    assert!(harness.runtime.rewind_buffer.is_empty());
    let linked_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    assert!(!linked_presentation.rewind_available);
    assert!(!linked_presentation.rewind_supported);
    let linked_hud = super::super::current_rewind_hud_snapshot(
        &harness.runtime,
        &harness.session,
        &harness.machine,
    );
    assert!(!linked_hud.supported);
}

#[test]
fn rewind_restore_once_resets_host_timeline_state() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("rewind-menu-restore", true, false, false);

    for _ in 0..8 {
        harness.machine.step_t_cycle();
    }
    let target_state = harness.machine.capture_save_state();
    assert!(
        harness
            .runtime
            .rewind_buffer
            .record_subframe(harness.machine.primary_machine())
    );
    harness
        .runtime
        .rewind_frame_tracker
        .observe(harness.machine.primary_machine());
    for _ in 0..32 {
        harness.machine.step_t_cycle();
    }
    assert_ne!(harness.machine.capture_save_state(), target_state);
    harness.frame_pacer.next_frame_start = Instant::now() + Duration::from_secs(60);

    let restored = super::super::rewind_desktop_session_once(
        &harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.frame_pacer,
    )
    .expect("rewind restore should execute");
    assert!(restored, "rewind restore should consume a snapshot");

    assert_eq!(harness.machine.capture_save_state(), target_state);
    assert!(harness.runtime.rewind_buffer.is_empty());
    assert_eq!(harness.runtime.rewind_frame_tracker.previous(), None);
    assert!(harness.frame_pacer.next_frame_start <= Instant::now() + Duration::from_millis(100));
}

#[test]
fn rewind_speed_presets_map_to_retuned_restore_steps() {
    assert_eq!(super::super::rewind_restore_steps_for_speed(1), 2);
    assert_eq!(super::super::rewind_restore_steps_for_speed(2), 4);
    assert_eq!(super::super::rewind_restore_steps_for_speed(4), 8);
    assert_eq!(
        super::super::rewind_restore_steps_for_speed(0),
        2,
        "invalid persisted zero speed should fall back to the slowest retuned preset"
    );
}

#[test]
fn rewind_speed_consumes_multiple_snapshots_per_restore_frame() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("rewind-speed-steps", true, false, false);
    let mut captured_states = Vec::new();

    for _ in 0..3 {
        for _ in 0..8 {
            harness.machine.step_t_cycle();
        }
        captured_states.push(harness.machine.capture_save_state());
        assert!(
            harness
                .runtime
                .rewind_buffer
                .record_frame_boundary(harness.machine.primary_machine())
        );
    }

    for _ in 0..8 {
        harness.machine.step_t_cycle();
    }
    assert_ne!(harness.machine.capture_save_state(), captured_states[1]);

    let restored = super::super::rewind_desktop_session_steps(
        &harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.frame_pacer,
        2,
    )
    .expect("multi-step rewind should execute");

    assert!(restored);
    assert_eq!(harness.machine.capture_save_state(), captured_states[1]);
    assert_eq!(harness.runtime.rewind_buffer.stats().len, 1);
}

#[test]
fn rewind_restore_releases_joypad_buttons_captured_in_snapshot() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("rewind-input-release", true, false, false);

    harness
        .machine
        .primary_machine_mut()
        .set_joypad_button_pressed(JoypadButton::Right, true);
    harness.machine.step_t_cycle();
    assert_ne!(
        harness
            .machine
            .primary_machine()
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );
    assert!(
        harness
            .runtime
            .rewind_buffer
            .record_subframe(harness.machine.primary_machine())
    );

    harness
        .machine
        .primary_machine_mut()
        .set_joypad_button_pressed(JoypadButton::Right, false);
    harness.machine.step_t_cycle();
    assert_eq!(
        harness
            .machine
            .primary_machine()
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );

    let restored = super::super::rewind_desktop_session_once(
        &harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.frame_pacer,
    )
    .expect("rewind restore should execute");
    assert!(restored, "rewind restore should consume a snapshot");
    harness.machine.step_t_cycle();

    assert_eq!(
        harness
            .machine
            .primary_machine()
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );
}

#[test]
fn host_hold_hotkey_state_tracks_rewind_and_fast_forward_key_events() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("rewind-hotkey", true, false, false);

    harness.push_key(Keycode::LShift, true);
    harness
        .process_events()
        .expect("rewind keydown should process");
    assert!(harness.runtime.rewind_hotkey_active);

    harness.push_key(Keycode::LShift, false);
    harness
        .process_events()
        .expect("rewind keyup should process");
    assert!(!harness.runtime.rewind_hotkey_active);

    harness.push_key(Keycode::RShift, true);
    harness
        .process_events()
        .expect("fast-forward keydown should process");
    assert!(harness.runtime.fast_forward_hotkey_active);

    harness.push_key(Keycode::RShift, false);
    harness
        .process_events()
        .expect("fast-forward keyup should process");
    assert!(!harness.runtime.fast_forward_hotkey_active);
}

#[test]
fn fast_forward_enabled_option_gates_momentary_acceleration() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("fast-forward-active", true, false, false);

    assert!(
        harness.session.config.fast_forward.enabled,
        "Fast Forward should be available by default"
    );
    assert!(!super::super::fast_forward_active(
        &harness.runtime,
        &harness.session,
        &harness.machine
    ));

    harness.runtime.fast_forward_hotkey_active = true;
    assert!(
        super::super::fast_forward_active(&harness.runtime, &harness.session, &harness.machine),
        "momentary hotkey should activate when Fast Forward is available"
    );
    harness.runtime.fast_forward_hotkey_active = false;

    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleFastForwardEnabled)
            .expect("fast-forward availability should toggle")
            .is_none()
    );
    assert!(!harness.session.config.fast_forward.enabled);
    harness.runtime.fast_forward_hotkey_active = true;
    assert!(
        !super::super::fast_forward_active(&harness.runtime, &harness.session, &harness.machine),
        "FAST FORWARD OFF should make the associated button a no-op"
    );
    harness.runtime.fast_forward_hotkey_active = false;

    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleFastForwardSpeed)
            .expect("fast-forward speed should cycle")
            .is_none()
    );
    assert_eq!(harness.session.config.fast_forward.speed_multiplier, 8);
}

#[test]
fn fast_forward_helpers_cover_hold_state_and_indicator() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("fast-forward-helpers", true, false, false);

    assert!(!super::super::fast_forward_hold_active(&harness.runtime));
    assert!(!super::super::fast_forward_active(
        &harness.runtime,
        &harness.session,
        &harness.machine
    ));
    assert!(!super::super::fast_forward_indicator_visible(
        &harness.runtime,
        &harness.session,
        &harness.machine,
        false,
    ));

    harness.runtime.fast_forward_gamepad_active = true;
    assert!(super::super::fast_forward_hold_active(&harness.runtime));
    assert!(super::super::fast_forward_active(
        &harness.runtime,
        &harness.session,
        &harness.machine
    ));
    assert!(super::super::fast_forward_indicator_visible(
        &harness.runtime,
        &harness.session,
        &harness.machine,
        false,
    ));

    harness.runtime.fast_forward_gamepad_active = false;
    assert!(super::super::fast_forward_indicator_visible(
        &harness.runtime,
        &harness.session,
        &harness.machine,
        true,
    ));

    harness.session.config.fast_forward.enabled = false;
    harness.runtime.fast_forward_hotkey_active = true;
    assert!(!super::super::fast_forward_active(
        &harness.runtime,
        &harness.session,
        &harness.machine
    ));
    assert!(!super::super::fast_forward_indicator_visible(
        &harness.runtime,
        &harness.session,
        &harness.machine,
        true,
    ));
}

#[test]
fn gamepad_hold_latches_clear_together_on_active_device_changes() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("gamepad-hold-latch-clear", true, false, false);

    harness.runtime.rewind_gamepad_active = true;
    harness.runtime.fast_forward_gamepad_active = true;
    harness.runtime.gamepad_trigger_state.left = true;
    harness.runtime.gamepad_trigger_state.right = true;
    super::super::clear_gamepad_hold_latches(&mut harness.runtime);

    assert!(!harness.runtime.rewind_gamepad_active);
    assert!(!harness.runtime.fast_forward_gamepad_active);
    assert_eq!(
        harness.runtime.gamepad_trigger_state,
        super::super::GamepadTriggerState::default()
    );
    assert!(!super::super::rewind_hold_active(&harness.runtime));
    assert!(!super::super::fast_forward_hold_active(&harness.runtime));
}

#[test]
fn fast_forward_audio_suppression_toggles_host_output() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("fast-forward-audio-suppression", true, true, false);

    assert!(!harness.runtime.fast_forward_audio_suppressed);
    super::super::sync_fast_forward_audio_state(&mut harness.runtime, true)
        .expect("entering Fast Forward should clear audio output");
    assert!(harness.runtime.fast_forward_audio_suppressed);
    super::super::sync_fast_forward_audio_state(&mut harness.runtime, true)
        .expect("already-suppressed Fast Forward audio state should be idempotent");
    assert!(harness.runtime.fast_forward_audio_suppressed);
    super::super::sync_fast_forward_audio_state(&mut harness.runtime, false)
        .expect("leaving Fast Forward should clear audio output");
    assert!(!harness.runtime.fast_forward_audio_suppressed);
    super::super::sync_fast_forward_audio_state(&mut harness.runtime, false)
        .expect("inactive Fast Forward audio state should be idempotent");
    assert!(!harness.runtime.fast_forward_audio_suppressed);
}

#[test]
fn fast_forward_host_pacing_temporarily_suppresses_renderer_vsync() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("fast-forward-host-pacing", true, false, false);

    assert!(harness.runtime.video_options.vsync);
    assert!(!harness.runtime.fast_forward_vsync_suppressed);
    super::super::sync_fast_forward_host_pacing_state(
        &mut harness.canvas,
        &mut harness.frame_pacer,
        &mut harness.runtime,
        true,
    )
    .expect("entering Fast Forward should disable renderer vsync");
    assert!(harness.runtime.video_options.vsync);
    assert!(harness.runtime.fast_forward_vsync_suppressed);

    super::super::sync_fast_forward_host_pacing_state(
        &mut harness.canvas,
        &mut harness.frame_pacer,
        &mut harness.runtime,
        true,
    )
    .expect("already-suppressed Fast Forward host pacing should be idempotent");
    assert!(harness.runtime.fast_forward_vsync_suppressed);

    super::super::sync_fast_forward_host_pacing_state(
        &mut harness.canvas,
        &mut harness.frame_pacer,
        &mut harness.runtime,
        false,
    )
    .expect("leaving Fast Forward should restore configured renderer vsync");
    assert!(harness.runtime.video_options.vsync);
    assert!(!harness.runtime.fast_forward_vsync_suppressed);

    harness.runtime.video_options.vsync = false;
    super::super::apply_renderer_vsync(&mut harness.canvas, &mut harness.frame_pacer, false)
        .expect("test should disable configured renderer vsync");
    super::super::sync_fast_forward_host_pacing_state(
        &mut harness.canvas,
        &mut harness.frame_pacer,
        &mut harness.runtime,
        true,
    )
    .expect("Fast Forward should not mark vsync suppressed when vsync is already off");
    assert!(!harness.runtime.fast_forward_vsync_suppressed);
}

#[test]
fn fast_forward_frame_budget_uses_speed_multiplier() {
    assert_eq!(super::super::fast_forward_frame_budget(0), 1);
    assert_eq!(super::super::fast_forward_frame_budget(4), 4);
    assert_eq!(super::super::fast_forward_frame_budget(8), 8);
    assert_eq!(super::super::fast_forward_frame_budget(16), 16);
}

#[test]
fn host_frame_pacing_is_skipped_for_test_runner_and_fast_forward() {
    assert!(!super::super::should_skip_host_frame_pacing(
        false, false, false
    ));
    assert!(super::super::should_skip_host_frame_pacing(
        true, false, false
    ));
    assert!(super::super::should_skip_host_frame_pacing(
        false, true, false
    ));
    assert!(super::super::should_skip_host_frame_pacing(
        false, false, true
    ));
}

#[test]
fn fast_forward_runtime_step_advances_multiple_emulated_frames() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("fast-forward-runtime-step", true, true, false);
    let mut stable_rom = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
    stable_rom[ENTRY_POINT_START..ENTRY_POINT_START + 2].copy_from_slice(&[0x18, 0xFE]);
    harness.session.loaded_rom = Some(super::super::LoadedRom {
        path: harness.root.join("fast-forward-runtime-step.gb"),
        bytes: stable_rom.clone(),
    });
    harness.machine = super::super::DesktopEmulationSession::new_single(
        super::super::load_machine_for_rom(
            &harness.session.config,
            &harness.session.current_dir,
            &stable_rom,
        )
        .expect("stable ROM should load for Fast Forward runtime test")
        .machine,
    );
    harness.session.config.fast_forward.enabled = true;
    harness.session.config.fast_forward.speed_multiplier = 4;
    harness.runtime.fast_forward_hotkey_active = true;

    let before_t_cycle = harness.machine.primary_machine().next_t_cycle().get();
    let (result, fast_forwarded_frames) = {
        let mut context = super::super::FrontendActionContext {
            session: &mut harness.session,
            machine: &mut harness.machine,
            runtime: &mut harness.runtime,
            performance_counter: &mut harness.performance_counter,
            frame_pacer: &mut harness.frame_pacer,
            settings_store: &mut harness.settings_store,
        };
        super::super::step_fast_forward_frames(
            &mut harness.event_pump,
            &mut harness.canvas,
            &mut context,
        )
        .expect("Fast Forward stepping should advance runtime frames")
    };
    let advanced_t_cycles = harness
        .machine
        .primary_machine()
        .next_t_cycle()
        .get()
        .saturating_sub(before_t_cycle);

    assert_eq!(result.signal, super::super::LoopSignal::Continue);
    assert_eq!(fast_forwarded_frames, 4);
    assert!(
        advanced_t_cycles >= gb_core::DMG_T_CYCLES_PER_FRAME.saturating_mul(4),
        "retuned 2x Fast Forward advanced only {advanced_t_cycles} T-cycles"
    );
    assert!(
        harness.runtime.rewind_buffer.is_empty(),
        "Fast Forward should not populate rewind history while held"
    );
}

#[test]
fn fast_forward_runtime_step_stops_before_counting_quit_frame() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("fast-forward-runtime-quit", true, true, false);
    harness.session.config.fast_forward.speed_multiplier = 4;
    harness.runtime.fast_forward_hotkey_active = true;
    harness
        .sdl
        .event()
        .expect("FF quit event subsystem")
        .push_event(Event::Quit { timestamp: 0 })
        .expect("quit event should be pushable");

    let (result, fast_forwarded_frames) = {
        let mut context = super::super::FrontendActionContext {
            session: &mut harness.session,
            machine: &mut harness.machine,
            runtime: &mut harness.runtime,
            performance_counter: &mut harness.performance_counter,
            frame_pacer: &mut harness.frame_pacer,
            settings_store: &mut harness.settings_store,
        };
        super::super::step_fast_forward_frames(
            &mut harness.event_pump,
            &mut harness.canvas,
            &mut context,
        )
        .expect("Fast Forward quit stepping should return cleanly")
    };

    assert_eq!(result.signal, super::super::LoopSignal::Quit);
    assert_eq!(fast_forwarded_frames, 0);
}

#[test]
fn fast_forward_runtime_step_stops_after_hold_release() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("fast-forward-runtime-release", true, true, false);
    let mut stable_rom = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
    stable_rom[ENTRY_POINT_START..ENTRY_POINT_START + 2].copy_from_slice(&[0x18, 0xFE]);
    harness.session.loaded_rom = Some(super::super::LoadedRom {
        path: harness.root.join("fast-forward-runtime-release.gb"),
        bytes: stable_rom.clone(),
    });
    harness.machine = super::super::DesktopEmulationSession::new_single(
        super::super::load_machine_for_rom(
            &harness.session.config,
            &harness.session.current_dir,
            &stable_rom,
        )
        .expect("stable ROM should load for Fast Forward release test")
        .machine,
    );
    harness.session.config.fast_forward.enabled = true;
    harness.session.config.fast_forward.speed_multiplier = 4;
    harness.runtime.fast_forward_hotkey_active = true;
    harness.push_key(Keycode::RShift, false);

    let (result, fast_forwarded_frames) = {
        let mut context = super::super::FrontendActionContext {
            session: &mut harness.session,
            machine: &mut harness.machine,
            runtime: &mut harness.runtime,
            performance_counter: &mut harness.performance_counter,
            frame_pacer: &mut harness.frame_pacer,
            settings_store: &mut harness.settings_store,
        };
        super::super::step_fast_forward_frames(
            &mut harness.event_pump,
            &mut harness.canvas,
            &mut context,
        )
        .expect("Fast Forward release stepping should return cleanly")
    };

    assert_eq!(result.signal, super::super::LoopSignal::Continue);
    assert_eq!(fast_forwarded_frames, 1);
    assert!(!harness.runtime.fast_forward_hotkey_active);
}

#[test]
fn rewind_empty_history_reports_no_restore_without_mutating_machine() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("rewind-empty-history", true, false, false);
    let before = harness.machine.capture_save_state();

    let restored = super::super::rewind_desktop_session_once(
        &harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.frame_pacer,
    )
    .expect("empty rewind should not surface a modal-worthy restore error");

    assert!(!restored);
    assert_eq!(harness.machine.capture_save_state(), before);
    assert!(harness.runtime.rewind_buffer.is_empty());
}

#[test]
fn external_save_helpers_cover_disabled_non_battery_and_io_error_paths() {
    let _guard = crate::lock_sdl_test();

    let no_rom_harness = FrontendHarness::new("external-save-no-rom", false, false, false);
    assert_eq!(
        super::super::external_save_default_file_name(&no_rom_harness.session),
        "save.sav"
    );
    assert_eq!(
        super::super::external_save_export_dialog_default_location(&no_rom_harness.session),
        no_rom_harness.root.join("saves/export/save.sav")
    );
    assert_eq!(
        super::super::external_save_import_dialog_default_location(&no_rom_harness.session),
        no_rom_harness.root.join("saves/import/save.sav")
    );
    assert_eq!(
        super::super::resolve_external_save_export_path(
            &no_rom_harness.session,
            PathBuf::from("relative/current")
        ),
        no_rom_harness.root.join("relative/current.sav")
    );
    assert_eq!(
        super::super::resolve_external_save_import_path(
            &no_rom_harness.session,
            PathBuf::from("relative/current")
        ),
        no_rom_harness.root.join("relative/current")
    );
    assert!(
        super::super::primary_save_root_and_key(&no_rom_harness.session)
            .expect("no-ROM sessions should not fail save root lookup")
            .is_none()
    );

    drop(no_rom_harness);

    let mut disabled_harness = FrontendHarness::new("external-save-disabled", true, false, false);
    let non_battery_import_path = disabled_harness.root.join("non-battery.sav");
    fs::write(&non_battery_import_path, vec![0x11; 8 * 1024])
        .expect("non-battery import fixture should be writable");
    {
        let FrontendHarness {
            session,
            machine,
            runtime,
            settings_store,
            performance_counter,
            frame_pacer,
            ..
        } = &mut disabled_harness;
        let mut context = super::super::FrontendActionContext {
            session,
            machine,
            runtime,
            performance_counter,
            frame_pacer,
            settings_store,
        };
        let import_error = super::super::import_external_save_for_current_rom(
            non_battery_import_path,
            &mut context,
        )
        .expect_err("non-battery games should reject external imports");
        assert!(import_error.contains("does not expose battery-backed persistence"));
        let export_error = super::super::export_current_external_save(
            PathBuf::from("no-battery.sav"),
            &mut context,
        )
        .expect_err("non-battery games should reject external exports");
        assert!(export_error.contains("does not expose battery-backed persistence"));
    }

    disabled_harness.session.config.saves.enabled = false;
    assert_eq!(
        super::super::external_save_default_file_name(&disabled_harness.session),
        "external-save-disabled.sav"
    );
    assert!(
        super::super::primary_save_root_and_key(&disabled_harness.session)
            .expect("disabled saves should resolve as absent")
            .is_none()
    );
    {
        let FrontendHarness {
            session,
            machine,
            runtime,
            settings_store,
            performance_counter,
            frame_pacer,
            ..
        } = &mut disabled_harness;
        let mut context = super::super::FrontendActionContext {
            session,
            machine,
            runtime,
            performance_counter,
            frame_pacer,
            settings_store,
        };
        let import_error = super::super::import_external_save_for_current_rom(
            PathBuf::from("missing-disabled.sav"),
            &mut context,
        )
        .expect_err("disabled saves should reject external imports before reading files");
        assert!(import_error.contains("save support is disabled"));
    }
    drop(disabled_harness);

    let mut bad_key_harness = FrontendHarness::new("external-save-bad-key", false, false, false);
    bad_key_harness.session.loaded_rom = Some(super::super::LoadedRom {
        path: bad_key_harness.root.join("bad*key.gb"),
        bytes: Vec::new(),
    });
    let key_error = super::super::primary_save_root_and_key(&bad_key_harness.session)
        .expect_err("unsafe ROM stems should fail save key resolution");
    assert!(key_error.contains("invalid character `*`"));
    drop(bad_key_harness);

    let mut battery_harness =
        FrontendHarness::new("external-save-error-paths", false, false, false);
    let battery_rom_name = "battery.gb";
    let battery_rom_path = battery_harness.root.join(battery_rom_name);
    fs::write(
        &battery_rom_path,
        build_test_rom(32 * 1024, 0x09, 0x00, 0x02),
    )
    .expect("battery ROM should be writable");
    battery_harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(battery_rom_name)))
        .expect("battery ROM selection should send");
    battery_harness
        .process_pending_open_rom_dialog()
        .expect("battery ROM should load");
    battery_harness
        .machine
        .primary_machine_mut()
        .restore_cartridge_persistent_state(&PersistentCartState::NoMbcRam {
            ram: vec![0x12; 8 * 1024],
        })
        .expect("battery RAM state should restore");

    let (save_root, save_key) = super::super::primary_save_root_and_key(&battery_harness.session)
        .expect("battery saves should resolve")
        .expect("battery saves should be enabled");

    {
        let FrontendHarness {
            session,
            machine,
            runtime,
            settings_store,
            performance_counter,
            frame_pacer,
            ..
        } = &mut battery_harness;
        let mut context = super::super::FrontendActionContext {
            session,
            machine,
            runtime,
            performance_counter,
            frame_pacer,
            settings_store,
        };

        let blocking_parent = context.session.current_dir.join("blocked-export-parent");
        fs::write(&blocking_parent, b"file").expect("blocking export parent should be writable");
        let export_directory_error = super::super::export_current_external_save(
            blocking_parent.join("current.sav"),
            &mut context,
        )
        .expect_err("file parents should block export directory creation");
        assert!(export_directory_error.contains("failed to create external save export directory"));

        let blocked_export_target = context.session.current_dir.join("blocked-write.sav");
        fs::create_dir_all(&blocked_export_target)
            .expect("blocked export target directory should be creatable");
        let export_write_error =
            super::super::export_current_external_save(blocked_export_target, &mut context)
                .expect_err("directory targets should block export writes");
        assert!(export_write_error.contains("failed to write external save"));

        let import_read_error = super::super::import_external_save_for_current_rom(
            PathBuf::from("missing-import.sav"),
            &mut context,
        )
        .expect_err("missing external saves should surface read errors");
        assert!(import_read_error.contains("failed to read external save"));

        let invalid_import_path = context.session.current_dir.join("invalid-import.sav");
        fs::write(&invalid_import_path, [0xAA]).expect("invalid external save should be writable");
        let invalid_import_error =
            super::super::import_external_save_for_current_rom(invalid_import_path, &mut context)
                .expect_err("invalid external saves should surface conversion errors");
        assert!(invalid_import_error.contains("failed to import external save"));

        let valid_import_path = context.session.current_dir.join("valid-import.sav");
        fs::write(&valid_import_path, vec![0x44; 8 * 1024])
            .expect("valid external save should be writable");
        let store = FilesystemCartridgeSaveStore::new(&save_root);
        let target_save_path = store.external_path_for_key(&save_key);
        let mut blocked_temp_path = target_save_path.as_os_str().to_os_string();
        blocked_temp_path.push(".tmp");
        fs::create_dir_all(PathBuf::from(blocked_temp_path))
            .expect("blocked save temp path should be creatable");
        let close_error = super::super::import_external_save_for_current_rom(
            valid_import_path.clone(),
            &mut context,
        )
        .expect_err("blocked save sessions should surface close errors");
        assert!(close_error.contains("failed to save cartridge persistence (close)"));
        assert!(
            context.runtime.save_sessions[super::super::PlayerSlot::P1.index()].is_some(),
            "failed imports restore the previous primary save session"
        );

        context.runtime.save_sessions[super::super::PlayerSlot::P1.index()] = None;
        let import_write_error =
            super::super::import_external_save_for_current_rom(valid_import_path, &mut context)
                .expect_err("runtime save store errors should surface");
        assert!(
            import_write_error.contains("failed to write imported save"),
            "{import_write_error}"
        );
    }

    assert!(
        super::super::format_external_save_error(
            "export",
            gb_persistence::ExternalSaveError::UnsupportedPersistentState {
                state_kind: "test-state"
            }
        )
        .contains("failed to export external save")
    );
}
