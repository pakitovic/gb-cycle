use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SgbLcdCompositionError {
    DisabledHost,
    InputLength { expected: usize, actual: usize },
    OutputLength { expected: usize, actual: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SgbFrameCompositionError {
    DisabledHost,
    InputLength { expected: usize, actual: usize },
    OutputLength { expected: usize, actual: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SgbLcdRgb555Frame {
    pub pixels: Vec<u16>,
}

impl Default for SgbLcdRgb555Frame {
    fn default() -> Self {
        Self {
            pixels: vec![0; SGB_LCD_PIXELS],
        }
    }
}

impl SgbLcdRgb555Frame {
    pub(in crate::sgb) fn dynamic_payload_bytes(&self) -> usize {
        self.pixels.len().saturating_mul(std::mem::size_of::<u16>())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbRgb555Color {
    raw: u16,
}

impl SgbRgb555Color {
    pub const fn new(raw: u16) -> Self {
        Self {
            raw: raw & SGB_RGB555_MASK,
        }
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }

    pub(in crate::sgb) const fn from_packet_bytes(low: u8, high: u8) -> Self {
        Self::new(u16::from_le_bytes([low, high]))
    }
}

impl Default for SgbRgb555Color {
    fn default() -> Self {
        SGB_RGB555_BLACK
    }
}
