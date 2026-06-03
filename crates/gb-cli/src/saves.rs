use crate::host_io::{
    resolve_path, validate_directory_input, write_bytes_with_parent, writeln_checked,
};
use crate::options::{SavesDirection, SavesOptions};
use crate::report::{
    format_cartridge_load_error, format_external_save_error, format_save_flush_error,
    format_save_load_error, write_cartridge_diagnostics,
};
use crate::save_key::{derive_save_key, parse_save_key};
use gb_core::{CartridgeSlot, CompatibilityPolicy};
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveEnvelope, CartridgeSaveKey, CartridgeSaveTimeSource,
    EXTERNAL_SAVE_FILE_EXTENSION, FilesystemCartridgeSaveBackend, FilesystemCartridgeSaveStore,
    FixedCartridgeSaveTimeSource, SystemCartridgeSaveTimeSource, export_external_cartridge_save,
    import_external_cartridge_save, uses_battery_backed_hardware_persistence,
};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) fn saves_command(
    options: SavesOptions,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    match options.direction {
        SavesDirection::Export => saves_export_command(options, stdout, stderr),
        SavesDirection::Import => saves_import_command(options, stdout, stderr),
    }
}

pub(crate) fn saves_export_command(
    options: SavesOptions,
    output: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let rom_path = resolve_path(&current_dir, &options.rom_path);
    let mut cartridge = load_cartridge_for_save_conversion(&rom_path, stderr)?;
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(format!(
            "ROM {} does not expose battery-backed cartridge persistence",
            rom_path.display()
        ));
    }

    let save_root = resolve_path(&current_dir, &options.save_dir);
    validate_directory_input("--save-dir", &save_root)?;
    let key = resolve_saves_key(options.save_key.as_deref(), &rom_path)?;
    let store = FilesystemCartridgeSaveStore::new(&save_root);
    let target_state = cartridge.persistent_state();
    let runtime_save_path = store.preferred_path_for_state(&key, metadata, &target_state);
    let backend = FilesystemCartridgeSaveBackend::new(&save_root);
    let legacy_save_path = backend.path_for_key(&key);
    let (envelope, source_save_path) = match store
        .load(&key, metadata, &target_state)
        .map_err(|error| format_save_load_error(&runtime_save_path, error))?
    {
        Some(load) => (load.envelope, load.path),
        None => load_save_envelope(&backend, &key)?.ok_or_else(|| {
            if runtime_save_path == legacy_save_path {
                format!("no gb-cycle save found at {}", runtime_save_path.display())
            } else {
                format!(
                    "no gb-cycle save found at {} or {}",
                    runtime_save_path.display(),
                    legacy_save_path.display()
                )
            }
        })?,
    };
    cartridge
        .restore_persistent_state(&envelope.persistent_state)
        .map_err(|error| {
            format!(
                "save {} is not compatible with ROM {}: {error:?}",
                source_save_path.display(),
                rom_path.display()
            )
        })?;

    let external_bytes = export_external_cartridge_save(&envelope, store.current_unix_seconds())
        .map_err(format_external_save_error)?;
    let external_path = resolve_path(&current_dir, &options.external_save_path);
    write_bytes_with_parent(&external_path, &external_bytes)?;

    writeln_checked(output, &format!("rom={}", rom_path.display()))?;
    writeln_checked(output, &format!("save_key={}", key.as_str()))?;
    writeln_checked(
        output,
        &format!("source_save={}", source_save_path.display()),
    )?;
    writeln_checked(
        output,
        &format!("external_save={}", external_path.display()),
    )?;
    writeln_checked(output, &format!("external_bytes={}", external_bytes.len()))?;
    Ok(())
}

pub(crate) fn load_save_envelope(
    backend: &FilesystemCartridgeSaveBackend,
    key: &CartridgeSaveKey,
) -> Result<Option<(CartridgeSaveEnvelope, PathBuf)>, String> {
    let save_path = backend.path_for_key(key);
    backend
        .load(key)
        .map_err(|error| format_save_load_error(&save_path, error))
        .map(|envelope| envelope.map(|envelope| (envelope, save_path)))
}

pub(crate) fn saves_import_command(
    options: SavesOptions,
    output: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let rom_path = resolve_path(&current_dir, &options.rom_path);
    let mut cartridge = load_cartridge_for_save_conversion(&rom_path, stderr)?;
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(format!(
            "ROM {} does not expose battery-backed cartridge persistence",
            rom_path.display()
        ));
    }

    let external_path = resolve_path(&current_dir, &options.external_save_path);
    let external_bytes = fs::read(&external_path).map_err(|error| {
        format!(
            "failed to read external .{} save {}: {error}",
            EXTERNAL_SAVE_FILE_EXTENSION,
            external_path.display()
        )
    })?;
    let target_state = cartridge.persistent_state();
    let save_root = resolve_path(&current_dir, &options.save_dir);
    validate_directory_input("--save-dir", &save_root)?;
    let import_unix_seconds = SystemCartridgeSaveTimeSource.now_unix_seconds();
    let mut store = FilesystemCartridgeSaveStore::with_time_source(
        &save_root,
        FixedCartridgeSaveTimeSource::new(import_unix_seconds),
    );
    let imported_state = import_external_cartridge_save(
        metadata,
        &target_state,
        &external_bytes,
        import_unix_seconds,
    )
    .map_err(format_external_save_error)?;
    cartridge
        .restore_persistent_state(&imported_state)
        .map_err(|error| {
            format!(
                "external save {} is not compatible with ROM {}: {error:?}",
                external_path.display(),
                rom_path.display()
            )
        })?;

    let key = resolve_saves_key(options.save_key.as_deref(), &rom_path)?;
    let target_save_path = store.preferred_path_for_state(&key, metadata, &imported_state);
    let write = store
        .save(&key, metadata, &imported_state)
        .map_err(|error| format_save_flush_error(&target_save_path, "saves-import", error))?;

    writeln_checked(output, &format!("rom={}", rom_path.display()))?;
    writeln_checked(output, &format!("save_key={}", key.as_str()))?;
    writeln_checked(
        output,
        &format!("external_save={}", external_path.display()),
    )?;
    writeln_checked(output, &format!("target_save={}", write.path.display()))?;
    writeln_checked(
        output,
        &format!(
            "saved_at_unix_seconds={}",
            write.envelope.backend_metadata.saved_at_unix_seconds
        ),
    )?;
    Ok(())
}

pub(crate) fn load_cartridge_for_save_conversion(
    rom_path: &Path,
    stderr: &mut dyn Write,
) -> Result<CartridgeSlot, String> {
    let rom_bytes = fs::read(rom_path)
        .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;
    let report = CartridgeSlot::load(rom_bytes, &CompatibilityPolicy::strict())
        .map_err(format_cartridge_load_error)?;
    write_cartridge_diagnostics(stderr, report.diagnostics())?;
    Ok(report.cartridge().clone())
}

pub(crate) fn resolve_saves_key(
    explicit_key: Option<&str>,
    rom_path: &Path,
) -> Result<CartridgeSaveKey, String> {
    if let Some(key) = explicit_key {
        parse_save_key(key)
    } else {
        derive_save_key(rom_path)
    }
}
