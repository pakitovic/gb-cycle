use super::*;

mod arbitration;
mod cgb;
mod fetch;
mod render;
mod same_x;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct SelectedSpriteSpec {
    oam_index: u8,
    y: u8,
    x: u8,
    tile_index: u8,
    attributes: u8,
}

impl SelectedSpriteSpec {
    const fn new(oam_index: u8, y: u8, x: u8, tile_index: u8, attributes: u8) -> Self {
        Self {
            oam_index,
            y,
            x,
            tile_index,
            attributes,
        }
    }
}

fn selected_sprite(spec: SelectedSpriteSpec) -> PpuSelectedSprite {
    PpuSelectedSprite {
        oam_index: spec.oam_index,
        y: spec.y,
        x: spec.x,
        tile_index: spec.tile_index,
        attributes: spec.attributes,
    }
}

fn push_selected_sprite(ppu: &mut Ppu, spec: SelectedSpriteSpec) -> PpuSelectedSprite {
    let sprite = selected_sprite(spec);
    ppu.mode2_scan_state.push(sprite);
    sprite
}

fn queue_current_obj_hit(ppu: &mut Ppu, sprite_slot: u8) {
    let ownership = ppu.current_obj_hit_ownership();
    let current_obj_height = ppu.current_obj_height();
    ppu.obj_pipeline_state
        .queue_fetch_hit(sprite_slot, ownership, current_obj_height);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ObjRenderRigConfig {
    lcdc: u8,
    ly: u8,
}

fn dmg_obj_render_rig(config: ObjRenderRigConfig) -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: config.lcdc,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: config.ly,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu
}

fn fill_bg_fifo(ppu: &mut Ppu, len: usize) {
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, len));
}
