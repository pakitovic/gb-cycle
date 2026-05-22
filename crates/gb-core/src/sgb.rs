use crate::cartridge::{CartridgeHeader, SgbFlag};
use crate::model::{HostPlatform, StartupMode};

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
const SGB_COMMAND_PAL01: u8 = 0x00;
const SGB_COMMAND_PAL23: u8 = 0x01;
const SGB_COMMAND_PAL03: u8 = 0x02;
const SGB_COMMAND_PAL12: u8 = 0x03;
const SGB_COMMAND_CHR_TRN: u8 = 0x13;
const SGB_COMMAND_PCT_TRN: u8 = 0x14;
const SGB_COMMAND_MASK_EN: u8 = 0x17;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbPendingVramTransfer {
    pub command_id: u8,
    pub target: SgbVramTransferTarget,
    pub frame_starts_until_capture: u8,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbPaletteState {
    pub screen_palettes: [SgbScreenPalette; SGB_SCREEN_PALETTE_COUNT],
    pub base_palette_index: u8,
}

impl SgbPaletteState {
    pub const fn palette(self, palette_index: u8) -> SgbScreenPalette {
        self.screen_palettes[(palette_index & 0x03) as usize]
    }

    pub const fn map_lcd_shade(self, shade: u8) -> SgbRgb555Color {
        self.palette(self.base_palette_index).color(shade)
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
        debug_assert!(profile.is_none_or(|profile| profile.host_platform() == host_platform));
        let active = host_platform.is_sgb();
        let profile = active.then_some(profile).flatten();
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

    pub(crate) fn apply_cartridge_header(&mut self, header: Option<&CartridgeHeader>) {
        self.startup.apply_cartridge_header(self.status(), header);
    }

    pub(crate) fn observe_joyp_write(&mut self, value: u8) {
        if !self.host_platform.is_sgb() {
            return;
        }

        let line_state = SgbJoypLineState::from_joyp_value(value);
        let previous_line_state = self.packet_transport.last_joyp_line_state;
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
        if packet_count != 1 {
            return;
        }

        if direct_palette_command_pair(command_id).is_some() {
            self.video
                .apply_direct_palette_command(command_id, &self.command.packet_buffer[0]);
        } else {
            match command_id {
                SGB_COMMAND_CHR_TRN => self
                    .video
                    .request_chr_transfer(command_id, &self.command.packet_buffer[0]),
                SGB_COMMAND_PCT_TRN => self.video.request_pct_transfer(command_id),
                SGB_COMMAND_MASK_EN => self
                    .video
                    .apply_mask_command(&self.command.packet_buffer[0]),
                _ => {}
            }
        }
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
        for (output_pixel, &shade) in frozen.pixels.iter_mut().zip(dmg_framebuffer.iter()) {
            *output_pixel = self.video.palette_state.map_lcd_shade(shade).raw();
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
        self.video.capture_pending_vram_transfer(vram_bytes)
    }

    pub(crate) fn advance_frame_start(
        &mut self,
        vram_bytes: &[u8],
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        if !self.host_platform.is_sgb() {
            return Err(SgbVramTransferError::DisabledHost);
        }
        self.video.advance_frame_start(vram_bytes)
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
        }
    }
}

impl Default for SgbMultiplayerState {
    fn default() -> Self {
        Self::default_for_active_host(false)
    }
}

impl SgbVideoState {
    fn default_for_active_host(active: bool) -> Self {
        Self {
            border_loaded: false,
            colorization_active: active,
            palette_state: SgbPaletteState::default(),
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

    pub const fn map_lcd_shade_to_rgb555(&self, shade: u8) -> SgbRgb555Color {
        self.palette_state.map_lcd_shade(shade)
    }

    pub fn lcd_pixel_for_shade(&self, shade: u8) -> SgbRgb555Color {
        match self.mask {
            SgbScreenMask::Cancel => self.palette_state.map_lcd_shade(shade),
            SgbScreenMask::Freeze => self.palette_state.map_lcd_shade(shade),
            SgbScreenMask::BlankBlack => SGB_RGB555_BLACK,
            SgbScreenMask::BlankColor0 => self.palette_state.map_lcd_shade(0),
        }
    }

    fn lcd_pixel_for_framebuffer_index(
        &self,
        framebuffer_index: usize,
        shade: u8,
    ) -> SgbRgb555Color {
        match self.mask {
            SgbScreenMask::Cancel => self.palette_state.map_lcd_shade(shade),
            SgbScreenMask::Freeze => self
                .frozen_lcd
                .as_ref()
                .and_then(|frame| frame.pixels.get(framebuffer_index).copied())
                .map(SgbRgb555Color::new)
                .unwrap_or_else(|| self.palette_state.map_lcd_shade(shade)),
            SgbScreenMask::BlankBlack => SGB_RGB555_BLACK,
            SgbScreenMask::BlankColor0 => self.palette_state.map_lcd_shade(0),
        }
    }

    fn apply_direct_palette_command(&mut self, command_id: u8, bytes: &[u8; SGB_PACKET_BYTES]) {
        self.palette_state
            .apply_direct_palette_command(command_id, bytes);
        self.colorization_active = true;
        self.last_palette_command_id = Some(command_id);
        self.palette_command_count = self.palette_command_count.saturating_add(1);
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
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        let Some(mut pending) = self.vram_transfer.pending else {
            return Ok(None);
        };
        if pending.frame_starts_until_capture > 1 {
            pending.frame_starts_until_capture -= 1;
            self.vram_transfer.pending = Some(pending);
            return Ok(None);
        }
        self.capture_pending_vram_transfer(vram_bytes)
    }

    fn capture_pending_vram_transfer(
        &mut self,
        vram_bytes: &[u8],
    ) -> Result<Option<SgbVramTransferTarget>, SgbVramTransferError> {
        let Some(pending) = self.vram_transfer.pending.take() else {
            return Err(SgbVramTransferError::NoPendingTransfer);
        };
        let payload = SgbVramTransferBuffer::from_source_bytes(vram_bytes)?;
        match pending.target {
            SgbVramTransferTarget::Chr(selection) => {
                self.border.apply_chr_transfer(selection, &payload);
            }
            SgbVramTransferTarget::Pct => {
                self.border.apply_pct_transfer(&payload);
                self.border_loaded = true;
            }
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

    fn test_header(sgb_flag: SgbFlag, old_licensee_code: u8) -> CartridgeHeader {
        CartridgeHeader {
            entry_point: [0; 4],
            nintendo_logo: [0; 48],
            title_bytes: [0; 16],
            raw_title_suffix_or_manufacturer_code: [0; 4],
            title: "SGBTEST".to_string(),
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

    fn accepted_sgb_host() -> SgbHost {
        let mut host = SgbHost::new(HostPlatform::Sgb);
        let header = test_header(SgbFlag::Supported, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
        host.apply_cartridge_header(Some(&header));
        host
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
