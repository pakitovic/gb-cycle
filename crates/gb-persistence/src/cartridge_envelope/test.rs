pub(super) use super::codec::*;
pub(super) use super::*;
pub(super) use crate::backend::CartridgeSaveBackendError;
pub(super) use crate::format::{CURRENT_SAVE_FORMAT_VERSION, MBC2_RAM_NIBBLE_COUNT};
pub(super) use crate::hardware::apply_elapsed_off_session_seconds;
pub(super) use crate::wire::{ByteCursor, write_bool, write_u32_checked};
pub(super) use gb_core::{
    CartridgePersistenceMetadata, CartridgePersistenceProfile, CartridgeRamPayloadKind,
    Huc3RtcPersistentState, Mbc3RtcPersistentState, PersistentCartState,
};

pub(super) fn assert_round_trip(envelope: CartridgeSaveEnvelope) {
    let bytes = encode_cartridge_save_envelope(&envelope).expect("encode should succeed");
    let decoded = decode_cartridge_save_envelope(&bytes).expect("decode should succeed");
    assert_eq!(decoded, envelope);
}

mod codec;
mod errors;
mod kind;
