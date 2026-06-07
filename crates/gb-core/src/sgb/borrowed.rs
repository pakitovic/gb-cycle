use crate::cartridge::{CartridgeHeader, SgbFlag};
use crate::model::{CompatibilityPolicy, ConsoleModel, MachineConfig, SgbHostProfile, StartupMode};
use crate::{DMG_T_CYCLES_PER_FRAME, Machine};

use super::protocol::SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED;
use super::*;

pub const SGB_BORROWED_BORDER_EXTRACTION_FRAME_LIMIT: u16 = 600;

/// Presentation-only grace window after the first completed borrowed-border PCT_TRN.
///
/// Some SGB-enhanced titles, notably Pokémon Yellow, send a palette command shortly after the border
/// transfer that changes the application backdrop used by transparent border pixels. The active
/// handheld machine must not receive those SGB commands, but the borrowed presentation state needs
/// their settled backdrop color so the border outside the LCD aperture matches SGB output.
pub const SGB_BORROWED_BORDER_PRESENTATION_SETTLE_FRAMES: u16 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SgbBorrowedBorder {
    border: SgbBorderState,
    backdrop_color: SgbRgb555Color,
}

impl SgbBorrowedBorder {
    pub fn new(border: SgbBorderState) -> Self {
        Self::with_backdrop_color(border, SgbRgb555Color::default())
    }

    pub fn with_backdrop_color(border: SgbBorderState, backdrop_color: SgbRgb555Color) -> Self {
        Self {
            border,
            backdrop_color,
        }
    }

    pub fn border(&self) -> &SgbBorderState {
        &self.border
    }

    pub fn backdrop_color(&self) -> SgbRgb555Color {
        self.backdrop_color
    }

    pub fn pixel_rgb555_outside_lcd(&self, x: usize, y: usize) -> Option<u16> {
        if x >= SGB_FRAME_WIDTH || y >= SGB_FRAME_HEIGHT {
            return None;
        }
        if (SGB_LCD_FRAME_ORIGIN_X..SGB_LCD_FRAME_ORIGIN_X + SGB_LCD_WIDTH).contains(&x)
            && (SGB_LCD_FRAME_ORIGIN_Y..SGB_LCD_FRAME_ORIGIN_Y + SGB_LCD_HEIGHT).contains(&y)
        {
            return None;
        }

        let (color, color_index) = self.border.pixel_color(x, y);
        Some(if color_index == 0 {
            self.backdrop_color.raw()
        } else {
            color.raw()
        })
    }
}

pub fn sgb_header_accepts_borrowed_border(header: &CartridgeHeader) -> bool {
    header.sgb_flag == SgbFlag::Supported
        && header.old_licensee_code == SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED
}

pub fn extract_initial_sgb_borrowed_border(
    rom_bytes: &[u8],
    compatibility: &CompatibilityPolicy,
) -> Option<SgbBorrowedBorder> {
    let header = CartridgeHeader::parse(rom_bytes).ok()?;
    if !sgb_header_accepts_borrowed_border(&header) {
        return None;
    }

    let config = MachineConfig::new(ConsoleModel::GameBoy)
        .with_startup_mode(StartupMode::SkipBoot)
        .with_sgb_profile(SgbHostProfile::SgbNtsc)
        .with_compatibility(compatibility.clone());
    let mut machine = Machine::new_summary(config);
    machine.load_cartridge(rom_bytes.to_vec()).ok()?;
    let initial_pct_transfer_count = machine
        .sgb_host()
        .snapshot()
        .video
        .border
        .pct_transfer_count;
    let mut borrowed_candidate = None;
    let mut last_presentation_signature = None;
    let mut last_presentation_change_frame = 0;
    for frame in 0..SGB_BORROWED_BORDER_EXTRACTION_FRAME_LIMIT {
        let target_t_cycle = machine
            .next_t_cycle()
            .get()
            .saturating_add(DMG_T_CYCLES_PER_FRAME);
        while machine.next_t_cycle().get() < target_t_cycle {
            machine.step_t_cycle();
        }

        let snapshot = machine.sgb_host().snapshot();
        let border = &snapshot.video.border;
        if border.pct_loaded && border.pct_transfer_count > initial_pct_transfer_count {
            let presentation_signature = (
                border.pct_transfer_count,
                border.chr_transfer_count,
                snapshot.video.palette_command_count,
                snapshot.video.backdrop_color,
            );
            if last_presentation_signature != Some(presentation_signature) {
                last_presentation_signature = Some(presentation_signature);
                last_presentation_change_frame = frame;
            }

            let borrowed = SgbBorrowedBorder::with_backdrop_color(
                border.clone(),
                snapshot.video.backdrop_color,
            );
            if frame.saturating_sub(last_presentation_change_frame)
                >= SGB_BORROWED_BORDER_PRESENTATION_SETTLE_FRAMES
            {
                return Some(borrowed);
            }
            borrowed_candidate = Some(borrowed);
        }
    }

    borrowed_candidate
}
