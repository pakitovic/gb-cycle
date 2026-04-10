use crate::bus::{BusMaster, OamBusView, VramBusView};
use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, InterruptSource};
use std::collections::VecDeque;

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
const MODE3_ABSTRACT_SOURCE_WINDOW_DOTS: u8 =
    (MODE3_BG_FETCH_PRIMING_DOTS - MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT) as u8;
const MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS: u8 =
    (MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT - MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT) as u8;
const MODE0_START_DOT: u16 = MODE2_DOTS + MODE3_BASELINE_DOTS;
const DMG_PALETTE_RETROACTIVE_PIXELS: usize = 4;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum PpuLcdRestartPhase {
    #[default]
    Inactive,
    FirstLineAfterEnable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PpuRasterState {
    Disabled,
    LcdRestartFirstLine {
        mode: PpuAccessMode,
        mode_dot: u16,
    },
    Active {
        mode: PpuAccessMode,
        mode_dot: u16,
        mode2_scan_active: bool,
    },
}

impl PpuRasterState {
    const fn access_mode(self) -> PpuAccessMode {
        match self {
            Self::Disabled => PpuAccessMode::HBlank,
            Self::LcdRestartFirstLine { mode, .. } | Self::Active { mode, .. } => mode,
        }
    }

    const fn mode_dot(self) -> u16 {
        match self {
            Self::Disabled => 0,
            Self::LcdRestartFirstLine { mode_dot, .. } | Self::Active { mode_dot, .. } => mode_dot,
        }
    }

    const fn is_mode2_scan(self) -> bool {
        matches!(
            self,
            Self::Active {
                mode2_scan_active: true,
                ..
            }
        )
    }
}

impl PpuLcdRestartPhase {
    const fn first_line_after_enable() -> Self {
        Self::FirstLineAfterEnable
    }

    const fn is_first_line_after_enable_active(self, ly: u8) -> bool {
        matches!(self, Self::FirstLineAfterEnable) && ly == 0
    }

    const fn raster_state(self, ly: u8, line_dot: u16) -> Option<PpuRasterState> {
        if self.is_first_line_after_enable_active(ly) {
            let (mode, mode_dot) = if line_dot < LCD_REENABLE_LINE0_MODE3_START_DOT {
                (PpuAccessMode::HBlank, line_dot)
            } else if line_dot < LCD_REENABLE_LINE0_MODE0_RESTORE_DOT {
                (
                    PpuAccessMode::Drawing,
                    line_dot.saturating_sub(LCD_REENABLE_LINE0_MODE3_START_DOT),
                )
            } else {
                (
                    PpuAccessMode::HBlank,
                    line_dot.saturating_sub(LCD_REENABLE_LINE0_MODE0_RESTORE_DOT),
                )
            };
            Some(PpuRasterState::LcdRestartFirstLine { mode, mode_dot })
        } else {
            None
        }
    }

    const fn advance(self, ly: u8, _line_dot: u16) -> Self {
        if self.is_first_line_after_enable_active(ly) {
            self
        } else {
            Self::Inactive
        }
    }
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
    current_scanline_mixed_pixels: [MixedPixel; SCREEN_WIDTH],
    framebuffer: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpuSnapshot {
    pub console_model: ConsoleModel,
    pub status: PpuStatus,
    pub lcdc: u8,
    pub stat_interrupt_enable: u8,
    pub lyc_coincidence: bool,
    pub stat_irq_line: bool,
    pub blank_frame_active: bool,
    pub lcd_state: PpuLcdState,
    pub visible_output: PpuVisibleOutputState,
    pub mode: PpuAccessMode,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub line_dot: u16,
    pub mode_dot: u16,
    pub mode0_start_dot: u16,
    pub current_oam_scan_row: Option<u8>,
    pub mode2_scanned_entries: u8,
    pub selected_sprites: Vec<PpuSelectedSprite>,
    pub bg_fetcher_source: PpuBgFetcherSource,
    pub bg_fetcher_stage: PpuBgFetcherStage,
    pub bg_fetcher_stage_dot: u8,
    pub bg_fetcher_tile_map_address: u16,
    pub bg_fetcher_tile_data_address: u16,
    pub bg_push_pending: bool,
    pub bg_fill_pending: bool,
    pub bg_fifo_pixels: Vec<u8>,
    pub bg_fifo_cached_pixels: Vec<Option<PpuBgFifoCachedPixelSnapshot>>,
    pub bg_startup_source_state: PpuMode3StartupSourceStateSnapshot,
    pub bg_startup_fetch_seam: PpuBgStartupFetchSeamSnapshot,
    pub bg_startup_fifo_placeholders: u8,
    pub bg_push_entry_delay_remaining: u8,
    pub bg_fill_startup_dummy_pixels: u8,
    pub bg_fetcher_post_alignment_restart_delay_dots: u8,
    pub bg_transfer_phase: PpuMode3TransferPhaseSnapshot,
    pub bg_current_transfer_x: u8,
    pub bg_current_transfer_lane: Option<PpuMode3TransferLaneSnapshot>,
    pub bg_current_transfer_source_window: Option<PpuMode3TransferSourceWindowSnapshot>,
    pub bg_current_transfer_backing: Option<PpuMode3TransferBackingSnapshot>,
    pub bg_current_transfer_readiness: Option<PpuMode3TransferReadinessSnapshot>,
    pub bg_current_transfer_kind: Option<PpuMode3TransferDotKindSnapshot>,
    pub obj_fetcher_stage: PpuObjFetcherStage,
    pub obj_fetcher_stage_dot: u8,
    pub obj_pending_hit_match_x: Option<u8>,
    pub obj_pending_hit_len: usize,
    pub obj_pending_hit_front_sprite_slot: Option<u8>,
    pub obj_fifo_pixels: Vec<Option<u8>>,
    pub scx_discard_remaining: u8,
    pub visible_pixels_output: u8,
    pub window_wy_latch: bool,
    pub window_started_this_line: bool,
    pub window_line_counter: u8,
    pub current_scanline_pixels: Vec<u8>,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: Option<u8>,
    pub obp1: Option<u8>,
    pub wy: u8,
    pub wx: u8,
    pub visible_lcdc: u8,
    pub visible_scy: u8,
    pub visible_scx: u8,
    pub visible_bgp: u8,
    pub visible_obp0: Option<u8>,
    pub visible_obp1: Option<u8>,
    pub visible_wy: u8,
    pub visible_wx: u8,
    pub pipeline_lcdc: u8,
    pub pipeline_scy: u8,
    pub pipeline_scx: u8,
    pub pipeline_bgp: u8,
    pub pipeline_obp0: Option<u8>,
    pub pipeline_obp1: Option<u8>,
    pub pipeline_wy: u8,
    pub pipeline_wx: u8,
    pub obj_palette_read_policy: DmgObjPaletteReadPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuBgCachedSliceOriginSnapshot {
    Ordinary,
    StartupAlignmentSeed,
    StartupAlignmentFill,
    StartupContinuationVisibleTile2,
    StartupContinuationVisibleTile3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PpuBgFifoCachedPixelSnapshot {
    pub source: PpuBgFetcherSource,
    pub origin: PpuBgCachedSliceOriginSnapshot,
    pub fetch_x: u16,
    pub pixel_index: u8,
    pub same_cycle_live_tilemap_refetch_window_open: bool,
    pub needs_live_tilemap_refetch: bool,
    pub needs_live_tile_data_refetch: bool,
    pub needs_live_tile_data_unsigned_reuse: bool,
    pub tile_map_address: u16,
    pub tile_data_address: u16,
    pub tile_index: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferPhaseSnapshot {
    Priming,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferLaneSnapshot {
    PreVisible,
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferSourceWindowSnapshot {
    AbstractStartup,
    FifoBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferBackingSnapshot {
    Abstract,
    FifoBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferReadinessSnapshot {
    WaitingForFifo,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3TransferDotKindSnapshot {
    NotServed,
    ServedPreVisibleTransfer,
    ServedHiddenTransfer,
    ServedVisiblePixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuMode3StartupSourceStateSnapshot {
    EntryDelay { remaining: u8 },
    Abstract { remaining: u8 },
    FifoBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuBgStartupContinuationSliceSnapshot {
    None,
    VisibleTile2,
    VisibleTile3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PpuBgStartupFetchSeamSnapshot {
    Inactive,
    AlignmentSeedPending,
    PostAlignment {
        first_real_push_skips_entry_delay: bool,
        next_startup_continuation_slice: PpuBgStartupContinuationSliceSnapshot,
        startup_continuation_visible_tiles_remaining: u8,
        delayed_background_tileindex_read_tiles_remaining: u8,
        delayed_background_tilemap_tiles_remaining: u8,
        delayed_background_tiledata_tiles_remaining: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
struct PpuVisibleRegisters {
    lcdc: u8,
    scy: u8,
    scx: u8,
    bgp: u8,
    obp0: Option<u8>,
    obp1: Option<u8>,
    wy: u8,
    wx: u8,
}

impl PpuVisibleRegisters {
    const fn bg_enabled(self) -> bool {
        self.lcdc & LCDC_BG_ENABLE_BIT != 0
    }

    const fn obj_enabled(self) -> bool {
        self.lcdc & LCDC_OBJ_ENABLE_BIT != 0
    }

    const fn window_enabled(self) -> bool {
        self.lcdc & LCDC_WINDOW_ENABLE_BIT != 0
    }

    const fn obj_height(self) -> u8 {
        if self.lcdc & LCDC_OBJ_SIZE_BIT != 0 {
            16
        } else {
            8
        }
    }

    fn obj_palette(self, palette_obp1: bool, policy: DmgObjPaletteReadPolicy) -> u8 {
        if palette_obp1 {
            self.obp1.unwrap_or(policy.default_read_value())
        } else {
            self.obp0.unwrap_or(policy.default_read_value())
        }
    }
}

impl Ppu {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: PpuStatus::RegistersReady,
            lcdc: 0,
            stat_interrupt_enable: 0,
            lcd_state: PpuLcdState::Disabled,
            lcd_enable_pending_delay_tcycles: 0,
            visible_output: PpuVisibleOutputState::ForcedBlank,
            scy: 0,
            scx: 0,
            ly: 0,
            line_dot: 0,
            lcd_restart_phase: PpuLcdRestartPhase::Inactive,
            lyc: 0,
            bgp: 0,
            obp0: None,
            obp1: None,
            wy: 0,
            wx: 0,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
            visible_registers: PpuVisibleRegisters::default(),
            pipeline_registers: PpuVisibleRegisters::default(),
            last_unsigned_tile_data_fetch: 0,
            last_unsigned_tile_data_low_fetch: 0,
            last_unsigned_tile_data_high_fetch: 0,
            startup_mode_latch: None,
            stat_state: StatState::default(),
            pending_interrupts: 0,
            blank_frame_active: false,
            system_stop_active: false,
            oam_corruption_controller: OamCorruptionController,
            mode2_scan_state: Mode2ScanState::default(),
            window_state: WindowState::default(),
            bg_pipeline_state: BgPipelineState::default(),
            obj_pipeline_state: ObjPipelineState::default(),
            current_scanline_pixels: [0; SCREEN_WIDTH],
            current_scanline_mixed_pixels: [MixedPixel::background(0); SCREEN_WIDTH],
            framebuffer: vec![0; FRAMEBUFFER_PIXELS],
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> PpuStatus {
        self.status
    }

    pub fn bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_bus_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn cpu_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_published_bus_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn cpu_write_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_published_video_write_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn cpu_oam_read_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_published_oam_read_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn cpu_oam_write_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_published_oam_write_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub(crate) fn owner_bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.current_bus_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub fn read_register(&self, address: u16) -> u8 {
        self.read_register_with_source(address, PpuRegisterReadSource::Immediate)
    }

    pub(crate) fn read_register_with_source(
        &self,
        address: u16,
        source: PpuRegisterReadSource,
    ) -> u8 {
        match address {
            0xFF40 => self.read_lcdc(),
            0xFF41 => self.read_stat(source),
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.read_ly(),
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self
                .obp0
                .unwrap_or(self.obj_palette_read_policy.default_read_value()),
            0xFF49 => self
                .obp1
                .unwrap_or(self.obj_palette_read_policy.default_read_value()),
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        self.write_register_with_source(address, value, PpuRegisterWriteSource::Immediate);
    }

    pub(crate) fn write_register_with_source(
        &mut self,
        address: u16,
        value: u8,
        source: PpuRegisterWriteSource,
    ) {
        let previous_lcdc = self.lcdc;
        match address {
            0xFF40 => self.write_lcdc(value, source),
            0xFF41 => self.write_stat(value),
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {}
            0xFF45 => {
                self.lyc = value;
                if self.is_lcd_enabled() {
                    self.refresh_stat_irq_line(false);
                }
            }
            0xFF47 => self.write_dmg_palette_register(PpuPaletteRegister::Bgp, value),
            0xFF48 => self.write_dmg_palette_register(PpuPaletteRegister::Obp0, value),
            0xFF49 => self.write_dmg_palette_register(PpuPaletteRegister::Obp1, value),
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            _ => {}
        }

        if matches!(address, 0xFF40 | 0xFF42)
            && self.current_access_mode() == PpuAccessMode::Drawing
        {
            if self.bg_pipeline_state.push.pending {
                self.bg_pipeline_state
                    .push
                    .cached
                    .mark_live_register_write_while_push_pending(
                        address,
                        previous_lcdc,
                        self.lcdc,
                        self.bg_pipeline_state.push.entry_delay_remaining > 0,
                    );
            }

            if self.bg_pipeline_state.fill.pending {
                self.bg_pipeline_state
                    .fill
                    .cached
                    .mark_live_register_write_while_fill_pending(
                        address,
                        previous_lcdc,
                        self.lcdc,
                        self.bg_pipeline_state.fill.includes_real_tile_pixels,
                        self.bg_pipeline_state.fill.startup_dummy_pixels,
                    );
            }

            if address == 0xFF40 {
                self.bg_pipeline_state
                    .mark_live_lcdc3_write_while_fifo_visible(previous_lcdc, self.lcdc);
                self.bg_pipeline_state
                    .fetcher
                    .mark_live_lcdc3_write_for_current_background_fetch(previous_lcdc, self.lcdc);
            }
        }

        if !self.is_lcd_enabled() {
            self.sync_visible_registers();
            self.sync_pipeline_registers();
        }
    }

    pub fn apply_startup_state(&mut self, startup_state: PpuStartupState) {
        self.lcdc = startup_state.lcdc;
        self.stat_interrupt_enable = startup_state.stat & STAT_WRITABLE_ENABLE_MASK;
        self.lcd_state = lcd_state_from_lcdc(self.lcdc);
        self.lcd_enable_pending_delay_tcycles = 0;
        self.visible_output = visible_output_for_lcd_state(self.lcd_state);
        self.scy = startup_state.scy;
        self.scx = startup_state.scx;
        self.ly = startup_state.ly;
        self.line_dot = 0;
        self.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        self.lyc = startup_state.lyc;
        self.bgp = startup_state.bgp;
        self.wy = startup_state.wy;
        self.wx = startup_state.wx;
        self.obp0 = None;
        self.obp1 = None;
        self.obj_palette_read_policy = startup_state.obj_palette_read_policy;
        self.blank_frame_active = false;
        self.oam_corruption_controller = OamCorruptionController;
        self.mode2_scan_state.reset();
        self.window_state.reset();
        self.bg_pipeline_state.reset();
        self.obj_pipeline_state.reset();
        self.pending_interrupts = 0;
        self.system_stop_active = false;
        self.current_scanline_pixels.fill(0);
        self.current_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
        self.framebuffer.fill(0);
        self.sync_visible_registers();
        self.sync_pipeline_registers();
        self.startup_mode_latch = if self.lcd_state.is_enabled() {
            let startup_mode = PpuAccessMode::from_stat_bits(startup_state.stat);
            let derived_mode =
                access_mode_from_raster(self.ly, self.line_dot, self.current_mode0_start_dot());
            (startup_mode != derived_mode).then_some(startup_mode)
        } else {
            None
        };
        self.stat_state.lcd_disabled_lyc_coincidence = startup_state.ly == startup_state.lyc;
        self.stat_state.irq_line = self.compute_stat_irq_line(false);
    }

    #[cfg(test)]
    pub(crate) fn tick_t_cycle(
        &mut self,
        context: &mut CycleContext,
        oam: OamBusView<'_>,
        vram: VramBusView<'_>,
        dma_oam_active: bool,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) {
        let mut observer = NoopPpuStepObserver;
        self.tick_t_cycle_with_observer(
            context,
            oam,
            vram,
            dma_oam_active,
            dma_oam_conflict,
            &mut observer,
        );
    }

    pub(crate) fn tick_t_cycle_with_observer<O>(
        &mut self,
        _context: &mut CycleContext,
        oam: OamBusView<'_>,
        vram: VramBusView<'_>,
        dma_oam_active: bool,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
        observer: &mut O,
    ) where
        O: PpuStepObserver,
    {
        debug_assert_eq!(oam.master(), BusMaster::Ppu);
        debug_assert_eq!(vram.master(), BusMaster::Ppu);
        debug_assert_eq!(
            oam.is_acquired_by_master(),
            self.is_lcd_enabled()
                && matches!(
                    self.current_bus_access_mode(),
                    PpuAccessMode::OamScan | PpuAccessMode::Drawing
                )
        );
        debug_assert_eq!(
            oam.is_acquired(),
            oam.is_acquired_by_master() || dma_oam_active
        );
        debug_assert_eq!(
            vram.is_acquired_by_master(),
            self.is_lcd_enabled() && self.current_bus_access_mode() == PpuAccessMode::Drawing
        );

        if !self.is_lcd_enabled() {
            if self.lcd_enable_pending_delay_tcycles > 0 {
                self.lcd_enable_pending_delay_tcycles -= 1;
                if self.lcd_enable_pending_delay_tcycles == 2 {
                    self.refresh_stat_irq_line(false);
                    return;
                }

                if self.lcd_enable_pending_delay_tcycles == 0 && self.lcdc & LCDC_ENABLE_BIT != 0 {
                    self.enter_lcd_enabled_restart_state();
                    self.refresh_stat_irq_line(false);
                } else {
                    return;
                }
            } else {
                return;
            }
        }

        let step_region = self.current_step_region_after_line_advance();
        let previous_mode = observe_ppu_step_region(observer, step_region, || {
            self.sync_pipeline_registers();
            self.sync_visible_registers();
            let previous_mode = self.current_access_mode();
            self.startup_mode_latch = None;
            self.line_dot += 1;
            self.advance_lcd_restart_phase();
            self.prepare_visible_scanline_state();
            previous_mode
        });
        observe_ppu_step_region(observer, PpuStepRegion::Mode2Scan, || {
            self.advance_mode2_scan(&oam, dma_oam_active);
        });
        self.advance_mode3_pipeline(&oam, &vram, dma_oam_conflict, observer);

        observe_ppu_step_region(observer, step_region, || {
            if self.line_dot == self.current_scanline_length() {
                let wraps_to_frame_start = self.ly + 1 == TOTAL_SCANLINES;
                if self.bg_pipeline_state.window_started_this_line {
                    self.window_state.window_line_counter =
                        self.window_state.window_line_counter.wrapping_add(1);
                }
                self.line_dot = 0;
                self.ly = if self.ly + 1 == TOTAL_SCANLINES {
                    0
                } else {
                    self.ly + 1
                };
                self.advance_lcd_restart_phase();
                if self.ly >= VISIBLE_SCANLINES {
                    self.window_state.reset();
                }
                self.mode2_scan_state.reset_scanline();
                self.bg_pipeline_state.reset();
                self.obj_pipeline_state.reset();
                self.current_scanline_pixels.fill(0);
                self.current_scanline_mixed_pixels
                    .fill(MixedPixel::background(0));
                if wraps_to_frame_start && self.blank_frame_active {
                    self.blank_frame_active = false;
                    self.refresh_visible_output();
                }
            }

            let current_mode = self.current_access_mode();
            if previous_mode != PpuAccessMode::VBlank && current_mode == PpuAccessMode::VBlank {
                self.queue_interrupt_request(InterruptSource::VBlank);
            }
            self.refresh_stat_irq_line(false);
        });
    }

    pub fn snapshot(&self) -> PpuSnapshot {
        let raster_state = self.current_raster_state();
        let mode = raster_state.access_mode();
        let current_transfer = self.current_transfer();
        let current_transfer_plan = current_transfer.map(Mode3CurrentTransfer::service_plan);

        PpuSnapshot {
            console_model: self.console_model,
            status: self.status,
            lcdc: self.lcdc,
            stat_interrupt_enable: self.stat_interrupt_enable,
            lyc_coincidence: self.effective_lyc_coincidence(),
            stat_irq_line: self.stat_state.irq_line,
            blank_frame_active: self.blank_frame_active,
            lcd_state: self.lcd_state,
            visible_output: self.visible_output,
            mode,
            scy: self.scy,
            scx: self.scx,
            ly: self.ly,
            line_dot: self.line_dot,
            mode_dot: raster_state.mode_dot(),
            mode0_start_dot: self.current_mode0_start_dot(),
            current_oam_scan_row: self.current_mode2_oam_row(),
            mode2_scanned_entries: self.mode2_scan_state.scanned_entries(),
            selected_sprites: self.mode2_scan_state.selected_sprites_snapshot(),
            bg_fetcher_source: self.bg_pipeline_state.fetcher.source,
            bg_fetcher_stage: self.bg_pipeline_state.fetcher.stage,
            bg_fetcher_stage_dot: self.bg_pipeline_state.fetcher.stage_dot,
            bg_fetcher_tile_map_address: self.bg_pipeline_state.fetcher.tile_map_address,
            bg_fetcher_tile_data_address: self.bg_pipeline_state.fetcher.tile_data_address,
            bg_push_pending: self.bg_pipeline_state.push.pending,
            bg_fill_pending: self.bg_pipeline_state.fill.pending,
            bg_fifo_pixels: self.bg_pipeline_state.fifo.iter().copied().collect(),
            bg_fifo_cached_pixels: self
                .bg_pipeline_state
                .fifo_cached_pixels
                .iter()
                .copied()
                .map(snapshot_bg_fifo_cached_pixel)
                .collect(),
            bg_startup_source_state: snapshot_bg_startup_source_state(
                self.bg_pipeline_state.startup_source_state,
            ),
            bg_startup_fetch_seam: snapshot_bg_startup_fetch_seam(
                self.bg_pipeline_state.startup_fetch_seam,
            ),
            bg_startup_fifo_placeholders: self.bg_pipeline_state.startup_fifo_placeholders,
            bg_push_entry_delay_remaining: self.bg_pipeline_state.push.entry_delay_remaining,
            bg_fill_startup_dummy_pixels: self.bg_pipeline_state.fill.startup_dummy_pixels,
            bg_fetcher_post_alignment_restart_delay_dots: self
                .bg_pipeline_state
                .fetcher
                .post_alignment_fetch_restart_delay_dots,
            bg_transfer_phase: snapshot_bg_transfer_phase(self.bg_pipeline_state.transfer_phase),
            bg_current_transfer_x: self.bg_pipeline_state.current_transfer_x,
            bg_current_transfer_lane: current_transfer
                .map(|transfer| snapshot_bg_transfer_lane(transfer.context.lane)),
            bg_current_transfer_source_window: current_transfer
                .map(|transfer| snapshot_bg_transfer_source_window(transfer.context.source_window)),
            bg_current_transfer_backing: current_transfer_plan
                .map(|plan| snapshot_bg_transfer_backing(plan.backing)),
            bg_current_transfer_readiness: current_transfer
                .map(|transfer| snapshot_bg_transfer_readiness(transfer.readiness)),
            bg_current_transfer_kind: current_transfer_plan
                .map(|plan| snapshot_bg_transfer_kind(plan.result_kind)),
            obj_fetcher_stage: self.obj_pipeline_state.fetch.stage,
            obj_fetcher_stage_dot: self.obj_pipeline_state.fetch.stage_dot,
            obj_pending_hit_match_x: self.obj_pipeline_state.pending_match_x,
            obj_pending_hit_len: self.obj_pipeline_state.pending_sprite_slots.len(),
            obj_pending_hit_front_sprite_slot: self
                .obj_pipeline_state
                .pending_sprite_slots
                .front()
                .copied(),
            obj_fifo_pixels: self
                .obj_pipeline_state
                .fifo
                .iter()
                .map(|pixel| (!pixel.is_transparent()).then_some(pixel.color))
                .collect(),
            scx_discard_remaining: self.bg_pipeline_state.scx_discard_remaining,
            visible_pixels_output: self.bg_pipeline_state.visible_pixels_output,
            window_wy_latch: self.bg_pipeline_state.window_wy_latch,
            window_started_this_line: self.bg_pipeline_state.window_started_this_line,
            window_line_counter: self.window_state.window_line_counter,
            current_scanline_pixels: self.current_scanline_pixels.to_vec(),
            lyc: self.lyc,
            bgp: self.bgp,
            obp0: self.obp0,
            obp1: self.obp1,
            wy: self.wy,
            wx: self.wx,
            visible_lcdc: self.visible_registers.lcdc,
            visible_scy: self.visible_registers.scy,
            visible_scx: self.visible_registers.scx,
            visible_bgp: self.visible_registers.bgp,
            visible_obp0: self.visible_registers.obp0,
            visible_obp1: self.visible_registers.obp1,
            visible_wy: self.visible_registers.wy,
            visible_wx: self.visible_registers.wx,
            pipeline_lcdc: self.pipeline_registers.lcdc,
            pipeline_scy: self.pipeline_registers.scy,
            pipeline_scx: self.pipeline_registers.scx,
            pipeline_bgp: self.pipeline_registers.bgp,
            pipeline_obp0: self.pipeline_registers.obp0,
            pipeline_obp1: self.pipeline_registers.obp1,
            pipeline_wy: self.pipeline_registers.wy,
            pipeline_wx: self.pipeline_registers.wx,
            obj_palette_read_policy: self.obj_palette_read_policy,
        }
    }

    pub fn ly(&self) -> u8 {
        self.ly
    }

    pub fn line_dot(&self) -> u16 {
        self.line_dot
    }

    pub fn mode0_start_dot(&self) -> u16 {
        self.current_mode0_start_dot()
    }

    pub fn access_mode(&self) -> PpuAccessMode {
        self.current_access_mode()
    }

    pub fn mode_dot(&self) -> u16 {
        self.current_raster_state().mode_dot()
    }

    pub fn lcd_state(&self) -> PpuLcdState {
        self.lcd_state
    }

    pub fn is_blank_frame_active(&self) -> bool {
        self.blank_frame_active
    }

    pub fn is_restart_first_line_active(&self) -> bool {
        self.lcd_restart_phase
            .is_first_line_after_enable_active(self.ly)
    }

    pub fn is_startup_mode0_window_active(&self) -> bool {
        self.is_restart_first_line_active()
    }

    fn current_scanline_length(&self) -> u16 {
        if self.is_restart_first_line_active() {
            LCD_REENABLE_LINE0_TOTAL_DOTS
        } else {
            DOTS_PER_SCANLINE
        }
    }

    fn current_ly_read_advance_start_dot(&self) -> u16 {
        if self.is_restart_first_line_active() {
            LCD_REENABLE_LINE0_LY_READ_ADVANCE_START_DOT
        } else {
            LY_READ_ADVANCE_START_DOT
        }
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    pub(crate) fn set_system_stop_active(&mut self, active: bool) {
        if self.system_stop_active == active {
            return;
        }

        self.system_stop_active = active;
        if active {
            self.clear_visible_buffers();
        }
        self.refresh_visible_output();
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        let bg_fifo_front_cached = self
            .bg_pipeline_state
            .fifo_cached_pixels
            .iter()
            .flatten()
            .next()
            .copied();
        let current_transfer = self.current_transfer();
        let current_transfer_plan = current_transfer.map(Mode3CurrentTransfer::service_plan);
        format!(
            concat!(
                "t_cycle={} phase={} console_model={:?} status={:?} ",
                "lcd_state={:?} visible_output={:?} ly={} lyc={} coincidence={} ",
                "line_dot={} mode={:?} stat_irq_line={} mode2_scanned_entries={} selected_sprites={} ",
                "bg_source={:?} bg_stage={:?} bg_stage_dot={} bg_fetch_origin={:?} ",
                "bg_push_pending={} bg_push_entry_delay_remaining={} bg_push_origin={:?} ",
                "bg_fill_pending={} bg_fill_startup_dummy_pixels={} bg_fill_origin={:?} ",
                "bg_fifo_len={} bg_startup_fifo_placeholders={} bg_fifo_front_cached_origin={:?} ",
                "bg_fifo_front_cached_fetch_x={:?} bg_fifo_front_cached_pixel_index={:?} ",
                "bg_startup_source_state={:?} bg_startup_fetch_seam={:?} ",
                "bg_fetcher_post_alignment_restart_delay_dots={} bg_transfer_phase={:?} ",
                "bg_current_transfer_x={} bg_current_transfer_lane={:?} ",
                "bg_current_transfer_source_window={:?} bg_current_transfer_backing={:?} ",
                "bg_current_transfer_readiness={:?} bg_current_transfer_kind={:?} ",
                "visible_pixels_output={}"
            ),
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.lcd_state,
            self.visible_output,
            self.ly,
            self.lyc,
            self.effective_lyc_coincidence(),
            self.line_dot,
            self.current_access_mode(),
            self.stat_state.irq_line,
            self.mode2_scan_state.scanned_entries(),
            self.mode2_scan_state.selected_sprite_count(),
            self.bg_pipeline_state.fetcher.source,
            self.bg_pipeline_state.fetcher.stage,
            self.bg_pipeline_state.fetcher.stage_dot,
            self.bg_pipeline_state.fetcher.cached_origin,
            self.bg_pipeline_state.push.pending,
            self.bg_pipeline_state.push.entry_delay_remaining,
            self.bg_pipeline_state.push.cached.origin,
            self.bg_pipeline_state.fill.pending,
            self.bg_pipeline_state.fill.startup_dummy_pixels,
            self.bg_pipeline_state.fill.cached.origin,
            self.bg_pipeline_state.fifo.len(),
            self.bg_pipeline_state.startup_fifo_placeholders,
            bg_fifo_front_cached.map(|pixel| pixel.cached.origin),
            bg_fifo_front_cached.map(|pixel| pixel.cached.fetch_x),
            bg_fifo_front_cached.map(|pixel| pixel.pixel_index),
            self.bg_pipeline_state.startup_source_state,
            self.bg_pipeline_state.startup_fetch_seam,
            self.bg_pipeline_state
                .fetcher
                .post_alignment_fetch_restart_delay_dots,
            self.bg_pipeline_state.transfer_phase,
            self.bg_pipeline_state.current_transfer_x,
            current_transfer.map(|transfer| transfer.context.lane),
            current_transfer.map(|transfer| transfer.context.source_window),
            current_transfer_plan.map(|plan| plan.backing),
            current_transfer.map(|transfer| snapshot_bg_transfer_readiness(transfer.readiness)),
            current_transfer_plan.map(|plan| snapshot_bg_transfer_kind(plan.result_kind)),
            self.bg_pipeline_state.visible_pixels_output,
        )
    }

    pub fn mmio_commit_trace_message(
        &self,
        context: &CycleContext,
        address: u16,
        value: u8,
    ) -> String {
        format!(
            concat!(
                "t_cycle={} phase={} console_model={:?} status={:?} ",
                "committed_write={:#06X}<-{:#04X} lcdc={:#04X} stat={:#04X} ",
                "scy={:#04X} scx={:#04X} ly={} lyc={:#04X} bgp={:#04X} wy={:#04X} wx={:#04X}"
            ),
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            address,
            value,
            self.read_register(0xFF40),
            self.read_register(0xFF41),
            self.read_register(0xFF42),
            self.read_register(0xFF43),
            self.read_register(0xFF44),
            self.read_register(0xFF45),
            self.read_register(0xFF47),
            self.read_register(0xFF4A),
            self.read_register(0xFF4B),
        )
    }
}

include!("ppu/control.rs");

include!("ppu/mode3.rs");

include!("ppu/pipeline.rs");

impl Ppu {
    pub(crate) fn drain_pending_interrupt_requests(&mut self) -> Vec<InterruptSource> {
        let mut requests = Vec::with_capacity(2);
        if self.pending_interrupts & PPU_PENDING_VBLANK_INTERRUPT_BIT != 0 {
            requests.push(InterruptSource::VBlank);
        }
        if self.pending_interrupts & PPU_PENDING_LCD_STAT_INTERRUPT_BIT != 0 {
            requests.push(InterruptSource::LcdStat);
        }
        self.pending_interrupts = 0;
        requests
    }

    pub(crate) fn pending_interrupt_request_mask(&self) -> u8 {
        let mut mask = 0;
        if self.pending_interrupts & PPU_PENDING_VBLANK_INTERRUPT_BIT != 0 {
            mask |= 0x01;
        }
        if self.pending_interrupts & PPU_PENDING_LCD_STAT_INTERRUPT_BIT != 0 {
            mask |= 0x02;
        }
        mask
    }
}

include!("ppu/state.rs");

const fn snapshot_bg_fifo_cached_origin(
    origin: BgCachedSliceOrigin,
) -> PpuBgCachedSliceOriginSnapshot {
    match origin {
        BgCachedSliceOrigin::Ordinary => PpuBgCachedSliceOriginSnapshot::Ordinary,
        BgCachedSliceOrigin::StartupAlignmentSeed => {
            PpuBgCachedSliceOriginSnapshot::StartupAlignmentSeed
        }
        BgCachedSliceOrigin::StartupAlignmentFill => {
            PpuBgCachedSliceOriginSnapshot::StartupAlignmentFill
        }
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2) => {
            PpuBgCachedSliceOriginSnapshot::StartupContinuationVisibleTile2
        }
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3) => {
            PpuBgCachedSliceOriginSnapshot::StartupContinuationVisibleTile3
        }
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::None) => {
            PpuBgCachedSliceOriginSnapshot::Ordinary
        }
    }
}

fn snapshot_bg_fifo_cached_pixel(
    cached: Option<BgFifoPixelCached>,
) -> Option<PpuBgFifoCachedPixelSnapshot> {
    let cached = cached?;
    Some(PpuBgFifoCachedPixelSnapshot {
        source: cached.cached.source,
        origin: snapshot_bg_fifo_cached_origin(cached.cached.origin),
        fetch_x: cached.cached.fetch_x,
        pixel_index: cached.pixel_index,
        same_cycle_live_tilemap_refetch_window_open: cached
            .cached
            .same_cycle_live_tilemap_refetch_window_open,
        needs_live_tilemap_refetch: cached.cached.needs_live_tilemap_refetch,
        needs_live_tile_data_refetch: cached.cached.needs_live_tile_data_refetch,
        needs_live_tile_data_unsigned_reuse: cached.cached.needs_live_tile_data_unsigned_reuse,
        tile_map_address: cached.cached.tile_map_address,
        tile_data_address: cached.cached.tile_data_address,
        tile_index: cached.cached.tile_index,
    })
}

const fn snapshot_bg_transfer_phase(phase: Mode3TransferPhase) -> PpuMode3TransferPhaseSnapshot {
    match phase {
        Mode3TransferPhase::Priming => PpuMode3TransferPhaseSnapshot::Priming,
        Mode3TransferPhase::Output => PpuMode3TransferPhaseSnapshot::Output,
    }
}

const fn snapshot_bg_transfer_lane(lane: Mode3TransferLane) -> PpuMode3TransferLaneSnapshot {
    match lane {
        Mode3TransferLane::PreVisible => PpuMode3TransferLaneSnapshot::PreVisible,
        Mode3TransferLane::Hidden => PpuMode3TransferLaneSnapshot::Hidden,
        Mode3TransferLane::Visible => PpuMode3TransferLaneSnapshot::Visible,
    }
}

const fn snapshot_bg_transfer_source_window(
    source_window: Mode3TransferSourceWindow,
) -> PpuMode3TransferSourceWindowSnapshot {
    match source_window {
        Mode3TransferSourceWindow::AbstractStartup => {
            PpuMode3TransferSourceWindowSnapshot::AbstractStartup
        }
        Mode3TransferSourceWindow::FifoBacked => PpuMode3TransferSourceWindowSnapshot::FifoBacked,
    }
}

const fn snapshot_bg_transfer_backing(
    backing: Mode3TransferBacking,
) -> PpuMode3TransferBackingSnapshot {
    match backing {
        Mode3TransferBacking::Abstract => PpuMode3TransferBackingSnapshot::Abstract,
        Mode3TransferBacking::FifoBacked => PpuMode3TransferBackingSnapshot::FifoBacked,
    }
}

const fn snapshot_bg_transfer_readiness(
    readiness: Mode3TransferReadiness,
) -> PpuMode3TransferReadinessSnapshot {
    match readiness {
        Mode3TransferReadiness::WaitingForFifo(_) => {
            PpuMode3TransferReadinessSnapshot::WaitingForFifo
        }
        Mode3TransferReadiness::Ready(_) => PpuMode3TransferReadinessSnapshot::Ready,
    }
}

const fn snapshot_bg_transfer_kind(kind: Mode3TransferDotKind) -> PpuMode3TransferDotKindSnapshot {
    match kind {
        Mode3TransferDotKind::NotServed => PpuMode3TransferDotKindSnapshot::NotServed,
        Mode3TransferDotKind::ServedPreVisibleTransfer => {
            PpuMode3TransferDotKindSnapshot::ServedPreVisibleTransfer
        }
        Mode3TransferDotKind::ServedHiddenTransfer => {
            PpuMode3TransferDotKindSnapshot::ServedHiddenTransfer
        }
        Mode3TransferDotKind::ServedVisiblePixel => {
            PpuMode3TransferDotKindSnapshot::ServedVisiblePixel
        }
    }
}

const fn snapshot_bg_startup_source_state(
    state: Mode3StartupSourceState,
) -> PpuMode3StartupSourceStateSnapshot {
    match state {
        Mode3StartupSourceState::EntryDelay { remaining } => {
            PpuMode3StartupSourceStateSnapshot::EntryDelay { remaining }
        }
        Mode3StartupSourceState::Abstract { remaining } => {
            PpuMode3StartupSourceStateSnapshot::Abstract { remaining }
        }
        Mode3StartupSourceState::FifoBacked => PpuMode3StartupSourceStateSnapshot::FifoBacked,
    }
}

const fn snapshot_bg_startup_continuation_slice(
    slice: BgStartupContinuationSlice,
) -> PpuBgStartupContinuationSliceSnapshot {
    match slice {
        BgStartupContinuationSlice::None => PpuBgStartupContinuationSliceSnapshot::None,
        BgStartupContinuationSlice::VisibleTile2 => {
            PpuBgStartupContinuationSliceSnapshot::VisibleTile2
        }
        BgStartupContinuationSlice::VisibleTile3 => {
            PpuBgStartupContinuationSliceSnapshot::VisibleTile3
        }
    }
}

const fn snapshot_bg_startup_fetch_seam(
    seam: BgStartupFetchSeamState,
) -> PpuBgStartupFetchSeamSnapshot {
    match seam {
        BgStartupFetchSeamState::Inactive => PpuBgStartupFetchSeamSnapshot::Inactive,
        BgStartupFetchSeamState::AlignmentSeedPending => {
            PpuBgStartupFetchSeamSnapshot::AlignmentSeedPending
        }
        BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay,
            next_startup_continuation_slice,
            startup_continuation_visible_tiles_remaining,
            delayed_background_tileindex_read_tiles_remaining,
            delayed_background_tilemap_tiles_remaining,
            delayed_background_tiledata_tiles_remaining,
        } => PpuBgStartupFetchSeamSnapshot::PostAlignment {
            first_real_push_skips_entry_delay,
            next_startup_continuation_slice: snapshot_bg_startup_continuation_slice(
                next_startup_continuation_slice,
            ),
            startup_continuation_visible_tiles_remaining,
            delayed_background_tileindex_read_tiles_remaining,
            delayed_background_tilemap_tiles_remaining,
            delayed_background_tiledata_tiles_remaining,
        },
    }
}

const fn lcd_state_from_lcdc(lcdc: u8) -> PpuLcdState {
    if lcdc & LCDC_ENABLE_BIT != 0 {
        PpuLcdState::Enabled
    } else {
        PpuLcdState::Disabled
    }
}

const fn visible_output_for_lcd_state(lcd_state: PpuLcdState) -> PpuVisibleOutputState {
    if lcd_state.is_enabled() {
        PpuVisibleOutputState::Driving
    } else {
        PpuVisibleOutputState::ForcedBlank
    }
}

const fn access_mode_from_raster(ly: u8, line_dot: u16, mode0_start_dot: u16) -> PpuAccessMode {
    if ly >= VISIBLE_SCANLINES {
        PpuAccessMode::VBlank
    } else if line_dot < MODE2_DOTS {
        PpuAccessMode::OamScan
    } else if line_dot < mode0_start_dot {
        PpuAccessMode::Drawing
    } else {
        PpuAccessMode::HBlank
    }
}

const fn mode_dot_from_raster_mode(
    mode: PpuAccessMode,
    line_dot: u16,
    mode0_start_dot: u16,
) -> u16 {
    match mode {
        PpuAccessMode::VBlank => line_dot,
        PpuAccessMode::OamScan => line_dot,
        PpuAccessMode::Drawing => line_dot.saturating_sub(MODE2_DOTS),
        PpuAccessMode::HBlank => line_dot.saturating_sub(mode0_start_dot),
    }
}

const fn bg_tile_data_base(lcdc: u8, tile_index: u8) -> u16 {
    if lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0 {
        tile_index as u16 * TILE_BYTES
    } else {
        (0x1000_i16 + (tile_index as i8 as i16) * TILE_BYTES as i16) as u16
    }
}

const fn bg_tile_pixel_value(tile_low: u8, tile_high: u8, pixel_index: u8) -> u8 {
    let bit = BG_TILE_WIDTH - 1 - pixel_index;
    let low_bit = (tile_low >> bit) & 0x01;
    let high_bit = (tile_high >> bit) & 0x01;
    (high_bit << 1) | low_bit
}

fn read_oam_sprite(oam: &OamBusView<'_>, oam_index: u8) -> Option<PpuSelectedSprite> {
    let entry_start = oam_index as usize * OAM_ENTRY_BYTES;
    Some(PpuSelectedSprite {
        oam_index,
        y: oam.read(entry_start)?,
        x: oam.read(entry_start + 1)?,
        tile_index: oam.read(entry_start + 2)?,
        attributes: oam.read(entry_start + 3)?,
    })
}

fn read_obj_fetch_sprite_metadata(
    oam: &OamBusView<'_>,
    sprite: PpuSelectedSprite,
    dma_oam_conflict: Option<PpuDmaOamConflict>,
) -> (u8, u8) {
    let nominal_word_address = 0xFE00_u16 + sprite.oam_index as u16 * OAM_ENTRY_BYTES as u16 + 2;
    let word_address = dma_oam_conflict
        .filter(|conflict| (0xFE00..=0xFE9F).contains(&conflict.address))
        .map(PpuDmaOamConflict::word_address)
        .unwrap_or(nominal_word_address);
    let word_offset = word_address.saturating_sub(0xFE00) as usize;
    let mut metadata = [
        oam.read(word_offset).unwrap_or(sprite.tile_index),
        oam.read(word_offset + 1).unwrap_or(sprite.attributes),
    ];
    if let Some(conflict) =
        dma_oam_conflict.filter(|conflict| conflict.word_address() == word_address)
    {
        metadata[conflict.byte_offset_in_word()] = conflict.value();
    }

    (metadata[0], metadata[1])
}

fn sprite_matches_line(sprite: PpuSelectedSprite, ly: u8, height: u8) -> bool {
    let current_line = ly as u16 + 16;
    let sprite_y = sprite.y as u16;
    let sprite_bottom = sprite_y + height as u16;

    current_line >= sprite_y && current_line < sprite_bottom
}

fn read_oam_row(oam_bytes: &[u8], row: u8) -> [u8; OAM_CORRUPTION_ROW_BYTES] {
    let row_start = row as usize * OAM_CORRUPTION_ROW_BYTES;
    let mut row_bytes = [0; OAM_CORRUPTION_ROW_BYTES];
    row_bytes.copy_from_slice(&oam_bytes[row_start..row_start + OAM_CORRUPTION_ROW_BYTES]);
    row_bytes
}

fn write_oam_row(oam_bytes: &mut [u8], row: u8, row_bytes: [u8; OAM_CORRUPTION_ROW_BYTES]) {
    let row_start = row as usize * OAM_CORRUPTION_ROW_BYTES;
    oam_bytes[row_start..row_start + OAM_CORRUPTION_ROW_BYTES].copy_from_slice(&row_bytes);
}

fn read_oam_word(oam_bytes: &[u8], row: u8, word_index: usize) -> u16 {
    let word_start = row as usize * OAM_CORRUPTION_ROW_BYTES + word_index * 2;
    u16::from_le_bytes([oam_bytes[word_start], oam_bytes[word_start + 1]])
}

fn write_oam_word(oam_bytes: &mut [u8], row: u8, word_index: usize, value: u16) {
    let word_start = row as usize * OAM_CORRUPTION_ROW_BYTES + word_index * 2;
    let [low, high] = value.to_le_bytes();
    oam_bytes[word_start] = low;
    oam_bytes[word_start + 1] = high;
}

fn copy_previous_row_tail(oam_bytes: &mut [u8], current_row: u8) {
    for word_index in 1..OAM_CORRUPTION_ROW_WORDS {
        let previous_word = read_oam_word(oam_bytes, current_row - 1, word_index);
        write_oam_word(oam_bytes, current_row, word_index, previous_word);
    }
}

fn sprite_screen_x(sprite: PpuSelectedSprite) -> i16 {
    sprite.x as i16 - 8
}

fn sprite_trigger_x(sprite: PpuSelectedSprite) -> Option<u8> {
    if sprite.x >= 168 {
        return None;
    }

    Some(sprite.x)
}

fn obj_pixel_has_priority(candidate: ObjPixel, current: ObjPixel) -> bool {
    if current.is_transparent() {
        return !candidate.is_transparent();
    }
    if candidate.is_transparent() {
        return false;
    }

    candidate.sprite_x < current.sprite_x
        || (candidate.sprite_x == current.sprite_x && candidate.oam_index < current.oam_index)
}

#[cfg(test)]
mod tests;
