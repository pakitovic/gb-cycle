use crate::cartridge::{CartridgeHeader, SgbFlag};
use crate::joypad::{JoypadButton, button_mask};
use crate::model::{HostPlatform, SgbHostProfile, StartupMode};

use super::protocol::*;
use super::shell::{SgbShellState, load_default_border};

mod backend;
mod command;
mod lifecycle;
mod multiplayer;
mod transport;
mod video;

pub use self::backend::*;
pub use self::command::*;
pub use self::lifecycle::*;
pub use self::multiplayer::*;
pub use self::transport::*;
pub use self::video::*;

#[derive(Debug, Clone)]
pub struct SgbHost {
    host_platform: HostPlatform,
    profile: Option<SgbHostProfile>,
    backend_kind: SgbHostBackendKind,
    startup: SgbStartupState,
    system: SgbSystemControlState,
    packet_gate: SgbPacketGateState,
    packet_transport: SgbPacketTransportState,
    command: SgbCommandState,
    pub(super) video: SgbVideoState,
    shell: SgbShellState,
    multiplayer: SgbMultiplayerState,
    audio: SgbAudioState,
    snes_host: SgbSnesHostState,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbHostSaveState {
    host_platform: HostPlatform,
    profile: Option<SgbHostProfile>,
    backend_kind: SgbHostBackendKind,
    startup: SgbStartupState,
    system: SgbSystemControlState,
    packet_gate: SgbPacketGateState,
    packet_transport: SgbPacketTransportState,
    command: SgbCommandState,
    video: SgbVideoState,
    shell: SgbShellState,
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
    pub startup: SgbStartupState,
    pub system: SgbSystemControlState,
    pub packet_gate: SgbPacketGateState,
    pub packet_transport: SgbPacketTransportState,
    pub command: SgbCommandState,
    pub video: SgbVideoState,
    pub shell: SgbShellState,
    pub multiplayer: SgbMultiplayerState,
    pub audio: SgbAudioState,
    pub snes_host: SgbSnesHostState,
}
