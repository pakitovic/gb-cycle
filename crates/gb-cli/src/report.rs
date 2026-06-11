use crate::host_io::writeln_checked;
use gb_core::{
    BootRomAssetError, CartridgeDiagnostic, CartridgeDiagnosticSeverity, CartridgeHeaderParseError,
    CartridgeLoadError, CartridgeSelection, CgbFlag, CompatibilityPolicy, ConsoleModel,
    ExecutionMode, HardwareRevision, HostPlatform, SgbFlag, StartupMode,
    UnsupportedCartridgeCategory,
};
use gb_persistence::{
    CartridgeSaveBackendError, EXTERNAL_SAVE_FILE_EXTENSION, ExternalSaveError,
    MACHINE_SAVE_STATE_FILE_EXTENSION,
};
use std::io::{self, Write};
use std::path::Path;

pub(crate) fn write_cartridge_diagnostics(
    stderr: &mut dyn Write,
    diagnostics: &[CartridgeDiagnostic],
) -> Result<(), String> {
    for diagnostic in diagnostics {
        writeln_checked(
            stderr,
            &format!(
                "{}: {}",
                diagnostic_severity_name(diagnostic.severity),
                diagnostic.message
            ),
        )?;
    }
    Ok(())
}

pub(crate) fn compatibility_for_execution_mode(
    execution_mode: ExecutionMode,
) -> CompatibilityPolicy {
    match execution_mode {
        ExecutionMode::Strict => CompatibilityPolicy::strict(),
        ExecutionMode::Permissive => CompatibilityPolicy::permissive(),
        ExecutionMode::Experimental => CompatibilityPolicy::experimental(),
    }
}

pub(crate) fn revision_argument_name(revision: HardwareRevision) -> &'static str {
    match revision {
        HardwareRevision::DmgCpu0 => "dmg-cpu-0",
        HardwareRevision::DmgCpuA => "dmg-cpu-a",
        HardwareRevision::DmgCpuB => "dmg-cpu-b",
        HardwareRevision::DmgCpuC => "dmg-cpu-c",
        HardwareRevision::CpuMgb => "cpu-mgb",
        HardwareRevision::CpuCgb0 => "cpu-cgb-0",
        HardwareRevision::CpuCgbA => "cpu-cgb-a",
        HardwareRevision::CpuCgbB => "cpu-cgb-b",
        HardwareRevision::CpuCgbC => "cpu-cgb-c",
        HardwareRevision::CpuCgbD => "cpu-cgb-d",
        HardwareRevision::CpuCgbE => "cpu-cgb-e",
        HardwareRevision::CpuAgbA => "cpu-agb-a",
    }
}

#[cfg(test)]
pub(crate) fn supported_revision_names(console_model: ConsoleModel) -> String {
    supported_revision_names_on_host(console_model, HostPlatform::Handheld)
}

pub(crate) fn supported_revision_names_on_host(
    console_model: ConsoleModel,
    host_platform: HostPlatform,
) -> String {
    console_model
        .active_revisions_on_host(host_platform)
        .iter()
        .map(|revision| revision_argument_name(*revision))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn format_framebuffer_artifact_error(path: &Path, error: io::Error) -> String {
    format!(
        "failed to encode framebuffer artifact {}: {error}",
        path.display()
    )
}

pub(crate) fn format_save_load_error(path: &Path, error: CartridgeSaveBackendError) -> String {
    format!("failed to load save {}: {error}", path.display())
}

pub(crate) fn format_save_flush_error(
    path: &Path,
    reason: &str,
    error: CartridgeSaveBackendError,
) -> String {
    format!(
        "failed to save cartridge persistence ({reason}) to {}: {error}",
        path.display()
    )
}

pub(crate) fn format_machine_save_state_io_error(
    operation: &str,
    path: &Path,
    error: CartridgeSaveBackendError,
) -> String {
    format!(
        "failed to {operation} .{} state {}: {error}",
        MACHINE_SAVE_STATE_FILE_EXTENSION,
        path.display()
    )
}

pub(crate) fn format_external_save_error(error: ExternalSaveError) -> String {
    format!("failed to convert external .{EXTERNAL_SAVE_FILE_EXTENSION} save: {error}")
}

pub(crate) fn format_boot_rom_asset_load_error(root: &Path, error: BootRomAssetError) -> String {
    format!(
        "failed to load boot ROM assets from {}: {error}",
        root.display()
    )
}

pub(crate) fn format_cartridge_load_error(error: CartridgeLoadError) -> String {
    match error {
        CartridgeLoadError::HeaderParse(error) => format_header_parse_error(error),
        CartridgeLoadError::Rejected {
            classification,
            execution_mode,
            reason,
            diagnostics,
        } => {
            let mut message = format!(
                "cartridge rejected under {}: mapper={} selection={} reason={}",
                execution_mode_name(execution_mode),
                classification.detected_name(),
                selection_name(classification.selection()),
                reason,
            );
            if !diagnostics.is_empty() {
                let joined = diagnostics
                    .into_iter()
                    .map(|diagnostic| {
                        format!(
                            "{} {}",
                            diagnostic_severity_name(diagnostic.severity),
                            diagnostic.message
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                message.push_str(&format!(" diagnostics=[{joined}]"));
            }
            message
        }
    }
}

pub(crate) fn format_header_parse_error(error: CartridgeHeaderParseError) -> String {
    match error {
        CartridgeHeaderParseError::ImageTooSmall {
            actual_size,
            minimum_size,
        } => format!(
            "ROM image is too small to contain a cartridge header: expected at least {} bytes, got {}",
            minimum_size, actual_size
        ),
    }
}

pub(crate) fn startup_mode_name(startup_mode: StartupMode) -> &'static str {
    match startup_mode {
        StartupMode::SkipBoot => "skip-boot",
        StartupMode::CustomBoot => "custom-boot",
        StartupMode::RealBoot => "real-boot",
    }
}

pub(crate) fn execution_mode_name(execution_mode: ExecutionMode) -> &'static str {
    match execution_mode {
        ExecutionMode::Strict => "strict",
        ExecutionMode::Permissive => "permissive",
        ExecutionMode::Experimental => "experimental",
    }
}

pub(crate) fn diagnostic_severity_name(severity: CartridgeDiagnosticSeverity) -> &'static str {
    match severity {
        CartridgeDiagnosticSeverity::Warning => "warning",
        CartridgeDiagnosticSeverity::Error => "error",
    }
}

pub(crate) fn selection_name(selection: CartridgeSelection) -> &'static str {
    match selection {
        CartridgeSelection::Supported(_) => "supported",
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::PlannedVariant) => {
            "unsupported-planned-variant"
        }
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::DocumentedButUnsupported) => {
            "unsupported-documented"
        }
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::ExperimentalHeuristic) => {
            "unsupported-experimental-heuristic"
        }
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::AccessorySpecialCase) => {
            "unsupported-accessory"
        }
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::UnknownCode) => {
            "unsupported-unknown"
        }
    }
}

pub(crate) fn cgb_flag_name(flag: CgbFlag) -> String {
    match flag {
        CgbFlag::None => "none".to_string(),
        CgbFlag::Supported => "supported".to_string(),
        CgbFlag::Only => "only".to_string(),
        CgbFlag::SupportedNonCanonical(value) => {
            format!("supported-noncanonical(0x{value:02X})")
        }
        CgbFlag::Unknown(value) => format!("unknown(0x{value:02X})"),
    }
}

pub(crate) fn sgb_flag_name(flag: SgbFlag) -> String {
    match flag {
        SgbFlag::None => "none".to_string(),
        SgbFlag::Supported => "supported".to_string(),
        SgbFlag::Unknown(value) => format!("unknown(0x{value:02X})"),
    }
}

pub(crate) fn optional_usize_name(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
