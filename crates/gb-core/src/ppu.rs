use crate::bus::{BusMaster, OamBusView, VramBusView};
use crate::cartridge::CartridgeHeader;
use crate::model::{ConsoleModel, OperatingMode, StartupMode};
use crate::scheduler::{CycleContext, InterruptSource};
use std::cell::Cell;
use std::collections::VecDeque;
use std::mem;
use std::ops::{Deref, DerefMut};

mod api;
mod compat_palettes;
mod control;
mod helpers;
mod mode3;
mod palette_conflicts;
mod pipeline;
mod snapshot;
mod state;

use self::compat_palettes::*;
use self::helpers::*;
use self::palette_conflicts::*;
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
const RGB555_WHITE: u16 = 0x7FFF;
const RGB555_LIGHT_GRAY: u16 = 0x5294;
const RGB555_DARK_GRAY: u16 = 0x294A;
const RGB555_BLACK: u16 = 0x0000;
const DOTS_PER_SCANLINE: u16 = 456;
const DMG_BOOT_POWER_ON_CPU_READ_DELAY_DOTS: u32 = 36;
const DMG0_DIRECT_BOOT_HANDOFF_PPU_PHASE_BASE_OFFSET_DOTS: u32 = 3_992;
const DMG_BOOT_POWER_ON_MAX_DELAY_M_CYCLES: u16 = 235;
const LY_READ_ADVANCE_START_DOT: u16 = 451;
const LCD_REENABLE_INITIAL_LINE_DOT: u16 = 0;
const DMG_REAL_BOOT_POWER_ON_LCD_ENABLE_INITIAL_LINE_DOT: u16 = 92;
const LCD_REENABLE_LINE0_TOTAL_DOTS: u16 = DOTS_PER_SCANLINE - 8;
const LCD_REENABLE_LINE0_LY_READ_ADVANCE_START_DOT: u16 = LCD_REENABLE_LINE0_TOTAL_DOTS - 4;
const LCD_REENABLE_LINE0_MODE3_START_DOT: u16 = MODE2_DOTS - 8;
const LCD_REENABLE_LINE0_MODE0_RESTORE_DOT: u16 =
    LCD_REENABLE_LINE0_MODE3_START_DOT + MODE3_BASELINE_DOTS;
const CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES: u8 = 5;
const VISIBLE_SCANLINES: u8 = 144;
const TOTAL_SCANLINES: u8 = 154;
const CGB_LINE_153_LY_READ_ZERO_DOT: u16 = 8;
const LINE_153_LY_READ_ZERO_DOT: u16 = 4;
const LINE_153_LYC153_COMPARE_START_DOT: u16 = 4;
const LINE_153_LYC153_COMPARE_END_DOT: u16 = 8;
const LINE_153_LYC0_COMPARE_START_DOT: u16 = 12;
const LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT: u16 = LINE_153_LYC0_COMPARE_START_DOT - 4;
const LINE0_VBLANK_WRAP_STAT_READBACK_DELAY_DOTS: u16 = 4;
const DMG_MODE2_VBLANK_ENTRY_STAT_PRETRIGGER_DOTS: u16 = 4;
const CGB_COMPAT_MODE2_VBLANK_ENTRY_STAT_PRETRIGGER_DOTS: u16 = 4;
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
#[allow(dead_code)]
const CGB_BG_ATTR_PALETTE_MASK: u8 = 0x07;
const CGB_BG_ATTR_VRAM_BANK_BIT: u8 = 0x08;
#[allow(dead_code)]
const CGB_BG_ATTR_IGNORED_BIT: u8 = 0x10;
const CGB_BG_ATTR_X_FLIP_BIT: u8 = 0x20;
const CGB_BG_ATTR_Y_FLIP_BIT: u8 = 0x40;
#[allow(dead_code)]
const CGB_BG_ATTR_PRIORITY_BIT: u8 = 0x80;
const CGB_BG_ATTR_BANK: u8 = 1;
#[allow(dead_code)]
const CGB_OBJ_ATTR_PALETTE_MASK: u8 = 0x07;
const CGB_OBJ_ATTR_VRAM_BANK_BIT: u8 = 0x08;
#[allow(dead_code)]
const CGB_OBJ_ATTR_DMG_PALETTE_BIT: u8 = 0x10;
const CGB_OBJ_ATTR_X_FLIP_BIT: u8 = 0x20;
const CGB_OBJ_ATTR_Y_FLIP_BIT: u8 = 0x40;
#[allow(dead_code)]
const CGB_OBJ_ATTR_BG_OVER_OBJ_BIT: u8 = 0x80;

const fn panel_shade_to_rgb555(shade: u8) -> u16 {
    match shade {
        0 => RGB555_WHITE,
        1 => RGB555_LIGHT_GRAY,
        2 => RGB555_DARK_GRAY,
        _ => RGB555_BLACK,
    }
}

const fn normalize_ppu_operating_mode(
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
) -> OperatingMode {
    if console_model.supports_operating_mode(operating_mode) {
        operating_mode
    } else {
        console_model.default_operating_mode()
    }
}

fn normalize_saved_ppu_operating_mode(
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
    cgb_obj_priority_mode: CgbObjPriorityMode,
) -> OperatingMode {
    if console_model.supports_operating_mode(operating_mode) {
        return operating_mode;
    }

    if console_model.is_cgb_family()
        && operating_mode == OperatingMode::Dmg
        && cgb_obj_priority_mode == CgbObjPriorityMode::DmgXCoordinate
    {
        return OperatingMode::GbCompatible;
    }

    console_model.default_operating_mode()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PpuStepRegion {
    Other,
    BusSync,
    BusState,
    BusView,
    BusSnapshot,
    PublishedAccess,
    Tick,
    ModeTiming,
    RasterAdvance,
    RasterPublication,
    StatIrq,
    VisiblePrep,
    Mode0Or1,
    Mode2Scan,
    Mode3Control,
    Mode3Startup,
    Mode3BgFetch,
    Mode3BgEdge,
    Mode3WindowFetch,
    Mode3WindowEdge,
    Mode3Push,
    Mode3ObjEdge,
    Mode3ObjFetch,
    Mode3PixelTransfer,
}

pub trait PpuStepObserver {
    fn records_ppu_regions(&self) -> bool {
        true
    }

    fn begin_ppu_region(&mut self, _region: PpuStepRegion) {}

    fn end_ppu_region(&mut self, _region: PpuStepRegion) {}
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct NoopPpuStepObserver;

impl PpuStepObserver for NoopPpuStepObserver {
    fn records_ppu_regions(&self) -> bool {
        false
    }
}

fn observe_ppu_step_region<O, R>(
    observer: &mut O,
    region: PpuStepRegion,
    observe: impl FnOnce() -> R,
) -> R
where
    O: PpuStepObserver,
{
    observe_ppu_step_region_when(observer, observer.records_ppu_regions(), region, observe)
}

fn observe_ppu_step_region_when<O, R>(
    observer: &mut O,
    enabled: bool,
    region: PpuStepRegion,
    observe: impl FnOnce() -> R,
) -> R
where
    O: PpuStepObserver,
{
    if !enabled {
        return observe();
    }

    observer.begin_ppu_region(region);
    let result = observe();
    observer.end_ppu_region(region);
    result
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct PpuBusStateSnapshot {
    pub(crate) owner: PpuBusState,
    pub(crate) cpu_read: PpuBusState,
    pub(crate) cpu_write: PpuBusState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum PpuVisibleOutputState {
    Driving,
    #[default]
    ForcedBlank,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum PpuBgFetcherStage {
    #[default]
    Idle,
    WindowActivating,
    TileIndex,
    TileDataLow,
    TileDataHigh,
    Push,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum PpuBgFetcherSource {
    #[default]
    Background,
    Window,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum PpuFramebufferLayerSource {
    #[default]
    Backdrop,
    Background,
    Window,
    Object,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum PpuObjFetcherStage {
    #[default]
    Idle,
    Startup,
    TileDataLow,
    TileDataHigh,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PpuSelectedSprite {
    pub oam_index: u8,
    pub y: u8,
    pub x: u8,
    pub tile_index: u8,
    pub attributes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PpuStatus {
    RegistersReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum PpuRegisterWriteSource {
    Immediate,
    CpuMmioCommit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum PpuRegisterReadSource {
    Immediate,
    CpuBusOperation,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PpuPanelState {
    visible_output: PpuVisibleOutputState,
    dmg_panel_live_write_state: DmgPanelLiveWriteState,
    #[serde(with = "serde_big_array::BigArray")]
    current_scanline_pixels: [u8; SCREEN_WIDTH],
    #[serde(with = "serde_big_array::BigArray")]
    current_scanline_bg_pixels: [u8; SCREEN_WIDTH],
    #[serde(with = "serde_big_array::BigArray")]
    current_scanline_mixed_pixels: [MixedPixel; SCREEN_WIDTH],
    #[serde(with = "serde_big_array::BigArray")]
    current_scanline_bg_dot_contexts: [Option<PpuRecentBgDotContext>; SCREEN_WIDTH],
    #[serde(with = "serde_big_array::BigArray")]
    current_scanline_dmg_bg_forced_white: [bool; SCREEN_WIDTH],
    #[serde(with = "serde_big_array::BigArray")]
    previous_scanline_mixed_pixels: [MixedPixel; SCREEN_WIDTH],
    #[serde(with = "serde_big_array::BigArray")]
    previous_scanline_dmg_bg_forced_white: [bool; SCREEN_WIDTH],
    previous_scanline_ly: Option<u8>,
    pending_dmg_window_lcdc4_output_repaint: Option<BgTileDataSelect>,
    framebuffer: Vec<u8>,
    #[serde(default = "default_framebuffer_rgb555")]
    framebuffer_rgb555: Vec<u16>,
    framebuffer_layer_sources: Vec<PpuFramebufferLayerSource>,
    framebuffer_bgwin_colors: Vec<u8>,
    framebuffer_bgwin_forced_white: Vec<bool>,
    framebuffer_bgwin_panel_shades: Vec<u8>,
    framebuffer_backdrop_panel_shades: Vec<u8>,
    framebuffer_bgwin_layer_sources: Vec<PpuFramebufferLayerSource>,
}

impl Default for PpuPanelState {
    fn default() -> Self {
        Self {
            visible_output: PpuVisibleOutputState::ForcedBlank,
            dmg_panel_live_write_state: DmgPanelLiveWriteState::default(),
            current_scanline_pixels: [0; SCREEN_WIDTH],
            current_scanline_bg_pixels: [0; SCREEN_WIDTH],
            current_scanline_mixed_pixels: [MixedPixel::background(0); SCREEN_WIDTH],
            current_scanline_bg_dot_contexts: [None; SCREEN_WIDTH],
            current_scanline_dmg_bg_forced_white: [false; SCREEN_WIDTH],
            previous_scanline_mixed_pixels: [MixedPixel::background(0); SCREEN_WIDTH],
            previous_scanline_dmg_bg_forced_white: [false; SCREEN_WIDTH],
            previous_scanline_ly: None,
            pending_dmg_window_lcdc4_output_repaint: None,
            framebuffer: vec![0; FRAMEBUFFER_PIXELS],
            framebuffer_rgb555: vec![RGB555_WHITE; FRAMEBUFFER_PIXELS],
            framebuffer_layer_sources: vec![
                PpuFramebufferLayerSource::Backdrop;
                FRAMEBUFFER_PIXELS
            ],
            framebuffer_bgwin_colors: vec![0; FRAMEBUFFER_PIXELS],
            framebuffer_bgwin_forced_white: vec![false; FRAMEBUFFER_PIXELS],
            framebuffer_bgwin_panel_shades: vec![0; FRAMEBUFFER_PIXELS],
            framebuffer_backdrop_panel_shades: vec![0; FRAMEBUFFER_PIXELS],
            framebuffer_bgwin_layer_sources: vec![
                PpuFramebufferLayerSource::Backdrop;
                FRAMEBUFFER_PIXELS
            ],
        }
    }
}

fn default_framebuffer_rgb555() -> Vec<u16> {
    vec![RGB555_WHITE; FRAMEBUFFER_PIXELS]
}

impl PpuPanelState {
    fn dynamic_payload_bytes(&self) -> usize {
        self.dmg_panel_live_write_state
            .dynamic_payload_bytes()
            .saturating_add(self.framebuffer.len())
            .saturating_add(
                self.framebuffer_rgb555
                    .len()
                    .saturating_mul(mem::size_of::<u16>()),
            )
            .saturating_add(
                self.framebuffer_layer_sources
                    .len()
                    .saturating_mul(mem::size_of::<PpuFramebufferLayerSource>()),
            )
            .saturating_add(self.framebuffer_bgwin_colors.len())
            .saturating_add(self.framebuffer_bgwin_forced_white.len())
            .saturating_add(self.framebuffer_bgwin_panel_shades.len())
            .saturating_add(self.framebuffer_backdrop_panel_shades.len())
            .saturating_add(
                self.framebuffer_bgwin_layer_sources
                    .len()
                    .saturating_mul(mem::size_of::<PpuFramebufferLayerSource>()),
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PpuScanlineLengthCacheEntry {
    restart_phase: PpuLcdRestartPhase,
    ly: u8,
    value: u16,
}

#[derive(Debug, Default)]
struct PpuModeTimingCache {
    scanline_length: Cell<Option<PpuScanlineLengthCacheEntry>>,
}

impl Clone for PpuModeTimingCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for PpuModeTimingCache {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for PpuModeTimingCache {}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PpuRuntimeState {
    visible_registers: PpuVisibleRegisters,
    pipeline_registers: PpuVisibleRegisters,
    startup_mode_latch: Option<PpuAccessMode>,
    stat_state: StatState,
    pending_interrupts: u8,
    pending_interrupts_hidden_from_cpu_if: u8,
    blank_frame_active: bool,
    system_stop_active: bool,
    oam_corruption_controller: OamCorruptionController,
    mode2_scan_state: Mode2ScanState,
    window_state: WindowState,
    bg_pipeline_state: BgPipelineState,
    obj_pipeline_state: ObjPipelineState,
    last_unsigned_tile_data_fetch: u8,
    last_unsigned_tile_data_low_fetch: u8,
    last_unsigned_tile_data_high_fetch: u8,
    panel: PpuPanelState,
    #[serde(skip, default)]
    mode_timing_cache: PpuModeTimingCache,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PpuRuntimeSaveState {
    visible_registers: PpuVisibleRegisters,
    pipeline_registers: PpuVisibleRegisters,
    startup_mode_latch: Option<PpuAccessMode>,
    stat_state: StatState,
    pending_interrupts: u8,
    #[serde(default)]
    pending_interrupts_hidden_from_cpu_if: u8,
    blank_frame_active: bool,
    system_stop_active: bool,
    oam_corruption_controller: OamCorruptionController,
    mode2_scan_state: Mode2ScanState,
    window_state: WindowState,
    bg_pipeline_state: BgPipelineState,
    obj_pipeline_state: ObjPipelineState,
    last_unsigned_tile_data_fetch: u8,
    last_unsigned_tile_data_low_fetch: u8,
    last_unsigned_tile_data_high_fetch: u8,
    panel: PpuPanelState,
}

impl PpuRuntimeState {
    fn capture_save_state(&self) -> PpuRuntimeSaveState {
        PpuRuntimeSaveState {
            visible_registers: self.visible_registers,
            pipeline_registers: self.pipeline_registers,
            startup_mode_latch: self.startup_mode_latch,
            stat_state: self.stat_state.clone(),
            pending_interrupts: self.pending_interrupts,
            pending_interrupts_hidden_from_cpu_if: self.pending_interrupts_hidden_from_cpu_if,
            blank_frame_active: self.blank_frame_active,
            system_stop_active: self.system_stop_active,
            oam_corruption_controller: self.oam_corruption_controller,
            mode2_scan_state: self.mode2_scan_state.clone(),
            window_state: self.window_state.clone(),
            bg_pipeline_state: self.bg_pipeline_state.clone(),
            obj_pipeline_state: self.obj_pipeline_state.clone(),
            last_unsigned_tile_data_fetch: self.last_unsigned_tile_data_fetch,
            last_unsigned_tile_data_low_fetch: self.last_unsigned_tile_data_low_fetch,
            last_unsigned_tile_data_high_fetch: self.last_unsigned_tile_data_high_fetch,
            panel: self.panel.clone(),
        }
    }

    fn restore_save_state(&mut self, state: &PpuRuntimeSaveState) {
        self.visible_registers = state.visible_registers;
        self.pipeline_registers = state.pipeline_registers;
        self.startup_mode_latch = state.startup_mode_latch;
        self.stat_state = state.stat_state.clone();
        self.pending_interrupts = state.pending_interrupts;
        self.pending_interrupts_hidden_from_cpu_if = state.pending_interrupts_hidden_from_cpu_if;
        self.blank_frame_active = state.blank_frame_active;
        self.system_stop_active = state.system_stop_active;
        self.oam_corruption_controller = state.oam_corruption_controller;
        self.mode2_scan_state = state.mode2_scan_state.clone();
        self.window_state = state.window_state.clone();
        self.bg_pipeline_state = state.bg_pipeline_state.clone();
        self.obj_pipeline_state = state.obj_pipeline_state.clone();
        self.last_unsigned_tile_data_fetch = state.last_unsigned_tile_data_fetch;
        self.last_unsigned_tile_data_low_fetch = state.last_unsigned_tile_data_low_fetch;
        self.last_unsigned_tile_data_high_fetch = state.last_unsigned_tile_data_high_fetch;
        self.panel = state.panel.clone();
        self.mode_timing_cache = PpuModeTimingCache::default();
    }
}

impl Default for PpuRuntimeState {
    fn default() -> Self {
        Self {
            visible_registers: PpuVisibleRegisters::default(),
            pipeline_registers: PpuVisibleRegisters::default(),
            startup_mode_latch: None,
            stat_state: StatState::default(),
            pending_interrupts: 0,
            pending_interrupts_hidden_from_cpu_if: 0,
            blank_frame_active: false,
            system_stop_active: false,
            oam_corruption_controller: OamCorruptionController,
            mode2_scan_state: Mode2ScanState::default(),
            window_state: WindowState::default(),
            bg_pipeline_state: BgPipelineState::default(),
            obj_pipeline_state: ObjPipelineState::default(),
            last_unsigned_tile_data_fetch: 0,
            last_unsigned_tile_data_low_fetch: 0,
            last_unsigned_tile_data_high_fetch: 0,
            panel: PpuPanelState::default(),
            mode_timing_cache: PpuModeTimingCache::default(),
        }
    }
}

impl Deref for PpuRuntimeState {
    type Target = PpuPanelState;

    fn deref(&self) -> &Self::Target {
        &self.panel
    }
}

impl DerefMut for PpuRuntimeState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.panel
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ppu {
    console_model: ConsoleModel,
    #[serde(default)]
    operating_mode: OperatingMode,
    #[serde(default)]
    cgb_obj_priority_mode: CgbObjPriorityMode,
    #[serde(default)]
    cgb_opri_latch: u8,
    status: PpuStatus,
    lcdc: u8,
    stat_interrupt_enable: u8,
    lcd_state: PpuLcdState,
    lcd_enable_pending_delay_tcycles: u8,
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
    cgb_palettes: CgbPaletteState,
    obj_palette_read_policy: DmgObjPaletteReadPolicy,
    runtime: PpuRuntimeState,
    #[serde(default)]
    dmg_real_boot_power_on_lcd_enable_phase_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PpuSaveState {
    console_model: ConsoleModel,
    #[serde(default)]
    operating_mode: OperatingMode,
    #[serde(default)]
    cgb_obj_priority_mode: CgbObjPriorityMode,
    #[serde(default)]
    cgb_opri_latch: u8,
    status: PpuStatus,
    lcdc: u8,
    stat_interrupt_enable: u8,
    lcd_state: PpuLcdState,
    lcd_enable_pending_delay_tcycles: u8,
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
    #[serde(default)]
    cgb_palettes: CgbPaletteState,
    obj_palette_read_policy: DmgObjPaletteReadPolicy,
    runtime: PpuRuntimeSaveState,
    #[serde(default)]
    dmg_real_boot_power_on_lcd_enable_phase_active: bool,
}

impl PpuSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.runtime.dynamic_payload_bytes()
    }
}

impl PpuRuntimeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        self.bg_pipeline_state
            .dynamic_payload_bytes()
            .saturating_add(self.obj_pipeline_state.dynamic_payload_bytes())
            .saturating_add(self.panel.dynamic_payload_bytes())
    }
}

impl Ppu {
    pub(crate) fn capture_save_state(&self) -> PpuSaveState {
        PpuSaveState {
            console_model: self.console_model,
            operating_mode: self.operating_mode,
            cgb_obj_priority_mode: self.cgb_obj_priority_mode,
            cgb_opri_latch: self.cgb_opri_latch,
            status: self.status,
            lcdc: self.lcdc,
            stat_interrupt_enable: self.stat_interrupt_enable,
            lcd_state: self.lcd_state,
            lcd_enable_pending_delay_tcycles: self.lcd_enable_pending_delay_tcycles,
            scy: self.scy,
            scx: self.scx,
            ly: self.ly,
            line_dot: self.line_dot,
            lcd_restart_phase: self.lcd_restart_phase,
            lyc: self.lyc,
            bgp: self.bgp,
            obp0: self.obp0,
            obp1: self.obp1,
            wy: self.wy,
            wx: self.wx,
            cgb_palettes: self.cgb_palettes.clone(),
            obj_palette_read_policy: self.obj_palette_read_policy,
            runtime: self.runtime.capture_save_state(),
            dmg_real_boot_power_on_lcd_enable_phase_active: self
                .dmg_real_boot_power_on_lcd_enable_phase_active,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &PpuSaveState) {
        self.console_model = state.console_model;
        self.operating_mode = normalize_saved_ppu_operating_mode(
            state.console_model,
            state.operating_mode,
            state.cgb_obj_priority_mode,
        );
        self.cgb_obj_priority_mode = state.cgb_obj_priority_mode;
        self.cgb_opri_latch = state.cgb_opri_latch & 0x01;
        self.status = state.status;
        self.lcdc = state.lcdc;
        self.stat_interrupt_enable = state.stat_interrupt_enable;
        self.lcd_state = state.lcd_state;
        self.lcd_enable_pending_delay_tcycles = state.lcd_enable_pending_delay_tcycles;
        self.scy = state.scy;
        self.scx = state.scx;
        self.ly = state.ly;
        self.line_dot = state.line_dot;
        self.lcd_restart_phase = state.lcd_restart_phase;
        self.lyc = state.lyc;
        self.bgp = state.bgp;
        self.obp0 = state.obp0;
        self.obp1 = state.obp1;
        self.wy = state.wy;
        self.wx = state.wx;
        self.cgb_palettes = state.cgb_palettes.clone();
        self.obj_palette_read_policy = state.obj_palette_read_policy;
        self.runtime.restore_save_state(&state.runtime);
        self.dmg_real_boot_power_on_lcd_enable_phase_active =
            state.dmg_real_boot_power_on_lcd_enable_phase_active;
    }
}

impl Deref for Ppu {
    type Target = PpuRuntimeState;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl DerefMut for Ppu {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runtime
    }
}

#[cfg(test)]
mod tests;
