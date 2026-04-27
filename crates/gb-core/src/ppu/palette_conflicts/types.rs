use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) enum PpuDmgBgpCpuCommitEffectKind {
    PipelineDelayed,
    CurrentDotTransient,
    RetroactivePanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuDmgBgpCpuCommitWrite {
    pub(in crate::ppu) effect_kind: PpuDmgBgpCpuCommitEffectKind,
    pub(in crate::ppu) transient_visible_x: u8,
    pub(in crate::ppu) transient_palette: u8,
    pub(in crate::ppu) repaint_visible_x: u8,
    pub(in crate::ppu) transfer_lead_pixels: u8,
    pub(in crate::ppu) value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuDmgBgpBoundaryRepaintWrite {
    pub(in crate::ppu) write: PpuDmgBgpCpuCommitWrite,
    pub(in crate::ppu) selected_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in crate::ppu) struct PpuRecentPanelDot {
    pub(in crate::ppu) visible_x: u8,
    pub(in crate::ppu) pixel: MixedPixel,
    pub(in crate::ppu) dmg_bg_forced_white: bool,
}
