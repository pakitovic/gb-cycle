use super::*;

pub(in crate::sgb) struct SgbBootPaletteAssignment {
    title: &'static [u8],
    palette_index: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::sgb) enum SgbBootPaletteSelection {
    Default,
    TitleMatch { palette_index: u8 },
}

impl SgbBootPaletteSelection {
    pub(in crate::sgb) const fn palette_index(self) -> u8 {
        match self {
            Self::Default => SGB_BOOT_PALETTE_DEFAULT_INDEX,
            Self::TitleMatch { palette_index } => palette_index,
        }
    }
}

// SGB BIOS built-in palettes used by the host boot program before command-driven game palettes take over.
pub(in crate::sgb) const SGB_BOOT_PALETTES: [SgbScreenPalette; SGB_BOOT_PALETTE_COUNT] = [
    sgb_screen_palette([0x67BF, 0x265B, 0x10B5, 0x2866]),
    sgb_screen_palette([0x637B, 0x3AD9, 0x0956, 0x0000]),
    sgb_screen_palette([0x7F1F, 0x2A7D, 0x30F3, 0x4CE7]),
    sgb_screen_palette([0x57FF, 0x2618, 0x001F, 0x006A]),
    sgb_screen_palette([0x5B7F, 0x3F0F, 0x222D, 0x10EB]),
    sgb_screen_palette([0x7FBB, 0x2A3C, 0x0015, 0x0900]),
    sgb_screen_palette([0x2800, 0x7680, 0x01EF, 0x2FFF]),
    sgb_screen_palette([0x73BF, 0x46FF, 0x0110, 0x0066]),
    sgb_screen_palette([0x533E, 0x2638, 0x01E5, 0x0000]),
    sgb_screen_palette([0x7FFF, 0x2BBF, 0x00DF, 0x2C0A]),
    sgb_screen_palette([0x7F1F, 0x463D, 0x74CF, 0x4CA5]),
    sgb_screen_palette([0x53FF, 0x03E0, 0x00DF, 0x2800]),
    sgb_screen_palette([0x433F, 0x72D2, 0x3045, 0x0822]),
    sgb_screen_palette([0x7FFA, 0x2A5F, 0x0014, 0x0003]),
    sgb_screen_palette([0x1EED, 0x215C, 0x42FC, 0x0060]),
    sgb_screen_palette([0x7FFF, 0x5EF7, 0x39CE, 0x0000]),
    sgb_screen_palette([0x4F5F, 0x630E, 0x159F, 0x3126]),
    sgb_screen_palette([0x637B, 0x121C, 0x0140, 0x0840]),
    sgb_screen_palette([0x66BC, 0x3FFF, 0x7EE0, 0x2C84]),
    sgb_screen_palette([0x5FFE, 0x3EBC, 0x0321, 0x0000]),
    sgb_screen_palette([0x63FF, 0x36DC, 0x11F6, 0x392A]),
    sgb_screen_palette([0x65EF, 0x7DBF, 0x035F, 0x2108]),
    sgb_screen_palette([0x2B6C, 0x7FFF, 0x1CD9, 0x0007]),
    sgb_screen_palette([0x53FC, 0x1F2F, 0x0E29, 0x0061]),
    sgb_screen_palette([0x36BE, 0x7EAF, 0x681A, 0x3C00]),
    sgb_screen_palette([0x7BBE, 0x329D, 0x1DE8, 0x0423]),
    sgb_screen_palette([0x739F, 0x6A9B, 0x7293, 0x0001]),
    sgb_screen_palette([0x5FFF, 0x6732, 0x3DA9, 0x2481]),
    sgb_screen_palette([0x577F, 0x3EBC, 0x456F, 0x1880]),
    sgb_screen_palette([0x6B57, 0x6E1B, 0x5010, 0x0007]),
    sgb_screen_palette([0x0F96, 0x2C97, 0x0045, 0x3200]),
    sgb_screen_palette([0x67FF, 0x2F17, 0x2230, 0x1548]),
];

// Exact NUL-padded raw header titles that the SGB BIOS maps to built-in palettes for DMG software that does not unlock SGB commands.
pub(in crate::sgb) const SGB_BOOT_TITLE_PALETTE_ASSIGNMENTS: [SgbBootPaletteAssignment; 26] = [
    SgbBootPaletteAssignment {
        title: b"ZELDA",
        palette_index: 5,
    },
    SgbBootPaletteAssignment {
        title: b"SUPER MARIOLAND",
        palette_index: 6,
    },
    SgbBootPaletteAssignment {
        title: b"MARIOLAND2",
        palette_index: 0x14,
    },
    SgbBootPaletteAssignment {
        title: b"SUPERMARIOLAND3",
        palette_index: 2,
    },
    SgbBootPaletteAssignment {
        title: b"KIRBY DREAM LAND",
        palette_index: 0x0B,
    },
    SgbBootPaletteAssignment {
        title: b"HOSHINOKA-BI",
        palette_index: 0x0B,
    },
    SgbBootPaletteAssignment {
        title: b"KIRBY'S PINBALL",
        palette_index: 3,
    },
    SgbBootPaletteAssignment {
        title: b"YOSSY NO TAMAGO",
        palette_index: 0x0C,
    },
    SgbBootPaletteAssignment {
        title: b"MARIO & YOSHI",
        palette_index: 0x0C,
    },
    SgbBootPaletteAssignment {
        title: b"YOSSY NO COOKIE",
        palette_index: 4,
    },
    SgbBootPaletteAssignment {
        title: b"YOSHI'S COOKIE",
        palette_index: 4,
    },
    SgbBootPaletteAssignment {
        title: b"DR.MARIO",
        palette_index: 0x12,
    },
    SgbBootPaletteAssignment {
        title: b"TETRIS",
        palette_index: 0x11,
    },
    SgbBootPaletteAssignment {
        title: b"YAKUMAN",
        palette_index: 0x13,
    },
    SgbBootPaletteAssignment {
        title: b"METROID2",
        palette_index: 0x1F,
    },
    SgbBootPaletteAssignment {
        title: b"KAERUNOTAMENI",
        palette_index: 9,
    },
    SgbBootPaletteAssignment {
        title: b"GOLF",
        palette_index: 0x18,
    },
    SgbBootPaletteAssignment {
        title: b"ALLEY WAY",
        palette_index: 0x16,
    },
    SgbBootPaletteAssignment {
        title: b"BASEBALL",
        palette_index: 0x0F,
    },
    SgbBootPaletteAssignment {
        title: b"TENNIS",
        palette_index: 0x17,
    },
    SgbBootPaletteAssignment {
        title: b"F1RACE",
        palette_index: 0x1E,
    },
    SgbBootPaletteAssignment {
        title: b"KID ICARUS",
        palette_index: 0x0E,
    },
    SgbBootPaletteAssignment {
        title: b"QIX",
        palette_index: 0x19,
    },
    SgbBootPaletteAssignment {
        title: b"SOLARSTRIKER",
        palette_index: 7,
    },
    SgbBootPaletteAssignment {
        title: b"X",
        palette_index: 0x1C,
    },
    SgbBootPaletteAssignment {
        title: b"GBWARS",
        palette_index: 0x15,
    },
];

pub(in crate::sgb) const fn sgb_screen_palette(
    raw_colors: [u16; SGB_SCREEN_PALETTE_COLORS],
) -> SgbScreenPalette {
    SgbScreenPalette {
        colors: [
            SgbRgb555Color::new(raw_colors[0]),
            SgbRgb555Color::new(raw_colors[1]),
            SgbRgb555Color::new(raw_colors[2]),
            SgbRgb555Color::new(raw_colors[3]),
        ],
    }
}

pub(in crate::sgb) fn sgb_boot_palette(palette_index: u8) -> SgbScreenPalette {
    let table_index = usize::from(palette_index.saturating_sub(1));
    SGB_BOOT_PALETTES
        .get(table_index)
        .copied()
        .unwrap_or(SGB_BOOT_PALETTES[0])
}

pub(in crate::sgb) fn sgb_boot_palette_selection_for_header(
    header: Option<&CartridgeHeader>,
    command_acceptance: SgbCommandAcceptance,
) -> SgbBootPaletteSelection {
    let Some(header) = header else {
        return SgbBootPaletteSelection::Default;
    };
    if command_acceptance != SgbCommandAcceptance::RejectedByHeader {
        return SgbBootPaletteSelection::Default;
    }
    sgb_title_palette_index(&header.title_bytes)
        .map(|palette_index| SgbBootPaletteSelection::TitleMatch { palette_index })
        .unwrap_or(SgbBootPaletteSelection::Default)
}

pub(in crate::sgb) fn sgb_title_palette_index(title_bytes: &[u8; 16]) -> Option<u8> {
    SGB_BOOT_TITLE_PALETTE_ASSIGNMENTS
        .iter()
        .find(|assignment| sgb_title_bytes_match(title_bytes, assignment.title))
        .map(|assignment| assignment.palette_index)
}

pub(in crate::sgb) fn sgb_title_bytes_match(title_bytes: &[u8; 16], expected_title: &[u8]) -> bool {
    if expected_title.len() > title_bytes.len() {
        return false;
    }
    title_bytes.starts_with(expected_title)
        && title_bytes[expected_title.len()..]
            .iter()
            .all(|&byte| byte == 0)
}
