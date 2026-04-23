use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmgPanelRangeRepaint {
    Lcdc0BgEnable { bg_enabled: bool },
    Lcdc1ObjDisable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmgPanelRepaintContext {
    visible_output_driving: bool,
    row_start: usize,
    historical_bgp: u8,
}

type PpuPublishedStatPredicate = fn(&Ppu) -> bool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PpuPublishedStatModeContext {
    published_mode: PpuAccessMode,
    current_mode: PpuAccessMode,
    sprite_extended_mode3: bool,
}

include!("control/panel.rs");
include!("control/registers.rs");
include!("control/published_stat.rs");
include!("control/mode2.rs");
include!("control/live_writes.rs");
include!("control/irq.rs");
