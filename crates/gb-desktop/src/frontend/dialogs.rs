fn map_path_dialog_result(result: Result<Vec<PathBuf>, DialogError>) -> PathDialogResult {
    match result {
        Ok(paths) => paths
            .into_iter()
            .next()
            .map(PathDialogResult::Selected)
            .unwrap_or(PathDialogResult::Canceled),
        Err(DialogError::Canceled) => PathDialogResult::Canceled,
        Err(error) => PathDialogResult::Failed(error.to_string()),
    }
}

fn show_error_message(window: Option<&Window>, title: &str, message: &str) {
    show_message_box(window, MessageBoxFlag::ERROR, title, message);
}

fn show_warning_message(window: Option<&Window>, title: &str, message: &str) {
    show_message_box(window, MessageBoxFlag::WARNING, title, message);
}

fn show_message_box(window: Option<&Window>, flags: MessageBoxFlag, title: &str, message: &str) {
    if let Err(error) = show_simple_message_box(flags, title, message, window) {
        eprintln!("warning: failed to show SDL3 message box '{title}': {error}");
    }
}

pub(crate) fn map_display_result<T, E>(result: Result<T, E>, context: &str) -> Result<T, String>
where
    E: Display,
{
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(format_display_error(context, &error.to_string())),
    }
}

fn format_display_error(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

fn format_debug_error(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

pub(crate) fn format_path_error(context: &str, path: &Path, error: &str) -> String {
    format!("{context} {}: {error}", path.display())
}

pub(crate) fn overflow_error(message: &str) -> String {
    message.to_string()
}

fn toggle_menu(
    event_pump: &sdl3::EventPump,
    window: &Window,
    session: &DesktopSession,
    machine: &mut DesktopEmulationSession,
    runtime: &mut FrontendRuntime,
) -> Result<(), String> {
    if runtime.menu_state.is_open() {
        close_menu(event_pump, machine, runtime)
    } else {
        open_menu(window, machine, session, runtime)
    }
}

fn process_pending_open_rom_dialog(
    event_pump: &sdl3::EventPump,
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.open_rom_dialog.take_result() else {
        return Ok(());
    };
    let open_rom_dialog_mode = context.runtime.open_rom_dialog_mode;
    context.runtime.open_rom_dialog_mode = OpenRomDialogMode::Primary;

    match result {
        PathDialogResult::Selected(path) => {
            let open_result = match open_rom_dialog_mode {
                OpenRomDialogMode::Primary => open_selected_rom(event_pump, canvas, path, context),
                OpenRomDialogMode::LinkedSecondary => {
                    open_selected_linked_secondary_rom(event_pump, canvas, path, context)
                }
                OpenRomDialogMode::CgbInfraredSecondary => {
                    open_selected_cgb_infrared_secondary_rom(event_pump, canvas, path, context)
                }
            };
            if let Err(error) = open_result {
                show_error_message(Some(canvas.window()), "Open ROM failed", &error);
                eprintln!("warning: {error}");
            }
        }
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Open ROM failed",
                &format!("failed to complete SDL3 open ROM dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 open ROM dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn process_pending_camera_image_dialog(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.camera_image_dialog.take_result() else {
        return Ok(());
    };

    match result {
        PathDialogResult::Selected(path) => {
            let result: Result<(), String> = (|| {
                let frame = load_selected_camera_image(path, context.session)?;
                context.runtime.pocket_camera_live.stop();
                apply_pocket_camera_frame_to_desktop_session(&frame, context.machine, "load")?;
                context.session.pocket_camera_frame = Some(frame);
                Ok(())
            })();
            if let Err(error) = result {
                show_error_message(Some(canvas.window()), "Pocket Camera image", &error);
                eprintln!("warning: {error}");
            }
        }
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Pocket Camera image",
                &format!("failed to complete SDL3 Pocket Camera image dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 Pocket Camera image dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn process_pocket_camera_live_frame(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) {
    if !context.runtime.pocket_camera_live.is_enabled() {
        return;
    }

    if !session_has_pocket_camera(context.machine) {
        context.runtime.pocket_camera_live.stop();
        return;
    }

    match context.runtime.pocket_camera_live.poll_frame() {
        Ok(Some(frame)) => {
            let first_live_frame = context.runtime.pocket_camera_live.frames_delivered() == 1;
            if let Err(error) =
                apply_pocket_camera_live_frame_to_desktop_session(&frame, context.machine)
            {
                context.runtime.pocket_camera_live.stop();
                show_warning_message(Some(canvas.window()), "Pocket Camera live", &error);
                eprintln!("warning: {error}");
            } else if first_live_frame {
                eprintln!(
                    "info: first Pocket Camera live frame applied ({}x{})",
                    frame.width, frame.height
                );
            }
        }
        Ok(None) => {
            if context.runtime.pocket_camera_live.polls_without_frame() == 180 {
                let permission_state = context
                    .runtime
                    .pocket_camera_live
                    .permission_state_label()
                    .unwrap_or("closed");
                eprintln!(
                    "info: Pocket Camera live input is still waiting for the first SDL3 camera frame (permission: {permission_state})"
                );
            }
        }
        Err(error) => {
            context.runtime.pocket_camera_live.stop();
            show_warning_message(Some(canvas.window()), "Pocket Camera live", &error);
            eprintln!("warning: {error}");
        }
    }
}

fn process_pending_boot_rom_directory_dialog(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.boot_rom_directory_dialog.take_result() else {
        return Ok(());
    };

    match result {
        PathDialogResult::Selected(path) => {
            apply_machine_settings_change(canvas, context, "Boot ROM directory", |config| {
                config.boot_rom.search_path = Some(path);
            })?;
        }
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Boot ROM directory",
                &format!("failed to complete SDL3 boot ROM directory dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 boot ROM directory dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn process_pending_save_directory_dialog(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.save_directory_dialog.take_result() else {
        return Ok(());
    };

    match result {
        PathDialogResult::Selected(path) => {
            apply_machine_settings_change(canvas, context, "Save directory", |config| {
                config.saves.directory_policy = SaveDirectoryPolicy::Custom(path);
            })?;
        }
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Save directory",
                &format!("failed to complete SDL3 save directory dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 save directory dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn process_pending_external_save_export_dialog(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.external_save_export_dialog.take_result() else {
        return Ok(());
    };

    match result {
        PathDialogResult::Selected(path) => match export_current_external_save(path, context) {
            Ok(export_path) => {
                eprintln!("info: exported external save to {}", export_path.display());
            }
            Err(error) => {
                show_error_message(Some(canvas.window()), "Export save failed", &error);
                eprintln!("warning: {error}");
            }
        },
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Export save failed",
                &format!("failed to complete SDL3 external save export dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 external save export dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn process_pending_external_save_import_dialog(
    canvas: &mut Canvas<Window>,
    context: &mut FrontendActionContext<'_>,
) -> Result<(), String> {
    let Some(result) = context.runtime.external_save_import_dialog.take_result() else {
        return Ok(());
    };

    match result {
        PathDialogResult::Selected(path) => {
            match import_external_save_for_current_rom(path, context) {
                Ok(import_path) => {
                    let message = format!(
                        "Imported external save into {}.\nReload or reset the game before continuing. The active primary save session is disabled until reload so it cannot overwrite the imported save.",
                        import_path.display()
                    );
                    show_warning_message(Some(canvas.window()), "Import save", &message);
                    eprintln!("info: {message}");
                }
                Err(error) => {
                    show_error_message(Some(canvas.window()), "Import save failed", &error);
                    eprintln!("warning: {error}");
                }
            }
        }
        PathDialogResult::Canceled => {}
        PathDialogResult::Failed(error) => {
            show_error_message(
                Some(canvas.window()),
                "Import save failed",
                &format!("failed to complete SDL3 external save import dialog: {error}"),
            );
            eprintln!("warning: failed to complete SDL3 external save import dialog: {error}");
        }
    }

    restore_window_after_native_dialog(canvas);
    Ok(())
}

fn restore_window_after_native_dialog(canvas: &mut Canvas<Window>) {
    let _ = canvas.window_mut().raise();
}

fn boot_rom_dialog_default_location(session: &DesktopSession) -> PathBuf {
    let configured_source = session
        .config
        .boot_rom
        .search_path
        .as_deref()
        .map(|path| resolve_path(&session.current_dir, path));
    match configured_source {
        Some(path) if path.is_dir() => path,
        Some(path) => path
            .parent()
            .unwrap_or(session.current_dir.as_path())
            .to_path_buf(),
        None => session.current_dir.clone(),
    }
}

fn save_directory_dialog_default_location(session: &DesktopSession) -> PathBuf {
    match &session.config.saves.directory_policy {
        SaveDirectoryPolicy::Custom(path) => {
            let path = resolve_path(&session.current_dir, path);
            if path.is_dir() {
                path
            } else {
                path.parent()
                    .unwrap_or(session.current_dir.as_path())
                    .to_path_buf()
            }
        }
        SaveDirectoryPolicy::RomFolderSavesSubdir => session.rom_directory_hint().to_path_buf(),
    }
}

fn external_save_export_dialog_default_location(session: &DesktopSession) -> PathBuf {
    external_save_dialog_default_location(session, "export")
}

fn external_save_import_dialog_default_location(session: &DesktopSession) -> PathBuf {
    external_save_dialog_default_location(session, "import")
}

fn external_save_dialog_default_location(session: &DesktopSession, subdirectory: &str) -> PathBuf {
    if let Some(rom_path) = session.rom_path() {
        return resolve_path(
            &session.current_dir,
            &session.config.saves.directory_policy.resolve(rom_path),
        )
        .join(subdirectory)
        .join(external_save_default_file_name(session));
    }

    session
        .current_dir
        .join("saves")
        .join(subdirectory)
        .join(external_save_default_file_name(session))
}

fn external_save_default_file_name(session: &DesktopSession) -> String {
    let stem = session
        .rom_path()
        .and_then(|rom_path| {
            session
                .config
                .saves
                .resolve_key(rom_path)
                .ok()
                .flatten()
                .map(|key| key.as_str().to_string())
                .or_else(|| {
                    rom_path
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                })
        })
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "save".to_string());
    format!("{stem}.{EXTERNAL_SAVE_FILE_EXTENSION}")
}

fn resolve_selected_external_save_path(
    session: &DesktopSession,
    selected_path: PathBuf,
) -> PathBuf {
    if selected_path.is_absolute() {
        selected_path
    } else {
        resolve_path(&session.current_dir, &selected_path)
    }
}

fn resolve_external_save_export_path(session: &DesktopSession, selected_path: PathBuf) -> PathBuf {
    let mut path = resolve_selected_external_save_path(session, selected_path);
    if path.extension().is_none() {
        path.set_extension(EXTERNAL_SAVE_FILE_EXTENSION);
    }
    path
}

fn resolve_external_save_import_path(session: &DesktopSession, selected_path: PathBuf) -> PathBuf {
    resolve_selected_external_save_path(session, selected_path)
}

fn primary_save_root_and_key(
    session: &DesktopSession,
) -> Result<Option<(PathBuf, CartridgeSaveKey)>, String> {
    let Some(rom_path) = session.rom_path() else {
        return Ok(None);
    };
    let Some(save_root) = session.config.saves.resolve_directory(rom_path) else {
        return Ok(None);
    };
    let save_key = session
        .config
        .saves
        .resolve_key(rom_path)
        .map_err(|error| error.to_string())?;
    Ok(save_key.map(|key| (resolve_path(&session.current_dir, &save_root), key)))
}

fn export_current_external_save(
    selected_path: PathBuf,
    context: &mut FrontendActionContext<'_>,
) -> Result<PathBuf, String> {
    context.runtime.rtc_sync.apply_to_machine(context.machine);
    let cartridge = context.machine.primary_machine().cartridge();
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(
            "current game does not expose battery-backed persistence to export".to_string(),
        );
    }

    let export_path = resolve_external_save_export_path(context.session, selected_path);
    let current_unix_seconds = SystemCartridgeSaveTimeSource.now_unix_seconds();
    let external_bytes = encode_external_cartridge_save(
        metadata,
        &cartridge.persistent_state(),
        current_unix_seconds,
        ExternalSaveExportFormat::default(),
    )
    .map_err(|error| format_external_save_error("export", error))?;

    if let Some(parent) = export_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format_path_error(
                "failed to create external save export directory",
                parent,
                &error.to_string(),
            )
        })?;
    }
    fs::write(&export_path, external_bytes).map_err(|error| {
        format_path_error(
            "failed to write external save",
            &export_path,
            &error.to_string(),
        )
    })?;
    Ok(export_path)
}

fn import_external_save_for_current_rom(
    selected_path: PathBuf,
    context: &mut FrontendActionContext<'_>,
) -> Result<PathBuf, String> {
    let import_path = resolve_external_save_import_path(context.session, selected_path);
    let (save_root, save_key) = primary_save_root_and_key(context.session)?.ok_or_else(|| {
        "save support is disabled or no current ROM save key could be resolved".to_string()
    })?;
    let external_bytes = fs::read(&import_path).map_err(|error| {
        format_path_error(
            "failed to read external save",
            &import_path,
            &error.to_string(),
        )
    })?;

    let cartridge = context.machine.primary_machine().cartridge();
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(
            "current game does not expose battery-backed persistence to import".to_string(),
        );
    }

    let import_unix_seconds = SystemCartridgeSaveTimeSource.now_unix_seconds();
    let imported_state = import_external_cartridge_save(
        metadata,
        &cartridge.persistent_state(),
        &external_bytes,
        import_unix_seconds,
    )
    .map_err(|error| format_external_save_error("import", error))?;

    let mut validation_cartridge = cartridge.clone();
    validation_cartridge
        .restore_persistent_state(&imported_state)
        .map_err(|error| {
            format_debug_error(
                "imported external save does not match the current cartridge",
                &format!("{error:?}"),
            )
        })?;

    let mut previous_primary_save_session =
        context.runtime.save_sessions[PlayerSlot::P1.index()].take();
    if let Some(save_session) = previous_primary_save_session.as_mut()
        && let Err(error) = save_session.close(context.machine.primary_machine())
    {
        context.runtime.save_sessions[PlayerSlot::P1.index()] = previous_primary_save_session;
        return Err(error);
    }

    let mut store = FilesystemCartridgeSaveStore::with_time_source(
        save_root,
        FixedCartridgeSaveTimeSource::new(import_unix_seconds),
    );
    let save_path = store.preferred_path_for_state(&save_key, metadata, &imported_state);
    let save_result = store.save(&save_key, metadata, &imported_state);
    match save_result {
        Ok(write) => {
            context.runtime.save_sessions[PlayerSlot::P1.index()] = None;
            Ok(write.path)
        }
        Err(error) => {
            context.runtime.save_sessions[PlayerSlot::P1.index()] = previous_primary_save_session;
            Err(format_path_error(
                "failed to write imported save",
                &save_path,
                &error.to_string(),
            ))
        }
    }
}

fn format_external_save_error(action: &str, error: ExternalSaveError) -> String {
    format!("failed to {action} external save: {error}")
}

fn load_selected_camera_image(
    selected_path: PathBuf,
    session: &DesktopSession,
) -> Result<PocketCameraFrame, String> {
    let image_path = if selected_path.is_absolute() {
        selected_path
    } else {
        resolve_path(&session.current_dir, &selected_path)
    };
    let file = fs::File::open(&image_path).map_err(|error| {
        format_path_error(
            "failed to read Pocket Camera image",
            &image_path,
            &error.to_string(),
        )
    })?;

    let mut decoder = Decoder::new(BufReader::new(file));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| {
        format_path_error(
            "failed to decode PNG metadata",
            &image_path,
            &error.to_string(),
        )
    })?;
    let output_buffer_size = reader.output_buffer_size().ok_or_else(|| {
        format_path_error(
            "failed to decode PNG metadata",
            &image_path,
            "decoded PNG output buffer is too large",
        )
    })?;
    let mut buffer = vec![0; output_buffer_size];
    let info = reader.next_frame(&mut buffer).map_err(|error| {
        format_path_error(
            "failed to decode PNG image",
            &image_path,
            &error.to_string(),
        )
    })?;
    let width = u16::try_from(info.width).map_err(|_| {
        format_path_error(
            "PNG width exceeds Pocket Camera limits",
            &image_path,
            &info.width.to_string(),
        )
    })?;
    let height = u16::try_from(info.height).map_err(|_| {
        format_path_error(
            "PNG height exceeds Pocket Camera limits",
            &image_path,
            &info.height.to_string(),
        )
    })?;
    let frame_bytes = &buffer[..info.buffer_size()];

    let grayscale_pixels = match info.color_type {
        ColorType::Grayscale => frame_bytes.to_vec(),
        ColorType::GrayscaleAlpha => frame_bytes.chunks_exact(2).map(|chunk| chunk[0]).collect(),
        ColorType::Rgb => frame_bytes
            .chunks_exact(3)
            .map(|chunk| grayscale_from_rgb(chunk[0], chunk[1], chunk[2]))
            .collect(),
        ColorType::Rgba => frame_bytes
            .chunks_exact(4)
            .map(|chunk| grayscale_from_rgb(chunk[0], chunk[1], chunk[2]))
            .collect(),
        ColorType::Indexed => {
            return Err(format_path_error(
                "unsupported indexed PNG after expansion",
                &image_path,
                "decoder left the image indexed",
            ));
        }
    };

    Ok(PocketCameraFrame {
        width,
        height,
        grayscale_pixels,
    })
}

fn grayscale_from_rgb(red: u8, green: u8, blue: u8) -> u8 {
    ((299_u32 * u32::from(red) + 587_u32 * u32::from(green) + 114_u32 * u32::from(blue) + 500)
        / 1000) as u8
}
