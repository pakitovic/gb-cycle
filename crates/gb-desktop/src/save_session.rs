use gb_core::{
    CartridgeLoadError, CartridgePersistentStateError, CartridgeSlot, Machine, PersistentCartState,
    TraceSummaryBuffer,
};
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveKey, FilesystemCartridgeSaveBackend,
    HardwarePersistenceFlushPolicy, uses_battery_backed_hardware_persistence,
};
use std::path::{Path, PathBuf};

pub struct DesktopSaveSession {
    backend: FilesystemCartridgeSaveBackend,
    key: CartridgeSaveKey,
    flush_policy: HardwarePersistenceFlushPolicy,
    last_saved_state: PersistentCartState,
}

impl DesktopSaveSession {
    pub fn open(
        save_root: Option<&Path>,
        flush_policy: HardwarePersistenceFlushPolicy,
        key: Option<CartridgeSaveKey>,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) -> Result<Option<Self>, String> {
        let Some(save_root) = save_root else {
            return Ok(None);
        };

        let metadata = machine.cartridge().persistence_metadata();
        if !uses_battery_backed_hardware_persistence(metadata) {
            return Ok(None);
        }

        let Some(key) = key else {
            return Ok(None);
        };

        let backend = FilesystemCartridgeSaveBackend::new(save_root);
        if let Some(envelope) = backend.load(&key).map_err(|error| {
            format!(
                "failed to load save {}: {error}",
                backend.path_for_key(&key).display()
            )
        })? {
            let elapsed_seconds = backend
                .current_unix_seconds()
                .saturating_sub(envelope.backend_metadata.saved_at_unix_seconds);
            let mut restored_state = envelope.persistent_state;
            apply_elapsed_off_session_seconds(&mut restored_state, elapsed_seconds);
            machine
                .restore_cartridge_persistent_state(&restored_state)
                .map_err(format_restore_error)?;
        }

        let last_saved_state = machine.cartridge().persistent_state();
        Ok(Some(Self {
            backend,
            key,
            flush_policy,
            last_saved_state,
        }))
    }

    pub fn save_path(&self) -> PathBuf {
        self.backend.path_for_key(&self.key)
    }

    pub fn flush_policy(&self) -> HardwarePersistenceFlushPolicy {
        self.flush_policy
    }

    pub fn flush_if_changed(
        &mut self,
        machine: &Machine<TraceSummaryBuffer>,
        reason: &str,
    ) -> Result<bool, String> {
        let current_state = machine.cartridge().persistent_state();
        if current_state == self.last_saved_state {
            return Ok(false);
        }

        self.backend
            .save(
                &self.key,
                machine.cartridge().persistence_metadata(),
                &current_state,
            )
            .map_err(|error| {
                format!(
                    "failed to save cartridge persistence ({reason}) to {}: {error}",
                    self.save_path().display()
                )
            })?;
        self.last_saved_state = current_state;
        Ok(true)
    }

    pub fn close(&mut self, machine: &Machine<TraceSummaryBuffer>) -> Result<(), String> {
        match self.flush_policy {
            HardwarePersistenceFlushPolicy::Manual => Ok(()),
            HardwarePersistenceFlushPolicy::SaveOnClose
            | HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite => {
                self.flush_if_changed(machine, "close").map(|_| ())
            }
        }
    }
}

fn apply_elapsed_off_session_seconds(state: &mut PersistentCartState, elapsed_seconds: u64) {
    match state {
        PersistentCartState::Mbc3Rtc { rtc } | PersistentCartState::Mbc3RamRtc { rtc, .. } => {
            rtc.apply_elapsed_seconds(elapsed_seconds);
        }
        _ => {}
    }
}

fn format_restore_error(error: CartridgePersistentStateError) -> String {
    format!("failed to restore cartridge persistence: {error:?}")
}

#[allow(dead_code)]
fn format_load_error(error: CartridgeLoadError) -> String {
    format!("{error:?}")
}

#[allow(dead_code)]
fn _cartridge(_slot: &CartridgeSlot) {}
