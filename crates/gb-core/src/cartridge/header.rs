use super::*;

impl RomSizeInfo {
    pub const fn decode(raw_code: u8) -> Self {
        match raw_code {
            0x00..=0x08 => {
                let bank_count = 2usize << raw_code;
                Self {
                    raw_code,
                    decoded_bytes: Some(16 * 1024 * bank_count),
                    bank_count: Some(bank_count),
                }
            }
            0x52 => Self {
                raw_code,
                decoded_bytes: Some(72 * 16 * 1024),
                bank_count: Some(72),
            },
            0x53 => Self {
                raw_code,
                decoded_bytes: Some(80 * 16 * 1024),
                bank_count: Some(80),
            },
            0x54 => Self {
                raw_code,
                decoded_bytes: Some(96 * 16 * 1024),
                bank_count: Some(96),
            },
            _ => Self {
                raw_code,
                decoded_bytes: None,
                bank_count: None,
            },
        }
    }
}

impl RamSizeInfo {
    pub const fn decode(raw_code: u8) -> Self {
        match raw_code {
            0x00 => Self {
                raw_code,
                decoded_bytes: Some(0),
                bank_count: Some(0),
            },
            0x01 => Self {
                raw_code,
                decoded_bytes: Some(2 * 1024),
                bank_count: Some(1),
            },
            0x02 => Self {
                raw_code,
                decoded_bytes: Some(8 * 1024),
                bank_count: Some(1),
            },
            0x03 => Self {
                raw_code,
                decoded_bytes: Some(32 * 1024),
                bank_count: Some(4),
            },
            0x04 => Self {
                raw_code,
                decoded_bytes: Some(128 * 1024),
                bank_count: Some(16),
            },
            0x05 => Self {
                raw_code,
                decoded_bytes: Some(64 * 1024),
                bank_count: Some(8),
            },
            _ => Self {
                raw_code,
                decoded_bytes: None,
                bank_count: None,
            },
        }
    }
}

impl CartridgeHeader {
    pub fn parse(rom_bytes: &[u8]) -> Result<Self, CartridgeHeaderParseError> {
        Self::parse_at_offset(rom_bytes, 0)
    }

    pub(in crate::cartridge) fn parse_for_load(
        rom_bytes: &[u8],
    ) -> Result<Self, CartridgeHeaderParseError> {
        let primary = Self::parse(rom_bytes)?;

        if let Some(menu_header) = Self::parse_mmm01_menu_header(rom_bytes) {
            return Ok(menu_header);
        }

        Ok(Self::parse_mani_mmm01_menu_header(rom_bytes, &primary).unwrap_or(primary))
    }

    pub(in crate::cartridge) fn parse_at_offset(
        rom_bytes: &[u8],
        base_offset: usize,
    ) -> Result<Self, CartridgeHeaderParseError> {
        let minimum_size = base_offset + HEADER_MINIMUM_ROM_LEN;
        if rom_bytes.len() < minimum_size {
            return Err(CartridgeHeaderParseError::ImageTooSmall {
                actual_size: rom_bytes.len(),
                minimum_size,
            });
        }

        let entry_point_start = base_offset + ENTRY_POINT_START;
        let logo_start = base_offset + NINTENDO_LOGO_START;
        let title_start = base_offset + TITLE_START;
        let manufacturer_code_start = base_offset + MANUFACTURER_CODE_START;
        let new_licensee_code_start = base_offset + NEW_LICENSEE_CODE_START;
        let cgb_flag_address = base_offset + CGB_FLAG_ADDRESS;
        let sgb_flag_address = base_offset + SGB_FLAG_ADDRESS;
        let cartridge_type_address = base_offset + CARTRIDGE_TYPE_ADDRESS;
        let rom_size_address = base_offset + ROM_SIZE_ADDRESS;
        let ram_size_address = base_offset + RAM_SIZE_ADDRESS;
        let destination_code_address = base_offset + DESTINATION_CODE_ADDRESS;
        let old_licensee_code_address = base_offset + OLD_LICENSEE_CODE_ADDRESS;
        let header_checksum_address = base_offset + HEADER_CHECKSUM_ADDRESS;

        let mut entry_point = [0; ENTRY_POINT_LEN];
        entry_point
            .copy_from_slice(&rom_bytes[entry_point_start..entry_point_start + ENTRY_POINT_LEN]);

        let mut nintendo_logo = [0; NINTENDO_LOGO_LEN];
        nintendo_logo.copy_from_slice(&rom_bytes[logo_start..logo_start + NINTENDO_LOGO_LEN]);

        let mut title_bytes = [0; TITLE_BYTES_LEN];
        title_bytes.copy_from_slice(&rom_bytes[title_start..title_start + TITLE_BYTES_LEN]);

        let mut raw_title_suffix_or_manufacturer_code = [0; MANUFACTURER_CODE_LEN];
        raw_title_suffix_or_manufacturer_code.copy_from_slice(
            &rom_bytes[manufacturer_code_start..manufacturer_code_start + MANUFACTURER_CODE_LEN],
        );

        let mut new_licensee_code = [0; NEW_LICENSEE_CODE_LEN];
        new_licensee_code.copy_from_slice(
            &rom_bytes[new_licensee_code_start..new_licensee_code_start + NEW_LICENSEE_CODE_LEN],
        );

        let raw_cgb_flag = rom_bytes[cgb_flag_address];
        let cgb_flag = decode_cgb_flag(raw_cgb_flag);
        let old_licensee_code = rom_bytes[old_licensee_code_address];
        let visible_title_bytes = if uses_cgb_title_layout(raw_cgb_flag) {
            // Pan Docs documents both 15-character and 11-character title
            // layouts for newer cartridges, but the raw header does not expose
            // a reliable discriminator for "manufacturer code is really active"
            // versus "these four bytes are still just part of a 15-character
            // title". Keep the visible title conservative at 15 characters when
            // 0x0143 has bit 7 set, and preserve 0x013F-0x0142 separately so
            // later compatibility work can reason about those bytes without
            // silently truncating valid CGB-era titles.
            &title_bytes[..TITLE_BYTES_LEN - 1]
        } else {
            &title_bytes[..]
        };
        let title_len = visible_title_bytes
            .iter()
            .position(|&byte| byte == 0 || byte == 0xFF)
            .unwrap_or(visible_title_bytes.len());
        let title = String::from_utf8_lossy(&visible_title_bytes[..title_len]).to_string();

        Ok(Self {
            entry_point,
            nintendo_logo,
            title_bytes,
            raw_title_suffix_or_manufacturer_code,
            title,
            cgb_flag,
            sgb_flag: decode_sgb_flag(rom_bytes[sgb_flag_address]),
            cartridge_type: rom_bytes[cartridge_type_address],
            rom_size: RomSizeInfo::decode(rom_bytes[rom_size_address]),
            ram_size: RamSizeInfo::decode(rom_bytes[ram_size_address]),
            new_licensee_code,
            destination_code: rom_bytes[destination_code_address],
            old_licensee_code,
            header_checksum: rom_bytes[header_checksum_address],
        })
    }

    fn parse_mmm01_menu_header(rom_bytes: &[u8]) -> Option<Self> {
        if rom_bytes.len() < MMM01_MIN_ROM_BYTES {
            return None;
        }

        let base_offset = rom_bytes.len().checked_sub(MMM01_MENU_BYTES)?;
        if base_offset == 0 {
            return None;
        }

        let candidate = Self::parse_at_offset(rom_bytes, base_offset).ok()?;
        if !matches!(candidate.cartridge_type, 0x0B..=0x0D) {
            return None;
        }
        if candidate.rom_size.decoded_bytes != Some(rom_bytes.len()) {
            return None;
        }
        if candidate.title.is_empty() {
            return None;
        }
        if count_standard_mmm01_subheaders(rom_bytes, &candidate.nintendo_logo) < 2 {
            return None;
        }

        Some(candidate)
    }

    fn parse_mani_mmm01_menu_header(rom_bytes: &[u8], primary: &Self) -> Option<Self> {
        if !matches!(primary.cartridge_type, 0x01..=0x03)
            || primary.ram_size.raw_code != 0x00
            || !MANI_MMM01_SUPPORTED_ROM_BYTES.contains(&rom_bytes.len())
        {
            return None;
        }

        let base_offset = rom_bytes.len().checked_sub(MMM01_MENU_BYTES)?;
        if base_offset == 0 {
            return None;
        }

        let candidate = Self::parse_at_offset(rom_bytes, base_offset).ok()?;
        if candidate.cartridge_type != MANI_MMM01_MENU_TYPE
            || candidate.ram_size.raw_code != 0x00
            || candidate.ram_size.decoded_bytes != Some(0)
            || candidate.rom_size.decoded_bytes != Some(rom_bytes.len())
            || !candidate.title.ends_with(MANI_MMM01_MENU_SUFFIX)
            || candidate.nintendo_logo != primary.nintendo_logo
        {
            return None;
        }

        (count_mani_mmm01_subheaders(rom_bytes, &candidate.nintendo_logo) >= 4).then_some(candidate)
    }
}

fn count_mani_mmm01_subheaders(rom_bytes: &[u8], expected_logo: &[u8; NINTENDO_LOGO_LEN]) -> usize {
    let mut match_count = 0;

    for base_offset in (0..rom_bytes.len()).step_by(0x4000) {
        let Ok(candidate) = CartridgeHeader::parse_at_offset(rom_bytes, base_offset) else {
            continue;
        };

        let Some(subrom_bytes) = candidate.rom_size.decoded_bytes else {
            continue;
        };

        if candidate.nintendo_logo != *expected_logo
            || !matches!(candidate.cartridge_type, 0x00..=0x03)
            || candidate.ram_size.raw_code != 0x00
            || subrom_bytes >= rom_bytes.len()
            || candidate.title.is_empty()
            || candidate.title.ends_with(MANI_MMM01_MENU_SUFFIX)
        {
            continue;
        }

        match_count += 1;
    }

    match_count
}

fn count_standard_mmm01_subheaders(
    rom_bytes: &[u8],
    expected_logo: &[u8; NINTENDO_LOGO_LEN],
) -> usize {
    let Some(menu_offset) = rom_bytes.len().checked_sub(MMM01_MENU_BYTES) else {
        return 0;
    };

    let mut match_count = 0;
    for base_offset in (0..menu_offset).step_by(0x4000) {
        let Ok(candidate) = CartridgeHeader::parse_at_offset(rom_bytes, base_offset) else {
            continue;
        };
        let Some(subrom_bytes) = candidate.rom_size.decoded_bytes else {
            continue;
        };

        if candidate.nintendo_logo != *expected_logo
            || matches!(candidate.cartridge_type, 0x0B..=0x0D)
            || subrom_bytes >= rom_bytes.len()
            || candidate.title.is_empty()
        {
            continue;
        }

        match_count += 1;
    }

    match_count
}

const fn uses_cgb_title_layout(raw_flag: u8) -> bool {
    raw_flag & 0x80 != 0
}

pub(in crate::cartridge) const fn decode_cgb_flag(raw_flag: u8) -> CgbFlag {
    match raw_flag {
        0x00 => CgbFlag::None,
        0x80 => CgbFlag::Supported,
        0xC0 => CgbFlag::Only,
        other if uses_cgb_title_layout(other) => CgbFlag::SupportedNonCanonical(other),
        other => CgbFlag::Unknown(other),
    }
}

pub(in crate::cartridge) const fn decode_sgb_flag(raw_flag: u8) -> SgbFlag {
    match raw_flag {
        0x00 => SgbFlag::None,
        0x03 => SgbFlag::Supported,
        other => SgbFlag::Unknown(other),
    }
}
