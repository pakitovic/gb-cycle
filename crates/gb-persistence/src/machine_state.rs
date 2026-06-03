mod envelope;
mod metadata;

pub use envelope::{
    MachineSaveStateBackendMetadata, MachineSaveStateEnvelope, decode_machine_save_state_envelope,
    encode_machine_save_state_envelope,
};

#[cfg(test)]
mod test;
