mod codec;
mod envelope;

pub(crate) use codec::persistent_state_kind_name;
pub use envelope::{
    CartridgeSaveBackendMetadata, CartridgeSaveEnvelope, decode_cartridge_save_envelope,
    encode_cartridge_save_envelope,
};

#[cfg(test)]
mod test;
