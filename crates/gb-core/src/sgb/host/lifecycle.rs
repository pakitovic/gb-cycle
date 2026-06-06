use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbStartupState {
    pub startup_mode: StartupMode,
    pub real_boot_asset: Option<SgbRealBootAsset>,
    pub cartridge_sgb_flag: Option<SgbFlag>,
    pub old_licensee_code: Option<u8>,
    pub command_acceptance: SgbCommandAcceptance,
}

impl Default for SgbHost {
    fn default() -> Self {
        Self::new(HostPlatform::Handheld)
    }
}

impl SgbHost {
    pub fn new(host_platform: HostPlatform) -> Self {
        Self::new_with_startup(host_platform, StartupMode::SkipBoot)
    }

    pub fn new_with_startup(host_platform: HostPlatform, startup_mode: StartupMode) -> Self {
        Self::new_with_profile(
            host_platform,
            SgbHostProfile::default_for_host_platform(host_platform),
            startup_mode,
        )
    }

    pub fn new_with_profile(
        host_platform: HostPlatform,
        profile: Option<SgbHostProfile>,
        startup_mode: StartupMode,
    ) -> Self {
        let active = host_platform.is_sgb();
        let profile = if active {
            match profile {
                Some(profile) if profile.host_platform() == host_platform => Some(profile),
                _ => SgbHostProfile::default_for_host_platform(host_platform),
            }
        } else {
            None
        };
        Self {
            host_platform,
            profile,
            backend_kind: SgbHostBackendKind::DeterministicHle,
            startup: SgbStartupState::new(active, profile, startup_mode),
            system: SgbSystemControlState::default(),
            packet_gate: SgbPacketGateState::default(),
            packet_transport: SgbPacketTransportState::default(),
            command: SgbCommandState::default(),
            video: SgbVideoState::default_for_active_host(active),
            shell: SgbShellState::default_for_active_host(active),
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

    pub const fn startup(&self) -> SgbStartupState {
        self.startup
    }

    pub const fn command_acceptance(&self) -> SgbCommandAcceptance {
        self.startup.command_acceptance
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
            startup: self.startup,
            system: self.system,
            packet_gate: self.packet_gate,
            packet_transport: self.packet_transport,
            command: self.command,
            video: self.video.clone(),
            shell: self.shell.clone(),
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
            startup: self.startup,
            system: self.system,
            packet_gate: self.packet_gate,
            packet_transport: self.packet_transport,
            command: self.command,
            video: self.video.clone(),
            shell: self.shell.clone(),
            multiplayer: self.multiplayer,
            audio: self.audio,
            snes_host: self.snes_host,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &SgbHostSaveState) {
        self.host_platform = state.host_platform;
        self.profile = state.profile;
        self.backend_kind = state.backend_kind;
        self.startup = state.startup;
        self.system = state.system;
        self.packet_gate = state.packet_gate;
        self.packet_transport = state.packet_transport;
        self.command = state.command;
        self.video = state.video.clone();
        self.shell = state.shell.clone();
        self.multiplayer = state.multiplayer;
        self.audio = state.audio;
        self.snes_host = state.snes_host;
    }

    pub(crate) fn finish_real_boot_handoff(&mut self) {
        if !self.host_platform.is_sgb() {
            return;
        }

        self.packet_transport.last_joyp_line_state = SgbJoypLineState::Idle;
        self.packet_transport.phase = SgbPacketTransportPhase::Idle;
        self.packet_transport.transfer_active = false;
        self.packet_transport.pending_data_bit = None;
        self.packet_transport.packet_bits_buffered = 0;
        self.packet_transport.packet_bytes_buffered = 0;
        self.packet_transport.current_packet = [0; SGB_PACKET_BYTES];
        self.command.active_command_id = None;
        self.command.expected_packet_count = 0;
        self.command.received_packet_count = 0;
        self.command.packet_buffer = [[0; SGB_COMMAND_PACKET_BYTES]; SGB_COMMAND_MAX_PACKETS];
    }

    pub(crate) fn apply_cartridge_header(&mut self, header: Option<&CartridgeHeader>) {
        self.startup.apply_cartridge_header(self.status(), header);
        self.apply_shell_default_border_policy();
        self.video.apply_boot_palette_for_cartridge_header(
            self.status(),
            header,
            self.startup.command_acceptance,
        );
    }

    fn apply_shell_default_border_policy(&mut self) {
        if !self.host_platform.is_sgb() {
            self.shell = SgbShellState::default_for_active_host(false);
            return;
        }

        if self.startup.command_acceptance == SgbCommandAcceptance::RejectedByHeader
            && !self.shell.default_border_loaded
        {
            load_default_border(&mut self.video.border);
            self.video.border_loaded = true;
            self.shell = SgbShellState::default_for_active_host(true);
        }
    }
}

impl SgbHostSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.video
            .dynamic_payload_bytes()
            .saturating_add(self.shell.dynamic_payload_bytes())
    }
}

impl SgbStartupState {
    const fn new(active: bool, profile: Option<SgbHostProfile>, startup_mode: StartupMode) -> Self {
        Self {
            startup_mode,
            real_boot_asset: match (startup_mode, profile) {
                (StartupMode::RealBoot, Some(profile)) => {
                    Some(SgbRealBootAsset::from_profile(profile))
                }
                _ => None,
            },
            cartridge_sgb_flag: None,
            old_licensee_code: None,
            command_acceptance: if active {
                SgbCommandAcceptance::AwaitingCartridgeHeader
            } else {
                SgbCommandAcceptance::Disabled
            },
        }
    }

    fn apply_cartridge_header(
        &mut self,
        host_status: SgbHostStatus,
        header: Option<&CartridgeHeader>,
    ) {
        if host_status == SgbHostStatus::Disabled {
            self.cartridge_sgb_flag = None;
            self.old_licensee_code = None;
            self.command_acceptance = SgbCommandAcceptance::Disabled;
            return;
        }

        let Some(header) = header else {
            self.cartridge_sgb_flag = None;
            self.old_licensee_code = None;
            self.command_acceptance = SgbCommandAcceptance::AwaitingCartridgeHeader;
            return;
        };

        self.cartridge_sgb_flag = Some(header.sgb_flag);
        self.old_licensee_code = Some(header.old_licensee_code);
        self.command_acceptance = if header.sgb_flag == SgbFlag::Supported
            && header.old_licensee_code == SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED
        {
            SgbCommandAcceptance::Accepted
        } else {
            SgbCommandAcceptance::RejectedByHeader
        };
    }
}
