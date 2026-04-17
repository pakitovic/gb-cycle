use super::*;

pub(super) fn dmg_mode3_startup_state(lcdc: u8, ly: u8, scx: u8) -> PpuStartupState {
    PpuStartupState {
        lcdc,
        stat: 0x82,
        scy: 0x00,
        scx,
        ly,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    }
}

pub(super) fn lcdc_write_context(
    previous_lcdc: u8,
    current_lcdc: u8,
) -> PpuMode3LiveRegisterWriteContext {
    PpuMode3LiveRegisterWriteContext::new(
        PpuVisibleRegisters {
            lcdc: previous_lcdc,
            ..PpuVisibleRegisters::default()
        },
        PpuVisibleRegisters {
            lcdc: current_lcdc,
            ..PpuVisibleRegisters::default()
        },
    )
}

pub(super) fn obj_toggle_sprite(
    oam_index: u8,
    y: u8,
    x: u8,
    tile_index: u8,
    attributes: u8,
) -> PpuSelectedSprite {
    PpuSelectedSprite {
        oam_index,
        y,
        x,
        tile_index,
        attributes,
    }
}
