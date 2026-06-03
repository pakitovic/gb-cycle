mod persistence;

pub(crate) use persistence::apply_elapsed_off_session_seconds;
pub use persistence::{
    HardwarePersistenceActionResult, HardwarePersistenceError, HardwarePersistenceFlushPolicy,
    HardwarePersistenceLoadResult, HardwarePersistenceManager, HardwarePersistenceSaveResult,
    HardwarePersistenceTrigger, load_hardware_cartridge_persistence,
    save_hardware_cartridge_persistence, uses_battery_backed_hardware_persistence,
};

#[cfg(test)]
mod test;
