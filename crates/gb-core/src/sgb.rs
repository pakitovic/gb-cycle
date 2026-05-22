use crate::model::HostPlatform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbVideoStandard {
    Ntsc,
    Pal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostProfile {
    SgbNtsc,
    SgbPal,
    Sgb2Ntsc,
}

impl SgbHostProfile {
    pub const ALL: [Self; 3] = [Self::SgbNtsc, Self::SgbPal, Self::Sgb2Ntsc];

    pub const fn default_for_host_platform(host_platform: HostPlatform) -> Option<Self> {
        match host_platform {
            HostPlatform::Handheld => None,
            HostPlatform::Sgb => Some(Self::SgbNtsc),
            HostPlatform::Sgb2 => Some(Self::Sgb2Ntsc),
        }
    }

    pub const fn host_platform(self) -> HostPlatform {
        match self {
            Self::SgbNtsc | Self::SgbPal => HostPlatform::Sgb,
            Self::Sgb2Ntsc => HostPlatform::Sgb2,
        }
    }

    pub const fn video_standard(self) -> SgbVideoStandard {
        match self {
            Self::SgbNtsc | Self::Sgb2Ntsc => SgbVideoStandard::Ntsc,
            Self::SgbPal => SgbVideoStandard::Pal,
        }
    }

    pub const fn ui_label(self) -> &'static str {
        match self {
            Self::SgbNtsc | Self::SgbPal => "SUPER GB",
            Self::Sgb2Ntsc => "SUPER GB 2",
        }
    }

    pub const fn machine_profile_name(self) -> &'static str {
        match self {
            Self::SgbNtsc | Self::SgbPal => "SGB",
            Self::Sgb2Ntsc => "SGB2",
        }
    }

    pub const fn revision_label(self) -> &'static str {
        match self {
            Self::SgbNtsc | Self::SgbPal => "SGB-CPU 01",
            Self::Sgb2Ntsc => "CPU SGB2",
        }
    }

    pub const fn real_boot_filename(self) -> &'static str {
        match self {
            Self::SgbNtsc | Self::SgbPal => "sgb_boot.bin",
            Self::Sgb2Ntsc => "sgb2_boot.bin",
        }
    }

    pub const fn game_link_supported(self) -> bool {
        matches!(self, Self::Sgb2Ntsc)
    }

    pub const fn corrected_clock(self) -> bool {
        matches!(self, Self::Sgb2Ntsc)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostBackendKind {
    DeterministicHle,
}

pub trait SgbHostBackend {
    fn backend_kind(&self) -> SgbHostBackendKind;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DeterministicHleSgbHostBackend;

impl SgbHostBackend for DeterministicHleSgbHostBackend {
    fn backend_kind(&self) -> SgbHostBackendKind {
        SgbHostBackendKind::DeterministicHle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostStatus {
    Disabled,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbHost {
    host_platform: HostPlatform,
    profile: Option<SgbHostProfile>,
    backend_kind: SgbHostBackendKind,
    packet_transport: SgbPacketTransportState,
    command: SgbCommandState,
    video: SgbVideoState,
    multiplayer: SgbMultiplayerState,
    audio: SgbAudioState,
    snes_host: SgbSnesHostState,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbHostSaveState {
    host_platform: HostPlatform,
    profile: Option<SgbHostProfile>,
    backend_kind: SgbHostBackendKind,
    packet_transport: SgbPacketTransportState,
    command: SgbCommandState,
    video: SgbVideoState,
    multiplayer: SgbMultiplayerState,
    audio: SgbAudioState,
    snes_host: SgbSnesHostState,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbHostSnapshot {
    pub host_platform: HostPlatform,
    pub status: SgbHostStatus,
    pub profile: Option<SgbHostProfile>,
    pub backend_kind: SgbHostBackendKind,
    pub packet_transport: SgbPacketTransportState,
    pub command: SgbCommandState,
    pub video: SgbVideoState,
    pub multiplayer: SgbMultiplayerState,
    pub audio: SgbAudioState,
    pub snes_host: SgbSnesHostState,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbPacketTransportState {
    pub packet_bits_buffered: u8,
    pub packet_bytes_buffered: u8,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbCommandState {
    pub last_command_id: Option<u8>,
    pub accepted_command_count: u64,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbVideoState {
    pub border_loaded: bool,
    pub colorization_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbMultiplayerState {
    pub player_count: u8,
    pub selected_player: u8,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbAudioState {
    pub pending_host_audio_events: u8,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbSnesHostState {
    pub execution_enabled: bool,
    pub uploaded_payload_bytes: u32,
}

impl Default for SgbHost {
    fn default() -> Self {
        Self::new(HostPlatform::Handheld)
    }
}

impl SgbHost {
    pub fn new(host_platform: HostPlatform) -> Self {
        Self::new_with_profile(
            host_platform,
            SgbHostProfile::default_for_host_platform(host_platform),
        )
    }

    pub fn new_with_profile(host_platform: HostPlatform, profile: Option<SgbHostProfile>) -> Self {
        debug_assert!(profile.is_none_or(|profile| profile.host_platform() == host_platform));
        let active = host_platform.is_sgb();
        Self {
            host_platform,
            profile: active.then_some(profile).flatten(),
            backend_kind: SgbHostBackendKind::DeterministicHle,
            packet_transport: SgbPacketTransportState::default(),
            command: SgbCommandState::default(),
            video: SgbVideoState::default(),
            multiplayer: SgbMultiplayerState::default_for_active_host(active),
            audio: SgbAudioState::default(),
            snes_host: SgbSnesHostState::default(),
        }
    }

    pub const fn host_platform(&self) -> HostPlatform {
        self.host_platform
    }

    pub const fn status(&self) -> SgbHostStatus {
        if self.host_platform.is_sgb() {
            SgbHostStatus::Ready
        } else {
            SgbHostStatus::Disabled
        }
    }

    pub const fn profile(&self) -> Option<SgbHostProfile> {
        self.profile
    }

    pub const fn backend_kind(&self) -> SgbHostBackendKind {
        self.backend_kind
    }

    pub const fn game_link_supported(&self) -> bool {
        match self.profile {
            Some(profile) => profile.game_link_supported(),
            None => false,
        }
    }

    pub const fn corrected_clock(&self) -> bool {
        match self.profile {
            Some(profile) => profile.corrected_clock(),
            None => false,
        }
    }

    pub fn snapshot(&self) -> SgbHostSnapshot {
        SgbHostSnapshot {
            host_platform: self.host_platform,
            status: self.status(),
            profile: self.profile,
            backend_kind: self.backend_kind,
            packet_transport: self.packet_transport,
            command: self.command,
            video: self.video,
            multiplayer: self.multiplayer,
            audio: self.audio,
            snes_host: self.snes_host,
        }
    }

    pub(crate) fn capture_save_state(&self) -> SgbHostSaveState {
        SgbHostSaveState {
            host_platform: self.host_platform,
            profile: self.profile,
            backend_kind: self.backend_kind,
            packet_transport: self.packet_transport,
            command: self.command,
            video: self.video,
            multiplayer: self.multiplayer,
            audio: self.audio,
            snes_host: self.snes_host,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &SgbHostSaveState) {
        self.host_platform = state.host_platform;
        self.profile = state.profile;
        self.backend_kind = state.backend_kind;
        self.packet_transport = state.packet_transport;
        self.command = state.command;
        self.video = state.video;
        self.multiplayer = state.multiplayer;
        self.audio = state.audio;
        self.snes_host = state.snes_host;
    }
}

impl SgbHostSaveState {
    pub(crate) const fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

impl SgbMultiplayerState {
    const fn default_for_active_host(active: bool) -> Self {
        Self {
            player_count: if active { 1 } else { 0 },
            selected_player: if active { 1 } else { 0 },
        }
    }
}

impl Default for SgbMultiplayerState {
    fn default() -> Self {
        Self::default_for_active_host(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_descriptors_capture_sgb_and_sgb2_capabilities() {
        assert_eq!(SgbHostProfile::ALL.len(), 3);
        assert_eq!(
            SgbHostProfile::default_for_host_platform(HostPlatform::Sgb),
            Some(SgbHostProfile::SgbNtsc)
        );
        assert_eq!(
            SgbHostProfile::default_for_host_platform(HostPlatform::Sgb2),
            Some(SgbHostProfile::Sgb2Ntsc)
        );
        assert_eq!(
            SgbHostProfile::default_for_host_platform(HostPlatform::Handheld),
            None
        );
        assert_eq!(
            SgbHostProfile::SgbPal.video_standard(),
            SgbVideoStandard::Pal
        );
        assert_eq!(SgbHostProfile::SgbNtsc.real_boot_filename(), "sgb_boot.bin");
        assert_eq!(
            SgbHostProfile::Sgb2Ntsc.real_boot_filename(),
            "sgb2_boot.bin"
        );
        assert!(!SgbHostProfile::SgbNtsc.game_link_supported());
        assert!(SgbHostProfile::Sgb2Ntsc.game_link_supported());
        assert!(!SgbHostProfile::SgbNtsc.corrected_clock());
        assert!(SgbHostProfile::Sgb2Ntsc.corrected_clock());
    }

    #[test]
    fn host_state_is_inert_for_handheld_and_ready_for_sgb_profiles() {
        let handheld = SgbHost::new(HostPlatform::Handheld);
        assert_eq!(handheld.status(), SgbHostStatus::Disabled);
        assert_eq!(handheld.profile(), None);
        assert_eq!(handheld.snapshot().multiplayer.player_count, 0);

        let sgb = SgbHost::new(HostPlatform::Sgb);
        assert_eq!(sgb.status(), SgbHostStatus::Ready);
        assert_eq!(sgb.profile(), Some(SgbHostProfile::SgbNtsc));
        assert_eq!(sgb.backend_kind(), SgbHostBackendKind::DeterministicHle);
        assert_eq!(sgb.snapshot().multiplayer.player_count, 1);
        assert!(!sgb.game_link_supported());

        let sgb2 = SgbHost::new(HostPlatform::Sgb2);
        assert_eq!(sgb2.profile(), Some(SgbHostProfile::Sgb2Ntsc));
        assert!(sgb2.game_link_supported());
        assert!(sgb2.corrected_clock());
    }

    #[test]
    fn save_state_restores_the_explicit_host_shell_state() {
        let mut host = SgbHost::new(HostPlatform::Sgb2);
        let saved = host.capture_save_state();
        host = SgbHost::new(HostPlatform::Handheld);
        host.restore_save_state(&saved);
        assert_eq!(host.capture_save_state(), saved);
        assert_eq!(host.profile(), Some(SgbHostProfile::Sgb2Ntsc));
    }

    #[test]
    fn configured_sgb_machines_construct_and_restore_with_host_state() {
        for (host_platform, profile) in [
            (HostPlatform::Sgb, SgbHostProfile::SgbNtsc),
            (HostPlatform::Sgb2, SgbHostProfile::Sgb2Ntsc),
        ] {
            let config = crate::MachineConfig::new(crate::ConsoleModel::GameBoy)
                .with_host_platform(host_platform)
                .with_startup_mode(crate::StartupMode::SkipBoot);
            let mut machine = crate::Machine::new(config.clone());
            assert_eq!(machine.config().operating_mode, crate::OperatingMode::Dmg);
            assert_eq!(machine.sgb_host().profile(), Some(profile));

            let saved = machine.capture_save_state();
            machine.step_t_cycle();
            machine
                .restore_save_state(&saved)
                .expect("matching SGB host save state should restore");
            assert_eq!(machine.capture_save_state(), saved);

            let fresh = crate::Machine::new(config);
            assert_eq!(fresh.sgb_host().profile(), Some(profile));
        }
    }
}
