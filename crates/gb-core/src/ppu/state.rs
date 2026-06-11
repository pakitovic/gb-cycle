use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum PpuPaletteRegister {
    Bgp,
    Obp0,
    Obp1,
}

impl PpuPaletteRegister {
    pub(super) const fn for_obj_palette(palette_obp1: bool) -> Self {
        if palette_obp1 { Self::Obp1 } else { Self::Obp0 }
    }

    pub(super) const fn affects_obj_palette(self, palette_obp1: bool) -> bool {
        matches!(
            (self, palette_obp1),
            (Self::Obp0, false) | (Self::Obp1, true)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum PpuRegister {
    Lcdc,
    Stat,
    Scy,
    Scx,
    Ly,
    Lyc,
    Bgp,
    Obp0,
    Obp1,
    Wy,
    Wx,
    Bcps,
    Bcpd,
    Ocps,
    Ocpd,
    Opri,
}

impl PpuRegister {
    pub(super) const fn from_address(address: u16) -> Option<Self> {
        match address {
            0xFF40 => Some(Self::Lcdc),
            0xFF41 => Some(Self::Stat),
            0xFF42 => Some(Self::Scy),
            0xFF43 => Some(Self::Scx),
            0xFF44 => Some(Self::Ly),
            0xFF45 => Some(Self::Lyc),
            0xFF47 => Some(Self::Bgp),
            0xFF48 => Some(Self::Obp0),
            0xFF49 => Some(Self::Obp1),
            0xFF4A => Some(Self::Wy),
            0xFF4B => Some(Self::Wx),
            0xFF68 => Some(Self::Bcps),
            0xFF69 => Some(Self::Bcpd),
            0xFF6A => Some(Self::Ocps),
            0xFF6B => Some(Self::Ocpd),
            0xFF6C => Some(Self::Opri),
            _ => None,
        }
    }

    pub(super) const fn palette_register(self) -> Option<PpuPaletteRegister> {
        match self {
            Self::Bgp => Some(PpuPaletteRegister::Bgp),
            Self::Obp0 => Some(PpuPaletteRegister::Obp0),
            Self::Obp1 => Some(PpuPaletteRegister::Obp1),
            Self::Lcdc
            | Self::Stat
            | Self::Scy
            | Self::Scx
            | Self::Ly
            | Self::Lyc
            | Self::Wy
            | Self::Wx
            | Self::Bcps
            | Self::Bcpd
            | Self::Ocps
            | Self::Ocpd
            | Self::Opri => None,
        }
    }

    pub(super) const fn cgb_palette_register(self) -> Option<CgbPaletteRegister> {
        match self {
            Self::Bcps => Some(CgbPaletteRegister::BgIndex),
            Self::Bcpd => Some(CgbPaletteRegister::BgData),
            Self::Ocps => Some(CgbPaletteRegister::ObjIndex),
            Self::Ocpd => Some(CgbPaletteRegister::ObjData),
            Self::Lcdc
            | Self::Stat
            | Self::Scy
            | Self::Scx
            | Self::Ly
            | Self::Lyc
            | Self::Bgp
            | Self::Obp0
            | Self::Obp1
            | Self::Wy
            | Self::Wx
            | Self::Opri => None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(super) enum CgbObjPriorityMode {
    #[default]
    CgbOamOrder,
    DmgXCoordinate,
}

impl CgbObjPriorityMode {
    pub(super) const fn for_model_and_mode(
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
    ) -> Self {
        if console_model.is_cgb_family() && matches!(operating_mode, OperatingMode::Cgb) {
            Self::CgbOamOrder
        } else {
            Self::DmgXCoordinate
        }
    }

    pub(super) const fn opri_bit(self) -> u8 {
        match self {
            Self::CgbOamOrder => 0,
            Self::DmgXCoordinate => 1,
        }
    }
}

const CGB_PALETTE_RAM_BYTES: usize = 64;
const CGB_PALETTE_INDEX_ADDRESS_MASK: u8 = 0x3F;
const CGB_PALETTE_INDEX_FORCED_READ_BITS: u8 = 0x40;
const CGB_PALETTE_INDEX_AUTO_INCREMENT_BIT: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum CgbPaletteKind {
    Background,
    Object,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum CgbPaletteRegister {
    BgIndex,
    BgData,
    ObjIndex,
    ObjData,
}

const CGB_NATIVE_BOOT_BG_PALETTE0_BYTES: [u8; 8] = [0xFF, 0xFF, 0xFF, 0x7F, 0x00, 0x00, 0x00, 0x00];

impl CgbPaletteRegister {
    pub(super) const fn kind(self) -> CgbPaletteKind {
        match self {
            Self::BgIndex | Self::BgData => CgbPaletteKind::Background,
            Self::ObjIndex | Self::ObjData => CgbPaletteKind::Object,
        }
    }

    pub(super) const fn is_data(self) -> bool {
        matches!(self, Self::BgData | Self::ObjData)
    }

    pub(super) const fn is_index(self) -> bool {
        matches!(self, Self::BgIndex | Self::ObjIndex)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct CgbPalettePort {
    index: u8,
    #[serde(with = "serde_big_array::BigArray")]
    data: [u8; CGB_PALETTE_RAM_BYTES],
}

impl CgbPalettePort {
    pub(super) const fn new() -> Self {
        Self {
            index: 0,
            data: [0; CGB_PALETTE_RAM_BYTES],
        }
    }

    pub(super) const fn read_index(&self) -> u8 {
        self.index | CGB_PALETTE_INDEX_FORCED_READ_BITS
    }

    pub(super) fn write_index(&mut self, value: u8) {
        self.index =
            value & (CGB_PALETTE_INDEX_AUTO_INCREMENT_BIT | CGB_PALETTE_INDEX_ADDRESS_MASK);
    }

    pub(super) fn read_data(&self, blocked: bool) -> u8 {
        if blocked {
            return 0xFF;
        }

        self.data[self.address()]
    }

    pub(super) fn write_data(&mut self, value: u8, blocked: bool) {
        if !blocked {
            let address = self.address();
            self.data[address] = value;
        }

        if self.index & CGB_PALETTE_INDEX_AUTO_INCREMENT_BIT != 0 {
            let next_address = self.index.wrapping_add(1) & CGB_PALETTE_INDEX_ADDRESS_MASK;
            self.index = CGB_PALETTE_INDEX_AUTO_INCREMENT_BIT | next_address;
        }
    }

    fn address(&self) -> usize {
        usize::from(self.index & CGB_PALETTE_INDEX_ADDRESS_MASK)
    }

    pub(super) fn rgb555(&self, palette_index: u8, color_index: u8) -> u16 {
        let address = usize::from((palette_index & 0x07) * 8 + (color_index & 0x03) * 2);
        u16::from_le_bytes([self.data[address], self.data[address + 1]]) & 0x7FFF
    }

    pub(super) fn write_palette_bytes(&mut self, palette_index: u8, bytes: [u8; 8]) {
        let address = usize::from((palette_index & 0x07) * 8);
        self.data[address..address + 8].copy_from_slice(&bytes);
    }
}

impl Default for CgbPalettePort {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct CgbPaletteState {
    background: CgbPalettePort,
    object: CgbPalettePort,
}

impl CgbPaletteState {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) const fn port(&self, kind: CgbPaletteKind) -> &CgbPalettePort {
        match kind {
            CgbPaletteKind::Background => &self.background,
            CgbPaletteKind::Object => &self.object,
        }
    }

    pub(super) fn port_mut(&mut self, kind: CgbPaletteKind) -> &mut CgbPalettePort {
        match kind {
            CgbPaletteKind::Background => &mut self.background,
            CgbPaletteKind::Object => &mut self.object,
        }
    }

    pub(super) fn apply_cgb_compatibility_palette_seed(
        &mut self,
        seed: CgbCompatibilityPaletteSeed,
    ) {
        self.background.write_palette_bytes(0, seed.bg_palette0);
        self.object.write_palette_bytes(0, seed.obj_palette0);
        self.object.write_palette_bytes(1, seed.obj_palette1);
        self.background.write_index(0xC8);
        self.object.write_index(0xD0);
    }

    pub(super) fn apply_cgb_native_boot_palette_seed(&mut self) {
        self.background
            .write_palette_bytes(0, CGB_NATIVE_BOOT_BG_PALETTE0_BYTES);
        self.background.write_index(0x80);
        self.object.write_index(0x81);
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(super) struct CgbBgTileAttributes {
    raw: u8,
}

impl CgbBgTileAttributes {
    pub(super) const fn new(raw: u8) -> Self {
        Self { raw }
    }

    #[allow(dead_code)]
    pub(super) const fn raw(self) -> u8 {
        self.raw
    }

    #[allow(dead_code)]
    pub(super) const fn palette_index(self) -> u8 {
        self.raw & CGB_BG_ATTR_PALETTE_MASK
    }

    pub(super) const fn tile_vram_bank(self) -> u8 {
        if self.raw & CGB_BG_ATTR_VRAM_BANK_BIT != 0 {
            1
        } else {
            0
        }
    }

    #[allow(dead_code)]
    pub(super) const fn ignored_bit4(self) -> bool {
        self.raw & CGB_BG_ATTR_IGNORED_BIT != 0
    }

    pub(super) const fn horizontal_flip(self) -> bool {
        self.raw & CGB_BG_ATTR_X_FLIP_BIT != 0
    }

    pub(super) const fn vertical_flip(self) -> bool {
        self.raw & CGB_BG_ATTR_Y_FLIP_BIT != 0
    }

    #[allow(dead_code)]
    pub(super) const fn bg_priority(self) -> bool {
        self.raw & CGB_BG_ATTR_PRIORITY_BIT != 0
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(super) struct CgbObjAttributes {
    raw: u8,
}

impl CgbObjAttributes {
    pub(super) const fn new(raw: u8) -> Self {
        Self { raw }
    }

    #[allow(dead_code)]
    pub(super) const fn raw(self) -> u8 {
        self.raw
    }

    #[allow(dead_code)]
    pub(super) const fn palette_index(self) -> u8 {
        self.raw & CGB_OBJ_ATTR_PALETTE_MASK
    }

    pub(super) const fn tile_vram_bank(self) -> u8 {
        if self.raw & CGB_OBJ_ATTR_VRAM_BANK_BIT != 0 {
            1
        } else {
            0
        }
    }

    #[allow(dead_code)]
    pub(super) const fn dmg_palette_obp1(self) -> bool {
        self.raw & CGB_OBJ_ATTR_DMG_PALETTE_BIT != 0
    }

    #[allow(dead_code)]
    pub(super) const fn horizontal_flip(self) -> bool {
        self.raw & CGB_OBJ_ATTR_X_FLIP_BIT != 0
    }

    #[allow(dead_code)]
    pub(super) const fn vertical_flip(self) -> bool {
        self.raw & CGB_OBJ_ATTR_Y_FLIP_BIT != 0
    }

    #[allow(dead_code)]
    pub(super) const fn bg_over_obj(self) -> bool {
        self.raw & CGB_OBJ_ATTR_BG_OVER_OBJ_BIT != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum PpuMode3LiveBackgroundRegister {
    Lcdc,
    Scy,
    Scx,
}

impl PpuMode3LiveBackgroundRegister {
    pub(super) const fn from_register(register: PpuRegister) -> Option<Self> {
        match register {
            PpuRegister::Lcdc => Some(Self::Lcdc),
            PpuRegister::Scy => Some(Self::Scy),
            PpuRegister::Scx => Some(Self::Scx),
            PpuRegister::Stat
            | PpuRegister::Ly
            | PpuRegister::Lyc
            | PpuRegister::Bgp
            | PpuRegister::Obp0
            | PpuRegister::Obp1
            | PpuRegister::Wy
            | PpuRegister::Wx
            | PpuRegister::Bcps
            | PpuRegister::Bcpd
            | PpuRegister::Ocps
            | PpuRegister::Ocpd
            | PpuRegister::Opri => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode3TransferDotKind {
    NotServed,
    ServedPreVisibleTransfer,
    ServedHiddenTransfer,
    ServedVisiblePixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct Mode3TransferDot {
    pub(super) kind: Mode3TransferDotKind,
    pub(super) consumed_scx_discard: bool,
}

impl Mode3TransferDot {
    pub(super) const fn not_served() -> Self {
        Self {
            kind: Mode3TransferDotKind::NotServed,
            consumed_scx_discard: false,
        }
    }

    pub(super) const fn served(kind: Mode3TransferDotKind, consumed_scx_discard: bool) -> Self {
        Self {
            kind,
            consumed_scx_discard,
        }
    }

    pub(super) fn is_served(self) -> bool {
        !matches!(self.kind, Mode3TransferDotKind::NotServed)
    }

    pub(super) fn can_start_window_after_x0_service(self) -> bool {
        matches!(
            self.kind,
            Mode3TransferDotKind::ServedHiddenTransfer | Mode3TransferDotKind::ServedVisiblePixel
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode3TransferPhase {
    #[default]
    Priming,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode3TransferLane {
    PreVisible,
    Hidden,
    Visible,
}

impl Mode3TransferLane {
    pub(super) const fn dot_kind(self) -> Mode3TransferDotKind {
        match self {
            Self::PreVisible => Mode3TransferDotKind::ServedPreVisibleTransfer,
            Self::Hidden => Mode3TransferDotKind::ServedHiddenTransfer,
            Self::Visible => Mode3TransferDotKind::ServedVisiblePixel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode3TransferSourceWindow {
    AbstractStartup,
    FifoBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct Mode3TransferContext {
    pub(super) lane: Mode3TransferLane,
    pub(super) source_window: Mode3TransferSourceWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct Mode3TransferServicePlan {
    pub(super) result_kind: Mode3TransferDotKind,
    pub(super) execution: Mode3TransferServiceExecution,
    pub(super) backing: Mode3TransferBacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct Mode3CurrentTransfer {
    pub(super) context: Mode3TransferContext,
    pub(super) readiness: Mode3TransferReadiness,
}

impl Mode3CurrentTransfer {
    pub(super) const fn service_plan(self) -> Mode3TransferServicePlan {
        match self.readiness {
            Mode3TransferReadiness::WaitingForFifo(plan) | Mode3TransferReadiness::Ready(plan) => {
                plan
            }
        }
    }

    pub(super) const fn can_start_obj_fetch_from_fifo_backed_transfer(
        self,
        real_bg_fifo_pixel_ready: bool,
    ) -> bool {
        self.readiness
            .can_start_obj_fetch_from_fifo_backed_transfer(real_bg_fifo_pixel_ready)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode3TransferReadiness {
    WaitingForFifo(Mode3TransferServicePlan),
    Ready(Mode3TransferServicePlan),
}

impl Mode3TransferReadiness {
    pub(super) const fn can_start_obj_fetch_from_fifo_backed_transfer(
        self,
        real_bg_fifo_pixel_ready: bool,
    ) -> bool {
        match self {
            Self::Ready(plan) => {
                plan.can_start_obj_fetch_from_fifo_backed_transfer(real_bg_fifo_pixel_ready)
            }
            Self::WaitingForFifo(_) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode3TransferServiceExecution {
    ConsumeScxDiscard,
    AdvancePreVisibleWithBgPop,
    AdvanceHiddenWithBgAndObjPop,
    EmitVisiblePixel,
}

impl Mode3TransferServiceExecution {
    pub(super) const fn can_start_obj_fetch_from_fifo_backed_transfer(self) -> bool {
        matches!(
            self,
            Self::AdvanceHiddenWithBgAndObjPop | Self::EmitVisiblePixel
        )
    }

    pub(super) const fn requires_effective_bg_fifo_pixel(self) -> bool {
        matches!(
            self,
            Self::ConsumeScxDiscard
                | Self::AdvancePreVisibleWithBgPop
                | Self::AdvanceHiddenWithBgAndObjPop
        )
    }

    pub(super) const fn requires_real_bg_fifo_pixel(self) -> bool {
        matches!(self, Self::EmitVisiblePixel)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode3TransferBacking {
    Abstract,
    FifoBacked,
}

impl Mode3TransferServicePlan {
    pub(super) const fn requires_effective_bg_fifo_pixel(self) -> bool {
        self.execution.requires_effective_bg_fifo_pixel() && !self.requires_real_bg_fifo_pixel()
    }

    pub(super) const fn requires_real_bg_fifo_pixel(self) -> bool {
        self.execution.requires_real_bg_fifo_pixel()
            || (matches!(self.backing, Mode3TransferBacking::FifoBacked)
                && matches!(
                    self.execution,
                    Mode3TransferServiceExecution::ConsumeScxDiscard
                        | Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop
                ))
    }

    pub(super) const fn can_start_obj_fetch_from_fifo_backed_transfer(
        self,
        real_bg_fifo_pixel_ready: bool,
    ) -> bool {
        matches!(self.backing, Mode3TransferBacking::FifoBacked)
            && real_bg_fifo_pixel_ready
            && self
                .execution
                .can_start_obj_fetch_from_fifo_backed_transfer()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode3StartupSourceState {
    EntryDelay { remaining: u8 },
    Abstract { remaining: u8 },
    FifoBacked,
}

pub(super) const fn register_affects_pixel(
    register: PpuPaletteRegister,
    pixel: MixedPixel,
) -> bool {
    matches!(
        (register, pixel.source),
        (PpuPaletteRegister::Bgp, MixedPixelSource::Background)
            | (
                PpuPaletteRegister::Obp0,
                MixedPixelSource::Object {
                    palette_obp1: false,
                },
            )
            | (
                PpuPaletteRegister::Obp1,
                MixedPixelSource::Object { palette_obp1: true },
            )
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum OamCorruptionEventKind {
    Read,
    Write,
    ReadWithIncDec,
    WriteWithIncDec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct OamCorruptionController;

impl OamCorruptionController {
    pub(super) fn apply(
        self,
        console_model: ConsoleModel,
        current_row: u8,
        event: OamCorruptionEventKind,
        oam_bytes: &mut [u8],
    ) -> bool {
        if !console_model.is_dmg_family()
            || current_row >= OAM_CORRUPTION_ROW_COUNT
            || oam_bytes.len() < OAM_CORRUPTION_ROW_COUNT as usize * OAM_CORRUPTION_ROW_BYTES
        {
            return false;
        }

        match event {
            OamCorruptionEventKind::Read => self.apply_read_corruption(current_row, oam_bytes),
            OamCorruptionEventKind::Write | OamCorruptionEventKind::WriteWithIncDec => {
                self.apply_write_corruption(current_row, oam_bytes)
            }
            OamCorruptionEventKind::ReadWithIncDec => {
                self.apply_read_with_incdec_corruption(current_row, oam_bytes)
            }
        }

        true
    }

    pub(super) fn apply_write_corruption(self, current_row: u8, oam_bytes: &mut [u8]) {
        if current_row == 0 {
            return;
        }

        let current_first = read_oam_word(oam_bytes, current_row, 0);
        let previous_first = read_oam_word(oam_bytes, current_row - 1, 0);
        let previous_third = read_oam_word(oam_bytes, current_row - 1, 2);
        let corrupted_first =
            ((current_first ^ previous_third) & (previous_first ^ previous_third)) ^ previous_third;
        write_oam_word(oam_bytes, current_row, 0, corrupted_first);
        copy_previous_row_tail(oam_bytes, current_row);
    }

    pub(super) fn apply_read_corruption(self, current_row: u8, oam_bytes: &mut [u8]) {
        if current_row == 0 {
            return;
        }

        let current_first = read_oam_word(oam_bytes, current_row, 0);
        let previous_first = read_oam_word(oam_bytes, current_row - 1, 0);
        let previous_third = read_oam_word(oam_bytes, current_row - 1, 2);
        let corrupted_first = previous_first | (current_first & previous_third);
        write_oam_word(oam_bytes, current_row - 1, 0, corrupted_first);
        write_oam_word(oam_bytes, current_row, 0, corrupted_first);
        copy_previous_row_tail(oam_bytes, current_row);
    }

    pub(super) fn apply_read_with_incdec_corruption(self, current_row: u8, oam_bytes: &mut [u8]) {
        if (4..(OAM_CORRUPTION_ROW_COUNT - 1)).contains(&current_row) {
            let row_minus_two = current_row - 2;
            let previous_row = current_row - 1;
            let a = read_oam_word(oam_bytes, row_minus_two, 0);
            let b = read_oam_word(oam_bytes, previous_row, 0);
            let c = read_oam_word(oam_bytes, current_row, 0);
            let d = read_oam_word(oam_bytes, previous_row, 2);
            let corrupted_previous_first = (b & (a | c | d)) | (a & c & d);
            write_oam_word(oam_bytes, previous_row, 0, corrupted_previous_first);

            let previous_row_bytes = read_oam_row(oam_bytes, previous_row);
            write_oam_row(oam_bytes, current_row, previous_row_bytes);
            write_oam_row(oam_bytes, row_minus_two, previous_row_bytes);
        }

        self.apply_read_corruption(current_row, oam_bytes);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct BgFifo {
    entries: VecDeque<BgFifoPixel>,
}

impl BgFifo {
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn dynamic_payload_bytes(&self) -> usize {
        self.entries
            .len()
            .saturating_mul(std::mem::size_of::<BgFifoPixel>())
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &u8> {
        self.entries.iter().map(|pixel| &pixel.color)
    }

    pub(super) fn cached_slots(&self) -> impl Iterator<Item = Option<BgFifoPixelCached>> + '_ {
        self.entries.iter().map(|pixel| pixel.cached)
    }

    pub(super) fn cached_pixels(&self) -> impl Iterator<Item = BgFifoPixelCached> + '_ {
        self.cached_slots().flatten()
    }

    pub(super) fn cached_pixels_mut(&mut self) -> impl Iterator<Item = &mut BgFifoPixelCached> {
        self.entries
            .iter_mut()
            .filter_map(|pixel| pixel.cached.as_mut())
    }

    #[cfg(test)]
    pub(super) fn cached_len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn cached_front(&self) -> Option<Option<BgFifoPixelCached>> {
        self.entries.front().map(|pixel| pixel.cached)
    }

    #[cfg(test)]
    pub(super) fn cached_slot(&self, index: usize) -> Option<Option<BgFifoPixelCached>> {
        self.entries.get(index).map(|pixel| pixel.cached)
    }

    pub(super) fn first_cached(&self) -> Option<BgFifoPixelCached> {
        self.entries.iter().find_map(|pixel| pixel.cached)
    }

    #[cfg(test)]
    pub(super) fn back(&self) -> Option<&u8> {
        self.entries.back().map(|pixel| &pixel.color)
    }

    pub(super) fn push_back(&mut self, color: u8) {
        self.push_back_pixel(BgFifoPixel::new(color, None));
    }

    pub(super) fn push_front(&mut self, color: u8) {
        self.push_front_pixel(BgFifoPixel::new(color, None));
    }

    #[cfg(test)]
    pub(super) fn push_back_cached_slot(&mut self, cached: Option<BgFifoPixelCached>) {
        if let Some(back) = self.entries.back_mut()
            && back.cached.is_none()
        {
            back.cached = cached;
            return;
        }

        self.push_back_pixel(BgFifoPixel::new(0, cached));
    }

    pub(super) fn push_back_pixel(&mut self, pixel: BgFifoPixel) {
        self.entries.push_back(pixel);
    }

    pub(super) fn push_front_pixel(&mut self, pixel: BgFifoPixel) {
        self.entries.push_front(pixel);
    }

    #[cfg(test)]
    pub(super) fn pop_front(&mut self) -> Option<u8> {
        self.pop_front_pixel().map(BgFifoPixel::color)
    }

    pub(super) fn pop_front_pixel(&mut self) -> Option<BgFifoPixel> {
        self.entries.pop_front()
    }
}

impl Extend<u8> for BgFifo {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        self.entries
            .extend(iter.into_iter().map(|color| BgFifoPixel::new(color, None)));
    }
}

impl FromIterator<u8> for BgFifo {
    fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
        let mut fifo = Self::default();
        fifo.extend(iter);
        fifo
    }
}

impl std::ops::Index<usize> for BgFifo {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index].color
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct BgPipelineState {
    pub(super) fetcher: BgFetcherState,
    pub(super) push: BgPushState,
    pub(super) fill: BgFifoFillState,
    pub(super) fifo: BgFifo,
    pub(super) startup_fetch_seam: BgStartupFetchSeamState,
    pub(super) startup_fifo_placeholders: u8,
    pub(super) mode3_started: bool,
    pub(super) initial_scx_capture_pending: bool,
    pub(super) mode0_start_dot: u16,
    pub(super) initial_scx_discard: u8,
    pub(super) scx_discard_remaining: u8,
    pub(super) startup_source_state: Mode3StartupSourceState,
    pub(super) startup_pre_visible_transfer_dots_remaining: u8,
    pub(super) transfer_phase: Mode3TransferPhase,
    pub(super) current_transfer_x: u8,
    pub(super) visible_pixels_output: u8,
    pub(super) saw_right_edge_visible_same_x_cluster_this_line: bool,
    pub(super) window_wy_latch: bool,
    pub(super) window_lcdc5_latch: bool,
    pub(super) window_force_x0_this_line: bool,
    pub(super) window_started_this_line: bool,
    pub(super) window_active_line_counter: u8,
    pub(super) window_start_count_this_line: u8,
    pub(super) wx0_scx_shortening_applied: bool,
    pub(super) wx166_armed_this_line: bool,
    pub(super) startup_visible_tile3_scx_boundary_next_slice_previous_scx: Option<u8>,
    pub(super) startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels: u8,
    pub(super) startup_scy_tiledata_latch: Option<BgStartupScyTiledataLatch>,
    pub(super) cgb_dmg_scy_startup_retarget_active: bool,
    pub(super) window_activation_tilemap_select_latch: Option<bool>,
    pub(super) dmg_wx0_window_disable_prefix_state: Option<DmgWx0WindowDisablePrefixState>,
    pub(super) dmg_late_window_enable_override: Option<DmgLateWindowEnableOverride>,
    pub(super) dmg_window_restart: DmgWindowRestartState,
    pub(super) dmg_mode3_live_lcdc_bg_state: DmgMode3LiveLcdcBgState,
}

impl BgPipelineState {
    pub(super) fn dynamic_payload_bytes(&self) -> usize {
        self.fifo.dynamic_payload_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgWx0WindowDisablePrefixState {
    pub(super) desired_prefix_pixels: u8,
    pub(super) prefix_bg_pixel: Option<u8>,
}

impl DmgWx0WindowDisablePrefixState {
    pub(super) const fn new(desired_prefix_pixels: u8) -> Self {
        Self {
            desired_prefix_pixels,
            prefix_bg_pixel: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgLateWindowEnableOverride {
    pub(super) onset_x: u8,
    pub(super) end_x: u8,
    pub(super) window_origin_x: u8,
}

impl DmgLateWindowEnableOverride {
    pub(super) const fn new(onset_x: u8, end_x: u8, window_origin_x: u8) -> Self {
        Self {
            onset_x,
            end_x,
            window_origin_x,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum DmgPrevisibleWxRetargetKind {
    CancelOnly,
    OneHiddenPrefixResume,
    RetainedFifoPrefixResume { advance_tilemap: bool },
    PlainRestart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgPrevisibleWxRetarget {
    pub(super) kind: DmgPrevisibleWxRetargetKind,
    pub(super) trigger_x: Option<u8>,
    pub(super) active_line_counter: u8,
    pub(super) window_pixel_offset: u16,
}

impl DmgPrevisibleWxRetarget {
    pub(super) const fn new(
        trigger_x: u8,
        active_line_counter: u8,
        window_pixel_offset: u16,
    ) -> Self {
        Self {
            kind: DmgPrevisibleWxRetargetKind::PlainRestart,
            trigger_x: Some(trigger_x),
            active_line_counter,
            window_pixel_offset,
        }
    }

    pub(super) const fn new_one_hidden_prefix_resume(
        trigger_x: u8,
        active_line_counter: u8,
        window_pixel_offset: u16,
    ) -> Self {
        Self {
            kind: DmgPrevisibleWxRetargetKind::OneHiddenPrefixResume,
            trigger_x: Some(trigger_x),
            active_line_counter,
            window_pixel_offset,
        }
    }

    pub(super) const fn new_retained_fifo_prefix_resume(
        trigger_x: u8,
        active_line_counter: u8,
        window_pixel_offset: u16,
        next_tilemap: bool,
    ) -> Self {
        Self {
            kind: DmgPrevisibleWxRetargetKind::RetainedFifoPrefixResume {
                advance_tilemap: next_tilemap,
            },
            trigger_x: Some(trigger_x),
            active_line_counter,
            window_pixel_offset,
        }
    }

    pub(super) const fn new_cancel_only(active_line_counter: u8, window_pixel_offset: u16) -> Self {
        Self {
            kind: DmgPrevisibleWxRetargetKind::CancelOnly,
            trigger_x: None,
            active_line_counter,
            window_pixel_offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgPendingPrevisibleWxCarry {
    pub(super) next_trigger_x: u8,
    pub(super) end_trigger_x: u8,
    pub(super) active_line_counter: u8,
    pub(super) next_window_pixel_offset: u16,
}

impl DmgPendingPrevisibleWxCarry {
    pub(super) const fn new(
        next_trigger_x: u8,
        end_trigger_x: u8,
        active_line_counter: u8,
        next_window_pixel_offset: u16,
    ) -> Self {
        Self {
            next_trigger_x,
            end_trigger_x,
            active_line_counter,
            next_window_pixel_offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgPendingLiveWxTriggerGlitch {
    pub(super) trigger_x: u8,
}

impl DmgPendingLiveWxTriggerGlitch {
    pub(super) const fn new(trigger_x: u8) -> Self {
        Self { trigger_x }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgPendingWindowReenableResume {
    pub(super) onset_x: u8,
    pub(super) window_origin_x: u8,
    pub(super) emitted_window_pixels: u8,
    pub(super) disable_stage: PpuBgFetcherStage,
    pub(super) disable_stage_dot: u8,
}

impl DmgPendingWindowReenableResume {
    pub(super) const fn new(
        onset_x: u8,
        window_origin_x: u8,
        emitted_window_pixels: u8,
        disable_stage: PpuBgFetcherStage,
        disable_stage_dot: u8,
    ) -> Self {
        Self {
            onset_x,
            window_origin_x,
            emitted_window_pixels,
            disable_stage,
            disable_stage_dot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum CgbPrevisibleWxPhaseRepaintMode {
    FixedPattern,
    Wx4ToWx5FixedPrefix,
    CurrentLowPlaneIntoHighPlane,
    CurrentHighPlaneWithWindowHighPlaneAsLowPlane {
        source_start_x: u16,
        window_line_counter: u8,
    },
    WindowPixels {
        source_start_x: u16,
        window_line_counter: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct CgbPendingPrevisibleWxPhaseRepaint {
    pub(super) cancel_guard_x: u8,
    pub(super) start_x: u8,
    pub(super) end_x: u8,
    pub(super) pattern_len: u8,
    pub(super) pixels: [u8; 16],
    pub(super) mode: CgbPrevisibleWxPhaseRepaintMode,
}

impl CgbPendingPrevisibleWxPhaseRepaint {
    pub(super) const fn new(
        cancel_guard_x: u8,
        start_x: u8,
        end_x: u8,
        pattern_len: u8,
        pixels: [u8; 16],
    ) -> Self {
        Self {
            cancel_guard_x,
            start_x,
            end_x,
            pattern_len,
            pixels,
            mode: CgbPrevisibleWxPhaseRepaintMode::FixedPattern,
        }
    }

    pub(super) const fn new_wx4_to_wx5_fixed_prefix() -> Self {
        Self {
            cancel_guard_x: 0,
            start_x: 0,
            end_x: 16,
            pattern_len: 16,
            pixels: [3, 3, 3, 3, 3, 0, 3, 3, 1, 1, 3, 3, 0, 3, 3, 1],
            mode: CgbPrevisibleWxPhaseRepaintMode::Wx4ToWx5FixedPrefix,
        }
    }

    pub(super) const fn new_current_low_plane_into_high_plane(
        cancel_guard_x: u8,
        start_x: u8,
        end_x: u8,
    ) -> Self {
        Self {
            cancel_guard_x,
            start_x,
            end_x,
            pattern_len: 1,
            pixels: [0; 16],
            mode: CgbPrevisibleWxPhaseRepaintMode::CurrentLowPlaneIntoHighPlane,
        }
    }

    pub(super) const fn new_current_high_plane_with_window_high_plane_as_low_plane(
        cancel_guard_x: u8,
        start_x: u8,
        end_x: u8,
        source_start_x: u16,
        window_line_counter: u8,
    ) -> Self {
        Self {
            cancel_guard_x,
            start_x,
            end_x,
            pattern_len: 1,
            pixels: [0; 16],
            mode: CgbPrevisibleWxPhaseRepaintMode::CurrentHighPlaneWithWindowHighPlaneAsLowPlane {
                source_start_x,
                window_line_counter,
            },
        }
    }

    pub(super) const fn new_window_pixels(
        cancel_guard_x: u8,
        start_x: u8,
        end_x: u8,
        source_start_x: u16,
        window_line_counter: u8,
    ) -> Self {
        Self {
            cancel_guard_x,
            start_x,
            end_x,
            pattern_len: 1,
            pixels: [0; 16],
            mode: CgbPrevisibleWxPhaseRepaintMode::WindowPixels {
                source_start_x,
                window_line_counter,
            },
        }
    }

    pub(super) fn pixel_at(self, x: u8) -> u8 {
        let offset = x.saturating_sub(self.start_x) % self.pattern_len.max(1);
        self.pixels[offset as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgWindowRestartState {
    pub(super) pending_window_reenable_resume: Option<DmgPendingWindowReenableResume>,
    pub(super) previsible_wx_retarget: Option<DmgPrevisibleWxRetarget>,
    pub(super) previsible_wx_cancel_uses_visible_wx_once: bool,
    pub(super) previsible_wx_cancel_background_override_onset_x: Option<u8>,
    pub(super) previsible_wx_retained_trigger_glitch_x: Option<u8>,
    pub(super) pending_previsible_wx_onset_glitch: Option<u8>,
    pub(super) pending_previsible_wx_carry: Option<DmgPendingPrevisibleWxCarry>,
    pub(super) pending_live_wx_trigger_glitch: Option<DmgPendingLiveWxTriggerGlitch>,
    pub(super) pending_cgb_previsible_wx_phase_repaint: Option<CgbPendingPrevisibleWxPhaseRepaint>,
}

impl DmgWindowRestartState {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn apply_followup_markers(
        &mut self,
        cancel_uses_visible_wx_once: bool,
        cancel_background_override_onset_x: Option<u8>,
        retained_trigger_glitch_x: Option<u8>,
    ) {
        self.previsible_wx_cancel_uses_visible_wx_once = cancel_uses_visible_wx_once;
        self.previsible_wx_cancel_background_override_onset_x = cancel_background_override_onset_x;
        self.previsible_wx_retained_trigger_glitch_x = retained_trigger_glitch_x;
    }

    pub(super) fn clear_followup_markers(&mut self) {
        self.apply_followup_markers(false, None, None);
    }

    pub(super) fn clear_followup_overrides(&mut self) {
        self.previsible_wx_cancel_background_override_onset_x = None;
        self.previsible_wx_retained_trigger_glitch_x = None;
    }

    pub(super) fn arm_previsible_wx_retarget_state(
        &mut self,
        retarget: DmgPrevisibleWxRetarget,
        onset_glitch: Option<u8>,
        carry: Option<DmgPendingPrevisibleWxCarry>,
    ) {
        self.previsible_wx_retarget = Some(retarget);
        self.pending_previsible_wx_onset_glitch = onset_glitch;
        self.pending_previsible_wx_carry = carry;
        self.clear_live_trigger_glitch();
    }

    pub(super) fn clear_retarget_state(&mut self) {
        self.previsible_wx_retarget = None;
    }

    pub(super) fn clear_carry(&mut self) {
        self.pending_previsible_wx_carry = None;
    }

    pub(super) fn clear_live_trigger_glitch(&mut self) {
        self.pending_live_wx_trigger_glitch = None;
    }

    pub(super) fn clear_onset_glitch(&mut self) {
        self.pending_previsible_wx_onset_glitch = None;
    }

    pub(super) fn clear_cgb_previsible_wx_phase_repaint(&mut self) {
        self.pending_cgb_previsible_wx_phase_repaint = None;
    }

    pub(super) fn clear_gap_artifacts(&mut self) {
        self.clear_onset_glitch();
        self.clear_carry();
    }

    pub(super) fn clear_retarget_and_gap_artifacts(&mut self) {
        self.clear_retarget_state();
        self.clear_gap_artifacts();
    }

    pub(super) fn clear_expired_retarget_state(&mut self) {
        self.clear_retarget_state();
        self.clear_followup_overrides();
        self.clear_gap_artifacts();
    }

    pub(super) fn clear_restart_transients(&mut self) {
        self.clear_retarget_state();
        self.clear_followup_markers();
        self.clear_carry();
        self.clear_live_trigger_glitch();
    }

    pub(super) fn arm_live_trigger_glitch(&mut self, trigger_x: u8) {
        self.pending_live_wx_trigger_glitch = Some(DmgPendingLiveWxTriggerGlitch::new(trigger_x));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct BgStartupScyTiledataLatch {
    lcdc: u8,
    tile_data_row: u16,
}

impl BgStartupScyTiledataLatch {
    pub(super) const fn new(lcdc: u8, tile_data_row: u16) -> Self {
        Self {
            lcdc,
            tile_data_row,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct PerPlane<T> {
    pub(super) low: T,
    pub(super) high: T,
}

impl<T> PerPlane<T> {
    pub(super) const fn new(low: T, high: T) -> Self {
        Self { low, high }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum BgTileDataSelect {
    Signed8800,
    Unsigned8000,
}

impl BgTileDataSelect {
    pub(super) const fn apply_to_lcdc(self, lcdc: u8) -> u8 {
        match self {
            Self::Signed8800 => lcdc & !LCDC_BG_WINDOW_TILE_DATA_BIT,
            Self::Unsigned8000 => lcdc | LCDC_BG_WINDOW_TILE_DATA_BIT,
        }
    }

    pub(super) const fn opposite(self) -> Self {
        match self {
            Self::Signed8800 => Self::Unsigned8000,
            Self::Unsigned8000 => Self::Signed8800,
        }
    }
}

pub(in crate::ppu) type BgTileDataSelectOverride = PerPlane<Option<BgTileDataSelect>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct StartupContinuationSliceOverrides<T> {
    pub(super) visible_tile2: T,
    pub(super) visible_tile3: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum BgVisibleStartupSlice {
    VisibleTile2,
    VisibleTile3,
}

impl<T: Copy> StartupContinuationSliceOverrides<T> {
    pub(super) fn set_for_slice(&mut self, slice: BgVisibleStartupSlice, value: T) {
        match slice {
            BgVisibleStartupSlice::VisibleTile2 => self.visible_tile2 = value,
            BgVisibleStartupSlice::VisibleTile3 => self.visible_tile3 = value,
        }
    }

    pub(super) const fn for_visible_slice(self, slice: BgVisibleStartupSlice) -> T {
        match slice {
            BgVisibleStartupSlice::VisibleTile2 => self.visible_tile2,
            BgVisibleStartupSlice::VisibleTile3 => self.visible_tile3,
        }
    }
}

impl<T: Copy + Default> StartupContinuationSliceOverrides<T> {
    pub(super) fn for_optional_visible_slice(self, slice: Option<BgVisibleStartupSlice>) -> T {
        slice.map_or_else(T::default, |slice| self.for_visible_slice(slice))
    }
}

type DmgLcdc3StartupTilemapSelectOverrides = StartupContinuationSliceOverrides<Option<bool>>;
type DmgLcdc4StartupTileDataSelectOverrides =
    StartupContinuationSliceOverrides<BgTileDataSelectOverride>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgStartupContinuationOverrides {
    pub(super) lcdc3_tilemap_select: DmgLcdc3StartupTilemapSelectOverrides,
    pub(super) lcdc4_tiledata_select: DmgLcdc4StartupTileDataSelectOverrides,
}

impl DmgStartupContinuationOverrides {
    pub(super) fn latch_lcdc3_tilemap_select(
        &mut self,
        tilemap_select: bool,
        applies_to_visible_tile2: bool,
        applies_to_visible_tile3: bool,
    ) {
        self.lcdc3_tilemap_select.set_for_slice(
            BgVisibleStartupSlice::VisibleTile2,
            applies_to_visible_tile2.then_some(tilemap_select),
        );
        self.lcdc3_tilemap_select.set_for_slice(
            BgVisibleStartupSlice::VisibleTile3,
            applies_to_visible_tile3.then_some(tilemap_select),
        );
    }

    pub(super) fn lcdc3_tilemap_select_for_cached_slice(
        self,
        cached: BgCachedSlice,
    ) -> Option<bool> {
        if !cached.is_background() {
            return None;
        }

        self.lcdc3_tilemap_select
            .for_optional_visible_slice(cached.visible_startup_continuation_slice())
    }

    pub(super) fn clear_lcdc3_tilemap_select_for_slice(&mut self, slice: BgVisibleStartupSlice) {
        self.lcdc3_tilemap_select.set_for_slice(slice, None);
    }

    pub(super) fn latch_lcdc4_tiledata_select(
        &mut self,
        slice: BgVisibleStartupSlice,
        override_select: BgTileDataSelectOverride,
    ) {
        self.lcdc4_tiledata_select
            .set_for_slice(slice, override_select);
    }

    pub(super) fn for_cached_slice(self, cached: BgCachedSlice) -> BgTileDataSelectOverride {
        if !cached.is_background() {
            return PerPlane::new(None, None);
        }

        self.lcdc4_tiledata_select
            .for_optional_visible_slice(cached.visible_startup_continuation_slice())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgMode3LiveLcdcBgState {
    pub(super) lcdc3_current_line_bg_tilemap_write_count: u8,
    pub(super) startup_continuation_overrides: DmgStartupContinuationOverrides,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgLcdc0PanelLiveWriteState {
    pub(super) current_line_bg_enable_write_count: u8,
    pub(super) bg_enable_visible_hold: DmgVisibleHold<bool>,
}

impl DmgLcdc0PanelLiveWriteState {
    pub(super) fn take_next_bg_enable_write_index(&mut self) -> usize {
        let write_index = self.current_line_bg_enable_write_count as usize;
        self.current_line_bg_enable_write_count =
            self.current_line_bg_enable_write_count.saturating_add(1);
        write_index
    }

    pub(super) fn clear_bg_enable_visible_hold(&mut self) {
        self.bg_enable_visible_hold.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgLcdc1PanelLiveWriteState {
    pub(super) obj_enable_visible_hold: DmgVisibleHold<bool>,
}

impl DmgLcdc1PanelLiveWriteState {
    pub(super) fn clear_obj_enable_visible_hold(&mut self) {
        self.obj_enable_visible_hold.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgVisibleHold<T> {
    pub(super) override_value: Option<T>,
    pub(super) pixels_remaining: u8,
}

impl<T> DmgVisibleHold<T> {
    pub(super) fn clear(&mut self) {
        self.override_value = None;
        self.pixels_remaining = 0;
    }

    pub(super) fn consume(&mut self) {
        if self.pixels_remaining == 0 {
            self.override_value = None;
            return;
        }

        self.pixels_remaining -= 1;
        if self.pixels_remaining == 0 {
            self.override_value = None;
        }
    }
}

impl<T: Copy> DmgVisibleHold<T> {
    pub(super) fn set(&mut self, override_value: T, pixels_remaining: u8) {
        self.override_value = Some(override_value);
        self.pixels_remaining = pixels_remaining;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum DmgLcdc2ObservedEffectState {
    Pending,
    Applied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgLcdc2ActiveObjSizeWrite {
    pub(super) write_index: u8,
    pub(super) visible_x: u8,
    pub(super) observed_effect_state: DmgLcdc2ObservedEffectState,
}

impl DmgLcdc2ActiveObjSizeWrite {
    pub(super) fn new(write_index: usize, visible_x: u8) -> Self {
        Self {
            write_index: write_index as u8,
            visible_x,
            observed_effect_state: DmgLcdc2ObservedEffectState::Pending,
        }
    }

    pub(super) fn observed_effects_pending(self) -> bool {
        matches!(
            self.observed_effect_state,
            DmgLcdc2ObservedEffectState::Pending
        )
    }

    pub(super) fn mark_observed_effects_applied(&mut self) {
        self.observed_effect_state = DmgLcdc2ObservedEffectState::Applied;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgLcdc2ObjSizeLiveWriteState {
    pub(super) current_line_obj_size_write_count: u8,
    pub(super) active_write: Option<DmgLcdc2ActiveObjSizeWrite>,
    pub(super) retained_pending_write: Option<DmgLcdc2ActiveObjSizeWrite>,
}

impl DmgLcdc2ObjSizeLiveWriteState {
    pub(super) fn take_next_obj_size_write_index(&mut self) -> usize {
        let write_index = self.current_line_obj_size_write_count as usize;
        self.current_line_obj_size_write_count =
            self.current_line_obj_size_write_count.saturating_add(1);
        write_index
    }

    pub(super) fn begin_active_shrink(&mut self, write_index: usize, visible_x: u8) {
        if self
            .active_write
            .is_some_and(|write| write.observed_effects_pending())
        {
            self.retained_pending_write = self.active_write;
        }
        self.active_write = Some(DmgLcdc2ActiveObjSizeWrite::new(write_index, visible_x));
    }

    pub(super) fn active_write(self) -> Option<DmgLcdc2ActiveObjSizeWrite> {
        self.active_write
    }

    pub(super) fn pending_writes(self) -> [Option<DmgLcdc2ActiveObjSizeWrite>; 2] {
        [self.retained_pending_write, self.active_write]
    }

    pub(super) fn mark_observed_effects_applied_for_write(&mut self, write_index: u8) {
        if self
            .retained_pending_write
            .is_some_and(|write| write.write_index == write_index)
        {
            self.retained_pending_write = None;
            return;
        }

        if let Some(active_write) = self
            .active_write
            .as_mut()
            .filter(|write| write.write_index == write_index)
        {
            active_write.mark_observed_effects_applied();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgBgpCpuCommitState {
    pub(super) output_palette_override: Option<u8>,
    pub(super) output_delay_pixels_remaining: u8,
    pub(super) output_followup_palette_override: Option<u8>,
    pub(super) output_followup_pixels_remaining: u8,
    pub(super) bg_visible_hold_palette_override: Option<u8>,
    pub(super) bg_visible_hold_bg_pixels_remaining: u8,
    pub(super) bg_visible_hold_fallback_palette: Option<u8>,
    pub(super) current_line_start_palette: u8,
    pub(super) previous_line_start_palette: u8,
    pub(super) current_line_writes: Vec<PpuDmgBgpCpuCommitWrite>,
    pub(super) previous_line_writes: Vec<PpuDmgBgpCpuCommitWrite>,
}

impl DmgBgpCpuCommitState {
    pub(super) fn dynamic_payload_bytes(&self) -> usize {
        self.current_line_writes
            .len()
            .saturating_add(self.previous_line_writes.len())
            .saturating_mul(std::mem::size_of::<PpuDmgBgpCpuCommitWrite>())
    }
}

impl DmgBgpCpuCommitState {
    pub(super) fn reset_for_startup(&mut self, bgp: u8) {
        self.output_palette_override = None;
        self.output_delay_pixels_remaining = 0;
        self.output_followup_palette_override = None;
        self.output_followup_pixels_remaining = 0;
        self.bg_visible_hold_palette_override = None;
        self.bg_visible_hold_bg_pixels_remaining = 0;
        self.bg_visible_hold_fallback_palette = None;
        self.current_line_start_palette = bgp;
        self.previous_line_start_palette = bgp;
        self.current_line_writes.clear();
        self.previous_line_writes.clear();
    }

    pub(super) fn reset_for_scanline_start(&mut self, bgp: u8) {
        self.bg_visible_hold_palette_override = None;
        self.bg_visible_hold_bg_pixels_remaining = 0;
        self.bg_visible_hold_fallback_palette = None;
        self.current_line_start_palette = bgp;
        self.current_line_writes.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DmgPanelLiveWriteState {
    pub(super) lcdc0: DmgLcdc0PanelLiveWriteState,
    pub(super) lcdc1: DmgLcdc1PanelLiveWriteState,
    pub(super) lcdc2: DmgLcdc2ObjSizeLiveWriteState,
    pub(super) bgp_cpu_commit: DmgBgpCpuCommitState,
    pub(super) recent_panel_dots: VecDeque<PpuRecentPanelDot>,
}

impl DmgPanelLiveWriteState {
    pub(super) fn dynamic_payload_bytes(&self) -> usize {
        self.bgp_cpu_commit.dynamic_payload_bytes().saturating_add(
            self.recent_panel_dots
                .len()
                .saturating_mul(std::mem::size_of::<PpuRecentPanelDot>()),
        )
    }
}

impl DmgPanelLiveWriteState {
    pub(super) fn reset_for_startup(&mut self, bgp: u8) {
        self.lcdc0 = DmgLcdc0PanelLiveWriteState::default();
        self.lcdc1 = DmgLcdc1PanelLiveWriteState::default();
        self.lcdc2 = DmgLcdc2ObjSizeLiveWriteState::default();
        self.bgp_cpu_commit.reset_for_startup(bgp);
        self.recent_panel_dots.clear();
    }

    pub(super) fn reset_for_scanline_start(&mut self, bgp: u8) {
        self.lcdc0 = DmgLcdc0PanelLiveWriteState::default();
        self.lcdc1 = DmgLcdc1PanelLiveWriteState::default();
        self.lcdc2 = DmgLcdc2ObjSizeLiveWriteState::default();
        self.bgp_cpu_commit.reset_for_scanline_start(bgp);
        self.recent_panel_dots.clear();
    }
}

impl Default for DmgPanelLiveWriteState {
    fn default() -> Self {
        Self {
            lcdc0: DmgLcdc0PanelLiveWriteState::default(),
            lcdc1: DmgLcdc1PanelLiveWriteState::default(),
            lcdc2: DmgLcdc2ObjSizeLiveWriteState::default(),
            bgp_cpu_commit: DmgBgpCpuCommitState::default(),
            recent_panel_dots: VecDeque::with_capacity(DMG_PALETTE_RETROACTIVE_DOT_HISTORY),
        }
    }
}

impl BgPipelineState {
    pub(super) fn reset(&mut self) {
        self.fetcher.reset();
        self.push.reset();
        self.fill.reset();
        self.fifo.clear();
        self.startup_fetch_seam = BgStartupFetchSeamState::Inactive;
        self.startup_fifo_placeholders = 0;
        self.mode3_started = false;
        self.initial_scx_capture_pending = false;
        self.mode0_start_dot = MODE0_START_DOT;
        self.initial_scx_discard = 0;
        self.scx_discard_remaining = 0;
        self.startup_source_state = Mode3StartupSourceState::FifoBacked;
        self.startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
        self.transfer_phase = Mode3TransferPhase::Priming;
        self.current_transfer_x = 0;
        self.visible_pixels_output = 0;
        self.saw_right_edge_visible_same_x_cluster_this_line = false;
        self.window_wy_latch = false;
        self.window_lcdc5_latch = false;
        self.window_force_x0_this_line = false;
        self.window_started_this_line = false;
        self.window_active_line_counter = 0;
        self.window_start_count_this_line = 0;
        self.wx0_scx_shortening_applied = false;
        self.wx166_armed_this_line = false;
        self.startup_visible_tile3_scx_boundary_next_slice_previous_scx = None;
        self.startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels = 0;
        self.startup_scy_tiledata_latch = None;
        self.cgb_dmg_scy_startup_retarget_active = false;
        self.window_activation_tilemap_select_latch = None;
        self.dmg_wx0_window_disable_prefix_state = None;
        self.dmg_late_window_enable_override = None;
        self.dmg_window_restart.clear();
        self.dmg_mode3_live_lcdc_bg_state = Default::default();
    }

    pub(super) fn start_line(&mut self, _scx: u8) {
        self.mode3_started = true;
        self.initial_scx_capture_pending = true;
        self.initial_scx_discard = 0;
        self.mode0_start_dot = MODE0_START_DOT;
        self.scx_discard_remaining = 0;
        self.fifo.clear();
        self.startup_fetch_seam = BgStartupFetchSeamState::AlignmentSeedPending;
        self.startup_scy_tiledata_latch = None;
        self.cgb_dmg_scy_startup_retarget_active = false;
        self.window_activation_tilemap_select_latch = None;
        self.dmg_mode3_live_lcdc_bg_state = Default::default();
        self.startup_fifo_placeholders = MODE3_ABSTRACT_SOURCE_WINDOW_DOTS;
        self.startup_source_state = Mode3StartupSourceState::EntryDelay {
            remaining: MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT as u8,
        };
        self.startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
        self.transfer_phase = Mode3TransferPhase::Priming;
        self.current_transfer_x = 0;
        self.saw_right_edge_visible_same_x_cluster_this_line = false;
        self.push.reset();
        self.fill.reset();
        self.fetcher.start_background();
    }

    pub(super) fn capture_initial_scx(&mut self, scx: u8) {
        if !self.initial_scx_capture_pending {
            return;
        }

        self.initial_scx_capture_pending = false;
        self.initial_scx_discard = scx & 0x07;
        self.mode0_start_dot = MODE0_START_DOT + u16::from(self.initial_scx_discard);
        self.scx_discard_remaining = self.initial_scx_discard;
    }

    pub(super) fn retune_previsible_scx_discard(&mut self, scx: u8) {
        if !self.mode3_started
            || self.initial_scx_capture_pending
            || self.visible_pixels_output != 0
            || self.current_transfer_x >= 8
        {
            return;
        }

        let new_scx_discard = scx & 0x07;
        if new_scx_discard == self.initial_scx_discard {
            return;
        }

        let consumed_scx_discard = self
            .initial_scx_discard
            .saturating_sub(self.scx_discard_remaining);
        let consumed_previsible_slots =
            consumed_scx_discard.saturating_add(self.current_transfer_x.min(8));
        let old_scx_discard = self.initial_scx_discard;
        self.initial_scx_discard = new_scx_discard;
        let consumed_scx_discard_after_retarget = consumed_previsible_slots.min(new_scx_discard);
        self.current_transfer_x = consumed_previsible_slots - consumed_scx_discard_after_retarget;
        self.scx_discard_remaining =
            new_scx_discard.saturating_sub(consumed_scx_discard_after_retarget);

        if new_scx_discard > old_scx_discard {
            self.mode0_start_dot += u16::from(new_scx_discard - old_scx_discard);
        } else {
            self.mode0_start_dot = self
                .mode0_start_dot
                .saturating_sub(u16::from(old_scx_discard - new_scx_discard));
        }
    }

    pub(super) fn prepare_window_line(
        &mut self,
        wy_latch: bool,
        lcdc5_latch: bool,
        force_x0_this_line: bool,
    ) {
        self.window_wy_latch = wy_latch;
        self.window_lcdc5_latch = lcdc5_latch;
        self.window_force_x0_this_line = force_x0_this_line;
        self.window_started_this_line = false;
        self.wx0_scx_shortening_applied = false;
        self.wx166_armed_this_line = false;
        self.startup_visible_tile3_scx_boundary_next_slice_previous_scx = None;
        self.startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels = 0;
        self.startup_scy_tiledata_latch = None;
        self.cgb_dmg_scy_startup_retarget_active = false;
        self.window_activation_tilemap_select_latch = None;
        self.dmg_wx0_window_disable_prefix_state = None;
        self.dmg_late_window_enable_override = None;
        self.dmg_window_restart.clear();
    }

    pub(super) fn extend_mode3_by_one_dot(&mut self) {
        self.mode0_start_dot += 1;
    }

    pub(super) fn consume_startup_transfer_entry_delay_dot(&mut self) -> bool {
        if !self.mode3_started {
            return false;
        }

        match self.startup_source_state {
            Mode3StartupSourceState::EntryDelay { remaining } => {
                debug_assert!(
                    remaining > 0,
                    "entry delay state must keep a positive countdown"
                );
                if remaining == 1 {
                    self.startup_source_state = Mode3StartupSourceState::Abstract {
                        remaining: MODE3_ABSTRACT_SOURCE_WINDOW_DOTS,
                    };
                } else {
                    self.startup_source_state = Mode3StartupSourceState::EntryDelay {
                        remaining: remaining - 1,
                    };
                }
                true
            }
            Mode3StartupSourceState::Abstract { .. } | Mode3StartupSourceState::FifoBacked => false,
        }
    }

    pub(super) fn consume_startup_source_window_dot(&mut self) {
        if !self.mode3_started {
            return;
        }

        match self.startup_source_state {
            Mode3StartupSourceState::Abstract { remaining } => {
                debug_assert!(
                    remaining > 0,
                    "abstract startup state must keep a positive countdown"
                );
                if remaining == 1 {
                    self.startup_source_state = Mode3StartupSourceState::FifoBacked;
                } else {
                    self.startup_source_state = Mode3StartupSourceState::Abstract {
                        remaining: remaining - 1,
                    };
                }
            }
            Mode3StartupSourceState::EntryDelay { .. } | Mode3StartupSourceState::FifoBacked => {}
        }
    }

    pub(super) fn consume_startup_pre_visible_transfer_dot(&mut self) {
        if self.startup_pre_visible_transfer_dots_remaining > 0 {
            self.startup_pre_visible_transfer_dots_remaining -= 1;
        }
    }

    pub(super) fn effective_fifo_is_empty(&self) -> bool {
        self.startup_fifo_placeholders == 0 && self.fifo.is_empty()
    }

    pub(super) fn fifo_contains_real_pixels(&self) -> bool {
        self.fifo.len() > self.startup_fifo_placeholders as usize
    }

    pub(super) fn consume_effective_fifo_pixel(&mut self) -> Option<u8> {
        if self.startup_fifo_placeholders > 0 {
            self.startup_fifo_placeholders -= 1;
            self.pop_fifo_pixel().map(BgFifoPixel::color).or(Some(0))
        } else {
            self.pop_real_fifo_pixel()
        }
    }

    pub(super) fn pop_real_fifo_pixel(&mut self) -> Option<u8> {
        self.pop_fifo_pixel().map(BgFifoPixel::color)
    }

    pub(super) fn pop_fifo_pixel(&mut self) -> Option<BgFifoPixel> {
        self.fifo.pop_front_pixel()
    }

    pub(super) fn pop_visible_fifo_pixel(&mut self) -> Option<BgFifoPixel> {
        if self.startup_fifo_placeholders == 1
            && self.fifo.len() > self.startup_fifo_placeholders as usize
            && matches!(self.fifo.cached_front(), Some(None))
        {
            self.startup_fifo_placeholders -= 1;
            let _ = self.pop_fifo_pixel();
        }

        self.pop_fifo_pixel()
    }

    pub(super) fn mark_live_lcdc3_write_while_fifo_visible(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
        current_fetcher: BgFetcherState,
        window_tile_row: u8,
        cgb_dmg_software_window_map_lead_in: bool,
    ) {
        let first_window_pixel_index = self
            .fifo
            .cached_pixels()
            .find(|cached| cached.cached.source == PpuBgFetcherSource::Window)
            .map(|cached| cached.pixel_index);
        let allow_current_window_tail_repaint = window_tile_row >= 24
            || cgb_dmg_software_window_map_lead_in
                && write_context.bgwin_tilemap_select_changed(PpuBgFetcherSource::Window);
        let mut mark_window_entries = current_fetcher.source == PpuBgFetcherSource::Window
            && current_fetcher.stage == PpuBgFetcherStage::TileIndex;
        if !mark_window_entries {
            mark_window_entries = if allow_current_window_tail_repaint {
                first_window_pixel_index.is_some()
            } else {
                first_window_pixel_index == Some(0)
            };
        }
        let preserved_current_window_tail_pixel_index = preserved_window_current_tail_pixel_index(
            current_fetcher,
            first_window_pixel_index,
            window_tile_row,
        );
        let mut previous_window_pixel_index = first_window_pixel_index;
        let mut crossed_window_tile_boundary = false;

        for cached in self.fifo.cached_pixels_mut() {
            if cached.cached.source == PpuBgFetcherSource::Window {
                cached
                    .cached
                    .latch_window_activation_previous_tilemap_select_if_unset(write_context);
                if let Some(previous_pixel_index) = previous_window_pixel_index
                    && cached.pixel_index < previous_pixel_index
                {
                    mark_window_entries = true;
                    crossed_window_tile_boundary = true;
                }
                previous_window_pixel_index = Some(cached.pixel_index);
                if !mark_window_entries {
                    continue;
                }
                if !crossed_window_tile_boundary
                    && preserved_current_window_tail_pixel_index == Some(cached.pixel_index)
                {
                    continue;
                }
                cached.cached.needs_live_tilemap_refetch |=
                    write_context.bgwin_tilemap_select_changed(PpuBgFetcherSource::Window);
                cached.cached.needs_live_tile_data_refetch |= crossed_window_tile_boundary
                    && write_context.bg_window_tile_data_select_changed();
                continue;
            }

            cached
                .cached
                .mark_live_lcdc3_write_while_fifo_visible(write_context);
        }
    }

    pub(super) fn latch_window_activation_tilemap_select_if_unset(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) {
        if self.window_activation_tilemap_select_latch.is_some()
            || !write_context.bgwin_tilemap_select_changed(PpuBgFetcherSource::Window)
        {
            return;
        }

        let seam_still_open = self.window_started_this_line
            && self.fetcher.source == PpuBgFetcherSource::Window
            && self.fetcher.fetch_x <= BG_TILE_WIDTH as u16 * 2;
        let fifo_contains_activation_tiles = self.fifo.cached_pixels().any(|cached| {
            cached.cached.source == PpuBgFetcherSource::Window
                && cached.cached.fetch_x <= BG_TILE_WIDTH as u16 * 2
        });
        let push_contains_activation_tiles = self.push.pending
            && self.push.cached.source == PpuBgFetcherSource::Window
            && self.push.cached.fetch_x <= BG_TILE_WIDTH as u16 * 2;
        let fill_contains_activation_tiles = self.fill.pending
            && self.fill.cached.source == PpuBgFetcherSource::Window
            && self.fill.cached.fetch_x <= BG_TILE_WIDTH as u16 * 2;

        if seam_still_open
            || fifo_contains_activation_tiles
            || push_contains_activation_tiles
            || fill_contains_activation_tiles
        {
            self.window_activation_tilemap_select_latch =
                Some(write_context.previous_lcdc() & LCDC_WINDOW_TILE_MAP_BIT != 0);
        }
    }

    pub(super) fn apply_window_activation_tilemap_select_latch_to_seam_slices(&mut self) {
        let Some(previous_tilemap_select) = self.window_activation_tilemap_select_latch else {
            return;
        };

        if self.fetcher.source == PpuBgFetcherSource::Window
            && self.fetcher.fetch_x <= BG_TILE_WIDTH as u16 * 2
            && self
                .fetcher
                .window_activation_first_pixel_previous_tilemap_select
                .is_none()
        {
            self.fetcher
                .window_activation_first_pixel_previous_tilemap_select =
                Some(previous_tilemap_select);
        }

        if self.push.pending {
            self.push
                .cached
                .force_window_activation_previous_tilemap_select(previous_tilemap_select);
        }
        if self.fill.pending {
            self.fill
                .cached
                .force_window_activation_previous_tilemap_select(previous_tilemap_select);
        }
        for cached in self.fifo.cached_pixels_mut() {
            cached
                .cached
                .force_window_activation_previous_tilemap_select(previous_tilemap_select);
        }
    }

    pub(super) fn apply_dmg_lcdc4_output_override_to_window_seam_slices(
        &mut self,
        previous_select: BgTileDataSelect,
    ) {
        self.apply_dmg_lcdc4_output_override_to_window_seam_slices_up_to(
            previous_select,
            BG_TILE_WIDTH as u16 * 2,
        );
    }

    pub(super) fn apply_dmg_lcdc4_output_override_to_window_seam_slices_up_to(
        &mut self,
        previous_select: BgTileDataSelect,
        max_fetch_x: u16,
    ) {
        if self.fetcher.source == PpuBgFetcherSource::Window
            && self.fetcher.fetch_x <= max_fetch_x
            && self
                .fetcher
                .dmg_lcdc4_previous_tiledata_select_for_output_override
                .is_none()
        {
            self.fetcher
                .dmg_lcdc4_previous_tiledata_select_for_output_override = Some(previous_select);
        }

        self.for_each_mut_cached_slice(|cached| {
            cached.force_dmg_lcdc4_previous_tiledata_select_for_output_override_up_to(
                previous_select,
                max_fetch_x,
            );
        });
    }

    fn for_each_mut_cached_slice(&mut self, mut f: impl FnMut(&mut BgCachedSlice)) {
        for cached in self.fifo.cached_pixels_mut() {
            f(&mut cached.cached);
        }
        f(&mut self.push.cached);
        f(&mut self.fill.cached);
    }

    fn for_each_mut_background_startup_continuation_slice(
        &mut self,
        slice: BgVisibleStartupSlice,
        mut f: impl FnMut(&mut BgCachedSlice),
    ) {
        self.for_each_mut_cached_slice(|cached| {
            if cached.matches_background_startup_continuation_slice(slice) {
                f(cached);
            }
        });
    }

    pub(super) fn take_next_dmg_lcdc3_current_line_bg_tilemap_write_index(&mut self) -> usize {
        let write_index = self
            .dmg_mode3_live_lcdc_bg_state
            .lcdc3_current_line_bg_tilemap_write_count as usize;
        self.dmg_mode3_live_lcdc_bg_state
            .lcdc3_current_line_bg_tilemap_write_count = self
            .dmg_mode3_live_lcdc_bg_state
            .lcdc3_current_line_bg_tilemap_write_count
            .saturating_add(1);
        write_index
    }

    pub(super) fn latch_dmg_lcdc3_startup_continuation_tilemap_select_override(
        &mut self,
        tilemap_select: bool,
        applies_to_visible_tile2: bool,
        applies_to_visible_tile3: bool,
    ) {
        self.dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides
            .latch_lcdc3_tilemap_select(
                tilemap_select,
                applies_to_visible_tile2,
                applies_to_visible_tile3,
            );

        if applies_to_visible_tile2 {
            self.for_each_mut_background_startup_continuation_slice(
                BgVisibleStartupSlice::VisibleTile2,
                |cached| cached.latch_dmg_lcdc3_tilemap_select_override(tilemap_select),
            );
        }
        if applies_to_visible_tile3 {
            self.for_each_mut_background_startup_continuation_slice(
                BgVisibleStartupSlice::VisibleTile3,
                |cached| cached.latch_dmg_lcdc3_tilemap_select_override(tilemap_select),
            );
        }
    }

    pub(super) fn maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_fill(
        &mut self,
    ) {
        let Some(tilemap_select) = self
            .dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides
            .lcdc3_tilemap_select_for_cached_slice(self.fill.cached)
        else {
            return;
        };

        self.fill
            .cached
            .latch_dmg_lcdc3_tilemap_select_override(tilemap_select);
    }

    pub(super) fn clear_dmg_lcdc3_startup_visible_tile2_live_refetch(&mut self) {
        self.dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides
            .clear_lcdc3_tilemap_select_for_slice(BgVisibleStartupSlice::VisibleTile2);
        self.for_each_mut_background_startup_continuation_slice(
            BgVisibleStartupSlice::VisibleTile2,
            |cached| {
                cached.needs_live_tilemap_refetch = false;
                cached.dmg_lcdc3_tilemap_select_override = None;
            },
        );
    }

    pub(super) fn maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_push(
        &mut self,
    ) {
        let Some(tilemap_select) = self
            .dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides
            .lcdc3_tilemap_select_for_cached_slice(self.push.cached)
        else {
            return;
        };

        self.push
            .cached
            .latch_dmg_lcdc3_tilemap_select_override(tilemap_select);
    }

    pub(super) fn latch_and_apply_dmg_lcdc4_startup_tiledata_select_override(
        &mut self,
        slice: BgVisibleStartupSlice,
        override_select: BgTileDataSelectOverride,
    ) {
        self.dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides
            .latch_lcdc4_tiledata_select(slice, override_select);

        let overrides = self
            .dmg_mode3_live_lcdc_bg_state
            .startup_continuation_overrides;
        self.for_each_mut_cached_slice(|cached| {
            apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_cached_slice(
                &mut *cached,
                overrides,
            );
        });
    }

    pub(super) fn maybe_apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_fill(
        &mut self,
    ) {
        apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_cached_slice(
            &mut self.fill.cached,
            self.dmg_mode3_live_lcdc_bg_state
                .startup_continuation_overrides,
        );
    }

    pub(super) fn maybe_apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_push(
        &mut self,
    ) {
        apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_cached_slice(
            &mut self.push.cached,
            self.dmg_mode3_live_lcdc_bg_state
                .startup_continuation_overrides,
        );
    }

    pub(super) fn mark_live_scy_write_while_startup_alignment_fifo_visible(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
        ly: u8,
    ) {
        if !write_context.bg_scy_tile_data_row_changed(ly) {
            return;
        }

        let has_unlatched_startup_alignment_pixel = self.fifo.cached_pixels().any(|cached| {
            matches!(
                cached.cached.origin,
                BgCachedSliceOrigin::StartupAlignmentFill
            ) && !cached.cached.needs_live_tile_data_refetch
        });
        if !has_unlatched_startup_alignment_pixel {
            return;
        }

        self.latch_startup_scy_tiledata_row(write_context, ly);
        let Some(latch) = self.startup_scy_tiledata_latch else {
            return;
        };

        for cached in self.fifo.cached_pixels_mut() {
            apply_startup_scy_tiledata_latch_to_cached(&mut cached.cached, latch);
        }
        apply_startup_scy_tiledata_latch_to_cached(&mut self.push.cached, latch);
        apply_startup_scy_tiledata_latch_to_cached(&mut self.fill.cached, latch);
    }

    pub(super) fn latch_startup_scy_tiledata_row(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
        ly: u8,
    ) {
        if self.startup_scy_tiledata_latch.is_some()
            || matches!(self.startup_fetch_seam, BgStartupFetchSeamState::Inactive)
            || !write_context.bg_scy_tile_data_row_changed(ly)
        {
            return;
        }

        self.startup_scy_tiledata_latch = Some(BgStartupScyTiledataLatch::new(
            write_context.current_lcdc(),
            write_context.current_scy_tile_data_row(ly),
        ));
    }

    pub(super) fn apply_startup_scy_tiledata_latch_to_fill(&mut self) {
        let Some(latch) = self.startup_scy_tiledata_latch else {
            return;
        };
        apply_startup_scy_tiledata_latch_to_cached(&mut self.fill.cached, latch);
    }

    pub(super) fn push_dummy_fifo_pixels(&mut self, count: u8) {
        self.fifo.extend(std::iter::repeat_n(0, count as usize));
    }

    #[cfg(test)]
    pub(super) fn push_cached_slice_fifo_pixels(&mut self, cached: BgCachedSlice) {
        self.push_cached_slice_fifo_pixels_with_skip(cached, 0);
    }

    pub(super) fn push_cached_slice_fifo_pixels_with_skip(
        &mut self,
        cached: BgCachedSlice,
        leading_pixel_skip: u8,
    ) {
        for pixel_index in leading_pixel_skip.min(BG_TILE_WIDTH)..BG_TILE_WIDTH {
            self.fifo.push_back_pixel(BgFifoPixel::new(
                cached.pixel_value(pixel_index),
                Some(BgFifoPixelCached::new(cached, pixel_index)),
            ));
        }
    }

    pub(super) fn apply_wx0_scx_shortening(&mut self) {
        if self.wx0_scx_shortening_applied || self.mode0_start_dot == 0 {
            return;
        }

        self.wx0_scx_shortening_applied = true;
        self.mode0_start_dot -= 1;
    }

    pub(super) fn peek_startup_background_fetch_origin(&self) -> BgCachedSliceOrigin {
        match self.startup_fetch_seam {
            BgStartupFetchSeamState::PostAlignment {
                next_startup_continuation_slice,
                startup_continuation_visible_tiles_remaining,
                ..
            } if startup_continuation_visible_tiles_remaining > 0 => {
                BgCachedSliceOrigin::from_startup_continuation_slice(
                    next_startup_continuation_slice,
                )
            }
            BgStartupFetchSeamState::Inactive
            | BgStartupFetchSeamState::AlignmentSeedPending
            | BgStartupFetchSeamState::PostAlignment { .. } => BgCachedSliceOrigin::Ordinary,
        }
    }

    pub(super) fn startup_alignment_seed_pending(&self) -> bool {
        matches!(
            self.startup_fetch_seam,
            BgStartupFetchSeamState::AlignmentSeedPending
        )
    }

    pub(super) fn startup_background_tilemap_uses_pipeline_snapshot(&self) -> bool {
        match self.startup_fetch_seam {
            BgStartupFetchSeamState::Inactive => false,
            BgStartupFetchSeamState::AlignmentSeedPending => true,
            BgStartupFetchSeamState::PostAlignment {
                delayed_background_tilemap_tiles_remaining,
                ..
            } => delayed_background_tilemap_tiles_remaining > 0,
        }
    }

    pub(super) fn startup_background_tiledata_uses_pipeline_snapshot(&self) -> bool {
        match self.startup_fetch_seam {
            BgStartupFetchSeamState::Inactive => false,
            BgStartupFetchSeamState::AlignmentSeedPending => true,
            BgStartupFetchSeamState::PostAlignment {
                delayed_background_tileindex_read_tiles_remaining: _,
                delayed_background_tiledata_tiles_remaining,
                ..
            } => delayed_background_tiledata_tiles_remaining > 0,
        }
    }

    pub(super) fn startup_background_tileindex_reads_on_stage_one(&self) -> bool {
        match self.startup_fetch_seam {
            BgStartupFetchSeamState::Inactive | BgStartupFetchSeamState::AlignmentSeedPending => {
                false
            }
            BgStartupFetchSeamState::PostAlignment {
                delayed_background_tileindex_read_tiles_remaining,
                ..
            } => delayed_background_tileindex_read_tiles_remaining > 0,
        }
    }

    pub(super) fn begin_post_alignment_followup(&mut self) {
        self.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: true,
            next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        };
    }

    pub(super) fn take_startup_first_real_push_skip_entry_delay(&mut self) -> bool {
        let skip_entry_delay = match &mut self.startup_fetch_seam {
            BgStartupFetchSeamState::PostAlignment {
                first_real_push_skips_entry_delay,
                ..
            } => {
                let skip = *first_real_push_skips_entry_delay;
                *first_real_push_skips_entry_delay = false;
                skip
            }
            BgStartupFetchSeamState::Inactive | BgStartupFetchSeamState::AlignmentSeedPending => {
                false
            }
        };
        self.maybe_finish_startup_fetch_seam();
        skip_entry_delay
    }

    pub(super) fn advance_startup_background_fetch_tile(&mut self) {
        if let BgStartupFetchSeamState::PostAlignment {
            next_startup_continuation_slice,
            startup_continuation_visible_tiles_remaining,
            delayed_background_tileindex_read_tiles_remaining,
            delayed_background_tilemap_tiles_remaining,
            delayed_background_tiledata_tiles_remaining,
            ..
        } = &mut self.startup_fetch_seam
        {
            if *startup_continuation_visible_tiles_remaining > 0 {
                *next_startup_continuation_slice = next_startup_continuation_slice.next();
                *startup_continuation_visible_tiles_remaining -= 1;
            }
            if *delayed_background_tileindex_read_tiles_remaining > 0 {
                *delayed_background_tileindex_read_tiles_remaining -= 1;
            }
            if *delayed_background_tilemap_tiles_remaining > 0 {
                *delayed_background_tilemap_tiles_remaining -= 1;
            }
            if *delayed_background_tiledata_tiles_remaining > 0 {
                *delayed_background_tiledata_tiles_remaining -= 1;
            }
        }
        self.maybe_finish_startup_fetch_seam();
    }

    pub(super) fn maybe_finish_startup_fetch_seam(&mut self) {
        if let BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: false,
            next_startup_continuation_slice: _,
            startup_continuation_visible_tiles_remaining: 0,
            delayed_background_tileindex_read_tiles_remaining: 0,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 0,
        } = self.startup_fetch_seam
        {
            self.startup_fetch_seam = BgStartupFetchSeamState::Inactive;
        }
    }

    pub(super) fn maybe_attach_startup_visible_tile3_scx_boundary_next_slice_to_fetcher(&mut self) {
        if self.fetcher.source != PpuBgFetcherSource::Background
            || self.fetcher.cached_origin != BgCachedSliceOrigin::Ordinary
        {
            return;
        }

        let Some(previous_scx) = self
            .startup_visible_tile3_scx_boundary_next_slice_previous_scx
            .take()
        else {
            return;
        };
        let prefix_pixels = self.startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels;
        self.startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels = 0;

        self.fetcher
            .arm_startup_visible_tile3_scx_boundary_old_prefix(previous_scx, prefix_pixels);
    }
}

fn preserved_window_current_tail_pixel_index(
    current_fetcher: BgFetcherState,
    first_window_pixel_index: Option<u8>,
    window_tile_row: u8,
) -> Option<u8> {
    if current_fetcher.source != PpuBgFetcherSource::Window
        || current_fetcher.stage != PpuBgFetcherStage::TileIndex
        || !matches!(first_window_pixel_index, Some(1..=7))
    {
        return None;
    }

    match window_tile_row & 0x07 {
        0 | 7 => None,
        1 | 6 => Some(1),
        2..=5 => Some(2),
        _ => None,
    }
}

fn apply_startup_scy_tiledata_latch_to_cached(
    cached: &mut BgCachedSlice,
    latch: BgStartupScyTiledataLatch,
) {
    if cached.source != PpuBgFetcherSource::Background
        || !matches!(cached.origin, BgCachedSliceOrigin::StartupAlignmentFill)
        || cached.needs_live_tile_data_refetch
    {
        return;
    }

    let tile_data_base = bg_tile_data_base(latch.lcdc, cached.tile_index);
    cached.tile_low_address = tile_data_base + latch.tile_data_row * TILE_ROW_BYTES;
    cached.tile_high_address = tile_data_base + latch.tile_data_row * TILE_ROW_BYTES + 1;
    cached.needs_live_tile_data_refetch = true;
}

fn apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_cached_slice(
    cached: &mut BgCachedSlice,
    overrides: DmgStartupContinuationOverrides,
) {
    cached.latch_dmg_lcdc4_tiledata_select_override(overrides.for_cached_slice(*cached));
}

impl Default for BgPipelineState {
    fn default() -> Self {
        Self {
            fetcher: BgFetcherState::default(),
            push: BgPushState::default(),
            fill: BgFifoFillState::default(),
            fifo: BgFifo::default(),
            startup_fetch_seam: BgStartupFetchSeamState::Inactive,
            startup_fifo_placeholders: 0,
            mode3_started: false,
            initial_scx_capture_pending: false,
            mode0_start_dot: MODE0_START_DOT,
            initial_scx_discard: 0,
            scx_discard_remaining: 0,
            startup_source_state: Mode3StartupSourceState::FifoBacked,
            startup_pre_visible_transfer_dots_remaining: MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS,
            transfer_phase: Mode3TransferPhase::Priming,
            current_transfer_x: 0,
            visible_pixels_output: 0,
            saw_right_edge_visible_same_x_cluster_this_line: false,
            window_wy_latch: false,
            window_lcdc5_latch: false,
            window_force_x0_this_line: false,
            window_started_this_line: false,
            window_active_line_counter: 0,
            window_start_count_this_line: 0,
            wx0_scx_shortening_applied: false,
            wx166_armed_this_line: false,
            startup_visible_tile3_scx_boundary_next_slice_previous_scx: None,
            startup_visible_tile3_scx_boundary_next_slice_old_prefix_pixels: 0,
            startup_scy_tiledata_latch: None,
            cgb_dmg_scy_startup_retarget_active: false,
            window_activation_tilemap_select_latch: None,
            dmg_wx0_window_disable_prefix_state: None,
            dmg_late_window_enable_override: None,
            dmg_window_restart: DmgWindowRestartState::default(),
            dmg_mode3_live_lcdc_bg_state: DmgMode3LiveLcdcBgState::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) enum BgStartupFetchSeamState {
    #[default]
    Inactive,
    AlignmentSeedPending,
    PostAlignment {
        first_real_push_skips_entry_delay: bool,
        next_startup_continuation_slice: BgStartupContinuationSlice,
        startup_continuation_visible_tiles_remaining: u8,
        delayed_background_tileindex_read_tiles_remaining: u8,
        delayed_background_tilemap_tiles_remaining: u8,
        delayed_background_tiledata_tiles_remaining: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) enum BgStartupContinuationSlice {
    #[default]
    None,
    VisibleTile2,
    VisibleTile3,
}

impl BgStartupContinuationSlice {
    pub(super) const fn next(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::VisibleTile2 => Self::VisibleTile3,
            Self::VisibleTile3 => Self::None,
        }
    }

    pub(super) const fn visible_slice(self) -> Option<BgVisibleStartupSlice> {
        match self {
            Self::None => None,
            Self::VisibleTile2 => Some(BgVisibleStartupSlice::VisibleTile2),
            Self::VisibleTile3 => Some(BgVisibleStartupSlice::VisibleTile3),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct BgFetcherState {
    pub(super) source: PpuBgFetcherSource,
    pub(super) stage: PpuBgFetcherStage,
    pub(super) stage_dot: u8,
    pub(super) cached_origin: BgCachedSliceOrigin,
    pub(super) window_activation_first_pixel_previous_tilemap_select: Option<bool>,
    pub(super) same_cycle_window_tilemap_lcdc_hold: bool,
    pub(super) dmg_lcdc4_previous_tiledata_select_on_next_low: Option<BgTileDataSelect>,
    pub(super) dmg_lcdc4_previous_tiledata_select_for_output_override: Option<BgTileDataSelect>,
    pub(super) dmg_lcdc4_skip_window_current_low_glitch: bool,
    pub(super) needs_live_tilemap_refetch_on_push: bool,
    pub(super) needs_live_tilemap_full_refetch_on_push: bool,
    pub(super) needs_live_tile_data_refetch_on_push: bool,
    pub(super) needs_live_tile_data_current_row_refetch_on_push: bool,
    pub(super) needs_live_tile_low_current_row_refetch_on_push: bool,
    pub(super) needs_live_tile_high_current_row_refetch_on_push: bool,
    pub(super) cgb_dmg_scy_high_plane_uses_low_row: bool,
    pub(super) startup_visible_tile3_scx_boundary_full_refetch_next_tile: bool,
    pub(super) startup_visible_tile3_scx_boundary_previous_scx: Option<u8>,
    pub(super) startup_visible_tile3_scx_boundary_old_tail_start_pixel: u8,
    pub(super) startup_visible_tile3_scx_boundary_old_prefix_pixels: u8,
    pub(super) fetch_x: u16,
    pub(super) next_fetch_pixel: u16,
    pub(super) post_alignment_fetch_restart_delay_dots: u8,
    pub(super) window_tilemap_x: u8,
    pub(super) bg_resume_fetch_pixel: u16,
    pub(super) rewind_bg_resume_after_first_tile_index_dot: bool,
    pub(super) first_window_tile_after_activation: bool,
    pub(super) first_window_tile_leading_pixel_skip: u8,
    pub(super) tile_map_address: u16,
    pub(super) tile_data_address: u16,
    pub(super) tile_low_address: u16,
    pub(super) tile_high_address: u16,
    pub(super) tile_index: u8,
    pub(super) cgb_bg_attrs: Option<CgbBgTileAttributes>,
    pub(super) tile_low: u8,
    pub(super) tile_high: u8,
}

impl BgFetcherState {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn start_background(&mut self) {
        self.source = PpuBgFetcherSource::Background;
        self.start_common(0);
    }

    pub(super) fn start_window(&mut self, bg_resume_fetch_pixel: u16) {
        self.start_window_with_pixel_offset(bg_resume_fetch_pixel, 0);
    }

    pub(super) fn start_window_with_pixel_offset(
        &mut self,
        bg_resume_fetch_pixel: u16,
        window_pixel_offset: u16,
    ) {
        let tile_width = u16::from(BG_TILE_WIDTH);
        let aligned_fetch_x = (window_pixel_offset / tile_width) * tile_width;
        self.source = PpuBgFetcherSource::Window;
        self.stage = PpuBgFetcherStage::WindowActivating;
        self.stage_dot = 0;
        self.cached_origin = BgCachedSliceOrigin::Ordinary;
        self.window_activation_first_pixel_previous_tilemap_select = None;
        self.same_cycle_window_tilemap_lcdc_hold = false;
        self.dmg_lcdc4_previous_tiledata_select_on_next_low = None;
        self.dmg_lcdc4_previous_tiledata_select_for_output_override = None;
        self.dmg_lcdc4_skip_window_current_low_glitch = false;
        self.needs_live_tilemap_refetch_on_push = false;
        self.needs_live_tilemap_full_refetch_on_push = false;
        self.needs_live_tile_data_refetch_on_push = false;
        self.needs_live_tile_data_current_row_refetch_on_push = false;
        self.needs_live_tile_low_current_row_refetch_on_push = false;
        self.needs_live_tile_high_current_row_refetch_on_push = false;
        self.cgb_dmg_scy_high_plane_uses_low_row = false;
        self.startup_visible_tile3_scx_boundary_full_refetch_next_tile = false;
        self.clear_startup_visible_tile3_scx_boundary_old_pixel_window();
        self.fetch_x = aligned_fetch_x;
        self.next_fetch_pixel = aligned_fetch_x;
        self.post_alignment_fetch_restart_delay_dots = 0;
        self.window_tilemap_x = (window_pixel_offset / tile_width) as u8;
        self.bg_resume_fetch_pixel = bg_resume_fetch_pixel;
        self.rewind_bg_resume_after_first_tile_index_dot = true;
        self.first_window_tile_after_activation = true;
        self.first_window_tile_leading_pixel_skip = (window_pixel_offset % tile_width) as u8;
        self.tile_map_address = 0;
        self.tile_data_address = 0;
        self.tile_low_address = 0;
        self.tile_high_address = 0;
        self.tile_index = 0;
        self.cgb_bg_attrs = None;
        self.tile_low = 0;
        self.tile_high = 0;
    }

    pub(super) fn start_common(&mut self, bg_resume_fetch_pixel: u16) {
        self.stage = PpuBgFetcherStage::TileIndex;
        self.stage_dot = 0;
        self.cached_origin = BgCachedSliceOrigin::Ordinary;
        self.window_activation_first_pixel_previous_tilemap_select = None;
        self.same_cycle_window_tilemap_lcdc_hold = false;
        self.dmg_lcdc4_previous_tiledata_select_on_next_low = None;
        self.dmg_lcdc4_previous_tiledata_select_for_output_override = None;
        self.dmg_lcdc4_skip_window_current_low_glitch = false;
        self.needs_live_tilemap_refetch_on_push = false;
        self.needs_live_tilemap_full_refetch_on_push = false;
        self.needs_live_tile_data_refetch_on_push = false;
        self.needs_live_tile_data_current_row_refetch_on_push = false;
        self.needs_live_tile_low_current_row_refetch_on_push = false;
        self.needs_live_tile_high_current_row_refetch_on_push = false;
        self.cgb_dmg_scy_high_plane_uses_low_row = false;
        self.startup_visible_tile3_scx_boundary_full_refetch_next_tile = false;
        self.clear_startup_visible_tile3_scx_boundary_old_pixel_window();
        self.fetch_x = 0;
        self.next_fetch_pixel = 0;
        self.post_alignment_fetch_restart_delay_dots = 0;
        self.window_tilemap_x = 0;
        self.bg_resume_fetch_pixel = bg_resume_fetch_pixel;
        self.rewind_bg_resume_after_first_tile_index_dot = false;
        self.first_window_tile_after_activation = false;
        self.first_window_tile_leading_pixel_skip = 0;
        self.tile_map_address = 0;
        self.tile_data_address = 0;
        self.tile_low_address = 0;
        self.tile_high_address = 0;
        self.tile_index = 0;
        self.cgb_bg_attrs = None;
        self.tile_low = 0;
        self.tile_high = 0;
    }

    pub(super) fn abort_window_to_background(&mut self) {
        if self.source != PpuBgFetcherSource::Window {
            return;
        }

        self.source = PpuBgFetcherSource::Background;
        self.cached_origin = BgCachedSliceOrigin::Ordinary;
        self.window_activation_first_pixel_previous_tilemap_select = None;
        self.same_cycle_window_tilemap_lcdc_hold = false;
        self.dmg_lcdc4_previous_tiledata_select_on_next_low = None;
        self.dmg_lcdc4_previous_tiledata_select_for_output_override = None;
        self.dmg_lcdc4_skip_window_current_low_glitch = false;
        self.needs_live_tilemap_refetch_on_push = false;
        self.needs_live_tilemap_full_refetch_on_push = false;
        self.needs_live_tile_data_refetch_on_push = false;
        self.needs_live_tile_data_current_row_refetch_on_push = false;
        self.needs_live_tile_low_current_row_refetch_on_push = false;
        self.needs_live_tile_high_current_row_refetch_on_push = false;
        self.cgb_dmg_scy_high_plane_uses_low_row = false;
        self.startup_visible_tile3_scx_boundary_full_refetch_next_tile = false;
        self.clear_startup_visible_tile3_scx_boundary_old_pixel_window();
        self.fetch_x = self.bg_resume_fetch_pixel;
        self.next_fetch_pixel = self.bg_resume_fetch_pixel;
        self.post_alignment_fetch_restart_delay_dots = 0;
        self.window_tilemap_x = 0;
        self.first_window_tile_after_activation = false;
        self.first_window_tile_leading_pixel_skip = 0;
        self.cgb_bg_attrs = None;
    }

    pub(super) fn mark_live_register_write_for_current_background_fetch(
        &mut self,
        register: PpuMode3LiveBackgroundRegister,
        write_context: PpuMode3LiveRegisterWriteContext,
        ly: u8,
        window_tile_row: u8,
        scy_routing: PpuMode3LiveScyWriteRouting,
    ) {
        if self.source == PpuBgFetcherSource::Window
            && self.first_window_tile_after_activation
            && matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bgwin_tilemap_select_changed(PpuBgFetcherSource::Window)
        {
            self.window_activation_first_pixel_previous_tilemap_select =
                Some(write_context.previous_lcdc() & LCDC_WINDOW_TILE_MAP_BIT != 0);
        }

        if self.source == PpuBgFetcherSource::Window
            && !self.first_window_tile_after_activation
            && self.stage == PpuBgFetcherStage::TileIndex
            && self.stage_dot == 0
            && matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bgwin_tilemap_select_changed(PpuBgFetcherSource::Window)
        {
            self.same_cycle_window_tilemap_lcdc_hold = true;
        }

        if self.source == PpuBgFetcherSource::Window
            && self.stage == PpuBgFetcherStage::TileIndex
            && self.stage_dot == 1
            && matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bg_window_tile_data_select_changed()
            && window_tile_row >= 24
            && write_context.previous_lcdc() & LCDC_BG_WINDOW_TILE_DATA_BIT != 0
        {
            self.dmg_lcdc4_previous_tiledata_select_on_next_low =
                Some(BgTileDataSelect::Unsigned8000);
        }

        if self.source == PpuBgFetcherSource::Window
            && matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bg_window_tile_data_select_changed()
            && window_tile_row >= 24
            && write_context.previous_lcdc() & LCDC_BG_WINDOW_TILE_DATA_BIT != 0
            && write_context.current_lcdc() & LCDC_BG_WINDOW_TILE_DATA_BIT == 0
        {
            self.dmg_lcdc4_previous_tiledata_select_for_output_override =
                Some(BgTileDataSelect::Unsigned8000);
        }

        if self.source == PpuBgFetcherSource::Window
            && self.stage == PpuBgFetcherStage::TileDataLow
            && self.stage_dot == 1
            && matches!(register, PpuMode3LiveBackgroundRegister::Lcdc)
            && write_context.bg_window_tile_data_select_changed()
            && window_tile_row >= 24
            && write_context.previous_lcdc() & LCDC_BG_WINDOW_TILE_DATA_BIT != 0
        {
            self.dmg_lcdc4_skip_window_current_low_glitch = true;
        }

        PpuMode3LiveBackgroundWriteEffects::for_current_background_fetch(
            *self,
            register,
            write_context,
            ly,
            window_tile_row,
            scy_routing,
        )
        .apply_to_fetcher(self);
    }

    pub(super) fn clear_startup_visible_tile3_scx_boundary_old_pixel_window(&mut self) {
        self.startup_visible_tile3_scx_boundary_previous_scx = None;
        self.startup_visible_tile3_scx_boundary_old_tail_start_pixel = BG_TILE_WIDTH;
        self.startup_visible_tile3_scx_boundary_old_prefix_pixels = 0;
    }

    pub(super) fn arm_startup_visible_tile3_scx_boundary_old_prefix(
        &mut self,
        previous_scx: u8,
        prefix_pixels: u8,
    ) {
        if prefix_pixels == 0 {
            return;
        }

        self.startup_visible_tile3_scx_boundary_previous_scx = Some(previous_scx);
        self.startup_visible_tile3_scx_boundary_old_tail_start_pixel = BG_TILE_WIDTH;
        self.startup_visible_tile3_scx_boundary_old_prefix_pixels = prefix_pixels;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct BgPushState {
    pub(super) pending: bool,
    pub(super) disposition: BgPushDisposition,
    pub(super) entry_delay_remaining: u8,
    pub(super) terminal_placeholder_tail_extra_hold_remaining: u8,
    pub(super) just_activated_window_tile: bool,
    pub(super) leading_pixel_skip: u8,
    pub(super) next_fetch_pixel: u16,
    pub(super) cached: BgCachedSlice,
}

impl BgPushState {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn queue_from_fetcher(&mut self, fetcher: BgFetcherState) {
        self.pending = true;
        self.disposition = BgPushDisposition::Ready;
        self.terminal_placeholder_tail_extra_hold_remaining = 0;
        self.just_activated_window_tile = fetcher.first_window_tile_after_activation;
        self.leading_pixel_skip = if self.just_activated_window_tile {
            fetcher.first_window_tile_leading_pixel_skip
        } else {
            0
        };
        self.entry_delay_remaining = if self.just_activated_window_tile {
            0
        } else {
            1
        };
        self.next_fetch_pixel = fetcher.fetch_x.wrapping_add(BG_TILE_WIDTH as u16);
        self.cached = BgCachedSlice::from_fetcher(fetcher);
    }

    pub(super) fn queue_startup_alignment_seed_from_fetcher(&mut self, fetcher: BgFetcherState) {
        self.pending = true;
        self.disposition = BgPushDisposition::Ready;
        self.terminal_placeholder_tail_extra_hold_remaining = 0;
        self.just_activated_window_tile = fetcher.first_window_tile_after_activation;
        self.leading_pixel_skip = if self.just_activated_window_tile {
            fetcher.first_window_tile_leading_pixel_skip
        } else {
            0
        };
        self.entry_delay_remaining = 0;
        self.next_fetch_pixel = fetcher.fetch_x.wrapping_add(BG_TILE_WIDTH as u16);
        self.cached = BgCachedSlice::from_fetcher(fetcher)
            .with_origin(BgCachedSliceOrigin::StartupAlignmentSeed);
    }

    pub(super) fn interrupt_for_object_fetch(&mut self) {
        if !self.pending {
            return;
        }

        self.disposition = BgPushDisposition::InterruptedByObjectFetch;
    }

    pub(super) fn resume_after_object_fetch(&mut self) {
        if self.pending && self.disposition == BgPushDisposition::InterruptedByObjectFetch {
            self.disposition = BgPushDisposition::Ready;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct BgFifoFillState {
    pub(super) pending: bool,
    pub(super) startup_dummy_pixels: u8,
    pub(super) leading_pixel_skip: u8,
    pub(super) includes_real_tile_pixels: bool,
    pub(super) cached: BgCachedSlice,
}

impl BgFifoFillState {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn queue_from_push(&mut self, push: BgPushState) {
        self.pending = true;
        self.startup_dummy_pixels = 0;
        self.leading_pixel_skip = push.leading_pixel_skip;
        self.includes_real_tile_pixels = true;
        self.cached = push.cached;
    }

    pub(super) fn queue_startup_alignment_from_push(
        &mut self,
        push: BgPushState,
        startup_dummy_pixels: u8,
    ) {
        self.pending = true;
        self.startup_dummy_pixels = startup_dummy_pixels;
        self.leading_pixel_skip = push.leading_pixel_skip;
        self.includes_real_tile_pixels = true;
        self.cached = push.cached.with_origin(push.cached.queued_fill_origin());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) enum BgCachedSliceOrigin {
    #[default]
    Ordinary,
    StartupAlignmentSeed,
    StartupAlignmentFill,
    StartupContinuation(BgStartupContinuationSlice),
}

impl BgCachedSliceOrigin {
    pub(super) const fn from_startup_continuation_slice(slice: BgStartupContinuationSlice) -> Self {
        match slice {
            BgStartupContinuationSlice::None => Self::Ordinary,
            slice => Self::StartupContinuation(slice),
        }
    }

    pub(super) const fn startup_continuation_slice(self) -> BgStartupContinuationSlice {
        match self {
            Self::StartupContinuation(slice) => slice,
            Self::Ordinary | Self::StartupAlignmentSeed | Self::StartupAlignmentFill => {
                BgStartupContinuationSlice::None
            }
        }
    }

    pub(super) const fn visible_startup_continuation_slice(self) -> Option<BgVisibleStartupSlice> {
        self.startup_continuation_slice().visible_slice()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct BgCachedSlice {
    pub(super) source: PpuBgFetcherSource,
    pub(super) origin: BgCachedSliceOrigin,
    pub(super) fetch_x: u16,
    pub(super) dmg_lcdc3_tilemap_select_override: Option<bool>,
    pub(super) dmg_lcdc4_tiledata_select_override: BgTileDataSelectOverride,
    pub(super) dmg_lcdc4_previous_tiledata_select_for_output_override: Option<BgTileDataSelect>,
    pub(super) window_activation_first_pixel_previous_tilemap_select: Option<bool>,
    pub(super) same_cycle_live_tilemap_refetch_window_open: bool,
    pub(super) startup_visible_tile3_scx_boundary_full_refetch_next_tile: bool,
    pub(super) startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx: Option<u8>,
    pub(super) startup_visible_tile3_scx_boundary_previous_scx: Option<u8>,
    pub(super) startup_visible_tile3_scx_boundary_old_tail_start_pixel: u8,
    pub(super) startup_visible_tile3_scx_boundary_old_prefix_pixels: u8,
    pub(super) needs_live_tilemap_refetch: bool,
    pub(super) needs_live_tilemap_full_refetch: bool,
    pub(super) needs_live_tile_data_refetch: bool,
    pub(super) needs_live_tile_data_current_row_refetch: bool,
    pub(super) needs_live_tile_low_current_row_refetch: bool,
    pub(super) needs_live_tile_high_current_row_refetch: bool,
    pub(super) needs_live_tile_data_unsigned_reuse: bool,
    #[serde(default)]
    pub(super) cgb_lcdc4_same_cycle_tile_high_override: Option<u8>,
    pub(super) tile_map_address: u16,
    pub(super) tile_data_address: u16,
    pub(super) tile_low_address: u16,
    pub(super) tile_high_address: u16,
    pub(super) tile_index: u8,
    pub(super) cgb_bg_attrs: Option<CgbBgTileAttributes>,
    pub(super) tile_low: u8,
    pub(super) tile_high: u8,
}

impl BgCachedSlice {
    pub(super) fn from_fetcher(fetcher: BgFetcherState) -> Self {
        Self {
            source: fetcher.source,
            origin: fetcher.cached_origin,
            fetch_x: fetcher.fetch_x,
            dmg_lcdc3_tilemap_select_override: None,
            dmg_lcdc4_tiledata_select_override: PerPlane::new(None, None),
            dmg_lcdc4_previous_tiledata_select_for_output_override: fetcher
                .dmg_lcdc4_previous_tiledata_select_for_output_override,
            window_activation_first_pixel_previous_tilemap_select: fetcher
                .window_activation_first_pixel_previous_tilemap_select,
            same_cycle_live_tilemap_refetch_window_open: false,
            startup_visible_tile3_scx_boundary_full_refetch_next_tile: fetcher
                .startup_visible_tile3_scx_boundary_full_refetch_next_tile,
            startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx: None,
            startup_visible_tile3_scx_boundary_previous_scx: fetcher
                .startup_visible_tile3_scx_boundary_previous_scx,
            startup_visible_tile3_scx_boundary_old_tail_start_pixel: fetcher
                .startup_visible_tile3_scx_boundary_old_tail_start_pixel,
            startup_visible_tile3_scx_boundary_old_prefix_pixels: fetcher
                .startup_visible_tile3_scx_boundary_old_prefix_pixels,
            needs_live_tilemap_refetch: fetcher.needs_live_tilemap_refetch_on_push,
            needs_live_tilemap_full_refetch: fetcher.needs_live_tilemap_full_refetch_on_push,
            needs_live_tile_data_refetch: fetcher.needs_live_tile_data_refetch_on_push,
            needs_live_tile_data_current_row_refetch: fetcher
                .needs_live_tile_data_current_row_refetch_on_push,
            needs_live_tile_low_current_row_refetch: fetcher
                .needs_live_tile_low_current_row_refetch_on_push,
            needs_live_tile_high_current_row_refetch: fetcher
                .needs_live_tile_high_current_row_refetch_on_push,
            needs_live_tile_data_unsigned_reuse: false,
            cgb_lcdc4_same_cycle_tile_high_override: None,
            tile_map_address: fetcher.tile_map_address,
            tile_data_address: fetcher.tile_data_address,
            tile_low_address: fetcher.tile_low_address,
            tile_high_address: fetcher.tile_high_address,
            tile_index: fetcher.tile_index,
            cgb_bg_attrs: fetcher.cgb_bg_attrs,
            tile_low: fetcher.tile_low,
            tile_high: fetcher.tile_high,
        }
    }

    pub(super) fn with_origin(mut self, origin: BgCachedSliceOrigin) -> Self {
        self.origin = origin;
        self
    }

    pub(super) fn latch_dmg_lcdc3_tilemap_select_override(&mut self, tilemap_select: bool) {
        self.dmg_lcdc3_tilemap_select_override = Some(tilemap_select);
        self.needs_live_tilemap_refetch = true;
    }

    pub(super) fn latch_dmg_lcdc4_tiledata_select_override(
        &mut self,
        override_select: BgTileDataSelectOverride,
    ) {
        if !matches!(
            self.origin,
            BgCachedSliceOrigin::StartupContinuation(
                BgStartupContinuationSlice::VisibleTile2 | BgStartupContinuationSlice::VisibleTile3
            )
        ) || !self.is_background()
        {
            return;
        }

        self.dmg_lcdc4_tiledata_select_override = override_select;
        if override_select.low.is_some() || override_select.high.is_some() {
            self.needs_live_tile_data_refetch = true;
        }
    }

    pub(super) const fn is_background(self) -> bool {
        matches!(self.source, PpuBgFetcherSource::Background)
    }

    pub(super) const fn matches_background_startup_continuation_slice(
        self,
        slice: BgVisibleStartupSlice,
    ) -> bool {
        self.is_background()
            && matches!(
                (self.origin.visible_startup_continuation_slice(), slice),
                (
                    Some(BgVisibleStartupSlice::VisibleTile2),
                    BgVisibleStartupSlice::VisibleTile2,
                ) | (
                    Some(BgVisibleStartupSlice::VisibleTile3),
                    BgVisibleStartupSlice::VisibleTile3,
                )
            )
    }

    pub(super) const fn is_startup_alignment_seed(self) -> bool {
        matches!(self.origin, BgCachedSliceOrigin::StartupAlignmentSeed)
    }

    pub(super) const fn cgb_bg_attrs_or_default(self) -> CgbBgTileAttributes {
        match self.cgb_bg_attrs {
            Some(attrs) => attrs,
            None => CgbBgTileAttributes::new(0),
        }
    }

    pub(super) const fn pixel_value(self, pixel_index: u8) -> u8 {
        bg_tile_pixel_value_with_cgb_attrs(
            self.tile_low,
            self.tile_high,
            pixel_index,
            self.cgb_bg_attrs_or_default(),
        )
    }

    pub(super) const fn queued_fill_origin(self) -> BgCachedSliceOrigin {
        match self.origin {
            BgCachedSliceOrigin::StartupAlignmentSeed => BgCachedSliceOrigin::StartupAlignmentFill,
            origin => origin,
        }
    }

    pub(super) const fn startup_continuation_slice(self) -> BgStartupContinuationSlice {
        self.origin.startup_continuation_slice()
    }

    pub(super) const fn visible_startup_continuation_slice(self) -> Option<BgVisibleStartupSlice> {
        self.origin.visible_startup_continuation_slice()
    }

    pub(super) const fn is_second_or_third_visible_post_startup_push(self) -> bool {
        matches!(
            (self.startup_continuation_slice(), self.fetch_x),
            (BgStartupContinuationSlice::VisibleTile2, x) if x == BG_TILE_WIDTH as u16
        ) || matches!(
            (self.startup_continuation_slice(), self.fetch_x),
            (BgStartupContinuationSlice::VisibleTile3, x) if x == BG_TILE_WIDTH as u16 * 2
        )
    }

    pub(super) fn mark_live_register_write_while_push_pending(
        &mut self,
        register: PpuMode3LiveBackgroundRegister,
        write_context: PpuMode3LiveRegisterWriteContext,
        entry_delay_active: bool,
        ly: u8,
        scy_routing: PpuMode3LiveScyWriteRouting,
    ) {
        if matches!(register, PpuMode3LiveBackgroundRegister::Lcdc) {
            self.latch_window_activation_previous_tilemap_select_if_unset(write_context);
        }
        PpuMode3LiveBackgroundWriteEffects::for_push_pending_slice(
            *self,
            register,
            write_context,
            entry_delay_active,
            ly,
            scy_routing,
        )
        .apply_to_cached_slice(self);
    }

    pub(super) fn mark_live_register_write_while_fill_pending(
        &mut self,
        register: PpuMode3LiveBackgroundRegister,
        write_context: PpuMode3LiveRegisterWriteContext,
        includes_real_tile_pixels: bool,
        startup_dummy_pixels: u8,
        ly: u8,
        scy_routing: PpuMode3LiveScyWriteRouting,
    ) {
        if matches!(register, PpuMode3LiveBackgroundRegister::Lcdc) {
            self.latch_window_activation_previous_tilemap_select_if_unset(write_context);
        }
        PpuMode3LiveBackgroundWriteEffects::for_fill_pending_slice(
            *self,
            register,
            write_context,
            includes_real_tile_pixels,
            startup_dummy_pixels,
            ly,
            scy_routing,
        )
        .apply_to_cached_slice(self);
    }

    pub(super) fn mark_live_lcdc3_write_while_fifo_visible(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) {
        self.latch_window_activation_previous_tilemap_select_if_unset(write_context);
        PpuMode3LiveBackgroundWriteEffects::for_visible_fifo_slice(*self, write_context)
            .apply_to_cached_slice(self);
    }

    fn latch_window_activation_previous_tilemap_select_if_unset(
        &mut self,
        write_context: PpuMode3LiveRegisterWriteContext,
    ) {
        if self.source != PpuBgFetcherSource::Window
            || self.fetch_x != 0
            || self
                .window_activation_first_pixel_previous_tilemap_select
                .is_some()
            || !write_context.bgwin_tilemap_select_changed(PpuBgFetcherSource::Window)
        {
            return;
        }

        self.window_activation_first_pixel_previous_tilemap_select =
            Some(write_context.previous_lcdc() & LCDC_WINDOW_TILE_MAP_BIT != 0);
    }

    fn force_window_activation_previous_tilemap_select(&mut self, previous_tilemap_select: bool) {
        if self.source != PpuBgFetcherSource::Window
            || self.fetch_x > BG_TILE_WIDTH as u16 * 2
            || self
                .window_activation_first_pixel_previous_tilemap_select
                .is_some()
        {
            return;
        }

        self.window_activation_first_pixel_previous_tilemap_select = Some(previous_tilemap_select);
    }

    fn force_dmg_lcdc4_previous_tiledata_select_for_output_override_up_to(
        &mut self,
        previous_select: BgTileDataSelect,
        max_fetch_x: u16,
    ) {
        if self.source != PpuBgFetcherSource::Window
            || self.fetch_x > max_fetch_x
            || self
                .dmg_lcdc4_previous_tiledata_select_for_output_override
                .is_some()
        {
            return;
        }

        self.dmg_lcdc4_previous_tiledata_select_for_output_override = Some(previous_select);
    }

    pub(super) fn arm_startup_visible_tile3_scx_boundary_old_tail(
        &mut self,
        previous_scx: u8,
        current_scx: u8,
    ) {
        let tail_pixels = startup_visible_tile3_scx_boundary_old_tail_pixels(current_scx);
        if tail_pixels == 0 {
            return;
        }

        self.startup_visible_tile3_scx_boundary_previous_scx = Some(previous_scx);
        self.startup_visible_tile3_scx_boundary_old_tail_start_pixel = BG_TILE_WIDTH - tail_pixels;
        self.startup_visible_tile3_scx_boundary_old_prefix_pixels = 0;
    }

    pub(super) fn arm_startup_visible_tile3_scx_boundary_next_tile_output_retarget(
        &mut self,
        scx: u8,
    ) {
        self.startup_visible_tile3_scx_boundary_next_tile_output_retarget_scx = Some(scx);
    }

    pub(super) fn preserve_old_startup_visible_tile3_scx_boundary_pixel(
        self,
        pixel_index: u8,
    ) -> Option<u8> {
        let previous_scx = self.startup_visible_tile3_scx_boundary_previous_scx?;
        let preserve_old_tail =
            pixel_index >= self.startup_visible_tile3_scx_boundary_old_tail_start_pixel;
        let preserve_old_prefix =
            pixel_index < self.startup_visible_tile3_scx_boundary_old_prefix_pixels;
        if preserve_old_tail || preserve_old_prefix {
            Some(previous_scx)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct BgFifoPixelCached {
    pub(super) cached: BgCachedSlice,
    pub(super) pixel_index: u8,
}

impl BgFifoPixelCached {
    pub(super) const fn new(cached: BgCachedSlice, pixel_index: u8) -> Self {
        Self {
            cached,
            pixel_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct BgFifoPixel {
    pub(super) color: u8,
    pub(super) cached: Option<BgFifoPixelCached>,
}

impl BgFifoPixel {
    pub(super) const fn new(color: u8, cached: Option<BgFifoPixelCached>) -> Self {
        Self { color, cached }
    }

    pub(super) const fn color(self) -> u8 {
        self.color
    }

    pub(super) const fn cgb_bg_attrs(self) -> Option<CgbBgTileAttributes> {
        match self.cached {
            Some(cached) => cached.cached.cgb_bg_attrs,
            None => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct BgOutputPixel {
    pub(super) color: u8,
    pub(super) cgb_bg_attrs: Option<CgbBgTileAttributes>,
}

impl BgOutputPixel {
    pub(super) const fn new(color: u8, cgb_bg_attrs: Option<CgbBgTileAttributes>) -> Self {
        Self {
            color,
            cgb_bg_attrs,
        }
    }
}

pub(super) fn recompute_live_background_cached_slice(
    mut cached: BgCachedSlice,
    vram: &VramBusView<'_>,
    context: PpuMode3LiveBackgroundRefetchContext,
) -> Option<BgCachedSlice> {
    if !cached.needs_live_tilemap_refetch
        && !cached.needs_live_tilemap_full_refetch
        && !cached.needs_live_tile_data_refetch
        && !cached.needs_live_tile_data_current_row_refetch
        && !cached.needs_live_tile_low_current_row_refetch
        && !cached.needs_live_tile_high_current_row_refetch
        && !cached.needs_live_tile_data_unsigned_reuse
    {
        return None;
    }

    let registers = context.registers();
    let mut tile_map_address = cached.tile_map_address;
    let mut tile_index = cached.tile_index;
    let mut cgb_bg_attrs = cached.cgb_bg_attrs;
    if cached.needs_live_tilemap_refetch {
        let tilemap_select_override = cached.dmg_lcdc3_tilemap_select_override;
        let full_refetch_fetch_x =
            if cached.startup_visible_tile3_scx_boundary_full_refetch_next_tile {
                cached.fetch_x + BG_TILE_WIDTH as u16
            } else {
                cached.fetch_x
            };
        tile_map_address = if cached.needs_live_tilemap_full_refetch {
            match cached.source {
                PpuBgFetcherSource::Background => PpuMode3BackgroundFetchContext::new(
                    registers,
                    registers,
                    full_refetch_fetch_x,
                    context.ly(),
                )
                .tile_index_address(),
                PpuBgFetcherSource::Window => {
                    let tile_map_offset = cached.tile_map_address & 0x03FF;
                    let tile_map_base = if registers.lcdc & LCDC_WINDOW_TILE_MAP_BIT != 0 {
                        0x1C00
                    } else {
                        0x1800
                    };
                    tile_map_base | tile_map_offset
                }
            }
        } else {
            let tile_map_offset = cached.tile_map_address & 0x03FF;
            let tile_map_base = match cached.source {
                PpuBgFetcherSource::Background => {
                    if tilemap_select_override.unwrap_or(registers.lcdc & LCDC_BG_TILE_MAP_BIT != 0)
                    {
                        0x1C00
                    } else {
                        0x1800
                    }
                }
                PpuBgFetcherSource::Window => {
                    if registers.lcdc & LCDC_WINDOW_TILE_MAP_BIT != 0 {
                        0x1C00
                    } else {
                        0x1800
                    }
                }
            };
            tile_map_base | tile_map_offset
        };
        tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        if cgb_bg_attrs.is_some() {
            cgb_bg_attrs = Some(CgbBgTileAttributes::new(
                vram.read_bank(CGB_BG_ATTR_BANK, tile_map_address as usize)
                    .unwrap_or(0),
            ));
        }
    }

    let attributes = cgb_bg_attrs.unwrap_or_default();
    let cached_tile_low_address = cached_tile_low_address(cached);
    let cached_tile_high_address = cached_tile_high_address(cached);
    let current_tile_row = match cached.source {
        PpuBgFetcherSource::Background => context.current_scanline_tile_row(),
        PpuBgFetcherSource::Window => context.current_window_tile_row(),
    };
    let tile_low_row = if cached.needs_live_tile_data_current_row_refetch
        || cached.needs_live_tile_low_current_row_refetch
    {
        cgb_bg_effective_tile_row(current_tile_row, attributes)
    } else {
        bg_tile_data_address_row(cached_tile_low_address)
    };
    let tile_high_row = if cached.needs_live_tile_data_current_row_refetch
        || cached.needs_live_tile_high_current_row_refetch
    {
        cgb_bg_effective_tile_row(current_tile_row, attributes)
    } else {
        bg_tile_data_address_row(cached_tile_high_address)
    };
    let tile_low_lcdc = if let Some(select) = cached.dmg_lcdc4_tiledata_select_override.low {
        select.apply_to_lcdc(registers.lcdc)
    } else {
        registers.lcdc
    };
    let tile_high_lcdc = if let Some(select) = cached.dmg_lcdc4_tiledata_select_override.high {
        select.apply_to_lcdc(registers.lcdc)
    } else {
        registers.lcdc
    };
    let tile_low_address =
        bg_tile_data_base(tile_low_lcdc, tile_index) + tile_low_row * TILE_ROW_BYTES;
    let tile_high_address =
        bg_tile_data_base(tile_high_lcdc, tile_index) + tile_high_row * TILE_ROW_BYTES + 1;
    let (tile_low, mut tile_high) =
        if cached.needs_live_tile_data_unsigned_reuse && !cached.needs_live_tilemap_refetch {
            (
                context.last_unsigned_tile_data_low_fetch(),
                context.last_unsigned_tile_data_high_fetch(),
            )
        } else {
            (
                vram.read_bank(attributes.tile_vram_bank(), tile_low_address as usize)
                    .unwrap_or(0),
                vram.read_bank(attributes.tile_vram_bank(), tile_high_address as usize)
                    .unwrap_or(0),
            )
        };
    if let Some(tile_high_override) = cached.cgb_lcdc4_same_cycle_tile_high_override {
        tile_high = tile_high_override;
    }

    cached.tile_map_address = tile_map_address;
    cached.tile_data_address = tile_high_address;
    cached.tile_low_address = tile_low_address;
    cached.tile_high_address = tile_high_address;
    cached.tile_index = tile_index;
    cached.cgb_bg_attrs = cgb_bg_attrs;
    cached.tile_low = tile_low;
    cached.tile_high = tile_high;
    cached.startup_visible_tile3_scx_boundary_full_refetch_next_tile = false;
    cached.needs_live_tilemap_refetch = false;
    cached.needs_live_tilemap_full_refetch = false;
    cached.needs_live_tile_data_refetch = false;
    cached.needs_live_tile_data_current_row_refetch = false;
    cached.needs_live_tile_low_current_row_refetch = false;
    cached.needs_live_tile_high_current_row_refetch = false;
    cached.needs_live_tile_data_unsigned_reuse = false;
    cached.cgb_lcdc4_same_cycle_tile_high_override = None;
    cached.dmg_lcdc3_tilemap_select_override = None;
    cached.dmg_lcdc4_tiledata_select_override = PerPlane::new(None, None);
    Some(cached)
}

const fn bg_tile_data_address_row(address: u16) -> u16 {
    (address & (TILE_BYTES - 1)) / TILE_ROW_BYTES
}

const fn cached_tile_low_address(cached: BgCachedSlice) -> u16 {
    if cached.tile_low_address == 0 && cached.tile_data_address != 0 {
        cached.tile_data_address & !1
    } else {
        cached.tile_low_address
    }
}

const fn cached_tile_high_address(cached: BgCachedSlice) -> u16 {
    if cached.tile_high_address == 0 && cached.tile_data_address != 0 {
        cached.tile_data_address | 1
    } else {
        cached.tile_high_address
    }
}

const fn startup_visible_tile3_scx_boundary_old_tail_pixels(scx: u8) -> u8 {
    match scx & 0x07 {
        0 => 0,
        low_bits => low_bits.saturating_sub(1),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) enum BgPushDisposition {
    #[default]
    Ready,
    InterruptedByObjectFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum BgPushDotResult {
    NotReady,
    EntryDelay,
    WaitingForEmptyFifo,
    HandedOffToObjectFetch,
    QueuedFillAndHandedOffToObjectFetch,
    QueuedFill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum BgPushDotOwnership {
    NotReady,
    EntryDelay,
    WaitingForEmptyFifo,
    FifoBackedTransferObjectFetch,
    QueueFill,
    QueueFillThenObjectFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct Mode3DotArbitration {
    pub(super) bg_transfer_can_advance: bool,
    pub(super) obj_fetch_can_start_from_fifo_backed_transfer: bool,
    pub(super) obj_fetch_can_start_from_queued_bg_fill: bool,
}

impl Mode3DotArbitration {
    pub(super) const fn can_serve_bg_transfer(self) -> bool {
        self.bg_transfer_can_advance
    }

    pub(super) const fn can_start_obj_fetch(self, start_source: ObjFetchStartSource) -> bool {
        match start_source {
            ObjFetchStartSource::FifoBackedTransfer => {
                self.obj_fetch_can_start_from_fifo_backed_transfer
            }
            ObjFetchStartSource::QueuedBgFill | ObjFetchStartSource::PushCachedBgFetch => {
                self.obj_fetch_can_start_from_queued_bg_fill
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum ObjFetchStartSource {
    FifoBackedTransfer,
    PushCachedBgFetch,
    QueuedBgFill,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct WindowState {
    pub(super) wy_triggered: bool,
    pub(super) pending_wx166_next_line: bool,
    pub(super) window_line_counter: u8,
}

impl WindowState {
    pub(super) fn reset(&mut self) {
        self.wy_triggered = false;
        self.pending_wx166_next_line = false;
        self.window_line_counter = 0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct StatState {
    pub(super) irq_line: bool,
    pub(super) lcd_disabled_lyc_coincidence: bool,
    #[serde(default)]
    pub(super) suppress_mode0_pretrigger_until_vblank: bool,
    #[serde(default)]
    pub(super) startup_mode0_irq_phase_active: bool,
    #[serde(default)]
    pub(super) real_boot_handoff_mode0_scx_seam_phase_active: bool,
    #[serde(default)]
    pub(super) vblank_wrap_line0_stat_delay_active: bool,
    #[serde(default)]
    pub(super) skip_boot_ly_read_lag_active: bool,
    #[serde(default)]
    pub(super) boot_power_on_ppu_phase_active: bool,
    #[serde(default)]
    pub(super) boot_power_on_ppu_phase_base_dot: u32,
    #[serde(default)]
    pub(super) boot_power_on_ppu_phase_extends_until_vblank: bool,
    #[serde(default)]
    pub(super) line_153_lyc0_stat_irq_pretrigger_pending: bool,
    #[serde(default)]
    pub(super) dmg_stat_write_quirk_blocks_line153_lyc0: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct ObjPipelineState {
    pub(super) fifo: VecDeque<ObjPixel>,
    pub(super) fetched_sprite_slots: [bool; MAX_SELECTED_SPRITES_PER_LINE],
    pub(super) pending_sprite_slots: VecDeque<u8>,
    pub(super) pending_sprite_obj_heights: [u8; MAX_SELECTED_SPRITES_PER_LINE],
    pub(super) mode3_line_start_obj_height: u8,
    pub(super) pending_match_x: Option<u8>,
    pub(super) late_metadata_word: Option<(u8, u8)>,
    pub(super) fetch: ObjFetchState,
}

impl ObjPipelineState {
    pub(super) fn dynamic_payload_bytes(&self) -> usize {
        self.fifo
            .len()
            .saturating_mul(std::mem::size_of::<ObjPixel>())
            .saturating_add(
                self.pending_sprite_slots
                    .len()
                    .saturating_mul(std::mem::size_of::<u8>()),
            )
    }
}

impl ObjPipelineState {
    pub(super) fn reset(&mut self) {
        self.fifo.clear();
        self.fetched_sprite_slots.fill(false);
        self.pending_sprite_slots.clear();
        self.pending_sprite_obj_heights.fill(0);
        self.mode3_line_start_obj_height = 8;
        self.pending_match_x = None;
        self.late_metadata_word = None;
        self.fetch = ObjFetchState::default();
    }

    pub(super) fn start_fetch(
        &mut self,
        sprite_slot: u8,
        sprite: PpuSelectedSprite,
        selected_obj_height: u8,
        latched_obj_height: u8,
    ) {
        self.fetch.stage = PpuObjFetcherStage::Startup;
        self.fetch.stage_dot = 0;
        self.fetch.sprite_slot = sprite_slot;
        self.fetch.sprite = Some(sprite);
        self.fetch.resolved_sprite = None;
        self.fetch.selected_obj_height = selected_obj_height;
        self.fetch.latched_obj_height = latched_obj_height;
        self.fetch.resolved_tile_index = None;
        self.fetch.resolved_tile_row = None;
        self.fetch.cancelled = false;
        self.fetch.count_terminal_push_dot = false;
        self.fetch.tile_low = 0;
        self.fetch.tile_high = 0;
    }

    pub(super) fn mark_fetched(&mut self, sprite_slot: u8) {
        self.fetched_sprite_slots[sprite_slot as usize] = true;
    }

    pub(super) fn has_fetched(&self, sprite_slot: u8) -> bool {
        self.fetched_sprite_slots[sprite_slot as usize]
    }

    pub(super) fn queue_fetch_hit(
        &mut self,
        sprite_slot: u8,
        owner: ObjHitOwnership,
        obj_height: u8,
    ) {
        if self.has_fetched(sprite_slot)
            || self
                .pending_sprite_slots
                .iter()
                .any(|queued_slot| *queued_slot == sprite_slot)
            || (self.fetch.stage != PpuObjFetcherStage::Idle
                && self.fetch.sprite_slot == sprite_slot)
        {
            return;
        }

        if self.pending_sprite_slots.is_empty() {
            self.pending_match_x = Some(owner.match_x);
        } else {
            debug_assert_eq!(self.pending_match_x, Some(owner.match_x));
        }
        self.pending_sprite_slots.push_back(sprite_slot);
        self.pending_sprite_obj_heights[sprite_slot as usize] = obj_height;
    }

    pub(super) fn pop_pending_fetch_hit(&mut self) -> Option<(u8, u8)> {
        let sprite_slot = self.pending_sprite_slots.pop_front()?;
        let obj_height = self.pending_sprite_obj_heights[sprite_slot as usize];
        if self.pending_sprite_slots.is_empty() {
            self.pending_match_x = None;
        }
        Some((sprite_slot, obj_height))
    }

    pub(super) fn pending_hits_own_current_dot(&self, current_owner: ObjHitOwnership) -> bool {
        self.pending_match_x == Some(current_owner.match_x) && !self.pending_sprite_slots.is_empty()
    }

    pub(super) fn clear_pending_fetch_hits(&mut self) {
        self.pending_sprite_slots.clear();
        self.pending_match_x = None;
    }

    pub(super) fn clear_pending_fetch_hits_if_stale(&mut self, current_owner: ObjHitOwnership) {
        if self.fetch.stage != PpuObjFetcherStage::Idle {
            return;
        }

        if self.pending_match_x.is_some() && self.pending_match_x != Some(current_owner.match_x) {
            self.clear_pending_fetch_hits();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct ObjHitOwnership {
    pub(super) match_x: u8,
    pub(super) phase: ObjHitPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum ObjHitPhase {
    PreVisible,
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct ObjFetchState {
    pub(super) stage: PpuObjFetcherStage,
    pub(super) stage_dot: u8,
    pub(super) sprite_slot: u8,
    pub(super) sprite: Option<PpuSelectedSprite>,
    pub(super) resolved_sprite: Option<PpuSelectedSprite>,
    pub(super) selected_obj_height: u8,
    pub(super) latched_obj_height: u8,
    pub(super) resolved_tile_index: Option<u8>,
    pub(super) resolved_tile_row: Option<u8>,
    pub(super) cancelled: bool,
    pub(super) count_terminal_push_dot: bool,
    pub(super) tile_low: u8,
    pub(super) tile_high: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct ObjPixel {
    pub(super) color: u8,
    pub(super) palette_obp1: bool,
    pub(super) bg_over_obj: bool,
    #[serde(default)]
    pub(super) cgb_obj_attrs: Option<CgbObjAttributes>,
    pub(super) sprite_x: u8,
    pub(super) oam_index: u8,
}

impl ObjPixel {
    pub(super) const fn transparent() -> Self {
        Self {
            color: 0,
            palette_obp1: false,
            bg_over_obj: false,
            cgb_obj_attrs: None,
            sprite_x: u8::MAX,
            oam_index: u8::MAX,
        }
    }

    pub(super) const fn is_transparent(self) -> bool {
        self.color == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct MixedPixel {
    pub(super) color: u8,
    pub(super) source: MixedPixelSource,
    #[serde(default)]
    pub(super) cgb_bg_attrs: Option<CgbBgTileAttributes>,
    #[serde(default)]
    pub(super) cgb_obj_attrs: Option<CgbObjAttributes>,
}

impl MixedPixel {
    pub(super) const fn background(color: u8) -> Self {
        Self::background_with_cgb_attrs(color, None)
    }

    pub(super) const fn background_with_cgb_attrs(
        color: u8,
        cgb_bg_attrs: Option<CgbBgTileAttributes>,
    ) -> Self {
        Self {
            color,
            source: MixedPixelSource::Background,
            cgb_bg_attrs,
            cgb_obj_attrs: None,
        }
    }

    #[allow(dead_code)]
    pub(super) const fn object(color: u8, palette_obp1: bool) -> Self {
        Self::object_with_cgb_attrs(color, palette_obp1, None)
    }

    pub(super) const fn object_with_cgb_attrs(
        color: u8,
        palette_obp1: bool,
        cgb_obj_attrs: Option<CgbObjAttributes>,
    ) -> Self {
        Self {
            color,
            source: MixedPixelSource::Object { palette_obp1 },
            cgb_bg_attrs: None,
            cgb_obj_attrs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum MixedPixelSource {
    Background,
    Object { palette_obp1: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct Mode2ScanState {
    pub(super) scanned_entries: u8,
    pub(super) selected_sprite_count: u8,
    pub(super) selected_sprites: [Option<PpuSelectedSprite>; MAX_SELECTED_SPRITES_PER_LINE],
    pub(super) latched_mode2_yx_word: Option<(u8, u8)>,
}

impl Mode2ScanState {
    pub(super) fn reset_scanline(&mut self) {
        self.scanned_entries = 0;
        self.selected_sprite_count = 0;
        self.selected_sprites.fill(None);
    }

    pub(super) fn reset(&mut self) {
        self.reset_scanline();
        self.latched_mode2_yx_word = None;
    }

    pub(super) fn scanned_entries(&self) -> u8 {
        self.scanned_entries
    }

    pub(super) fn increment_scanned_entries(&mut self) {
        self.scanned_entries += 1;
    }

    pub(super) fn latch_mode2_yx_word(&mut self, y: u8, x: u8) {
        self.latched_mode2_yx_word = Some((y, x));
    }

    pub(super) fn latched_mode2_yx_word(&self) -> Option<(u8, u8)> {
        self.latched_mode2_yx_word
    }

    pub(super) fn selected_sprite_count(&self) -> u8 {
        self.selected_sprite_count
    }

    pub(super) fn is_full(&self) -> bool {
        self.selected_sprite_count as usize == MAX_SELECTED_SPRITES_PER_LINE
    }

    pub(super) fn push(&mut self, sprite: PpuSelectedSprite) {
        if self.is_full() {
            return;
        }

        let slot = self.selected_sprite_count as usize;
        self.selected_sprites[slot] = Some(sprite);
        self.selected_sprite_count += 1;
    }

    pub(super) fn selected_sprites_snapshot(&self) -> Vec<PpuSelectedSprite> {
        self.selected_sprites
            .iter()
            .take(self.selected_sprite_count as usize)
            .flatten()
            .copied()
            .collect()
    }

    pub(super) fn selected_sprite(&self, slot: u8) -> Option<PpuSelectedSprite> {
        self.selected_sprites
            .get(slot as usize)
            .and_then(|sprite| *sprite)
    }
}

impl Default for Mode2ScanState {
    fn default() -> Self {
        Self {
            scanned_entries: 0,
            selected_sprite_count: 0,
            selected_sprites: [None; MAX_SELECTED_SPRITES_PER_LINE],
            latched_mode2_yx_word: None,
        }
    }
}
