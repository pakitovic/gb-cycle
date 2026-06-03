#[cfg(test)]
fn open_save_session_for_session(
    session: &DesktopSession,
    machine: &mut Machine<TraceSummaryBuffer>,
) -> Result<Option<DesktopSaveSession>, String> {
    open_save_session_for_player_slot(session, PlayerSlot::P1, machine)
}

fn empty_save_sessions() -> [Option<DesktopSaveSession>; PLAYER_SLOT_COUNT] {
    std::array::from_fn(|_| None)
}

fn open_save_sessions_for_session(
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
) -> Result<[Option<DesktopSaveSession>; PLAYER_SLOT_COUNT], String> {
    let mut save_sessions = empty_save_sessions();
    for slot in PlayerSlot::ALL {
        let Some(slot_machine) = machine.machine_for_player_slot_mut(slot) else {
            continue;
        };
        save_sessions[slot.index()] =
            open_save_session_for_player_slot(session, slot, slot_machine)?;
    }
    Ok(save_sessions)
}

fn open_save_session_for_player_slot(
    session: &DesktopSession,
    slot: PlayerSlot,
    machine: &mut Machine<TraceSummaryBuffer>,
) -> Result<Option<DesktopSaveSession>, String> {
    let Some(rom_path) = save_rom_path_for_player_slot(session, slot) else {
        return Ok(None);
    };

    let save_root = session
        .config
        .saves
        .resolve_directory(rom_path)
        .map(|path| resolve_path(&session.current_dir, &path));
    let save_key = save_key_for_player_slot(session, rom_path)?;
    if slot == PlayerSlot::P1 {
        return DesktopSaveSession::open(
            save_root.as_deref(),
            session.config.saves.flush_policy,
            save_key,
            machine,
        );
    }
    DesktopSaveSession::open_with_file_extension(
        save_root.as_deref(),
        session.config.saves.flush_policy,
        save_key,
        save_file_extension_for_player_slot(slot),
        machine,
    )
}

fn save_rom_path_for_player_slot(session: &DesktopSession, slot: PlayerSlot) -> Option<&Path> {
    if session.cgb_infrared_link_active {
        return match slot {
            PlayerSlot::P1 => session.rom_path(),
            PlayerSlot::P2 => session.linked_secondary_rom_path(),
            PlayerSlot::P3 | PlayerSlot::P4 => None,
        };
    }

    match (session.external_port_selection, slot) {
        (_, PlayerSlot::P1) => session.rom_path(),
        (DesktopExternalPortSelection::GameLink, PlayerSlot::P2) => {
            session.linked_secondary_rom_path()
        }
        (
            DesktopExternalPortSelection::FourPlayerAdapter,
            PlayerSlot::P2 | PlayerSlot::P3 | PlayerSlot::P4,
        ) => session
            .dmg07_player_count
            .is_some_and(|player_count| slot.index() < player_count.get())
            .then(|| session.rom_path())
            .flatten(),
        _ => None,
    }
}

fn save_key_for_player_slot(
    session: &DesktopSession,
    rom_path: &Path,
) -> Result<Option<CartridgeSaveKey>, String> {
    let save_key = session
        .config
        .saves
        .resolve_key(rom_path)
        .map_err(|error| error.to_string())?;
    let Some(save_key) = save_key else {
        return Ok(None);
    };

    Ok(Some(save_key))
}

fn save_file_extension_for_player_slot(slot: PlayerSlot) -> CartridgeSaveFileExtension {
    match slot {
        PlayerSlot::P1 => CartridgeSaveFileExtension::P1,
        PlayerSlot::P2 => CartridgeSaveFileExtension::P2,
        PlayerSlot::P3 => CartridgeSaveFileExtension::P3,
        PlayerSlot::P4 => CartridgeSaveFileExtension::P4,
    }
}

fn window_title(session: &DesktopSession, config: &DesktopConfig) -> String {
    let rom_name = match (
        session.rom_path(),
        session.linked_secondary_rom_path(),
        session.external_port_selection,
        session.dmg07_player_count,
    ) {
        (
            Some(primary_path),
            _,
            DesktopExternalPortSelection::FourPlayerAdapter,
            Some(player_count),
        ) => format!(
            "{} x{}",
            primary_path
                .file_name()
                .unwrap_or(primary_path.as_os_str())
                .to_string_lossy(),
            player_count.get()
        ),
        (Some(primary_path), Some(secondary_path), _, _) => format!(
            "{} + {}",
            primary_path
                .file_name()
                .unwrap_or(primary_path.as_os_str())
                .to_string_lossy(),
            secondary_path
                .file_name()
                .unwrap_or(secondary_path.as_os_str())
                .to_string_lossy(),
        ),
        (Some(rom_path), None, _, _) => rom_path
            .file_name()
            .unwrap_or(rom_path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        (None, _, _, _) => "no ROM loaded".to_string(),
    };
    format!(
        "gb-desktop | {} | {} | {} | {}",
        rom_name,
        config.launch.console_model.name(),
        startup_mode_name(config.launch.startup_mode),
        execution_mode_name(config.launch.execution_mode),
    )
}

fn battery_backed_states_by_player_slot(
    machine: &DesktopEmulationSession,
) -> [Option<PersistentCartState>; PLAYER_SLOT_COUNT] {
    std::array::from_fn(|index| {
        let slot = PlayerSlot::from_machine_index(index)?;
        let machine = machine.machine_for_player_slot(slot)?;
        uses_battery_backed_hardware_persistence(machine.cartridge().persistence_metadata())
            .then(|| machine.cartridge().persistent_state())
    })
}

fn restore_battery_backed_states_by_player_slot(
    machine: &mut DesktopEmulationSession,
    states: &[Option<PersistentCartState>; PLAYER_SLOT_COUNT],
    context: &str,
) -> Result<(), String> {
    for slot in PlayerSlot::ALL {
        let Some(persistent_state) = states[slot.index()].as_ref() else {
            continue;
        };
        let Some(slot_machine) = machine.machine_for_player_slot_mut(slot) else {
            continue;
        };
        if let Err(error) = slot_machine.restore_cartridge_persistent_state(persistent_state) {
            return Err(format!(
                "failed to restore {} battery-backed persistence {context}: {error:?}",
                slot.label()
            ));
        }
    }
    Ok(())
}

fn close_runtime_save_sessions(
    runtime: &mut FrontendRuntime,
    machine: &DesktopEmulationSession,
) -> Result<(), String> {
    for slot in PlayerSlot::ALL {
        if let Some(save_session) = &mut runtime.save_sessions[slot.index()]
            && let Some(slot_machine) = machine.machine_for_player_slot(slot)
        {
            save_session.close(slot_machine)?;
        }
    }
    Ok(())
}

fn flush_runtime_save_sessions_if_changed(
    runtime: &mut FrontendRuntime,
    machine: &DesktopEmulationSession,
    reason: &str,
) -> Result<(), String> {
    for slot in PlayerSlot::ALL {
        if let Some(save_session) = &mut runtime.save_sessions[slot.index()]
            && let Some(slot_machine) = machine.machine_for_player_slot(slot)
        {
            let _ = save_session.flush_if_changed(slot_machine, reason)?;
        }
    }
    Ok(())
}

fn maybe_flush_runtime_save_sessions_at_frame_boundary(
    runtime: &mut FrontendRuntime,
    machine: &DesktopEmulationSession,
    now: Instant,
) -> Result<(), String> {
    for slot in PlayerSlot::ALL {
        if let Some(save_session) = &mut runtime.save_sessions[slot.index()]
            && let Some(slot_machine) = machine.machine_for_player_slot(slot)
        {
            let _ = save_session.maybe_flush_at_frame_boundary(slot_machine, now)?;
        }
    }
    Ok(())
}
