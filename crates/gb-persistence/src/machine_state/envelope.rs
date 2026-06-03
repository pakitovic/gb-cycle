use super::metadata::{decode_machine_save_state_metadata, encode_machine_save_state_metadata};
use crate::backend::CartridgeSaveBackendError;
use crate::format::{CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION, MACHINE_SAVE_STATE_MAGIC};
use crate::wire::{ByteCursor, write_u16, write_u32_checked};
use gb_core::{MachineSaveState, MachineSaveStateMetadata};

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
