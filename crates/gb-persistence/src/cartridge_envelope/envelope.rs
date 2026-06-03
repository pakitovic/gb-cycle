use super::codec::{
    decode_persistence_profile, decode_persistent_state, encode_persistence_profile,
    encode_persistent_state,
};
use crate::backend::CartridgeSaveBackendError;
use crate::format::{CURRENT_SAVE_FORMAT_VERSION, SAVE_MAGIC};
use crate::wire::{ByteCursor, write_bool, write_u16, write_u64};
use gb_core::{CartridgePersistenceMetadata, PersistentCartState};

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
