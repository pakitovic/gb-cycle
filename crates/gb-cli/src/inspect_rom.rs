use crate::host_io::{resolve_path, writeln_checked};
use crate::options::InspectRomOptions;
use crate::report::{
    cgb_flag_name, compatibility_for_execution_mode, diagnostic_severity_name, execution_mode_name,
    format_header_parse_error, optional_usize_name, selection_name, sgb_flag_name,
};
use gb_core::{CartridgeHeader, CartridgeLoadError, CartridgeSlot};
use std::env;
use std::fs;
use std::io::Write;

pub(crate) fn inspect_rom_command(
    options: InspectRomOptions,
    output: &mut dyn Write,
) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let rom_path = resolve_path(&current_dir, &options.rom_path);
    let rom_bytes = fs::read(&rom_path)
        .map_err(|error| format!("failed to read ROM {}: {error}", rom_path.display()))?;
    let header = CartridgeHeader::parse(&rom_bytes).map_err(format_header_parse_error)?;
    let compatibility = compatibility_for_execution_mode(options.execution_mode);

    let (load_status, classification, diagnostics, rejection_reason, effective_rom_layout) =
        match CartridgeSlot::load(rom_bytes, &compatibility) {
            Ok(report) => {
                let classification = report
                    .cartridge()
                    .classification()
                    .expect("loaded cartridges should always expose a classification");
                let effective_rom_layout = report.effective_rom_layout();
                (
                    "ok",
                    classification,
                    report.diagnostics().to_vec(),
                    None::<String>,
                    effective_rom_layout,
                )
            }
            Err(CartridgeLoadError::Rejected {
                classification,
                reason,
                diagnostics,
                ..
            }) => ("rejected", classification, diagnostics, Some(reason), None),
            Err(CartridgeLoadError::HeaderParse(error)) => {
                return Err(format_header_parse_error(error));
            }
        };

    writeln_checked(output, &format!("rom={}", rom_path.display()))?;
    writeln_checked(output, &format!("title={}", header.title))?;
    writeln_checked(
        output,
        &format!(
            "execution_mode={}",
            execution_mode_name(options.execution_mode)
        ),
    )?;
    writeln_checked(output, &format!("load_status={load_status}"))?;
    writeln_checked(
        output,
        &format!("cartridge_type=0x{:02X}", header.cartridge_type),
    )?;
    writeln_checked(
        output,
        &format!("mapper_name={}", classification.detected_name()),
    )?;
    writeln_checked(
        output,
        &format!("selection={}", selection_name(classification.selection())),
    )?;
    writeln_checked(
        output,
        &format!("selection_reason={}", classification.reason()),
    )?;
    writeln_checked(
        output,
        &format!("cgb_flag={}", cgb_flag_name(header.cgb_flag)),
    )?;
    writeln_checked(
        output,
        &format!("sgb_flag={}", sgb_flag_name(header.sgb_flag)),
    )?;
    writeln_checked(
        output,
        &format!("rom_size_code=0x{:02X}", header.rom_size.raw_code),
    )?;
    writeln_checked(
        output,
        &format!(
            "rom_size_bytes={}",
            optional_usize_name(header.rom_size.decoded_bytes)
        ),
    )?;
    writeln_checked(
        output,
        &format!(
            "rom_bank_count={}",
            optional_usize_name(header.rom_size.bank_count)
        ),
    )?;
    writeln_checked(
        output,
        &format!(
            "effective_rom_size_bytes={}",
            optional_usize_name(effective_rom_layout.map(|layout| layout.effective_bytes))
        ),
    )?;
    writeln_checked(
        output,
        &format!(
            "effective_rom_bank_count={}",
            optional_usize_name(effective_rom_layout.map(|layout| layout.effective_bank_count))
        ),
    )?;
    writeln_checked(
        output,
        &format!(
            "rom_size_source={}",
            effective_rom_layout.map_or("unknown", |layout| layout.source.name())
        ),
    )?;
    writeln_checked(
        output,
        &format!("ram_size_code=0x{:02X}", header.ram_size.raw_code),
    )?;
    writeln_checked(
        output,
        &format!(
            "ram_size_bytes={}",
            optional_usize_name(header.ram_size.decoded_bytes)
        ),
    )?;
    writeln_checked(
        output,
        &format!(
            "ram_bank_count={}",
            optional_usize_name(header.ram_size.bank_count)
        ),
    )?;
    writeln_checked(output, &format!("diagnostic_count={}", diagnostics.len()))?;
    for diagnostic in diagnostics {
        writeln_checked(
            output,
            &format!(
                "diagnostic={} {}",
                diagnostic_severity_name(diagnostic.severity),
                diagnostic.message
            ),
        )?;
    }
    if let Some(reason) = rejection_reason {
        writeln_checked(output, &format!("rejection_reason={reason}"))?;
    }

    Ok(())
}
