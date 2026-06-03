// Domain facade for the SDL3 desktop frontend.
// Keep this file to module wiring; domain implementation chunks live in `frontend/*.rs`.

include!("frontend/runtime.rs");
include!("frontend/startup.rs");
include!("frontend/frame_loop.rs");
include!("frontend/timing.rs");
include!("frontend/diagnostics.rs");
include!("frontend/persistence.rs");
include!("frontend/presentation.rs");
include!("frontend/controls.rs");
include!("frontend/dialogs.rs");
include!("frontend/session.rs");
include!("frontend/host_audio.rs");
include!("frontend/benchmark.rs");

#[cfg(test)]
mod test;
