use super::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbObjOamPayload {
    pub bytes: Vec<u8>,
}

impl SgbObjOamPayload {
    pub(in crate::sgb) fn from_source_bytes(source: &[u8]) -> Result<Self, SgbVramTransferError> {
        let expected = SGB_OBJ_OAM_SOURCE_OFFSET + SGB_OBJ_OAM_PAYLOAD_BYTES;
        if source.len() < expected {
            return Err(SgbVramTransferError::SourceLength {
                expected,
                actual: source.len(),
            });
        }

        Ok(Self {
            bytes: source
                [SGB_OBJ_OAM_SOURCE_OFFSET..SGB_OBJ_OAM_SOURCE_OFFSET + SGB_OBJ_OAM_PAYLOAD_BYTES]
                .to_vec(),
        })
    }

    pub(in crate::sgb) fn from_display_memory(
        vram_bytes: &[u8],
        display: SgbVramTransferDisplayState,
    ) -> Result<Self, SgbVramTransferError> {
        let payload = SgbVramTransferBuffer::from_display_memory(vram_bytes, display)?;
        Self::from_source_bytes(&payload.bytes)
    }

    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.bytes.len()
    }
}

impl Default for SgbObjOamPayload {
    fn default() -> Self {
        Self {
            bytes: vec![0; SGB_OBJ_OAM_PAYLOAD_BYTES],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SgbObjTransferState {
    pub enabled: bool,
    pub color_transfer_requested: bool,
    pub last_control: u8,
    pub palette_ids: [u16; 4],
    pub palettes: [SgbBorderPalette; 4],
    pub command_count: u64,
    pub frame_capture_count: u64,
    pub last_oam_payload: Option<SgbObjOamPayload>,
}

impl SgbObjTransferState {
    pub(in crate::sgb) fn apply_obj_trn(
        &mut self,
        system_palettes: &SgbSystemPaletteState,
        bytes: &[u8; SGB_PACKET_BYTES],
    ) {
        self.last_control = bytes[1] & 0x03;
        self.enabled = self.last_control & 0x01 != 0;
        self.color_transfer_requested = self.last_control & 0x02 != 0;
        self.command_count = self.command_count.saturating_add(1);
        for palette_index in 0..4 {
            let byte_index = 2 + palette_index * 2;
            self.palette_ids[palette_index] =
                u16::from_le_bytes([bytes[byte_index], bytes[byte_index + 1]]) & 0x01FF;
        }

        if self.color_transfer_requested {
            self.reload_palettes(system_palettes);
        }
        if !self.enabled {
            self.last_oam_payload = None;
        }
    }

    pub(in crate::sgb) fn reload_palettes(&mut self, system_palettes: &SgbSystemPaletteState) {
        for obj_palette_index in 0..4 {
            let base_palette_id = self.palette_ids[obj_palette_index] as usize;
            for sub_palette_index in 0..4 {
                let system_palette =
                    system_palettes.palette_wrapping(base_palette_id + sub_palette_index);
                for color_index in 0..SGB_SCREEN_PALETTE_COLORS {
                    self.palettes[obj_palette_index].colors
                        [sub_palette_index * SGB_SCREEN_PALETTE_COLORS + color_index] =
                        system_palette.colors[color_index];
                }
            }
        }
    }

    pub(in crate::sgb) fn capture_frame(
        &mut self,
        vram_bytes: &[u8],
        display: SgbVramTransferDisplayState,
    ) -> Result<(), SgbVramTransferError> {
        if !self.enabled {
            return Ok(());
        }
        self.last_oam_payload = Some(SgbObjOamPayload::from_display_memory(vram_bytes, display)?);
        self.frame_capture_count = self.frame_capture_count.saturating_add(1);
        Ok(())
    }

    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.last_oam_payload
            .as_ref()
            .map(SgbObjOamPayload::dynamic_payload_bytes)
            .unwrap_or(0)
    }
}
