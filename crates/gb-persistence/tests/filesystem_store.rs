mod common;

use common::*;
use gb_core::{
    CartridgePersistenceMetadata, CartridgePersistenceProfile, CartridgeRamPayloadKind,
    Mbc3RtcPersistentState, PersistentCartState,
};
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveBackendError, CartridgeSaveFileExtension, CartridgeSaveKey,
    EXTERNAL_SAVE_FILE_EXTENSION, EXTERNAL_SAVE_FILE_EXTENSION_P2, EXTERNAL_SAVE_FILE_EXTENSION_P3,
    EXTERNAL_SAVE_FILE_EXTENSION_P4, FilesystemCartridgeSaveBackend,
    FilesystemCartridgeSaveStorageFormat, FilesystemCartridgeSaveStore,
    FixedCartridgeSaveTimeSource,
};
use std::fs;

#[path = "filesystem_store/errors.rs"]
mod errors;
#[path = "filesystem_store/extensions.rs"]
mod extensions;
#[path = "filesystem_store/fallback.rs"]
mod fallback;
#[path = "filesystem_store/legacy.rs"]
mod legacy;
#[path = "filesystem_store/rtc.rs"]
mod rtc;
