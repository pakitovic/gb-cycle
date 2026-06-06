use super::border::{DEFAULT_BORDER_PALETTE, DEFAULT_BORDER_TILEMAP, DEFAULT_BORDER_TILES};
use super::{SGB_BORDER_PALETTE_COLORS, SgbBorderMapEntry, SgbBorderState, SgbRgb555Color};

pub const SGB_SHELL_BORDER_FADE_FRAMES: u8 = 96;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbShellState {
    pub enabled: bool,
    pub default_border_loaded: bool,
    pub border_transition: SgbShellBorderTransitionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SgbShellBorderTransitionState {
    pub phase: SgbShellBorderTransitionPhase,
    pub frame: u8,
    pub game_border_ready: bool,
    pub fallback_border: Option<Box<SgbBorderState>>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbShellBorderTransitionPhase {
    #[default]
    Idle,
    FadeFallbackToBlack,
    HoldBlackUntilGameBorder,
    FadeBlackToGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SgbShellBorderPixel {
    pub color: SgbRgb555Color,
    pub lcd_scale: Option<u8>,
}

impl SgbShellState {
    pub const fn default_for_active_host(active: bool) -> Self {
        Self {
            enabled: active,
            default_border_loaded: active,
            border_transition: SgbShellBorderTransitionState {
                phase: SgbShellBorderTransitionPhase::Idle,
                frame: 0,
                game_border_ready: false,
                fallback_border: None,
            },
        }
    }

    pub fn preserve_default_border_before_game_transfer(&mut self, border: &SgbBorderState) {
        if self.enabled
            && self.default_border_loaded
            && self.border_transition.fallback_border.is_none()
        {
            self.border_transition.fallback_border = Some(Box::new(border.clone()));
        }
    }

    pub fn start_game_border_fade_out(&mut self, border: &SgbBorderState) {
        if !self.enabled || !self.default_border_loaded {
            return;
        }

        self.preserve_default_border_before_game_transfer(border);
        self.default_border_loaded = false;
        self.border_transition.game_border_ready = false;
        if self.border_transition.fallback_border.is_some() {
            self.border_transition.phase = SgbShellBorderTransitionPhase::FadeFallbackToBlack;
            self.border_transition.frame = 0;
        }
    }

    pub fn start_game_border_transition(&mut self) {
        if !self.enabled {
            self.default_border_loaded = false;
            return;
        }

        self.border_transition.game_border_ready = true;
        if self.default_border_loaded {
            self.default_border_loaded = false;
            if self.border_transition.fallback_border.is_some() {
                self.border_transition.phase = SgbShellBorderTransitionPhase::FadeFallbackToBlack;
                self.border_transition.frame = 0;
            } else {
                self.border_transition.phase = SgbShellBorderTransitionPhase::Idle;
            }
        } else if self.border_transition.phase
            == SgbShellBorderTransitionPhase::HoldBlackUntilGameBorder
        {
            self.border_transition.phase = SgbShellBorderTransitionPhase::FadeBlackToGame;
            self.border_transition.frame = 0;
        }
    }

    pub fn advance_frame(&mut self) {
        match self.border_transition.phase {
            SgbShellBorderTransitionPhase::Idle => {}
            SgbShellBorderTransitionPhase::FadeFallbackToBlack => {
                self.border_transition.frame = self.border_transition.frame.saturating_add(1);
                if self.border_transition.frame >= SGB_SHELL_BORDER_FADE_FRAMES {
                    self.border_transition.phase = if self.border_transition.game_border_ready {
                        SgbShellBorderTransitionPhase::FadeBlackToGame
                    } else {
                        SgbShellBorderTransitionPhase::HoldBlackUntilGameBorder
                    };
                    self.border_transition.frame = 0;
                }
            }
            SgbShellBorderTransitionPhase::HoldBlackUntilGameBorder => {}
            SgbShellBorderTransitionPhase::FadeBlackToGame => {
                self.border_transition.frame = self.border_transition.frame.saturating_add(1);
                if self.border_transition.frame >= SGB_SHELL_BORDER_FADE_FRAMES {
                    self.border_transition.phase = SgbShellBorderTransitionPhase::Idle;
                    self.border_transition.frame = 0;
                    self.border_transition.game_border_ready = false;
                    self.border_transition.fallback_border = None;
                }
            }
        }
    }

    pub fn presentation_border_pixel(
        &self,
        game_border: &SgbBorderState,
        backdrop_color: SgbRgb555Color,
        x: usize,
        y: usize,
    ) -> SgbShellBorderPixel {
        match self.border_transition.phase {
            SgbShellBorderTransitionPhase::Idle => {
                let border = self
                    .border_transition
                    .fallback_border
                    .as_deref()
                    .filter(|_| self.default_border_loaded)
                    .unwrap_or(game_border);
                let (color, color_index) = border.pixel_color(x, y);
                SgbShellBorderPixel {
                    color: border_presentation_color(color, color_index, backdrop_color),
                    lcd_scale: (color_index == 0).then_some(SGB_SHELL_BORDER_FADE_FRAMES),
                }
            }
            SgbShellBorderTransitionPhase::FadeFallbackToBlack => {
                let border = self
                    .border_transition
                    .fallback_border
                    .as_deref()
                    .unwrap_or(game_border);
                let (color, color_index) = border.pixel_color(x, y);
                let color = border_presentation_color(color, color_index, backdrop_color);
                let scale = SGB_SHELL_BORDER_FADE_FRAMES
                    .saturating_sub(self.border_transition.frame)
                    .min(SGB_SHELL_BORDER_FADE_FRAMES);
                SgbShellBorderPixel {
                    color: scale_rgb555(color, scale),
                    lcd_scale: (color_index == 0).then_some(scale),
                }
            }
            SgbShellBorderTransitionPhase::HoldBlackUntilGameBorder => SgbShellBorderPixel {
                color: SgbRgb555Color::new(0),
                lcd_scale: None,
            },
            SgbShellBorderTransitionPhase::FadeBlackToGame => {
                let (color, color_index) = game_border.pixel_color(x, y);
                let color = border_presentation_color(color, color_index, backdrop_color);
                let scale = self
                    .border_transition
                    .frame
                    .saturating_add(1)
                    .min(SGB_SHELL_BORDER_FADE_FRAMES);
                SgbShellBorderPixel {
                    color: scale_rgb555(color, scale),
                    lcd_scale: (color_index == 0).then_some(scale),
                }
            }
        }
    }

    pub(super) fn dynamic_payload_bytes(&self) -> usize {
        self.border_transition
            .fallback_border
            .as_deref()
            .map(SgbBorderState::dynamic_payload_bytes)
            .unwrap_or(0)
    }
}

impl Default for SgbShellState {
    fn default() -> Self {
        Self::default_for_active_host(false)
    }
}

pub fn load_default_border(border: &mut SgbBorderState) {
    *border = SgbBorderState::default();

    for (index, &color) in DEFAULT_BORDER_PALETTE.iter().enumerate() {
        border.palettes[0].colors[index] = SgbRgb555Color::new(color);
    }
    for color_index in DEFAULT_BORDER_PALETTE.len()..SGB_BORDER_PALETTE_COLORS {
        border.palettes[0].colors[color_index] = SgbRgb555Color::new(0);
    }
    for palette_index in 1..border.palettes.len() {
        border.palettes[palette_index] = border.palettes[0];
    }

    border.tile_data.bytes.fill(0);
    border.tile_data.bytes[..DEFAULT_BORDER_TILES.len()].copy_from_slice(&DEFAULT_BORDER_TILES);

    for entry in &mut border.tile_map.entries {
        *entry = SgbBorderMapEntry::default();
    }
    for (index, &entry) in DEFAULT_BORDER_TILEMAP.iter().enumerate() {
        border.tile_map.entries[index] = SgbBorderMapEntry::new(entry);
    }

    border.chr0_loaded = true;
    border.chr1_loaded = true;
    border.pct_loaded = true;
}

pub(in crate::sgb) fn scale_rgb555(color: SgbRgb555Color, scale: u8) -> SgbRgb555Color {
    let scale = u16::from(scale.min(SGB_SHELL_BORDER_FADE_FRAMES));
    let denominator = u16::from(SGB_SHELL_BORDER_FADE_FRAMES);
    let raw = color.raw();
    let r = (raw & 0x1F) * scale / denominator;
    let g = ((raw >> 5) & 0x1F) * scale / denominator;
    let b = ((raw >> 10) & 0x1F) * scale / denominator;
    SgbRgb555Color::new(r | (g << 5) | (b << 10))
}

fn border_presentation_color(
    color: SgbRgb555Color,
    color_index: u8,
    backdrop_color: SgbRgb555Color,
) -> SgbRgb555Color {
    if color_index == 0 {
        backdrop_color
    } else {
        color
    }
}
