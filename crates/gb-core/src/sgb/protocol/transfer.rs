use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbVramTransferError {
    DisabledHost,
    NoPendingTransfer,
    SourceLength { expected: usize, actual: usize },
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbScreenMask {
    #[default]
    Cancel,
    Freeze,
    BlankBlack,
    BlankColor0,
}

impl SgbScreenMask {
    pub(in crate::sgb) const fn from_command_byte(value: u8) -> Self {
        match value & 0x03 {
            0 => Self::Cancel,
            1 => Self::Freeze,
            2 => Self::BlankBlack,
            _ => Self::BlankColor0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbChrTransferTileType {
    Bg,
    Obj,
}

impl SgbChrTransferTileType {
    pub(in crate::sgb) const fn from_command_byte(value: u8) -> Self {
        if value & 0x02 == 0 {
            Self::Bg
        } else {
            Self::Obj
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbChrTransferSelection {
    pub tile_block: u8,
    pub tile_type: SgbChrTransferTileType,
}

impl SgbChrTransferSelection {
    pub(in crate::sgb) const fn from_command_byte(value: u8) -> Self {
        Self {
            tile_block: value & 0x01,
            tile_type: SgbChrTransferTileType::from_command_byte(value),
        }
    }

    pub(in crate::sgb) fn destination_offset(self) -> usize {
        usize::from(self.tile_block) * SGB_VRAM_TRANSFER_BYTES
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbVramTransferTarget {
    Chr(SgbChrTransferSelection),
    Pct,
    Pal,
    Attr,
    Sound,
    SnesData(SgbSnesAddress),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbVramTransferPhase {
    #[default]
    WaitingForNextFrame,
    Capturing,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbVramTransferSourceMode {
    #[default]
    Unresolved,
    Raw,
    DisplayOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbPendingVramTransfer {
    pub command_id: u8,
    pub target: SgbVramTransferTarget,
    pub frame_starts_until_capture: u8,
    pub phase: SgbVramTransferPhase,
    pub frames_captured: u8,
    pub total_frames: u8,
    #[serde(default)]
    pub source_mode: SgbVramTransferSourceMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SgbVramTransferDisplayState {
    pub lcdc: u8,
    pub scy: u8,
    pub scx: u8,
    pub bgp: u8,
}

impl SgbVramTransferDisplayState {
    pub const fn new(lcdc: u8, scy: u8, scx: u8, bgp: u8) -> Self {
        Self {
            lcdc,
            scy,
            scx,
            bgp,
        }
    }

    pub(in crate::sgb) const fn can_extract_display_order(
        self,
        allow_lcd_disabled_tail: bool,
    ) -> bool {
        // The transfer source is the prepared BG layout, not the current LCD enable bit alone. Some
        // software disables LCDC at the tail of the SGB transfer window after leaving VRAM and the
        // layout registers intact; falling back to raw VRAM for that final chunk corrupts the payload.
        (allow_lcd_disabled_tail || self.lcdc & SGB_LCDC_ENABLE_BIT != 0)
            && self.lcdc & SGB_LCDC_BG_ENABLE_BIT != 0
            && self.scy == 0
            && self.scx == 0
            && self.bgp == SGB_TRANSFER_REQUIRED_BGP
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbVramTransferBuffer {
    pub bytes: Vec<u8>,
}

impl SgbVramTransferBuffer {
    pub(in crate::sgb) fn from_source_bytes(source: &[u8]) -> Result<Self, SgbVramTransferError> {
        if source.len() < SGB_VRAM_TRANSFER_BYTES {
            return Err(SgbVramTransferError::SourceLength {
                expected: SGB_VRAM_TRANSFER_BYTES,
                actual: source.len(),
            });
        }

        Ok(Self {
            bytes: source[..SGB_VRAM_TRANSFER_BYTES].to_vec(),
        })
    }

    pub(in crate::sgb) fn from_display_memory(
        vram_bytes: &[u8],
        display: SgbVramTransferDisplayState,
    ) -> Result<Self, SgbVramTransferError> {
        Self::from_display_memory_with_source_mode(
            vram_bytes,
            display,
            SgbVramTransferSourceMode::Unresolved,
        )
        .map(|(payload, _source_mode)| payload)
    }

    pub(in crate::sgb) fn from_display_memory_with_source_mode(
        vram_bytes: &[u8],
        display: SgbVramTransferDisplayState,
        current_source_mode: SgbVramTransferSourceMode,
    ) -> Result<(Self, SgbVramTransferSourceMode), SgbVramTransferError> {
        if current_source_mode == SgbVramTransferSourceMode::Raw {
            return Self::from_source_bytes(vram_bytes)
                .map(|payload| (payload, SgbVramTransferSourceMode::Raw));
        }
        let allow_lcd_disabled_tail =
            current_source_mode == SgbVramTransferSourceMode::DisplayOrder;
        let frame_source_mode = if display.can_extract_display_order(allow_lcd_disabled_tail) {
            SgbVramTransferSourceMode::DisplayOrder
        } else {
            SgbVramTransferSourceMode::Raw
        };
        if frame_source_mode == SgbVramTransferSourceMode::Raw {
            return Self::from_source_bytes(vram_bytes)
                .map(|payload| (payload, SgbVramTransferSourceMode::Raw));
        }
        if vram_bytes.len() < SGB_GB_VRAM_BYTES {
            return Err(SgbVramTransferError::SourceLength {
                expected: SGB_GB_VRAM_BYTES,
                actual: vram_bytes.len(),
            });
        }

        let tile_map_base = if display.lcdc & SGB_LCDC_BG_TILE_MAP_BIT != 0 {
            SGB_GB_BG_MAP_9C00_OFFSET
        } else {
            SGB_GB_BG_MAP_9800_OFFSET
        };
        let mut bytes = vec![0; SGB_VRAM_TRANSFER_BYTES];
        for transfer_tile_index in 0..SGB_TRANSFER_DISPLAY_TILE_COUNT {
            let tile_x = transfer_tile_index % SGB_TRANSFER_DISPLAY_TILE_COLUMNS;
            let tile_y = transfer_tile_index / SGB_TRANSFER_DISPLAY_TILE_COLUMNS;
            let tile_map_offset = tile_map_base + tile_y * SGB_GB_TILEMAP_WIDTH + tile_x;
            let tile_index = vram_bytes[tile_map_offset];
            let source_offset = gb_tile_data_offset(display.lcdc, tile_index);
            let destination_offset = transfer_tile_index * SGB_GB_TILE_BYTES;
            bytes[destination_offset..destination_offset + SGB_GB_TILE_BYTES]
                .copy_from_slice(&vram_bytes[source_offset..source_offset + SGB_GB_TILE_BYTES]);
        }

        Ok((Self { bytes }, SgbVramTransferSourceMode::DisplayOrder))
    }

    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.bytes.len()
    }
}

impl Default for SgbVramTransferBuffer {
    fn default() -> Self {
        Self {
            bytes: vec![0; SGB_VRAM_TRANSFER_BYTES],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbCompletedVramTransfer {
    pub command_id: u8,
    pub target: SgbVramTransferTarget,
    pub payload: SgbVramTransferBuffer,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SgbVramTransferState {
    pub pending: Option<SgbPendingVramTransfer>,
    pub partial_payload: Option<SgbVramTransferBuffer>,
    #[serde(default)]
    pub display_order_payload: Option<SgbVramTransferBuffer>,
    pub last_completed: Option<SgbCompletedVramTransfer>,
    pub requested_transfer_count: u64,
    pub completed_transfer_count: u64,
}

impl SgbVramTransferState {
    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.last_completed
            .as_ref()
            .map(|transfer| transfer.payload.dynamic_payload_bytes())
            .unwrap_or(0)
            .saturating_add(
                self.partial_payload
                    .as_ref()
                    .map(SgbVramTransferBuffer::dynamic_payload_bytes)
                    .unwrap_or(0),
            )
            .saturating_add(
                self.display_order_payload
                    .as_ref()
                    .map(SgbVramTransferBuffer::dynamic_payload_bytes)
                    .unwrap_or(0),
            )
    }
}

pub(in crate::sgb) const fn gb_tile_data_offset(lcdc: u8, tile_index: u8) -> usize {
    if lcdc & SGB_LCDC_BG_WINDOW_TILE_DATA_BIT != 0 {
        tile_index as usize * SGB_GB_TILE_BYTES
    } else {
        (SGB_GB_SIGNED_TILE_DATA_BASE_OFFSET + (tile_index as i8 as i32) * SGB_GB_TILE_BYTES as i32)
            as usize
    }
}

pub(in crate::sgb) fn vram_transfer_chunk_range(
    frame_index: u8,
    total_frames: u8,
) -> (usize, usize) {
    let total_frames = usize::from(total_frames.max(1));
    let frame_index = usize::from(frame_index).min(total_frames - 1);
    let chunk_start = SGB_VRAM_TRANSFER_BYTES * frame_index / total_frames;
    let chunk_end = SGB_VRAM_TRANSFER_BYTES * (frame_index + 1) / total_frames;
    (chunk_start, chunk_end)
}
