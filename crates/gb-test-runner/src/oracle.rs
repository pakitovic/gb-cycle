mod catalog;
pub(crate) mod fibonacci_result;
pub(crate) mod framebuffer;
pub(crate) mod memory_byte_equals;
pub(crate) mod serial_contains;
pub(crate) mod serial_hex_exact;
pub(crate) mod snapshot;
pub(crate) mod trace;

#[cfg(test)]
mod test;

pub(crate) use catalog::{
    CPU_OBSERVATION_WINDOW_BACKTRACK, CPU_OBSERVATION_WINDOW_BYTES, CpuObservation,
    FramebufferObservation, MemoryObservation, Oracle, OracleConfig, OracleFixtureRoots,
    OracleObservations, OracleOutcome, OracleStep,
};
#[cfg(test)]
pub(crate) use catalog::{LinkedParticipantObservation, LinkedSessionObservation};
pub(crate) use framebuffer::{FramebufferArtifactDescriptor, FramebufferArtifactSource};
