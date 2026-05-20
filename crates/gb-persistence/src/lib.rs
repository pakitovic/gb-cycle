use gb_core::{
    BootRomKind, CartridgeSlotState, CompatibilityPolicy, ConsoleModel, DiagnosticPolicy,
    ExecutionMode, HeuristicPolicy, HostPlatform, OperatingMode, OverridePolicy, StartupMode,
    TCycle, ValidationPolicy,
};
use gb_core::{
    CartridgePersistenceMetadata, CartridgePersistenceProfile, CartridgePersistentStateError,
    CartridgeRamPayloadKind, CartridgeSlot, Huc3RtcPersistentState, MachineSaveState,
    MachineSaveStateMetadata, Mbc3RtcPersistentState, PersistentCartState,
    SaveStateByteFingerprint,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SAVE_MAGIC: [u8; 8] = *b"GBCSAVE\0";
const MACHINE_SAVE_STATE_MAGIC: [u8; 8] = *b"GBSTATE\0";
pub const CURRENT_SAVE_FORMAT_VERSION: u16 = 1;
pub const CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION: u16 = 1;
pub const SAVE_FILE_EXTENSION: &str = "gbsav";
pub const SAVE_FILE_EXTENSION_P2: &str = "gbsa2";
pub const SAVE_FILE_EXTENSION_P3: &str = "gbsa3";
pub const SAVE_FILE_EXTENSION_P4: &str = "gbsa4";
pub const EXTERNAL_SAVE_FILE_EXTENSION: &str = "sav";
pub const EXTERNAL_SAVE_FILE_EXTENSION_P2: &str = "sa2";
pub const EXTERNAL_SAVE_FILE_EXTENSION_P3: &str = "sa3";
pub const EXTERNAL_SAVE_FILE_EXTENSION_P4: &str = "sa4";
pub const MACHINE_SAVE_STATE_FILE_EXTENSION: &str = "gbstate";
const MBC2_RAM_NIBBLE_COUNT: usize = 512;
const MBC2_MGBA_PACKED_BYTE_COUNT: usize = MBC2_RAM_NIBBLE_COUNT / 2;
const MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP: usize = 44;
const MBC3_EXTERNAL_RTC_SUFFIX_LEN: usize = 48;
const RAM_KIND_LINEAR_TAG: u8 = 0;
const RAM_KIND_MBC2_TAG: u8 = 1;
const PROFILE_NONE_TAG: u8 = 0;
const PROFILE_NON_PERSISTENT_RAM_TAG: u8 = 1;
const PROFILE_PERSISTENT_RAM_TAG: u8 = 2;
const PROFILE_PERSISTENT_RTC_TAG: u8 = 3;
const PROFILE_PERSISTENT_RAM_AND_RTC_TAG: u8 = 4;
const PROFILE_PERSISTENT_RAM_AND_FLASH_TAG: u8 = 5;
const PROFILE_PERSISTENT_EEPROM_TAG: u8 = 6;
const STATE_NONE_TAG: u8 = 0;
const STATE_NO_MBC_RAM_TAG: u8 = 1;
const STATE_MBC1_RAM_TAG: u8 = 2;
const STATE_MBC2_RAM_TAG: u8 = 3;
const STATE_MBC3_RTC_TAG: u8 = 4;
const STATE_MBC3_RAM_TAG: u8 = 5;
const STATE_MBC3_RAM_RTC_TAG: u8 = 6;
const STATE_MBC5_RAM_TAG: u8 = 7;
const STATE_MMM01_RAM_TAG: u8 = 8;
const STATE_HUC1_RAM_TAG: u8 = 9;
const STATE_HUC3_TAG: u8 = 10;
const STATE_POCKET_CAMERA_RAM_TAG: u8 = 11;
const STATE_MBC6_TAG: u8 = 12;
const STATE_MBC7_EEPROM_TAG: u8 = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum CartridgeSaveFileExtension {
    #[default]
    P1,
    P2,
    P3,
    P4,
}

impl CartridgeSaveFileExtension {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::P1 => SAVE_FILE_EXTENSION,
            Self::P2 => SAVE_FILE_EXTENSION_P2,
            Self::P3 => SAVE_FILE_EXTENSION_P3,
            Self::P4 => SAVE_FILE_EXTENSION_P4,
        }
    }

    pub const fn external_as_str(self) -> &'static str {
        match self {
            Self::P1 => EXTERNAL_SAVE_FILE_EXTENSION,
            Self::P2 => EXTERNAL_SAVE_FILE_EXTENSION_P2,
            Self::P3 => EXTERNAL_SAVE_FILE_EXTENSION_P3,
            Self::P4 => EXTERNAL_SAVE_FILE_EXTENSION_P4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CartridgeSaveKey(String);

impl CartridgeSaveKey {
    pub fn new(key: impl Into<String>) -> Result<Self, CartridgeSaveKeyError> {
        let key = key.into();
        if key.is_empty() {
            return Err(CartridgeSaveKeyError::Empty);
        }

        for (index, character) in key.chars().enumerate() {
            if !is_portable_save_key_character(character) {
                return Err(CartridgeSaveKeyError::InvalidCharacter { index, character });
            }
        }

        Ok(Self(key))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_portable_save_key_character(character: char) -> bool {
    !character.is_control()
        && !matches!(
            character,
            '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
        )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CartridgeSaveKeyError {
    Empty,
    InvalidCharacter { index: usize, character: char },
}

impl fmt::Display for CartridgeSaveKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "save key must not be empty"),
            Self::InvalidCharacter { index, character } => write!(
                f,
                "save key contains invalid character `{character}` at index {index}"
            ),
        }
    }
}

impl std::error::Error for CartridgeSaveKeyError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CartridgeSaveBackendMetadata {
    pub format_version: u16,
    pub saved_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeSaveEnvelope {
    pub backend_metadata: CartridgeSaveBackendMetadata,
    pub cartridge_metadata: CartridgePersistenceMetadata,
    pub persistent_state: PersistentCartState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineSaveStateBackendMetadata {
    pub format_version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineSaveStateEnvelope {
    pub backend_metadata: MachineSaveStateBackendMetadata,
    pub state_metadata: MachineSaveStateMetadata,
    pub state: MachineSaveState,
}

impl MachineSaveStateEnvelope {
    pub fn new(state: MachineSaveState) -> Self {
        Self {
            backend_metadata: MachineSaveStateBackendMetadata {
                format_version: CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION,
            },
            state_metadata: state.metadata().clone(),
            state,
        }
    }
}

pub trait CartridgeSaveTimeSource {
    fn now_unix_seconds(&self) -> u64;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCartridgeSaveTimeSource;

impl CartridgeSaveTimeSource for SystemCartridgeSaveTimeSource {
    fn now_unix_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedCartridgeSaveTimeSource {
    unix_seconds: u64,
}

impl FixedCartridgeSaveTimeSource {
    pub const fn new(unix_seconds: u64) -> Self {
        Self { unix_seconds }
    }
}

impl CartridgeSaveTimeSource for FixedCartridgeSaveTimeSource {
    fn now_unix_seconds(&self) -> u64 {
        self.unix_seconds
    }
}

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

// These host-side result enums intentionally carry the full save envelope so
// callers and tests can inspect the exact persisted payload without a second
// lookup or hidden indirection.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwarePersistenceLoadResult {
    SkippedNotBatteryBacked,
    NoSavePresent,
    Restored {
        persisted: CartridgeSaveEnvelope,
        elapsed_off_session_seconds: u64,
    },
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwarePersistenceSaveResult {
    SkippedNotBatteryBacked,
    Saved(CartridgeSaveEnvelope),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwarePersistenceFlushPolicy {
    Manual,
    SaveOnClose,
    AutoFlushAfterPersistibleWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwarePersistenceTrigger {
    PersistibleWrite,
    ManualFlush,
    ForcedSave,
    Close,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwarePersistenceActionResult {
    SkippedNotBatteryBacked,
    Deferred,
    NoPendingSave,
    SkippedByFlushPolicy {
        trigger: HardwarePersistenceTrigger,
    },
    Saved {
        trigger: HardwarePersistenceTrigger,
        envelope: CartridgeSaveEnvelope,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExternalSaveExportFormat {
    #[default]
    Mgba,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalSaveError {
    UnsupportedPersistentState {
        state_kind: &'static str,
    },
    UnsupportedPersistenceProfile {
        profile: CartridgePersistenceProfile,
    },
    StateProfileMismatch {
        state_kind: &'static str,
        profile: CartridgePersistenceProfile,
    },
    InvalidLength {
        context: &'static str,
        expected: ExternalSaveLengthExpectation,
        actual: usize,
    },
    UnsupportedStateShape {
        state_kind: &'static str,
        reason: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSaveLengthExpectation {
    Exact(usize),
    Either { first: usize, second: usize },
}

impl fmt::Display for ExternalSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPersistentState { state_kind } => {
                write!(f, "external .sav conversion does not support {state_kind}")
            }
            Self::UnsupportedPersistenceProfile { profile } => {
                write!(
                    f,
                    "external .sav conversion does not support persistence profile {profile:?}"
                )
            }
            Self::StateProfileMismatch {
                state_kind,
                profile,
            } => write!(
                f,
                "persistent state {state_kind} does not match cartridge persistence profile {profile:?}"
            ),
            Self::InvalidLength {
                context,
                expected,
                actual,
            } => match expected {
                ExternalSaveLengthExpectation::Exact(expected) => write!(
                    f,
                    "invalid external .sav length for {context}: expected {expected} bytes, got {actual}"
                ),
                ExternalSaveLengthExpectation::Either { first, second } => write!(
                    f,
                    "invalid external .sav length for {context}: expected {first} or {second} bytes, got {actual}"
                ),
            },
            Self::UnsupportedStateShape { state_kind, reason } => {
                write!(
                    f,
                    "external .sav conversion does not support {state_kind}: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for ExternalSaveError {}

#[derive(Debug)]
pub struct HardwarePersistenceManager<B> {
    backend: B,
    key: CartridgeSaveKey,
    flush_policy: HardwarePersistenceFlushPolicy,
    dirty: bool,
}

#[derive(Debug)]
pub enum HardwarePersistenceError {
    Backend(CartridgeSaveBackendError),
    Restore(CartridgePersistentStateError),
}

impl fmt::Display for HardwarePersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(error) => error.fmt(f),
            Self::Restore(error) => write!(f, "cartridge restore failed: {error:?}"),
        }
    }
}

impl std::error::Error for HardwarePersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend(error) => Some(error),
            Self::Restore(_) => None,
        }
    }
}

pub fn uses_battery_backed_hardware_persistence(metadata: CartridgePersistenceMetadata) -> bool {
    matches!(
        metadata.profile,
        CartridgePersistenceProfile::PersistentEeprom { .. }
    ) || metadata.has_battery
        && matches!(
            metadata.profile,
            CartridgePersistenceProfile::PersistentRam { .. }
                | CartridgePersistenceProfile::PersistentRtc
                | CartridgePersistenceProfile::PersistentRamAndRtc { .. }
                | CartridgePersistenceProfile::PersistentRamAndFlash { .. }
        )
}

pub fn load_hardware_cartridge_persistence<B: CartridgeSaveBackend>(
    backend: &B,
    key: &CartridgeSaveKey,
    cartridge: &mut CartridgeSlot,
) -> Result<HardwarePersistenceLoadResult, HardwarePersistenceError> {
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Ok(HardwarePersistenceLoadResult::SkippedNotBatteryBacked);
    }

    match backend
        .load(key)
        .map_err(HardwarePersistenceError::Backend)?
    {
        Some(envelope) => {
            let elapsed_off_session_seconds = backend
                .current_unix_seconds()
                .saturating_sub(envelope.backend_metadata.saved_at_unix_seconds);
            let mut restored_state = envelope.persistent_state.clone();
            apply_elapsed_off_session_seconds(&mut restored_state, elapsed_off_session_seconds);
            cartridge
                .restore_persistent_state(&restored_state)
                .map_err(HardwarePersistenceError::Restore)?;
            Ok(HardwarePersistenceLoadResult::Restored {
                persisted: envelope,
                elapsed_off_session_seconds,
            })
        }
        None => Ok(HardwarePersistenceLoadResult::NoSavePresent),
    }
}

pub fn save_hardware_cartridge_persistence<B: CartridgeSaveBackend>(
    backend: &mut B,
    key: &CartridgeSaveKey,
    cartridge: &CartridgeSlot,
) -> Result<HardwarePersistenceSaveResult, HardwarePersistenceError> {
    let metadata = cartridge.persistence_metadata();
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Ok(HardwarePersistenceSaveResult::SkippedNotBatteryBacked);
    }

    let envelope = backend
        .save(key, metadata, &cartridge.persistent_state())
        .map_err(HardwarePersistenceError::Backend)?;
    Ok(HardwarePersistenceSaveResult::Saved(envelope))
}

pub fn export_external_cartridge_save(
    envelope: &CartridgeSaveEnvelope,
    current_unix_seconds: u64,
) -> Result<Vec<u8>, ExternalSaveError> {
    let mut state = envelope.persistent_state.clone();
    let elapsed_off_session_seconds =
        current_unix_seconds.saturating_sub(envelope.backend_metadata.saved_at_unix_seconds);
    apply_elapsed_off_session_seconds(&mut state, elapsed_off_session_seconds);
    encode_external_cartridge_save(
        envelope.cartridge_metadata,
        &state,
        current_unix_seconds,
        ExternalSaveExportFormat::default(),
    )
}

pub fn encode_external_cartridge_save(
    metadata: CartridgePersistenceMetadata,
    state: &PersistentCartState,
    current_unix_seconds: u64,
    format: ExternalSaveExportFormat,
) -> Result<Vec<u8>, ExternalSaveError> {
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        });
    }

    match (metadata.profile, state) {
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::NoMbcRam { ram }
            | PersistentCartState::Mmm01Ram { ram }
            | PersistentCartState::Huc1Ram { ram }
            | PersistentCartState::Mbc1Ram { ram }
            | PersistentCartState::Mbc3Ram { ram }
            | PersistentCartState::Mbc5Ram { ram }
            | PersistentCartState::PocketCameraRam { ram },
        ) => encode_external_linear_ram(ram, byte_len),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count },
            },
            PersistentCartState::Mbc2Ram { ram_nibbles },
        ) => encode_external_mbc2_ram(ram_nibbles, cell_count, format),
        (CartridgePersistenceProfile::PersistentRtc, PersistentCartState::Mbc3Rtc { rtc }) => {
            let mut bytes = Vec::with_capacity(MBC3_EXTERNAL_RTC_SUFFIX_LEN);
            encode_external_mbc3_rtc_suffix(&mut bytes, *rtc, current_unix_seconds);
            Ok(bytes)
        }
        (
            CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc3RamRtc { ram, rtc },
        ) => {
            let mut bytes = encode_external_linear_ram(ram, byte_len)?;
            encode_external_mbc3_rtc_suffix(&mut bytes, *rtc, current_unix_seconds);
            Ok(bytes)
        }
        (
            CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
                flash_byte_len,
                hidden_byte_len,
            },
            PersistentCartState::Mbc6 {
                ram,
                flash,
                hidden_region,
                sector0_protected,
            },
        ) => encode_external_mbc6_save(
            ram,
            flash,
            hidden_region,
            *sector0_protected,
            byte_len,
            flash_byte_len,
            hidden_byte_len,
        ),
        (
            CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { .. },
                ..
            },
            _,
        ) => Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        }),
        (
            CartridgePersistenceProfile::PersistentEeprom { byte_len },
            PersistentCartState::Mbc7Eeprom { eeprom },
        ) => encode_external_linear_ram(eeprom, byte_len),
        (
            CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { .. },
            },
            _,
        ) => Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        }),
        (
            CartridgePersistenceProfile::PersistentRamAndRtc { .. },
            PersistentCartState::Huc3 { .. },
        ) => Err(ExternalSaveError::UnsupportedPersistentState {
            state_kind: persistent_state_kind_name(state),
        }),
        (CartridgePersistenceProfile::PersistentRam { .. }, PersistentCartState::Huc3 { .. })
        | (CartridgePersistenceProfile::PersistentRtc, PersistentCartState::Huc3 { .. }) => {
            Err(ExternalSaveError::UnsupportedPersistentState {
                state_kind: persistent_state_kind_name(state),
            })
        }
        (profile, _) => Err(ExternalSaveError::StateProfileMismatch {
            state_kind: persistent_state_kind_name(state),
            profile,
        }),
    }
}

pub fn import_external_cartridge_save(
    metadata: CartridgePersistenceMetadata,
    target_state: &PersistentCartState,
    bytes: &[u8],
    current_unix_seconds: u64,
) -> Result<PersistentCartState, ExternalSaveError> {
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        });
    }

    match (metadata.profile, target_state) {
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::NoMbcRam { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "linear RAM")
            .map(|ram| PersistentCartState::NoMbcRam { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mmm01Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MMM01 RAM")
            .map(|ram| PersistentCartState::Mmm01Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Huc1Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "HuC1 RAM")
            .map(|ram| PersistentCartState::Huc1Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc1Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MBC1 RAM")
            .map(|ram| PersistentCartState::Mbc1Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc3Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MBC3 RAM")
            .map(|ram| PersistentCartState::Mbc3Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc5Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MBC5 RAM")
            .map(|ram| PersistentCartState::Mbc5Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::PocketCameraRam { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "Pocket Camera RAM")
            .map(|ram| PersistentCartState::PocketCameraRam { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count },
            },
            PersistentCartState::Mbc2Ram { .. },
        ) => decode_external_mbc2_ram(bytes, cell_count)
            .map(|ram_nibbles| PersistentCartState::Mbc2Ram { ram_nibbles }),
        (CartridgePersistenceProfile::PersistentRtc, PersistentCartState::Mbc3Rtc { .. }) => {
            if !is_external_mbc3_rtc_suffix_len(bytes.len()) {
                return Err(ExternalSaveError::InvalidLength {
                    context: "MBC3 RTC",
                    expected: mbc3_external_rtc_suffix_length_expectation(),
                    actual: bytes.len(),
                });
            }
            let rtc = decode_external_mbc3_rtc_suffix(bytes, current_unix_seconds)?;
            Ok(PersistentCartState::Mbc3Rtc { rtc })
        }
        (
            CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc3RamRtc { .. },
        ) => {
            let expected_len_32bit_timestamp =
                byte_len + MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP;
            let expected_len = byte_len + MBC3_EXTERNAL_RTC_SUFFIX_LEN;
            if bytes.len() != expected_len_32bit_timestamp && bytes.len() != expected_len {
                return Err(ExternalSaveError::InvalidLength {
                    context: "MBC3 RAM+RTC",
                    expected: ExternalSaveLengthExpectation::Either {
                        first: expected_len_32bit_timestamp,
                        second: expected_len,
                    },
                    actual: bytes.len(),
                });
            }
            let ram = bytes[..byte_len].to_vec();
            let rtc = decode_external_mbc3_rtc_suffix(&bytes[byte_len..], current_unix_seconds)?;
            Ok(PersistentCartState::Mbc3RamRtc { ram, rtc })
        }
        (
            CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
                flash_byte_len,
                hidden_byte_len,
            },
            PersistentCartState::Mbc6 {
                hidden_region,
                sector0_protected,
                ..
            },
        ) => decode_external_mbc6_save(
            bytes,
            byte_len,
            flash_byte_len,
            hidden_byte_len,
            hidden_region,
            *sector0_protected,
        ),
        (
            CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { .. },
                ..
            },
            _,
        ) => Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        }),
        (
            CartridgePersistenceProfile::PersistentEeprom { byte_len },
            PersistentCartState::Mbc7Eeprom { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MBC7 EEPROM")
            .map(|eeprom| PersistentCartState::Mbc7Eeprom { eeprom }),
        (
            CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { .. },
            },
            _,
        ) => Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        }),
        (
            CartridgePersistenceProfile::PersistentRamAndRtc { .. },
            PersistentCartState::Huc3 { .. },
        ) => Err(ExternalSaveError::UnsupportedPersistentState {
            state_kind: persistent_state_kind_name(target_state),
        }),
        (profile, _) => Err(ExternalSaveError::StateProfileMismatch {
            state_kind: persistent_state_kind_name(target_state),
            profile,
        }),
    }
}

impl<B> HardwarePersistenceManager<B> {
    pub fn new(
        backend: B,
        key: CartridgeSaveKey,
        flush_policy: HardwarePersistenceFlushPolicy,
    ) -> Self {
        Self {
            backend,
            key,
            flush_policy,
            dirty: false,
        }
    }

    pub fn key(&self) -> &CartridgeSaveKey {
        &self.key
    }

    pub fn flush_policy(&self) -> HardwarePersistenceFlushPolicy {
        self.flush_policy
    }

    pub fn set_flush_policy(&mut self, flush_policy: HardwarePersistenceFlushPolicy) {
        self.flush_policy = flush_policy;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn into_backend(self) -> B {
        self.backend
    }
}

impl<B: CartridgeSaveBackend> HardwarePersistenceManager<B> {
    pub fn load_into(
        &mut self,
        cartridge: &mut CartridgeSlot,
    ) -> Result<HardwarePersistenceLoadResult, HardwarePersistenceError> {
        let result = load_hardware_cartridge_persistence(&self.backend, &self.key, cartridge)?;
        self.dirty = false;
        Ok(result)
    }

    pub fn note_persistible_write(
        &mut self,
        cartridge: &CartridgeSlot,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        if !uses_battery_backed_hardware_persistence(cartridge.persistence_metadata()) {
            self.dirty = false;
            return Ok(HardwarePersistenceActionResult::SkippedNotBatteryBacked);
        }

        self.dirty = true;
        match self.flush_policy {
            HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite => self.perform_save(
                cartridge,
                HardwarePersistenceTrigger::PersistibleWrite,
                true,
            ),
            HardwarePersistenceFlushPolicy::Manual
            | HardwarePersistenceFlushPolicy::SaveOnClose => {
                Ok(HardwarePersistenceActionResult::Deferred)
            }
        }
    }

    pub fn flush(
        &mut self,
        cartridge: &CartridgeSlot,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        self.perform_save(cartridge, HardwarePersistenceTrigger::ManualFlush, false)
    }

    pub fn force_save(
        &mut self,
        cartridge: &CartridgeSlot,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        self.perform_save(cartridge, HardwarePersistenceTrigger::ForcedSave, true)
    }

    pub fn close(
        &mut self,
        cartridge: &CartridgeSlot,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        if !uses_battery_backed_hardware_persistence(cartridge.persistence_metadata()) {
            self.dirty = false;
            return Ok(HardwarePersistenceActionResult::SkippedNotBatteryBacked);
        }

        if !self.dirty {
            return Ok(HardwarePersistenceActionResult::NoPendingSave);
        }

        match self.flush_policy {
            HardwarePersistenceFlushPolicy::Manual => {
                Ok(HardwarePersistenceActionResult::SkippedByFlushPolicy {
                    trigger: HardwarePersistenceTrigger::Close,
                })
            }
            HardwarePersistenceFlushPolicy::SaveOnClose
            | HardwarePersistenceFlushPolicy::AutoFlushAfterPersistibleWrite => {
                self.perform_save(cartridge, HardwarePersistenceTrigger::Close, true)
            }
        }
    }

    fn perform_save(
        &mut self,
        cartridge: &CartridgeSlot,
        trigger: HardwarePersistenceTrigger,
        save_when_clean: bool,
    ) -> Result<HardwarePersistenceActionResult, HardwarePersistenceError> {
        if !uses_battery_backed_hardware_persistence(cartridge.persistence_metadata()) {
            self.dirty = false;
            return Ok(HardwarePersistenceActionResult::SkippedNotBatteryBacked);
        }

        if !save_when_clean && !self.dirty {
            return Ok(HardwarePersistenceActionResult::NoPendingSave);
        }

        match save_hardware_cartridge_persistence(&mut self.backend, &self.key, cartridge)? {
            HardwarePersistenceSaveResult::SkippedNotBatteryBacked => {
                self.dirty = false;
                Ok(HardwarePersistenceActionResult::SkippedNotBatteryBacked)
            }
            HardwarePersistenceSaveResult::Saved(envelope) => {
                self.dirty = false;
                Ok(HardwarePersistenceActionResult::Saved { trigger, envelope })
            }
        }
    }
}

#[derive(Debug)]
pub enum CartridgeSaveBackendError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    InvalidMagic {
        actual: [u8; SAVE_MAGIC.len()],
    },
    UnexpectedEof {
        offset: usize,
        needed: usize,
        remaining: usize,
    },
    UnsupportedFormatVersion {
        version: u16,
    },
    UnsupportedRamPayloadKindTag {
        tag: u8,
    },
    UnsupportedPersistenceProfileTag {
        tag: u8,
    },
    UnsupportedPersistentStateTag {
        tag: u8,
    },
    UnsupportedMachineSaveStateTag {
        field: &'static str,
        tag: u8,
    },
    InvalidBooleanTag {
        field: &'static str,
        value: u8,
    },
    LengthOverflow {
        field: &'static str,
        value: usize,
    },
    InvalidMbc2NibbleValue {
        index: usize,
        value: u8,
    },
    InvalidHuc3NibbleValue {
        index: usize,
        value: u8,
    },
    MachineSaveStateCodec {
        operation: &'static str,
        message: String,
    },
    ExternalSave {
        operation: &'static str,
        path: PathBuf,
        source: ExternalSaveError,
    },
    MachineSaveStateMetadataMismatch,
    TrailingBytes {
        remaining: usize,
    },
}

impl fmt::Display for CartridgeSaveBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation, path, ..
            } => write!(f, "{operation} failed for {}", path.display()),
            Self::InvalidMagic { actual } => write!(f, "invalid save magic: {actual:?}"),
            Self::UnexpectedEof {
                offset,
                needed,
                remaining,
            } => write!(
                f,
                "unexpected end of save payload at offset {offset}: needed {needed} bytes but only {remaining} remain"
            ),
            Self::UnsupportedFormatVersion { version } => {
                write!(f, "unsupported save format version {version}")
            }
            Self::UnsupportedRamPayloadKindTag { tag } => {
                write!(f, "unsupported RAM payload kind tag {tag:#04X}")
            }
            Self::UnsupportedPersistenceProfileTag { tag } => {
                write!(f, "unsupported persistence profile tag {tag:#04X}")
            }
            Self::UnsupportedPersistentStateTag { tag } => {
                write!(f, "unsupported persistent state tag {tag:#04X}")
            }
            Self::UnsupportedMachineSaveStateTag { field, tag } => {
                write!(
                    f,
                    "unsupported machine save-state tag for {field}: {tag:#04X}"
                )
            }
            Self::InvalidBooleanTag { field, value } => {
                write!(f, "invalid boolean tag for {field}: {value:#04X}")
            }
            Self::LengthOverflow { field, value } => {
                write!(f, "{field} length {value} exceeds format capacity")
            }
            Self::InvalidMbc2NibbleValue { index, value } => write!(
                f,
                "invalid MBC2 nibble value {value:#04X} at logical cell {index}"
            ),
            Self::InvalidHuc3NibbleValue { index, value } => write!(
                f,
                "invalid HuC-3 nibble value {value:#04X} at logical cell {index}"
            ),
            Self::MachineSaveStateCodec { operation, message } => {
                write!(f, "machine save-state {operation} failed: {message}")
            }
            Self::ExternalSave {
                operation,
                path,
                source,
            } => write!(f, "{operation} failed for {}: {source}", path.display()),
            Self::MachineSaveStateMetadataMismatch => {
                write!(
                    f,
                    "machine save-state envelope metadata does not match payload metadata"
                )
            }
            Self::TrailingBytes { remaining } => {
                write!(f, "save payload has {remaining} trailing bytes")
            }
        }
    }
}

impl std::error::Error for CartridgeSaveBackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::ExternalSave { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct InMemoryCartridgeSaveBackend<C = SystemCartridgeSaveTimeSource> {
    clock: C,
    entries: BTreeMap<CartridgeSaveKey, Vec<u8>>,
}

impl InMemoryCartridgeSaveBackend<SystemCartridgeSaveTimeSource> {
    pub fn new() -> Self {
        Self::with_time_source(SystemCartridgeSaveTimeSource)
    }
}

impl Default for InMemoryCartridgeSaveBackend<SystemCartridgeSaveTimeSource> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> InMemoryCartridgeSaveBackend<C> {
    pub fn with_time_source(clock: C) -> Self {
        Self {
            clock,
            entries: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<C: CartridgeSaveTimeSource> CartridgeSaveBackend for InMemoryCartridgeSaveBackend<C> {
    fn current_unix_seconds(&self) -> u64 {
        self.clock.now_unix_seconds()
    }

    fn load(
        &self,
        key: &CartridgeSaveKey,
    ) -> Result<Option<CartridgeSaveEnvelope>, CartridgeSaveBackendError> {
        self.entries
            .get(key)
            .map(|bytes| decode_cartridge_save_envelope(bytes))
            .transpose()
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
        self.entries.insert(key.clone(), bytes);
        Ok(envelope)
    }

    fn delete(&mut self, key: &CartridgeSaveKey) -> Result<(), CartridgeSaveBackendError> {
        self.entries.remove(key);
        Ok(())
    }
}

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

fn external_save_error_allows_internal_fallback(error: &ExternalSaveError) -> bool {
    matches!(
        error,
        ExternalSaveError::UnsupportedPersistentState { .. }
            | ExternalSaveError::UnsupportedPersistenceProfile { .. }
            | ExternalSaveError::UnsupportedStateShape { .. }
    )
}

pub fn encode_cartridge_save_envelope(
    envelope: &CartridgeSaveEnvelope,
) -> Result<Vec<u8>, CartridgeSaveBackendError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&SAVE_MAGIC);
    write_u16(&mut bytes, envelope.backend_metadata.format_version);
    write_u64(&mut bytes, envelope.backend_metadata.saved_at_unix_seconds);
    write_bool(&mut bytes, envelope.cartridge_metadata.has_battery);
    write_bool(&mut bytes, envelope.cartridge_metadata.has_rtc);
    encode_persistence_profile(&mut bytes, envelope.cartridge_metadata.profile)?;
    encode_persistent_state(&mut bytes, &envelope.persistent_state)?;
    Ok(bytes)
}

pub fn decode_cartridge_save_envelope(
    bytes: &[u8],
) -> Result<CartridgeSaveEnvelope, CartridgeSaveBackendError> {
    let mut cursor = ByteCursor::new(bytes);
    let actual_magic = cursor.read_array::<{ SAVE_MAGIC.len() }>()?;
    if actual_magic != SAVE_MAGIC {
        return Err(CartridgeSaveBackendError::InvalidMagic {
            actual: actual_magic,
        });
    }

    let format_version = cursor.read_u16()?;
    if format_version != CURRENT_SAVE_FORMAT_VERSION {
        return Err(CartridgeSaveBackendError::UnsupportedFormatVersion {
            version: format_version,
        });
    }

    let saved_at_unix_seconds = cursor.read_u64()?;
    let has_battery = cursor.read_bool("has_battery")?;
    let has_rtc = cursor.read_bool("has_rtc")?;
    let profile = decode_persistence_profile(&mut cursor)?;
    let persistent_state = decode_persistent_state(&mut cursor)?;

    if cursor.remaining() != 0 {
        return Err(CartridgeSaveBackendError::TrailingBytes {
            remaining: cursor.remaining(),
        });
    }

    Ok(CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version,
            saved_at_unix_seconds,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery,
            has_rtc,
            profile,
        },
        persistent_state,
    })
}

pub fn encode_machine_save_state_envelope(
    envelope: &MachineSaveStateEnvelope,
) -> Result<Vec<u8>, CartridgeSaveBackendError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MACHINE_SAVE_STATE_MAGIC);
    write_u16(&mut bytes, envelope.backend_metadata.format_version);
    encode_machine_save_state_metadata(&mut bytes, &envelope.state_metadata)?;

    let mut payload = Vec::new();
    ciborium::into_writer(&envelope.state, &mut payload).map_err(|error| {
        CartridgeSaveBackendError::MachineSaveStateCodec {
            operation: "encode",
            message: error.to_string(),
        }
    })?;
    write_u32_checked(
        &mut bytes,
        payload.len(),
        "machine save-state payload byte_len",
    )?;
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

pub fn decode_machine_save_state_envelope(
    bytes: &[u8],
) -> Result<MachineSaveStateEnvelope, CartridgeSaveBackendError> {
    let mut cursor = ByteCursor::new(bytes);
    let actual_magic = cursor.read_array::<{ MACHINE_SAVE_STATE_MAGIC.len() }>()?;
    if actual_magic != MACHINE_SAVE_STATE_MAGIC {
        return Err(CartridgeSaveBackendError::InvalidMagic {
            actual: actual_magic,
        });
    }

    let format_version = cursor.read_u16()?;
    if format_version != CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION {
        return Err(CartridgeSaveBackendError::UnsupportedFormatVersion {
            version: format_version,
        });
    }

    let state_metadata = decode_machine_save_state_metadata(&mut cursor)?;
    let payload_len = cursor.read_u32()? as usize;
    let payload = cursor.read_exact(payload_len)?;
    let state: MachineSaveState = ciborium::from_reader(payload).map_err(|error| {
        CartridgeSaveBackendError::MachineSaveStateCodec {
            operation: "decode",
            message: error.to_string(),
        }
    })?;

    if state.metadata() != &state_metadata {
        return Err(CartridgeSaveBackendError::MachineSaveStateMetadataMismatch);
    }
    if cursor.remaining() != 0 {
        return Err(CartridgeSaveBackendError::TrailingBytes {
            remaining: cursor.remaining(),
        });
    }

    Ok(MachineSaveStateEnvelope {
        backend_metadata: MachineSaveStateBackendMetadata { format_version },
        state_metadata,
        state,
    })
}

fn write_save_file_with_safe_replace(
    path: &Path,
    bytes: &[u8],
) -> Result<(), CartridgeSaveBackendError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| CartridgeSaveBackendError::Io {
        operation: "create save directory",
        path: parent.to_path_buf(),
        source,
    })?;

    let temp_path = append_extension_suffix(path, ".tmp");
    let backup_path = append_extension_suffix(path, ".bak");
    if backup_path.exists() {
        fs::remove_file(&backup_path).map_err(|source| CartridgeSaveBackendError::Io {
            operation: "remove stale backup save file",
            path: backup_path.clone(),
            source,
        })?;
    }

    {
        let mut file =
            File::create(&temp_path).map_err(|source| CartridgeSaveBackendError::Io {
                operation: "create temporary save file",
                path: temp_path.clone(),
                source,
            })?;
        file.write_all(bytes)
            .map_err(|source| CartridgeSaveBackendError::Io {
                operation: "write temporary save file",
                path: temp_path.clone(),
                source,
            })?;
        file.sync_all()
            .map_err(|source| CartridgeSaveBackendError::Io {
                operation: "sync temporary save file",
                path: temp_path.clone(),
                source,
            })?;
    }

    let had_existing_target = path.exists();
    if had_existing_target {
        fs::rename(path, &backup_path).map_err(|source| CartridgeSaveBackendError::Io {
            operation: "move previous save file to backup",
            path: path.to_path_buf(),
            source,
        })?;
    }

    match fs::rename(&temp_path, path) {
        Ok(()) => {}
        Err(source) => {
            if had_existing_target && !path.exists() {
                let _ = fs::rename(&backup_path, path);
            }
            let _ = fs::remove_file(&temp_path);
            return Err(CartridgeSaveBackendError::Io {
                operation: "replace save file",
                path: path.to_path_buf(),
                source,
            });
        }
    }

    if had_existing_target && backup_path.exists() {
        fs::remove_file(&backup_path).map_err(|source| CartridgeSaveBackendError::Io {
            operation: "remove backup save file",
            path: backup_path,
            source,
        })?;
    }

    Ok(())
}

fn append_extension_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = OsString::from(path.as_os_str());
    os.push(suffix);
    PathBuf::from(os)
}

fn apply_elapsed_off_session_seconds(state: &mut PersistentCartState, elapsed_seconds: u64) {
    match state {
        PersistentCartState::Mbc3Rtc { rtc } => rtc.apply_elapsed_seconds(elapsed_seconds),
        PersistentCartState::Mbc3RamRtc { rtc, .. } => rtc.apply_elapsed_seconds(elapsed_seconds),
        PersistentCartState::Huc3 { rtc, .. } => rtc.apply_elapsed_seconds(elapsed_seconds),
        _ => {}
    }
}

fn encode_external_linear_ram(
    ram: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, ExternalSaveError> {
    if ram.len() != expected_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "linear RAM state",
            expected: ExternalSaveLengthExpectation::Exact(expected_len),
            actual: ram.len(),
        });
    }
    Ok(ram.to_vec())
}

fn decode_external_linear_ram(
    bytes: &[u8],
    expected_len: usize,
    context: &'static str,
) -> Result<Vec<u8>, ExternalSaveError> {
    if bytes.len() != expected_len {
        return Err(ExternalSaveError::InvalidLength {
            context,
            expected: ExternalSaveLengthExpectation::Exact(expected_len),
            actual: bytes.len(),
        });
    }
    Ok(bytes.to_vec())
}

fn encode_external_mbc6_save(
    ram: &[u8],
    flash: &[u8],
    hidden_region: &[u8],
    sector0_protected: bool,
    expected_ram_len: usize,
    expected_flash_len: usize,
    expected_hidden_len: usize,
) -> Result<Vec<u8>, ExternalSaveError> {
    if ram.len() != expected_ram_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 RAM state",
            expected: ExternalSaveLengthExpectation::Exact(expected_ram_len),
            actual: ram.len(),
        });
    }
    if flash.len() != expected_flash_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 flash state",
            expected: ExternalSaveLengthExpectation::Exact(expected_flash_len),
            actual: flash.len(),
        });
    }
    if hidden_region.len() != expected_hidden_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 hidden flash state",
            expected: ExternalSaveLengthExpectation::Exact(expected_hidden_len),
            actual: hidden_region.len(),
        });
    }
    if sector0_protected || hidden_region.iter().any(|byte| *byte != 0xFF) {
        return Err(ExternalSaveError::UnsupportedStateShape {
            state_kind: "Mbc6",
            reason: "raw .sav only carries SRAM followed by main flash, not hidden flash or the non-volatile sector-0 protection bit",
        });
    }

    let mut bytes = Vec::with_capacity(expected_ram_len + expected_flash_len);
    bytes.extend_from_slice(ram);
    bytes.extend_from_slice(flash);
    Ok(bytes)
}

fn decode_external_mbc6_save(
    bytes: &[u8],
    expected_ram_len: usize,
    expected_flash_len: usize,
    expected_hidden_len: usize,
    target_hidden_region: &[u8],
    target_sector0_protected: bool,
) -> Result<PersistentCartState, ExternalSaveError> {
    if target_hidden_region.len() != expected_hidden_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 hidden flash target",
            expected: ExternalSaveLengthExpectation::Exact(expected_hidden_len),
            actual: target_hidden_region.len(),
        });
    }
    if target_sector0_protected || target_hidden_region.iter().any(|byte| *byte != 0xFF) {
        return Err(ExternalSaveError::UnsupportedStateShape {
            state_kind: "Mbc6",
            reason: "raw .sav import cannot merge into a target with hidden flash data or sector-0 protection already set",
        });
    }

    let expected_len = expected_ram_len + expected_flash_len;
    if bytes.len() != expected_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 RAM+flash",
            expected: ExternalSaveLengthExpectation::Exact(expected_len),
            actual: bytes.len(),
        });
    }

    Ok(PersistentCartState::Mbc6 {
        ram: bytes[..expected_ram_len].to_vec(),
        flash: bytes[expected_ram_len..].to_vec(),
        hidden_region: vec![0xFF; expected_hidden_len],
        sector0_protected: false,
    })
}

fn encode_external_mbc2_ram(
    ram_nibbles: &[u8; MBC2_RAM_NIBBLE_COUNT],
    expected_cell_count: usize,
    format: ExternalSaveExportFormat,
) -> Result<Vec<u8>, ExternalSaveError> {
    if expected_cell_count != MBC2_RAM_NIBBLE_COUNT {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC2 metadata",
            expected: ExternalSaveLengthExpectation::Exact(MBC2_RAM_NIBBLE_COUNT),
            actual: expected_cell_count,
        });
    }

    match format {
        ExternalSaveExportFormat::Mgba => {
            let mut bytes = Vec::with_capacity(MBC2_MGBA_PACKED_BYTE_COUNT);
            for pair in ram_nibbles.chunks_exact(2) {
                bytes.push((pair[0] & 0x0F) | ((pair[1] & 0x0F) << 4));
            }
            Ok(bytes)
        }
    }
}

fn decode_external_mbc2_ram(
    bytes: &[u8],
    expected_cell_count: usize,
) -> Result<[u8; MBC2_RAM_NIBBLE_COUNT], ExternalSaveError> {
    if expected_cell_count != MBC2_RAM_NIBBLE_COUNT {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC2 metadata",
            expected: ExternalSaveLengthExpectation::Exact(MBC2_RAM_NIBBLE_COUNT),
            actual: expected_cell_count,
        });
    }

    let mut ram_nibbles = [0; MBC2_RAM_NIBBLE_COUNT];
    match bytes.len() {
        MBC2_MGBA_PACKED_BYTE_COUNT => {
            for (index, byte) in bytes.iter().copied().enumerate() {
                ram_nibbles[index * 2] = byte & 0x0F;
                ram_nibbles[index * 2 + 1] = (byte >> 4) & 0x0F;
            }
            Ok(ram_nibbles)
        }
        MBC2_RAM_NIBBLE_COUNT => {
            for (index, byte) in bytes.iter().copied().enumerate() {
                ram_nibbles[index] = byte & 0x0F;
            }
            Ok(ram_nibbles)
        }
        actual => Err(ExternalSaveError::InvalidLength {
            context: "MBC2 RAM",
            expected: ExternalSaveLengthExpectation::Either {
                first: MBC2_MGBA_PACKED_BYTE_COUNT,
                second: MBC2_RAM_NIBBLE_COUNT,
            },
            actual,
        }),
    }
}

fn encode_external_mbc3_rtc_suffix(
    bytes: &mut Vec<u8>,
    rtc: Mbc3RtcPersistentState,
    current_unix_seconds: u64,
) {
    let day_low = (rtc.day_counter & 0x00FF) as u8;
    let day_high =
        ((rtc.day_counter >> 8) as u8 & 0x01) | ((rtc.halt as u8) << 6) | ((rtc.carry as u8) << 7);
    let fields = [rtc.seconds, rtc.minutes, rtc.hours, day_low, day_high];

    for field in fields {
        write_u32(bytes, u32::from(field));
    }
    for field in fields {
        write_u32(bytes, u32::from(field));
    }
    write_u64(bytes, current_unix_seconds);
}

fn decode_external_mbc3_rtc_suffix(
    bytes: &[u8],
    current_unix_seconds: u64,
) -> Result<Mbc3RtcPersistentState, ExternalSaveError> {
    if !is_external_mbc3_rtc_suffix_len(bytes.len()) {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC3 RTC",
            expected: mbc3_external_rtc_suffix_length_expectation(),
            actual: bytes.len(),
        });
    }

    let seconds = read_external_u32_low_u8(bytes, 0) & 0x3F;
    let minutes = read_external_u32_low_u8(bytes, 4) & 0x3F;
    let hours = read_external_u32_low_u8(bytes, 8) & 0x1F;
    let day_low = read_external_u32_low_u8(bytes, 12);
    let day_high = read_external_u32_low_u8(bytes, 16);
    let saved_unix_seconds = match bytes.len() {
        MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP => {
            u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as u64
        }
        MBC3_EXTERNAL_RTC_SUFFIX_LEN => u64::from_le_bytes([
            bytes[40], bytes[41], bytes[42], bytes[43], bytes[44], bytes[45], bytes[46], bytes[47],
        ]),
        _ => unreachable!("MBC3 RTC suffix length should be validated before timestamp decode"),
    };

    let mut rtc = Mbc3RtcPersistentState {
        seconds,
        minutes,
        hours,
        day_counter: u16::from(day_low) | (u16::from(day_high & 0x01) << 8),
        halt: day_high & 0x40 != 0,
        carry: day_high & 0x80 != 0,
    };
    rtc.apply_elapsed_seconds(current_unix_seconds.saturating_sub(saved_unix_seconds));
    Ok(rtc)
}

fn is_external_mbc3_rtc_suffix_len(len: usize) -> bool {
    matches!(
        len,
        MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP | MBC3_EXTERNAL_RTC_SUFFIX_LEN
    )
}

fn mbc3_external_rtc_suffix_length_expectation() -> ExternalSaveLengthExpectation {
    ExternalSaveLengthExpectation::Either {
        first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
        second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
    }
}

fn read_external_u32_low_u8(bytes: &[u8], offset: usize) -> u8 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]) as u8
}

fn persistent_state_kind_name(state: &PersistentCartState) -> &'static str {
    match state {
        PersistentCartState::None => "None",
        PersistentCartState::NoMbcRam { .. } => "NoMbcRam",
        PersistentCartState::Mmm01Ram { .. } => "Mmm01Ram",
        PersistentCartState::Huc1Ram { .. } => "Huc1Ram",
        PersistentCartState::Huc3 { .. } => "Huc3",
        PersistentCartState::Mbc1Ram { .. } => "Mbc1Ram",
        PersistentCartState::Mbc2Ram { .. } => "Mbc2Ram",
        PersistentCartState::Mbc3Rtc { .. } => "Mbc3Rtc",
        PersistentCartState::Mbc3Ram { .. } => "Mbc3Ram",
        PersistentCartState::Mbc3RamRtc { .. } => "Mbc3RamRtc",
        PersistentCartState::Mbc5Ram { .. } => "Mbc5Ram",
        PersistentCartState::Mbc6 { .. } => "Mbc6",
        PersistentCartState::Mbc7Eeprom { .. } => "Mbc7Eeprom",
        PersistentCartState::PocketCameraRam { .. } => "PocketCameraRam",
    }
}

fn encode_machine_save_state_metadata(
    bytes: &mut Vec<u8>,
    metadata: &MachineSaveStateMetadata,
) -> Result<(), CartridgeSaveBackendError> {
    bytes.push(encode_console_model(metadata.console_model));
    bytes.push(encode_operating_mode(metadata.operating_mode));
    bytes.push(encode_host_platform(metadata.host_platform));
    bytes.push(encode_startup_mode(metadata.startup_mode));
    encode_compatibility_policy(bytes, &metadata.compatibility);
    write_u64(bytes, metadata.next_t_cycle.get());
    bytes.push(encode_cartridge_slot_state(metadata.cartridge.state));
    encode_fingerprint(bytes, metadata.cartridge.rom_fingerprint);
    bytes.push(encode_startup_mode(metadata.boot.startup_mode));
    bytes.push(encode_boot_rom_kind(metadata.boot.boot_rom_kind));
    write_bool(bytes, metadata.boot.boot_rom_mapped);
    encode_fingerprint(bytes, metadata.boot.boot_rom_fingerprint);
    Ok(())
}

fn decode_machine_save_state_metadata(
    cursor: &mut ByteCursor<'_>,
) -> Result<MachineSaveStateMetadata, CartridgeSaveBackendError> {
    let console_model = decode_console_model(cursor.read_u8()?, "console_model")?;
    let operating_mode = decode_operating_mode(cursor.read_u8()?, "operating_mode")?;
    let host_platform = decode_host_platform(cursor.read_u8()?, "host_platform")?;
    let startup_mode = decode_startup_mode(cursor.read_u8()?, "startup_mode")?;
    let compatibility = decode_compatibility_policy(cursor)?;
    let next_t_cycle = TCycle::new(cursor.read_u64()?);
    let cartridge_state = decode_cartridge_slot_state(cursor.read_u8()?, "cartridge.state")?;
    let rom_fingerprint = decode_fingerprint(cursor, "cartridge.rom_fingerprint")?;
    let boot_startup_mode = decode_startup_mode(cursor.read_u8()?, "boot.startup_mode")?;
    let boot_rom_kind = decode_boot_rom_kind(cursor.read_u8()?, "boot.boot_rom_kind")?;
    let boot_rom_mapped = cursor.read_bool("boot.boot_rom_mapped")?;
    let boot_rom_fingerprint = decode_fingerprint(cursor, "boot.boot_rom_fingerprint")?;

    Ok(MachineSaveStateMetadata {
        console_model,
        operating_mode,
        host_platform,
        startup_mode,
        compatibility,
        next_t_cycle,
        cartridge: gb_core::MachineCartridgeSaveStateMetadata {
            state: cartridge_state,
            rom_fingerprint,
        },
        boot: gb_core::MachineBootSaveStateMetadata {
            startup_mode: boot_startup_mode,
            boot_rom_kind,
            boot_rom_mapped,
            boot_rom_fingerprint,
        },
    })
}

fn encode_compatibility_policy(bytes: &mut Vec<u8>, policy: &CompatibilityPolicy) {
    bytes.push(encode_execution_mode(policy.execution_mode));
    bytes.push(encode_validation_policy(policy.validation_policy));
    bytes.push(encode_heuristic_policy(policy.heuristic_policy));
    encode_override_policy(bytes, &policy.override_policy);
    bytes.push(encode_diagnostic_policy(policy.diagnostic_policy));
}

fn decode_compatibility_policy(
    cursor: &mut ByteCursor<'_>,
) -> Result<CompatibilityPolicy, CartridgeSaveBackendError> {
    Ok(CompatibilityPolicy {
        execution_mode: decode_execution_mode(cursor.read_u8()?, "compatibility.execution_mode")?,
        validation_policy: decode_validation_policy(
            cursor.read_u8()?,
            "compatibility.validation_policy",
        )?,
        heuristic_policy: decode_heuristic_policy(
            cursor.read_u8()?,
            "compatibility.heuristic_policy",
        )?,
        override_policy: decode_override_policy(cursor)?,
        diagnostic_policy: decode_diagnostic_policy(
            cursor.read_u8()?,
            "compatibility.diagnostic_policy",
        )?,
    })
}

fn encode_override_policy(bytes: &mut Vec<u8>, policy: &OverridePolicy) {
    encode_optional_tag(bytes, policy.forced_console_model.map(encode_console_model));
    encode_optional_tag(
        bytes,
        policy.forced_operating_mode.map(encode_operating_mode),
    );
    encode_optional_tag(bytes, policy.forced_host_platform.map(encode_host_platform));
    encode_optional_tag(bytes, policy.forced_startup_mode.map(encode_startup_mode));
}

fn decode_override_policy(
    cursor: &mut ByteCursor<'_>,
) -> Result<OverridePolicy, CartridgeSaveBackendError> {
    Ok(OverridePolicy {
        forced_console_model: decode_optional_tag(cursor, "override.forced_console_model")?
            .map(|tag| decode_console_model(tag, "override.forced_console_model"))
            .transpose()?,
        forced_operating_mode: decode_optional_tag(cursor, "override.forced_operating_mode")?
            .map(|tag| decode_operating_mode(tag, "override.forced_operating_mode"))
            .transpose()?,
        forced_host_platform: decode_optional_tag(cursor, "override.forced_host_platform")?
            .map(|tag| decode_host_platform(tag, "override.forced_host_platform"))
            .transpose()?,
        forced_startup_mode: decode_optional_tag(cursor, "override.forced_startup_mode")?
            .map(|tag| decode_startup_mode(tag, "override.forced_startup_mode"))
            .transpose()?,
    })
}

fn encode_optional_tag(bytes: &mut Vec<u8>, tag: Option<u8>) {
    write_bool(bytes, tag.is_some());
    if let Some(tag) = tag {
        bytes.push(tag);
    }
}

fn decode_optional_tag(
    cursor: &mut ByteCursor<'_>,
    field: &'static str,
) -> Result<Option<u8>, CartridgeSaveBackendError> {
    let present = cursor.read_bool(field)?;
    if present {
        Ok(Some(cursor.read_u8()?))
    } else {
        Ok(None)
    }
}

fn encode_fingerprint(bytes: &mut Vec<u8>, fingerprint: Option<SaveStateByteFingerprint>) {
    write_bool(bytes, fingerprint.is_some());
    if let Some(fingerprint) = fingerprint {
        write_u64(bytes, fingerprint.len);
        write_u64(bytes, fingerprint.fnv1a64);
    }
}

fn decode_fingerprint(
    cursor: &mut ByteCursor<'_>,
    field: &'static str,
) -> Result<Option<SaveStateByteFingerprint>, CartridgeSaveBackendError> {
    let present = cursor.read_bool(field)?;
    if !present {
        return Ok(None);
    }

    Ok(Some(SaveStateByteFingerprint {
        len: cursor.read_u64()?,
        fnv1a64: cursor.read_u64()?,
    }))
}

fn encode_console_model(value: ConsoleModel) -> u8 {
    match value {
        ConsoleModel::GameBoy => 1,
        ConsoleModel::GameBoyPocket => 2,
        ConsoleModel::GameBoyColor => 3,
        ConsoleModel::GameBoyLight => 4,
    }
}

fn decode_console_model(
    tag: u8,
    field: &'static str,
) -> Result<ConsoleModel, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(ConsoleModel::GameBoy),
        1 => Ok(ConsoleModel::GameBoy),
        2 => Ok(ConsoleModel::GameBoyPocket),
        3 => Ok(ConsoleModel::GameBoyColor),
        4 => Ok(ConsoleModel::GameBoyLight),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn encode_operating_mode(value: OperatingMode) -> u8 {
    match value {
        OperatingMode::Dmg => 0,
        OperatingMode::Cgb => 1,
        OperatingMode::GbCompatible => 2,
        OperatingMode::CgbDmgExt => 3,
    }
}

fn decode_operating_mode(
    tag: u8,
    field: &'static str,
) -> Result<OperatingMode, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(OperatingMode::Dmg),
        1 => Ok(OperatingMode::Cgb),
        2 => Ok(OperatingMode::GbCompatible),
        3 => Ok(OperatingMode::CgbDmgExt),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn encode_host_platform(value: HostPlatform) -> u8 {
    match value {
        HostPlatform::Handheld => 0,
        HostPlatform::Sgb1 => 1,
        HostPlatform::Sgb2 => 2,
    }
}

fn decode_host_platform(
    tag: u8,
    field: &'static str,
) -> Result<HostPlatform, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(HostPlatform::Handheld),
        1 => Ok(HostPlatform::Sgb1),
        2 => Ok(HostPlatform::Sgb2),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn encode_startup_mode(value: StartupMode) -> u8 {
    match value {
        StartupMode::SkipBoot => 0,
        StartupMode::RealBoot => 1,
        StartupMode::CustomBoot => 2,
    }
}

fn decode_startup_mode(
    tag: u8,
    field: &'static str,
) -> Result<StartupMode, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(StartupMode::SkipBoot),
        1 => Ok(StartupMode::RealBoot),
        2 => Ok(StartupMode::CustomBoot),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn encode_execution_mode(value: ExecutionMode) -> u8 {
    match value {
        ExecutionMode::Strict => 0,
        ExecutionMode::Permissive => 1,
        ExecutionMode::Experimental => 2,
    }
}

fn decode_execution_mode(
    tag: u8,
    field: &'static str,
) -> Result<ExecutionMode, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(ExecutionMode::Strict),
        1 => Ok(ExecutionMode::Permissive),
        2 => Ok(ExecutionMode::Experimental),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn encode_validation_policy(value: ValidationPolicy) -> u8 {
    match value {
        ValidationPolicy::Strict => 0,
        ValidationPolicy::Warn => 1,
        ValidationPolicy::Ignore => 2,
    }
}

fn decode_validation_policy(
    tag: u8,
    field: &'static str,
) -> Result<ValidationPolicy, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(ValidationPolicy::Strict),
        1 => Ok(ValidationPolicy::Warn),
        2 => Ok(ValidationPolicy::Ignore),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn encode_heuristic_policy(value: HeuristicPolicy) -> u8 {
    match value {
        HeuristicPolicy::Disabled => 0,
        HeuristicPolicy::AllowExperimental => 1,
    }
}

fn decode_heuristic_policy(
    tag: u8,
    field: &'static str,
) -> Result<HeuristicPolicy, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(HeuristicPolicy::Disabled),
        1 => Ok(HeuristicPolicy::AllowExperimental),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn encode_diagnostic_policy(value: DiagnosticPolicy) -> u8 {
    match value {
        DiagnosticPolicy::Quiet => 0,
        DiagnosticPolicy::Standard => 1,
        DiagnosticPolicy::Verbose => 2,
    }
}

fn decode_diagnostic_policy(
    tag: u8,
    field: &'static str,
) -> Result<DiagnosticPolicy, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(DiagnosticPolicy::Quiet),
        1 => Ok(DiagnosticPolicy::Standard),
        2 => Ok(DiagnosticPolicy::Verbose),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn encode_cartridge_slot_state(value: CartridgeSlotState) -> u8 {
    match value {
        CartridgeSlotState::Empty => 0,
        CartridgeSlotState::NoMbc => 1,
        CartridgeSlotState::Mmm01 => 2,
        CartridgeSlotState::M161 => 3,
        CartridgeSlotState::Huc1 => 4,
        CartridgeSlotState::Huc3 => 5,
        CartridgeSlotState::Mbc1 => 6,
        CartridgeSlotState::Mbc2 => 7,
        CartridgeSlotState::Mbc3 => 8,
        CartridgeSlotState::Mbc5 => 9,
        CartridgeSlotState::PocketCamera => 10,
        CartridgeSlotState::Mbc6 => 11,
        CartridgeSlotState::Mbc7 => 12,
    }
}

fn decode_cartridge_slot_state(
    tag: u8,
    field: &'static str,
) -> Result<CartridgeSlotState, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(CartridgeSlotState::Empty),
        1 => Ok(CartridgeSlotState::NoMbc),
        2 => Ok(CartridgeSlotState::Mmm01),
        3 => Ok(CartridgeSlotState::M161),
        4 => Ok(CartridgeSlotState::Huc1),
        5 => Ok(CartridgeSlotState::Huc3),
        6 => Ok(CartridgeSlotState::Mbc1),
        7 => Ok(CartridgeSlotState::Mbc2),
        8 => Ok(CartridgeSlotState::Mbc3),
        9 => Ok(CartridgeSlotState::Mbc5),
        10 => Ok(CartridgeSlotState::PocketCamera),
        11 => Ok(CartridgeSlotState::Mbc6),
        12 => Ok(CartridgeSlotState::Mbc7),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn encode_boot_rom_kind(value: BootRomKind) -> u8 {
    match value {
        BootRomKind::Dmg0 => 0,
        BootRomKind::Dmg => 1,
        BootRomKind::Mgb => 2,
        BootRomKind::Cgb0 => 4,
        BootRomKind::Cgb => 3,
        BootRomKind::CgbE => 5,
    }
}

fn decode_boot_rom_kind(
    tag: u8,
    field: &'static str,
) -> Result<BootRomKind, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(BootRomKind::Dmg0),
        1 => Ok(BootRomKind::Dmg),
        2 => Ok(BootRomKind::Mgb),
        3 => Ok(BootRomKind::Cgb),
        4 => Ok(BootRomKind::Cgb0),
        5 => Ok(BootRomKind::CgbE),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

fn unsupported_machine_save_state_tag<T>(
    field: &'static str,
    tag: u8,
) -> Result<T, CartridgeSaveBackendError> {
    Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { field, tag })
}

fn encode_persistence_profile(
    bytes: &mut Vec<u8>,
    profile: CartridgePersistenceProfile,
) -> Result<(), CartridgeSaveBackendError> {
    match profile {
        CartridgePersistenceProfile::None => bytes.push(PROFILE_NONE_TAG),
        CartridgePersistenceProfile::NonPersistentRam { ram } => {
            bytes.push(PROFILE_NON_PERSISTENT_RAM_TAG);
            encode_ram_payload_kind(bytes, ram)?;
        }
        CartridgePersistenceProfile::PersistentRam { ram } => {
            bytes.push(PROFILE_PERSISTENT_RAM_TAG);
            encode_ram_payload_kind(bytes, ram)?;
        }
        CartridgePersistenceProfile::PersistentRtc => bytes.push(PROFILE_PERSISTENT_RTC_TAG),
        CartridgePersistenceProfile::PersistentRamAndRtc { ram } => {
            bytes.push(PROFILE_PERSISTENT_RAM_AND_RTC_TAG);
            encode_ram_payload_kind(bytes, ram)?;
        }
        CartridgePersistenceProfile::PersistentRamAndFlash {
            ram,
            flash_byte_len,
            hidden_byte_len,
        } => {
            bytes.push(PROFILE_PERSISTENT_RAM_AND_FLASH_TAG);
            encode_ram_payload_kind(bytes, ram)?;
            write_u32_checked(bytes, flash_byte_len, "MBC6 flash byte_len")?;
            write_u32_checked(bytes, hidden_byte_len, "MBC6 hidden flash byte_len")?;
        }
        CartridgePersistenceProfile::PersistentEeprom { byte_len } => {
            bytes.push(PROFILE_PERSISTENT_EEPROM_TAG);
            write_u32_checked(bytes, byte_len, "persistent EEPROM byte_len")?;
        }
    }
    Ok(())
}

fn decode_persistence_profile(
    cursor: &mut ByteCursor<'_>,
) -> Result<CartridgePersistenceProfile, CartridgeSaveBackendError> {
    let tag = cursor.read_u8()?;
    match tag {
        PROFILE_NONE_TAG => Ok(CartridgePersistenceProfile::None),
        PROFILE_NON_PERSISTENT_RAM_TAG => Ok(CartridgePersistenceProfile::NonPersistentRam {
            ram: decode_ram_payload_kind(cursor)?,
        }),
        PROFILE_PERSISTENT_RAM_TAG => Ok(CartridgePersistenceProfile::PersistentRam {
            ram: decode_ram_payload_kind(cursor)?,
        }),
        PROFILE_PERSISTENT_RTC_TAG => Ok(CartridgePersistenceProfile::PersistentRtc),
        PROFILE_PERSISTENT_RAM_AND_RTC_TAG => {
            Ok(CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: decode_ram_payload_kind(cursor)?,
            })
        }
        PROFILE_PERSISTENT_RAM_AND_FLASH_TAG => {
            Ok(CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: decode_ram_payload_kind(cursor)?,
                flash_byte_len: cursor.read_u32()? as usize,
                hidden_byte_len: cursor.read_u32()? as usize,
            })
        }
        PROFILE_PERSISTENT_EEPROM_TAG => Ok(CartridgePersistenceProfile::PersistentEeprom {
            byte_len: cursor.read_u32()? as usize,
        }),
        _ => Err(CartridgeSaveBackendError::UnsupportedPersistenceProfileTag { tag }),
    }
}

fn encode_ram_payload_kind(
    bytes: &mut Vec<u8>,
    kind: CartridgeRamPayloadKind,
) -> Result<(), CartridgeSaveBackendError> {
    match kind {
        CartridgeRamPayloadKind::Linear { byte_len } => {
            bytes.push(RAM_KIND_LINEAR_TAG);
            write_u32_checked(bytes, byte_len, "linear RAM byte_len")?;
        }
        CartridgeRamPayloadKind::Mbc2Nibbles { cell_count } => {
            bytes.push(RAM_KIND_MBC2_TAG);
            write_u32_checked(bytes, cell_count, "MBC2 RAM cell_count")?;
        }
    }
    Ok(())
}

fn decode_ram_payload_kind(
    cursor: &mut ByteCursor<'_>,
) -> Result<CartridgeRamPayloadKind, CartridgeSaveBackendError> {
    let tag = cursor.read_u8()?;
    let len = cursor.read_u32()? as usize;
    match tag {
        RAM_KIND_LINEAR_TAG => Ok(CartridgeRamPayloadKind::Linear { byte_len: len }),
        RAM_KIND_MBC2_TAG => Ok(CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: len }),
        _ => Err(CartridgeSaveBackendError::UnsupportedRamPayloadKindTag { tag }),
    }
}

fn encode_persistent_state(
    bytes: &mut Vec<u8>,
    state: &PersistentCartState,
) -> Result<(), CartridgeSaveBackendError> {
    match state {
        PersistentCartState::None => bytes.push(STATE_NONE_TAG),
        PersistentCartState::NoMbcRam { ram } => {
            bytes.push(STATE_NO_MBC_RAM_TAG);
            encode_linear_ram(bytes, ram, "NoMBC RAM")?;
        }
        PersistentCartState::Mbc1Ram { ram } => {
            bytes.push(STATE_MBC1_RAM_TAG);
            encode_linear_ram(bytes, ram, "MBC1 RAM")?;
        }
        PersistentCartState::Mbc2Ram { ram_nibbles } => {
            bytes.push(STATE_MBC2_RAM_TAG);
            write_u32_checked(bytes, ram_nibbles.len(), "MBC2 RAM nibble count")?;
            for (index, value) in ram_nibbles.iter().copied().enumerate() {
                if value > 0x0F {
                    return Err(CartridgeSaveBackendError::InvalidMbc2NibbleValue { index, value });
                }
                bytes.push(value);
            }
        }
        PersistentCartState::Mbc3Rtc { rtc } => {
            bytes.push(STATE_MBC3_RTC_TAG);
            encode_rtc(bytes, *rtc);
        }
        PersistentCartState::Mbc3Ram { ram } => {
            bytes.push(STATE_MBC3_RAM_TAG);
            encode_linear_ram(bytes, ram, "MBC3 RAM")?;
        }
        PersistentCartState::Mbc3RamRtc { ram, rtc } => {
            bytes.push(STATE_MBC3_RAM_RTC_TAG);
            encode_linear_ram(bytes, ram, "MBC3 RAM")?;
            encode_rtc(bytes, *rtc);
        }
        PersistentCartState::Mbc5Ram { ram } => {
            bytes.push(STATE_MBC5_RAM_TAG);
            encode_linear_ram(bytes, ram, "MBC5 RAM")?;
        }
        PersistentCartState::Mbc6 {
            ram,
            flash,
            hidden_region,
            sector0_protected,
        } => {
            bytes.push(STATE_MBC6_TAG);
            encode_linear_ram(bytes, ram, "MBC6 RAM")?;
            encode_linear_ram(bytes, flash, "MBC6 flash")?;
            encode_linear_ram(bytes, hidden_region, "MBC6 hidden flash")?;
            write_bool(bytes, *sector0_protected);
        }
        PersistentCartState::Mmm01Ram { ram } => {
            bytes.push(STATE_MMM01_RAM_TAG);
            encode_linear_ram(bytes, ram, "MMM01 RAM")?;
        }
        PersistentCartState::Huc1Ram { ram } => {
            bytes.push(STATE_HUC1_RAM_TAG);
            encode_linear_ram(bytes, ram, "HuC1 RAM")?;
        }
        PersistentCartState::Huc3 {
            ram,
            mcu_ram,
            rtc,
            rom_bank,
            ram_bank,
            select_mode,
            access_address,
            mailbox_command,
            mailbox_argument,
            last_response_nybble,
            semaphore_ready,
            ir_emitter_on,
            ir_light_detected,
            last_control_write,
            last_unsupported_command,
            last_unsupported_argument,
        } => {
            bytes.push(STATE_HUC3_TAG);
            encode_linear_ram(bytes, ram, "HuC-3 RAM")?;
            write_u32_checked(bytes, mcu_ram.len(), "HuC-3 MCU RAM nibble count")?;
            for (index, value) in mcu_ram.iter().copied().enumerate() {
                if value > 0x0F {
                    return Err(CartridgeSaveBackendError::InvalidHuc3NibbleValue { index, value });
                }
                bytes.push(value);
            }
            encode_huc3_rtc(bytes, *rtc);
            bytes.push(*rom_bank);
            bytes.push(*ram_bank);
            bytes.push(*select_mode);
            bytes.push(*access_address);
            bytes.push(*mailbox_command);
            bytes.push(*mailbox_argument);
            bytes.push(*last_response_nybble);
            write_bool(bytes, *semaphore_ready);
            write_bool(bytes, *ir_emitter_on);
            write_bool(bytes, *ir_light_detected);
            encode_optional_u8(bytes, *last_control_write);
            encode_optional_u8(bytes, *last_unsupported_command);
            encode_optional_u8(bytes, *last_unsupported_argument);
        }
        PersistentCartState::PocketCameraRam { ram } => {
            bytes.push(STATE_POCKET_CAMERA_RAM_TAG);
            encode_linear_ram(bytes, ram, "Pocket Camera RAM")?;
        }
        PersistentCartState::Mbc7Eeprom { eeprom } => {
            bytes.push(STATE_MBC7_EEPROM_TAG);
            encode_linear_ram(bytes, eeprom, "MBC7 EEPROM")?;
        }
    }
    Ok(())
}

fn decode_persistent_state(
    cursor: &mut ByteCursor<'_>,
) -> Result<PersistentCartState, CartridgeSaveBackendError> {
    let tag = cursor.read_u8()?;
    match tag {
        STATE_NONE_TAG => Ok(PersistentCartState::None),
        STATE_NO_MBC_RAM_TAG => Ok(PersistentCartState::NoMbcRam {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC1_RAM_TAG => Ok(PersistentCartState::Mbc1Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC2_RAM_TAG => {
            let cell_count = cursor.read_u32()? as usize;
            let nibble_bytes = cursor.read_vec(cell_count)?;
            let mut ram_nibbles = [0; MBC2_RAM_NIBBLE_COUNT];
            if cell_count != ram_nibbles.len() {
                return Err(CartridgeSaveBackendError::LengthOverflow {
                    field: "decoded MBC2 RAM nibble count",
                    value: cell_count,
                });
            }
            for (index, value) in nibble_bytes.into_iter().enumerate() {
                if value > 0x0F {
                    return Err(CartridgeSaveBackendError::InvalidMbc2NibbleValue { index, value });
                }
                ram_nibbles[index] = value;
            }
            Ok(PersistentCartState::Mbc2Ram { ram_nibbles })
        }
        STATE_MBC3_RTC_TAG => Ok(PersistentCartState::Mbc3Rtc {
            rtc: decode_rtc(cursor)?,
        }),
        STATE_MBC3_RAM_TAG => Ok(PersistentCartState::Mbc3Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC3_RAM_RTC_TAG => Ok(PersistentCartState::Mbc3RamRtc {
            ram: decode_linear_ram(cursor)?,
            rtc: decode_rtc(cursor)?,
        }),
        STATE_MBC5_RAM_TAG => Ok(PersistentCartState::Mbc5Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC6_TAG => Ok(PersistentCartState::Mbc6 {
            ram: decode_linear_ram(cursor)?,
            flash: decode_linear_ram(cursor)?,
            hidden_region: decode_linear_ram(cursor)?,
            sector0_protected: cursor.read_bool("mbc6.sector0_protected")?,
        }),
        STATE_MMM01_RAM_TAG => Ok(PersistentCartState::Mmm01Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_HUC1_RAM_TAG => Ok(PersistentCartState::Huc1Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_HUC3_TAG => {
            let ram = decode_linear_ram(cursor)?;
            let nibble_count = cursor.read_u32()? as usize;
            if nibble_count != 256 {
                return Err(CartridgeSaveBackendError::LengthOverflow {
                    field: "decoded HuC-3 MCU RAM nibble count",
                    value: nibble_count,
                });
            }
            let nibble_bytes = cursor.read_vec(nibble_count)?;
            let mut mcu_ram = [0; 256];
            for (index, value) in nibble_bytes.into_iter().enumerate() {
                if value > 0x0F {
                    return Err(CartridgeSaveBackendError::InvalidHuc3NibbleValue { index, value });
                }
                mcu_ram[index] = value;
            }
            Ok(PersistentCartState::Huc3 {
                ram,
                mcu_ram,
                rtc: decode_huc3_rtc(cursor)?,
                rom_bank: cursor.read_u8()?,
                ram_bank: cursor.read_u8()?,
                select_mode: cursor.read_u8()?,
                access_address: cursor.read_u8()?,
                mailbox_command: cursor.read_u8()?,
                mailbox_argument: cursor.read_u8()?,
                last_response_nybble: cursor.read_u8()?,
                semaphore_ready: cursor.read_bool("huc3.semaphore_ready")?,
                ir_emitter_on: cursor.read_bool("huc3.ir_emitter_on")?,
                ir_light_detected: cursor.read_bool("huc3.ir_light_detected")?,
                last_control_write: decode_optional_u8(cursor, "huc3.last_control_write")?,
                last_unsupported_command: decode_optional_u8(
                    cursor,
                    "huc3.last_unsupported_command",
                )?,
                last_unsupported_argument: decode_optional_u8(
                    cursor,
                    "huc3.last_unsupported_argument",
                )?,
            })
        }
        STATE_POCKET_CAMERA_RAM_TAG => Ok(PersistentCartState::PocketCameraRam {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC7_EEPROM_TAG => Ok(PersistentCartState::Mbc7Eeprom {
            eeprom: decode_linear_ram(cursor)?,
        }),
        _ => Err(CartridgeSaveBackendError::UnsupportedPersistentStateTag { tag }),
    }
}

fn encode_linear_ram(
    bytes: &mut Vec<u8>,
    ram: &[u8],
    field: &'static str,
) -> Result<(), CartridgeSaveBackendError> {
    write_u32_checked(bytes, ram.len(), field)?;
    bytes.extend_from_slice(ram);
    Ok(())
}

fn decode_linear_ram(cursor: &mut ByteCursor<'_>) -> Result<Vec<u8>, CartridgeSaveBackendError> {
    let len = cursor.read_u32()? as usize;
    cursor.read_vec(len)
}

fn encode_rtc(bytes: &mut Vec<u8>, rtc: Mbc3RtcPersistentState) {
    bytes.push(rtc.seconds);
    bytes.push(rtc.minutes);
    bytes.push(rtc.hours);
    write_u16(bytes, rtc.day_counter);
    write_bool(bytes, rtc.halt);
    write_bool(bytes, rtc.carry);
}

fn decode_rtc(
    cursor: &mut ByteCursor<'_>,
) -> Result<Mbc3RtcPersistentState, CartridgeSaveBackendError> {
    Ok(Mbc3RtcPersistentState {
        seconds: cursor.read_u8()?,
        minutes: cursor.read_u8()?,
        hours: cursor.read_u8()?,
        day_counter: cursor.read_u16()?,
        halt: cursor.read_bool("rtc.halt")?,
        carry: cursor.read_bool("rtc.carry")?,
    })
}

fn encode_huc3_rtc(bytes: &mut Vec<u8>, rtc: Huc3RtcPersistentState) {
    write_u16(bytes, rtc.current_minutes_of_day);
    write_u16(bytes, rtc.current_days);
    bytes.push(rtc.current_subminute_seconds);
    write_u16(bytes, rtc.event_minutes_of_day);
    write_u16(bytes, rtc.event_days);
}

fn decode_huc3_rtc(
    cursor: &mut ByteCursor<'_>,
) -> Result<Huc3RtcPersistentState, CartridgeSaveBackendError> {
    Ok(Huc3RtcPersistentState {
        current_minutes_of_day: cursor.read_u16()?,
        current_days: cursor.read_u16()?,
        current_subminute_seconds: cursor.read_u8()?,
        event_minutes_of_day: cursor.read_u16()?,
        event_days: cursor.read_u16()?,
    })
}

fn encode_optional_u8(bytes: &mut Vec<u8>, value: Option<u8>) {
    write_bool(bytes, value.is_some());
    bytes.push(value.unwrap_or(0));
}

fn decode_optional_u8(
    cursor: &mut ByteCursor<'_>,
    field: &'static str,
) -> Result<Option<u8>, CartridgeSaveBackendError> {
    let present = cursor.read_bool(field)?;
    let value = cursor.read_u8()?;
    Ok(present.then_some(value))
}

fn write_bool(bytes: &mut Vec<u8>, value: bool) {
    bytes.push(u8::from(value));
}

fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u32_checked(
    bytes: &mut Vec<u8>,
    value: usize,
    field: &'static str,
) -> Result<(), CartridgeSaveBackendError> {
    let value = u32::try_from(value)
        .map_err(|_| CartridgeSaveBackendError::LengthOverflow { field, value })?;
    write_u32(bytes, value);
    Ok(())
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn read_u8(&mut self) -> Result<u8, CartridgeSaveBackendError> {
        let bytes = self.read_exact(1)?;
        Ok(bytes[0])
    }

    fn read_u16(&mut self) -> Result<u16, CartridgeSaveBackendError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, CartridgeSaveBackendError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, CartridgeSaveBackendError> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_bool(&mut self, field: &'static str) -> Result<bool, CartridgeSaveBackendError> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(CartridgeSaveBackendError::InvalidBooleanTag { field, value }),
        }
    }

    fn read_vec(&mut self, len: usize) -> Result<Vec<u8>, CartridgeSaveBackendError> {
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], CartridgeSaveBackendError> {
        let bytes = self.read_exact(N)?;
        let mut array = [0; N];
        array.copy_from_slice(bytes);
        Ok(array)
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], CartridgeSaveBackendError> {
        if self.remaining() < len {
            return Err(CartridgeSaveBackendError::UnexpectedEof {
                offset: self.offset,
                needed: len,
                remaining: self.remaining(),
            });
        }
        let start = self.offset;
        self.offset += len;
        Ok(&self.bytes[start..self.offset])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assert_round_trip(envelope: CartridgeSaveEnvelope) {
        let bytes = encode_cartridge_save_envelope(&envelope).expect("encode should succeed");
        let decoded = decode_cartridge_save_envelope(&bytes).expect("decode should succeed");
        assert_eq!(decoded, envelope);
    }

    fn machine_save_state_envelope() -> MachineSaveStateEnvelope {
        let mut machine = gb_core::Machine::new(
            gb_core::MachineConfig::new(ConsoleModel::GameBoy)
                .with_startup_mode(StartupMode::SkipBoot),
        );
        for _ in 0..16 {
            machine.step_t_cycle();
        }
        MachineSaveStateEnvelope::new(machine.capture_save_state())
    }

    #[test]
    fn machine_save_state_metadata_codec_covers_tags_fingerprints_and_overrides() {
        for value in [
            ConsoleModel::GameBoy,
            ConsoleModel::GameBoy,
            ConsoleModel::GameBoyPocket,
            ConsoleModel::GameBoyColor,
        ] {
            assert_eq!(
                decode_console_model(encode_console_model(value), "console_model")
                    .expect("console model tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_console_model(0xFF, "console_model"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag {
                field: "console_model",
                tag: 0xFF,
            })
        ));

        for value in [
            OperatingMode::Dmg,
            OperatingMode::Cgb,
            OperatingMode::GbCompatible,
            OperatingMode::CgbDmgExt,
        ] {
            assert_eq!(
                decode_operating_mode(encode_operating_mode(value), "operating_mode")
                    .expect("operating mode tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_operating_mode(0xFF, "operating_mode"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
        ));

        for value in [
            HostPlatform::Handheld,
            HostPlatform::Sgb1,
            HostPlatform::Sgb2,
        ] {
            assert_eq!(
                decode_host_platform(encode_host_platform(value), "host_platform")
                    .expect("host platform tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_host_platform(0xFF, "host_platform"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
        ));

        for value in [
            StartupMode::SkipBoot,
            StartupMode::CustomBoot,
            StartupMode::RealBoot,
        ] {
            assert_eq!(
                decode_startup_mode(encode_startup_mode(value), "startup_mode")
                    .expect("startup mode tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_startup_mode(0xFF, "startup_mode"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
        ));

        for value in [
            ExecutionMode::Strict,
            ExecutionMode::Permissive,
            ExecutionMode::Experimental,
        ] {
            assert_eq!(
                decode_execution_mode(encode_execution_mode(value), "execution_mode")
                    .expect("execution mode tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_execution_mode(0xFF, "execution_mode"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
        ));

        for value in [
            ValidationPolicy::Strict,
            ValidationPolicy::Warn,
            ValidationPolicy::Ignore,
        ] {
            assert_eq!(
                decode_validation_policy(encode_validation_policy(value), "validation_policy")
                    .expect("validation policy tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_validation_policy(0xFF, "validation_policy"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
        ));

        for value in [
            HeuristicPolicy::Disabled,
            HeuristicPolicy::AllowExperimental,
        ] {
            assert_eq!(
                decode_heuristic_policy(encode_heuristic_policy(value), "heuristic_policy")
                    .expect("heuristic policy tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_heuristic_policy(0xFF, "heuristic_policy"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
        ));

        for value in [
            DiagnosticPolicy::Quiet,
            DiagnosticPolicy::Standard,
            DiagnosticPolicy::Verbose,
        ] {
            assert_eq!(
                decode_diagnostic_policy(encode_diagnostic_policy(value), "diagnostic_policy")
                    .expect("diagnostic policy tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_diagnostic_policy(0xFF, "diagnostic_policy"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
        ));

        for value in [
            CartridgeSlotState::Empty,
            CartridgeSlotState::NoMbc,
            CartridgeSlotState::Mmm01,
            CartridgeSlotState::M161,
            CartridgeSlotState::Huc1,
            CartridgeSlotState::Huc3,
            CartridgeSlotState::Mbc1,
            CartridgeSlotState::Mbc2,
            CartridgeSlotState::Mbc3,
            CartridgeSlotState::Mbc5,
            CartridgeSlotState::Mbc6,
            CartridgeSlotState::Mbc7,
            CartridgeSlotState::PocketCamera,
        ] {
            assert_eq!(
                decode_cartridge_slot_state(encode_cartridge_slot_state(value), "cartridge.state")
                    .expect("cartridge slot tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_cartridge_slot_state(0xFF, "cartridge.state"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
        ));

        for value in [
            BootRomKind::Dmg0,
            BootRomKind::Dmg,
            BootRomKind::Mgb,
            BootRomKind::Cgb,
        ] {
            assert_eq!(
                decode_boot_rom_kind(encode_boot_rom_kind(value), "boot.boot_rom_kind")
                    .expect("boot ROM kind tag should decode"),
                value
            );
        }
        assert!(matches!(
            decode_boot_rom_kind(0xFF, "boot.boot_rom_kind"),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
        ));

        let metadata = MachineSaveStateMetadata {
            console_model: ConsoleModel::GameBoyColor,
            operating_mode: OperatingMode::GbCompatible,
            host_platform: HostPlatform::Sgb2,
            startup_mode: StartupMode::RealBoot,
            compatibility: CompatibilityPolicy {
                execution_mode: ExecutionMode::Experimental,
                validation_policy: ValidationPolicy::Ignore,
                heuristic_policy: HeuristicPolicy::AllowExperimental,
                override_policy: OverridePolicy {
                    forced_console_model: Some(ConsoleModel::GameBoyPocket),
                    forced_operating_mode: Some(OperatingMode::Dmg),
                    forced_host_platform: Some(HostPlatform::Sgb1),
                    forced_startup_mode: Some(StartupMode::SkipBoot),
                },
                diagnostic_policy: DiagnosticPolicy::Verbose,
            },
            next_t_cycle: TCycle::new(0x1234_5678),
            cartridge: gb_core::MachineCartridgeSaveStateMetadata {
                state: CartridgeSlotState::PocketCamera,
                rom_fingerprint: Some(SaveStateByteFingerprint {
                    len: 1024 * 1024,
                    fnv1a64: 0xA5A5_5A5A_DEAD_BEEF,
                }),
            },
            boot: gb_core::MachineBootSaveStateMetadata {
                startup_mode: StartupMode::RealBoot,
                boot_rom_kind: BootRomKind::Cgb,
                boot_rom_mapped: true,
                boot_rom_fingerprint: Some(SaveStateByteFingerprint {
                    len: 0x900,
                    fnv1a64: 0x55AA_AA55_1234_5678,
                }),
            },
        };

        let mut bytes = Vec::new();
        encode_machine_save_state_metadata(&mut bytes, &metadata).expect("metadata should encode");
        let mut cursor = ByteCursor::new(&bytes);
        assert_eq!(
            decode_machine_save_state_metadata(&mut cursor).expect("metadata should decode"),
            metadata
        );
        assert_eq!(cursor.remaining(), 0);
    }

    #[test]
    fn machine_save_state_envelope_round_trips_the_versioned_payload() {
        let envelope = machine_save_state_envelope();
        let bytes = encode_machine_save_state_envelope(&envelope).expect("encode should succeed");

        assert_eq!(
            &bytes[..MACHINE_SAVE_STATE_MAGIC.len()],
            MACHINE_SAVE_STATE_MAGIC.as_slice()
        );

        let decoded = decode_machine_save_state_envelope(&bytes).expect("decode should succeed");
        assert_eq!(decoded, envelope);
    }

    #[test]
    fn machine_save_state_decode_rejects_invalid_headers_and_payload_shape() {
        let envelope = machine_save_state_envelope();
        let encoded = encode_machine_save_state_envelope(&envelope).expect("encode should succeed");

        let mut invalid_magic = encoded.clone();
        invalid_magic[0] = b'X';
        assert!(matches!(
            decode_machine_save_state_envelope(&invalid_magic),
            Err(CartridgeSaveBackendError::InvalidMagic { .. })
        ));

        let mut future_version = encoded.clone();
        future_version[MACHINE_SAVE_STATE_MAGIC.len()..MACHINE_SAVE_STATE_MAGIC.len() + 2]
            .copy_from_slice(&(CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION + 1).to_le_bytes());
        assert!(matches!(
            decode_machine_save_state_envelope(&future_version),
            Err(CartridgeSaveBackendError::UnsupportedFormatVersion {
                version
            }) if version == CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION + 1
        ));

        let mut invalid_model_tag = encoded.clone();
        invalid_model_tag[MACHINE_SAVE_STATE_MAGIC.len() + 2] = 0xFF;
        assert!(matches!(
            decode_machine_save_state_envelope(&invalid_model_tag),
            Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag {
                field: "console_model",
                tag: 0xFF,
            })
        ));

        let truncated = &encoded[..encoded.len() - 1];
        assert!(matches!(
            decode_machine_save_state_envelope(truncated),
            Err(CartridgeSaveBackendError::UnexpectedEof { .. })
        ));

        let mut corrupt_payload = encoded.clone();
        let last = corrupt_payload
            .last_mut()
            .expect("payload should not be empty");
        *last ^= 0x5A;
        assert!(matches!(
            decode_machine_save_state_envelope(&corrupt_payload),
            Err(CartridgeSaveBackendError::MachineSaveStateCodec {
                operation: "decode",
                ..
            })
        ));

        let mut trailing = encoded.clone();
        trailing.push(0xAA);
        assert!(matches!(
            decode_machine_save_state_envelope(&trailing),
            Err(CartridgeSaveBackendError::TrailingBytes { remaining: 1 })
        ));
    }

    #[test]
    fn machine_save_state_decode_rejects_metadata_that_disagrees_with_payload() {
        let envelope = machine_save_state_envelope();
        let mut encoded =
            encode_machine_save_state_envelope(&envelope).expect("encode should succeed");

        let operating_mode_offset = MACHINE_SAVE_STATE_MAGIC.len() + 3;
        encoded[operating_mode_offset] = encode_operating_mode(OperatingMode::GbCompatible);

        assert!(matches!(
            decode_machine_save_state_envelope(&encoded),
            Err(CartridgeSaveBackendError::MachineSaveStateMetadataMismatch)
        ));
    }

    #[test]
    fn save_key_rejects_invalid_characters_and_empty_values() {
        assert_eq!(CartridgeSaveKey::new(""), Err(CartridgeSaveKeyError::Empty));
        assert_eq!(
            CartridgeSaveKey::new("phase/6"),
            Err(CartridgeSaveKeyError::InvalidCharacter {
                index: 5,
                character: '/',
            })
        );
        assert!(CartridgeSaveKey::new("phase6_save").is_ok());
    }

    #[test]
    fn encode_and_decode_round_trip_the_versioned_envelope() {
        let envelope = CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 1_700_000_000,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: true,
                profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 4 },
                },
            },
            persistent_state: PersistentCartState::Mbc3RamRtc {
                ram: vec![0x11, 0x22, 0x33, 0x44],
                rtc: Mbc3RtcPersistentState {
                    seconds: 59,
                    minutes: 58,
                    hours: 12,
                    day_counter: 0x101,
                    halt: true,
                    carry: false,
                },
            },
        };

        assert_round_trip(envelope);
    }

    #[test]
    fn round_trip_covers_remaining_profile_and_state_variants() {
        let mut huc3_mcu_ram = [0; 256];
        huc3_mcu_ram[0] = 0x0A;
        huc3_mcu_ram[1] = 0x0B;
        huc3_mcu_ram[255] = 0x0F;

        let profiles_and_states = vec![
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 11,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: false,
                    has_rtc: false,
                    profile: CartridgePersistenceProfile::None,
                },
                persistent_state: PersistentCartState::None,
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 12,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: false,
                    has_rtc: false,
                    profile: CartridgePersistenceProfile::NonPersistentRam {
                        ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
                    },
                },
                persistent_state: PersistentCartState::NoMbcRam {
                    ram: vec![0x11, 0x22],
                },
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 13,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: true,
                    has_rtc: true,
                    profile: CartridgePersistenceProfile::PersistentRtc,
                },
                persistent_state: PersistentCartState::Mbc3Rtc {
                    rtc: Mbc3RtcPersistentState {
                        seconds: 59,
                        minutes: 58,
                        hours: 7,
                        day_counter: 0x81,
                        halt: false,
                        carry: true,
                    },
                },
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 14,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: true,
                    has_rtc: false,
                    profile: CartridgePersistenceProfile::PersistentRam {
                        ram: CartridgeRamPayloadKind::Linear { byte_len: 3 },
                    },
                },
                persistent_state: PersistentCartState::Mbc3Ram {
                    ram: vec![0x33, 0x44, 0x55],
                },
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 15,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: true,
                    has_rtc: false,
                    profile: CartridgePersistenceProfile::PersistentRam {
                        ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
                    },
                },
                persistent_state: PersistentCartState::Mbc5Ram {
                    ram: vec![0x66, 0x77],
                },
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 151,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: true,
                    has_rtc: false,
                    profile: CartridgePersistenceProfile::PersistentRamAndFlash {
                        ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
                        flash_byte_len: 4,
                        hidden_byte_len: 3,
                    },
                },
                persistent_state: PersistentCartState::Mbc6 {
                    ram: vec![0x12, 0x34],
                    flash: vec![0xFF, 0xFE, 0xFC, 0xF8],
                    hidden_region: vec![0xAA, 0xBB, 0xCC],
                    sector0_protected: true,
                },
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 16,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: true,
                    has_rtc: false,
                    profile: CartridgePersistenceProfile::PersistentRam {
                        ram: CartridgeRamPayloadKind::Linear { byte_len: 4 },
                    },
                },
                persistent_state: PersistentCartState::Mmm01Ram {
                    ram: vec![0x88, 0x99, 0xAA, 0xBB],
                },
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 17,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: true,
                    has_rtc: false,
                    profile: CartridgePersistenceProfile::PersistentRam {
                        ram: CartridgeRamPayloadKind::Linear { byte_len: 3 },
                    },
                },
                persistent_state: PersistentCartState::Huc1Ram {
                    ram: vec![0xCC, 0xDD, 0xEE],
                },
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 18,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: true,
                    has_rtc: true,
                    profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                        ram: CartridgeRamPayloadKind::Linear { byte_len: 5 },
                    },
                },
                persistent_state: PersistentCartState::Huc3 {
                    ram: vec![0x01, 0x23, 0x45, 0x67, 0x89],
                    mcu_ram: huc3_mcu_ram,
                    rtc: Huc3RtcPersistentState {
                        current_minutes_of_day: 1439,
                        current_days: 0x0FFF,
                        current_subminute_seconds: 59,
                        event_minutes_of_day: 123,
                        event_days: 0x0123,
                    },
                    rom_bank: 0x3F,
                    ram_bank: 0x02,
                    select_mode: 0x0E,
                    access_address: 0xA5,
                    mailbox_command: 0x06,
                    mailbox_argument: 0x02,
                    last_response_nybble: 0x01,
                    semaphore_ready: true,
                    ir_emitter_on: true,
                    ir_light_detected: false,
                    last_control_write: Some(0x77),
                    last_unsupported_command: Some(0x06),
                    last_unsupported_argument: Some(0x0E),
                },
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 19,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: true,
                    has_rtc: false,
                    profile: CartridgePersistenceProfile::PersistentRam {
                        ram: CartridgeRamPayloadKind::Linear { byte_len: 4 },
                    },
                },
                persistent_state: PersistentCartState::PocketCameraRam {
                    ram: vec![0x88, 0x99, 0xAA, 0xBB],
                },
            },
            CartridgeSaveEnvelope {
                backend_metadata: CartridgeSaveBackendMetadata {
                    format_version: CURRENT_SAVE_FORMAT_VERSION,
                    saved_at_unix_seconds: 20,
                },
                cartridge_metadata: CartridgePersistenceMetadata {
                    has_battery: false,
                    has_rtc: false,
                    profile: CartridgePersistenceProfile::PersistentEeprom { byte_len: 4 },
                },
                persistent_state: PersistentCartState::Mbc7Eeprom {
                    eeprom: vec![0x12, 0x34, 0xAB, 0xCD],
                },
            },
        ];

        for envelope in profiles_and_states {
            assert_round_trip(envelope);
        }

        let mut rtc_only_state = PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 59,
                minutes: 59,
                hours: 23,
                day_counter: 511,
                halt: false,
                carry: false,
            },
        };
        apply_elapsed_off_session_seconds(&mut rtc_only_state, 2);
        assert_eq!(
            rtc_only_state,
            PersistentCartState::Mbc3Rtc {
                rtc: Mbc3RtcPersistentState {
                    seconds: 1,
                    minutes: 0,
                    hours: 0,
                    day_counter: 0,
                    halt: false,
                    carry: true,
                },
            }
        );

        let mut huc3_state = PersistentCartState::Huc3 {
            ram: vec![],
            mcu_ram: [0; 256],
            rtc: Huc3RtcPersistentState {
                current_minutes_of_day: 1439,
                current_days: 0x0FFF,
                current_subminute_seconds: 59,
                event_minutes_of_day: 3,
                event_days: 0,
            },
            rom_bank: 0,
            ram_bank: 0,
            select_mode: 0x0D,
            access_address: 0,
            mailbox_command: 0,
            mailbox_argument: 0,
            last_response_nybble: 0,
            semaphore_ready: true,
            ir_emitter_on: false,
            ir_light_detected: false,
            last_control_write: None,
            last_unsupported_command: None,
            last_unsupported_argument: None,
        };
        apply_elapsed_off_session_seconds(&mut huc3_state, 2);
        assert_eq!(
            huc3_state,
            PersistentCartState::Huc3 {
                ram: vec![],
                mcu_ram: [0; 256],
                rtc: Huc3RtcPersistentState {
                    current_minutes_of_day: 0,
                    current_days: 0,
                    current_subminute_seconds: 1,
                    event_minutes_of_day: 3,
                    event_days: 0,
                },
                rom_bank: 0,
                ram_bank: 0,
                select_mode: 0x0D,
                access_address: 0,
                mailbox_command: 0,
                mailbox_argument: 0,
                last_response_nybble: 0,
                semaphore_ready: true,
                ir_emitter_on: false,
                ir_light_detected: false,
                last_control_write: None,
                last_unsupported_command: None,
                last_unsupported_argument: None,
            }
        );
    }

    #[test]
    fn huc3_and_mbc2_error_paths_are_reported_explicitly() {
        let mbc2_error = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 21,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 1 },
                },
            },
            persistent_state: PersistentCartState::Mbc2Ram {
                ram_nibbles: {
                    let mut ram_nibbles = [0; MBC2_RAM_NIBBLE_COUNT];
                    ram_nibbles[0] = 0x10;
                    ram_nibbles
                },
            },
        })
        .expect_err("invalid MBC2 nibbles should fail to encode");
        assert_eq!(
            mbc2_error.to_string(),
            "invalid MBC2 nibble value 0x10 at logical cell 0"
        );

        let mut invalid_huc3_mcu_ram = [0; 256];
        invalid_huc3_mcu_ram[7] = 0x10;
        let huc3_error = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 22,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: true,
                profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 0 },
                },
            },
            persistent_state: PersistentCartState::Huc3 {
                ram: vec![],
                mcu_ram: invalid_huc3_mcu_ram,
                rtc: Huc3RtcPersistentState {
                    current_minutes_of_day: 0,
                    current_days: 0,
                    current_subminute_seconds: 0,
                    event_minutes_of_day: 0,
                    event_days: 0,
                },
                rom_bank: 0,
                ram_bank: 0,
                select_mode: 0x0D,
                access_address: 0,
                mailbox_command: 0,
                mailbox_argument: 0,
                last_response_nybble: 0,
                semaphore_ready: true,
                ir_emitter_on: false,
                ir_light_detected: false,
                last_control_write: None,
                last_unsupported_command: None,
                last_unsupported_argument: None,
            },
        })
        .expect_err("invalid HuC-3 nibbles should fail to encode");
        assert!(matches!(
            huc3_error,
            CartridgeSaveBackendError::InvalidHuc3NibbleValue {
                index: 7,
                value: 0x10,
            }
        ));
        assert_eq!(
            huc3_error.to_string(),
            "invalid HuC-3 nibble value 0x10 at logical cell 7"
        );
    }

    #[test]
    fn huc3_decode_rejects_invalid_mcu_lengths_and_nibbles() {
        let rtc = Huc3RtcPersistentState {
            current_minutes_of_day: 1,
            current_days: 2,
            current_subminute_seconds: 3,
            event_minutes_of_day: 4,
            event_days: 5,
        };

        let mut bad_len_bytes = Vec::new();
        bad_len_bytes.push(STATE_HUC3_TAG);
        encode_linear_ram(&mut bad_len_bytes, &[0xAA], "HuC-3 RAM").expect("RAM should encode");
        write_u32_checked(
            &mut bad_len_bytes,
            255,
            "decoded HuC-3 MCU RAM nibble count",
        )
        .expect("length should encode");
        bad_len_bytes.extend(std::iter::repeat_n(0x00, 255));
        encode_huc3_rtc(&mut bad_len_bytes, rtc);
        bad_len_bytes.extend_from_slice(&[0x3F, 0x02, 0x0D, 0xA5, 0x06, 0x02, 0x01]);
        write_bool(&mut bad_len_bytes, true);
        write_bool(&mut bad_len_bytes, false);
        write_bool(&mut bad_len_bytes, true);
        encode_optional_u8(&mut bad_len_bytes, Some(0x77));
        encode_optional_u8(&mut bad_len_bytes, Some(0x06));
        encode_optional_u8(&mut bad_len_bytes, Some(0x0E));

        let mut bad_len_cursor = ByteCursor::new(&bad_len_bytes);
        let bad_len_error =
            decode_persistent_state(&mut bad_len_cursor).expect_err("invalid nibble count");
        assert!(matches!(
            bad_len_error,
            CartridgeSaveBackendError::LengthOverflow {
                field: "decoded HuC-3 MCU RAM nibble count",
                value: 255,
            }
        ));

        let mut bad_nibble_bytes = Vec::new();
        bad_nibble_bytes.push(STATE_HUC3_TAG);
        encode_linear_ram(&mut bad_nibble_bytes, &[0xBB], "HuC-3 RAM").expect("RAM should encode");
        write_u32_checked(
            &mut bad_nibble_bytes,
            256,
            "decoded HuC-3 MCU RAM nibble count",
        )
        .expect("length should encode");
        let mut nibble_bytes = [0u8; 256];
        nibble_bytes[9] = 0x10;
        bad_nibble_bytes.extend_from_slice(&nibble_bytes);
        encode_huc3_rtc(&mut bad_nibble_bytes, rtc);
        bad_nibble_bytes.extend_from_slice(&[0x3F, 0x02, 0x0D, 0xA5, 0x06, 0x02, 0x01]);
        write_bool(&mut bad_nibble_bytes, true);
        write_bool(&mut bad_nibble_bytes, false);
        write_bool(&mut bad_nibble_bytes, true);
        encode_optional_u8(&mut bad_nibble_bytes, Some(0x77));
        encode_optional_u8(&mut bad_nibble_bytes, Some(0x06));
        encode_optional_u8(&mut bad_nibble_bytes, Some(0x0E));

        let mut bad_nibble_cursor = ByteCursor::new(&bad_nibble_bytes);
        let bad_nibble_error =
            decode_persistent_state(&mut bad_nibble_cursor).expect_err("invalid HuC-3 nibble");
        assert!(matches!(
            bad_nibble_error,
            CartridgeSaveBackendError::InvalidHuc3NibbleValue {
                index: 9,
                value: 0x10,
            }
        ));
    }

    #[test]
    fn default_backends_time_sources_and_battery_policy_are_explicit() {
        let _ = SystemCartridgeSaveTimeSource.now_unix_seconds();

        let empty_backend = InMemoryCartridgeSaveBackend::new();
        assert!(empty_backend.is_empty());

        let default_backend = InMemoryCartridgeSaveBackend::default();
        assert_eq!(default_backend.len(), 0);

        assert_eq!(FixedCartridgeSaveTimeSource::new(42).now_unix_seconds(), 42);

        assert!(uses_battery_backed_hardware_persistence(
            CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: true,
                profile: CartridgePersistenceProfile::PersistentRtc,
            }
        ));
        assert!(uses_battery_backed_hardware_persistence(
            CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 8 },
                },
            }
        ));
        assert!(!uses_battery_backed_hardware_persistence(
            CartridgePersistenceMetadata {
                has_battery: false,
                has_rtc: true,
                profile: CartridgePersistenceProfile::PersistentRtc,
            }
        ));
        assert!(!uses_battery_backed_hardware_persistence(
            CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::None,
            }
        ));
        assert!(uses_battery_backed_hardware_persistence(
            CartridgePersistenceMetadata {
                has_battery: false,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentEeprom { byte_len: 256 },
            }
        ));
    }

    #[test]
    fn display_and_error_sources_are_human_readable() {
        assert_eq!(
            CartridgeSaveKeyError::Empty.to_string(),
            "save key must not be empty"
        );
        let exact_rom_stem_key =
            CartridgeSaveKey::new("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)")
                .expect("ordinary ROM filename punctuation should be valid");
        assert_eq!(
            exact_rom_stem_key.as_str(),
            "Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)"
        );
        assert_eq!(
            FilesystemCartridgeSaveBackend::new("saves").path_for_key(&exact_rom_stem_key),
            PathBuf::from(
                "saves/Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gbsav"
            )
        );
        let slot_extensions = [
            (CartridgeSaveFileExtension::P1, SAVE_FILE_EXTENSION),
            (CartridgeSaveFileExtension::P2, SAVE_FILE_EXTENSION_P2),
            (CartridgeSaveFileExtension::P3, SAVE_FILE_EXTENSION_P3),
            (CartridgeSaveFileExtension::P4, SAVE_FILE_EXTENSION_P4),
        ];
        for (file_extension, expected_suffix) in slot_extensions {
            let backend =
                FilesystemCartridgeSaveBackend::with_file_extension("saves", file_extension);
            assert_eq!(backend.file_extension(), file_extension);
            assert_eq!(
                backend.path_for_key(&exact_rom_stem_key),
                PathBuf::from(format!(
                    "saves/Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).{expected_suffix}"
                ))
            );
        }
        assert_eq!(
            CartridgeSaveKeyError::InvalidCharacter {
                index: 3,
                character: '/',
            }
            .to_string(),
            "save key contains invalid character `/` at index 3"
        );

        let io_error = CartridgeSaveBackendError::Io {
            operation: "read save file",
            path: PathBuf::from("slot.gbsav"),
            source: io::Error::other("disk error"),
        };
        assert_eq!(io_error.to_string(), "read save file failed for slot.gbsav");
        assert!(std::error::Error::source(&io_error).is_some());

        assert_eq!(
            ExternalSaveError::UnsupportedPersistentState { state_kind: "Huc3" }.to_string(),
            "external .sav conversion does not support Huc3"
        );
        assert_eq!(
            ExternalSaveError::UnsupportedPersistenceProfile {
                profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                    ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 512 },
                },
            }
            .to_string(),
            "external .sav conversion does not support persistence profile PersistentRamAndRtc { ram: Mbc2Nibbles { cell_count: 512 } }"
        );
        assert_eq!(
            ExternalSaveError::StateProfileMismatch {
                state_kind: "Mbc2Ram",
                profile: CartridgePersistenceProfile::PersistentRtc,
            }
            .to_string(),
            "persistent state Mbc2Ram does not match cartridge persistence profile PersistentRtc"
        );
        assert_eq!(
            ExternalSaveError::InvalidLength {
                context: "linear RAM",
                expected: ExternalSaveLengthExpectation::Exact(8),
                actual: 4,
            }
            .to_string(),
            "invalid external .sav length for linear RAM: expected 8 bytes, got 4"
        );
        assert_eq!(
            ExternalSaveError::InvalidLength {
                context: "MBC2 RAM",
                expected: ExternalSaveLengthExpectation::Either {
                    first: 256,
                    second: 512,
                },
                actual: 257,
            }
            .to_string(),
            "invalid external .sav length for MBC2 RAM: expected 256 or 512 bytes, got 257"
        );

        let other_backend_errors = [
            (
                CartridgeSaveBackendError::InvalidMagic {
                    actual: *b"BADSAVE!",
                },
                "invalid save magic",
            ),
            (
                CartridgeSaveBackendError::UnexpectedEof {
                    offset: 3,
                    needed: 4,
                    remaining: 1,
                },
                "unexpected end of save payload at offset 3: needed 4 bytes but only 1 remain",
            ),
            (
                CartridgeSaveBackendError::UnsupportedFormatVersion { version: 7 },
                "unsupported save format version 7",
            ),
            (
                CartridgeSaveBackendError::UnsupportedRamPayloadKindTag { tag: 0xAA },
                "unsupported RAM payload kind tag 0xAA",
            ),
            (
                CartridgeSaveBackendError::UnsupportedPersistenceProfileTag { tag: 0xBB },
                "unsupported persistence profile tag 0xBB",
            ),
            (
                CartridgeSaveBackendError::UnsupportedPersistentStateTag { tag: 0xCC },
                "unsupported persistent state tag 0xCC",
            ),
            (
                CartridgeSaveBackendError::UnsupportedMachineSaveStateTag {
                    field: "console_model",
                    tag: 0xDD,
                },
                "unsupported machine save-state tag for console_model: 0xDD",
            ),
            (
                CartridgeSaveBackendError::InvalidBooleanTag {
                    field: "rtc.halt",
                    value: 2,
                },
                "invalid boolean tag for rtc.halt: 0x02",
            ),
            (
                CartridgeSaveBackendError::LengthOverflow {
                    field: "MBC2 RAM cell_count",
                    value: 1usize << 40,
                },
                "MBC2 RAM cell_count length 1099511627776 exceeds format capacity",
            ),
            (
                CartridgeSaveBackendError::InvalidMbc2NibbleValue {
                    index: 7,
                    value: 0x1F,
                },
                "invalid MBC2 nibble value 0x1F at logical cell 7",
            ),
            (
                CartridgeSaveBackendError::MachineSaveStateCodec {
                    operation: "decode",
                    message: "bad payload".to_string(),
                },
                "machine save-state decode failed: bad payload",
            ),
            (
                CartridgeSaveBackendError::MachineSaveStateMetadataMismatch,
                "machine save-state envelope metadata does not match payload metadata",
            ),
            (
                CartridgeSaveBackendError::TrailingBytes { remaining: 9 },
                "save payload has 9 trailing bytes",
            ),
        ];

        for (error, expected) in other_backend_errors {
            assert!(error.to_string().contains(expected));
            assert!(std::error::Error::source(&error).is_none());
        }

        let backend_error = HardwarePersistenceError::Backend(CartridgeSaveBackendError::Io {
            operation: "delete save file",
            path: PathBuf::from("slot.gbsav"),
            source: io::Error::other("permission denied"),
        });
        assert_eq!(
            backend_error.to_string(),
            "delete save file failed for slot.gbsav"
        );
        assert!(std::error::Error::source(&backend_error).is_some());

        let restore_error =
            HardwarePersistenceError::Restore(CartridgePersistentStateError::KindMismatch {
                expected: "MBC1 RAM",
                actual: "MBC2 RAM",
            });
        assert_eq!(
            restore_error.to_string(),
            "cartridge restore failed: KindMismatch { expected: \"MBC1 RAM\", actual: \"MBC2 RAM\" }"
        );
        assert!(std::error::Error::source(&restore_error).is_none());
    }

    #[test]
    fn decode_rejects_invalid_magic_version_and_truncated_payloads() {
        let mut bytes = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 123,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 1 },
                },
            },
            persistent_state: PersistentCartState::Mbc1Ram { ram: vec![0xAA] },
        })
        .expect("encode should succeed");

        bytes[0] ^= 0xFF;
        assert!(matches!(
            decode_cartridge_save_envelope(&bytes),
            Err(CartridgeSaveBackendError::InvalidMagic { .. })
        ));

        let original_bytes = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 123,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 1 },
                },
            },
            persistent_state: PersistentCartState::Mbc1Ram { ram: vec![0xAA] },
        })
        .expect("encode should succeed");
        let mut version_bytes = original_bytes.clone();
        version_bytes[8..10].copy_from_slice(&(CURRENT_SAVE_FORMAT_VERSION + 1).to_le_bytes());
        assert!(matches!(
            decode_cartridge_save_envelope(&version_bytes),
            Err(CartridgeSaveBackendError::UnsupportedFormatVersion { .. })
        ));

        let truncated = &original_bytes[..original_bytes.len() - 1];
        assert!(matches!(
            decode_cartridge_save_envelope(truncated),
            Err(CartridgeSaveBackendError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn decode_rejects_invalid_mbc2_nibbles() {
        let mut bytes = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 123,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 512 },
                },
            },
            persistent_state: PersistentCartState::Mbc2Ram {
                ram_nibbles: [0; MBC2_RAM_NIBBLE_COUNT],
            },
        })
        .expect("encode should succeed");

        let nibble_offset = bytes.len() - MBC2_RAM_NIBBLE_COUNT;
        bytes[nibble_offset] = 0xFE;

        assert!(matches!(
            decode_cartridge_save_envelope(&bytes),
            Err(CartridgeSaveBackendError::InvalidMbc2NibbleValue {
                index: 0,
                value: 0xFE
            })
        ));
    }

    #[test]
    fn decode_rejects_invalid_boolean_tags_unsupported_tags_and_trailing_bytes() {
        let envelope = CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 123,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 1 },
                },
            },
            persistent_state: PersistentCartState::Mbc1Ram { ram: vec![0xAA] },
        };
        let encoded = encode_cartridge_save_envelope(&envelope).expect("encode should succeed");

        let mut invalid_has_battery = encoded.clone();
        invalid_has_battery[18] = 0x02;
        assert!(matches!(
            decode_cartridge_save_envelope(&invalid_has_battery),
            Err(CartridgeSaveBackendError::InvalidBooleanTag {
                field: "has_battery",
                value: 0x02
            })
        ));

        let mut invalid_profile_tag = encoded.clone();
        invalid_profile_tag[20] = 0xFF;
        assert!(matches!(
            decode_cartridge_save_envelope(&invalid_profile_tag),
            Err(CartridgeSaveBackendError::UnsupportedPersistenceProfileTag { tag: 0xFF })
        ));

        let mut invalid_ram_kind_tag = encoded.clone();
        invalid_ram_kind_tag[21] = 0xFE;
        assert!(matches!(
            decode_cartridge_save_envelope(&invalid_ram_kind_tag),
            Err(CartridgeSaveBackendError::UnsupportedRamPayloadKindTag { tag: 0xFE })
        ));

        let mut invalid_state_tag = encoded.clone();
        invalid_state_tag[26] = 0xFD;
        assert!(matches!(
            decode_cartridge_save_envelope(&invalid_state_tag),
            Err(CartridgeSaveBackendError::UnsupportedPersistentStateTag { tag: 0xFD })
        ));

        let mut trailing_bytes = encoded;
        trailing_bytes.push(0x99);
        assert!(matches!(
            decode_cartridge_save_envelope(&trailing_bytes),
            Err(CartridgeSaveBackendError::TrailingBytes { remaining: 1 })
        ));
    }

    #[test]
    fn encode_and_decode_reject_length_overflows_and_invalid_mbc2_lengths() {
        let overflow_profile = CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 456,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear {
                        byte_len: usize::MAX,
                    },
                },
            },
            persistent_state: PersistentCartState::None,
        };
        assert!(matches!(
            encode_cartridge_save_envelope(&overflow_profile),
            Err(CartridgeSaveBackendError::LengthOverflow {
                field: "linear RAM byte_len",
                value: usize::MAX
            })
        ));

        let mut mbc2_bytes = encode_cartridge_save_envelope(&CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 789,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 512 },
                },
            },
            persistent_state: PersistentCartState::Mbc2Ram {
                ram_nibbles: [0; MBC2_RAM_NIBBLE_COUNT],
            },
        })
        .expect("encode should succeed");

        let nibble_count_offset = 27;
        mbc2_bytes[nibble_count_offset..nibble_count_offset + 4]
            .copy_from_slice(&(511u32).to_le_bytes());
        assert!(matches!(
            decode_cartridge_save_envelope(&mbc2_bytes),
            Err(CartridgeSaveBackendError::LengthOverflow {
                field: "decoded MBC2 RAM nibble count",
                value: 511
            })
        ));
    }

    #[test]
    fn external_save_exports_linear_ram_as_raw_bytes() {
        let metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 4 },
            },
        };
        let state = PersistentCartState::Mbc1Ram {
            ram: vec![0x10, 0x20, 0x30, 0x40],
        };

        let external = encode_external_cartridge_save(
            metadata,
            &state,
            1_700_000_000,
            ExternalSaveExportFormat::default(),
        )
        .expect("linear RAM should export");
        assert_eq!(external, [0x10, 0x20, 0x30, 0x40]);

        let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_001)
            .expect("linear RAM should import");
        assert_eq!(imported, state);
    }

    #[test]
    fn external_save_round_trips_all_linear_ram_state_kinds() {
        let metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
            },
        };
        let states = [
            PersistentCartState::NoMbcRam {
                ram: vec![0x01, 0x02],
            },
            PersistentCartState::Mmm01Ram {
                ram: vec![0x03, 0x04],
            },
            PersistentCartState::Huc1Ram {
                ram: vec![0x05, 0x06],
            },
            PersistentCartState::Mbc3Ram {
                ram: vec![0x07, 0x08],
            },
            PersistentCartState::Mbc5Ram {
                ram: vec![0x09, 0x0A],
            },
            PersistentCartState::PocketCameraRam {
                ram: vec![0x0B, 0x0C],
            },
        ];

        for state in states {
            let external = encode_external_cartridge_save(
                metadata,
                &state,
                1_700_000_000,
                ExternalSaveExportFormat::default(),
            )
            .expect("linear RAM state should export");
            let imported =
                import_external_cartridge_save(metadata, &state, &external, 1_700_000_001)
                    .expect("linear RAM state should import");
            assert_eq!(imported, state);
        }
    }

    #[test]
    fn external_save_round_trips_mbc6_sram_plus_main_flash_when_hidden_state_is_default() {
        let metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
                flash_byte_len: 4,
                hidden_byte_len: 3,
            },
        };
        let state = PersistentCartState::Mbc6 {
            ram: vec![0x10, 0x20],
            flash: vec![0xFF, 0x7F, 0x3F, 0x1F],
            hidden_region: vec![0xFF; 3],
            sector0_protected: false,
        };

        let external = encode_external_cartridge_save(
            metadata,
            &state,
            1_700_000_000,
            ExternalSaveExportFormat::default(),
        )
        .expect("default MBC6 hidden state should export");
        assert_eq!(external, [0x10, 0x20, 0xFF, 0x7F, 0x3F, 0x1F]);

        let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_001)
            .expect("default MBC6 hidden state should import");
        assert_eq!(imported, state);
    }

    #[test]
    fn external_save_exports_mbc2_in_mgba_packed_form_and_imports_sameboy_form() {
        let metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                    cell_count: MBC2_RAM_NIBBLE_COUNT,
                },
            },
        };
        let mut ram_nibbles = [0; MBC2_RAM_NIBBLE_COUNT];
        ram_nibbles[0] = 0x01;
        ram_nibbles[1] = 0x02;
        ram_nibbles[2] = 0x0A;
        ram_nibbles[3] = 0x0B;
        ram_nibbles[511] = 0x0F;
        let state = PersistentCartState::Mbc2Ram { ram_nibbles };

        let external = encode_external_cartridge_save(
            metadata,
            &state,
            1_700_000_000,
            ExternalSaveExportFormat::default(),
        )
        .expect("MBC2 should export");
        assert_eq!(external.len(), MBC2_MGBA_PACKED_BYTE_COUNT);
        assert_eq!(external[0], 0x21);
        assert_eq!(external[1], 0xBA);
        assert_eq!(external[255], 0xF0);
        assert_eq!(
            import_external_cartridge_save(metadata, &state, &external, 1_700_000_000)
                .expect("mGBA packed MBC2 should import"),
            state
        );

        let mut sameboy = vec![0; MBC2_RAM_NIBBLE_COUNT];
        sameboy[0] = 0xF1;
        sameboy[1] = 0xE2;
        sameboy[2] = 0xCA;
        sameboy[3] = 0xBB;
        sameboy[511] = 0xFF;
        let imported = import_external_cartridge_save(metadata, &state, &sameboy, 1_700_000_000)
            .expect("SameBoy one-byte-per-nibble MBC2 should import");
        let PersistentCartState::Mbc2Ram { ram_nibbles } = imported else {
            panic!("expected MBC2 state");
        };
        assert_eq!(ram_nibbles[0], 0x01);
        assert_eq!(ram_nibbles[1], 0x02);
        assert_eq!(ram_nibbles[2], 0x0A);
        assert_eq!(ram_nibbles[3], 0x0B);
        assert_eq!(ram_nibbles[511], 0x0F);
    }

    #[test]
    fn external_save_round_trips_mbc3_rtc_only_suffix() {
        let metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        };
        let state = PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 1,
                minutes: 2,
                hours: 3,
                day_counter: 4,
                halt: false,
                carry: false,
            },
        };
        let external = encode_external_cartridge_save(
            metadata,
            &state,
            1_700_000_010,
            ExternalSaveExportFormat::default(),
        )
        .expect("MBC3 RTC-only state should export");
        assert_eq!(external.len(), MBC3_EXTERNAL_RTC_SUFFIX_LEN);

        let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_012)
            .expect("MBC3 RTC-only state should import");
        assert_eq!(
            imported,
            PersistentCartState::Mbc3Rtc {
                rtc: Mbc3RtcPersistentState {
                    seconds: 3,
                    minutes: 2,
                    hours: 3,
                    day_counter: 4,
                    halt: false,
                    carry: false,
                },
            }
        );
    }

    #[test]
    fn external_save_round_trips_mbc3_rtc_suffix_with_elapsed_time() {
        let metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
            },
        };
        let state = PersistentCartState::Mbc3RamRtc {
            ram: vec![0xAB, 0xCD],
            rtc: Mbc3RtcPersistentState {
                seconds: 58,
                minutes: 59,
                hours: 23,
                day_counter: 7,
                halt: false,
                carry: false,
            },
        };

        let envelope = CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 100,
            },
            cartridge_metadata: metadata,
            persistent_state: state.clone(),
        };
        let external =
            export_external_cartridge_save(&envelope, 103).expect("MBC3 RAM+RTC should export");
        assert_eq!(external.len(), 2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN);
        assert_eq!(&external[..2], &[0xAB, 0xCD]);
        assert_eq!(external[2], 1);
        assert_eq!(external[6], 0);
        assert_eq!(external[10], 0);
        assert_eq!(
            u64::from_le_bytes(external[42..50].try_into().unwrap()),
            103
        );

        let imported = import_external_cartridge_save(metadata, &state, &external, 105)
            .expect("MBC3 RAM+RTC should import");
        assert_eq!(
            imported,
            PersistentCartState::Mbc3RamRtc {
                ram: vec![0xAB, 0xCD],
                rtc: Mbc3RtcPersistentState {
                    seconds: 3,
                    minutes: 0,
                    hours: 0,
                    day_counter: 8,
                    halt: false,
                    carry: false,
                },
            }
        );
    }

    #[test]
    fn external_save_imports_mbc3_rtc_suffixes_with_32_bit_timestamps() {
        let rtc_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        };
        let rtc_state = PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 1,
                minutes: 2,
                hours: 3,
                day_counter: 4,
                halt: false,
                carry: false,
            },
        };
        let mut rtc_external = encode_external_cartridge_save(
            rtc_metadata,
            &rtc_state,
            1_700_000_010,
            ExternalSaveExportFormat::default(),
        )
        .expect("MBC3 RTC-only state should export");
        assert_eq!(rtc_external.len(), MBC3_EXTERNAL_RTC_SUFFIX_LEN);
        rtc_external.truncate(MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP);

        let rtc_imported =
            import_external_cartridge_save(rtc_metadata, &rtc_state, &rtc_external, 1_700_000_012)
                .expect("MBC3 RTC-only 32-bit timestamp suffix should import");
        assert_eq!(
            rtc_imported,
            PersistentCartState::Mbc3Rtc {
                rtc: Mbc3RtcPersistentState {
                    seconds: 3,
                    minutes: 2,
                    hours: 3,
                    day_counter: 4,
                    halt: false,
                    carry: false,
                },
            }
        );

        let ram_rtc_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
            },
        };
        let ram_rtc_state = PersistentCartState::Mbc3RamRtc {
            ram: vec![0xAB, 0xCD],
            rtc: Mbc3RtcPersistentState {
                seconds: 58,
                minutes: 59,
                hours: 23,
                day_counter: 7,
                halt: false,
                carry: false,
            },
        };
        let envelope = CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 100,
            },
            cartridge_metadata: ram_rtc_metadata,
            persistent_state: ram_rtc_state.clone(),
        };
        let mut ram_rtc_external =
            export_external_cartridge_save(&envelope, 103).expect("MBC3 RAM+RTC should export");
        assert_eq!(ram_rtc_external.len(), 2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN);
        ram_rtc_external.truncate(2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP);

        let ram_rtc_imported = import_external_cartridge_save(
            ram_rtc_metadata,
            &ram_rtc_state,
            &ram_rtc_external,
            105,
        )
        .expect("MBC3 RAM+RTC 32-bit timestamp suffix should import");
        assert_eq!(
            ram_rtc_imported,
            PersistentCartState::Mbc3RamRtc {
                ram: vec![0xAB, 0xCD],
                rtc: Mbc3RtcPersistentState {
                    seconds: 3,
                    minutes: 0,
                    hours: 0,
                    day_counter: 8,
                    halt: false,
                    carry: false,
                },
            }
        );
    }

    #[test]
    fn external_save_round_trips_mbc30_sized_ram_plus_mbc3_rtc_suffix() {
        let metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 64 * 1024,
                },
            },
        };
        let mut ram = vec![0; 64 * 1024];
        ram[0] = 0x30;
        ram[0x3FFF] = 0x3F;
        ram[0x8000] = 0x80;
        ram[0xFFFF] = 0xFF;
        let state = PersistentCartState::Mbc3RamRtc {
            ram: ram.clone(),
            rtc: Mbc3RtcPersistentState {
                seconds: 7,
                minutes: 8,
                hours: 9,
                day_counter: 10,
                halt: false,
                carry: false,
            },
        };

        let external = encode_external_cartridge_save(
            metadata,
            &state,
            1_700_000_000,
            ExternalSaveExportFormat::default(),
        )
        .expect("MBC30-sized MBC3 RAM+RTC should export");
        assert_eq!(external.len(), 64 * 1024 + MBC3_EXTERNAL_RTC_SUFFIX_LEN);

        let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_000)
            .expect("MBC30-sized MBC3 RAM+RTC should import");
        assert_eq!(imported, state);
    }

    #[test]
    fn external_save_round_trips_mbc7_raw_eeprom_without_battery_flag() {
        let metadata = CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentEeprom { byte_len: 256 },
        };
        let mut eeprom = vec![0xFF; 256];
        eeprom[0] = 0x12;
        eeprom[1] = 0x34;
        eeprom[254] = 0xAB;
        eeprom[255] = 0xCD;
        let state = PersistentCartState::Mbc7Eeprom {
            eeprom: eeprom.clone(),
        };

        let external = encode_external_cartridge_save(
            metadata,
            &state,
            1_700_000_000,
            ExternalSaveExportFormat::default(),
        )
        .expect("MBC7 EEPROM should export as a raw .sav payload");
        assert_eq!(external, eeprom);

        let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_000)
            .expect("MBC7 EEPROM should import from raw .sav bytes");
        assert_eq!(imported, state);
    }

    #[test]
    fn external_save_rejects_ambiguous_or_invalid_payloads() {
        let linear_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
            },
        };
        let linear_state = PersistentCartState::NoMbcRam { ram: vec![0xAA] };
        assert!(matches!(
            encode_external_cartridge_save(
                linear_metadata,
                &linear_state,
                0,
                ExternalSaveExportFormat::default(),
            ),
            Err(ExternalSaveError::InvalidLength {
                context: "linear RAM state",
                ..
            })
        ));
        assert!(matches!(
            import_external_cartridge_save(
                linear_metadata,
                &PersistentCartState::NoMbcRam { ram: vec![0; 2] },
                &[0xAA],
                0,
            ),
            Err(ExternalSaveError::InvalidLength {
                context: "linear RAM",
                ..
            })
        ));

        let mbc2_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                    cell_count: MBC2_RAM_NIBBLE_COUNT,
                },
            },
        };
        let mbc2_state = PersistentCartState::Mbc2Ram {
            ram_nibbles: [0; MBC2_RAM_NIBBLE_COUNT],
        };
        assert!(matches!(
            import_external_cartridge_save(mbc2_metadata, &mbc2_state, &[0; 257], 0),
            Err(ExternalSaveError::InvalidLength {
                context: "MBC2 RAM",
                ..
            })
        ));
        let invalid_mbc2_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 511 },
            },
        };
        assert!(matches!(
            encode_external_cartridge_save(
                invalid_mbc2_metadata,
                &mbc2_state,
                0,
                ExternalSaveExportFormat::default(),
            ),
            Err(ExternalSaveError::InvalidLength {
                context: "MBC2 metadata",
                ..
            })
        ));
        assert!(matches!(
            import_external_cartridge_save(invalid_mbc2_metadata, &mbc2_state, &[0; 256], 0),
            Err(ExternalSaveError::InvalidLength {
                context: "MBC2 metadata",
                ..
            })
        ));

        let mbc3_rtc_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        };
        let mbc3_rtc_state = PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 0,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        };
        assert!(matches!(
            import_external_cartridge_save(
                mbc3_rtc_metadata,
                &mbc3_rtc_state,
                &[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN - 1],
                0,
            ),
            Err(ExternalSaveError::InvalidLength {
                context: "MBC3 RTC",
                expected: ExternalSaveLengthExpectation::Either {
                    first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
                    second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
                },
                actual,
            }) if actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN - 1
        ));
        assert!(matches!(
            import_external_cartridge_save(
                mbc3_rtc_metadata,
                &mbc3_rtc_state,
                &[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP - 1],
                0,
            ),
            Err(ExternalSaveError::InvalidLength {
                context: "MBC3 RTC",
                expected: ExternalSaveLengthExpectation::Either {
                    first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
                    second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
                },
                actual,
            }) if actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP - 1
        ));
        assert!(matches!(
            decode_external_mbc3_rtc_suffix(
                &[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP - 1],
                0
            ),
            Err(ExternalSaveError::InvalidLength {
                context: "MBC3 RTC",
                expected: ExternalSaveLengthExpectation::Either {
                    first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
                    second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
                },
                actual,
            }) if actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP - 1
        ));
        assert!(matches!(
            decode_external_mbc3_rtc_suffix(&[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN - 1], 0),
            Err(ExternalSaveError::InvalidLength {
                context: "MBC3 RTC",
                expected: ExternalSaveLengthExpectation::Either {
                    first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
                    second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
                },
                actual,
            }) if actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN - 1
        ));

        let ram_rtc_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
            },
        };
        let ram_rtc_state = PersistentCartState::Mbc3RamRtc {
            ram: vec![0; 2],
            rtc: Mbc3RtcPersistentState {
                seconds: 0,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        };
        assert!(matches!(
            import_external_cartridge_save(
                ram_rtc_metadata,
                &ram_rtc_state,
                &[0; MBC3_EXTERNAL_RTC_SUFFIX_LEN + 1],
                0,
            ),
            Err(ExternalSaveError::InvalidLength {
                context: "MBC3 RAM+RTC",
                expected: ExternalSaveLengthExpectation::Either {
                    first,
                    second,
                },
                actual,
            }) if first == 2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP
                && second == 2 + MBC3_EXTERNAL_RTC_SUFFIX_LEN
                && actual == MBC3_EXTERNAL_RTC_SUFFIX_LEN + 1
        ));

        let unsupported_profile = CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 1 },
            },
        };
        assert!(matches!(
            encode_external_cartridge_save(
                unsupported_profile,
                &PersistentCartState::NoMbcRam { ram: vec![0] },
                0,
                ExternalSaveExportFormat::default(),
            ),
            Err(ExternalSaveError::UnsupportedPersistenceProfile { .. })
        ));
        assert!(matches!(
            import_external_cartridge_save(
                unsupported_profile,
                &PersistentCartState::NoMbcRam { ram: vec![0] },
                &[0],
                0,
            ),
            Err(ExternalSaveError::UnsupportedPersistenceProfile { .. })
        ));

        let unsupported_mbc2_rtc_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                    cell_count: MBC2_RAM_NIBBLE_COUNT,
                },
            },
        };
        assert!(matches!(
            encode_external_cartridge_save(
                unsupported_mbc2_rtc_metadata,
                &ram_rtc_state,
                0,
                ExternalSaveExportFormat::default(),
            ),
            Err(ExternalSaveError::UnsupportedPersistenceProfile { .. })
        ));
        assert!(matches!(
            import_external_cartridge_save(
                unsupported_mbc2_rtc_metadata,
                &ram_rtc_state,
                &[0; 2],
                0,
            ),
            Err(ExternalSaveError::UnsupportedPersistenceProfile { .. })
        ));

        let huc3_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
            },
        };
        let huc3_state = PersistentCartState::Huc3 {
            ram: vec![0; 2],
            mcu_ram: [0; 256],
            rtc: Huc3RtcPersistentState {
                current_minutes_of_day: 0,
                current_days: 0,
                current_subminute_seconds: 0,
                event_minutes_of_day: 0,
                event_days: 0,
            },
            rom_bank: 0,
            ram_bank: 0,
            select_mode: 0,
            access_address: 0,
            mailbox_command: 0,
            mailbox_argument: 0,
            last_response_nybble: 0,
            semaphore_ready: true,
            ir_emitter_on: false,
            ir_light_detected: false,
            last_control_write: None,
            last_unsupported_command: None,
            last_unsupported_argument: None,
        };
        assert!(matches!(
            encode_external_cartridge_save(
                huc3_metadata,
                &huc3_state,
                0,
                ExternalSaveExportFormat::default(),
            ),
            Err(ExternalSaveError::UnsupportedPersistentState { state_kind: "Huc3" })
        ));
        assert!(matches!(
            import_external_cartridge_save(huc3_metadata, &huc3_state, &[0; 50], 0),
            Err(ExternalSaveError::UnsupportedPersistentState { state_kind: "Huc3" })
        ));
        assert!(matches!(
            encode_external_cartridge_save(
                linear_metadata,
                &huc3_state,
                0,
                ExternalSaveExportFormat::default(),
            ),
            Err(ExternalSaveError::UnsupportedPersistentState { state_kind: "Huc3" })
        ));
        assert!(matches!(
            encode_external_cartridge_save(
                linear_metadata,
                &mbc2_state,
                0,
                ExternalSaveExportFormat::default(),
            ),
            Err(ExternalSaveError::StateProfileMismatch {
                state_kind: "Mbc2Ram",
                ..
            })
        ));
        assert!(matches!(
            import_external_cartridge_save(linear_metadata, &mbc2_state, &[0; 2], 0),
            Err(ExternalSaveError::StateProfileMismatch {
                state_kind: "Mbc2Ram",
                ..
            })
        ));

        let mbc6_metadata = CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
                flash_byte_len: 4,
                hidden_byte_len: 2,
            },
        };
        let protected_mbc6_state = PersistentCartState::Mbc6 {
            ram: vec![0; 2],
            flash: vec![0xFF; 4],
            hidden_region: vec![0xFF; 2],
            sector0_protected: true,
        };
        assert!(matches!(
            encode_external_cartridge_save(
                mbc6_metadata,
                &protected_mbc6_state,
                0,
                ExternalSaveExportFormat::default(),
            ),
            Err(ExternalSaveError::UnsupportedStateShape {
                state_kind: "Mbc6",
                ..
            })
        ));
        assert!(matches!(
            import_external_cartridge_save(mbc6_metadata, &protected_mbc6_state, &[0; 6], 0),
            Err(ExternalSaveError::UnsupportedStateShape {
                state_kind: "Mbc6",
                ..
            })
        ));
        let mbc6_state = PersistentCartState::Mbc6 {
            ram: vec![0; 2],
            flash: vec![0xFF; 4],
            hidden_region: vec![0xFF; 2],
            sector0_protected: false,
        };
        assert!(matches!(
            import_external_cartridge_save(mbc6_metadata, &mbc6_state, &[0; 5], 0),
            Err(ExternalSaveError::InvalidLength {
                context: "MBC6 RAM+flash",
                ..
            })
        ));
    }

    #[test]
    fn persistent_state_kind_names_cover_all_public_variants() {
        let huc3_state = PersistentCartState::Huc3 {
            ram: vec![],
            mcu_ram: [0; 256],
            rtc: Huc3RtcPersistentState {
                current_minutes_of_day: 0,
                current_days: 0,
                current_subminute_seconds: 0,
                event_minutes_of_day: 0,
                event_days: 0,
            },
            rom_bank: 0,
            ram_bank: 0,
            select_mode: 0,
            access_address: 0,
            mailbox_command: 0,
            mailbox_argument: 0,
            last_response_nybble: 0,
            semaphore_ready: true,
            ir_emitter_on: false,
            ir_light_detected: false,
            last_control_write: None,
            last_unsupported_command: None,
            last_unsupported_argument: None,
        };
        let states = [
            (PersistentCartState::None, "None"),
            (PersistentCartState::NoMbcRam { ram: vec![] }, "NoMbcRam"),
            (PersistentCartState::Mmm01Ram { ram: vec![] }, "Mmm01Ram"),
            (PersistentCartState::Huc1Ram { ram: vec![] }, "Huc1Ram"),
            (huc3_state, "Huc3"),
            (PersistentCartState::Mbc1Ram { ram: vec![] }, "Mbc1Ram"),
            (
                PersistentCartState::Mbc2Ram {
                    ram_nibbles: [0; MBC2_RAM_NIBBLE_COUNT],
                },
                "Mbc2Ram",
            ),
            (
                PersistentCartState::Mbc3Rtc {
                    rtc: Mbc3RtcPersistentState {
                        seconds: 0,
                        minutes: 0,
                        hours: 0,
                        day_counter: 0,
                        halt: false,
                        carry: false,
                    },
                },
                "Mbc3Rtc",
            ),
            (PersistentCartState::Mbc3Ram { ram: vec![] }, "Mbc3Ram"),
            (
                PersistentCartState::Mbc3RamRtc {
                    ram: vec![],
                    rtc: Mbc3RtcPersistentState {
                        seconds: 0,
                        minutes: 0,
                        hours: 0,
                        day_counter: 0,
                        halt: false,
                        carry: false,
                    },
                },
                "Mbc3RamRtc",
            ),
            (PersistentCartState::Mbc5Ram { ram: vec![] }, "Mbc5Ram"),
            (
                PersistentCartState::Mbc6 {
                    ram: vec![],
                    flash: vec![],
                    hidden_region: vec![],
                    sector0_protected: false,
                },
                "Mbc6",
            ),
            (
                PersistentCartState::Mbc7Eeprom { eeprom: vec![] },
                "Mbc7Eeprom",
            ),
            (
                PersistentCartState::PocketCameraRam { ram: vec![] },
                "PocketCameraRam",
            ),
        ];

        for (state, expected_name) in states {
            assert_eq!(persistent_state_kind_name(&state), expected_name);
        }
    }
}
