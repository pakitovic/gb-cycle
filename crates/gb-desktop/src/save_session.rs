use gb_core::{
    CartridgeLoadError, CartridgePersistentStateError, CartridgeSlot, Machine, PersistentCartState,
    TraceSummaryBuffer,
};
use gb_desktop::{DEFAULT_SAVE_FLUSH_DEBOUNCE, DesktopSaveFlushPolicy};
use gb_persistence::{
    CartridgeSaveFileExtension, CartridgeSaveKey, FilesystemCartridgeSaveStore,
    uses_battery_backed_hardware_persistence,
};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct DesktopSaveSession {
    backend: FilesystemCartridgeSaveStore,
    key: CartridgeSaveKey,
    save_path: PathBuf,
    flush_policy: DesktopSaveFlushPolicy,
    last_saved_state: PersistentCartState,
    pending_debounced_flush_deadline: Option<Instant>,
}

impl DesktopSaveSession {
    pub fn open(
        save_root: Option<&Path>,
        flush_policy: DesktopSaveFlushPolicy,
        key: Option<CartridgeSaveKey>,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) -> Result<Option<Self>, String> {
        Self::open_with_file_extension(
            save_root,
            flush_policy,
            key,
            CartridgeSaveFileExtension::P1,
            machine,
        )
    }

    pub fn open_with_file_extension(
        save_root: Option<&Path>,
        flush_policy: DesktopSaveFlushPolicy,
        key: Option<CartridgeSaveKey>,
        file_extension: CartridgeSaveFileExtension,
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

        let backend = FilesystemCartridgeSaveStore::with_file_extension(save_root, file_extension);
        let target_state = machine.cartridge().persistent_state();
        let mut save_path = backend.preferred_path_for_state(&key, metadata, &target_state);
        let load_result = backend
            .load(&key, metadata, &target_state)
            .map_err(|error| format!("failed to load save {}: {error}", save_path.display()))?;

        if let Some(load) = load_result {
            save_path = load.path;
            let elapsed_seconds = backend
                .current_unix_seconds()
                .saturating_sub(load.envelope.backend_metadata.saved_at_unix_seconds);
            let mut restored_state = load.envelope.persistent_state;
            apply_elapsed_off_session_seconds(&mut restored_state, elapsed_seconds);
            machine
                .restore_cartridge_persistent_state(&restored_state)
                .map_err(format_restore_error)?;
        }

        let last_saved_state = machine.cartridge().persistent_state();
        Ok(Some(Self {
            backend,
            key,
            save_path,
            flush_policy,
            last_saved_state,
            pending_debounced_flush_deadline: None,
        }))
    }

    #[allow(dead_code)]
    pub fn save_path(&self) -> PathBuf {
        self.save_path.clone()
    }

    pub fn flush_policy(&self) -> DesktopSaveFlushPolicy {
        self.flush_policy
    }

    pub fn reset_baseline_from_machine(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        self.last_saved_state = machine.cartridge().persistent_state();
        self.pending_debounced_flush_deadline = None;
    }

    pub fn maybe_flush_at_frame_boundary(
        &mut self,
        machine: &Machine<TraceSummaryBuffer>,
        now: Instant,
    ) -> Result<bool, String> {
        match self.flush_policy {
            DesktopSaveFlushPolicy::Manual | DesktopSaveFlushPolicy::OnClose => Ok(false),
            DesktopSaveFlushPolicy::OnWrite => self.flush_if_changed(machine, "frame-boundary"),
            DesktopSaveFlushPolicy::Debounced => self.flush_if_debounced(machine, now),
        }
    }

    pub fn flush_if_changed(
        &mut self,
        machine: &Machine<TraceSummaryBuffer>,
        reason: &str,
    ) -> Result<bool, String> {
        let current_state = machine.cartridge().persistent_state();
        self.flush_current_state_if_changed(machine, current_state, reason)
    }

    pub fn close(&mut self, machine: &Machine<TraceSummaryBuffer>) -> Result<(), String> {
        if self.flush_policy.flush_on_close() {
            self.flush_if_changed(machine, "close").map(|_| ())
        } else {
            Ok(())
        }
    }

    fn flush_if_debounced(
        &mut self,
        machine: &Machine<TraceSummaryBuffer>,
        now: Instant,
    ) -> Result<bool, String> {
        let current_state = machine.cartridge().persistent_state();
        if current_state == self.last_saved_state {
            self.pending_debounced_flush_deadline = None;
            return Ok(false);
        }

        let deadline = self
            .pending_debounced_flush_deadline
            .get_or_insert(now + DEFAULT_SAVE_FLUSH_DEBOUNCE);
        if now < *deadline {
            return Ok(false);
        }

        self.flush_current_state_if_changed(machine, current_state, "debounced-frame-boundary")
    }

    fn flush_current_state_if_changed(
        &mut self,
        machine: &Machine<TraceSummaryBuffer>,
        current_state: PersistentCartState,
        reason: &str,
    ) -> Result<bool, String> {
        if current_state == self.last_saved_state {
            self.pending_debounced_flush_deadline = None;
            return Ok(false);
        }

        let save_path = self.backend.preferred_path_for_state(
            &self.key,
            machine.cartridge().persistence_metadata(),
            &current_state,
        );
        let write = self
            .backend
            .save(
                &self.key,
                machine.cartridge().persistence_metadata(),
                &current_state,
            )
            .map_err(|error| {
                format!(
                    "failed to save cartridge persistence ({reason}) to {}: {error}",
                    save_path.display()
                )
            })?;
        self.save_path = write.path;
        self.last_saved_state = current_state;
        self.pending_debounced_flush_deadline = None;
        Ok(true)
    }
}

fn apply_elapsed_off_session_seconds(state: &mut PersistentCartState, elapsed_seconds: u64) {
    match state {
        PersistentCartState::Mbc3Rtc { rtc } | PersistentCartState::Mbc3RamRtc { rtc, .. } => {
            rtc.apply_elapsed_seconds(elapsed_seconds);
        }
        PersistentCartState::Huc3 { rtc, .. } => rtc.apply_elapsed_seconds(elapsed_seconds),
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

#[cfg(test)]
mod test;
