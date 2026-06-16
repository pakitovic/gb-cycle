use super::super::*;

mod snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
