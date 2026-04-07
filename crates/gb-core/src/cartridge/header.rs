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
        if rom_bytes.len() < HEADER_MINIMUM_ROM_LEN {
            return Err(CartridgeHeaderParseError::ImageTooSmall {
                actual_size: rom_bytes.len(),
                minimum_size: HEADER_MINIMUM_ROM_LEN,
            });
        }

        let mut entry_point = [0; ENTRY_POINT_LEN];
        entry_point
            .copy_from_slice(&rom_bytes[ENTRY_POINT_START..ENTRY_POINT_START + ENTRY_POINT_LEN]);

        let mut nintendo_logo = [0; NINTENDO_LOGO_LEN];
        nintendo_logo.copy_from_slice(
            &rom_bytes[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN],
        );

        let mut title_bytes = [0; TITLE_BYTES_LEN];
        title_bytes.copy_from_slice(&rom_bytes[TITLE_START..=TITLE_END_INCLUSIVE]);

        let mut raw_title_suffix_or_manufacturer_code = [0; MANUFACTURER_CODE_LEN];
        raw_title_suffix_or_manufacturer_code
            .copy_from_slice(&rom_bytes[MANUFACTURER_CODE_START..=MANUFACTURER_CODE_END_INCLUSIVE]);

        let mut new_licensee_code = [0; NEW_LICENSEE_CODE_LEN];
        new_licensee_code.copy_from_slice(
            &rom_bytes[NEW_LICENSEE_CODE_START..NEW_LICENSEE_CODE_START + NEW_LICENSEE_CODE_LEN],
        );

        let raw_cgb_flag = rom_bytes[CGB_FLAG_ADDRESS];
        let cgb_flag = decode_cgb_flag(raw_cgb_flag);
        let old_licensee_code = rom_bytes[OLD_LICENSEE_CODE_ADDRESS];
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
            sgb_flag: decode_sgb_flag(rom_bytes[SGB_FLAG_ADDRESS]),
            cartridge_type: rom_bytes[CARTRIDGE_TYPE_ADDRESS],
            rom_size: RomSizeInfo::decode(rom_bytes[ROM_SIZE_ADDRESS]),
            ram_size: RamSizeInfo::decode(rom_bytes[RAM_SIZE_ADDRESS]),
            new_licensee_code,
            destination_code: rom_bytes[DESTINATION_CODE_ADDRESS],
            old_licensee_code,
            header_checksum: rom_bytes[HEADER_CHECKSUM_ADDRESS],
        })
    }
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
