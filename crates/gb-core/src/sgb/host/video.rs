use super::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbVideoState {
    pub border_loaded: bool,
    pub colorization_active: bool,
    pub backdrop_color: SgbRgb555Color,
    pub palette_state: SgbPaletteState,
    pub system_palettes: SgbSystemPaletteState,
    pub player_palette_override: SgbPlayerPaletteOverrideState,
    pub attributes: SgbAttributeState,
    pub last_palette_command_id: Option<u8>,
    pub palette_command_count: u64,
    pub mask: SgbScreenMask,
    pub mask_command_count: u64,
    pub freeze_capture_pending: bool,
    pub frozen_lcd: Option<SgbLcdRgb555Frame>,
    pub vram_transfer: SgbVramTransferState,
    pub border: SgbBorderState,
    pub obj: SgbObjTransferState,
}

impl SgbHost {
    pub fn compose_lcd_rgb555(
        &self,
        dmg_framebuffer: &[u8],
    ) -> Result<Vec<u16>, SgbLcdCompositionError> {
        let mut output = vec![0; SGB_LCD_PIXELS];
        self.compose_lcd_rgb555_into(dmg_framebuffer, &mut output)?;
        Ok(output)
    }

    pub fn compose_lcd_rgb555_into(
        &self,
        dmg_framebuffer: &[u8],
        output: &mut [u16],
    ) -> Result<(), SgbLcdCompositionError> {
        if !self.host_platform.is_sgb() || !self.video.colorization_active {
            return Err(SgbLcdCompositionError::DisabledHost);
        }
        if dmg_framebuffer.len() != SGB_LCD_PIXELS {
            return Err(SgbLcdCompositionError::InputLength {
                expected: SGB_LCD_PIXELS,
                actual: dmg_framebuffer.len(),
            });
        }
        if output.len() != SGB_LCD_PIXELS {
            return Err(SgbLcdCompositionError::OutputLength {
                expected: SGB_LCD_PIXELS,
                actual: output.len(),
            });
        }
        for (framebuffer_index, (output_pixel, &shade)) in
            output.iter_mut().zip(dmg_framebuffer.iter()).enumerate()
        {
            *output_pixel = self
                .video
                .lcd_pixel_for_framebuffer_index(framebuffer_index, shade)
                .raw();
        }
        Ok(())
    }

    pub fn compose_frame_rgb555(
        &self,
        dmg_framebuffer: &[u8],
    ) -> Result<Vec<u16>, SgbFrameCompositionError> {
        let mut output = vec![0; SGB_FRAME_PIXELS];
        self.compose_frame_rgb555_into(dmg_framebuffer, &mut output)?;
        Ok(output)
    }

    pub fn compose_frame_rgb555_into(
        &self,
        dmg_framebuffer: &[u8],
        output: &mut [u16],
    ) -> Result<(), SgbFrameCompositionError> {
        if !self.host_platform.is_sgb() || !self.video.colorization_active {
            return Err(SgbFrameCompositionError::DisabledHost);
        }
        if dmg_framebuffer.len() != SGB_LCD_PIXELS {
            return Err(SgbFrameCompositionError::InputLength {
                expected: SGB_LCD_PIXELS,
                actual: dmg_framebuffer.len(),
            });
        }
        if output.len() != SGB_FRAME_PIXELS {
            return Err(SgbFrameCompositionError::OutputLength {
                expected: SGB_FRAME_PIXELS,
                actual: output.len(),
            });
        }
        for y in 0..SGB_FRAME_HEIGHT {
            for x in 0..SGB_FRAME_WIDTH {
                let output_index = y * SGB_FRAME_WIDTH + x;
                let in_lcd_window =
                    (SGB_LCD_FRAME_ORIGIN_X..SGB_LCD_FRAME_ORIGIN_X + SGB_LCD_WIDTH).contains(&x)
                        && (SGB_LCD_FRAME_ORIGIN_Y..SGB_LCD_FRAME_ORIGIN_Y + SGB_LCD_HEIGHT)
                            .contains(&y);

                let border_pixel = self.shell.presentation_border_pixel(
                    &self.video.border,
                    self.video.application_backdrop_color(),
                    x,
                    y,
                );
                if in_lcd_window && let Some(lcd_scale) = border_pixel.lcd_scale {
                    let lcd_x = x - SGB_LCD_FRAME_ORIGIN_X;
                    let lcd_y = y - SGB_LCD_FRAME_ORIGIN_Y;
                    let lcd_index = lcd_y * SGB_LCD_WIDTH + lcd_x;
                    let lcd_pixel = self
                        .video
                        .lcd_pixel_for_framebuffer_index(lcd_index, dmg_framebuffer[lcd_index]);
                    output[output_index] =
                        super::super::shell::scale_rgb555(lcd_pixel, lcd_scale).raw();
                } else {
                    output[output_index] = border_pixel.color.raw();
                }
            }
        }

        Ok(())
    }

    pub fn capture_pending_lcd_freeze(
        &mut self,
        dmg_framebuffer: &[u8],
    ) -> Result<(), SgbLcdCompositionError> {
        if !self.host_platform.is_sgb() || !self.video.colorization_active {
            return Err(SgbLcdCompositionError::DisabledHost);
        }
        if dmg_framebuffer.len() != SGB_LCD_PIXELS {
            return Err(SgbLcdCompositionError::InputLength {
                expected: SGB_LCD_PIXELS,
                actual: dmg_framebuffer.len(),
            });
        }
        if !self.video.freeze_capture_pending {
            return Ok(());
        }

        let mut frozen = SgbLcdRgb555Frame::default();
        for (framebuffer_index, (output_pixel, &shade)) in frozen
            .pixels
            .iter_mut()
            .zip(dmg_framebuffer.iter())
            .enumerate()
        {
            *output_pixel = self
                .video
                .live_lcd_pixel_for_framebuffer_index(framebuffer_index, shade)
                .raw();
        }
        self.video.frozen_lcd = Some(frozen);
        self.video.freeze_capture_pending = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn capture_pending_vram_transfer(
        &mut self,
        vram_bytes: &[u8],
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        if !self.host_platform.is_sgb() {
            return Err(SgbVramTransferError::DisabledHost);
        }
        let target = self.video.capture_pending_vram_transfer(vram_bytes)?;
        self.dispatch_completed_vram_transfer(target);
        if target.is_some() {
            self.packet_gate.clear_busy();
        }
        Ok(target)
    }

    pub(crate) fn advance_frame_start(
        &mut self,
        vram_bytes: &[u8],
        display: SgbVramTransferDisplayState,
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        if !self.host_platform.is_sgb() {
            return Err(SgbVramTransferError::DisabledHost);
        }
        self.shell.advance_frame();
        let target = self.video.advance_frame_start(vram_bytes, display)?;
        self.dispatch_completed_vram_transfer(target);
        if target.is_some() {
            self.packet_gate.clear_busy();
        } else {
            self.packet_gate.advance_frame();
        }
        Ok(target)
    }

    pub(in crate::sgb) fn dispatch_completed_vram_transfer(
        &mut self,
        target: Option<SgbVramTransferTarget>,
    ) {
        let Some(target) = target else {
            return;
        };
        let Some(completed) = self.video.vram_transfer.last_completed.as_ref() else {
            return;
        };
        match target {
            SgbVramTransferTarget::Sound => {
                let request =
                    SgbSoundTransferRequest::from_vram_transfer_payload(&completed.payload);
                self.dispatch_host_backend_request(SgbHostBackendRequest::Audio(
                    SgbHostAudioRequest::SoundTransfer(request),
                ));
            }
            SgbVramTransferTarget::SnesData(destination) => {
                let payload_bytes = completed.payload.bytes.len() as u32;
                self.dispatch_host_backend_request(SgbHostBackendRequest::Snes(
                    SgbSnesHostRequest::DataTransfer(SgbDataTransferRequest {
                        destination,
                        payload_bytes,
                    }),
                ));
            }
            SgbVramTransferTarget::Pct => {
                self.shell.start_game_border_transition();
            }
            SgbVramTransferTarget::Chr(_)
            | SgbVramTransferTarget::Pal
            | SgbVramTransferTarget::Attr => {}
        }
    }
}

impl SgbVideoState {
    pub(in crate::sgb) fn default_for_active_host(active: bool) -> Self {
        let palette_state = SgbPaletteState::default_for_active_host(active);
        let backdrop_color = if active {
            palette_state.palette(0).color(0)
        } else {
            SgbRgb555Color::default()
        };
        let mut state = Self {
            border_loaded: false,
            colorization_active: active,
            backdrop_color,
            palette_state,
            system_palettes: SgbSystemPaletteState::default(),
            player_palette_override: SgbPlayerPaletteOverrideState::default(),
            attributes: SgbAttributeState::default(),
            last_palette_command_id: None,
            palette_command_count: 0,
            mask: SgbScreenMask::Cancel,
            mask_command_count: 0,
            freeze_capture_pending: false,
            frozen_lcd: None,
            vram_transfer: SgbVramTransferState::default(),
            border: SgbBorderState::default(),
            obj: SgbObjTransferState::default(),
        };
        if active {
            load_default_border(&mut state.border);
            state.border_loaded = true;
        }
        state
    }

    pub fn map_lcd_shade_to_rgb555(&self, shade: u8) -> SgbRgb555Color {
        self.visible_palette_color(self.visible_palette_state().base_palette_index, shade)
    }

    pub fn lcd_pixel_for_shade(&self, shade: u8) -> SgbRgb555Color {
        match self.mask {
            SgbScreenMask::Cancel => self.map_lcd_shade_to_rgb555(shade),
            SgbScreenMask::Freeze => self.map_lcd_shade_to_rgb555(shade),
            SgbScreenMask::BlankBlack => SGB_RGB555_BLACK,
            SgbScreenMask::BlankColor0 => self.visible_lcd_backdrop_color(),
        }
    }

    pub(in crate::sgb::host) fn set_player_palette_override(
        &mut self,
        palette: SgbScreenPalette,
    ) -> bool {
        self.player_palette_override.set_uniform_palette(palette)
    }

    pub(in crate::sgb::host) fn clear_player_palette_override(&mut self) -> bool {
        self.player_palette_override.clear_by_player()
    }

    fn visible_palette_state(&self) -> &SgbPaletteState {
        if self.player_palette_override.active {
            &self.player_palette_override.palette_state
        } else {
            &self.palette_state
        }
    }

    fn application_backdrop_color(&self) -> SgbRgb555Color {
        self.backdrop_color
    }

    pub(in crate::sgb) fn visible_lcd_backdrop_color(&self) -> SgbRgb555Color {
        if self.player_palette_override.active {
            self.player_palette_override
                .palette_state
                .palette(0)
                .color(0)
        } else {
            self.palette_state.palette(0).color(0)
        }
    }

    fn visible_palette_color(&self, palette_index: u8, color_index: u8) -> SgbRgb555Color {
        if color_index & 0x03 == 0 {
            self.visible_lcd_backdrop_color()
        } else {
            self.visible_palette_state()
                .palette(palette_index)
                .color(color_index)
        }
    }

    fn visible_attribute_map(&self) -> &SgbAttributeMap {
        if self.player_palette_override.active {
            &self.player_palette_override.attributes
        } else {
            &self.attributes.map
        }
    }

    pub(in crate::sgb::host) fn apply_boot_palette_for_cartridge_header(
        &mut self,
        host_status: SgbHostStatus,
        header: Option<&CartridgeHeader>,
        command_acceptance: SgbCommandAcceptance,
    ) {
        if host_status == SgbHostStatus::Disabled {
            self.colorization_active = false;
            return;
        }

        let selection = sgb_boot_palette_selection_for_header(header, command_acceptance);
        self.palette_state.apply_boot_palette(selection);
        self.backdrop_color = self.palette_state.palette(0).color(0);
        self.colorization_active = true;
    }

    fn lcd_pixel_for_framebuffer_index(
        &self,
        framebuffer_index: usize,
        shade: u8,
    ) -> SgbRgb555Color {
        match self.mask {
            SgbScreenMask::Cancel => {
                self.live_lcd_pixel_for_framebuffer_index(framebuffer_index, shade)
            }
            SgbScreenMask::Freeze => self
                .frozen_lcd
                .as_ref()
                .and_then(|frame| frame.pixels.get(framebuffer_index).copied())
                .map(SgbRgb555Color::new)
                .unwrap_or_else(|| {
                    self.live_lcd_pixel_for_framebuffer_index(framebuffer_index, shade)
                }),
            SgbScreenMask::BlankBlack => SGB_RGB555_BLACK,
            SgbScreenMask::BlankColor0 => self.visible_lcd_backdrop_color(),
        }
    }

    fn live_lcd_pixel_for_framebuffer_index(
        &self,
        framebuffer_index: usize,
        shade: u8,
    ) -> SgbRgb555Color {
        let palette_index = self
            .visible_attribute_map()
            .palette_index_for_framebuffer_index(framebuffer_index);
        self.visible_palette_color(palette_index, shade)
    }

    pub(in crate::sgb::host) fn apply_direct_palette_command(
        &mut self,
        command_id: u8,
        bytes: &[u8; SGB_PACKET_BYTES],
    ) {
        self.palette_state
            .apply_direct_palette_command(command_id, bytes);
        self.backdrop_color = SgbRgb555Color::from_packet_bytes(bytes[1], bytes[2]);
        self.colorization_active = true;
        self.last_palette_command_id = Some(command_id);
        self.palette_command_count = self.palette_command_count.saturating_add(1);
        self.apply_pal_pri_application_priority();
    }

    pub(in crate::sgb::host) fn apply_pal_set_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        let options = self
            .system_palettes
            .apply_pal_set(&mut self.palette_state, bytes);
        self.backdrop_color = self
            .system_palettes
            .color_zero_for_last_pal_set()
            .unwrap_or_else(|| self.palette_state.palette(0).color(0));
        self.palette_state
            .set_shared_color_zero(self.backdrop_color);
        self.colorization_active = true;
        self.last_palette_command_id = Some(SGB_COMMAND_PAL_SET);
        self.palette_command_count = self.palette_command_count.saturating_add(1);
        if options.cancel_mask {
            self.cancel_mask();
        }
        if options.apply_atf {
            self.apply_atf_index(options.atf_index);
        }
        self.apply_pal_pri_application_priority();
    }

    pub(in crate::sgb::host) fn apply_pal_pri_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.system_palettes.apply_pal_pri(bytes);
    }

    pub(in crate::sgb::host) fn apply_attr_blk_command(&mut self, payload: &[u8]) {
        self.attributes.apply_attr_blk(payload);
        self.colorization_active = true;
        self.apply_pal_pri_application_priority();
    }

    pub(in crate::sgb::host) fn apply_attr_lin_command(&mut self, payload: &[u8]) {
        self.attributes.apply_attr_lin(payload);
        self.colorization_active = true;
        self.apply_pal_pri_application_priority();
    }

    pub(in crate::sgb::host) fn apply_attr_div_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.attributes.apply_attr_div(bytes);
        self.colorization_active = true;
        self.apply_pal_pri_application_priority();
    }

    pub(in crate::sgb::host) fn apply_attr_chr_command(&mut self, payload: &[u8]) {
        self.attributes.apply_attr_chr(payload);
        self.colorization_active = true;
        self.apply_pal_pri_application_priority();
    }

    pub(in crate::sgb::host) fn apply_attr_set_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        let atf_index = bytes[1] & 0x3F;
        if bytes[1] & 0x40 != 0 {
            self.cancel_mask();
        }
        if self.apply_atf_index(atf_index) {
            self.colorization_active = true;
        }
        self.apply_pal_pri_application_priority();
    }

    fn apply_atf_index(&mut self, atf_index: u8) -> bool {
        self.attributes.apply_attr_set(atf_index)
    }

    fn apply_pal_pri_application_priority(&mut self) {
        if self.system_palettes.pal_pri_enabled {
            self.player_palette_override
                .return_to_application_due_to_pal_pri();
        }
    }

    fn cancel_mask(&mut self) {
        self.mask = SgbScreenMask::Cancel;
        self.freeze_capture_pending = false;
        self.frozen_lcd = None;
    }

    pub(in crate::sgb::host) fn apply_mask_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.mask = SgbScreenMask::from_command_byte(bytes[1]);
        self.mask_command_count = self.mask_command_count.saturating_add(1);
        self.freeze_capture_pending = self.mask == SgbScreenMask::Freeze;
        if self.mask != SgbScreenMask::Freeze {
            self.frozen_lcd = None;
        }
    }

    pub(in crate::sgb::host) fn apply_obj_trn_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.obj.apply_obj_trn(&self.system_palettes, bytes);
    }

    pub(in crate::sgb::host) fn request_chr_transfer(
        &mut self,
        command_id: u8,
        bytes: &[u8; SGB_PACKET_BYTES],
    ) {
        self.request_vram_transfer(
            command_id,
            SgbVramTransferTarget::Chr(SgbChrTransferSelection::from_command_byte(bytes[1])),
        );
    }

    pub(in crate::sgb::host) fn request_pct_transfer(&mut self, command_id: u8) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::Pct);
    }

    pub(in crate::sgb) fn request_pal_transfer(&mut self, command_id: u8) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::Pal);
    }

    pub(in crate::sgb::host) fn request_attr_transfer(&mut self, command_id: u8) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::Attr);
    }

    pub(in crate::sgb::host) fn request_sound_transfer(&mut self, command_id: u8) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::Sound);
    }

    pub(in crate::sgb::host) fn request_snes_data_transfer(
        &mut self,
        command_id: u8,
        destination: SgbSnesAddress,
    ) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::SnesData(destination));
    }

    fn request_vram_transfer(&mut self, command_id: u8, target: SgbVramTransferTarget) {
        self.vram_transfer.pending = Some(SgbPendingVramTransfer {
            command_id,
            target,
            frame_starts_until_capture: 1,
            phase: SgbVramTransferPhase::WaitingForNextFrame,
            frames_captured: 0,
            total_frames: SGB_VRAM_TRANSFER_TOTAL_FRAMES,
            source_mode: SgbVramTransferSourceMode::Unresolved,
        });
        self.vram_transfer.partial_payload = Some(SgbVramTransferBuffer::default());
        self.vram_transfer.display_order_payload = None;
        self.vram_transfer.requested_transfer_count = self
            .vram_transfer
            .requested_transfer_count
            .saturating_add(1);
    }

    fn advance_frame_start(
        &mut self,
        vram_bytes: &[u8],
        display: SgbVramTransferDisplayState,
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        let completed_target = self.advance_pending_vram_transfer(vram_bytes, display)?;
        self.obj.capture_frame(vram_bytes, display)?;
        Ok(completed_target)
    }

    fn advance_pending_vram_transfer(
        &mut self,
        vram_bytes: &[u8],
        display: SgbVramTransferDisplayState,
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        let Some(mut pending) = self.vram_transfer.pending else {
            return Ok(None);
        };
        if pending.frame_starts_until_capture > 1 {
            pending.frame_starts_until_capture -= 1;
            self.vram_transfer.pending = Some(pending);
            return Ok(None);
        }
        let (payload, source_mode) = self.vram_transfer_payload(vram_bytes, display, pending)?;
        if pending.source_mode == SgbVramTransferSourceMode::Unresolved {
            pending.source_mode = source_mode;
        }
        pending.frame_starts_until_capture = 0;
        pending.phase = SgbVramTransferPhase::Capturing;
        let frame_index = pending
            .frames_captured
            .min(pending.total_frames.saturating_sub(1));
        let (chunk_start, chunk_end) =
            vram_transfer_chunk_range(frame_index, pending.total_frames.max(1));
        {
            let partial_payload = self
                .vram_transfer
                .partial_payload
                .get_or_insert_with(SgbVramTransferBuffer::default);
            partial_payload.bytes[chunk_start..chunk_end]
                .copy_from_slice(&payload.bytes[chunk_start..chunk_end]);
        }
        pending.frames_captured = pending.frames_captured.saturating_add(1);
        if pending.frames_captured >= pending.total_frames {
            let final_payload = self.vram_transfer.partial_payload.take().unwrap_or(payload);
            self.complete_pending_vram_transfer_payload(final_payload)
        } else {
            self.vram_transfer.pending = Some(pending);
            Ok(None)
        }
    }

    fn vram_transfer_payload(
        &mut self,
        vram_bytes: &[u8],
        display: SgbVramTransferDisplayState,
        pending: SgbPendingVramTransfer,
    ) -> Result<(SgbVramTransferBuffer, SgbVramTransferSourceMode), SgbVramTransferError> {
        let lcd_disabled_tail = pending.source_mode == SgbVramTransferSourceMode::DisplayOrder
            && display.lcdc & SGB_LCDC_ENABLE_BIT == 0
            && display.can_extract_display_order(true);

        if lcd_disabled_tail && let Some(payload) = self.vram_transfer.display_order_payload.clone()
        {
            return Ok((payload, SgbVramTransferSourceMode::DisplayOrder));
        }

        let (payload, source_mode) = SgbVramTransferBuffer::from_display_memory_with_source_mode(
            vram_bytes,
            display,
            pending.source_mode,
        )?;
        if source_mode == SgbVramTransferSourceMode::DisplayOrder
            && display.lcdc & SGB_LCDC_ENABLE_BIT != 0
        {
            self.vram_transfer.display_order_payload = Some(payload.clone());
        }
        Ok((payload, source_mode))
    }

    #[cfg(test)]
    fn capture_pending_vram_transfer(
        &mut self,
        vram_bytes: &[u8],
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        let payload = SgbVramTransferBuffer::from_source_bytes(vram_bytes)?;
        self.complete_pending_vram_transfer_payload(payload)
    }

    fn complete_pending_vram_transfer_payload(
        &mut self,
        payload: SgbVramTransferBuffer,
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        let Some(pending) = self.vram_transfer.pending.take() else {
            return Err(SgbVramTransferError::NoPendingTransfer);
        };
        self.vram_transfer.partial_payload = None;
        self.vram_transfer.display_order_payload = None;
        match pending.target {
            SgbVramTransferTarget::Chr(selection) => {
                self.border.apply_chr_transfer(selection, &payload);
            }
            SgbVramTransferTarget::Pct => {
                self.border.apply_pct_transfer(&payload);
                self.border_loaded = true;
            }
            SgbVramTransferTarget::Pal => {
                self.system_palettes.apply_pal_trn(&payload);
            }
            SgbVramTransferTarget::Attr => {
                self.attributes.apply_attr_trn(&payload);
            }
            SgbVramTransferTarget::Sound | SgbVramTransferTarget::SnesData(_) => {}
        }
        self.vram_transfer.last_completed = Some(SgbCompletedVramTransfer {
            command_id: pending.command_id,
            target: pending.target,
            payload,
        });
        self.vram_transfer.completed_transfer_count = self
            .vram_transfer
            .completed_transfer_count
            .saturating_add(1);
        Ok(Some(pending.target))
    }

    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.frozen_lcd
            .as_ref()
            .map(SgbLcdRgb555Frame::dynamic_payload_bytes)
            .unwrap_or(0)
            .saturating_add(self.vram_transfer.dynamic_payload_bytes())
            .saturating_add(self.system_palettes.dynamic_payload_bytes())
            .saturating_add(self.player_palette_override.dynamic_payload_bytes())
            .saturating_add(self.attributes.dynamic_payload_bytes())
            .saturating_add(self.border.dynamic_payload_bytes())
            .saturating_add(self.obj.dynamic_payload_bytes())
    }
}

impl Default for SgbVideoState {
    fn default() -> Self {
        Self::default_for_active_host(false)
    }
}
