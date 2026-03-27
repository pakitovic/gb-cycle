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
    startup_mode_latch: Option<PpuAccessMode>,
    stat_state: StatState,
    pending_interrupts: u8,
    blank_frame_active: bool,
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
            startup_mode_latch: None,
            stat_state: StatState::default(),
            pending_interrupts: 0,
            blank_frame_active: false,
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

        if !self.is_lcd_enabled() {
            self.sync_visible_registers();
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
        self.current_scanline_pixels.fill(0);
        self.current_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
        self.framebuffer.fill(0);
        self.sync_visible_registers();
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

    pub(crate) fn tick_t_cycle(
        &mut self,
        _context: &mut CycleContext,
        oam: OamBusView<'_>,
        vram: VramBusView<'_>,
        dma_oam_active: bool,
        dma_oam_conflict_address: Option<u16>,
    ) {
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

        self.sync_visible_registers();
        let previous_mode = self.current_access_mode();
        self.startup_mode_latch = None;
        self.line_dot += 1;
        self.advance_lcd_restart_phase();
        self.prepare_visible_scanline_state();
        self.advance_mode2_scan(&oam, dma_oam_active);
        self.advance_mode3_pipeline(&oam, &vram, dma_oam_conflict_address);

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
            obj_palette_read_policy: self.obj_palette_read_policy,
        }
    }

    pub fn ly(&self) -> u8 {
        self.ly
    }

    pub fn line_dot(&self) -> u16 {
        self.line_dot
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
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
            let Some((y, x)) = self.mode2_scan_state.latched_oam_word() else {
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
            self.mode2_scan_state.latch_oam_word(sprite.y, sprite.x);
            sprite
        };

        if sprite_matches_line(sprite, self.ly, self.current_obj_height()) {
            self.mode2_scan_state.push(sprite);
        }
    }

    fn current_obj_height(&self) -> u8 {
        self.visible_registers.obj_height()
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

    fn advance_mode3_pipeline(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict_address: Option<u16>,
    ) {
        if self.ly >= VISIBLE_SCANLINES
            || self.line_dot < MODE2_DOTS
            || self.line_dot >= self.current_mode0_start_dot()
        {
            return;
        }

        if !self.bg_pipeline_state.mode3_started {
            self.bg_pipeline_state
                .start_line(self.visible_registers.scx);
        }

        self.flush_pending_bg_fifo_fill();

        if self.advance_mode3_object_phase(oam, vram, dma_oam_conflict_address) {
            return;
        }

        let output_dot = self.advance_mode3_output_phase();
        self.maybe_apply_wx0_shortening_after_transfer_dot(output_dot);
        let _ = self.maybe_start_window_after_transfer_dot(output_dot);
        let _ = self.advance_bg_fetcher(vram);
    }

    fn advance_mode3_object_phase(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict_address: Option<u16>,
    ) -> bool {
        self.sync_pending_obj_hit_ownership();
        self.latch_object_fetch_hits();
        self.try_start_object_fetch_from_current_dot(
            ObjFetchStartSource::FifoBackedTransfer,
            false,
        );
        self.advance_object_fetch(oam, vram, dma_oam_conflict_address)
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
            match self.current_transfer_readiness() {
                None => return Mode3TransferDot::not_served(),
                Some(Mode3TransferReadiness::WaitingForFifo(_)) => {
                    self.bg_pipeline_state.extend_mode3_by_one_dot();
                    Mode3TransferDot::not_served()
                }
                Some(Mode3TransferReadiness::Ready(plan)) => {
                    self.execute_transfer_service_plan(plan)
                }
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
        let current_transfer_is_fifo_backed = self
            .current_transfer_readiness()
            .is_some_and(|readiness| readiness.can_start_obj_fetch_from_fifo_backed_transfer());

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

    fn current_transfer_service_plan(&self) -> Option<Mode3TransferServicePlan> {
        let context = self.current_transfer_context()?;
        let action = if self.bg_pipeline_state.scx_discard_remaining > 0 {
            Mode3TransferServiceAction::ConsumeScxDiscard
        } else if self.bg_pipeline_state.current_transfer_x < 8 {
            Mode3TransferServiceAction::AdvanceHiddenX
        } else if context.lane == Mode3TransferLane::Visible {
            Mode3TransferServiceAction::EmitVisiblePixel
        } else {
            return None;
        };

        let result_kind = if matches!(action, Mode3TransferServiceAction::EmitVisiblePixel) {
            Mode3TransferDotKind::ServedVisiblePixel
        } else {
            context.lane.dot_kind()
        };

        let backing = match context.source_window {
            Mode3TransferSourceWindow::AbstractStartup => {
                if context.lane == Mode3TransferLane::Hidden {
                    Mode3TransferBacking::HiddenStartupFifoGated
                } else {
                    Mode3TransferBacking::Abstract
                }
            }
            Mode3TransferSourceWindow::FifoBacked => Mode3TransferBacking::FifoBacked,
        };

        Some(Mode3TransferServicePlan {
            result_kind,
            action,
            backing,
        })
    }

    fn current_transfer_readiness(&self) -> Option<Mode3TransferReadiness> {
        let plan = self.current_transfer_service_plan()?;
        if plan.backing.requires_fifo_backing() && self.bg_pipeline_state.fifo.is_empty() {
            Some(Mode3TransferReadiness::WaitingForFifo(plan))
        } else {
            Some(Mode3TransferReadiness::Ready(plan))
        }
    }

    fn advance_bg_fetcher(&mut self, vram: &VramBusView<'_>) -> bool {
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

        let fetcher = self.bg_pipeline_state.fetcher;
        match (fetcher.stage, fetcher.stage_dot) {
            (PpuBgFetcherStage::TileIndex, 0) => {
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileIndex, 1) => {
                self.bg_pipeline_state.fetcher.tile_index =
                    self.read_fetch_tile_index(vram, fetcher.source, fetcher.next_fetch_pixel);
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
            }
            (PpuBgFetcherStage::TileDataLow, 0) => {
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileDataLow, 1) => {
                self.bg_pipeline_state.fetcher.tile_low =
                    self.read_fetch_tile_data_byte(vram, fetcher.source, fetcher.tile_index, 0);
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
            }
            (PpuBgFetcherStage::TileDataHigh, 0) => {
                self.bg_pipeline_state.fetcher.stage_dot = 1;
            }
            (PpuBgFetcherStage::TileDataHigh, 1) => {
                self.bg_pipeline_state.fetcher.tile_high =
                    self.read_fetch_tile_data_byte(vram, fetcher.source, fetcher.tile_index, 1);
                self.bg_pipeline_state
                    .push
                    .queue_from_fetcher(self.bg_pipeline_state.fetcher);
                self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
                self.bg_pipeline_state.fetcher.stage_dot = 0;
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

    fn advance_bg_push_stage(&mut self) -> BgPushDotResult {
        if !self.bg_pipeline_state.push.pending {
            return BgPushDotResult::NotReady;
        }

        if !self.bg_pipeline_state.push.advance_entry_delay() {
            return BgPushDotResult::EntryDelay;
        }

        self.advance_bg_push()
    }

    fn advance_bg_push(&mut self) -> BgPushDotResult {
        if !self.bg_pipeline_state.push.is_ready_for_fifo_push() {
            return BgPushDotResult::NotReady;
        }

        if !self.bg_pipeline_state.fifo.is_empty() {
            if self.try_start_object_fetch_from_current_dot(
                ObjFetchStartSource::FifoBackedTransfer,
                true,
            ) {
                return BgPushDotResult::HandedOffToObjectFetch;
            }
            return BgPushDotResult::WaitingForEmptyFifo;
        }

        let push = self.bg_pipeline_state.push;
        self.bg_pipeline_state.fill.queue_from_push(push);
        self.bg_pipeline_state.fetcher.next_fetch_pixel = push.next_fetch_pixel;
        self.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
        self.bg_pipeline_state.push.reset();

        if self.try_start_object_fetch_from_current_dot(ObjFetchStartSource::QueuedBgFill, true) {
            return BgPushDotResult::QueuedFillAndHandedOffToObjectFetch;
        }

        BgPushDotResult::QueuedFill
    }

    fn flush_pending_bg_fifo_fill(&mut self) {
        if !self.bg_pipeline_state.fill.pending {
            return;
        }

        let fill = self.bg_pipeline_state.fill;
        push_bg_tile_pixels(
            &mut self.bg_pipeline_state.fifo,
            fill.tile_low,
            fill.tile_high,
        );
        self.bg_pipeline_state.fill.reset();
    }

    fn execute_transfer_service_plan(
        &mut self,
        plan: Mode3TransferServicePlan,
    ) -> Mode3TransferDot {
        let pixel = if plan.backing.pops_bg_fifo() {
            Some(
                self.bg_pipeline_state
                    .fifo
                    .pop_front()
                    .expect("fifo-backed transfer plans must run only when the BG FIFO is ready"),
            )
        } else {
            None
        };

        self.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        if !matches!(plan.action, Mode3TransferServiceAction::EmitVisiblePixel) {
            self.bg_pipeline_state
                .consume_startup_pre_visible_transfer_dot();
        }

        match plan.action {
            Mode3TransferServiceAction::ConsumeScxDiscard => {
                self.bg_pipeline_state.scx_discard_remaining -= 1;
                Mode3TransferDot::served(plan.result_kind, true)
            }
            Mode3TransferServiceAction::AdvanceHiddenX => {
                self.bg_pipeline_state.current_transfer_x += 1;
                if plan.backing.pops_obj_fifo_on_hidden_service() {
                    let _ = self.pop_obj_fifo_pixel();
                }
                Mode3TransferDot::served(plan.result_kind, false)
            }
            Mode3TransferServiceAction::EmitVisiblePixel => {
                let bg_pixel = if self.bg_enabled() {
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

    fn bg_enabled(&self) -> bool {
        self.visible_registers.bg_enabled()
    }

    fn obj_enabled(&self) -> bool {
        self.visible_registers.obj_enabled()
    }

    fn maybe_apply_wx0_shortening_after_transfer_dot(&mut self, transfer_dot: Mode3TransferDot) {
        if !transfer_dot.consumed_scx_discard
            || self.bg_pipeline_state.window_started_this_line
            || !self.bg_pipeline_state.window_wy_latch
            || !self.window_runtime_enabled()
            || self.visible_registers.wx != 0
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

        if self.visible_registers.wx == 166 && !self.bg_pipeline_state.window_force_x0_this_line {
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
        self.visible_registers.window_enabled() && self.bg_enabled()
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
            if matches!(start_source, ObjFetchStartSource::FifoBackedTransfer) {
                self.bg_pipeline_state.push.interrupt_for_object_fetch();
            }
            self.bg_pipeline_state.extend_mode3_by_one_dot();
            self.obj_pipeline_state.fetch.stage_dot = 1;
        }
        true
    }

    fn current_obj_hit_ownership(&self) -> ObjHitOwnership {
        let phase = self
            .current_transfer_context()
            .map_or(ObjHitPhase::PreVisible, |context| match context.lane {
                Mode3TransferLane::PreVisible => ObjHitPhase::PreVisible,
                Mode3TransferLane::Hidden => ObjHitPhase::Hidden,
                Mode3TransferLane::Visible => ObjHitPhase::Visible,
            });

        ObjHitOwnership {
            match_x: self.bg_pipeline_state.current_transfer_x,
            phase,
        }
    }

    fn advance_object_fetch(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict_address: Option<u16>,
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
                let resolved_sprite = fetch.sprite.map(|sprite| {
                    self.resolve_obj_fetch_sprite(oam, sprite, dma_oam_conflict_address)
                });
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
        dma_oam_conflict_address: Option<u16>,
    ) -> PpuSelectedSprite {
        let (tile_index, attributes) =
            read_obj_fetch_sprite_metadata(oam, sprite, dma_oam_conflict_address);
        self.mode2_scan_state.latch_oam_word(tile_index, attributes);

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

        match self.visible_registers.wx {
            0..=166 => Some(self.visible_registers.wx.saturating_sub(7)),
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
        self.bg_pipeline_state.fifo.clear();
        self.bg_pipeline_state.push.reset();
        self.bg_pipeline_state.fill.reset();
        self.bg_pipeline_state.fetcher.start_window();
        self.bg_pipeline_state.scx_discard_remaining = 0;
        self.bg_pipeline_state.window_started_this_line = true;
        self.bg_pipeline_state.window_force_x0_this_line = false;
    }

    fn read_fetch_tile_index(
        &self,
        vram: &VramBusView<'_>,
        source: PpuBgFetcherSource,
        next_fetch_pixel: u16,
    ) -> u8 {
        let (tile_map_base, tile_x, tile_y) = match source {
            PpuBgFetcherSource::Background => {
                let bg_x = self
                    .visible_registers
                    .scx
                    .wrapping_add(next_fetch_pixel as u8);
                let bg_y = self.visible_registers.scy.wrapping_add(self.ly);
                let tile_map_base = if self.visible_registers.lcdc & LCDC_BG_TILE_MAP_BIT != 0 {
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
                let tile_map_base = if self.visible_registers.lcdc & LCDC_WINDOW_TILE_MAP_BIT != 0 {
                    0x1C00
                } else {
                    0x1800
                };
                (
                    tile_map_base,
                    (next_fetch_pixel / BG_TILE_WIDTH as u16) as usize,
                    (self.window_state.window_line_counter / BG_TILE_WIDTH) as usize,
                )
            }
        };
        let tile_map_address = tile_map_base + tile_y * BG_TILE_MAP_WIDTH as usize + tile_x;

        vram.read(tile_map_address).unwrap_or(0)
    }

    fn read_fetch_tile_data_byte(
        &self,
        vram: &VramBusView<'_>,
        source: PpuBgFetcherSource,
        tile_index: u8,
        plane: u16,
    ) -> u8 {
        let tile_row = match source {
            PpuBgFetcherSource::Background => {
                (self.visible_registers.scy.wrapping_add(self.ly) % BG_TILE_WIDTH) as u16
            }
            PpuBgFetcherSource::Window => {
                (self.window_state.window_line_counter % BG_TILE_WIDTH) as u16
            }
        };
        let tile_data_base = bg_tile_data_base(self.visible_registers.lcdc, tile_index);
        let byte_address = tile_data_base + tile_row * TILE_ROW_BYTES + plane;

        vram.read(byte_address as usize).unwrap_or(0)
    }

    fn read_obj_tile_data_byte(
        &self,
        vram: &VramBusView<'_>,
        sprite: PpuSelectedSprite,
        plane: u16,
    ) -> u8 {
        let Some((tile_index, tile_row)) = self.obj_tile_index_and_row(sprite) else {
            return 0;
        };
        let byte_address =
            tile_index as u16 * TILE_BYTES + tile_row as u16 * TILE_ROW_BYTES + plane;

        vram.read(byte_address as usize).unwrap_or(0)
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
        if !self.obj_enabled() || obj_pixel.is_transparent() {
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
                self.apply_dmg_palette(self.visible_registers.bgp, pixel.color)
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
        self.visible_output = if self.is_lcd_enabled() && !self.blank_frame_active {
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
        self.pending_interrupts = 0;
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
    action: Mode3TransferServiceAction,
    backing: Mode3TransferBacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferReadiness {
    WaitingForFifo(Mode3TransferServicePlan),
    Ready(Mode3TransferServicePlan),
}

impl Mode3TransferReadiness {
    const fn can_start_obj_fetch_from_fifo_backed_transfer(self) -> bool {
        match self {
            Self::Ready(plan) => plan.backing.pops_bg_fifo(),
            Self::WaitingForFifo(_) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferServiceAction {
    ConsumeScxDiscard,
    AdvanceHiddenX,
    EmitVisiblePixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode3TransferBacking {
    Abstract,
    HiddenStartupFifoGated,
    FifoBacked,
}

impl Mode3TransferBacking {
    const fn requires_fifo_backing(self) -> bool {
        matches!(self, Self::HiddenStartupFifoGated | Self::FifoBacked)
    }

    const fn pops_bg_fifo(self) -> bool {
        matches!(self, Self::FifoBacked)
    }

    const fn pops_obj_fifo_on_hidden_service(self) -> bool {
        matches!(self, Self::FifoBacked)
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

    fn apply_wx0_scx_shortening(&mut self) {
        if self.wx0_scx_shortening_applied || self.mode0_start_dot == 0 {
            return;
        }

        self.wx0_scx_shortening_applied = true;
        self.mode0_start_dot -= 1;
    }
}

impl Default for BgPipelineState {
    fn default() -> Self {
        Self {
            fetcher: BgFetcherState::default(),
            push: BgPushState::default(),
            fill: BgFifoFillState::default(),
            fifo: VecDeque::default(),
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
struct BgFetcherState {
    source: PpuBgFetcherSource,
    stage: PpuBgFetcherStage,
    stage_dot: u8,
    next_fetch_pixel: u16,
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
        self.start_common();
    }

    fn start_window(&mut self) {
        self.source = PpuBgFetcherSource::Window;
        self.stage = PpuBgFetcherStage::WindowActivating;
        self.stage_dot = 0;
        self.next_fetch_pixel = 0;
        self.tile_index = 0;
        self.tile_low = 0;
        self.tile_high = 0;
    }

    fn start_common(&mut self) {
        self.stage = PpuBgFetcherStage::TileIndex;
        self.stage_dot = 0;
        self.next_fetch_pixel = 0;
        self.tile_index = 0;
        self.tile_low = 0;
        self.tile_high = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct BgPushState {
    pending: bool,
    disposition: BgPushDisposition,
    entry_delay_remaining: u8,
    source: PpuBgFetcherSource,
    tile_low: u8,
    tile_high: u8,
    next_fetch_pixel: u16,
}

impl BgPushState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn queue_from_fetcher(&mut self, fetcher: BgFetcherState) {
        self.pending = true;
        self.disposition = BgPushDisposition::Ready;
        self.entry_delay_remaining = 1;
        self.source = fetcher.source;
        self.tile_low = fetcher.tile_low;
        self.tile_high = fetcher.tile_high;
        self.next_fetch_pixel = fetcher.next_fetch_pixel.wrapping_add(BG_TILE_WIDTH as u16);
    }

    fn advance_entry_delay(&mut self) -> bool {
        if self.entry_delay_remaining == 0 {
            return true;
        }

        self.entry_delay_remaining -= 1;
        false
    }

    fn is_ready_for_fifo_push(self) -> bool {
        self.pending && self.disposition == BgPushDisposition::Ready
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
    source: PpuBgFetcherSource,
    tile_low: u8,
    tile_high: u8,
}

impl BgFifoFillState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn queue_from_push(&mut self, push: BgPushState) {
        self.pending = true;
        self.source = push.source;
        self.tile_low = push.tile_low;
        self.tile_high = push.tile_high;
    }
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
            ObjFetchStartSource::QueuedBgFill => self.obj_fetch_can_start_from_queued_bg_fill,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjFetchStartSource {
    FifoBackedTransfer,
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
    fetch: ObjFetchState,
}

impl ObjPipelineState {
    fn reset(&mut self) {
        self.fifo.clear();
        self.fetched_sprite_slots.fill(false);
        self.pending_sprite_slots.clear();
        self.pending_match_x = None;
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
    latched_oam_word: Option<(u8, u8)>,
}

impl Mode2ScanState {
    fn reset_scanline(&mut self) {
        self.scanned_entries = 0;
        self.selected_sprite_count = 0;
        self.selected_sprites.fill(None);
    }

    fn reset(&mut self) {
        self.reset_scanline();
        self.latched_oam_word = None;
    }

    fn scanned_entries(&self) -> u8 {
        self.scanned_entries
    }

    fn increment_scanned_entries(&mut self) {
        self.scanned_entries += 1;
    }

    fn latch_oam_word(&mut self, first: u8, second: u8) {
        self.latched_oam_word = Some((first, second));
    }

    fn latched_oam_word(&self) -> Option<(u8, u8)> {
        self.latched_oam_word
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
            latched_oam_word: None,
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
    dma_oam_conflict_address: Option<u16>,
) -> (u8, u8) {
    let nominal_word_address = 0xFE00_u16 + sprite.oam_index as u16 * OAM_ENTRY_BYTES as u16 + 2;
    let word_address = dma_oam_conflict_address
        .filter(|address| (0xFE00..=0xFE9F).contains(address))
        .map(|address| address & !0x0001)
        .unwrap_or(nominal_word_address);
    let word_offset = word_address.saturating_sub(0xFE00) as usize;
    let tile_index = oam.read(word_offset).unwrap_or(sprite.tile_index);
    let attributes = oam.read(word_offset + 1).unwrap_or(sprite.attributes);

    (tile_index, attributes)
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
mod tests {
    use super::*;
    use crate::bus::BusMaster;
    use crate::scheduler::TCycle;

    const TEST_VRAM_BYTES: usize = 0x2000;

    fn sync_test_video_ownership(
        ppu: &Ppu,
        oam: &mut crate::bus::OamDomain,
        vram: &mut crate::bus::VramDomain,
        dma_oam_active: bool,
    ) {
        let bus_state = ppu.bus_state();
        let ppu_vram = bus_state.is_lcd_enabled() && bus_state.mode() == PpuAccessMode::Drawing;
        let ppu_oam = bus_state.is_lcd_enabled()
            && matches!(
                bus_state.mode(),
                PpuAccessMode::OamScan | PpuAccessMode::Drawing
            );

        oam.set_acquired(BusMaster::Ppu, ppu_oam);
        vram.set_acquired(BusMaster::Ppu, ppu_vram);
        oam.set_acquired(BusMaster::Dma, dma_oam_active);
        vram.set_acquired(BusMaster::Dma, false);
    }

    fn tick_ppu(ppu: &mut Ppu, t_cycle: u64, oam_bytes: &[u8]) -> CycleContext {
        tick_ppu_with_vram(ppu, t_cycle, oam_bytes, &[0; TEST_VRAM_BYTES])
    }

    fn tick_ppu_with_vram(
        ppu: &mut Ppu,
        t_cycle: u64,
        oam_bytes: &[u8],
        vram_bytes: &[u8],
    ) -> CycleContext {
        tick_ppu_with_vram_and_dma(ppu, t_cycle, oam_bytes, vram_bytes, false, None)
    }

    fn tick_ppu_with_vram_and_dma(
        ppu: &mut Ppu,
        t_cycle: u64,
        oam_bytes: &[u8],
        vram_bytes: &[u8],
        dma_oam_active: bool,
        dma_oam_conflict_address: Option<u16>,
    ) -> CycleContext {
        let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
        let mut oam = crate::bus::OamDomain::from_bytes(oam_bytes);
        let mut vram = crate::bus::VramDomain::from_bytes(vram_bytes);
        sync_test_video_ownership(ppu, &mut oam, &mut vram, dma_oam_active);
        ppu.tick_t_cycle(
            &mut context,
            OamBusView::new(BusMaster::Ppu, &mut oam),
            VramBusView::new(BusMaster::Ppu, &mut vram),
            dma_oam_active,
            dma_oam_conflict_address,
        );
        context
    }

    fn drain_ppu_interrupts(ppu: &mut Ppu) -> Vec<InterruptSource> {
        ppu.drain_pending_interrupt_requests()
    }

    fn write_oam_entry(oam_bytes: &mut [u8; 160], index: u8, y: u8, x: u8, tile_index: u8) {
        write_oam_entry_with_attributes(oam_bytes, index, y, x, tile_index, 0);
    }

    fn write_oam_entry_with_attributes(
        oam_bytes: &mut [u8; 160],
        index: u8,
        y: u8,
        x: u8,
        tile_index: u8,
        attributes: u8,
    ) {
        let entry_start = index as usize * OAM_ENTRY_BYTES;
        oam_bytes[entry_start] = y;
        oam_bytes[entry_start + 1] = x;
        oam_bytes[entry_start + 2] = tile_index;
        oam_bytes[entry_start + 3] = attributes;
    }

    fn write_oam_corruption_row(oam_bytes: &mut [u8; 160], row: u8, words: [u16; 4]) {
        for (word_index, value) in words.into_iter().enumerate() {
            write_oam_word(oam_bytes, row, word_index, value);
        }
    }

    fn write_bg_tile_row(
        vram_bytes: &mut [u8; TEST_VRAM_BYTES],
        tile_index: u8,
        row: u8,
        low: u8,
        high: u8,
    ) {
        let tile_address =
            tile_index as usize * TILE_BYTES as usize + row as usize * TILE_ROW_BYTES as usize;
        vram_bytes[tile_address] = low;
        vram_bytes[tile_address + 1] = high;
    }

    fn write_bg_tilemap_entry(
        vram_bytes: &mut [u8; TEST_VRAM_BYTES],
        x: u8,
        y: u8,
        tile_index: u8,
    ) {
        let tile_map_address = 0x1800 + y as usize * BG_TILE_MAP_WIDTH as usize + x as usize;
        vram_bytes[tile_map_address] = tile_index;
    }

    fn write_window_tilemap_entry(
        vram_bytes: &mut [u8; TEST_VRAM_BYTES],
        x: u8,
        y: u8,
        tile_index: u8,
    ) {
        let tile_map_address = 0x1C00 + y as usize * BG_TILE_MAP_WIDTH as usize + x as usize;
        vram_bytes[tile_map_address] = tile_index;
    }

    fn tick_until_hblank(
        ppu: &mut Ppu,
        mut t_cycle: u64,
        oam_bytes: &[u8],
        vram_bytes: &[u8],
    ) -> u64 {
        let start_t_cycle = t_cycle;
        while ppu.snapshot().mode != PpuAccessMode::HBlank {
            tick_ppu_with_vram(ppu, t_cycle, oam_bytes, vram_bytes);
            t_cycle += 1;
            assert!(t_cycle - start_t_cycle < 2 * DOTS_PER_SCANLINE as u64);
        }

        t_cycle
    }

    fn tick_until_line_start(
        ppu: &mut Ppu,
        mut t_cycle: u64,
        oam_bytes: &[u8],
        vram_bytes: &[u8],
        target_ly: u8,
    ) -> u64 {
        while !(ppu.snapshot().ly == target_ly && ppu.snapshot().line_dot == 0) {
            tick_ppu_with_vram(ppu, t_cycle, oam_bytes, vram_bytes);
            t_cycle += 1;
            assert!(t_cycle < 20 * DOTS_PER_SCANLINE as u64);
        }

        t_cycle
    }

    fn tick_until_next_frame_start(
        ppu: &mut Ppu,
        mut t_cycle: u64,
        oam_bytes: &[u8],
        vram_bytes: &[u8],
    ) -> u64 {
        while !(t_cycle > 0 && ppu.snapshot().ly == 0 && ppu.snapshot().line_dot == 0) {
            tick_ppu_with_vram(ppu, t_cycle, oam_bytes, vram_bytes);
            t_cycle += 1;
            assert!(t_cycle < 2 * TOTAL_SCANLINES as u64 * DOTS_PER_SCANLINE as u64);
        }

        t_cycle
    }

    #[test]
    fn startup_state_recreates_the_documented_post_boot_lcd_snapshot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x85,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        assert_eq!(ppu.read_register(0xFF40), 0x91);
        assert_eq!(ppu.read_register(0xFF41), 0x85);
        assert_eq!(ppu.read_register(0xFF42), 0x00);
        assert_eq!(ppu.read_register(0xFF43), 0x00);
        assert_eq!(ppu.read_register(0xFF44), 0x00);
        assert_eq!(ppu.read_register(0xFF45), 0x00);
        assert_eq!(ppu.read_register(0xFF47), 0xFC);
        assert_eq!(ppu.read_register(0xFF4A), 0x00);
        assert_eq!(ppu.read_register(0xFF4B), 0x00);
        assert_eq!(ppu.snapshot().lcd_state, PpuLcdState::Enabled);
        assert_eq!(
            ppu.snapshot().visible_output,
            PpuVisibleOutputState::Driving
        );
        assert_eq!(ppu.snapshot().line_dot, 0);
        assert_eq!(
            ppu.bus_state(),
            PpuBusState::lcd_enabled(PpuAccessMode::VBlank)
        );
    }

    #[test]
    fn stat_keeps_live_mode_and_coincidence_bits_outside_the_writable_mask() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x81,
            scy: 0x00,
            scx: 0x00,
            ly: 0x12,
            lyc: 0x12,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        ppu.write_register(0xFF41, 0xFF);

        assert_eq!(ppu.read_register(0xFF41), 0xFD);
    }

    #[test]
    fn lyc_writes_reevaluate_coincidence_immediately_and_can_raise_lcd_stat() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x42,
            scy: 0x00,
            scx: 0x00,
            ly: 0x12,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        assert!(!ppu.snapshot().lyc_coincidence);
        assert!(!ppu.snapshot().stat_irq_line);
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());

        ppu.write_register(0xFF45, 0x12);

        assert_eq!(ppu.read_register(0xFF41), 0xC6);
        assert!(ppu.snapshot().lyc_coincidence);
        assert!(ppu.snapshot().stat_irq_line);
        assert_eq!(
            drain_ppu_interrupts(&mut ppu),
            vec![InterruptSource::LcdStat]
        );
    }

    #[test]
    fn stat_line_blocks_new_requests_while_an_enabled_source_keeps_it_high() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x62,
            scy: 0x00,
            scx: 0x00,
            ly: 0x21,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        assert!(ppu.snapshot().stat_irq_line);
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());

        ppu.write_register(0xFF45, 0x21);

        assert!(ppu.snapshot().lyc_coincidence);
        assert!(ppu.snapshot().stat_irq_line);
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());
    }

    #[test]
    fn dmg_mode2_enable_requests_lcd_stat_at_vblank_entry_only() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
            scy: 0x00,
            scx: 0x00,
            ly: 143,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu.line_dot = DOTS_PER_SCANLINE - 1;
        ppu.refresh_stat_irq_line(false);
        assert!(!ppu.snapshot().stat_irq_line);
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());

        tick_ppu(&mut ppu, 0, &oam_bytes);

        assert_eq!(ppu.snapshot().ly, 144);
        assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);
        assert!(ppu.snapshot().stat_irq_line);
        assert_eq!(
            drain_ppu_interrupts(&mut ppu),
            vec![InterruptSource::VBlank, InterruptSource::LcdStat]
        );

        tick_ppu(&mut ppu, 1, &oam_bytes);

        assert_eq!(ppu.snapshot().ly, 144);
        assert_eq!(ppu.snapshot().line_dot, 1);
        assert!(!ppu.snapshot().stat_irq_line);
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());
    }

    #[test]
    fn mode2_enable_alone_does_not_hold_stat_high_past_vblank_entry() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
            scy: 0x00,
            scx: 0x00,
            ly: 144,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu.line_dot = 8;
        ppu.refresh_stat_irq_line(false);
        assert!(!ppu.snapshot().stat_irq_line);
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());
    }

    #[test]
    fn stat_write_quirk_requests_in_mode2_and_coincidence_but_not_plain_mode3() {
        let mut mode2 = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];

        mode2.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x20,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        assert!(drain_ppu_interrupts(&mut mode2).is_empty());

        mode2.write_register(0xFF41, 0x00);

        assert!(mode2.snapshot().stat_irq_line);
        assert_eq!(
            drain_ppu_interrupts(&mut mode2),
            vec![InterruptSource::LcdStat]
        );

        let mut mode3 = Ppu::new(ConsoleModel::Dmg);
        mode3.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x20,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        for t_cycle in 0..80 {
            tick_ppu(&mut mode3, t_cycle, &oam_bytes);
        }
        assert_eq!(mode3.snapshot().mode, PpuAccessMode::Drawing);
        assert!(drain_ppu_interrupts(&mut mode3).is_empty());

        mode3.write_register(0xFF41, 0x00);

        assert!(!mode3.snapshot().stat_irq_line);
        assert!(drain_ppu_interrupts(&mut mode3).is_empty());

        let mut coincidence = Ppu::new(ConsoleModel::Dmg);
        coincidence.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        for t_cycle in 0..80 {
            tick_ppu(&mut coincidence, t_cycle, &oam_bytes);
        }
        assert_eq!(coincidence.snapshot().mode, PpuAccessMode::Drawing);
        assert!(drain_ppu_interrupts(&mut coincidence).is_empty());

        coincidence.write_register(0xFF41, 0x00);

        assert!(coincidence.snapshot().stat_irq_line);
        assert_eq!(
            drain_ppu_interrupts(&mut coincidence),
            vec![InterruptSource::LcdStat]
        );
    }

    #[test]
    fn lyc_coincidence_tracks_vblank_lines_and_the_153_to_0_wrap() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 143,
            lyc: 144,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let mut t_cycle =
            tick_until_line_start(&mut ppu, 0, &oam_bytes, &[0; TEST_VRAM_BYTES], 144);
        assert_eq!(ppu.snapshot().ly, 144);
        assert!(ppu.snapshot().lyc_coincidence);

        ppu.write_register(0xFF45, 153);
        assert!(!ppu.snapshot().lyc_coincidence);

        t_cycle = tick_until_line_start(&mut ppu, t_cycle, &oam_bytes, &[0; TEST_VRAM_BYTES], 153);
        assert_eq!(ppu.snapshot().ly, 153);
        assert!(ppu.snapshot().lyc_coincidence);

        ppu.write_register(0xFF45, 0);
        assert!(!ppu.snapshot().lyc_coincidence);

        let _ = tick_until_line_start(&mut ppu, t_cycle, &oam_bytes, &[0; TEST_VRAM_BYTES], 0);
        assert_eq!(ppu.snapshot().ly, 0);
        assert!(ppu.snapshot().lyc_coincidence);
    }

    #[test]
    fn ly_is_read_only_and_obj_palettes_keep_an_explicit_uninitialized_policy() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x85,
            scy: 0x00,
            scx: 0x00,
            ly: 0x22,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        ppu.write_register(0xFF44, 0x99);

        assert_eq!(ppu.read_register(0xFF44), 0x22);
        assert_eq!(ppu.read_register(0xFF48), 0xFF);
        assert_eq!(ppu.read_register(0xFF49), 0xFF);
    }

    #[test]
    fn skip_boot_mode_latch_preserves_the_published_stat_mode_until_the_first_dot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x85,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);
        assert_eq!(ppu.snapshot().line_dot, 0);
        assert_eq!(ppu.snapshot().mode_dot, 0);

        tick_ppu(&mut ppu, 0, &[0; 160]);

        assert_eq!(ppu.snapshot().ly, 0);
        assert_eq!(ppu.snapshot().line_dot, 1);
        assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
        assert_eq!(ppu.snapshot().mode_dot, 1);
    }

    #[test]
    fn visible_mode3_registers_lag_enabled_writes_until_the_next_t_cycle() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..80 {
            tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        }

        let before = ppu.snapshot();
        assert_eq!(before.mode, PpuAccessMode::Drawing);
        assert_eq!(before.visible_lcdc, 0x80);
        assert_eq!(before.visible_scy, 0x00);
        assert_eq!(before.visible_scx, 0x00);
        assert_eq!(before.visible_bgp, 0xFC);
        assert_eq!(before.visible_wy, 0x00);
        assert_eq!(before.visible_wx, 0x00);

        ppu.write_register(0xFF40, 0x91);
        ppu.write_register(0xFF42, 0x12);
        ppu.write_register(0xFF43, 0x34);
        ppu.write_register(0xFF47, 0x1B);
        ppu.write_register(0xFF4A, 0x56);
        ppu.write_register(0xFF4B, 0x78);

        let pending = ppu.snapshot();
        assert_eq!(pending.lcdc, 0x91);
        assert_eq!(pending.scy, 0x12);
        assert_eq!(pending.scx, 0x34);
        assert_eq!(pending.bgp, 0x1B);
        assert_eq!(pending.wy, 0x56);
        assert_eq!(pending.wx, 0x78);
        assert_eq!(pending.visible_lcdc, 0x80);
        assert_eq!(pending.visible_scy, 0x00);
        assert_eq!(pending.visible_scx, 0x00);
        assert_eq!(pending.visible_bgp, 0xFC);
        assert_eq!(pending.visible_wy, 0x00);
        assert_eq!(pending.visible_wx, 0x00);

        tick_ppu(&mut ppu, 80, &oam_bytes);

        let after = ppu.snapshot();
        assert_eq!(after.visible_lcdc, 0x91);
        assert_eq!(after.visible_scy, 0x12);
        assert_eq!(after.visible_scx, 0x34);
        assert_eq!(after.visible_bgp, 0x1B);
        assert_eq!(after.visible_wy, 0x56);
        assert_eq!(after.visible_wx, 0x78);
    }

    #[test]
    fn tick_advances_the_raster_through_the_baseline_visible_line_modes() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..79 {
            tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        }

        assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
        assert_eq!(ppu.snapshot().line_dot, 79);
        assert_eq!(ppu.snapshot().mode_dot, 79);

        tick_ppu(&mut ppu, 79, &oam_bytes);
        assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
        assert_eq!(ppu.snapshot().line_dot, 80);
        assert_eq!(ppu.snapshot().mode_dot, 0);

        for t_cycle in 80..251 {
            tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        }

        assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
        assert_eq!(ppu.snapshot().line_dot, 251);
        assert_eq!(ppu.snapshot().mode_dot, 171);

        tick_ppu(&mut ppu, 251, &oam_bytes);
        assert_eq!(ppu.snapshot().mode, PpuAccessMode::HBlank);
        assert_eq!(ppu.snapshot().line_dot, 252);
        assert_eq!(ppu.snapshot().mode_dot, 0);

        for t_cycle in 252..=455 {
            tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        }

        assert_eq!(ppu.snapshot().ly, 1);
        assert_eq!(ppu.snapshot().line_dot, 0);
        assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
        assert_eq!(ppu.snapshot().mode_dot, 0);
    }

    #[test]
    fn lcd_disabled_state_freezes_the_raster_and_forces_blank_output() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x44,
            lyc: 0x12,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..32 {
            tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        }

        ppu.write_register(0xFF40, 0x00);

        let snapshot = ppu.snapshot();
        assert_eq!(snapshot.lcd_state, PpuLcdState::Disabled);
        assert_eq!(snapshot.visible_output, PpuVisibleOutputState::ForcedBlank);
        assert!(!snapshot.blank_frame_active);
        assert_eq!(snapshot.ly, 0x00);
        assert_eq!(snapshot.line_dot, 0);
        assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
        assert_eq!(ppu.bus_state(), PpuBusState::lcd_disabled());
    }

    #[test]
    fn lcd_disable_resets_the_live_pipeline_and_reenable_starts_with_mode0_readback() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 16, 8, 0);
        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x00, 0xFF);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x82,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..100 {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        }

        let drawing = ppu.snapshot();
        assert_eq!(drawing.mode, PpuAccessMode::Drawing);
        assert!(!drawing.bg_fifo_pixels.is_empty());

        ppu.write_register(0xFF40, 0x00);

        let disabled = ppu.snapshot();
        assert_eq!(disabled.lcd_state, PpuLcdState::Disabled);
        assert_eq!(disabled.visible_output, PpuVisibleOutputState::ForcedBlank);
        assert!(!disabled.blank_frame_active);
        assert_eq!(disabled.ly, 0);
        assert_eq!(disabled.line_dot, 0);
        assert!(disabled.bg_fifo_pixels.is_empty());
        assert!(disabled.obj_fifo_pixels.is_empty());
        assert!(disabled.selected_sprites.is_empty());
        assert_eq!(disabled.mode2_scanned_entries, 0);
        assert_eq!(disabled.window_line_counter, 0);

        ppu.write_register(0xFF40, 0x82);

        let reenabled = ppu.snapshot();
        assert_eq!(reenabled.lcd_state, PpuLcdState::Enabled);
        assert_eq!(reenabled.mode, PpuAccessMode::HBlank);
        assert_eq!(reenabled.visible_output, PpuVisibleOutputState::ForcedBlank);
        assert!(reenabled.blank_frame_active);
        assert_eq!(reenabled.ly, 0);
        assert_eq!(reenabled.line_dot, LCD_REENABLE_INITIAL_LINE_DOT);
        assert_eq!(reenabled.mode_dot, 0);
        assert!(reenabled.bg_fifo_pixels.is_empty());
        assert!(reenabled.obj_fifo_pixels.is_empty());
        assert!(reenabled.selected_sprites.is_empty());
        assert_eq!(reenabled.mode2_scanned_entries, 0);
    }

    #[test]
    fn lcd_reenable_startup_window_keeps_mode2_idle_until_the_ordinary_raster_resumes() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x00,
            stat: 0x80,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        ppu.write_register(0xFF40, 0x82);

        let restart = ppu.snapshot();
        assert_eq!(restart.mode, PpuAccessMode::HBlank);
        assert_eq!(restart.line_dot, LCD_REENABLE_INITIAL_LINE_DOT);
        assert_eq!(restart.mode_dot, 0);
        assert_eq!(restart.mode2_scanned_entries, 0);

        for t_cycle in 0..15 {
            tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        }

        let startup_window_end = ppu.snapshot();
        assert_eq!(startup_window_end.line_dot, 19);
        assert_eq!(startup_window_end.mode, PpuAccessMode::HBlank);
        assert_eq!(startup_window_end.mode_dot, 15);
        assert_eq!(startup_window_end.mode2_scanned_entries, 0);

        tick_ppu(&mut ppu, 15, &oam_bytes);

        let first_mode2_dot = ppu.snapshot();
        assert_eq!(first_mode2_dot.line_dot, 20);
        assert_eq!(first_mode2_dot.mode, PpuAccessMode::OamScan);
        assert_eq!(first_mode2_dot.mode_dot, 20);
        assert_eq!(first_mode2_dot.mode2_scanned_entries, 1);
    }

    #[test]
    fn lcd_off_retains_the_lyc_bit_and_ignores_lyc_writes_until_lcd_restarts() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x40,
            scy: 0x00,
            scx: 0x00,
            ly: 0x90,
            lyc: 0x90,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        ppu.write_register(0xFF40, 0x00);
        assert_eq!(ppu.read_register(0xFF41), 0xC4);
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());

        ppu.write_register(0xFF45, 0x01);
        assert_eq!(ppu.read_register(0xFF41), 0xC4);
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());

        ppu.write_register(0xFF40, 0x80);
        assert_eq!(ppu.read_register(0xFF41), 0xC0);
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());
    }

    #[test]
    fn lcd_reenable_requests_lcd_stat_only_when_the_retained_lyc_result_rises() {
        let mut unchanged_true = Ppu::new(ConsoleModel::Dmg);
        unchanged_true.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x40,
            scy: 0x00,
            scx: 0x00,
            ly: 0x90,
            lyc: 0x90,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        unchanged_true.write_register(0xFF40, 0x00);
        unchanged_true.write_register(0xFF45, 0x00);
        drain_ppu_interrupts(&mut unchanged_true);

        unchanged_true.write_register(0xFF40, 0x80);

        assert_eq!(unchanged_true.read_register(0xFF41), 0xC4);
        assert!(drain_ppu_interrupts(&mut unchanged_true).is_empty());

        let mut rising = Ppu::new(ConsoleModel::Dmg);
        rising.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x40,
            scy: 0x00,
            scx: 0x00,
            ly: 0x90,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        rising.write_register(0xFF40, 0x00);
        assert_eq!(rising.read_register(0xFF41), 0xC0);
        drain_ppu_interrupts(&mut rising);

        rising.write_register(0xFF40, 0x80);

        assert_eq!(rising.read_register(0xFF41), 0xC4);
        assert_eq!(
            drain_ppu_interrupts(&mut rising),
            vec![InterruptSource::LcdStat]
        );
    }

    #[test]
    fn first_frame_after_lcd_reenable_stays_visibly_blank_while_the_raster_runs() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x00, 0xFF);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x00,
            stat: 0x80,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        ppu.write_register(0xFF40, 0x91);

        let mut t_cycle = 0;
        while ppu.snapshot().mode == PpuAccessMode::HBlank {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
            t_cycle += 1;
            assert!(t_cycle < DOTS_PER_SCANLINE as u64);
        }

        t_cycle = tick_until_hblank(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        let first_blank_line = ppu.snapshot();
        assert_eq!(
            first_blank_line.visible_output,
            PpuVisibleOutputState::ForcedBlank
        );
        assert!(first_blank_line.blank_frame_active);
        assert_eq!(first_blank_line.visible_pixels_output, 160);
        assert_eq!(&first_blank_line.current_scanline_pixels[..8], &[0; 8]);

        t_cycle = tick_until_next_frame_start(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        let second_frame_start = ppu.snapshot();
        assert_eq!(second_frame_start.ly, 0);
        assert_eq!(second_frame_start.line_dot, 0);
        assert_eq!(
            second_frame_start.visible_output,
            PpuVisibleOutputState::Driving
        );
        assert!(!second_frame_start.blank_frame_active);

        let _ = tick_until_hblank(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        let visible_line = ppu.snapshot();
        assert_eq!(visible_line.visible_output, PpuVisibleOutputState::Driving);
        assert_eq!(&visible_line.current_scanline_pixels[..8], &[2; 8]);
    }

    #[test]
    fn mode2_scans_oam_in_order_and_caps_the_selected_list_at_ten_entries() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];

        for index in 0..12 {
            let x = match index {
                0 => 0,
                1 => 168,
                _ => 8 + index,
            };
            write_oam_entry(&mut oam_bytes, index, 16, x, 0x20 + index);
        }
        write_oam_entry(&mut oam_bytes, 20, 8, 32, 0x99);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..80 {
            tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        }

        let snapshot = ppu.snapshot();
        assert_eq!(snapshot.mode2_scanned_entries, 40);
        assert_eq!(snapshot.selected_sprites.len(), 10);
        assert_eq!(
            snapshot
                .selected_sprites
                .iter()
                .map(|sprite| sprite.oam_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
        assert_eq!(snapshot.selected_sprites[0].x, 0);
        assert_eq!(snapshot.selected_sprites[1].x, 168);
    }

    #[test]
    fn dmg_mode2_oam_dma_reuses_the_last_latched_oam_word_for_selection() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];

        write_oam_entry(&mut oam_bytes, 0, 16, 24, 0x20);
        write_oam_entry(&mut oam_bytes, 1, 0, 0, 0x21);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        tick_ppu(&mut ppu, 0, &oam_bytes);
        tick_ppu(&mut ppu, 1, &oam_bytes);
        tick_ppu_with_vram_and_dma(&mut ppu, 2, &oam_bytes, &[0; TEST_VRAM_BYTES], true, None);
        tick_ppu_with_vram_and_dma(&mut ppu, 3, &oam_bytes, &[0; TEST_VRAM_BYTES], true, None);

        let snapshot = ppu.snapshot();
        assert_eq!(snapshot.mode2_scanned_entries, 2);
        assert_eq!(snapshot.selected_sprites.len(), 2);
        assert_eq!(snapshot.selected_sprites[1].oam_index, 1);
        assert_eq!(snapshot.selected_sprites[1].y, 16);
        assert_eq!(snapshot.selected_sprites[1].x, 24);
    }

    #[test]
    fn mode2_scanline_reset_preserves_the_latched_oam_word_for_dma_blocked_reads() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];

        write_oam_entry(&mut oam_bytes, 0, 0, 0, 0x20);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu.mode2_scan_state.latch_oam_word(16, 79);
        ppu.mode2_scan_state.reset_scanline();

        tick_ppu_with_vram_and_dma(&mut ppu, 0, &oam_bytes, &[0; TEST_VRAM_BYTES], true, None);
        tick_ppu_with_vram_and_dma(&mut ppu, 1, &oam_bytes, &[0; TEST_VRAM_BYTES], true, None);

        let snapshot = ppu.snapshot();
        assert_eq!(snapshot.mode2_scanned_entries, 1);
        assert_eq!(snapshot.selected_sprites.len(), 1);
        assert_eq!(snapshot.selected_sprites[0].oam_index, 0);
        assert_eq!(snapshot.selected_sprites[0].y, 16);
        assert_eq!(snapshot.selected_sprites[0].x, 79);
    }

    #[test]
    fn mode2_uses_the_live_lcdc2_size_when_each_oam_entry_is_scanned() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];

        write_oam_entry(&mut oam_bytes, 0, 0, 24, 0x10);
        write_oam_entry(&mut oam_bytes, 1, 1, 32, 0x11);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        tick_ppu(&mut ppu, 0, &oam_bytes);
        tick_ppu(&mut ppu, 1, &oam_bytes);
        assert!(ppu.snapshot().selected_sprites.is_empty());

        ppu.write_register(0xFF40, 0x84);

        tick_ppu(&mut ppu, 2, &oam_bytes);
        tick_ppu(&mut ppu, 3, &oam_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(snapshot.mode2_scanned_entries, 2);
        assert_eq!(snapshot.selected_sprites.len(), 1);
        assert_eq!(snapshot.selected_sprites[0].oam_index, 1);
        assert_eq!(snapshot.selected_sprites[0].y, 1);
    }

    #[test]
    fn mode3_bg_fetcher_fills_the_fifo_before_visible_pixels_begin() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..80 {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        }

        let drawing_start = ppu.snapshot();
        assert_eq!(drawing_start.mode, PpuAccessMode::Drawing);
        assert_eq!(drawing_start.line_dot, 80);
        assert_eq!(drawing_start.mode_dot, 0);
        assert_eq!(drawing_start.mode0_start_dot, 252);
        assert_eq!(drawing_start.bg_fetcher_stage, PpuBgFetcherStage::TileIndex);
        assert_eq!(drawing_start.bg_fetcher_stage_dot, 1);
        assert!(drawing_start.bg_fifo_pixels.is_empty());
        assert_eq!(drawing_start.visible_pixels_output, 0);

        for t_cycle in 80..87 {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        }

        let after_first_push = ppu.snapshot();
        assert_eq!(after_first_push.line_dot, 87);
        assert_eq!(
            after_first_push.bg_fetcher_stage,
            PpuBgFetcherStage::TileIndex
        );
        assert_eq!(after_first_push.bg_fetcher_stage_dot, 0);
        assert!(after_first_push.bg_fifo_pixels.is_empty());
        assert!(!after_first_push.bg_push_pending);
        assert!(after_first_push.bg_fill_pending);
        assert_eq!(after_first_push.visible_pixels_output, 0);

        for t_cycle in 87..92 {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        }

        let first_visible = ppu.snapshot();
        assert_eq!(first_visible.line_dot, 92);
        assert_eq!(first_visible.mode_dot, 12);
        assert_eq!(first_visible.visible_pixels_output, 1);
        assert_eq!(first_visible.current_scanline_pixels[0], 0);
    }

    #[test]
    fn bg_push_waits_for_fifo_space_without_losing_the_fetched_tile() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);

        ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
        ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
        ppu.bg_pipeline_state.push.pending = true;
        ppu.bg_pipeline_state.push.source = PpuBgFetcherSource::Background;
        ppu.bg_pipeline_state.push.tile_low = 0x55;
        ppu.bg_pipeline_state.push.tile_high = 0x33;
        ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
        ppu.bg_pipeline_state.fifo = (0..=8).collect();

        let result = ppu.advance_bg_push();

        assert_eq!(result, BgPushDotResult::WaitingForEmptyFifo);
        assert!(ppu.bg_pipeline_state.push.pending);
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
        assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 0);
        assert_eq!(ppu.bg_pipeline_state.fifo.len(), 9);

        for _ in 0..9 {
            let _ = ppu.bg_pipeline_state.fifo.pop_front();
        }
        let result = ppu.advance_bg_push();

        assert_eq!(result, BgPushDotResult::QueuedFill);
        assert!(!ppu.bg_pipeline_state.push.pending);
        assert!(ppu.bg_pipeline_state.fill.pending);
        assert_eq!(
            ppu.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileIndex
        );
        assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 8);
        assert_eq!(ppu.bg_pipeline_state.fifo.len(), 0);

        ppu.flush_pending_bg_fifo_fill();

        assert!(!ppu.bg_pipeline_state.fill.pending);
        assert_eq!(ppu.bg_pipeline_state.fifo.len(), 8);
        assert_eq!(
            ppu.bg_pipeline_state
                .fifo
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 0, 1, 2, 3]
        );
    }

    #[test]
    fn latching_object_hits_queues_all_matching_sprite_slots_once() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.bg_pipeline_state.current_transfer_x = 8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;

        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: 0,
            y: 16,
            x: 8,
            tile_index: 0,
            attributes: 0,
        });
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: 1,
            y: 16,
            x: 8,
            tile_index: 1,
            attributes: 0,
        });

        ppu.latch_object_fetch_hits();
        ppu.latch_object_fetch_hits();

        assert_eq!(
            ppu.obj_pipeline_state
                .pending_sprite_slots
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn bg_push_can_handoff_to_a_latched_object_fetch_without_losing_the_tile() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.current_transfer_x = 8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
        ppu.bg_pipeline_state.push.pending = true;
        ppu.bg_pipeline_state.push.source = PpuBgFetcherSource::Background;
        ppu.bg_pipeline_state.push.tile_low = 0x55;
        ppu.bg_pipeline_state.push.tile_high = 0x33;
        ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
        ppu.bg_pipeline_state.fifo.push_back(0);

        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: 0,
            y: 16,
            x: 8,
            tile_index: 0,
            attributes: 0,
        });
        ppu.obj_pipeline_state
            .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

        assert_eq!(
            ppu.advance_bg_push(),
            BgPushDotResult::HandedOffToObjectFetch
        );
        assert!(ppu.bg_pipeline_state.push.pending);
        assert_eq!(
            ppu.bg_pipeline_state.push.disposition,
            BgPushDisposition::InterruptedByObjectFetch
        );
        assert_eq!(
            ppu.obj_pipeline_state.fetch.stage,
            PpuObjFetcherStage::Startup
        );
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
        assert_eq!(ppu.bg_pipeline_state.fifo.len(), 1);
        assert!(!ppu.bg_pipeline_state.fill.pending);
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    }

    #[test]
    fn bg_push_with_an_empty_fifo_can_queue_fill_and_start_object_fetch_on_the_same_dot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.bg_pipeline_state.current_transfer_x = 8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
        ppu.bg_pipeline_state.push.pending = true;
        ppu.bg_pipeline_state.push.source = PpuBgFetcherSource::Background;
        ppu.bg_pipeline_state.push.tile_low = 0x55;
        ppu.bg_pipeline_state.push.tile_high = 0x33;
        ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: 0,
            y: 16,
            x: 8,
            tile_index: 0,
            attributes: 0,
        });
        ppu.obj_pipeline_state
            .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

        assert_eq!(
            ppu.advance_bg_push(),
            BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
        );
        assert!(!ppu.bg_pipeline_state.push.pending);
        assert!(ppu.bg_pipeline_state.fill.pending);
        assert_eq!(
            ppu.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileIndex
        );
        assert_eq!(
            ppu.obj_pipeline_state.fetch.stage,
            PpuObjFetcherStage::Startup
        );
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
        assert!(ppu.bg_pipeline_state.fifo.is_empty());
    }

    #[test]
    fn bg_push_stage_waits_one_dot_on_entry_then_retries_every_dot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);

        ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
        ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
        ppu.bg_pipeline_state.push.pending = true;
        ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
        ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
        ppu.bg_pipeline_state.push.source = PpuBgFetcherSource::Background;
        ppu.bg_pipeline_state.push.tile_low = 0x55;
        ppu.bg_pipeline_state.push.tile_high = 0x33;
        ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
        ppu.bg_pipeline_state.fifo = (0..=8).collect();

        assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::EntryDelay);
        assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
        assert!(ppu.bg_pipeline_state.push.pending);
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
        assert_eq!(ppu.bg_pipeline_state.fifo.len(), 9);

        assert_eq!(
            ppu.advance_bg_push_stage(),
            BgPushDotResult::WaitingForEmptyFifo
        );
        assert!(ppu.bg_pipeline_state.push.pending);
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
        assert_eq!(ppu.bg_pipeline_state.fifo.len(), 9);

        for _ in 0..9 {
            let _ = ppu.bg_pipeline_state.fifo.pop_front();
        }
        assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::QueuedFill);
        assert!(!ppu.bg_pipeline_state.push.pending);
        assert!(ppu.bg_pipeline_state.fill.pending);
        assert_eq!(
            ppu.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileIndex
        );
        assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 8);
        assert_eq!(ppu.bg_pipeline_state.fifo.len(), 0);

        ppu.flush_pending_bg_fifo_fill();

        assert!(!ppu.bg_pipeline_state.fill.pending);
        assert_eq!(ppu.bg_pipeline_state.fifo.len(), 8);
    }

    #[test]
    fn bg_push_queues_fifo_fill_before_the_fill_phase_materializes_pixels() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);

        ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
        ppu.bg_pipeline_state.push.pending = true;
        ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
        ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
        ppu.bg_pipeline_state.push.source = PpuBgFetcherSource::Background;
        ppu.bg_pipeline_state.push.tile_low = 0x55;
        ppu.bg_pipeline_state.push.tile_high = 0x33;
        ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

        assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::QueuedFill);
        assert!(!ppu.bg_pipeline_state.push.pending);
        assert!(ppu.bg_pipeline_state.fill.pending);
        assert!(ppu.bg_pipeline_state.fifo.is_empty());

        ppu.flush_pending_bg_fifo_fill();

        assert!(!ppu.bg_pipeline_state.fill.pending);
        assert_eq!(
            ppu.bg_pipeline_state
                .fifo
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 0, 1, 2, 3]
        );
    }

    #[test]
    fn bg_push_stage_reports_not_ready_when_no_cached_slice_is_pending() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);

        assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::NotReady);
    }

    #[test]
    fn current_dot_arbitration_distinguishes_fifo_backed_and_queued_fill_obj_start() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.current_transfer_x = 8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: 0,
            y: 16,
            x: 8,
            tile_index: 0,
            attributes: 0,
        });
        ppu.obj_pipeline_state
            .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

        let empty_fifo = ppu.current_dot_arbitration();
        assert!(!empty_fifo.can_serve_bg_transfer());
        assert!(!empty_fifo.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
        assert!(empty_fifo.can_start_obj_fetch(ObjFetchStartSource::QueuedBgFill));

        ppu.bg_pipeline_state.fifo.push_back(0);

        let fifo_backed = ppu.current_dot_arbitration();
        assert!(!fifo_backed.can_serve_bg_transfer());
        assert!(fifo_backed.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
        assert!(fifo_backed.can_start_obj_fetch(ObjFetchStartSource::QueuedBgFill));
    }

    #[test]
    fn transfer_service_plan_distinguishes_abstract_hidden_and_fifo_backed_visible_paths() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 7;
        ppu.bg_pipeline_state.fifo.push_back(0);

        assert_eq!(
            ppu.current_transfer_service_plan(),
            Some(Mode3TransferServicePlan {
                result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
                action: Mode3TransferServiceAction::AdvanceHiddenX,
                backing: Mode3TransferBacking::HiddenStartupFifoGated,
            })
        );

        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;

        assert_eq!(
            ppu.current_transfer_service_plan(),
            Some(Mode3TransferServicePlan {
                result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
                action: Mode3TransferServiceAction::AdvanceHiddenX,
                backing: Mode3TransferBacking::FifoBacked,
            })
        );

        ppu.bg_pipeline_state.current_transfer_x = 8;

        assert_eq!(
            ppu.current_transfer_service_plan(),
            Some(Mode3TransferServicePlan {
                result_kind: Mode3TransferDotKind::ServedVisiblePixel,
                action: Mode3TransferServiceAction::EmitVisiblePixel,
                backing: Mode3TransferBacking::FifoBacked,
            })
        );
    }

    #[test]
    fn fifo_backed_obj_start_requires_a_fifo_backed_transfer_dot_not_just_a_nonempty_fifo() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 7;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
        ppu.bg_pipeline_state.fifo.push_back(0);
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: 0,
            y: 16,
            x: 15,
            tile_index: 0,
            attributes: 0,
        });
        ppu.obj_pipeline_state
            .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

        let arbitration = ppu.current_dot_arbitration();
        assert!(!arbitration.can_serve_bg_transfer());
        assert!(!arbitration.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
        assert!(arbitration.can_start_obj_fetch(ObjFetchStartSource::QueuedBgFill));
        assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    }

    #[test]
    fn abstract_startup_service_kind_tracks_served_progress_not_raw_mode3_dot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.line_dot = MODE2_DOTS + MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT;
        ppu.bg_pipeline_state.current_transfer_x = 2;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining =
            MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS - 2;

        assert_eq!(
            ppu.current_transfer_service_plan(),
            Some(Mode3TransferServicePlan {
                result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
                action: Mode3TransferServiceAction::AdvanceHiddenX,
                backing: Mode3TransferBacking::Abstract,
            })
        );

        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;

        assert_eq!(
            ppu.current_transfer_service_plan(),
            Some(Mode3TransferServicePlan {
                result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
                action: Mode3TransferServiceAction::AdvanceHiddenX,
                backing: Mode3TransferBacking::HiddenStartupFifoGated,
            })
        );
    }

    #[test]
    fn obj_hit_ownership_tracks_served_startup_progress_not_raw_mode3_dot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.line_dot = MODE2_DOTS + MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT;
        ppu.bg_pipeline_state.current_transfer_x = 2;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining =
            MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS - 2;

        assert_eq!(
            ppu.current_obj_hit_ownership().phase,
            ObjHitPhase::PreVisible
        );

        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;

        assert_eq!(ppu.current_obj_hit_ownership().phase, ObjHitPhase::Hidden);
    }

    #[test]
    fn pending_obj_hit_blocks_output_phase_and_stretches_mode3() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.bg_pipeline_state.current_transfer_x = 20;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.visible_pixels_output = 12;
        ppu.bg_pipeline_state.fifo.push_back(3);
        ppu.obj_pipeline_state
            .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

        let result = ppu.advance_mode3_output_phase();

        assert_eq!(result.kind, Mode3TransferDotKind::NotServed);
        assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 12);
        assert_eq!(
            ppu.bg_pipeline_state
                .fifo
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
    }

    #[test]
    fn pending_obj_hit_stalls_pre_visible_match_x_until_fetch_service() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.bg_pipeline_state.current_transfer_x = 5;
        ppu.obj_pipeline_state
            .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

        ppu.advance_mode3_output_phase();

        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
    }

    #[test]
    fn hidden_startup_dot_advances_pre_visible_match_x_without_bg_fifo_pop() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.bg_pipeline_state.current_transfer_x = 5;

        let result = ppu.advance_mode3_output_phase();

        assert_eq!(result.kind, Mode3TransferDotKind::ServedPreVisibleTransfer);
        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 6);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
        assert!(ppu.bg_pipeline_state.fifo.is_empty());
    }

    #[test]
    fn mode3_started_uses_explicit_startup_entry_delay_before_transfer_service() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + 1;
        ppu.bg_pipeline_state.mode3_started = true;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.bg_pipeline_state.startup_source_state =
            Mode3StartupSourceState::EntryDelay { remaining: 2 };
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
        ppu.bg_pipeline_state.current_transfer_x = 5;

        let first = ppu.advance_mode3_output_phase();
        assert_eq!(first.kind, Mode3TransferDotKind::NotServed);
        assert_eq!(
            ppu.bg_pipeline_state.startup_source_state,
            Mode3StartupSourceState::EntryDelay { remaining: 1 }
        );
        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);

        let second = ppu.advance_mode3_output_phase();
        assert_eq!(second.kind, Mode3TransferDotKind::NotServed);
        assert_eq!(
            ppu.bg_pipeline_state.startup_source_state,
            Mode3StartupSourceState::Abstract {
                remaining: MODE3_ABSTRACT_SOURCE_WINDOW_DOTS
            }
        );
        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);

        let third = ppu.advance_mode3_output_phase();
        assert_eq!(third.kind, Mode3TransferDotKind::ServedPreVisibleTransfer);
        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 6);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    }

    #[test]
    fn mode3_started_keeps_an_explicit_abstract_source_window_before_fifo_backed_transfer() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.mode3_started = true;
        ppu.bg_pipeline_state.startup_source_state =
            Mode3StartupSourceState::Abstract { remaining: 1 };
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
        ppu.bg_pipeline_state.current_transfer_x = 5;

        assert_eq!(
            ppu.current_transfer_service_plan(),
            Some(Mode3TransferServicePlan {
                result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
                action: Mode3TransferServiceAction::AdvanceHiddenX,
                backing: Mode3TransferBacking::Abstract,
            })
        );

        let transfer_dot = ppu.advance_mode3_output_phase();
        assert_eq!(
            transfer_dot.kind,
            Mode3TransferDotKind::ServedPreVisibleTransfer
        );
        assert_eq!(
            ppu.bg_pipeline_state.startup_source_state,
            Mode3StartupSourceState::FifoBacked
        );

        assert_eq!(
            ppu.current_transfer_service_plan(),
            Some(Mode3TransferServicePlan {
                result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
                action: Mode3TransferServiceAction::AdvanceHiddenX,
                backing: Mode3TransferBacking::FifoBacked,
            })
        );
    }

    #[test]
    fn mode3_started_keeps_an_explicit_previsible_lane_before_hidden_transfer() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.mode3_started = true;
        ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 1;
        ppu.bg_pipeline_state.current_transfer_x = 5;
        ppu.bg_pipeline_state.fifo.push_back(0);

        assert_eq!(
            ppu.current_transfer_service_plan(),
            Some(Mode3TransferServicePlan {
                result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
                action: Mode3TransferServiceAction::AdvanceHiddenX,
                backing: Mode3TransferBacking::FifoBacked,
            })
        );

        let transfer_dot = ppu.advance_mode3_output_phase();
        assert_eq!(
            transfer_dot.kind,
            Mode3TransferDotKind::ServedPreVisibleTransfer
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .startup_pre_visible_transfer_dots_remaining,
            0
        );

        ppu.bg_pipeline_state.fifo.push_back(0);
        assert_eq!(
            ppu.current_transfer_service_plan(),
            Some(Mode3TransferServicePlan {
                result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
                action: Mode3TransferServiceAction::AdvanceHiddenX,
                backing: Mode3TransferBacking::FifoBacked,
            })
        );
    }

    #[test]
    fn bg_fifo_starvation_after_priming_does_not_advance_pre_visible_match_x() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.bg_pipeline_state.current_transfer_x = 5;

        ppu.advance_mode3_output_phase();

        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
    }

    #[test]
    fn abstract_previsible_scx_discard_keeps_lx_zero_until_hidden_transfer_begins() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.bg_pipeline_state.current_transfer_x = 0;
        ppu.bg_pipeline_state.scx_discard_remaining = 1;

        let result = ppu.advance_mode3_output_phase();

        assert_eq!(result.kind, Mode3TransferDotKind::ServedPreVisibleTransfer);
        assert!(result.consumed_scx_discard);
        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
        assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
        assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
        assert_eq!(
            ppu.bg_pipeline_state.transfer_phase,
            Mode3TransferPhase::Output
        );
    }

    #[test]
    fn fifo_backed_hidden_service_moves_transfer_phase_to_output() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 7;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
        ppu.bg_pipeline_state.fifo.push_back(0);

        let result = ppu.advance_mode3_output_phase();

        assert_eq!(result.kind, Mode3TransferDotKind::ServedHiddenTransfer);
        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 8);
        assert_eq!(
            ppu.bg_pipeline_state.transfer_phase,
            Mode3TransferPhase::Output
        );
        assert_eq!(
            ppu.bg_pipeline_state
                .fifo
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![0]
        );
    }

    #[test]
    fn late_previsible_dot_advances_only_after_the_first_bg_fifo_fill_exists() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 7;

        ppu.advance_mode3_output_phase();

        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 7);
        assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);

        ppu.bg_pipeline_state.fifo.push_back(2);

        let result = ppu.advance_mode3_output_phase();

        assert_eq!(result.kind, Mode3TransferDotKind::ServedHiddenTransfer);
        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 8);
        assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
        assert_eq!(
            ppu.bg_pipeline_state
                .fifo
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![2]
        );
    }

    #[test]
    fn late_hidden_scx_discard_waits_for_fifo_then_consumes_the_last_discard_with_fifo_backing() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 0;
        ppu.bg_pipeline_state.scx_discard_remaining = 1;

        ppu.advance_mode3_output_phase();

        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
        assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 1);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);

        ppu.bg_pipeline_state.fifo.push_back(0);
        ppu.advance_mode3_output_phase();

        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
        assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
        assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
        assert_eq!(
            ppu.bg_pipeline_state
                .fifo
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![0]
        );
    }

    #[test]
    fn wx_zero_previsible_window_start_requires_a_late_fifo_backed_served_dot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0xF1;
        ppu.visible_registers.wx = 0;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
        ppu.bg_pipeline_state.window_wy_latch = true;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 7;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;

        let not_ready_dot = ppu.advance_mode3_output_phase();
        assert_eq!(not_ready_dot.kind, Mode3TransferDotKind::NotServed);
        assert!(!ppu.maybe_start_window_after_transfer_dot(not_ready_dot));

        ppu.bg_pipeline_state.fifo.push_back(0);
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;

        let ready_dot = ppu.advance_mode3_output_phase();

        assert_eq!(ready_dot.kind, Mode3TransferDotKind::ServedHiddenTransfer);
        assert!(ppu.maybe_start_window_after_transfer_dot(ready_dot));
        assert!(ppu.bg_pipeline_state.window_started_this_line);
        assert_eq!(
            ppu.bg_pipeline_state.fetcher.source,
            PpuBgFetcherSource::Window
        );
    }

    #[test]
    fn wx_zero_last_scx_discard_shortening_is_applied_from_the_served_transfer_dot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0xF1;
        ppu.visible_registers.wx = 0;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT + 2;
        ppu.bg_pipeline_state.window_wy_latch = true;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 3;
        ppu.bg_pipeline_state.initial_scx_discard = 3;
        ppu.bg_pipeline_state.scx_discard_remaining = 1;
        ppu.bg_pipeline_state.current_transfer_x = 0;
        let transfer_dot = ppu.advance_mode3_output_phase();
        ppu.maybe_apply_wx0_shortening_after_transfer_dot(transfer_dot);

        assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 2);
        assert!(!ppu.bg_pipeline_state.window_started_this_line);
    }

    #[test]
    fn wx_seven_starts_window_from_the_first_served_x0_transfer_dot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0xF1;
        ppu.visible_registers.wx = 7;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
        ppu.bg_pipeline_state.window_wy_latch = true;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 7;
        ppu.bg_pipeline_state.fifo.push_back(0);
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;

        let transfer_dot = ppu.advance_mode3_output_phase();

        assert_eq!(
            transfer_dot.kind,
            Mode3TransferDotKind::ServedHiddenTransfer
        );
        assert!(ppu.maybe_start_window_after_transfer_dot(transfer_dot));
        assert!(ppu.bg_pipeline_state.window_started_this_line);
    }

    #[test]
    fn pending_obj_hit_blocks_window_start_because_the_output_dot_is_not_served() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x93;
        ppu.visible_registers.wx = 15;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.window_wy_latch = true;
        ppu.bg_pipeline_state.current_transfer_x = 8;
        ppu.bg_pipeline_state.visible_pixels_output = 8;
        ppu.bg_pipeline_state.fifo.push_back(1);
        ppu.obj_pipeline_state
            .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

        let transfer_dot = ppu.advance_mode3_output_phase();

        assert_eq!(transfer_dot.kind, Mode3TransferDotKind::NotServed);
        assert!(!ppu.maybe_start_window_after_transfer_dot(transfer_dot));
        assert!(!ppu.bg_pipeline_state.window_started_this_line);
    }

    #[test]
    fn bg_fifo_discard_after_priming_keeps_lx_zero_until_discard_finishes() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 0;
        ppu.bg_pipeline_state.scx_discard_remaining = 1;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.fifo.push_back(0);

        let result = ppu.advance_mode3_output_phase();

        assert_eq!(result.kind, Mode3TransferDotKind::ServedHiddenTransfer);
        assert!(result.consumed_scx_discard);
        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
        assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
        assert_eq!(
            ppu.bg_pipeline_state.transfer_phase,
            Mode3TransferPhase::Output
        );
    }

    #[test]
    fn visible_bg_pixel_output_reports_a_visible_pixel_dot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x91;
        ppu.ly = 0;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.current_transfer_x = 8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.fifo.push_back(2);

        let result = ppu.advance_mode3_output_phase();

        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 9);
        assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 1);
    }

    #[test]
    fn current_obj_hit_ownership_tracks_x_and_dot_phase() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);

        ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
        ppu.bg_pipeline_state.current_transfer_x = 6;
        assert_eq!(
            ppu.current_obj_hit_ownership(),
            ObjHitOwnership {
                match_x: 6,
                phase: ObjHitPhase::PreVisible,
            }
        );

        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 0;
        ppu.bg_pipeline_state.visible_pixels_output = 0;
        ppu.bg_pipeline_state.scx_discard_remaining = 1;
        assert_eq!(
            ppu.current_obj_hit_ownership(),
            ObjHitOwnership {
                match_x: 0,
                phase: ObjHitPhase::Hidden,
            }
        );

        ppu.bg_pipeline_state.scx_discard_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 20;
        ppu.bg_pipeline_state.visible_pixels_output = 12;
        assert_eq!(
            ppu.current_obj_hit_ownership(),
            ObjHitOwnership {
                match_x: 20,
                phase: ObjHitPhase::Visible,
            }
        );
    }

    #[test]
    fn stale_pending_obj_hit_is_cleared_once_current_x_moves_on() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.bg_pipeline_state.current_transfer_x = 13;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.visible_pixels_output = 5;
        ppu.obj_pipeline_state.queue_fetch_hit(
            0,
            ObjHitOwnership {
                match_x: 12,
                phase: ObjHitPhase::Visible,
            },
        );

        ppu.sync_pending_obj_hit_ownership();

        assert!(ppu.obj_pipeline_state.pending_sprite_slots.is_empty());
        assert_eq!(ppu.obj_pipeline_state.pending_match_x, None);
    }

    #[test]
    fn pending_obj_hit_survives_dot_phase_changes_while_current_x_is_still_the_same() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.visible_registers.lcdc = 0x82;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.current_transfer_x = 6;
        ppu.bg_pipeline_state.scx_discard_remaining = 1;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.obj_pipeline_state.queue_fetch_hit(
            0,
            ObjHitOwnership {
                match_x: 6,
                phase: ObjHitPhase::PreVisible,
            },
        );

        ppu.sync_pending_obj_hit_ownership();

        assert_eq!(
            ppu.obj_pipeline_state
                .pending_sprite_slots
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![0]
        );
        assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(6));
    }

    #[test]
    fn bg_fetcher_stage_dot_is_an_explicit_one_dot_automaton() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
        let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
        vram.set_acquired(BusMaster::Ppu, true);

        ppu.visible_registers.lcdc = 0x91;
        ppu.bg_pipeline_state.fetcher.start_background();

        assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
        assert_eq!(
            ppu.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileIndex
        );
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

        assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
        assert_eq!(
            ppu.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileDataLow
        );
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

        assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
        assert_eq!(
            ppu.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileDataLow
        );
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

        assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
        assert_eq!(
            ppu.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileDataHigh
        );
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

        assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
        assert_eq!(
            ppu.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileDataHigh
        );
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

        assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
        assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);
        assert!(ppu.bg_pipeline_state.push.pending);
        assert!(!ppu.bg_pipeline_state.fill.pending);
    }

    #[test]
    fn mode3_scx_discard_shifts_visible_pixels_and_delays_hblank_entry() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
        write_bg_tile_row(&mut vram_bytes, 1, 0, 0xAA, 0xCC);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
        write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 1);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x82,
            scy: 0x00,
            scx: 0x03,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..252 {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        }

        let extended_drawing = ppu.snapshot();
        assert_eq!(extended_drawing.line_dot, 252);
        assert_eq!(extended_drawing.mode, PpuAccessMode::Drawing);
        assert_eq!(extended_drawing.mode0_start_dot, 255);

        for t_cycle in 252..255 {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        }

        let hblank = ppu.snapshot();
        assert_eq!(hblank.line_dot, 255);
        assert_eq!(hblank.mode, PpuAccessMode::HBlank);
        assert_eq!(hblank.mode_dot, 0);
        assert_eq!(hblank.visible_pixels_output, 160);
        assert_eq!(
            &hblank.current_scanline_pixels[..8],
            &[3, 0, 1, 2, 3, 3, 2, 1]
        );
    }

    #[test]
    fn window_start_restarts_the_fetcher_and_switches_to_window_pixels_mid_scanline() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
        write_bg_tile_row(&mut vram_bytes, 1, 0, 0xCC, 0xF0);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
        write_window_tilemap_entry(&mut vram_bytes, 0, 0, 1);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0xF1,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x0F,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
        assert!(snapshot.window_wy_latch);
        assert!(snapshot.window_started_this_line);
        assert_eq!(
            &snapshot.current_scanline_pixels[..16],
            &[0, 1, 2, 3, 0, 1, 2, 3, 3, 3, 2, 2, 1, 1, 0, 0]
        );
    }

    #[test]
    fn wy_latch_is_sampled_at_mode2_start_and_not_recomputed_mid_line() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
        write_bg_tile_row(&mut vram_bytes, 1, 0, 0xCC, 0xF0);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
        write_window_tilemap_entry(&mut vram_bytes, 0, 0, 1);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0xF1,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x01,
            wx: 0x07,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..100 {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        }

        ppu.write_register(0xFF4A, 0x00);

        let _ = tick_until_hblank(&mut ppu, 100, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert!(!snapshot.window_wy_latch);
        assert!(!snapshot.window_started_this_line);
        assert_eq!(snapshot.window_line_counter, 0);
        assert_eq!(
            &snapshot.current_scanline_pixels[..16],
            &[0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3]
        );
    }

    #[test]
    fn window_line_counter_advances_only_on_lines_where_window_actually_starts() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
        write_bg_tile_row(&mut vram_bytes, 1, 0, 0xCC, 0xF0);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
        write_window_tilemap_entry(&mut vram_bytes, 0, 0, 1);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0xF1,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0xA7,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let t_cycle = tick_until_line_start(&mut ppu, 0, &oam_bytes, &vram_bytes, 1);
        assert_eq!(ppu.snapshot().window_line_counter, 0);

        ppu.write_register(0xFF4B, 0x07);

        let _t_cycle = tick_until_line_start(&mut ppu, t_cycle, &oam_bytes, &vram_bytes, 2);
        let line_2_start = ppu.snapshot();
        assert_eq!(line_2_start.window_line_counter, 1);
    }

    #[test]
    fn wx_zero_with_scx_discard_shortens_window_start_timing_by_one_dot() {
        let oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
        write_window_tilemap_entry(&mut vram_bytes, 0, 0, 0);

        let mut wx_zero = Ppu::new(ConsoleModel::Dmg);
        wx_zero.apply_startup_state(PpuStartupState {
            lcdc: 0xF1,
            stat: 0x82,
            scy: 0x00,
            scx: 0x03,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let mut wx_seven = Ppu::new(ConsoleModel::Dmg);
        wx_seven.apply_startup_state(PpuStartupState {
            lcdc: 0xF1,
            stat: 0x82,
            scy: 0x00,
            scx: 0x03,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x07,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut wx_zero, 0, &oam_bytes, &vram_bytes);
        let _ = tick_until_hblank(&mut wx_seven, 0, &oam_bytes, &vram_bytes);

        assert_eq!(
            wx_zero.snapshot().mode0_start_dot + 1,
            wx_seven.snapshot().mode0_start_dot
        );
    }

    #[test]
    fn wx_166_defers_window_start_to_the_following_scanline() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
        write_bg_tile_row(&mut vram_bytes, 1, 0, 0xCC, 0xF0);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
        write_window_tilemap_entry(&mut vram_bytes, 0, 0, 1);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0xF1,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 166,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let t_cycle = tick_until_line_start(&mut ppu, 0, &oam_bytes, &vram_bytes, 1);
        let first_line = ppu.snapshot();
        assert_eq!(first_line.window_line_counter, 0);

        let _ = tick_until_hblank(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        let second_line = ppu.snapshot();
        assert!(second_line.window_started_this_line);
        assert_eq!(
            &second_line.current_scanline_pixels[..8],
            &[3, 3, 2, 2, 1, 1, 0, 0]
        );
    }

    #[test]
    fn obj_priority_prefers_lower_x_before_oam_order() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 16, 20, 0);
        write_oam_entry(&mut oam_bytes, 1, 16, 18, 1);
        write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);
        write_bg_tile_row(&mut vram_bytes, 1, 0, 0x00, 0xFF);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x82,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(
            &snapshot.current_scanline_pixels[10..20],
            &[2, 2, 2, 2, 2, 2, 2, 2, 1, 1]
        );
    }

    #[test]
    fn object_fetch_reads_tile_and_attributes_from_live_oam_metadata() {
        let sprite = PpuSelectedSprite {
            oam_index: 3,
            y: 16,
            x: 24,
            tile_index: 0x11,
            attributes: 0x22,
        };
        let mut oam_bytes = [0; 160];
        write_oam_entry_with_attributes(
            &mut oam_bytes,
            sprite.oam_index,
            sprite.y,
            sprite.x,
            0x44,
            0xA0,
        );

        let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
        let oam = OamBusView::new(BusMaster::Ppu, &mut oam);

        let (tile_index, attributes) = read_obj_fetch_sprite_metadata(&oam, sprite, None);

        assert_eq!(tile_index, 0x44);
        assert_eq!(attributes, 0xA0);
    }

    #[test]
    fn object_fetch_uses_the_dma_conflict_word_address_for_late_oam_metadata_reads() {
        let sprite = PpuSelectedSprite {
            oam_index: 0,
            y: 16,
            x: 24,
            tile_index: 0x11,
            attributes: 0x22,
        };
        let mut oam_bytes = [0; 160];
        write_oam_entry_with_attributes(
            &mut oam_bytes,
            sprite.oam_index,
            sprite.y,
            sprite.x,
            0x44,
            0xA0,
        );
        write_oam_entry_with_attributes(&mut oam_bytes, 5, 32, 40, 0x99, 0x10);

        let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
        let oam = OamBusView::new(BusMaster::Ppu, &mut oam);

        let (tile_index, attributes) = read_obj_fetch_sprite_metadata(&oam, sprite, Some(0xFE17));

        assert_eq!(tile_index, 0x99);
        assert_eq!(attributes, 0x10);
    }

    #[test]
    fn obj_priority_uses_oam_order_when_x_matches() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 16, 20, 0);
        write_oam_entry(&mut oam_bytes, 1, 16, 20, 1);
        write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);
        write_bg_tile_row(&mut vram_bytes, 1, 0, 0x00, 0xFF);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x82,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(&snapshot.current_scanline_pixels[12..20], &[1; 8]);
    }

    #[test]
    fn transparent_obj_pixels_do_not_hide_lower_priority_obj_pixels() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 16, 20, 0);
        write_oam_entry(&mut oam_bytes, 1, 16, 20, 1);
        write_bg_tile_row(&mut vram_bytes, 0, 0, 0xAA, 0x00);
        write_bg_tile_row(&mut vram_bytes, 1, 0, 0x00, 0xFF);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x82,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(
            &snapshot.current_scanline_pixels[12..20],
            &[1, 2, 1, 2, 1, 2, 1, 2]
        );
    }

    #[test]
    fn bg_over_obj_priority_blocks_only_nonzero_bg_pixels() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry_with_attributes(&mut oam_bytes, 0, 16, 8, 0, 0x80);
        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x00, 0xFF);
        write_bg_tile_row(&mut vram_bytes, 1, 0, 0xAA, 0x00);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 1);
        write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 1);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x93,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(
            &snapshot.current_scanline_pixels[..8],
            &[1, 2, 1, 2, 1, 2, 1, 2]
        );
    }

    #[test]
    fn framebuffer_applies_bgp_without_changing_logical_scanline_colors() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
        write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x1B,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(
            &snapshot.current_scanline_pixels[..8],
            &[0, 1, 2, 3, 0, 1, 2, 3]
        );
        assert_eq!(&ppu.framebuffer()[..8], &[3, 2, 1, 0, 3, 2, 1, 0]);
    }

    #[test]
    fn pixel_output_uses_visible_bgp_state_rather_than_the_new_live_mmio_value() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x83,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xE4,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu.visible_output = PpuVisibleOutputState::Driving;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.visible_registers.bgp = 0x1B;
        ppu.bgp = 0xE4;
        ppu.bg_pipeline_state.scx_discard_remaining = 0;
        ppu.bg_pipeline_state.current_transfer_x = 8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.fifo.push_back(1);

        let _ = ppu.advance_mode3_output_phase();

        assert_eq!(ppu.snapshot().current_scanline_pixels[0], 1);
        assert_eq!(ppu.framebuffer()[0], 2);
    }

    #[test]
    fn framebuffer_applies_obj_palette_selection_without_changing_logical_obj_colors() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry_with_attributes(&mut oam_bytes, 0, 16, 8, 0, 0x10);
        write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x92,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xE4,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu.write_register(0xFF48, 0xE4);
        ppu.write_register(0xFF49, 0x0C);

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(&snapshot.current_scanline_pixels[..8], &[1; 8]);
        assert_eq!(&ppu.framebuffer()[..8], &[3; 8]);
    }

    #[test]
    fn dmg_bgp_write_during_mode3_recolors_recent_bg_pixels_with_transient_then_final_palette() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x83,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x01,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu.visible_output = PpuVisibleOutputState::Driving;
        ppu.line_dot = 200;
        ppu.bg_pipeline_state.visible_pixels_output = 8;
        ppu.current_scanline_mixed_pixels[4..8].fill(MixedPixel::background(0));
        ppu.framebuffer[4..8].fill(1);

        ppu.write_register(0xFF47, 0x00);

        assert_eq!(&ppu.framebuffer()[4..8], &[1, 0, 0, 0]);
    }

    #[test]
    fn dmg_bgp_write_in_early_hblank_recolors_only_last_three_visible_bg_pixels() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x80,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x01,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu.visible_output = PpuVisibleOutputState::Driving;
        ppu.line_dot = MODE0_START_DOT;
        ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
        ppu.current_scanline_mixed_pixels[156..160].fill(MixedPixel::background(0));
        ppu.framebuffer[156..160].fill(1);

        ppu.write_register(0xFF47, 0x00);

        assert_eq!(&ppu.framebuffer()[156..160], &[1, 1, 0, 0]);
    }

    #[test]
    fn dmg_obp0_write_during_mode3_recolors_five_recent_obj_pixels() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x82,
            stat: 0x83,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xE4,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu.visible_output = PpuVisibleOutputState::Driving;
        ppu.line_dot = 200;
        ppu.bg_pipeline_state.visible_pixels_output = 8;
        ppu.current_scanline_mixed_pixels[3..8].fill(MixedPixel::object(1, false));
        ppu.framebuffer[3..8].fill(3);

        ppu.write_register(0xFF48, 0x04);

        assert_eq!(&ppu.framebuffer()[3..8], &[3, 1, 1, 1, 1]);
    }

    #[test]
    fn obj_8x16_uses_even_aligned_tile_pairs_for_lower_half_rows() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 8, 8, 0x11);
        write_bg_tile_row(&mut vram_bytes, 0x10, 0, 0xFF, 0x00);
        write_bg_tile_row(&mut vram_bytes, 0x11, 0, 0x00, 0xFF);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x86,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(&snapshot.current_scanline_pixels[..8], &[2; 8]);
    }

    #[test]
    fn partially_visible_top_clipped_8x16_sprite_uses_the_correct_row() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 2, 8, 0x10);
        write_bg_tile_row(&mut vram_bytes, 0x11, 6, 0x00, 0xFF);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x86,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(&snapshot.current_scanline_pixels[..8], &[2; 8]);
    }

    #[test]
    fn partially_visible_bottom_clipped_sprite_uses_the_correct_final_rows() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 154, 8, 0x12);
        write_bg_tile_row(&mut vram_bytes, 0x12, 5, 0xFF, 0xFF);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x82,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 143,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(&snapshot.current_scanline_pixels[..8], &[3; 8]);
    }

    #[test]
    fn live_obj_size_shrink_drops_out_of_range_y_flipped_rows_without_panicking() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.lcdc = 0x82;
        ppu.ly = 0;

        let sprite = PpuSelectedSprite {
            oam_index: 0,
            y: 2,
            x: 8,
            tile_index: 0x10,
            attributes: 0x40,
        };

        assert_eq!(ppu.obj_tile_index_and_row(sprite), None);
    }

    #[test]
    fn turning_off_lcdc1_during_object_fetch_cancels_sprite_pixels_but_keeps_timing_cost() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 16, 8, 0);
        write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x82,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..80 {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        }

        let mut t_cycle = 80;
        loop {
            let fetching = ppu.snapshot();
            if fetching.obj_fetcher_stage == PpuObjFetcherStage::Startup {
                break;
            }

            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
            t_cycle += 1;
            assert!(
                t_cycle < 96,
                "left-edge OBJ fetch should begin during early Mode 3"
            );
        }

        ppu.write_register(0xFF40, 0x80);

        let _ = tick_until_hblank(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);

        let snapshot = ppu.snapshot();
        assert_eq!(snapshot.mode0_start_dot, MODE0_START_DOT + 8);
        assert_eq!(&snapshot.current_scanline_pixels[..8], &[0; 8]);
    }

    #[test]
    fn smaller_raw_obj_x_values_start_fetch_earlier_during_mode3_startup() {
        fn fetch_start_line_dot(sprite_x: u8) -> u16 {
            let mut ppu = Ppu::new(ConsoleModel::Dmg);
            let mut oam_bytes = [0; 160];
            let mut vram_bytes = [0; TEST_VRAM_BYTES];

            write_oam_entry(&mut oam_bytes, 0, 16, sprite_x, 0);
            write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);

            ppu.apply_startup_state(PpuStartupState {
                lcdc: 0x82,
                stat: 0x82,
                scy: 0x00,
                scx: 0x00,
                ly: 0x00,
                lyc: 0x00,
                bgp: 0x00,
                wy: 0x00,
                wx: 0x00,
                obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
            });

            for t_cycle in 0..96 {
                tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
                let snapshot = ppu.snapshot();
                if snapshot.obj_fetcher_stage == PpuObjFetcherStage::Startup {
                    return snapshot.line_dot;
                }
            }

            panic!("sprite fetch did not begin during early Mode 3");
        }

        let left_edge = fetch_start_line_dot(1);
        let first_visible = fetch_start_line_dot(8);

        assert!(left_edge < first_visible);
        assert!(left_edge >= MODE2_DOTS);
        assert!(first_visible >= MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1);
        assert!(first_visible <= MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS);
    }

    #[test]
    fn overlapped_obj_fetch_uses_explicit_one_dot_stage_progression() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 16, 8, 0);
        write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);

        let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
        let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
        oam.set_acquired(BusMaster::Ppu, true);
        vram.set_acquired(BusMaster::Ppu, true);

        ppu.visible_registers.lcdc = 0x82;
        ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
        ppu.bg_pipeline_state.current_transfer_x = 8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.fifo.push_back(0);
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: 0,
            y: 16,
            x: 8,
            tile_index: 0,
            attributes: 0,
        });
        ppu.obj_pipeline_state
            .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

        assert!(ppu.try_start_object_fetch_from_current_dot(
            ObjFetchStartSource::FifoBackedTransfer,
            true,
        ));
        assert_eq!(
            ppu.obj_pipeline_state.fetch.stage,
            PpuObjFetcherStage::Startup
        );
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

        assert!(ppu.advance_object_fetch(
            &OamBusView::new(BusMaster::Ppu, &mut oam),
            &VramBusView::new(BusMaster::Ppu, &mut vram),
            None,
        ));
        assert_eq!(
            ppu.obj_pipeline_state.fetch.stage,
            PpuObjFetcherStage::TileDataLow
        );
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);

        assert!(ppu.advance_object_fetch(
            &OamBusView::new(BusMaster::Ppu, &mut oam),
            &VramBusView::new(BusMaster::Ppu, &mut vram),
            None,
        ));
        assert_eq!(
            ppu.obj_pipeline_state.fetch.stage,
            PpuObjFetcherStage::TileDataLow
        );
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

        assert!(ppu.advance_object_fetch(
            &OamBusView::new(BusMaster::Ppu, &mut oam),
            &VramBusView::new(BusMaster::Ppu, &mut vram),
            None,
        ));
        assert_eq!(
            ppu.obj_pipeline_state.fetch.stage,
            PpuObjFetcherStage::TileDataHigh
        );
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);

        assert!(ppu.advance_object_fetch(
            &OamBusView::new(BusMaster::Ppu, &mut oam),
            &VramBusView::new(BusMaster::Ppu, &mut vram),
            None,
        ));
        assert_eq!(
            ppu.obj_pipeline_state.fetch.stage,
            PpuObjFetcherStage::TileDataHigh
        );
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

        assert!(ppu.advance_object_fetch(
            &OamBusView::new(BusMaster::Ppu, &mut oam),
            &VramBusView::new(BusMaster::Ppu, &mut vram),
            None,
        ));
        assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Push);
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);

        assert!(ppu.advance_object_fetch(
            &OamBusView::new(BusMaster::Ppu, &mut oam),
            &VramBusView::new(BusMaster::Ppu, &mut vram),
            None,
        ));
        assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Push);
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

        assert!(ppu.advance_object_fetch(
            &OamBusView::new(BusMaster::Ppu, &mut oam),
            &VramBusView::new(BusMaster::Ppu, &mut vram),
            None,
        ));
        assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
        assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
    }

    #[test]
    fn current_mode2_oam_row_tracks_the_live_four_dot_slices() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let oam_bytes = [0; 160];

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        assert_eq!(ppu.snapshot().current_oam_scan_row, Some(0));

        for t_cycle in 0..5 {
            tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        }

        assert_eq!(ppu.snapshot().current_oam_scan_row, Some(1));

        for t_cycle in 4..80 {
            tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        }

        let drawing = ppu.snapshot();
        assert_eq!(drawing.mode, PpuAccessMode::Drawing);
        assert_eq!(drawing.current_oam_scan_row, None);
    }

    #[test]
    fn first_oam_row_is_immune_to_basic_read_and_write_corruption_patterns() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut read_oam = [0; 160];
        let mut write_oam = [0; 160];

        write_oam_corruption_row(&mut read_oam, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
        write_oam_corruption_row(&mut write_oam, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Read, &mut read_oam));
        assert_eq!(
            &read_oam[..8],
            &[0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB]
        );

        assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Write, &mut write_oam));
        assert_eq!(
            &write_oam[..8],
            &[0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB]
        );
    }

    #[test]
    fn write_corruption_uses_the_documented_first_word_formula_and_previous_row_tail() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];

        write_oam_corruption_row(&mut oam_bytes, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
        write_oam_corruption_row(&mut oam_bytes, 1, [0x0F0F, 0x1111, 0x2222, 0x3333]);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..5 {
            tick_ppu(&mut ppu, t_cycle, &[0; 160]);
        }

        assert_eq!(ppu.snapshot().current_oam_scan_row, Some(1));
        assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Write, &mut oam_bytes));

        let expected_first = ((0x0F0F_u16 ^ 0xAAAA) & (0x1357 ^ 0xAAAA)) ^ 0xAAAA;
        assert_eq!(read_oam_word(&oam_bytes, 1, 0), expected_first);
        assert_eq!(read_oam_word(&oam_bytes, 1, 1), 0x2468);
        assert_eq!(read_oam_word(&oam_bytes, 1, 2), 0xAAAA);
        assert_eq!(read_oam_word(&oam_bytes, 1, 3), 0xBBBB);
    }

    #[test]
    fn read_corruption_uses_the_documented_first_word_formula_and_previous_row_tail() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];

        write_oam_corruption_row(&mut oam_bytes, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
        write_oam_corruption_row(&mut oam_bytes, 1, [0x0F0F, 0x1111, 0x2222, 0x3333]);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..5 {
            tick_ppu(&mut ppu, t_cycle, &[0; 160]);
        }

        assert_eq!(ppu.snapshot().current_oam_scan_row, Some(1));
        assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Read, &mut oam_bytes));

        let expected_first = 0x1357_u16 | (0x0F0F & 0xAAAA);
        assert_eq!(read_oam_word(&oam_bytes, 1, 0), expected_first);
        assert_eq!(read_oam_word(&oam_bytes, 1, 1), 0x2468);
        assert_eq!(read_oam_word(&oam_bytes, 1, 2), 0xAAAA);
        assert_eq!(read_oam_word(&oam_bytes, 1, 3), 0xBBBB);
    }

    #[test]
    fn read_plus_incdec_uses_its_dedicated_complex_path_in_rows_four_through_eighteen() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];

        write_oam_corruption_row(&mut oam_bytes, 2, [0x0F0F, 0x1212, 0x3434, 0x5656]);
        write_oam_corruption_row(&mut oam_bytes, 3, [0xAAAA, 0x1111, 0xC0C0, 0x2222]);
        write_oam_corruption_row(&mut oam_bytes, 4, [0x00FF, 0x3333, 0x4444, 0x5555]);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..17 {
            tick_ppu(&mut ppu, t_cycle, &[0; 160]);
        }

        assert_eq!(ppu.snapshot().current_oam_scan_row, Some(4));
        assert!(
            ppu.apply_oam_corruption_event(OamCorruptionEventKind::ReadWithIncDec, &mut oam_bytes)
        );

        let expected_previous_first = 0xAAAA_u16 & (0x0F0F | 0x00FF | 0xC0C0);
        let expected_row = [expected_previous_first, 0x1111, 0xC0C0, 0x2222];

        for (word_index, expected) in expected_row.into_iter().enumerate() {
            assert_eq!(read_oam_word(&oam_bytes, 2, word_index), expected);
            assert_eq!(read_oam_word(&oam_bytes, 4, word_index), expected);
        }
        assert_eq!(read_oam_word(&oam_bytes, 3, 0), expected_previous_first);
        assert_eq!(read_oam_word(&oam_bytes, 3, 1), 0x1111);
        assert_eq!(read_oam_word(&oam_bytes, 3, 2), 0xC0C0);
        assert_eq!(read_oam_word(&oam_bytes, 3, 3), 0x2222);
    }

    #[test]
    fn read_plus_incdec_on_the_last_row_falls_back_to_ordinary_read_corruption() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];

        write_oam_corruption_row(&mut oam_bytes, 18, [0x1234, 0x1111, 0x00FF, 0x2222]);
        write_oam_corruption_row(&mut oam_bytes, 19, [0x0F0F, 0xAAAA, 0xBBBB, 0xCCCC]);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..77 {
            tick_ppu(&mut ppu, t_cycle, &[0; 160]);
        }

        assert_eq!(ppu.snapshot().current_oam_scan_row, Some(19));
        assert!(
            ppu.apply_oam_corruption_event(OamCorruptionEventKind::ReadWithIncDec, &mut oam_bytes)
        );

        let expected_first = 0x1234_u16 | (0x0F0F & 0x00FF);
        assert_eq!(read_oam_word(&oam_bytes, 19, 0), expected_first);
        assert_eq!(read_oam_word(&oam_bytes, 19, 1), 0x1111);
        assert_eq!(read_oam_word(&oam_bytes, 19, 2), 0x00FF);
        assert_eq!(read_oam_word(&oam_bytes, 19, 3), 0x2222);
        assert_eq!(read_oam_word(&oam_bytes, 17, 0), 0x0000);
    }

    #[test]
    fn cgb_models_do_not_apply_dmg_family_oam_corruption() {
        let mut ppu = Ppu::new(ConsoleModel::Cgb);
        let mut oam_bytes = [0; 160];

        write_oam_corruption_row(&mut oam_bytes, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
        write_oam_corruption_row(&mut oam_bytes, 1, [0x0F0F, 0x1111, 0x2222, 0x3333]);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..4 {
            tick_ppu(&mut ppu, t_cycle, &[0; 160]);
        }

        let before = oam_bytes;
        assert!(!ppu.apply_oam_corruption_event(OamCorruptionEventKind::Write, &mut oam_bytes));
        assert_eq!(oam_bytes, before);
    }
}
