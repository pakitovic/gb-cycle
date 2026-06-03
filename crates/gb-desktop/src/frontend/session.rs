fn emulation_paused(machine: &Machine<TraceSummaryBuffer>, runtime: &FrontendRuntime) -> bool {
    machine.cartridge().is_empty() || runtime.paused || runtime.menu_state.is_open()
}

fn player_session_kind(machine: &DesktopEmulationSession) -> DesktopPlayerSessionKind {
    if let Some(player_count) = machine.dmg07_player_count() {
        return DesktopPlayerSessionKind::LinkedDmg07 { player_count };
    }
    if machine.is_linked_cgb_infrared_two_player() {
        return DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer;
    }
    if machine.is_linked_dmg04_two_player() {
        return DesktopPlayerSessionKind::LinkedDmg04TwoPlayer;
    }
    DesktopPlayerSessionKind::Single
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlayerInputRoute {
    keyboard_profile: PlayerKeyboardProfile,
    target: FrontendJoypadTarget,
}

fn input_route_for_player_slot(
    machine: &DesktopEmulationSession,
    session_kind: DesktopPlayerSessionKind,
    slot: PlayerSlot,
) -> Option<PlayerInputRoute> {
    let policy = host_policy_for_slot(session_kind, slot);
    if policy.keyboard_profile != PlayerKeyboardProfile::Disabled {
        return Some(PlayerInputRoute {
            keyboard_profile: policy.keyboard_profile,
            target: FrontendJoypadTarget::Local,
        });
    }

    sgb_input_route_for_player_slot(machine, slot)
}

fn sgb_input_route_for_player_slot(
    machine: &DesktopEmulationSession,
    slot: PlayerSlot,
) -> Option<PlayerInputRoute> {
    let DesktopEmulationSession::Single(primary) = machine else {
        return None;
    };
    if !primary.config().host_platform.is_sgb() {
        return None;
    }

    let keyboard_profile = match slot {
        PlayerSlot::P1 => return None,
        PlayerSlot::P2 => PlayerKeyboardProfile::LinkedDmg04P2,
        PlayerSlot::P3 => PlayerKeyboardProfile::LinkedDmg07P3,
        PlayerSlot::P4 => PlayerKeyboardProfile::LinkedDmg07P4,
    };

    Some(PlayerInputRoute {
        keyboard_profile,
        target: FrontendJoypadTarget::SgbPlayer((slot.index() + 1) as u8),
    })
}

fn machine_for_player_input_route_mut(
    machine: &mut DesktopEmulationSession,
    slot: PlayerSlot,
    route: PlayerInputRoute,
) -> Option<&mut Machine<TraceSummaryBuffer>> {
    match route.target {
        FrontendJoypadTarget::Local => machine.machine_for_player_slot_mut(slot),
        FrontendJoypadTarget::SgbPlayer(_) => Some(machine.primary_machine_mut()),
    }
}

fn audio_source_machine(machine: &DesktopEmulationSession) -> &Machine<TraceSummaryBuffer> {
    let slot = audio_source_slot(player_session_kind(machine));
    machine
        .machine_for_player_slot(slot)
        .expect("desktop audio source slot should map to an active machine")
}

fn apply_machine_settings_change(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
    title: &str,
    update: impl FnOnce(&mut DesktopConfig),
) -> Result<(), String> {
    let previous_config = context.session.config.clone();
    let mut next_config = previous_config.clone();
    update(&mut next_config);
    if next_config == previous_config {
        return Ok(());
    }

    let effective_config = match rebuild_machine_for_config(canvas, context, &next_config) {
        Ok(effective_config) => effective_config,
        Err(error) => {
            show_warning_message(Some(canvas.window()), title, &error);
            eprintln!("warning: {error}");
            return Ok(());
        }
    };

    context.session.config = effective_config;
    sanitize_external_port_session_for_model(context.session);
    if !context.session.test_runner {
        context
            .settings_store
            .persist_machine_preferences(&context.session.config)?;
    }
    Ok(())
}

fn apply_execution_mode_cycle_change(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let previous_mode = context.session.config.launch.execution_mode;
    let mut candidate_mode = next_execution_mode(previous_mode);
    let mut skipped_errors = Vec::new();

    for _ in 0..2 {
        let mut candidate_config = context.session.config.clone();
        candidate_config.launch.execution_mode = candidate_mode;

        match check_current_session_rebuilds_with_config(context.session, &candidate_config) {
            Ok(()) => {
                for (skipped_mode, error) in skipped_errors {
                    eprintln!(
                        "warning: skipping execution mode {} for current session: {error}",
                        execution_mode_name(skipped_mode)
                    );
                }
                return apply_machine_settings_change(
                    canvas,
                    context,
                    "Execution mode",
                    |config| {
                        config.launch.execution_mode = candidate_mode;
                    },
                );
            }
            Err(error) => {
                skipped_errors.push((candidate_mode, error));
                candidate_mode = next_execution_mode(candidate_mode);
            }
        }
    }

    let message = skipped_errors
        .into_iter()
        .map(|(mode, error)| format!("{}: {error}", execution_mode_name(mode)))
        .collect::<Vec<_>>()
        .join("\n\n");
    let message =
        format!("No alternate execution mode can reload the current session.\n\n{message}");
    show_warning_message(Some(canvas.window()), "Execution mode", &message);
    eprintln!("warning: {message}");
    Ok(())
}

fn check_current_session_rebuilds_with_config(
    session: &DesktopSession,
    config: &DesktopConfig,
) -> Result<(), String> {
    match (
        session.rom_bytes(),
        session.linked_secondary_rom_bytes(),
        session.cgb_infrared_link_active,
        session.pokemon_pikachu_color_active,
        session.pokemon_mystery_gift_active,
        session.external_port_selection,
        session.dmg07_player_count,
    ) {
        (Some(primary_rom_bytes), Some(secondary_rom_bytes), true, _, _, _, _) => {
            load_cgb_infrared_machines_for_roms(
                config,
                &session.current_dir,
                primary_rom_bytes,
                secondary_rom_bytes,
                "checking a CGB IR session",
            )
            .map(|_| ())
        }
        (Some(primary_rom_bytes), _, false, true, _, _, _) => {
            load_machine_for_rom(config, &session.current_dir, primary_rom_bytes).map(|_| ())
        }
        (Some(primary_rom_bytes), _, false, false, true, _, _) => {
            load_machine_for_rom(config, &session.current_dir, primary_rom_bytes).map(|_| ())
        }
        (
            Some(primary_rom_bytes),
            Some(secondary_rom_bytes),
            false,
            false,
            false,
            DesktopExternalPortSelection::GameLink,
            _,
        ) => {
            let primary_loaded =
                load_machine_for_rom(config, &session.current_dir, primary_rom_bytes)?;
            let secondary_loaded =
                load_machine_for_rom(config, &session.current_dir, secondary_rom_bytes)?;
            if primary_loaded.effective_config != secondary_loaded.effective_config {
                return Err(
                    "checking a linked DMG-04 session produced divergent effective configs between the primary and secondary machines"
                        .to_string(),
                );
            }
            Ok(())
        }
        (
            Some(primary_rom_bytes),
            _,
            false,
            false,
            false,
            DesktopExternalPortSelection::FourPlayerAdapter,
            Some(player_count),
        ) => load_dmg07_machines_for_rom(
            config,
            &session.current_dir,
            primary_rom_bytes,
            player_count,
            "checking a DMG-07 session",
        )
        .map(|_| ()),
        (Some(rom_bytes), _, _, _, _, _, _) => {
            load_machine_for_rom(config, &session.current_dir, rom_bytes).map(|_| ())
        }
        (None, _, _, _, _, _, _) => {
            prepare_machine_config(config, &session.current_dir).map(|_| ())
        }
    }
}

fn rebuild_pokemon_mystery_gift_for_config(
    next_config: &DesktopConfig,
    next_session: &DesktopSession,
    primary_rom_bytes: &[u8],
    battery_backed_states: &[Option<PersistentCartState>; PLAYER_SLOT_COUNT],
    mut boot_rom_fallback_warnings: Vec<String>,
) -> Result<RebuildMachineResult, String> {
    let loaded = load_machine_for_rom(next_config, &next_session.current_dir, primary_rom_bytes)?;
    write_cartridge_diagnostics(&loaded.diagnostics);
    if let Some(warning) = loaded.boot_rom_fallback_warning {
        boot_rom_fallback_warnings.push(warning);
    }

    let mut next_machine = DesktopEmulationSession::new_pokemon_mystery_gift(
        loaded.machine,
        next_session.pokemon_mystery_gift_kind,
        next_session.pokemon_mystery_gift_code,
    );
    restore_battery_backed_states_by_player_slot(
        &mut next_machine,
        battery_backed_states,
        "after reconfigure",
    )?;
    apply_session_pocket_camera_frame_to_desktop_session(next_session, &mut next_machine)?;

    let effective_config = loaded.effective_config;
    let next_save_sessions = open_save_sessions_for_session(
        &DesktopSession {
            config: effective_config.clone(),
            linked_secondary_rom: None,
            external_port_selection: DesktopExternalPortSelection::None,
            dmg07_player_count: None,
            cgb_infrared_link_active: false,
            pokemon_pikachu_color_active: false,
            pokemon_mystery_gift_active: true,
            ..next_session.clone()
        },
        &mut next_machine,
    )?;
    Ok((
        effective_config,
        boot_rom_fallback_warnings,
        next_machine,
        next_save_sessions,
    ))
}

fn rebuild_machine_for_config(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
    next_config: &DesktopConfig,
) -> Result<DesktopConfig, String> {
    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let battery_backed_states = battery_backed_states_by_player_slot(context.machine);

    let mut previous_save_sessions =
        std::mem::replace(&mut context.runtime.save_sessions, empty_save_sessions());
    for slot in PlayerSlot::ALL {
        if let Some(save_session) = previous_save_sessions[slot.index()].as_mut()
            && let Some(slot_machine) = context.machine.machine_for_player_slot(slot)
            && let Err(error) = save_session.close(slot_machine)
        {
            context.runtime.save_sessions = previous_save_sessions;
            return Err(error);
        }
    }

    let rebuild_result: Result<RebuildMachineResult, String> = (|| {
        let mut boot_rom_fallback_warnings = Vec::new();

        let next_external_port_selection = supported_external_port_selection_for_model(
            next_config.launch.console_model,
            context.session.external_port_selection,
        );
        let next_linked_secondary_rom = if context.session.cgb_infrared_link_active
            || next_external_port_selection == DesktopExternalPortSelection::GameLink
        {
            context.session.linked_secondary_rom.clone()
        } else {
            None
        };
        let next_dmg07_player_count =
            if next_external_port_selection == DesktopExternalPortSelection::FourPlayerAdapter {
                context.session.dmg07_player_count
            } else {
                None
            };

        let next_session = DesktopSession {
            config: next_config.clone(),
            test_runner: context.session.test_runner,
            benchmark: context.session.benchmark.clone(),
            current_dir: context.session.current_dir.clone(),
            loaded_rom: context.session.loaded_rom.clone(),
            linked_secondary_rom: next_linked_secondary_rom,
            dmg07_player_count: next_dmg07_player_count,
            cgb_infrared_link_active: context.session.cgb_infrared_link_active,
            pokemon_pikachu_color_active: context.session.pokemon_pikachu_color_active,
            pokemon_pikachu_color_gift: context.session.pokemon_pikachu_color_gift,
            pokemon_mystery_gift_active: context.session.pokemon_mystery_gift_active,
            pokemon_mystery_gift_kind: context.session.pokemon_mystery_gift_kind,
            pokemon_mystery_gift_code: context.session.pokemon_mystery_gift_code,
            last_open_directory: context.session.last_open_directory.clone(),
            recent_roms: context.session.recent_roms.clone(),
            pocket_camera_frame: context.session.pocket_camera_frame.clone(),
            external_port_selection: next_external_port_selection,
        };

        match (
            next_session.rom_bytes(),
            next_session.linked_secondary_rom_bytes(),
            next_session.cgb_infrared_link_active,
            next_session.pokemon_pikachu_color_active,
            next_session.pokemon_mystery_gift_active,
            next_session.external_port_selection,
            next_session.dmg07_player_count,
        ) {
            (Some(primary_rom_bytes), Some(secondary_rom_bytes), true, _, _, _, _) => {
                let loaded = load_cgb_infrared_machines_for_roms(
                    next_config,
                    &context.session.current_dir,
                    primary_rom_bytes,
                    secondary_rom_bytes,
                    "reconfiguring a CGB IR session",
                )?;
                write_cartridge_diagnostics(&loaded.diagnostics);
                boot_rom_fallback_warnings.extend(loaded.boot_rom_fallback_warnings);
                let mut next_machine = loaded.machine;
                restore_battery_backed_states_by_player_slot(
                    &mut next_machine,
                    &battery_backed_states,
                    "after reconfigure",
                )?;
                apply_session_pocket_camera_frame_to_desktop_session(
                    &next_session,
                    &mut next_machine,
                )?;

                let effective_config = loaded.effective_config;
                let next_save_sessions = open_save_sessions_for_session(
                    &DesktopSession {
                        config: effective_config.clone(),
                        external_port_selection: DesktopExternalPortSelection::None,
                        dmg07_player_count: None,
                        cgb_infrared_link_active: true,
                        pokemon_pikachu_color_active: false,
                        pokemon_mystery_gift_active: false,
                        ..next_session
                    },
                    &mut next_machine,
                )?;
                Ok((
                    effective_config,
                    boot_rom_fallback_warnings,
                    next_machine,
                    next_save_sessions,
                ))
            }
            (Some(primary_rom_bytes), _, false, true, _, _, _) => {
                let loaded = load_machine_for_rom(
                    next_config,
                    &context.session.current_dir,
                    primary_rom_bytes,
                )?;
                write_cartridge_diagnostics(&loaded.diagnostics);
                if let Some(warning) = loaded.boot_rom_fallback_warning {
                    boot_rom_fallback_warnings.push(warning);
                }

                let mut next_machine = DesktopEmulationSession::new_pokemon_pikachu_color(
                    loaded.machine,
                    next_session.pokemon_pikachu_color_gift,
                    PokemonPikachuColorRegion::Auto,
                );
                restore_battery_backed_states_by_player_slot(
                    &mut next_machine,
                    &battery_backed_states,
                    "after reconfigure",
                )?;
                apply_session_pocket_camera_frame_to_desktop_session(
                    &next_session,
                    &mut next_machine,
                )?;

                let effective_config = loaded.effective_config;
                let next_save_sessions = open_save_sessions_for_session(
                    &DesktopSession {
                        config: effective_config.clone(),
                        linked_secondary_rom: None,
                        external_port_selection: DesktopExternalPortSelection::None,
                        dmg07_player_count: None,
                        cgb_infrared_link_active: false,
                        pokemon_pikachu_color_active: true,
                        pokemon_mystery_gift_active: false,
                        ..next_session
                    },
                    &mut next_machine,
                )?;
                Ok((
                    effective_config,
                    boot_rom_fallback_warnings,
                    next_machine,
                    next_save_sessions,
                ))
            }
            (Some(primary_rom_bytes), _, false, false, true, _, _) => {
                rebuild_pokemon_mystery_gift_for_config(
                    next_config,
                    &next_session,
                    primary_rom_bytes,
                    &battery_backed_states,
                    boot_rom_fallback_warnings,
                )
            }
            (
                Some(primary_rom_bytes),
                Some(secondary_rom_bytes),
                false,
                false,
                false,
                DesktopExternalPortSelection::GameLink,
                _,
            ) => {
                let primary_loaded = load_machine_for_rom(
                    next_config,
                    &context.session.current_dir,
                    primary_rom_bytes,
                )?;
                let secondary_loaded = load_machine_for_rom(
                    next_config,
                    &context.session.current_dir,
                    secondary_rom_bytes,
                )?;
                if primary_loaded.effective_config != secondary_loaded.effective_config {
                    return Err(
                        "reconfiguring a linked DMG-04 session produced divergent effective configs between primary and secondary machines"
                            .to_string(),
                    );
                }

                write_cartridge_diagnostics(&primary_loaded.diagnostics);
                write_cartridge_diagnostics(&secondary_loaded.diagnostics);
                if let Some(warning) = primary_loaded.boot_rom_fallback_warning {
                    boot_rom_fallback_warnings.push(warning);
                }
                if let Some(warning) = secondary_loaded.boot_rom_fallback_warning {
                    boot_rom_fallback_warnings.push(warning);
                }

                let mut next_machine = DesktopEmulationSession::new_single(primary_loaded.machine);
                next_machine.attach_secondary_dmg04(secondary_loaded.machine)?;
                restore_battery_backed_states_by_player_slot(
                    &mut next_machine,
                    &battery_backed_states,
                    "after reconfigure",
                )?;
                apply_session_pocket_camera_frame_to_desktop_session(
                    &next_session,
                    &mut next_machine,
                )?;

                let effective_config = primary_loaded.effective_config;
                let next_save_sessions = open_save_sessions_for_session(
                    &DesktopSession {
                        config: effective_config.clone(),
                        cgb_infrared_link_active: false,
                        pokemon_pikachu_color_active: false,
                        pokemon_mystery_gift_active: false,
                        ..next_session
                    },
                    &mut next_machine,
                )?;
                Ok((
                    effective_config,
                    boot_rom_fallback_warnings,
                    next_machine,
                    next_save_sessions,
                ))
            }
            (
                Some(primary_rom_bytes),
                _,
                false,
                false,
                false,
                DesktopExternalPortSelection::FourPlayerAdapter,
                Some(player_count),
            ) => {
                let loaded = load_dmg07_machines_for_rom(
                    next_config,
                    &context.session.current_dir,
                    primary_rom_bytes,
                    player_count,
                    "reconfiguring a DMG-07 session",
                )?;
                write_cartridge_diagnostics(&loaded.diagnostics);
                boot_rom_fallback_warnings.extend(loaded.boot_rom_fallback_warnings);
                let mut next_machine = loaded.machine;
                restore_battery_backed_states_by_player_slot(
                    &mut next_machine,
                    &battery_backed_states,
                    "after reconfigure",
                )?;
                apply_session_pocket_camera_frame_to_desktop_session(
                    &next_session,
                    &mut next_machine,
                )?;

                let effective_config = loaded.effective_config;
                let next_save_sessions = open_save_sessions_for_session(
                    &DesktopSession {
                        config: effective_config.clone(),
                        linked_secondary_rom: None,
                        external_port_selection: DesktopExternalPortSelection::FourPlayerAdapter,
                        dmg07_player_count: Some(player_count),
                        cgb_infrared_link_active: false,
                        pokemon_pikachu_color_active: false,
                        pokemon_mystery_gift_active: false,
                        ..next_session
                    },
                    &mut next_machine,
                )?;
                Ok((
                    effective_config,
                    boot_rom_fallback_warnings,
                    next_machine,
                    next_save_sessions,
                ))
            }
            (Some(rom_bytes), _, _, _, _, _, _) => {
                let loaded =
                    load_machine_for_rom(next_config, &context.session.current_dir, rom_bytes)?;
                write_cartridge_diagnostics(&loaded.diagnostics);
                if let Some(warning) = loaded.boot_rom_fallback_warning {
                    boot_rom_fallback_warnings.push(warning);
                }
                let mut next_machine = DesktopEmulationSession::new_single(loaded.machine);
                apply_external_port_selection_to_machine(
                    next_machine.primary_machine_mut(),
                    next_session.external_port_selection,
                );
                restore_battery_backed_states_by_player_slot(
                    &mut next_machine,
                    &battery_backed_states,
                    "after reconfigure",
                )?;
                apply_session_pocket_camera_frame_to_desktop_session(
                    &next_session,
                    &mut next_machine,
                )?;

                let effective_config = loaded.effective_config;
                let next_save_sessions = open_save_sessions_for_session(
                    &DesktopSession {
                        config: effective_config.clone(),
                        linked_secondary_rom: None,
                        dmg07_player_count: None,
                        cgb_infrared_link_active: false,
                        pokemon_pikachu_color_active: false,
                        pokemon_mystery_gift_active: false,
                        external_port_selection: next_session.external_port_selection,
                        ..next_session
                    },
                    &mut next_machine,
                )?;
                Ok((
                    effective_config,
                    boot_rom_fallback_warnings,
                    next_machine,
                    next_save_sessions,
                ))
            }
            (None, _, _, _, _, _, _) => {
                let prepared = prepare_machine_config(next_config, &context.session.current_dir)?;
                if let Some(warning) = prepared.boot_rom_fallback_warning {
                    boot_rom_fallback_warnings.push(warning);
                }

                let mut next_machine = DesktopEmulationSession::new_single(Machine::new_summary(
                    prepared.machine_config,
                ));
                apply_external_port_selection_to_machine(
                    next_machine.primary_machine_mut(),
                    next_session.external_port_selection,
                );
                Ok((
                    prepared.effective_config,
                    boot_rom_fallback_warnings,
                    next_machine,
                    empty_save_sessions(),
                ))
            }
        }
    })();

    let (effective_config, boot_rom_fallback_warnings, next_machine, next_save_sessions) =
        match rebuild_result {
            Ok(value) => value,
            Err(error) => {
                context.runtime.save_sessions = previous_save_sessions;
                return Err(error);
            }
        };

    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.clear_buffer()?;
    }

    for warning in &boot_rom_fallback_warnings {
        log_boot_rom_fallback_warning(Some(warning));
    }
    clear_live_input_state(context.machine, context.runtime);
    *context.machine = next_machine;
    reset_frontend_timeline_state(context.runtime);
    context.runtime.save_sessions = next_save_sessions;
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &effective_config),
    )?;
    Ok(effective_config)
}
