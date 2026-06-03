mod backend;
mod cartridge_envelope;
mod external_save;
mod file_io;
mod filesystem_store;
mod format;
mod hardware;
mod key;
mod machine_state;
mod time;
mod wire;

pub use backend::{
    CartridgeSaveBackend, CartridgeSaveBackendError, FilesystemCartridgeSaveBackend,
    InMemoryCartridgeSaveBackend,
};
pub use cartridge_envelope::{
    CartridgeSaveBackendMetadata, CartridgeSaveEnvelope, decode_cartridge_save_envelope,
    encode_cartridge_save_envelope,
};
pub use external_save::{
    ExternalSaveError, ExternalSaveExportFormat, ExternalSaveLengthExpectation,
    encode_external_cartridge_save, export_external_cartridge_save, import_external_cartridge_save,
};
pub use filesystem_store::{
    FilesystemCartridgeSaveLoad, FilesystemCartridgeSaveStorageFormat,
    FilesystemCartridgeSaveStore, FilesystemCartridgeSaveWrite,
};
pub use format::{
    CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION, CURRENT_SAVE_FORMAT_VERSION,
    EXTERNAL_SAVE_FILE_EXTENSION, EXTERNAL_SAVE_FILE_EXTENSION_P2, EXTERNAL_SAVE_FILE_EXTENSION_P3,
    EXTERNAL_SAVE_FILE_EXTENSION_P4, MACHINE_SAVE_STATE_FILE_EXTENSION, SAVE_FILE_EXTENSION,
    SAVE_FILE_EXTENSION_P2, SAVE_FILE_EXTENSION_P3, SAVE_FILE_EXTENSION_P4,
};
pub use hardware::{
    HardwarePersistenceActionResult, HardwarePersistenceError, HardwarePersistenceFlushPolicy,
    HardwarePersistenceLoadResult, HardwarePersistenceManager, HardwarePersistenceSaveResult,
    HardwarePersistenceTrigger, load_hardware_cartridge_persistence,
    save_hardware_cartridge_persistence, uses_battery_backed_hardware_persistence,
};
pub use key::{CartridgeSaveFileExtension, CartridgeSaveKey, CartridgeSaveKeyError};
pub use machine_state::{
    MachineSaveStateBackendMetadata, MachineSaveStateEnvelope, decode_machine_save_state_envelope,
    encode_machine_save_state_envelope,
};
pub use time::{
    CartridgeSaveTimeSource, FixedCartridgeSaveTimeSource, SystemCartridgeSaveTimeSource,
};
