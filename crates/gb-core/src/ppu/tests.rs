use super::*;
use crate::bus::BusMaster;
use crate::scheduler::TCycle;

const TEST_VRAM_BYTES: usize = 0x2000;

fn sync_test_video_ownership(
    ppu: &Ppu,
    oam: &mut crate::bus::OamDomain,
    vram: &mut crate::bus::VramDomain,
    dma_oam_active: bool,
) {
    let bus_state = ppu.bus_state();
    let ppu_vram = bus_state.is_lcd_enabled() && bus_state.mode() == PpuAccessMode::Drawing;
    let ppu_oam = bus_state.is_lcd_enabled()
        && matches!(
            bus_state.mode(),
            PpuAccessMode::OamScan | PpuAccessMode::Drawing
        );

    oam.set_acquired(BusMaster::Ppu, ppu_oam);
    vram.set_acquired(BusMaster::Ppu, ppu_vram);
    oam.set_acquired(BusMaster::Dma, dma_oam_active);
    vram.set_acquired(BusMaster::Dma, false);
}

fn tick_ppu(ppu: &mut Ppu, t_cycle: u64, oam_bytes: &[u8]) -> CycleContext {
    tick_ppu_with_vram(ppu, t_cycle, oam_bytes, &[0; TEST_VRAM_BYTES])
}

fn tick_ppu_with_vram(
    ppu: &mut Ppu,
    t_cycle: u64,
    oam_bytes: &[u8],
    vram_bytes: &[u8],
) -> CycleContext {
    tick_ppu_with_vram_and_dma(ppu, t_cycle, oam_bytes, vram_bytes, false, None)
}

fn tick_ppu_with_vram_and_dma(
    ppu: &mut Ppu,
    t_cycle: u64,
    oam_bytes: &[u8],
    vram_bytes: &[u8],
    dma_oam_active: bool,
    dma_oam_conflict_address: Option<u16>,
) -> CycleContext {
    let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
    let mut oam = crate::bus::OamDomain::from_bytes(oam_bytes);
    let mut vram = crate::bus::VramDomain::from_bytes(vram_bytes);
    sync_test_video_ownership(ppu, &mut oam, &mut vram, dma_oam_active);
    ppu.tick_t_cycle(
        &mut context,
        OamBusView::new(BusMaster::Ppu, &mut oam),
        VramBusView::new(BusMaster::Ppu, &mut vram),
        dma_oam_active,
        dma_oam_conflict_address,
    );
    context
}

fn drain_ppu_interrupts(ppu: &mut Ppu) -> Vec<InterruptSource> {
    ppu.drain_pending_interrupt_requests()
}

fn write_oam_entry(oam_bytes: &mut [u8; 160], index: u8, y: u8, x: u8, tile_index: u8) {
    write_oam_entry_with_attributes(oam_bytes, index, y, x, tile_index, 0);
}

fn write_oam_entry_with_attributes(
    oam_bytes: &mut [u8; 160],
    index: u8,
    y: u8,
    x: u8,
    tile_index: u8,
    attributes: u8,
) {
    let entry_start = index as usize * OAM_ENTRY_BYTES;
    oam_bytes[entry_start] = y;
    oam_bytes[entry_start + 1] = x;
    oam_bytes[entry_start + 2] = tile_index;
    oam_bytes[entry_start + 3] = attributes;
}

fn write_oam_corruption_row(oam_bytes: &mut [u8; 160], row: u8, words: [u16; 4]) {
    for (word_index, value) in words.into_iter().enumerate() {
        write_oam_word(oam_bytes, row, word_index, value);
    }
}

fn write_bg_tile_row(
    vram_bytes: &mut [u8; TEST_VRAM_BYTES],
    tile_index: u8,
    row: u8,
    low: u8,
    high: u8,
) {
    let tile_address =
        tile_index as usize * TILE_BYTES as usize + row as usize * TILE_ROW_BYTES as usize;
    vram_bytes[tile_address] = low;
    vram_bytes[tile_address + 1] = high;
}

fn write_bg_tilemap_entry(vram_bytes: &mut [u8; TEST_VRAM_BYTES], x: u8, y: u8, tile_index: u8) {
    let tile_map_address = 0x1800 + y as usize * BG_TILE_MAP_WIDTH as usize + x as usize;
    vram_bytes[tile_map_address] = tile_index;
}

fn write_window_tilemap_entry(
    vram_bytes: &mut [u8; TEST_VRAM_BYTES],
    x: u8,
    y: u8,
    tile_index: u8,
) {
    let tile_map_address = 0x1C00 + y as usize * BG_TILE_MAP_WIDTH as usize + x as usize;
    vram_bytes[tile_map_address] = tile_index;
}

fn tick_until_hblank(ppu: &mut Ppu, mut t_cycle: u64, oam_bytes: &[u8], vram_bytes: &[u8]) -> u64 {
    let start_t_cycle = t_cycle;
    while ppu.snapshot().mode != PpuAccessMode::HBlank {
        tick_ppu_with_vram(ppu, t_cycle, oam_bytes, vram_bytes);
        t_cycle += 1;
        assert!(t_cycle - start_t_cycle < 2 * DOTS_PER_SCANLINE as u64);
    }

    t_cycle
}

fn tick_until_line_start(
    ppu: &mut Ppu,
    mut t_cycle: u64,
    oam_bytes: &[u8],
    vram_bytes: &[u8],
    target_ly: u8,
) -> u64 {
    while !(ppu.snapshot().ly == target_ly && ppu.snapshot().line_dot == 0) {
        tick_ppu_with_vram(ppu, t_cycle, oam_bytes, vram_bytes);
        t_cycle += 1;
        assert!(t_cycle < 20 * DOTS_PER_SCANLINE as u64);
    }

    t_cycle
}

fn tick_until_next_frame_start(
    ppu: &mut Ppu,
    mut t_cycle: u64,
    oam_bytes: &[u8],
    vram_bytes: &[u8],
) -> u64 {
    while !(t_cycle > 0 && ppu.snapshot().ly == 0 && ppu.snapshot().line_dot == 0) {
        tick_ppu_with_vram(ppu, t_cycle, oam_bytes, vram_bytes);
        t_cycle += 1;
        assert!(t_cycle < 2 * TOTAL_SCANLINES as u64 * DOTS_PER_SCANLINE as u64);
    }

    t_cycle
}

#[test]
fn startup_state_recreates_the_documented_post_boot_lcd_snapshot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    assert_eq!(ppu.read_register(0xFF40), 0x91);
    assert_eq!(ppu.read_register(0xFF41), 0x85);
    assert_eq!(ppu.read_register(0xFF42), 0x00);
    assert_eq!(ppu.read_register(0xFF43), 0x00);
    assert_eq!(ppu.read_register(0xFF44), 0x00);
    assert_eq!(ppu.read_register(0xFF45), 0x00);
    assert_eq!(ppu.read_register(0xFF47), 0xFC);
    assert_eq!(ppu.read_register(0xFF4A), 0x00);
    assert_eq!(ppu.read_register(0xFF4B), 0x00);
    assert_eq!(ppu.snapshot().lcd_state, PpuLcdState::Enabled);
    assert_eq!(
        ppu.snapshot().visible_output,
        PpuVisibleOutputState::Driving
    );
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(
        ppu.bus_state(),
        PpuBusState::lcd_enabled(PpuAccessMode::VBlank)
    );
}

#[test]
fn stat_keeps_live_mode_and_coincidence_bits_outside_the_writable_mask() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x81,
        scy: 0x00,
        scx: 0x00,
        ly: 0x12,
        lyc: 0x12,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF41, 0xFF);

    assert_eq!(ppu.read_register(0xFF41), 0xFD);
}

#[test]
fn lyc_writes_reevaluate_coincidence_immediately_and_can_raise_lcd_stat() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x42,
        scy: 0x00,
        scx: 0x00,
        ly: 0x12,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    assert!(!ppu.snapshot().lyc_coincidence);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF45, 0x12);

    assert_eq!(ppu.read_register(0xFF41), 0xC6);
    assert!(ppu.snapshot().lyc_coincidence);
    assert!(ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn stat_line_blocks_new_requests_while_an_enabled_source_keeps_it_high() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x62,
        scy: 0x00,
        scx: 0x00,
        ly: 0x21,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    assert!(ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF45, 0x21);

    assert!(ppu.snapshot().lyc_coincidence);
    assert!(ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
}

#[test]
fn dmg_mode2_enable_requests_lcd_stat_at_vblank_entry_only() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 143,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = DOTS_PER_SCANLINE - 1;
    ppu.refresh_stat_irq_line(false);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    tick_ppu(&mut ppu, 0, &oam_bytes);

    assert_eq!(ppu.snapshot().ly, 144);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);
    assert!(ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::VBlank, InterruptSource::LcdStat]
    );

    tick_ppu(&mut ppu, 1, &oam_bytes);

    assert_eq!(ppu.snapshot().ly, 144);
    assert_eq!(ppu.snapshot().line_dot, 1);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
}

#[test]
fn mode2_enable_alone_does_not_hold_stat_high_past_vblank_entry() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 144,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 8;
    ppu.refresh_stat_irq_line(false);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
}

#[test]
fn stat_write_quirk_requests_in_mode2_and_coincidence_but_not_plain_mode3() {
    let mut mode2 = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];

    mode2.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x20,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    assert!(drain_ppu_interrupts(&mut mode2).is_empty());

    mode2.write_register(0xFF41, 0x00);

    assert!(mode2.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut mode2),
        vec![InterruptSource::LcdStat]
    );

    let mut mode3 = Ppu::new(ConsoleModel::Dmg);
    mode3.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x20,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    for t_cycle in 0..80 {
        tick_ppu(&mut mode3, t_cycle, &oam_bytes);
    }
    assert_eq!(mode3.snapshot().mode, PpuAccessMode::Drawing);
    assert!(drain_ppu_interrupts(&mut mode3).is_empty());

    mode3.write_register(0xFF41, 0x00);

    assert!(!mode3.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut mode3).is_empty());

    let mut coincidence = Ppu::new(ConsoleModel::Dmg);
    coincidence.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    for t_cycle in 0..80 {
        tick_ppu(&mut coincidence, t_cycle, &oam_bytes);
    }
    assert_eq!(coincidence.snapshot().mode, PpuAccessMode::Drawing);
    assert!(drain_ppu_interrupts(&mut coincidence).is_empty());

    coincidence.write_register(0xFF41, 0x00);

    assert!(coincidence.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut coincidence),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn lyc_coincidence_tracks_vblank_lines_and_the_153_to_0_wrap() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 143,
        lyc: 144,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let mut t_cycle = tick_until_line_start(&mut ppu, 0, &oam_bytes, &[0; TEST_VRAM_BYTES], 144);
    assert_eq!(ppu.snapshot().ly, 144);
    assert!(ppu.snapshot().lyc_coincidence);

    ppu.write_register(0xFF45, 153);
    assert!(!ppu.snapshot().lyc_coincidence);

    t_cycle = tick_until_line_start(&mut ppu, t_cycle, &oam_bytes, &[0; TEST_VRAM_BYTES], 153);
    assert_eq!(ppu.snapshot().ly, 153);
    assert!(ppu.snapshot().lyc_coincidence);

    ppu.write_register(0xFF45, 0);
    assert!(!ppu.snapshot().lyc_coincidence);

    let _ = tick_until_line_start(&mut ppu, t_cycle, &oam_bytes, &[0; TEST_VRAM_BYTES], 0);
    assert_eq!(ppu.snapshot().ly, 0);
    assert!(ppu.snapshot().lyc_coincidence);
}

#[test]
fn ly_is_read_only_and_obj_palettes_keep_an_explicit_uninitialized_policy() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 0x22,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF44, 0x99);

    assert_eq!(ppu.read_register(0xFF44), 0x22);
    assert_eq!(ppu.read_register(0xFF48), 0xFF);
    assert_eq!(ppu.read_register(0xFF49), 0xFF);
}

#[test]
fn skip_boot_mode_latch_preserves_the_published_stat_mode_until_the_first_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(ppu.snapshot().mode_dot, 0);

    tick_ppu(&mut ppu, 0, &[0; 160]);

    assert_eq!(ppu.snapshot().ly, 0);
    assert_eq!(ppu.snapshot().line_dot, 1);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(ppu.snapshot().mode_dot, 1);
}

#[test]
fn visible_mode3_registers_lag_enabled_writes_until_the_next_t_cycle() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..80 {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
    }

    let before = ppu.snapshot();
    assert_eq!(before.mode, PpuAccessMode::Drawing);
    assert_eq!(before.visible_lcdc, 0x80);
    assert_eq!(before.visible_scy, 0x00);
    assert_eq!(before.visible_scx, 0x00);
    assert_eq!(before.visible_bgp, 0xFC);
    assert_eq!(before.visible_wy, 0x00);
    assert_eq!(before.visible_wx, 0x00);

    ppu.write_register(0xFF40, 0x91);
    ppu.write_register(0xFF42, 0x12);
    ppu.write_register(0xFF43, 0x34);
    ppu.write_register(0xFF47, 0x1B);
    ppu.write_register(0xFF4A, 0x56);
    ppu.write_register(0xFF4B, 0x78);

    let pending = ppu.snapshot();
    assert_eq!(pending.lcdc, 0x91);
    assert_eq!(pending.scy, 0x12);
    assert_eq!(pending.scx, 0x34);
    assert_eq!(pending.bgp, 0x1B);
    assert_eq!(pending.wy, 0x56);
    assert_eq!(pending.wx, 0x78);
    assert_eq!(pending.visible_lcdc, 0x80);
    assert_eq!(pending.visible_scy, 0x00);
    assert_eq!(pending.visible_scx, 0x00);
    assert_eq!(pending.visible_bgp, 0xFC);
    assert_eq!(pending.visible_wy, 0x00);
    assert_eq!(pending.visible_wx, 0x00);

    tick_ppu(&mut ppu, 80, &oam_bytes);

    let after = ppu.snapshot();
    assert_eq!(after.visible_lcdc, 0x91);
    assert_eq!(after.visible_scy, 0x12);
    assert_eq!(after.visible_scx, 0x34);
    assert_eq!(after.visible_bgp, 0x1B);
    assert_eq!(after.visible_wy, 0x56);
    assert_eq!(after.visible_wx, 0x78);
}

#[test]
fn tick_advances_the_raster_through_the_baseline_visible_line_modes() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..79 {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
    }

    assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(ppu.snapshot().line_dot, 79);
    assert_eq!(ppu.snapshot().mode_dot, 79);

    tick_ppu(&mut ppu, 79, &oam_bytes);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
    assert_eq!(ppu.snapshot().line_dot, 80);
    assert_eq!(ppu.snapshot().mode_dot, 0);

    for t_cycle in 80..251 {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
    }

    assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
    assert_eq!(ppu.snapshot().line_dot, 251);
    assert_eq!(ppu.snapshot().mode_dot, 171);

    tick_ppu(&mut ppu, 251, &oam_bytes);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::HBlank);
    assert_eq!(ppu.snapshot().line_dot, 252);
    assert_eq!(ppu.snapshot().mode_dot, 0);

    for t_cycle in 252..=455 {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
    }

    assert_eq!(ppu.snapshot().ly, 1);
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(ppu.snapshot().mode_dot, 0);
}

#[test]
fn lcd_disabled_state_freezes_the_raster_and_forces_blank_output() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x44,
        lyc: 0x12,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..32 {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
    }

    ppu.write_register(0xFF40, 0x00);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.lcd_state, PpuLcdState::Disabled);
    assert_eq!(snapshot.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!snapshot.blank_frame_active);
    assert_eq!(snapshot.ly, 0x00);
    assert_eq!(snapshot.line_dot, 0);
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert_eq!(ppu.bus_state(), PpuBusState::lcd_disabled());
}

#[test]
fn lcd_disable_resets_the_live_pipeline_and_reenable_starts_with_mode0_readback() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 8, 0);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x00, 0xFF);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..100 {
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    }

    let drawing = ppu.snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert!(!drawing.bg_fifo_pixels.is_empty());

    ppu.write_register(0xFF40, 0x00);

    let disabled = ppu.snapshot();
    assert_eq!(disabled.lcd_state, PpuLcdState::Disabled);
    assert_eq!(disabled.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!disabled.blank_frame_active);
    assert_eq!(disabled.ly, 0);
    assert_eq!(disabled.line_dot, 0);
    assert!(disabled.bg_fifo_pixels.is_empty());
    assert!(disabled.obj_fifo_pixels.is_empty());
    assert!(disabled.selected_sprites.is_empty());
    assert_eq!(disabled.mode2_scanned_entries, 0);
    assert_eq!(disabled.window_line_counter, 0);

    ppu.write_register(0xFF40, 0x82);

    let reenabled = ppu.snapshot();
    assert_eq!(reenabled.lcd_state, PpuLcdState::Enabled);
    assert_eq!(reenabled.mode, PpuAccessMode::HBlank);
    assert_eq!(reenabled.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(reenabled.blank_frame_active);
    assert_eq!(reenabled.ly, 0);
    assert_eq!(reenabled.line_dot, LCD_REENABLE_INITIAL_LINE_DOT);
    assert_eq!(reenabled.mode_dot, 0);
    assert!(reenabled.bg_fifo_pixels.is_empty());
    assert!(reenabled.obj_fifo_pixels.is_empty());
    assert!(reenabled.selected_sprites.is_empty());
    assert_eq!(reenabled.mode2_scanned_entries, 0);
}

#[test]
fn lcd_reenable_startup_window_keeps_mode2_idle_until_the_ordinary_raster_resumes() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x00,
        stat: 0x80,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF40, 0x82);

    let restart = ppu.snapshot();
    assert_eq!(restart.mode, PpuAccessMode::HBlank);
    assert_eq!(restart.line_dot, LCD_REENABLE_INITIAL_LINE_DOT);
    assert_eq!(restart.mode_dot, 0);
    assert_eq!(restart.mode2_scanned_entries, 0);

    for t_cycle in 0..15 {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
    }

    let startup_window_end = ppu.snapshot();
    assert_eq!(startup_window_end.line_dot, 19);
    assert_eq!(startup_window_end.mode, PpuAccessMode::HBlank);
    assert_eq!(startup_window_end.mode_dot, 15);
    assert_eq!(startup_window_end.mode2_scanned_entries, 0);

    tick_ppu(&mut ppu, 15, &oam_bytes);

    let first_mode2_dot = ppu.snapshot();
    assert_eq!(first_mode2_dot.line_dot, 20);
    assert_eq!(first_mode2_dot.mode, PpuAccessMode::OamScan);
    assert_eq!(first_mode2_dot.mode_dot, 20);
    assert_eq!(first_mode2_dot.mode2_scanned_entries, 1);
}

#[test]
fn lcd_off_retains_the_lyc_bit_and_ignores_lyc_writes_until_lcd_restarts() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x40,
        scy: 0x00,
        scx: 0x00,
        ly: 0x90,
        lyc: 0x90,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF40, 0x00);
    assert_eq!(ppu.read_register(0xFF41), 0xC4);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF45, 0x01);
    assert_eq!(ppu.read_register(0xFF41), 0xC4);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF40, 0x80);
    assert_eq!(ppu.read_register(0xFF41), 0xC0);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
}

#[test]
fn lcd_reenable_requests_lcd_stat_only_when_the_retained_lyc_result_rises() {
    let mut unchanged_true = Ppu::new(ConsoleModel::Dmg);
    unchanged_true.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x40,
        scy: 0x00,
        scx: 0x00,
        ly: 0x90,
        lyc: 0x90,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    unchanged_true.write_register(0xFF40, 0x00);
    unchanged_true.write_register(0xFF45, 0x00);
    drain_ppu_interrupts(&mut unchanged_true);

    unchanged_true.write_register(0xFF40, 0x80);

    assert_eq!(unchanged_true.read_register(0xFF41), 0xC4);
    assert!(drain_ppu_interrupts(&mut unchanged_true).is_empty());

    let mut rising = Ppu::new(ConsoleModel::Dmg);
    rising.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x40,
        scy: 0x00,
        scx: 0x00,
        ly: 0x90,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    rising.write_register(0xFF40, 0x00);
    assert_eq!(rising.read_register(0xFF41), 0xC0);
    drain_ppu_interrupts(&mut rising);

    rising.write_register(0xFF40, 0x80);

    assert_eq!(rising.read_register(0xFF41), 0xC4);
    assert_eq!(
        drain_ppu_interrupts(&mut rising),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn first_frame_after_lcd_reenable_stays_visibly_blank_while_the_raster_runs() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x00, 0xFF);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x00,
        stat: 0x80,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF40, 0x91);

    let mut t_cycle = 0;
    while ppu.snapshot().mode == PpuAccessMode::HBlank {
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        t_cycle += 1;
        assert!(t_cycle < DOTS_PER_SCANLINE as u64);
    }

    t_cycle = tick_until_hblank(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    let first_blank_line = ppu.snapshot();
    assert_eq!(
        first_blank_line.visible_output,
        PpuVisibleOutputState::ForcedBlank
    );
    assert!(first_blank_line.blank_frame_active);
    assert_eq!(first_blank_line.visible_pixels_output, 160);
    assert_eq!(&first_blank_line.current_scanline_pixels[..8], &[0; 8]);

    t_cycle = tick_until_next_frame_start(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    let second_frame_start = ppu.snapshot();
    assert_eq!(second_frame_start.ly, 0);
    assert_eq!(second_frame_start.line_dot, 0);
    assert_eq!(
        second_frame_start.visible_output,
        PpuVisibleOutputState::Driving
    );
    assert!(!second_frame_start.blank_frame_active);

    let _ = tick_until_hblank(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    let visible_line = ppu.snapshot();
    assert_eq!(visible_line.visible_output, PpuVisibleOutputState::Driving);
    assert_eq!(&visible_line.current_scanline_pixels[..8], &[2; 8]);
}

#[test]
fn mode2_scans_oam_in_order_and_caps_the_selected_list_at_ten_entries() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];

    for index in 0..12 {
        let x = match index {
            0 => 0,
            1 => 168,
            _ => 8 + index,
        };
        write_oam_entry(&mut oam_bytes, index, 16, x, 0x20 + index);
    }
    write_oam_entry(&mut oam_bytes, 20, 8, 32, 0x99);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..80 {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
    }

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 40);
    assert_eq!(snapshot.selected_sprites.len(), 10);
    assert_eq!(
        snapshot
            .selected_sprites
            .iter()
            .map(|sprite| sprite.oam_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    assert_eq!(snapshot.selected_sprites[0].x, 0);
    assert_eq!(snapshot.selected_sprites[1].x, 168);
}

#[test]
fn dmg_mode2_oam_dma_reuses_the_last_latched_oam_word_for_selection() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];

    write_oam_entry(&mut oam_bytes, 0, 16, 24, 0x20);
    write_oam_entry(&mut oam_bytes, 1, 0, 0, 0x21);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    tick_ppu(&mut ppu, 0, &oam_bytes);
    tick_ppu(&mut ppu, 1, &oam_bytes);
    tick_ppu_with_vram_and_dma(&mut ppu, 2, &oam_bytes, &[0; TEST_VRAM_BYTES], true, None);
    tick_ppu_with_vram_and_dma(&mut ppu, 3, &oam_bytes, &[0; TEST_VRAM_BYTES], true, None);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 2);
    assert_eq!(snapshot.selected_sprites.len(), 2);
    assert_eq!(snapshot.selected_sprites[1].oam_index, 1);
    assert_eq!(snapshot.selected_sprites[1].y, 16);
    assert_eq!(snapshot.selected_sprites[1].x, 24);
}

#[test]
fn mode2_scanline_reset_preserves_the_latched_oam_word_for_dma_blocked_reads() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];

    write_oam_entry(&mut oam_bytes, 0, 0, 0, 0x20);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.mode2_scan_state.latch_oam_word(16, 79);
    ppu.mode2_scan_state.reset_scanline();

    tick_ppu_with_vram_and_dma(&mut ppu, 0, &oam_bytes, &[0; TEST_VRAM_BYTES], true, None);
    tick_ppu_with_vram_and_dma(&mut ppu, 1, &oam_bytes, &[0; TEST_VRAM_BYTES], true, None);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 1);
    assert_eq!(snapshot.selected_sprites.len(), 1);
    assert_eq!(snapshot.selected_sprites[0].oam_index, 0);
    assert_eq!(snapshot.selected_sprites[0].y, 16);
    assert_eq!(snapshot.selected_sprites[0].x, 79);
}

#[test]
fn mode2_uses_the_live_lcdc2_size_when_each_oam_entry_is_scanned() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];

    write_oam_entry(&mut oam_bytes, 0, 0, 24, 0x10);
    write_oam_entry(&mut oam_bytes, 1, 1, 32, 0x11);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    tick_ppu(&mut ppu, 0, &oam_bytes);
    tick_ppu(&mut ppu, 1, &oam_bytes);
    assert!(ppu.snapshot().selected_sprites.is_empty());

    ppu.write_register(0xFF40, 0x84);

    tick_ppu(&mut ppu, 2, &oam_bytes);
    tick_ppu(&mut ppu, 3, &oam_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 2);
    assert_eq!(snapshot.selected_sprites.len(), 1);
    assert_eq!(snapshot.selected_sprites[0].oam_index, 1);
    assert_eq!(snapshot.selected_sprites[0].y, 1);
}

#[test]
fn mode3_startup_keeps_dummy_occupancy_out_of_the_fifo_until_alignment_push() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..80 {
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    }

    let drawing_start = ppu.snapshot();
    assert_eq!(drawing_start.mode, PpuAccessMode::Drawing);
    assert_eq!(drawing_start.line_dot, 80);
    assert_eq!(drawing_start.mode_dot, 0);
    assert_eq!(drawing_start.mode0_start_dot, 252);
    assert_eq!(drawing_start.bg_fetcher_stage, PpuBgFetcherStage::TileIndex);
    assert_eq!(drawing_start.bg_fetcher_stage_dot, 1);
    assert!(drawing_start.bg_fifo_pixels.is_empty());
    assert_eq!(drawing_start.visible_pixels_output, 0);

    for t_cycle in 80..87 {
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    }

    let after_first_push = ppu.snapshot();
    assert_eq!(after_first_push.line_dot, 87);
    assert_eq!(
        after_first_push.bg_fetcher_stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(after_first_push.bg_fetcher_stage_dot, 1);
    assert_eq!(
        after_first_push.bg_fifo_pixels,
        vec![0, 0, 0, 0, 0, 1, 2, 3, 0, 1, 2, 3]
    );
    assert!(!after_first_push.bg_push_pending);
    assert!(!after_first_push.bg_fill_pending);
    assert_eq!(after_first_push.visible_pixels_output, 0);

    for t_cycle in 87..110 {
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        if ppu.snapshot().visible_pixels_output == 1 {
            break;
        }
    }

    let first_visible = ppu.snapshot();
    assert_eq!(first_visible.visible_pixels_output, 1);
    assert!(first_visible.line_dot >= 92);
    assert_eq!(first_visible.current_scanline_pixels[0], 0);
}

#[test]
fn mode3_startup_fetches_the_first_three_visible_background_tiles_in_order() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x00, 0x00);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xFF, 0x00);
    write_bg_tile_row(&mut vram_bytes, 2, 0, 0x00, 0xFF);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 1);
    write_bg_tilemap_entry(&mut vram_bytes, 2, 0, 2);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..140 {
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        if ppu.snapshot().visible_pixels_output == 24 {
            break;
        }
    }

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.visible_pixels_output, 24);
    assert_eq!(snapshot.current_scanline_pixels[..8], [0; 8]);
    assert_eq!(snapshot.current_scanline_pixels[8..16], [1; 8]);
    assert_eq!(snapshot.current_scanline_pixels[16..24], [2; 8]);
}

#[test]
fn startup_post_alignment_seam_labels_only_the_second_and_third_visible_tiles() {
    let mut pipeline = BgPipelineState::default();

    pipeline.begin_post_alignment_followup();
    assert_eq!(
        pipeline
            .peek_startup_background_fetch_origin()
            .startup_continuation_slice(),
        BgStartupContinuationSlice::VisibleTile2
    );
    pipeline.advance_startup_background_fetch_tile();

    assert_eq!(
        pipeline
            .peek_startup_background_fetch_origin()
            .startup_continuation_slice(),
        BgStartupContinuationSlice::VisibleTile3
    );
    pipeline.advance_startup_background_fetch_tile();

    assert_eq!(
        pipeline.peek_startup_background_fetch_origin(),
        BgCachedSliceOrigin::Ordinary
    );
    assert_eq!(
        pipeline.startup_fetch_seam,
        BgStartupFetchSeamState::Inactive
    );
}

#[test]
fn bg_push_waits_for_fifo_space_without_losing_the_fetched_tile() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.fetch_x = 0;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
    ppu.bg_pipeline_state.fifo = (0..=8).collect();

    let result = ppu.advance_bg_push();

    assert_eq!(result, BgPushDotResult::WaitingForEmptyFifo);
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 0);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 9);

    for _ in 0..9 {
        let _ = ppu.bg_pipeline_state.fifo.pop_front();
    }
    let result = ppu.advance_bg_push();

    assert_eq!(result, BgPushDotResult::QueuedFill);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 8);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 0);

    ppu.flush_pending_bg_fifo_fill();

    assert!(!ppu.bg_pipeline_state.fill.pending);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 8);
    assert_eq!(
        ppu.bg_pipeline_state
            .fifo
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 0, 1, 2, 3]
    );
}

#[test]
fn first_window_tile_skips_the_normal_push_entry_delay() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; TEST_VRAM_BYTES]);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0xB1;
    ppu.bg_pipeline_state.fetcher.start_window(8);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x55;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x33;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(ppu.bg_pipeline_state.push.just_activated_window_tile);
    assert!(
        !ppu.bg_pipeline_state
            .fetcher
            .first_window_tile_after_activation
    );
}

#[test]
fn first_window_tile_push_ignores_pending_obj_fetch_start() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0xA3;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.push.just_activated_window_tile = true;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFill
    );
}

#[test]
fn current_bg_push_dot_ownership_distinguishes_fill_wait_and_obj_handoff_paths() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFill
    );

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFillThenObjectFetch
    );

    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.push_back(0);
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFillThenObjectFetch
    );

    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::FifoBackedTransferObjectFetch
    );

    ppu.obj_pipeline_state.clear_pending_fetch_hits();
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::WaitingForEmptyFifo
    );

    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::EntryDelay
    );

    ppu.bg_pipeline_state.push.pending = false;
    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::NotReady
    );
}

#[test]
fn startup_dummy_fifo_pixels_do_not_block_the_first_real_bg_fill() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend([0, 0, 0, 0]);

    assert_eq!(
        ppu.current_bg_push_dot_ownership(),
        BgPushDotOwnership::QueueFill
    );
}

#[test]
fn latching_object_hits_queues_all_matching_sprite_slots_once() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 8,
        tile_index: 1,
        attributes: 0,
    });

    ppu.latch_object_fetch_hits();
    ppu.latch_object_fetch_hits();

    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn bg_push_can_handoff_to_a_latched_object_fetch_without_losing_the_tile() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
    ppu.bg_pipeline_state.fifo.push_back(0);

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    assert_eq!(
        ppu.advance_bg_push(),
        BgPushDotResult::HandedOffToObjectFetch
    );
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.push.disposition,
        BgPushDisposition::InterruptedByObjectFetch
    );
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::Startup
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 1);
    assert!(!ppu.bg_pipeline_state.fill.pending);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
}

#[test]
fn bg_push_with_an_empty_fifo_can_queue_fill_and_start_object_fetch_on_the_same_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    assert_eq!(
        ppu.advance_bg_push(),
        BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
    );
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::Startup
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn bg_push_stage_waits_one_dot_on_entry_then_retries_every_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.fetch_x = 0;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;
    ppu.bg_pipeline_state.fifo = (0..=8).collect();

    assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::EntryDelay);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 9);

    assert_eq!(
        ppu.advance_bg_push_stage(),
        BgPushDotResult::WaitingForEmptyFifo
    );
    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 9);

    for _ in 0..9 {
        let _ = ppu.bg_pipeline_state.fifo.pop_front();
    }
    assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::QueuedFill);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 8);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 0);

    ppu.flush_pending_bg_fifo_fill();

    assert!(!ppu.bg_pipeline_state.fill.pending);
    assert_eq!(ppu.bg_pipeline_state.fifo.len(), 8);
}

#[test]
fn bg_push_queues_fifo_fill_before_the_fill_phase_materializes_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x33;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 8;

    assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::QueuedFill);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());

    ppu.flush_pending_bg_fifo_fill();

    assert!(!ppu.bg_pipeline_state.fill.pending);
    assert_eq!(
        ppu.bg_pipeline_state
            .fifo
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 0, 1, 2, 3]
    );
}

#[test]
fn bg_push_stage_reports_not_ready_when_no_cached_slice_is_pending() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    assert_eq!(ppu.advance_bg_push_stage(), BgPushDotResult::NotReady);
}

#[test]
fn current_dot_arbitration_distinguishes_fifo_backed_and_queued_fill_obj_start() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    let empty_fifo = ppu.current_dot_arbitration();
    assert!(!empty_fifo.can_serve_bg_transfer());
    assert!(!empty_fifo.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
    assert!(empty_fifo.can_start_obj_fetch(ObjFetchStartSource::QueuedBgFill));

    ppu.bg_pipeline_state.fifo.push_back(0);

    let fifo_backed = ppu.current_dot_arbitration();
    assert!(!fifo_backed.can_serve_bg_transfer());
    assert!(fifo_backed.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
    assert!(fifo_backed.can_start_obj_fetch(ObjFetchStartSource::QueuedBgFill));
}

#[test]
fn current_transfer_snapshot_keeps_context_and_readiness_together() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;

    let waiting = ppu
        .current_transfer()
        .expect("hidden startup dot must have transfer state");
    assert_eq!(
        waiting.context,
        Mode3TransferContext {
            lane: Mode3TransferLane::Hidden,
            source_window: Mode3TransferSourceWindow::FifoBacked,
        }
    );
    assert_eq!(
        waiting.readiness,
        Mode3TransferReadiness::WaitingForFifo(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );

    ppu.bg_pipeline_state.fifo.push_back(0);

    let ready = ppu
        .current_transfer()
        .expect("same hidden dot must stay describable");
    assert_eq!(ready.context, waiting.context);
    assert_eq!(ready.service_plan(), waiting.service_plan());
    assert_eq!(
        ready.readiness,
        Mode3TransferReadiness::Ready(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );
}

#[test]
fn transfer_service_plan_distinguishes_abstract_hidden_and_fifo_backed_visible_paths() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.fifo.push_back(0);

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::Abstract,
        })
    );

    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );

    ppu.bg_pipeline_state.current_transfer_x = 8;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedVisiblePixel,
            execution: Mode3TransferServiceExecution::EmitVisiblePixel,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );
}

#[test]
fn fifo_backed_obj_start_requires_a_fifo_backed_transfer_dot_not_just_a_nonempty_fifo() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 15,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    let arbitration = ppu.current_dot_arbitration();
    assert!(!arbitration.can_serve_bg_transfer());
    assert!(!arbitration.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
    assert!(arbitration.can_start_obj_fetch(ObjFetchStartSource::QueuedBgFill));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
}

#[test]
fn fifo_backed_obj_start_waits_until_bg_fetcher_leaves_tile_data_low() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    let tile_index = ppu.current_dot_arbitration();
    assert!(!tile_index.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    let tile_data_low = ppu.current_dot_arbitration();
    assert!(!tile_data_low.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));

    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    let tile_data_high = ppu.current_dot_arbitration();
    assert!(tile_data_high.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
}

#[test]
fn abstract_startup_service_kind_tracks_served_progress_not_raw_mode3_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS - 2;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
            execution: Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop,
            backing: Mode3TransferBacking::Abstract,
        })
    );

    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::Abstract,
        })
    );
}

#[test]
fn obj_hit_ownership_tracks_served_startup_progress_not_raw_mode3_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.line_dot = MODE2_DOTS + MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS - 2;

    assert_eq!(
        ppu.current_obj_hit_ownership().phase,
        ObjHitPhase::PreVisible
    );

    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;

    assert_eq!(ppu.current_obj_hit_ownership().phase, ObjHitPhase::Hidden);
}

#[test]
fn pending_obj_hit_blocks_output_phase_and_stretches_mode3() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 20;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.visible_pixels_output = 12;
    ppu.bg_pipeline_state.fifo.push_back(3);
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::NotServed);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 12);
    assert_eq!(
        ppu.bg_pipeline_state
            .fifo
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![3]
    );
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
}

#[test]
fn pending_obj_hit_stalls_pre_visible_match_x_until_fetch_service() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 5;
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    ppu.advance_mode3_output_phase();

    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
}

#[test]
fn hidden_startup_dot_advances_pre_visible_match_x_without_bg_fifo_pop() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.current_transfer_x = 5;

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedPreVisibleTransfer);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 6);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn mode3_started_uses_explicit_startup_entry_delay_before_transfer_service() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + 1;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = MODE3_ABSTRACT_SOURCE_WINDOW_DOTS;
    ppu.bg_pipeline_state.startup_source_state =
        Mode3StartupSourceState::EntryDelay { remaining: 2 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 5;

    let first = ppu.advance_mode3_output_phase();
    assert_eq!(first.kind, Mode3TransferDotKind::NotServed);
    assert_eq!(
        ppu.bg_pipeline_state.startup_source_state,
        Mode3StartupSourceState::EntryDelay { remaining: 1 }
    );
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);

    let second = ppu.advance_mode3_output_phase();
    assert_eq!(second.kind, Mode3TransferDotKind::NotServed);
    assert_eq!(
        ppu.bg_pipeline_state.startup_source_state,
        Mode3StartupSourceState::Abstract {
            remaining: MODE3_ABSTRACT_SOURCE_WINDOW_DOTS
        }
    );
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);

    let third = ppu.advance_mode3_output_phase();
    assert_eq!(third.kind, Mode3TransferDotKind::ServedPreVisibleTransfer);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 6);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
}

#[test]
fn mode3_started_keeps_an_explicit_abstract_source_window_before_fifo_backed_transfer() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 1 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 5;

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
            execution: Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop,
            backing: Mode3TransferBacking::Abstract,
        })
    );

    let transfer_dot = ppu.advance_mode3_output_phase();
    assert_eq!(
        transfer_dot.kind,
        Mode3TransferDotKind::ServedPreVisibleTransfer
    );
    assert_eq!(
        ppu.bg_pipeline_state.startup_source_state,
        Mode3StartupSourceState::FifoBacked
    );

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
            execution: Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );
}

#[test]
fn mode3_started_keeps_an_explicit_previsible_lane_before_hidden_transfer() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 1;
    ppu.bg_pipeline_state.current_transfer_x = 5;
    ppu.bg_pipeline_state.fifo.push_back(0);

    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedPreVisibleTransfer,
            execution: Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );

    let transfer_dot = ppu.advance_mode3_output_phase();
    assert_eq!(
        transfer_dot.kind,
        Mode3TransferDotKind::ServedPreVisibleTransfer
    );
    assert_eq!(
        ppu.bg_pipeline_state
            .startup_pre_visible_transfer_dots_remaining,
        0
    );

    ppu.bg_pipeline_state.fifo.push_back(0);
    assert_eq!(
        ppu.current_transfer_service_plan(),
        Some(Mode3TransferServicePlan {
            result_kind: Mode3TransferDotKind::ServedHiddenTransfer,
            execution: Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop,
            backing: Mode3TransferBacking::FifoBacked,
        })
    );
}

#[test]
fn bg_fifo_starvation_after_priming_does_not_advance_pre_visible_match_x() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 5;

    ppu.advance_mode3_output_phase();

    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 5);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
}

#[test]
fn abstract_previsible_scx_discard_keeps_lx_zero_until_hidden_transfer_begins() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedPreVisibleTransfer);
    assert!(result.consumed_scx_discard);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
    assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(
        ppu.bg_pipeline_state.transfer_phase,
        Mode3TransferPhase::Output
    );
}

#[test]
fn fifo_backed_hidden_service_moves_transfer_phase_to_output() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
    ppu.bg_pipeline_state.fifo.push_back(0);

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedHiddenTransfer);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 8);
    assert_eq!(
        ppu.bg_pipeline_state.transfer_phase,
        Mode3TransferPhase::Output
    );
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn late_hidden_dot_can_consume_a_startup_placeholder_before_the_first_real_fill() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedHiddenTransfer);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 8);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn late_hidden_scx_discard_can_consume_a_startup_placeholder_before_real_fifo_backing() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;

    ppu.advance_mode3_output_phase();

    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
    assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 0);
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
}

#[test]
fn wx_zero_previsible_window_start_requires_a_late_fifo_backed_served_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 0;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;

    let not_ready_dot = ppu.advance_mode3_output_phase();
    assert_eq!(not_ready_dot.kind, Mode3TransferDotKind::NotServed);
    assert!(!ppu.maybe_start_window_after_transfer_dot(not_ready_dot));

    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;

    let ready_dot = ppu.advance_mode3_output_phase();

    assert_eq!(ready_dot.kind, Mode3TransferDotKind::ServedHiddenTransfer);
    assert!(ppu.maybe_start_window_after_transfer_dot(ready_dot));
    assert!(ppu.bg_pipeline_state.window_started_this_line);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Window
    );
}

#[test]
fn wx_zero_last_scx_discard_shortening_is_applied_from_the_served_transfer_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 0;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT + 2;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 3;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.initial_scx_discard = 3;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    let transfer_dot = ppu.advance_mode3_output_phase();
    ppu.maybe_apply_wx0_shortening_after_transfer_dot(transfer_dot);

    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 2);
    assert!(!ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn wx_seven_starts_window_from_the_first_served_x0_transfer_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 7;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;

    let transfer_dot = ppu.advance_mode3_output_phase();

    assert_eq!(
        transfer_dot.kind,
        Mode3TransferDotKind::ServedHiddenTransfer
    );
    assert!(ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert!(ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn dmg_window_trigger_uses_the_previous_dot_wx_snapshot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0xF1;
    ppu.visible_registers.wx = 8;
    ppu.pipeline_registers.lcdc = 0xF1;
    ppu.pipeline_registers.wx = 7;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;

    let transfer_dot = ppu.advance_mode3_output_phase();

    assert_eq!(
        transfer_dot.kind,
        Mode3TransferDotKind::ServedHiddenTransfer
    );
    assert!(ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert!(ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn pending_obj_hit_blocks_window_start_because_the_output_dot_is_not_served() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x93;
    ppu.visible_registers.wx = 15;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.window_wy_latch = true;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.visible_pixels_output = 8;
    ppu.bg_pipeline_state.fifo.push_back(1);
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    let transfer_dot = ppu.advance_mode3_output_phase();

    assert_eq!(transfer_dot.kind, Mode3TransferDotKind::NotServed);
    assert!(!ppu.maybe_start_window_after_transfer_dot(transfer_dot));
    assert!(!ppu.bg_pipeline_state.window_started_this_line);
}

#[test]
fn bg_fifo_discard_after_priming_keeps_lx_zero_until_discard_finishes() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedHiddenTransfer);
    assert!(result.consumed_scx_discard);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 0);
    assert_eq!(ppu.bg_pipeline_state.scx_discard_remaining, 0);
    assert_eq!(
        ppu.bg_pipeline_state.transfer_phase,
        Mode3TransferPhase::Output
    );
}

#[test]
fn visible_bg_pixel_output_reports_a_visible_pixel_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x91;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(2);

    let result = ppu.advance_mode3_output_phase();

    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.bg_pipeline_state.current_transfer_x, 9);
    assert_eq!(ppu.bg_pipeline_state.visible_pixels_output, 1);
}

#[test]
fn current_obj_hit_ownership_tracks_x_and_dot_phase() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Priming;
    ppu.bg_pipeline_state.current_transfer_x = 6;
    assert_eq!(
        ppu.current_obj_hit_ownership(),
        ObjHitOwnership {
            match_x: 6,
            phase: ObjHitPhase::PreVisible,
        }
    );

    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;
    assert_eq!(
        ppu.current_obj_hit_ownership(),
        ObjHitOwnership {
            match_x: 0,
            phase: ObjHitPhase::Hidden,
        }
    );

    ppu.bg_pipeline_state.scx_discard_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 20;
    ppu.bg_pipeline_state.visible_pixels_output = 12;
    assert_eq!(
        ppu.current_obj_hit_ownership(),
        ObjHitOwnership {
            match_x: 20,
            phase: ObjHitPhase::Visible,
        }
    );
}

#[test]
fn stale_pending_obj_hit_is_cleared_once_current_x_moves_on() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.bg_pipeline_state.current_transfer_x = 13;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.visible_pixels_output = 5;
    ppu.obj_pipeline_state.queue_fetch_hit(
        0,
        ObjHitOwnership {
            match_x: 12,
            phase: ObjHitPhase::Visible,
        },
    );

    ppu.sync_pending_obj_hit_ownership();

    assert!(ppu.obj_pipeline_state.pending_sprite_slots.is_empty());
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, None);
}

#[test]
fn pending_obj_hit_survives_dot_phase_changes_while_current_x_is_still_the_same() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 6;
    ppu.bg_pipeline_state.scx_discard_remaining = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.obj_pipeline_state.queue_fetch_hit(
        0,
        ObjHitOwnership {
            match_x: 6,
            phase: ObjHitPhase::PreVisible,
        },
    );

    ppu.sync_pending_obj_hit_ownership();

    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(6));
}

#[test]
fn bg_fetcher_stage_dot_is_an_explicit_one_dot_automaton() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.bg_pipeline_state.fetcher.start_background();

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataLow
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataLow
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage, PpuBgFetcherStage::Push);
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);
    assert!(ppu.bg_pipeline_state.push.pending);
    assert!(!ppu.bg_pipeline_state.fill.pending);
}

#[test]
fn bg_fetcher_records_the_tilemap_address_for_the_current_phase() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    vram_bytes[0x1C64] = 0x66;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x99;
    ppu.visible_registers.scx = 24;
    ppu.visible_registers.scy = 16;
    ppu.ly = 8;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 8;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C64);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x66);
}

#[test]
fn window_fetcher_aborts_to_background_and_restores_bg_progress_when_win_enable_turns_off() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 0x11);
    write_window_tilemap_entry(&mut vram_bytes, 0, 0, 0x22);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 0;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 0;
    ppu.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = 8;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Background
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.next_fetch_pixel, 8);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x11);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1801);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 1);
}

#[test]
fn first_window_tile_index_dot_rewinds_bg_resume_progress_by_one_tile() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 0x11);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.bg_pipeline_state.fetcher.start_window(8);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.source,
        PpuBgFetcherSource::Background
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.bg_resume_fetch_pixel, 0);
}

#[test]
fn window_fetcher_advances_tilemap_x_on_tile_index_dot_zero() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_window_tilemap_entry(&mut vram_bytes, 0, 0, 0x11);
    write_window_tilemap_entry(&mut vram_bytes, 1, 0, 0x22);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0xE1;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.bg_pipeline_state.fetcher.start_window(8);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C00);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x11);
    assert_eq!(ppu.bg_pipeline_state.fetcher.window_tilemap_x, 1);

    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C01);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 0x22);
    assert_eq!(ppu.bg_pipeline_state.fetcher.window_tilemap_x, 2);
}

#[test]
fn bg_fetcher_recomputes_scy_for_each_tile_data_plane_read_on_dmg() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x12, 0x34);
    write_bg_tile_row(&mut vram_bytes, 0, 1, 0x56, 0x78);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.visible_registers.scy = 0;
    ppu.pipeline_registers = ppu.visible_registers;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0000);

    ppu.visible_registers.scy = 1;
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x78);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0003);
}

#[test]
fn bg_fetcher_recomputes_tile_data_address_when_tile_selector_changes_between_planes() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 1, 0, 0x12, 0x34);
    vram_bytes[0x1011] = 0xAB;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.visible_registers.scy = 0;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0010);

    ppu.visible_registers.lcdc = 0x81;
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1011);
}

#[test]
fn cached_background_push_recomputes_tilemap_and_tiledata_on_push_dot_zero_map_change() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 0);
    write_window_tilemap_entry(&mut vram_bytes, 1, 0, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x12, 0x34);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xAB, 0xCD);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_map_address, 0x1C01);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_index, 1);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0011);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0xCD);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.push.cached.tile_high, 0xCD);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
}

#[test]
fn cached_background_push_accepts_same_tcycle_tilemap_refetch_after_entry_delay_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 0);
    write_window_tilemap_entry(&mut vram_bytes, 1, 0, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x12, 0x34);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xAB, 0xCD);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.fetch_x = 8;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = 16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = 16;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::EntryDelay);
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );

    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0xCD);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(
        !ppu.bg_pipeline_state
            .push
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
}

#[test]
fn cached_background_fill_recomputes_tilemap_before_the_next_flush_when_same_tcycle_window_is_open()
{
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 0);
    write_window_tilemap_entry(&mut vram_bytes, 1, 0, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x12, 0x34);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xAB, 0xCD);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 0;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;
    ppu.bg_pipeline_state
        .fill
        .cached
        .same_cycle_live_tilemap_refetch_window_open = true;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.fill.cached.needs_live_tilemap_refetch);

    ppu.maybe_recompute_pending_background_fill(&VramBusView::new(BusMaster::Ppu, &mut vram));
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_map_address, 0x1C01);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_index, 1);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_data_address, 0x0011);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0xCD);
}

#[test]
fn third_startup_continuation_fill_marks_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fill.cached.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;

    ppu.write_register(0xFF40, 0x99);
    assert!(ppu.bg_pipeline_state.fill.cached.needs_live_tilemap_refetch);
}

#[test]
fn ordinary_cached_fill_ignores_lcdc3_write_without_the_narrow_live_window() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1803;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;

    ppu.write_register(0xFF40, 0x99);
    assert!(!ppu.bg_pipeline_state.fill.cached.needs_live_tilemap_refetch);
}

#[test]
fn queue_from_push_preserves_the_same_tcycle_tilemap_refetch_window() {
    let mut fill = BgFifoFillState::default();
    let mut push = BgPushState {
        pending: true,
        ..BgPushState::default()
    };
    push.cached.source = PpuBgFetcherSource::Background;
    push.cached.same_cycle_live_tilemap_refetch_window_open = true;

    fill.queue_from_push(push);

    assert!(fill.cached.same_cycle_live_tilemap_refetch_window_open);
}

#[test]
fn cached_background_fill_recomputes_tiledata_before_the_next_flush() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 0);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x12, 0x34);
    write_bg_tile_row(&mut vram_bytes, 0, 1, 0xAB, 0xCD);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 0;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.fill.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fill.cached.tile_index = 0;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x34;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    ppu.write_register(0xFF42, 0x01);
    assert!(
        ppu.bg_pipeline_state
            .fill
            .cached
            .needs_live_tile_data_refetch
    );

    ppu.maybe_recompute_pending_background_fill(&VramBusView::new(BusMaster::Ppu, &mut vram));
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_data_address, 0x0003);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_low, 0xAB);
    assert_eq!(ppu.bg_pipeline_state.fill.cached.tile_high, 0xCD);
}

#[test]
fn bg_fetcher_uses_last_unsigned_fetch_data_when_tile_selector_flips_to_unsigned_on_low1() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    vram_bytes[0x1010] = 0x12;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x81;
    ppu.pipeline_registers.lcdc = 0x81;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;
    ppu.last_unsigned_tile_data_fetch = 0xCD;
    ppu.last_unsigned_tile_data_low_fetch = 0xCD;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1010);

    ppu.pipeline_registers.lcdc = 0x81;
    ppu.visible_registers.lcdc = 0x91;
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0xCD);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
}

#[test]
fn window_fetcher_uses_last_unsigned_fetch_data_when_tile_selector_flips_to_unsigned_on_high1() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    vram_bytes[0x1011] = 0x34;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x81;
    ppu.pipeline_registers.lcdc = 0x81;
    ppu.window_state.window_line_counter = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;
    ppu.last_unsigned_tile_data_fetch = 0xEF;
    ppu.last_unsigned_tile_data_high_fetch = 0xEF;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x34);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1011);

    ppu.pipeline_registers.lcdc = 0x81;
    ppu.visible_registers.lcdc = 0x91;
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0xEF);
    assert!(ppu.bg_pipeline_state.push.pending);
}

#[test]
fn mode3_scx_discard_shifts_visible_pixels_and_delays_hblank_entry() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xAA, 0xCC);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x03,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..252 {
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    }

    let extended_drawing = ppu.snapshot();
    assert_eq!(extended_drawing.line_dot, 252);
    assert_eq!(extended_drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(extended_drawing.mode0_start_dot, 255);

    for t_cycle in 252..255 {
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    }

    let hblank = ppu.snapshot();
    assert_eq!(hblank.line_dot, 255);
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode_dot, 0);
    assert_eq!(hblank.visible_pixels_output, 160);
    assert_eq!(
        &hblank.current_scanline_pixels[..8],
        &[3, 0, 1, 2, 3, 3, 2, 1]
    );
}

#[test]
fn window_start_restarts_the_fetcher_and_switches_to_window_pixels_mid_scanline() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xCC, 0xF0);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
    write_window_tilemap_entry(&mut vram_bytes, 0, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x0F,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert!(snapshot.window_wy_latch);
    assert!(snapshot.window_started_this_line);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 3, 3, 2, 2, 1, 1, 0, 0]
    );
}

#[test]
fn wy_latch_is_sampled_at_mode2_start_and_not_recomputed_mid_line() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xCC, 0xF0);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
    write_window_tilemap_entry(&mut vram_bytes, 0, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x01,
        wx: 0x07,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..100 {
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    }

    ppu.write_register(0xFF4A, 0x00);

    let _ = tick_until_hblank(&mut ppu, 100, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert!(!snapshot.window_wy_latch);
    assert!(!snapshot.window_started_this_line);
    assert_eq!(snapshot.window_line_counter, 0);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3]
    );
}

#[test]
fn window_line_counter_advances_only_on_lines_where_window_actually_starts() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xCC, 0xF0);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
    write_window_tilemap_entry(&mut vram_bytes, 0, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0xA7,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let t_cycle = tick_until_line_start(&mut ppu, 0, &oam_bytes, &vram_bytes, 1);
    assert_eq!(ppu.snapshot().window_line_counter, 0);

    ppu.write_register(0xFF4B, 0x07);

    let _t_cycle = tick_until_line_start(&mut ppu, t_cycle, &oam_bytes, &vram_bytes, 2);
    let line_2_start = ppu.snapshot();
    assert_eq!(line_2_start.window_line_counter, 1);
}

#[test]
fn wx_zero_with_scx_discard_shortens_window_start_timing_by_one_dot() {
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_window_tilemap_entry(&mut vram_bytes, 0, 0, 0);

    let mut wx_zero = Ppu::new(ConsoleModel::Dmg);
    wx_zero.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x03,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let mut wx_seven = Ppu::new(ConsoleModel::Dmg);
    wx_seven.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x03,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x07,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut wx_zero, 0, &oam_bytes, &vram_bytes);
    let _ = tick_until_hblank(&mut wx_seven, 0, &oam_bytes, &vram_bytes);

    assert_eq!(
        wx_zero.snapshot().mode0_start_dot + 1,
        wx_seven.snapshot().mode0_start_dot
    );
}

#[test]
fn wx_166_defers_window_start_to_the_following_scanline() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xCC, 0xF0);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);
    write_window_tilemap_entry(&mut vram_bytes, 0, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0xF1,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 166,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let t_cycle = tick_until_line_start(&mut ppu, 0, &oam_bytes, &vram_bytes, 1);
    let first_line = ppu.snapshot();
    assert_eq!(first_line.window_line_counter, 0);

    let _ = tick_until_hblank(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
    let second_line = ppu.snapshot();
    assert!(second_line.window_started_this_line);
    assert_eq!(
        &second_line.current_scanline_pixels[..8],
        &[3, 3, 2, 2, 1, 1, 0, 0]
    );
}

#[test]
fn obj_priority_prefers_lower_x_before_oam_order() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 20, 0);
    write_oam_entry(&mut oam_bytes, 1, 16, 18, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0x00, 0xFF);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(
        &snapshot.current_scanline_pixels[10..20],
        &[2, 2, 2, 2, 2, 2, 2, 2, 1, 1]
    );
}

#[test]
fn object_fetch_reads_tile_and_attributes_from_live_oam_metadata() {
    let sprite = PpuSelectedSprite {
        oam_index: 3,
        y: 16,
        x: 24,
        tile_index: 0x11,
        attributes: 0x22,
    };
    let mut oam_bytes = [0; 160];
    write_oam_entry_with_attributes(
        &mut oam_bytes,
        sprite.oam_index,
        sprite.y,
        sprite.x,
        0x44,
        0xA0,
    );

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let oam = OamBusView::new(BusMaster::Ppu, &mut oam);

    let (tile_index, attributes) = read_obj_fetch_sprite_metadata(&oam, sprite, None);

    assert_eq!(tile_index, 0x44);
    assert_eq!(attributes, 0xA0);
}

#[test]
fn object_fetch_uses_the_dma_conflict_word_address_for_late_oam_metadata_reads() {
    let sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 24,
        tile_index: 0x11,
        attributes: 0x22,
    };
    let mut oam_bytes = [0; 160];
    write_oam_entry_with_attributes(
        &mut oam_bytes,
        sprite.oam_index,
        sprite.y,
        sprite.x,
        0x44,
        0xA0,
    );
    write_oam_entry_with_attributes(&mut oam_bytes, 5, 32, 40, 0x99, 0x10);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let oam = OamBusView::new(BusMaster::Ppu, &mut oam);

    let (tile_index, attributes) = read_obj_fetch_sprite_metadata(&oam, sprite, Some(0xFE17));

    assert_eq!(tile_index, 0x99);
    assert_eq!(attributes, 0x10);
}

#[test]
fn obj_priority_uses_oam_order_when_x_matches() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 20, 0);
    write_oam_entry(&mut oam_bytes, 1, 16, 20, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0x00, 0xFF);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[12..20], &[1; 8]);
}

#[test]
fn transparent_obj_pixels_do_not_hide_lower_priority_obj_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 20, 0);
    write_oam_entry(&mut oam_bytes, 1, 16, 20, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0xAA, 0x00);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0x00, 0xFF);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(
        &snapshot.current_scanline_pixels[12..20],
        &[1, 2, 1, 2, 1, 2, 1, 2]
    );
}

#[test]
fn bg_over_obj_priority_blocks_only_nonzero_bg_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry_with_attributes(&mut oam_bytes, 0, 16, 8, 0, 0x80);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x00, 0xFF);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xAA, 0x00);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 1);
    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 1);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(
        &snapshot.current_scanline_pixels[..8],
        &[1, 2, 1, 2, 1, 2, 1, 2]
    );
}

#[test]
fn framebuffer_applies_bgp_without_changing_logical_scanline_colors() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);
    write_bg_tilemap_entry(&mut vram_bytes, 0, 0, 0);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x1B,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(
        &snapshot.current_scanline_pixels[..8],
        &[0, 1, 2, 3, 0, 1, 2, 3]
    );
    assert_eq!(&ppu.framebuffer()[..8], &[3, 2, 1, 0, 3, 2, 1, 0]);
}

#[test]
fn dmg_pixel_output_uses_or_of_current_and_previous_bgp_for_one_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.visible_registers.bgp = 0x1B;
    ppu.pipeline_registers.bgp = 0xE4;
    ppu.bgp = 0xE4;
    ppu.bg_pipeline_state.scx_discard_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[0], 1);
    assert_eq!(ppu.framebuffer()[0], 3);
}

#[test]
fn first_visible_pixel_uses_live_lcdc_instead_of_the_delayed_copy() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 2,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 8,
        oam_index: 0,
    });

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[0], 2);
}

#[test]
fn later_visible_pixels_use_the_delayed_lcdc_copy() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.visible_registers.lcdc = 0x93;
    ppu.pipeline_registers.lcdc = 0x91;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 1;
    ppu.bg_pipeline_state.current_transfer_x = 9;
    ppu.bg_pipeline_state.visible_pixels_output = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(1);
    ppu.obj_pipeline_state.fifo.push_back(ObjPixel {
        color: 2,
        palette_obp1: false,
        bg_over_obj: false,
        sprite_x: 9,
        oam_index: 0,
    });

    let _ = ppu.advance_mode3_output_phase();

    assert_eq!(ppu.snapshot().current_scanline_pixels[1], 1);
}

#[test]
fn framebuffer_applies_obj_palette_selection_without_changing_logical_obj_colors() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry_with_attributes(&mut oam_bytes, 0, 16, 8, 0, 0x10);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x92,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.write_register(0xFF48, 0xE4);
    ppu.write_register(0xFF49, 0x0C);

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[..8], &[1; 8]);
    assert_eq!(&ppu.framebuffer()[..8], &[3; 8]);
}

#[test]
fn dmg_bgp_write_during_mode3_recolors_recent_bg_pixels_with_transient_then_final_palette() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = 200;
    ppu.bg_pipeline_state.visible_pixels_output = 8;
    ppu.current_scanline_mixed_pixels[4..8].fill(MixedPixel::background(0));
    ppu.framebuffer[4..8].fill(1);

    ppu.write_register(0xFF47, 0x00);

    assert_eq!(&ppu.framebuffer()[4..8], &[1, 0, 0, 0]);
}

#[test]
fn dmg_bgp_write_in_early_hblank_recolors_only_last_three_visible_bg_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x80,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x01,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.current_scanline_mixed_pixels[156..160].fill(MixedPixel::background(0));
    ppu.framebuffer[156..160].fill(1);

    ppu.write_register(0xFF47, 0x00);

    assert_eq!(&ppu.framebuffer()[156..160], &[1, 1, 0, 0]);
}

#[test]
fn dmg_obp0_write_during_mode3_recolors_five_recent_obj_pixels() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.visible_output = PpuVisibleOutputState::Driving;
    ppu.line_dot = 200;
    ppu.bg_pipeline_state.visible_pixels_output = 8;
    ppu.current_scanline_mixed_pixels[3..8].fill(MixedPixel::object(1, false));
    ppu.framebuffer[3..8].fill(3);

    ppu.write_register(0xFF48, 0x04);

    assert_eq!(&ppu.framebuffer()[3..8], &[3, 1, 1, 1, 1]);
}

#[test]
fn obj_8x16_uses_even_aligned_tile_pairs_for_lower_half_rows() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 8, 8, 0x11);
    write_bg_tile_row(&mut vram_bytes, 0x10, 0, 0xFF, 0x00);
    write_bg_tile_row(&mut vram_bytes, 0x11, 0, 0x00, 0xFF);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x86,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[..8], &[2; 8]);
}

#[test]
fn partially_visible_top_clipped_8x16_sprite_uses_the_correct_row() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 2, 8, 0x10);
    write_bg_tile_row(&mut vram_bytes, 0x11, 6, 0x00, 0xFF);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x86,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[..8], &[2; 8]);
}

#[test]
fn partially_visible_bottom_clipped_sprite_uses_the_correct_final_rows() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 154, 8, 0x12);
    write_bg_tile_row(&mut vram_bytes, 0x12, 5, 0xFF, 0xFF);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x82,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 143,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let _ = tick_until_hblank(&mut ppu, 0, &oam_bytes, &vram_bytes);

    let snapshot = ppu.snapshot();
    assert_eq!(&snapshot.current_scanline_pixels[..8], &[3; 8]);
}

#[test]
fn live_obj_size_shrink_drops_out_of_range_y_flipped_rows_without_panicking() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.lcdc = 0x82;
    ppu.ly = 0;

    let sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 2,
        x: 8,
        tile_index: 0x10,
        attributes: 0x40,
    };

    assert_eq!(ppu.obj_tile_index_and_row(sprite), None);
}

#[test]
fn turning_off_lcdc1_during_object_fetch_cancels_sprite_pixels_but_keeps_timing_cost() {
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 8, 0);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);

    fn run_case(
        disable_obj_during_fetch: bool,
        oam_bytes: &[u8; 160],
        vram_bytes: &[u8; TEST_VRAM_BYTES],
    ) -> PpuSnapshot {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x82,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..80 {
            tick_ppu_with_vram(&mut ppu, t_cycle, oam_bytes, vram_bytes);
        }

        let mut t_cycle = 80;
        loop {
            let fetching = ppu.snapshot();
            if fetching.obj_fetcher_stage == PpuObjFetcherStage::Startup {
                break;
            }

            tick_ppu_with_vram(&mut ppu, t_cycle, oam_bytes, vram_bytes);
            t_cycle += 1;
            assert!(
                ppu.current_access_mode() == PpuAccessMode::Drawing,
                "left-edge OBJ fetch must still begin during Mode 3"
            );
            assert!(
                ppu.snapshot().visible_pixels_output <= 1,
                "left-edge OBJ fetch should still begin around the left edge"
            );
        }

        if disable_obj_during_fetch {
            ppu.write_register(0xFF40, 0x80);
        }

        let _ = tick_until_hblank(&mut ppu, t_cycle, oam_bytes, vram_bytes);
        ppu.snapshot()
    }

    let enabled = run_case(false, &oam_bytes, &vram_bytes);
    let disabled = run_case(true, &oam_bytes, &vram_bytes);

    assert_eq!(disabled.mode0_start_dot, enabled.mode0_start_dot);
    assert_ne!(enabled.current_scanline_pixels[0], 0);
    assert_eq!(&disabled.current_scanline_pixels[..8], &[0; 8]);
}

#[test]
fn smaller_raw_obj_x_values_start_fetch_earlier_during_mode3_startup() {
    fn fetch_start_line_dot(sprite_x: u8) -> u16 {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        let mut oam_bytes = [0; 160];
        let mut vram_bytes = [0; TEST_VRAM_BYTES];

        write_oam_entry(&mut oam_bytes, 0, 16, sprite_x, 0);
        write_bg_tile_row(&mut vram_bytes, 0, 0, 0xFF, 0x00);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x82,
            stat: 0x82,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        for t_cycle in 0..160 {
            tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
            let snapshot = ppu.snapshot();
            if snapshot.obj_fetcher_stage == PpuObjFetcherStage::Startup {
                return snapshot.line_dot;
            }
        }

        panic!("sprite fetch did not begin during early Mode 3");
    }

    let left_edge = fetch_start_line_dot(1);
    let first_visible = fetch_start_line_dot(8);

    assert!(left_edge < first_visible);
    assert!(left_edge >= MODE2_DOTS);
    assert!(first_visible >= MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1);
}

#[test]
fn overlapped_obj_fetch_uses_explicit_one_dot_stage_progression() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 8, 0);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    oam.set_acquired(BusMaster::Ppu, true);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    assert!(
        ppu.try_start_object_fetch_from_current_dot(ObjFetchStartSource::FifoBackedTransfer, true,)
    );
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::Startup
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Push);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Push);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
}

#[test]
fn current_mode2_oam_row_tracks_the_live_four_dot_slices() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(0));

    for t_cycle in 0..5 {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
    }

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(1));

    for t_cycle in 4..80 {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
    }

    let drawing = ppu.snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(drawing.current_oam_scan_row, None);
}

#[test]
fn first_oam_row_is_immune_to_basic_read_and_write_corruption_patterns() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut read_oam = [0; 160];
    let mut write_oam = [0; 160];

    write_oam_corruption_row(&mut read_oam, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
    write_oam_corruption_row(&mut write_oam, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Read, &mut read_oam));
    assert_eq!(
        &read_oam[..8],
        &[0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB]
    );

    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Write, &mut write_oam));
    assert_eq!(
        &write_oam[..8],
        &[0x57, 0x13, 0x68, 0x24, 0xAA, 0xAA, 0xBB, 0xBB]
    );
}

#[test]
fn write_corruption_uses_the_documented_first_word_formula_and_previous_row_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
    write_oam_corruption_row(&mut oam_bytes, 1, [0x0F0F, 0x1111, 0x2222, 0x3333]);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..5 {
        tick_ppu(&mut ppu, t_cycle, &[0; 160]);
    }

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(1));
    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Write, &mut oam_bytes));

    let expected_first = ((0x0F0F_u16 ^ 0xAAAA) & (0x1357 ^ 0xAAAA)) ^ 0xAAAA;
    assert_eq!(read_oam_word(&oam_bytes, 1, 0), expected_first);
    assert_eq!(read_oam_word(&oam_bytes, 1, 1), 0x2468);
    assert_eq!(read_oam_word(&oam_bytes, 1, 2), 0xAAAA);
    assert_eq!(read_oam_word(&oam_bytes, 1, 3), 0xBBBB);
}

#[test]
fn read_corruption_uses_the_documented_first_word_formula_and_previous_row_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
    write_oam_corruption_row(&mut oam_bytes, 1, [0x0F0F, 0x1111, 0x2222, 0x3333]);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..5 {
        tick_ppu(&mut ppu, t_cycle, &[0; 160]);
    }

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(1));
    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::Read, &mut oam_bytes));

    let expected_first = 0x1357_u16 | (0x0F0F & 0xAAAA);
    assert_eq!(read_oam_word(&oam_bytes, 1, 0), expected_first);
    assert_eq!(read_oam_word(&oam_bytes, 1, 1), 0x2468);
    assert_eq!(read_oam_word(&oam_bytes, 1, 2), 0xAAAA);
    assert_eq!(read_oam_word(&oam_bytes, 1, 3), 0xBBBB);
}

#[test]
fn read_plus_incdec_uses_its_dedicated_complex_path_in_rows_four_through_eighteen() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 2, [0x0F0F, 0x1212, 0x3434, 0x5656]);
    write_oam_corruption_row(&mut oam_bytes, 3, [0xAAAA, 0x1111, 0xC0C0, 0x2222]);
    write_oam_corruption_row(&mut oam_bytes, 4, [0x00FF, 0x3333, 0x4444, 0x5555]);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..17 {
        tick_ppu(&mut ppu, t_cycle, &[0; 160]);
    }

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(4));
    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::ReadWithIncDec, &mut oam_bytes));

    let expected_previous_first = 0xAAAA_u16 & (0x0F0F | 0x00FF | 0xC0C0);
    let expected_row = [expected_previous_first, 0x1111, 0xC0C0, 0x2222];

    for (word_index, expected) in expected_row.into_iter().enumerate() {
        assert_eq!(read_oam_word(&oam_bytes, 2, word_index), expected);
        assert_eq!(read_oam_word(&oam_bytes, 4, word_index), expected);
    }
    assert_eq!(read_oam_word(&oam_bytes, 3, 0), expected_previous_first);
    assert_eq!(read_oam_word(&oam_bytes, 3, 1), 0x1111);
    assert_eq!(read_oam_word(&oam_bytes, 3, 2), 0xC0C0);
    assert_eq!(read_oam_word(&oam_bytes, 3, 3), 0x2222);
}

#[test]
fn read_plus_incdec_on_the_last_row_falls_back_to_ordinary_read_corruption() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 18, [0x1234, 0x1111, 0x00FF, 0x2222]);
    write_oam_corruption_row(&mut oam_bytes, 19, [0x0F0F, 0xAAAA, 0xBBBB, 0xCCCC]);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..77 {
        tick_ppu(&mut ppu, t_cycle, &[0; 160]);
    }

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(19));
    assert!(ppu.apply_oam_corruption_event(OamCorruptionEventKind::ReadWithIncDec, &mut oam_bytes));

    let expected_first = 0x1234_u16 | (0x0F0F & 0x00FF);
    assert_eq!(read_oam_word(&oam_bytes, 19, 0), expected_first);
    assert_eq!(read_oam_word(&oam_bytes, 19, 1), 0x1111);
    assert_eq!(read_oam_word(&oam_bytes, 19, 2), 0x00FF);
    assert_eq!(read_oam_word(&oam_bytes, 19, 3), 0x2222);
    assert_eq!(read_oam_word(&oam_bytes, 17, 0), 0x0000);
}

#[test]
fn cgb_models_do_not_apply_dmg_family_oam_corruption() {
    let mut ppu = Ppu::new(ConsoleModel::Cgb);
    let mut oam_bytes = [0; 160];

    write_oam_corruption_row(&mut oam_bytes, 0, [0x1357, 0x2468, 0xAAAA, 0xBBBB]);
    write_oam_corruption_row(&mut oam_bytes, 1, [0x0F0F, 0x1111, 0x2222, 0x3333]);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    for t_cycle in 0..4 {
        tick_ppu(&mut ppu, t_cycle, &[0; 160]);
    }

    let before = oam_bytes;
    assert!(!ppu.apply_oam_corruption_event(OamCorruptionEventKind::Write, &mut oam_bytes));
    assert_eq!(oam_bytes, before);
}
