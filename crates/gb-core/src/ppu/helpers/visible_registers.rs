use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ppu) enum PpuDmgBgpCpuCommitEffectKind {
    PipelineDelayed,
    CurrentDotTransient,
    RetroactivePanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ppu) struct PpuDmgBgpCpuCommitWrite {
    pub(in crate::ppu) effect_kind: PpuDmgBgpCpuCommitEffectKind,
    pub(in crate::ppu) transient_visible_x: u8,
    pub(in crate::ppu) transient_palette: u8,
    pub(in crate::ppu) repaint_visible_x: u8,
    pub(in crate::ppu) transfer_lead_pixels: u8,
    pub(in crate::ppu) value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ppu) struct PpuDmgBgpBoundaryRepaintWrite {
    pub(in crate::ppu) write: PpuDmgBgpCpuCommitWrite,
    pub(in crate::ppu) selected_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ppu) struct PpuRecentPanelDot {
    pub(in crate::ppu) visible_x: u8,
    pub(in crate::ppu) pixel: MixedPixel,
    pub(in crate::ppu) dmg_bg_forced_white: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
