use super::*;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbSystemControlState {
    pub attraction_disabled: bool,
    pub atrc_en_count: u64,
    pub test_mode_enabled: bool,
    pub test_en_count: u64,
    pub icon_disable_bits: u8,
    pub icon_en_count: u64,
    pub last_system_command_id: Option<u8>,
}

impl SgbSystemControlState {
    fn apply_atrc_en(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.attraction_disabled = bytes[1] & 0x01 != 0;
        self.atrc_en_count = self.atrc_en_count.saturating_add(1);
        self.last_system_command_id = Some(SGB_COMMAND_ATRC_EN);
    }

    fn apply_test_en(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.test_mode_enabled = bytes[1] & 0x01 != 0;
        self.test_en_count = self.test_en_count.saturating_add(1);
        self.last_system_command_id = Some(SGB_COMMAND_TEST_EN);
    }

    fn apply_icon_en(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.icon_disable_bits = bytes[1] & 0x7F;
        self.icon_en_count = self.icon_en_count.saturating_add(1);
        self.last_system_command_id = Some(SGB_COMMAND_ICON_EN);
    }

    const fn register_file_transfer_disabled(self) -> bool {
        self.icon_disable_bits & 0x04 != 0
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbCommandState {
    pub active_command_id: Option<u8>,
    pub expected_packet_count: u8,
    pub received_packet_count: u8,
    pub packet_buffer: [[u8; SGB_COMMAND_PACKET_BYTES]; SGB_COMMAND_MAX_PACKETS],
    pub last_command_id: Option<u8>,
    pub accepted_command_count: u64,
    pub rejected_packet_count: u64,
    pub invalid_packet_count: u64,
}

impl SgbCommandState {
    fn command_payload(&self, packet_count: u8) -> Vec<u8> {
        let packet_count = packet_count.min(SGB_PACKET_COUNT_MAX);
        let mut payload = Vec::with_capacity(15 + usize::from(packet_count.saturating_sub(1)) * 16);
        payload.extend_from_slice(&self.packet_buffer[0][1..]);
        for packet_index in 1..usize::from(packet_count) {
            payload.extend_from_slice(&self.packet_buffer[packet_index]);
        }
        payload
    }
}

impl SgbHost {
    pub(in crate::sgb::host) fn decode_complete_packet(&mut self, bytes: [u8; SGB_PACKET_BYTES]) {
        if self.startup.command_acceptance != SgbCommandAcceptance::Accepted {
            self.command.rejected_packet_count =
                self.command.rejected_packet_count.saturating_add(1);
            self.packet_transport.last_trace = SgbPacketTrace {
                status: SgbPacketTraceStatus::RejectedByHeader,
                command_id: Some(bytes[0] >> 3),
                packet_count: bytes[0] & 0x07,
                packet_index: self.command.received_packet_count.saturating_add(1),
                bits_buffered: self.packet_transport.packet_bits_buffered,
                bytes,
            };
            return;
        }

        if self.command.active_command_id.is_none() {
            let command_id = bytes[0] >> 3;
            let packet_count = bytes[0] & 0x07;
            if self.system.register_file_transfer_disabled() {
                self.packet_gate.icon_suppressed_packet_count = self
                    .packet_gate
                    .icon_suppressed_packet_count
                    .saturating_add(1);
                self.packet_gate.last_suppressed_command_id = Some(command_id);
                self.packet_transport.last_trace = SgbPacketTrace {
                    status: SgbPacketTraceStatus::SuppressedByIcon,
                    command_id: Some(command_id),
                    packet_count,
                    packet_index: 1,
                    bits_buffered: self.packet_transport.packet_bits_buffered,
                    bytes,
                };
                return;
            }
            if self.packet_gate.busy_frames_remaining != 0 {
                self.packet_gate.busy_rejected_packet_count = self
                    .packet_gate
                    .busy_rejected_packet_count
                    .saturating_add(1);
                self.packet_gate.last_busy_command_id = Some(command_id);
                self.packet_transport.last_trace = SgbPacketTrace {
                    status: SgbPacketTraceStatus::RejectedWhileBusy,
                    command_id: Some(command_id),
                    packet_count,
                    packet_index: 1,
                    bits_buffered: self.packet_transport.packet_bits_buffered,
                    bytes,
                };
                return;
            }
            if !(SGB_PACKET_COUNT_MIN..=SGB_PACKET_COUNT_MAX).contains(&packet_count) {
                self.command.invalid_packet_count =
                    self.command.invalid_packet_count.saturating_add(1);
                self.packet_transport.last_trace = SgbPacketTrace {
                    status: SgbPacketTraceStatus::InvalidPacketLength,
                    command_id: Some(command_id),
                    packet_count,
                    packet_index: 1,
                    bits_buffered: self.packet_transport.packet_bits_buffered,
                    bytes,
                };
                return;
            }

            self.command.expected_packet_count = packet_count;
            self.command.received_packet_count = 1;
            self.command.packet_buffer = [[0; SGB_COMMAND_PACKET_BYTES]; SGB_COMMAND_MAX_PACKETS];
            self.command.packet_buffer[0] = bytes;
            self.packet_transport.last_trace = SgbPacketTrace {
                status: SgbPacketTraceStatus::Complete,
                command_id: Some(command_id),
                packet_count,
                packet_index: 1,
                bits_buffered: self.packet_transport.packet_bits_buffered,
                bytes,
            };

            if packet_count == 1 {
                self.complete_accepted_command(command_id, packet_count);
            } else {
                self.command.active_command_id = Some(command_id);
            }
            return;
        }

        let command_id = self.command.active_command_id;
        let packet_count = self.command.expected_packet_count;
        self.command.received_packet_count = self.command.received_packet_count.saturating_add(1);
        if self.command.received_packet_count <= SGB_PACKET_COUNT_MAX {
            let packet_index = usize::from(self.command.received_packet_count - 1);
            self.command.packet_buffer[packet_index] = bytes;
        }
        self.packet_transport.last_trace = SgbPacketTrace {
            status: SgbPacketTraceStatus::Complete,
            command_id,
            packet_count,
            packet_index: self.command.received_packet_count,
            bits_buffered: self.packet_transport.packet_bits_buffered,
            bytes,
        };

        if self.command.received_packet_count >= self.command.expected_packet_count
            && let Some(command_id) = command_id
        {
            self.complete_accepted_command(command_id, packet_count);
        }
    }

    fn complete_accepted_command(&mut self, command_id: u8, packet_count: u8) {
        self.command.last_command_id = Some(command_id);
        self.command.accepted_command_count = self.command.accepted_command_count.saturating_add(1);
        self.dispatch_completed_command(command_id, packet_count);
        self.command.active_command_id = None;
    }

    fn dispatch_completed_command(&mut self, command_id: u8, packet_count: u8) {
        if direct_palette_command_pair(command_id).is_some() && packet_count == 1 {
            self.video
                .apply_direct_palette_command(command_id, &self.command.packet_buffer[0]);
            return;
        }

        match command_id {
            SGB_COMMAND_ATTR_BLK => {
                let payload = self.command.command_payload(packet_count);
                self.video.apply_attr_blk_command(&payload);
            }
            SGB_COMMAND_ATTR_LIN => {
                let payload = self.command.command_payload(packet_count);
                self.video.apply_attr_lin_command(&payload);
            }
            SGB_COMMAND_ATTR_DIV if packet_count == 1 => self
                .video
                .apply_attr_div_command(&self.command.packet_buffer[0]),
            SGB_COMMAND_ATTR_CHR => {
                let payload = self.command.command_payload(packet_count);
                self.video.apply_attr_chr_command(&payload);
            }
            SGB_COMMAND_SOUND if packet_count == 1 => {
                self.dispatch_host_backend_request(SgbHostBackendRequest::Audio(
                    SgbHostAudioRequest::Sound(SgbSoundRequest::from_packet(
                        &self.command.packet_buffer[0],
                    )),
                ));
            }
            SGB_COMMAND_SOU_TRN if packet_count == 1 => {
                self.video.request_sound_transfer(command_id);
                self.packet_gate
                    .start_busy_frames(SGB_VRAM_TRANSFER_TOTAL_FRAMES);
            }
            SGB_COMMAND_ATRC_EN if packet_count == 1 => {
                self.system.apply_atrc_en(&self.command.packet_buffer[0]);
            }
            SGB_COMMAND_TEST_EN if packet_count == 1 => {
                self.system.apply_test_en(&self.command.packet_buffer[0]);
            }
            SGB_COMMAND_ICON_EN if packet_count == 1 => {
                self.system.apply_icon_en(&self.command.packet_buffer[0]);
            }
            SGB_COMMAND_PAL_SET if packet_count == 1 => {
                self.video
                    .apply_pal_set_command(&self.command.packet_buffer[0]);
            }
            SGB_COMMAND_PAL_TRN if packet_count == 1 => {
                self.video.request_pal_transfer(command_id);
                self.packet_gate
                    .start_busy_frames(SGB_VRAM_TRANSFER_TOTAL_FRAMES);
            }
            SGB_COMMAND_DATA_SND if packet_count == 1 => {
                self.dispatch_host_backend_request(SgbHostBackendRequest::Snes(
                    SgbSnesHostRequest::DataSend(SgbDataSendRequest::from_packet(
                        &self.command.packet_buffer[0],
                    )),
                ));
            }
            SGB_COMMAND_DATA_TRN if packet_count == 1 => {
                self.video.request_snes_data_transfer(
                    command_id,
                    SgbSnesAddress::from_packet_bytes(
                        self.command.packet_buffer[0][1],
                        self.command.packet_buffer[0][2],
                        self.command.packet_buffer[0][3],
                    ),
                );
                self.packet_gate
                    .start_busy_frames(SGB_VRAM_TRANSFER_TOTAL_FRAMES);
            }
            SGB_COMMAND_MLT_REQ if packet_count == 1 => self
                .multiplayer
                .apply_mlt_req_command(&self.command.packet_buffer[0]),
            SGB_COMMAND_JUMP if packet_count == 1 => {
                self.dispatch_host_backend_request(SgbHostBackendRequest::Snes(
                    SgbSnesHostRequest::Jump(SgbJumpRequest::from_packet(
                        &self.command.packet_buffer[0],
                    )),
                ));
            }
            SGB_COMMAND_CHR_TRN if packet_count == 1 => {
                self.shell.start_game_border_fade_out(&self.video.border);
                self.video
                    .request_chr_transfer(command_id, &self.command.packet_buffer[0]);
                self.packet_gate
                    .start_busy_frames(SGB_VRAM_TRANSFER_TOTAL_FRAMES);
            }
            SGB_COMMAND_PCT_TRN if packet_count == 1 => {
                self.shell.start_game_border_fade_out(&self.video.border);
                self.video.request_pct_transfer(command_id);
                self.packet_gate
                    .start_busy_frames(SGB_VRAM_TRANSFER_TOTAL_FRAMES);
            }
            SGB_COMMAND_ATTR_TRN if packet_count == 1 => {
                self.video.request_attr_transfer(command_id);
                self.packet_gate
                    .start_busy_frames(SGB_VRAM_TRANSFER_TOTAL_FRAMES);
            }
            SGB_COMMAND_ATTR_SET if packet_count == 1 => self
                .video
                .apply_attr_set_command(&self.command.packet_buffer[0]),
            SGB_COMMAND_MASK_EN if packet_count == 1 => self
                .video
                .apply_mask_command(&self.command.packet_buffer[0]),
            SGB_COMMAND_OBJ_TRN if packet_count == 1 => {
                self.video
                    .apply_obj_trn_command(&self.command.packet_buffer[0]);
                self.packet_gate.start_busy_frames(SGB_OBJ_TRN_BUSY_FRAMES);
            }
            SGB_COMMAND_PAL_PRI if packet_count == 1 => self
                .video
                .apply_pal_pri_command(&self.command.packet_buffer[0]),
            _ => {}
        }
    }

    pub(in crate::sgb::host) fn dispatch_host_backend_request(
        &mut self,
        request: SgbHostBackendRequest,
    ) -> SgbHostBackendResponse {
        let mut backend = DeterministicHleSgbHostBackend;
        let response = backend.handle_request(request, &mut self.audio, &mut self.snes_host);
        self.backend_kind = response.backend_kind;
        response
    }

    pub(in crate::sgb::host) fn record_packet_trace(&mut self, status: SgbPacketTraceStatus) {
        self.packet_transport.last_trace = SgbPacketTrace {
            status,
            command_id: self.command.active_command_id,
            packet_count: self.command.expected_packet_count,
            packet_index: self.command.received_packet_count,
            bits_buffered: self.packet_transport.packet_bits_buffered,
            bytes: self.packet_transport.current_packet,
        };
    }
}
