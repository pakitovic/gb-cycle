use super::*;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbPacketGateState {
    pub busy_frames_remaining: u8,
    pub busy_rejected_packet_count: u64,
    pub icon_suppressed_packet_count: u64,
    pub last_busy_command_id: Option<u8>,
    pub last_suppressed_command_id: Option<u8>,
}

impl SgbPacketGateState {
    pub(in crate::sgb::host) fn start_busy_frames(&mut self, frames: u8) {
        self.busy_frames_remaining = self.busy_frames_remaining.max(frames);
    }

    pub(in crate::sgb::host) fn clear_busy(&mut self) {
        self.busy_frames_remaining = 0;
    }

    pub(in crate::sgb::host) fn advance_frame(&mut self) {
        self.busy_frames_remaining = self.busy_frames_remaining.saturating_sub(1);
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbPacketTransportState {
    pub last_joyp_line_state: SgbJoypLineState,
    pub phase: SgbPacketTransportPhase,
    pub transfer_active: bool,
    pub pending_data_bit: Option<u8>,
    pub packet_bits_buffered: u8,
    pub packet_bytes_buffered: u8,
    pub current_packet: [u8; SGB_PACKET_BYTES],
    pub reset_pulse_count: u64,
    pub data_pulse_count: u64,
    pub invalid_pulse_count: u64,
    pub invalid_stop_bit_count: u64,
    pub last_trace: SgbPacketTrace,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbPacketTrace {
    pub status: SgbPacketTraceStatus,
    pub command_id: Option<u8>,
    pub packet_count: u8,
    pub packet_index: u8,
    pub bits_buffered: u8,
    pub bytes: [u8; SGB_PACKET_BYTES],
}

impl SgbHost {
    pub(crate) fn observe_joyp_write(&mut self, value: u8) {
        if !self.host_platform.is_sgb() {
            return;
        }

        let line_state = SgbJoypLineState::from_joyp_value(value);
        let previous_line_state = self.packet_transport.last_joyp_line_state;
        self.multiplayer
            .observe_joyp_write(previous_line_state, value);
        match line_state {
            SgbJoypLineState::Idle => {
                self.observe_joyp_idle();
            }
            SgbJoypLineState::Start => {
                if previous_line_state != SgbJoypLineState::Start {
                    self.observe_joyp_start();
                }
            }
            SgbJoypLineState::Zero | SgbJoypLineState::One => {
                self.observe_joyp_data_candidate(line_state.data_bit().expect("data line"));
            }
            SgbJoypLineState::Invalid => {
                self.record_packet_trace(SgbPacketTraceStatus::ConflictingPulse);
                self.packet_transport.invalid_pulse_count =
                    self.packet_transport.invalid_pulse_count.saturating_add(1);
                self.command.invalid_packet_count =
                    self.command.invalid_packet_count.saturating_add(1);
            }
        }
        self.packet_transport.last_joyp_line_state = line_state;
    }

    fn observe_joyp_idle(&mut self) {
        match self.packet_transport.phase {
            SgbPacketTransportPhase::Idle | SgbPacketTransportPhase::Receiving => {}
            SgbPacketTransportPhase::StartPending => {
                self.confirm_packet_start();
            }
            SgbPacketTransportPhase::DataPending => {
                if let Some(bit) = self.packet_transport.pending_data_bit.take() {
                    self.confirm_packet_data_bit(bit);
                }
            }
        }
    }

    fn observe_joyp_start(&mut self) {
        match self.packet_transport.phase {
            SgbPacketTransportPhase::StartPending => {}
            SgbPacketTransportPhase::Idle => self.begin_packet_start_pulse(false),
            SgbPacketTransportPhase::Receiving | SgbPacketTransportPhase::DataPending => {
                self.begin_packet_start_pulse(true);
            }
        }
    }

    fn observe_joyp_data_candidate(&mut self, bit: u8) {
        match self.packet_transport.phase {
            SgbPacketTransportPhase::Receiving | SgbPacketTransportPhase::DataPending => {
                self.packet_transport.phase = SgbPacketTransportPhase::DataPending;
                self.packet_transport.pending_data_bit = Some(bit);
                self.packet_transport.data_pulse_count =
                    self.packet_transport.data_pulse_count.saturating_add(1);
            }
            SgbPacketTransportPhase::StartPending => {
                self.record_packet_trace(SgbPacketTraceStatus::ConflictingPulse);
                self.packet_transport.invalid_pulse_count =
                    self.packet_transport.invalid_pulse_count.saturating_add(1);
                self.command.invalid_packet_count =
                    self.command.invalid_packet_count.saturating_add(1);
                self.packet_transport.phase = SgbPacketTransportPhase::Idle;
                self.packet_transport.transfer_active = false;
                self.packet_transport.pending_data_bit = None;
            }
            SgbPacketTransportPhase::Idle => {
                self.record_packet_trace(SgbPacketTraceStatus::OrphanDataPulse);
                self.packet_transport.invalid_pulse_count =
                    self.packet_transport.invalid_pulse_count.saturating_add(1);
                self.command.invalid_packet_count =
                    self.command.invalid_packet_count.saturating_add(1);
            }
        }
    }

    fn begin_packet_start_pulse(&mut self, incomplete_reset: bool) {
        if incomplete_reset
            && (self.packet_transport.packet_bits_buffered != 0
                || self.packet_transport.pending_data_bit.is_some())
        {
            self.record_packet_trace(SgbPacketTraceStatus::IncompleteReset);
            self.command.invalid_packet_count = self.command.invalid_packet_count.saturating_add(1);
        }

        self.packet_transport.phase = SgbPacketTransportPhase::StartPending;
        self.packet_transport.transfer_active = false;
        self.packet_transport.pending_data_bit = None;
        self.packet_transport.packet_bits_buffered = 0;
        self.packet_transport.packet_bytes_buffered = 0;
        self.packet_transport.current_packet = [0; SGB_PACKET_BYTES];
        self.packet_transport.reset_pulse_count =
            self.packet_transport.reset_pulse_count.saturating_add(1);
    }

    fn confirm_packet_start(&mut self) {
        self.packet_transport.phase = SgbPacketTransportPhase::Receiving;
        self.packet_transport.transfer_active = true;
    }

    fn confirm_packet_data_bit(&mut self, bit: u8) {
        if self.packet_transport.packet_bits_buffered < SGB_PACKET_BITS {
            let bit_index = self.packet_transport.packet_bits_buffered;
            if bit != 0 {
                let byte_index = usize::from(bit_index / 8);
                let bit_in_byte = bit_index % 8;
                self.packet_transport.current_packet[byte_index] |= 1 << bit_in_byte;
            }
            self.packet_transport.packet_bits_buffered =
                self.packet_transport.packet_bits_buffered.saturating_add(1);
            self.packet_transport.packet_bytes_buffered =
                self.packet_transport.packet_bits_buffered.div_ceil(8);
            self.packet_transport.phase = SgbPacketTransportPhase::Receiving;
            return;
        }

        if bit != 0 {
            self.record_packet_trace(SgbPacketTraceStatus::InvalidStopBit);
            self.packet_transport.invalid_pulse_count =
                self.packet_transport.invalid_pulse_count.saturating_add(1);
            self.packet_transport.invalid_stop_bit_count = self
                .packet_transport
                .invalid_stop_bit_count
                .saturating_add(1);
            self.command.invalid_packet_count = self.command.invalid_packet_count.saturating_add(1);
        }
        self.complete_packet_transfer();
    }

    fn complete_packet_transfer(&mut self) {
        self.packet_transport.phase = SgbPacketTransportPhase::Idle;
        self.packet_transport.transfer_active = false;
        self.packet_transport.pending_data_bit = None;
        let bytes = self.packet_transport.current_packet;
        self.decode_complete_packet(bytes);
    }
}
