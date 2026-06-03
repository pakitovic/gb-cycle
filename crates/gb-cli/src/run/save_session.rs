use crate::host_io::writeln_checked;
use crate::options::RunOptions;
use crate::report::{format_save_flush_error, format_save_load_error};
use crate::run::machine::CliMachine;
use crate::save_key::{derive_save_key, parse_save_key};
use gb_core::PersistentCartState;
use gb_persistence::{
    CartridgeSaveKey, FilesystemCartridgeSaveStore, uses_battery_backed_hardware_persistence,
};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct SaveSession {
    pub(crate) backend: FilesystemCartridgeSaveStore,
    pub(crate) key: CartridgeSaveKey,
    pub(crate) save_path: PathBuf,
    pub(crate) last_saved_state: PersistentCartState,
    pub(crate) loaded_existing_save: bool,
    pub(crate) save_writes: usize,
}

impl SaveSession {
    pub(crate) fn save_path(&self) -> PathBuf {
        self.save_path.clone()
    }
}

pub(crate) fn open_save_session(
    save_root: Option<&Path>,
    options: &RunOptions,
    rom_path: &Path,
    machine: &mut CliMachine,
    stderr: &mut dyn Write,
    load_existing_save: bool,
) -> Result<Option<SaveSession>, String> {
    let Some(save_root) = save_root else {
        return Ok(None);
    };

    let metadata = machine.cartridge().persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        writeln_checked(stderr, "save=skipped not_battery_backed=true")?;
        return Ok(None);
    }

    let key = if let Some(key) = &options.save_key {
        parse_save_key(key)?
    } else {
        derive_save_key(rom_path)?
    };

    let backend = FilesystemCartridgeSaveStore::new(save_root);
    let mut loaded_existing_save = false;
    let mut last_saved_state = machine.cartridge().persistent_state();
    let mut save_path =
        backend.preferred_path_for_state(&key, metadata, &machine.cartridge().persistent_state());

    if load_existing_save
        && let Some(load) = backend
            .load(&key, metadata, &machine.cartridge().persistent_state())
            .map_err(|error| {
                format_save_load_error(
                    &backend.preferred_path_for_state(
                        &key,
                        metadata,
                        &machine.cartridge().persistent_state(),
                    ),
                    error,
                )
            })?
    {
        save_path = load.path;
        let elapsed_seconds = backend
            .current_unix_seconds()
            .saturating_sub(load.envelope.backend_metadata.saved_at_unix_seconds);
        let mut restored_state = load.envelope.persistent_state;
        apply_elapsed_off_session_seconds(&mut restored_state, elapsed_seconds);
        machine
            .restore_cartridge_persistent_state(&restored_state)
            .map_err(|error| format!("failed to restore cartridge persistence: {error:?}"))?;
        last_saved_state = machine.cartridge().persistent_state();
        loaded_existing_save = true;
        writeln_checked(
            stderr,
            &format!(
                "save_loaded path={} elapsed_seconds={elapsed_seconds}",
                save_path.display()
            ),
        )?;
    }

    Ok(Some(SaveSession {
        backend,
        key,
        save_path,
        last_saved_state,
        loaded_existing_save,
        save_writes: 0,
    }))
}

pub(crate) fn flush_save_if_changed(
    save_session: &mut SaveSession,
    machine: &CliMachine,
    reason: &str,
) -> Result<bool, String> {
    let current_state = machine.cartridge().persistent_state();
    if current_state == save_session.last_saved_state {
        return Ok(false);
    }

    let save_path = save_session.backend.preferred_path_for_state(
        &save_session.key,
        machine.cartridge().persistence_metadata(),
        &current_state,
    );
    let write = save_session
        .backend
        .save(
            &save_session.key,
            machine.cartridge().persistence_metadata(),
            &current_state,
        )
        .map_err(|error| format_save_flush_error(&save_path, reason, error))?;
    save_session.save_path = write.path;
    save_session.last_saved_state = current_state;
    save_session.save_writes += 1;
    Ok(true)
}

pub(crate) fn apply_elapsed_off_session_seconds(
    state: &mut PersistentCartState,
    elapsed_seconds: u64,
) {
    match state {
        PersistentCartState::Mbc3Rtc { rtc } | PersistentCartState::Mbc3RamRtc { rtc, .. } => {
            rtc.apply_elapsed_seconds(elapsed_seconds);
        }
        PersistentCartState::Huc3 { rtc, .. } => rtc.apply_elapsed_seconds(elapsed_seconds),
        _ => {}
    }
}
