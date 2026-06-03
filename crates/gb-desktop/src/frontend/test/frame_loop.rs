use super::*;

#[test]
fn step_until_next_frame_returns_quit_when_process_events_requests_exit() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("step-quit", true, false, false);
    harness
        .sdl
        .event()
        .expect("quit-path event subsystem")
        .push_event(Event::Quit { timestamp: 0 })
        .expect("quit event should be pushable");
    let FrontendHarness {
        event_pump,
        canvas,
        session,
        machine,
        runtime,
        settings_store,
        performance_counter,
        frame_pacer,
        ..
    } = &mut harness;
    let mut context = super::super::FrontendActionContext {
        session,
        machine,
        runtime,
        performance_counter,
        frame_pacer,
        settings_store,
    };
    let result = super::super::step_until_next_frame(event_pump, canvas, &mut context)
        .expect("quit-path stepping should succeed");
    assert_eq!(result.signal, super::super::LoopSignal::Quit);
    assert!(result.emulation_profile_request.is_none());
    assert_eq!(
        result.frame_loop_telemetry,
        super::super::FrameLoopTelemetry::default()
    );
}

#[test]
fn test_runner_events_ignore_input_and_step_polls_only_at_frame_boundary() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("test-runner-events", true, false, false);
    harness.session.test_runner = true;

    harness.push_key(Keycode::Z, true);
    assert_eq!(
        harness
            .process_test_runner_events()
            .expect("test-runner events should process"),
        super::super::LoopSignal::Continue
    );

    harness
        .sdl
        .event()
        .expect("test-runner event subsystem")
        .push_event(Event::Quit { timestamp: 0 })
        .expect("quit event should be pushable");
    assert_eq!(
        harness
            .step_until_next_frame()
            .expect("test-runner stepping should ignore queued SDL events"),
        super::super::LoopSignal::Continue
    );
    assert_eq!(
        harness
            .process_test_runner_events()
            .expect("test-runner quit event should process"),
        super::super::LoopSignal::Quit
    );
}

#[test]
fn test_runner_events_preserve_explicit_gamepad_input_only() {
    let _guard = crate::lock_sdl_test();
    let virtual_gamepad = VirtualGamepad::attach("Test Runner Pad");
    let mut harness = FrontendHarness::new("test-runner-gamepad", true, false, true);
    harness.session.test_runner = true;
    harness
        ._gamepad_subsystem
        .as_ref()
        .expect("gamepad subsystem")
        .update();
    harness
        .runtime
        .gamepad_manager
        .as_mut()
        .expect("gamepad manager")
        .set_preferred_device(
            gb_desktop::PreferredGamepadIdentity {
                path: None,
                name: Some("Test Runner Pad".to_string()),
            },
            harness
                .runtime
                .player_inputs
                .input_mut(super::super::PlayerSlot::P1),
            harness
                .machine
                .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        );

    harness.push_key(Keycode::Return, true);
    virtual_gamepad.set_button(Button::South, true);
    assert_eq!(
        harness
            .process_test_runner_events()
            .expect("test-runner gamepad events should process"),
        super::super::LoopSignal::Continue
    );
    harness.machine.step_t_cycle();
    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P1)
            .expect("P1 should always map to an active desktop machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0x20,
        "test-runner should poll explicitly enabled gamepad input but keep keyboard ignored"
    );
    assert!(!harness.runtime.menu_state.is_open());

    virtual_gamepad.set_button(Button::South, false);
    assert_eq!(
        harness
            .process_test_runner_events()
            .expect("test-runner gamepad release should process"),
        super::super::LoopSignal::Continue
    );
    harness.machine.step_t_cycle();
    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P1)
            .expect("P1 should always map to an active desktop machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );
}

#[test]
fn step_until_next_frame_returns_continue_without_profile_when_paused() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("step-paused", true, false, false);
    harness.runtime.paused = true;
    let FrontendHarness {
        event_pump,
        canvas,
        session,
        machine,
        runtime,
        settings_store,
        performance_counter,
        frame_pacer,
        ..
    } = &mut harness;
    let mut context = super::super::FrontendActionContext {
        session,
        machine,
        runtime,
        performance_counter,
        frame_pacer,
        settings_store,
    };
    let result = super::super::step_until_next_frame(event_pump, canvas, &mut context)
        .expect("paused stepping should succeed");
    assert_eq!(result.signal, super::super::LoopSignal::Continue);
    assert!(result.emulation_profile_request.is_none());
    assert_eq!(
        result.frame_loop_telemetry,
        super::super::FrameLoopTelemetry::default()
    );
}

#[test]
fn step_until_next_frame_releases_pending_mbc3_ticks_during_emulation() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("step-mbc3-rtc", true, false, false);
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(32 * 1024, 0x0F, 0x00, 0x00))
        .expect("MBC3 RTC cartridge should load");
    machine.advance_mbc3_cartridge_rtc_clock_ticks(32_767);
    harness.machine = super::super::DesktopEmulationSession::new_single(machine);
    harness.runtime.rtc_sync = super::super::HostRtcSync::new(SystemTime::now());
    harness.runtime.rtc_sync.pending_mbc3_clock_ticks = 1;

    let result = harness
        .step_until_next_frame()
        .expect("desktop stepping should succeed");

    assert_eq!(result, super::super::LoopSignal::Continue);
    assert_eq!(
        harness.machine.cartridge().persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: gb_core::Mbc3RtcPersistentState {
                seconds: 1,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );
}

#[test]
fn step_until_next_frame_skips_detailed_frame_telemetry_when_emulation_profiling_is_disabled() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("step-no-telemetry", true, true, false);
    harness.performance_counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | step-no-telemetry".to_string(),
        super::super::EmulationProfileMode::Disabled,
    );
    let FrontendHarness {
        event_pump,
        canvas,
        session,
        machine,
        runtime,
        settings_store,
        performance_counter,
        frame_pacer,
        ..
    } = &mut harness;
    let mut context = super::super::FrontendActionContext {
        session,
        machine,
        runtime,
        performance_counter,
        frame_pacer,
        settings_store,
    };
    let result = super::super::step_until_next_frame(event_pump, canvas, &mut context)
        .expect("stepping should still succeed without emulation profiling");
    assert_eq!(result.signal, super::super::LoopSignal::Continue);
    assert!(result.emulation_profile_request.is_none());
    assert_eq!(
        result.frame_loop_telemetry,
        super::super::FrameLoopTelemetry::default()
    );
}

#[test]
fn step_until_next_frame_returns_to_present_stop_forced_blank_without_frame_boundary() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("step-stop-forced-blank", true, false, false);
    harness.performance_counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | step-stop-forced-blank".to_string(),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 2,
            detail: super::super::EmulationProfileDetail::Full,
        },
    );
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_stop_test_rom())
        .expect("STOP test ROM should load");
    harness.machine = super::super::DesktopEmulationSession::new_single(machine);

    let FrontendHarness {
        event_pump,
        canvas,
        session,
        machine,
        runtime,
        settings_store,
        performance_counter,
        frame_pacer,
        ..
    } = &mut harness;
    let mut context = super::super::FrontendActionContext {
        session,
        machine,
        runtime,
        performance_counter,
        frame_pacer,
        settings_store,
    };
    let result = super::super::step_until_next_frame(event_pump, canvas, &mut context)
        .expect("STOP forced blank should return for presentation");

    assert_eq!(result.signal, super::super::LoopSignal::Continue);
    assert!(matches!(
        context.machine.cpu().execution_state(),
        CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
    ));
    assert_eq!(
        context.machine.ppu().snapshot().visible_output,
        PpuVisibleOutputState::ForcedBlank
    );
    assert_eq!(result.frame_loop_telemetry.frame_origin_crossings, 0);
    assert!(result.frame_loop_telemetry.stepped_t_cycles < 70_224);
}

#[test]
fn step_until_next_frame_returns_profile_requests_for_sampled_frames() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("step-profile", true, true, false);
    harness.performance_counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | step-profile".to_string(),
        super::super::EmulationProfileMode::SampledSummary {
            sample_every_frames: 2,
            detail: super::super::EmulationProfileDetail::CoreOnly,
        },
    );
    harness.performance_counter.presented_frames_total = 1;
    let FrontendHarness {
        event_pump,
        canvas,
        session,
        machine,
        runtime,
        settings_store,
        performance_counter,
        frame_pacer,
        ..
    } = &mut harness;
    let mut context = super::super::FrontendActionContext {
        session,
        machine,
        runtime,
        performance_counter,
        frame_pacer,
        settings_store,
    };
    let result = super::super::step_until_next_frame(event_pump, canvas, &mut context)
        .expect("sampled stepping should succeed");
    assert_eq!(result.signal, super::super::LoopSignal::Continue);
    let request = result
        .emulation_profile_request
        .expect("sampled frames should snapshot a profile request");
    assert_eq!(result.frame_loop_telemetry.start_ly, 0);
    assert_eq!(result.frame_loop_telemetry.start_dot, 0);
    assert_eq!(result.frame_loop_telemetry.end_ly, 0);
    assert_eq!(result.frame_loop_telemetry.end_dot, 0);
    assert!(result.frame_loop_telemetry.stepped_t_cycles > 0);
    assert_eq!(result.frame_loop_telemetry.frame_origin_crossings, 1);
    assert_eq!(
        request.detail,
        super::super::EmulationProfileDetail::CoreOnly
    );
    assert!(request.breakdown.host_event_poll_duration <= Duration::from_millis(50));
    assert!(request.breakdown.host_audio_submit_duration <= Duration::from_millis(50));
}
