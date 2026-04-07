use super::*;
use crate::model::{CompatibilityPolicy, DiagnosticPolicy, ValidationPolicy};

pub(in crate::cartridge) fn validate_no_mbc(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<(), CartridgeLoadError> {
    let expected_ram_code = match classification.raw_type() {
        0x00 => 0x00,
        0x08 | 0x09 => 0x02,
        _ => unreachable!("non-NoMbc type entered NoMbc validation"),
    };

    if header.ram_size.raw_code != expected_ram_code {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects RAM size code {expected_ram_code:#04X}, but the header declared {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if header.ram_size.decoded_bytes != Some(expected_ram_code_decompressed(expected_ram_code)) {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} resolved to an unsupported RAM configuration from code {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if header.rom_size.raw_code != 0x00 {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects ROM size code 0x00, but the header declared {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if header.rom_size.decoded_bytes != Some(NO_MBC_SUPPORTED_ROM_BYTES) {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects a 32 KiB ROM declaration, but the header resolved to {:?} bytes",
                classification.detected_name(),
                header.rom_size.decoded_bytes
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if actual_rom_size != NO_MBC_SUPPORTED_ROM_BYTES {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects a 32 KiB image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                actual_rom_size
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if matches!(classification.raw_type(), 0x08 | 0x09) {
        diagnostics.push(CartridgeDiagnostic {
            severity: CartridgeDiagnosticSeverity::Warning,
            message: format!(
                "{} is rare but still treated as a valid No MBC variant",
                classification.detected_name()
            ),
        });
    }

    Ok(())
}

pub(in crate::cartridge) fn validate_mbc1(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<Mbc1Layout, CartridgeLoadError> {
    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared an unsupported ROM size code {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
            diagnostics: diagnostics.to_vec(),
        });
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} expects a {}-byte image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                declared_rom_bytes,
                actual_rom_size
            ),
            diagnostics: diagnostics.to_vec(),
        });
    }

    if classification.detected_name() == "MBC1M" {
        if header.ram_size.raw_code != 0x00 {
            return Err(CartridgeLoadError::Rejected {
                classification: *classification,
                execution_mode: compatibility.execution_mode,
                reason: format!(
                    "{} currently only supports the no-RAM 1 MiB multicart baseline",
                    classification.detected_name()
                ),
                diagnostics: diagnostics.to_vec(),
            });
        }

        if compatibility.diagnostic_policy != DiagnosticPolicy::Quiet {
            diagnostics.push(CartridgeDiagnostic {
                severity: CartridgeDiagnosticSeverity::Warning,
                message: format!(
                    "{} banking was enabled through an explicit experimental multicart heuristic and remains non-oracle",
                    classification.detected_name()
                ),
            });
        }

        return Ok(Mbc1Layout {
            wiring: Mbc1Wiring::LargeRom,
            variant: Mbc1Variant::Mbc1M,
            ram_len: 0,
        });
    }

    let wiring = if MBC1_STANDARD_ROM_SIZES.contains(&declared_rom_bytes) {
        Mbc1Wiring::Standard
    } else if MBC1_LARGE_ROM_SIZES.contains(&declared_rom_bytes) {
        Mbc1Wiring::LargeRom
    } else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared a ROM size that is not valid for the current MBC1 baseline: {} bytes",
                classification.detected_name(),
                declared_rom_bytes
            ),
            diagnostics: diagnostics.to_vec(),
        });
    };

    let has_ram = matches!(classification.raw_type(), 0x02 | 0x03);
    let allowed_ram_codes = match (has_ram, wiring) {
        (false, _) => [0x00].as_slice(),
        (true, Mbc1Wiring::Standard) => [0x02, 0x03].as_slice(),
        (true, Mbc1Wiring::LargeRom) => [0x02].as_slice(),
    };
    if !allowed_ram_codes.contains(&header.ram_size.raw_code) {
        let capability_label = if has_ram {
            "MBC1+RAM"
        } else {
            "MBC1 without RAM"
        };
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} declared RAM size code {:#04X}, which contradicts the current {} {:?} wiring baseline",
                classification.detected_name(),
                header.ram_size.raw_code,
                capability_label,
                wiring
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.to_vec(),
        })?;
    }

    let ram_len = match (has_ram, wiring, header.ram_size.raw_code) {
        (false, _, _) => 0,
        (true, Mbc1Wiring::Standard, 0x02 | 0x03) => header
            .ram_size
            .decoded_bytes
            .unwrap_or(MBC1_STANDARD_RAM_BYTES_MAX),
        (true, Mbc1Wiring::Standard, _) => MBC1_STANDARD_RAM_BYTES_MAX,
        (true, Mbc1Wiring::LargeRom, _) => MBC1_LARGE_ROM_RAM_BYTES,
    };

    Ok(Mbc1Layout {
        wiring,
        variant: Mbc1Variant::Standard,
        ram_len,
    })
}

pub(in crate::cartridge) fn validate_mbc2(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<(), CartridgeLoadError> {
    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared an unsupported ROM size code {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
            diagnostics: diagnostics.clone(),
        });
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} expects a {}-byte image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                declared_rom_bytes,
                actual_rom_size
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if declared_rom_bytes > MBC2_SUPPORTED_ROM_BYTES_MAX {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} exceeds the current MBC2 ROM limit of {} bytes with {} bytes",
                classification.detected_name(),
                MBC2_SUPPORTED_ROM_BYTES_MAX,
                declared_rom_bytes
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if header.ram_size.raw_code != 0x00 {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects RAM size code 0x00 because MBC2 RAM is internal, but the header declared {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    Ok(())
}

pub(in crate::cartridge) fn validate_mbc3(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<Mbc3Variant, CartridgeLoadError> {
    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared an unsupported ROM size code {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
            diagnostics: diagnostics.clone(),
        });
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} expects a {}-byte image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                declared_rom_bytes,
                actual_rom_size
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if declared_rom_bytes > MBC3_SUPPORTED_ROM_BYTES_MAX {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} exceeds the current MBC3 ROM limit of {} bytes with {} bytes",
                classification.detected_name(),
                MBC3_SUPPORTED_ROM_BYTES_MAX,
                declared_rom_bytes
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    let has_ram = matches!(classification.raw_type(), 0x10 | 0x12 | 0x13);
    if has_ram && header.ram_size.raw_code == 0x05 {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} with 64 KiB SRAM is reserved for the future MBC30 variant, not standard MBC3",
                classification.detected_name()
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if has_ram {
        if !matches!(header.ram_size.raw_code, 0x01..=0x03) {
            return Err(CartridgeLoadError::Rejected {
                classification: *classification,
                execution_mode: compatibility.execution_mode,
                reason: format!(
                    "{} declared RAM size code {:#04X}, which is not valid for the current standard MBC3 baseline",
                    classification.detected_name(),
                    header.ram_size.raw_code
                ),
                diagnostics: diagnostics.clone(),
            });
        }
    } else if header.ram_size.raw_code != 0x00 {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} does not provide external RAM, but the header declared RAM size code {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    Ok(Mbc3Variant::Standard)
}

pub(in crate::cartridge) fn validate_mbc5(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<Mbc5Variant, CartridgeLoadError> {
    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared an unsupported ROM size code {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
            diagnostics: diagnostics.clone(),
        });
    };

    if actual_rom_size > MBC5_SUPPORTED_ROM_BYTES_MAX {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} exceeds the current MBC5 ROM limit of {} bytes with {} bytes",
                classification.detected_name(),
                MBC5_SUPPORTED_ROM_BYTES_MAX,
                actual_rom_size
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if actual_rom_size != declared_rom_bytes {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} expects a {}-byte image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                declared_rom_bytes,
                actual_rom_size
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    let variant = match classification.raw_type() {
        0x19 => Mbc5Variant::NoRam,
        0x1A => Mbc5Variant::Ram,
        0x1B => Mbc5Variant::RamBattery,
        0x1C => Mbc5Variant::Rumble,
        0x1D => Mbc5Variant::RumbleRam,
        0x1E => Mbc5Variant::RumbleRamBattery,
        _ => unreachable!("non-MBC5 type entered MBC5 validation"),
    };

    if variant.has_ram() {
        let allowed_ram_codes = if variant.has_rumble() {
            [0x02, 0x03, 0x05].as_slice()
        } else {
            [0x02, 0x03, 0x04, 0x05].as_slice()
        };

        if !allowed_ram_codes.contains(&header.ram_size.raw_code) {
            return Err(CartridgeLoadError::Rejected {
                classification: *classification,
                execution_mode: compatibility.execution_mode,
                reason: format!(
                    "{} declared RAM size code {:#04X}, which is not valid for the current {} MBC5 baseline",
                    classification.detected_name(),
                    header.ram_size.raw_code,
                    if variant.has_rumble() {
                        "rumble-capable"
                    } else {
                        "standard"
                    }
                ),
                diagnostics: diagnostics.clone(),
            });
        }
    } else if header.ram_size.raw_code != 0x00 {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} does not provide external RAM, but the header declared RAM size code {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    Ok(variant)
}

pub(in crate::cartridge) fn record_degradable_issue(
    diagnostics: &mut Vec<CartridgeDiagnostic>,
    validation_policy: ValidationPolicy,
    message: String,
) -> Result<(), String> {
    match validation_policy {
        ValidationPolicy::Strict => Err(message),
        ValidationPolicy::Warn => {
            diagnostics.push(CartridgeDiagnostic {
                severity: CartridgeDiagnosticSeverity::Warning,
                message,
            });
            Ok(())
        }
        ValidationPolicy::Ignore => Ok(()),
    }
}

pub(in crate::cartridge) const fn expected_ram_code_decompressed(code: u8) -> usize {
    match code {
        0x00 => 0,
        0x02 => NO_MBC_SUPPORTED_RAM_BYTES,
        _ => 0,
    }
}
