use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbScreenPalette {
    pub colors: [SgbRgb555Color; SGB_SCREEN_PALETTE_COLORS],
}

impl SgbScreenPalette {
    pub const fn dmg_grayscale() -> Self {
        Self {
            colors: [
                SGB_RGB555_WHITE,
                SGB_RGB555_LIGHT_GRAY,
                SGB_RGB555_DARK_GRAY,
                SGB_RGB555_BLACK,
            ],
        }
    }

    pub const fn color(self, color_index: u8) -> SgbRgb555Color {
        self.colors[(color_index & 0x03) as usize]
    }

    pub(in crate::sgb) fn set_color(&mut self, color_index: usize, color: SgbRgb555Color) {
        self.colors[color_index] = color;
    }
}

impl Default for SgbScreenPalette {
    fn default() -> Self {
        Self::dmg_grayscale()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbPaletteState {
    pub screen_palettes: [SgbScreenPalette; SGB_SCREEN_PALETTE_COUNT],
    pub base_palette_index: u8,
}

impl SgbPaletteState {
    pub(in crate::sgb) fn default_for_active_host(active: bool) -> Self {
        let mut state = Self::default();
        if active {
            state.apply_boot_palette(SgbBootPaletteSelection::Default);
        }
        state
    }

    pub const fn palette(self, palette_index: u8) -> SgbScreenPalette {
        self.screen_palettes[(palette_index & 0x03) as usize]
    }

    pub const fn map_lcd_shade(self, shade: u8) -> SgbRgb555Color {
        self.palette(self.base_palette_index).color(shade)
    }

    pub(in crate::sgb) fn apply_boot_palette(&mut self, selection: SgbBootPaletteSelection) {
        self.screen_palettes[0] = sgb_boot_palette(selection.palette_index());
        self.base_palette_index = 0;
    }

    pub(in crate::sgb) fn set_shared_color_zero(&mut self, color: SgbRgb555Color) {
        for palette in &mut self.screen_palettes {
            palette.set_color(0, color);
        }
    }

    pub(in crate::sgb) fn apply_direct_palette_command(
        &mut self,
        command_id: u8,
        bytes: &[u8; SGB_PACKET_BYTES],
    ) {
        let Some((first_palette, second_palette)) = direct_palette_command_pair(command_id) else {
            return;
        };

        let shared_color_zero = SgbRgb555Color::from_packet_bytes(bytes[1], bytes[2]);
        self.set_shared_color_zero(shared_color_zero);

        let first_palette_colors = [
            SgbRgb555Color::from_packet_bytes(bytes[3], bytes[4]),
            SgbRgb555Color::from_packet_bytes(bytes[5], bytes[6]),
            SgbRgb555Color::from_packet_bytes(bytes[7], bytes[8]),
        ];
        let second_palette_colors = [
            SgbRgb555Color::from_packet_bytes(bytes[9], bytes[10]),
            SgbRgb555Color::from_packet_bytes(bytes[11], bytes[12]),
            SgbRgb555Color::from_packet_bytes(bytes[13], bytes[14]),
        ];

        for (color_index, color) in first_palette_colors.into_iter().enumerate() {
            self.screen_palettes[first_palette].set_color(color_index + 1, color);
        }
        for (color_index, color) in second_palette_colors.into_iter().enumerate() {
            self.screen_palettes[second_palette].set_color(color_index + 1, color);
        }
    }
}

impl Default for SgbPaletteState {
    fn default() -> Self {
        Self {
            screen_palettes: [SgbScreenPalette::dmg_grayscale(); SGB_SCREEN_PALETTE_COUNT],
            base_palette_index: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbPlayerPaletteOverrideState {
    pub active: bool,
    pub palette_state: SgbPaletteState,
    pub attributes: SgbAttributeMap,
    pub activation_count: u64,
    pub manual_release_count: u64,
    pub pal_pri_release_count: u64,
}

impl SgbPlayerPaletteOverrideState {
    pub(in crate::sgb) fn set_uniform_palette(&mut self, palette: SgbScreenPalette) -> bool {
        let palette_state = SgbPaletteState {
            screen_palettes: [palette; SGB_SCREEN_PALETTE_COUNT],
            ..SgbPaletteState::default()
        };
        let attributes = SgbAttributeMap::default();
        let changed =
            !self.active || self.palette_state != palette_state || self.attributes != attributes;
        self.active = true;
        self.palette_state = palette_state;
        self.attributes = attributes;
        if changed {
            self.activation_count = self.activation_count.saturating_add(1);
        }
        changed
    }

    pub(in crate::sgb) fn clear_by_player(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.active = false;
        self.manual_release_count = self.manual_release_count.saturating_add(1);
        true
    }

    pub(in crate::sgb) fn return_to_application_due_to_pal_pri(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.active = false;
        self.pal_pri_release_count = self.pal_pri_release_count.saturating_add(1);
        true
    }

    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.attributes.dynamic_payload_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbSystemPaletteState {
    pub palettes: Vec<SgbScreenPalette>,
    pub loaded: bool,
    pub last_pal_set_ids: [u16; SGB_SCREEN_PALETTE_COUNT],
    pub pal_trn_count: u64,
    pub pal_set_count: u64,
    pub pal_pri_enabled: bool,
    pub pal_pri_command_count: u64,
}

impl SgbSystemPaletteState {
    pub(in crate::sgb) fn palette_wrapping(&self, palette_index: usize) -> SgbScreenPalette {
        self.palettes[palette_index % SGB_SYSTEM_PALETTE_COUNT]
    }

    pub(in crate::sgb) fn color_zero_for_last_pal_set(&self) -> Option<SgbRgb555Color> {
        self.palettes
            .get(usize::from(self.last_pal_set_ids[0]))
            .map(|palette| palette.color(0))
    }

    pub(in crate::sgb) fn apply_pal_trn(&mut self, payload: &SgbVramTransferBuffer) {
        for palette_index in 0..SGB_SYSTEM_PALETTE_COUNT {
            for color_index in 0..SGB_SCREEN_PALETTE_COLORS {
                let byte_index = palette_index * SGB_SCREEN_PALETTE_COLORS * 2 + color_index * 2;
                self.palettes[palette_index].colors[color_index] =
                    SgbRgb555Color::from_packet_bytes(
                        payload.bytes[byte_index],
                        payload.bytes[byte_index + 1],
                    );
            }
        }
        self.loaded = true;
        self.pal_trn_count = self.pal_trn_count.saturating_add(1);
    }

    pub(in crate::sgb) fn apply_pal_set(
        &mut self,
        palette_state: &mut SgbPaletteState,
        bytes: &[u8; SGB_PACKET_BYTES],
    ) -> SgbPalSetOptions {
        for palette_index in 0..SGB_SCREEN_PALETTE_COUNT {
            let byte_index = 1 + palette_index * 2;
            let palette_id = u16::from_le_bytes([bytes[byte_index], bytes[byte_index + 1]]);
            self.last_pal_set_ids[palette_index] = palette_id;
            palette_state.screen_palettes[palette_index] = self
                .palettes
                .get(usize::from(palette_id))
                .copied()
                .unwrap_or_default();
        }
        self.pal_set_count = self.pal_set_count.saturating_add(1);
        SgbPalSetOptions::from_flags(bytes[9])
    }

    pub(in crate::sgb) fn apply_pal_pri(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.pal_pri_enabled = bytes[1] & 0x01 != 0;
        self.pal_pri_command_count = self.pal_pri_command_count.saturating_add(1);
    }

    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.palettes
            .len()
            .saturating_mul(std::mem::size_of::<SgbScreenPalette>())
    }
}

impl Default for SgbSystemPaletteState {
    fn default() -> Self {
        Self {
            palettes: vec![SgbScreenPalette::dmg_grayscale(); SGB_SYSTEM_PALETTE_COUNT],
            loaded: false,
            last_pal_set_ids: [0; SGB_SCREEN_PALETTE_COUNT],
            pal_trn_count: 0,
            pal_set_count: 0,
            pal_pri_enabled: false,
            pal_pri_command_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbPalSetOptions {
    pub apply_atf: bool,
    pub cancel_mask: bool,
    pub atf_index: u8,
}

impl SgbPalSetOptions {
    pub(in crate::sgb) const fn from_flags(flags: u8) -> Self {
        Self {
            apply_atf: flags & 0x80 != 0,
            cancel_mask: flags & 0x40 != 0,
            atf_index: flags & 0x3F,
        }
    }
}
