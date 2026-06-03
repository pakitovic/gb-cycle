mod common;

use common::*;
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveKey, FilesystemCartridgeSaveBackend,
    FixedCartridgeSaveTimeSource, HardwarePersistenceActionResult, HardwarePersistenceError,
    HardwarePersistenceFlushPolicy, HardwarePersistenceLoadResult, HardwarePersistenceManager,
    HardwarePersistenceTrigger, InMemoryCartridgeSaveBackend,
};
use std::fs;

#[path = "manager/accessors.rs"]
mod accessors;
#[path = "manager/errors_retry.rs"]
mod errors_retry;
#[path = "manager/flush_policy.rs"]
mod flush_policy;
#[path = "manager/skip_paths.rs"]
mod skip_paths;
