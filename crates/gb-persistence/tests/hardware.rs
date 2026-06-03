mod common;

use common::*;
use gb_core::PersistentCartState;
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveKey, FilesystemCartridgeSaveBackend,
    FixedCartridgeSaveTimeSource, HardwarePersistenceLoadResult, HardwarePersistenceSaveResult,
    InMemoryCartridgeSaveBackend, load_hardware_cartridge_persistence,
    save_hardware_cartridge_persistence, uses_battery_backed_hardware_persistence,
};
use std::fs;

#[path = "hardware/battery_gate.rs"]
mod battery_gate;
#[path = "hardware/mbc2.rs"]
mod mbc2;
#[path = "hardware/mbc3.rs"]
mod mbc3;
#[path = "hardware/mbc7.rs"]
mod mbc7;
