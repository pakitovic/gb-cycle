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

        let title_bytes = &rom_bytes[TITLE_START..TITLE_END_INCLUSIVE];
        let title_len = title_bytes
            .iter()
            .position(|&byte| byte == 0 || byte == 0xFF)
            .unwrap_or(title_bytes.len());
        let title = String::from_utf8_lossy(&title_bytes[..title_len]).to_string();

        Ok(Self {
            entry_point,
            nintendo_logo,
            title,
            cgb_flag: decode_cgb_flag(rom_bytes[CGB_FLAG_ADDRESS]),
            sgb_flag: decode_sgb_flag(rom_bytes[SGB_FLAG_ADDRESS]),
            cartridge_type: rom_bytes[CARTRIDGE_TYPE_ADDRESS],
            rom_size: RomSizeInfo::decode(rom_bytes[ROM_SIZE_ADDRESS]),
            ram_size: RamSizeInfo::decode(rom_bytes[RAM_SIZE_ADDRESS]),
            header_checksum: rom_bytes[HEADER_CHECKSUM_ADDRESS],
        })
    }
}

pub(in crate::cartridge) const fn decode_cgb_flag(raw_flag: u8) -> CgbFlag {
    match raw_flag {
        0x00 => CgbFlag::None,
        0x80 => CgbFlag::Supported,
        0xC0 => CgbFlag::Only,
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
