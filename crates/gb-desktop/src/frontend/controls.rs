fn open_selected_rom(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    selected_path: PathBuf,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let next_loaded_rom = load_selected_rom(selected_path, context.session)?;
    let loaded = load_machine_for_rom(
        &context.session.config,
        &context.session.current_dir,
        &next_loaded_rom.bytes,
    )?;
    log_boot_rom_fallback_warning(loaded.boot_rom_fallback_warning.as_deref());
    write_cartridge_diagnostics(&loaded.diagnostics);
    let effective_config = loaded.effective_config;
    let config_fell_back = effective_config != context.session.config;
    let mut next_machine = DesktopEmulationSession::new_single(loaded.machine);
    let next_external_port_selection = supported_external_port_selection_for_model(
        effective_config.launch.console_model,
        next_single_external_port_selection(context.session.external_port_selection),
    );
    apply_external_port_selection_to_machine(
        next_machine.primary_machine_mut(),
        next_external_port_selection,
    );
    let next_session = DesktopSession {
        config: effective_config.clone(),
        test_runner: context.session.test_runner,
        benchmark: context.session.benchmark.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: Some(next_loaded_rom),
        linked_secondary_rom: None,
        dmg07_player_count: None,
        cgb_infrared_link_active: false,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: context.session.pokemon_pikachu_color_gift,
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: context.session.pokemon_mystery_gift_kind,
        pokemon_mystery_gift_code: context.session.pokemon_mystery_gift_code,
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
        pocket_camera_frame: context.session.pocket_camera_frame.clone(),
        external_port_selection: next_external_port_selection,
    };
    apply_session_pocket_camera_frame_to_desktop_session(&next_session, &mut next_machine)?;
    let next_save_sessions = open_save_sessions_for_session(&next_session, &mut next_machine)?;

    close_runtime_save_sessions(context.runtime, context.machine)?;
    let next_console_model = next_machine.primary_machine().apu().console_model();

    context.session.config = effective_config;
    context.session.loaded_rom = next_session.loaded_rom;
    context.session.linked_secondary_rom = None;
    context.session.dmg07_player_count = None;
    context.session.cgb_infrared_link_active = false;
    context.session.pokemon_pikachu_color_active = false;
    context.session.pokemon_mystery_gift_active = false;
    context.session.last_open_directory = context
        .session
        .loaded_rom
        .as_ref()
        .and_then(|rom| rom.path.parent().map(Path::to_path_buf));
    context.session.external_port_selection = next_external_port_selection;
    if config_fell_back && !context.session.test_runner {
        context
            .settings_store
            .persist_machine_preferences(&context.session.config)?;
    }
    if !context.session.test_runner
        && let Some(rom_path) = context.session.rom_path()
    {
        context.settings_store.remember_loaded_rom(rom_path)?;
        context.session.recent_roms = context.settings_store.recent_roms().to_vec();
    }
    clear_live_input_state(context.machine, context.runtime);
    *context.machine = next_machine;
    reset_frontend_timeline_state(context.runtime);
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.reset_for_session_swap(next_console_model)?;
    }
    match context.runtime.audio_recording_mode {
        DesktopAudioRecordingMode::Disabled => {
            finish_audio_recorder(&mut context.runtime.audio_recorder)?;
        }
        DesktopAudioRecordingMode::Automatic => {
            restart_automatic_audio_recorder(context.runtime, context.session, context.machine)?;
        }
        DesktopAudioRecordingMode::Explicit(_) => {
            if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
                audio_recorder.reset_for_session_swap(next_console_model)?;
            }
        }
    }
    context.runtime.save_sessions = next_save_sessions;
    context.runtime.rtc_sync.resync_to_host_clock();
    match autoload_machine_state_slot_if_available(
        context.session,
        context.machine,
        context.runtime,
        context.frame_pacer,
    ) {
        Ok(Some(path)) => {
            eprintln!("info: autoloaded state from {}", path.display());
        }
        Ok(None) => {}
        Err(error) => {
            show_warning_message(Some(canvas.window()), "Autoload State", &error);
            eprintln!("warning: {error}");
        }
    }
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;
    context.runtime.paused = false;

    if context.runtime.menu_state.is_open() {
        close_menu(event_pump, context.machine, context.runtime)?;
    }

    Ok(())
}

fn open_selected_linked_secondary_rom(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    selected_path: PathBuf,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    if !context.session.has_loaded_rom() {
        return Err(
            "GAME LINK requires a primary ROM before selecting a second cartridge".to_string(),
        );
    }
    if !context
        .session
        .config
        .launch
        .console_model
        .allows_ext_port_menu()
    {
        return Ok(());
    }

    let next_secondary_rom = load_selected_rom(selected_path, context.session)?;
    activate_game_link_with_secondary_rom(event_pump, canvas, next_secondary_rom, context)
}

fn open_game_link_secondary_rom_dialog(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) {
    if !context.session.has_loaded_rom()
        || !context
            .session
            .config
            .launch
            .console_model
            .allows_ext_port_menu()
    {
        return;
    }

    context.runtime.open_rom_dialog_mode = OpenRomDialogMode::LinkedSecondary;
    let default_location = context.session.rom_directory_hint();
    if let Err(error) = context.runtime.open_rom_dialog.show_file(
        &ROM_FILE_DIALOG_FILTERS,
        canvas.window(),
        default_location,
    ) {
        context.runtime.open_rom_dialog_mode = OpenRomDialogMode::Primary;
        show_warning_message(Some(canvas.window()), "GAME LINK", &error);
        eprintln!("warning: {error}");
    }
}

fn activate_game_link_with_secondary_rom(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    next_secondary_rom: LoadedRom,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    if !context.session.has_loaded_rom() {
        return Err(
            "GAME LINK requires a primary ROM before selecting a second cartridge".to_string(),
        );
    }
    if !context
        .session
        .config
        .launch
        .console_model
        .allows_ext_port_menu()
    {
        return Ok(());
    }

    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let Some(primary_rom_bytes) = context.session.rom_bytes() else {
        return Err(
            "GAME LINK requires a primary ROM before selecting a second cartridge".to_string(),
        );
    };
    let primary_battery_backed_state = uses_battery_backed_hardware_persistence(
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistence_metadata(),
    )
    .then(|| {
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistent_state()
    });

    let loaded_primary = load_machine_for_rom(
        &context.session.config,
        &context.session.current_dir,
        primary_rom_bytes,
    )?;
    let loaded_secondary = load_machine_for_rom(
        &context.session.config,
        &context.session.current_dir,
        &next_secondary_rom.bytes,
    )?;
    if loaded_primary.effective_config != loaded_secondary.effective_config {
        return Err(
            "activating GAME LINK produced divergent effective configs between primary and secondary machines"
                .to_string(),
        );
    }

    log_boot_rom_fallback_warning(loaded_primary.boot_rom_fallback_warning.as_deref());
    write_cartridge_diagnostics(&loaded_primary.diagnostics);
    log_boot_rom_fallback_warning(loaded_secondary.boot_rom_fallback_warning.as_deref());
    write_cartridge_diagnostics(&loaded_secondary.diagnostics);

    let effective_config = loaded_primary.effective_config;
    let config_fell_back = effective_config != context.session.config;
    let mut next_machine = DesktopEmulationSession::new_single(loaded_primary.machine);
    if let Some(persistent_state) = primary_battery_backed_state
        && let Err(error) = next_machine
            .primary_machine_mut()
            .restore_cartridge_persistent_state(&persistent_state)
    {
        return Err(format!(
            "failed to restore battery-backed persistence while activating GAME LINK: {error:?}"
        ));
    }
    next_machine.attach_secondary_dmg04(loaded_secondary.machine)?;

    let next_session = DesktopSession {
        config: effective_config.clone(),
        test_runner: context.session.test_runner,
        benchmark: context.session.benchmark.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: context.session.loaded_rom.clone(),
        linked_secondary_rom: Some(next_secondary_rom),
        dmg07_player_count: None,
        cgb_infrared_link_active: false,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: context.session.pokemon_pikachu_color_gift,
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: context.session.pokemon_mystery_gift_kind,
        pokemon_mystery_gift_code: context.session.pokemon_mystery_gift_code,
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
        pocket_camera_frame: context.session.pocket_camera_frame.clone(),
        external_port_selection: DesktopExternalPortSelection::GameLink,
    };
    apply_session_pocket_camera_frame_to_desktop_session(&next_session, &mut next_machine)?;
    let next_save_sessions = open_save_sessions_for_session(&next_session, &mut next_machine)?;

    close_runtime_save_sessions(context.runtime, context.machine)?;
    let next_console_model = next_machine.primary_machine().apu().console_model();

    context.session.config = effective_config;
    context.session.linked_secondary_rom = next_session.linked_secondary_rom;
    context.session.dmg07_player_count = None;
    context.session.cgb_infrared_link_active = false;
    context.session.pokemon_pikachu_color_active = false;
    context.session.pokemon_mystery_gift_active = false;
    context.session.last_open_directory = context
        .session
        .linked_secondary_rom
        .as_ref()
        .and_then(|rom| rom.path.parent().map(Path::to_path_buf));
    context.session.external_port_selection = DesktopExternalPortSelection::GameLink;
    if config_fell_back && !context.session.test_runner {
        context
            .settings_store
            .persist_machine_preferences(&context.session.config)?;
    }
    clear_live_input_state(context.machine, context.runtime);
    *context.machine = next_machine;
    reset_frontend_timeline_state(context.runtime);
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.reset_for_session_swap(next_console_model)?;
    }
    if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
        audio_recorder.reset_for_session_swap(next_console_model)?;
    }
    context.runtime.save_sessions = next_save_sessions;
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;

    if context.runtime.menu_state.is_open() {
        close_menu(event_pump, context.machine, context.runtime)?;
    }

    Ok(())
}

fn deactivate_cgb_infrared_pair(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);

    if context.machine.is_linked_cgb_infrared_two_player()
        || context.machine.is_pokemon_pikachu_color()
        || context.machine.is_pokemon_mystery_gift()
    {
        close_runtime_save_sessions(context.runtime, context.machine)?;
        context.machine.detach_to_single_primary();
    }

    context.session.linked_secondary_rom = None;
    context.session.dmg07_player_count = None;
    context.session.cgb_infrared_link_active = false;
    context.session.pokemon_pikachu_color_active = false;
    context.session.pokemon_mystery_gift_active = false;
    context.session.external_port_selection = DesktopExternalPortSelection::None;
    apply_external_port_selection_to_machine(
        context.machine.primary_machine_mut(),
        DesktopExternalPortSelection::None,
    );
    context.runtime.save_sessions =
        open_save_sessions_for_session(context.session, context.machine)?;
    reset_frontend_timeline_state(context.runtime);
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;
    context.runtime.rtc_sync.resync_to_host_clock();

    Ok(())
}

fn activate_cgb_infrared_same_game(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    if !context.session.has_loaded_rom()
        || context.session.config.launch.console_model != DesktopConsoleModel::GameBoyColor
    {
        return Ok(());
    }
    if cgb_infrared_same_game_active(context.session)
        && context.machine.is_linked_cgb_infrared_two_player()
    {
        return Ok(());
    }

    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let Some(primary_rom) = context.session.loaded_rom.clone() else {
        return Ok(());
    };
    let primary_battery_backed_state = uses_battery_backed_hardware_persistence(
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistence_metadata(),
    )
    .then(|| {
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistent_state()
    });

    let loaded = load_cgb_infrared_machines_for_roms(
        &context.session.config,
        &context.session.current_dir,
        &primary_rom.bytes,
        &primary_rom.bytes,
        "activating CGB IR SAME GAME",
    )?;
    for warning in &loaded.boot_rom_fallback_warnings {
        log_boot_rom_fallback_warning(Some(warning));
    }
    write_cartridge_diagnostics(&loaded.diagnostics);

    let effective_config = loaded.effective_config;
    let config_fell_back = effective_config != context.session.config;
    let mut next_machine = loaded.machine;
    if let Some(persistent_state) = primary_battery_backed_state
        && let Err(error) = next_machine
            .primary_machine_mut()
            .restore_cartridge_persistent_state(&persistent_state)
    {
        return Err(format!(
            "failed to restore battery-backed persistence while activating CGB IR SAME GAME: {error:?}"
        ));
    }

    let next_session = DesktopSession {
        config: effective_config.clone(),
        test_runner: context.session.test_runner,
        benchmark: context.session.benchmark.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: context.session.loaded_rom.clone(),
        linked_secondary_rom: Some(primary_rom),
        dmg07_player_count: None,
        cgb_infrared_link_active: true,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: context.session.pokemon_pikachu_color_gift,
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: context.session.pokemon_mystery_gift_kind,
        pokemon_mystery_gift_code: context.session.pokemon_mystery_gift_code,
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
        pocket_camera_frame: context.session.pocket_camera_frame.clone(),
        external_port_selection: DesktopExternalPortSelection::None,
    };
    apply_session_pocket_camera_frame_to_desktop_session(&next_session, &mut next_machine)?;
    let next_save_sessions = open_save_sessions_for_session(&next_session, &mut next_machine)?;

    close_runtime_save_sessions(context.runtime, context.machine)?;
    let next_console_model = next_machine.primary_machine().apu().console_model();

    context.session.config = effective_config;
    context.session.linked_secondary_rom = next_session.linked_secondary_rom;
    context.session.dmg07_player_count = None;
    context.session.cgb_infrared_link_active = true;
    context.session.pokemon_pikachu_color_active = false;
    context.session.pokemon_mystery_gift_active = false;
    context.session.last_open_directory = context
        .session
        .linked_secondary_rom
        .as_ref()
        .and_then(|rom| rom.path.parent().map(Path::to_path_buf));
    context.session.external_port_selection = DesktopExternalPortSelection::None;
    if config_fell_back && !context.session.test_runner {
        context
            .settings_store
            .persist_machine_preferences(&context.session.config)?;
    }
    clear_live_input_state(context.machine, context.runtime);
    *context.machine = next_machine;
    reset_frontend_timeline_state(context.runtime);
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.reset_for_session_swap(next_console_model)?;
    }
    if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
        audio_recorder.reset_for_session_swap(next_console_model)?;
    }
    context.runtime.save_sessions = next_save_sessions;
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;

    if context.runtime.menu_state.is_open() {
        close_menu(event_pump, context.machine, context.runtime)?;
    }

    Ok(())
}

fn activate_pokemon_pikachu_color(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    if !context.session.has_loaded_rom()
        || context.session.config.launch.console_model != DesktopConsoleModel::GameBoyColor
    {
        return Ok(());
    }
    if context.session.pokemon_pikachu_color_active && context.machine.is_pokemon_pikachu_color() {
        return Ok(());
    }

    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    close_runtime_save_sessions(context.runtime, context.machine)?;
    clear_live_input_state(context.machine, context.runtime);

    context.machine.detach_to_single_primary();
    let current_machine =
        std::mem::replace(context.machine, DesktopEmulationSession::Transitioning);
    let primary_machine = current_machine.into_primary_machine();
    *context.machine = DesktopEmulationSession::new_pokemon_pikachu_color(
        primary_machine,
        context.session.pokemon_pikachu_color_gift,
        PokemonPikachuColorRegion::Auto,
    );
    apply_external_port_selection_to_machine(
        context.machine.primary_machine_mut(),
        DesktopExternalPortSelection::None,
    );

    let next_session = DesktopSession {
        config: context.session.config.clone(),
        test_runner: context.session.test_runner,
        benchmark: context.session.benchmark.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: context.session.loaded_rom.clone(),
        linked_secondary_rom: None,
        dmg07_player_count: None,
        cgb_infrared_link_active: false,
        pokemon_pikachu_color_active: true,
        pokemon_pikachu_color_gift: context.session.pokemon_pikachu_color_gift,
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: context.session.pokemon_mystery_gift_kind,
        pokemon_mystery_gift_code: context.session.pokemon_mystery_gift_code,
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
        pocket_camera_frame: context.session.pocket_camera_frame.clone(),
        external_port_selection: DesktopExternalPortSelection::None,
    };
    apply_session_pocket_camera_frame_to_desktop_session(&next_session, context.machine)?;
    let next_save_sessions = open_save_sessions_for_session(&next_session, context.machine)?;
    let next_console_model = context.machine.primary_machine().apu().console_model();

    context.session.linked_secondary_rom = None;
    context.session.dmg07_player_count = None;
    context.session.cgb_infrared_link_active = false;
    context.session.pokemon_pikachu_color_active = true;
    context.session.pokemon_mystery_gift_active = false;
    context.session.external_port_selection = DesktopExternalPortSelection::None;
    context.runtime.save_sessions = next_save_sessions;
    reset_frontend_timeline_state(context.runtime);
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.reset_for_session_swap(next_console_model)?;
    }
    if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
        audio_recorder.reset_for_session_swap(next_console_model)?;
    }
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;

    Ok(())
}

fn activate_pokemon_mystery_gift(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    if !context.session.has_loaded_rom()
        || context.session.config.launch.console_model != DesktopConsoleModel::GameBoyColor
    {
        return Ok(());
    }
    if context.session.pokemon_mystery_gift_active && context.machine.is_pokemon_mystery_gift() {
        return Ok(());
    }

    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    close_runtime_save_sessions(context.runtime, context.machine)?;
    clear_live_input_state(context.machine, context.runtime);

    context.machine.detach_to_single_primary();
    let current_machine =
        std::mem::replace(context.machine, DesktopEmulationSession::Transitioning);
    let primary_machine = current_machine.into_primary_machine();
    *context.machine = DesktopEmulationSession::new_pokemon_mystery_gift(
        primary_machine,
        context.session.pokemon_mystery_gift_kind,
        context.session.pokemon_mystery_gift_code,
    );
    apply_external_port_selection_to_machine(
        context.machine.primary_machine_mut(),
        DesktopExternalPortSelection::None,
    );

    let next_session = DesktopSession {
        config: context.session.config.clone(),
        test_runner: context.session.test_runner,
        benchmark: context.session.benchmark.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: context.session.loaded_rom.clone(),
        linked_secondary_rom: None,
        dmg07_player_count: None,
        cgb_infrared_link_active: false,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: context.session.pokemon_pikachu_color_gift,
        pokemon_mystery_gift_active: true,
        pokemon_mystery_gift_kind: context.session.pokemon_mystery_gift_kind,
        pokemon_mystery_gift_code: context.session.pokemon_mystery_gift_code,
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
        pocket_camera_frame: context.session.pocket_camera_frame.clone(),
        external_port_selection: DesktopExternalPortSelection::None,
    };
    apply_session_pocket_camera_frame_to_desktop_session(&next_session, context.machine)?;
    let next_save_sessions = open_save_sessions_for_session(&next_session, context.machine)?;
    let next_console_model = context.machine.primary_machine().apu().console_model();

    context.session.linked_secondary_rom = None;
    context.session.dmg07_player_count = None;
    context.session.cgb_infrared_link_active = false;
    context.session.pokemon_pikachu_color_active = false;
    context.session.pokemon_mystery_gift_active = true;
    context.session.external_port_selection = DesktopExternalPortSelection::None;
    context.runtime.save_sessions = next_save_sessions;
    reset_frontend_timeline_state(context.runtime);
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.reset_for_session_swap(next_console_model)?;
    }
    if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
        audio_recorder.reset_for_session_swap(next_console_model)?;
    }
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;

    Ok(())
}

fn open_selected_cgb_infrared_secondary_rom(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    selected_path: PathBuf,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    if !context.session.has_loaded_rom() {
        return Err(
            "CGB IR requires a primary ROM before selecting a second cartridge".to_string(),
        );
    }
    if context.session.config.launch.console_model != DesktopConsoleModel::GameBoyColor {
        return Err("CGB IR requires MODEL GB COLOR".to_string());
    }

    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let Some(primary_rom_bytes) = context.session.rom_bytes() else {
        return Err(
            "CGB IR requires a primary ROM before selecting a second cartridge".to_string(),
        );
    };
    let primary_battery_backed_state = uses_battery_backed_hardware_persistence(
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistence_metadata(),
    )
    .then(|| {
        context
            .machine
            .primary_machine()
            .cartridge()
            .persistent_state()
    });

    let next_secondary_rom = load_selected_rom(selected_path, context.session)?;
    let loaded = load_cgb_infrared_machines_for_roms(
        &context.session.config,
        &context.session.current_dir,
        primary_rom_bytes,
        &next_secondary_rom.bytes,
        "activating CGB IR",
    )?;
    for warning in &loaded.boot_rom_fallback_warnings {
        log_boot_rom_fallback_warning(Some(warning));
    }
    write_cartridge_diagnostics(&loaded.diagnostics);

    let effective_config = loaded.effective_config;
    let config_fell_back = effective_config != context.session.config;
    let mut next_machine = loaded.machine;
    if let Some(persistent_state) = primary_battery_backed_state
        && let Err(error) = next_machine
            .primary_machine_mut()
            .restore_cartridge_persistent_state(&persistent_state)
    {
        return Err(format!(
            "failed to restore battery-backed persistence while activating CGB IR: {error:?}"
        ));
    }

    let next_session = DesktopSession {
        config: effective_config.clone(),
        test_runner: context.session.test_runner,
        benchmark: context.session.benchmark.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: context.session.loaded_rom.clone(),
        linked_secondary_rom: Some(next_secondary_rom),
        dmg07_player_count: None,
        cgb_infrared_link_active: true,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: context.session.pokemon_pikachu_color_gift,
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: context.session.pokemon_mystery_gift_kind,
        pokemon_mystery_gift_code: context.session.pokemon_mystery_gift_code,
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
        pocket_camera_frame: context.session.pocket_camera_frame.clone(),
        external_port_selection: DesktopExternalPortSelection::None,
    };
    apply_session_pocket_camera_frame_to_desktop_session(&next_session, &mut next_machine)?;
    let next_save_sessions = open_save_sessions_for_session(&next_session, &mut next_machine)?;

    close_runtime_save_sessions(context.runtime, context.machine)?;
    let next_console_model = next_machine.primary_machine().apu().console_model();

    context.session.config = effective_config;
    context.session.linked_secondary_rom = next_session.linked_secondary_rom;
    context.session.dmg07_player_count = None;
    context.session.cgb_infrared_link_active = true;
    context.session.pokemon_pikachu_color_active = false;
    context.session.pokemon_mystery_gift_active = false;
    context.session.last_open_directory = context
        .session
        .linked_secondary_rom
        .as_ref()
        .and_then(|rom| rom.path.parent().map(Path::to_path_buf));
    context.session.external_port_selection = DesktopExternalPortSelection::None;
    if config_fell_back && !context.session.test_runner {
        context
            .settings_store
            .persist_machine_preferences(&context.session.config)?;
    }
    clear_live_input_state(context.machine, context.runtime);
    *context.machine = next_machine;
    reset_frontend_timeline_state(context.runtime);
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.reset_for_session_swap(next_console_model)?;
    }
    if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
        audio_recorder.reset_for_session_swap(next_console_model)?;
    }
    context.runtime.save_sessions = next_save_sessions;
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;

    if context.runtime.menu_state.is_open() {
        close_menu(event_pump, context.machine, context.runtime)?;
    }

    Ok(())
}

fn activate_dmg07_adapter(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    player_count: DesktopDmg07PlayerCount,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    if !context.session.has_loaded_rom()
        || !context
            .session
            .config
            .launch
            .console_model
            .allows_ext_port_menu()
    {
        return Ok(());
    }

    drain_printed_pages_into_printer_output(
        canvas.window(),
        context.session,
        context.runtime,
        context.machine,
    );
    flush_pending_printer_output(canvas.window(), context.session, context.runtime);
    context.runtime.rtc_sync.apply_to_machine(context.machine);

    let Some(primary_rom_bytes) = context.session.rom_bytes() else {
        return Ok(());
    };
    let battery_backed_states = battery_backed_states_by_player_slot(context.machine);
    let loaded = load_dmg07_machines_for_rom(
        &context.session.config,
        &context.session.current_dir,
        primary_rom_bytes,
        player_count,
        "activating 4 PLAYER ADAPTER",
    )?;
    for warning in &loaded.boot_rom_fallback_warnings {
        log_boot_rom_fallback_warning(Some(warning));
    }
    write_cartridge_diagnostics(&loaded.diagnostics);

    let effective_config = loaded.effective_config;
    let config_fell_back = effective_config != context.session.config;
    let mut next_machine = loaded.machine;
    restore_battery_backed_states_by_player_slot(
        &mut next_machine,
        &battery_backed_states,
        "while activating 4 PLAYER ADAPTER",
    )?;

    let next_session = DesktopSession {
        config: effective_config.clone(),
        test_runner: context.session.test_runner,
        benchmark: context.session.benchmark.clone(),
        current_dir: context.session.current_dir.clone(),
        loaded_rom: context.session.loaded_rom.clone(),
        linked_secondary_rom: None,
        dmg07_player_count: Some(player_count),
        cgb_infrared_link_active: false,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: context.session.pokemon_pikachu_color_gift,
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: context.session.pokemon_mystery_gift_kind,
        pokemon_mystery_gift_code: context.session.pokemon_mystery_gift_code,
        last_open_directory: context.session.last_open_directory.clone(),
        recent_roms: context.session.recent_roms.clone(),
        pocket_camera_frame: context.session.pocket_camera_frame.clone(),
        external_port_selection: DesktopExternalPortSelection::FourPlayerAdapter,
    };
    apply_session_pocket_camera_frame_to_desktop_session(&next_session, &mut next_machine)?;
    let next_save_sessions = open_save_sessions_for_session(&next_session, &mut next_machine)?;

    close_runtime_save_sessions(context.runtime, context.machine)?;
    let next_console_model = next_machine.primary_machine().apu().console_model();

    context.session.config = effective_config;
    context.session.linked_secondary_rom = None;
    context.session.dmg07_player_count = Some(player_count);
    context.session.cgb_infrared_link_active = false;
    context.session.pokemon_pikachu_color_active = false;
    context.session.pokemon_mystery_gift_active = false;
    context.session.external_port_selection = DesktopExternalPortSelection::FourPlayerAdapter;
    if config_fell_back && !context.session.test_runner {
        context
            .settings_store
            .persist_machine_preferences(&context.session.config)?;
    }
    clear_live_input_state(context.machine, context.runtime);
    *context.machine = next_machine;
    reset_frontend_timeline_state(context.runtime);
    if let Some(audio_output) = &mut context.runtime.audio_output {
        audio_output.reset_for_session_swap(next_console_model)?;
    }
    if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
        audio_recorder.reset_for_session_swap(next_console_model)?;
    }
    context.runtime.save_sessions = next_save_sessions;
    context.runtime.rtc_sync.resync_to_host_clock();
    context.performance_counter.reset_base_title(
        canvas.window_mut(),
        window_title(context.session, &context.session.config),
    )?;
    context.runtime.paused = false;

    if context.runtime.menu_state.is_open() {
        close_menu(event_pump, context.machine, context.runtime)?;
    }

    Ok(())
}

fn open_menu(
    window: &Window,
    machine: &mut DesktopEmulationSession,
    session: &DesktopSession,
    runtime: &mut FrontendRuntime,
) -> Result<(), String> {
    runtime
        .menu_state
        .open(current_menu_presentation(window, runtime, machine, session));
    runtime.rewind_hotkey_active = false;
    runtime.rewind_gamepad_active = false;
    runtime.fast_forward_hotkey_active = false;
    runtime.fast_forward_gamepad_active = false;
    sync_fast_forward_audio_state(runtime, false)?;
    clear_live_input_state(machine, runtime);
    sync_audio_playback_state(machine, runtime)
}

fn close_menu(
    event_pump: &sdl3::EventPump,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
) -> Result<(), String> {
    runtime.menu_state.close();
    let keyboard_bindings = runtime.keyboard_bindings;
    sync_live_input_state(event_pump, &keyboard_bindings, machine, runtime);
    sync_audio_playback_state(machine, runtime)
}

fn next_machine_state_slot(slot: u8) -> u8 {
    if slot >= MACHINE_STATE_SLOT_COUNT {
        1
    } else {
        slot.saturating_add(1).max(1)
    }
}

fn next_machine_state_autoload_slot(slot: Option<u8>) -> Option<u8> {
    match slot {
        None => Some(1),
        Some(current) if current < MACHINE_STATE_SLOT_COUNT => Some(current.saturating_add(1)),
        Some(_) => None,
    }
}

fn machine_state_actions_available(
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
) -> bool {
    session.has_loaded_rom()
        && !machine.primary_machine().cartridge().is_empty()
        && !machine.is_linked_dmg04_two_player()
        && !machine.is_linked_cgb_infrared_two_player()
        && !machine.is_pokemon_pikachu_color()
        && !machine.is_pokemon_mystery_gift()
        && !machine.is_linked_dmg07()
}

fn rewind_actions_available(session: &DesktopSession, machine: &DesktopEmulationSession) -> bool {
    session.config.rewind.enabled && rewind_session_supported(session, machine)
}

fn rewind_session_supported(session: &DesktopSession, machine: &DesktopEmulationSession) -> bool {
    session.has_loaded_rom()
        && !machine.primary_machine().cartridge().is_empty()
        && !machine.is_linked_dmg04_two_player()
        && !machine.is_linked_cgb_infrared_two_player()
        && !machine.is_pokemon_pikachu_color()
        && !machine.is_pokemon_mystery_gift()
        && !machine.is_linked_dmg07()
}

fn reset_rewind_state(runtime: &mut FrontendRuntime) {
    runtime.rewind_buffer.clear();
    runtime.rewind_frame_tracker.reset();
    runtime.rewind_hotkey_active = false;
    runtime.rewind_gamepad_active = false;
}

fn reset_frontend_timeline_state(runtime: &mut FrontendRuntime) {
    reset_rewind_state(runtime);
    runtime.frame_blending_state.reset();
}

fn rebuild_rewind_state(runtime: &mut FrontendRuntime, options: RewindOptions) {
    runtime.rewind_buffer = MachineRewindBuffer::new(options.machine_rewind_config());
    runtime.rewind_frame_tracker.reset();
    runtime.rewind_hotkey_active = false;
    runtime.rewind_gamepad_active = false;
}

fn apply_rewind_options(
    context: &mut FrontendActionContext<'_>,
    options: RewindOptions,
) -> Result<(), String> {
    let previous = context.session.config.rewind;
    if previous == options {
        return Ok(());
    }

    context.session.config.rewind = options;
    context.settings_store.set_rewind_options(options)?;
    if previous.enabled != options.enabled
        || previous.machine_rewind_config() != options.machine_rewind_config()
    {
        rebuild_rewind_state(context.runtime, options);
    }
    Ok(())
}

fn apply_fast_forward_options(
    context: &mut FrontendActionContext<'_>,
    options: FastForwardOptions,
) -> Result<(), String> {
    if context.session.config.fast_forward == options {
        return Ok(());
    }

    context.session.config.fast_forward = options;
    context.settings_store.set_fast_forward_options(options)?;
    if !fast_forward_active(context.runtime, context.session, context.machine) {
        sync_fast_forward_audio_state(context.runtime, false)?;
    }
    Ok(())
}

fn rewind_history_seconds_from_stats(stats: gb_core::MachineRewindStats) -> f64 {
    match (stats.oldest_next_t_cycle, stats.newest_next_t_cycle) {
        (Some(oldest), Some(newest)) => {
            newest.get().saturating_sub(oldest.get()) as f64 / DMG_T_CYCLES_PER_SECOND as f64
        }
        _ => 0.0,
    }
}

fn current_rewind_hud_snapshot(
    runtime: &FrontendRuntime,
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
) -> RewindHudSnapshot {
    let stats = runtime.rewind_buffer.stats();
    RewindHudSnapshot {
        supported: rewind_session_supported(session, machine),
        enabled: session.config.rewind.enabled,
        rewinding: rewind_hold_active(runtime) && rewind_actions_available(session, machine),
        snapshot_count: stats.len,
        history_seconds: rewind_history_seconds_from_stats(stats),
        accounted_bytes: stats.estimated_bytes,
        max_bytes: runtime.rewind_buffer.config().max_estimated_bytes,
    }
}

fn rewind_indicator_visible(
    runtime: &FrontendRuntime,
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
    rewound_this_frame: bool,
) -> bool {
    rewind_actions_available(session, machine)
        && (rewind_hold_active(runtime) || rewound_this_frame)
}

fn rewind_hold_active(runtime: &FrontendRuntime) -> bool {
    runtime.rewind_hotkey_active || runtime.rewind_gamepad_active
}

fn clear_gamepad_hold_latches(runtime: &mut FrontendRuntime) {
    runtime.rewind_gamepad_active = false;
    runtime.fast_forward_gamepad_active = false;
    runtime.gamepad_trigger_state = GamepadTriggerState::default();
}

fn fast_forward_hold_active(runtime: &FrontendRuntime) -> bool {
    runtime.fast_forward_hotkey_active || runtime.fast_forward_gamepad_active
}

fn fast_forward_actions_available(
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
) -> bool {
    session.config.fast_forward.enabled && !machine.cartridge().is_empty()
}

fn fast_forward_active(
    runtime: &FrontendRuntime,
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
) -> bool {
    fast_forward_hold_active(runtime) && fast_forward_actions_available(session, machine)
}

fn fast_forward_indicator_visible(
    runtime: &FrontendRuntime,
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
    fast_forwarded_this_frame: bool,
) -> bool {
    fast_forward_actions_available(session, machine)
        && (fast_forward_hold_active(runtime) || fast_forwarded_this_frame)
}

fn rewind_restore_steps_for_speed(speed_multiplier: u8) -> usize {
    usize::from(speed_multiplier.max(1)).saturating_mul(2)
}

fn current_performance_hud_snapshot(
    snapshot: Option<PerformanceHudSnapshot>,
    runtime: &FrontendRuntime,
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
) -> Option<PerformanceHudSnapshot> {
    snapshot.map(|mut snapshot| {
        snapshot.rewind = current_rewind_hud_snapshot(runtime, session, machine);
        snapshot
    })
}

fn current_cgb_ir_hud_snapshot(
    machine: &DesktopEmulationSession,
) -> Option<CgbInfraredHudSnapshot> {
    if machine.is_pokemon_pikachu_color() {
        let p1 = machine.primary_machine().cgb_infrared_status()?;
        let accessory = machine.pokemon_pikachu_color_status()?;
        return Some(CgbInfraredHudSnapshot {
            p1: cgb_ir_participant_hud_snapshot(p1),
            p2: CgbInfraredParticipantHudSnapshot {
                emitter_on: accessory.emitter_on,
                read_enabled: true,
                optical_input_active: accessory.game_emitter_on,
                sensor_warmed: true,
                effective_signal_detected: accessory.game_emitter_on,
            },
        });
    }

    if machine.is_pokemon_mystery_gift() {
        let p1 = machine.primary_machine().cgb_infrared_status()?;
        let accessory = machine.pokemon_mystery_gift_status()?;
        return Some(CgbInfraredHudSnapshot {
            p1: cgb_ir_participant_hud_snapshot(p1),
            p2: CgbInfraredParticipantHudSnapshot {
                emitter_on: accessory.emitter_on,
                read_enabled: true,
                optical_input_active: accessory.game_emitter_on,
                sensor_warmed: true,
                effective_signal_detected: accessory.game_emitter_on,
            },
        });
    }

    if !machine.is_linked_cgb_infrared_two_player() {
        return None;
    }

    let p1 = machine
        .machine_for_player_slot(PlayerSlot::P1)?
        .cgb_infrared_status()?;
    let p2 = machine
        .machine_for_player_slot(PlayerSlot::P2)?
        .cgb_infrared_status()?;
    Some(CgbInfraredHudSnapshot {
        p1: cgb_ir_participant_hud_snapshot(p1),
        p2: cgb_ir_participant_hud_snapshot(p2),
    })
}

fn cgb_ir_participant_hud_snapshot(status: CgbInfraredStatus) -> CgbInfraredParticipantHudSnapshot {
    CgbInfraredParticipantHudSnapshot {
        emitter_on: status.emitter_on,
        read_enabled: status.read_enabled,
        optical_input_active: status.optical_input_active,
        sensor_warmed: status.sensor_warmed,
        effective_signal_detected: status.effective_signal_detected,
    }
}

fn reset_host_state_after_machine_restore(
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    frame_pacer: &mut FramePacer,
) -> Result<(), String> {
    clear_live_input_state(machine, runtime);
    if let Some(audio_output) = &mut runtime.audio_output {
        audio_output.clear_buffer()?;
    }
    if let Some(save_session) = &mut runtime.save_sessions[PlayerSlot::P1.index()] {
        save_session.reset_baseline_from_machine(machine.primary_machine());
    }
    runtime.rtc_sync.resync_to_host_clock();
    runtime.rewind_frame_tracker.reset();
    frame_pacer.reset_host_pacing();
    Ok(())
}

#[cfg(test)]
fn record_desktop_rewind_point(
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
) {
    if !desktop_rewind_recording_active(session, machine, runtime) {
        return;
    }

    record_desktop_rewind_point_active(machine, runtime);
}

fn desktop_rewind_recording_active(
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
    runtime: &FrontendRuntime,
) -> bool {
    rewind_actions_available(session, machine)
        && !rewind_hold_active(runtime)
        && !fast_forward_active(runtime, session, machine)
}

fn record_desktop_rewind_point_active(
    machine: &DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
) {
    let primary = machine.primary_machine();
    if runtime.rewind_frame_tracker.observe(primary) {
        runtime.rewind_buffer.record_frame_boundary(primary);
    } else {
        runtime.rewind_buffer.record_subframe(primary);
    }
}

#[cfg(test)]
fn rewind_desktop_session_once(
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    frame_pacer: &mut FramePacer,
) -> Result<bool, String> {
    rewind_desktop_session_steps(session, machine, runtime, frame_pacer, 1)
}

fn rewind_desktop_session_steps(
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    frame_pacer: &mut FramePacer,
    steps: usize,
) -> Result<bool, String> {
    if !rewind_actions_available(session, machine) {
        return Err("rewind is only available for single-machine sessions".to_string());
    }

    let mut restored_any = false;
    for _ in 0..steps.max(1) {
        let Some(_restored) = runtime
            .rewind_buffer
            .rewind_one(machine.primary_machine_mut())
            .map_err(|error| format!("failed to restore rewind snapshot: {error}"))?
        else {
            break;
        };
        restored_any = true;
    }

    if !restored_any {
        return Ok(false);
    }

    reset_host_state_after_machine_restore(machine, runtime, frame_pacer)?;
    Ok(true)
}

fn autoload_machine_state_slot_if_available(
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    frame_pacer: &mut FramePacer,
) -> Result<Option<PathBuf>, String> {
    let Some(slot) = session
        .config
        .machine_state
        .normalized_autoload_slot(MACHINE_STATE_SLOT_COUNT)
    else {
        return Ok(None);
    };
    if !machine_state_actions_available(session, machine) {
        return Ok(None);
    }
    if !machine_state_slot_path(session, slot).is_ok_and(|path| path.is_file()) {
        return Ok(None);
    }

    load_machine_state_slot(session, machine, runtime, frame_pacer, slot).map(Some)
}

fn machine_state_slot_load_available(
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
    slot: u8,
) -> bool {
    machine_state_actions_available(session, machine)
        && machine_state_slot_path(session, slot).is_ok_and(|path| path.is_file())
}

fn machine_state_slot_path(session: &DesktopSession, slot: u8) -> Result<PathBuf, String> {
    let slot = slot.clamp(1, MACHINE_STATE_SLOT_COUNT);
    let rom_path = session
        .rom_path()
        .ok_or_else(|| "machine save states require a loaded ROM".to_string())?;
    let state_key = session
        .config
        .saves
        .key_policy
        .resolve(rom_path)
        .map_err(|error| format!("failed to resolve machine state key: {error}"))?;
    let state_dir = rom_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("states");
    Ok(state_dir.join(format!(
        "{}.slot{slot}.{}",
        state_key.as_str(),
        MACHINE_SAVE_STATE_FILE_EXTENSION
    )))
}

fn save_machine_state_slot(
    session: &DesktopSession,
    machine: &DesktopEmulationSession,
    slot: u8,
) -> Result<PathBuf, String> {
    if !machine_state_actions_available(session, machine) {
        return Err(
            "machine save states are only available for single-machine sessions".to_string(),
        );
    }

    let path = machine_state_slot_path(session, slot)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create machine state directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let envelope = MachineSaveStateEnvelope::new(machine.primary_machine().capture_save_state());
    let bytes = encode_machine_save_state_envelope(&envelope).map_err(|error| {
        format!(
            "failed to encode .{} state {}: {error}",
            MACHINE_SAVE_STATE_FILE_EXTENSION,
            path.display()
        )
    })?;
    fs::write(&path, bytes).map_err(|error| {
        format!(
            "failed to write .{} state {}: {error}",
            MACHINE_SAVE_STATE_FILE_EXTENSION,
            path.display()
        )
    })?;
    Ok(path)
}

fn load_machine_state_slot(
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    frame_pacer: &mut FramePacer,
    slot: u8,
) -> Result<PathBuf, String> {
    if !machine_state_actions_available(session, machine) {
        return Err(
            "machine save states are only available for single-machine sessions".to_string(),
        );
    }

    let path = machine_state_slot_path(session, slot)?;
    let bytes = fs::read(&path).map_err(|error| {
        format!(
            "failed to read .{} state {}: {error}",
            MACHINE_SAVE_STATE_FILE_EXTENSION,
            path.display()
        )
    })?;
    restore_machine_state_slot_from_bytes(&path, bytes, machine, runtime, frame_pacer)
}

fn load_machine_state_slot_if_present(
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    frame_pacer: &mut FramePacer,
    slot: u8,
) -> Result<Option<PathBuf>, String> {
    if !machine_state_actions_available(session, machine) {
        return Err(
            "machine save states are only available for single-machine sessions".to_string(),
        );
    }

    let path = machine_state_slot_path(session, slot)?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read .{} state {}: {error}",
                MACHINE_SAVE_STATE_FILE_EXTENSION,
                path.display()
            ));
        }
    };
    restore_machine_state_slot_from_bytes(&path, bytes, machine, runtime, frame_pacer).map(Some)
}

fn restore_machine_state_slot_from_bytes(
    path: &Path,
    bytes: Vec<u8>,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    frame_pacer: &mut FramePacer,
) -> Result<PathBuf, String> {
    let envelope = decode_machine_save_state_envelope(&bytes).map_err(|error| {
        format!(
            "failed to decode .{} state {}: {error}",
            MACHINE_SAVE_STATE_FILE_EXTENSION,
            path.display()
        )
    })?;
    machine
        .primary_machine_mut()
        .restore_save_state(&envelope.state)
        .map_err(|error| format!("failed to restore state {}: {error}", path.display()))?;

    reset_host_state_after_machine_restore(machine, runtime, frame_pacer)?;
    reset_frontend_timeline_state(runtime);
    Ok(path.to_path_buf())
}

fn handle_load_machine_state_action(
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
    performance_counter: &mut PerformanceCounter,
    frame_pacer: &mut FramePacer,
    canvas: &mut Canvas<Window>,
) -> Result<(), String> {
    let slot = runtime.machine_state_slot;
    match load_machine_state_slot_if_present(session, machine, runtime, frame_pacer, slot) {
        Ok(Some(path)) => {
            eprintln!("info: state loaded from {}", path.display());
            sync_audio_playback_state(machine, runtime)?;
            performance_counter
                .reset_base_title(canvas.window_mut(), window_title(session, &session.config))?;
        }
        Ok(None) => {}
        Err(error) => {
            show_warning_message(Some(canvas.window()), "Load State", &error);
            eprintln!("warning: {error}");
        }
    }
    Ok(())
}

fn execute_menu_action(
    action: MenuAction,
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<Option<LoopSignal>, String> {
    match action {
        MenuAction::Resume => {
            context.runtime.paused = false;
            close_menu(event_pump, context.machine, context.runtime)?;
            Ok(None)
        }
        MenuAction::OpenRom => {
            let default_location = context.session.rom_directory_hint();
            context.runtime.open_rom_dialog_mode = OpenRomDialogMode::Primary;
            if let Err(error) = context.runtime.open_rom_dialog.show_file(
                &ROM_FILE_DIALOG_FILTERS,
                canvas.window(),
                default_location,
            ) {
                show_warning_message(Some(canvas.window()), "Open ROM", &error);
                eprintln!("warning: {error}");
            }
            Ok(None)
        }
        MenuAction::CycleConsoleModel => {
            let previous_model = context.session.config.launch.console_model;
            apply_machine_settings_change(canvas, context, "Console model", |config| {
                config.launch.console_model = next_console_model(config.launch.console_model);
                config.launch.normalize_revision_for_model();
            })?;
            let next_model = context.session.config.launch.console_model;
            if next_model != previous_model {
                context.runtime.video_options.display_palette =
                    DesktopDisplayPalette::default_for_console_model(next_model);
                context
                    .settings_store
                    .set_display_palette(context.runtime.video_options.display_palette)?;
            }
            Ok(None)
        }
        MenuAction::CycleHardwareRevision => {
            apply_machine_settings_change(canvas, context, "Hardware revision", |config| {
                config.launch.revision = next_revision(
                    config.launch.console_model,
                    config.launch.effective_revision(),
                );
            })?;
            Ok(None)
        }
        MenuAction::CycleSgbVideoStandard => {
            apply_machine_settings_change(canvas, context, "SGB video standard", |config| {
                if config
                    .launch
                    .console_model
                    .allows_sgb_video_standard_selection()
                {
                    config.launch.sgb_video_standard =
                        next_sgb_video_standard(config.launch.sgb_video_standard);
                }
            })?;
            Ok(None)
        }
        MenuAction::CycleStartupMode => {
            apply_machine_settings_change(canvas, context, "Startup mode", |config| {
                config.launch.startup_mode = next_startup_mode(config.launch.startup_mode);
            })?;
            Ok(None)
        }
        MenuAction::CycleExecutionMode => {
            apply_execution_mode_cycle_change(canvas, context)?;
            Ok(None)
        }
        MenuAction::SelectBootRomDirectoryPath => {
            let default_location = boot_rom_dialog_default_location(context.session);
            context
                .runtime
                .boot_rom_directory_dialog
                .show_folder(canvas.window(), &default_location);
            Ok(None)
        }
        MenuAction::CycleBootRomVerify => {
            apply_machine_settings_change(canvas, context, "Boot ROM verification", |config| {
                config.boot_rom.verification =
                    next_boot_rom_verification_mode(config.boot_rom.verification);
            })?;
            Ok(None)
        }
        MenuAction::ToggleSavesEnabled => {
            apply_machine_settings_change(canvas, context, "Save support", |config| {
                config.saves.enabled = !config.saves.enabled;
            })?;
            Ok(None)
        }
        MenuAction::CycleSavePolicy => {
            apply_machine_settings_change(canvas, context, "Save policy", |config| {
                config.saves.flush_policy = next_save_flush_policy(config.saves.flush_policy);
            })?;
            Ok(None)
        }
        MenuAction::ToggleRewindEnabled => {
            let mut rewind = context.session.config.rewind;
            rewind.enabled = !rewind.enabled;
            apply_rewind_options(context, rewind)?;
            Ok(None)
        }
        MenuAction::CycleRewindHistory => {
            let mut rewind = context.session.config.rewind;
            rewind.history_seconds = next_rewind_history_seconds(rewind.history_seconds);
            apply_rewind_options(context, rewind)?;
            Ok(None)
        }
        MenuAction::CycleRewindSubframes => {
            let mut rewind = context.session.config.rewind;
            rewind.subframes_per_frame =
                next_rewind_subframes_per_frame(rewind.subframes_per_frame);
            apply_rewind_options(context, rewind)?;
            Ok(None)
        }
        MenuAction::CycleRewindSpeed => {
            let mut rewind = context.session.config.rewind;
            rewind.speed_multiplier = next_rewind_speed_multiplier(rewind.speed_multiplier);
            apply_rewind_options(context, rewind)?;
            Ok(None)
        }
        MenuAction::CycleRewindMemory => {
            let mut rewind = context.session.config.rewind;
            rewind.max_memory_mib = next_rewind_max_memory_mib(rewind.max_memory_mib);
            apply_rewind_options(context, rewind)?;
            Ok(None)
        }
        MenuAction::ResetRewindDefaults => {
            apply_rewind_options(context, RewindOptions::default())?;
            Ok(None)
        }
        MenuAction::ToggleFastForwardEnabled => {
            let mut fast_forward = context.session.config.fast_forward;
            fast_forward.enabled = !fast_forward.enabled;
            apply_fast_forward_options(context, fast_forward)?;
            Ok(None)
        }
        MenuAction::CycleFastForwardSpeed => {
            let mut fast_forward = context.session.config.fast_forward;
            fast_forward.speed_multiplier =
                next_fast_forward_speed_multiplier(fast_forward.speed_multiplier);
            apply_fast_forward_options(context, fast_forward)?;
            Ok(None)
        }
        MenuAction::ResetFastForwardDefaults => {
            apply_fast_forward_options(context, FastForwardOptions::default())?;
            Ok(None)
        }
        MenuAction::ClearSaveDirectoryPath => {
            apply_machine_settings_change(canvas, context, "Save directory", |config| {
                config.saves.directory_policy = SaveDirectoryPolicy::RomFolderSavesSubdir;
            })?;
            Ok(None)
        }
        MenuAction::SelectSaveDirectoryPath => {
            let default_location = save_directory_dialog_default_location(context.session);
            context
                .runtime
                .save_directory_dialog
                .show_folder(canvas.window(), &default_location);
            Ok(None)
        }
        MenuAction::OpenRecentRom(index) => {
            let Some(rom_path) = context.session.recent_roms().get(index).cloned() else {
                return Ok(None);
            };
            if !rom_path.exists() {
                context.settings_store.remove_recent_rom(&rom_path)?;
                context.session.recent_roms = context.settings_store.recent_roms().to_vec();
                let error = format!("recent ROM no longer exists: {}", rom_path.display());
                show_warning_message(Some(canvas.window()), "Open Recent", &error);
                eprintln!("warning: {error}");
                return Ok(None);
            }

            if let Err(error) = open_selected_rom(event_pump, canvas, rom_path, context) {
                show_warning_message(Some(canvas.window()), "Open Recent", &error);
                eprintln!("warning: {error}");
            }
            Ok(None)
        }
        MenuAction::ClearRecentList => {
            context.settings_store.clear_recent_roms()?;
            context.session.recent_roms = context.settings_store.recent_roms().to_vec();
            context.runtime.menu_state.open(current_menu_presentation(
                canvas.window(),
                context.runtime,
                context.machine,
                context.session,
            ));
            Ok(None)
        }
        MenuAction::SaveState => {
            match save_machine_state_slot(
                context.session,
                context.machine,
                context.runtime.machine_state_slot,
            ) {
                Ok(path) => eprintln!("info: state saved to {}", path.display()),
                Err(error) => {
                    show_warning_message(Some(canvas.window()), "Save State", &error);
                    eprintln!("warning: {error}");
                }
            }
            Ok(None)
        }
        MenuAction::LoadState => {
            handle_load_machine_state_action(
                context.session,
                context.machine,
                context.runtime,
                context.performance_counter,
                context.frame_pacer,
                canvas,
            )?;
            Ok(None)
        }
        MenuAction::CycleStateSlot => {
            context.runtime.machine_state_slot =
                next_machine_state_slot(context.runtime.machine_state_slot);
            Ok(None)
        }
        MenuAction::CycleStateAutoloadSlot => {
            context.session.config.machine_state.autoload_slot = next_machine_state_autoload_slot(
                context.session.config.machine_state.autoload_slot,
            );
            context
                .settings_store
                .set_machine_state_options(context.session.config.machine_state)?;
            Ok(None)
        }
        MenuAction::SaveBattery => {
            flush_runtime_save_sessions_if_changed(context.runtime, context.machine, "menu")?;
            Ok(None)
        }
        MenuAction::ExportSave => {
            let default_location = external_save_export_dialog_default_location(context.session);
            if let Err(error) = context.runtime.external_save_export_dialog.show_save_file(
                &EXTERNAL_SAVE_FILE_DIALOG_FILTERS,
                canvas.window(),
                &default_location,
            ) {
                show_warning_message(Some(canvas.window()), "Export save", &error);
                eprintln!("warning: {error}");
            }
            Ok(None)
        }
        MenuAction::ImportSave => {
            let default_location = external_save_import_dialog_default_location(context.session);
            if let Err(error) = context.runtime.external_save_import_dialog.show_file(
                &EXTERNAL_SAVE_FILE_DIALOG_FILTERS,
                canvas.window(),
                &default_location,
            ) {
                show_warning_message(Some(canvas.window()), "Import save", &error);
                eprintln!("warning: {error}");
            }
            Ok(None)
        }
        MenuAction::SelectCameraImage => {
            let default_location = context.session.rom_directory_hint();
            if let Err(error) = context.runtime.camera_image_dialog.show_file(
                &CAMERA_IMAGE_FILE_DIALOG_FILTERS,
                canvas.window(),
                default_location,
            ) {
                show_warning_message(Some(canvas.window()), "Pocket Camera image", &error);
                eprintln!("warning: {error}");
            }
            Ok(None)
        }
        MenuAction::ToggleCameraLive => {
            if context.runtime.pocket_camera_live.is_enabled() {
                context.runtime.pocket_camera_live.stop();
            } else {
                match context.runtime.pocket_camera_live.start() {
                    Ok(()) => {
                        context.session.pocket_camera_frame = None;
                        if let Some(camera_name) = context.runtime.pocket_camera_live.camera_name()
                        {
                            eprintln!("info: Pocket Camera live input started from {camera_name}");
                        } else {
                            eprintln!("info: Pocket Camera live input started");
                        }
                    }
                    Err(error) => {
                        show_warning_message(Some(canvas.window()), "Pocket Camera live", &error);
                        eprintln!("warning: {error}");
                    }
                }
            }
            Ok(None)
        }
        MenuAction::ResetCameraImage => {
            context.runtime.pocket_camera_live.stop();
            context.session.pocket_camera_frame = None;
            clear_pocket_camera_frame_from_desktop_session(context.machine)?;
            Ok(None)
        }
        MenuAction::SaveScreenshot => {
            match save_screenshot_for_session(
                context.session,
                context.machine,
                &context.runtime.video_options,
            ) {
                Ok(path) => {
                    eprintln!("info: screenshot saved to {}", path.display());
                }
                Err(error) => {
                    show_warning_message(Some(canvas.window()), "Screenshot", &error);
                    eprintln!("warning: {error}");
                }
            }
            Ok(None)
        }
        MenuAction::ToggleFullscreen => {
            toggle_fullscreen(canvas.window_mut())?;
            context.runtime.video_options.fullscreen =
                canvas.window().fullscreen_state() != FullscreenType::Off;
            if canvas.window().fullscreen_state() == FullscreenType::Off {
                apply_window_scale_for_dimensions(
                    canvas.window_mut(),
                    context.runtime.video_options.window_scale,
                    framebuffer_dimensions_for_session(
                        context.machine,
                        &context.runtime.video_options,
                        context.session.has_loaded_rom(),
                    ),
                )?;
            }
            context
                .settings_store
                .set_fullscreen(context.runtime.video_options.fullscreen)?;
            Ok(None)
        }
        MenuAction::ToggleVsync => {
            context.runtime.video_options.vsync = !context.runtime.video_options.vsync;
            apply_renderer_vsync(
                canvas,
                context.frame_pacer,
                context.runtime.video_options.vsync,
            )?;
            context
                .settings_store
                .set_vsync(context.runtime.video_options.vsync)?;
            Ok(None)
        }
        MenuAction::CycleWindowScale => {
            context.runtime.video_options.window_scale =
                next_window_scale(context.runtime.video_options.window_scale);
            if canvas.window().fullscreen_state() == FullscreenType::Off {
                apply_window_scale_for_dimensions(
                    canvas.window_mut(),
                    context.runtime.video_options.window_scale,
                    framebuffer_dimensions_for_session(
                        context.machine,
                        &context.runtime.video_options,
                        context.session.has_loaded_rom(),
                    ),
                )?;
            }
            context
                .settings_store
                .set_window_scale(context.runtime.video_options.window_scale)?;
            Ok(None)
        }
        MenuAction::ToggleIntegerScale => {
            context.runtime.video_options.integer_scale =
                !context.runtime.video_options.integer_scale;
            context
                .settings_store
                .set_integer_scale(context.runtime.video_options.integer_scale)?;
            Ok(None)
        }
        MenuAction::TogglePresentationFilter => {
            context.runtime.video_options.presentation_filter =
                !context.runtime.video_options.presentation_filter;
            context
                .settings_store
                .set_presentation_filter(context.runtime.video_options.presentation_filter)?;
            Ok(None)
        }
        MenuAction::CycleFrameBlending => {
            context.runtime.video_options.frame_blending =
                context.runtime.video_options.frame_blending.next();
            context.runtime.frame_blending_state.reset();
            context
                .settings_store
                .set_frame_blending(context.runtime.video_options.frame_blending)?;
            Ok(None)
        }
        MenuAction::CycleDisplayPalette => {
            if context.session.config.launch.console_model == DesktopConsoleModel::GameBoyColor {
                return Ok(None);
            }
            context.runtime.video_options.display_palette =
                context.runtime.video_options.display_palette.next();
            context
                .settings_store
                .set_display_palette(context.runtime.video_options.display_palette)?;
            Ok(None)
        }
        MenuAction::ToggleSgbBorder => {
            if context
                .session
                .config
                .launch
                .console_model
                .sgb_profile()
                .is_none()
            {
                return Ok(None);
            }
            context.runtime.video_options.show_sgb_border =
                !context.runtime.video_options.show_sgb_border;
            context.runtime.frame_blending_state.reset();
            context
                .settings_store
                .set_show_sgb_border(context.runtime.video_options.show_sgb_border)?;
            Ok(None)
        }
        MenuAction::ToggleBackgroundLayer => {
            context.runtime.video_options.show_background =
                !context.runtime.video_options.show_background;
            context
                .settings_store
                .set_show_background(context.runtime.video_options.show_background)?;
            Ok(None)
        }
        MenuAction::ToggleWindowLayer => {
            context.runtime.video_options.show_window = !context.runtime.video_options.show_window;
            context
                .settings_store
                .set_show_window(context.runtime.video_options.show_window)?;
            Ok(None)
        }
        MenuAction::ToggleObjectLayer => {
            context.runtime.video_options.show_objects =
                !context.runtime.video_options.show_objects;
            context
                .settings_store
                .set_show_objects(context.runtime.video_options.show_objects)?;
            Ok(None)
        }
        MenuAction::TogglePerformanceHud => {
            context.runtime.video_options.show_performance_hud =
                !context.runtime.video_options.show_performance_hud;
            context
                .settings_store
                .set_show_performance_hud(context.runtime.video_options.show_performance_hud)?;
            Ok(None)
        }
        MenuAction::ToggleCgbInfraredHelper => {
            context.runtime.video_options.show_cgb_infrared_helper =
                !context.runtime.video_options.show_cgb_infrared_helper;
            context.settings_store.set_show_cgb_infrared_helper(
                context.runtime.video_options.show_cgb_infrared_helper,
            )?;
            Ok(None)
        }
        MenuAction::ResetVideoDefaults => {
            let defaults = VideoOptions::default_for_console_model(
                context.session.config.launch.console_model,
            );
            context.runtime.video_options = defaults.clone();
            context.runtime.frame_blending_state.reset();
            apply_renderer_vsync(canvas, context.frame_pacer, defaults.vsync)?;
            set_fullscreen_state(canvas.window_mut(), defaults.fullscreen)?;
            if canvas.window().fullscreen_state() == FullscreenType::Off {
                apply_window_scale_for_dimensions(
                    canvas.window_mut(),
                    defaults.window_scale,
                    framebuffer_dimensions_for_session(
                        context.machine,
                        &context.runtime.video_options,
                        context.session.has_loaded_rom(),
                    ),
                )?;
            }
            context
                .settings_store
                .reset_video_defaults(context.session.config.launch.console_model)?;
            Ok(None)
        }
        MenuAction::ToggleMute => {
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.set_muted(!audio_output.is_muted())?;
                context
                    .settings_store
                    .set_audio_muted(audio_output.is_muted())?;
            }
            Ok(None)
        }
        MenuAction::CycleAudioVolume => {
            context.runtime.audio_volume_percent =
                next_audio_volume_percent(context.runtime.audio_volume_percent);
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.set_volume_percent(context.runtime.audio_volume_percent)?;
            }
            context
                .settings_store
                .set_audio_volume_percent(context.runtime.audio_volume_percent)?;
            Ok(None)
        }
        MenuAction::ToggleAudioRecording => {
            if matches!(
                context.runtime.audio_recording_mode,
                DesktopAudioRecordingMode::Disabled
            ) {
                let recording_mode = DesktopAudioRecordingMode::Automatic;
                context.runtime.audio_recorder = create_audio_recorder(
                    &recording_mode,
                    context.runtime.audio_channel_mask,
                    context.session,
                    context.machine,
                )?;
                context.runtime.audio_recording_mode = recording_mode;
            } else {
                finish_audio_recorder(&mut context.runtime.audio_recorder)?;
                context.runtime.audio_recording_mode = DesktopAudioRecordingMode::Disabled;
            }
            Ok(None)
        }
        MenuAction::ToggleAudioChannel(channel) => {
            context.runtime.audio_channel_mask =
                context.runtime.audio_channel_mask.toggled(channel);
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.set_channel_mask(context.runtime.audio_channel_mask)?;
            }
            if let Some(audio_recorder) = &mut context.runtime.audio_recorder {
                audio_recorder.set_channel_mask(context.runtime.audio_channel_mask)?;
            }
            Ok(None)
        }
        MenuAction::ResetAudioDefaults => {
            let defaults = gb_desktop::AudioOptions::default();
            context.runtime.audio_volume_percent = defaults.volume_percent;
            if let Some(audio_output) = &mut context.runtime.audio_output {
                audio_output.set_muted(false)?;
                audio_output.set_volume_percent(defaults.volume_percent)?;
                audio_output.set_channel_mask(ApuRecordedChannelMask::ALL)?;
            }
            finish_audio_recorder(&mut context.runtime.audio_recorder)?;
            context.runtime.audio_recording_mode = DesktopAudioRecordingMode::Disabled;
            context.runtime.audio_channel_mask = ApuRecordedChannelMask::ALL;
            context.settings_store.reset_audio_defaults()?;
            Ok(None)
        }
        MenuAction::SetCgbInfraredNone => {
            if context.session.cgb_infrared_link_active
                || context.machine.is_linked_cgb_infrared_two_player()
                || context.session.pokemon_pikachu_color_active
                || context.machine.is_pokemon_pikachu_color()
                || context.session.pokemon_mystery_gift_active
                || context.machine.is_pokemon_mystery_gift()
            {
                deactivate_cgb_infrared_pair(canvas, context)?;
            }
            Ok(None)
        }
        MenuAction::SetCgbInfraredSameGame => {
            activate_cgb_infrared_same_game(event_pump, canvas, context)?;
            Ok(None)
        }
        MenuAction::SetCgbInfraredPikachuColor => {
            activate_pokemon_pikachu_color(canvas, context)?;
            Ok(None)
        }
        MenuAction::SetCgbInfraredMysteryGift => {
            activate_pokemon_mystery_gift(canvas, context)?;
            Ok(None)
        }
        MenuAction::CycleCgbInfraredPikachuGift => {
            context.session.pokemon_pikachu_color_gift =
                context.session.pokemon_pikachu_color_gift.next();
            context
                .machine
                .set_pokemon_pikachu_color_gift(context.session.pokemon_pikachu_color_gift);
            Ok(None)
        }
        MenuAction::CycleCgbInfraredMysteryGiftKind => {
            context.session.pokemon_mystery_gift_kind =
                context.session.pokemon_mystery_gift_kind.next();
            context
                .machine
                .set_pokemon_mystery_gift_kind(context.session.pokemon_mystery_gift_kind);
            Ok(None)
        }
        MenuAction::CycleCgbInfraredMysteryGiftCode => {
            context.session.pokemon_mystery_gift_code =
                context.session.pokemon_mystery_gift_code.next();
            context
                .machine
                .set_pokemon_mystery_gift_code(context.session.pokemon_mystery_gift_code);
            Ok(None)
        }
        MenuAction::SelectCgbInfraredSecondary => {
            if !context.session.has_loaded_rom()
                || context.session.config.launch.console_model != DesktopConsoleModel::GameBoyColor
            {
                return Ok(None);
            }

            context.runtime.open_rom_dialog_mode = OpenRomDialogMode::CgbInfraredSecondary;
            let default_location = context.session.rom_directory_hint();
            if let Err(error) = context.runtime.open_rom_dialog.show_file(
                &ROM_FILE_DIALOG_FILTERS,
                canvas.window(),
                default_location,
            ) {
                context.runtime.open_rom_dialog_mode = OpenRomDialogMode::Primary;
                show_warning_message(Some(canvas.window()), "CGB IR", &error);
                eprintln!("warning: {error}");
            }
            Ok(None)
        }
        MenuAction::SetExternalPort(selection) => {
            if selection != DesktopExternalPortSelection::None
                && !context
                    .session
                    .config
                    .launch
                    .console_model
                    .allows_ext_port_menu()
            {
                return Ok(None);
            }
            drain_printed_pages_into_printer_output(
                canvas.window(),
                context.session,
                context.runtime,
                context.machine,
            );
            flush_pending_printer_output(canvas.window(), context.session, context.runtime);
            match selection {
                DesktopExternalPortSelection::GameLink => {
                    open_game_link_secondary_rom_dialog(canvas, context);
                }
                DesktopExternalPortSelection::None | DesktopExternalPortSelection::Printer => {
                    if context.machine.is_linked_dmg04_two_player()
                        || context.machine.is_linked_dmg07()
                        || context.machine.is_linked_cgb_infrared_two_player()
                        || context.machine.is_pokemon_pikachu_color()
                        || context.machine.is_pokemon_mystery_gift()
                    {
                        close_runtime_save_sessions(context.runtime, context.machine)?;
                        context.machine.detach_to_single_primary();
                    }

                    context.session.linked_secondary_rom = None;
                    context.session.dmg07_player_count = None;
                    context.session.cgb_infrared_link_active = false;
                    context.session.pokemon_pikachu_color_active = false;
                    context.session.pokemon_mystery_gift_active = false;
                    context.session.external_port_selection = selection;
                    apply_external_port_selection_to_machine(
                        context.machine.primary_machine_mut(),
                        selection,
                    );
                    context.runtime.save_sessions =
                        open_save_sessions_for_session(context.session, context.machine)?;
                    reset_frontend_timeline_state(context.runtime);
                    context.performance_counter.reset_base_title(
                        canvas.window_mut(),
                        window_title(context.session, &context.session.config),
                    )?;
                    context.runtime.rtc_sync.resync_to_host_clock();
                }
                DesktopExternalPortSelection::FourPlayerAdapter => {}
            }
            Ok(None)
        }
        MenuAction::SetGameLinkSameGame => {
            if !context
                .session
                .config
                .launch
                .console_model
                .allows_ext_port_menu()
            {
                return Ok(None);
            }
            let Some(next_secondary_rom) = context.session.loaded_rom.clone() else {
                return Ok(None);
            };
            activate_game_link_with_secondary_rom(event_pump, canvas, next_secondary_rom, context)?;
            Ok(None)
        }
        MenuAction::SelectGameLinkRom => {
            if !context
                .session
                .config
                .launch
                .console_model
                .allows_ext_port_menu()
            {
                return Ok(None);
            }
            drain_printed_pages_into_printer_output(
                canvas.window(),
                context.session,
                context.runtime,
                context.machine,
            );
            flush_pending_printer_output(canvas.window(), context.session, context.runtime);
            open_game_link_secondary_rom_dialog(canvas, context);
            Ok(None)
        }
        MenuAction::SetFourPlayerAdapter(player_count) => {
            if !context
                .session
                .config
                .launch
                .console_model
                .allows_ext_port_menu()
            {
                return Ok(None);
            }
            activate_dmg07_adapter(event_pump, canvas, player_count, context)?;
            Ok(None)
        }
        MenuAction::CycleGamepadDirectionalSource => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let next_directional_source =
                    next_gamepad_directional_source(gamepad_manager.directional_source());
                gamepad_manager.set_directional_source(
                    next_directional_source,
                    context.runtime.player_inputs.input_mut(PlayerSlot::P1),
                    context
                        .machine
                        .machine_for_player_slot_mut(PlayerSlot::P1)
                        .expect("P1 should always map to an active desktop machine"),
                );
                clear_live_input_state(context.machine, context.runtime);
                context
                    .settings_store
                    .set_gamepad_directional_source(next_directional_source)?;
            }
            Ok(None)
        }
        MenuAction::CycleGamepadRumbleMode => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let next_rumble_mode = next_gamepad_rumble_mode(gamepad_manager.rumble_mode());
                gamepad_manager.set_rumble_mode(next_rumble_mode);
                context
                    .settings_store
                    .set_gamepad_rumble_mode(next_rumble_mode)?;
                sync_gamepad_rumble(context.runtime, context.machine, Instant::now())?;
            }
            Ok(None)
        }
        MenuAction::CycleGamepadGyroMode => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let next_gyro_mode = next_gamepad_gyro_mode(gamepad_manager.gyro_mode());
                gamepad_manager.set_gyro_mode(
                    next_gyro_mode,
                    context
                        .machine
                        .machine_for_player_slot_mut(PlayerSlot::P1)
                        .expect("P1 should always map to an active desktop machine"),
                )?;
                context
                    .settings_store
                    .set_gamepad_gyro_mode(next_gyro_mode)?;
            }
            Ok(None)
        }
        MenuAction::TogglePreferredGamepad => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let preferred_device = toggled_preferred_gamepad_device(gamepad_manager);
                gamepad_manager.set_preferred_device(
                    preferred_device.clone(),
                    context.runtime.player_inputs.input_mut(PlayerSlot::P1),
                    context
                        .machine
                        .machine_for_player_slot_mut(PlayerSlot::P1)
                        .expect("P1 should always map to an active desktop machine"),
                );
                context
                    .settings_store
                    .set_preferred_gamepad_device(preferred_device)?;
            }
            Ok(None)
        }
        MenuAction::SetKeyboardBinding(target, key) => {
            assign_keyboard_binding(&mut context.runtime.keyboard_bindings, target, key);
            match target {
                KeyboardBindingTarget::Up
                | KeyboardBindingTarget::Down
                | KeyboardBindingTarget::Left
                | KeyboardBindingTarget::Right
                | KeyboardBindingTarget::A
                | KeyboardBindingTarget::B
                | KeyboardBindingTarget::Select
                | KeyboardBindingTarget::Start => {
                    context
                        .settings_store
                        .set_keyboard_joypad_bindings(context.runtime.keyboard_bindings.joypad)?;
                }
                KeyboardBindingTarget::Pause
                | KeyboardBindingTarget::SaveState
                | KeyboardBindingTarget::LoadState
                | KeyboardBindingTarget::StateSlot1
                | KeyboardBindingTarget::StateSlot2
                | KeyboardBindingTarget::StateSlot3
                | KeyboardBindingTarget::StateSlot4
                | KeyboardBindingTarget::Reset
                | KeyboardBindingTarget::Rewind
                | KeyboardBindingTarget::FastForward
                | KeyboardBindingTarget::ToggleFullscreen
                | KeyboardBindingTarget::TogglePerformanceHud
                | KeyboardBindingTarget::SaveBattery => {
                    context
                        .settings_store
                        .set_keyboard_hotkey_bindings(context.runtime.keyboard_bindings.hotkeys)?;
                }
            }
            Ok(None)
        }
        MenuAction::ResetInputDefaults => {
            let defaults = gb_desktop::InputOptions::default();
            context.runtime.keyboard_bindings = defaults.keyboard;
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                gamepad_manager.set_button_bindings(
                    defaults.gamepad.bindings,
                    context.runtime.player_inputs.input_mut(PlayerSlot::P1),
                    context
                        .machine
                        .machine_for_player_slot_mut(PlayerSlot::P1)
                        .expect("P1 should always map to an active desktop machine"),
                );
                gamepad_manager.set_action_bindings(defaults.gamepad.actions);
                gamepad_manager.set_menu_bindings(defaults.gamepad.menu);
                gamepad_manager.set_directional_source(
                    defaults.gamepad.directional_source,
                    context.runtime.player_inputs.input_mut(PlayerSlot::P1),
                    context
                        .machine
                        .machine_for_player_slot_mut(PlayerSlot::P1)
                        .expect("P1 should always map to an active desktop machine"),
                );
                gamepad_manager.set_rumble_mode(defaults.gamepad.rumble_mode);
                gamepad_manager.set_gyro_mode(
                    defaults.gamepad.gyro_mode,
                    context
                        .machine
                        .machine_for_player_slot_mut(PlayerSlot::P1)
                        .expect("P1 should always map to an active desktop machine"),
                )?;
                gamepad_manager.set_preferred_device(
                    defaults.gamepad.preferred_device,
                    context.runtime.player_inputs.input_mut(PlayerSlot::P1),
                    context
                        .machine
                        .machine_for_player_slot_mut(PlayerSlot::P1)
                        .expect("P1 should always map to an active desktop machine"),
                );
            }
            context.settings_store.reset_input_defaults()?;
            sync_gamepad_rumble(context.runtime, context.machine, Instant::now())?;
            Ok(None)
        }
        MenuAction::SetKeyboardMenuBinding(target, key) => {
            assign_keyboard_menu_binding(&mut context.runtime.keyboard_bindings.menu, target, key);
            context
                .settings_store
                .set_keyboard_menu_bindings(context.runtime.keyboard_bindings.menu)?;
            Ok(None)
        }
        MenuAction::SetGamepadBinding(target, binding) => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let mut bindings = gamepad_manager.button_bindings();
                assign_gamepad_binding(&mut bindings, target, binding);
                gamepad_manager.set_button_bindings(
                    bindings,
                    context.runtime.player_inputs.input_mut(PlayerSlot::P1),
                    context
                        .machine
                        .machine_for_player_slot_mut(PlayerSlot::P1)
                        .expect("P1 should always map to an active desktop machine"),
                );
                context.settings_store.set_gamepad_bindings(bindings)?;
            }
            Ok(None)
        }
        MenuAction::SetGamepadActionBinding(target, binding) => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let mut bindings = gamepad_manager.action_bindings();
                assign_gamepad_action_binding(&mut bindings, target, binding);
                gamepad_manager.set_action_bindings(bindings);
                context
                    .settings_store
                    .set_gamepad_action_bindings(bindings)?;
            }
            Ok(None)
        }
        MenuAction::SetGamepadMenuBinding(target, binding) => {
            if let Some(gamepad_manager) = &mut context.runtime.gamepad_manager {
                let mut bindings = gamepad_manager.menu_bindings();
                assign_gamepad_menu_binding(&mut bindings, target, binding);
                gamepad_manager.set_menu_bindings(bindings);
                context.settings_store.set_gamepad_menu_bindings(bindings)?;
            }
            Ok(None)
        }
        MenuAction::Reset => {
            reset_machine(
                canvas.window(),
                context.session,
                context.machine,
                context.runtime,
                context.settings_store,
            )?;
            close_menu(event_pump, context.machine, context.runtime)?;
            Ok(None)
        }
        MenuAction::Quit => {
            flush_pending_printer_output(canvas.window(), context.session, context.runtime);
            Ok(Some(LoopSignal::Quit))
        }
    }
}

fn current_menu_presentation(
    window: &Window,
    runtime: &FrontendRuntime,
    machine: &DesktopEmulationSession,
    session: &DesktopSession,
) -> MenuPresentation {
    let gamepad_available = runtime.gamepad_manager.is_some();
    let active_gamepad_label = runtime
        .gamepad_manager
        .as_ref()
        .and_then(GamepadManager::active_gamepad_name)
        .map(CompactMenuLabel::from_gamepad_name)
        .unwrap_or_default();
    let cartridge_mbc7_accelerometer_supported = machine.primary_machine().has_mbc7_accelerometer();
    let cartridge_rumble_supported = machine.primary_machine().cartridge().has_rumble();
    let external_save_available = !machine.cartridge().is_empty()
        && uses_battery_backed_hardware_persistence(
            machine.primary_machine().cartridge().persistence_metadata(),
        );
    let machine_state_available = machine_state_actions_available(session, machine);
    let machine_state_load_available =
        machine_state_slot_load_available(session, machine, runtime.machine_state_slot);
    let rewind_supported = rewind_session_supported(session, machine);
    let rewind_available =
        rewind_actions_available(session, machine) && !runtime.rewind_buffer.is_empty();
    let cartridge_pocket_camera_supported = session_has_pocket_camera(machine);
    let preferred_gamepad_configured = runtime
        .gamepad_manager
        .as_ref()
        .is_some_and(|manager| manager.preferred_device().is_configured());
    let preferred_gamepad_label = runtime
        .gamepad_manager
        .as_ref()
        .and_then(GamepadManager::preferred_device_name)
        .map(CompactMenuLabel::from_gamepad_name)
        .unwrap_or(if preferred_gamepad_configured {
            CompactMenuLabel::from_text("SAVED")
        } else {
            CompactMenuLabel::default()
        });
    let mut recent_rom_labels = [CompactRecentRomLabel::default(); RECENT_ROM_MENU_CAPACITY];
    for (slot, rom_path) in session
        .recent_roms()
        .iter()
        .take(RECENT_ROM_MENU_CAPACITY)
        .enumerate()
    {
        recent_rom_labels[slot] = compact_recent_rom_label(rom_path);
    }

    MenuPresentation {
        rom_loaded: !machine.cartridge().is_empty(),
        recent_rom_count: session.recent_roms().len().min(RECENT_ROM_MENU_CAPACITY) as u8,
        recent_rom_labels,
        console_model: session.config.launch.console_model,
        revision: session.config.launch.effective_revision(),
        sgb_video_standard: session.config.launch.effective_sgb_video_standard(),
        startup_mode: session.config.launch.startup_mode,
        execution_mode: session.config.launch.execution_mode,
        external_port_selection: session.external_port_selection,
        cgb_infrared_link_active: session.cgb_infrared_link_active(),
        cgb_infrared_same_game_active: cgb_infrared_same_game_active(session),
        pokemon_pikachu_color_active: session.pokemon_pikachu_color_active(),
        pokemon_pikachu_color_gift: session.pokemon_pikachu_color_gift,
        pokemon_mystery_gift_active: session.pokemon_mystery_gift_active(),
        pokemon_mystery_gift_kind: session.pokemon_mystery_gift_kind,
        pokemon_mystery_gift_code: session.pokemon_mystery_gift_code,
        boot_rom_verification: session.config.boot_rom.verification,
        saves_enabled: session.config.saves.enabled,
        save_flush_policy: session.config.saves.flush_policy,
        save_directory_uses_default_path: match &session.config.saves.directory_policy {
            SaveDirectoryPolicy::RomFolderSavesSubdir => true,
            SaveDirectoryPolicy::Custom(_) => false,
        },
        fullscreen: window.fullscreen_state() != FullscreenType::Off,
        vsync: runtime.video_options.vsync,
        window_scale: runtime.video_options.window_scale.max(1),
        integer_scale: runtime.video_options.integer_scale,
        presentation_filter: runtime.video_options.presentation_filter,
        frame_blending: runtime.video_options.frame_blending,
        display_palette: runtime.video_options.display_palette,
        show_background: runtime.video_options.show_background,
        show_window: runtime.video_options.show_window,
        show_objects: runtime.video_options.show_objects,
        show_sgb_border: runtime.video_options.show_sgb_border,
        show_performance_hud: runtime.video_options.show_performance_hud,
        show_cgb_infrared_helper: runtime.video_options.show_cgb_infrared_helper,
        muted: runtime
            .audio_output
            .as_ref()
            .is_some_and(DesktopAudioOutput::is_muted),
        audio_available: runtime.audio_output.is_some(),
        audio_volume_percent: runtime.audio_volume_percent.min(100),
        audio_recording_enabled: !matches!(
            runtime.audio_recording_mode,
            DesktopAudioRecordingMode::Disabled
        ),
        ch1_enabled: runtime.audio_channel_mask.contains(ApuRecordedChannel::Ch1),
        ch2_enabled: runtime.audio_channel_mask.contains(ApuRecordedChannel::Ch2),
        ch3_enabled: runtime.audio_channel_mask.contains(ApuRecordedChannel::Ch3),
        ch4_enabled: runtime.audio_channel_mask.contains(ApuRecordedChannel::Ch4),
        manual_save_available: runtime
            .save_sessions
            .iter()
            .flatten()
            .any(|session| session.flush_policy() == DesktopSaveFlushPolicy::Manual),
        external_save_available,
        external_save_import_available: external_save_available && session.config.saves.enabled,
        machine_state_available,
        machine_state_load_available,
        machine_state_slot: runtime.machine_state_slot,
        machine_state_autoload_slot: session
            .config
            .machine_state
            .normalized_autoload_slot(MACHINE_STATE_SLOT_COUNT),
        rewind_supported,
        rewind_options: session.config.rewind,
        fast_forward_options: session.config.fast_forward,
        rewind_available,
        any_dialog_pending: runtime.any_dialog_pending(),
        cartridge_pocket_camera_supported,
        pocket_camera_live_enabled: runtime.pocket_camera_live.is_enabled(),
        gamepad_available,
        gamepad_directional_source: runtime.gamepad_manager.as_ref().map_or(
            GamepadDirectionalSource::default(),
            GamepadManager::directional_source,
        ),
        gamepad_gyro_mode: runtime
            .gamepad_manager
            .as_ref()
            .map_or(GamepadGyroMode::default(), GamepadManager::gyro_mode),
        gamepad_rumble_mode: runtime
            .gamepad_manager
            .as_ref()
            .map_or(GamepadRumbleMode::default(), GamepadManager::rumble_mode),
        gamepad_bindings: runtime.gamepad_manager.as_ref().map_or(
            GamepadButtonBindings::default(),
            GamepadManager::button_bindings,
        ),
        gamepad_action_bindings: runtime.gamepad_manager.as_ref().map_or(
            GamepadActionBindings::default(),
            GamepadManager::action_bindings,
        ),
        gamepad_menu_bindings: runtime.gamepad_manager.as_ref().map_or(
            GamepadMenuBindings::default(),
            GamepadManager::menu_bindings,
        ),
        active_gamepad_connected: runtime
            .gamepad_manager
            .as_ref()
            .is_some_and(GamepadManager::has_connected_gamepad),
        cartridge_mbc7_accelerometer_supported,
        cartridge_rumble_supported,
        active_gamepad_accelerometer_supported: runtime
            .gamepad_manager
            .as_ref()
            .is_some_and(GamepadManager::active_gamepad_has_accelerometer),
        active_gamepad_rumble_supported: runtime
            .gamepad_manager
            .as_ref()
            .is_some_and(GamepadManager::active_gamepad_has_rumble),
        active_gamepad_label,
        preferred_gamepad_configured,
        preferred_gamepad_label,
        keyboard_bindings: runtime.keyboard_bindings.joypad,
        keyboard_menu_bindings: runtime.keyboard_bindings.menu,
        hotkey_bindings: runtime.keyboard_bindings.hotkeys,
    }
}

fn next_console_model(console_model: DesktopConsoleModel) -> DesktopConsoleModel {
    match console_model {
        DesktopConsoleModel::GameBoy => DesktopConsoleModel::GameBoyPocket,
        DesktopConsoleModel::GameBoyPocket => DesktopConsoleModel::GameBoyLight,
        DesktopConsoleModel::GameBoyLight => DesktopConsoleModel::GameBoyColor,
        DesktopConsoleModel::GameBoyColor => DesktopConsoleModel::GameBoyAdvance,
        DesktopConsoleModel::GameBoyAdvance => DesktopConsoleModel::SuperGameBoy,
        DesktopConsoleModel::SuperGameBoy => DesktopConsoleModel::SuperGameBoy2,
        DesktopConsoleModel::SuperGameBoy2 => DesktopConsoleModel::GameBoy,
    }
}

fn next_revision(
    console_model: DesktopConsoleModel,
    revision: HardwareRevision,
) -> HardwareRevision {
    let active = console_model.console_model().active_revisions();
    let current_index = active
        .iter()
        .position(|candidate| *candidate == revision)
        .unwrap_or(0);
    active[(current_index + 1) % active.len()]
}

fn next_sgb_video_standard(video_standard: SgbVideoStandard) -> SgbVideoStandard {
    match video_standard {
        SgbVideoStandard::Ntsc => SgbVideoStandard::Pal,
        SgbVideoStandard::Pal => SgbVideoStandard::Ntsc,
    }
}

fn next_startup_mode(startup_mode: StartupMode) -> StartupMode {
    match startup_mode {
        StartupMode::SkipBoot => StartupMode::CustomBoot,
        StartupMode::CustomBoot => StartupMode::RealBoot,
        StartupMode::RealBoot => StartupMode::SkipBoot,
    }
}

fn next_execution_mode(execution_mode: ExecutionMode) -> ExecutionMode {
    match execution_mode {
        ExecutionMode::Strict => ExecutionMode::Permissive,
        ExecutionMode::Permissive => ExecutionMode::Experimental,
        ExecutionMode::Experimental => ExecutionMode::Strict,
    }
}

fn next_boot_rom_verification_mode(
    verification_mode: BootRomVerificationMode,
) -> BootRomVerificationMode {
    match verification_mode {
        BootRomVerificationMode::Strict => BootRomVerificationMode::Warn,
        BootRomVerificationMode::Warn => BootRomVerificationMode::Off,
        BootRomVerificationMode::Off => BootRomVerificationMode::Strict,
    }
}

fn next_save_flush_policy(flush_policy: DesktopSaveFlushPolicy) -> DesktopSaveFlushPolicy {
    match flush_policy {
        DesktopSaveFlushPolicy::Manual => DesktopSaveFlushPolicy::OnClose,
        DesktopSaveFlushPolicy::OnClose => DesktopSaveFlushPolicy::OnWrite,
        DesktopSaveFlushPolicy::OnWrite => DesktopSaveFlushPolicy::Debounced,
        DesktopSaveFlushPolicy::Debounced => DesktopSaveFlushPolicy::Manual,
    }
}

fn next_rewind_history_seconds(current: u16) -> u16 {
    next_rewind_option(current, &REWIND_HISTORY_SECONDS_OPTIONS)
}

fn next_rewind_subframes_per_frame(current: u8) -> u8 {
    next_rewind_option(current, &REWIND_SUBFRAMES_PER_FRAME_OPTIONS)
}

fn next_rewind_speed_multiplier(current: u8) -> u8 {
    next_rewind_option(current, &REWIND_SPEED_MULTIPLIER_OPTIONS)
}

fn next_rewind_max_memory_mib(current: u16) -> u16 {
    next_rewind_option(current, &REWIND_MAX_MEMORY_MIB_OPTIONS)
}

fn next_fast_forward_speed_multiplier(current: u8) -> u8 {
    next_rewind_option(current, &FAST_FORWARD_SPEED_MULTIPLIER_OPTIONS)
}

fn next_rewind_option<T>(current: T, options: &[T]) -> T
where
    T: Copy + Ord,
{
    options
        .iter()
        .copied()
        .find(|option| *option > current)
        .unwrap_or_else(|| {
            options
                .first()
                .copied()
                .expect("rewind option tables must not be empty")
        })
}

fn next_gamepad_directional_source(
    directional_source: GamepadDirectionalSource,
) -> GamepadDirectionalSource {
    match directional_source {
        GamepadDirectionalSource::DpadOnly => GamepadDirectionalSource::LeftStickOnly,
        GamepadDirectionalSource::LeftStickOnly => GamepadDirectionalSource::DpadAndLeftStick,
        GamepadDirectionalSource::DpadAndLeftStick => GamepadDirectionalSource::DpadOnly,
    }
}

fn next_gamepad_gyro_mode(gyro_mode: GamepadGyroMode) -> GamepadGyroMode {
    match gyro_mode {
        GamepadGyroMode::Off => GamepadGyroMode::PadGyro,
        GamepadGyroMode::PadGyro => GamepadGyroMode::PadInput,
        GamepadGyroMode::PadInput => GamepadGyroMode::Off,
    }
}

fn next_gamepad_rumble_mode(rumble_mode: GamepadRumbleMode) -> GamepadRumbleMode {
    match rumble_mode {
        GamepadRumbleMode::Off => GamepadRumbleMode::Strong,
        GamepadRumbleMode::Strong => GamepadRumbleMode::Weak,
        GamepadRumbleMode::Weak => GamepadRumbleMode::Off,
    }
}

fn next_window_scale(current_scale: u8) -> u8 {
    match current_scale {
        1..=7 => current_scale + 1,
        _ => 1,
    }
}

fn next_audio_volume_percent(current_volume_percent: u8) -> u8 {
    match current_volume_percent {
        0..=24 => 25,
        25..=49 => 50,
        50..=74 => 75,
        75..=99 => 100,
        _ => 25,
    }
}

fn compact_recent_rom_label(path: &Path) -> CompactRecentRomLabel {
    let stem = path
        .file_stem()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let trimmed = stem
        .split(['(', '['])
        .next()
        .unwrap_or(stem.as_ref())
        .trim();
    let mut compact = String::new();
    let mut pending_space = false;
    for character in trimmed.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_space && !compact.is_empty() {
                compact.push(' ');
            }
            compact.push(character.to_ascii_uppercase());
            pending_space = false;
        } else if !compact.is_empty() {
            pending_space = true;
        }
    }

    if compact.is_empty() {
        CompactRecentRomLabel::from_text("ROM")
    } else {
        CompactRecentRomLabel::from_text(&compact)
    }
}

fn toggled_preferred_gamepad_device(gamepad_manager: &GamepadManager) -> PreferredGamepadIdentity {
    if gamepad_manager.preferred_device().is_configured()
        && !gamepad_manager.has_connected_gamepad()
    {
        return PreferredGamepadIdentity::default();
    }

    if gamepad_manager.active_matches_preferred() {
        return PreferredGamepadIdentity::default();
    }

    gamepad_manager
        .active_gamepad_identity()
        .unwrap_or_default()
}

fn assign_keyboard_binding(
    bindings: &mut KeyboardBindings,
    target: KeyboardBindingTarget,
    key: DesktopKey,
) {
    let previous_key = keyboard_binding_value(*bindings, target);
    if previous_key == key {
        return;
    }

    let other_target = match target {
        KeyboardBindingTarget::Up
        | KeyboardBindingTarget::Down
        | KeyboardBindingTarget::Left
        | KeyboardBindingTarget::Right
        | KeyboardBindingTarget::A
        | KeyboardBindingTarget::B
        | KeyboardBindingTarget::Select
        | KeyboardBindingTarget::Start => joypad_binding_target_for_key(bindings.joypad, key),
        KeyboardBindingTarget::Pause
        | KeyboardBindingTarget::SaveState
        | KeyboardBindingTarget::LoadState
        | KeyboardBindingTarget::StateSlot1
        | KeyboardBindingTarget::StateSlot2
        | KeyboardBindingTarget::StateSlot3
        | KeyboardBindingTarget::StateSlot4
        | KeyboardBindingTarget::Reset
        | KeyboardBindingTarget::Rewind
        | KeyboardBindingTarget::FastForward
        | KeyboardBindingTarget::ToggleFullscreen
        | KeyboardBindingTarget::TogglePerformanceHud
        | KeyboardBindingTarget::SaveBattery => {
            hotkey_binding_target_for_key(bindings.hotkeys, key)
        }
    };

    if let Some(other_target) = other_target {
        set_keyboard_binding_value(bindings, other_target, previous_key);
    }
    set_keyboard_binding_value(bindings, target, key);
}

fn assign_keyboard_menu_binding(
    bindings: &mut MenuKeyboardBindings,
    target: KeyboardMenuBindingTarget,
    key: DesktopKey,
) {
    let previous_key = keyboard_menu_binding_value(*bindings, target);
    if previous_key == key {
        return;
    }

    if let Some(other_target) = keyboard_menu_binding_target_for_key(*bindings, key) {
        set_keyboard_menu_binding_value(bindings, other_target, previous_key);
    }
    set_keyboard_menu_binding_value(bindings, target, key);
}

fn assign_gamepad_binding(
    bindings: &mut GamepadButtonBindings,
    target: GamepadBindingTarget,
    binding: GamepadButtonBinding,
) {
    let previous_binding = gamepad_binding_value(*bindings, target);
    if previous_binding == binding {
        return;
    }

    if let Some(other_target) = gamepad_binding_target_for_binding(*bindings, binding) {
        set_gamepad_binding_value(bindings, other_target, previous_binding);
    }
    set_gamepad_binding_value(bindings, target, binding);
}

fn assign_gamepad_action_binding(
    bindings: &mut GamepadActionBindings,
    target: GamepadActionBindingTarget,
    binding: GamepadButtonBinding,
) {
    let previous_binding = gamepad_action_binding_value(*bindings, target);
    if previous_binding == Some(binding) {
        return;
    }

    if let Some(other_target) = gamepad_action_binding_target_for_binding(*bindings, binding) {
        set_gamepad_action_binding_value(bindings, other_target, previous_binding);
    }
    set_gamepad_action_binding_value(bindings, target, Some(binding));
}

fn assign_gamepad_menu_binding(
    bindings: &mut GamepadMenuBindings,
    target: GamepadMenuBindingTarget,
    binding: GamepadButtonBinding,
) {
    let previous_binding = gamepad_menu_binding_value(*bindings, target);
    if previous_binding == binding {
        return;
    }

    if let Some(other_target) = gamepad_menu_binding_target_for_binding(*bindings, binding) {
        set_gamepad_menu_binding_value(bindings, other_target, previous_binding);
    }
    set_gamepad_menu_binding_value(bindings, target, binding);
}

fn gamepad_binding_target_for_binding(
    bindings: GamepadButtonBindings,
    binding: GamepadButtonBinding,
) -> Option<GamepadBindingTarget> {
    [
        GamepadBindingTarget::Up,
        GamepadBindingTarget::Down,
        GamepadBindingTarget::Left,
        GamepadBindingTarget::Right,
        GamepadBindingTarget::A,
        GamepadBindingTarget::B,
        GamepadBindingTarget::Select,
        GamepadBindingTarget::Start,
    ]
    .into_iter()
    .find(|target| gamepad_binding_value(bindings, *target) == binding)
}

fn gamepad_menu_binding_target_for_binding(
    bindings: GamepadMenuBindings,
    binding: GamepadButtonBinding,
) -> Option<GamepadMenuBindingTarget> {
    [
        GamepadMenuBindingTarget::Up,
        GamepadMenuBindingTarget::Down,
        GamepadMenuBindingTarget::Confirm,
        GamepadMenuBindingTarget::Cancel,
    ]
    .into_iter()
    .find(|target| gamepad_menu_binding_value(bindings, *target) == binding)
}

fn gamepad_action_binding_target_for_binding(
    bindings: GamepadActionBindings,
    binding: GamepadButtonBinding,
) -> Option<GamepadActionBindingTarget> {
    [
        GamepadActionBindingTarget::SaveState,
        GamepadActionBindingTarget::LoadState,
        GamepadActionBindingTarget::Rewind,
        GamepadActionBindingTarget::FastForward,
    ]
    .into_iter()
    .find(|target| gamepad_action_binding_value(bindings, *target) == Some(binding))
}

fn gamepad_binding_value(
    bindings: GamepadButtonBindings,
    target: GamepadBindingTarget,
) -> GamepadButtonBinding {
    match target {
        GamepadBindingTarget::Up => bindings.up,
        GamepadBindingTarget::Down => bindings.down,
        GamepadBindingTarget::Left => bindings.left,
        GamepadBindingTarget::Right => bindings.right,
        GamepadBindingTarget::A => bindings.a,
        GamepadBindingTarget::B => bindings.b,
        GamepadBindingTarget::Select => bindings.select,
        GamepadBindingTarget::Start => bindings.start,
    }
}

fn gamepad_action_binding_value(
    bindings: GamepadActionBindings,
    target: GamepadActionBindingTarget,
) -> Option<GamepadButtonBinding> {
    match target {
        GamepadActionBindingTarget::SaveState => bindings.save_state,
        GamepadActionBindingTarget::LoadState => bindings.load_state,
        GamepadActionBindingTarget::Rewind => bindings.rewind,
        GamepadActionBindingTarget::FastForward => bindings.fast_forward,
    }
}

fn gamepad_menu_binding_value(
    bindings: GamepadMenuBindings,
    target: GamepadMenuBindingTarget,
) -> GamepadButtonBinding {
    match target {
        GamepadMenuBindingTarget::Up => bindings.up,
        GamepadMenuBindingTarget::Down => bindings.down,
        GamepadMenuBindingTarget::Confirm => bindings.confirm,
        GamepadMenuBindingTarget::Cancel => bindings.cancel,
    }
}

fn set_gamepad_binding_value(
    bindings: &mut GamepadButtonBindings,
    target: GamepadBindingTarget,
    binding: GamepadButtonBinding,
) {
    match target {
        GamepadBindingTarget::Up => bindings.up = binding,
        GamepadBindingTarget::Down => bindings.down = binding,
        GamepadBindingTarget::Left => bindings.left = binding,
        GamepadBindingTarget::Right => bindings.right = binding,
        GamepadBindingTarget::A => bindings.a = binding,
        GamepadBindingTarget::B => bindings.b = binding,
        GamepadBindingTarget::Select => bindings.select = binding,
        GamepadBindingTarget::Start => bindings.start = binding,
    }
}

fn set_gamepad_action_binding_value(
    bindings: &mut GamepadActionBindings,
    target: GamepadActionBindingTarget,
    binding: Option<GamepadButtonBinding>,
) {
    match target {
        GamepadActionBindingTarget::SaveState => bindings.save_state = binding,
        GamepadActionBindingTarget::LoadState => bindings.load_state = binding,
        GamepadActionBindingTarget::Rewind => bindings.rewind = binding,
        GamepadActionBindingTarget::FastForward => bindings.fast_forward = binding,
    }
}

fn set_gamepad_menu_binding_value(
    bindings: &mut GamepadMenuBindings,
    target: GamepadMenuBindingTarget,
    binding: GamepadButtonBinding,
) {
    match target {
        GamepadMenuBindingTarget::Up => bindings.up = binding,
        GamepadMenuBindingTarget::Down => bindings.down = binding,
        GamepadMenuBindingTarget::Confirm => bindings.confirm = binding,
        GamepadMenuBindingTarget::Cancel => bindings.cancel = binding,
    }
}

fn joypad_binding_target_for_key(
    bindings: JoypadKeyboardBindings,
    key: DesktopKey,
) -> Option<KeyboardBindingTarget> {
    [
        KeyboardBindingTarget::Up,
        KeyboardBindingTarget::Down,
        KeyboardBindingTarget::Left,
        KeyboardBindingTarget::Right,
        KeyboardBindingTarget::A,
        KeyboardBindingTarget::B,
        KeyboardBindingTarget::Select,
        KeyboardBindingTarget::Start,
    ]
    .into_iter()
    .find(|target| {
        keyboard_binding_value(
            KeyboardBindings {
                joypad: bindings,
                ..KeyboardBindings::default()
            },
            *target,
        ) == key
    })
}

fn keyboard_menu_binding_target_for_key(
    bindings: MenuKeyboardBindings,
    key: DesktopKey,
) -> Option<KeyboardMenuBindingTarget> {
    [
        KeyboardMenuBindingTarget::Up,
        KeyboardMenuBindingTarget::Down,
        KeyboardMenuBindingTarget::Confirm,
        KeyboardMenuBindingTarget::Cancel,
    ]
    .into_iter()
    .find(|target| keyboard_menu_binding_value(bindings, *target) == key)
}

fn hotkey_binding_target_for_key(
    bindings: HotkeyBindings,
    key: DesktopKey,
) -> Option<KeyboardBindingTarget> {
    [
        KeyboardBindingTarget::Pause,
        KeyboardBindingTarget::SaveState,
        KeyboardBindingTarget::LoadState,
        KeyboardBindingTarget::StateSlot1,
        KeyboardBindingTarget::StateSlot2,
        KeyboardBindingTarget::StateSlot3,
        KeyboardBindingTarget::StateSlot4,
        KeyboardBindingTarget::Reset,
        KeyboardBindingTarget::Rewind,
        KeyboardBindingTarget::FastForward,
        KeyboardBindingTarget::ToggleFullscreen,
        KeyboardBindingTarget::TogglePerformanceHud,
        KeyboardBindingTarget::SaveBattery,
    ]
    .into_iter()
    .find(|target| {
        keyboard_binding_value(
            KeyboardBindings {
                hotkeys: bindings,
                ..KeyboardBindings::default()
            },
            *target,
        ) == key
    })
}

fn keyboard_menu_binding_value(
    bindings: MenuKeyboardBindings,
    target: KeyboardMenuBindingTarget,
) -> DesktopKey {
    match target {
        KeyboardMenuBindingTarget::Up => bindings.up,
        KeyboardMenuBindingTarget::Down => bindings.down,
        KeyboardMenuBindingTarget::Confirm => bindings.confirm,
        KeyboardMenuBindingTarget::Cancel => bindings.cancel,
    }
}

fn keyboard_binding_value(bindings: KeyboardBindings, target: KeyboardBindingTarget) -> DesktopKey {
    match target {
        KeyboardBindingTarget::Up => bindings.joypad.up,
        KeyboardBindingTarget::Down => bindings.joypad.down,
        KeyboardBindingTarget::Left => bindings.joypad.left,
        KeyboardBindingTarget::Right => bindings.joypad.right,
        KeyboardBindingTarget::A => bindings.joypad.a,
        KeyboardBindingTarget::B => bindings.joypad.b,
        KeyboardBindingTarget::Select => bindings.joypad.select,
        KeyboardBindingTarget::Start => bindings.joypad.start,
        KeyboardBindingTarget::Pause => bindings.hotkeys.pause,
        KeyboardBindingTarget::SaveState => bindings.hotkeys.save_state,
        KeyboardBindingTarget::LoadState => bindings.hotkeys.load_state,
        KeyboardBindingTarget::StateSlot1 => bindings.hotkeys.state_slot_1,
        KeyboardBindingTarget::StateSlot2 => bindings.hotkeys.state_slot_2,
        KeyboardBindingTarget::StateSlot3 => bindings.hotkeys.state_slot_3,
        KeyboardBindingTarget::StateSlot4 => bindings.hotkeys.state_slot_4,
        KeyboardBindingTarget::Reset => bindings.hotkeys.reset,
        KeyboardBindingTarget::Rewind => bindings.hotkeys.rewind,
        KeyboardBindingTarget::FastForward => bindings.hotkeys.fast_forward,
        KeyboardBindingTarget::ToggleFullscreen => bindings.hotkeys.toggle_fullscreen,
        KeyboardBindingTarget::TogglePerformanceHud => bindings.hotkeys.toggle_performance_hud,
        KeyboardBindingTarget::SaveBattery => bindings.hotkeys.save_battery,
    }
}

fn set_keyboard_menu_binding_value(
    bindings: &mut MenuKeyboardBindings,
    target: KeyboardMenuBindingTarget,
    key: DesktopKey,
) {
    match target {
        KeyboardMenuBindingTarget::Up => bindings.up = key,
        KeyboardMenuBindingTarget::Down => bindings.down = key,
        KeyboardMenuBindingTarget::Confirm => bindings.confirm = key,
        KeyboardMenuBindingTarget::Cancel => bindings.cancel = key,
    }
}

fn set_keyboard_binding_value(
    bindings: &mut KeyboardBindings,
    target: KeyboardBindingTarget,
    key: DesktopKey,
) {
    match target {
        KeyboardBindingTarget::Up => bindings.joypad.up = key,
        KeyboardBindingTarget::Down => bindings.joypad.down = key,
        KeyboardBindingTarget::Left => bindings.joypad.left = key,
        KeyboardBindingTarget::Right => bindings.joypad.right = key,
        KeyboardBindingTarget::A => bindings.joypad.a = key,
        KeyboardBindingTarget::B => bindings.joypad.b = key,
        KeyboardBindingTarget::Select => bindings.joypad.select = key,
        KeyboardBindingTarget::Start => bindings.joypad.start = key,
        KeyboardBindingTarget::Pause => bindings.hotkeys.pause = key,
        KeyboardBindingTarget::SaveState => bindings.hotkeys.save_state = key,
        KeyboardBindingTarget::LoadState => bindings.hotkeys.load_state = key,
        KeyboardBindingTarget::StateSlot1 => bindings.hotkeys.state_slot_1 = key,
        KeyboardBindingTarget::StateSlot2 => bindings.hotkeys.state_slot_2 = key,
        KeyboardBindingTarget::StateSlot3 => bindings.hotkeys.state_slot_3 = key,
        KeyboardBindingTarget::StateSlot4 => bindings.hotkeys.state_slot_4 = key,
        KeyboardBindingTarget::Reset => bindings.hotkeys.reset = key,
        KeyboardBindingTarget::Rewind => bindings.hotkeys.rewind = key,
        KeyboardBindingTarget::FastForward => bindings.hotkeys.fast_forward = key,
        KeyboardBindingTarget::ToggleFullscreen => bindings.hotkeys.toggle_fullscreen = key,
        KeyboardBindingTarget::TogglePerformanceHud => {
            bindings.hotkeys.toggle_performance_hud = key;
        }
        KeyboardBindingTarget::SaveBattery => bindings.hotkeys.save_battery = key,
    }
}

fn sync_live_input_state(
    event_pump: &sdl3::EventPump,
    keyboard_bindings: &KeyboardBindings,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
) {
    clear_live_input_state(machine, runtime);
    let session_kind = player_session_kind(machine);
    for slot in PlayerSlot::ALL {
        let Some(route) = input_route_for_player_slot(machine, session_kind, slot) else {
            runtime.player_inputs.input_mut(slot).reset();
            continue;
        };
        let slot_machine = machine_for_player_input_route_mut(machine, slot, route);
        sync_player_keyboard_state(
            event_pump,
            keyboard_bindings,
            route.keyboard_profile,
            route.target,
            runtime.player_inputs.input_mut(slot),
            slot_machine,
        );
    }
    if let Some(gamepad_manager) = &mut runtime.gamepad_manager {
        gamepad_manager.sync_active_gamepad_state(
            runtime.player_inputs.input_mut(PlayerSlot::P1),
            machine
                .machine_for_player_slot_mut(PlayerSlot::P1)
                .expect("P1 should always map to an active desktop machine"),
        );
    }
}

fn clear_live_input_state(machine: &mut DesktopEmulationSession, runtime: &mut FrontendRuntime) {
    let session_kind = player_session_kind(machine);
    for slot in PlayerSlot::ALL {
        let Some(route) = input_route_for_player_slot(machine, session_kind, slot) else {
            runtime.player_inputs.input_mut(slot).reset();
            continue;
        };
        match machine_for_player_input_route_mut(machine, slot, route) {
            Some(machine) => runtime
                .player_inputs
                .input_mut(slot)
                .clear_all_for_target(machine, route.target),
            None => runtime.player_inputs.input_mut(slot).reset(),
        }
    }
}

fn sync_player_keyboard_state(
    event_pump: &sdl3::EventPump,
    keyboard_bindings: &KeyboardBindings,
    keyboard_profile: PlayerKeyboardProfile,
    target: FrontendJoypadTarget,
    input_state: &mut FrontendInputState,
    machine: Option<&mut Machine<TraceSummaryBuffer>>,
) {
    let Some(machine) = machine else {
        input_state.reset();
        return;
    };

    let keyboard_state = event_pump.keyboard_state();
    match keyboard_profile {
        PlayerKeyboardProfile::ConfiguredJoypad => {
            let joypad = keyboard_bindings.joypad;
            let bindings = [
                (JoypadButton::Up, desktop_key_scancode(joypad.up)),
                (JoypadButton::Down, desktop_key_scancode(joypad.down)),
                (JoypadButton::Left, desktop_key_scancode(joypad.left)),
                (JoypadButton::Right, desktop_key_scancode(joypad.right)),
                (JoypadButton::A, desktop_key_scancode(joypad.a)),
                (JoypadButton::B, desktop_key_scancode(joypad.b)),
                (JoypadButton::Select, desktop_key_scancode(joypad.select)),
                (JoypadButton::Start, desktop_key_scancode(joypad.start)),
            ];
            for (joypad_button, scancode) in bindings {
                input_state.set_keyboard_button_for_target(
                    machine,
                    target,
                    joypad_button,
                    keyboard_state.is_scancode_pressed(scancode),
                );
            }
        }
        PlayerKeyboardProfile::LinkedDmg04P2 => {
            for (joypad_button, scancode) in player_slots::LINKED_DMG04_P2_KEYBOARD_BINDINGS {
                input_state.set_keyboard_button_for_target(
                    machine,
                    target,
                    joypad_button,
                    keyboard_state.is_scancode_pressed(scancode),
                );
            }
        }
        PlayerKeyboardProfile::LinkedDmg07P3 => {
            for (joypad_button, scancode) in player_slots::LINKED_DMG07_P3_KEYBOARD_BINDINGS {
                input_state.set_keyboard_button_for_target(
                    machine,
                    target,
                    joypad_button,
                    keyboard_state.is_scancode_pressed(scancode),
                );
            }
        }
        PlayerKeyboardProfile::LinkedDmg07P4 => {
            for (joypad_button, scancode) in player_slots::LINKED_DMG07_P4_KEYBOARD_BINDINGS {
                input_state.set_keyboard_button_for_target(
                    machine,
                    target,
                    joypad_button,
                    keyboard_state.is_scancode_pressed(scancode),
                );
            }
        }
        PlayerKeyboardProfile::Disabled => {
            input_state.reset();
        }
    }
}

fn apply_keyboard_event_to_player_slots(
    runtime: &mut FrontendRuntime,
    machine: &mut DesktopEmulationSession,
    keycode: Option<Keycode>,
    scancode: Option<Scancode>,
    pressed: bool,
) {
    let session_kind = player_session_kind(machine);
    let keyboard_bindings = runtime.keyboard_bindings;
    for slot in PlayerSlot::ALL {
        let Some(route) = input_route_for_player_slot(machine, session_kind, slot) else {
            continue;
        };
        let Some(button) = joypad_button_for_player_keyboard_event(
            route.keyboard_profile,
            keyboard_bindings,
            keycode,
            scancode,
        ) else {
            continue;
        };
        let Some(slot_machine) = machine_for_player_input_route_mut(machine, slot, route) else {
            continue;
        };
        runtime
            .player_inputs
            .input_mut(slot)
            .set_keyboard_button_for_target(slot_machine, route.target, button, pressed);
    }
}

fn joypad_button_for_player_keyboard_event(
    keyboard_profile: PlayerKeyboardProfile,
    keyboard_bindings: KeyboardBindings,
    keycode: Option<Keycode>,
    scancode: Option<Scancode>,
) -> Option<JoypadButton> {
    match keyboard_profile {
        PlayerKeyboardProfile::ConfiguredJoypad => {
            joypad_button_for_key_event(keyboard_bindings.joypad, keycode, scancode)
        }
        PlayerKeyboardProfile::LinkedDmg04P2 => {
            scancode.and_then(linked_dmg04_p2_button_for_scancode)
        }
        PlayerKeyboardProfile::LinkedDmg07P3 => {
            scancode.and_then(linked_dmg07_p3_button_for_scancode)
        }
        PlayerKeyboardProfile::LinkedDmg07P4 => {
            scancode.and_then(linked_dmg07_p4_button_for_scancode)
        }
        PlayerKeyboardProfile::Disabled => None,
    }
}

fn desktop_key_scancode(binding: DesktopKey) -> Scancode {
    match binding {
        DesktopKey::Escape => Scancode::Escape,
        DesktopKey::ArrowUp => Scancode::Up,
        DesktopKey::ArrowDown => Scancode::Down,
        DesktopKey::ArrowLeft => Scancode::Left,
        DesktopKey::ArrowRight => Scancode::Right,
        DesktopKey::Tab => Scancode::Tab,
        DesktopKey::Backspace => Scancode::Backspace,
        DesktopKey::Return => Scancode::Return,
        DesktopKey::Space => Scancode::Space,
        DesktopKey::R => Scancode::R,
        DesktopKey::X => Scancode::X,
        DesktopKey::Z => Scancode::Z,
        DesktopKey::Digit1 => Scancode::_1,
        DesktopKey::Digit2 => Scancode::_2,
        DesktopKey::Digit3 => Scancode::_3,
        DesktopKey::Digit4 => Scancode::_4,
        DesktopKey::F1 => Scancode::F1,
        DesktopKey::F2 => Scancode::F2,
        DesktopKey::F3 => Scancode::F3,
        DesktopKey::F4 => Scancode::F4,
        DesktopKey::F5 => Scancode::F5,
        DesktopKey::F6 => Scancode::F6,
        DesktopKey::F7 => Scancode::F7,
        DesktopKey::F8 => Scancode::F8,
        DesktopKey::F9 => Scancode::F9,
        DesktopKey::F10 => Scancode::F10,
        DesktopKey::F11 => Scancode::F11,
        DesktopKey::F12 => Scancode::F12,
        DesktopKey::LeftShift => Scancode::LShift,
        DesktopKey::RightShift => Scancode::RShift,
        DesktopKey::LeftControl => Scancode::LCtrl,
        DesktopKey::RightControl => Scancode::RCtrl,
        DesktopKey::LeftAlt => Scancode::LAlt,
        DesktopKey::RightAlt => Scancode::RAlt,
        DesktopKey::LeftGui => Scancode::LGui,
        DesktopKey::RightGui => Scancode::RGui,
    }
}

fn gamepad_event_joystick_id(which: u32) -> sdl3::joystick::JoystickId {
    sdl3::sys::joystick::SDL_JoystickID(which)
}

#[cfg(test)]
fn menu_input_for_key(bindings: MenuKeyboardBindings, keycode: Keycode) -> Option<MenuInput> {
    menu_input_for_key_event(bindings, Some(keycode), None)
}

fn menu_input_for_key_event(
    bindings: MenuKeyboardBindings,
    keycode: Option<Keycode>,
    scancode: Option<Scancode>,
) -> Option<MenuInput> {
    if key_event_matches(DesktopKey::Escape, keycode, scancode) {
        return Some(MenuInput::Cancel);
    }

    if key_event_matches(bindings.up, keycode, scancode) {
        Some(MenuInput::Up)
    } else if key_event_matches(bindings.down, keycode, scancode) {
        Some(MenuInput::Down)
    } else if key_event_matches(bindings.confirm, keycode, scancode) {
        Some(MenuInput::Confirm)
    } else if key_event_matches(bindings.cancel, keycode, scancode) {
        Some(MenuInput::Cancel)
    } else {
        None
    }
}

fn menu_input_for_gamepad_button(
    bindings: GamepadMenuBindings,
    button: Button,
) -> Option<MenuInput> {
    let binding = gamepad_button_binding_from_sdl_button(button)?;
    menu_input_for_gamepad_binding(bindings, binding)
}

fn menu_input_for_gamepad_binding(
    bindings: GamepadMenuBindings,
    binding: GamepadButtonBinding,
) -> Option<MenuInput> {
    if binding == bindings.up {
        Some(MenuInput::Up)
    } else if binding == bindings.down {
        Some(MenuInput::Down)
    } else if binding == bindings.confirm {
        Some(MenuInput::Confirm)
    } else if binding == bindings.cancel {
        Some(MenuInput::Cancel)
    } else {
        None
    }
}

#[cfg(test)]
fn joypad_button_for_key(
    bindings: JoypadKeyboardBindings,
    keycode: Keycode,
) -> Option<JoypadButton> {
    joypad_button_for_key_event(bindings, Some(keycode), None)
}

fn joypad_button_for_key_event(
    bindings: JoypadKeyboardBindings,
    keycode: Option<Keycode>,
    scancode: Option<Scancode>,
) -> Option<JoypadButton> {
    if key_event_matches(bindings.up, keycode, scancode) {
        Some(JoypadButton::Up)
    } else if key_event_matches(bindings.down, keycode, scancode) {
        Some(JoypadButton::Down)
    } else if key_event_matches(bindings.left, keycode, scancode) {
        Some(JoypadButton::Left)
    } else if key_event_matches(bindings.right, keycode, scancode) {
        Some(JoypadButton::Right)
    } else if key_event_matches(bindings.a, keycode, scancode) {
        Some(JoypadButton::A)
    } else if key_event_matches(bindings.b, keycode, scancode) {
        Some(JoypadButton::B)
    } else if key_event_matches(bindings.select, keycode, scancode) {
        Some(JoypadButton::Select)
    } else if key_event_matches(bindings.start, keycode, scancode) {
        Some(JoypadButton::Start)
    } else {
        None
    }
}

#[cfg(test)]
fn hotkey_action(keyboard_bindings: &KeyboardBindings, keycode: Keycode) -> HotkeyAction {
    hotkey_action_for_key_event(keyboard_bindings, Some(keycode), None)
}

fn hotkey_action_for_key_event(
    keyboard_bindings: &KeyboardBindings,
    keycode: Option<Keycode>,
    scancode: Option<Scancode>,
) -> HotkeyAction {
    if key_event_matches(keyboard_bindings.hotkeys.save_battery, keycode, scancode) {
        HotkeyAction::ManualSave
    } else if key_event_matches(keyboard_bindings.hotkeys.save_state, keycode, scancode) {
        HotkeyAction::SaveState
    } else if key_event_matches(keyboard_bindings.hotkeys.load_state, keycode, scancode) {
        HotkeyAction::LoadState
    } else if key_event_matches(keyboard_bindings.hotkeys.state_slot_1, keycode, scancode) {
        HotkeyAction::SelectStateSlot(1)
    } else if key_event_matches(keyboard_bindings.hotkeys.state_slot_2, keycode, scancode) {
        HotkeyAction::SelectStateSlot(2)
    } else if key_event_matches(keyboard_bindings.hotkeys.state_slot_3, keycode, scancode) {
        HotkeyAction::SelectStateSlot(3)
    } else if key_event_matches(keyboard_bindings.hotkeys.state_slot_4, keycode, scancode) {
        HotkeyAction::SelectStateSlot(4)
    } else if key_event_matches(keyboard_bindings.hotkeys.reset, keycode, scancode) {
        HotkeyAction::Reset
    } else if key_event_matches(keyboard_bindings.hotkeys.rewind, keycode, scancode) {
        HotkeyAction::Rewind
    } else if key_event_matches(keyboard_bindings.hotkeys.fast_forward, keycode, scancode) {
        HotkeyAction::FastForward
    } else if key_event_matches(
        keyboard_bindings.hotkeys.toggle_fullscreen,
        keycode,
        scancode,
    ) {
        HotkeyAction::ToggleFullscreen
    } else if key_event_matches(
        keyboard_bindings.hotkeys.toggle_performance_hud,
        keycode,
        scancode,
    ) {
        HotkeyAction::TogglePerformanceHud
    } else {
        HotkeyAction::None
    }
}

fn gamepad_action_for_button(bindings: GamepadActionBindings, button: Button) -> HotkeyAction {
    let Some(binding) = gamepad_button_binding_from_sdl_button(button) else {
        return HotkeyAction::None;
    };

    gamepad_action_for_binding(bindings, binding)
}

fn gamepad_action_for_binding(
    bindings: GamepadActionBindings,
    binding: GamepadButtonBinding,
) -> HotkeyAction {
    if bindings.save_state == Some(binding) {
        HotkeyAction::SaveState
    } else if bindings.load_state == Some(binding) {
        HotkeyAction::LoadState
    } else if bindings.rewind == Some(binding) {
        HotkeyAction::Rewind
    } else if bindings.fast_forward == Some(binding) {
        HotkeyAction::FastForward
    } else {
        HotkeyAction::None
    }
}

fn gamepad_trigger_event_binding(
    state: &mut GamepadTriggerState,
    axis: Axis,
    value: i16,
) -> Option<(GamepadButtonBinding, bool)> {
    let binding = gamepad_button_binding_from_sdl_axis(axis)?;
    let current = state.pressed_mut(binding)?;
    let next = gamepad_trigger_axis_next_pressed(value, *current);
    if *current == next {
        return None;
    }
    *current = next;
    Some((binding, next))
}

fn apply_gamepad_action_event(
    event: GamepadActionEvent,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    if !event.pressed {
        if matches!(event.action, HotkeyAction::Rewind) {
            context.runtime.rewind_gamepad_active = false;
        }
        if matches!(event.action, HotkeyAction::FastForward) {
            context.runtime.fast_forward_gamepad_active = false;
        }
        return Ok(());
    }

    match event.action {
        HotkeyAction::SaveState => {
            match save_machine_state_slot(
                context.session,
                context.machine,
                context.runtime.machine_state_slot,
            ) {
                Ok(path) => eprintln!("info: state saved to {}", path.display()),
                Err(error) => {
                    show_warning_message(Some(canvas.window()), "Save State", &error);
                    eprintln!("warning: {error}");
                }
            }
        }
        HotkeyAction::LoadState => {
            handle_load_machine_state_action(
                context.session,
                context.machine,
                context.runtime,
                context.performance_counter,
                context.frame_pacer,
                canvas,
            )?;
        }
        HotkeyAction::Rewind => {
            context.runtime.rewind_gamepad_active = true;
        }
        HotkeyAction::FastForward => {
            context.runtime.fast_forward_gamepad_active = true;
        }
        HotkeyAction::None
        | HotkeyAction::ManualSave
        | HotkeyAction::SelectStateSlot(_)
        | HotkeyAction::Reset
        | HotkeyAction::ToggleFullscreen
        | HotkeyAction::TogglePerformanceHud => {}
    }

    Ok(())
}

fn desktop_key_from_keycode(keycode: Keycode) -> Option<DesktopKey> {
    match keycode {
        Keycode::Escape => Some(DesktopKey::Escape),
        Keycode::Up => Some(DesktopKey::ArrowUp),
        Keycode::Down => Some(DesktopKey::ArrowDown),
        Keycode::Left => Some(DesktopKey::ArrowLeft),
        Keycode::Right => Some(DesktopKey::ArrowRight),
        Keycode::Tab => Some(DesktopKey::Tab),
        Keycode::Backspace => Some(DesktopKey::Backspace),
        Keycode::Return => Some(DesktopKey::Return),
        Keycode::Space => Some(DesktopKey::Space),
        Keycode::R => Some(DesktopKey::R),
        Keycode::X => Some(DesktopKey::X),
        Keycode::Z => Some(DesktopKey::Z),
        Keycode::_1 => Some(DesktopKey::Digit1),
        Keycode::_2 => Some(DesktopKey::Digit2),
        Keycode::_3 => Some(DesktopKey::Digit3),
        Keycode::_4 => Some(DesktopKey::Digit4),
        Keycode::F1 => Some(DesktopKey::F1),
        Keycode::F2 => Some(DesktopKey::F2),
        Keycode::F3 => Some(DesktopKey::F3),
        Keycode::F4 => Some(DesktopKey::F4),
        Keycode::F5 => Some(DesktopKey::F5),
        Keycode::F6 => Some(DesktopKey::F6),
        Keycode::F7 => Some(DesktopKey::F7),
        Keycode::F8 => Some(DesktopKey::F8),
        Keycode::F9 => Some(DesktopKey::F9),
        Keycode::F10 => Some(DesktopKey::F10),
        Keycode::F11 => Some(DesktopKey::F11),
        Keycode::F12 => Some(DesktopKey::F12),
        Keycode::LShift => Some(DesktopKey::LeftShift),
        Keycode::RShift => Some(DesktopKey::RightShift),
        Keycode::LCtrl => Some(DesktopKey::LeftControl),
        Keycode::RCtrl => Some(DesktopKey::RightControl),
        Keycode::LAlt => Some(DesktopKey::LeftAlt),
        Keycode::RAlt => Some(DesktopKey::RightAlt),
        Keycode::LGui => Some(DesktopKey::LeftGui),
        Keycode::RGui => Some(DesktopKey::RightGui),
        _ => None,
    }
}

fn desktop_modifier_key_from_keycode(keycode: Keycode) -> Option<DesktopKey> {
    match keycode {
        Keycode::LShift => Some(DesktopKey::LeftShift),
        Keycode::RShift => Some(DesktopKey::RightShift),
        Keycode::LCtrl => Some(DesktopKey::LeftControl),
        Keycode::RCtrl => Some(DesktopKey::RightControl),
        Keycode::LAlt => Some(DesktopKey::LeftAlt),
        Keycode::RAlt => Some(DesktopKey::RightAlt),
        Keycode::LGui => Some(DesktopKey::LeftGui),
        Keycode::RGui => Some(DesktopKey::RightGui),
        _ => None,
    }
}

fn desktop_key_from_scancode(scancode: Scancode) -> Option<DesktopKey> {
    match scancode {
        Scancode::Escape => Some(DesktopKey::Escape),
        Scancode::Up => Some(DesktopKey::ArrowUp),
        Scancode::Down => Some(DesktopKey::ArrowDown),
        Scancode::Left => Some(DesktopKey::ArrowLeft),
        Scancode::Right => Some(DesktopKey::ArrowRight),
        Scancode::Tab => Some(DesktopKey::Tab),
        Scancode::Backspace => Some(DesktopKey::Backspace),
        Scancode::Return => Some(DesktopKey::Return),
        Scancode::Space => Some(DesktopKey::Space),
        Scancode::R => Some(DesktopKey::R),
        Scancode::X => Some(DesktopKey::X),
        Scancode::Z => Some(DesktopKey::Z),
        Scancode::_1 => Some(DesktopKey::Digit1),
        Scancode::_2 => Some(DesktopKey::Digit2),
        Scancode::_3 => Some(DesktopKey::Digit3),
        Scancode::_4 => Some(DesktopKey::Digit4),
        Scancode::F1 => Some(DesktopKey::F1),
        Scancode::F2 => Some(DesktopKey::F2),
        Scancode::F3 => Some(DesktopKey::F3),
        Scancode::F4 => Some(DesktopKey::F4),
        Scancode::F5 => Some(DesktopKey::F5),
        Scancode::F6 => Some(DesktopKey::F6),
        Scancode::F7 => Some(DesktopKey::F7),
        Scancode::F8 => Some(DesktopKey::F8),
        Scancode::F9 => Some(DesktopKey::F9),
        Scancode::F10 => Some(DesktopKey::F10),
        Scancode::F11 => Some(DesktopKey::F11),
        Scancode::F12 => Some(DesktopKey::F12),
        Scancode::LShift => Some(DesktopKey::LeftShift),
        Scancode::RShift => Some(DesktopKey::RightShift),
        Scancode::LCtrl => Some(DesktopKey::LeftControl),
        Scancode::RCtrl => Some(DesktopKey::RightControl),
        Scancode::LAlt => Some(DesktopKey::LeftAlt),
        Scancode::RAlt => Some(DesktopKey::RightAlt),
        Scancode::LGui => Some(DesktopKey::LeftGui),
        Scancode::RGui => Some(DesktopKey::RightGui),
        _ => None,
    }
}

fn desktop_key_from_key_event(
    keycode: Option<Keycode>,
    scancode: Option<Scancode>,
) -> Option<DesktopKey> {
    if let Some(modifier_key) = keycode.and_then(desktop_modifier_key_from_keycode) {
        return Some(modifier_key);
    }

    match scancode {
        Some(scancode) => desktop_key_from_scancode(scancode),
        None => keycode.and_then(desktop_key_from_keycode),
    }
}

#[cfg(test)]
fn assignable_key_for_binding_target_from_keycode(
    keycode: Keycode,
    target: KeyboardBindingTarget,
) -> Option<DesktopKey> {
    let key = desktop_key_from_keycode(keycode)?;
    assignable_key_for_binding_target(key, target)
}

fn assignable_key_for_binding_target_from_key_event(
    keycode: Option<Keycode>,
    scancode: Option<Scancode>,
    target: KeyboardBindingTarget,
) -> Option<DesktopKey> {
    let key = desktop_key_from_key_event(keycode, scancode)?;
    assignable_key_for_binding_target(key, target)
}

fn assignable_key_for_binding_target(
    key: DesktopKey,
    target: KeyboardBindingTarget,
) -> Option<DesktopKey> {
    match target {
        KeyboardBindingTarget::Pause
        | KeyboardBindingTarget::SaveState
        | KeyboardBindingTarget::LoadState
        | KeyboardBindingTarget::StateSlot1
        | KeyboardBindingTarget::StateSlot2
        | KeyboardBindingTarget::StateSlot3
        | KeyboardBindingTarget::StateSlot4
        | KeyboardBindingTarget::Reset
        | KeyboardBindingTarget::Rewind
        | KeyboardBindingTarget::FastForward
        | KeyboardBindingTarget::ToggleFullscreen
        | KeyboardBindingTarget::TogglePerformanceHud
        | KeyboardBindingTarget::SaveBattery => is_hotkey_assignable_key(key).then_some(key),
        KeyboardBindingTarget::Up
        | KeyboardBindingTarget::Down
        | KeyboardBindingTarget::Left
        | KeyboardBindingTarget::Right
        | KeyboardBindingTarget::A
        | KeyboardBindingTarget::B
        | KeyboardBindingTarget::Select
        | KeyboardBindingTarget::Start => is_joypad_assignable_key(key).then_some(key),
    }
}

#[cfg(test)]
fn assignable_menu_key_for_binding_target_from_keycode(
    keycode: Keycode,
    target: KeyboardMenuBindingTarget,
) -> Option<DesktopKey> {
    let key = desktop_key_from_keycode(keycode)?;
    assignable_menu_key_for_binding_target(key, target)
}

fn assignable_menu_key_for_binding_target_from_key_event(
    keycode: Option<Keycode>,
    scancode: Option<Scancode>,
    target: KeyboardMenuBindingTarget,
) -> Option<DesktopKey> {
    let key = desktop_key_from_key_event(keycode, scancode)?;
    assignable_menu_key_for_binding_target(key, target)
}

fn assignable_menu_key_for_binding_target(
    key: DesktopKey,
    target: KeyboardMenuBindingTarget,
) -> Option<DesktopKey> {
    match target {
        KeyboardMenuBindingTarget::Cancel => Some(key),
        KeyboardMenuBindingTarget::Up
        | KeyboardMenuBindingTarget::Down
        | KeyboardMenuBindingTarget::Confirm => (!matches!(key, DesktopKey::Escape)).then_some(key),
    }
}

fn is_joypad_assignable_key(key: DesktopKey) -> bool {
    !matches!(
        key,
        DesktopKey::Escape
            | DesktopKey::Digit1
            | DesktopKey::Digit2
            | DesktopKey::Digit3
            | DesktopKey::Digit4
            | DesktopKey::F1
            | DesktopKey::F2
            | DesktopKey::F3
            | DesktopKey::F4
            | DesktopKey::F5
            | DesktopKey::F6
            | DesktopKey::F7
            | DesktopKey::F8
            | DesktopKey::F9
            | DesktopKey::F10
            | DesktopKey::F11
            | DesktopKey::F12
    )
}

fn is_hotkey_assignable_key(key: DesktopKey) -> bool {
    !matches!(key, DesktopKey::Escape)
}

#[cfg(test)]
fn key_matches(binding: DesktopKey, keycode: Keycode) -> bool {
    desktop_key_from_keycode(keycode) == Some(binding)
}

fn key_event_matches(
    binding: DesktopKey,
    keycode: Option<Keycode>,
    scancode: Option<Scancode>,
) -> bool {
    desktop_key_from_key_event(keycode, scancode) == Some(binding)
}

fn startup_mode_name(startup_mode: StartupMode) -> &'static str {
    match startup_mode {
        StartupMode::SkipBoot => "skip-boot",
        StartupMode::CustomBoot => "custom-boot",
        StartupMode::RealBoot => "real-boot",
    }
}

fn execution_mode_name(execution_mode: ExecutionMode) -> &'static str {
    match execution_mode {
        ExecutionMode::Strict => "strict",
        ExecutionMode::Permissive => "permissive",
        ExecutionMode::Experimental => "experimental",
    }
}

fn display_palette_for_desktop_palette(display_palette: DesktopDisplayPalette) -> DisplayPalette {
    match display_palette {
        DesktopDisplayPalette::Grey => DMG_GREY_DISPLAY_PALETTE,
        DesktopDisplayPalette::GameBoy => DMG_DISPLAY_PALETTE,
        DesktopDisplayPalette::Pocket => MGB_DISPLAY_PALETTE,
        DesktopDisplayPalette::Light => GBL_DISPLAY_PALETTE,
    }
}
