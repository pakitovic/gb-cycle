use super::*;

impl CartridgeClassification {
    pub const fn classify(raw_type: u8) -> Self {
        match raw_type {
            0x00 => supported(raw_type, "ROM ONLY", SupportedCartridgeFamily::NoMbc),
            0x08 => supported(raw_type, "ROM+RAM", SupportedCartridgeFamily::NoMbc),
            0x09 => supported(raw_type, "ROM+RAM+BATTERY", SupportedCartridgeFamily::NoMbc),
            0x01 => supported(raw_type, "MBC1", SupportedCartridgeFamily::Mbc1),
            0x02 => supported(raw_type, "MBC1+RAM", SupportedCartridgeFamily::Mbc1),
            0x03 => supported(raw_type, "MBC1+RAM+BATTERY", SupportedCartridgeFamily::Mbc1),
            0x05 => supported(raw_type, "MBC2", SupportedCartridgeFamily::Mbc2),
            0x06 => supported(raw_type, "MBC2+BATTERY", SupportedCartridgeFamily::Mbc2),
            0x0F => supported(
                raw_type,
                "MBC3+TIMER+BATTERY",
                SupportedCartridgeFamily::Mbc3,
            ),
            0x10 => supported(
                raw_type,
                "MBC3+TIMER+RAM+BATTERY",
                SupportedCartridgeFamily::Mbc3,
            ),
            0x11 => supported(raw_type, "MBC3", SupportedCartridgeFamily::Mbc3),
            0x12 => supported(raw_type, "MBC3+RAM", SupportedCartridgeFamily::Mbc3),
            0x13 => supported(raw_type, "MBC3+RAM+BATTERY", SupportedCartridgeFamily::Mbc3),
            0x19 => supported(raw_type, "MBC5", SupportedCartridgeFamily::Mbc5),
            0x1A => supported(raw_type, "MBC5+RAM", SupportedCartridgeFamily::Mbc5),
            0x1B => supported(raw_type, "MBC5+RAM+BATTERY", SupportedCartridgeFamily::Mbc5),
            0x1C => supported(raw_type, "MBC5+RUMBLE", SupportedCartridgeFamily::Mbc5),
            0x1D => supported(raw_type, "MBC5+RUMBLE+RAM", SupportedCartridgeFamily::Mbc5),
            0x1E => supported(
                raw_type,
                "MBC5+RUMBLE+RAM+BATTERY",
                SupportedCartridgeFamily::Mbc5,
            ),
            0x0B => unsupported(
                raw_type,
                "MMM01",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MMM01 is a documented multicart family reserved for later support",
            ),
            0x0C => unsupported(
                raw_type,
                "MMM01+RAM",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MMM01 is a documented multicart family reserved for later support",
            ),
            0x0D => unsupported(
                raw_type,
                "MMM01+RAM+BATTERY",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MMM01 is a documented multicart family reserved for later support",
            ),
            0x20 => unsupported(
                raw_type,
                "MBC6",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MBC6 requires a dedicated cartridge-local implementation",
            ),
            0x22 => unsupported(
                raw_type,
                "MBC7+SENSOR+RUMBLE+RAM+BATTERY",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MBC7 requires EEPROM and accelerometer behavior that is not implemented yet",
            ),
            0xFC => unsupported(
                raw_type,
                "POCKET CAMERA",
                UnsupportedCartridgeCategory::AccessorySpecialCase,
                "Pocket Camera needs dedicated accessory hardware",
            ),
            0xFD => unsupported(
                raw_type,
                "BANDAI TAMA5",
                UnsupportedCartridgeCategory::AccessorySpecialCase,
                "Bandai TAMA5 needs dedicated accessory hardware",
            ),
            0xFE => unsupported(
                raw_type,
                "HuC-3",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "HuC-3 is a documented special cartridge with its own protocol",
            ),
            0xFF => unsupported(
                raw_type,
                "HuC1+RAM+BATTERY",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "HuC1 needs dedicated IR-capable cartridge behavior",
            ),
            _ => unsupported(
                raw_type,
                "UNKNOWN",
                UnsupportedCartridgeCategory::UnknownCode,
                "The cartridge type code is not recognized",
            ),
        }
    }

    pub const fn raw_type(self) -> u8 {
        self.raw_type
    }

    pub const fn detected_name(self) -> &'static str {
        self.detected_name
    }

    pub const fn selection(self) -> CartridgeSelection {
        self.selection
    }

    pub const fn reason(self) -> &'static str {
        self.reason
    }
}

pub(in crate::cartridge) fn classify_loaded_cartridge(
    header: &CartridgeHeader,
    rom_bytes: &[u8],
    compatibility: &crate::model::CompatibilityPolicy,
) -> CartridgeClassification {
    if let Some(classification) = classify_planned_variant(header) {
        return classification;
    }

    if let Some(classification) = classify_documented_special_variant(header, rom_bytes) {
        return classification;
    }

    if compatibility.heuristic_policy == crate::model::HeuristicPolicy::AllowExperimental
        && let Some(classification) = classify_experimental_heuristic(header, rom_bytes)
    {
        return classification;
    }

    CartridgeClassification::classify(header.cartridge_type)
}

fn classify_planned_variant(header: &CartridgeHeader) -> Option<CartridgeClassification> {
    match header.cartridge_type {
        0x0F..=0x13 if header.ram_size.raw_code == 0x05 => Some(unsupported(
            header.cartridge_type,
            "MBC30",
            UnsupportedCartridgeCategory::PlannedVariant,
            "MBC30 is a known MBC3-family variant reserved for later support",
        )),
        _ => None,
    }
}

fn classify_documented_special_variant(
    header: &CartridgeHeader,
    rom_bytes: &[u8],
) -> Option<CartridgeClassification> {
    if is_m161_multicart_signature(header, rom_bytes) {
        return Some(unsupported(
            header.cartridge_type,
            "M161",
            UnsupportedCartridgeCategory::DocumentedButUnsupported,
            "M161 multicart classification came from the explicit Mani 4-in-1 signature path",
        ));
    }

    None
}

fn classify_experimental_heuristic(
    header: &CartridgeHeader,
    rom_bytes: &[u8],
) -> Option<CartridgeClassification> {
    let title_bytes = &rom_bytes[TITLE_START..=TITLE_END_INCLUSIVE];
    let destination_code = rom_bytes
        .get(DESTINATION_CODE_ADDRESS)
        .copied()
        .unwrap_or(0x00);

    if is_mbc1m_multicart_signature(header, rom_bytes) {
        return Some(supported_with_reason(
            header.cartridge_type,
            "MBC1M",
            SupportedCartridgeFamily::Mbc1,
            "MBC1 multicart classification came from an explicit experimental heuristic path",
        ));
    }

    if header.cartridge_type == 0xBE {
        return Some(unsupported(
            header.cartridge_type,
            "BUNG",
            UnsupportedCartridgeCategory::ExperimentalHeuristic,
            "Bung multicart classification came from an explicit experimental heuristic path",
        ));
    }

    if is_ems_multicart_signature(title_bytes, header.cartridge_type, destination_code) {
        return Some(unsupported(
            header.cartridge_type,
            "EMS",
            UnsupportedCartridgeCategory::ExperimentalHeuristic,
            "EMS multicart classification came from an explicit experimental heuristic path",
        ));
    }

    if is_wisdom_tree_signature(
        title_bytes,
        header.cartridge_type,
        header.rom_size.raw_code,
        rom_bytes.len(),
        destination_code,
    ) {
        return Some(unsupported(
            header.cartridge_type,
            "WISDOM TREE",
            UnsupportedCartridgeCategory::ExperimentalHeuristic,
            "Wisdom Tree classification came from an explicit experimental heuristic path",
        ));
    }

    None
}

pub(in crate::cartridge) fn is_mbc1m_multicart_signature(
    header: &CartridgeHeader,
    rom_bytes: &[u8],
) -> bool {
    if header.cartridge_type != 0x01 {
        return false;
    }
    if header.rom_size.decoded_bytes != Some(1024 * 1024) || rom_bytes.len() != 1024 * 1024 {
        return false;
    }
    if header.ram_size.raw_code != 0x00 {
        return false;
    }

    [0x10usize, 0x20, 0x30].into_iter().all(|bank| {
        let start = bank * 0x4000 + NINTENDO_LOGO_START;
        let end = start + NINTENDO_LOGO_LEN;
        rom_bytes.get(start..end) == Some(header.nintendo_logo.as_slice())
    })
}

fn is_m161_multicart_signature(header: &CartridgeHeader, rom_bytes: &[u8]) -> bool {
    if rom_bytes.len() < M161_KNOWN_SUBTITLE_SET.len() * M161_BANK_BYTES
        || rom_bytes.len() > M161_SUPPORTED_ROM_BYTES_MAX
        || !rom_bytes.len().is_multiple_of(M161_BANK_BYTES)
    {
        return false;
    }

    let mut seen_titles = [false; M161_KNOWN_SUBTITLE_SET.len()];

    for bank_start in (0..rom_bytes.len()).step_by(M161_BANK_BYTES) {
        let bank_logo_start = bank_start + NINTENDO_LOGO_START;
        let bank_logo_end = bank_logo_start + NINTENDO_LOGO_LEN;
        let bank_title_start = bank_start + TITLE_START;
        let bank_title_end = bank_start + TITLE_END_INCLUSIVE;

        let Some(bank_logo) = rom_bytes.get(bank_logo_start..bank_logo_end) else {
            return false;
        };
        let Some(bank_title) = rom_bytes.get(bank_title_start..=bank_title_end) else {
            return false;
        };

        for (title_index, expected_title) in M161_KNOWN_SUBTITLE_SET.iter().enumerate() {
            if !matches_padded_title(bank_title, expected_title) {
                continue;
            }

            if seen_titles[title_index]
                || bank_logo != header.nintendo_logo.as_slice()
                || rom_bytes.get(bank_start + CARTRIDGE_TYPE_ADDRESS).copied() != Some(0x00)
                || rom_bytes.get(bank_start + ROM_SIZE_ADDRESS).copied() != Some(0x00)
                || rom_bytes.get(bank_start + RAM_SIZE_ADDRESS).copied() != Some(0x00)
            {
                return false;
            }

            seen_titles[title_index] = true;
        }
    }

    seen_titles.into_iter().all(|seen| seen)
}

fn is_ems_multicart_signature(title_bytes: &[u8], raw_type: u8, destination_code: u8) -> bool {
    matches_padded_title(title_bytes, b"EMSMENU")
        || matches_padded_title(title_bytes, b"GB16M")
        || (raw_type == 0x1B && destination_code == 0xE1)
}

pub(in crate::cartridge) fn is_wisdom_tree_signature(
    title_bytes: &[u8],
    raw_type: u8,
    rom_size_code: u8,
    actual_rom_size: usize,
    destination_code: u8,
) -> bool {
    matches_padded_title(title_bytes, b"WISDOM TREE")
        || matches_padded_title(title_bytes, b"WISDOM\0TREE")
        || (raw_type == 0x00
            && rom_size_code == 0x00
            && actual_rom_size > NO_MBC_SUPPORTED_ROM_BYTES)
        || (raw_type == 0xC0 && destination_code == 0xD1)
}

fn matches_padded_title(title_bytes: &[u8], expected: &[u8]) -> bool {
    if title_bytes.len() < expected.len() {
        return false;
    }

    title_bytes.starts_with(expected)
        && title_bytes[expected.len()..]
            .iter()
            .all(|&byte| byte == 0x00 || byte == 0xFF)
}

pub(in crate::cartridge) const fn supported(
    raw_type: u8,
    detected_name: &'static str,
    family: SupportedCartridgeFamily,
) -> CartridgeClassification {
    supported_with_reason(
        raw_type,
        detected_name,
        family,
        "supported cartridge family",
    )
}

const fn supported_with_reason(
    raw_type: u8,
    detected_name: &'static str,
    family: SupportedCartridgeFamily,
    reason: &'static str,
) -> CartridgeClassification {
    CartridgeClassification {
        raw_type,
        detected_name,
        selection: CartridgeSelection::Supported(family),
        reason,
    }
}

const fn unsupported(
    raw_type: u8,
    detected_name: &'static str,
    category: UnsupportedCartridgeCategory,
    reason: &'static str,
) -> CartridgeClassification {
    CartridgeClassification {
        raw_type,
        detected_name,
        selection: CartridgeSelection::Unsupported(category),
        reason,
    }
}

pub(in crate::cartridge) fn unsupported_load_reason(
    classification: CartridgeClassification,
    category: UnsupportedCartridgeCategory,
) -> String {
    match category {
        UnsupportedCartridgeCategory::PlannedVariant => format!(
            "{} is a known reserved variant that is not implemented yet: {}",
            classification.detected_name(),
            classification.reason()
        ),
        UnsupportedCartridgeCategory::ExperimentalHeuristic => format!(
            "{} was identified through an explicit experimental heuristic path and is not implemented: {}",
            classification.detected_name(),
            classification.reason()
        ),
        UnsupportedCartridgeCategory::DocumentedButUnsupported
        | UnsupportedCartridgeCategory::AccessorySpecialCase
        | UnsupportedCartridgeCategory::UnknownCode => format!(
            "{} ({category:?}) is not implemented: {}",
            classification.detected_name(),
            classification.reason()
        ),
    }
}
