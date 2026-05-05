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
            0xFC => supported(
                raw_type,
                "POCKET CAMERA",
                SupportedCartridgeFamily::PocketCamera,
            ),
            0x0B => supported(raw_type, "MMM01", SupportedCartridgeFamily::Mmm01),
            0x0C => supported(raw_type, "MMM01+RAM", SupportedCartridgeFamily::Mmm01),
            0x0D => supported(
                raw_type,
                "MMM01+RAM+BATTERY",
                SupportedCartridgeFamily::Mmm01,
            ),
            0x20 => supported(raw_type, "MBC6", SupportedCartridgeFamily::Mbc6),
            0x22 => unsupported(
                raw_type,
                "MBC7+SENSOR+RUMBLE+RAM+BATTERY",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MBC7 requires EEPROM and accelerometer behavior that is not implemented yet",
            ),
            0xFD => unsupported(
                raw_type,
                "BANDAI TAMA5",
                UnsupportedCartridgeCategory::AccessorySpecialCase,
                "Bandai TAMA5 needs dedicated accessory hardware",
            ),
            0xFE => supported(raw_type, "HuC-3", SupportedCartridgeFamily::Huc3),
            0xFF => supported(raw_type, "HuC1+RAM+BATTERY", SupportedCartridgeFamily::Huc1),
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
    if let Some(classification) = classify_supported_signature_variant(header, rom_bytes) {
        return classification;
    }

    if compatibility.heuristic_policy == crate::model::HeuristicPolicy::AllowExperimental
        && let Some(classification) = classify_experimental_heuristic(header, rom_bytes)
    {
        return classification;
    }

    CartridgeClassification::classify(header.cartridge_type)
}

fn classify_supported_signature_variant(
    header: &CartridgeHeader,
    rom_bytes: &[u8],
) -> Option<CartridgeClassification> {
    if is_m161_multicart_signature(header, rom_bytes) {
        return Some(supported_with_reason(
            header.cartridge_type,
            "M161",
            SupportedCartridgeFamily::M161,
            "M161 multicart classification came from the explicit Mani 4-in-1 signature path",
        ));
    }

    if is_mani_mmm01_signature(header, rom_bytes) {
        return Some(supported_with_reason(
            header.cartridge_type,
            "MMM01",
            SupportedCartridgeFamily::Mmm01,
            "MMM01 classification came from the explicit later Mani trailing-menu signature path",
        ));
    }

    if is_mbc1m_multicart_signature(header, rom_bytes) {
        return Some(supported_with_reason(
            header.cartridge_type,
            "MBC1M",
            SupportedCartridgeFamily::Mbc1,
            "MBC1 multicart classification came from the explicit subheader signature path",
        ));
    }

    if is_mbc30_header_variant(header) {
        return Some(supported_with_reason(
            header.cartridge_type,
            "MBC30",
            SupportedCartridgeFamily::Mbc3,
            "MBC30 classification came from the MBC3 64 KiB SRAM header shape",
        ));
    }

    None
}

fn is_mbc30_header_variant(header: &CartridgeHeader) -> bool {
    matches!(header.cartridge_type, 0x10 | 0x12 | 0x13) && header.ram_size.raw_code == 0x05
}
fn classify_experimental_heuristic(
    header: &CartridgeHeader,
    rom_bytes: &[u8],
) -> Option<CartridgeClassification> {
    if header.cartridge_type == 0xBE {
        return Some(unsupported(
            header.cartridge_type,
            "BUNG",
            UnsupportedCartridgeCategory::ExperimentalHeuristic,
            "Bung multicart classification came from an explicit experimental heuristic path",
        ));
    }

    if is_ems_multicart_signature(
        &header.title_bytes,
        header.cartridge_type,
        header.destination_code,
    ) {
        return Some(unsupported(
            header.cartridge_type,
            "EMS",
            UnsupportedCartridgeCategory::ExperimentalHeuristic,
            "EMS multicart classification came from an explicit experimental heuristic path",
        ));
    }

    if is_wisdom_tree_signature(
        &header.title_bytes,
        header.cartridge_type,
        header.rom_size.raw_code,
        rom_bytes.len(),
        header.destination_code,
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
    if !matches!(header.cartridge_type, 0x01..=0x03) {
        return false;
    }
    if header.rom_size.decoded_bytes != Some(1024 * 1024) || rom_bytes.len() != 1024 * 1024 {
        return false;
    }

    [0x10usize, 0x20, 0x30]
        .into_iter()
        .filter(|bank| {
            let start = bank * 0x4000 + NINTENDO_LOGO_START;
            let end = start + NINTENDO_LOGO_LEN;
            rom_bytes.get(start..end) == Some(header.nintendo_logo.as_slice())
        })
        .count()
        >= 2
}

fn is_m161_multicart_signature(header: &CartridgeHeader, rom_bytes: &[u8]) -> bool {
    if rom_bytes.len() < M161_SUPPORTED_ROM_BYTES_MIN
        || rom_bytes.len() > M161_SUPPORTED_ROM_BYTES_MAX
        || !rom_bytes.len().is_multiple_of(M161_BANK_BYTES)
    {
        return false;
    }

    if !matches_padded_title(&header.title_bytes, M161_SYNTHETIC_MENU_TITLE)
        && !matches_padded_title(&header.title_bytes, M161_COMMERCIAL_MENU_TITLE)
    {
        return false;
    }

    if header.ram_size.raw_code != 0x00 {
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

fn is_mani_mmm01_signature(header: &CartridgeHeader, rom_bytes: &[u8]) -> bool {
    if header.cartridge_type != MANI_MMM01_MENU_TYPE
        || header.ram_size.raw_code != 0x00
        || header.ram_size.decoded_bytes != Some(0)
        || header.rom_size.decoded_bytes != Some(rom_bytes.len())
        || !header.title.ends_with(MANI_MMM01_MENU_SUFFIX)
        || !MANI_MMM01_SUPPORTED_ROM_BYTES.contains(&rom_bytes.len())
    {
        return false;
    }

    let Ok(primary_header) = CartridgeHeader::parse(rom_bytes) else {
        return false;
    };
    if !matches!(primary_header.cartridge_type, 0x01..=0x03)
        || primary_header.ram_size.raw_code != 0x00
        || primary_header.nintendo_logo != header.nintendo_logo
    {
        return false;
    }

    let mut game_header_count = 0;
    for base_offset in (0..rom_bytes.len()).step_by(0x4000) {
        let Ok(candidate) = CartridgeHeader::parse_at_offset(rom_bytes, base_offset) else {
            continue;
        };
        let Some(subrom_bytes) = candidate.rom_size.decoded_bytes else {
            continue;
        };

        if candidate.nintendo_logo != header.nintendo_logo
            || !matches!(candidate.cartridge_type, 0x00..=0x03)
            || candidate.ram_size.raw_code != 0x00
            || subrom_bytes >= rom_bytes.len()
            || candidate.title.is_empty()
            || candidate.title.ends_with(MANI_MMM01_MENU_SUFFIX)
        {
            continue;
        }

        game_header_count += 1;
    }

    game_header_count >= 4
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

pub(in crate::cartridge) fn matches_padded_title(title_bytes: &[u8], expected: &[u8]) -> bool {
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
