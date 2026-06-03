mod common;

use common::*;
use gb_core::{
    CartridgePersistenceProfile, CartridgeRamPayloadKind, ConsoleModel, Huc3RtcPersistentState,
    Machine, MachineConfig, Mbc3RtcPersistentState, PersistentCartState,
};
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveBackendError, CartridgeSaveFileExtension, CartridgeSaveKey,
    FilesystemCartridgeSaveBackend, FixedCartridgeSaveTimeSource, InMemoryCartridgeSaveBackend,
    SAVE_FILE_EXTENSION, SAVE_FILE_EXTENSION_P2, SAVE_FILE_EXTENSION_P3, SAVE_FILE_EXTENSION_P4,
};
use std::fs;
use std::path::{Path, PathBuf};

#[path = "backend/filesystem.rs"]
mod filesystem;
#[path = "backend/in_memory.rs"]
mod in_memory;
