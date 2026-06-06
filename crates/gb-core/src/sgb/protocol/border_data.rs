use super::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbBorderTileData {
    pub bytes: Vec<u8>,
}

impl SgbBorderTileData {
    pub(in crate::sgb) fn apply_chr_transfer(
        &mut self,
        selection: SgbChrTransferSelection,
        payload: &SgbVramTransferBuffer,
    ) {
        let offset = selection.destination_offset();
        self.bytes[offset..offset + SGB_VRAM_TRANSFER_BYTES].copy_from_slice(&payload.bytes);
    }

    pub(in crate::sgb) fn pixel_color_index(&self, tile_index: usize, x: usize, y: usize) -> u8 {
        let tile_index = tile_index % SGB_BORDER_TILE_COUNT;
        let x = x & 0x07;
        let y = y & 0x07;
        let tile_offset = tile_index * SGB_BORDER_TILE_BYTES;
        let row_offset = tile_offset + y * 2;
        let low_plane_01 = self.bytes[row_offset];
        let high_plane_01 = self.bytes[row_offset + 1];
        let low_plane_23 = self.bytes[row_offset + 16];
        let high_plane_23 = self.bytes[row_offset + 17];
        let bit = 7 - x;

        ((low_plane_01 >> bit) & 0x01)
            | (((high_plane_01 >> bit) & 0x01) << 1)
            | (((low_plane_23 >> bit) & 0x01) << 2)
            | (((high_plane_23 >> bit) & 0x01) << 3)
    }
}

impl Default for SgbBorderTileData {
    fn default() -> Self {
        Self {
            bytes: vec![0; SGB_BORDER_TILE_DATA_BYTES],
        }
    }
}

impl SgbBorderTileData {
    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.bytes.len()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbBorderMapEntry {
    pub raw: u16,
}

impl SgbBorderMapEntry {
    pub const fn new(raw: u16) -> Self {
        Self { raw }
    }

    pub(in crate::sgb) const fn tile_index(self) -> usize {
        (self.raw as usize) & 0x03FF
    }

    pub(in crate::sgb) const fn palette_index(self) -> usize {
        match (self.raw >> 10) & 0x07 {
            4 => 0,
            5 => 1,
            6 => 2,
            _ => 0,
        }
    }

    pub(in crate::sgb) const fn x_flip(self) -> bool {
        self.raw & 0x4000 != 0
    }

    pub(in crate::sgb) const fn y_flip(self) -> bool {
        self.raw & 0x8000 != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbBorderTileMap {
    pub entries: Vec<SgbBorderMapEntry>,
}

impl SgbBorderTileMap {
    pub(in crate::sgb) fn apply_pct_transfer(&mut self, payload: &SgbVramTransferBuffer) {
        for (entry_index, entry) in self.entries.iter_mut().enumerate() {
            let byte_index = entry_index * 2;
            *entry = SgbBorderMapEntry::new(u16::from_le_bytes([
                payload.bytes[byte_index],
                payload.bytes[byte_index + 1],
            ]));
        }
    }

    pub(in crate::sgb) fn entry(&self, tile_x: usize, tile_y: usize) -> SgbBorderMapEntry {
        self.entries[tile_y * SGB_BORDER_TILEMAP_WIDTH + tile_x]
    }
}

impl Default for SgbBorderTileMap {
    fn default() -> Self {
        Self {
            entries: vec![SgbBorderMapEntry::default(); SGB_BORDER_TILEMAP_ENTRIES],
        }
    }
}

impl SgbBorderTileMap {
    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.entries
            .len()
            .saturating_mul(std::mem::size_of::<SgbBorderMapEntry>())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbBorderPalette {
    pub colors: [SgbRgb555Color; SGB_BORDER_PALETTE_COLORS],
}

impl SgbBorderPalette {
    pub const fn color(self, color_index: u8) -> SgbRgb555Color {
        self.colors[(color_index & 0x0F) as usize]
    }
}

impl Default for SgbBorderPalette {
    fn default() -> Self {
        Self {
            colors: [SgbRgb555Color::default(); SGB_BORDER_PALETTE_COLORS],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbBorderState {
    pub tile_data: SgbBorderTileData,
    pub tile_map: SgbBorderTileMap,
    pub palettes: [SgbBorderPalette; SGB_BORDER_PALETTE_COUNT],
    pub chr0_loaded: bool,
    pub chr1_loaded: bool,
    pub pct_loaded: bool,
    pub last_chr_selection: Option<SgbChrTransferSelection>,
    pub chr_transfer_count: u64,
    pub pct_transfer_count: u64,
}

impl SgbBorderState {
    pub(in crate::sgb) fn apply_chr_transfer(
        &mut self,
        selection: SgbChrTransferSelection,
        payload: &SgbVramTransferBuffer,
    ) {
        self.tile_data.apply_chr_transfer(selection, payload);
        if selection.tile_block == 0 {
            self.chr0_loaded = true;
        } else {
            self.chr1_loaded = true;
        }
        self.last_chr_selection = Some(selection);
        self.chr_transfer_count = self.chr_transfer_count.saturating_add(1);
    }

    pub(in crate::sgb) fn apply_pct_transfer(&mut self, payload: &SgbVramTransferBuffer) {
        self.tile_map.apply_pct_transfer(payload);
        for palette_index in 0..SGB_BORDER_PALETTE_COUNT {
            for color_index in 0..SGB_BORDER_PALETTE_COLORS {
                let byte_index =
                    0x800 + palette_index * SGB_BORDER_PALETTE_COLORS * 2 + color_index * 2;
                let color = SgbRgb555Color::from_packet_bytes(
                    payload.bytes[byte_index],
                    payload.bytes[byte_index + 1],
                );
                self.palettes[palette_index].colors[color_index] = color;
            }
        }
        self.pct_loaded = true;
        self.pct_transfer_count = self.pct_transfer_count.saturating_add(1);
    }

    pub(in crate::sgb) fn pixel_color(&self, x: usize, y: usize) -> (SgbRgb555Color, u8) {
        let tile_x = x / 8;
        let tile_y = (y / 8).min(SGB_BORDER_TILEMAP_VISIBLE_HEIGHT - 1);
        let entry = self.tile_map.entry(tile_x, tile_y);
        let mut pixel_x = x & 0x07;
        let mut pixel_y = y & 0x07;
        if entry.x_flip() {
            pixel_x = 7 - pixel_x;
        }
        if entry.y_flip() {
            pixel_y = 7 - pixel_y;
        }

        let color_index = self
            .tile_data
            .pixel_color_index(entry.tile_index(), pixel_x, pixel_y);
        (
            self.palettes[entry.palette_index()].color(color_index),
            color_index,
        )
    }
}

impl Default for SgbBorderState {
    fn default() -> Self {
        Self {
            tile_data: SgbBorderTileData::default(),
            tile_map: SgbBorderTileMap::default(),
            palettes: [SgbBorderPalette::default(); SGB_BORDER_PALETTE_COUNT],
            chr0_loaded: false,
            chr1_loaded: false,
            pct_loaded: false,
            last_chr_selection: None,
            chr_transfer_count: 0,
            pct_transfer_count: 0,
        }
    }
}

impl SgbBorderState {
    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.tile_data
            .dynamic_payload_bytes()
            .saturating_add(self.tile_map.dynamic_payload_bytes())
    }
}
