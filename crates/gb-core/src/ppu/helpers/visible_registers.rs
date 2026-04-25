use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuRecentBgDotContext {
    pub(in crate::ppu) source: PpuBgFetcherSource,
    pub(in crate::ppu) fetch_x: u16,
    pub(in crate::ppu) pixel_index: u8,
    pub(in crate::ppu) tile_index: u8,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(in crate::ppu) struct PpuVisibleRegisters {
    pub(in crate::ppu) lcdc: u8,
    pub(in crate::ppu) scy: u8,
    pub(in crate::ppu) scx: u8,
    pub(in crate::ppu) bgp: u8,
    pub(in crate::ppu) obp0: Option<u8>,
    pub(in crate::ppu) obp1: Option<u8>,
    pub(in crate::ppu) wy: u8,
    pub(in crate::ppu) wx: u8,
}

impl PpuVisibleRegisters {
    pub(in crate::ppu) const fn bg_enabled(self) -> bool {
        self.lcdc & LCDC_BG_ENABLE_BIT != 0
    }

    pub(in crate::ppu) const fn obj_enabled(self) -> bool {
        self.lcdc & LCDC_OBJ_ENABLE_BIT != 0
    }

    pub(in crate::ppu) const fn window_enabled(self) -> bool {
        self.lcdc & LCDC_WINDOW_ENABLE_BIT != 0
    }

    pub(in crate::ppu) const fn obj_height(self) -> u8 {
        if self.lcdc & LCDC_OBJ_SIZE_BIT != 0 {
            16
        } else {
            8
        }
    }

    pub(in crate::ppu) fn obj_palette(
        self,
        palette_obp1: bool,
        policy: DmgObjPaletteReadPolicy,
    ) -> u8 {
        if palette_obp1 {
            self.obp1.unwrap_or(policy.default_read_value())
        } else {
            self.obp0.unwrap_or(policy.default_read_value())
        }
    }

    pub(in crate::ppu) fn palette_register(
        self,
        register: PpuPaletteRegister,
        policy: DmgObjPaletteReadPolicy,
    ) -> u8 {
        match register {
            PpuPaletteRegister::Bgp => self.bgp,
            PpuPaletteRegister::Obp0 => self.obj_palette(false, policy),
            PpuPaletteRegister::Obp1 => self.obj_palette(true, policy),
        }
    }

    pub(in crate::ppu) fn palette_for_mixed_pixel(
        self,
        pixel: MixedPixel,
        bg_palette: u8,
        policy: DmgObjPaletteReadPolicy,
    ) -> u8 {
        match pixel.source {
            MixedPixelSource::Background => bg_palette,
            MixedPixelSource::Object { palette_obp1 } => {
                self.palette_register(PpuPaletteRegister::for_obj_palette(palette_obp1), policy)
            }
        }
    }

    pub(in crate::ppu) fn palette_for_mixed_pixel_with_override(
        self,
        pixel: MixedPixel,
        register: PpuPaletteRegister,
        palette_override: u8,
        bg_palette: u8,
        policy: DmgObjPaletteReadPolicy,
    ) -> u8 {
        match pixel.source {
            MixedPixelSource::Background => {
                if matches!(register, PpuPaletteRegister::Bgp) {
                    palette_override
                } else {
                    bg_palette
                }
            }
            MixedPixelSource::Object { palette_obp1 } => {
                if register.affects_obj_palette(palette_obp1) {
                    palette_override
                } else {
                    self.palette_register(PpuPaletteRegister::for_obj_palette(palette_obp1), policy)
                }
            }
        }
    }
}
