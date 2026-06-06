use super::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbAttributeMap {
    pub cells: Vec<u8>,
}

impl SgbAttributeMap {
    pub fn palette_index(&self, cell_x: usize, cell_y: usize) -> u8 {
        self.cells[cell_y * SGB_ATTR_MAP_WIDTH + cell_x] & 0x03
    }

    pub(in crate::sgb) fn palette_index_for_framebuffer_index(
        &self,
        framebuffer_index: usize,
    ) -> u8 {
        let pixel_x = framebuffer_index % SGB_LCD_WIDTH;
        let pixel_y = framebuffer_index / SGB_LCD_WIDTH;
        self.palette_index(pixel_x / 8, pixel_y / 8)
    }

    pub(in crate::sgb) fn set_cell(&mut self, cell_x: usize, cell_y: usize, palette_index: u8) {
        if cell_x < SGB_ATTR_MAP_WIDTH && cell_y < SGB_ATTR_MAP_HEIGHT {
            self.cells[cell_y * SGB_ATTR_MAP_WIDTH + cell_x] = palette_index & 0x03;
        }
    }

    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.cells.len()
    }
}

impl Default for SgbAttributeMap {
    fn default() -> Self {
        Self {
            cells: vec![0; SGB_ATTR_MAP_CELLS],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbAttributeFileState {
    pub bytes: Vec<u8>,
    pub loaded: bool,
}

impl SgbAttributeFileState {
    pub(in crate::sgb) fn apply_attr_trn(&mut self, payload: &SgbVramTransferBuffer) {
        self.bytes
            .copy_from_slice(&payload.bytes[..SGB_ATF_TOTAL_BYTES]);
        self.loaded = true;
    }

    pub(in crate::sgb) fn apply_to_map(&self, atf_index: u8, map: &mut SgbAttributeMap) -> bool {
        let atf_index = usize::from(atf_index);
        if atf_index >= SGB_ATF_COUNT {
            return false;
        }
        for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
            for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                map.set_cell(
                    cell_x,
                    cell_y,
                    self.palette_index(atf_index, cell_x, cell_y),
                );
            }
        }
        true
    }

    pub(in crate::sgb) fn palette_index(
        &self,
        atf_index: usize,
        cell_x: usize,
        cell_y: usize,
    ) -> u8 {
        let byte_index = atf_index * SGB_ATF_BYTES + cell_y * 5 + cell_x / 4;
        let shift = 6 - (cell_x % 4) * 2;
        (self.bytes[byte_index] >> shift) & 0x03
    }

    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.bytes.len()
    }
}

impl Default for SgbAttributeFileState {
    fn default() -> Self {
        Self {
            bytes: vec![0; SGB_ATF_TOTAL_BYTES],
            loaded: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SgbAttributeState {
    pub map: SgbAttributeMap,
    pub files: SgbAttributeFileState,
    pub last_atf_index: Option<u8>,
    pub attr_blk_count: u64,
    pub attr_lin_count: u64,
    pub attr_div_count: u64,
    pub attr_chr_count: u64,
    pub attr_trn_count: u64,
    pub attr_set_count: u64,
    pub invalid_atf_count: u64,
}

impl SgbAttributeState {
    pub(in crate::sgb) fn apply_attr_blk(&mut self, payload: &[u8]) {
        if payload.is_empty() {
            return;
        }
        let data_set_count = usize::from(payload[0]).min(0x12);
        for data_set_index in 0..data_set_count {
            let offset = 1 + data_set_index * 6;
            let Some(data_set) = payload.get(offset..offset + 6) else {
                break;
            };
            self.apply_attr_blk_data_set(data_set);
        }
        self.attr_blk_count = self.attr_blk_count.saturating_add(1);
    }

    pub(in crate::sgb) fn apply_attr_blk_data_set(&mut self, data_set: &[u8]) {
        let control = data_set[0] & 0x07;
        let palette_designation = data_set[1];
        let inside_palette = palette_designation & 0x03;
        let mut line_palette = (palette_designation >> 2) & 0x03;
        let outside_palette = (palette_designation >> 4) & 0x03;
        let change_inside = control & 0x01 != 0;
        let mut change_line = control & 0x02 != 0;
        let change_outside = control & 0x04 != 0;

        if control == 0x01 {
            change_line = true;
            line_palette = inside_palette;
        } else if control == 0x04 {
            change_line = true;
            line_palette = outside_palette;
        }

        let x1 = usize::from(data_set[2]).min(SGB_ATTR_MAP_WIDTH - 1);
        let y1 = usize::from(data_set[3]).min(SGB_ATTR_MAP_HEIGHT - 1);
        let x2 = usize::from(data_set[4]).min(SGB_ATTR_MAP_WIDTH - 1);
        let y2 = usize::from(data_set[5]).min(SGB_ATTR_MAP_HEIGHT - 1);
        let (left, right) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        let (top, bottom) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };

        for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
            for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                let inside_rect =
                    (left..=right).contains(&cell_x) && (top..=bottom).contains(&cell_y);
                let on_line = inside_rect
                    && (cell_x == left || cell_x == right || cell_y == top || cell_y == bottom);
                if change_outside && !inside_rect {
                    self.map.set_cell(cell_x, cell_y, outside_palette);
                }
                if change_inside && inside_rect && !on_line {
                    self.map.set_cell(cell_x, cell_y, inside_palette);
                }
                if change_line && on_line {
                    self.map.set_cell(cell_x, cell_y, line_palette);
                }
            }
        }
    }

    pub(in crate::sgb) fn apply_attr_lin(&mut self, payload: &[u8]) {
        if payload.is_empty() {
            return;
        }
        let data_set_count = usize::from(payload[0]).min(0x6E);
        for &line in payload.iter().skip(1).take(data_set_count) {
            let coordinate = usize::from(line & 0x1F);
            let palette_index = (line >> 5) & 0x03;
            let horizontal = line & 0x80 != 0;
            if horizontal {
                if coordinate < SGB_ATTR_MAP_HEIGHT {
                    for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                        self.map.set_cell(cell_x, coordinate, palette_index);
                    }
                }
            } else if coordinate < SGB_ATTR_MAP_WIDTH {
                for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
                    self.map.set_cell(coordinate, cell_y, palette_index);
                }
            }
        }
        self.attr_lin_count = self.attr_lin_count.saturating_add(1);
    }

    pub(in crate::sgb) fn apply_attr_div(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        let palettes = bytes[1];
        let below_or_right_palette = palettes & 0x03;
        let above_or_left_palette = (palettes >> 2) & 0x03;
        let line_palette = (palettes >> 4) & 0x03;
        let horizontal = palettes & 0x40 != 0;
        let coordinate = usize::from(bytes[2]);

        if horizontal {
            for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
                let palette_index = if cell_y < coordinate {
                    above_or_left_palette
                } else if cell_y == coordinate {
                    line_palette
                } else {
                    below_or_right_palette
                };
                for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                    self.map.set_cell(cell_x, cell_y, palette_index);
                }
            }
        } else {
            for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                let palette_index = if cell_x < coordinate {
                    above_or_left_palette
                } else if cell_x == coordinate {
                    line_palette
                } else {
                    below_or_right_palette
                };
                for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
                    self.map.set_cell(cell_x, cell_y, palette_index);
                }
            }
        }
        self.attr_div_count = self.attr_div_count.saturating_add(1);
    }

    pub(in crate::sgb) fn apply_attr_chr(&mut self, payload: &[u8]) {
        if payload.len() < 5 {
            return;
        }
        let mut cell_x = usize::from(payload[0]);
        let mut cell_y = usize::from(payload[1]);
        if cell_x >= SGB_ATTR_MAP_WIDTH || cell_y >= SGB_ATTR_MAP_HEIGHT {
            return;
        }
        let data_set_count =
            usize::from(u16::from_le_bytes([payload[2], payload[3]])).min(SGB_ATTR_MAP_CELLS);
        let top_to_bottom = payload[4] & 0x01 != 0;

        for data_set_index in 0..data_set_count {
            let packed_byte_index = 5 + data_set_index / 4;
            let Some(&packed) = payload.get(packed_byte_index) else {
                break;
            };
            let shift = 6 - (data_set_index % 4) * 2;
            self.map.set_cell(cell_x, cell_y, (packed >> shift) & 0x03);

            if top_to_bottom {
                cell_y += 1;
                if cell_y == SGB_ATTR_MAP_HEIGHT {
                    cell_y = 0;
                    cell_x = (cell_x + 1) % SGB_ATTR_MAP_WIDTH;
                }
            } else {
                cell_x += 1;
                if cell_x == SGB_ATTR_MAP_WIDTH {
                    cell_x = 0;
                    cell_y = (cell_y + 1) % SGB_ATTR_MAP_HEIGHT;
                }
            }
        }
        self.attr_chr_count = self.attr_chr_count.saturating_add(1);
    }

    pub(in crate::sgb) fn apply_attr_trn(&mut self, payload: &SgbVramTransferBuffer) {
        self.files.apply_attr_trn(payload);
        self.attr_trn_count = self.attr_trn_count.saturating_add(1);
    }

    pub(in crate::sgb) fn apply_attr_set(&mut self, atf_index: u8) -> bool {
        if self.files.apply_to_map(atf_index, &mut self.map) {
            self.last_atf_index = Some(atf_index);
            self.attr_set_count = self.attr_set_count.saturating_add(1);
            true
        } else {
            self.invalid_atf_count = self.invalid_atf_count.saturating_add(1);
            false
        }
    }

    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.map
            .dynamic_payload_bytes()
            .saturating_add(self.files.dynamic_payload_bytes())
    }
}
