use crate::host_io::write_bytes_with_parent;
use crate::report::format_machine_save_state_io_error;
use crate::run::machine::CliMachine;
use gb_persistence::{
    MACHINE_SAVE_STATE_FILE_EXTENSION, MachineSaveStateEnvelope,
    decode_machine_save_state_envelope, encode_machine_save_state_envelope,
};
use std::fs;
use std::path::Path;

pub(crate) fn restore_machine_save_state_from_path(
    machine: &mut CliMachine,
    path: &Path,
) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read .{} state {}: {error}",
            MACHINE_SAVE_STATE_FILE_EXTENSION,
            path.display()
        )
    })?;
    let envelope = decode_machine_save_state_envelope(&bytes)
        .map_err(|error| format_machine_save_state_io_error("decode", path, error))?;
    machine
        .restore_save_state(&envelope.state)
        .map_err(|error| format!("failed to restore state {}: {error}", path.display()))
}

pub(crate) fn write_machine_save_state_to_path(
    machine: &CliMachine,
    path: &Path,
) -> Result<(), String> {
    let envelope = MachineSaveStateEnvelope::new(machine.capture_save_state());
    let bytes = encode_machine_save_state_envelope(&envelope)
        .map_err(|error| format_machine_save_state_io_error("encode", path, error))?;
    write_bytes_with_parent(path, &bytes)
        .map_err(|error| format!("failed to write state {}: {error}", path.display()))
}
