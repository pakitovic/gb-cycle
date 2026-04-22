use super::*;
use crate::model::{CompatibilityPolicy, ValidationPolicy};

/// Bundles the parameters shared by every mapper validator, providing
/// helpers that eliminate the repeated `CartridgeLoadError::Rejected`
/// construction and `record_degradable_issue` + `map_err` pattern.
struct ValidationContext<'a> {
    compatibility: &'a CompatibilityPolicy,
    classification: &'a CartridgeClassification,
    diagnostics: &'a mut Vec<CartridgeDiagnostic>,
}

impl<'a> ValidationContext<'a> {
    fn name(&self) -> &str {
        self.classification.detected_name()
    }

    fn reject(&self, reason: String) -> CartridgeLoadError {
        CartridgeLoadError::Rejected {
            classification: *self.classification,
            execution_mode: self.compatibility.execution_mode,
            reason,
            diagnostics: self.diagnostics.clone(),
        }
    }

    fn check_degradable(&mut self, message: String) -> Result<(), CartridgeLoadError> {
        record_degradable_issue(
            self.diagnostics,
            self.compatibility.validation_policy,
            message,
        )
        .map_err(|reason| self.reject(reason))
    }
}

pub(in crate::cartridge) fn validate_no_mbc(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<(), CartridgeLoadError> {
    let mut ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    let expected_ram_code = match classification.raw_type() {
        0x00 => 0x00,
        0x08 | 0x09 => 0x02,
        _ => unreachable!("non-NoMbc type entered NoMbc validation"),
    };

    if header.ram_size.raw_code != expected_ram_code {
        ctx.check_degradable(format!(
            "{} expects RAM size code {expected_ram_code:#04X}, but the header declared {:#04X}",
            ctx.name(),
            header.ram_size.raw_code
        ))?;
    }

    if header.ram_size.decoded_bytes != Some(expected_ram_code_decompressed(expected_ram_code)) {
        ctx.check_degradable(format!(
            "{} resolved to an unsupported RAM configuration from code {:#04X}",
            ctx.name(),
            header.ram_size.raw_code
        ))?;
    }

    if header.rom_size.raw_code != 0x00 {
        ctx.check_degradable(format!(
            "{} expects ROM size code 0x00, but the header declared {:#04X}",
            ctx.name(),
            header.rom_size.raw_code
        ))?;
    }

    if header.rom_size.decoded_bytes != Some(NO_MBC_SUPPORTED_ROM_BYTES) {
        ctx.check_degradable(format!(
            "{} expects a 32 KiB ROM declaration, but the header resolved to {:?} bytes",
            ctx.name(),
            header.rom_size.decoded_bytes
        ))?;
    }

    if actual_rom_size != NO_MBC_SUPPORTED_ROM_BYTES {
        ctx.check_degradable(format!(
            "{} expects a 32 KiB image, but the loaded ROM is {} bytes",
            ctx.name(),
            actual_rom_size
        ))?;
    }

    if matches!(classification.raw_type(), 0x08 | 0x09) {
        ctx.diagnostics.push(CartridgeDiagnostic {
            severity: CartridgeDiagnosticSeverity::Warning,
            message: format!(
                "{} is rare but still treated as a valid No MBC variant",
                ctx.name()
            ),
        });
    }

    Ok(())
}

pub(in crate::cartridge) fn validate_mmm01(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<usize, CartridgeLoadError> {
    let ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(ctx.reject(format!(
            "{} declared an unsupported ROM size code {:#04X}",
            ctx.name(),
            header.rom_size.raw_code
        )));
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(ctx.reject(format!(
            "{} expects a {}-byte image, but the loaded ROM is {} bytes",
            ctx.name(),
            declared_rom_bytes,
            actual_rom_size
        )));
    }

    if !(MMM01_MIN_ROM_BYTES..=MMM01_SUPPORTED_ROM_BYTES_MAX).contains(&declared_rom_bytes) {
        return Err(ctx.reject(format!(
            "{} expects a total ROM size between {} and {} bytes, but the header resolved to {} bytes",
            ctx.name(),
            MMM01_MIN_ROM_BYTES,
            MMM01_SUPPORTED_ROM_BYTES_MAX,
            declared_rom_bytes
        )));
    }

    let has_ram = matches!(classification.raw_type(), 0x0C | 0x0D);
    let allowed_ram_codes = if has_ram {
        [0x02, 0x03, 0x04].as_slice()
    } else {
        [0x00].as_slice()
    };
    if !allowed_ram_codes.contains(&header.ram_size.raw_code) {
        let capability_label = if has_ram {
            "MMM01 with external RAM"
        } else {
            "MMM01 without RAM"
        };
        return Err(ctx.reject(format!(
            "{} declared RAM size code {:#04X}, which contradicts the current {} baseline",
            ctx.name(),
            header.ram_size.raw_code,
            capability_label
        )));
    }

    Ok(header.ram_size.decoded_bytes.unwrap_or(0))
}

pub(in crate::cartridge) fn validate_huc1(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<usize, CartridgeLoadError> {
    let ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(ctx.reject(format!(
            "{} declared an unsupported ROM size code {:#04X}",
            ctx.name(),
            header.rom_size.raw_code
        )));
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(ctx.reject(format!(
            "{} expects a {}-byte image, but the loaded ROM is {} bytes",
            ctx.name(),
            declared_rom_bytes,
            actual_rom_size
        )));
    }

    if declared_rom_bytes > HUC1_SUPPORTED_ROM_BYTES_MAX {
        return Err(ctx.reject(format!(
            "{} exceeds the current HuC1 ROM limit of {} bytes with {} bytes",
            ctx.name(),
            HUC1_SUPPORTED_ROM_BYTES_MAX,
            declared_rom_bytes
        )));
    }

    let Some(ram_len) = header.ram_size.decoded_bytes else {
        return Err(ctx.reject(format!(
            "{} declared an unsupported RAM size code {:#04X}",
            ctx.name(),
            header.ram_size.raw_code
        )));
    };

    if ram_len == 0 || ram_len > HUC1_SUPPORTED_RAM_BYTES_MAX {
        return Err(ctx.reject(format!(
            "{} expects cartridge RAM between 1 and {} bytes, but the header resolved to {} bytes",
            ctx.name(),
            HUC1_SUPPORTED_RAM_BYTES_MAX,
            ram_len
        )));
    }

    Ok(ram_len)
}

pub(in crate::cartridge) fn validate_mbc1(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<Mbc1Layout, CartridgeLoadError> {
    let mut ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(ctx.reject(format!(
            "{} declared an unsupported ROM size code {:#04X}",
            ctx.name(),
            header.rom_size.raw_code
        )));
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(ctx.reject(format!(
            "{} expects a {}-byte image, but the loaded ROM is {} bytes",
            ctx.name(),
            declared_rom_bytes,
            actual_rom_size
        )));
    }

    if classification.detected_name() == "MBC1M" {
        let has_ram = matches!(classification.raw_type(), 0x02 | 0x03);
        let expected_ram_code = if has_ram { 0x02 } else { 0x00 };

        if header.ram_size.raw_code != expected_ram_code {
            let capability_label = if has_ram { "fixed 8 KiB RAM" } else { "no RAM" };
            return Err(ctx.reject(format!(
                "{} currently only supports the 1 MiB multicart baseline with {}",
                ctx.name(),
                capability_label
            )));
        }

        return Ok(Mbc1Layout {
            wiring: Mbc1Wiring::LargeRom,
            variant: Mbc1Variant::Mbc1M,
            ram_len: if has_ram { 8 * 1024 } else { 0 },
        });
    }

    let wiring = if MBC1_STANDARD_ROM_SIZES.contains(&declared_rom_bytes) {
        Mbc1Wiring::Standard
    } else if MBC1_LARGE_ROM_SIZES.contains(&declared_rom_bytes) {
        Mbc1Wiring::LargeRom
    } else {
        return Err(ctx.reject(format!(
            "{} declared a ROM size that is not valid for the current MBC1 baseline: {} bytes",
            ctx.name(),
            declared_rom_bytes
        )));
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
        ctx.check_degradable(format!(
            "{} declared RAM size code {:#04X}, which contradicts the current {} {:?} wiring baseline",
            ctx.name(),
            header.ram_size.raw_code,
            capability_label,
            wiring
        ))?;
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
    let mut ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(ctx.reject(format!(
            "{} declared an unsupported ROM size code {:#04X}",
            ctx.name(),
            header.rom_size.raw_code
        )));
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(ctx.reject(format!(
            "{} expects a {}-byte image, but the loaded ROM is {} bytes",
            ctx.name(),
            declared_rom_bytes,
            actual_rom_size
        )));
    }

    if declared_rom_bytes > MBC2_SUPPORTED_ROM_BYTES_MAX {
        return Err(ctx.reject(format!(
            "{} exceeds the current MBC2 ROM limit of {} bytes with {} bytes",
            ctx.name(),
            MBC2_SUPPORTED_ROM_BYTES_MAX,
            declared_rom_bytes
        )));
    }

    if header.ram_size.raw_code != 0x00 {
        ctx.check_degradable(format!(
            "{} expects RAM size code 0x00 because MBC2 RAM is internal, but the header declared {:#04X}",
            ctx.name(),
            header.ram_size.raw_code
        ))?;
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
    let mut ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(ctx.reject(format!(
            "{} declared an unsupported ROM size code {:#04X}",
            ctx.name(),
            header.rom_size.raw_code
        )));
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(ctx.reject(format!(
            "{} expects a {}-byte image, but the loaded ROM is {} bytes",
            ctx.name(),
            declared_rom_bytes,
            actual_rom_size
        )));
    }

    if declared_rom_bytes > MBC3_SUPPORTED_ROM_BYTES_MAX {
        return Err(ctx.reject(format!(
            "{} exceeds the current MBC3 ROM limit of {} bytes with {} bytes",
            ctx.name(),
            MBC3_SUPPORTED_ROM_BYTES_MAX,
            declared_rom_bytes
        )));
    }

    let has_ram = matches!(classification.raw_type(), 0x10 | 0x12 | 0x13);
    if has_ram && header.ram_size.raw_code == 0x05 {
        return Err(ctx.reject(format!(
            "{} with 64 KiB SRAM is reserved for the future MBC30 variant, not standard MBC3",
            ctx.name()
        )));
    }

    if has_ram {
        if !matches!(header.ram_size.raw_code, 0x01..=0x03) {
            return Err(ctx.reject(format!(
                "{} declared RAM size code {:#04X}, which is not valid for the current standard MBC3 baseline",
                ctx.name(),
                header.ram_size.raw_code
            )));
        }
    } else if header.ram_size.raw_code != 0x00 {
        ctx.check_degradable(format!(
            "{} does not provide external RAM, but the header declared RAM size code {:#04X}",
            ctx.name(),
            header.ram_size.raw_code
        ))?;
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
    let mut ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(ctx.reject(format!(
            "{} declared an unsupported ROM size code {:#04X}",
            ctx.name(),
            header.rom_size.raw_code
        )));
    };

    if actual_rom_size > MBC5_SUPPORTED_ROM_BYTES_MAX {
        return Err(ctx.reject(format!(
            "{} exceeds the current MBC5 ROM limit of {} bytes with {} bytes",
            ctx.name(),
            MBC5_SUPPORTED_ROM_BYTES_MAX,
            actual_rom_size
        )));
    }

    if actual_rom_size != declared_rom_bytes {
        return Err(ctx.reject(format!(
            "{} expects a {}-byte image, but the loaded ROM is {} bytes",
            ctx.name(),
            declared_rom_bytes,
            actual_rom_size
        )));
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
            return Err(ctx.reject(format!(
                "{} declared RAM size code {:#04X}, which is not valid for the current {} MBC5 baseline",
                ctx.name(),
                header.ram_size.raw_code,
                if variant.has_rumble() {
                    "rumble-capable"
                } else {
                    "standard"
                }
            )));
        }
    } else if header.ram_size.raw_code != 0x00 {
        ctx.check_degradable(format!(
            "{} does not provide external RAM, but the header declared RAM size code {:#04X}",
            ctx.name(),
            header.ram_size.raw_code
        ))?;
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
