use crate::cartridge::{CartridgeHeader, SgbFlag};
use crate::joypad::{JoypadButton, button_mask};
use crate::model::{HostPlatform, SgbHostProfile, StartupMode};

const JOYP_SELECT_BITS_MASK: u8 = 0x30;
const SGB_JOYP_IDLE_BITS: u8 = 0x30;
const SGB_JOYP_START_BITS: u8 = 0x00;
const SGB_JOYP_ZERO_BITS: u8 = 0x20;
const SGB_JOYP_ONE_BITS: u8 = 0x10;
pub const SGB_COMMAND_PACKET_BYTES: usize = 16;
pub const SGB_COMMAND_MAX_PACKETS: usize = 7;
pub const SGB_LCD_WIDTH: usize = 160;
pub const SGB_LCD_HEIGHT: usize = 144;
pub const SGB_LCD_PIXELS: usize = SGB_LCD_WIDTH * SGB_LCD_HEIGHT;
pub const SGB_FRAME_WIDTH: usize = 256;
pub const SGB_FRAME_HEIGHT: usize = 224;
pub const SGB_FRAME_PIXELS: usize = SGB_FRAME_WIDTH * SGB_FRAME_HEIGHT;
pub const SGB_LCD_FRAME_ORIGIN_X: usize = 48;
pub const SGB_LCD_FRAME_ORIGIN_Y: usize = 40;
pub const SGB_SCREEN_PALETTE_COUNT: usize = 4;
pub const SGB_SCREEN_PALETTE_COLORS: usize = 4;
pub const SGB_SYSTEM_PALETTE_COUNT: usize = 512;
pub const SGB_ATTR_MAP_WIDTH: usize = 20;
pub const SGB_ATTR_MAP_HEIGHT: usize = 18;
pub const SGB_ATTR_MAP_CELLS: usize = SGB_ATTR_MAP_WIDTH * SGB_ATTR_MAP_HEIGHT;
pub const SGB_ATF_COUNT: usize = 45;
pub const SGB_ATF_BYTES: usize = 90;
pub const SGB_ATF_TOTAL_BYTES: usize = SGB_ATF_COUNT * SGB_ATF_BYTES;
pub const SGB_VRAM_TRANSFER_BYTES: usize = 0x1000;
pub const SGB_BORDER_TILE_BYTES: usize = 32;
pub const SGB_BORDER_TILE_COUNT: usize = 256;
pub const SGB_BORDER_TILE_DATA_BYTES: usize = SGB_BORDER_TILE_BYTES * SGB_BORDER_TILE_COUNT;
pub const SGB_BORDER_TILEMAP_WIDTH: usize = 32;
pub const SGB_BORDER_TILEMAP_VISIBLE_HEIGHT: usize = 28;
pub const SGB_BORDER_TILEMAP_STORED_HEIGHT: usize = 29;
pub const SGB_BORDER_TILEMAP_ENTRIES: usize =
    SGB_BORDER_TILEMAP_WIDTH * SGB_BORDER_TILEMAP_STORED_HEIGHT;
pub const SGB_BORDER_PALETTE_COUNT: usize = 3;
pub const SGB_BORDER_PALETTE_COLORS: usize = 16;
pub const SGB_CONTROLLER_COUNT: usize = 4;
pub const SGB_DATA_SND_INLINE_BYTES: usize = 11;
pub const SGB_SNES_DATA_TRN_BYTES: u32 = SGB_VRAM_TRANSFER_BYTES as u32;

const SGB_TRANSFER_DISPLAY_TILE_COLUMNS: usize = 20;
const SGB_TRANSFER_DISPLAY_TILE_COUNT: usize = SGB_VRAM_TRANSFER_BYTES / SGB_GB_TILE_BYTES;
const SGB_GB_VRAM_BYTES: usize = 0x2000;
const SGB_GB_TILE_BYTES: usize = 16;
const SGB_GB_TILEMAP_WIDTH: usize = 32;
const SGB_GB_BG_MAP_9800_OFFSET: usize = 0x1800;
const SGB_GB_BG_MAP_9C00_OFFSET: usize = 0x1C00;
const SGB_GB_SIGNED_TILE_DATA_BASE_OFFSET: i32 = 0x1000;
const SGB_TRANSFER_REQUIRED_BGP: u8 = 0xE4;
const SGB_LCDC_ENABLE_BIT: u8 = 0x80;
const SGB_LCDC_BG_TILE_MAP_BIT: u8 = 0x08;
const SGB_LCDC_BG_WINDOW_TILE_DATA_BIT: u8 = 0x10;
const SGB_LCDC_BG_ENABLE_BIT: u8 = 0x01;
const SGB_PACKET_BYTES: usize = SGB_COMMAND_PACKET_BYTES;
const SGB_PACKET_BITS: u8 = 128;
const SGB_PACKET_COUNT_MIN: u8 = 1;
const SGB_PACKET_COUNT_MAX: u8 = SGB_COMMAND_MAX_PACKETS as u8;
const SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED: u8 = 0x33;
const SGB_RGB555_MASK: u16 = 0x7FFF;
const SGB_RGB555_WHITE: SgbRgb555Color = SgbRgb555Color::new(0x7FFF);
const SGB_RGB555_LIGHT_GRAY: SgbRgb555Color = SgbRgb555Color::new(0x5294);
const SGB_RGB555_DARK_GRAY: SgbRgb555Color = SgbRgb555Color::new(0x294A);
const SGB_RGB555_BLACK: SgbRgb555Color = SgbRgb555Color::new(0x0000);
const SGB_BOOT_PALETTE_COUNT: usize = 32;
const SGB_BOOT_PALETTE_DEFAULT_INDEX: u8 = 1;
const SGB_COMMAND_PAL01: u8 = 0x00;
const SGB_COMMAND_PAL23: u8 = 0x01;
const SGB_COMMAND_PAL03: u8 = 0x02;
const SGB_COMMAND_PAL12: u8 = 0x03;
const SGB_COMMAND_ATTR_BLK: u8 = 0x04;
const SGB_COMMAND_ATTR_LIN: u8 = 0x05;
const SGB_COMMAND_ATTR_DIV: u8 = 0x06;
const SGB_COMMAND_ATTR_CHR: u8 = 0x07;
const SGB_COMMAND_SOUND: u8 = 0x08;
const SGB_COMMAND_SOU_TRN: u8 = 0x09;
const SGB_COMMAND_PAL_SET: u8 = 0x0A;
const SGB_COMMAND_PAL_TRN: u8 = 0x0B;
const SGB_COMMAND_DATA_SND: u8 = 0x0F;
const SGB_COMMAND_DATA_TRN: u8 = 0x10;
const SGB_COMMAND_CHR_TRN: u8 = 0x13;
const SGB_COMMAND_PCT_TRN: u8 = 0x14;
const SGB_COMMAND_ATTR_TRN: u8 = 0x15;
const SGB_COMMAND_ATTR_SET: u8 = 0x16;
const SGB_COMMAND_MASK_EN: u8 = 0x17;
const SGB_COMMAND_MLT_REQ: u8 = 0x11;
const SGB_COMMAND_JUMP: u8 = 0x12;
const SGB_COMMAND_PAL_PRI: u8 = 0x19;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbRealBootAsset {
    SgbBoot,
    Sgb2Boot,
}

impl SgbRealBootAsset {
    pub const fn from_profile(profile: SgbHostProfile) -> Self {
        match profile {
            SgbHostProfile::SgbNtsc | SgbHostProfile::SgbPal => Self::SgbBoot,
            SgbHostProfile::Sgb2Ntsc => Self::Sgb2Boot,
        }
    }

    pub const fn filename(self) -> &'static str {
        match self {
            Self::SgbBoot => "sgb_boot.bin",
            Self::Sgb2Boot => "sgb2_boot.bin",
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostAudioRequest {
    Sound(SgbSoundRequest),
    SoundTransfer(SgbSoundTransferRequest),
}

impl SgbHostAudioRequest {
    pub const fn kind(self) -> SgbHostBackendRequestKind {
        match self {
            Self::Sound(_) => SgbHostBackendRequestKind::Sound,
            Self::SoundTransfer(_) => SgbHostBackendRequestKind::SoundTransfer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbSnesHostRequest {
    DataSend(SgbDataSendRequest),
    DataTransfer(SgbDataTransferRequest),
    Jump(SgbJumpRequest),
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbSoundEffectControl {
    pub code: u8,
    pub pitch: u8,
    pub volume: u8,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbSoundRequest {
    pub effect_a: SgbSoundEffectControl,
    pub effect_b: SgbSoundEffectControl,
    pub music_score: u8,
    pub raw_attributes: u8,
}

impl SgbSoundRequest {
    const fn from_packet(bytes: &[u8; SGB_PACKET_BYTES]) -> Self {
        let raw_attributes = bytes[3];
        Self {
            effect_a: SgbSoundEffectControl {
                code: bytes[1],
                pitch: raw_attributes & 0x03,
                volume: (raw_attributes >> 2) & 0x03,
            },
            effect_b: SgbSoundEffectControl {
                code: bytes[2],
                pitch: (raw_attributes >> 4) & 0x03,
                volume: (raw_attributes >> 6) & 0x03,
            },
            music_score: bytes[4],
            raw_attributes,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbApuRamAddress {
    pub address: u16,
}

impl SgbApuRamAddress {
    pub const fn new(address: u16) -> Self {
        Self { address }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbSoundTransferPacket {
    Data {
        size: u16,
        destination: SgbApuRamAddress,
    },
    Jump {
        address: SgbApuRamAddress,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbSoundTransferRequest {
    pub first_packet: SgbSoundTransferPacket,
    pub payload_bytes: u32,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbSnesAddress {
    pub bank: u8,
    pub address: u16,
}

impl SgbSnesAddress {
    pub const fn new(bank: u8, address: u16) -> Self {
        Self { bank, address }
    }

    const fn from_packet_bytes(low: u8, high: u8, bank: u8) -> Self {
        Self {
            bank,
            address: u16::from_le_bytes([low, high]),
        }
    }

    pub const fn raw24(self) -> u32 {
        (self.bank as u32) << 16 | self.address as u32
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbDataSendRequest {
    pub destination: SgbSnesAddress,
    pub declared_len: u8,
    pub data: [u8; SGB_DATA_SND_INLINE_BYTES],
}

impl SgbDataSendRequest {
    const fn from_packet(bytes: &[u8; SGB_PACKET_BYTES]) -> Self {
        let mut data = [0; SGB_DATA_SND_INLINE_BYTES];
        let mut index = 0;
        while index < SGB_DATA_SND_INLINE_BYTES {
            data[index] = bytes[5 + index];
            index += 1;
        }
        Self {
            destination: SgbSnesAddress::from_packet_bytes(bytes[1], bytes[2], bytes[3]),
            declared_len: bytes[4],
            data,
        }
    }

    pub const fn payload_len(self) -> usize {
        if self.declared_len as usize > SGB_DATA_SND_INLINE_BYTES {
            SGB_DATA_SND_INLINE_BYTES
        } else {
            self.declared_len as usize
        }
    }

    pub fn payload(&self) -> &[u8] {
        &self.data[..self.payload_len()]
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbDataTransferRequest {
    pub destination: SgbSnesAddress,
    pub payload_bytes: u32,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbJumpRequest {
    pub program_counter: SgbSnesAddress,
    pub nmi_handler: SgbSnesAddress,
}

impl SgbJumpRequest {
    const fn from_packet(bytes: &[u8; SGB_PACKET_BYTES]) -> Self {
        Self {
            program_counter: SgbSnesAddress::from_packet_bytes(bytes[1], bytes[2], bytes[3]),
            nmi_handler: SgbSnesAddress::from_packet_bytes(bytes[4], bytes[5], bytes[6]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostStatus {
    Disabled,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbCommandAcceptance {
    Disabled,
    AwaitingCartridgeHeader,
    RejectedByHeader,
    Accepted,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbJoypLineState {
    #[default]
    Idle,
    Start,
    Zero,
    One,
    Invalid,
}

impl SgbJoypLineState {
    const fn from_joyp_value(value: u8) -> Self {
        match value & JOYP_SELECT_BITS_MASK {
            SGB_JOYP_IDLE_BITS => Self::Idle,
            SGB_JOYP_START_BITS => Self::Start,
            SGB_JOYP_ZERO_BITS => Self::Zero,
            SGB_JOYP_ONE_BITS => Self::One,
            _ => Self::Invalid,
        }
    }

    const fn data_bit(self) -> Option<u8> {
        match self {
            Self::Zero => Some(0),
            Self::One => Some(1),
            Self::Idle | Self::Start | Self::Invalid => None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbPacketTraceStatus {
    #[default]
    None,
    Complete,
    RejectedByHeader,
    InvalidPacketLength,
    InvalidStopBit,
    IncompleteReset,
    OrphanDataPulse,
    ConflictingPulse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SgbLcdCompositionError {
    DisabledHost,
    InputLength { expected: usize, actual: usize },
    OutputLength { expected: usize, actual: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SgbFrameCompositionError {
    DisabledHost,
    InputLength { expected: usize, actual: usize },
    OutputLength { expected: usize, actual: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    const fn from_command_byte(value: u8) -> Self {
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
    const fn from_command_byte(value: u8) -> Self {
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
    const fn from_command_byte(value: u8) -> Self {
        Self {
            tile_block: value & 0x01,
            tile_type: SgbChrTransferTileType::from_command_byte(value),
        }
    }

    fn destination_offset(self) -> usize {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbPendingVramTransfer {
    pub command_id: u8,
    pub target: SgbVramTransferTarget,
    pub frame_starts_until_capture: u8,
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

    const fn can_extract_display_order(self) -> bool {
        self.lcdc & SGB_LCDC_ENABLE_BIT != 0
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
    fn from_source_bytes(source: &[u8]) -> Result<Self, SgbVramTransferError> {
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

    fn from_display_memory(
        vram_bytes: &[u8],
        display: SgbVramTransferDisplayState,
    ) -> Result<Self, SgbVramTransferError> {
        if !display.can_extract_display_order() {
            return Self::from_source_bytes(vram_bytes);
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

        Ok(Self { bytes })
    }

    fn dynamic_payload_bytes(&self) -> usize {
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

impl SgbSoundTransferRequest {
    fn from_vram_transfer_payload(payload: &SgbVramTransferBuffer) -> Self {
        let size = u16::from_le_bytes([
            payload.bytes.first().copied().unwrap_or(0),
            payload.bytes.get(1).copied().unwrap_or(0),
        ]);
        let address = SgbApuRamAddress::new(u16::from_le_bytes([
            payload.bytes.get(2).copied().unwrap_or(0),
            payload.bytes.get(3).copied().unwrap_or(0),
        ]));
        Self {
            first_packet: if size == 0 {
                SgbSoundTransferPacket::Jump { address }
            } else {
                SgbSoundTransferPacket::Data {
                    size,
                    destination: address,
                }
            },
            payload_bytes: payload.bytes.len() as u32,
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
    pub last_completed: Option<SgbCompletedVramTransfer>,
    pub requested_transfer_count: u64,
    pub completed_transfer_count: u64,
}

impl SgbVramTransferState {
    fn dynamic_payload_bytes(&self) -> usize {
        self.last_completed
            .as_ref()
            .map(|transfer| transfer.payload.dynamic_payload_bytes())
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbLcdRgb555Frame {
    pub pixels: Vec<u16>,
}

impl Default for SgbLcdRgb555Frame {
    fn default() -> Self {
        Self {
            pixels: vec![0; SGB_LCD_PIXELS],
        }
    }
}

impl SgbLcdRgb555Frame {
    fn dynamic_payload_bytes(&self) -> usize {
        self.pixels.len().saturating_mul(std::mem::size_of::<u16>())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbRgb555Color {
    raw: u16,
}

impl SgbRgb555Color {
    pub const fn new(raw: u16) -> Self {
        Self {
            raw: raw & SGB_RGB555_MASK,
        }
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }

    const fn from_packet_bytes(low: u8, high: u8) -> Self {
        Self::new(u16::from_le_bytes([low, high]))
    }
}

impl Default for SgbRgb555Color {
    fn default() -> Self {
        SGB_RGB555_BLACK
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbScreenPalette {
    pub colors: [SgbRgb555Color; SGB_SCREEN_PALETTE_COLORS],
}

impl SgbScreenPalette {
    pub const fn dmg_grayscale() -> Self {
        Self {
            colors: [
                SGB_RGB555_WHITE,
                SGB_RGB555_LIGHT_GRAY,
                SGB_RGB555_DARK_GRAY,
                SGB_RGB555_BLACK,
            ],
        }
    }

    pub const fn color(self, color_index: u8) -> SgbRgb555Color {
        self.colors[(color_index & 0x03) as usize]
    }

    fn set_color(&mut self, color_index: usize, color: SgbRgb555Color) {
        self.colors[color_index] = color;
    }
}

impl Default for SgbScreenPalette {
    fn default() -> Self {
        Self::dmg_grayscale()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SgbBootPaletteAssignment {
    title: &'static [u8],
    palette_index: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SgbBootPaletteSelection {
    Default,
    TitleMatch { palette_index: u8 },
}

impl SgbBootPaletteSelection {
    const fn palette_index(self) -> u8 {
        match self {
            Self::Default => SGB_BOOT_PALETTE_DEFAULT_INDEX,
            Self::TitleMatch { palette_index } => palette_index,
        }
    }
}

// SGB BIOS built-in palettes used by the host boot program before command-driven game palettes take over.
const SGB_BOOT_PALETTES: [SgbScreenPalette; SGB_BOOT_PALETTE_COUNT] = [
    sgb_screen_palette([0x67BF, 0x265B, 0x10B5, 0x2866]),
    sgb_screen_palette([0x637B, 0x3AD9, 0x0956, 0x0000]),
    sgb_screen_palette([0x7F1F, 0x2A7D, 0x30F3, 0x4CE7]),
    sgb_screen_palette([0x57FF, 0x2618, 0x001F, 0x006A]),
    sgb_screen_palette([0x5B7F, 0x3F0F, 0x222D, 0x10EB]),
    sgb_screen_palette([0x7FBB, 0x2A3C, 0x0015, 0x0900]),
    sgb_screen_palette([0x2800, 0x7680, 0x01EF, 0x2FFF]),
    sgb_screen_palette([0x73BF, 0x46FF, 0x0110, 0x0066]),
    sgb_screen_palette([0x533E, 0x2638, 0x01E5, 0x0000]),
    sgb_screen_palette([0x7FFF, 0x2BBF, 0x00DF, 0x2C0A]),
    sgb_screen_palette([0x7F1F, 0x463D, 0x74CF, 0x4CA5]),
    sgb_screen_palette([0x53FF, 0x03E0, 0x00DF, 0x2800]),
    sgb_screen_palette([0x433F, 0x72D2, 0x3045, 0x0822]),
    sgb_screen_palette([0x7FFA, 0x2A5F, 0x0014, 0x0003]),
    sgb_screen_palette([0x1EED, 0x215C, 0x42FC, 0x0060]),
    sgb_screen_palette([0x7FFF, 0x5EF7, 0x39CE, 0x0000]),
    sgb_screen_palette([0x4F5F, 0x630E, 0x159F, 0x3126]),
    sgb_screen_palette([0x637B, 0x121C, 0x0140, 0x0840]),
    sgb_screen_palette([0x66BC, 0x3FFF, 0x7EE0, 0x2C84]),
    sgb_screen_palette([0x5FFE, 0x3EBC, 0x0321, 0x0000]),
    sgb_screen_palette([0x63FF, 0x36DC, 0x11F6, 0x392A]),
    sgb_screen_palette([0x65EF, 0x7DBF, 0x035F, 0x2108]),
    sgb_screen_palette([0x2B6C, 0x7FFF, 0x1CD9, 0x0007]),
    sgb_screen_palette([0x53FC, 0x1F2F, 0x0E29, 0x0061]),
    sgb_screen_palette([0x36BE, 0x7EAF, 0x681A, 0x3C00]),
    sgb_screen_palette([0x7BBE, 0x329D, 0x1DE8, 0x0423]),
    sgb_screen_palette([0x739F, 0x6A9B, 0x7293, 0x0001]),
    sgb_screen_palette([0x5FFF, 0x6732, 0x3DA9, 0x2481]),
    sgb_screen_palette([0x577F, 0x3EBC, 0x456F, 0x1880]),
    sgb_screen_palette([0x6B57, 0x6E1B, 0x5010, 0x0007]),
    sgb_screen_palette([0x0F96, 0x2C97, 0x0045, 0x3200]),
    sgb_screen_palette([0x67FF, 0x2F17, 0x2230, 0x1548]),
];

// Exact NUL-padded raw header titles that the SGB BIOS maps to built-in palettes for DMG software that does not unlock SGB commands.
const SGB_BOOT_TITLE_PALETTE_ASSIGNMENTS: [SgbBootPaletteAssignment; 26] = [
    SgbBootPaletteAssignment {
        title: b"ZELDA",
        palette_index: 5,
    },
    SgbBootPaletteAssignment {
        title: b"SUPER MARIOLAND",
        palette_index: 6,
    },
    SgbBootPaletteAssignment {
        title: b"MARIOLAND2",
        palette_index: 0x14,
    },
    SgbBootPaletteAssignment {
        title: b"SUPERMARIOLAND3",
        palette_index: 2,
    },
    SgbBootPaletteAssignment {
        title: b"KIRBY DREAM LAND",
        palette_index: 0x0B,
    },
    SgbBootPaletteAssignment {
        title: b"HOSHINOKA-BI",
        palette_index: 0x0B,
    },
    SgbBootPaletteAssignment {
        title: b"KIRBY'S PINBALL",
        palette_index: 3,
    },
    SgbBootPaletteAssignment {
        title: b"YOSSY NO TAMAGO",
        palette_index: 0x0C,
    },
    SgbBootPaletteAssignment {
        title: b"MARIO & YOSHI",
        palette_index: 0x0C,
    },
    SgbBootPaletteAssignment {
        title: b"YOSSY NO COOKIE",
        palette_index: 4,
    },
    SgbBootPaletteAssignment {
        title: b"YOSHI'S COOKIE",
        palette_index: 4,
    },
    SgbBootPaletteAssignment {
        title: b"DR.MARIO",
        palette_index: 0x12,
    },
    SgbBootPaletteAssignment {
        title: b"TETRIS",
        palette_index: 0x11,
    },
    SgbBootPaletteAssignment {
        title: b"YAKUMAN",
        palette_index: 0x13,
    },
    SgbBootPaletteAssignment {
        title: b"METROID2",
        palette_index: 0x1F,
    },
    SgbBootPaletteAssignment {
        title: b"KAERUNOTAMENI",
        palette_index: 9,
    },
    SgbBootPaletteAssignment {
        title: b"GOLF",
        palette_index: 0x18,
    },
    SgbBootPaletteAssignment {
        title: b"ALLEY WAY",
        palette_index: 0x16,
    },
    SgbBootPaletteAssignment {
        title: b"BASEBALL",
        palette_index: 0x0F,
    },
    SgbBootPaletteAssignment {
        title: b"TENNIS",
        palette_index: 0x17,
    },
    SgbBootPaletteAssignment {
        title: b"F1RACE",
        palette_index: 0x1E,
    },
    SgbBootPaletteAssignment {
        title: b"KID ICARUS",
        palette_index: 0x0E,
    },
    SgbBootPaletteAssignment {
        title: b"QIX",
        palette_index: 0x19,
    },
    SgbBootPaletteAssignment {
        title: b"SOLARSTRIKER",
        palette_index: 7,
    },
    SgbBootPaletteAssignment {
        title: b"X",
        palette_index: 0x1C,
    },
    SgbBootPaletteAssignment {
        title: b"GBWARS",
        palette_index: 0x15,
    },
];

const fn sgb_screen_palette(raw_colors: [u16; SGB_SCREEN_PALETTE_COLORS]) -> SgbScreenPalette {
    SgbScreenPalette {
        colors: [
            SgbRgb555Color::new(raw_colors[0]),
            SgbRgb555Color::new(raw_colors[1]),
            SgbRgb555Color::new(raw_colors[2]),
            SgbRgb555Color::new(raw_colors[3]),
        ],
    }
}

fn sgb_boot_palette(palette_index: u8) -> SgbScreenPalette {
    let table_index = usize::from(palette_index.saturating_sub(1));
    SGB_BOOT_PALETTES
        .get(table_index)
        .copied()
        .unwrap_or(SGB_BOOT_PALETTES[0])
}

fn sgb_boot_palette_selection_for_header(
    header: Option<&CartridgeHeader>,
    command_acceptance: SgbCommandAcceptance,
) -> SgbBootPaletteSelection {
    let Some(header) = header else {
        return SgbBootPaletteSelection::Default;
    };
    if command_acceptance != SgbCommandAcceptance::RejectedByHeader {
        return SgbBootPaletteSelection::Default;
    }
    sgb_title_palette_index(&header.title_bytes)
        .map(|palette_index| SgbBootPaletteSelection::TitleMatch { palette_index })
        .unwrap_or(SgbBootPaletteSelection::Default)
}

fn sgb_title_palette_index(title_bytes: &[u8; 16]) -> Option<u8> {
    SGB_BOOT_TITLE_PALETTE_ASSIGNMENTS
        .iter()
        .find(|assignment| sgb_title_bytes_match(title_bytes, assignment.title))
        .map(|assignment| assignment.palette_index)
}

fn sgb_title_bytes_match(title_bytes: &[u8; 16], expected_title: &[u8]) -> bool {
    if expected_title.len() > title_bytes.len() {
        return false;
    }
    title_bytes.starts_with(expected_title)
        && title_bytes[expected_title.len()..]
            .iter()
            .all(|&byte| byte == 0)
}

const fn gb_tile_data_offset(lcdc: u8, tile_index: u8) -> usize {
    if lcdc & SGB_LCDC_BG_WINDOW_TILE_DATA_BIT != 0 {
        tile_index as usize * SGB_GB_TILE_BYTES
    } else {
        (SGB_GB_SIGNED_TILE_DATA_BASE_OFFSET + (tile_index as i8 as i32) * SGB_GB_TILE_BYTES as i32)
            as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbPaletteState {
    pub screen_palettes: [SgbScreenPalette; SGB_SCREEN_PALETTE_COUNT],
    pub base_palette_index: u8,
}

impl SgbPaletteState {
    fn default_for_active_host(active: bool) -> Self {
        let mut state = Self::default();
        if active {
            state.apply_boot_palette(SgbBootPaletteSelection::Default);
        }
        state
    }

    pub const fn palette(self, palette_index: u8) -> SgbScreenPalette {
        self.screen_palettes[(palette_index & 0x03) as usize]
    }

    pub const fn map_lcd_shade(self, shade: u8) -> SgbRgb555Color {
        self.palette(self.base_palette_index).color(shade)
    }

    fn apply_boot_palette(&mut self, selection: SgbBootPaletteSelection) {
        self.screen_palettes[0] = sgb_boot_palette(selection.palette_index());
        self.base_palette_index = 0;
    }

    fn apply_direct_palette_command(&mut self, command_id: u8, bytes: &[u8; SGB_PACKET_BYTES]) {
        let Some((first_palette, second_palette)) = direct_palette_command_pair(command_id) else {
            return;
        };

        let shared_color_zero = SgbRgb555Color::from_packet_bytes(bytes[1], bytes[2]);
        self.screen_palettes[first_palette].set_color(0, shared_color_zero);
        self.screen_palettes[second_palette].set_color(0, shared_color_zero);

        let first_palette_colors = [
            SgbRgb555Color::from_packet_bytes(bytes[3], bytes[4]),
            SgbRgb555Color::from_packet_bytes(bytes[5], bytes[6]),
            SgbRgb555Color::from_packet_bytes(bytes[7], bytes[8]),
        ];
        let second_palette_colors = [
            SgbRgb555Color::from_packet_bytes(bytes[9], bytes[10]),
            SgbRgb555Color::from_packet_bytes(bytes[11], bytes[12]),
            SgbRgb555Color::from_packet_bytes(bytes[13], bytes[14]),
        ];

        for (color_index, color) in first_palette_colors.into_iter().enumerate() {
            self.screen_palettes[first_palette].set_color(color_index + 1, color);
        }
        for (color_index, color) in second_palette_colors.into_iter().enumerate() {
            self.screen_palettes[second_palette].set_color(color_index + 1, color);
        }
    }
}

impl Default for SgbPaletteState {
    fn default() -> Self {
        Self {
            screen_palettes: [SgbScreenPalette::dmg_grayscale(); SGB_SCREEN_PALETTE_COUNT],
            base_palette_index: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbPlayerPaletteOverrideState {
    pub active: bool,
    pub palette_state: SgbPaletteState,
    pub attributes: SgbAttributeMap,
    pub activation_count: u64,
    pub manual_release_count: u64,
    pub pal_pri_release_count: u64,
}

impl SgbPlayerPaletteOverrideState {
    fn set_uniform_palette(&mut self, palette: SgbScreenPalette) -> bool {
        let palette_state = SgbPaletteState {
            screen_palettes: [palette; SGB_SCREEN_PALETTE_COUNT],
            ..SgbPaletteState::default()
        };
        let attributes = SgbAttributeMap::default();
        let changed =
            !self.active || self.palette_state != palette_state || self.attributes != attributes;
        self.active = true;
        self.palette_state = palette_state;
        self.attributes = attributes;
        if changed {
            self.activation_count = self.activation_count.saturating_add(1);
        }
        changed
    }

    fn clear_by_player(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.active = false;
        self.manual_release_count = self.manual_release_count.saturating_add(1);
        true
    }

    fn return_to_application_due_to_pal_pri(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.active = false;
        self.pal_pri_release_count = self.pal_pri_release_count.saturating_add(1);
        true
    }

    fn dynamic_payload_bytes(&self) -> usize {
        self.attributes.dynamic_payload_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbSystemPaletteState {
    pub palettes: Vec<SgbScreenPalette>,
    pub loaded: bool,
    pub last_pal_set_ids: [u16; SGB_SCREEN_PALETTE_COUNT],
    pub pal_trn_count: u64,
    pub pal_set_count: u64,
    pub pal_pri_enabled: bool,
    pub pal_pri_command_count: u64,
}

impl SgbSystemPaletteState {
    fn apply_pal_trn(&mut self, payload: &SgbVramTransferBuffer) {
        for palette_index in 0..SGB_SYSTEM_PALETTE_COUNT {
            for color_index in 0..SGB_SCREEN_PALETTE_COLORS {
                let byte_index = palette_index * SGB_SCREEN_PALETTE_COLORS * 2 + color_index * 2;
                self.palettes[palette_index].colors[color_index] =
                    SgbRgb555Color::from_packet_bytes(
                        payload.bytes[byte_index],
                        payload.bytes[byte_index + 1],
                    );
            }
        }
        self.loaded = true;
        self.pal_trn_count = self.pal_trn_count.saturating_add(1);
    }

    fn apply_pal_set(
        &mut self,
        palette_state: &mut SgbPaletteState,
        bytes: &[u8; SGB_PACKET_BYTES],
    ) -> SgbPalSetOptions {
        for palette_index in 0..SGB_SCREEN_PALETTE_COUNT {
            let byte_index = 1 + palette_index * 2;
            let palette_id = u16::from_le_bytes([bytes[byte_index], bytes[byte_index + 1]]);
            self.last_pal_set_ids[palette_index] = palette_id;
            palette_state.screen_palettes[palette_index] = self
                .palettes
                .get(usize::from(palette_id))
                .copied()
                .unwrap_or_default();
        }
        self.pal_set_count = self.pal_set_count.saturating_add(1);
        SgbPalSetOptions::from_flags(bytes[9])
    }

    fn apply_pal_pri(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.pal_pri_enabled = bytes[1] & 0x01 != 0;
        self.pal_pri_command_count = self.pal_pri_command_count.saturating_add(1);
    }

    fn dynamic_payload_bytes(&self) -> usize {
        self.palettes
            .len()
            .saturating_mul(std::mem::size_of::<SgbScreenPalette>())
    }
}

impl Default for SgbSystemPaletteState {
    fn default() -> Self {
        Self {
            palettes: vec![SgbScreenPalette::dmg_grayscale(); SGB_SYSTEM_PALETTE_COUNT],
            loaded: false,
            last_pal_set_ids: [0; SGB_SCREEN_PALETTE_COUNT],
            pal_trn_count: 0,
            pal_set_count: 0,
            pal_pri_enabled: false,
            pal_pri_command_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbPalSetOptions {
    pub apply_atf: bool,
    pub cancel_mask: bool,
    pub atf_index: u8,
}

impl SgbPalSetOptions {
    const fn from_flags(flags: u8) -> Self {
        Self {
            apply_atf: flags & 0x80 != 0,
            cancel_mask: flags & 0x40 != 0,
            atf_index: flags & 0x3F,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbAttributeMap {
    pub cells: Vec<u8>,
}

impl SgbAttributeMap {
    pub fn palette_index(&self, cell_x: usize, cell_y: usize) -> u8 {
        self.cells[cell_y * SGB_ATTR_MAP_WIDTH + cell_x] & 0x03
    }

    fn palette_index_for_framebuffer_index(&self, framebuffer_index: usize) -> u8 {
        let pixel_x = framebuffer_index % SGB_LCD_WIDTH;
        let pixel_y = framebuffer_index / SGB_LCD_WIDTH;
        self.palette_index(pixel_x / 8, pixel_y / 8)
    }

    fn set_cell(&mut self, cell_x: usize, cell_y: usize, palette_index: u8) {
        if cell_x < SGB_ATTR_MAP_WIDTH && cell_y < SGB_ATTR_MAP_HEIGHT {
            self.cells[cell_y * SGB_ATTR_MAP_WIDTH + cell_x] = palette_index & 0x03;
        }
    }

    fn dynamic_payload_bytes(&self) -> usize {
        self.cells.len()
    }
}

impl Default for SgbAttributeMap {
    fn default() -> Self {
        Self {
            cells: vec![0; SGB_ATTR_MAP_CELLS],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbAttributeFileState {
    pub bytes: Vec<u8>,
    pub loaded: bool,
}

impl SgbAttributeFileState {
    fn apply_attr_trn(&mut self, payload: &SgbVramTransferBuffer) {
        self.bytes
            .copy_from_slice(&payload.bytes[..SGB_ATF_TOTAL_BYTES]);
        self.loaded = true;
    }

    fn apply_to_map(&self, atf_index: u8, map: &mut SgbAttributeMap) -> bool {
        let atf_index = usize::from(atf_index);
        if atf_index >= SGB_ATF_COUNT {
            return false;
        }
        for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
            for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                map.set_cell(
                    cell_x,
                    cell_y,
                    self.palette_index(atf_index, cell_x, cell_y),
                );
            }
        }
        true
    }

    fn palette_index(&self, atf_index: usize, cell_x: usize, cell_y: usize) -> u8 {
        let byte_index = atf_index * SGB_ATF_BYTES + cell_y * 5 + cell_x / 4;
        let shift = 6 - (cell_x % 4) * 2;
        (self.bytes[byte_index] >> shift) & 0x03
    }

    fn dynamic_payload_bytes(&self) -> usize {
        self.bytes.len()
    }
}

impl Default for SgbAttributeFileState {
    fn default() -> Self {
        Self {
            bytes: vec![0; SGB_ATF_TOTAL_BYTES],
            loaded: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SgbAttributeState {
    pub map: SgbAttributeMap,
    pub files: SgbAttributeFileState,
    pub last_atf_index: Option<u8>,
    pub attr_blk_count: u64,
    pub attr_lin_count: u64,
    pub attr_div_count: u64,
    pub attr_chr_count: u64,
    pub attr_trn_count: u64,
    pub attr_set_count: u64,
    pub invalid_atf_count: u64,
}

impl SgbAttributeState {
    fn apply_attr_blk(&mut self, payload: &[u8]) {
        if payload.is_empty() {
            return;
        }
        let data_set_count = usize::from(payload[0]).min(0x12);
        for data_set_index in 0..data_set_count {
            let offset = 1 + data_set_index * 6;
            let Some(data_set) = payload.get(offset..offset + 6) else {
                break;
            };
            self.apply_attr_blk_data_set(data_set);
        }
        self.attr_blk_count = self.attr_blk_count.saturating_add(1);
    }

    fn apply_attr_blk_data_set(&mut self, data_set: &[u8]) {
        let control = data_set[0] & 0x07;
        let palette_designation = data_set[1];
        let inside_palette = palette_designation & 0x03;
        let mut line_palette = (palette_designation >> 2) & 0x03;
        let outside_palette = (palette_designation >> 4) & 0x03;
        let change_inside = control & 0x01 != 0;
        let mut change_line = control & 0x02 != 0;
        let change_outside = control & 0x04 != 0;

        if control == 0x01 {
            change_line = true;
            line_palette = inside_palette;
        } else if control == 0x04 {
            change_line = true;
            line_palette = outside_palette;
        }

        let x1 = usize::from(data_set[2]).min(SGB_ATTR_MAP_WIDTH - 1);
        let y1 = usize::from(data_set[3]).min(SGB_ATTR_MAP_HEIGHT - 1);
        let x2 = usize::from(data_set[4]).min(SGB_ATTR_MAP_WIDTH - 1);
        let y2 = usize::from(data_set[5]).min(SGB_ATTR_MAP_HEIGHT - 1);
        let (left, right) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        let (top, bottom) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };

        for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
            for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                let inside_rect =
                    (left..=right).contains(&cell_x) && (top..=bottom).contains(&cell_y);
                let on_line = inside_rect
                    && (cell_x == left || cell_x == right || cell_y == top || cell_y == bottom);
                if change_outside && !inside_rect {
                    self.map.set_cell(cell_x, cell_y, outside_palette);
                }
                if change_inside && inside_rect && !on_line {
                    self.map.set_cell(cell_x, cell_y, inside_palette);
                }
                if change_line && on_line {
                    self.map.set_cell(cell_x, cell_y, line_palette);
                }
            }
        }
    }

    fn apply_attr_lin(&mut self, payload: &[u8]) {
        if payload.is_empty() {
            return;
        }
        let data_set_count = usize::from(payload[0]).min(0x6E);
        for &line in payload.iter().skip(1).take(data_set_count) {
            let coordinate = usize::from(line & 0x1F);
            let palette_index = (line >> 5) & 0x03;
            let horizontal = line & 0x80 != 0;
            if horizontal {
                if coordinate < SGB_ATTR_MAP_HEIGHT {
                    for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                        self.map.set_cell(cell_x, coordinate, palette_index);
                    }
                }
            } else if coordinate < SGB_ATTR_MAP_WIDTH {
                for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
                    self.map.set_cell(coordinate, cell_y, palette_index);
                }
            }
        }
        self.attr_lin_count = self.attr_lin_count.saturating_add(1);
    }

    fn apply_attr_div(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        let palettes = bytes[1];
        let below_or_right_palette = palettes & 0x03;
        let above_or_left_palette = (palettes >> 2) & 0x03;
        let line_palette = (palettes >> 4) & 0x03;
        let horizontal = palettes & 0x40 != 0;
        let coordinate = usize::from(bytes[2]);

        if horizontal {
            for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
                let palette_index = if cell_y < coordinate {
                    above_or_left_palette
                } else if cell_y == coordinate {
                    line_palette
                } else {
                    below_or_right_palette
                };
                for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                    self.map.set_cell(cell_x, cell_y, palette_index);
                }
            }
        } else {
            for cell_x in 0..SGB_ATTR_MAP_WIDTH {
                let palette_index = if cell_x < coordinate {
                    above_or_left_palette
                } else if cell_x == coordinate {
                    line_palette
                } else {
                    below_or_right_palette
                };
                for cell_y in 0..SGB_ATTR_MAP_HEIGHT {
                    self.map.set_cell(cell_x, cell_y, palette_index);
                }
            }
        }
        self.attr_div_count = self.attr_div_count.saturating_add(1);
    }

    fn apply_attr_chr(&mut self, payload: &[u8]) {
        if payload.len() < 5 {
            return;
        }
        let mut cell_x = usize::from(payload[0]);
        let mut cell_y = usize::from(payload[1]);
        if cell_x >= SGB_ATTR_MAP_WIDTH || cell_y >= SGB_ATTR_MAP_HEIGHT {
            return;
        }
        let data_set_count =
            usize::from(u16::from_le_bytes([payload[2], payload[3]])).min(SGB_ATTR_MAP_CELLS);
        let top_to_bottom = payload[4] & 0x01 != 0;

        for data_set_index in 0..data_set_count {
            let packed_byte_index = 5 + data_set_index / 4;
            let Some(&packed) = payload.get(packed_byte_index) else {
                break;
            };
            let shift = 6 - (data_set_index % 4) * 2;
            self.map.set_cell(cell_x, cell_y, (packed >> shift) & 0x03);

            if top_to_bottom {
                cell_y += 1;
                if cell_y == SGB_ATTR_MAP_HEIGHT {
                    cell_y = 0;
                    cell_x = (cell_x + 1) % SGB_ATTR_MAP_WIDTH;
                }
            } else {
                cell_x += 1;
                if cell_x == SGB_ATTR_MAP_WIDTH {
                    cell_x = 0;
                    cell_y = (cell_y + 1) % SGB_ATTR_MAP_HEIGHT;
                }
            }
        }
        self.attr_chr_count = self.attr_chr_count.saturating_add(1);
    }

    fn apply_attr_trn(&mut self, payload: &SgbVramTransferBuffer) {
        self.files.apply_attr_trn(payload);
        self.attr_trn_count = self.attr_trn_count.saturating_add(1);
    }

    fn apply_attr_set(&mut self, atf_index: u8) -> bool {
        if self.files.apply_to_map(atf_index, &mut self.map) {
            self.last_atf_index = Some(atf_index);
            self.attr_set_count = self.attr_set_count.saturating_add(1);
            true
        } else {
            self.invalid_atf_count = self.invalid_atf_count.saturating_add(1);
            false
        }
    }

    fn dynamic_payload_bytes(&self) -> usize {
        self.map
            .dynamic_payload_bytes()
            .saturating_add(self.files.dynamic_payload_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbBorderTileData {
    pub bytes: Vec<u8>,
}

impl SgbBorderTileData {
    fn apply_chr_transfer(
        &mut self,
        selection: SgbChrTransferSelection,
        payload: &SgbVramTransferBuffer,
    ) {
        let offset = selection.destination_offset();
        self.bytes[offset..offset + SGB_VRAM_TRANSFER_BYTES].copy_from_slice(&payload.bytes);
    }

    fn pixel_color_index(&self, tile_index: usize, x: usize, y: usize) -> u8 {
        let tile_index = tile_index % SGB_BORDER_TILE_COUNT;
        let x = x & 0x07;
        let y = y & 0x07;
        let tile_offset = tile_index * SGB_BORDER_TILE_BYTES;
        let row_offset = tile_offset + y * 2;
        let low_plane_01 = self.bytes[row_offset];
        let high_plane_01 = self.bytes[row_offset + 1];
        let low_plane_23 = self.bytes[row_offset + 16];
        let high_plane_23 = self.bytes[row_offset + 17];
        let bit = 7 - x;

        ((low_plane_01 >> bit) & 0x01)
            | (((high_plane_01 >> bit) & 0x01) << 1)
            | (((low_plane_23 >> bit) & 0x01) << 2)
            | (((high_plane_23 >> bit) & 0x01) << 3)
    }
}

impl Default for SgbBorderTileData {
    fn default() -> Self {
        Self {
            bytes: vec![0; SGB_BORDER_TILE_DATA_BYTES],
        }
    }
}

impl SgbBorderTileData {
    fn dynamic_payload_bytes(&self) -> usize {
        self.bytes.len()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbBorderMapEntry {
    pub raw: u16,
}

impl SgbBorderMapEntry {
    pub const fn new(raw: u16) -> Self {
        Self { raw }
    }

    const fn tile_index(self) -> usize {
        (self.raw as usize) & 0x03FF
    }

    const fn palette_index(self) -> usize {
        match (self.raw >> 10) & 0x07 {
            4 => 0,
            5 => 1,
            6 => 2,
            _ => 0,
        }
    }

    const fn x_flip(self) -> bool {
        self.raw & 0x4000 != 0
    }

    const fn y_flip(self) -> bool {
        self.raw & 0x8000 != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbBorderTileMap {
    pub entries: Vec<SgbBorderMapEntry>,
}

impl SgbBorderTileMap {
    fn apply_pct_transfer(&mut self, payload: &SgbVramTransferBuffer) {
        for (entry_index, entry) in self.entries.iter_mut().enumerate() {
            let byte_index = entry_index * 2;
            *entry = SgbBorderMapEntry::new(u16::from_le_bytes([
                payload.bytes[byte_index],
                payload.bytes[byte_index + 1],
            ]));
        }
    }

    fn entry(&self, tile_x: usize, tile_y: usize) -> SgbBorderMapEntry {
        self.entries[tile_y * SGB_BORDER_TILEMAP_WIDTH + tile_x]
    }
}

impl Default for SgbBorderTileMap {
    fn default() -> Self {
        Self {
            entries: vec![SgbBorderMapEntry::default(); SGB_BORDER_TILEMAP_ENTRIES],
        }
    }
}

impl SgbBorderTileMap {
    fn dynamic_payload_bytes(&self) -> usize {
        self.entries
            .len()
            .saturating_mul(std::mem::size_of::<SgbBorderMapEntry>())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbBorderPalette {
    pub colors: [SgbRgb555Color; SGB_BORDER_PALETTE_COLORS],
}

impl SgbBorderPalette {
    pub const fn color(self, color_index: u8) -> SgbRgb555Color {
        self.colors[(color_index & 0x0F) as usize]
    }
}

impl Default for SgbBorderPalette {
    fn default() -> Self {
        Self {
            colors: [SgbRgb555Color::new(0); SGB_BORDER_PALETTE_COLORS],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbBorderState {
    pub tile_data: SgbBorderTileData,
    pub tile_map: SgbBorderTileMap,
    pub palettes: [SgbBorderPalette; SGB_BORDER_PALETTE_COUNT],
    pub chr0_loaded: bool,
    pub chr1_loaded: bool,
    pub pct_loaded: bool,
    pub last_chr_selection: Option<SgbChrTransferSelection>,
    pub chr_transfer_count: u64,
    pub pct_transfer_count: u64,
}

impl SgbBorderState {
    fn apply_chr_transfer(
        &mut self,
        selection: SgbChrTransferSelection,
        payload: &SgbVramTransferBuffer,
    ) {
        self.tile_data.apply_chr_transfer(selection, payload);
        if selection.tile_block == 0 {
            self.chr0_loaded = true;
        } else {
            self.chr1_loaded = true;
        }
        self.last_chr_selection = Some(selection);
        self.chr_transfer_count = self.chr_transfer_count.saturating_add(1);
    }

    fn apply_pct_transfer(&mut self, payload: &SgbVramTransferBuffer) {
        self.tile_map.apply_pct_transfer(payload);
        for palette_index in 0..SGB_BORDER_PALETTE_COUNT {
            for color_index in 0..SGB_BORDER_PALETTE_COLORS {
                let byte_index =
                    0x800 + palette_index * SGB_BORDER_PALETTE_COLORS * 2 + color_index * 2;
                self.palettes[palette_index].colors[color_index] =
                    SgbRgb555Color::from_packet_bytes(
                        payload.bytes[byte_index],
                        payload.bytes[byte_index + 1],
                    );
            }
        }
        self.pct_loaded = true;
        self.pct_transfer_count = self.pct_transfer_count.saturating_add(1);
    }

    fn pixel_color(&self, x: usize, y: usize) -> (SgbRgb555Color, u8) {
        let tile_x = x / 8;
        let tile_y = (y / 8).min(SGB_BORDER_TILEMAP_VISIBLE_HEIGHT - 1);
        let entry = self.tile_map.entry(tile_x, tile_y);
        let mut pixel_x = x & 0x07;
        let mut pixel_y = y & 0x07;
        if entry.x_flip() {
            pixel_x = 7 - pixel_x;
        }
        if entry.y_flip() {
            pixel_y = 7 - pixel_y;
        }

        let color_index = self
            .tile_data
            .pixel_color_index(entry.tile_index(), pixel_x, pixel_y);
        (
            self.palettes[entry.palette_index()].color(color_index),
            color_index,
        )
    }
}

impl Default for SgbBorderState {
    fn default() -> Self {
        Self {
            tile_data: SgbBorderTileData::default(),
            tile_map: SgbBorderTileMap::default(),
            palettes: [SgbBorderPalette::default(); SGB_BORDER_PALETTE_COUNT],
            chr0_loaded: false,
            chr1_loaded: false,
            pct_loaded: false,
            last_chr_selection: None,
            chr_transfer_count: 0,
            pct_transfer_count: 0,
        }
    }
}

impl SgbBorderState {
    fn dynamic_payload_bytes(&self) -> usize {
        self.tile_data
            .dynamic_payload_bytes()
            .saturating_add(self.tile_map.dynamic_payload_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbHost {
    host_platform: HostPlatform,
    profile: Option<SgbHostProfile>,
    backend_kind: SgbHostBackendKind,
    startup: SgbStartupState,
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
    startup: SgbStartupState,
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
    pub startup: SgbStartupState,
    pub packet_transport: SgbPacketTransportState,
    pub command: SgbCommandState,
    pub video: SgbVideoState,
    pub multiplayer: SgbMultiplayerState,
    pub audio: SgbAudioState,
    pub snes_host: SgbSnesHostState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbStartupState {
    pub startup_mode: StartupMode,
    pub real_boot_asset: Option<SgbRealBootAsset>,
    pub cartridge_sgb_flag: Option<SgbFlag>,
    pub old_licensee_code: Option<u8>,
    pub command_acceptance: SgbCommandAcceptance,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbPacketTransportState {
    pub last_joyp_line_state: SgbJoypLineState,
    pub transfer_active: bool,
    pub packet_bits_buffered: u8,
    pub packet_bytes_buffered: u8,
    pub current_packet: [u8; SGB_PACKET_BYTES],
    pub reset_pulse_count: u64,
    pub data_pulse_count: u64,
    pub invalid_pulse_count: u64,
    pub last_trace: SgbPacketTrace,
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbVideoState {
    pub border_loaded: bool,
    pub colorization_active: bool,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbMultiplayerState {
    pub player_count: u8,
    pub selected_player: u8,
    pub player_pressed_masks: [u8; SGB_CONTROLLER_COUNT],
    pub last_mlt_req_control: u8,
    pub mlt_req_count: u64,
    pub player_cycle_count: u64,
}

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

impl SgbAudioState {
    fn record_request(&mut self, request: SgbHostAudioRequest) {
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
    fn record_request(&mut self, request: SgbSnesHostRequest) {
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
            packet_transport: SgbPacketTransportState::default(),
            command: SgbCommandState::default(),
            video: SgbVideoState::default_for_active_host(active),
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
            packet_transport: self.packet_transport,
            command: self.command,
            video: self.video.clone(),
            multiplayer: self.multiplayer,
            audio: self.audio,
            snes_host: self.snes_host,
        }
    }

    pub const fn selected_player_pressed_mask(&self) -> u8 {
        self.multiplayer.selected_player_pressed_mask()
    }

    pub fn set_player_pressed_mask(&mut self, player: u8, pressed_mask: u8) -> bool {
        self.multiplayer
            .set_player_pressed_mask(player, pressed_mask)
    }

    pub fn set_player_pressed_masks(&mut self, pressed_masks: [u8; SGB_CONTROLLER_COUNT]) -> bool {
        self.multiplayer.set_player_pressed_masks(pressed_masks)
    }

    pub fn set_player_button_pressed(
        &mut self,
        player: u8,
        button: JoypadButton,
        pressed: bool,
    ) -> bool {
        self.multiplayer
            .set_player_button_pressed(player, button, pressed)
    }

    pub fn set_player_palette_override(&mut self, palette: SgbScreenPalette) -> bool {
        if !self.host_platform.is_sgb() {
            return false;
        }
        self.video.set_player_palette_override(palette)
    }

    pub fn clear_player_palette_override(&mut self) -> bool {
        if !self.host_platform.is_sgb() {
            return false;
        }
        self.video.clear_player_palette_override()
    }

    pub const fn player_pressed_masks(&self) -> [u8; SGB_CONTROLLER_COUNT] {
        self.multiplayer.player_pressed_masks
    }

    pub fn joyp_read_value(&self, value: u8) -> u8 {
        if self.host_platform.is_sgb() {
            self.multiplayer.joyp_read_value(value)
        } else {
            value
        }
    }

    pub(crate) fn capture_save_state(&self) -> SgbHostSaveState {
        SgbHostSaveState {
            host_platform: self.host_platform,
            profile: self.profile,
            backend_kind: self.backend_kind,
            startup: self.startup,
            packet_transport: self.packet_transport,
            command: self.command,
            video: self.video.clone(),
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
        self.packet_transport = state.packet_transport;
        self.command = state.command;
        self.video = state.video.clone();
        self.multiplayer = state.multiplayer;
        self.audio = state.audio;
        self.snes_host = state.snes_host;
    }

    pub(crate) fn finish_real_boot_handoff(&mut self) {
        if !self.host_platform.is_sgb() {
            return;
        }

        self.packet_transport.last_joyp_line_state = SgbJoypLineState::Idle;
        self.packet_transport.transfer_active = false;
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
        self.video.apply_boot_palette_for_cartridge_header(
            self.status(),
            header,
            self.startup.command_acceptance,
        );
    }

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
                self.packet_transport.last_joyp_line_state = line_state;
            }
            SgbJoypLineState::Start => {
                if !matches!(previous_line_state, SgbJoypLineState::Start) {
                    self.begin_packet_transfer();
                }
                self.packet_transport.last_joyp_line_state = line_state;
            }
            SgbJoypLineState::Zero | SgbJoypLineState::One => {
                if previous_line_state == SgbJoypLineState::Idle {
                    self.observe_packet_data_bit(line_state.data_bit().expect("data line"));
                } else if previous_line_state != line_state {
                    self.record_packet_trace(SgbPacketTraceStatus::ConflictingPulse);
                    self.packet_transport.invalid_pulse_count =
                        self.packet_transport.invalid_pulse_count.saturating_add(1);
                    self.command.invalid_packet_count =
                        self.command.invalid_packet_count.saturating_add(1);
                }
                self.packet_transport.last_joyp_line_state = line_state;
            }
            SgbJoypLineState::Invalid => {
                self.record_packet_trace(SgbPacketTraceStatus::ConflictingPulse);
                self.packet_transport.invalid_pulse_count =
                    self.packet_transport.invalid_pulse_count.saturating_add(1);
                self.command.invalid_packet_count =
                    self.command.invalid_packet_count.saturating_add(1);
                self.packet_transport.last_joyp_line_state = line_state;
            }
        }
    }

    fn begin_packet_transfer(&mut self) {
        if self.packet_transport.transfer_active
            && self.packet_transport.packet_bits_buffered != 0
            && self.packet_transport.packet_bits_buffered <= SGB_PACKET_BITS
        {
            self.record_packet_trace(SgbPacketTraceStatus::IncompleteReset);
            self.command.invalid_packet_count = self.command.invalid_packet_count.saturating_add(1);
        }

        self.packet_transport.transfer_active = true;
        self.packet_transport.packet_bits_buffered = 0;
        self.packet_transport.packet_bytes_buffered = 0;
        self.packet_transport.current_packet = [0; SGB_PACKET_BYTES];
        self.packet_transport.reset_pulse_count =
            self.packet_transport.reset_pulse_count.saturating_add(1);
    }

    fn observe_packet_data_bit(&mut self, bit: u8) {
        self.packet_transport.data_pulse_count =
            self.packet_transport.data_pulse_count.saturating_add(1);

        if !self.packet_transport.transfer_active {
            self.record_packet_trace(SgbPacketTraceStatus::OrphanDataPulse);
            self.packet_transport.invalid_pulse_count =
                self.packet_transport.invalid_pulse_count.saturating_add(1);
            self.command.invalid_packet_count = self.command.invalid_packet_count.saturating_add(1);
            return;
        }

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
            return;
        }

        if bit == 0 {
            self.complete_packet_transfer();
        } else {
            self.record_packet_trace(SgbPacketTraceStatus::InvalidStopBit);
            self.packet_transport.invalid_pulse_count =
                self.packet_transport.invalid_pulse_count.saturating_add(1);
            self.command.invalid_packet_count = self.command.invalid_packet_count.saturating_add(1);
            self.packet_transport.transfer_active = false;
        }
    }

    fn complete_packet_transfer(&mut self) {
        self.packet_transport.transfer_active = false;
        let bytes = self.packet_transport.current_packet;
        self.decode_complete_packet(bytes);
    }

    fn decode_complete_packet(&mut self, bytes: [u8; SGB_PACKET_BYTES]) {
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
            }
            SGB_COMMAND_PAL_SET if packet_count == 1 => {
                self.video
                    .apply_pal_set_command(&self.command.packet_buffer[0]);
            }
            SGB_COMMAND_PAL_TRN if packet_count == 1 => {
                self.video.request_pal_transfer(command_id);
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
            SGB_COMMAND_CHR_TRN if packet_count == 1 => self
                .video
                .request_chr_transfer(command_id, &self.command.packet_buffer[0]),
            SGB_COMMAND_PCT_TRN if packet_count == 1 => self.video.request_pct_transfer(command_id),
            SGB_COMMAND_ATTR_TRN if packet_count == 1 => {
                self.video.request_attr_transfer(command_id);
            }
            SGB_COMMAND_ATTR_SET if packet_count == 1 => self
                .video
                .apply_attr_set_command(&self.command.packet_buffer[0]),
            SGB_COMMAND_MASK_EN if packet_count == 1 => self
                .video
                .apply_mask_command(&self.command.packet_buffer[0]),
            SGB_COMMAND_PAL_PRI if packet_count == 1 => self
                .video
                .apply_pal_pri_command(&self.command.packet_buffer[0]),
            _ => {}
        }
    }

    fn dispatch_host_backend_request(
        &mut self,
        request: SgbHostBackendRequest,
    ) -> SgbHostBackendResponse {
        let mut backend = DeterministicHleSgbHostBackend;
        let response = backend.handle_request(request, &mut self.audio, &mut self.snes_host);
        self.backend_kind = response.backend_kind;
        response
    }

    fn record_packet_trace(&mut self, status: SgbPacketTraceStatus) {
        self.packet_transport.last_trace = SgbPacketTrace {
            status,
            command_id: self.command.active_command_id,
            packet_count: self.command.expected_packet_count,
            packet_index: self.command.received_packet_count,
            bits_buffered: self.packet_transport.packet_bits_buffered,
            bytes: self.packet_transport.current_packet,
        };
    }

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

                let (border_pixel, border_color_index) = self.video.border.pixel_color(x, y);
                if in_lcd_window && border_color_index == 0 {
                    let lcd_x = x - SGB_LCD_FRAME_ORIGIN_X;
                    let lcd_y = y - SGB_LCD_FRAME_ORIGIN_Y;
                    output[output_index] = self
                        .video
                        .lcd_pixel_for_framebuffer_index(
                            lcd_y * SGB_LCD_WIDTH + lcd_x,
                            dmg_framebuffer[lcd_y * SGB_LCD_WIDTH + lcd_x],
                        )
                        .raw();
                } else {
                    output[output_index] = border_pixel.raw();
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

    pub fn capture_pending_vram_transfer(
        &mut self,
        vram_bytes: &[u8],
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        if !self.host_platform.is_sgb() {
            return Err(SgbVramTransferError::DisabledHost);
        }
        let target = self.video.capture_pending_vram_transfer(vram_bytes)?;
        self.dispatch_completed_vram_transfer(target);
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
        let target = self.video.advance_frame_start(vram_bytes, display)?;
        self.dispatch_completed_vram_transfer(target);
        Ok(target)
    }

    fn dispatch_completed_vram_transfer(&mut self, target: Option<SgbVramTransferTarget>) {
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
            SgbVramTransferTarget::Chr(_)
            | SgbVramTransferTarget::Pct
            | SgbVramTransferTarget::Pal
            | SgbVramTransferTarget::Attr => {}
        }
    }
}

impl SgbHostSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.video.dynamic_payload_bytes()
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

impl SgbMultiplayerState {
    const fn default_for_active_host(active: bool) -> Self {
        Self {
            player_count: if active { 1 } else { 0 },
            selected_player: if active { 1 } else { 0 },
            player_pressed_masks: [0; SGB_CONTROLLER_COUNT],
            last_mlt_req_control: 0,
            mlt_req_count: 0,
            player_cycle_count: 0,
        }
    }

    fn apply_mlt_req_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        if self.player_count == 0 {
            return;
        }

        let control = bytes[1] & 0x03;
        let player_count = match control {
            0 => 1,
            1 => 2,
            2 => 3,
            _ => 4,
        };
        let current_player_index = self.selected_player_index();
        let selected_player_index = if control == 2 {
            (current_player_index + 1) & 0x02
        } else {
            current_player_index & (player_count - 1)
        };

        self.player_count = player_count;
        self.selected_player = selected_player_index + 1;
        self.last_mlt_req_control = control;
        self.mlt_req_count = self.mlt_req_count.saturating_add(1);
    }

    fn observe_joyp_write(&mut self, previous_line_state: SgbJoypLineState, value: u8) {
        if !self.cycles_players_on_p15_rise() {
            return;
        }

        let previous_p15_low = matches!(
            previous_line_state,
            SgbJoypLineState::Start | SgbJoypLineState::One
        );
        let current_p15_high = value & 0x20 != 0;
        if previous_p15_low && current_p15_high {
            self.cycle_selected_player();
        }
    }

    fn cycle_selected_player(&mut self) {
        let player_count = self.player_count.min(SGB_CONTROLLER_COUNT as u8);
        if player_count == 0 {
            self.selected_player = 0;
            return;
        }

        let selected_player_index = (self.selected_player_index() + 1) & (player_count - 1);
        self.selected_player = selected_player_index + 1;
        self.player_cycle_count = self.player_cycle_count.saturating_add(1);
    }

    const fn cycles_players_on_p15_rise(&self) -> bool {
        self.player_count != 0 && self.player_count & 0x01 == 0
    }

    const fn selected_player_index(&self) -> u8 {
        if self.selected_player == 0 {
            0
        } else if self.selected_player > SGB_CONTROLLER_COUNT as u8 {
            SGB_CONTROLLER_COUNT as u8 - 1
        } else {
            self.selected_player - 1
        }
    }

    pub const fn selected_player_pressed_mask(&self) -> u8 {
        if self.player_count == 0 {
            0
        } else {
            self.player_pressed_masks[self.selected_player_index() as usize]
        }
    }

    pub fn set_player_pressed_mask(&mut self, player: u8, pressed_mask: u8) -> bool {
        let Some(player_index) = player_index(player) else {
            return false;
        };
        if self.player_pressed_masks[player_index] == pressed_mask {
            return false;
        }

        self.player_pressed_masks[player_index] = pressed_mask;
        true
    }

    pub fn set_player_pressed_masks(&mut self, pressed_masks: [u8; SGB_CONTROLLER_COUNT]) -> bool {
        if self.player_pressed_masks == pressed_masks {
            return false;
        }

        self.player_pressed_masks = pressed_masks;
        true
    }

    pub fn set_player_button_pressed(
        &mut self,
        player: u8,
        button: JoypadButton,
        pressed: bool,
    ) -> bool {
        let Some(player_index) = player_index(player) else {
            return false;
        };
        let bit = button_mask(button);
        let previous_mask = self.player_pressed_masks[player_index];
        let pressed_mask = if pressed {
            previous_mask | bit
        } else {
            previous_mask & !bit
        };
        if pressed_mask == previous_mask {
            return false;
        }

        self.player_pressed_masks[player_index] = pressed_mask;
        true
    }

    fn joyp_read_value(self, value: u8) -> u8 {
        if self.player_count > 1 && value & JOYP_SELECT_BITS_MASK == JOYP_SELECT_BITS_MASK {
            (value & 0xF0) | (0x0F - self.selected_player_index())
        } else {
            value
        }
    }
}

impl Default for SgbMultiplayerState {
    fn default() -> Self {
        Self::default_for_active_host(false)
    }
}

const fn player_index(player: u8) -> Option<usize> {
    if player == 0 || player > SGB_CONTROLLER_COUNT as u8 {
        None
    } else {
        Some((player - 1) as usize)
    }
}

impl SgbVideoState {
    fn default_for_active_host(active: bool) -> Self {
        Self {
            border_loaded: false,
            colorization_active: active,
            palette_state: SgbPaletteState::default_for_active_host(active),
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
        }
    }

    pub fn map_lcd_shade_to_rgb555(&self, shade: u8) -> SgbRgb555Color {
        self.visible_palette_state().map_lcd_shade(shade)
    }

    pub fn lcd_pixel_for_shade(&self, shade: u8) -> SgbRgb555Color {
        match self.mask {
            SgbScreenMask::Cancel => self.visible_palette_state().map_lcd_shade(shade),
            SgbScreenMask::Freeze => self.visible_palette_state().map_lcd_shade(shade),
            SgbScreenMask::BlankBlack => SGB_RGB555_BLACK,
            SgbScreenMask::BlankColor0 => self.visible_palette_state().map_lcd_shade(0),
        }
    }

    fn set_player_palette_override(&mut self, palette: SgbScreenPalette) -> bool {
        self.player_palette_override.set_uniform_palette(palette)
    }

    fn clear_player_palette_override(&mut self) -> bool {
        self.player_palette_override.clear_by_player()
    }

    fn visible_palette_state(&self) -> &SgbPaletteState {
        if self.player_palette_override.active {
            &self.player_palette_override.palette_state
        } else {
            &self.palette_state
        }
    }

    fn visible_attribute_map(&self) -> &SgbAttributeMap {
        if self.player_palette_override.active {
            &self.player_palette_override.attributes
        } else {
            &self.attributes.map
        }
    }

    fn apply_boot_palette_for_cartridge_header(
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
            SgbScreenMask::BlankColor0 => self.visible_palette_state().palette(0).color(0),
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
        self.visible_palette_state()
            .palette(palette_index)
            .color(shade)
    }

    fn apply_direct_palette_command(&mut self, command_id: u8, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.palette_state
            .apply_direct_palette_command(command_id, bytes);
        self.colorization_active = true;
        self.last_palette_command_id = Some(command_id);
        self.palette_command_count = self.palette_command_count.saturating_add(1);
        self.apply_pal_pri_application_priority();
    }

    fn apply_pal_set_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        let options = self
            .system_palettes
            .apply_pal_set(&mut self.palette_state, bytes);
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

    fn apply_pal_pri_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.system_palettes.apply_pal_pri(bytes);
    }

    fn apply_attr_blk_command(&mut self, payload: &[u8]) {
        self.attributes.apply_attr_blk(payload);
        self.colorization_active = true;
        self.apply_pal_pri_application_priority();
    }

    fn apply_attr_lin_command(&mut self, payload: &[u8]) {
        self.attributes.apply_attr_lin(payload);
        self.colorization_active = true;
        self.apply_pal_pri_application_priority();
    }

    fn apply_attr_div_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.attributes.apply_attr_div(bytes);
        self.colorization_active = true;
        self.apply_pal_pri_application_priority();
    }

    fn apply_attr_chr_command(&mut self, payload: &[u8]) {
        self.attributes.apply_attr_chr(payload);
        self.colorization_active = true;
        self.apply_pal_pri_application_priority();
    }

    fn apply_attr_set_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
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

    fn apply_mask_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.mask = SgbScreenMask::from_command_byte(bytes[1]);
        self.mask_command_count = self.mask_command_count.saturating_add(1);
        self.freeze_capture_pending = self.mask == SgbScreenMask::Freeze;
        if self.mask != SgbScreenMask::Freeze {
            self.frozen_lcd = None;
        }
    }

    fn request_chr_transfer(&mut self, command_id: u8, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.request_vram_transfer(
            command_id,
            SgbVramTransferTarget::Chr(SgbChrTransferSelection::from_command_byte(bytes[1])),
        );
    }

    fn request_pct_transfer(&mut self, command_id: u8) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::Pct);
    }

    fn request_pal_transfer(&mut self, command_id: u8) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::Pal);
    }

    fn request_attr_transfer(&mut self, command_id: u8) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::Attr);
    }

    fn request_sound_transfer(&mut self, command_id: u8) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::Sound);
    }

    fn request_snes_data_transfer(&mut self, command_id: u8, destination: SgbSnesAddress) {
        self.request_vram_transfer(command_id, SgbVramTransferTarget::SnesData(destination));
    }

    fn request_vram_transfer(&mut self, command_id: u8, target: SgbVramTransferTarget) {
        self.vram_transfer.pending = Some(SgbPendingVramTransfer {
            command_id,
            target,
            frame_starts_until_capture: 1,
        });
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
        let Some(mut pending) = self.vram_transfer.pending else {
            return Ok(None);
        };
        if pending.frame_starts_until_capture > 1 {
            pending.frame_starts_until_capture -= 1;
            self.vram_transfer.pending = Some(pending);
            return Ok(None);
        }
        let payload = SgbVramTransferBuffer::from_display_memory(vram_bytes, display)?;
        self.capture_pending_vram_transfer_payload(payload)
    }

    fn capture_pending_vram_transfer(
        &mut self,
        vram_bytes: &[u8],
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        let payload = SgbVramTransferBuffer::from_source_bytes(vram_bytes)?;
        self.capture_pending_vram_transfer_payload(payload)
    }

    fn capture_pending_vram_transfer_payload(
        &mut self,
        payload: SgbVramTransferBuffer,
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        let Some(pending) = self.vram_transfer.pending.take() else {
            return Err(SgbVramTransferError::NoPendingTransfer);
        };
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

    fn dynamic_payload_bytes(&self) -> usize {
        self.frozen_lcd
            .as_ref()
            .map(SgbLcdRgb555Frame::dynamic_payload_bytes)
            .unwrap_or(0)
            .saturating_add(self.vram_transfer.dynamic_payload_bytes())
            .saturating_add(self.system_palettes.dynamic_payload_bytes())
            .saturating_add(self.player_palette_override.dynamic_payload_bytes())
            .saturating_add(self.attributes.dynamic_payload_bytes())
            .saturating_add(self.border.dynamic_payload_bytes())
    }
}

impl Default for SgbVideoState {
    fn default() -> Self {
        Self::default_for_active_host(false)
    }
}

const fn direct_palette_command_pair(command_id: u8) -> Option<(usize, usize)> {
    match command_id {
        SGB_COMMAND_PAL01 => Some((0, 1)),
        SGB_COMMAND_PAL23 => Some((2, 3)),
        SGB_COMMAND_PAL03 => Some((0, 3)),
        SGB_COMMAND_PAL12 => Some((1, 2)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SgbVideoStandard;

    fn test_header(sgb_flag: SgbFlag, old_licensee_code: u8) -> CartridgeHeader {
        test_header_with_title(b"SGBTEST", sgb_flag, old_licensee_code)
    }

    fn test_header_with_title(
        title: &[u8],
        sgb_flag: SgbFlag,
        old_licensee_code: u8,
    ) -> CartridgeHeader {
        assert!(title.len() <= 16);
        let mut title_bytes = [0; 16];
        title_bytes[..title.len()].copy_from_slice(title);
        CartridgeHeader {
            entry_point: [0; 4],
            nintendo_logo: [0; 48],
            title_bytes,
            raw_title_suffix_or_manufacturer_code: [0; 4],
            title: String::from_utf8_lossy(title).to_string(),
            cgb_flag: crate::CgbFlag::None,
            sgb_flag,
            cartridge_type: 0,
            rom_size: crate::RomSizeInfo::decode(0x00),
            ram_size: crate::RamSizeInfo::decode(0x00),
            new_licensee_code: *b"01",
            destination_code: 0,
            old_licensee_code,
            header_checksum: 0,
        }
    }

    fn sgb_command_packet(command_id: u8, packet_count: u8) -> [u8; SGB_PACKET_BYTES] {
        let mut bytes = [0; SGB_PACKET_BYTES];
        bytes[0] = (command_id << 3) | packet_count;
        bytes
    }

    fn write_packet_color(bytes: &mut [u8; SGB_PACKET_BYTES], offset: usize, rgb555: u16) {
        let [low, high] = rgb555.to_le_bytes();
        bytes[offset] = low;
        bytes[offset + 1] = high;
    }

    fn sgb_pal01_packet() -> [u8; SGB_PACKET_BYTES] {
        let mut bytes = sgb_command_packet(SGB_COMMAND_PAL01, 1);
        write_packet_color(&mut bytes, 1, 0x001F);
        write_packet_color(&mut bytes, 3, 0x03E0);
        write_packet_color(&mut bytes, 5, 0x7C00);
        write_packet_color(&mut bytes, 7, 0x4210);
        write_packet_color(&mut bytes, 9, 0x0001);
        write_packet_color(&mut bytes, 11, 0x0002);
        write_packet_color(&mut bytes, 13, 0x8003);
        bytes
    }

    fn sgb_chr_trn_packet(destination: u8) -> [u8; SGB_PACKET_BYTES] {
        let mut bytes = sgb_command_packet(SGB_COMMAND_CHR_TRN, 1);
        bytes[1] = destination;
        bytes
    }

    fn sgb_pct_trn_packet() -> [u8; SGB_PACKET_BYTES] {
        sgb_command_packet(SGB_COMMAND_PCT_TRN, 1)
    }

    fn sgb_mask_packet(mask: SgbScreenMask) -> [u8; SGB_PACKET_BYTES] {
        let mut bytes = sgb_command_packet(SGB_COMMAND_MASK_EN, 1);
        bytes[1] = match mask {
            SgbScreenMask::Cancel => 0,
            SgbScreenMask::Freeze => 1,
            SgbScreenMask::BlankBlack => 2,
            SgbScreenMask::BlankColor0 => 3,
        };
        bytes
    }

    fn sgb_pal_trn_packet() -> [u8; SGB_PACKET_BYTES] {
        sgb_command_packet(SGB_COMMAND_PAL_TRN, 1)
    }

    fn sgb_attr_trn_packet() -> [u8; SGB_PACKET_BYTES] {
        sgb_command_packet(SGB_COMMAND_ATTR_TRN, 1)
    }

    fn sgb_mlt_req_packet(control: u8) -> [u8; SGB_PACKET_BYTES] {
        let mut bytes = sgb_command_packet(SGB_COMMAND_MLT_REQ, 1);
        bytes[1] = control;
        bytes
    }

    fn sgb_sound_packet() -> [u8; SGB_PACKET_BYTES] {
        let mut bytes = sgb_command_packet(SGB_COMMAND_SOUND, 1);
        bytes[1] = 0x17;
        bytes[2] = 0x24;
        bytes[3] = 0b10_01_11_00;
        bytes[4] = 0x05;
        bytes
    }

    fn sgb_sou_trn_packet() -> [u8; SGB_PACKET_BYTES] {
        sgb_command_packet(SGB_COMMAND_SOU_TRN, 1)
    }

    fn sgb_data_snd_packet() -> [u8; SGB_PACKET_BYTES] {
        let mut bytes = sgb_command_packet(SGB_COMMAND_DATA_SND, 1);
        bytes[1] = 0x00;
        bytes[2] = 0x21;
        bytes[3] = 0x7E;
        bytes[4] = 3;
        bytes[5] = 0xAA;
        bytes[6] = 0xBB;
        bytes[7] = 0xCC;
        bytes
    }

    fn sgb_data_trn_packet() -> [u8; SGB_PACKET_BYTES] {
        let mut bytes = sgb_command_packet(SGB_COMMAND_DATA_TRN, 1);
        bytes[1] = 0x00;
        bytes[2] = 0x22;
        bytes[3] = 0x7E;
        bytes
    }

    fn write_transfer_screen_tile(vram: &mut [u8], transfer_tile_index: usize, tile_index: u8) {
        let tile_x = transfer_tile_index % SGB_TRANSFER_DISPLAY_TILE_COLUMNS;
        let tile_y = transfer_tile_index / SGB_TRANSFER_DISPLAY_TILE_COLUMNS;
        vram[SGB_GB_BG_MAP_9800_OFFSET + tile_y * SGB_GB_TILEMAP_WIDTH + tile_x] = tile_index;
    }

    fn sgb_jump_packet() -> [u8; SGB_PACKET_BYTES] {
        let mut bytes = sgb_command_packet(SGB_COMMAND_JUMP, 1);
        bytes[1] = 0x34;
        bytes[2] = 0x12;
        bytes[3] = 0x7E;
        bytes[4] = 0x78;
        bytes[5] = 0x56;
        bytes[6] = 0x7E;
        bytes
    }

    fn write_system_palette_color(
        bytes: &mut [u8; SGB_VRAM_TRANSFER_BYTES],
        palette_index: usize,
        color_index: usize,
        rgb555: u16,
    ) {
        let [low, high] = rgb555.to_le_bytes();
        let offset = palette_index * SGB_SCREEN_PALETTE_COLORS * 2 + color_index * 2;
        bytes[offset] = low;
        bytes[offset + 1] = high;
    }

    fn write_atf_cell(
        bytes: &mut [u8; SGB_VRAM_TRANSFER_BYTES],
        atf_index: usize,
        cell_x: usize,
        cell_y: usize,
        palette_index: u8,
    ) {
        let offset = atf_index * SGB_ATF_BYTES + cell_y * 5 + cell_x / 4;
        let shift = 6 - (cell_x % 4) * 2;
        bytes[offset] &= !(0x03 << shift);
        bytes[offset] |= (palette_index & 0x03) << shift;
    }

    fn write_border_map_entry(bytes: &mut [u8; SGB_VRAM_TRANSFER_BYTES], entry: usize, raw: u16) {
        let [low, high] = raw.to_le_bytes();
        bytes[entry * 2] = low;
        bytes[entry * 2 + 1] = high;
    }

    fn write_border_palette_color(
        bytes: &mut [u8; SGB_VRAM_TRANSFER_BYTES],
        palette_index: usize,
        color_index: usize,
        rgb555: u16,
    ) {
        let [low, high] = rgb555.to_le_bytes();
        let offset = 0x800 + palette_index * SGB_BORDER_PALETTE_COLORS * 2 + color_index * 2;
        bytes[offset] = low;
        bytes[offset + 1] = high;
    }

    fn solid_tile_color_1_transfer() -> [u8; SGB_VRAM_TRANSFER_BYTES] {
        let mut bytes = [0; SGB_VRAM_TRANSFER_BYTES];
        for row in 0..8 {
            bytes[row * 2] = 0xFF;
        }
        bytes
    }

    fn write_joyp_idle(host: &mut SgbHost) {
        host.observe_joyp_write(SGB_JOYP_IDLE_BITS);
    }

    fn write_joyp_start(host: &mut SgbHost) {
        host.observe_joyp_write(SGB_JOYP_START_BITS);
        write_joyp_idle(host);
    }

    fn write_joyp_data_bit(host: &mut SgbHost, bit: u8) {
        host.observe_joyp_write(if bit == 0 {
            SGB_JOYP_ZERO_BITS
        } else {
            SGB_JOYP_ONE_BITS
        });
        write_joyp_idle(host);
    }

    fn write_joyp_packet(host: &mut SgbHost, bytes: [u8; SGB_PACKET_BYTES]) {
        write_joyp_start(host);
        for byte in bytes {
            for bit_index in 0..8 {
                write_joyp_data_bit(host, (byte >> bit_index) & 0x01);
            }
        }
        write_joyp_data_bit(host, 0);
    }

    fn cycle_sgb_player(host: &mut SgbHost) {
        host.observe_joyp_write(SGB_JOYP_ONE_BITS);
        host.observe_joyp_write(SGB_JOYP_IDLE_BITS);
    }

    fn sgb_player_id_value(host: &SgbHost) -> u8 {
        host.joyp_read_value(0xFF)
    }

    fn accepted_sgb_host() -> SgbHost {
        let mut host = SgbHost::new(HostPlatform::Sgb);
        let header = test_header(SgbFlag::Supported, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
        host.apply_cartridge_header(Some(&header));
        host
    }

    #[test]
    fn vram_transfer_display_extraction_follows_signed_tiledata_transfer_screen() {
        let mut vram = vec![0; SGB_GB_VRAM_BYTES];
        for transfer_tile_index in 0..SGB_TRANSFER_DISPLAY_TILE_COUNT {
            let tile_index = 0x80u8.wrapping_add(transfer_tile_index as u8);
            write_transfer_screen_tile(&mut vram, transfer_tile_index, tile_index);
            let tile_offset = gb_tile_data_offset(0x81, tile_index);
            for byte_index in 0..SGB_GB_TILE_BYTES {
                vram[tile_offset + byte_index] = transfer_tile_index.wrapping_add(byte_index) as u8;
            }
        }

        let payload = SgbVramTransferBuffer::from_display_memory(
            &vram,
            SgbVramTransferDisplayState::new(0x81, 0, 0, SGB_TRANSFER_REQUIRED_BGP),
        )
        .expect("SGB transfer display should decode into a 4 KiB payload");

        assert_eq!(payload.bytes[0], 0);
        assert_eq!(payload.bytes[15], 15);
        assert_eq!(payload.bytes[128 * SGB_GB_TILE_BYTES], 128);
        assert_eq!(payload.bytes[255 * SGB_GB_TILE_BYTES], 255);
    }

    #[test]
    fn vram_transfer_display_extraction_falls_back_to_raw_when_display_is_not_prepared() {
        let mut vram = vec![0; SGB_GB_VRAM_BYTES];
        vram[0] = 0x5A;
        vram[gb_tile_data_offset(0x81, 0x80)] = 0xA5;
        write_transfer_screen_tile(&mut vram, 0, 0x80);

        let payload = SgbVramTransferBuffer::from_display_memory(
            &vram,
            SgbVramTransferDisplayState::new(0x81, 1, 0, SGB_TRANSFER_REQUIRED_BGP),
        )
        .expect("unprepared transfer display falls back to the legacy raw payload path");

        assert_eq!(payload.bytes[0], 0x5A);
    }

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
        assert_eq!(
            SgbHostProfile::SgbNtsc
                .timing()
                .gb_master_clock_hz
                .rounded_hz(),
            4_295_454
        );
        assert_eq!(
            SgbHostProfile::SgbPal
                .timing()
                .gb_master_clock_hz
                .rounded_hz(),
            4_256_274
        );
        assert_eq!(
            SgbHostProfile::Sgb2Ntsc
                .timing()
                .gb_master_clock_hz
                .rounded_hz(),
            4_194_304
        );
    }

    #[test]
    fn host_state_is_inert_for_handheld_and_ready_for_sgb_profiles() {
        let handheld = SgbHost::new(HostPlatform::Handheld);
        assert_eq!(handheld.status(), SgbHostStatus::Disabled);
        assert_eq!(handheld.profile(), None);
        assert_eq!(
            handheld.command_acceptance(),
            SgbCommandAcceptance::Disabled
        );
        assert_eq!(handheld.snapshot().multiplayer.player_count, 0);

        let sgb = SgbHost::new(HostPlatform::Sgb);
        assert_eq!(sgb.status(), SgbHostStatus::Ready);
        assert_eq!(sgb.profile(), Some(SgbHostProfile::SgbNtsc));
        assert_eq!(sgb.backend_kind(), SgbHostBackendKind::DeterministicHle);
        assert_eq!(
            sgb.command_acceptance(),
            SgbCommandAcceptance::AwaitingCartridgeHeader
        );
        assert_eq!(sgb.snapshot().multiplayer.player_count, 1);
        assert!(!sgb.game_link_supported());

        let sgb2 = SgbHost::new(HostPlatform::Sgb2);
        assert_eq!(sgb2.profile(), Some(SgbHostProfile::Sgb2Ntsc));
        assert!(sgb2.game_link_supported());
        assert!(sgb2.corrected_clock());
    }

    #[test]
    fn host_backend_contract_records_sound_data_and_jump_requests() {
        let mut host = accepted_sgb_host();

        write_joyp_packet(&mut host, sgb_sound_packet());
        let sound = SgbSoundRequest::from_packet(&sgb_sound_packet());
        assert_eq!(
            host.snapshot().audio.last_request,
            Some(SgbHostAudioRequest::Sound(sound))
        );
        assert_eq!(host.snapshot().audio.sound_command_count, 1);
        assert_eq!(host.snapshot().audio.pending_host_audio_events, 1);
        assert_eq!(sound.effect_a.code, 0x17);
        assert_eq!(sound.effect_a.pitch, 0);
        assert_eq!(sound.effect_a.volume, 3);
        assert_eq!(sound.effect_b.code, 0x24);
        assert_eq!(sound.effect_b.pitch, 1);
        assert_eq!(sound.effect_b.volume, 2);
        assert_eq!(sound.music_score, 0x05);

        write_joyp_packet(&mut host, sgb_data_snd_packet());
        let data_snd = SgbDataSendRequest::from_packet(&sgb_data_snd_packet());
        assert_eq!(data_snd.destination, SgbSnesAddress::new(0x7E, 0x2100));
        assert_eq!(data_snd.payload(), &[0xAA, 0xBB, 0xCC]);
        assert_eq!(
            host.snapshot().snes_host.last_request,
            Some(SgbSnesHostRequest::DataSend(data_snd))
        );
        assert_eq!(host.snapshot().snes_host.data_snd_count, 1);
        assert_eq!(host.snapshot().snes_host.uploaded_payload_bytes, 3);

        write_joyp_packet(&mut host, sgb_data_trn_packet());
        assert_eq!(
            host.snapshot().video.vram_transfer.pending,
            Some(SgbPendingVramTransfer {
                command_id: SGB_COMMAND_DATA_TRN,
                target: SgbVramTransferTarget::SnesData(SgbSnesAddress::new(0x7E, 0x2200)),
                frame_starts_until_capture: 1,
            })
        );
        host.capture_pending_vram_transfer(&[0x42; SGB_VRAM_TRANSFER_BYTES])
            .expect("DATA_TRN should capture through the shared 4 KiB transfer seam");
        assert_eq!(
            host.snapshot().snes_host.last_request,
            Some(SgbSnesHostRequest::DataTransfer(SgbDataTransferRequest {
                destination: SgbSnesAddress::new(0x7E, 0x2200),
                payload_bytes: SGB_SNES_DATA_TRN_BYTES,
            }))
        );
        assert_eq!(host.snapshot().snes_host.data_trn_count, 1);
        assert_eq!(
            host.snapshot().snes_host.uploaded_payload_bytes,
            3 + SGB_SNES_DATA_TRN_BYTES
        );

        write_joyp_packet(&mut host, sgb_jump_packet());
        assert_eq!(
            host.snapshot().snes_host.last_request,
            Some(SgbSnesHostRequest::Jump(SgbJumpRequest {
                program_counter: SgbSnesAddress::new(0x7E, 0x1234),
                nmi_handler: SgbSnesAddress::new(0x7E, 0x5678),
            }))
        );
        assert!(host.snapshot().snes_host.execution_enabled);
        assert_eq!(
            host.snapshot().snes_host.program_counter,
            Some(SgbSnesAddress::new(0x7E, 0x1234))
        );
        assert_eq!(host.snapshot().snes_host.jump_count, 1);
    }

    #[test]
    fn sound_transfer_uses_the_shared_vram_transfer_backend_seam_and_survives_save_state() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_sou_trn_packet());
        assert_eq!(
            host.snapshot().video.vram_transfer.pending,
            Some(SgbPendingVramTransfer {
                command_id: SGB_COMMAND_SOU_TRN,
                target: SgbVramTransferTarget::Sound,
                frame_starts_until_capture: 1,
            })
        );

        let mut payload = [0; SGB_VRAM_TRANSFER_BYTES];
        payload[0] = 0x04;
        payload[1] = 0x00;
        payload[2] = 0x80;
        payload[3] = 0x21;
        host.capture_pending_vram_transfer(&payload)
            .expect("SOU_TRN should capture through the shared 4 KiB transfer seam");

        let expected = SgbHostAudioRequest::SoundTransfer(SgbSoundTransferRequest {
            first_packet: SgbSoundTransferPacket::Data {
                size: 4,
                destination: SgbApuRamAddress::new(0x2180),
            },
            payload_bytes: SGB_VRAM_TRANSFER_BYTES as u32,
        });
        assert_eq!(host.snapshot().audio.last_request, Some(expected));
        assert_eq!(host.snapshot().audio.sound_transfer_count, 1);
        assert_eq!(
            host.snapshot().audio.transferred_payload_bytes,
            SGB_VRAM_TRANSFER_BYTES as u32
        );

        let saved = host.capture_save_state();
        let mut restored = SgbHost::new(HostPlatform::Sgb);
        restored.restore_save_state(&saved);
        assert_eq!(restored.snapshot().audio.last_request, Some(expected));
        assert_eq!(
            restored.snapshot().video.vram_transfer.last_completed,
            host.snapshot().video.vram_transfer.last_completed
        );
    }

    #[test]
    fn real_boot_startup_selects_profile_specific_boot_asset() {
        let sgb = SgbHost::new_with_startup(HostPlatform::Sgb, StartupMode::RealBoot);
        assert_eq!(sgb.startup().startup_mode, StartupMode::RealBoot);
        assert_eq!(
            sgb.startup().real_boot_asset,
            Some(SgbRealBootAsset::SgbBoot)
        );
        assert_eq!(
            sgb.startup()
                .real_boot_asset
                .map(SgbRealBootAsset::filename),
            Some("sgb_boot.bin")
        );

        let sgb2 = SgbHost::new_with_startup(HostPlatform::Sgb2, StartupMode::RealBoot);
        assert_eq!(
            sgb2.startup().real_boot_asset,
            Some(SgbRealBootAsset::Sgb2Boot)
        );
        assert_eq!(
            sgb2.startup()
                .real_boot_asset
                .map(SgbRealBootAsset::filename),
            Some("sgb2_boot.bin")
        );

        let handheld = SgbHost::new_with_startup(HostPlatform::Handheld, StartupMode::RealBoot);
        assert_eq!(handheld.startup().real_boot_asset, None);
    }

    #[test]
    fn cartridge_header_controls_sgb_command_acceptance() {
        let supported = test_header(SgbFlag::Supported, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
        let unsupported = test_header(SgbFlag::None, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
        let wrong_licensee = test_header(SgbFlag::Supported, 0x01);

        let mut sgb = SgbHost::new(HostPlatform::Sgb);
        assert_eq!(
            sgb.command_acceptance(),
            SgbCommandAcceptance::AwaitingCartridgeHeader
        );
        sgb.apply_cartridge_header(Some(&supported));
        assert_eq!(sgb.command_acceptance(), SgbCommandAcceptance::Accepted);
        assert_eq!(sgb.startup().cartridge_sgb_flag, Some(SgbFlag::Supported));
        assert_eq!(sgb.startup().old_licensee_code, Some(0x33));

        sgb.apply_cartridge_header(Some(&unsupported));
        assert_eq!(
            sgb.command_acceptance(),
            SgbCommandAcceptance::RejectedByHeader
        );

        sgb.apply_cartridge_header(Some(&wrong_licensee));
        assert_eq!(
            sgb.command_acceptance(),
            SgbCommandAcceptance::RejectedByHeader
        );

        sgb.apply_cartridge_header(None);
        assert_eq!(
            sgb.command_acceptance(),
            SgbCommandAcceptance::AwaitingCartridgeHeader
        );

        let mut handheld = SgbHost::new(HostPlatform::Handheld);
        handheld.apply_cartridge_header(Some(&supported));
        assert_eq!(
            handheld.command_acceptance(),
            SgbCommandAcceptance::Disabled
        );
    }

    #[test]
    fn sgb_boot_palette_seeds_default_and_title_matched_dmg_palettes() {
        let mut sgb = SgbHost::new(HostPlatform::Sgb);
        assert_eq!(
            sgb.snapshot().video.palette_state.palette(0),
            sgb_boot_palette(SGB_BOOT_PALETTE_DEFAULT_INDEX)
        );

        let alleyway = test_header_with_title(
            b"ALLEY WAY",
            SgbFlag::None,
            SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
        );
        sgb.apply_cartridge_header(Some(&alleyway));
        assert_eq!(
            sgb.command_acceptance(),
            SgbCommandAcceptance::RejectedByHeader
        );
        assert_eq!(
            sgb.snapshot().video.palette_state.palette(0),
            sgb_boot_palette(0x16)
        );
        assert_eq!(
            sgb.snapshot().video.map_lcd_shade_to_rgb555(0).raw(),
            0x65EF
        );
        assert_eq!(
            sgb.snapshot().video.map_lcd_shade_to_rgb555(3).raw(),
            0x2108
        );

        let unknown = test_header_with_title(
            b"UNKNOWN",
            SgbFlag::None,
            SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
        );
        sgb.apply_cartridge_header(Some(&unknown));
        assert_eq!(
            sgb.snapshot().video.palette_state.palette(0),
            sgb_boot_palette(SGB_BOOT_PALETTE_DEFAULT_INDEX)
        );
    }

    #[test]
    fn sgb_boot_title_palette_requires_rejected_exact_padded_title() {
        let sgb_capable_alleyway = test_header_with_title(
            b"ALLEY WAY",
            SgbFlag::Supported,
            SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
        );
        let mut sgb = SgbHost::new(HostPlatform::Sgb);
        sgb.apply_cartridge_header(Some(&sgb_capable_alleyway));
        assert_eq!(sgb.command_acceptance(), SgbCommandAcceptance::Accepted);
        assert_eq!(
            sgb.snapshot().video.palette_state.palette(0),
            sgb_boot_palette(SGB_BOOT_PALETTE_DEFAULT_INDEX),
            "SGB-command-capable games keep the default boot palette until commands override it"
        );

        let mut space_padded = test_header_with_title(
            b"ALLEY WAY",
            SgbFlag::None,
            SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
        );
        space_padded.title_bytes[b"ALLEY WAY".len()] = b' ';
        sgb.apply_cartridge_header(Some(&space_padded));
        assert_eq!(
            sgb.snapshot().video.palette_state.palette(0),
            sgb_boot_palette(SGB_BOOT_PALETTE_DEFAULT_INDEX),
            "the SGB BIOS title table uses exact NUL-padded header title matches"
        );

        let handheld = SgbHost::new(HostPlatform::Handheld);
        assert_eq!(
            handheld.snapshot().video.palette_state.palette(0),
            SgbScreenPalette::dmg_grayscale()
        );
    }

    #[test]
    fn joyp_transport_decodes_single_packet_commands_lsb_first() {
        let mut host = accepted_sgb_host();
        let packet = sgb_command_packet(0x11, 1);
        write_joyp_packet(&mut host, packet);

        let snapshot = host.snapshot();
        assert_eq!(snapshot.packet_transport.reset_pulse_count, 1);
        assert_eq!(snapshot.packet_transport.data_pulse_count, 129);
        assert_eq!(snapshot.packet_transport.packet_bits_buffered, 128);
        assert_eq!(snapshot.packet_transport.packet_bytes_buffered, 16);
        assert_eq!(
            snapshot.packet_transport.last_trace.status,
            SgbPacketTraceStatus::Complete
        );
        assert_eq!(snapshot.packet_transport.last_trace.command_id, Some(0x11));
        assert_eq!(snapshot.packet_transport.last_trace.packet_count, 1);
        assert_eq!(snapshot.packet_transport.last_trace.packet_index, 1);
        assert_eq!(snapshot.command.last_command_id, Some(0x11));
        assert_eq!(snapshot.command.accepted_command_count, 1);
    }

    #[test]
    fn mlt_req_selects_one_two_and_four_player_modes() {
        let mut host = accepted_sgb_host();

        write_joyp_packet(&mut host, sgb_mlt_req_packet(1));
        assert_eq!(host.snapshot().multiplayer.player_count, 2);
        assert_eq!(host.snapshot().multiplayer.selected_player, 1);
        assert_eq!(sgb_player_id_value(&host), 0xFF);

        cycle_sgb_player(&mut host);
        assert_eq!(host.snapshot().multiplayer.selected_player, 2);
        assert_eq!(sgb_player_id_value(&host), 0xFE);

        write_joyp_packet(&mut host, sgb_mlt_req_packet(0));
        assert_eq!(host.snapshot().multiplayer.player_count, 1);
        assert_eq!(host.snapshot().multiplayer.selected_player, 1);
        assert_eq!(
            sgb_player_id_value(&host),
            0xFF,
            "one-player mode keeps both P1 rows deselected as ordinary open lines"
        );

        write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
        assert_eq!(host.snapshot().multiplayer.player_count, 4);
        assert_eq!(host.snapshot().multiplayer.selected_player, 1);
        assert_eq!(sgb_player_id_value(&host), 0xFF);
        cycle_sgb_player(&mut host);
        assert_eq!(sgb_player_id_value(&host), 0xFE);
        cycle_sgb_player(&mut host);
        assert_eq!(sgb_player_id_value(&host), 0xFD);
        cycle_sgb_player(&mut host);
        assert_eq!(sgb_player_id_value(&host), 0xFC);
        assert_eq!(
            host.snapshot().multiplayer.player_cycle_count,
            8,
            "SGB player cycling also observes the P15 rises in MLT_REQ packet transport while multiplayer is already enabled"
        );
    }

    #[test]
    fn mlt_req_packet_transport_cycles_player_before_mode_change() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
        assert_eq!(sgb_player_id_value(&host), 0xFF);

        write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
        assert_eq!(
            sgb_player_id_value(&host),
            0xFD,
            "sending MLT_REQ 3 while already in four-player mode cycles the player through the command transport pulses before the command side effect"
        );

        write_joyp_packet(&mut host, sgb_mlt_req_packet(1));
        assert_eq!(
            sgb_player_id_value(&host),
            0xFE,
            "switching from four-player to two-player mode masks the already-cycled player index"
        );
    }

    #[test]
    fn mlt_req_control_2_preserves_hardware_glitched_three_player_state() {
        let mut host = accepted_sgb_host();

        write_joyp_packet(&mut host, sgb_mlt_req_packet(0));
        write_joyp_packet(&mut host, sgb_mlt_req_packet(2));
        assert_eq!(host.snapshot().multiplayer.player_count, 3);
        assert_eq!(sgb_player_id_value(&host), 0xFF);
        cycle_sgb_player(&mut host);
        assert_eq!(
            sgb_player_id_value(&host),
            0xFF,
            "control 2 leaves an odd three-player selector that does not cycle on P15 rises"
        );

        write_joyp_packet(&mut host, sgb_mlt_req_packet(0));
        write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
        write_joyp_packet(&mut host, sgb_mlt_req_packet(2));
        assert_eq!(
            sgb_player_id_value(&host),
            0xFD,
            "control 2 maps the transport-cycled four-player index onto the hardware-observed player 1/player 3 pair"
        );

        write_joyp_packet(&mut host, sgb_mlt_req_packet(0));
        write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
        cycle_sgb_player(&mut host);
        cycle_sgb_player(&mut host);
        write_joyp_packet(&mut host, sgb_mlt_req_packet(2));
        assert_eq!(sgb_player_id_value(&host), 0xFF);
    }

    #[test]
    fn palette_commands_update_host_palette_state_and_rgb555_mapping() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_pal01_packet());

        let snapshot = host.snapshot();
        assert_eq!(snapshot.command.last_command_id, Some(SGB_COMMAND_PAL01));
        assert_eq!(
            snapshot.video.last_palette_command_id,
            Some(SGB_COMMAND_PAL01)
        );
        assert_eq!(snapshot.video.palette_command_count, 1);
        assert!(snapshot.video.colorization_active);

        let palette_0 = snapshot.video.palette_state.palette(0);
        assert_eq!(palette_0.colors[0].raw(), 0x001F);
        assert_eq!(palette_0.colors[1].raw(), 0x03E0);
        assert_eq!(palette_0.colors[2].raw(), 0x7C00);
        assert_eq!(palette_0.colors[3].raw(), 0x4210);

        let palette_1 = snapshot.video.palette_state.palette(1);
        assert_eq!(palette_1.colors[0].raw(), 0x001F);
        assert_eq!(palette_1.colors[1].raw(), 0x0001);
        assert_eq!(palette_1.colors[2].raw(), 0x0002);
        assert_eq!(
            palette_1.colors[3].raw(),
            0x0003,
            "SGB RGB555 colors mask off the ignored high bit"
        );
        assert_eq!(
            snapshot.video.palette_state.palette(2),
            SgbScreenPalette::dmg_grayscale()
        );
        assert_eq!(snapshot.video.map_lcd_shade_to_rgb555(0).raw(), 0x001F);
        assert_eq!(snapshot.video.map_lcd_shade_to_rgb555(1).raw(), 0x03E0);
        assert_eq!(snapshot.video.map_lcd_shade_to_rgb555(2).raw(), 0x7C00);
        assert_eq!(snapshot.video.map_lcd_shade_to_rgb555(3).raw(), 0x4210);
    }

    #[test]
    fn direct_palette_commands_target_documented_palette_pairs() {
        for (command_id, first_palette, second_palette) in [
            (SGB_COMMAND_PAL01, 0, 1),
            (SGB_COMMAND_PAL23, 2, 3),
            (SGB_COMMAND_PAL03, 0, 3),
            (SGB_COMMAND_PAL12, 1, 2),
        ] {
            let mut host = accepted_sgb_host();
            let mut packet = sgb_command_packet(command_id, 1);
            write_packet_color(&mut packet, 1, 0x001F);
            write_packet_color(&mut packet, 3, 0x03E0);
            write_packet_color(&mut packet, 5, 0x7C00);
            write_packet_color(&mut packet, 7, 0x4210);
            write_packet_color(&mut packet, 9, 0x0001);
            write_packet_color(&mut packet, 11, 0x0002);
            write_packet_color(&mut packet, 13, 0x0003);

            write_joyp_packet(&mut host, packet);

            let palettes = host.snapshot().video.palette_state.screen_palettes;
            assert_eq!(palettes[first_palette].colors[0].raw(), 0x001F);
            assert_eq!(palettes[first_palette].colors[1].raw(), 0x03E0);
            assert_eq!(palettes[first_palette].colors[2].raw(), 0x7C00);
            assert_eq!(palettes[first_palette].colors[3].raw(), 0x4210);
            assert_eq!(palettes[second_palette].colors[0].raw(), 0x001F);
            assert_eq!(palettes[second_palette].colors[1].raw(), 0x0001);
            assert_eq!(palettes[second_palette].colors[2].raw(), 0x0002);
            assert_eq!(palettes[second_palette].colors[3].raw(), 0x0003);
        }
    }

    #[test]
    fn sgb_lcd_composition_maps_dmg_shades_through_base_palette() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_pal01_packet());
        let mut dmg_framebuffer = vec![0; SGB_LCD_PIXELS];
        dmg_framebuffer[..8].copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7]);

        let rgb555 = host
            .compose_lcd_rgb555(&dmg_framebuffer)
            .expect("active SGB host should compose the GB LCD image");
        assert_eq!(
            &rgb555[..8],
            &[
                0x001F, 0x03E0, 0x7C00, 0x4210, 0x001F, 0x03E0, 0x7C00, 0x4210
            ]
        );

        let handheld = SgbHost::new(HostPlatform::Handheld);
        assert_eq!(
            handheld.compose_lcd_rgb555(&dmg_framebuffer),
            Err(SgbLcdCompositionError::DisabledHost)
        );
        assert_eq!(
            host.compose_lcd_rgb555(&dmg_framebuffer[..SGB_LCD_PIXELS - 1]),
            Err(SgbLcdCompositionError::InputLength {
                expected: SGB_LCD_PIXELS,
                actual: SGB_LCD_PIXELS - 1,
            })
        );
    }

    #[test]
    fn attribute_commands_update_host_attribute_map_and_lcd_composition() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_pal01_packet());

        let mut attr_div = sgb_command_packet(SGB_COMMAND_ATTR_DIV, 1);
        attr_div[1] = (1 << 2) | (2 << 4);
        attr_div[2] = 1;
        write_joyp_packet(&mut host, attr_div);
        assert_eq!(host.snapshot().video.attributes.map.palette_index(0, 0), 1);
        assert_eq!(host.snapshot().video.attributes.map.palette_index(1, 0), 2);
        assert_eq!(host.snapshot().video.attributes.map.palette_index(2, 0), 0);

        let mut attr_lin = sgb_command_packet(SGB_COMMAND_ATTR_LIN, 1);
        attr_lin[1] = 1;
        attr_lin[2] = 0x80 | (3 << 5);
        write_joyp_packet(&mut host, attr_lin);
        assert_eq!(host.snapshot().video.attributes.map.palette_index(0, 0), 3);
        assert_eq!(host.snapshot().video.attributes.attr_lin_count, 1);

        let mut attr_blk = sgb_command_packet(SGB_COMMAND_ATTR_BLK, 1);
        attr_blk[1] = 1;
        attr_blk[2] = 0x03;
        attr_blk[3] = 2 | (1 << 2);
        attr_blk[4] = 1;
        attr_blk[5] = 1;
        attr_blk[6] = 3;
        attr_blk[7] = 3;
        write_joyp_packet(&mut host, attr_blk);
        assert_eq!(
            host.snapshot().video.attributes.map.palette_index(1, 1),
            1,
            "ATTR_BLK line cells use the surrounding palette"
        );
        assert_eq!(
            host.snapshot().video.attributes.map.palette_index(2, 2),
            2,
            "ATTR_BLK inner cells use the inside palette"
        );

        let mut attr_chr = sgb_command_packet(SGB_COMMAND_ATTR_CHR, 1);
        attr_chr[1] = 0;
        attr_chr[2] = 0;
        attr_chr[3] = 4;
        attr_chr[4] = 0;
        attr_chr[5] = 0;
        attr_chr[6] = 0b00_01_10_11;
        write_joyp_packet(&mut host, attr_chr);
        assert_eq!(host.snapshot().video.attributes.map.palette_index(0, 0), 0);
        assert_eq!(host.snapshot().video.attributes.map.palette_index(1, 0), 1);
        assert_eq!(host.snapshot().video.attributes.map.palette_index(2, 0), 2);
        assert_eq!(host.snapshot().video.attributes.map.palette_index(3, 0), 3);

        let dmg_framebuffer = vec![1; SGB_LCD_PIXELS];
        let rgb555 = host
            .compose_lcd_rgb555(&dmg_framebuffer)
            .expect("SGB LCD composition should use host attribute palettes");
        assert_eq!(rgb555[8], 0x0001);
    }

    #[test]
    fn attr_chr_uses_multi_packet_payload_data_after_the_first_packet() {
        let mut host = accepted_sgb_host();
        let mut first_packet = sgb_command_packet(SGB_COMMAND_ATTR_CHR, 2);
        first_packet[1] = 0;
        first_packet[2] = 0;
        first_packet[3] = 44;
        first_packet[4] = 0;
        first_packet[5] = 0;
        let mut second_packet = [0; SGB_PACKET_BYTES];
        second_packet[0] = 0b11_00_00_00;

        write_joyp_packet(&mut host, first_packet);
        write_joyp_packet(&mut host, second_packet);

        assert_eq!(
            host.snapshot().command.last_command_id,
            Some(SGB_COMMAND_ATTR_CHR)
        );
        assert_eq!(
            host.snapshot().video.attributes.map.palette_index(0, 2),
            3,
            "data set 41 is stored in the first byte of the second SGB packet"
        );
    }

    #[test]
    fn pal_trn_pal_set_and_pal_pri_update_system_palette_state() {
        let mut host = accepted_sgb_host();
        let mut transfer = [0; SGB_VRAM_TRANSFER_BYTES];
        write_system_palette_color(&mut transfer, 7, 0, 0x001F);
        write_system_palette_color(&mut transfer, 7, 1, 0x03E0);
        write_system_palette_color(&mut transfer, 7, 2, 0x7C00);
        write_system_palette_color(&mut transfer, 7, 3, 0x8421);

        write_joyp_packet(&mut host, sgb_pal_trn_packet());
        assert_eq!(
            host.capture_pending_vram_transfer(&transfer)
                .expect("PAL_TRN should capture system palette data"),
            Some(SgbVramTransferTarget::Pal)
        );

        let mut pal_set = sgb_command_packet(SGB_COMMAND_PAL_SET, 1);
        for palette_index in 0..SGB_SCREEN_PALETTE_COUNT {
            let [low, high] = 7u16.to_le_bytes();
            pal_set[1 + palette_index * 2] = low;
            pal_set[2 + palette_index * 2] = high;
        }
        write_joyp_packet(&mut host, pal_set);

        let snapshot = host.snapshot();
        assert!(snapshot.video.system_palettes.loaded);
        assert_eq!(snapshot.video.system_palettes.pal_trn_count, 1);
        assert_eq!(snapshot.video.system_palettes.pal_set_count, 1);
        assert_eq!(
            snapshot.video.palette_state.palette(0).colors[0].raw(),
            0x001F
        );
        assert_eq!(
            snapshot.video.palette_state.palette(0).colors[1].raw(),
            0x03E0
        );
        assert_eq!(
            snapshot.video.palette_state.palette(0).colors[3].raw(),
            0x0421,
            "PAL_TRN colors keep RGB555 bit 15 ignored"
        );

        let mut pal_pri = sgb_command_packet(SGB_COMMAND_PAL_PRI, 1);
        pal_pri[1] = 1;
        write_joyp_packet(&mut host, pal_pri);
        assert!(host.snapshot().video.system_palettes.pal_pri_enabled);
        assert_eq!(
            host.snapshot().video.system_palettes.pal_pri_command_count,
            1
        );
    }

    #[test]
    fn pal_pri_controls_player_palette_override_priority() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_pal01_packet());

        let player_palette = sgb_screen_palette([0x1111, 0x2222, 0x3333, 0x4444]);
        assert!(host.set_player_palette_override(player_palette));
        assert!(host.snapshot().video.player_palette_override.active);
        assert_eq!(
            host.snapshot().video.map_lcd_shade_to_rgb555(2).raw(),
            0x3333
        );

        write_joyp_packet(&mut host, sgb_pal01_packet());
        assert!(
            host.snapshot().video.player_palette_override.active,
            "with PAL_PRI disabled, application palette commands update host state but do not override the player's selected palette"
        );
        assert_eq!(
            host.snapshot().video.map_lcd_shade_to_rgb555(1).raw(),
            0x2222
        );

        let mut pal_pri = sgb_command_packet(SGB_COMMAND_PAL_PRI, 1);
        pal_pri[1] = 1;
        write_joyp_packet(&mut host, pal_pri);
        assert!(host.snapshot().video.system_palettes.pal_pri_enabled);
        assert!(
            host.snapshot().video.player_palette_override.active,
            "PAL_PRI itself only changes the priority policy; a later application palette command switches back"
        );

        write_joyp_packet(&mut host, sgb_pal01_packet());
        let snapshot = host.snapshot();
        assert!(!snapshot.video.player_palette_override.active);
        assert_eq!(
            snapshot.video.player_palette_override.pal_pri_release_count,
            1
        );
        assert_eq!(
            snapshot.video.map_lcd_shade_to_rgb555(1).raw(),
            0x03E0,
            "once PAL_PRI gives priority back to the application, visible output uses the game palette state"
        );
    }

    #[test]
    fn pal_pri_does_not_switch_on_transfer_loads_but_switches_on_attribute_commands() {
        let mut host = accepted_sgb_host();
        let player_palette = sgb_screen_palette([0x1111, 0x2222, 0x3333, 0x4444]);
        assert!(host.set_player_palette_override(player_palette));

        let mut pal_pri = sgb_command_packet(SGB_COMMAND_PAL_PRI, 1);
        pal_pri[1] = 1;
        write_joyp_packet(&mut host, pal_pri);

        write_joyp_packet(&mut host, sgb_pal_trn_packet());
        assert!(
            host.snapshot().video.player_palette_override.active,
            "PAL_TRN loads backing system palette memory and must not by itself switch away from the player palette"
        );

        let mut attr_set = sgb_command_packet(SGB_COMMAND_ATTR_SET, 1);
        attr_set[1] = 0;
        write_joyp_packet(&mut host, attr_set);
        assert!(!host.snapshot().video.player_palette_override.active);
        assert_eq!(
            host.snapshot()
                .video
                .player_palette_override
                .pal_pri_release_count,
            1
        );
    }

    #[test]
    fn attr_trn_attr_set_and_pal_set_apply_attribute_files() {
        let mut host = accepted_sgb_host();
        let mut transfer = [0; SGB_VRAM_TRANSFER_BYTES];
        write_atf_cell(&mut transfer, 2, 0, 0, 1);
        write_atf_cell(&mut transfer, 2, 1, 0, 2);
        write_atf_cell(&mut transfer, 2, 0, 1, 3);

        write_joyp_packet(&mut host, sgb_attr_trn_packet());
        assert_eq!(
            host.capture_pending_vram_transfer(&transfer)
                .expect("ATTR_TRN should capture ATF data"),
            Some(SgbVramTransferTarget::Attr)
        );

        write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::Freeze));
        let mut attr_set = sgb_command_packet(SGB_COMMAND_ATTR_SET, 1);
        attr_set[1] = 0x40 | 2;
        write_joyp_packet(&mut host, attr_set);

        let snapshot = host.snapshot();
        assert_eq!(snapshot.video.mask, SgbScreenMask::Cancel);
        assert_eq!(snapshot.video.attributes.last_atf_index, Some(2));
        assert_eq!(snapshot.video.attributes.attr_trn_count, 1);
        assert_eq!(snapshot.video.attributes.attr_set_count, 1);
        assert_eq!(snapshot.video.attributes.map.palette_index(0, 0), 1);
        assert_eq!(snapshot.video.attributes.map.palette_index(1, 0), 2);
        assert_eq!(snapshot.video.attributes.map.palette_index(0, 1), 3);

        let mut pal_set = sgb_command_packet(SGB_COMMAND_PAL_SET, 1);
        pal_set[9] = 0x80 | 2;
        write_joyp_packet(&mut host, pal_set);
        assert_eq!(host.snapshot().video.attributes.attr_set_count, 2);
        assert_eq!(host.snapshot().video.attributes.map.palette_index(0, 0), 1);
    }

    #[test]
    fn chr_trn_captures_vram_transfer_into_border_tile_blocks() {
        let mut host = accepted_sgb_host();
        let first_block = solid_tile_color_1_transfer();
        write_joyp_packet(&mut host, sgb_chr_trn_packet(0));
        assert!(matches!(
            host.snapshot().video.vram_transfer.pending,
            Some(SgbPendingVramTransfer {
                target: SgbVramTransferTarget::Chr(SgbChrTransferSelection {
                    tile_block: 0,
                    tile_type: SgbChrTransferTileType::Bg,
                }),
                ..
            })
        ));

        assert_eq!(
            host.capture_pending_vram_transfer(&first_block)
                .expect("CHR_TRN should capture the pending VRAM payload"),
            Some(SgbVramTransferTarget::Chr(SgbChrTransferSelection {
                tile_block: 0,
                tile_type: SgbChrTransferTileType::Bg,
            }))
        );
        let snapshot = host.snapshot();
        assert!(snapshot.video.border.chr0_loaded);
        assert!(!snapshot.video.border.chr1_loaded);
        assert_eq!(snapshot.video.border.chr_transfer_count, 1);
        assert_eq!(snapshot.video.border.tile_data.bytes[0], 0xFF);
        assert_eq!(
            snapshot
                .video
                .vram_transfer
                .last_completed
                .as_ref()
                .expect("last transfer should be retained for save-state observability")
                .payload
                .bytes[0],
            0xFF
        );

        let mut second_block = [0; SGB_VRAM_TRANSFER_BYTES];
        second_block[0] = 0x80;
        write_joyp_packet(&mut host, sgb_chr_trn_packet(1));
        host.capture_pending_vram_transfer(&second_block)
            .expect("second CHR_TRN should replace the high tile block");
        let snapshot = host.snapshot();
        assert!(snapshot.video.border.chr0_loaded);
        assert!(snapshot.video.border.chr1_loaded);
        assert_eq!(snapshot.video.border.chr_transfer_count, 2);
        assert_eq!(
            snapshot.video.border.tile_data.bytes[SGB_VRAM_TRANSFER_BYTES],
            0x80
        );
    }

    #[test]
    fn pct_trn_decodes_border_tilemap_and_border_palettes() {
        let mut host = accepted_sgb_host();
        let mut pct = [0; SGB_VRAM_TRANSFER_BYTES];
        write_border_map_entry(&mut pct, 0, (4 << 10) | 3);
        write_border_map_entry(
            &mut pct,
            SGB_BORDER_TILEMAP_WIDTH * SGB_BORDER_TILEMAP_VISIBLE_HEIGHT,
            0x8000 | (6 << 10) | 4,
        );
        write_border_palette_color(&mut pct, 0, 1, 0x03E0);
        write_border_palette_color(&mut pct, 1, 2, 0x7C00);
        write_border_palette_color(&mut pct, 2, 3, 0x801F);

        write_joyp_packet(&mut host, sgb_pct_trn_packet());
        host.capture_pending_vram_transfer(&pct)
            .expect("PCT_TRN should capture the pending VRAM payload");

        let snapshot = host.snapshot();
        assert!(snapshot.video.border_loaded);
        assert!(snapshot.video.border.pct_loaded);
        assert_eq!(snapshot.video.border.pct_transfer_count, 1);
        assert_eq!(snapshot.video.border.tile_map.entries[0].raw, (4 << 10) | 3);
        assert_eq!(
            snapshot.video.border.tile_map.entries
                [SGB_BORDER_TILEMAP_WIDTH * SGB_BORDER_TILEMAP_VISIBLE_HEIGHT]
                .raw,
            0x8000 | (6 << 10) | 4
        );
        assert_eq!(snapshot.video.border.palettes[0].colors[1].raw(), 0x03E0);
        assert_eq!(snapshot.video.border.palettes[1].colors[2].raw(), 0x7C00);
        assert_eq!(
            snapshot.video.border.palettes[2].colors[3].raw(),
            0x001F,
            "border RGB555 palette data masks off the ignored high bit"
        );
    }

    #[test]
    fn sgb_frame_composition_combines_border_and_lcd_window() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_pal01_packet());
        write_joyp_packet(&mut host, sgb_chr_trn_packet(0));
        host.capture_pending_vram_transfer(&solid_tile_color_1_transfer())
            .expect("CHR_TRN should load the visible border tile");

        let mut pct = [0; SGB_VRAM_TRANSFER_BYTES];
        for y in 0..SGB_BORDER_TILEMAP_VISIBLE_HEIGHT {
            for x in 0..SGB_BORDER_TILEMAP_WIDTH {
                write_border_map_entry(&mut pct, y * SGB_BORDER_TILEMAP_WIDTH + x, 4 << 10);
            }
        }
        for y in 5..23 {
            for x in 6..26 {
                write_border_map_entry(&mut pct, y * SGB_BORDER_TILEMAP_WIDTH + x, (4 << 10) | 1);
            }
        }
        write_border_palette_color(&mut pct, 0, 0, 0x0000);
        write_border_palette_color(&mut pct, 0, 1, 0x03E0);
        write_joyp_packet(&mut host, sgb_pct_trn_packet());
        host.capture_pending_vram_transfer(&pct)
            .expect("PCT_TRN should load the border map and palettes");

        let dmg_framebuffer = vec![0; SGB_LCD_PIXELS];
        let frame = host
            .compose_frame_rgb555(&dmg_framebuffer)
            .expect("SGB host should compose the full 256x224 host frame");
        assert_eq!(frame.len(), SGB_FRAME_PIXELS);
        assert_eq!(frame[0], 0x03E0);
        let lcd_origin = SGB_LCD_FRAME_ORIGIN_Y * SGB_FRAME_WIDTH + SGB_LCD_FRAME_ORIGIN_X;
        assert_eq!(
            frame[lcd_origin], 0x001F,
            "color-index 0 border pixels inside the GB window let the SGB LCD image show through"
        );
    }

    #[test]
    fn mask_en_blanks_and_freezes_the_host_lcd_image() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_pal01_packet());
        let current_lcd = vec![0; SGB_LCD_PIXELS];
        let mut changed_lcd = vec![3; SGB_LCD_PIXELS];

        write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::Freeze));
        assert!(host.snapshot().video.freeze_capture_pending);
        host.capture_pending_lcd_freeze(&current_lcd)
            .expect("freeze should capture the current host LCD image");
        assert!(!host.snapshot().video.freeze_capture_pending);
        assert_eq!(
            host.compose_lcd_rgb555(&changed_lcd)
                .expect("frozen SGB LCD should still compose")[0],
            0x001F
        );

        write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::BlankBlack));
        assert_eq!(
            host.compose_lcd_rgb555(&changed_lcd)
                .expect("blank-black SGB LCD should compose")[0],
            0x0000
        );

        write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::BlankColor0));
        changed_lcd[0] = 3;
        assert_eq!(
            host.compose_lcd_rgb555(&changed_lcd)
                .expect("blank-color0 SGB LCD should compose")[0],
            0x001F
        );

        write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::Cancel));
        assert_eq!(
            host.compose_lcd_rgb555(&changed_lcd)
                .expect("unmasked SGB LCD should compose")[0],
            0x4210
        );
    }

    #[test]
    fn joyp_transport_rejects_complete_packet_until_header_unlocks_sgb() {
        let mut host = SgbHost::new(HostPlatform::Sgb);
        write_joyp_packet(&mut host, sgb_command_packet(0x11, 1));

        let snapshot = host.snapshot();
        assert_eq!(
            snapshot.packet_transport.last_trace.status,
            SgbPacketTraceStatus::RejectedByHeader
        );
        assert_eq!(snapshot.command.rejected_packet_count, 1);
        assert_eq!(snapshot.command.accepted_command_count, 0);
    }

    #[test]
    fn joyp_transport_records_invalid_packet_count_and_stop_bit() {
        let mut invalid_count = accepted_sgb_host();
        write_joyp_packet(&mut invalid_count, sgb_command_packet(0x11, 0));
        assert_eq!(
            invalid_count.snapshot().packet_transport.last_trace.status,
            SgbPacketTraceStatus::InvalidPacketLength
        );
        assert_eq!(invalid_count.snapshot().command.invalid_packet_count, 1);

        let mut invalid_stop = accepted_sgb_host();
        write_joyp_start(&mut invalid_stop);
        for byte in sgb_command_packet(0x11, 1) {
            for bit_index in 0..8 {
                write_joyp_data_bit(&mut invalid_stop, (byte >> bit_index) & 0x01);
            }
        }
        write_joyp_data_bit(&mut invalid_stop, 1);
        assert_eq!(
            invalid_stop.snapshot().packet_transport.last_trace.status,
            SgbPacketTraceStatus::InvalidStopBit
        );
        assert_eq!(invalid_stop.snapshot().command.invalid_packet_count, 1);
    }

    #[test]
    fn joyp_transport_records_incomplete_reset_and_orphan_data_pulse() {
        let mut incomplete = accepted_sgb_host();
        write_joyp_start(&mut incomplete);
        write_joyp_data_bit(&mut incomplete, 1);
        write_joyp_start(&mut incomplete);
        assert_eq!(
            incomplete.snapshot().packet_transport.last_trace.status,
            SgbPacketTraceStatus::IncompleteReset
        );
        assert_eq!(incomplete.snapshot().command.invalid_packet_count, 1);

        let mut orphan = accepted_sgb_host();
        write_joyp_data_bit(&mut orphan, 1);
        assert_eq!(
            orphan.snapshot().packet_transport.last_trace.status,
            SgbPacketTraceStatus::OrphanDataPulse
        );
        assert_eq!(orphan.snapshot().command.invalid_packet_count, 1);
    }

    #[test]
    fn joyp_transport_ignores_handheld_hosts() {
        let mut host = SgbHost::new(HostPlatform::Handheld);
        write_joyp_packet(&mut host, sgb_command_packet(0x11, 1));

        let snapshot = host.snapshot();
        assert_eq!(snapshot.packet_transport.reset_pulse_count, 0);
        assert_eq!(snapshot.packet_transport.data_pulse_count, 0);
        assert_eq!(snapshot.command.accepted_command_count, 0);
        assert_eq!(
            snapshot.packet_transport.last_trace.status,
            SgbPacketTraceStatus::None
        );
    }

    #[test]
    fn save_state_restores_partial_packet_transport() {
        let mut host = accepted_sgb_host();
        let packet = sgb_command_packet(0x11, 1);
        write_joyp_start(&mut host);
        for bit_index in 0..32 {
            let byte = packet[bit_index / 8];
            write_joyp_data_bit(&mut host, (byte >> (bit_index % 8)) & 0x01);
        }

        let saved = host.capture_save_state();
        let mut restored = SgbHost::new(HostPlatform::Sgb);
        restored.restore_save_state(&saved);

        for bit_index in 32..128 {
            let byte = packet[bit_index / 8];
            write_joyp_data_bit(&mut restored, (byte >> (bit_index % 8)) & 0x01);
        }
        write_joyp_data_bit(&mut restored, 0);

        let snapshot = restored.snapshot();
        assert_eq!(snapshot.packet_transport.packet_bits_buffered, 128);
        assert_eq!(
            snapshot.packet_transport.last_trace.status,
            SgbPacketTraceStatus::Complete
        );
        assert_eq!(snapshot.command.last_command_id, Some(0x11));
        assert_eq!(snapshot.command.accepted_command_count, 1);
    }

    #[test]
    fn save_state_restores_sgb_palette_state() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_pal01_packet());

        let saved = host.capture_save_state();
        let mut restored = SgbHost::new(HostPlatform::Sgb);
        restored.restore_save_state(&saved);

        assert_eq!(
            restored.snapshot().video.palette_state,
            host.snapshot().video.palette_state
        );
        assert_eq!(restored.snapshot().video.palette_command_count, 1);
        assert_eq!(
            restored.snapshot().video.map_lcd_shade_to_rgb555(3).raw(),
            0x4210
        );
    }

    #[test]
    fn save_state_restores_sgb_player_palette_override_state() {
        let mut host = accepted_sgb_host();
        let player_palette = sgb_screen_palette([0x1111, 0x2222, 0x3333, 0x4444]);
        assert!(host.set_player_palette_override(player_palette));

        let saved = host.capture_save_state();
        let mut restored = SgbHost::new(HostPlatform::Sgb);
        restored.restore_save_state(&saved);

        assert_eq!(
            restored.snapshot().video.player_palette_override,
            host.snapshot().video.player_palette_override
        );
        assert_eq!(
            restored.snapshot().video.map_lcd_shade_to_rgb555(3).raw(),
            0x4444
        );
    }

    #[test]
    fn save_state_restores_sgb_boot_title_palette_seed() {
        let mut host = SgbHost::new(HostPlatform::Sgb);
        let header = test_header_with_title(
            b"MARIOLAND2",
            SgbFlag::None,
            SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED,
        );
        host.apply_cartridge_header(Some(&header));

        let saved = host.capture_save_state();
        let mut restored = SgbHost::new(HostPlatform::Sgb);
        restored.restore_save_state(&saved);

        assert_eq!(
            restored.snapshot().video.palette_state,
            host.snapshot().video.palette_state
        );
        assert_eq!(
            restored.snapshot().video.map_lcd_shade_to_rgb555(0).raw(),
            0x5FFE
        );
        assert_eq!(
            restored.snapshot().video.map_lcd_shade_to_rgb555(3).raw(),
            0x0000
        );
    }

    #[test]
    fn save_state_restores_sgb_transfer_border_and_mask_state() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_pal01_packet());
        write_joyp_packet(&mut host, sgb_chr_trn_packet(0));
        host.capture_pending_vram_transfer(&solid_tile_color_1_transfer())
            .expect("CHR_TRN should load border tile data before save");
        let mut pct = [0; SGB_VRAM_TRANSFER_BYTES];
        write_border_map_entry(&mut pct, 0, 4 << 10);
        write_border_palette_color(&mut pct, 0, 1, 0x03E0);
        write_joyp_packet(&mut host, sgb_pct_trn_packet());
        host.capture_pending_vram_transfer(&pct)
            .expect("PCT_TRN should load border map before save");
        write_joyp_packet(&mut host, sgb_mask_packet(SgbScreenMask::Freeze));
        host.capture_pending_lcd_freeze(&vec![0; SGB_LCD_PIXELS])
            .expect("freeze should persist its captured LCD image");

        let saved = host.capture_save_state();
        let mut restored = SgbHost::new(HostPlatform::Sgb);
        restored.restore_save_state(&saved);

        assert_eq!(
            restored
                .snapshot()
                .video
                .vram_transfer
                .completed_transfer_count,
            2
        );
        assert!(restored.snapshot().video.border_loaded);
        assert_eq!(restored.snapshot().video.border.tile_data.bytes[0], 0xFF);
        assert_eq!(
            restored.snapshot().video.border.palettes[0].colors[1].raw(),
            0x03E0
        );
        assert_eq!(restored.snapshot().video.mask, SgbScreenMask::Freeze);
        assert!(restored.snapshot().video.frozen_lcd.is_some());
        assert_eq!(
            restored
                .compose_lcd_rgb555(&vec![3; SGB_LCD_PIXELS])
                .expect("restored frozen LCD should compose")[0],
            0x001F
        );
    }

    #[test]
    fn save_state_restores_sgb_attribute_and_system_palette_state() {
        let mut host = accepted_sgb_host();
        let mut palettes = [0; SGB_VRAM_TRANSFER_BYTES];
        write_system_palette_color(&mut palettes, 3, 0, 0x001F);
        write_system_palette_color(&mut palettes, 3, 1, 0x03E0);
        write_joyp_packet(&mut host, sgb_pal_trn_packet());
        host.capture_pending_vram_transfer(&palettes)
            .expect("PAL_TRN should load system palette memory before save");
        let mut attributes = [0; SGB_VRAM_TRANSFER_BYTES];
        write_atf_cell(&mut attributes, 4, 0, 0, 1);
        write_atf_cell(&mut attributes, 4, 1, 0, 2);
        write_joyp_packet(&mut host, sgb_attr_trn_packet());
        host.capture_pending_vram_transfer(&attributes)
            .expect("ATTR_TRN should load ATF memory before save");

        let mut pal_set = sgb_command_packet(SGB_COMMAND_PAL_SET, 1);
        for palette_index in 0..SGB_SCREEN_PALETTE_COUNT {
            let [low, high] = 3u16.to_le_bytes();
            pal_set[1 + palette_index * 2] = low;
            pal_set[2 + palette_index * 2] = high;
        }
        pal_set[9] = 0x80 | 4;
        write_joyp_packet(&mut host, pal_set);

        let saved = host.capture_save_state();
        let mut restored = SgbHost::new(HostPlatform::Sgb);
        restored.restore_save_state(&saved);

        let snapshot = restored.snapshot();
        assert!(snapshot.video.system_palettes.loaded);
        assert!(snapshot.video.attributes.files.loaded);
        assert_eq!(snapshot.video.system_palettes.last_pal_set_ids, [3; 4]);
        assert_eq!(snapshot.video.attributes.last_atf_index, Some(4));
        assert_eq!(snapshot.video.attributes.map.palette_index(0, 0), 1);
        assert_eq!(snapshot.video.attributes.map.palette_index(1, 0), 2);
        assert_eq!(
            snapshot.video.palette_state.palette(0).colors[1].raw(),
            0x03E0
        );
    }

    #[test]
    fn save_state_restores_sgb_multiplayer_state_and_input_slots() {
        let mut host = accepted_sgb_host();
        write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
        cycle_sgb_player(&mut host);
        assert!(host.set_player_button_pressed(2, JoypadButton::A, true));
        assert!(host.set_player_button_pressed(4, JoypadButton::Start, true));

        let saved = host.capture_save_state();
        let mut restored = SgbHost::new(HostPlatform::Sgb);
        restored.restore_save_state(&saved);

        let snapshot = restored.snapshot();
        assert_eq!(snapshot.multiplayer.player_count, 4);
        assert_eq!(snapshot.multiplayer.selected_player, 2);
        assert_eq!(snapshot.multiplayer.player_pressed_masks[1], 0x10);
        assert_eq!(snapshot.multiplayer.player_pressed_masks[3], 0x80);
        assert_eq!(restored.selected_player_pressed_mask(), 0x10);
        assert_eq!(restored.joyp_read_value(0xFF), 0xFE);
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
