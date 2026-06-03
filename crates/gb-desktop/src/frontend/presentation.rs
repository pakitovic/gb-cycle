fn reset_pokemon_mystery_gift_session(
    session: &DesktopSession,
    rom_bytes: &[u8],
    battery_backed_states: &[Option<PersistentCartState>; PLAYER_SLOT_COUNT],
) -> Result<RebuildMachineResult, String> {
    let loaded = match load_machine_for_rom(&session.config, &session.current_dir, rom_bytes) {
        Ok(result) => result,
        Err(error) => {
            return Err(format_display_error(
                "failed to reload cartridge during Pokemon Mystery Gift reset",
                &error,
            ));
        }
    };
    let mut boot_rom_fallback_warnings = Vec::new();
    if let Some(warning) = loaded.boot_rom_fallback_warning {
        boot_rom_fallback_warnings.push(warning);
    }
    write_cartridge_diagnostics(&loaded.diagnostics);
    let mut reset_machine = DesktopEmulationSession::new_pokemon_mystery_gift(
        loaded.machine,
        session.pokemon_mystery_gift_kind,
        session.pokemon_mystery_gift_code,
    );
    restore_battery_backed_states_by_player_slot(
        &mut reset_machine,
        battery_backed_states,
        "after reset",
    )?;
    apply_session_pocket_camera_frame_to_desktop_session(session, &mut reset_machine)?;

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
            ..session.clone()
        },
        &mut reset_machine,
    )?;
    Ok((
        effective_config,
        boot_rom_fallback_warnings,
        reset_machine,
        next_save_sessions,
    ))
}

fn reset_machine(
    main_window: &Window,
    session: &mut DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    settings_store: &mut DesktopSettingsStore,
) -> Result<(), String> {
    let Some(rom_bytes) = session.rom_bytes() else {
        return Ok(());
    };
    drain_printed_pages_into_printer_output(main_window, session, runtime, machine);
    flush_pending_printer_output(main_window, session, runtime);
    runtime.rtc_sync.apply_to_machine(machine);
    let battery_backed_states = battery_backed_states_by_player_slot(machine);

    close_runtime_save_sessions(runtime, machine)?;

    let (effective_config, boot_rom_fallback_warnings, reset_machine, next_save_sessions) = match (
        session.linked_secondary_rom_bytes(),
        session.cgb_infrared_link_active,
        session.pokemon_pikachu_color_active,
        session.pokemon_mystery_gift_active,
        session.external_port_selection,
        session.dmg07_player_count,
    ) {
        (Some(secondary_rom_bytes), true, _, _, _, _) => {
            let loaded = load_cgb_infrared_machines_for_roms(
                &session.config,
                &session.current_dir,
                rom_bytes,
                secondary_rom_bytes,
                "resetting a CGB IR session",
            )?;
            let boot_rom_fallback_warnings = loaded.boot_rom_fallback_warnings;
            write_cartridge_diagnostics(&loaded.diagnostics);
            let mut reset_machine = loaded.machine;
            restore_battery_backed_states_by_player_slot(
                &mut reset_machine,
                &battery_backed_states,
                "after reset",
            )?;
            apply_session_pocket_camera_frame_to_desktop_session(session, &mut reset_machine)?;

            let effective_config = loaded.effective_config;
            let next_save_sessions = open_save_sessions_for_session(
                &DesktopSession {
                    config: effective_config.clone(),
                    external_port_selection: DesktopExternalPortSelection::None,
                    dmg07_player_count: None,
                    cgb_infrared_link_active: true,
                    pokemon_pikachu_color_active: false,
                    pokemon_mystery_gift_active: false,
                    ..session.clone()
                },
                &mut reset_machine,
            )?;
            (
                effective_config,
                boot_rom_fallback_warnings,
                reset_machine,
                next_save_sessions,
            )
        }
        (
            Some(secondary_rom_bytes),
            false,
            false,
            false,
            DesktopExternalPortSelection::GameLink,
            _,
        ) => {
            let primary_loaded =
                match load_machine_for_rom(&session.config, &session.current_dir, rom_bytes) {
                    Ok(result) => result,
                    Err(error) => {
                        return Err(format_display_error(
                            "failed to reload primary cartridge during linked reset",
                            &error,
                        ));
                    }
                };
            let secondary_loaded = match load_machine_for_rom(
                &session.config,
                &session.current_dir,
                secondary_rom_bytes,
            ) {
                Ok(result) => result,
                Err(error) => {
                    return Err(format_display_error(
                        "failed to reload secondary cartridge during linked reset",
                        &error,
                    ));
                }
            };
            if primary_loaded.effective_config != secondary_loaded.effective_config {
                return Err(
                    "linked reset produced divergent effective configs between the primary and secondary machines"
                        .to_string(),
                );
            }

            let mut boot_rom_fallback_warnings = Vec::new();
            if let Some(warning) = primary_loaded.boot_rom_fallback_warning {
                boot_rom_fallback_warnings.push(warning);
            }
            if let Some(warning) = secondary_loaded.boot_rom_fallback_warning {
                boot_rom_fallback_warnings.push(warning);
            }
            write_cartridge_diagnostics(&primary_loaded.diagnostics);
            write_cartridge_diagnostics(&secondary_loaded.diagnostics);

            let mut reset_machine = DesktopEmulationSession::new_single(primary_loaded.machine);
            reset_machine.attach_secondary_dmg04(secondary_loaded.machine)?;
            restore_battery_backed_states_by_player_slot(
                &mut reset_machine,
                &battery_backed_states,
                "after reset",
            )?;
            apply_session_pocket_camera_frame_to_desktop_session(session, &mut reset_machine)?;

            let effective_config = primary_loaded.effective_config;
            let next_save_sessions = open_save_sessions_for_session(
                &DesktopSession {
                    config: effective_config.clone(),
                    ..session.clone()
                },
                &mut reset_machine,
            )?;
            (
                effective_config,
                boot_rom_fallback_warnings,
                reset_machine,
                next_save_sessions,
            )
        }
        (_, false, true, _, _, _) => {
            let loaded =
                match load_machine_for_rom(&session.config, &session.current_dir, rom_bytes) {
                    Ok(result) => result,
                    Err(error) => {
                        return Err(format_display_error(
                            "failed to reload cartridge during Pokemon Pikachu Color reset",
                            &error,
                        ));
                    }
                };
            let mut boot_rom_fallback_warnings = Vec::new();
            if let Some(warning) = loaded.boot_rom_fallback_warning {
                boot_rom_fallback_warnings.push(warning);
            }
            write_cartridge_diagnostics(&loaded.diagnostics);
            let mut reset_machine = DesktopEmulationSession::new_pokemon_pikachu_color(
                loaded.machine,
                session.pokemon_pikachu_color_gift,
                PokemonPikachuColorRegion::Auto,
            );
            restore_battery_backed_states_by_player_slot(
                &mut reset_machine,
                &battery_backed_states,
                "after reset",
            )?;
            apply_session_pocket_camera_frame_to_desktop_session(session, &mut reset_machine)?;

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
                    ..session.clone()
                },
                &mut reset_machine,
            )?;
            (
                effective_config,
                boot_rom_fallback_warnings,
                reset_machine,
                next_save_sessions,
            )
        }
        (_, false, false, true, _, _) => {
            reset_pokemon_mystery_gift_session(session, rom_bytes, &battery_backed_states)?
        }
        (
            _,
            false,
            false,
            false,
            DesktopExternalPortSelection::FourPlayerAdapter,
            Some(player_count),
        ) => {
            let loaded = load_dmg07_machines_for_rom(
                &session.config,
                &session.current_dir,
                rom_bytes,
                player_count,
                "resetting a DMG-07 session",
            )?;
            let boot_rom_fallback_warnings = loaded.boot_rom_fallback_warnings;
            write_cartridge_diagnostics(&loaded.diagnostics);
            let mut reset_machine = loaded.machine;
            restore_battery_backed_states_by_player_slot(
                &mut reset_machine,
                &battery_backed_states,
                "after reset",
            )?;
            apply_session_pocket_camera_frame_to_desktop_session(session, &mut reset_machine)?;

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
                    ..session.clone()
                },
                &mut reset_machine,
            )?;
            (
                effective_config,
                boot_rom_fallback_warnings,
                reset_machine,
                next_save_sessions,
            )
        }
        _ => {
            let loaded =
                match load_machine_for_rom(&session.config, &session.current_dir, rom_bytes) {
                    Ok(result) => result,
                    Err(error) => {
                        return Err(format_display_error(
                            "failed to reload cartridge during reset",
                            &error,
                        ));
                    }
                };
            let mut boot_rom_fallback_warnings = Vec::new();
            if let Some(warning) = loaded.boot_rom_fallback_warning {
                boot_rom_fallback_warnings.push(warning);
            }
            write_cartridge_diagnostics(&loaded.diagnostics);
            let mut reset_machine = DesktopEmulationSession::new_single(loaded.machine);
            restore_battery_backed_states_by_player_slot(
                &mut reset_machine,
                &battery_backed_states,
                "after reset",
            )?;
            apply_external_port_selection_to_machine(
                reset_machine.primary_machine_mut(),
                session.external_port_selection,
            );
            apply_session_pocket_camera_frame_to_desktop_session(session, &mut reset_machine)?;

            let effective_config = loaded.effective_config;
            let next_save_sessions = open_save_sessions_for_session(
                &DesktopSession {
                    config: effective_config.clone(),
                    linked_secondary_rom: None,
                    dmg07_player_count: None,
                    cgb_infrared_link_active: false,
                    pokemon_pikachu_color_active: false,
                    pokemon_mystery_gift_active: false,
                    ..session.clone()
                },
                &mut reset_machine,
            )?;
            (
                effective_config,
                boot_rom_fallback_warnings,
                reset_machine,
                next_save_sessions,
            )
        }
    };

    for warning in &boot_rom_fallback_warnings {
        log_boot_rom_fallback_warning(Some(warning));
    }
    let config_fell_back = effective_config != session.config;
    session.config = effective_config;
    if config_fell_back && !session.test_runner {
        settings_store.persist_machine_preferences(&session.config)?;
    }
    let reset_console_model = reset_machine.primary_machine().apu().console_model();

    clear_live_input_state(machine, runtime);
    *machine = reset_machine;
    reset_frontend_timeline_state(runtime);
    if let Some(audio_output) = &mut runtime.audio_output {
        audio_output.reset_for_session_swap(reset_console_model)?;
    }
    if let Some(audio_recorder) = &mut runtime.audio_recorder {
        audio_recorder.reset_for_session_swap(reset_console_model)?;
    }
    runtime.save_sessions = next_save_sessions;
    runtime.rtc_sync.resync_to_host_clock();
    Ok(())
}

fn save_screenshot_for_session(
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
    video_options: &VideoOptions,
) -> Result<PathBuf, String> {
    let dimensions =
        framebuffer_dimensions_for_session(machine, video_options, session.has_loaded_rom());
    let rendered = screenshot_output::render_screenshot(
        framebuffer_render_input_for_session(
            machine,
            dimensions,
            video_options,
            session.has_loaded_rom(),
        ),
        video_options,
    );
    let output_path = screenshot_output::resolve_next_screenshot_output_path(
        session.rom_path(),
        session.current_dir.as_path(),
    )?;
    screenshot_output::save_rendered_screenshot_png(&rendered, &output_path)?;
    Ok(output_path)
}

fn save_screenshot_for_session_to_path(
    machine: &DesktopEmulationSession,
    video_options: &VideoOptions,
    output_path: &Path,
    session_has_loaded_rom: bool,
) -> Result<(), String> {
    let dimensions =
        framebuffer_dimensions_for_session(machine, video_options, session_has_loaded_rom);
    let rendered = screenshot_output::render_screenshot(
        framebuffer_render_input_for_session(
            machine,
            dimensions,
            video_options,
            session_has_loaded_rom,
        ),
        video_options,
    );
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create benchmark screenshot directory {}: {error}",
                parent.display()
            )
        })?;
    }
    screenshot_output::save_rendered_screenshot_png(&rendered, output_path)
}

fn write_benchmark_artifacts_for_session(
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
    video_options: &VideoOptions,
    performance_counter: &PerformanceCounter,
) -> Result<(), String> {
    let Some(benchmark) = session.benchmark.as_ref() else {
        return Ok(());
    };

    let elapsed_seconds = benchmark.started_at.elapsed().as_secs_f64();
    let executed_tcycles = machine
        .primary_machine()
        .next_t_cycle()
        .get()
        .saturating_sub(benchmark.started_t_cycle);
    let screenshot_path = benchmark
        .case
        .screenshot
        .then(|| frontend_screenshot_path(GB_DESKTOP_FRONTEND, &benchmark.case.artifact_id));
    if let Some(screenshot_path) = &screenshot_path {
        let output_path = resolve_path(session.current_dir.as_path(), screenshot_path);
        save_screenshot_for_session_to_path(
            machine,
            video_options,
            &output_path,
            session.has_loaded_rom(),
        )?;
    }
    if benchmark.case.stats {
        let stats_path = frontend_stats_path(GB_DESKTOP_FRONTEND, &benchmark.case.artifact_id);
        let stats = BenchmarkStats::new(
            GB_DESKTOP_FRONTEND,
            &benchmark.case,
            session.test_runner,
            performance_counter.presented_frames_total,
            elapsed_seconds,
            Some(executed_tcycles),
            screenshot_path.as_deref(),
        );
        let encoded_stats = encode_stats_toml(&stats)
            .map_err(|error| format!("failed to encode benchmark stats TOML: {error}"))?;
        write_text_file_with_parent(
            &resolve_path(session.current_dir.as_path(), &stats_path),
            &encoded_stats,
        )?;
    }

    Ok(())
}

fn write_text_file_with_parent(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create artifact directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(path, text).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn toggle_fullscreen(window: &mut Window) -> Result<(), String> {
    let target_state = window.fullscreen_state() == FullscreenType::Off;
    map_display_result(
        window.set_fullscreen(target_state),
        "failed to toggle SDL3 fullscreen state",
    )
}

fn set_fullscreen_state(window: &mut Window, enabled: bool) -> Result<(), String> {
    if (window.fullscreen_state() != FullscreenType::Off) == enabled {
        return Ok(());
    }

    map_display_result(
        window.set_fullscreen(enabled),
        "failed to set SDL3 fullscreen state",
    )
}

fn apply_renderer_vsync(
    canvas: &mut Canvas<Window>,
    frame_pacer: &mut FramePacer,
    vsync_enabled: bool,
) -> Result<(), String> {
    let interval = if vsync_enabled {
        1
    } else {
        sys::render::SDL_RENDERER_VSYNC_DISABLED
    };
    // SDL3 exposes render-vsync control on the renderer, not on the window.
    let success = unsafe { sys::render::SDL_SetRenderVSync(canvas.raw(), interval) };
    if !success {
        return Err(format!(
            "failed to configure SDL3 renderer vsync: {}",
            sdl3::get_error()
        ));
    }

    frame_pacer.set_vsync_enabled(vsync_enabled);
    Ok(())
}

#[cfg(test)]
fn apply_window_scale(window: &mut Window, scale: u8) -> Result<(), String> {
    apply_window_scale_for_dimensions(
        window,
        scale,
        FramebufferDimensions {
            width: FRAMEBUFFER_WIDTH,
            height: FRAMEBUFFER_HEIGHT,
        },
    )
}

fn apply_window_scale_for_dimensions(
    window: &mut Window,
    scale: u8,
    dimensions: FramebufferDimensions,
) -> Result<(), String> {
    let scale = u32::from(scale.max(1));
    let width = dimensions
        .width
        .checked_mul(scale)
        .ok_or_else(|| overflow_error("window width overflowed while applying window scale"))?;
    let height = dimensions
        .height
        .checked_mul(scale)
        .ok_or_else(|| overflow_error("window height overflowed while applying window scale"))?;
    map_display_result(
        window.set_size(width, height),
        "failed to resize SDL3 window",
    )
}

fn apply_canvas_video_options_for_dimensions(
    canvas: &mut Canvas<Window>,
    video_options: &VideoOptions,
    dimensions: FramebufferDimensions,
) -> Result<(), String> {
    let presentation_mode = if video_options.integer_scale {
        sys::render::SDL_LOGICAL_PRESENTATION_INTEGER_SCALE
    } else {
        sys::render::SDL_LOGICAL_PRESENTATION_LETTERBOX
    };
    map_display_result(
        canvas.set_logical_size(dimensions.width, dimensions.height, presentation_mode),
        "failed to configure SDL3 logical presentation",
    )
}

fn sync_audio_playback_state(
    machine: &DesktopEmulationSession,
    runtime: &FrontendRuntime,
) -> Result<(), String> {
    let Some(audio_output) = runtime.audio_output.as_ref() else {
        return Ok(());
    };

    if emulation_paused(audio_source_machine(machine), runtime) {
        audio_output.pause()
    } else {
        audio_output.resume()
    }
}

fn sync_fast_forward_audio_state(
    runtime: &mut FrontendRuntime,
    fast_forward_active: bool,
) -> Result<(), String> {
    if fast_forward_active {
        if !runtime.fast_forward_audio_suppressed {
            if let Some(audio_output) = &mut runtime.audio_output {
                audio_output.clear_buffer()?;
            }
            runtime.fast_forward_audio_suppressed = true;
        }
    } else if runtime.fast_forward_audio_suppressed {
        if let Some(audio_output) = &mut runtime.audio_output {
            audio_output.clear_buffer()?;
        }
        runtime.fast_forward_audio_suppressed = false;
    }
    Ok(())
}

fn sync_fast_forward_host_pacing_state(
    canvas: &mut Canvas<Window>,
    frame_pacer: &mut FramePacer,
    runtime: &mut FrontendRuntime,
    fast_forward_active: bool,
) -> Result<(), String> {
    if fast_forward_active && runtime.video_options.vsync && !runtime.fast_forward_vsync_suppressed
    {
        apply_renderer_vsync(canvas, frame_pacer, false)?;
        runtime.fast_forward_vsync_suppressed = true;
    } else if !fast_forward_active && runtime.fast_forward_vsync_suppressed {
        apply_renderer_vsync(canvas, frame_pacer, runtime.video_options.vsync)?;
        runtime.fast_forward_vsync_suppressed = false;
    }
    Ok(())
}

fn framebuffer_dimensions_for_session(
    machine: &DesktopEmulationSession,
    video_options: &VideoOptions,
    session_has_loaded_rom: bool,
) -> FramebufferDimensions {
    let layout = view_layout_for_session(player_session_kind(machine));
    let cell_dimensions =
        framebuffer_cell_dimensions_for_session(machine, video_options, session_has_loaded_rom);
    FramebufferDimensions {
        width: cell_dimensions.width * layout.columns as u32,
        height: cell_dimensions.height * layout.rows as u32,
    }
}

fn framebuffer_cell_dimensions_for_session(
    machine: &DesktopEmulationSession,
    video_options: &VideoOptions,
    session_has_loaded_rom: bool,
) -> FramebufferDimensions {
    let layout = view_layout_for_session(player_session_kind(machine));
    layout
        .slots
        .into_iter()
        .flatten()
        .filter_map(|slot| machine.machine_for_player_slot(slot))
        .map(|machine| {
            framebuffer_panel_dimensions_for_machine(machine, video_options, session_has_loaded_rom)
        })
        .fold(
            FramebufferDimensions {
                width: FRAMEBUFFER_WIDTH,
                height: FRAMEBUFFER_HEIGHT,
            },
            |acc, dimensions| FramebufferDimensions {
                width: acc.width.max(dimensions.width),
                height: acc.height.max(dimensions.height),
            },
        )
}

fn framebuffer_panel_dimensions_for_machine(
    machine: &Machine<TraceSummaryBuffer>,
    video_options: &VideoOptions,
    session_has_loaded_rom: bool,
) -> FramebufferDimensions {
    if session_has_loaded_rom
        && machine.sgb_host().profile().is_some()
        && video_options.show_sgb_border
    {
        FramebufferDimensions {
            width: SGB_HOST_FRAMEBUFFER_WIDTH,
            height: SGB_HOST_FRAMEBUFFER_HEIGHT,
        }
    } else {
        FramebufferDimensions {
            width: FRAMEBUFFER_WIDTH,
            height: FRAMEBUFFER_HEIGHT,
        }
    }
}

pub(crate) fn framebuffer_cell_dimensions_for_panels(
    panels: &[Option<FramebufferPanelInput<'_>>; PLAYER_SLOT_COUNT],
) -> FramebufferDimensions {
    panels
        .iter()
        .filter_map(|panel| panel.as_ref())
        .map(|panel| panel.dimensions)
        .fold(
            FramebufferDimensions {
                width: FRAMEBUFFER_WIDTH,
                height: FRAMEBUFFER_HEIGHT,
            },
            |acc, dimensions| FramebufferDimensions {
                width: acc.width.max(dimensions.width),
                height: acc.height.max(dimensions.height),
            },
        )
}

fn framebuffer_panel_input_for_player_slot<'a>(
    machine: &'a DesktopEmulationSession,
    slot: PlayerSlot,
    display_palette: DisplayPalette,
    video_options: &VideoOptions,
    session_has_loaded_rom: bool,
) -> Option<FramebufferPanelInput<'a>> {
    let machine = machine.machine_for_player_slot(slot)?;
    let sgb_framebuffer_rgb555 = if session_has_loaded_rom && machine.sgb_host().profile().is_some()
    {
        if video_options.show_sgb_border {
            machine.sgb_framebuffer_rgb555()
        } else {
            machine.sgb_lcd_framebuffer_rgb555()
        }
    } else {
        None
    };
    let dimensions = if sgb_framebuffer_rgb555.is_some() {
        framebuffer_panel_dimensions_for_machine(machine, video_options, session_has_loaded_rom)
    } else {
        FramebufferDimensions {
            width: FRAMEBUFFER_WIDTH,
            height: FRAMEBUFFER_HEIGHT,
        }
    };
    Some(FramebufferPanelInput {
        dimensions,
        framebuffer: machine.ppu().framebuffer(),
        framebuffer_layer_sources: machine.ppu().framebuffer_layer_sources(),
        bgwin_framebuffer: machine.ppu().framebuffer_bgwin_panel_shades(),
        backdrop_framebuffer: machine.ppu().framebuffer_backdrop_panel_shades(),
        bgwin_framebuffer_layer_sources: machine.ppu().framebuffer_bgwin_layer_sources(),
        display_palette,
        cgb_framebuffer_rgb555: machine.ppu().cgb_framebuffer_rgb555(),
        sgb_framebuffer_rgb555,
    })
}

fn framebuffer_render_input_for_session<'a>(
    machine: &'a DesktopEmulationSession,
    dimensions: FramebufferDimensions,
    video_options: &VideoOptions,
    session_has_loaded_rom: bool,
) -> FramebufferRenderInput<'a> {
    let layout = view_layout_for_session(player_session_kind(machine));
    let display_palette = display_palette_for_desktop_palette(video_options.display_palette);
    FramebufferRenderInput {
        dimensions,
        panels: layout.slots.map(|slot| {
            slot.and_then(|slot| {
                framebuffer_panel_input_for_player_slot(
                    machine,
                    slot,
                    display_palette,
                    video_options,
                    session_has_loaded_rom,
                )
            })
        }),
    }
}

pub(crate) fn framebuffer_pitch_bytes_for_dimensions(dimensions: FramebufferDimensions) -> usize {
    dimensions.width as usize * 3
}

struct FrameBlendGammaTables {
    half: Vec<u8>,
}

impl FrameBlendGammaTables {
    fn new() -> Self {
        Self {
            half: frame_blend_gamma_table(0.5),
        }
    }

    fn table_for(&self, mode: DesktopFrameBlendingMode) -> Option<&[u8]> {
        match mode {
            DesktopFrameBlendingMode::Off => None,
            DesktopFrameBlendingMode::On => Some(&self.half),
        }
    }
}

fn frame_blend_gamma_tables() -> &'static FrameBlendGammaTables {
    static TABLES: OnceLock<FrameBlendGammaTables> = OnceLock::new();
    TABLES.get_or_init(FrameBlendGammaTables::new)
}

fn frame_blend_gamma_table(previous_weight: f32) -> Vec<u8> {
    const GAMMA: f32 = 2.2;
    let current_weight = 1.0 - previous_weight;
    let mut table = vec![0_u8; 256 * 256];
    for current in 0..=u8::MAX {
        let current_linear = (f32::from(current) / 255.0).powf(GAMMA);
        for previous in 0..=u8::MAX {
            let previous_linear = (f32::from(previous) / 255.0).powf(GAMMA);
            let blended = (current_linear * current_weight + previous_linear * previous_weight)
                .powf(1.0 / GAMMA);
            table[frame_blend_table_index(current, previous)] =
                (blended * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    table
}

fn frame_blend_table_index(current: u8, previous: u8) -> usize {
    usize::from(current) * 256 + usize::from(previous)
}

fn blend_rgb24_frames(
    target_rgb_frame: &mut [u8],
    current_rgb_frame: &[u8],
    previous_rgb_frame: &[u8],
    dimensions: FramebufferDimensions,
    mode: DesktopFrameBlendingMode,
) {
    if mode == DesktopFrameBlendingMode::Off
        || target_rgb_frame.len() != current_rgb_frame.len()
        || target_rgb_frame.len() != previous_rgb_frame.len()
    {
        return;
    }

    let tables = frame_blend_gamma_tables();
    let Some(table) = tables.table_for(mode) else {
        return;
    };
    let pitch = framebuffer_pitch_bytes_for_dimensions(dimensions);
    let height = dimensions.height as usize;
    for y in 0..height {
        let row_start = y * pitch;
        let row_end = row_start.saturating_add(pitch).min(target_rgb_frame.len());
        for index in row_start..row_end {
            target_rgb_frame[index] =
                table[frame_blend_table_index(current_rgb_frame[index], previous_rgb_frame[index])];
        }
    }
}

fn bgwin_layer_source_visible(
    video_options: &VideoOptions,
    source: PpuFramebufferLayerSource,
) -> bool {
    match source {
        PpuFramebufferLayerSource::Backdrop => false,
        PpuFramebufferLayerSource::Background => video_options.show_background,
        PpuFramebufferLayerSource::Window => video_options.show_window,
        PpuFramebufferLayerSource::Object => false,
    }
}

fn composite_framebuffer_panel_shade(
    final_shade: u8,
    final_source: PpuFramebufferLayerSource,
    bgwin_shade: u8,
    bgwin_source: PpuFramebufferLayerSource,
    backdrop_shade: u8,
    video_options: &VideoOptions,
) -> u8 {
    if video_options.show_objects && final_source == PpuFramebufferLayerSource::Object {
        return final_shade;
    }

    if matches!(
        final_source,
        PpuFramebufferLayerSource::Background | PpuFramebufferLayerSource::Window
    ) {
        if bgwin_layer_source_visible(video_options, final_source) {
            return final_shade;
        }
        return backdrop_shade;
    }

    if bgwin_layer_source_visible(video_options, bgwin_source) {
        bgwin_shade
    } else {
        backdrop_shade
    }
}

fn framebuffer_texture_scale_mode(video_options: &VideoOptions) -> ScaleMode {
    if video_options.presentation_filter {
        ScaleMode::Linear
    } else {
        ScaleMode::Nearest
    }
}

fn sync_framebuffer_texture_video_options(
    texture: &mut sdl3::render::Texture<'_>,
    video_options: &VideoOptions,
) {
    let expected_scale_mode = framebuffer_texture_scale_mode(video_options);
    if texture.scale_mode() != expected_scale_mode {
        texture.set_scale_mode(expected_scale_mode);
    }
}

fn create_framebuffer_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    dimensions: FramebufferDimensions,
) -> Result<sdl3::render::Texture<'a>, String> {
    map_display_result(
        texture_creator.create_texture_streaming(
            PixelFormat::RGB24,
            dimensions.width,
            dimensions.height,
        ),
        "failed to create framebuffer texture",
    )
}

fn sync_framebuffer_presentation_resources<'a>(
    canvas: &mut Canvas<Window>,
    texture_creator: &'a TextureCreator<WindowContext>,
    texture: &mut sdl3::render::Texture<'a>,
    rgb_frame: &mut Vec<u8>,
    current_dimensions: &mut FramebufferDimensions,
    source: FramebufferPresentationSource<'_>,
) -> Result<(), String> {
    let next_dimensions = framebuffer_dimensions_for_session(
        source.machine,
        source.video_options,
        source.session_has_loaded_rom,
    );
    if next_dimensions == *current_dimensions {
        return Ok(());
    }

    *texture = create_framebuffer_texture(texture_creator, next_dimensions)?;
    rgb_frame.resize(
        next_dimensions.height as usize * framebuffer_pitch_bytes_for_dimensions(next_dimensions),
        0,
    );
    if canvas.window().fullscreen_state() == FullscreenType::Off {
        apply_window_scale_for_dimensions(
            canvas.window_mut(),
            source.video_options.window_scale,
            next_dimensions,
        )?;
    }
    apply_canvas_video_options_for_dimensions(canvas, source.video_options, next_dimensions)?;
    *current_dimensions = next_dimensions;
    Ok(())
}

pub(crate) fn write_framebuffer_region(
    target_rgb_frame: &mut [u8],
    target_dimensions: FramebufferDimensions,
    target_origin_x: usize,
    target_origin_y: usize,
    source_panel: FramebufferPanelInput<'_>,
    video_options: &VideoOptions,
) {
    if let Some(sgb_framebuffer_rgb555) = source_panel.sgb_framebuffer_rgb555.as_deref() {
        write_rgb555_framebuffer_region(
            target_rgb_frame,
            target_dimensions,
            target_origin_x,
            target_origin_y,
            source_panel.dimensions,
            sgb_framebuffer_rgb555,
        );
        return;
    }

    if let Some(cgb_framebuffer_rgb555) = source_panel.cgb_framebuffer_rgb555 {
        write_rgb555_framebuffer_region(
            target_rgb_frame,
            target_dimensions,
            target_origin_x,
            target_origin_y,
            source_panel.dimensions,
            cgb_framebuffer_rgb555,
        );
        return;
    }

    write_monochrome_framebuffer_region(
        target_rgb_frame,
        target_dimensions,
        target_origin_x,
        target_origin_y,
        source_panel,
        video_options,
    );
}

fn write_rgb555_framebuffer_region(
    target_rgb_frame: &mut [u8],
    target_dimensions: FramebufferDimensions,
    target_origin_x: usize,
    target_origin_y: usize,
    source_dimensions: FramebufferDimensions,
    framebuffer_rgb555: &[u16],
) {
    let target_pitch_bytes = framebuffer_pitch_bytes_for_dimensions(target_dimensions);
    let target_width = target_dimensions.width as usize;
    let target_height = target_dimensions.height as usize;
    let source_width = source_dimensions.width as usize;
    let source_height = source_dimensions.height as usize;
    for y in 0..source_height {
        if target_origin_y + y >= target_height {
            break;
        }
        for x in 0..source_width {
            if target_origin_x + x >= target_width {
                break;
            }

            let source_index = y * source_width + x;
            let Some(&rgb555_pixel) = framebuffer_rgb555.get(source_index) else {
                continue;
            };
            let target_pixel_index =
                (target_origin_y + y) * target_pitch_bytes + ((target_origin_x + x) * 3);
            let [r, g, b] = rgb555_to_rgb888(rgb555_pixel);
            target_rgb_frame[target_pixel_index] = r;
            target_rgb_frame[target_pixel_index + 1] = g;
            target_rgb_frame[target_pixel_index + 2] = b;
        }
    }
}

fn rgb555_to_rgb888(color: u16) -> [u8; 3] {
    let red = (color & 0x001F) as u8;
    let green = ((color >> 5) & 0x001F) as u8;
    let blue = ((color >> 10) & 0x001F) as u8;
    [
        scale_5_bit_to_8_bit(red),
        scale_5_bit_to_8_bit(green),
        scale_5_bit_to_8_bit(blue),
    ]
}

fn scale_5_bit_to_8_bit(component: u8) -> u8 {
    (component << 3) | (component >> 2)
}

fn write_monochrome_framebuffer_region(
    target_rgb_frame: &mut [u8],
    target_dimensions: FramebufferDimensions,
    target_origin_x: usize,
    target_origin_y: usize,
    source_panel: FramebufferPanelInput<'_>,
    video_options: &VideoOptions,
) {
    let target_pitch_bytes = framebuffer_pitch_bytes_for_dimensions(target_dimensions);
    let target_width = target_dimensions.width as usize;
    let target_height = target_dimensions.height as usize;
    let source_width = source_panel.dimensions.width as usize;
    let source_height = source_panel.dimensions.height as usize;
    for y in 0..source_height {
        if target_origin_y + y >= target_height {
            break;
        }
        for x in 0..source_width {
            if target_origin_x + x >= target_width {
                break;
            }

            let source_index = y * source_width + x;
            let Some(&framebuffer_shade) = source_panel.framebuffer.get(source_index) else {
                continue;
            };
            let Some(&framebuffer_source) =
                source_panel.framebuffer_layer_sources.get(source_index)
            else {
                continue;
            };
            let Some(&bgwin_shade) = source_panel.bgwin_framebuffer.get(source_index) else {
                continue;
            };
            let Some(&bgwin_source) = source_panel
                .bgwin_framebuffer_layer_sources
                .get(source_index)
            else {
                continue;
            };
            let Some(&backdrop_shade) = source_panel.backdrop_framebuffer.get(source_index) else {
                continue;
            };
            let target_pixel_index =
                (target_origin_y + y) * target_pitch_bytes + ((target_origin_x + x) * 3);
            let panel_shade = composite_framebuffer_panel_shade(
                framebuffer_shade,
                framebuffer_source,
                bgwin_shade,
                bgwin_source,
                backdrop_shade,
                video_options,
            );
            let [r, g, b] = source_panel.display_palette.shade_rgb(panel_shade);
            target_rgb_frame[target_pixel_index] = r;
            target_rgb_frame[target_pixel_index + 1] = g;
            target_rgb_frame[target_pixel_index + 2] = b;
        }
    }
}

fn render_frame(
    canvas: &mut Canvas<Window>,
    texture: &mut sdl3::render::Texture<'_>,
    rgb_frame: &mut [u8],
    framebuffer: FramebufferRenderInput<'_>,
    video_options: &VideoOptions,
    presentation: RenderPresentationInput<'_>,
) -> Result<Duration, String> {
    let RenderPresentationInput {
        frame_blending_state,
        menu_state,
        hud,
    } = presentation;
    let menu_open = menu_state.is_some();
    apply_canvas_video_options_for_dimensions(canvas, video_options, framebuffer.dimensions)?;
    sync_framebuffer_texture_video_options(texture, video_options);
    rgb_frame.fill(0);
    let cell_dimensions = framebuffer_cell_dimensions_for_panels(&framebuffer.panels);
    let columns = (framebuffer.dimensions.width / cell_dimensions.width).max(1) as usize;
    for (panel_index, panel) in framebuffer.panels.into_iter().enumerate() {
        let Some(panel) = panel else {
            continue;
        };
        let column = panel_index % columns;
        let row = panel_index / columns;
        write_framebuffer_region(
            rgb_frame,
            framebuffer.dimensions,
            column * cell_dimensions.width as usize,
            row * cell_dimensions.height as usize,
            panel,
            video_options,
        );
    }
    if let Some(frame_blending_state) = frame_blending_state {
        frame_blending_state.apply(
            rgb_frame,
            framebuffer.dimensions,
            video_options.frame_blending,
        );
    }
    if let Some((menu_state, menu_presentation)) = menu_state {
        menu_state.render_overlay(
            rgb_frame,
            framebuffer.dimensions.width as usize,
            framebuffer.dimensions.height as usize,
            menu_presentation,
        );
    }
    if !menu_open
        && video_options.show_performance_hud
        && let Some(snapshot) = hud.performance
    {
        render_performance_hud(
            rgb_frame,
            framebuffer.dimensions.width as usize,
            framebuffer.dimensions.height as usize,
            snapshot,
        );
    }
    if !menu_open
        && video_options.show_cgb_infrared_helper
        && let Some(snapshot) = hud.cgb_ir
    {
        render_cgb_ir_indicator(
            rgb_frame,
            framebuffer.dimensions.width as usize,
            framebuffer.dimensions.height as usize,
            snapshot,
        );
    }
    if !menu_open && hud.rewind_indicator {
        render_rewind_indicator(
            rgb_frame,
            framebuffer.dimensions.width as usize,
            framebuffer.dimensions.height as usize,
        );
    }
    if !menu_open && hud.fast_forward_indicator {
        render_fast_forward_indicator(
            rgb_frame,
            framebuffer.dimensions.width as usize,
            framebuffer.dimensions.height as usize,
        );
    }

    map_display_result(
        texture.update(
            None,
            rgb_frame,
            framebuffer_pitch_bytes_for_dimensions(framebuffer.dimensions),
        ),
        "failed to update framebuffer texture",
    )?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    map_display_result(
        canvas.copy(texture, None, None),
        "failed to present framebuffer texture",
    )?;
    let present_started_at = Instant::now();
    canvas.present();
    Ok(present_started_at.elapsed())
}
