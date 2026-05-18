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
        let resolved_ram_len = header.ram_size.decoded_bytes.map_or_else(
            || "an unsupported length".to_string(),
            |bytes| format!("{bytes} bytes"),
        );
        ctx.check_degradable(format!(
            "{} uses the fixed {}, but header RAM size code {:#04X} resolved to {resolved_ram_len}",
            ctx.name(),
            no_mbc_ram_baseline_description(expected_ram_code),
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

pub(in crate::cartridge) fn validate_m161(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<(), CartridgeLoadError> {
    let ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    if header.ram_size.raw_code != 0x00 || header.ram_size.decoded_bytes != Some(0) {
        return Err(ctx.reject(format!(
            "{} expects no external RAM, but the header resolved to {:?} bytes from code {:#04X}",
            ctx.name(),
            header.ram_size.decoded_bytes,
            header.ram_size.raw_code
        )));
    }

    if !actual_rom_size.is_multiple_of(M161_BANK_BYTES) {
        return Err(ctx.reject(format!(
            "{} expects a ROM image sized in 32 KiB banks, but the loaded ROM is {} bytes",
            ctx.name(),
            actual_rom_size
        )));
    }

    let bank_count = actual_rom_size / M161_BANK_BYTES;
    if !(M161_SUPPORTED_ROM_BANKS_MIN..=M161_SUPPORTED_ROM_BANKS_MAX).contains(&bank_count) {
        return Err(ctx.reject(format!(
            "{} expects between {} and {} total 32 KiB banks, but the loaded ROM resolves to {} banks",
            ctx.name(),
            M161_SUPPORTED_ROM_BANKS_MIN,
            M161_SUPPORTED_ROM_BANKS_MAX,
            bank_count
        )));
    }

    Ok(())
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

pub(in crate::cartridge) fn validate_huc3(
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

    if declared_rom_bytes > HUC3_SUPPORTED_ROM_BYTES_MAX {
        return Err(ctx.reject(format!(
            "{} exceeds the current HuC-3 ROM limit of {} bytes with {} bytes",
            ctx.name(),
            HUC3_SUPPORTED_ROM_BYTES_MAX,
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

    if ram_len == 0 || ram_len > HUC3_SUPPORTED_RAM_BYTES_MAX {
        return Err(ctx.reject(format!(
            "{} expects cartridge RAM between 1 and {} bytes, but the header resolved to {} bytes",
            ctx.name(),
            HUC3_SUPPORTED_RAM_BYTES_MAX,
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

    let has_ram = matches!(classification.raw_type(), 0x10 | 0x12 | 0x13);
    let variant = if has_ram && header.ram_size.raw_code == 0x05 {
        Mbc3Variant::Mbc30
    } else {
        Mbc3Variant::Standard
    };
    let rom_limit = match variant {
        Mbc3Variant::Standard => MBC3_SUPPORTED_ROM_BYTES_MAX,
        Mbc3Variant::Mbc30 => MBC30_SUPPORTED_ROM_BYTES_MAX,
    };
    let variant_name = match variant {
        Mbc3Variant::Standard => "MBC3",
        Mbc3Variant::Mbc30 => "MBC30",
    };

    if declared_rom_bytes > rom_limit {
        return Err(ctx.reject(format!(
            "{} exceeds the current {} ROM limit of {} bytes with {} bytes",
            ctx.name(),
            variant_name,
            rom_limit,
            declared_rom_bytes
        )));
    }

    match (has_ram, variant) {
        (true, Mbc3Variant::Standard) => {
            if !matches!(header.ram_size.raw_code, 0x01..=0x03) {
                return Err(ctx.reject(format!(
                    "{} declared RAM size code {:#04X}, which is not valid for the current standard MBC3 baseline",
                    ctx.name(),
                    header.ram_size.raw_code
                )));
            }
        }
        (true, Mbc3Variant::Mbc30) => {
            if header.ram_size.decoded_bytes != Some(64 * 1024)
                || header.ram_size.bank_count != Some(8)
            {
                return Err(ctx.reject(format!(
                    "{} declared RAM size code {:#04X}, which did not resolve to the required MBC30 64 KiB / 8-bank SRAM shape",
                    ctx.name(),
                    header.ram_size.raw_code
                )));
            }
        }
        (false, _) if header.ram_size.raw_code != 0x00 => {
            ctx.check_degradable(format!(
                "{} does not provide external RAM, but the header declared RAM size code {:#04X}",
                ctx.name(),
                header.ram_size.raw_code
            ))?;
        }
        (false, _) => {}
    }

    Ok(variant)
}

pub(in crate::cartridge) fn validate_mbc5(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<Mbc5ValidationLayout, CartridgeLoadError> {
    let mut ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    let rom_layout = validate_mbc5_rom_layout(&mut ctx, header, actual_rom_size)?;
    let variant = match classification.raw_type() {
        0x19 => Mbc5Variant::NoRam,
        0x1A => Mbc5Variant::Ram,
        0x1B => Mbc5Variant::RamBattery,
        0x1C => Mbc5Variant::Rumble,
        0x1D => Mbc5Variant::RumbleRam,
        0x1E => Mbc5Variant::RumbleRamBattery,
        _ => unreachable!("non-MBC5 type entered MBC5 validation"),
    };

    let ram_len;
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
        ram_len = header.ram_size.decoded_bytes.unwrap_or(0);
    } else if header.ram_size.raw_code != 0x00 {
        ctx.check_degradable(format!(
            "{} does not provide external RAM, but the header declared RAM size code {:#04X}",
            ctx.name(),
            header.ram_size.raw_code
        ))?;
        ram_len = 0;
    } else {
        ram_len = 0;
    }

    Ok(Mbc5ValidationLayout {
        variant,
        rom_layout,
        ram_len,
    })
}

fn validate_mbc5_rom_layout(
    ctx: &mut ValidationContext<'_>,
    header: &CartridgeHeader,
    actual_rom_size: usize,
) -> Result<CartridgeRomLayout, CartridgeLoadError> {
    if actual_rom_size > MBC5_SUPPORTED_ROM_BYTES_MAX {
        return Err(ctx.reject(format!(
            "{} exceeds the current MBC5 ROM limit of {} bytes with {} bytes",
            ctx.name(),
            MBC5_SUPPORTED_ROM_BYTES_MAX,
            actual_rom_size
        )));
    }

    if let Some(declared_rom_bytes) = header.rom_size.decoded_bytes {
        if actual_rom_size == declared_rom_bytes {
            return Ok(CartridgeRomLayout::declared_exact(
                declared_rom_bytes,
                actual_rom_size,
            ));
        }

        if ctx.compatibility.validation_policy == ValidationPolicy::Strict {
            return Err(ctx.reject(format!(
                "{} expects a {}-byte image, but the loaded ROM is {} bytes",
                ctx.name(),
                declared_rom_bytes,
                actual_rom_size
            )));
        }

        let effective_rom_size = rounded_mbc5_actual_rom_capacity(actual_rom_size);
        ctx.check_degradable(format!(
            "{} declared a {}-byte ROM from size code {:#04X}, but the loaded ROM is {} bytes; using a {}-byte permissive ROM capacity padded with 0xFF",
            ctx.name(),
            declared_rom_bytes,
            header.rom_size.raw_code,
            actual_rom_size,
            effective_rom_size
        ))?;
        return Ok(CartridgeRomLayout {
            declared_bytes: Some(declared_rom_bytes),
            actual_bytes: actual_rom_size,
            effective_bytes: effective_rom_size,
            effective_bank_count: effective_rom_size / ROM_BANK_BYTES,
            source: CartridgeRomLayoutSource::PermissiveRoundedActual,
        });
    }

    if ctx.compatibility.validation_policy == ValidationPolicy::Strict {
        return Err(ctx.reject(format!(
            "{} declared an unsupported ROM size code {:#04X}",
            ctx.name(),
            header.rom_size.raw_code
        )));
    }

    let effective_rom_size = rounded_mbc5_actual_rom_capacity(actual_rom_size);
    ctx.check_degradable(format!(
        "{} declared unsupported ROM size code {:#04X}; using a {}-byte permissive ROM capacity derived from the {}-byte image and padded with 0xFF",
        ctx.name(),
        header.rom_size.raw_code,
        effective_rom_size,
        actual_rom_size
    ))?;
    Ok(CartridgeRomLayout {
        declared_bytes: None,
        actual_bytes: actual_rom_size,
        effective_bytes: effective_rom_size,
        effective_bank_count: effective_rom_size / ROM_BANK_BYTES,
        source: CartridgeRomLayoutSource::PermissiveRoundedActual,
    })
}

fn rounded_mbc5_actual_rom_capacity(actual_rom_size: usize) -> usize {
    let bank_rounded = actual_rom_size
        .div_ceil(ROM_BANK_BYTES)
        .max(NO_MBC_SUPPORTED_ROM_BYTES / ROM_BANK_BYTES)
        * ROM_BANK_BYTES;
    bank_rounded.next_power_of_two()
}

pub(in crate::cartridge) fn validate_mbc6(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<(), CartridgeLoadError> {
    let ctx = ValidationContext {
        compatibility,
        classification,
        diagnostics,
    };

    if !header.cgb_flag.enables_cgb_native_mode() {
        return Err(ctx.reject(format!(
            "{} expects a CGB-capable header because the only documented MBC6 cartridge is a CGB-era Net de Get board",
            ctx.name()
        )));
    }

    if header.rom_size.raw_code != 0x05
        || header.rom_size.decoded_bytes != Some(MBC6_SUPPORTED_ROM_BYTES)
    {
        return Err(ctx.reject(format!(
            "{} expects the official 1 MiB ROM declaration (code 0x05), but the header declared code {:#04X} ({:?} bytes)",
            ctx.name(),
            header.rom_size.raw_code,
            header.rom_size.decoded_bytes
        )));
    }

    if actual_rom_size != MBC6_SUPPORTED_ROM_BYTES {
        return Err(ctx.reject(format!(
            "{} expects a {}-byte image, but the loaded ROM is {} bytes",
            ctx.name(),
            MBC6_SUPPORTED_ROM_BYTES,
            actual_rom_size
        )));
    }

    if header.ram_size.raw_code != 0x03
        || header.ram_size.decoded_bytes != Some(MBC6_SUPPORTED_RAM_BYTES)
    {
        return Err(ctx.reject(format!(
            "{} expects the official 32 KiB SRAM declaration (code 0x03), but the header declared code {:#04X} ({:?} bytes)",
            ctx.name(),
            header.ram_size.raw_code,
            header.ram_size.decoded_bytes
        )));
    }

    Ok(())
}

pub(in crate::cartridge) fn validate_mbc7(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<(), CartridgeLoadError> {
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

    if declared_rom_bytes > MBC7_SUPPORTED_ROM_BYTES_MAX {
        return Err(ctx.reject(format!(
            "{} exceeds the current MBC7 ROM limit of {} bytes with {} bytes",
            ctx.name(),
            MBC7_SUPPORTED_ROM_BYTES_MAX,
            declared_rom_bytes
        )));
    }

    if !header.cgb_flag.enables_cgb_native_mode() {
        return Err(ctx.reject(format!(
            "{} requires a CGB-capable header flag, but the header decoded as {:?}",
            ctx.name(),
            header.cgb_flag
        )));
    }

    if header.ram_size.raw_code != 0x00 || header.ram_size.decoded_bytes != Some(0) {
        return Err(ctx.reject(format!(
            "{} uses a fixed 256-byte serial EEPROM instead of decoded SRAM, but the header declared RAM code {:#04X} ({:?} bytes)",
            ctx.name(),
            header.ram_size.raw_code,
            header.ram_size.decoded_bytes
        )));
    }

    Ok(())
}

pub(in crate::cartridge) fn validate_pocket_camera(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<(), CartridgeLoadError> {
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

    if header.rom_size.raw_code != 0x05 || declared_rom_bytes != POCKET_CAMERA_SUPPORTED_ROM_BYTES {
        return Err(ctx.reject(format!(
            "{} expects the official 1 MiB ROM declaration (code 0x05), but the header declared code {:#04X} ({:?} bytes)",
            ctx.name(),
            header.rom_size.raw_code,
            header.rom_size.decoded_bytes
        )));
    }

    if actual_rom_size != POCKET_CAMERA_SUPPORTED_ROM_BYTES {
        return Err(ctx.reject(format!(
            "{} expects a {}-byte image, but the loaded ROM is {} bytes",
            ctx.name(),
            POCKET_CAMERA_SUPPORTED_ROM_BYTES,
            actual_rom_size
        )));
    }

    if header.ram_size.raw_code != 0x04
        || header.ram_size.decoded_bytes != Some(POCKET_CAMERA_SUPPORTED_RAM_BYTES)
    {
        return Err(ctx.reject(format!(
            "{} expects the official 128 KiB RAM declaration (code 0x04), but the header declared code {:#04X} ({:?} bytes)",
            ctx.name(),
            header.ram_size.raw_code,
            header.ram_size.decoded_bytes
        )));
    }

    Ok(())
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

const fn no_mbc_ram_baseline_description(expected_ram_code: u8) -> &'static str {
    match expected_ram_code {
        0x00 => "No MBC baseline with no external RAM",
        0x02 => "No MBC baseline with 8 KiB linear external RAM",
        _ => "unknown No MBC RAM baseline",
    }
}
