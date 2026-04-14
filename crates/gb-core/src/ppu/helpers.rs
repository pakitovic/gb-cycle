use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum PpuLcdRestartPhase {
    #[default]
    Inactive,
    FirstLineAfterEnable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PpuRasterState {
    Disabled,
    LcdRestartFirstLine {
        mode: PpuAccessMode,
        mode_dot: u16,
    },
    Active {
        mode: PpuAccessMode,
        mode_dot: u16,
        mode2_scan_active: bool,
    },
}

impl PpuRasterState {
    pub(super) const fn access_mode(self) -> PpuAccessMode {
        match self {
            Self::Disabled => PpuAccessMode::HBlank,
            Self::LcdRestartFirstLine { mode, .. } | Self::Active { mode, .. } => mode,
        }
    }

    pub(super) const fn mode_dot(self) -> u16 {
        match self {
            Self::Disabled => 0,
            Self::LcdRestartFirstLine { mode_dot, .. } | Self::Active { mode_dot, .. } => mode_dot,
        }
    }

    pub(super) const fn is_mode2_scan(self) -> bool {
        matches!(
            self,
            Self::Active {
                mode2_scan_active: true,
                ..
            }
        )
    }
}

impl PpuLcdRestartPhase {
    pub(super) const fn first_line_after_enable() -> Self {
        Self::FirstLineAfterEnable
    }

    pub(super) const fn is_first_line_after_enable_active(self, ly: u8) -> bool {
        matches!(self, Self::FirstLineAfterEnable) && ly == 0
    }

    pub(super) const fn raster_state(self, ly: u8, line_dot: u16) -> Option<PpuRasterState> {
        if self.is_first_line_after_enable_active(ly) {
            let (mode, mode_dot) = if line_dot < LCD_REENABLE_LINE0_MODE3_START_DOT {
                (PpuAccessMode::HBlank, line_dot)
            } else if line_dot < LCD_REENABLE_LINE0_MODE0_RESTORE_DOT {
                (
                    PpuAccessMode::Drawing,
                    line_dot.saturating_sub(LCD_REENABLE_LINE0_MODE3_START_DOT),
                )
            } else {
                (
                    PpuAccessMode::HBlank,
                    line_dot.saturating_sub(LCD_REENABLE_LINE0_MODE0_RESTORE_DOT),
                )
            };
            Some(PpuRasterState::LcdRestartFirstLine { mode, mode_dot })
        } else {
            None
        }
    }

    pub(super) const fn advance(self, ly: u8, _line_dot: u16) -> Self {
        if self.is_first_line_after_enable_active(ly) {
            self
        } else {
            Self::Inactive
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PpuDmgBgpCpuCommitEffectKind {
    PipelineDelayed,
    CurrentDotTransient,
    RetroactivePanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PpuDmgBgpCpuCommitWrite {
    pub(super) effect_kind: PpuDmgBgpCpuCommitEffectKind,
    pub(super) transient_visible_x: u8,
    pub(super) transient_palette: u8,
    pub(super) repaint_visible_x: u8,
    pub(super) transfer_lead_pixels: u8,
    pub(super) value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PpuDmgBgpBoundaryRepaintWrite {
    pub(super) write: PpuDmgBgpCpuCommitWrite,
    pub(super) selected_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PpuRecentPanelDot {
    pub(super) visible_x: u8,
    pub(super) pixel: MixedPixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) struct PpuVisibleRegisters {
    pub(super) lcdc: u8,
    pub(super) scy: u8,
    pub(super) scx: u8,
    pub(super) bgp: u8,
    pub(super) obp0: Option<u8>,
    pub(super) obp1: Option<u8>,
    pub(super) wy: u8,
    pub(super) wx: u8,
}

impl PpuVisibleRegisters {
    pub(super) const fn bg_enabled(self) -> bool {
        self.lcdc & LCDC_BG_ENABLE_BIT != 0
    }

    pub(super) const fn obj_enabled(self) -> bool {
        self.lcdc & LCDC_OBJ_ENABLE_BIT != 0
    }

    pub(super) const fn window_enabled(self) -> bool {
        self.lcdc & LCDC_WINDOW_ENABLE_BIT != 0
    }

    pub(super) const fn obj_height(self) -> u8 {
        if self.lcdc & LCDC_OBJ_SIZE_BIT != 0 {
            16
        } else {
            8
        }
    }

    pub(super) fn obj_palette(self, palette_obp1: bool, policy: DmgObjPaletteReadPolicy) -> u8 {
        if palette_obp1 {
            self.obp1.unwrap_or(policy.default_read_value())
        } else {
            self.obp0.unwrap_or(policy.default_read_value())
        }
    }
}

pub(super) const fn lcd_state_from_lcdc(lcdc: u8) -> PpuLcdState {
    if lcdc & LCDC_ENABLE_BIT != 0 {
        PpuLcdState::Enabled
    } else {
        PpuLcdState::Disabled
    }
}

pub(super) const fn visible_output_for_lcd_state(lcd_state: PpuLcdState) -> PpuVisibleOutputState {
    if lcd_state.is_enabled() {
        PpuVisibleOutputState::Driving
    } else {
        PpuVisibleOutputState::ForcedBlank
    }
}

pub(super) const fn access_mode_from_raster(
    ly: u8,
    line_dot: u16,
    mode0_start_dot: u16,
) -> PpuAccessMode {
    if ly >= VISIBLE_SCANLINES {
        PpuAccessMode::VBlank
    } else if line_dot < MODE2_DOTS {
        PpuAccessMode::OamScan
    } else if line_dot < mode0_start_dot {
        PpuAccessMode::Drawing
    } else {
        PpuAccessMode::HBlank
    }
}

pub(super) const fn mode_dot_from_raster_mode(
    mode: PpuAccessMode,
    line_dot: u16,
    mode0_start_dot: u16,
) -> u16 {
    match mode {
        PpuAccessMode::VBlank => line_dot,
        PpuAccessMode::OamScan => line_dot,
        PpuAccessMode::Drawing => line_dot.saturating_sub(MODE2_DOTS),
        PpuAccessMode::HBlank => line_dot.saturating_sub(mode0_start_dot),
    }
}

pub(super) const fn bg_tile_data_base(lcdc: u8, tile_index: u8) -> u16 {
    if lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0 {
        tile_index as u16 * TILE_BYTES
    } else {
        (0x1000_i16 + (tile_index as i8 as i16) * TILE_BYTES as i16) as u16
    }
}

pub(super) const fn bg_tile_pixel_value(tile_low: u8, tile_high: u8, pixel_index: u8) -> u8 {
    let bit = BG_TILE_WIDTH - 1 - pixel_index;
    let low_bit = (tile_low >> bit) & 0x01;
    let high_bit = (tile_high >> bit) & 0x01;
    (high_bit << 1) | low_bit
}

pub(super) fn read_oam_sprite(oam: &OamBusView<'_>, oam_index: u8) -> Option<PpuSelectedSprite> {
    let entry_start = oam_index as usize * OAM_ENTRY_BYTES;
    Some(PpuSelectedSprite {
        oam_index,
        y: oam.read(entry_start)?,
        x: oam.read(entry_start + 1)?,
        tile_index: oam.read(entry_start + 2)?,
        attributes: oam.read(entry_start + 3)?,
    })
}

pub(super) fn read_obj_fetch_sprite_metadata(
    oam: &OamBusView<'_>,
    sprite: PpuSelectedSprite,
    dma_oam_conflict: Option<PpuDmaOamConflict>,
) -> (u8, u8) {
    let nominal_word_address = 0xFE00_u16 + sprite.oam_index as u16 * OAM_ENTRY_BYTES as u16 + 2;
    let word_address = dma_oam_conflict
        .filter(|conflict| (0xFE00..=0xFE9F).contains(&conflict.address))
        .map(PpuDmaOamConflict::word_address)
        .unwrap_or(nominal_word_address);
    let word_offset = word_address.saturating_sub(0xFE00) as usize;
    let mut metadata = [
        oam.read(word_offset).unwrap_or(sprite.tile_index),
        oam.read(word_offset + 1).unwrap_or(sprite.attributes),
    ];
    if let Some(conflict) =
        dma_oam_conflict.filter(|conflict| conflict.word_address() == word_address)
    {
        metadata[conflict.byte_offset_in_word()] = conflict.value();
    }

    (metadata[0], metadata[1])
}

pub(super) fn sprite_matches_line(sprite: PpuSelectedSprite, ly: u8, height: u8) -> bool {
    let current_line = ly as u16 + 16;
    let sprite_y = sprite.y as u16;
    let sprite_bottom = sprite_y + height as u16;

    current_line >= sprite_y && current_line < sprite_bottom
}

pub(super) fn read_oam_row(oam_bytes: &[u8], row: u8) -> [u8; OAM_CORRUPTION_ROW_BYTES] {
    let row_start = row as usize * OAM_CORRUPTION_ROW_BYTES;
    let mut row_bytes = [0; OAM_CORRUPTION_ROW_BYTES];
    row_bytes.copy_from_slice(&oam_bytes[row_start..row_start + OAM_CORRUPTION_ROW_BYTES]);
    row_bytes
}

pub(super) fn write_oam_row(
    oam_bytes: &mut [u8],
    row: u8,
    row_bytes: [u8; OAM_CORRUPTION_ROW_BYTES],
) {
    let row_start = row as usize * OAM_CORRUPTION_ROW_BYTES;
    oam_bytes[row_start..row_start + OAM_CORRUPTION_ROW_BYTES].copy_from_slice(&row_bytes);
}

pub(super) fn read_oam_word(oam_bytes: &[u8], row: u8, word_index: usize) -> u16 {
    let word_start = row as usize * OAM_CORRUPTION_ROW_BYTES + word_index * 2;
    u16::from_le_bytes([oam_bytes[word_start], oam_bytes[word_start + 1]])
}

pub(super) fn write_oam_word(oam_bytes: &mut [u8], row: u8, word_index: usize, value: u16) {
    let word_start = row as usize * OAM_CORRUPTION_ROW_BYTES + word_index * 2;
    let [low, high] = value.to_le_bytes();
    oam_bytes[word_start] = low;
    oam_bytes[word_start + 1] = high;
}

pub(super) fn copy_previous_row_tail(oam_bytes: &mut [u8], current_row: u8) {
    for word_index in 1..OAM_CORRUPTION_ROW_WORDS {
        let previous_word = read_oam_word(oam_bytes, current_row - 1, word_index);
        write_oam_word(oam_bytes, current_row, word_index, previous_word);
    }
}

pub(super) fn sprite_screen_x(sprite: PpuSelectedSprite) -> i16 {
    sprite.x as i16 - 8
}

pub(super) fn sprite_trigger_x(sprite: PpuSelectedSprite) -> Option<u8> {
    if sprite.x >= 168 {
        return None;
    }

    Some(sprite.x)
}

pub(super) fn obj_pixel_has_priority(candidate: ObjPixel, current: ObjPixel) -> bool {
    if current.is_transparent() {
        return !candidate.is_transparent();
    }
    if candidate.is_transparent() {
        return false;
    }

    candidate.sprite_x < current.sprite_x
        || (candidate.sprite_x == current.sprite_x && candidate.oam_index < current.oam_index)
}
