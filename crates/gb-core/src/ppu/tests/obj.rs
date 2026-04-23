use super::*;

mod arbitration;
mod fetch;
mod render;
mod same_x;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    const fn same_x(oam_index: u8, x: u8) -> Self {
        Self::new(oam_index, 16, x, oam_index, 0)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

fn same_x_test_sprite(oam_index: u8, x: u8) -> PpuSelectedSprite {
    selected_sprite(SelectedSpriteSpec::same_x(oam_index, x))
}

fn push_same_x_test_sprites(ppu: &mut Ppu, x: u8, count: u8) {
    for sprite_slot in 0..count {
        ppu.mode2_scan_state
            .push(same_x_test_sprite(sprite_slot, x));
    }
}

fn fill_bg_fifo(ppu: &mut Ppu, len: usize) {
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, len));
}

fn arm_object_fetch_push_stage(ppu: &mut Ppu, sprite_slot: u8, sprite: PpuSelectedSprite) {
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = sprite_slot;
    ppu.obj_pipeline_state.fetch.sprite = Some(sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(sprite);
    ppu.obj_pipeline_state.fetch.selected_obj_height = ppu.current_obj_height();
    ppu.obj_pipeline_state.fetch.latched_obj_height = ppu.current_obj_height();
    let (tile_index, tile_row) = ppu
        .obj_tile_index_and_row(sprite)
        .expect("armed object fetch should resolve tile metadata");
    ppu.obj_pipeline_state.fetch.resolved_tile_index = Some(tile_index);
    ppu.obj_pipeline_state.fetch.resolved_tile_row = Some(tile_row);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;
}
