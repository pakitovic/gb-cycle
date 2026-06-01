mod catalog;
pub(crate) mod fibonacci_result;
pub(crate) mod framebuffer;
pub(crate) mod serial_contains;

#[cfg(test)]
mod test;

pub(crate) use catalog::{
    CPU_OBSERVATION_WINDOW_BACKTRACK, CPU_OBSERVATION_WINDOW_BYTES, CpuObservation,
    FramebufferObservation, Oracle, OracleConfig, OracleObservations, OracleOutcome, OracleStep,
};
