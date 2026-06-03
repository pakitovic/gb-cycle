use super::error::CartridgeSaveBackendError;
use crate::cartridge_envelope::CartridgeSaveEnvelope;
use crate::key::CartridgeSaveKey;
use gb_core::{CartridgePersistenceMetadata, PersistentCartState};

pub trait CartridgeSaveBackend {
    fn current_unix_seconds(&self) -> u64;

    fn load(
        &self,
        key: &CartridgeSaveKey,
    ) -> Result<Option<CartridgeSaveEnvelope>, CartridgeSaveBackendError>;

    fn save(
        &mut self,
        key: &CartridgeSaveKey,
        cartridge_metadata: CartridgePersistenceMetadata,
        persistent_state: &PersistentCartState,
    ) -> Result<CartridgeSaveEnvelope, CartridgeSaveBackendError>;

    fn delete(&mut self, key: &CartridgeSaveKey) -> Result<(), CartridgeSaveBackendError>;
}
