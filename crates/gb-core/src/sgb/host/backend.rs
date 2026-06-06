use super::*;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbAudioState {
    pub pending_host_audio_events: u8,
    pub last_request: Option<SgbHostAudioRequest>,
    pub sound_command_count: u64,
    pub sound_transfer_count: u64,
    pub transferred_payload_bytes: u32,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbSnesHostState {
    pub execution_enabled: bool,
    pub uploaded_payload_bytes: u32,
    pub last_request: Option<SgbSnesHostRequest>,
    pub data_snd_count: u64,
    pub data_trn_count: u64,
    pub jump_count: u64,
    pub program_counter: Option<SgbSnesAddress>,
    pub nmi_handler: Option<SgbSnesAddress>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostBackendKind {
    DeterministicHle,
}

pub trait SgbHostBackend {
    fn backend_kind(&self) -> SgbHostBackendKind;

    fn handle_request(
        &mut self,
        request: SgbHostBackendRequest,
        audio: &mut SgbAudioState,
        snes_host: &mut SgbSnesHostState,
    ) -> SgbHostBackendResponse;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DeterministicHleSgbHostBackend;

impl SgbHostBackend for DeterministicHleSgbHostBackend {
    fn backend_kind(&self) -> SgbHostBackendKind {
        SgbHostBackendKind::DeterministicHle
    }

    fn handle_request(
        &mut self,
        request: SgbHostBackendRequest,
        audio: &mut SgbAudioState,
        snes_host: &mut SgbSnesHostState,
    ) -> SgbHostBackendResponse {
        match request {
            SgbHostBackendRequest::Audio(request) => audio.record_request(request),
            SgbHostBackendRequest::Snes(request) => snes_host.record_request(request),
        }
        SgbHostBackendResponse {
            backend_kind: self.backend_kind(),
            request_kind: request.kind(),
            accepted: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostBackendRequestKind {
    Sound,
    SoundTransfer,
    DataSend,
    DataTransfer,
    Jump,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostBackendRequest {
    Audio(SgbHostAudioRequest),
    Snes(SgbSnesHostRequest),
}

impl SgbHostBackendRequest {
    pub const fn kind(self) -> SgbHostBackendRequestKind {
        match self {
            Self::Audio(request) => request.kind(),
            Self::Snes(request) => request.kind(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbHostBackendResponse {
    pub backend_kind: SgbHostBackendKind,
    pub request_kind: SgbHostBackendRequestKind,
    pub accepted: bool,
}

impl SgbHostAudioRequest {
    pub const fn kind(self) -> SgbHostBackendRequestKind {
        match self {
            Self::Sound(_) => SgbHostBackendRequestKind::Sound,
            Self::SoundTransfer(_) => SgbHostBackendRequestKind::SoundTransfer,
        }
    }
}

impl SgbSnesHostRequest {
    pub const fn kind(self) -> SgbHostBackendRequestKind {
        match self {
            Self::DataSend(_) => SgbHostBackendRequestKind::DataSend,
            Self::DataTransfer(_) => SgbHostBackendRequestKind::DataTransfer,
            Self::Jump(_) => SgbHostBackendRequestKind::Jump,
        }
    }
}

impl SgbAudioState {
    pub(in crate::sgb) fn record_request(&mut self, request: SgbHostAudioRequest) {
        self.last_request = Some(request);
        self.pending_host_audio_events = self.pending_host_audio_events.saturating_add(1);
        match request {
            SgbHostAudioRequest::Sound(_) => {
                self.sound_command_count = self.sound_command_count.saturating_add(1);
            }
            SgbHostAudioRequest::SoundTransfer(request) => {
                self.sound_transfer_count = self.sound_transfer_count.saturating_add(1);
                self.transferred_payload_bytes = self
                    .transferred_payload_bytes
                    .saturating_add(request.payload_bytes);
            }
        }
    }
}

impl SgbSnesHostState {
    pub(in crate::sgb) fn record_request(&mut self, request: SgbSnesHostRequest) {
        self.last_request = Some(request);
        match request {
            SgbSnesHostRequest::DataSend(request) => {
                self.data_snd_count = self.data_snd_count.saturating_add(1);
                self.uploaded_payload_bytes = self
                    .uploaded_payload_bytes
                    .saturating_add(request.payload_len() as u32);
            }
            SgbSnesHostRequest::DataTransfer(request) => {
                self.data_trn_count = self.data_trn_count.saturating_add(1);
                self.uploaded_payload_bytes = self
                    .uploaded_payload_bytes
                    .saturating_add(request.payload_bytes);
            }
            SgbSnesHostRequest::Jump(request) => {
                self.jump_count = self.jump_count.saturating_add(1);
                self.execution_enabled = true;
                self.program_counter = Some(request.program_counter);
                self.nmi_handler = Some(request.nmi_handler);
            }
        }
    }
}
