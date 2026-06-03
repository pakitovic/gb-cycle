use super::{CartridgeSaveBackend, CartridgeSaveBackendError};
use crate::cartridge_envelope::{
    CartridgeSaveBackendMetadata, CartridgeSaveEnvelope, decode_cartridge_save_envelope,
    encode_cartridge_save_envelope,
};
use crate::format::CURRENT_SAVE_FORMAT_VERSION;
use crate::key::CartridgeSaveKey;
use crate::time::{CartridgeSaveTimeSource, SystemCartridgeSaveTimeSource};
use gb_core::{CartridgePersistenceMetadata, PersistentCartState};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct InMemoryCartridgeSaveBackend<C = SystemCartridgeSaveTimeSource> {
    clock: C,
    entries: BTreeMap<CartridgeSaveKey, Vec<u8>>,
}

impl InMemoryCartridgeSaveBackend<SystemCartridgeSaveTimeSource> {
    pub fn new() -> Self {
        Self::with_time_source(SystemCartridgeSaveTimeSource)
    }
}

impl Default for InMemoryCartridgeSaveBackend<SystemCartridgeSaveTimeSource> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> InMemoryCartridgeSaveBackend<C> {
    pub fn with_time_source(clock: C) -> Self {
        Self {
            clock,
            entries: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<C: CartridgeSaveTimeSource> CartridgeSaveBackend for InMemoryCartridgeSaveBackend<C> {
    fn current_unix_seconds(&self) -> u64 {
        self.clock.now_unix_seconds()
    }

    fn load(
        &self,
        key: &CartridgeSaveKey,
    ) -> Result<Option<CartridgeSaveEnvelope>, CartridgeSaveBackendError> {
        self.entries
            .get(key)
            .map(|bytes| decode_cartridge_save_envelope(bytes))
            .transpose()
    }

    fn save(
        &mut self,
        key: &CartridgeSaveKey,
        cartridge_metadata: CartridgePersistenceMetadata,
        persistent_state: &PersistentCartState,
    ) -> Result<CartridgeSaveEnvelope, CartridgeSaveBackendError> {
        let envelope = CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: self.clock.now_unix_seconds(),
            },
            cartridge_metadata,
            persistent_state: persistent_state.clone(),
        };
        let bytes = encode_cartridge_save_envelope(&envelope)?;
        self.entries.insert(key.clone(), bytes);
        Ok(envelope)
    }

    fn delete(&mut self, key: &CartridgeSaveKey) -> Result<(), CartridgeSaveBackendError> {
        self.entries.remove(key);
        Ok(())
    }
}
