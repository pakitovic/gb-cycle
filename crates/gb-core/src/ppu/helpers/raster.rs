use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(in crate::ppu) enum PpuLcdRestartPhase {
    #[default]
    Inactive,
    FirstLineAfterEnable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ppu) enum PpuRasterState {
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
    pub(in crate::ppu) const fn access_mode(self) -> PpuAccessMode {
        match self {
            Self::Disabled => PpuAccessMode::HBlank,
            Self::LcdRestartFirstLine { mode, .. } | Self::Active { mode, .. } => mode,
        }
    }

    pub(in crate::ppu) const fn mode_dot(self) -> u16 {
        match self {
            Self::Disabled => 0,
            Self::LcdRestartFirstLine { mode_dot, .. } | Self::Active { mode_dot, .. } => mode_dot,
        }
    }

    pub(in crate::ppu) const fn is_mode2_scan(self) -> bool {
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
    pub(in crate::ppu) const fn first_line_after_enable() -> Self {
        Self::FirstLineAfterEnable
    }

    pub(in crate::ppu) const fn is_first_line_after_enable_active(self, ly: u8) -> bool {
        matches!(self, Self::FirstLineAfterEnable) && ly == 0
    }

    pub(in crate::ppu) const fn raster_state(
        self,
        ly: u8,
        line_dot: u16,
    ) -> Option<PpuRasterState> {
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

    pub(in crate::ppu) const fn advance(self, ly: u8, _line_dot: u16) -> Self {
        if self.is_first_line_after_enable_active(ly) {
            self
        } else {
            Self::Inactive
        }
    }
}
