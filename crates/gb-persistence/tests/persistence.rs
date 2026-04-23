use gb_core::{
    CartridgePersistenceProfile, CartridgeRamPayloadKind, CartridgeSlot, CompatibilityPolicy,
    ConsoleModel, Machine, MachineConfig, Mbc3RtcPersistentState, PersistentCartState,
};
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveBackendError, CartridgeSaveKey,
    FilesystemCartridgeSaveBackend, FixedCartridgeSaveTimeSource, HardwarePersistenceActionResult,
    HardwarePersistenceError, HardwarePersistenceFlushPolicy, HardwarePersistenceLoadResult,
    HardwarePersistenceManager, HardwarePersistenceSaveResult, HardwarePersistenceTrigger,
    InMemoryCartridgeSaveBackend, SAVE_FILE_EXTENSION, load_hardware_cartridge_persistence,
    save_hardware_cartridge_persistence, uses_battery_backed_hardware_persistence,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{env, fs};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const ENTRY_POINT_START: usize = 0x0100;
const LOGO_START: usize = 0x0104;
const TITLE_START: usize = 0x0134;
const CGB_FLAG_ADDRESS: usize = 0x0143;
const SGB_FLAG_ADDRESS: usize = 0x0146;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
const ROM_SIZE_ADDRESS: usize = 0x0148;
const RAM_SIZE_ADDRESS: usize = 0x0149;
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn build_test_rom(len: usize, cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"PERSIST!");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[ROM_SIZE_ADDRESS] = rom_size_code;
    rom[RAM_SIZE_ADDRESS] = ram_size_code;
    rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
    rom
}

fn build_banked_mbc1_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        _ => panic!("unsupported MBC1 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_mmm01_rom(rom_size_code: u8, ram_size_code: u8, cartridge_type: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        0x07 => 4 * 1024 * 1024,
        0x08 => 8 * 1024 * 1024,
        _ => panic!("unsupported MMM01 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = vec![0xFF; rom_size.max(HEADER_MINIMUM_ROM_LEN)];

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"GAMEONE");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[ROM_SIZE_ADDRESS] = 0x00;
    rom[RAM_SIZE_ADDRESS] = 0x00;
    rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;

    let menu_offset = rom_size - 32 * 1024;
    rom[menu_offset] = ((bank_count - 2) & 0xFF) as u8;
    rom[menu_offset + ENTRY_POINT_START..menu_offset + ENTRY_POINT_START + 4]
        .copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[menu_offset + LOGO_START..menu_offset + LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[menu_offset + TITLE_START..menu_offset + TITLE_START + 7].copy_from_slice(b"MMM01!!");
    rom[menu_offset + CGB_FLAG_ADDRESS] = 0x80;
    rom[menu_offset + SGB_FLAG_ADDRESS] = 0x03;
    rom[menu_offset + CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[menu_offset + ROM_SIZE_ADDRESS] = rom_size_code;
    rom[menu_offset + RAM_SIZE_ADDRESS] = ram_size_code;
    rom[menu_offset + HEADER_CHECKSUM_ADDRESS] = 0x7F;

    rom
}

fn build_banked_huc1_rom(rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        _ => panic!("unsupported HuC1 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, 0xFF, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc2_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        _ => panic!("unsupported MBC2 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc3_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        _ => panic!("unsupported MBC3 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn temp_save_root() -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "gb-cycle-persistence-tests-{}-{id}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale temp save root should be removable");
    }
    fs::create_dir_all(&root).expect("temp save root should be creatable");
    root
}

fn load_cartridge(rom: Vec<u8>) -> CartridgeSlot {
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect("test cartridge should load");
    let (cartridge, _) = report.into_parts();
    cartridge
}

#[test]
fn in_memory_backend_round_trips_full_mbc1_ram_backing_store() {
    let mut cartridge = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x00);
    cartridge.write_ram(0xA000, 0x11);
    cartridge.write_rom(0x4000, 0x01);
    cartridge.write_ram(0xA000, 0x22);
    cartridge.write_rom(0x0000, 0x00);

    let key = CartridgeSaveKey::new("mbc1_roundtrip").expect("key should be valid");
    let mut backend = InMemoryCartridgeSaveBackend::with_time_source(
        FixedCartridgeSaveTimeSource::new(1_700_000_000),
    );
    let saved = backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    assert_eq!(saved.backend_metadata.saved_at_unix_seconds, 1_700_000_000);
    assert_eq!(
        saved.cartridge_metadata.profile,
        CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024,
            },
        }
    );

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    let mut restored = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    restored
        .restore_persistent_state(&loaded.persistent_state)
        .expect("restore should accept the persisted payload");

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x6000, 0x01);
    restored.write_rom(0x4000, 0x00);
    assert_eq!(restored.read_ram(0xA000), 0x11);
    restored.write_rom(0x4000, 0x01);
    assert_eq!(restored.read_ram(0xA000), 0x22);
}

#[test]
fn filesystem_backend_round_trips_mbc2_nibbles_and_cleans_temp_artifacts() {
    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x0B);
    cartridge.write_ram(0xA200, 0x05);
    cartridge.write_rom(0x0000, 0x00);

    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc2_nibbles").expect("key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(7),
    );
    backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    let save_path = backend.path_for_key(&key);
    let temp_path = PathBuf::from(format!("{}.tmp", save_path.display()));
    let backup_path = PathBuf::from(format!("{}.bak", save_path.display()));
    assert!(save_path.is_file());
    assert!(!temp_path.exists());
    assert!(!backup_path.exists());
    assert_eq!(
        save_path.extension().and_then(|ext| ext.to_str()),
        Some(SAVE_FILE_EXTENSION)
    );

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    let mut restored = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    restored
        .restore_persistent_state(&loaded.persistent_state)
        .expect("restore should accept the persisted payload");
    restored.write_rom(0x0000, 0x0A);
    assert_eq!(restored.read_ram(0xA000) & 0x0F, 0x05);
    assert_eq!(restored.read_ram(0xA200) & 0x0F, 0x05);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn filesystem_backend_round_trips_mbc3_rtc_payload_and_exposes_saved_timestamp() {
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::Dmg));
    machine
        .load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03))
        .expect("MBC3 cartridge should load");
    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x00);
    machine.write_bus(0xA000, 0x44);
    machine.advance_cartridge_rtc_seconds(93_784);
    machine.write_bus(0x0000, 0x00);

    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc3_rtc").expect("key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(1_800_000_000),
    );
    backend
        .save(
            &key,
            machine.cartridge().persistence_metadata(),
            &machine.cartridge().persistent_state(),
        )
        .expect("save should succeed");

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    assert_eq!(loaded.backend_metadata.saved_at_unix_seconds, 1_800_000_000);
    match &loaded.persistent_state {
        PersistentCartState::Mbc3RamRtc { rtc, .. } => {
            assert_eq!(
                *rtc,
                Mbc3RtcPersistentState {
                    seconds: 4,
                    minutes: 3,
                    hours: 2,
                    day_counter: 1,
                    halt: false,
                    carry: false,
                }
            );
        }
        other => panic!("expected MBC3 RAM+RTC payload, got {other:?}"),
    }

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn in_memory_backend_round_trips_mmm01_battery_ram_backing_store() {
    let mut cartridge = load_cartridge(build_mmm01_rom(0x03, 0x03, 0x0D));
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_rom(0x0000, 0x2A);
    cartridge.write_rom(0x0000, 0x6A);
    cartridge.write_ram(0xA000, 0x11);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_ram(0xA000, 0x22);
    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);
    cartridge.write_rom(0x0000, 0x00);

    let key = CartridgeSaveKey::new("mmm01_roundtrip").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(808));
    let saved = backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    assert_eq!(
        saved.cartridge_metadata.profile,
        CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024,
            },
        }
    );
    match saved.persistent_state {
        PersistentCartState::Mmm01Ram { ref ram } => {
            assert_eq!(ram[2 * 0x2000], 0x22);
            assert_eq!(ram[3 * 0x2000], 0x33);
        }
        ref other => panic!("expected MMM01 RAM payload, got {other:?}"),
    }

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    let mut restored = load_cartridge(build_mmm01_rom(0x03, 0x03, 0x0D));
    restored
        .restore_persistent_state(&loaded.persistent_state)
        .expect("restore should accept the persisted payload");

    restored.write_rom(0x4000, 0x02);
    restored.write_rom(0x0000, 0x2A);
    restored.write_rom(0x0000, 0x6A);
    assert_eq!(restored.read_ram(0xA000), 0x22);
    restored.write_rom(0x6000, 0x01);
    assert_eq!(restored.read_ram(0xA000), 0x22);
    restored.write_rom(0x4000, 0x03);
    assert_eq!(restored.read_ram(0xA000), 0x33);
}

#[test]
fn in_memory_backend_round_trips_huc1_battery_ram_backing_store() {
    let mut cartridge = load_cartridge(build_banked_huc1_rom(0x03, 0x03));
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_ram(0xA000, 0x22);
    cartridge.write_rom(0x0000, 0x0E);
    cartridge.write_ram(0xA000, 0x01);
    cartridge.write_rom(0x0000, 0x00);
    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);

    let key = CartridgeSaveKey::new("huc1_roundtrip").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(909));
    let saved = backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    assert_eq!(
        saved.cartridge_metadata.profile,
        CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024,
            },
        }
    );
    match saved.persistent_state {
        PersistentCartState::Huc1Ram { ref ram } => {
            assert_eq!(ram[2 * 0x2000], 0x22);
            assert_eq!(ram[3 * 0x2000], 0x33);
        }
        ref other => panic!("expected HuC1 RAM payload, got {other:?}"),
    }

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    let mut restored = load_cartridge(build_banked_huc1_rom(0x03, 0x03));
    restored
        .restore_persistent_state(&loaded.persistent_state)
        .expect("restore should accept the persisted payload");

    restored.write_rom(0x4000, 0x02);
    assert_eq!(restored.read_ram(0xA000), 0x22);
    restored.write_rom(0x0000, 0x0E);
    assert_eq!(restored.read_ram(0xA000), 0xC0);
    restored.write_rom(0x0000, 0x00);
    restored.write_rom(0x4000, 0x03);
    assert_eq!(restored.read_ram(0xA000), 0x33);
}

#[test]
fn backend_can_store_non_persistent_metadata_without_forcing_auto_save_policy() {
    let cartridge = load_cartridge(build_banked_mbc1_rom(0x02, 0x03, 0x03));
    let key = CartridgeSaveKey::new("non_persistent").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(99));

    let saved = backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("save should succeed");

    assert_eq!(
        saved.cartridge_metadata.profile,
        CartridgePersistenceProfile::NonPersistentRam {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024,
            },
        }
    );
    assert_eq!(saved.persistent_state, PersistentCartState::None);
}

#[test]
fn manual_flush_policy_requires_explicit_flush_and_supports_force_save() {
    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x09);
    cartridge.write_rom(0x0000, 0x00);

    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(300));
    let key = CartridgeSaveKey::new("manual_policy").expect("key should be valid");
    let mut manager =
        HardwarePersistenceManager::new(backend, key, HardwarePersistenceFlushPolicy::Manual);

    let write_result = manager
        .note_persistible_write(&cartridge)
        .expect("manual policy write notification should succeed");
    assert_eq!(write_result, HardwarePersistenceActionResult::Deferred);
    assert!(manager.is_dirty());
    assert_eq!(manager.backend().len(), 0);

    let close_result = manager
        .close(&cartridge)
        .expect("manual policy close should not fail");
    assert_eq!(
        close_result,
        HardwarePersistenceActionResult::SkippedByFlushPolicy {
            trigger: HardwarePersistenceTrigger::Close,
        }
    );
    assert!(manager.is_dirty());
    assert_eq!(manager.backend().len(), 0);

    let flush_result = manager.flush(&cartridge).expect("flush should succeed");
    assert!(matches!(
        flush_result,
        HardwarePersistenceActionResult::Saved {
            trigger: HardwarePersistenceTrigger::ManualFlush,
            ..
        }
    ));
    assert!(!manager.is_dirty());
    assert_eq!(manager.backend().len(), 1);

    let force_result = manager
        .force_save(&cartridge)
        .expect("force save should succeed even when clean");
    assert!(matches!(
        force_result,
        HardwarePersistenceActionResult::Saved {
            trigger: HardwarePersistenceTrigger::ForcedSave,
            ..
        }
    ));
    assert!(!manager.is_dirty());
    assert_eq!(manager.backend().len(), 1);
}

#[test]
fn save_on_close_policy_flushes_when_session_closes() {
    let mut cartridge = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x01);
    cartridge.write_ram(0xA000, 0x5A);

    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(400));
    let key = CartridgeSaveKey::new("save_on_close").expect("key should be valid");
    let mut manager =
        HardwarePersistenceManager::new(backend, key, HardwarePersistenceFlushPolicy::SaveOnClose);

    assert_eq!(
        manager
            .note_persistible_write(&cartridge)
            .expect("write notification should succeed"),
        HardwarePersistenceActionResult::Deferred
    );
    assert!(manager.is_dirty());
    assert_eq!(manager.backend().len(), 0);

    let close_result = manager.close(&cartridge).expect("close should succeed");
    assert!(matches!(
        close_result,
        HardwarePersistenceActionResult::Saved {
            trigger: HardwarePersistenceTrigger::Close,
            ..
        }
    ));
    assert!(!manager.is_dirty());
    assert_eq!(manager.backend().len(), 1);
}

#[test]
fn auto_flush_policy_saves_immediately_after_persistible_writes() {
    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x07);

    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(500));
    let key = CartridgeSaveKey::new("auto_flush").expect("key should be valid");
    let mut manager = HardwarePersistenceManager::new(
        backend,
        key,
        HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite,
    );

    let write_result = manager
        .note_persistible_write(&cartridge)
        .expect("auto-flush write notification should succeed");
    assert!(matches!(
        write_result,
        HardwarePersistenceActionResult::Saved {
            trigger: HardwarePersistenceTrigger::PersistibleWrite,
            ..
        }
    ));
    assert!(!manager.is_dirty());
    assert_eq!(manager.backend().len(), 1);

    assert_eq!(
        manager.close(&cartridge).expect("close should succeed"),
        HardwarePersistenceActionResult::NoPendingSave
    );
}

#[test]
fn filesystem_backend_replaces_existing_save_without_leaving_temp_or_backup_files() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("replace_existing").expect("key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(600),
    );

    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x01);
    backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("first save should succeed");

    cartridge.write_ram(0xA000, 0x0E);
    backend
        .save(
            &key,
            cartridge.persistence_metadata(),
            &cartridge.persistent_state(),
        )
        .expect("replacement save should succeed");

    let save_path = backend.path_for_key(&key);
    let temp_path = PathBuf::from(format!("{}.tmp", save_path.display()));
    let backup_path = PathBuf::from(format!("{}.bak", save_path.display()));
    assert!(save_path.is_file());
    assert!(!temp_path.exists());
    assert!(!backup_path.exists());

    let loaded = backend
        .load(&key)
        .expect("load should succeed")
        .expect("save should exist");
    match loaded.persistent_state {
        PersistentCartState::Mbc2Ram { ram_nibbles } => {
            assert_eq!(ram_nibbles[0], 0x0E);
        }
        other => panic!("expected MBC2 save after replacement, got {other:?}"),
    }

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn auto_flush_errors_surface_and_leave_the_manager_dirty_for_retry() {
    let root = temp_save_root();
    let occupied_path = root.join("occupied");
    fs::write(&occupied_path, b"not a directory").expect("occupied file should be creatable");

    let mut cartridge = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x03);

    let backend = FilesystemCartridgeSaveBackend::with_time_source(
        &occupied_path,
        FixedCartridgeSaveTimeSource::new(700),
    );
    let key = CartridgeSaveKey::new("flush_error").expect("key should be valid");
    let mut manager = HardwarePersistenceManager::new(
        backend,
        key,
        HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite,
    );

    let error = manager
        .note_persistible_write(&cartridge)
        .expect_err("auto-flush should surface filesystem errors");
    assert!(format!("{error}").contains("create save directory"));
    assert!(manager.is_dirty());

    let close_error = manager
        .close(&cartridge)
        .expect_err("close should retry and surface the same error");
    assert!(format!("{close_error}").contains("create save directory"));
    assert!(manager.is_dirty());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn battery_gated_helper_skips_non_battery_ram_cartridges_by_default() {
    let mut cartridge = load_cartridge(build_banked_mbc1_rom(0x02, 0x03, 0x03));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x01);
    cartridge.write_ram(0xA000, 0x66);

    assert!(!uses_battery_backed_hardware_persistence(
        cartridge.persistence_metadata()
    ));

    let key = CartridgeSaveKey::new("skip_non_battery").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(123));
    let save_result = save_hardware_cartridge_persistence(&mut backend, &key, &cartridge)
        .expect("save helper should not fail");

    assert_eq!(
        save_result,
        HardwarePersistenceSaveResult::SkippedNotBatteryBacked
    );
    assert_eq!(backend.len(), 0);

    let mut battery_backed_source = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    battery_backed_source.write_rom(0x0000, 0x0A);
    battery_backed_source.write_rom(0x6000, 0x01);
    battery_backed_source.write_rom(0x4000, 0x00);
    battery_backed_source.write_ram(0xA000, 0x11);
    backend
        .save(
            &key,
            battery_backed_source.persistence_metadata(),
            &battery_backed_source.persistent_state(),
        )
        .expect("raw backend save should succeed");

    let load_result = load_hardware_cartridge_persistence(&backend, &key, &mut cartridge)
        .expect("load helper should not fail");
    assert_eq!(
        load_result,
        HardwarePersistenceLoadResult::SkippedNotBatteryBacked
    );

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x66);
}

#[test]
fn manager_operations_keep_non_battery_cartridges_on_the_explicit_skipped_path() {
    let mut cartridge = load_cartridge(build_banked_mbc1_rom(0x02, 0x03, 0x03));
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x44);

    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(321));
    let key = CartridgeSaveKey::new("manager_skip_non_battery").expect("key should be valid");
    let mut manager =
        HardwarePersistenceManager::new(backend, key, HardwarePersistenceFlushPolicy::Manual);

    assert_eq!(
        manager
            .note_persistible_write(&cartridge)
            .expect("note should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert!(!manager.is_dirty());

    assert_eq!(
        manager.flush(&cartridge).expect("flush should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert_eq!(
        manager
            .force_save(&cartridge)
            .expect("force save should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert_eq!(
        manager.close(&cartridge).expect("close should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert_eq!(manager.backend().len(), 0);
}

#[test]
fn battery_gated_helper_round_trips_mbc2_nibble_ram() {
    let mut source = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    source.write_rom(0x0000, 0x0A);
    source.write_ram(0xA000, 0x0C);
    source.write_ram(0xA001, 0x03);
    source.write_rom(0x0000, 0x00);

    let key = CartridgeSaveKey::new("mbc2_hardware").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(5));
    let save_result = save_hardware_cartridge_persistence(&mut backend, &key, &source)
        .expect("battery-backed MBC2 save should succeed");
    assert!(matches!(
        save_result,
        HardwarePersistenceSaveResult::Saved(_)
    ));

    let mut restored = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let load_result = load_hardware_cartridge_persistence(&backend, &key, &mut restored)
        .expect("battery-backed MBC2 load should succeed");
    assert!(matches!(
        load_result,
        HardwarePersistenceLoadResult::Restored { .. }
    ));

    restored.write_rom(0x0000, 0x0A);
    assert_eq!(restored.read_ram(0xA000) & 0x0F, 0x0C);
    assert_eq!(restored.read_ram(0xA001) & 0x0F, 0x03);
}

#[test]
fn battery_gated_helper_round_trips_mbc3_ram_and_rtc() {
    let mut source = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    source.write_rom(0x0000, 0x0A);
    source.write_rom(0x4000, 0x00);
    source.write_ram(0xA000, 0x44);
    source.write_rom(0x4000, 0x01);
    source.write_ram(0xA000, 0x99);

    source.write_rom(0x4000, 0x08);
    source.write_ram(0xA000, 42);
    source.write_rom(0x4000, 0x09);
    source.write_ram(0xA000, 17);
    source.write_rom(0x4000, 0x0A);
    source.write_ram(0xA000, 9);
    source.write_rom(0x4000, 0x0B);
    source.write_ram(0xA000, 1);
    source.write_rom(0x4000, 0x0C);
    source.write_ram(0xA000, 0x00);
    source.write_rom(0x0000, 0x00);

    let key = CartridgeSaveKey::new("mbc3_hardware").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(77));
    let save_result = save_hardware_cartridge_persistence(&mut backend, &key, &source)
        .expect("battery-backed MBC3 save should succeed");
    assert!(matches!(
        save_result,
        HardwarePersistenceSaveResult::Saved(_)
    ));

    let mut restored = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    let load_result = load_hardware_cartridge_persistence(&backend, &key, &mut restored)
        .expect("battery-backed MBC3 load should succeed");
    assert!(matches!(
        load_result,
        HardwarePersistenceLoadResult::Restored { .. }
    ));

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x4000, 0x00);
    assert_eq!(restored.read_ram(0xA000), 0x44);
    restored.write_rom(0x4000, 0x01);
    assert_eq!(restored.read_ram(0xA000), 0x99);

    restored.write_rom(0x6000, 0x00);
    restored.write_rom(0x6000, 0x01);

    restored.write_rom(0x4000, 0x08);
    assert_eq!(restored.read_ram(0xA000), 42);
    restored.write_rom(0x4000, 0x09);
    assert_eq!(restored.read_ram(0xA000), 17);
    restored.write_rom(0x4000, 0x0A);
    assert_eq!(restored.read_ram(0xA000), 9);
    restored.write_rom(0x4000, 0x0B);
    assert_eq!(restored.read_ram(0xA000), 1);
    restored.write_rom(0x4000, 0x0C);
    assert_eq!(restored.read_ram(0xA000), 0x00);
}

#[test]
fn battery_gated_helper_applies_elapsed_seconds_to_mbc3_on_reload() {
    let mut source = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    source.write_rom(0x0000, 0x0A);
    source.write_rom(0x4000, 0x08);
    source.write_ram(0xA000, 59);
    source.write_rom(0x4000, 0x09);
    source.write_ram(0xA000, 59);
    source.write_rom(0x4000, 0x0A);
    source.write_ram(0xA000, 23);
    source.write_rom(0x4000, 0x0B);
    source.write_ram(0xA000, 0xFF);
    source.write_rom(0x4000, 0x0C);
    source.write_ram(0xA000, 0x01);
    source.write_rom(0x0000, 0x00);

    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc3_elapsed").expect("key should be valid");
    let mut save_backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(100),
    );
    save_hardware_cartridge_persistence(&mut save_backend, &key, &source)
        .expect("save should succeed");

    let load_backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(102),
    );
    let mut restored = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    let load_result = load_hardware_cartridge_persistence(&load_backend, &key, &mut restored)
        .expect("load should succeed");
    assert!(matches!(
        load_result,
        HardwarePersistenceLoadResult::Restored {
            elapsed_off_session_seconds: 2,
            ..
        }
    ));

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x6000, 0x00);
    restored.write_rom(0x6000, 0x01);
    restored.write_rom(0x4000, 0x08);
    assert_eq!(restored.read_ram(0xA000), 1);
    restored.write_rom(0x4000, 0x09);
    assert_eq!(restored.read_ram(0xA000), 0);
    restored.write_rom(0x4000, 0x0A);
    assert_eq!(restored.read_ram(0xA000), 0);
    restored.write_rom(0x4000, 0x0B);
    assert_eq!(restored.read_ram(0xA000), 0);
    restored.write_rom(0x4000, 0x0C);
    assert_eq!(restored.read_ram(0xA000), 0x80);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn battery_gated_helper_does_not_advance_halted_mbc3_on_reload() {
    let mut source = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    source.write_rom(0x0000, 0x0A);
    source.write_rom(0x4000, 0x08);
    source.write_ram(0xA000, 12);
    source.write_rom(0x4000, 0x09);
    source.write_ram(0xA000, 34);
    source.write_rom(0x4000, 0x0A);
    source.write_ram(0xA000, 5);
    source.write_rom(0x4000, 0x0B);
    source.write_ram(0xA000, 9);
    source.write_rom(0x4000, 0x0C);
    source.write_ram(0xA000, 0x40);
    source.write_rom(0x0000, 0x00);

    let root = temp_save_root();
    let key = CartridgeSaveKey::new("mbc3_halted_elapsed").expect("key should be valid");
    let mut save_backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(200),
    );
    save_hardware_cartridge_persistence(&mut save_backend, &key, &source)
        .expect("save should succeed");

    let load_backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(400),
    );
    let mut restored = load_cartridge(build_banked_mbc3_rom(0x10, 0x02, 0x03));
    let load_result = load_hardware_cartridge_persistence(&load_backend, &key, &mut restored)
        .expect("load should succeed");
    assert!(matches!(
        load_result,
        HardwarePersistenceLoadResult::Restored {
            elapsed_off_session_seconds: 200,
            ..
        }
    ));

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x6000, 0x00);
    restored.write_rom(0x6000, 0x01);
    restored.write_rom(0x4000, 0x08);
    assert_eq!(restored.read_ram(0xA000), 12);
    restored.write_rom(0x4000, 0x09);
    assert_eq!(restored.read_ram(0xA000), 34);
    restored.write_rom(0x4000, 0x0A);
    assert_eq!(restored.read_ram(0xA000), 5);
    restored.write_rom(0x4000, 0x0B);
    assert_eq!(restored.read_ram(0xA000), 9);
    restored.write_rom(0x4000, 0x0C);
    assert_eq!(restored.read_ram(0xA000), 0x40);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn filesystem_backend_delete_is_idempotent_for_missing_and_existing_saves() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("delete_me").expect("key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::new(&root);

    backend
        .delete(&key)
        .expect("delete should ignore missing files");

    let save_path = backend.path_for_key(&key);
    fs::write(&save_path, b"placeholder").expect("placeholder save should be creatable");
    backend
        .delete(&key)
        .expect("delete should remove the existing file");
    assert!(!Path::new(&save_path).exists());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn in_memory_backend_reports_queries_and_missing_saves_explicitly() {
    let key = CartridgeSaveKey::new("in_memory_queries").expect("key should be valid");
    let mut backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(321));

    assert!(backend.is_empty());
    assert_eq!(backend.len(), 0);
    assert_eq!(backend.current_unix_seconds(), 321);
    assert_eq!(backend.load(&key).expect("load should succeed"), None);

    backend.delete(&key).expect("delete should succeed");
    assert!(backend.is_empty());
}

#[test]
fn filesystem_backend_exposes_root_path_and_missing_load_cleanly() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("missing_filesystem_save").expect("key should be valid");
    let backend = FilesystemCartridgeSaveBackend::with_time_source(
        &root,
        FixedCartridgeSaveTimeSource::new(654),
    );

    assert_eq!(backend.root(), Path::new(&root));
    assert_eq!(backend.current_unix_seconds(), 654);
    assert_eq!(
        backend.path_for_key(&key),
        root.join(format!("{key}.{}", SAVE_FILE_EXTENSION, key = key.as_str()))
    );
    assert_eq!(backend.load(&key).expect("load should succeed"), None);

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn manager_accessors_and_load_into_cover_no_save_and_skip_paths() {
    let key = CartridgeSaveKey::new("manager_accessors").expect("key should be valid");
    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(900));
    let mut manager = HardwarePersistenceManager::new(
        backend,
        key.clone(),
        HardwarePersistenceFlushPolicy::Manual,
    );

    assert_eq!(manager.key(), &key);
    assert_eq!(
        manager.flush_policy(),
        HardwarePersistenceFlushPolicy::Manual
    );
    assert_eq!(manager.backend().current_unix_seconds(), 900);
    manager.set_flush_policy(HardwarePersistenceFlushPolicy::SaveOnClose);
    assert_eq!(
        manager.flush_policy(),
        HardwarePersistenceFlushPolicy::SaveOnClose
    );

    let battery_backed = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    assert_eq!(
        manager
            .flush(&battery_backed)
            .expect("clean flush should succeed"),
        HardwarePersistenceActionResult::NoPendingSave
    );

    let non_battery = load_cartridge(build_banked_mbc1_rom(0x02, 0x03, 0x03));
    assert_eq!(
        manager
            .note_persistible_write(&non_battery)
            .expect("non-battery note should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert_eq!(
        manager
            .close(&non_battery)
            .expect("non-battery close should succeed"),
        HardwarePersistenceActionResult::SkippedNotBatteryBacked
    );
    assert!(!manager.is_dirty());

    let mut restore_target = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    assert_eq!(
        manager
            .load_into(&mut restore_target)
            .expect("load_into should succeed"),
        HardwarePersistenceLoadResult::NoSavePresent
    );

    let backend = manager.into_backend();
    assert!(backend.is_empty());
}

#[test]
fn filesystem_backend_surfaces_targeted_io_failures() {
    let root = temp_save_root();
    let key = CartridgeSaveKey::new("io_failures").expect("key should be valid");
    let occupied_root = root.join("occupied");
    fs::write(&occupied_root, b"not a directory").expect("occupied file should be creatable");

    let mut occupied_backend = FilesystemCartridgeSaveBackend::new(occupied_root.as_path());
    let _ = occupied_backend.current_unix_seconds();

    let load_error = occupied_backend
        .load(&key)
        .expect_err("load should surface path errors");
    assert!(matches!(
        load_error,
        CartridgeSaveBackendError::Io {
            operation: "read save file",
            ..
        }
    ));

    let delete_error = occupied_backend
        .delete(&key)
        .expect_err("delete should surface path errors");
    assert!(matches!(
        delete_error,
        CartridgeSaveBackendError::Io {
            operation: "delete save file",
            ..
        }
    ));

    let source = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let stale_key = CartridgeSaveKey::new("stale_backup_cleanup").expect("key should be valid");
    let mut stale_backend = FilesystemCartridgeSaveBackend::with_time_source(
        root.as_path(),
        FixedCartridgeSaveTimeSource::new(1_001),
    );
    let stale_save_path = stale_backend.path_for_key(&stale_key);
    let stale_backup_path = PathBuf::from(format!("{}.bak", stale_save_path.display()));
    fs::create_dir_all(&stale_backup_path).expect("backup directory should be creatable");

    let stale_backup_error = stale_backend
        .save(
            &stale_key,
            source.persistence_metadata(),
            &source.persistent_state(),
        )
        .expect_err("save should reject stale backup paths that are not files");
    assert!(matches!(
        stale_backup_error,
        CartridgeSaveBackendError::Io {
            operation: "remove stale backup save file",
            ..
        }
    ));
    fs::remove_dir_all(&stale_backup_path).expect("backup directory should be removable");

    let temp_key = CartridgeSaveKey::new("temp_create_failure").expect("key should be valid");
    let mut temp_backend = FilesystemCartridgeSaveBackend::with_time_source(
        root.as_path(),
        FixedCartridgeSaveTimeSource::new(1_002),
    );
    let temp_save_path = temp_backend.path_for_key(&temp_key);
    let temp_path = PathBuf::from(format!("{}.tmp", temp_save_path.display()));
    fs::create_dir_all(&temp_path).expect("temporary directory should be creatable");

    let create_temp_error = temp_backend
        .save(
            &temp_key,
            source.persistence_metadata(),
            &source.persistent_state(),
        )
        .expect_err("save should fail when the temporary path is already a directory");
    assert!(matches!(
        create_temp_error,
        CartridgeSaveBackendError::Io {
            operation: "create temporary save file",
            ..
        }
    ));

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn manager_backend_mut_can_seed_saves_and_surface_restore_failures() {
    let key = CartridgeSaveKey::new("restore_failure").expect("key should be valid");
    let backend =
        InMemoryCartridgeSaveBackend::with_time_source(FixedCartridgeSaveTimeSource::new(1_200));
    let mut manager = HardwarePersistenceManager::new(
        backend,
        key.clone(),
        HardwarePersistenceFlushPolicy::Manual,
    );

    let mut source = load_cartridge(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    source.write_rom(0x0000, 0x0A);
    source.write_ram(0xA000, 0x0D);
    source.write_rom(0x0000, 0x00);

    manager
        .backend_mut()
        .save(
            &key,
            source.persistence_metadata(),
            &source.persistent_state(),
        )
        .expect("manual backend seeding should succeed");
    assert_eq!(manager.backend().len(), 1);

    let mut incompatible_target = load_cartridge(build_banked_mbc1_rom(0x03, 0x03, 0x03));
    let error = manager
        .load_into(&mut incompatible_target)
        .expect_err("restore should reject a mismatched persistent state kind");
    assert!(matches!(
        error,
        HardwarePersistenceError::Restore(
            gb_core::CartridgePersistentStateError::KindMismatch { .. }
        )
    ));
    assert!(format!("{error}").contains("cartridge restore failed"));
    assert!(std::error::Error::source(&error).is_none());
}
