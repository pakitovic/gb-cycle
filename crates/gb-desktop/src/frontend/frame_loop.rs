fn process_events(
    event_pump: &mut sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<LoopSignal, String> {
    let session = &mut *context.session;
    let machine = &mut *context.machine;
    let runtime = &mut *context.runtime;
    let performance_counter = &mut *context.performance_counter;
    let frame_pacer = &mut *context.frame_pacer;
    let settings_store = &mut *context.settings_store;
    let events = event_pump.poll_iter().collect::<Vec<_>>();
    for event in events {
        process_gamepad_manager_event(runtime, machine, &event)?;

        if runtime.printer_output.handle_event(&event)? {
            continue;
        }

        if runtime.menu_state.is_open() && runtime.menu_state.is_capturing_binding() {
            match &event {
                Event::Quit { .. } => return Ok(LoopSignal::Quit),
                Event::KeyDown {
                    keycode,
                    scancode,
                    repeat: false,
                    ..
                } if key_event_matches(DesktopKey::Escape, *keycode, *scancode) => {
                    runtime.menu_state.cancel_binding_capture();
                    continue;
                }
                Event::KeyDown {
                    keycode,
                    scancode,
                    repeat: false,
                    ..
                } => {
                    if let Some(target) = runtime.menu_state.pending_keyboard_binding_target() {
                        if let Some(key) = assignable_key_for_binding_target_from_key_event(
                            *keycode, *scancode, target,
                        ) && let Some(action) =
                            runtime.menu_state.handle_keyboard_binding_capture(key)
                        {
                            let mut context = FrontendActionContext {
                                session,
                                machine,
                                runtime,
                                performance_counter,
                                frame_pacer,
                                settings_store,
                            };
                            let _ = execute_menu_action(action, event_pump, canvas, &mut context)?;
                        }
                    } else if let Some(target) =
                        runtime.menu_state.pending_keyboard_menu_binding_target()
                        && let Some(key) = assignable_menu_key_for_binding_target_from_key_event(
                            *keycode, *scancode, target,
                        )
                        && let Some(action) =
                            runtime.menu_state.handle_keyboard_binding_capture(key)
                    {
                        let mut context = FrontendActionContext {
                            session,
                            machine,
                            runtime,
                            performance_counter,
                            frame_pacer,
                            settings_store,
                        };
                        let _ = execute_menu_action(action, event_pump, canvas, &mut context)?;
                    }
                    continue;
                }
                Event::ControllerButtonDown { which, button, .. }
                    if runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                        manager.is_active_gamepad(gamepad_event_joystick_id(*which))
                    }) =>
                {
                    if (runtime
                        .menu_state
                        .pending_gamepad_binding_target()
                        .is_some()
                        || runtime
                            .menu_state
                            .pending_gamepad_action_binding_target()
                            .is_some()
                        || runtime
                            .menu_state
                            .pending_gamepad_menu_binding_target()
                            .is_some())
                        && let Some(binding) = gamepad_button_binding_from_sdl_button(*button)
                        && let Some(action) =
                            runtime.menu_state.handle_gamepad_binding_capture(binding)
                    {
                        let mut context = FrontendActionContext {
                            session,
                            machine,
                            runtime,
                            performance_counter,
                            frame_pacer,
                            settings_store,
                        };
                        let _ = execute_menu_action(action, event_pump, canvas, &mut context)?;
                    }
                    continue;
                }
                Event::ControllerAxisMotion {
                    which, axis, value, ..
                } if runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                    manager.is_active_gamepad(gamepad_event_joystick_id(*which))
                }) =>
                {
                    if (runtime
                        .menu_state
                        .pending_gamepad_binding_target()
                        .is_some()
                        || runtime
                            .menu_state
                            .pending_gamepad_action_binding_target()
                            .is_some()
                        || runtime
                            .menu_state
                            .pending_gamepad_menu_binding_target()
                            .is_some())
                        && gamepad_trigger_axis_is_pressed(*value)
                        && let Some(binding) = gamepad_button_binding_from_sdl_axis(*axis)
                        && let Some(action) =
                            runtime.menu_state.handle_gamepad_binding_capture(binding)
                    {
                        let mut context = FrontendActionContext {
                            session,
                            machine,
                            runtime,
                            performance_counter,
                            frame_pacer,
                            settings_store,
                        };
                        let _ = execute_menu_action(action, event_pump, canvas, &mut context)?;
                    }
                    continue;
                }
                _ => continue,
            }
        }

        match &event {
            Event::Quit { .. } => return Ok(LoopSignal::Quit),
            Event::KeyDown {
                keycode,
                scancode,
                repeat: false,
                ..
            } if !runtime.menu_state.is_open()
                && key_event_matches(DesktopKey::Escape, *keycode, *scancode) =>
            {
                toggle_menu(event_pump, canvas.window(), session, machine, runtime)?;
                continue;
            }
            Event::ControllerButtonDown { which, button, .. }
                if *button == Button::Guide
                    && runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                        manager.is_active_gamepad(gamepad_event_joystick_id(*which))
                    }) =>
            {
                if runtime.menu_state.is_open() {
                    let presentation =
                        current_menu_presentation(canvas.window(), runtime, machine, session);
                    if let Some(action) = runtime
                        .menu_state
                        .handle_input(MenuInput::Cancel, presentation)
                    {
                        let mut context = FrontendActionContext {
                            session,
                            machine,
                            runtime,
                            performance_counter,
                            frame_pacer,
                            settings_store,
                        };
                        let _ = execute_menu_action(action, event_pump, canvas, &mut context)?;
                    }
                } else {
                    toggle_menu(event_pump, canvas.window(), session, machine, runtime)?;
                }
                continue;
            }
            _ => {}
        }

        if runtime.menu_state.is_open() {
            let presentation =
                current_menu_presentation(canvas.window(), runtime, machine, session);
            let menu_action = match &event {
                Event::KeyDown {
                    keycode,
                    scancode,
                    repeat: false,
                    ..
                } => menu_input_for_key_event(runtime.keyboard_bindings.menu, *keycode, *scancode)
                    .and_then(|input| runtime.menu_state.handle_input(input, presentation)),
                Event::ControllerButtonDown { which, button, .. }
                    if runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                        manager.is_active_gamepad(gamepad_event_joystick_id(*which))
                    }) =>
                {
                    runtime
                        .gamepad_manager
                        .as_ref()
                        .and_then(|manager| {
                            menu_input_for_gamepad_button(manager.menu_bindings(), *button)
                        })
                        .and_then(|input| runtime.menu_state.handle_input(input, presentation))
                }
                Event::ControllerAxisMotion {
                    which, axis, value, ..
                } if runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                    manager.is_active_gamepad(gamepad_event_joystick_id(*which))
                }) =>
                {
                    if let Some((binding, true)) = gamepad_trigger_event_binding(
                        &mut runtime.gamepad_trigger_state,
                        *axis,
                        *value,
                    ) {
                        runtime
                            .gamepad_manager
                            .as_ref()
                            .and_then(|manager| {
                                menu_input_for_gamepad_binding(manager.menu_bindings(), binding)
                            })
                            .and_then(|input| runtime.menu_state.handle_input(input, presentation))
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(action) = menu_action {
                let mut context = FrontendActionContext {
                    session,
                    machine,
                    runtime,
                    performance_counter,
                    frame_pacer,
                    settings_store,
                };
                if let Some(signal) = execute_menu_action(action, event_pump, canvas, &mut context)?
                {
                    return Ok(signal);
                }
            }
            continue;
        }

        match event {
            Event::KeyDown {
                keycode,
                scancode,
                repeat,
                ..
            } => {
                if !repeat {
                    match hotkey_action_for_key_event(&runtime.keyboard_bindings, keycode, scancode)
                    {
                        HotkeyAction::None => {}
                        HotkeyAction::ManualSave => {
                            flush_runtime_save_sessions_if_changed(
                                runtime,
                                machine,
                                "manual-hotkey",
                            )?;
                        }
                        HotkeyAction::SaveState => {
                            match save_machine_state_slot(
                                session,
                                machine,
                                runtime.machine_state_slot,
                            ) {
                                Ok(path) => eprintln!("info: state saved to {}", path.display()),
                                Err(error) => {
                                    show_warning_message(
                                        Some(canvas.window()),
                                        "Save State",
                                        &error,
                                    );
                                    eprintln!("warning: {error}");
                                }
                            }
                        }
                        HotkeyAction::LoadState => {
                            handle_load_machine_state_action(
                                session,
                                machine,
                                runtime,
                                performance_counter,
                                frame_pacer,
                                canvas,
                            )?;
                        }
                        HotkeyAction::SelectStateSlot(slot) => {
                            runtime.machine_state_slot = slot;
                        }
                        HotkeyAction::Reset => {
                            reset_machine(
                                canvas.window(),
                                session,
                                machine,
                                runtime,
                                settings_store,
                            )?;
                            let keyboard_bindings = runtime.keyboard_bindings;
                            sync_live_input_state(event_pump, &keyboard_bindings, machine, runtime);
                        }
                        HotkeyAction::Rewind => {
                            runtime.rewind_hotkey_active = true;
                        }
                        HotkeyAction::FastForward => {
                            runtime.fast_forward_hotkey_active = true;
                        }
                        HotkeyAction::ToggleFullscreen => {
                            toggle_fullscreen(canvas.window_mut())?;
                            runtime.video_options.fullscreen =
                                canvas.window().fullscreen_state() != FullscreenType::Off;
                            if !session.test_runner {
                                settings_store.set_fullscreen(runtime.video_options.fullscreen)?;
                            }
                        }
                        HotkeyAction::TogglePerformanceHud => {
                            runtime.video_options.show_performance_hud =
                                !runtime.video_options.show_performance_hud;
                            if !session.test_runner {
                                settings_store.set_show_performance_hud(
                                    runtime.video_options.show_performance_hud,
                                )?;
                            }
                        }
                    }

                    if key_event_matches(runtime.keyboard_bindings.hotkeys.pause, keycode, scancode)
                    {
                        runtime.paused = !runtime.paused;
                        sync_audio_playback_state(machine, runtime)?;
                    }
                }
                apply_keyboard_event_to_player_slots(runtime, machine, keycode, scancode, true);
            }
            Event::KeyUp {
                keycode,
                scancode,
                repeat,
                ..
            } => {
                if repeat {
                    continue;
                }
                if key_event_matches(runtime.keyboard_bindings.hotkeys.rewind, keycode, scancode) {
                    runtime.rewind_hotkey_active = false;
                }
                if key_event_matches(
                    runtime.keyboard_bindings.hotkeys.fast_forward,
                    keycode,
                    scancode,
                ) {
                    runtime.fast_forward_hotkey_active = false;
                }
                apply_keyboard_event_to_player_slots(runtime, machine, keycode, scancode, false);
            }
            Event::ControllerButtonDown { which, button, .. }
                if runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                    manager.is_active_gamepad(gamepad_event_joystick_id(which))
                }) =>
            {
                let action = runtime
                    .gamepad_manager
                    .as_ref()
                    .map(GamepadManager::action_bindings)
                    .map(|bindings| gamepad_action_for_button(bindings, button))
                    .unwrap_or(HotkeyAction::None);
                match action {
                    HotkeyAction::SaveState => {
                        match save_machine_state_slot(session, machine, runtime.machine_state_slot)
                        {
                            Ok(path) => eprintln!("info: state saved to {}", path.display()),
                            Err(error) => {
                                show_warning_message(Some(canvas.window()), "Save State", &error);
                                eprintln!("warning: {error}");
                            }
                        }
                    }
                    HotkeyAction::LoadState => {
                        handle_load_machine_state_action(
                            session,
                            machine,
                            runtime,
                            performance_counter,
                            frame_pacer,
                            canvas,
                        )?;
                    }
                    HotkeyAction::Rewind => {
                        runtime.rewind_gamepad_active = true;
                    }
                    HotkeyAction::FastForward => {
                        runtime.fast_forward_gamepad_active = true;
                    }
                    HotkeyAction::None
                    | HotkeyAction::ManualSave
                    | HotkeyAction::SelectStateSlot(_)
                    | HotkeyAction::Reset
                    | HotkeyAction::ToggleFullscreen
                    | HotkeyAction::TogglePerformanceHud => {}
                }
            }
            Event::ControllerButtonUp { which, button, .. }
                if runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                    manager.is_active_gamepad(gamepad_event_joystick_id(which))
                }) =>
            {
                let action = runtime
                    .gamepad_manager
                    .as_ref()
                    .map(GamepadManager::action_bindings)
                    .map(|bindings| gamepad_action_for_button(bindings, button))
                    .unwrap_or(HotkeyAction::None);
                if matches!(action, HotkeyAction::Rewind) {
                    runtime.rewind_gamepad_active = false;
                }
                if matches!(action, HotkeyAction::FastForward) {
                    runtime.fast_forward_gamepad_active = false;
                }
            }
            Event::ControllerAxisMotion {
                which, axis, value, ..
            } if runtime.gamepad_manager.as_ref().is_some_and(|manager| {
                manager.is_active_gamepad(gamepad_event_joystick_id(which))
            }) =>
            {
                if let Some((binding, pressed)) =
                    gamepad_trigger_event_binding(&mut runtime.gamepad_trigger_state, axis, value)
                {
                    let action = runtime
                        .gamepad_manager
                        .as_ref()
                        .map(GamepadManager::action_bindings)
                        .map(|bindings| gamepad_action_for_binding(bindings, binding))
                        .unwrap_or(HotkeyAction::None);
                    let mut context = FrontendActionContext {
                        session,
                        machine,
                        runtime,
                        performance_counter,
                        frame_pacer,
                        settings_store,
                    };
                    apply_gamepad_action_event(
                        GamepadActionEvent { action, pressed },
                        canvas,
                        &mut context,
                    )?;
                }
            }
            _ => {}
        }
    }

    if runtime.menu_state.is_open() {
        sync_gamepad_rumble(runtime, machine, Instant::now())?;
        return Ok(LoopSignal::Continue);
    }

    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager.poll_active_gamepad_state(
            runtime.player_inputs.input_mut(PlayerSlot::P1),
            machine
                .machine_for_player_slot_mut(PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        );
    }
    sync_gamepad_rumble(runtime, machine, Instant::now())?;

    Ok(LoopSignal::Continue)
}

fn process_gamepad_manager_event(
    runtime: &mut FrontendRuntime,
    machine: &mut DesktopEmulationSession,
    event: &Event,
) -> Result<(), String> {
    let mut clear_gamepad_hold_latches_after_event = false;
    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        let active_gamepad_before_event = gamepad_manager.active_gamepad_joystick_id();
        gamepad_manager.handle_event(
            event,
            runtime.player_inputs.input_mut(PlayerSlot::P1),
            machine
                .machine_for_player_slot_mut(PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        )?;
        if active_gamepad_before_event != gamepad_manager.active_gamepad_joystick_id() {
            clear_gamepad_hold_latches_after_event = true;
        }
        let should_activate_from_input = match event {
            Event::ControllerButtonDown { .. } => true,
            Event::ControllerAxisMotion { axis, value, .. } => {
                gamepad_button_binding_from_sdl_axis(*axis).is_some()
                    && gamepad_trigger_axis_is_pressed(*value)
            }
            _ => false,
        };
        if should_activate_from_input {
            let input_joystick_id = match event {
                Event::ControllerButtonDown { which, .. }
                | Event::ControllerAxisMotion { which, .. } => {
                    Some(gamepad_event_joystick_id(*which))
                }
                _ => None,
            };
            if let Some(input_joystick_id) = input_joystick_id
                && gamepad_manager.activate_gamepad_from_input(
                    input_joystick_id,
                    runtime.player_inputs.input_mut(PlayerSlot::P1),
                    machine
                        .machine_for_player_slot_mut(PlayerSlot::P1)
                        .expect("P1 should always map to an active desktop machine"),
                )
            {
                clear_gamepad_hold_latches_after_event = true;
            }
        }
    }
    if clear_gamepad_hold_latches_after_event {
        clear_gamepad_hold_latches(runtime);
    }

    Ok(())
}

fn process_test_runner_events(
    event_pump: &mut sdl3::EventPump,
    context: &mut FrontendActionContext<'_>,
) -> Result<LoopSignal, String> {
    let events = event_pump.poll_iter().collect::<Vec<_>>();
    for event in events {
        if matches!(event, Event::Quit { .. }) {
            return Ok(LoopSignal::Quit);
        }
        process_gamepad_manager_event(context.runtime, context.machine, &event)?;
    }

    if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
        gamepad_manager.poll_active_gamepad_state(
            context.runtime.player_inputs.input_mut(PlayerSlot::P1),
            context
                .machine
                .machine_for_player_slot_mut(PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        );
    }

    Ok(LoopSignal::Continue)
}

fn step_fast_forward_frames(
    event_pump: &mut sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(StepUntilNextFrameResult, usize), String> {
    sync_fast_forward_audio_state(context.runtime, true)?;
    let frame_budget =
        fast_forward_frame_budget(context.session.config.fast_forward.speed_multiplier);
    let mut fast_forwarded_frames = 0usize;
    let mut final_result = StepUntilNextFrameResult {
        signal: LoopSignal::Continue,
        emulation_profile_request: None,
        frame_loop_telemetry: FrameLoopTelemetry::default(),
    };

    for _ in 0..frame_budget {
        final_result = step_until_next_frame(event_pump, canvas, context)?;
        if !matches!(final_result.signal, LoopSignal::Continue) {
            break;
        }
        fast_forwarded_frames = fast_forwarded_frames.saturating_add(1);
        if !fast_forward_active(context.runtime, context.session, context.machine) {
            break;
        }
    }

    Ok((final_result, fast_forwarded_frames))
}

fn fast_forward_frame_budget(speed_multiplier: u8) -> usize {
    usize::from(speed_multiplier.max(1))
}

fn should_skip_host_frame_pacing(
    test_runner: bool,
    fast_forward_active: bool,
    fast_forwarded_this_frame: bool,
) -> bool {
    test_runner || fast_forward_active || fast_forwarded_this_frame
}

fn step_until_next_frame(
    event_pump: &mut sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<StepUntilNextFrameResult, String> {
    let collect_frame_telemetry =
        !context.session.test_runner && context.performance_counter.emulation_profile_enabled();
    let frame_start_ly = context.machine.ppu().ly();
    let frame_start_dot = context.machine.ppu().line_dot();
    let mut current_scanline_ly = frame_start_ly;
    let mut current_scanline_t_cycles = 0usize;
    let mut at_frame_origin = frame_start_ly == 0 && frame_start_dot == 0;
    let mut previous_ly = frame_start_ly;
    let mut previous_dot = frame_start_dot;
    let mut previous_cpu_execution_state = context.machine.cpu().execution_state();
    let profile_this_frame =
        !context.session.test_runner && context.performance_counter.should_profile_next_frame();
    let mut profile_request = None::<EmulationProfileRequest>;
    let mut pending_event_poll_duration = Duration::ZERO;
    let mut stepped_t_cycles = 0usize;
    let mut video_dots = 0usize;
    let mut frame_origin_crossings = 0u8;
    let mut scanline_transitions = 0u16;
    let mut scanlines_over_456 = 0u16;
    let mut max_scanline_t_cycles = 0usize;
    let mut max_scanline_ly = frame_start_ly;
    let mut max_mode0_start_dot = context.machine.ppu().mode0_start_dot();
    let mut max_mode0_start_dot_ly = frame_start_ly;
    let mut ly_153_to_0_transitions = 0u8;
    let mut ly_153_to_0_startup_mode0 = 0u8;
    let mut ly_153_to_0_blank_frame = 0u8;
    let mut ly_0_self_wraps = 0u8;
    let mut ly_0_self_wrap_startup_mode0 = 0u8;
    let mut ly_0_self_wrap_blank_frame = 0u8;
    let mut ly_0_to_1_transitions = 0u8;
    let mut ly_0_scanline_t_cycles = 0usize;
    let mut ly_0_max_mode0_start_dot = if frame_start_ly == 0 {
        max_mode0_start_dot
    } else {
        0
    };
    let mut ly_0_stall_t_cycles = 0usize;
    let mut ly_0_stall_hblank_t_cycles = 0usize;
    let mut ly_0_stall_oam_t_cycles = 0usize;
    let mut ly_0_stall_drawing_t_cycles = 0usize;
    let mut ly_0_stall_startup_mode0_t_cycles = 0usize;
    let mut ly_0_stall_blank_frame_t_cycles = 0usize;
    let mut ly_0_stall_runs = 0u16;
    let mut ly_0_current_stall_run_t_cycles = 0usize;
    let mut ly_0_max_stall_run_t_cycles = 0usize;
    let mut ly_0_max_stall_dot = 0u16;
    let mut ly_0_max_stall_mode_dot = 0u16;
    let mut cpu_stop_t_cycles = 0usize;
    let mut cpu_zombie_stop_t_cycles = 0usize;
    let mut ly_0_cpu_stop_t_cycles = 0usize;
    let mut ly_0_cpu_zombie_stop_t_cycles = 0usize;
    let mut ly_0_stall_cpu_stop_t_cycles = 0usize;
    let mut ly_0_stall_cpu_zombie_stop_t_cycles = 0usize;
    let mut lcd_disabled_t_cycles = 0usize;
    let mut lcd_disable_transitions = 0u8;
    let mut lcd_enable_transitions = 0u8;
    let mut ly_0_lcd_disabled_t_cycles = 0usize;
    let mut ly_0_stall_lcd_disabled_t_cycles = 0usize;
    let mut previous_lcd_enabled = context.machine.ppu().lcd_state().is_enabled();

    loop {
        if !context.session.test_runner {
            let process_events_started_at = profile_this_frame.then(Instant::now);
            let loop_signal = process_events(event_pump, canvas, context)?;
            if let Some(process_events_started_at) = process_events_started_at {
                let duration = process_events_started_at.elapsed();
                if let Some(profile_request) = &mut profile_request {
                    profile_request.record_host_event_poll_duration(duration);
                } else {
                    pending_event_poll_duration += duration;
                }
            }
            match loop_signal {
                LoopSignal::Continue => {}
                LoopSignal::Quit => {
                    return Ok(StepUntilNextFrameResult {
                        signal: LoopSignal::Quit,
                        emulation_profile_request: None,
                        frame_loop_telemetry: FrameLoopTelemetry::default(),
                    });
                }
            }
        }
        if emulation_paused(context.machine, context.runtime) {
            return Ok(StepUntilNextFrameResult {
                signal: LoopSignal::Continue,
                emulation_profile_request: None,
                frame_loop_telemetry: FrameLoopTelemetry::default(),
            });
        }
        if profile_this_frame && profile_request.is_none() {
            let mut request = EmulationProfileRequest::new_with_detail(
                context.machine.clone(),
                context
                    .performance_counter
                    .emulation_profile_detail()
                    .expect("enabled emulation profile mode should expose a detail mode"),
            );
            request.record_host_event_poll_duration(pending_event_poll_duration);
            profile_request = Some(request);
            pending_event_poll_duration = Duration::ZERO;
        }

        context
            .runtime
            .rtc_sync
            .sync_host_elapsed_to_machine(context.machine);
        let tcycle_host_services = DesktopTcycleHostServices::from_runtime_state(
            context.session,
            context.machine,
            context.runtime,
        );
        for _ in 0..INPUT_POLL_SLICE_T_CYCLES {
            let scheduler_t_cycle = tcycle_host_services
                .capture_audio
                .then(|| context.machine.next_t_cycle().get());
            if let Some(benchmark) = &mut context.session.benchmark {
                let benchmark_t_cycle = context.machine.next_t_cycle().get();
                let completed_frames = context.performance_counter.presented_frames_total;
                benchmark.stimuli.apply_due(
                    benchmark_t_cycle,
                    completed_frames,
                    |button, pressed| {
                        context
                            .machine
                            .primary_machine_mut()
                            .set_joypad_button_pressed(button, pressed);
                    },
                );
            }
            context.machine.step_t_cycle();
            context
                .runtime
                .rtc_sync
                .tick_mbc3_for_emulated_t_cycle(context.machine);
            stepped_t_cycles += 1;
            if tcycle_host_services.drain_printer {
                drain_printed_pages_into_printer_output(
                    canvas.window(),
                    context.session,
                    context.runtime,
                    context.machine,
                );
            }
            if tcycle_host_services.traces.any() {
                let trace_machine = audio_source_machine(context.machine);
                if tcycle_host_services.traces.trace_capture {
                    context.runtime.trace_capture.record_t_cycle(trace_machine);
                }
                if tcycle_host_services.traces.watch_trace {
                    context.runtime.watch_trace.record_t_cycle(trace_machine);
                }
                if tcycle_host_services.traces.pc_watch_trace {
                    context.runtime.pc_watch_trace.record_t_cycle(trace_machine);
                }
                if tcycle_host_services.traces.edge_trace {
                    context.runtime.edge_trace.record_t_cycle(trace_machine);
                }
                if tcycle_host_services.traces.cgb_ir_trace {
                    context.runtime.cgb_ir_trace.record_t_cycle(context.machine);
                }
                if tcycle_host_services.traces.ch4_nr43_trace {
                    context.runtime.ch4_nr43_trace.record_t_cycle(trace_machine);
                }
                if tcycle_host_services.traces.ch4_startup_trace {
                    context
                        .runtime
                        .ch4_startup_trace
                        .record_t_cycle(trace_machine);
                }
                if tcycle_host_services.traces.cpu_window_trace {
                    context
                        .runtime
                        .cpu_window_trace
                        .record_t_cycle(trace_machine);
                }
            }

            let host_audio_capture_due = tcycle_host_services.capture_audio && {
                let audio_machine = audio_source_machine(context.machine);
                host_audio_capture_due_for_t_cycle(
                    audio_machine.speed().current_speed(),
                    scheduler_t_cycle.expect("audio capture should record the scheduler T-cycle"),
                    audio_machine.cpu().execution_state(),
                )
            };
            if host_audio_capture_due {
                if let Some(audio_output) = &mut context.runtime.audio_output {
                    audio_output.capture_t_cycle(audio_source_machine(context.machine).apu());
                }
                if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
                    audio_recorder.capture_t_cycle(audio_source_machine(context.machine).apu());
                }
            }

            if tcycle_host_services.sync_gamepad_rumble {
                sync_gamepad_rumble(context.runtime, context.machine, Instant::now())?;
            }

            let current_ly = context.machine.ppu().ly();
            let current_dot = context.machine.ppu().line_dot();
            let current_cpu_execution_state = context.machine.cpu().execution_state();
            let stop_forced_blank_present_requested = !matches!(
                previous_cpu_execution_state,
                CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
            ) && matches!(
                current_cpu_execution_state,
                CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
            );
            if tcycle_host_services.record_rewind {
                record_desktop_rewind_point_active(context.machine, context.runtime);
            }
            if collect_frame_telemetry {
                let current_mode0_start_dot = context.machine.ppu().mode0_start_dot();
                let current_access_mode = context.machine.ppu().access_mode();
                let current_mode_dot = context.machine.ppu().mode_dot();
                let startup_mode0_active = context.machine.ppu().is_startup_mode0_window_active();
                let blank_frame_active = context.machine.ppu().is_blank_frame_active();
                let current_lcd_enabled = context.machine.ppu().lcd_state().is_enabled();
                current_scanline_t_cycles += 1;
                if current_ly != previous_ly || current_dot != previous_dot {
                    video_dots = video_dots.saturating_add(1);
                }
                if !current_lcd_enabled {
                    lcd_disabled_t_cycles = lcd_disabled_t_cycles.saturating_add(1);
                    if current_ly == 0 {
                        ly_0_lcd_disabled_t_cycles = ly_0_lcd_disabled_t_cycles.saturating_add(1);
                    }
                }
                match (previous_lcd_enabled, current_lcd_enabled) {
                    (true, false) => {
                        lcd_disable_transitions = lcd_disable_transitions.saturating_add(1);
                    }
                    (false, true) => {
                        lcd_enable_transitions = lcd_enable_transitions.saturating_add(1);
                    }
                    _ => {}
                }
                match current_cpu_execution_state {
                    CpuExecutionState::Stopped => {
                        cpu_stop_t_cycles = cpu_stop_t_cycles.saturating_add(1);
                        if current_ly == 0 {
                            ly_0_cpu_stop_t_cycles = ly_0_cpu_stop_t_cycles.saturating_add(1);
                        }
                    }
                    CpuExecutionState::ZombieStopped => {
                        cpu_zombie_stop_t_cycles = cpu_zombie_stop_t_cycles.saturating_add(1);
                        if current_ly == 0 {
                            ly_0_cpu_zombie_stop_t_cycles =
                                ly_0_cpu_zombie_stop_t_cycles.saturating_add(1);
                        }
                    }
                    _ => {}
                }
                if current_mode0_start_dot > max_mode0_start_dot {
                    max_mode0_start_dot = current_mode0_start_dot;
                    max_mode0_start_dot_ly = current_ly;
                }
                if current_ly == 0 {
                    ly_0_max_mode0_start_dot =
                        ly_0_max_mode0_start_dot.max(current_mode0_start_dot);
                }
                if current_ly == 0 && current_ly == previous_ly && current_dot == previous_dot {
                    ly_0_stall_t_cycles = ly_0_stall_t_cycles.saturating_add(1);
                    match current_access_mode {
                        PpuAccessMode::HBlank => {
                            ly_0_stall_hblank_t_cycles =
                                ly_0_stall_hblank_t_cycles.saturating_add(1);
                        }
                        PpuAccessMode::OamScan => {
                            ly_0_stall_oam_t_cycles = ly_0_stall_oam_t_cycles.saturating_add(1);
                        }
                        PpuAccessMode::Drawing => {
                            ly_0_stall_drawing_t_cycles =
                                ly_0_stall_drawing_t_cycles.saturating_add(1);
                        }
                        PpuAccessMode::VBlank => {}
                    }
                    if startup_mode0_active {
                        ly_0_stall_startup_mode0_t_cycles =
                            ly_0_stall_startup_mode0_t_cycles.saturating_add(1);
                    }
                    if blank_frame_active {
                        ly_0_stall_blank_frame_t_cycles =
                            ly_0_stall_blank_frame_t_cycles.saturating_add(1);
                    }
                    if !current_lcd_enabled {
                        ly_0_stall_lcd_disabled_t_cycles =
                            ly_0_stall_lcd_disabled_t_cycles.saturating_add(1);
                    }
                    match current_cpu_execution_state {
                        CpuExecutionState::Stopped => {
                            ly_0_stall_cpu_stop_t_cycles =
                                ly_0_stall_cpu_stop_t_cycles.saturating_add(1);
                        }
                        CpuExecutionState::ZombieStopped => {
                            ly_0_stall_cpu_zombie_stop_t_cycles =
                                ly_0_stall_cpu_zombie_stop_t_cycles.saturating_add(1);
                        }
                        _ => {}
                    }
                    if ly_0_current_stall_run_t_cycles == 0 {
                        ly_0_stall_runs = ly_0_stall_runs.saturating_add(1);
                    }
                    ly_0_current_stall_run_t_cycles =
                        ly_0_current_stall_run_t_cycles.saturating_add(1);
                    if ly_0_current_stall_run_t_cycles > ly_0_max_stall_run_t_cycles {
                        ly_0_max_stall_run_t_cycles = ly_0_current_stall_run_t_cycles;
                        ly_0_max_stall_dot = current_dot;
                        ly_0_max_stall_mode_dot = current_mode_dot;
                    }
                } else {
                    ly_0_current_stall_run_t_cycles = 0;
                }
                if current_dot == 0 && previous_dot != 0 {
                    match (previous_ly, current_ly) {
                        (153, 0) => {
                            ly_153_to_0_transitions = ly_153_to_0_transitions.saturating_add(1);
                            if startup_mode0_active {
                                ly_153_to_0_startup_mode0 =
                                    ly_153_to_0_startup_mode0.saturating_add(1);
                            }
                            if blank_frame_active {
                                ly_153_to_0_blank_frame = ly_153_to_0_blank_frame.saturating_add(1);
                            }
                        }
                        (0, 0) => {
                            ly_0_self_wraps = ly_0_self_wraps.saturating_add(1);
                            if startup_mode0_active {
                                ly_0_self_wrap_startup_mode0 =
                                    ly_0_self_wrap_startup_mode0.saturating_add(1);
                            }
                            if blank_frame_active {
                                ly_0_self_wrap_blank_frame =
                                    ly_0_self_wrap_blank_frame.saturating_add(1);
                            }
                        }
                        (0, 1) => {
                            ly_0_to_1_transitions = ly_0_to_1_transitions.saturating_add(1);
                            ly_0_scanline_t_cycles = current_scanline_t_cycles;
                        }
                        _ => {}
                    }
                }
                if current_dot == 0 && current_ly != current_scanline_ly {
                    scanline_transitions = scanline_transitions.saturating_add(1);
                    if current_scanline_t_cycles > max_scanline_t_cycles {
                        max_scanline_t_cycles = current_scanline_t_cycles;
                        max_scanline_ly = current_scanline_ly;
                    }
                    if current_scanline_t_cycles > EXPECTED_SCANLINE_T_CYCLES {
                        scanlines_over_456 = scanlines_over_456.saturating_add(1);
                    }
                    current_scanline_ly = current_ly;
                    current_scanline_t_cycles = 0;
                }
                previous_ly = current_ly;
                previous_dot = current_dot;
                previous_lcd_enabled = current_lcd_enabled;
            }

            let now_at_frame_origin = current_ly == 0 && current_dot == 0;
            let frame_boundary_reached = now_at_frame_origin && !at_frame_origin;
            let benchmark_tcycle_limit_reached = should_exit_after_benchmark_tcycles(
                context.session.benchmark.as_ref(),
                context.machine,
            );
            // STOP forces the core framebuffer into the visible blank state and can
            // also freeze PPU frame-origin progress. Return once for presentation
            // so the SDL texture does not keep showing the last diagnostic frame.
            if frame_boundary_reached
                || stop_forced_blank_present_requested
                || benchmark_tcycle_limit_reached
            {
                if collect_frame_telemetry && frame_boundary_reached {
                    frame_origin_crossings = frame_origin_crossings.saturating_add(1);
                }
                if frame_boundary_reached {
                    if context.runtime.audio_output.is_some()
                        || context.runtime.audio_recorder.is_some()
                    {
                        let audio_submit_started_at =
                            profile_request.as_ref().map(|_| Instant::now());
                        if let Some(audio_output) = &mut context.runtime.audio_output {
                            audio_output.submit_captured_samples()?;
                        }
                        if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
                            audio_recorder.write_captured_samples()?;
                        }
                        if let Some(audio_submit_started_at) = audio_submit_started_at
                            && let Some(profile_request) = &mut profile_request
                        {
                            profile_request.record_host_audio_submit_duration(
                                audio_submit_started_at.elapsed(),
                            );
                        }
                    }
                    let save_flush_started_at = profile_request.as_ref().map(|_| Instant::now());
                    maybe_flush_runtime_save_sessions_at_frame_boundary(
                        context.runtime,
                        context.machine,
                        Instant::now(),
                    )?;
                    if let Some(save_flush_started_at) = save_flush_started_at
                        && let Some(profile_request) = &mut profile_request
                    {
                        profile_request
                            .record_host_save_flush_duration(save_flush_started_at.elapsed());
                    }
                }
                return Ok(StepUntilNextFrameResult {
                    signal: LoopSignal::Continue,
                    emulation_profile_request: profile_request,
                    frame_loop_telemetry: if collect_frame_telemetry {
                        FrameLoopTelemetry {
                            speed_mode: Some(context.machine.speed().current_speed()),
                            start_ly: frame_start_ly,
                            start_dot: frame_start_dot,
                            end_ly: current_ly,
                            end_dot: current_dot,
                            stepped_t_cycles,
                            video_dots,
                            frame_origin_crossings,
                            scanline_transitions,
                            scanlines_over_456,
                            max_scanline_t_cycles,
                            max_scanline_ly,
                            max_mode0_start_dot,
                            max_mode0_start_dot_ly,
                            ly_153_to_0_transitions,
                            ly_153_to_0_startup_mode0,
                            ly_153_to_0_blank_frame,
                            ly_0_self_wraps,
                            ly_0_self_wrap_startup_mode0,
                            ly_0_self_wrap_blank_frame,
                            ly_0_to_1_transitions,
                            ly_0_scanline_t_cycles,
                            ly_0_max_mode0_start_dot,
                            ly_0_stall_t_cycles,
                            ly_0_stall_hblank_t_cycles,
                            ly_0_stall_oam_t_cycles,
                            ly_0_stall_drawing_t_cycles,
                            ly_0_stall_startup_mode0_t_cycles,
                            ly_0_stall_blank_frame_t_cycles,
                            ly_0_stall_runs,
                            ly_0_max_stall_run_t_cycles,
                            ly_0_max_stall_dot,
                            ly_0_max_stall_mode_dot,
                            cpu_stop_t_cycles,
                            cpu_zombie_stop_t_cycles,
                            ly_0_cpu_stop_t_cycles,
                            ly_0_cpu_zombie_stop_t_cycles,
                            ly_0_stall_cpu_stop_t_cycles,
                            ly_0_stall_cpu_zombie_stop_t_cycles,
                            lcd_disabled_t_cycles,
                            lcd_disable_transitions,
                            lcd_enable_transitions,
                            ly_0_lcd_disabled_t_cycles,
                            ly_0_stall_lcd_disabled_t_cycles,
                        }
                    } else {
                        FrameLoopTelemetry::default()
                    },
                });
            }
            at_frame_origin = now_at_frame_origin;
            previous_cpu_execution_state = current_cpu_execution_state;
        }
    }
}
