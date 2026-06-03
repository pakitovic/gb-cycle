fn load_initial_emulation_session(
    session: &mut DesktopSession,
) -> Result<(DesktopEmulationSession, Vec<CartridgeDiagnostic>), String> {
    sanitize_external_port_session_for_model(session);
    match (
        session.rom_bytes(),
        session.linked_secondary_rom_bytes(),
        session.cgb_infrared_link_active,
        session.external_port_selection,
    ) {
        (Some(primary_rom_bytes), Some(secondary_rom_bytes), true, _) => {
            let loaded = load_cgb_infrared_machines_for_roms(
                &session.config,
                &session.current_dir,
                primary_rom_bytes,
                secondary_rom_bytes,
                "linked CGB IR desktop startup",
            )?;
            for warning in &loaded.boot_rom_fallback_warnings {
                log_boot_rom_fallback_warning(Some(warning));
            }
            session.config = loaded.effective_config;
            let mut machine = loaded.machine;
            apply_session_pocket_camera_frame_to_desktop_session(session, &mut machine)?;
            Ok((machine, loaded.diagnostics))
        }
        (
            Some(primary_rom_bytes),
            Some(secondary_rom_bytes),
            false,
            DesktopExternalPortSelection::GameLink,
        ) => {
            let primary_loaded =
                load_machine_for_rom(&session.config, &session.current_dir, primary_rom_bytes)?;
            let secondary_loaded =
                load_machine_for_rom(&session.config, &session.current_dir, secondary_rom_bytes)?;
            if primary_loaded.effective_config != secondary_loaded.effective_config {
                return Err(
                    "linked desktop startup produced divergent effective configs between the primary and secondary machines"
                        .to_string(),
                );
            }

            log_boot_rom_fallback_warning(primary_loaded.boot_rom_fallback_warning.as_deref());
            log_boot_rom_fallback_warning(secondary_loaded.boot_rom_fallback_warning.as_deref());
            session.config = primary_loaded.effective_config;
            let mut machine = DesktopEmulationSession::new_linked_dmg04_two_player(
                primary_loaded.machine,
                secondary_loaded.machine,
            )?;
            apply_session_pocket_camera_frame_to_desktop_session(session, &mut machine)?;
            let mut diagnostics = primary_loaded.diagnostics;
            diagnostics.extend(secondary_loaded.diagnostics);
            Ok((machine, diagnostics))
        }
        (Some(rom_bytes), _, _, _) => {
            let loaded = load_machine_for_rom(&session.config, &session.current_dir, rom_bytes)?;
            log_boot_rom_fallback_warning(loaded.boot_rom_fallback_warning.as_deref());
            session.config = loaded.effective_config;
            let mut machine = DesktopEmulationSession::new_single(loaded.machine);
            apply_external_port_selection_to_machine(&mut machine, session.external_port_selection);
            apply_session_pocket_camera_frame_to_desktop_session(session, &mut machine)?;
            Ok((machine, loaded.diagnostics))
        }
        (None, _, _, _) => {
            let prepared = prepare_machine_config(&session.config, &session.current_dir)?;
            log_boot_rom_fallback_warning(prepared.boot_rom_fallback_warning.as_deref());
            session.config = prepared.effective_config;
            let mut machine =
                DesktopEmulationSession::new_single(Machine::new_summary(prepared.machine_config));
            apply_external_port_selection_to_machine(&mut machine, session.external_port_selection);
            apply_session_pocket_camera_frame_to_desktop_session(session, &mut machine)?;
            Ok((machine, Vec::new()))
        }
    }
}

#[derive(Debug)]
struct PreparedMachineConfig {
    effective_config: DesktopConfig,
    machine_config: MachineConfig,
    boot_rom_fallback_warning: Option<String>,
}

#[derive(Debug)]
struct LoadedMachine {
    effective_config: DesktopConfig,
    machine: Machine<TraceSummaryBuffer>,
    diagnostics: Vec<CartridgeDiagnostic>,
    boot_rom_fallback_warning: Option<String>,
}

struct LoadedDmg07Machines {
    effective_config: DesktopConfig,
    machine: DesktopEmulationSession,
    diagnostics: Vec<CartridgeDiagnostic>,
    boot_rom_fallback_warnings: Vec<String>,
}

struct LoadedCgbInfraredMachines {
    effective_config: DesktopConfig,
    machine: DesktopEmulationSession,
    diagnostics: Vec<CartridgeDiagnostic>,
    boot_rom_fallback_warnings: Vec<String>,
}

type RebuildMachineResult = (
    DesktopConfig,
    Vec<String>,
    DesktopEmulationSession,
    [Option<DesktopSaveSession>; PLAYER_SLOT_COUNT],
);

fn prepare_machine_config(
    config: &DesktopConfig,
    current_dir: &Path,
) -> Result<PreparedMachineConfig, String> {
    let mut effective_config = config.clone();
    effective_config.launch.normalize_revision_for_model();
    let boot_rom_fallback_warning =
        maybe_apply_missing_boot_rom_fallback(&mut effective_config, current_dir)?;
    let machine_config = effective_config.machine_config_without_boot_rom_assets();
    let boot_rom_assets = load_boot_rom_assets(
        effective_config.boot_rom.search_path.as_deref(),
        effective_config.boot_rom.verification,
        machine_config.boot_rom_asset_kind(),
        effective_config.launch.startup_mode,
        current_dir,
    )?;

    Ok(PreparedMachineConfig {
        machine_config: machine_config.with_boot_rom_assets(boot_rom_assets),
        effective_config,
        boot_rom_fallback_warning,
    })
}

fn maybe_apply_missing_boot_rom_fallback(
    config: &mut DesktopConfig,
    current_dir: &Path,
) -> Result<Option<String>, String> {
    if config.launch.startup_mode != StartupMode::RealBoot {
        return Ok(None);
    }

    let machine_config = config.machine_config_without_boot_rom_assets();
    let Some(missing_asset) = missing_boot_rom_asset(
        config.boot_rom.search_path.as_deref(),
        machine_config.boot_rom_asset_kind(),
        current_dir,
    )?
    else {
        return Ok(None);
    };

    config.launch.startup_mode = StartupMode::SkipBoot;
    Ok(Some(format!("{missing_asset}; falling back to skip-boot")))
}

fn log_boot_rom_fallback_warning(warning: Option<&str>) {
    if let Some(warning) = warning {
        eprintln!("warning: {warning}");
    }
}

fn load_machine_for_rom(
    config: &DesktopConfig,
    current_dir: &Path,
    rom_bytes: &[u8],
) -> Result<LoadedMachine, String> {
    let prepared = prepare_machine_config(config, current_dir)?;
    let mut machine = Machine::new_summary(prepared.machine_config);
    let diagnostics = match machine.load_cartridge(rom_bytes.to_vec()) {
        Ok(diagnostics) => diagnostics,
        Err(error) => {
            return Err(format_debug_error(
                "failed to load cartridge",
                &format!("{error:?}"),
            ));
        }
    };
    Ok(LoadedMachine {
        effective_config: prepared.effective_config,
        machine,
        diagnostics,
        boot_rom_fallback_warning: prepared.boot_rom_fallback_warning,
    })
}

fn load_dmg07_machines_for_rom(
    config: &DesktopConfig,
    current_dir: &Path,
    rom_bytes: &[u8],
    player_count: DesktopDmg07PlayerCount,
    operation: &str,
) -> Result<LoadedDmg07Machines, String> {
    let mut loaded_machines = Vec::with_capacity(player_count.get());
    let mut diagnostics = Vec::new();
    let mut boot_rom_fallback_warnings = Vec::new();
    let mut effective_config = None;

    for player_index in 0..player_count.get() {
        let loaded = load_machine_for_rom(config, current_dir, rom_bytes)?;
        if let Some(expected_config) = &effective_config {
            if expected_config != &loaded.effective_config {
                return Err(format!(
                    "{operation} produced divergent effective configs at DMG-07 player index {player_index}"
                ));
            }
        } else {
            effective_config = Some(loaded.effective_config.clone());
        }

        diagnostics.extend(loaded.diagnostics);
        if let Some(warning) = loaded.boot_rom_fallback_warning {
            boot_rom_fallback_warnings.push(warning);
        }
        loaded_machines.push(loaded.machine);
    }

    let effective_config = effective_config.unwrap_or_else(|| config.clone());
    let machine = DesktopEmulationSession::new_linked_dmg07(loaded_machines, player_count)?;
    Ok(LoadedDmg07Machines {
        effective_config,
        machine,
        diagnostics,
        boot_rom_fallback_warnings,
    })
}

fn load_cgb_infrared_machines_for_roms(
    config: &DesktopConfig,
    current_dir: &Path,
    primary_rom_bytes: &[u8],
    secondary_rom_bytes: &[u8],
    operation: &str,
) -> Result<LoadedCgbInfraredMachines, String> {
    if config.launch.console_model != DesktopConsoleModel::GameBoyColor {
        return Err(format!("{operation} requires MODEL GB COLOR"));
    }

    let LoadedMachine {
        effective_config,
        machine: primary_machine,
        mut diagnostics,
        boot_rom_fallback_warning: primary_boot_rom_fallback_warning,
    } = load_machine_for_rom(config, current_dir, primary_rom_bytes)?;
    let primary_native_cgb = primary_machine
        .config()
        .capability_set()
        .cgb_extensions_enabled();
    let mut machine = DesktopEmulationSession::new_single(primary_machine);

    let LoadedMachine {
        effective_config: secondary_effective_config,
        machine: secondary_machine,
        diagnostics: secondary_diagnostics,
        boot_rom_fallback_warning: secondary_boot_rom_fallback_warning,
    } = load_machine_for_rom(config, current_dir, secondary_rom_bytes)?;
    if effective_config != secondary_effective_config {
        return Err(format!(
            "{operation} produced divergent effective configs between primary and secondary machines"
        ));
    }

    let secondary_native_cgb = secondary_machine
        .config()
        .capability_set()
        .cgb_extensions_enabled();
    if !primary_native_cgb || !secondary_native_cgb {
        return Err(format!(
            "{operation} requires native CGB mode for both cartridges"
        ));
    }

    diagnostics.extend(secondary_diagnostics);
    let mut boot_rom_fallback_warnings = Vec::new();
    if let Some(warning) = primary_boot_rom_fallback_warning {
        boot_rom_fallback_warnings.push(warning);
    }
    if let Some(warning) = secondary_boot_rom_fallback_warning {
        boot_rom_fallback_warnings.push(warning);
    }

    let cgb_ir_optical_delay_t_cycles = cgb_ir_optical_delay_t_cycles_from_env()?;
    if cgb_ir_optical_delay_t_cycles == DEFAULT_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES {
        machine.attach_secondary_cgb_infrared(secondary_machine)?;
    } else {
        machine.attach_secondary_cgb_infrared_with_optical_delay(
            secondary_machine,
            cgb_ir_optical_delay_t_cycles,
        )?;
    }

    Ok(LoadedCgbInfraredMachines {
        effective_config,
        machine,
        diagnostics,
        boot_rom_fallback_warnings,
    })
}

fn apply_external_port_selection_to_machine(
    machine: &mut Machine<TraceSummaryBuffer>,
    selection: DesktopExternalPortSelection,
) {
    machine.set_external_port_attachment(selection.core_attachment_kind());
}

fn supported_external_port_selection_for_model(
    console_model: DesktopConsoleModel,
    selection: DesktopExternalPortSelection,
) -> DesktopExternalPortSelection {
    if console_model.allows_ext_port_menu() {
        selection
    } else {
        DesktopExternalPortSelection::None
    }
}

fn sanitize_external_port_session_for_model(session: &mut DesktopSession) {
    let supported_selection = supported_external_port_selection_for_model(
        session.config.launch.console_model,
        session.external_port_selection,
    );
    if supported_selection == session.external_port_selection {
        return;
    }

    session.external_port_selection = supported_selection;
    if !session.cgb_infrared_link_active {
        session.linked_secondary_rom = None;
    }
    session.dmg07_player_count = None;
}

fn session_has_pocket_camera(machine: &DesktopEmulationSession) -> bool {
    PlayerSlot::ALL.into_iter().any(|slot| {
        machine
            .machine_for_player_slot(slot)
            .is_some_and(Machine::has_pocket_camera)
    })
}

fn apply_pocket_camera_frame_to_machine(
    frame: &PocketCameraFrame,
    machine: &mut Machine<TraceSummaryBuffer>,
    slot: PlayerSlot,
    action: &str,
) -> Result<(), String> {
    if !machine.has_pocket_camera() {
        return Ok(());
    }

    machine
        .set_pocket_camera_frame(frame.clone())
        .map_err(|error| {
            format_debug_error(
                &format!(
                    "failed to {action} Pocket Camera frame for {}",
                    slot.label()
                ),
                &format!("{error:?}"),
            )
        })
}

fn apply_pocket_camera_frame_to_desktop_session(
    frame: &PocketCameraFrame,
    machine: &mut DesktopEmulationSession,
    action: &str,
) -> Result<(), String> {
    for slot in PlayerSlot::ALL {
        if let Some(slot_machine) = machine.machine_for_player_slot_mut(slot) {
            apply_pocket_camera_frame_to_machine(frame, slot_machine, slot, action)?;
        }
    }
    Ok(())
}

fn apply_session_pocket_camera_frame_to_desktop_session(
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
) -> Result<(), String> {
    let Some(frame) = session.pocket_camera_frame.as_ref() else {
        return Ok(());
    };
    apply_pocket_camera_frame_to_desktop_session(frame, machine, "apply")
}

fn apply_pocket_camera_live_frame_to_desktop_session(
    frame: &PocketCameraFrame,
    machine: &mut DesktopEmulationSession,
) -> Result<(), String> {
    apply_pocket_camera_frame_to_desktop_session(frame, machine, "apply live")
}

fn clear_pocket_camera_frame_from_desktop_session(
    machine: &mut DesktopEmulationSession,
) -> Result<(), String> {
    for slot in PlayerSlot::ALL {
        let Some(slot_machine) = machine.machine_for_player_slot_mut(slot) else {
            continue;
        };
        if !slot_machine.has_pocket_camera() {
            continue;
        }
        slot_machine.clear_pocket_camera_frame().map_err(|error| {
            format_debug_error(
                &format!("failed to reset Pocket Camera image on {}", slot.label()),
                &format!("{error:?}"),
            )
        })?;
    }
    Ok(())
}

fn drain_printed_pages_into_printer_output(
    main_window: &Window,
    session: &DesktopSession,
    runtime: &mut FrontendRuntime,
    machine: &mut Machine<TraceSummaryBuffer>,
) {
    let printed_pages = machine.take_printed_pages();
    if printed_pages.is_empty() {
        return;
    }

    for printed_page in printed_pages {
        if let Err(error) = runtime.printer_output.handle_printed_page(
            main_window,
            session.rom_path(),
            session.current_dir.as_path(),
            &printed_page,
        ) {
            eprintln!("printer output failed: {error}");
        }
    }
}

fn flush_pending_printer_output(
    main_window: &Window,
    session: &DesktopSession,
    runtime: &mut FrontendRuntime,
) {
    if let Err(error) = runtime.printer_output.flush_pending_document(
        main_window,
        session.rom_path(),
        session.current_dir.as_path(),
    ) {
        eprintln!("printer output failed: {error}");
    }
}

fn load_initial_rom(
    options: &DesktopRunOptions,
    current_dir: &Path,
) -> Result<Option<LoadedRom>, String> {
    let Some(rom_path) = options.rom_path.as_ref() else {
        return Ok(None);
    };
    load_rom_from_cli_path(current_dir, rom_path, "failed to read ROM").map(Some)
}

fn apply_benchmark_case_to_desktop_options(
    options: &mut DesktopRunOptions,
    benchmark_case: &BenchmarkCase,
) {
    options.rom_path = Some(benchmark_case.rom.clone());
    options.exit_after_frames = Some(u64::from(target_frames_for_duration(
        benchmark_case.duration_seconds,
    )));
    options.config.launch.console_model = desktop_model_from_benchmark(benchmark_case.model);
    options.config.launch.normalize_revision_for_model();
    options.config.launch.startup_mode = startup_mode_from_benchmark(benchmark_case.startup);
    options.config.launch.execution_mode = execution_mode_from_benchmark(benchmark_case.mode);
    if options.config.launch.console_model == DesktopConsoleModel::GameBoy
        && let Some(palette) = benchmark_case.palette
    {
        options.config.video.display_palette = desktop_display_palette_from_benchmark(palette);
    }
    if options.test_runner {
        options.config.launch.execution_mode = ExecutionMode::Permissive;
        if options.config.launch.console_model == DesktopConsoleModel::GameBoy {
            options.config.video.display_palette = DesktopDisplayPalette::Grey;
        }
        options.config.video.show_sgb_border = false;
    }
}

fn load_initial_linked_secondary_rom(
    options: &DesktopRunOptions,
    current_dir: &Path,
) -> Result<Option<LoadedRom>, String> {
    let Some(rom_path) = options.linked_peer_rom_path.as_ref() else {
        return Ok(None);
    };
    load_rom_from_cli_path(current_dir, rom_path, "failed to read linked peer ROM").map(Some)
}

fn load_rom_from_cli_path(
    current_dir: &Path,
    rom_path: &Path,
    read_error_label: &str,
) -> Result<LoadedRom, String> {
    let rom_path = resolve_path(current_dir, rom_path);
    let rom_bytes = match fs::read(&rom_path) {
        Ok(rom_bytes) => rom_bytes,
        Err(error) => {
            return Err(format_path_error(
                read_error_label,
                &rom_path,
                &error.to_string(),
            ));
        }
    };
    Ok(LoadedRom {
        path: rom_path,
        bytes: rom_bytes,
    })
}

fn desktop_model_from_benchmark(model: BenchmarkModel) -> DesktopConsoleModel {
    match model {
        BenchmarkModel::Dmg => DesktopConsoleModel::GameBoy,
        BenchmarkModel::Mgb => DesktopConsoleModel::GameBoyPocket,
        BenchmarkModel::Lgb => DesktopConsoleModel::GameBoyLight,
        BenchmarkModel::Cgb => DesktopConsoleModel::GameBoyColor,
    }
}

fn startup_mode_from_benchmark(startup: BenchmarkStartup) -> StartupMode {
    match startup {
        BenchmarkStartup::SkipBoot => StartupMode::SkipBoot,
        BenchmarkStartup::CustomBoot => StartupMode::CustomBoot,
        BenchmarkStartup::RealBoot => StartupMode::RealBoot,
    }
}

fn execution_mode_from_benchmark(mode: BenchmarkMode) -> ExecutionMode {
    match mode {
        BenchmarkMode::Strict => ExecutionMode::Strict,
        BenchmarkMode::Permissive => ExecutionMode::Permissive,
        BenchmarkMode::Experimental => ExecutionMode::Experimental,
    }
}

fn desktop_display_palette_from_benchmark(palette: BenchmarkPalette) -> DesktopDisplayPalette {
    match palette {
        BenchmarkPalette::Grey => DesktopDisplayPalette::Grey,
    }
}

fn load_selected_rom(
    selected_path: PathBuf,
    session: &DesktopSession,
) -> Result<LoadedRom, String> {
    let rom_path = if selected_path.is_absolute() {
        selected_path
    } else {
        resolve_path(&session.current_dir, &selected_path)
    };
    let rom_bytes = match fs::read(&rom_path) {
        Ok(rom_bytes) => rom_bytes,
        Err(error) => {
            return Err(format_path_error(
                "failed to read ROM",
                &rom_path,
                &error.to_string(),
            ));
        }
    };

    Ok(LoadedRom {
        path: rom_path,
        bytes: rom_bytes,
    })
}

fn next_single_external_port_selection(
    current_selection: DesktopExternalPortSelection,
) -> DesktopExternalPortSelection {
    match current_selection {
        DesktopExternalPortSelection::None | DesktopExternalPortSelection::Printer => {
            current_selection
        }
        DesktopExternalPortSelection::GameLink
        | DesktopExternalPortSelection::FourPlayerAdapter => DesktopExternalPortSelection::None,
    }
}
