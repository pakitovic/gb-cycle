use gb_core::{
    CartridgePersistenceMetadata, CartridgePersistenceProfile, CartridgePersistentStateError,
    CartridgeRamPayloadKind, CartridgeSlot, Huc3RtcPersistentState, Mbc3RtcPersistentState,
    PersistentCartState,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SAVE_MAGIC: [u8; 8] = *b"GBCSAVE\0";
pub const CURRENT_SAVE_FORMAT_VERSION: u16 = 1;
pub const SAVE_FILE_EXTENSION: &str = "gbsav";
const MBC2_RAM_NIBBLE_COUNT: usize = 512;
const RAM_KIND_LINEAR_TAG: u8 = 0;
const RAM_KIND_MBC2_TAG: u8 = 1;
const PROFILE_NONE_TAG: u8 = 0;
const PROFILE_NON_PERSISTENT_RAM_TAG: u8 = 1;
const PROFILE_PERSISTENT_RAM_TAG: u8 = 2;
const PROFILE_PERSISTENT_RTC_TAG: u8 = 3;
const PROFILE_PERSISTENT_RAM_AND_RTC_TAG: u8 = 4;
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CartridgeSaveKey(String);

impl CartridgeSaveKey {
    pub fn new(key: impl Into<String>) -> Result<Self, CartridgeSaveKeyError> {
        let key = key.into();
        if key.is_empty() {
            return Err(CartridgeSaveKeyError::Empty);
        }

        for (index, character) in key.chars().enumerate() {
            let allowed = character.is_ascii_alphanumeric() || matches!(character, '_' | '-');
            if !allowed {
                return Err(CartridgeSaveKeyError::InvalidCharacter { index, character });
            }
        }

        Ok(Self(key))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
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
    metadata.has_battery
        && matches!(
            metadata.profile,
            CartridgePersistenceProfile::PersistentRam { .. }
                | CartridgePersistenceProfile::PersistentRtc
                | CartridgePersistenceProfile::PersistentRamAndRtc { .. }
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
}

impl FilesystemCartridgeSaveBackend<SystemCartridgeSaveTimeSource> {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_time_source(root, SystemCartridgeSaveTimeSource)
    }
}

impl<C> FilesystemCartridgeSaveBackend<C> {
    pub fn with_time_source(root: impl Into<PathBuf>, clock: C) -> Self {
        Self {
            root: root.into(),
            clock,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path_for_key(&self, key: &CartridgeSaveKey) -> PathBuf {
        self.root
            .join(format!("{}.{}", key.as_str(), SAVE_FILE_EXTENSION))
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
    }

    #[test]
    fn display_and_error_sources_are_human_readable() {
        assert_eq!(
            CartridgeSaveKeyError::Empty.to_string(),
            "save key must not be empty"
        );
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
}
