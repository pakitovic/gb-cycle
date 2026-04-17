use crate::bus::{BusMaster, OamBusView, VramBusView};
use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, InterruptSource};
use std::collections::VecDeque;

mod api;
mod control;
mod helpers;
mod mode3;
mod palette_conflicts;
mod pipeline;
mod snapshot;
mod state;

use self::helpers::*;
pub use self::snapshot::{
    PpuBgCachedSliceOriginSnapshot, PpuBgFifoCachedPixelSnapshot, PpuBgPushDispositionSnapshot,
    PpuBgStartupContinuationSliceSnapshot, PpuBgStartupFetchSeamSnapshot,
    PpuMode3StartupSourceStateSnapshot, PpuMode3TransferBackingSnapshot,
    PpuMode3TransferDotKindSnapshot, PpuMode3TransferLaneSnapshot, PpuMode3TransferPhaseSnapshot,
    PpuMode3TransferReadinessSnapshot, PpuMode3TransferSourceWindowSnapshot, PpuSnapshot,
};
pub(crate) use self::state::OamCorruptionEventKind;
use self::state::*;

#[cfg(test)]
use self::snapshot::snapshot_bg_transfer_lane;

const LCDC_ENABLE_BIT: u8 = 0x80;
const LCDC_WINDOW_TILE_MAP_BIT: u8 = 0x40;
const LCDC_WINDOW_ENABLE_BIT: u8 = 0x20;
const LCDC_BG_ENABLE_BIT: u8 = 0x01;
const LCDC_OBJ_ENABLE_BIT: u8 = 0x02;
const LCDC_BG_TILE_MAP_BIT: u8 = 0x08;
const LCDC_BG_WINDOW_TILE_DATA_BIT: u8 = 0x10;
const STAT_WRITABLE_ENABLE_MASK: u8 = 0x78;
const STAT_FORCED_HIGH_BIT: u8 = 0x80;
const STAT_LYC_INTERRUPT_ENABLE_BIT: u8 = 0x40;
const STAT_MODE2_INTERRUPT_ENABLE_BIT: u8 = 0x20;
const STAT_MODE1_INTERRUPT_ENABLE_BIT: u8 = 0x10;
const STAT_MODE0_INTERRUPT_ENABLE_BIT: u8 = 0x08;
const SCREEN_WIDTH: usize = 160;
const SCREEN_HEIGHT: usize = 144;
const FRAMEBUFFER_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
const DOTS_PER_SCANLINE: u16 = 456;
const LY_READ_ADVANCE_START_DOT: u16 = 449;
const LCD_REENABLE_INITIAL_LINE_DOT: u16 = 0;
const LCD_REENABLE_LINE0_TOTAL_DOTS: u16 = DOTS_PER_SCANLINE - 8;
const LCD_REENABLE_LINE0_LY_READ_ADVANCE_START_DOT: u16 = LCD_REENABLE_LINE0_TOTAL_DOTS - 4;
const LCD_REENABLE_LINE0_MODE3_START_DOT: u16 = MODE2_DOTS - 8;
const LCD_REENABLE_LINE0_MODE0_RESTORE_DOT: u16 =
    LCD_REENABLE_LINE0_MODE3_START_DOT + MODE3_BASELINE_DOTS;
const CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES: u8 = 5;
const VISIBLE_SCANLINES: u8 = 144;
const TOTAL_SCANLINES: u8 = 154;
const MODE2_DOTS: u16 = 80;
const MODE3_BASELINE_DOTS: u16 = 172;
const MODE3_BG_FETCH_PRIMING_DOTS: u16 = 12;
const MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT: u16 = MODE3_BG_FETCH_PRIMING_DOTS - 4;
const MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT: u16 = MODE3_BG_FETCH_PRIMING_DOTS - 8;
const MODE3_INITIAL_SCX_CAPTURE_DOT: u16 = MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT - 1;
const MODE3_ABSTRACT_SOURCE_WINDOW_DOTS: u8 =
    (MODE3_BG_FETCH_PRIMING_DOTS - MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT) as u8;
const MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS: u8 =
    (MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT - MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT) as u8;
const MODE0_START_DOT: u16 = MODE2_DOTS + MODE3_BASELINE_DOTS;
const DMG_PALETTE_RETROACTIVE_PIXELS: usize = 4;
const DMG_PALETTE_RETROACTIVE_DOT_HISTORY: usize = DMG_PALETTE_RETROACTIVE_PIXELS + 1;
const OAM_ENTRY_BYTES: usize = 4;
const OAM_SPRITE_COUNT: u8 = 40;
const MODE2_T_CYCLES_PER_OAM_ENTRY: u16 = 2;
const MAX_SELECTED_SPRITES_PER_LINE: usize = 10;
const LCDC_OBJ_SIZE_BIT: u8 = 0x04;
const BG_TILE_WIDTH: u8 = 8;
const BG_TILE_MAP_WIDTH: u8 = 32;
const TILE_BYTES: u16 = 16;
const TILE_ROW_BYTES: u16 = 2;
const PPU_PENDING_VBLANK_INTERRUPT_BIT: u8 = 0x01;
const PPU_PENDING_LCD_STAT_INTERRUPT_BIT: u8 = 0x02;
const OAM_CORRUPTION_DOTS_PER_ROW: u16 = 4;
const OAM_CORRUPTION_ROW_BYTES: usize = 8;
const OAM_CORRUPTION_ROW_WORDS: usize = 4;
const OAM_CORRUPTION_ROW_COUNT: u8 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuStepRegion {
    Other,
    Mode0Or1,
    Mode2Scan,
    Mode3Startup,
    Mode3BgFetch,
    Mode3WindowFetch,
    Mode3Push,
    Mode3ObjFetch,
    Mode3PixelTransfer,
}

pub trait PpuStepObserver {
    fn begin_ppu_region(&mut self, _region: PpuStepRegion) {}

    fn end_ppu_region(&mut self, _region: PpuStepRegion) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NoopPpuStepObserver;

impl PpuStepObserver for NoopPpuStepObserver {}

fn observe_ppu_step_region<O, R>(
    observer: &mut O,
    region: PpuStepRegion,
    observe: impl FnOnce() -> R,
) -> R
where
    O: PpuStepObserver,
{
    observer.begin_ppu_region(region);
    let result = observe();
    observer.end_ppu_region(region);
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PpuAccessMode {
    #[default]
    HBlank,
    VBlank,
    OamScan,
    Drawing,
}

impl PpuAccessMode {
    pub const fn from_stat_bits(bits: u8) -> Self {
        match bits & 0x03 {
            0 => Self::HBlank,
            1 => Self::VBlank,
            2 => Self::OamScan,
            _ => Self::Drawing,
        }
    }

    pub const fn stat_bits(self) -> u8 {
        match self {
            Self::HBlank => 0,
            Self::VBlank => 1,
            Self::OamScan => 2,
            Self::Drawing => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PpuBusState {
    lcd_enabled: bool,
    mode: PpuAccessMode,
}

impl PpuBusState {
    pub const fn lcd_enabled(mode: PpuAccessMode) -> Self {
        Self {
            lcd_enabled: true,
            mode,
        }
    }

    pub const fn lcd_disabled() -> Self {
        Self {
            lcd_enabled: false,
            mode: PpuAccessMode::HBlank,
        }
    }

    pub const fn is_lcd_enabled(self) -> bool {
        self.lcd_enabled
    }

    pub const fn mode(self) -> PpuAccessMode {
        self.mode
    }
}

impl Default for PpuBusState {
    fn default() -> Self {
        Self::lcd_disabled()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PpuDmaOamConflict {
    address: u16,
    value: u8,
}

impl PpuDmaOamConflict {
    pub(crate) const fn new(address: u16, value: u8) -> Self {
        Self { address, value }
    }

    const fn word_address(self) -> u16 {
        self.address & !0x0001
    }

    const fn byte_offset_in_word(self) -> usize {
        (self.address & 0x0001) as usize
    }

    const fn value(self) -> u8 {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PpuLcdState {
    Enabled,
    #[default]
    Disabled,
}

impl PpuLcdState {
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PpuVisibleOutputState {
    Driving,
    #[default]
    ForcedBlank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PpuBgFetcherStage {
    #[default]
    Idle,
    WindowActivating,
    TileIndex,
    TileDataLow,
    TileDataHigh,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PpuBgFetcherSource {
    #[default]
    Background,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PpuObjFetcherStage {
    #[default]
    Idle,
    Startup,
    TileDataLow,
    TileDataHigh,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PpuSelectedSprite {
    pub oam_index: u8,
    pub y: u8,
    pub x: u8,
    pub tile_index: u8,
    pub attributes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpuStatus {
    RegistersReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PpuRegisterWriteSource {
    Immediate,
    CpuMmioCommit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PpuRegisterReadSource {
    Immediate,
    CpuBusOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DmgObjPaletteReadPolicy {
    #[default]
    ReadAsFfUntilWritten,
}

impl DmgObjPaletteReadPolicy {
    pub const fn default_read_value(self) -> u8 {
        match self {
            Self::ReadAsFfUntilWritten => 0xFF,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PpuStartupState {
    pub lcdc: u8,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub wy: u8,
    pub wx: u8,
    pub obj_palette_read_policy: DmgObjPaletteReadPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ppu {
    console_model: ConsoleModel,
    status: PpuStatus,
    lcdc: u8,
    stat_interrupt_enable: u8,
    lcd_state: PpuLcdState,
    lcd_enable_pending_delay_tcycles: u8,
    visible_output: PpuVisibleOutputState,
    scy: u8,
    scx: u8,
    ly: u8,
    line_dot: u16,
    lcd_restart_phase: PpuLcdRestartPhase,
    lyc: u8,
    bgp: u8,
    obp0: Option<u8>,
    obp1: Option<u8>,
    wy: u8,
    wx: u8,
    obj_palette_read_policy: DmgObjPaletteReadPolicy,
    visible_registers: PpuVisibleRegisters,
    pipeline_registers: PpuVisibleRegisters,
    dmg_panel_live_write_state: DmgPanelLiveWriteState,
    last_unsigned_tile_data_fetch: u8,
    last_unsigned_tile_data_low_fetch: u8,
    last_unsigned_tile_data_high_fetch: u8,
    startup_mode_latch: Option<PpuAccessMode>,
    stat_state: StatState,
    pending_interrupts: u8,
    blank_frame_active: bool,
    system_stop_active: bool,
    oam_corruption_controller: OamCorruptionController,
    mode2_scan_state: Mode2ScanState,
    window_state: WindowState,
    bg_pipeline_state: BgPipelineState,
    obj_pipeline_state: ObjPipelineState,
    current_scanline_pixels: [u8; SCREEN_WIDTH],
    current_scanline_bg_pixels: [u8; SCREEN_WIDTH],
    current_scanline_mixed_pixels: [MixedPixel; SCREEN_WIDTH],
    current_scanline_dmg_bg_forced_white: [bool; SCREEN_WIDTH],
    previous_scanline_mixed_pixels: [MixedPixel; SCREEN_WIDTH],
    previous_scanline_dmg_bg_forced_white: [bool; SCREEN_WIDTH],
    previous_scanline_ly: Option<u8>,
    framebuffer: Vec<u8>,
}

#[cfg(test)]
mod tests;
