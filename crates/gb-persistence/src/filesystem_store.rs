use crate::backend::CartridgeSaveBackendError;
use crate::cartridge_envelope::{
    CartridgeSaveBackendMetadata, CartridgeSaveEnvelope, decode_cartridge_save_envelope,
    encode_cartridge_save_envelope,
};
use crate::external_save::{
    ExternalSaveExportFormat, encode_external_cartridge_save,
    external_save_error_allows_internal_fallback, import_external_cartridge_save,
};
use crate::file_io::write_save_file_with_safe_replace;
use crate::format::CURRENT_SAVE_FORMAT_VERSION;
use crate::key::{CartridgeSaveFileExtension, CartridgeSaveKey};
use crate::time::{CartridgeSaveTimeSource, SystemCartridgeSaveTimeSource};
use gb_core::{CartridgePersistenceMetadata, CartridgePersistenceProfile, PersistentCartState};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemCartridgeSaveStorageFormat {
    External,
    InternalEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemCartridgeSaveLoad {
    pub envelope: CartridgeSaveEnvelope,
    pub path: PathBuf,
    pub format: FilesystemCartridgeSaveStorageFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemCartridgeSaveWrite {
    pub envelope: CartridgeSaveEnvelope,
    pub path: PathBuf,
    pub format: FilesystemCartridgeSaveStorageFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilesystemCartridgeSaveStoragePolicy {
    ExternalPrimary,
    InternalOnly,
    DynamicMbc6,
}

#[derive(Debug)]
pub struct FilesystemCartridgeSaveStore<C = SystemCartridgeSaveTimeSource> {
    root: PathBuf,
    clock: C,
    file_extension: CartridgeSaveFileExtension,
}

impl FilesystemCartridgeSaveStore<SystemCartridgeSaveTimeSource> {
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

impl<C> FilesystemCartridgeSaveStore<C> {
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

    pub fn external_path_for_key(&self, key: &CartridgeSaveKey) -> PathBuf {
        self.root.join(format!(
            "{}.{}",
            key.as_str(),
            self.file_extension.external_as_str()
        ))
    }

    pub fn internal_path_for_key(&self, key: &CartridgeSaveKey) -> PathBuf {
        self.root
            .join(format!("{}.{}", key.as_str(), self.file_extension.as_str()))
    }

    pub fn preferred_path_for_state(
        &self,
        key: &CartridgeSaveKey,
        metadata: CartridgePersistenceMetadata,
        state: &PersistentCartState,
    ) -> PathBuf {
        match filesystem_cartridge_save_storage_policy(metadata, state) {
            FilesystemCartridgeSaveStoragePolicy::ExternalPrimary => {
                self.external_path_for_key(key)
            }
            FilesystemCartridgeSaveStoragePolicy::InternalOnly => self.internal_path_for_key(key),
            FilesystemCartridgeSaveStoragePolicy::DynamicMbc6 => {
                let internal_path = self.internal_path_for_key(key);
                if internal_path.exists() {
                    internal_path
                } else {
                    self.external_path_for_key(key)
                }
            }
        }
    }
}

impl<C: CartridgeSaveTimeSource> FilesystemCartridgeSaveStore<C> {
    pub fn current_unix_seconds(&self) -> u64 {
        self.clock.now_unix_seconds()
    }

    pub fn load(
        &self,
        key: &CartridgeSaveKey,
        metadata: CartridgePersistenceMetadata,
        target_state: &PersistentCartState,
    ) -> Result<Option<FilesystemCartridgeSaveLoad>, CartridgeSaveBackendError> {
        match filesystem_cartridge_save_storage_policy(metadata, target_state) {
            FilesystemCartridgeSaveStoragePolicy::ExternalPrimary => {
                self.load_external(key, metadata, target_state)
            }
            FilesystemCartridgeSaveStoragePolicy::InternalOnly => self.load_internal(key),
            FilesystemCartridgeSaveStoragePolicy::DynamicMbc6 => {
                let internal_path = self.internal_path_for_key(key);
                if internal_path.exists() {
                    self.load_internal(key)
                } else {
                    self.load_external(key, metadata, target_state)
                }
            }
        }
    }

    pub fn save(
        &mut self,
        key: &CartridgeSaveKey,
        metadata: CartridgePersistenceMetadata,
        persistent_state: &PersistentCartState,
    ) -> Result<FilesystemCartridgeSaveWrite, CartridgeSaveBackendError> {
        match filesystem_cartridge_save_storage_policy(metadata, persistent_state) {
            FilesystemCartridgeSaveStoragePolicy::InternalOnly => {
                self.save_internal(key, metadata, persistent_state)
            }
            FilesystemCartridgeSaveStoragePolicy::DynamicMbc6
                if self.internal_path_for_key(key).exists() =>
            {
                self.save_internal(key, metadata, persistent_state)
            }
            FilesystemCartridgeSaveStoragePolicy::ExternalPrimary
            | FilesystemCartridgeSaveStoragePolicy::DynamicMbc6 => {
                self.save_external_or_internal_fallback(key, metadata, persistent_state)
            }
        }
    }

    fn load_external(
        &self,
        key: &CartridgeSaveKey,
        metadata: CartridgePersistenceMetadata,
        target_state: &PersistentCartState,
    ) -> Result<Option<FilesystemCartridgeSaveLoad>, CartridgeSaveBackendError> {
        let path = self.external_path_for_key(key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(CartridgeSaveBackendError::Io {
                    operation: "read external save file",
                    path,
                    source,
                });
            }
        };

        let saved_at_unix_seconds = self.clock.now_unix_seconds();
        let persistent_state =
            import_external_cartridge_save(metadata, target_state, &bytes, saved_at_unix_seconds)
                .map_err(|source| CartridgeSaveBackendError::ExternalSave {
                operation: "import external save",
                path: path.clone(),
                source,
            })?;
        Ok(Some(FilesystemCartridgeSaveLoad {
            envelope: CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds,
                },
                cartridge_metadata: metadata,
                persistent_state,
            },
            path,
            format: FilesystemCartridgeSaveStorageFormat::External,
        }))
    }

    fn load_internal(
        &self,
        key: &CartridgeSaveKey,
    ) -> Result<Option<FilesystemCartridgeSaveLoad>, CartridgeSaveBackendError> {
        let path = self.internal_path_for_key(key);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(FilesystemCartridgeSaveLoad {
                envelope: decode_cartridge_save_envelope(&bytes)?,
                path,
                format: FilesystemCartridgeSaveStorageFormat::InternalEnvelope,
            })),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(CartridgeSaveBackendError::Io {
                operation: "read save file",
                path,
                source,
            }),
        }
    }

    fn save_external_or_internal_fallback(
        &mut self,
        key: &CartridgeSaveKey,
        metadata: CartridgePersistenceMetadata,
        persistent_state: &PersistentCartState,
    ) -> Result<FilesystemCartridgeSaveWrite, CartridgeSaveBackendError> {
        let saved_at_unix_seconds = self.clock.now_unix_seconds();
        match encode_external_cartridge_save(
            metadata,
            persistent_state,
            saved_at_unix_seconds,
            ExternalSaveExportFormat::default(),
        ) {
            Ok(bytes) => {
                let path = self.external_path_for_key(key);
                write_save_file_with_safe_replace(&path, &bytes)?;
                Ok(FilesystemCartridgeSaveWrite {
                    envelope: CartridgeSaveEnvelope {
                        backend_metadata: CartridgeSaveBackendMetadata {
                            format_version: CURRENT_SAVE_FORMAT_VERSION,
                            saved_at_unix_seconds,
                        },
                        cartridge_metadata: metadata,
                        persistent_state: persistent_state.clone(),
                    },
                    path,
                    format: FilesystemCartridgeSaveStorageFormat::External,
                })
            }
            Err(error) if external_save_error_allows_internal_fallback(&error) => {
                self.save_internal_at(key, metadata, persistent_state, saved_at_unix_seconds)
            }
            Err(source) => Err(CartridgeSaveBackendError::ExternalSave {
                operation: "export external save",
                path: self.external_path_for_key(key),
                source,
            }),
        }
    }

    fn save_internal(
        &mut self,
        key: &CartridgeSaveKey,
        metadata: CartridgePersistenceMetadata,
        persistent_state: &PersistentCartState,
    ) -> Result<FilesystemCartridgeSaveWrite, CartridgeSaveBackendError> {
        self.save_internal_at(
            key,
            metadata,
            persistent_state,
            self.clock.now_unix_seconds(),
        )
    }

    fn save_internal_at(
        &mut self,
        key: &CartridgeSaveKey,
        metadata: CartridgePersistenceMetadata,
        persistent_state: &PersistentCartState,
        saved_at_unix_seconds: u64,
    ) -> Result<FilesystemCartridgeSaveWrite, CartridgeSaveBackendError> {
        let envelope = CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds,
            },
            cartridge_metadata: metadata,
            persistent_state: persistent_state.clone(),
        };
        let bytes = encode_cartridge_save_envelope(&envelope)?;
        let path = self.internal_path_for_key(key);
        write_save_file_with_safe_replace(&path, &bytes)?;
        Ok(FilesystemCartridgeSaveWrite {
            envelope,
            path,
            format: FilesystemCartridgeSaveStorageFormat::InternalEnvelope,
        })
    }
}

fn filesystem_cartridge_save_storage_policy(
    metadata: CartridgePersistenceMetadata,
    state: &PersistentCartState,
) -> FilesystemCartridgeSaveStoragePolicy {
    match (metadata.profile, state) {
        (_, PersistentCartState::Huc3 { .. }) => FilesystemCartridgeSaveStoragePolicy::InternalOnly,
        (
            CartridgePersistenceProfile::PersistentRamAndFlash { .. },
            PersistentCartState::Mbc6 { .. },
        ) => FilesystemCartridgeSaveStoragePolicy::DynamicMbc6,
        _ => FilesystemCartridgeSaveStoragePolicy::ExternalPrimary,
    }
}
