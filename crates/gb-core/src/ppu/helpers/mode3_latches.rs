use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(in crate::ppu) struct PpuMode3RegisterLatches {
    visible: PpuVisibleRegisters,
    pipeline: PpuVisibleRegisters,
}

impl PpuMode3RegisterLatches {
    pub(in crate::ppu) const fn from_mmio(registers: PpuVisibleRegisters) -> Self {
        Self {
            visible: registers,
            pipeline: registers,
        }
    }

    pub(in crate::ppu) const fn new(
        visible: PpuVisibleRegisters,
        pipeline: PpuVisibleRegisters,
    ) -> Self {
        Self { visible, pipeline }
    }

    pub(in crate::ppu) const fn visible(self) -> PpuVisibleRegisters {
        self.visible
    }

    pub(in crate::ppu) const fn pipeline(self) -> PpuVisibleRegisters {
        self.pipeline
    }

    pub(in crate::ppu) const fn advance(self, next_visible: PpuVisibleRegisters) -> Self {
        Self {
            visible: next_visible,
            pipeline: self.visible,
        }
    }

    pub(in crate::ppu) const fn bg_fetch_registers(
        self,
        use_pipeline_snapshot: bool,
    ) -> PpuVisibleRegisters {
        if use_pipeline_snapshot {
            self.pipeline
        } else {
            self.visible
        }
    }

    pub(in crate::ppu) const fn window_fetch_registers(self) -> PpuVisibleRegisters {
        self.visible
    }

    pub(in crate::ppu) const fn window_activation_registers(
        self,
        console_model: ConsoleModel,
    ) -> PpuVisibleRegisters {
        if console_model.is_dmg_family() {
            self.pipeline
        } else {
            self.visible
        }
    }

    pub(in crate::ppu) const fn mode3_start_scx(self) -> u8 {
        self.visible.scx
    }

    pub(in crate::ppu) const fn current_obj_height(self) -> u8 {
        self.visible.obj_height()
    }

    pub(in crate::ppu) const fn pixel_pipeline_lcdc(
        self,
        console_model: ConsoleModel,
        current_transfer_x: u8,
    ) -> u8 {
        if !console_model.is_dmg_family() || current_transfer_x == 8 {
            self.visible.lcdc
        } else {
            self.pipeline.lcdc
        }
    }

    pub(in crate::ppu) const fn pixel_pipeline_bgp(
        self,
        console_model: ConsoleModel,
        output_override: Option<u8>,
        bg_visible_hold_override: Option<u8>,
    ) -> u8 {
        if !console_model.is_dmg_family() {
            return self.visible.bgp;
        }

        if let Some(override_palette) = output_override {
            return override_palette;
        }
        if let Some(override_palette) = bg_visible_hold_override {
            return override_palette;
        }

        self.visible.bgp | self.pipeline.bgp
    }

    pub(in crate::ppu) const fn pixel_transfer_bg_enabled(
        self,
        console_model: ConsoleModel,
        current_transfer_x: u8,
    ) -> bool {
        if console_model.is_dmg_family() {
            self.visible.lcdc & LCDC_BG_ENABLE_BIT != 0
        } else {
            self.pixel_pipeline_lcdc(console_model, current_transfer_x) & LCDC_BG_ENABLE_BIT != 0
        }
    }

    pub(in crate::ppu) const fn pixel_transfer_obj_enabled(
        self,
        console_model: ConsoleModel,
        current_transfer_x: u8,
    ) -> bool {
        self.pixel_pipeline_lcdc(console_model, current_transfer_x) & LCDC_OBJ_ENABLE_BIT != 0
    }

    pub(in crate::ppu) const fn lcdc_bit_changed(self, bit: u8) -> bool {
        (self.pipeline.lcdc & bit) != (self.visible.lcdc & bit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3LiveRegisterWriteContext {
    previous: PpuVisibleRegisters,
    current: PpuVisibleRegisters,
}

impl PpuMode3LiveRegisterWriteContext {
    pub(in crate::ppu) const fn new(
        previous: PpuVisibleRegisters,
        current: PpuVisibleRegisters,
    ) -> Self {
        Self { previous, current }
    }

    pub(in crate::ppu) const fn lcdc_changed(self, mask: u8) -> bool {
        (self.previous.lcdc ^ self.current.lcdc) & mask != 0
    }

    pub(in crate::ppu) const fn bg_tilemap_select_changed(self) -> bool {
        self.lcdc_changed(LCDC_BG_TILE_MAP_BIT)
    }

    pub(in crate::ppu) const fn bg_window_tile_data_select_changed(self) -> bool {
        self.lcdc_changed(LCDC_BG_WINDOW_TILE_DATA_BIT)
    }

    pub(in crate::ppu) const fn bg_scx_tilemap_column_changed(self) -> bool {
        (self.previous.scx >> 3) != (self.current.scx >> 3)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3LiveBackgroundRefetchContext {
    registers: PpuVisibleRegisters,
    ly: u8,
    last_unsigned_tile_data_low_fetch: u8,
    last_unsigned_tile_data_high_fetch: u8,
}

impl PpuMode3LiveBackgroundRefetchContext {
    pub(in crate::ppu) const fn new(
        registers: PpuVisibleRegisters,
        ly: u8,
        last_unsigned_tile_data_low_fetch: u8,
        last_unsigned_tile_data_high_fetch: u8,
    ) -> Self {
        Self {
            registers,
            ly,
            last_unsigned_tile_data_low_fetch,
            last_unsigned_tile_data_high_fetch,
        }
    }

    pub(in crate::ppu) const fn registers(self) -> PpuVisibleRegisters {
        self.registers
    }

    pub(in crate::ppu) const fn ly(self) -> u8 {
        self.ly
    }

    pub(in crate::ppu) const fn current_scanline_tile_row(self) -> u16 {
        (self.registers.scy.wrapping_add(self.ly) % BG_TILE_WIDTH) as u16
    }

    pub(in crate::ppu) const fn last_unsigned_tile_data_low_fetch(self) -> u8 {
        self.last_unsigned_tile_data_low_fetch
    }

    pub(in crate::ppu) const fn last_unsigned_tile_data_high_fetch(self) -> u8 {
        self.last_unsigned_tile_data_high_fetch
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3BackgroundFetchContext {
    tilemap_registers: PpuVisibleRegisters,
    tiledata_registers: PpuVisibleRegisters,
    next_fetch_pixel: u16,
    ly: u8,
}

impl PpuMode3BackgroundFetchContext {
    pub(in crate::ppu) const fn new(
        tilemap_registers: PpuVisibleRegisters,
        tiledata_registers: PpuVisibleRegisters,
        next_fetch_pixel: u16,
        ly: u8,
    ) -> Self {
        Self {
            tilemap_registers,
            tiledata_registers,
            next_fetch_pixel,
            ly,
        }
    }

    pub(in crate::ppu) const fn tile_index_address(self) -> u16 {
        let bg_x = self
            .tilemap_registers
            .scx
            .wrapping_add(self.next_fetch_pixel as u8);
        let bg_y = self.tiledata_registers.scy.wrapping_add(self.ly);
        let tile_map_base = if self.tilemap_registers.lcdc & LCDC_BG_TILE_MAP_BIT != 0 {
            0x1C00
        } else {
            0x1800
        };
        let tile_x = (bg_x / BG_TILE_WIDTH) as usize;
        let tile_y = (bg_y / BG_TILE_WIDTH) as usize;
        (tile_map_base + tile_y * BG_TILE_MAP_WIDTH as usize + tile_x) as u16
    }

    pub(in crate::ppu) const fn tile_data_address(self, tile_index: u8, plane: u16) -> u16 {
        let tile_row = (self.tiledata_registers.scy.wrapping_add(self.ly) % BG_TILE_WIDTH) as u16;
        let tile_data_base = bg_tile_data_base(self.tiledata_registers.lcdc, tile_index);
        tile_data_base + tile_row * TILE_ROW_BYTES + plane
    }

    pub(in crate::ppu) const fn uses_unsigned_tile_data(self) -> bool {
        self.tiledata_registers.lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3WindowFetchContext {
    registers: PpuVisibleRegisters,
    window_line_counter: u8,
    window_tilemap_x: u8,
}

impl PpuMode3WindowFetchContext {
    pub(in crate::ppu) const fn new(
        registers: PpuVisibleRegisters,
        window_line_counter: u8,
        window_tilemap_x: u8,
    ) -> Self {
        Self {
            registers,
            window_line_counter,
            window_tilemap_x,
        }
    }

    pub(in crate::ppu) const fn tile_index_address(self) -> u16 {
        let tile_map_base = if self.registers.lcdc & LCDC_WINDOW_TILE_MAP_BIT != 0 {
            0x1C00
        } else {
            0x1800
        };
        let tile_x = self.window_tilemap_x as usize;
        let tile_y = (self.window_line_counter / BG_TILE_WIDTH) as usize;
        (tile_map_base + tile_y * BG_TILE_MAP_WIDTH as usize + tile_x) as u16
    }

    pub(in crate::ppu) const fn tile_data_address(self, tile_index: u8, plane: u16) -> u16 {
        let tile_row = (self.window_line_counter % BG_TILE_WIDTH) as u16;
        let tile_data_base = bg_tile_data_base(self.registers.lcdc, tile_index);
        tile_data_base + tile_row * TILE_ROW_BYTES + plane
    }

    pub(in crate::ppu) const fn uses_unsigned_tile_data(self) -> bool {
        self.registers.lcdc & LCDC_BG_WINDOW_TILE_DATA_BIT != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3WindowActivationState {
    registers: PpuVisibleRegisters,
    force_x0_this_line: bool,
}

impl PpuMode3WindowActivationState {
    pub(in crate::ppu) const fn new(
        registers: PpuVisibleRegisters,
        force_x0_this_line: bool,
    ) -> Self {
        Self {
            registers,
            force_x0_this_line,
        }
    }

    pub(in crate::ppu) const fn runtime_enabled(self) -> bool {
        self.registers.window_enabled() && self.registers.bg_enabled()
    }

    pub(in crate::ppu) const fn is_wx_zero(self) -> bool {
        self.registers.wx == 0
    }

    pub(in crate::ppu) const fn is_wx_166(self) -> bool {
        self.registers.wx == 166 && !self.force_x0_this_line
    }

    pub(in crate::ppu) const fn trigger_x(self) -> Option<u8> {
        if self.force_x0_this_line {
            return Some(0);
        }

        match self.registers.wx {
            0..=166 => Some(self.registers.wx.saturating_sub(7)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) struct PpuMode3PreparedWindowLine {
    wy_triggered: bool,
    wy_latch: bool,
    force_x0_this_line: bool,
}

impl PpuMode3PreparedWindowLine {
    pub(in crate::ppu) const fn new(
        wy_triggered: bool,
        wy_latch: bool,
        force_x0_this_line: bool,
    ) -> Self {
        Self {
            wy_triggered,
            wy_latch,
            force_x0_this_line,
        }
    }

    pub(in crate::ppu) const fn wy_triggered(self) -> bool {
        self.wy_triggered
    }

    pub(in crate::ppu) const fn wy_latch(self) -> bool {
        self.wy_latch
    }

    pub(in crate::ppu) const fn force_x0_this_line(self) -> bool {
        self.force_x0_this_line
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) enum PpuMode3WindowStartDecision {
    NotReady,
    ArmWx166NextLine,
    StartNow,
}
