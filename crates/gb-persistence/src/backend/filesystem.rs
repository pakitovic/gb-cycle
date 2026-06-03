use super::{CartridgeSaveBackend, CartridgeSaveBackendError};
use crate::cartridge_envelope::{
    CartridgeSaveBackendMetadata, CartridgeSaveEnvelope, decode_cartridge_save_envelope,
    encode_cartridge_save_envelope,
};
use crate::file_io::write_save_file_with_safe_replace;
use crate::format::CURRENT_SAVE_FORMAT_VERSION;
use crate::key::{CartridgeSaveFileExtension, CartridgeSaveKey};
use crate::time::{CartridgeSaveTimeSource, SystemCartridgeSaveTimeSource};
use gb_core::{CartridgePersistenceMetadata, PersistentCartState};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct FilesystemCartridgeSaveBackend<C = SystemCartridgeSaveTimeSource> {
    root: PathBuf,
    clock: C,
    file_extension: CartridgeSaveFileExtension,
}

impl FilesystemCartridgeSaveBackend<SystemCartridgeSaveTimeSource> {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_time_source(root, SystemCartridgeSaveTimeSource)
    }

    pub fn with_file_extension(
        root: impl Into<PathBuf>,
        file_extension: CartridgeSaveFileExtension,
    ) -> Self {
        Self::with_time_source_and_file_extension(
            root,
            SystemCartridgeSaveTimeSource,
            file_extension,
        )
    }
}

impl<C> FilesystemCartridgeSaveBackend<C> {
    pub fn with_time_source(root: impl Into<PathBuf>, clock: C) -> Self {
        Self::with_time_source_and_file_extension(
            root,
            clock,
            CartridgeSaveFileExtension::default(),
        )
    }

    pub fn with_time_source_and_file_extension(
        root: impl Into<PathBuf>,
        clock: C,
        file_extension: CartridgeSaveFileExtension,
    ) -> Self {
        Self {
            root: root.into(),
            clock,
            file_extension,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn file_extension(&self) -> CartridgeSaveFileExtension {
        self.file_extension
    }

    pub fn path_for_key(&self, key: &CartridgeSaveKey) -> PathBuf {
        self.root
            .join(format!("{}.{}", key.as_str(), self.file_extension.as_str()))
    }
}

impl<C: CartridgeSaveTimeSource> CartridgeSaveBackend for FilesystemCartridgeSaveBackend<C> {
    fn current_unix_seconds(&self) -> u64 {
        self.clock.now_unix_seconds()
    }

    fn load(
        &self,
        key: &CartridgeSaveKey,
    ) -> Result<Option<CartridgeSaveEnvelope>, CartridgeSaveBackendError> {
        let path = self.path_for_key(key);
        match fs::read(&path) {
            Ok(bytes) => decode_cartridge_save_envelope(&bytes).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(CartridgeSaveBackendError::Io {
                operation: "read save file",
                path,
                source,
            }),
        }
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
        let path = self.path_for_key(key);
        write_save_file_with_safe_replace(&path, &bytes)?;
        Ok(envelope)
    }

    fn delete(&mut self, key: &CartridgeSaveKey) -> Result<(), CartridgeSaveBackendError> {
        let path = self.path_for_key(key);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(CartridgeSaveBackendError::Io {
                operation: "delete save file",
                path,
                source,
            }),
        }
    }
}
