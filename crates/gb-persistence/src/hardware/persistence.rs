use crate::backend::{CartridgeSaveBackend, CartridgeSaveBackendError};
use crate::cartridge_envelope::CartridgeSaveEnvelope;
use crate::key::CartridgeSaveKey;
use gb_core::{
    CartridgePersistenceMetadata, CartridgePersistenceProfile, CartridgePersistentStateError,
    CartridgeSlot, PersistentCartState,
};
use std::fmt;

// These host-side result enums intentionally carry the full save envelope so
// callers and tests can inspect the exact persisted payload without a second
// lookup or hidden indirection.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwarePersistenceLoadResult {
    SkippedNotBatteryBacked,
    NoSavePresent,
    Restored {
        persisted: CartridgeSaveEnvelope,
        elapsed_off_session_seconds: u64,
    },
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwarePersistenceSaveResult {
    SkippedNotBatteryBacked,
    Saved(CartridgeSaveEnvelope),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwarePersistenceFlushPolicy {
    Manual,
    SaveOnClose,
    AutoFlushAfterPersistibleWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwarePersistenceTrigger {
    PersistibleWrite,
    ManualFlush,
    ForcedSave,
    Close,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwarePersistenceActionResult {
    SkippedNotBatteryBacked,
    Deferred,
    NoPendingSave,
    SkippedByFlushPolicy {
        trigger: HardwarePersistenceTrigger,
    },
    Saved {
        trigger: HardwarePersistenceTrigger,
        envelope: CartridgeSaveEnvelope,
    },
}

#[derive(Debug)]
pub struct HardwarePersistenceManager<B> {
    backend: B,
    key: CartridgeSaveKey,
    flush_policy: HardwarePersistenceFlushPolicy,
    dirty: bool,
}

#[derive(Debug)]
pub enum HardwarePersistenceError {
    Backend(CartridgeSaveBackendError),
    Restore(CartridgePersistentStateError),
}

impl fmt::Display for HardwarePersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(error) => error.fmt(f),
            Self::Restore(error) => write!(f, "cartridge restore failed: {error:?}"),
        }
    }
}

impl std::error::Error for HardwarePersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend(error) => Some(error),
            Self::Restore(_) => None,
        }
    }
}

pub fn uses_battery_backed_hardware_persistence(metadata: CartridgePersistenceMetadata) -> bool {
    matches!(
        metadata.profile,
        CartridgePersistenceProfile::PersistentEeprom { .. }
    ) || metadata.has_battery
        && matches!(
            metadata.profile,
            CartridgePersistenceProfile::PersistentRam { .. }
                | CartridgePersistenceProfile::PersistentRtc
                | CartridgePersistenceProfile::PersistentRamAndRtc { .. }
                | CartridgePersistenceProfile::PersistentRamAndFlash { .. }
        )
}

pub fn load_hardware_cartridge_persistence<B: CartridgeSaveBackend>(
    backend: &B,
    key: &CartridgeSaveKey,
    cartridge: &mut CartridgeSlot,
) -> Result<HardwarePersistenceLoadResult, HardwarePersistenceError> {
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Ok(HardwarePersistenceLoadResult::SkippedNotBatteryBacked);
    }

    match backend
        .load(key)
        .map_err(HardwarePersistenceError::Backend)?
    {
        Some(envelope) => {
            let elapsed_off_session_seconds = backend
                .current_unix_seconds()
                .saturating_sub(envelope.backend_metadata.saved_at_unix_seconds);
            let mut restored_state = envelope.persistent_state.clone();
            apply_elapsed_off_session_seconds(&mut restored_state, elapsed_off_session_seconds);
            cartridge
                .restore_persistent_state(&restored_state)
                .map_err(HardwarePersistenceError::Restore)?;
            Ok(HardwarePersistenceLoadResult::Restored {
                persisted: envelope,
                elapsed_off_session_seconds,
            })
        }
        None => Ok(HardwarePersistenceLoadResult::NoSavePresent),
    }
}

pub fn save_hardware_cartridge_persistence<B: CartridgeSaveBackend>(
    backend: &mut B,
    key: &CartridgeSaveKey,
    cartridge: &CartridgeSlot,
) -> Result<HardwarePersistenceSaveResult, HardwarePersistenceError> {
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Ok(HardwarePersistenceSaveResult::SkippedNotBatteryBacked);
    }

    let envelope = backend
        .save(key, metadata, &cartridge.persistent_state())
        .map_err(HardwarePersistenceError::Backend)?;
    Ok(HardwarePersistenceSaveResult::Saved(envelope))
}

impl<B> HardwarePersistenceManager<B> {
    pub fn new(
        backend: B,
        key: CartridgeSaveKey,
        flush_policy: HardwarePersistenceFlushPolicy,
    ) -> Self {
        Self {
            backend,
            key,
            flush_policy,
            dirty: false,
        }
    }

    pub fn key(&self) -> &CartridgeSaveKey {
        &self.key
    }

    pub fn flush_policy(&self) -> HardwarePersistenceFlushPolicy {
        self.flush_policy
    }

    pub fn set_flush_policy(&mut self, flush_policy: HardwarePersistenceFlushPolicy) {
        self.flush_policy = flush_policy;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn into_backend(self) -> B {
        self.backend
    }
}

impl<B: CartridgeSaveBackend> HardwarePersistenceManager<B> {
    pub fn load_into(
        &mut self,
        cartridge: &mut CartridgeSlot,
    ) -> Result<HardwarePersistenceLoadResult, HardwarePersistenceError> {
        let result = load_hardware_cartridge_persistence(&self.backend, &self.key, cartridge)?;
        self.dirty = false;
        Ok(result)
    }

    pub fn note_persistible_write(
        &mut self,
        cartridge: &CartridgeSlot,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        if !uses_battery_backed_hardware_persistence(cartridge.persistence_metadata()) {
            self.dirty = false;
            return Ok(HardwarePersistenceActionResult::SkippedNotBatteryBacked);
        }

        self.dirty = true;
        match self.flush_policy {
            HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite => self.perform_save(
                cartridge,
                HardwarePersistenceTrigger::PersistibleWrite,
                true,
            ),
            HardwarePersistenceFlushPolicy::Manual
            | HardwarePersistenceFlushPolicy::SaveOnClose => {
                Ok(HardwarePersistenceActionResult::Deferred)
            }
        }
    }

    pub fn flush(
        &mut self,
        cartridge: &CartridgeSlot,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        self.perform_save(cartridge, HardwarePersistenceTrigger::ManualFlush, false)
    }

    pub fn force_save(
        &mut self,
        cartridge: &CartridgeSlot,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        self.perform_save(cartridge, HardwarePersistenceTrigger::ForcedSave, true)
    }

    pub fn close(
        &mut self,
        cartridge: &CartridgeSlot,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        if !uses_battery_backed_hardware_persistence(cartridge.persistence_metadata()) {
            self.dirty = false;
            return Ok(HardwarePersistenceActionResult::SkippedNotBatteryBacked);
        }

        if !self.dirty {
            return Ok(HardwarePersistenceActionResult::NoPendingSave);
        }

        match self.flush_policy {
            HardwarePersistenceFlushPolicy::Manual => {
                Ok(HardwarePersistenceActionResult::SkippedByFlushPolicy {
                    trigger: HardwarePersistenceTrigger::Close,
                })
            }
            HardwarePersistenceFlushPolicy::SaveOnClose
            | HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite => {
                self.perform_save(cartridge, HardwarePersistenceTrigger::Close, true)
            }
        }
    }

    fn perform_save(
        &mut self,
        cartridge: &CartridgeSlot,
        trigger: HardwarePersistenceTrigger,
        save_when_clean: bool,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        if !uses_battery_backed_hardware_persistence(cartridge.persistence_metadata()) {
            self.dirty = false;
            return Ok(HardwarePersistenceActionResult::SkippedNotBatteryBacked);
        }

        if !save_when_clean && !self.dirty {
            return Ok(HardwarePersistenceActionResult::NoPendingSave);
        }

        match save_hardware_cartridge_persistence(&mut self.backend, &self.key, cartridge)? {
            HardwarePersistenceSaveResult::SkippedNotBatteryBacked => {
                self.dirty = false;
                Ok(HardwarePersistenceActionResult::SkippedNotBatteryBacked)
            }
            HardwarePersistenceSaveResult::Saved(envelope) => {
                self.dirty = false;
                Ok(HardwarePersistenceActionResult::Saved { trigger, envelope })
            }
        }
    }
}

pub(crate) fn apply_elapsed_off_session_seconds(
    state: &mut PersistentCartState,
    elapsed_seconds: u64,
) {
    match state {
        PersistentCartState::Mbc3Rtc { rtc } => rtc.apply_elapsed_seconds(elapsed_seconds),
        PersistentCartState::Mbc3RamRtc { rtc, .. } => rtc.apply_elapsed_seconds(elapsed_seconds),
        PersistentCartState::Huc3 { rtc, .. } => rtc.apply_elapsed_seconds(elapsed_seconds),
        _ => {}
    }
}
