use super::super::*;

mod lcdc0;
mod lcdc3;
mod lcdc4;
mod live_refetch;
mod snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObservabilityRigConfig {
    lcdc: u8,
    ly: u8,
    wy: u8,
}

impl ObservabilityRigConfig {
    const fn new(lcdc: u8, ly: u8, wy: u8) -> Self {
        Self { lcdc, ly, wy }
    }
}

fn dmg_observability_rig(config: ObservabilityRigConfig) -> PpuTestRig {
    PpuTestRig::dmg().with_startup_state(PpuStartupState {
        lcdc: config.lcdc,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: config.ly,
        lyc: 0x00,
        bgp: 0xE4,
        wy: config.wy,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    })
}

fn advance_visible_output_step(ppu: &mut PpuTestRig) -> Mode3TransferDot {
    ppu.maybe_recompute_pending_background_fill_with_ppu_vram();
    ppu.flush_pending_bg_fifo_fill();
    ppu.advance_mode3_output_phase_with_ppu_vram()
}
