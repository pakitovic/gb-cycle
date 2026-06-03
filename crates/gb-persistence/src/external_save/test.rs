pub(super) use super::mbc3::*;
pub(super) use super::*;
pub(super) use crate::cartridge_envelope::{CartridgeSaveBackendMetadata, CartridgeSaveEnvelope};
pub(super) use crate::format::{
    CURRENT_SAVE_FORMAT_VERSION, MBC2_MGBA_PACKED_BYTE_COUNT, MBC2_RAM_NIBBLE_COUNT,
    MBC3_EXTERNAL_RTC_SUFFIX_LEN, MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
};
pub(super) use gb_core::{
    CartridgePersistenceMetadata, CartridgePersistenceProfile, CartridgeRamPayloadKind,
    Huc3RtcPersistentState, Mbc3RtcPersistentState, PersistentCartState,
};

mod errors;
mod linear;
mod mbc2;
mod mbc3;
mod mbc6;
