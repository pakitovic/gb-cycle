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
const LCD_REENABLE_INITIAL_LINE_DOT: u16 = 4;
const LCD_REENABLE_STARTUP_HBLANK_END_DOT: u16 = 20;
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
    StartupMode0Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PpuRasterState {
    Disabled,
    LcdRestartStartupMode0 {
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
            Self::Disabled | Self::LcdRestartStartupMode0 { .. } => PpuAccessMode::HBlank,
            Self::Active { mode, .. } => mode,
        }
    }

    const fn mode_dot(self) -> u16 {
        match self {
            Self::Disabled => 0,
            Self::LcdRestartStartupMode0 { mode_dot } | Self::Active { mode_dot, .. } => mode_dot,
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
    const fn startup_mode0_window() -> Self {
        Self::StartupMode0Window
    }

    const fn is_startup_mode0_window_active(self, ly: u8, line_dot: u16) -> bool {
        matches!(self, Self::StartupMode0Window)
            && ly == 0
            && line_dot < LCD_REENABLE_STARTUP_HBLANK_END_DOT
    }

    const fn raster_state(self, ly: u8, line_dot: u16) -> Option<PpuRasterState> {
        if self.is_startup_mode0_window_active(ly, line_dot) {
            Some(PpuRasterState::LcdRestartStartupMode0 {
                mode_dot: line_dot.saturating_sub(LCD_REENABLE_INITIAL_LINE_DOT),
            })
        } else {
            None
        }
    }

    const fn advance(self, ly: u8, line_dot: u16) -> Self {
        if self.is_startup_mode0_window_active(ly, line_dot) {
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
    pub obj_fetcher_stage: PpuObjFetcherStage,
    pub obj_fetcher_stage_dot: u8,
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
            PpuBusState::lcd_enabled(self.current_access_mode())
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            0xFF40 => self.read_lcdc(),
            0xFF41 => self.read_stat(),
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
        let previous_lcdc = self.lcdc;
        match address {
            0xFF40 => self.write_lcdc(value),
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
                    self.current_access_mode(),
                    PpuAccessMode::OamScan | PpuAccessMode::Drawing
                )
        );
        debug_assert_eq!(
            oam.is_acquired(),
            oam.is_acquired_by_master() || dma_oam_active
        );
        debug_assert_eq!(
            vram.is_acquired_by_master(),
            self.is_lcd_enabled() && self.current_access_mode() == PpuAccessMode::Drawing
        );

        if !self.is_lcd_enabled() {
            return;
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
            if self.line_dot == DOTS_PER_SCANLINE {
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
            obj_fetcher_stage: self.obj_pipeline_state.fetch.stage,
            obj_fetcher_stage_dot: self.obj_pipeline_state.fetch.stage_dot,
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

    pub fn is_blank_frame_active(&self) -> bool {
        self.blank_frame_active
    }

    pub fn is_startup_mode0_window_active(&self) -> bool {
        self.lcd_restart_phase
            .is_startup_mode0_window_active(self.ly, self.line_dot)
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
        format!(
            concat!(
                "t_cycle={} phase={} console_model={:?} status={:?} ",
                "lcd_state={:?} visible_output={:?} ly={} lyc={} coincidence={} ",
                "line_dot={} mode={:?} stat_irq_line={} mode2_scanned_entries={} selected_sprites={}"
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

    fn is_lcd_enabled(&self) -> bool {
        self.lcd_state.is_enabled()
    }

    fn sync_visible_registers(&mut self) {
        self.visible_registers = PpuVisibleRegisters {
            lcdc: self.lcdc,
            scy: self.scy,
            scx: self.scx,
            bgp: self.bgp,
            obp0: self.obp0,
            obp1: self.obp1,
            wy: self.wy,
            wx: self.wx,
        };
    }

    fn sync_pipeline_registers(&mut self) {
        self.pipeline_registers = self.visible_registers;
    }

    fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    fn write_lcdc(&mut self, value: u8) {
        let was_lcd_enabled = self.is_lcd_enabled();
        self.lcdc = value;
        self.startup_mode_latch = None;

        match (was_lcd_enabled, value & LCDC_ENABLE_BIT != 0) {
            (true, false) => self.enter_lcd_disabled_state(),
            (false, true) => self.enter_lcd_enabled_restart_state(),
            _ => {
                self.lcd_state = lcd_state_from_lcdc(value);
                self.refresh_visible_output();
            }
        }

        self.refresh_stat_irq_line(false);
    }

    fn read_stat(&self) -> u8 {
        STAT_FORCED_HIGH_BIT
            | self.stat_interrupt_enable
            | if self.effective_lyc_coincidence() {
                0x04
            } else {
                0x00
            }
            | if self.is_lcd_enabled() {
                self.current_access_mode().stat_bits()
            } else {
                PpuAccessMode::HBlank.stat_bits()
            }
    }

    fn write_stat(&mut self, value: u8) {
        self.stat_interrupt_enable = value & STAT_WRITABLE_ENABLE_MASK;
        self.refresh_stat_irq_line(self.stat_write_quirk_active());
    }

    fn read_ly(&self) -> u8 {
        if self.is_lcd_enabled()
            && !self.blank_frame_active
            && self.line_dot >= LY_READ_ADVANCE_START_DOT
            && self.ly + 1 < TOTAL_SCANLINES
        {
            self.ly + 1
        } else {
            self.ly
        }
    }

    fn current_access_mode(&self) -> PpuAccessMode {
        self.current_raster_state().access_mode()
    }

    fn current_raster_state(&self) -> PpuRasterState {
        if !self.is_lcd_enabled() {
            return PpuRasterState::Disabled;
        }

        if let Some(raster_state) = self.lcd_restart_phase.raster_state(self.ly, self.line_dot) {
            return raster_state;
        }

        let mode0_start_dot = self.current_mode0_start_dot();
        let mode = self
            .startup_mode_latch
            .unwrap_or_else(|| access_mode_from_raster(self.ly, self.line_dot, mode0_start_dot));

        PpuRasterState::Active {
            mode,
            mode_dot: mode_dot_from_raster_mode(mode, self.line_dot, mode0_start_dot),
            mode2_scan_active: self.ly < VISIBLE_SCANLINES
                && self.line_dot != 0
                && self.line_dot <= MODE2_DOTS,
        }
    }

    fn current_mode0_start_dot(&self) -> u16 {
        if self.ly >= VISIBLE_SCANLINES {
            return MODE0_START_DOT;
        }

        if self.bg_pipeline_state.mode3_started {
            self.bg_pipeline_state.mode0_start_dot
        } else {
            MODE0_START_DOT + u16::from(self.visible_registers.scx & 0x07)
        }
    }

    pub(crate) fn current_mode2_oam_row(&self) -> Option<u8> {
        if !self.is_lcd_enabled()
            || self.ly >= VISIBLE_SCANLINES
            || self.current_access_mode() != PpuAccessMode::OamScan
        {
            return None;
        }

        Some((self.line_dot / OAM_CORRUPTION_DOTS_PER_ROW) as u8)
    }

    pub(crate) fn apply_oam_corruption_event(
        &mut self,
        event: OamCorruptionEventKind,
        oam_bytes: &mut [u8],
    ) -> bool {
        let Some(row) = self.current_mode2_oam_row() else {
            return false;
        };

        self.oam_corruption_controller
            .apply(self.console_model, row, event, oam_bytes)
    }

    fn advance_mode2_scan(&mut self, oam: &OamBusView<'_>, dma_oam_active: bool) {
        let raster_state = self.current_raster_state();

        if self.ly >= VISIBLE_SCANLINES
            || !raster_state.is_mode2_scan()
            || self.line_dot == 0
            || !self.line_dot.is_multiple_of(MODE2_T_CYCLES_PER_OAM_ENTRY)
            || self.mode2_scan_state.scanned_entries() >= OAM_SPRITE_COUNT
        {
            return;
        }

        let oam_index = self.mode2_scan_state.scanned_entries();
        self.mode2_scan_state.increment_scanned_entries();

        if self.mode2_scan_state.is_full() {
            return;
        }

        let nominal_sprite = read_oam_sprite(oam, oam_index);
        let sprite = if dma_oam_active && self.console_model.is_dmg_family() {
            let Some((y, x)) = self.mode2_scan_state.latched_mode2_yx_word() else {
                return;
            };
            let (tile_index, attributes) = nominal_sprite
                .map(|sprite| (sprite.tile_index, sprite.attributes))
                .unwrap_or((0xFF, 0xFF));
            PpuSelectedSprite {
                oam_index,
                y,
                x,
                tile_index,
                attributes,
            }
        } else {
            let sprite = match nominal_sprite {
                Some(sprite) => sprite,
                None => return,
            };
            self.mode2_scan_state
                .latch_mode2_yx_word(sprite.y, sprite.x);
            sprite
        };

        if sprite_matches_line(sprite, self.ly, self.current_obj_height()) {
            self.mode2_scan_state.push(sprite);
        }
    }

    fn current_obj_height(&self) -> u8 {
        self.visible_registers.obj_height()
    }

    fn window_activation_registers(&self) -> PpuVisibleRegisters {
        if self.console_model.is_dmg_family() {
            self.pipeline_registers
        } else {
            self.visible_registers
        }
    }

    fn pixel_pipeline_lcdc(&self) -> u8 {
        if !self.console_model.is_dmg_family() {
            return self.visible_registers.lcdc;
        }

        if self.bg_pipeline_state.current_transfer_x == 8 {
            self.visible_registers.lcdc
        } else {
            self.pipeline_registers.lcdc
        }
    }

    fn pixel_pipeline_bgp(&self) -> u8 {
        if self.console_model.is_dmg_family() {
            self.visible_registers.bgp | self.pipeline_registers.bgp
        } else {
            self.visible_registers.bgp
        }
    }

    fn pixel_transfer_bg_enabled(&self) -> bool {
        self.pixel_pipeline_lcdc() & LCDC_BG_ENABLE_BIT != 0
    }

    fn pixel_transfer_obj_enabled(&self) -> bool {
        self.pixel_pipeline_lcdc() & LCDC_OBJ_ENABLE_BIT != 0
    }

    fn prepare_visible_scanline_state(&mut self) {
        if self.line_dot != 1 || self.ly >= VISIBLE_SCANLINES {
            return;
        }

        if self.visible_registers.wy < VISIBLE_SCANLINES && self.ly == self.visible_registers.wy {
            self.window_state.wy_triggered = true;
        }

        let wy_latch =
            self.window_state.wy_triggered && self.visible_registers.wy < VISIBLE_SCANLINES;
        let force_x0_this_line = wy_latch && self.window_state.pending_wx166_next_line;
        self.window_state.pending_wx166_next_line = false;
        self.bg_pipeline_state
            .prepare_window_line(wy_latch, force_x0_this_line);
    }

    fn advance_mode3_pipeline<O>(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
        observer: &mut O,
    ) where
        O: PpuStepObserver,
    {
        if self.ly >= VISIBLE_SCANLINES
            || self.line_dot < MODE2_DOTS
            || self.line_dot >= self.current_mode0_start_dot()
        {
            return;
        }

        if !self.bg_pipeline_state.mode3_started {
            observe_ppu_step_region(observer, PpuStepRegion::Mode3Startup, || {
                self.bg_pipeline_state
                    .start_line(self.visible_registers.scx);
            });
        }

        let bg_pipeline_region = self.current_mode3_bg_pipeline_region();
        observe_ppu_step_region(observer, bg_pipeline_region, || {
            self.maybe_recompute_pending_background_fill(vram);
            self.flush_pending_bg_fifo_fill();
        });

        if observe_ppu_step_region(observer, PpuStepRegion::Mode3ObjFetch, || {
            self.advance_mode3_object_phase(oam, vram, dma_oam_conflict)
        }) {
            return;
        }

        let output_dot =
            observe_ppu_step_region(observer, PpuStepRegion::Mode3PixelTransfer, || {
                self.advance_mode3_output_phase()
            });
        observe_ppu_step_region(observer, PpuStepRegion::Mode3WindowFetch, || {
            self.maybe_apply_wx0_shortening_after_transfer_dot(output_dot);
            let _ = self.maybe_start_window_after_transfer_dot(output_dot);
        });
        let bg_pipeline_region = self.current_mode3_bg_pipeline_region();
        let _ = observe_ppu_step_region(observer, bg_pipeline_region, || {
            self.advance_bg_fetcher(vram)
        });
    }

    fn advance_mode3_object_phase(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        self.sync_pending_obj_hit_ownership();
        self.latch_object_fetch_hits();
        self.try_start_object_fetch_from_current_dot(
            ObjFetchStartSource::FifoBackedTransfer,
            false,
        );
        self.advance_object_fetch(oam, vram, dma_oam_conflict)
    }

    fn advance_mode3_output_phase(&mut self) -> Mode3TransferDot {
        if self
            .bg_pipeline_state
            .consume_startup_transfer_entry_delay_dot()
        {
            return Mode3TransferDot::not_served();
        }

        let transfer_dot = if !self.current_dot_arbitration().can_serve_bg_transfer() {
            self.bg_pipeline_state.extend_mode3_by_one_dot();
            Mode3TransferDot::not_served()
        } else {
            match self.current_transfer() {
                None => return Mode3TransferDot::not_served(),
                Some(Mode3CurrentTransfer {
                    readiness: Mode3TransferReadiness::WaitingForFifo(_),
                    ..
                }) => {
                    self.bg_pipeline_state.extend_mode3_by_one_dot();
                    Mode3TransferDot::not_served()
                }
                Some(Mode3CurrentTransfer {
                    readiness: Mode3TransferReadiness::Ready(plan),
                    ..
                }) => self.execute_transfer_service_plan(plan),
            }
        };

        self.bg_pipeline_state.consume_startup_source_window_dot();
        transfer_dot
    }

    fn current_dot_has_pending_obj_hit(&self) -> bool {
        self.obj_enabled()
            && self
                .obj_pipeline_state
                .pending_hits_own_current_dot(self.current_obj_hit_ownership())
    }

    fn current_dot_arbitration(&self) -> Mode3DotArbitration {
        let has_pending_obj_hit = self.current_dot_has_pending_obj_hit();
        let obj_fetch_can_start = self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_enabled()
            && has_pending_obj_hit;
        let current_transfer_is_fifo_backed = self.current_transfer().is_some_and(|transfer| {
            transfer.can_start_obj_fetch_from_fifo_backed_transfer(
                !self.bg_pipeline_state.fifo.is_empty(),
            ) && self.bg_fetcher_ready_for_fifo_backed_obj_start()
        });

        Mode3DotArbitration {
            bg_transfer_can_advance: !has_pending_obj_hit,
            obj_fetch_can_start_from_fifo_backed_transfer: obj_fetch_can_start
                && current_transfer_is_fifo_backed,
            obj_fetch_can_start_from_queued_bg_fill: obj_fetch_can_start,
        }
    }

    fn current_transfer_context(&self) -> Option<Mode3TransferContext> {
        let mode3_dot = self.line_dot.saturating_sub(MODE2_DOTS);
        if !self
            .bg_pipeline_state
            .startup_transfer_window_open(mode3_dot)
        {
            return None;
        }
        if self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH {
            return None;
        }

        let lane = if self.bg_pipeline_state.scx_discard_remaining > 0
            || self.bg_pipeline_state.current_transfer_x < 8
        {
            self.bg_pipeline_state.current_startup_transfer_lane()
        } else {
            Mode3TransferLane::Visible
        };

        let source_window = self
            .bg_pipeline_state
            .current_startup_source_window(mode3_dot);

        Some(Mode3TransferContext {
            lane,
            source_window,
        })
    }

    fn transfer_service_plan_from_context(
        &self,
        context: Mode3TransferContext,
    ) -> Option<Mode3TransferServicePlan> {
        let execution = if self.bg_pipeline_state.scx_discard_remaining > 0 {
            Mode3TransferServiceExecution::ConsumeScxDiscard
        } else if self.bg_pipeline_state.current_transfer_x < 8 {
            match context.lane {
                Mode3TransferLane::PreVisible => {
                    Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop
                }
                Mode3TransferLane::Hidden => {
                    Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop
                }
                Mode3TransferLane::Visible => unreachable!("x < 8 cannot be a visible transfer"),
            }
        } else if context.lane == Mode3TransferLane::Visible {
            Mode3TransferServiceExecution::EmitVisiblePixel
        } else {
            return None;
        };

        let result_kind = if matches!(execution, Mode3TransferServiceExecution::EmitVisiblePixel) {
            Mode3TransferDotKind::ServedVisiblePixel
        } else {
            context.lane.dot_kind()
        };

        let backing = match context.source_window {
            Mode3TransferSourceWindow::AbstractStartup => Mode3TransferBacking::Abstract,
            Mode3TransferSourceWindow::FifoBacked => Mode3TransferBacking::FifoBacked,
        };

        Some(Mode3TransferServicePlan {
            result_kind,
            execution,
            backing,
        })
    }

    #[cfg(test)]
    fn current_transfer_service_plan(&self) -> Option<Mode3TransferServicePlan> {
        self.current_transfer()
            .map(|transfer| transfer.service_plan())
    }

    fn current_transfer(&self) -> Option<Mode3CurrentTransfer> {
        let context = self.current_transfer_context()?;
        let plan = self.transfer_service_plan_from_context(context)?;
        let readiness = if plan.requires_real_bg_fifo_pixel() {
            if self.bg_pipeline_state.fifo.is_empty() {
                Mode3TransferReadiness::WaitingForFifo(plan)
            } else {
                Mode3TransferReadiness::Ready(plan)
            }
        } else if plan.requires_effective_bg_fifo_pixel()
            && self.bg_pipeline_state.effective_fifo_is_empty()
        {
            Mode3TransferReadiness::WaitingForFifo(plan)
        } else {
            Mode3TransferReadiness::Ready(plan)
        };

        Some(Mode3CurrentTransfer { context, readiness })
    }

    fn advance_bg_fetcher(&mut self, vram: &VramBusView<'_>) -> bool {
        self.maybe_abort_window_fetcher_to_background();
        self.maybe_recompute_pending_background_push(vram);

        match (
            self.bg_pipeline_state.fetcher.stage,
            self.bg_pipeline_state.fetcher.stage_dot,
        ) {
            (PpuBgFetcherStage::Idle, _) => {
                self.bg_pipeline_state.fetcher.start_background();
                return false;
            }
            (PpuBgFetcherStage::WindowActivating, _) => {
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
                return false;
            }
            (PpuBgFetcherStage::Push, _) => {
                return matches!(
                    self.advance_bg_push_stage(),
                    BgPushDotResult::HandedOffToObjectFetch
                        | BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
                );
            }
            _ => {}
        }

        if self
            .bg_pipeline_state
            .fetcher
            .post_alignment_fetch_restart_delay_dots
            > 0
        {
            self.bg_pipeline_state
                .fetcher
                .post_alignment_fetch_restart_delay_dots -= 1;
            return false;
        }

        let fetcher = self.bg_pipeline_state.fetcher;
        match (fetcher.stage, fetcher.stage_dot) {
            (PpuBgFetcherStage::TileIndex, 0) => {
                if fetcher.source == PpuBgFetcherSource::Background {
                    self.bg_pipeline_state.fetcher.cached_origin = self
                        .bg_pipeline_state
                        .peek_startup_background_fetch_origin();
                }
                let tile_map_address =
                    self.compute_fetch_tile_index_address(fetcher.source, fetcher.fetch_x);
                self.bg_pipeline_state.fetcher.tile_map_address = tile_map_address;
                let delay_tileindex_read = fetcher.source == PpuBgFetcherSource::Background
                    && self
                        .bg_pipeline_state
                        .startup_background_tileindex_reads_on_stage_one();
                if !delay_tileindex_read {
                    self.bg_pipeline_state.fetcher.tile_index =
                        vram.read(tile_map_address as usize).unwrap_or(0);
                }
                if fetcher.source == PpuBgFetcherSource::Window {
                    self.bg_pipeline_state.fetcher.window_tilemap_x = self
                        .bg_pipeline_state
                        .fetcher
                        .window_tilemap_x
                        .wrapping_add(1);
                }
                if self
                    .bg_pipeline_state
                    .fetcher
                    .rewind_bg_resume_after_first_tile_index_dot
                {
                    self.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = self
                        .bg_pipeline_state
                        .fetcher
                        .bg_resume_fetch_pixel
                        .wrapping_sub(BG_TILE_WIDTH as u16);
                    self.bg_pipeline_state
                        .fetcher
                        .rewind_bg_resume_after_first_tile_index_dot = false;
                }
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileIndex, 1) => {
                if fetcher.source == PpuBgFetcherSource::Background
                    && self
                        .bg_pipeline_state
                        .startup_background_tileindex_reads_on_stage_one()
                {
                    self.bg_pipeline_state.fetcher.tile_index = vram
                        .read(self.bg_pipeline_state.fetcher.tile_map_address as usize)
                        .unwrap_or(0);
                }
                self.maybe_apply_bgwin_tilemap_selector_glitch(vram, fetcher.source);
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
            }
            (PpuBgFetcherStage::TileDataLow, 0) => {
                let tile_data_address = self.compute_fetch_tile_data_address(
                    fetcher.source,
                    fetcher.fetch_x,
                    fetcher.tile_index,
                    0,
                );
                self.bg_pipeline_state.fetcher.tile_data_address = tile_data_address;
                let tile_data = vram.read(tile_data_address as usize).unwrap_or(0);
                self.bg_pipeline_state.fetcher.tile_low = tile_data;
                self.maybe_cache_unsigned_bgwin_tile_data_fetch(
                    fetcher.source,
                    fetcher.fetch_x,
                    0,
                    tile_data,
                );
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileDataLow, 1) => {
                self.maybe_apply_bgwin_tile_data_selector_glitch(vram, fetcher.source, 0);
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
            }
            (PpuBgFetcherStage::TileDataHigh, 0) => {
                let tile_data_address = self.compute_fetch_tile_data_address(
                    fetcher.source,
                    fetcher.fetch_x,
                    fetcher.tile_index,
                    1,
                );
                self.bg_pipeline_state.fetcher.tile_data_address = tile_data_address;
                let tile_data = vram.read(tile_data_address as usize).unwrap_or(0);
                self.bg_pipeline_state.fetcher.tile_high = tile_data;
                self.maybe_cache_unsigned_bgwin_tile_data_fetch(
                    fetcher.source,
                    fetcher.fetch_x,
                    1,
                    tile_data,
                );
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileDataHigh, 1) => {
                self.maybe_apply_bgwin_tile_data_selector_glitch(vram, fetcher.source, 1);
                if self.bg_pipeline_state.startup_alignment_seed_pending() {
                    self.bg_pipeline_state
                        .push
                        .queue_startup_alignment_seed_from_fetcher(self.bg_pipeline_state.fetcher);
                    self.bg_pipeline_state
                        .fetcher
                        .first_window_tile_after_activation = false;
                    self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
                    self.bg_pipeline_state.fetcher.stage_dot = 0;
                    let push_result = self.advance_bg_push_stage();
                    if matches!(
                        push_result,
                        BgPushDotResult::HandedOffToObjectFetch
                            | BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
                    ) {
                        return true;
                    }
                    return false;
                }
                self.bg_pipeline_state
                    .push
                    .queue_from_fetcher(self.bg_pipeline_state.fetcher);
                if fetcher.source == PpuBgFetcherSource::Background {
                    self.bg_pipeline_state
                        .advance_startup_background_fetch_tile();
                }
                let mut advance_push_immediately = false;
                if self
                    .bg_pipeline_state
                    .take_startup_first_real_push_skip_entry_delay()
                {
                    self.bg_pipeline_state.push.entry_delay_remaining = 0;
                    advance_push_immediately = true;
                }
                self.bg_pipeline_state
                    .fetcher
                    .first_window_tile_after_activation = false;
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
                if advance_push_immediately {
                    return matches!(
                        self.advance_bg_push_stage(),
                        BgPushDotResult::HandedOffToObjectFetch
                            | BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
                    );
                }
            }
            (PpuBgFetcherStage::Idle, _)
            | (PpuBgFetcherStage::WindowActivating, _)
            | (PpuBgFetcherStage::Push, _) => unreachable!(
                "special BG fetcher stages are handled before the explicit dot-stage automaton"
            ),
            (_, other_dot) => unreachable!(
                "invalid BG fetcher stage_dot {other_dot} for non-push stage {:?}",
                fetcher.stage
            ),
        }

        false
    }

    fn maybe_abort_window_fetcher_to_background(&mut self) {
        if self.bg_pipeline_state.fetcher.source != PpuBgFetcherSource::Window {
            return;
        }

        if self.visible_registers.window_enabled() {
            return;
        }

        self.bg_pipeline_state.fetcher.abort_window_to_background();
    }

    fn advance_bg_push_stage(&mut self) -> BgPushDotResult {
        let ownership = self.current_bg_push_dot_ownership();
        self.execute_bg_push_dot_ownership(ownership)
    }

    fn current_step_region_after_line_advance(&self) -> PpuStepRegion {
        let next_line_dot = self.line_dot + 1;
        let next_lcd_restart_phase = self.lcd_restart_phase.advance(self.ly, next_line_dot);
        if next_lcd_restart_phase.is_startup_mode0_window_active(self.ly, next_line_dot)
            || self.ly >= VISIBLE_SCANLINES
            || next_line_dot >= self.current_mode0_start_dot()
        {
            return PpuStepRegion::Mode0Or1;
        }

        if next_line_dot < MODE2_DOTS {
            return PpuStepRegion::Mode2Scan;
        }

        if !self.bg_pipeline_state.mode3_started {
            return PpuStepRegion::Mode3Startup;
        }

        PpuStepRegion::Other
    }

    fn current_mode3_bg_pipeline_region(&self) -> PpuStepRegion {
        if self.bg_pipeline_state.fill.pending
            || self.bg_pipeline_state.push.pending
            || matches!(
                self.bg_pipeline_state.fetcher.stage,
                PpuBgFetcherStage::Push
            )
        {
            return PpuStepRegion::Mode3Push;
        }

        if matches!(
            self.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::WindowActivating
        ) || self.bg_pipeline_state.fetcher.source == PpuBgFetcherSource::Window
        {
            PpuStepRegion::Mode3WindowFetch
        } else {
            PpuStepRegion::Mode3BgFetch
        }
    }

    #[cfg(test)]
    fn advance_bg_push(&mut self) -> BgPushDotResult {
        self.execute_bg_push_dot_ownership(self.current_bg_push_dot_ownership())
    }

    fn current_bg_push_dot_ownership(&self) -> BgPushDotOwnership {
        let push = self.bg_pipeline_state.push;
        if !push.pending || push.disposition != BgPushDisposition::Ready {
            return BgPushDotOwnership::NotReady;
        }

        if push.entry_delay_remaining > 0 {
            return BgPushDotOwnership::EntryDelay;
        }

        let push_can_start_object_fetch = self.obj_pipeline_state.fetch.stage
            == PpuObjFetcherStage::Idle
            && !push.just_activated_window_tile
            && self.obj_enabled()
            && self.current_dot_has_pending_obj_hit()
            && (!push.cached.is_startup_alignment_seed()
                || self.bg_pipeline_state.current_transfer_x < 8);
        if self.bg_pipeline_state.fifo_contains_real_pixels() {
            if push_can_start_object_fetch {
                BgPushDotOwnership::FifoBackedTransferObjectFetch
            } else {
                BgPushDotOwnership::WaitingForEmptyFifo
            }
        } else if push_can_start_object_fetch {
            BgPushDotOwnership::QueueFillThenObjectFetch
        } else {
            BgPushDotOwnership::QueueFill
        }
    }

    fn execute_bg_push_dot_ownership(&mut self, ownership: BgPushDotOwnership) -> BgPushDotResult {
        match ownership {
            BgPushDotOwnership::NotReady => BgPushDotResult::NotReady,
            BgPushDotOwnership::EntryDelay => {
                debug_assert!(self.bg_pipeline_state.push.entry_delay_remaining > 0);
                self.bg_pipeline_state.push.entry_delay_remaining -= 1;
                self.bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open = true;
                BgPushDotResult::EntryDelay
            }
            BgPushDotOwnership::WaitingForEmptyFifo => {
                self.bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open =
                    self.bg_pipeline_state.push.cached.source == PpuBgFetcherSource::Background
                        && self.bg_pipeline_state.push.cached.fetch_x == BG_TILE_WIDTH as u16
                        && self.bg_pipeline_state.fifo.len()
                            == self.bg_pipeline_state.startup_fifo_placeholders as usize + 2;
                BgPushDotResult::WaitingForEmptyFifo
            }
            BgPushDotOwnership::FifoBackedTransferObjectFetch => {
                self.bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open = false;
                let started = self.try_start_object_fetch_from_current_dot(
                    ObjFetchStartSource::PushCachedBgFetch,
                    true,
                );
                debug_assert!(
                    started,
                    "fifo-backed push ownership must only be selected when OBJ fetch can start"
                );
                BgPushDotResult::HandedOffToObjectFetch
            }
            BgPushDotOwnership::QueueFill | BgPushDotOwnership::QueueFillThenObjectFetch => {
                self.bg_pipeline_state
                    .push
                    .cached
                    .same_cycle_live_tilemap_refetch_window_open = false;
                self.queue_bg_fill_from_push();
                if matches!(ownership, BgPushDotOwnership::QueueFillThenObjectFetch) {
                    let started = self.try_start_object_fetch_from_current_dot(
                        ObjFetchStartSource::QueuedBgFill,
                        true,
                    );
                    debug_assert!(
                        started,
                        "queued-fill push ownership must only be selected when OBJ fetch can start"
                    );
                    BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
                } else {
                    BgPushDotResult::QueuedFill
                }
            }
        }
    }

    fn queue_bg_fill_from_push(&mut self) {
        let push = self.bg_pipeline_state.push;
        if push.cached.is_startup_alignment_seed() {
            self.bg_pipeline_state.begin_post_alignment_followup();
            self.bg_pipeline_state
                .fill
                .queue_startup_alignment_from_push(
                    push,
                    self.bg_pipeline_state.startup_fifo_placeholders,
                );
        } else {
            self.bg_pipeline_state.fill.queue_from_push(push);
        }
        self.bg_pipeline_state.fetcher.fetch_x = push.next_fetch_pixel;
        self.bg_pipeline_state.fetcher.next_fetch_pixel = push.next_fetch_pixel;
        self.bg_pipeline_state
            .fetcher
            .post_alignment_fetch_restart_delay_dots = if push.cached.is_startup_alignment_seed() {
            1
        } else {
            0
        };
        self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
        self.bg_pipeline_state.push.reset();
    }

    fn flush_pending_bg_fifo_fill(&mut self) {
        if !self.bg_pipeline_state.fill.pending {
            return;
        }

        let fill = self.bg_pipeline_state.fill;
        if fill.startup_dummy_pixels > 0 {
            self.bg_pipeline_state
                .fifo
                .extend(std::iter::repeat_n(0, fill.startup_dummy_pixels as usize));
        }
        if fill.includes_real_tile_pixels {
            push_bg_tile_pixels(
                &mut self.bg_pipeline_state.fifo,
                fill.cached.tile_low,
                fill.cached.tile_high,
            );
        }
        self.bg_pipeline_state.fill.reset();
    }

    fn maybe_recompute_pending_background_fill(&mut self, vram: &VramBusView<'_>) {
        if !self.bg_pipeline_state.fill.pending
            || self.bg_pipeline_state.fill.cached.source != PpuBgFetcherSource::Background
            || !self.bg_pipeline_state.fill.includes_real_tile_pixels
        {
            return;
        }

        let Some(recomputed) = recompute_live_background_cached_slice(
            self.bg_pipeline_state.fill.cached,
            vram,
            self.lcdc,
            self.scy,
            self.ly,
            self.last_unsigned_tile_data_low_fetch,
            self.last_unsigned_tile_data_high_fetch,
        ) else {
            return;
        };

        self.bg_pipeline_state.fill.cached = recomputed;
    }

    fn maybe_recompute_pending_background_push(&mut self, vram: &VramBusView<'_>) {
        if !self.bg_pipeline_state.push.pending
            || self.bg_pipeline_state.push.cached.source != PpuBgFetcherSource::Background
        {
            return;
        }

        let Some(recomputed) = recompute_live_background_cached_slice(
            self.bg_pipeline_state.push.cached,
            vram,
            self.lcdc,
            self.scy,
            self.ly,
            self.last_unsigned_tile_data_low_fetch,
            self.last_unsigned_tile_data_high_fetch,
        ) else {
            return;
        };

        self.bg_pipeline_state.push.cached = recomputed;
        self.bg_pipeline_state.fetcher.tile_map_address = recomputed.tile_map_address;
        self.bg_pipeline_state.fetcher.tile_index = recomputed.tile_index;
        self.bg_pipeline_state.fetcher.tile_data_address = recomputed.tile_data_address;
        self.bg_pipeline_state.fetcher.tile_low = recomputed.tile_low;
        self.bg_pipeline_state.fetcher.tile_high = recomputed.tile_high;
    }

    fn execute_transfer_service_plan(
        &mut self,
        plan: Mode3TransferServicePlan,
    ) -> Mode3TransferDot {
        let pixel = if plan.requires_real_bg_fifo_pixel() {
            self.bg_pipeline_state.fifo.pop_front()
        } else if plan.requires_effective_bg_fifo_pixel() {
            self.bg_pipeline_state.consume_effective_fifo_pixel()
        } else {
            None
        };

        self.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        if !matches!(
            plan.execution,
            Mode3TransferServiceExecution::ConsumeScxDiscard
                | Mode3TransferServiceExecution::EmitVisiblePixel
        ) {
            self.bg_pipeline_state
                .consume_startup_pre_visible_transfer_dot();
        }

        match plan.execution {
            Mode3TransferServiceExecution::ConsumeScxDiscard => {
                let _ = pixel.expect(
                    "startup scx discard must consume one effective BG FIFO slot before output",
                );
                self.bg_pipeline_state.scx_discard_remaining -= 1;
                Mode3TransferDot::served(plan.result_kind, true)
            }
            Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop => {
                let _ = pixel
                    .expect("pre-visible startup transfer must consume one effective BG FIFO slot");
                self.bg_pipeline_state.current_transfer_x += 1;
                Mode3TransferDot::served(plan.result_kind, false)
            }
            Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop => {
                let _ = pixel.expect("hidden transfer must consume one effective BG FIFO slot");
                self.bg_pipeline_state.current_transfer_x += 1;
                let _ = self.pop_obj_fifo_pixel();
                Mode3TransferDot::served(plan.result_kind, false)
            }
            Mode3TransferServiceExecution::EmitVisiblePixel => {
                let bg_pixel = if self.pixel_transfer_bg_enabled() {
                    pixel.expect("visible transfer plans must carry a BG pixel")
                } else {
                    0
                };
                let obj_pixel = self.pop_obj_fifo_pixel();
                let output_pixel = self.mix_bg_and_obj(bg_pixel, obj_pixel);
                let panel_pixel = if self.visible_output == PpuVisibleOutputState::Driving {
                    self.map_mixed_pixel_to_panel_shade(output_pixel)
                } else {
                    0
                };
                let scanline_pixel = if self.visible_output == PpuVisibleOutputState::Driving {
                    output_pixel.color
                } else {
                    0
                };
                let visible_x = self.bg_pipeline_state.visible_pixels_output as usize;
                self.current_scanline_mixed_pixels[visible_x] = output_pixel;
                self.current_scanline_pixels[visible_x] = scanline_pixel;
                self.framebuffer[self.ly as usize * SCREEN_WIDTH + visible_x] = panel_pixel;
                self.bg_pipeline_state.current_transfer_x =
                    self.bg_pipeline_state.current_transfer_x.saturating_add(1);
                self.bg_pipeline_state.visible_pixels_output += 1;
                Mode3TransferDot::served(plan.result_kind, false)
            }
        }
    }

    fn obj_enabled(&self) -> bool {
        self.visible_registers.obj_enabled()
    }

    fn maybe_apply_wx0_shortening_after_transfer_dot(&mut self, transfer_dot: Mode3TransferDot) {
        if !transfer_dot.consumed_scx_discard
            || self.bg_pipeline_state.window_started_this_line
            || !self.bg_pipeline_state.window_wy_latch
            || !self.window_runtime_enabled()
            || self.window_activation_registers().wx != 0
            || self.bg_pipeline_state.window_force_x0_this_line
            || self.bg_pipeline_state.visible_pixels_output != 0
            || self.bg_pipeline_state.current_transfer_x >= 8
            || self.bg_pipeline_state.initial_scx_discard == 0
            || self.bg_pipeline_state.scx_discard_remaining != 0
        {
            return;
        }

        self.bg_pipeline_state.apply_wx0_scx_shortening();
    }

    fn maybe_start_window_after_transfer_dot(&mut self, transfer_dot: Mode3TransferDot) -> bool {
        if !transfer_dot.is_served()
            || self.bg_pipeline_state.window_started_this_line
            || !self.bg_pipeline_state.window_wy_latch
            || !self.window_runtime_enabled()
        {
            return false;
        }

        if self.window_activation_registers().wx == 166
            && !self.bg_pipeline_state.window_force_x0_this_line
        {
            if self.bg_pipeline_state.visible_pixels_output as usize == SCREEN_WIDTH
                && self.bg_pipeline_state.scx_discard_remaining == 0
                && !self.bg_pipeline_state.wx166_armed_this_line
            {
                self.window_state.pending_wx166_next_line = true;
                self.bg_pipeline_state.wx166_armed_this_line = true;
            }
            return false;
        }

        let Some(trigger_x) = self.window_trigger_x_for_current_line() else {
            return false;
        };

        if !self.should_start_window_after_transfer_dot_now(trigger_x, transfer_dot) {
            return false;
        }

        self.start_window_fetcher_restart();
        true
    }

    fn window_runtime_enabled(&self) -> bool {
        let registers = self.window_activation_registers();
        registers.window_enabled() && registers.bg_enabled()
    }

    fn latch_object_fetch_hits(&mut self) {
        if !self.obj_enabled() {
            return;
        }

        let current_owner = self.current_obj_hit_ownership();
        for sprite_slot in 0..self.mode2_scan_state.selected_sprite_count() {
            if self.obj_pipeline_state.has_fetched(sprite_slot) {
                continue;
            }

            let Some(sprite) = self.mode2_scan_state.selected_sprite(sprite_slot) else {
                continue;
            };
            let Some(trigger_x) = sprite_trigger_x(sprite) else {
                continue;
            };

            if trigger_x == current_owner.match_x {
                self.obj_pipeline_state
                    .queue_fetch_hit(sprite_slot, current_owner);
            }
        }
    }

    fn sync_pending_obj_hit_ownership(&mut self) {
        if !self.obj_enabled() {
            self.obj_pipeline_state.clear_pending_fetch_hits();
            return;
        }

        let current_owner = self.current_obj_hit_ownership();
        self.obj_pipeline_state
            .clear_pending_fetch_hits_if_stale(current_owner);
    }

    fn try_start_object_fetch_from_current_dot(
        &mut self,
        start_source: ObjFetchStartSource,
        overlap_current_dot: bool,
    ) -> bool {
        if !self
            .current_dot_arbitration()
            .can_start_obj_fetch(start_source)
        {
            return false;
        }

        let Some(sprite_slot) = self.obj_pipeline_state.pop_pending_fetch_hit() else {
            return false;
        };
        let Some(sprite) = self.mode2_scan_state.selected_sprite(sprite_slot) else {
            return false;
        };

        self.obj_pipeline_state.start_fetch(sprite_slot, sprite);
        if overlap_current_dot {
            if matches!(
                start_source,
                ObjFetchStartSource::FifoBackedTransfer | ObjFetchStartSource::PushCachedBgFetch
            ) {
                self.bg_pipeline_state.push.interrupt_for_object_fetch();
            }
            self.bg_pipeline_state.extend_mode3_by_one_dot();
            self.obj_pipeline_state.fetch.stage_dot = 1;
        }
        true
    }

    fn current_obj_hit_ownership(&self) -> ObjHitOwnership {
        let phase = self
            .current_transfer()
            .map_or(ObjHitPhase::PreVisible, |transfer| {
                match transfer.context.lane {
                    Mode3TransferLane::PreVisible => ObjHitPhase::PreVisible,
                    Mode3TransferLane::Hidden => ObjHitPhase::Hidden,
                    Mode3TransferLane::Visible => ObjHitPhase::Visible,
                }
            });

        ObjHitOwnership {
            match_x: self.bg_pipeline_state.current_transfer_x,
            phase,
        }
    }

    fn bg_fetcher_ready_for_fifo_backed_obj_start(&self) -> bool {
        !matches!(
            self.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileIndex | PpuBgFetcherStage::TileDataLow
        )
    }

    fn advance_object_fetch(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        if self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle {
            return false;
        }

        if self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Startup
            && !self.obj_fetch_startup_ready()
        {
            return false;
        }

        self.bg_pipeline_state.extend_mode3_by_one_dot();
        if !self.obj_enabled() {
            self.obj_pipeline_state.fetch.cancelled = true;
        }

        let fetch = self.obj_pipeline_state.fetch;
        match (fetch.stage, fetch.stage_dot) {
            (PpuObjFetcherStage::Startup, 0) => {
                self.obj_pipeline_state.fetch.stage_dot = 1;
            }
            (PpuObjFetcherStage::Startup, 1) => {
                let resolved_sprite = fetch
                    .sprite
                    .map(|sprite| self.resolve_obj_fetch_sprite(oam, sprite, dma_oam_conflict));
                self.obj_pipeline_state.fetch.resolved_sprite = resolved_sprite;
                self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataLow;
                self.obj_pipeline_state.fetch.stage_dot = 0;
            }
            (PpuObjFetcherStage::TileDataLow, 0) => {
                self.obj_pipeline_state.fetch.stage_dot = 1;
            }
            (PpuObjFetcherStage::TileDataLow, 1) => {
                let resolved_sprite = fetch
                    .resolved_sprite
                    .expect("active OBJ fetch must resolve tile metadata before reading tile data");
                self.obj_pipeline_state.fetch.tile_low =
                    self.read_obj_tile_data_byte(vram, resolved_sprite, 0);
                self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
                self.obj_pipeline_state.fetch.stage_dot = 0;
            }
            (PpuObjFetcherStage::TileDataHigh, 0) => {
                self.obj_pipeline_state.fetch.stage_dot = 1;
            }
            (PpuObjFetcherStage::TileDataHigh, 1) => {
                let resolved_sprite = fetch
                    .resolved_sprite
                    .expect("active OBJ fetch must resolve tile metadata before reading tile data");
                self.obj_pipeline_state.fetch.tile_high =
                    self.read_obj_tile_data_byte(vram, resolved_sprite, 1);
                self.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
                self.obj_pipeline_state.fetch.stage_dot = 0;
            }
            (PpuObjFetcherStage::Push, 0) => {
                self.obj_pipeline_state.fetch.stage_dot = 1;
            }
            (PpuObjFetcherStage::Push, 1) => {
                let resolved_sprite = fetch
                    .resolved_sprite
                    .expect("active OBJ fetch must keep resolved metadata until FIFO push");
                if !fetch.cancelled && self.obj_enabled() {
                    self.push_obj_pixels(
                        resolved_sprite,
                        fetch.tile_low,
                        fetch.tile_high,
                        self.bg_pipeline_state.visible_pixels_output,
                    );
                }
                self.obj_pipeline_state.mark_fetched(fetch.sprite_slot);
                self.obj_pipeline_state.fetch = ObjFetchState::default();
                self.bg_pipeline_state.push.resume_after_object_fetch();
            }
            (PpuObjFetcherStage::Idle, _) => unreachable!(
                "idle OBJ fetch must have returned before entering the explicit dot automaton"
            ),
            (_, other_dot) => unreachable!(
                "invalid OBJ fetcher stage_dot {other_dot} for stage {:?}",
                fetch.stage
            ),
        }

        true
    }

    fn obj_fetch_startup_ready(&self) -> bool {
        let fifo_ready = !self.bg_pipeline_state.fifo.is_empty();
        let Some(sprite) = self.obj_pipeline_state.fetch.sprite else {
            return fifo_ready;
        };

        if sprite.x >= 8 {
            return fifo_ready;
        }

        fifo_ready
            && !matches!(
                self.bg_pipeline_state.fetcher.stage,
                PpuBgFetcherStage::TileIndex
            )
    }

    fn resolve_obj_fetch_sprite(
        &mut self,
        oam: &OamBusView<'_>,
        sprite: PpuSelectedSprite,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> PpuSelectedSprite {
        let (tile_index, attributes) =
            read_obj_fetch_sprite_metadata(oam, sprite, dma_oam_conflict);
        self.obj_pipeline_state.late_metadata_word = Some((tile_index, attributes));

        PpuSelectedSprite {
            tile_index,
            attributes,
            ..sprite
        }
    }

    fn window_trigger_x_for_current_line(&self) -> Option<u8> {
        if self.bg_pipeline_state.window_force_x0_this_line {
            return Some(0);
        }

        let registers = self.window_activation_registers();
        match registers.wx {
            0..=166 => Some(registers.wx.saturating_sub(7)),
            _ => None,
        }
    }

    fn should_start_window_after_transfer_dot_now(
        &self,
        trigger_x: u8,
        transfer_dot: Mode3TransferDot,
    ) -> bool {
        if self.bg_pipeline_state.visible_pixels_output != trigger_x {
            return false;
        }

        if trigger_x == 0 {
            return self.bg_pipeline_state.scx_discard_remaining == 0
                && self.bg_pipeline_state.current_transfer_x >= 8
                && transfer_dot.can_start_window_after_x0_service();
        }

        self.bg_pipeline_state.scx_discard_remaining == 0
            && transfer_dot.kind == Mode3TransferDotKind::ServedVisiblePixel
    }

    fn start_window_fetcher_restart(&mut self) {
        let bg_resume_fetch_pixel = self.bg_pipeline_state.fetcher.next_fetch_pixel;
        self.bg_pipeline_state.fifo.clear();
        self.bg_pipeline_state.startup_fifo_placeholders = 0;
        self.bg_pipeline_state.push.reset();
        self.bg_pipeline_state.fill.reset();
        self.bg_pipeline_state
            .fetcher
            .start_window(bg_resume_fetch_pixel);
        self.bg_pipeline_state.scx_discard_remaining = 0;
        self.bg_pipeline_state.window_started_this_line = true;
        self.bg_pipeline_state.window_force_x0_this_line = false;
    }

    fn compute_fetch_tile_index_address(
        &self,
        source: PpuBgFetcherSource,
        next_fetch_pixel: u16,
    ) -> u16 {
        let (tile_map_base, tile_x, tile_y) = match source {
            PpuBgFetcherSource::Background => {
                let bg_x = self
                    .visible_registers
                    .scx
                    .wrapping_add(next_fetch_pixel as u8);
                let bg_fetch_scy = self.bg_fetch_scy(next_fetch_pixel);
                let bg_fetch_lcdc = self.bg_fetch_tilemap_lcdc(next_fetch_pixel);
                let bg_y = bg_fetch_scy.wrapping_add(self.ly);
                let tile_map_base = if bg_fetch_lcdc & LCDC_BG_TILE_MAP_BIT != 0 {
                    0x1C00
                } else {
                    0x1800
                };
                (
                    tile_map_base,
                    (bg_x / BG_TILE_WIDTH) as usize,
                    (bg_y / BG_TILE_WIDTH) as usize,
                )
            }
            PpuBgFetcherSource::Window => {
                let tile_map_base = if self.window_fetch_lcdc() & LCDC_WINDOW_TILE_MAP_BIT != 0 {
                    0x1C00
                } else {
                    0x1800
                };
                (
                    tile_map_base,
                    self.bg_pipeline_state.fetcher.window_tilemap_x as usize,
                    (self.window_state.window_line_counter / BG_TILE_WIDTH) as usize,
                )
            }
        };
        (tile_map_base + tile_y * BG_TILE_MAP_WIDTH as usize + tile_x) as u16
    }

    fn compute_fetch_tile_data_address(
        &self,
        source: PpuBgFetcherSource,
        fetch_x: u16,
        tile_index: u8,
        plane: u16,
    ) -> u16 {
        let tile_row = match source {
            PpuBgFetcherSource::Background => {
                (self.bg_fetch_scy(fetch_x).wrapping_add(self.ly) % BG_TILE_WIDTH) as u16
            }
            PpuBgFetcherSource::Window => {
                (self.window_state.window_line_counter % BG_TILE_WIDTH) as u16
            }
        };
        let tile_data_base = bg_tile_data_base(
            match source {
                PpuBgFetcherSource::Background => self.bg_fetch_tiledata_lcdc(fetch_x),
                PpuBgFetcherSource::Window => self.window_fetch_lcdc(),
            },
            tile_index,
        );
        tile_data_base + tile_row * TILE_ROW_BYTES + plane
    }

    fn bg_fetch_tilemap_uses_pipeline_snapshot(&self, next_fetch_pixel: u16) -> bool {
        let _ = next_fetch_pixel;
        self.console_model.is_dmg_family()
            && self
                .bg_pipeline_state
                .startup_background_tilemap_uses_pipeline_snapshot()
    }

    fn bg_fetch_tiledata_uses_pipeline_snapshot(&self, next_fetch_pixel: u16) -> bool {
        let _ = next_fetch_pixel;
        self.console_model.is_dmg_family()
            && self
                .bg_pipeline_state
                .startup_background_tiledata_uses_pipeline_snapshot()
    }

    fn bg_fetch_tilemap_lcdc(&self, next_fetch_pixel: u16) -> u8 {
        if self.bg_fetch_tilemap_uses_pipeline_snapshot(next_fetch_pixel) {
            self.pipeline_registers.lcdc
        } else {
            self.visible_registers.lcdc
        }
    }

    fn bg_fetch_tiledata_lcdc(&self, next_fetch_pixel: u16) -> u8 {
        if self.bg_fetch_tiledata_uses_pipeline_snapshot(next_fetch_pixel) {
            self.pipeline_registers.lcdc
        } else {
            self.visible_registers.lcdc
        }
    }

    fn bg_fetch_scy(&self, next_fetch_pixel: u16) -> u8 {
        if self.bg_fetch_tiledata_uses_pipeline_snapshot(next_fetch_pixel) {
            self.pipeline_registers.scy
        } else {
            self.visible_registers.scy
        }
    }

    fn window_fetch_lcdc(&self) -> u8 {
        self.visible_registers.lcdc
    }

    fn maybe_cache_unsigned_bgwin_tile_data_fetch(
        &mut self,
        source: PpuBgFetcherSource,
        next_fetch_pixel: u16,
        plane: u16,
        tile_data: u8,
    ) {
        if self.bg_pipeline_state.startup_alignment_seed_pending() {
            return;
        }
        let lcdc = match source {
            PpuBgFetcherSource::Background => self.bg_fetch_tiledata_lcdc(next_fetch_pixel),
            PpuBgFetcherSource::Window => self.window_fetch_lcdc(),
        };
        if lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0 {
            self.last_unsigned_tile_data_fetch = tile_data;
            if plane == 0 {
                self.last_unsigned_tile_data_low_fetch = tile_data;
            } else {
                self.last_unsigned_tile_data_high_fetch = tile_data;
            }
        }
    }

    fn maybe_apply_bgwin_tile_data_selector_glitch(
        &mut self,
        vram: &VramBusView<'_>,
        source: PpuBgFetcherSource,
        plane: u16,
    ) {
        if !self.console_model.is_dmg_family() {
            return;
        }

        let previous_uses_unsigned =
            self.pipeline_registers.lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;
        let current_uses_unsigned = self.visible_registers.lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;
        if previous_uses_unsigned == current_uses_unsigned {
            return;
        }

        let tile_index = self.bg_pipeline_state.fetcher.tile_index;
        let reread_address = if previous_uses_unsigned && !current_uses_unsigned {
            Some(self.compute_fetch_tile_data_address(
                source,
                self.bg_pipeline_state.fetcher.fetch_x,
                tile_index,
                plane,
            ))
        } else {
            None
        };
        let tile_byte = if let Some(tile_data_address) = reread_address {
            vram.read(tile_data_address as usize).unwrap_or(0)
        } else {
            match plane {
                0 => self.last_unsigned_tile_data_low_fetch,
                _ => self.last_unsigned_tile_data_high_fetch,
            }
        };

        let fetcher = &mut self.bg_pipeline_state.fetcher;
        if let Some(tile_data_address) = reread_address {
            fetcher.tile_data_address = tile_data_address;
        }

        if plane == 0 {
            fetcher.tile_low = tile_byte;
        } else {
            fetcher.tile_high = tile_byte;
        }
    }

    fn maybe_apply_bgwin_tilemap_selector_glitch(
        &mut self,
        vram: &VramBusView<'_>,
        source: PpuBgFetcherSource,
    ) {
        if !self.console_model.is_dmg_family() {
            return;
        }

        let map_bit = match source {
            PpuBgFetcherSource::Background => LCDC_BG_TILE_MAP_BIT,
            PpuBgFetcherSource::Window => LCDC_WINDOW_TILE_MAP_BIT,
        };
        let previous_selects_high = self.pipeline_registers.lcdc & map_bit != 0;
        let current_selects_high = self.visible_registers.lcdc & map_bit != 0;
        if previous_selects_high == current_selects_high {
            return;
        }

        let tile_map_address =
            self.compute_fetch_tile_index_address(source, self.bg_pipeline_state.fetcher.fetch_x);
        let tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
        let fetcher = &mut self.bg_pipeline_state.fetcher;
        fetcher.tile_map_address = tile_map_address;
        fetcher.tile_index = tile_index;
    }

    fn read_obj_tile_data_byte(
        &mut self,
        vram: &VramBusView<'_>,
        sprite: PpuSelectedSprite,
        plane: u16,
    ) -> u8 {
        let Some((tile_index, tile_row)) = self.obj_tile_index_and_row(sprite) else {
            return 0;
        };
        let byte_address =
            tile_index as u16 * TILE_BYTES + tile_row as u16 * TILE_ROW_BYTES + plane;
        let tile_data = vram.read(byte_address as usize).unwrap_or(0);
        self.last_unsigned_tile_data_fetch = tile_data;
        tile_data
    }

    fn obj_tile_index_and_row(&self, sprite: PpuSelectedSprite) -> Option<(u8, u8)> {
        let sprite_top = sprite.y.wrapping_sub(16);
        let height = self.current_obj_height();
        let mut row = self.ly.wrapping_sub(sprite_top);
        if row >= height {
            return None;
        }
        if sprite.attributes & 0x40 != 0 {
            row = height - 1 - row;
        }

        if height == 16 {
            let base_tile = sprite.tile_index & !0x01;
            if row < 8 {
                Some((base_tile, row))
            } else {
                Some((base_tile + 1, row - 8))
            }
        } else {
            Some((sprite.tile_index, row))
        }
    }

    fn push_obj_pixels(
        &mut self,
        sprite: PpuSelectedSprite,
        tile_low: u8,
        tile_high: u8,
        current_visible_x: u8,
    ) {
        let sprite_screen_x = sprite_screen_x(sprite);
        for tile_pixel in 0..BG_TILE_WIDTH {
            let bit = if sprite.attributes & 0x20 != 0 {
                tile_pixel
            } else {
                7 - tile_pixel
            };
            let low_bit = (tile_low >> bit) & 0x01;
            let high_bit = (tile_high >> bit) & 0x01;
            let color = (high_bit << 1) | low_bit;
            let screen_x = sprite_screen_x + tile_pixel as i16;
            if !(0..SCREEN_WIDTH as i16).contains(&screen_x) {
                continue;
            }
            if screen_x < current_visible_x as i16 {
                continue;
            }

            let offset = (screen_x as usize).saturating_sub(current_visible_x as usize);
            while self.obj_pipeline_state.fifo.len() <= offset {
                self.obj_pipeline_state
                    .fifo
                    .push_back(ObjPixel::transparent());
            }

            let candidate = ObjPixel {
                color,
                palette_obp1: sprite.attributes & 0x10 != 0,
                bg_over_obj: sprite.attributes & 0x80 != 0,
                sprite_x: sprite.x,
                oam_index: sprite.oam_index,
            };

            let slot = self
                .obj_pipeline_state
                .fifo
                .get_mut(offset)
                .expect("OBJ FIFO was extended to cover the target offset");
            if obj_pixel_has_priority(candidate, *slot) {
                *slot = candidate;
            }
        }
    }

    fn pop_obj_fifo_pixel(&mut self) -> ObjPixel {
        self.obj_pipeline_state
            .fifo
            .pop_front()
            .unwrap_or_else(ObjPixel::transparent)
    }

    fn mix_bg_and_obj(&self, bg_pixel: u8, obj_pixel: ObjPixel) -> MixedPixel {
        if !self.pixel_transfer_obj_enabled() || obj_pixel.is_transparent() {
            return MixedPixel::background(bg_pixel);
        }

        if obj_pixel.bg_over_obj && bg_pixel != 0 {
            MixedPixel::background(bg_pixel)
        } else {
            MixedPixel::object(obj_pixel.color, obj_pixel.palette_obp1)
        }
    }

    fn map_mixed_pixel_to_panel_shade(&self, pixel: MixedPixel) -> u8 {
        match pixel.source {
            MixedPixelSource::Background => {
                self.apply_dmg_palette(self.pixel_pipeline_bgp(), pixel.color)
            }
            MixedPixelSource::Object { palette_obp1 } => {
                let palette = self
                    .visible_registers
                    .obj_palette(palette_obp1, self.obj_palette_read_policy);
                self.apply_dmg_palette(palette, pixel.color)
            }
        }
    }

    fn apply_dmg_palette(&self, palette: u8, color: u8) -> u8 {
        (palette >> (u32::from(color & 0x03) * 2)) & 0x03
    }

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

    fn live_lyc_coincidence(&self) -> bool {
        self.ly == self.lyc
    }

    fn effective_lyc_coincidence(&self) -> bool {
        if self.is_lcd_enabled() {
            self.live_lyc_coincidence()
        } else {
            self.stat_state.lcd_disabled_lyc_coincidence
        }
    }

    fn ordinary_stat_irq_line(&self) -> bool {
        let coincidence_source = self.stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT != 0
            && self.effective_lyc_coincidence();

        if !self.is_lcd_enabled() {
            return coincidence_source;
        }

        let mode0_start_dot = self.current_mode0_start_dot();
        let mode0_pretrigger_source = self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT
            != 0
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot < mode0_start_dot
            && self.line_dot + 4 >= mode0_start_dot;
        let mode2_pretrigger_source = self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT
            != 0
            && self.ly + 1 < VISIBLE_SCANLINES
            && self.line_dot + 4 >= DOTS_PER_SCANLINE;
        let dmg_mode2_vblank_entry_source = self.console_model.is_dmg_family()
            && self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.current_access_mode() == PpuAccessMode::VBlank
            && self.ly == VISIBLE_SCANLINES
            && self.line_dot == 0;
        let mode_source = match self.current_access_mode() {
            PpuAccessMode::HBlank => {
                self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0
            }
            PpuAccessMode::VBlank => {
                self.stat_interrupt_enable & STAT_MODE1_INTERRUPT_ENABLE_BIT != 0
            }
            PpuAccessMode::OamScan => {
                self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            }
            PpuAccessMode::Drawing => false,
        };

        coincidence_source
            || mode_source
            || mode0_pretrigger_source
            || mode2_pretrigger_source
            || dmg_mode2_vblank_entry_source
    }

    fn compute_stat_irq_line(&self, quirk_active: bool) -> bool {
        self.ordinary_stat_irq_line() || quirk_active
    }

    fn refresh_stat_irq_line(&mut self, quirk_active: bool) {
        let new_line = self.compute_stat_irq_line(quirk_active);
        if !self.stat_state.irq_line && new_line {
            self.queue_interrupt_request(InterruptSource::LcdStat);
        }
        self.stat_state.irq_line = new_line;
    }

    fn queue_interrupt_request(&mut self, source: InterruptSource) {
        let bit = match source {
            InterruptSource::VBlank => PPU_PENDING_VBLANK_INTERRUPT_BIT,
            InterruptSource::LcdStat => PPU_PENDING_LCD_STAT_INTERRUPT_BIT,
            InterruptSource::Timer | InterruptSource::Serial | InterruptSource::Joypad => {
                return;
            }
        };
        self.pending_interrupts |= bit;
    }

    fn stat_write_quirk_active(&self) -> bool {
        self.console_model.is_dmg_family()
            && self.is_lcd_enabled()
            && (self.current_access_mode() != PpuAccessMode::Drawing || self.live_lyc_coincidence())
    }

    fn refresh_visible_output(&mut self) {
        self.visible_output =
            if self.is_lcd_enabled() && !self.blank_frame_active && !self.system_stop_active {
                PpuVisibleOutputState::Driving
            } else {
                PpuVisibleOutputState::ForcedBlank
            };
    }

    fn advance_lcd_restart_phase(&mut self) {
        self.lcd_restart_phase = self.lcd_restart_phase.advance(self.ly, self.line_dot);
    }

    fn reset_runtime_pipeline_state(&mut self) {
        self.startup_mode_latch = None;
        self.mode2_scan_state.reset();
        self.window_state.reset();
        self.bg_pipeline_state.reset();
        self.obj_pipeline_state.reset();
        self.current_scanline_pixels.fill(0);
        self.current_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
    }

    fn clear_visible_buffers(&mut self) {
        self.current_scanline_pixels.fill(0);
        self.framebuffer.fill(0);
    }

    fn enter_lcd_disabled_state(&mut self) {
        self.lcd_state = PpuLcdState::Disabled;
        self.blank_frame_active = false;
        self.stat_state.lcd_disabled_lyc_coincidence = self.live_lyc_coincidence();
        self.ly = 0;
        self.line_dot = 0;
        self.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        self.reset_runtime_pipeline_state();
        self.sync_visible_registers();
        self.sync_pipeline_registers();
        self.clear_visible_buffers();
        self.refresh_visible_output();
    }

    fn enter_lcd_enabled_restart_state(&mut self) {
        self.lcd_state = PpuLcdState::Enabled;
        self.blank_frame_active = true;
        self.ly = 0;
        self.line_dot = LCD_REENABLE_INITIAL_LINE_DOT;
        self.lcd_restart_phase = PpuLcdRestartPhase::startup_mode0_window();
        self.stat_state.lcd_disabled_lyc_coincidence = false;
        self.reset_runtime_pipeline_state();
        self.sync_visible_registers();
        self.sync_pipeline_registers();
        self.clear_visible_buffers();
        self.refresh_visible_output();
    }

    fn write_dmg_palette_register(&mut self, register: PpuPaletteRegister, value: u8) {
        let previous_visible = match register {
            PpuPaletteRegister::Bgp => self.visible_registers.bgp,
            PpuPaletteRegister::Obp0 => self
                .visible_registers
                .obj_palette(false, self.obj_palette_read_policy),
            PpuPaletteRegister::Obp1 => self
                .visible_registers
                .obj_palette(true, self.obj_palette_read_policy),
        };

        match register {
            PpuPaletteRegister::Bgp => self.bgp = value,
            PpuPaletteRegister::Obp0 => self.obp0 = Some(value),
            PpuPaletteRegister::Obp1 => self.obp1 = Some(value),
        }

        if let Some(retroactive_pixels) = self.dmg_palette_conflict_retroactive_pixels(register) {
            self.retroactively_recolor_recent_pixels(
                register,
                previous_visible | value,
                value,
                retroactive_pixels,
            );
        }
    }

    fn retroactively_recolor_recent_pixels(
        &mut self,
        register: PpuPaletteRegister,
        transient_palette: u8,
        final_palette: u8,
        retroactive_pixels: usize,
    ) {
        if self.visible_output != PpuVisibleOutputState::Driving {
            return;
        }

        let visible_x = self.bg_pipeline_state.visible_pixels_output as usize;
        let start = visible_x.saturating_sub(retroactive_pixels);
        for x in start..visible_x {
            let mixed_pixel = self.current_scanline_mixed_pixels[x];
            if !register_affects_pixel(register, mixed_pixel) {
                continue;
            }

            let use_transient_palette = match register {
                PpuPaletteRegister::Bgp => x == start,
                PpuPaletteRegister::Obp0 | PpuPaletteRegister::Obp1 => x == start,
            };
            let palette = if use_transient_palette {
                transient_palette
            } else {
                final_palette
            };
            let panel_pixel = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                mixed_pixel,
                register,
                palette,
            );
            self.framebuffer[self.ly as usize * SCREEN_WIDTH + x] = panel_pixel;
        }
    }

    fn map_mixed_pixel_to_panel_shade_with_palette_override(
        &self,
        pixel: MixedPixel,
        register: PpuPaletteRegister,
        palette_override: u8,
    ) -> u8 {
        match pixel.source {
            MixedPixelSource::Background => {
                let palette = if register == PpuPaletteRegister::Bgp {
                    palette_override
                } else {
                    self.visible_registers.bgp
                };
                self.apply_dmg_palette(palette, pixel.color)
            }
            MixedPixelSource::Object { palette_obp1 } => {
                let palette = match (register, palette_obp1) {
                    (PpuPaletteRegister::Obp0, false) | (PpuPaletteRegister::Obp1, true) => {
                        palette_override
                    }
                    _ => self
                        .visible_registers
                        .obj_palette(palette_obp1, self.obj_palette_read_policy),
                };
                self.apply_dmg_palette(palette, pixel.color)
            }
        }
    }

    fn dmg_palette_conflict_retroactive_pixels(
        &self,
        register: PpuPaletteRegister,
    ) -> Option<usize> {
        if !self.console_model.is_dmg_family() || self.ly >= VISIBLE_SCANLINES {
            return None;
        }

        let retroactive_pixels = match register {
            PpuPaletteRegister::Bgp => DMG_PALETTE_RETROACTIVE_PIXELS,
            PpuPaletteRegister::Obp0 | PpuPaletteRegister::Obp1 => {
                DMG_PALETTE_RETROACTIVE_PIXELS + 1
            }
        };

        match self.current_raster_state() {
            PpuRasterState::Active {
                mode: PpuAccessMode::Drawing,
                ..
            } => Some(retroactive_pixels),
            PpuRasterState::Active {
                mode: PpuAccessMode::HBlank,
                mode_dot,
                ..
            } if mode_dot < 4 => Some(retroactive_pixels.saturating_sub(1)),
            PpuRasterState::Disabled
            | PpuRasterState::LcdRestartStartupMode0 { .. }
            | PpuRasterState::Active { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PpuPaletteRegister {
    Bgp,
    Obp0,
    Obp1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferDotKind {
    NotServed,
    ServedPreVisibleTransfer,
    ServedHiddenTransfer,
    ServedVisiblePixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3TransferDot {
    kind: Mode3TransferDotKind,
    consumed_scx_discard: bool,
}

impl Mode3TransferDot {
    const fn not_served() -> Self {
        Self {
            kind: Mode3TransferDotKind::NotServed,
            consumed_scx_discard: false,
        }
    }

    const fn served(kind: Mode3TransferDotKind, consumed_scx_discard: bool) -> Self {
        Self {
            kind,
            consumed_scx_discard,
        }
    }

    fn is_served(self) -> bool {
        !matches!(self.kind, Mode3TransferDotKind::NotServed)
    }

    fn can_start_window_after_x0_service(self) -> bool {
        matches!(
            self.kind,
            Mode3TransferDotKind::ServedHiddenTransfer | Mode3TransferDotKind::ServedVisiblePixel
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode3TransferPhase {
    #[default]
    Priming,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferLane {
    PreVisible,
    Hidden,
    Visible,
}

impl Mode3TransferLane {
    const fn dot_kind(self) -> Mode3TransferDotKind {
        match self {
            Self::PreVisible => Mode3TransferDotKind::ServedPreVisibleTransfer,
            Self::Hidden => Mode3TransferDotKind::ServedHiddenTransfer,
            Self::Visible => Mode3TransferDotKind::ServedVisiblePixel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferSourceWindow {
    AbstractStartup,
    FifoBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3TransferContext {
    lane: Mode3TransferLane,
    source_window: Mode3TransferSourceWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3TransferServicePlan {
    result_kind: Mode3TransferDotKind,
    execution: Mode3TransferServiceExecution,
    backing: Mode3TransferBacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3CurrentTransfer {
    context: Mode3TransferContext,
    readiness: Mode3TransferReadiness,
}

impl Mode3CurrentTransfer {
    #[cfg(test)]
    const fn service_plan(self) -> Mode3TransferServicePlan {
        match self.readiness {
            Mode3TransferReadiness::WaitingForFifo(plan) | Mode3TransferReadiness::Ready(plan) => {
                plan
            }
        }
    }

    const fn can_start_obj_fetch_from_fifo_backed_transfer(
        self,
        real_bg_fifo_pixel_ready: bool,
    ) -> bool {
        self.readiness
            .can_start_obj_fetch_from_fifo_backed_transfer(real_bg_fifo_pixel_ready)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferReadiness {
    WaitingForFifo(Mode3TransferServicePlan),
    Ready(Mode3TransferServicePlan),
}

impl Mode3TransferReadiness {
    const fn can_start_obj_fetch_from_fifo_backed_transfer(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferServiceExecution {
    ConsumeScxDiscard,
    AdvancePreVisibleWithBgPop,
    AdvanceHiddenWithBgAndObjPop,
    EmitVisiblePixel,
}

impl Mode3TransferServiceExecution {
    const fn can_start_obj_fetch_from_fifo_backed_transfer(self) -> bool {
        matches!(
            self,
            Self::AdvanceHiddenWithBgAndObjPop | Self::EmitVisiblePixel
        )
    }

    const fn requires_effective_bg_fifo_pixel(self) -> bool {
        matches!(
            self,
            Self::ConsumeScxDiscard
                | Self::AdvancePreVisibleWithBgPop
                | Self::AdvanceHiddenWithBgAndObjPop
        )
    }

    const fn requires_real_bg_fifo_pixel(self) -> bool {
        matches!(self, Self::EmitVisiblePixel)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferBacking {
    Abstract,
    FifoBacked,
}

impl Mode3TransferServicePlan {
    const fn requires_effective_bg_fifo_pixel(self) -> bool {
        self.execution.requires_effective_bg_fifo_pixel() && !self.requires_real_bg_fifo_pixel()
    }

    const fn requires_real_bg_fifo_pixel(self) -> bool {
        self.execution.requires_real_bg_fifo_pixel()
            || (matches!(self.backing, Mode3TransferBacking::FifoBacked)
                && matches!(
                    self.execution,
                    Mode3TransferServiceExecution::ConsumeScxDiscard
                        | Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop
                ))
    }

    const fn can_start_obj_fetch_from_fifo_backed_transfer(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3StartupSourceState {
    EntryDelay { remaining: u8 },
    Abstract { remaining: u8 },
    FifoBacked,
}

const fn register_affects_pixel(register: PpuPaletteRegister, pixel: MixedPixel) -> bool {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OamCorruptionEventKind {
    Read,
    Write,
    ReadWithIncDec,
    WriteWithIncDec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct OamCorruptionController;

impl OamCorruptionController {
    fn apply(
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

    fn apply_write_corruption(self, current_row: u8, oam_bytes: &mut [u8]) {
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

    fn apply_read_corruption(self, current_row: u8, oam_bytes: &mut [u8]) {
        if current_row == 0 {
            return;
        }

        let current_first = read_oam_word(oam_bytes, current_row, 0);
        let previous_first = read_oam_word(oam_bytes, current_row - 1, 0);
        let previous_third = read_oam_word(oam_bytes, current_row - 1, 2);
        let corrupted_first = previous_first | (current_first & previous_third);
        write_oam_word(oam_bytes, current_row, 0, corrupted_first);
        copy_previous_row_tail(oam_bytes, current_row);
    }

    fn apply_read_with_incdec_corruption(self, current_row: u8, oam_bytes: &mut [u8]) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct BgPipelineState {
    fetcher: BgFetcherState,
    push: BgPushState,
    fill: BgFifoFillState,
    fifo: VecDeque<u8>,
    startup_fetch_seam: BgStartupFetchSeamState,
    startup_fifo_placeholders: u8,
    mode3_started: bool,
    mode0_start_dot: u16,
    initial_scx_discard: u8,
    scx_discard_remaining: u8,
    startup_source_state: Mode3StartupSourceState,
    startup_pre_visible_transfer_dots_remaining: u8,
    transfer_phase: Mode3TransferPhase,
    current_transfer_x: u8,
    visible_pixels_output: u8,
    window_wy_latch: bool,
    window_force_x0_this_line: bool,
    window_started_this_line: bool,
    wx0_scx_shortening_applied: bool,
    wx166_armed_this_line: bool,
}

impl BgPipelineState {
    fn reset(&mut self) {
        self.fetcher.reset();
        self.push.reset();
        self.fill.reset();
        self.fifo.clear();
        self.startup_fetch_seam = BgStartupFetchSeamState::Inactive;
        self.startup_fifo_placeholders = 0;
        self.mode3_started = false;
        self.mode0_start_dot = MODE0_START_DOT;
        self.initial_scx_discard = 0;
        self.scx_discard_remaining = 0;
        self.startup_source_state = Mode3StartupSourceState::FifoBacked;
        self.startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
        self.transfer_phase = Mode3TransferPhase::Priming;
        self.current_transfer_x = 0;
        self.visible_pixels_output = 0;
        self.window_wy_latch = false;
        self.window_force_x0_this_line = false;
        self.window_started_this_line = false;
        self.wx0_scx_shortening_applied = false;
        self.wx166_armed_this_line = false;
    }

    fn start_line(&mut self, scx: u8) {
        self.mode3_started = true;
        self.initial_scx_discard = scx & 0x07;
        self.mode0_start_dot = MODE0_START_DOT + u16::from(self.initial_scx_discard);
        self.scx_discard_remaining = self.initial_scx_discard;
        self.fifo.clear();
        self.startup_fetch_seam = BgStartupFetchSeamState::AlignmentSeedPending;
        self.startup_fifo_placeholders = MODE3_ABSTRACT_SOURCE_WINDOW_DOTS;
        self.startup_source_state = Mode3StartupSourceState::EntryDelay {
            remaining: MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT as u8,
        };
        self.startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
        self.transfer_phase = Mode3TransferPhase::Priming;
        self.current_transfer_x = 0;
        self.push.reset();
        self.fill.reset();
        self.fetcher.start_background();
    }

    fn prepare_window_line(&mut self, wy_latch: bool, force_x0_this_line: bool) {
        self.window_wy_latch = wy_latch;
        self.window_force_x0_this_line = force_x0_this_line;
        self.window_started_this_line = false;
        self.wx0_scx_shortening_applied = false;
        self.wx166_armed_this_line = false;
    }

    fn extend_mode3_by_one_dot(&mut self) {
        self.mode0_start_dot += 1;
    }

    fn startup_transfer_window_open(&self, mode3_dot: u16) -> bool {
        if !self.mode3_started {
            return mode3_dot >= MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
        }

        !matches!(
            self.startup_source_state,
            Mode3StartupSourceState::EntryDelay { .. }
        )
    }

    fn consume_startup_transfer_entry_delay_dot(&mut self) -> bool {
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

    fn current_startup_source_window(&self, mode3_dot: u16) -> Mode3TransferSourceWindow {
        if !self.mode3_started {
            if mode3_dot < MODE3_BG_FETCH_PRIMING_DOTS {
                return Mode3TransferSourceWindow::AbstractStartup;
            }

            return Mode3TransferSourceWindow::FifoBacked;
        }

        match self.startup_source_state {
            Mode3StartupSourceState::EntryDelay { .. }
            | Mode3StartupSourceState::Abstract { .. } => {
                Mode3TransferSourceWindow::AbstractStartup
            }
            Mode3StartupSourceState::FifoBacked => Mode3TransferSourceWindow::FifoBacked,
        }
    }

    fn current_startup_transfer_lane(&self) -> Mode3TransferLane {
        if self.startup_pre_visible_transfer_dots_remaining > 0 {
            Mode3TransferLane::PreVisible
        } else {
            Mode3TransferLane::Hidden
        }
    }

    fn consume_startup_source_window_dot(&mut self) {
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

    fn consume_startup_pre_visible_transfer_dot(&mut self) {
        if self.startup_pre_visible_transfer_dots_remaining > 0 {
            self.startup_pre_visible_transfer_dots_remaining -= 1;
        }
    }

    fn effective_fifo_is_empty(&self) -> bool {
        self.startup_fifo_placeholders == 0 && self.fifo.is_empty()
    }

    fn fifo_contains_real_pixels(&self) -> bool {
        self.fifo.len() > self.startup_fifo_placeholders as usize
    }

    fn consume_effective_fifo_pixel(&mut self) -> Option<u8> {
        if self.startup_fifo_placeholders > 0 {
            self.startup_fifo_placeholders -= 1;
            self.fifo.pop_front().or(Some(0))
        } else {
            self.fifo.pop_front()
        }
    }

    fn apply_wx0_scx_shortening(&mut self) {
        if self.wx0_scx_shortening_applied || self.mode0_start_dot == 0 {
            return;
        }

        self.wx0_scx_shortening_applied = true;
        self.mode0_start_dot -= 1;
    }

    fn peek_startup_background_fetch_origin(&self) -> BgCachedSliceOrigin {
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

    fn startup_alignment_seed_pending(&self) -> bool {
        matches!(
            self.startup_fetch_seam,
            BgStartupFetchSeamState::AlignmentSeedPending
        )
    }

    fn startup_background_tilemap_uses_pipeline_snapshot(&self) -> bool {
        match self.startup_fetch_seam {
            BgStartupFetchSeamState::Inactive => false,
            BgStartupFetchSeamState::AlignmentSeedPending => true,
            BgStartupFetchSeamState::PostAlignment {
                delayed_background_tilemap_tiles_remaining,
                ..
            } => delayed_background_tilemap_tiles_remaining > 0,
        }
    }

    fn startup_background_tiledata_uses_pipeline_snapshot(&self) -> bool {
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

    fn startup_background_tileindex_reads_on_stage_one(&self) -> bool {
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

    fn begin_post_alignment_followup(&mut self) {
        self.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: false,
            next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        };
    }

    fn take_startup_first_real_push_skip_entry_delay(&mut self) -> bool {
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

    fn advance_startup_background_fetch_tile(&mut self) {
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

    fn maybe_finish_startup_fetch_seam(&mut self) {
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
}

impl Default for BgPipelineState {
    fn default() -> Self {
        Self {
            fetcher: BgFetcherState::default(),
            push: BgPushState::default(),
            fill: BgFifoFillState::default(),
            fifo: VecDeque::default(),
            startup_fetch_seam: BgStartupFetchSeamState::Inactive,
            startup_fifo_placeholders: 0,
            mode3_started: false,
            mode0_start_dot: MODE0_START_DOT,
            initial_scx_discard: 0,
            scx_discard_remaining: 0,
            startup_source_state: Mode3StartupSourceState::FifoBacked,
            startup_pre_visible_transfer_dots_remaining: MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS,
            transfer_phase: Mode3TransferPhase::Priming,
            current_transfer_x: 0,
            visible_pixels_output: 0,
            window_wy_latch: false,
            window_force_x0_this_line: false,
            window_started_this_line: false,
            wx0_scx_shortening_applied: false,
            wx166_armed_this_line: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BgStartupFetchSeamState {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BgStartupContinuationSlice {
    #[default]
    None,
    VisibleTile2,
    VisibleTile3,
}

impl BgStartupContinuationSlice {
    const fn next(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::VisibleTile2 => Self::VisibleTile3,
            Self::VisibleTile3 => Self::None,
        }
    }

    const fn is_third_visible_tile(self) -> bool {
        matches!(self, Self::VisibleTile3)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct BgFetcherState {
    source: PpuBgFetcherSource,
    stage: PpuBgFetcherStage,
    stage_dot: u8,
    cached_origin: BgCachedSliceOrigin,
    fetch_x: u16,
    next_fetch_pixel: u16,
    post_alignment_fetch_restart_delay_dots: u8,
    window_tilemap_x: u8,
    bg_resume_fetch_pixel: u16,
    rewind_bg_resume_after_first_tile_index_dot: bool,
    first_window_tile_after_activation: bool,
    tile_map_address: u16,
    tile_data_address: u16,
    tile_index: u8,
    tile_low: u8,
    tile_high: u8,
}

impl BgFetcherState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn start_background(&mut self) {
        self.source = PpuBgFetcherSource::Background;
        self.start_common(0);
    }

    fn start_window(&mut self, bg_resume_fetch_pixel: u16) {
        self.source = PpuBgFetcherSource::Window;
        self.stage = PpuBgFetcherStage::WindowActivating;
        self.stage_dot = 0;
        self.cached_origin = BgCachedSliceOrigin::Ordinary;
        self.fetch_x = 0;
        self.next_fetch_pixel = 0;
        self.post_alignment_fetch_restart_delay_dots = 0;
        self.window_tilemap_x = 0;
        self.bg_resume_fetch_pixel = bg_resume_fetch_pixel;
        self.rewind_bg_resume_after_first_tile_index_dot = true;
        self.first_window_tile_after_activation = true;
        self.tile_map_address = 0;
        self.tile_data_address = 0;
        self.tile_index = 0;
        self.tile_low = 0;
        self.tile_high = 0;
    }

    fn start_common(&mut self, bg_resume_fetch_pixel: u16) {
        self.stage = PpuBgFetcherStage::TileIndex;
        self.stage_dot = 0;
        self.cached_origin = BgCachedSliceOrigin::Ordinary;
        self.fetch_x = 0;
        self.next_fetch_pixel = 0;
        self.post_alignment_fetch_restart_delay_dots = 0;
        self.window_tilemap_x = 0;
        self.bg_resume_fetch_pixel = bg_resume_fetch_pixel;
        self.rewind_bg_resume_after_first_tile_index_dot = false;
        self.first_window_tile_after_activation = false;
        self.tile_map_address = 0;
        self.tile_data_address = 0;
        self.tile_index = 0;
        self.tile_low = 0;
        self.tile_high = 0;
    }

    fn abort_window_to_background(&mut self) {
        if self.source != PpuBgFetcherSource::Window {
            return;
        }

        self.source = PpuBgFetcherSource::Background;
        self.cached_origin = BgCachedSliceOrigin::Ordinary;
        self.fetch_x = self.bg_resume_fetch_pixel;
        self.next_fetch_pixel = self.bg_resume_fetch_pixel;
        self.post_alignment_fetch_restart_delay_dots = 0;
        self.window_tilemap_x = 0;
        self.first_window_tile_after_activation = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct BgPushState {
    pending: bool,
    disposition: BgPushDisposition,
    entry_delay_remaining: u8,
    just_activated_window_tile: bool,
    next_fetch_pixel: u16,
    cached: BgCachedSlice,
}

impl BgPushState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn queue_from_fetcher(&mut self, fetcher: BgFetcherState) {
        self.pending = true;
        self.disposition = BgPushDisposition::Ready;
        self.just_activated_window_tile = fetcher.first_window_tile_after_activation;
        self.entry_delay_remaining = if self.just_activated_window_tile {
            0
        } else {
            1
        };
        self.next_fetch_pixel = fetcher.fetch_x.wrapping_add(BG_TILE_WIDTH as u16);
        self.cached = BgCachedSlice::from_fetcher(fetcher);
    }

    fn queue_startup_alignment_seed_from_fetcher(&mut self, fetcher: BgFetcherState) {
        self.pending = true;
        self.disposition = BgPushDisposition::Ready;
        self.just_activated_window_tile = fetcher.first_window_tile_after_activation;
        self.entry_delay_remaining = 0;
        self.next_fetch_pixel = fetcher.fetch_x.wrapping_add(BG_TILE_WIDTH as u16);
        self.cached = BgCachedSlice::from_fetcher(fetcher)
            .with_origin(BgCachedSliceOrigin::StartupAlignmentSeed);
    }

    fn interrupt_for_object_fetch(&mut self) {
        if !self.pending {
            return;
        }

        self.disposition = BgPushDisposition::InterruptedByObjectFetch;
    }

    fn resume_after_object_fetch(&mut self) {
        if self.pending && self.disposition == BgPushDisposition::InterruptedByObjectFetch {
            self.disposition = BgPushDisposition::Ready;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct BgFifoFillState {
    pending: bool,
    startup_dummy_pixels: u8,
    includes_real_tile_pixels: bool,
    cached: BgCachedSlice,
}

impl BgFifoFillState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn queue_from_push(&mut self, push: BgPushState) {
        self.pending = true;
        self.startup_dummy_pixels = 0;
        self.includes_real_tile_pixels = true;
        self.cached = push.cached;
    }

    fn queue_startup_alignment_from_push(&mut self, push: BgPushState, startup_dummy_pixels: u8) {
        self.pending = true;
        self.startup_dummy_pixels = startup_dummy_pixels;
        self.includes_real_tile_pixels = true;
        self.cached = push.cached.with_origin(push.cached.queued_fill_origin());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BgCachedSliceOrigin {
    #[default]
    Ordinary,
    StartupAlignmentSeed,
    StartupAlignmentFill,
    StartupContinuation(BgStartupContinuationSlice),
}

impl BgCachedSliceOrigin {
    const fn from_startup_continuation_slice(slice: BgStartupContinuationSlice) -> Self {
        match slice {
            BgStartupContinuationSlice::None => Self::Ordinary,
            slice => Self::StartupContinuation(slice),
        }
    }

    const fn startup_continuation_slice(self) -> BgStartupContinuationSlice {
        match self {
            Self::StartupContinuation(slice) => slice,
            Self::Ordinary | Self::StartupAlignmentSeed | Self::StartupAlignmentFill => {
                BgStartupContinuationSlice::None
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct BgCachedSlice {
    source: PpuBgFetcherSource,
    origin: BgCachedSliceOrigin,
    fetch_x: u16,
    same_cycle_live_tilemap_refetch_window_open: bool,
    needs_live_tilemap_refetch: bool,
    needs_live_tile_data_refetch: bool,
    needs_live_tile_data_unsigned_reuse: bool,
    tile_map_address: u16,
    tile_data_address: u16,
    tile_index: u8,
    tile_low: u8,
    tile_high: u8,
}

impl BgCachedSlice {
    fn from_fetcher(fetcher: BgFetcherState) -> Self {
        Self {
            source: fetcher.source,
            origin: fetcher.cached_origin,
            fetch_x: fetcher.fetch_x,
            same_cycle_live_tilemap_refetch_window_open: false,
            needs_live_tilemap_refetch: false,
            needs_live_tile_data_refetch: false,
            needs_live_tile_data_unsigned_reuse: false,
            tile_map_address: fetcher.tile_map_address,
            tile_data_address: fetcher.tile_data_address,
            tile_index: fetcher.tile_index,
            tile_low: fetcher.tile_low,
            tile_high: fetcher.tile_high,
        }
    }

    fn with_origin(mut self, origin: BgCachedSliceOrigin) -> Self {
        self.origin = origin;
        self
    }

    const fn is_background(self) -> bool {
        matches!(self.source, PpuBgFetcherSource::Background)
    }

    const fn is_startup_alignment_seed(self) -> bool {
        matches!(self.origin, BgCachedSliceOrigin::StartupAlignmentSeed)
    }

    const fn queued_fill_origin(self) -> BgCachedSliceOrigin {
        match self.origin {
            BgCachedSliceOrigin::StartupAlignmentSeed => BgCachedSliceOrigin::StartupAlignmentFill,
            origin => origin,
        }
    }

    const fn startup_continuation_slice(self) -> BgStartupContinuationSlice {
        self.origin.startup_continuation_slice()
    }

    const fn is_third_visible_post_startup_push(self) -> bool {
        self.startup_continuation_slice().is_third_visible_tile()
            && self.fetch_x == BG_TILE_WIDTH as u16 * 2
    }

    fn mark_live_register_write_while_push_pending(
        &mut self,
        address: u16,
        previous_lcdc: u8,
        lcdc: u8,
        entry_delay_active: bool,
    ) {
        if !self.is_background() || self.is_startup_alignment_seed() {
            return;
        }

        let tile_data_selector_changed = (previous_lcdc ^ lcdc) & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;
        let needs_tilemap_refetch = address == 0xFF40
            && (previous_lcdc ^ lcdc) & LCDC_BG_TILE_MAP_BIT != 0
            && (entry_delay_active
                || self.same_cycle_live_tilemap_refetch_window_open
                || self.is_third_visible_post_startup_push());
        let needs_tile_data_refetch = match address {
            0xFF40 => tile_data_selector_changed && lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT == 0,
            0xFF42 => true,
            _ => false,
        };
        let needs_tile_data_unsigned_reuse = address == 0xFF40
            && tile_data_selector_changed
            && lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;

        self.needs_live_tilemap_refetch |= needs_tilemap_refetch;
        self.needs_live_tile_data_refetch |= needs_tile_data_refetch;
        self.needs_live_tile_data_unsigned_reuse |= needs_tile_data_unsigned_reuse;
    }

    fn mark_live_register_write_while_fill_pending(
        &mut self,
        address: u16,
        previous_lcdc: u8,
        lcdc: u8,
        includes_real_tile_pixels: bool,
        startup_dummy_pixels: u8,
    ) {
        if !self.is_background() || !includes_real_tile_pixels {
            return;
        }

        if address == 0xFF40
            && (previous_lcdc ^ lcdc) & LCDC_BG_TILE_MAP_BIT != 0
            && startup_dummy_pixels == 0
            && (self.same_cycle_live_tilemap_refetch_window_open
                || self.is_third_visible_post_startup_push())
        {
            self.needs_live_tilemap_refetch = true;
        }

        let tile_data_selector_changed = (previous_lcdc ^ lcdc) & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;
        let needs_tile_data_refetch = match address {
            0xFF40 => tile_data_selector_changed && lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT == 0,
            0xFF42 => true,
            _ => false,
        };
        let needs_tile_data_unsigned_reuse = address == 0xFF40
            && tile_data_selector_changed
            && lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0;

        self.needs_live_tile_data_refetch |= needs_tile_data_refetch;
        self.needs_live_tile_data_unsigned_reuse |= needs_tile_data_unsigned_reuse;
    }
}

fn recompute_live_background_cached_slice(
    mut cached: BgCachedSlice,
    vram: &VramBusView<'_>,
    lcdc: u8,
    scy: u8,
    ly: u8,
    last_unsigned_tile_data_low_fetch: u8,
    last_unsigned_tile_data_high_fetch: u8,
) -> Option<BgCachedSlice> {
    if cached.source != PpuBgFetcherSource::Background
        || (!cached.needs_live_tilemap_refetch
            && !cached.needs_live_tile_data_refetch
            && !cached.needs_live_tile_data_unsigned_reuse)
    {
        return None;
    }

    let mut tile_map_address = cached.tile_map_address;
    let mut tile_index = cached.tile_index;
    if cached.needs_live_tilemap_refetch {
        let tile_map_offset = cached.tile_map_address & 0x03FF;
        let tile_map_base = if lcdc & LCDC_BG_TILE_MAP_BIT != 0 {
            0x1C00
        } else {
            0x1800
        };
        tile_map_address = tile_map_base | tile_map_offset;
        tile_index = vram.read(tile_map_address as usize).unwrap_or(0);
    }

    let tile_data_row = if cached.needs_live_tile_data_refetch {
        u16::from(scy.wrapping_add(ly) % BG_TILE_WIDTH)
    } else {
        (cached.tile_data_address.saturating_sub(1) & (TILE_BYTES - 1)) / TILE_ROW_BYTES
    };
    let tile_low_address = bg_tile_data_base(lcdc, tile_index) + tile_data_row * TILE_ROW_BYTES;
    let tile_high_address = tile_low_address + 1;
    let (tile_low, tile_high) =
        if cached.needs_live_tile_data_unsigned_reuse && !cached.needs_live_tilemap_refetch {
            (
                last_unsigned_tile_data_low_fetch,
                last_unsigned_tile_data_high_fetch,
            )
        } else {
            (
                vram.read(tile_low_address as usize).unwrap_or(0),
                vram.read(tile_high_address as usize).unwrap_or(0),
            )
        };

    cached.tile_map_address = tile_map_address;
    cached.tile_data_address = tile_high_address;
    cached.tile_index = tile_index;
    cached.tile_low = tile_low;
    cached.tile_high = tile_high;
    cached.needs_live_tilemap_refetch = false;
    cached.needs_live_tile_data_refetch = false;
    cached.needs_live_tile_data_unsigned_reuse = false;
    Some(cached)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BgPushDisposition {
    #[default]
    Ready,
    InterruptedByObjectFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BgPushDotResult {
    NotReady,
    EntryDelay,
    WaitingForEmptyFifo,
    HandedOffToObjectFetch,
    QueuedFillAndHandedOffToObjectFetch,
    QueuedFill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BgPushDotOwnership {
    NotReady,
    EntryDelay,
    WaitingForEmptyFifo,
    FifoBackedTransferObjectFetch,
    QueueFill,
    QueueFillThenObjectFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode3DotArbitration {
    bg_transfer_can_advance: bool,
    obj_fetch_can_start_from_fifo_backed_transfer: bool,
    obj_fetch_can_start_from_queued_bg_fill: bool,
}

impl Mode3DotArbitration {
    const fn can_serve_bg_transfer(self) -> bool {
        self.bg_transfer_can_advance
    }

    const fn can_start_obj_fetch(self, start_source: ObjFetchStartSource) -> bool {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjFetchStartSource {
    FifoBackedTransfer,
    PushCachedBgFetch,
    QueuedBgFill,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct WindowState {
    wy_triggered: bool,
    pending_wx166_next_line: bool,
    window_line_counter: u8,
}

impl WindowState {
    fn reset(&mut self) {
        self.wy_triggered = false;
        self.pending_wx166_next_line = false;
        self.window_line_counter = 0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct StatState {
    irq_line: bool,
    lcd_disabled_lyc_coincidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ObjPipelineState {
    fifo: VecDeque<ObjPixel>,
    fetched_sprite_slots: [bool; MAX_SELECTED_SPRITES_PER_LINE],
    pending_sprite_slots: VecDeque<u8>,
    pending_match_x: Option<u8>,
    late_metadata_word: Option<(u8, u8)>,
    fetch: ObjFetchState,
}

impl ObjPipelineState {
    fn reset(&mut self) {
        self.fifo.clear();
        self.fetched_sprite_slots.fill(false);
        self.pending_sprite_slots.clear();
        self.pending_match_x = None;
        self.late_metadata_word = None;
        self.fetch = ObjFetchState::default();
    }

    fn start_fetch(&mut self, sprite_slot: u8, sprite: PpuSelectedSprite) {
        self.fetch.stage = PpuObjFetcherStage::Startup;
        self.fetch.stage_dot = 0;
        self.fetch.sprite_slot = sprite_slot;
        self.fetch.sprite = Some(sprite);
        self.fetch.resolved_sprite = None;
        self.fetch.cancelled = false;
        self.fetch.tile_low = 0;
        self.fetch.tile_high = 0;
    }

    fn mark_fetched(&mut self, sprite_slot: u8) {
        self.fetched_sprite_slots[sprite_slot as usize] = true;
    }

    fn has_fetched(&self, sprite_slot: u8) -> bool {
        self.fetched_sprite_slots[sprite_slot as usize]
    }

    fn queue_fetch_hit(&mut self, sprite_slot: u8, owner: ObjHitOwnership) {
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
    }

    fn pop_pending_fetch_hit(&mut self) -> Option<u8> {
        let sprite_slot = self.pending_sprite_slots.pop_front();
        if self.pending_sprite_slots.is_empty() {
            self.pending_match_x = None;
        }
        sprite_slot
    }

    fn pending_hits_own_current_dot(&self, current_owner: ObjHitOwnership) -> bool {
        self.pending_match_x == Some(current_owner.match_x) && !self.pending_sprite_slots.is_empty()
    }

    fn clear_pending_fetch_hits(&mut self) {
        self.pending_sprite_slots.clear();
        self.pending_match_x = None;
    }

    fn clear_pending_fetch_hits_if_stale(&mut self, current_owner: ObjHitOwnership) {
        if self.fetch.stage != PpuObjFetcherStage::Idle {
            return;
        }

        if self.pending_match_x.is_some() && self.pending_match_x != Some(current_owner.match_x) {
            self.clear_pending_fetch_hits();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjHitOwnership {
    match_x: u8,
    phase: ObjHitPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjHitPhase {
    PreVisible,
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ObjFetchState {
    stage: PpuObjFetcherStage,
    stage_dot: u8,
    sprite_slot: u8,
    sprite: Option<PpuSelectedSprite>,
    resolved_sprite: Option<PpuSelectedSprite>,
    cancelled: bool,
    tile_low: u8,
    tile_high: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjPixel {
    color: u8,
    palette_obp1: bool,
    bg_over_obj: bool,
    sprite_x: u8,
    oam_index: u8,
}

impl ObjPixel {
    const fn transparent() -> Self {
        Self {
            color: 0,
            palette_obp1: false,
            bg_over_obj: false,
            sprite_x: u8::MAX,
            oam_index: u8::MAX,
        }
    }

    const fn is_transparent(self) -> bool {
        self.color == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MixedPixel {
    color: u8,
    source: MixedPixelSource,
}

impl MixedPixel {
    const fn background(color: u8) -> Self {
        Self {
            color,
            source: MixedPixelSource::Background,
        }
    }

    const fn object(color: u8, palette_obp1: bool) -> Self {
        Self {
            color,
            source: MixedPixelSource::Object { palette_obp1 },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MixedPixelSource {
    Background,
    Object { palette_obp1: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mode2ScanState {
    scanned_entries: u8,
    selected_sprite_count: u8,
    selected_sprites: [Option<PpuSelectedSprite>; MAX_SELECTED_SPRITES_PER_LINE],
    latched_mode2_yx_word: Option<(u8, u8)>,
}

impl Mode2ScanState {
    fn reset_scanline(&mut self) {
        self.scanned_entries = 0;
        self.selected_sprite_count = 0;
        self.selected_sprites.fill(None);
    }

    fn reset(&mut self) {
        self.reset_scanline();
        self.latched_mode2_yx_word = None;
    }

    fn scanned_entries(&self) -> u8 {
        self.scanned_entries
    }

    fn increment_scanned_entries(&mut self) {
        self.scanned_entries += 1;
    }

    fn latch_mode2_yx_word(&mut self, y: u8, x: u8) {
        self.latched_mode2_yx_word = Some((y, x));
    }

    fn latched_mode2_yx_word(&self) -> Option<(u8, u8)> {
        self.latched_mode2_yx_word
    }

    fn selected_sprite_count(&self) -> u8 {
        self.selected_sprite_count
    }

    fn is_full(&self) -> bool {
        self.selected_sprite_count as usize == MAX_SELECTED_SPRITES_PER_LINE
    }

    fn push(&mut self, sprite: PpuSelectedSprite) {
        if self.is_full() {
            return;
        }

        let slot = self.selected_sprite_count as usize;
        self.selected_sprites[slot] = Some(sprite);
        self.selected_sprite_count += 1;
    }

    fn selected_sprites_snapshot(&self) -> Vec<PpuSelectedSprite> {
        self.selected_sprites
            .iter()
            .take(self.selected_sprite_count as usize)
            .flatten()
            .copied()
            .collect()
    }

    fn selected_sprite(&self, slot: u8) -> Option<PpuSelectedSprite> {
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

fn push_bg_tile_pixels(fifo: &mut VecDeque<u8>, tile_low: u8, tile_high: u8) {
    for bit in (0..BG_TILE_WIDTH).rev() {
        let low_bit = (tile_low >> bit) & 0x01;
        let high_bit = (tile_high >> bit) & 0x01;
        fifo.push_back((high_bit << 1) | low_bit);
    }
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
