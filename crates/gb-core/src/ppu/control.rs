use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum DmgPanelRangeRepaint {
    Lcdc0BgEnable { bg_enabled: bool },
    Lcdc1ObjDisable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct DmgPanelRepaintContext {
    visible_output_driving: bool,
    row_start: usize,
    historical_bgp: u8,
}

type PpuPublishedStatPredicate = fn(&Ppu) -> bool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PpuPublishedStatModeContext {
    published_mode: PpuAccessMode,
    current_mode: PpuAccessMode,
    sprite_extended_mode3: bool,
}

mod irq;
mod live_writes;
mod mode2;
mod panel;
mod published_stat;
mod registers;
