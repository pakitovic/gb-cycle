mod catalog;
pub(crate) mod framebuffer;
pub(crate) mod serial_contains;

#[cfg(test)]
mod test;

pub(crate) use catalog::{
    FramebufferObservation, Oracle, OracleConfig, OracleObservations, OracleOutcome, OracleStep,
};
