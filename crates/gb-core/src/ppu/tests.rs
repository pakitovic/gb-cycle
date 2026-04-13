use super::*;
use crate::bus::BusMaster;
use crate::scheduler::TCycle;
use crate::{ConsoleModel, Machine, MachineConfig, StartupMode, TraceSummaryBuffer};

const TEST_VRAM_BYTES: usize = 0x2000;
const DMG_BOOT_LOGO_TILE_VRAM_START: u16 = 0x8010;
const DMG_BOOT_LOGO_MAP_VRAM_START: u16 = 0x9904;
const DMG_BOOT_LOGO_TILE_BYTES: [u8; 200] = [
    0xF0, 0xF0, 0xFC, 0xFC, 0xFC, 0xFC, 0xF3, 0xF3, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0xF0, 0xF0, 0xF0, 0xF0, 0x00, 0x00, 0xF3, 0xF3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xCF, 0xCF,
    0x00, 0x00, 0x0F, 0x0F, 0x3F, 0x3F, 0x0F, 0x0F, 0x00, 0x00, 0x00, 0x00, 0xC0, 0xC0, 0x0F, 0x0F,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF3, 0xF3,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0xC0, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0xFF, 0xFF,
    0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC3, 0xC3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFC, 0xFC,
    0xF3, 0xF3, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0x3C, 0x3C, 0xFC, 0xFC, 0xFC, 0xFC, 0x3C, 0x3C,
    0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3,
    0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0x3C, 0x3C, 0x3F, 0x3F, 0x3C, 0x3C, 0x0F, 0x0F,
    0x3C, 0x3C, 0xFC, 0xFC, 0x00, 0x00, 0xFC, 0xFC, 0xFC, 0xFC, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0,
    0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF0, 0xF0, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xFF, 0xFF,
    0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xC3, 0xC3, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0xFC, 0xFC,
    0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C,
];
const DMG_BOOT_LOGO_MAP_BYTES: [u8; 44] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x19, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct HacktixStrikethroughLine68Observation {
    t_cycle: u64,
    line_dot: u16,
    visible_pixels_output: u8,
    mode0_start_dot: u16,
    current_transfer_x: u8,
    current_transfer_lane: Option<PpuMode3TransferLaneSnapshot>,
    obj_fetcher_stage: PpuObjFetcherStage,
    obj_fetcher_stage_dot: u8,
    fetch_sprite_slot: u8,
    fetch_sprite_oam_index: Option<u8>,
    fetch_sprite_x: Option<u8>,
    resolved_tile_index: Option<u8>,
    resolved_attributes: Option<u8>,
    late_metadata_word: Option<(u8, u8)>,
    dma_byte_destination_address: Option<u16>,
}

fn sync_test_video_ownership(
    ppu: &Ppu,
    oam: &mut crate::bus::OamDomain,
    vram: &mut crate::bus::VramDomain,
    dma_oam_active: bool,
) {
    let bus_state = ppu.owner_bus_state();
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
    dma_oam_conflict: Option<PpuDmaOamConflict>,
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
        dma_oam_conflict,
    );
    context
}

fn drain_ppu_interrupts(ppu: &mut Ppu) -> Vec<InterruptSource> {
    ppu.drain_pending_interrupt_requests()
}

fn seed_hacktix_dmg_boot_logo_vram(machine: &mut Machine<TraceSummaryBuffer>) {
    for (index, byte) in DMG_BOOT_LOGO_TILE_BYTES.iter().copied().enumerate() {
        machine.write_bus(DMG_BOOT_LOGO_TILE_VRAM_START + (index as u16 * 2), byte);
    }
    for (index, byte) in DMG_BOOT_LOGO_MAP_BYTES.iter().copied().enumerate() {
        machine.write_bus(DMG_BOOT_LOGO_MAP_VRAM_START + index as u16, byte);
    }
}

fn load_hacktix_strikethrough_machine() -> Machine<TraceSummaryBuffer> {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/hacktix/strikethrough.gb");
    let rom = std::fs::read(&rom_path).expect("hacktix strikethrough ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("hacktix ROM should load");
    seed_hacktix_dmg_boot_logo_vram(&mut machine);
    machine
}

fn sample_hacktix_strikethrough_line(
    target_ly: u8,
    max_events: usize,
) -> (
    Vec<PpuSelectedSprite>,
    Vec<HacktixStrikethroughLine68Observation>,
    [u8; 8],
    [u8; 8],
) {
    let mut machine = load_hacktix_strikethrough_machine();
    let mut current_selected_sprites = Vec::new();
    let mut current_events = Vec::with_capacity(max_events);
    let mut last_completed_line68 = None;

    for _ in 0..3_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu();
        if ppu.ly != target_ly {
            if machine.cpu().execution_state() == crate::CpuExecutionState::Halted
                && let Some(line68) = last_completed_line68
            {
                return line68;
            }
            continue;
        }

        if ppu.line_dot == MODE2_DOTS {
            current_selected_sprites = ppu.mode2_scan_state.selected_sprites_snapshot();
            current_events.clear();
        }

        let dma_progress = machine.dma().transfer_progress();
        let dma_byte_destination_address = dma_progress
            .filter(|progress| {
                progress.completed_bytes() > 0 && progress.byte_phase_t_cycles() == 0
            })
            .map(|progress| {
                progress
                    .transfer()
                    .destination_address_for_byte(progress.completed_bytes() - 1)
            });

        if (ppu.obj_pipeline_state.fetch.stage != PpuObjFetcherStage::Idle
            || dma_byte_destination_address.is_some())
            && current_events.len() < max_events
        {
            current_events.push(HacktixStrikethroughLine68Observation {
                t_cycle: machine.next_t_cycle().get().saturating_sub(1),
                line_dot: ppu.line_dot,
                visible_pixels_output: ppu.bg_pipeline_state.visible_pixels_output,
                mode0_start_dot: ppu.current_mode0_start_dot(),
                current_transfer_x: ppu.bg_pipeline_state.current_transfer_x,
                current_transfer_lane: ppu
                    .current_transfer()
                    .map(|transfer| snapshot_bg_transfer_lane(transfer.context.lane)),
                obj_fetcher_stage: ppu.obj_pipeline_state.fetch.stage,
                obj_fetcher_stage_dot: ppu.obj_pipeline_state.fetch.stage_dot,
                fetch_sprite_slot: ppu.obj_pipeline_state.fetch.sprite_slot,
                fetch_sprite_oam_index: ppu
                    .obj_pipeline_state
                    .fetch
                    .sprite
                    .map(|sprite| sprite.oam_index),
                fetch_sprite_x: ppu.obj_pipeline_state.fetch.sprite.map(|sprite| sprite.x),
                resolved_tile_index: ppu
                    .obj_pipeline_state
                    .fetch
                    .resolved_sprite
                    .map(|sprite| sprite.tile_index),
                resolved_attributes: ppu
                    .obj_pipeline_state
                    .fetch
                    .resolved_sprite
                    .map(|sprite| sprite.attributes),
                late_metadata_word: ppu.obj_pipeline_state.late_metadata_word,
                dma_byte_destination_address,
            });
        }

        if ppu.ly == target_ly && ppu.current_access_mode() == PpuAccessMode::HBlank {
            let mut segment = [0_u8; 8];
            segment.copy_from_slice(&ppu.current_scanline_pixels[71..79]);
            let mut framebuffer_segment = [0_u8; 8];
            let framebuffer_start = target_ly as usize * SCREEN_WIDTH + 71;
            framebuffer_segment
                .copy_from_slice(&ppu.framebuffer[framebuffer_start..framebuffer_start + 8]);
            last_completed_line68 = Some((
                current_selected_sprites.clone(),
                current_events.clone(),
                segment,
                framebuffer_segment,
            ));
        }
    }

    if let Some(line68) = last_completed_line68 {
        return line68;
    }

    panic!(
        "hacktix strikethrough line sample did not reach the halted framebuffer; target_ly={} pc={:#06X} state={:?} ly={} line_dot={} mode={:?}",
        target_ly,
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().ly,
        machine.ppu().line_dot,
        machine.ppu().current_access_mode()
    );
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

fn apply_tile_sel_line_write_replay(ppu: &mut Ppu) {
    let snapshot = ppu.snapshot();
    if snapshot.mode != PpuAccessMode::Drawing {
        return;
    }

    if snapshot.line_dot == 104 {
        ppu.write_register(0xFF40, 0x93);
    } else if snapshot.line_dot == 112 {
        ppu.write_register(0xFF40, 0x83);
    }
}

fn tick_until_tile_sel_replay_position(
    ppu: &mut Ppu,
    mut t_cycle: u64,
    oam_bytes: &[u8],
    vram_bytes: &[u8],
    target_ly: u8,
    target_line_dot: u16,
) -> u64 {
    while !(ppu.snapshot().ly == target_ly && ppu.snapshot().line_dot == target_line_dot) {
        apply_tile_sel_line_write_replay(ppu);
        tick_ppu_with_vram(ppu, t_cycle, oam_bytes, vram_bytes);
        t_cycle += 1;
        assert!(t_cycle < 40 * TOTAL_SCANLINES as u64 * DOTS_PER_SCANLINE as u64);
    }

    t_cycle
}

#[test]
fn startup_state_recreates_the_documented_post_boot_lcd_snapshot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
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
    assert_eq!(ppu.read_register(0xFF41), 0x8C);
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
        PpuBusState::lcd_enabled(PpuAccessMode::HBlank)
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
        stat: 0x08,
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
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    assert_eq!(ppu.snapshot().mode, PpuAccessMode::HBlank);
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(ppu.snapshot().mode_dot, 0);

    tick_ppu(&mut ppu, 0, &[0; 160]);

    assert_eq!(ppu.snapshot().ly, 0);
    assert_eq!(ppu.snapshot().line_dot, 1);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(ppu.snapshot().mode_dot, 1);
}

#[test]
fn cpu_oam_write_bus_state_only_opens_the_restart_probe_window_at_line_start_and_mode2_end() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.line_dot = 0;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    assert_eq!(ppu.cpu_oam_write_bus_state().mode(), PpuAccessMode::HBlank);

    ppu.line_dot = 4;
    assert_eq!(ppu.cpu_oam_write_bus_state().mode(), PpuAccessMode::OamScan);

    ppu.line_dot = MODE2_DOTS;
    assert_eq!(ppu.cpu_write_bus_state().mode(), PpuAccessMode::OamScan);
    assert_eq!(ppu.cpu_oam_write_bus_state().mode(), PpuAccessMode::HBlank);

    ppu.line_dot = MODE2_DOTS + 4;
    assert_eq!(ppu.cpu_oam_write_bus_state().mode(), PpuAccessMode::Drawing);
}

#[test]
fn cpu_oam_read_bus_state_only_opens_the_mode2_end_probe_window() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    ppu.line_dot = MODE2_DOTS - 1;
    assert_eq!(ppu.cpu_bus_state().mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::Drawing);

    ppu.line_dot = MODE2_DOTS;
    assert_eq!(ppu.cpu_bus_state().mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::HBlank);

    ppu.line_dot = MODE2_DOTS + 1;
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::Drawing);
}

#[test]
fn cpu_oam_read_bus_state_switches_to_hblank_on_the_exact_mode0_start_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    ppu.line_dot = MODE0_START_DOT - 1;
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::Drawing);

    ppu.line_dot = MODE0_START_DOT;
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::HBlank);

    ppu.line_dot = MODE0_START_DOT + 1;
    assert_eq!(ppu.cpu_oam_read_bus_state().mode(), PpuAccessMode::HBlank);
}

#[test]
fn cpu_stat_read_switches_to_mode3_on_the_exact_mode2_end_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    ppu.line_dot = MODE2_DOTS - 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x02
    );

    ppu.line_dot = MODE2_DOTS;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );

    ppu.line_dot = MODE2_DOTS + 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_switches_to_hblank_on_the_exact_mode0_start_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    ppu.line_dot = MODE0_START_DOT - 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );

    ppu.line_dot = MODE0_START_DOT;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );

    ppu.line_dot = MODE0_START_DOT + 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
#[ignore = "diagnostic direct-read experiment for offscreen-right mode0 publication"]
fn cpu_stat_read_switches_to_hblank_one_dot_before_mode0_start_for_offscreen_right_sprites() {
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    for oam_index in 0..10 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 168,
            tile_index: 0,
            attributes: 0,
        });
    }

    ppu.line_dot = MODE0_START_DOT - 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
#[ignore = "diagnostic state for the sprite-extended post-visible publication seam without startup placeholders"]
fn cpu_stat_read_logs_sprite_extended_post_visible_tail_without_startup_placeholders() {
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

    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 58;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 8,
        tile_index: 0,
        attributes: 0,
    });

    for line_dot in [
        ppu.bg_pipeline_state.mode0_start_dot + 2,
        ppu.bg_pipeline_state.mode0_start_dot + 3,
    ] {
        ppu.line_dot = line_dot;
        let stat = ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation);
        println!(
            "x8_tail line_dot={} stat_mode={} current_mode={:?} current_mode0_start_dot={} bg_base_mode0_start_dot={} current_transfer_x={} bg_lane={:?} bg_source_window={:?} bg_readiness={:?} startup_fifo_placeholders={} bg_fifo_len={} obj_stage={:?} obj_pending_hit_match_x={:?} obj_pending_hit_len={}",
            line_dot,
            stat & 0x03,
            ppu.current_access_mode(),
            ppu.current_mode0_start_dot(),
            ppu.bg_pipeline_state.mode0_start_dot,
            ppu.bg_pipeline_state.current_transfer_x,
            ppu.current_transfer().map(|transfer| transfer.context.lane),
            ppu.current_transfer()
                .map(|transfer| transfer.context.source_window),
            ppu.current_transfer().map(|transfer| transfer.readiness),
            ppu.bg_pipeline_state.startup_fifo_placeholders,
            ppu.bg_pipeline_state.fifo.len(),
            ppu.obj_pipeline_state.fetch.stage,
            ppu.obj_pipeline_state.pending_match_x,
            ppu.obj_pipeline_state.pending_sprite_slots.len(),
        );
    }
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x167_visible_tail_without_obj_work() {
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 15;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 0,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 17,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_terminal_x167_visible_tail_with_pending_same_x_work() {
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 15;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 16;
    ppu.obj_pipeline_state.pending_match_x = Some(167);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(8);

    for slot in 0..MAX_SELECTED_SPRITES_PER_LINE {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: slot as u8,
            y: 16,
            x: 167,
            tile_index: slot as u8,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 17,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x167_visible_tail_with_ready_push_and_pending_same_x_chain()
 {
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;
    ppu.obj_pipeline_state.pending_match_x = Some(167);

    for slot in 0..5 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: slot as u8,
            y: 16,
            x: 71,
            tile_index: slot as u8,
            attributes: 0,
        });
    }
    for slot in 5..MAX_SELECTED_SPRITES_PER_LINE {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: slot as u8,
            y: 16,
            x: 167,
            tile_index: slot as u8,
            attributes: 0,
        });
    }
    for sprite_slot in 5..9 {
        ppu.obj_pipeline_state.mark_fetched(sprite_slot as u8);
    }
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(9);

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot before the CPU-visible read"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x167_visible_tail_while_blank_frame_is_active() {
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 15;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 0,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 17,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x165_visible_tail_while_blank_frame_is_active() {
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 54;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 165;
    ppu.bg_pipeline_state.visible_pixels_output = 157;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.line_dot = MODE0_START_DOT + 56;

    for oam_index in 0..9 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 0,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 57,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x166_visible_tail_without_blank_frame() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x93,
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 59;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 166;
    ppu.bg_pipeline_state.visible_pixels_output = 158;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 2));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.line_dot = MODE0_START_DOT + 59;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 160,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 60,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_placeholder_only_visible_tail() {
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 24;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 164;
    ppu.bg_pipeline_state.visible_pixels_output = 156;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 4));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 24;

    for oam_index in 0..4 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 0,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 25,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x163_visible_tail_even_with_one_real_fifo_pixel() {
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

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 28;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 163;
    ppu.bg_pipeline_state.visible_pixels_output = 155;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 5));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 28;

    for oam_index in 0..5 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 0,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 29,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x163_visible_tail_on_saturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 163;
    ppu.bg_pipeline_state.visible_pixels_output = 155;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 5));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 2,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_single_x2_placeholder_backed_terminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 12;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 163;
    ppu.bg_pipeline_state.visible_pixels_output = 155;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 5));
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 8;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 2,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 12,
        "placeholder-backed visible tail still stretches four live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_single_x4_placeholder_backed_preterminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 9;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 162;
    ppu.bg_pipeline_state.visible_pixels_output = 154;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 6));
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 4;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 4,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 9,
        "placeholder-backed preterminal tail still stretches five live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_single_x5_placeholder_backed_preterminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 8;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 163;
    ppu.bg_pipeline_state.visible_pixels_output = 155;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 3;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 5));
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 4;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 5,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 8,
        "placeholder-backed x=5 tail still stretches four live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_single_x6_placeholder_backed_preterminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 7;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 164;
    ppu.bg_pipeline_state.visible_pixels_output = 156;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 4));
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 4;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 6,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 7,
        "placeholder-backed x=6 tail still stretches three live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_single_x7_placeholder_backed_preterminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 6;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 165;
    ppu.bg_pipeline_state.visible_pixels_output = 157;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 3));
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 4;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 7,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 6,
        "placeholder-backed x=7 tail still stretches two live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_two_sprite_staggered_x2_x0a_fifo_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 18;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 164;
    ppu.bg_pipeline_state.visible_pixels_output = 156;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 4));
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 2,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 0x0A,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 18,
        "staggered two-sprite tail still stretches three live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_two_sprite_staggered_x4_x0c_fifo_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 19;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 159;
    ppu.bg_pipeline_state.visible_pixels_output = 151;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.line_dot = MODE0_START_DOT + 12;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 4,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 0x0C,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 19,
        "staggered two-sprite FIFO tail still stretches eight live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_two_sprite_staggered_x8_x10_preterminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 17;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.line_dot = MODE0_START_DOT + 16;

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
        x: 0x10,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 17,
        "staggered x=8/16 pair still has one live drawing dot before internal HBlank"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_two_sprite_staggered_x0_x08_terminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 13;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
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

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 13,
        "internal HBlank already started for the staggered x=0/8 pair"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_two_sprite_staggered_x1_x09_terminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 12;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 1,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 9,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 12,
        "internal HBlank already started for the staggered x=1/9 pair"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_two_sprite_staggered_x9_x11_terminal_boundary() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 15;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.line_dot = MODE0_START_DOT + 16;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 9,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 17,
        tile_index: 1,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 15,
        "internal HBlank already starts one dot before the published boundary for x=9/17"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_drawing_for_ten_sprite_step8_terminal_tails() {
    for (min_x, placeholders, push_pending, terminal_offset) in
        [(0, 2, true, 4), (1, 1, false, 4), (2, 4, true, 4)]
    {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        ppu.ly = 68;
        ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        ppu.blank_frame_active = false;
        ppu.bg_pipeline_state.mode3_started = true;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 20;
        ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
        ppu.bg_pipeline_state.current_transfer_x = 168;
        ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.startup_fifo_placeholders = placeholders;
        ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
        ppu.bg_pipeline_state.push.pending = push_pending;
        ppu.line_dot = ppu.bg_pipeline_state.mode0_start_dot + terminal_offset;

        for sprite_slot in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
            ppu.mode2_scan_state.push(PpuSelectedSprite {
                oam_index: sprite_slot,
                y: 16,
                x: min_x + sprite_slot * 8,
                tile_index: sprite_slot,
                attributes: 0,
            });
        }

        assert_eq!(
            ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
            0x03,
            "step-8 terminal tail with min_x={min_x} should keep published drawing"
        );
    }
}

#[test]
fn cpu_stat_read_publishes_hblank_for_ten_sprite_step8_preterminal_tails() {
    for (min_x, current_transfer_x, fifo_len) in [
        (4, 160, 8_usize),
        (5, 152, 8_usize),
        (6, 152, 8_usize),
        (7, 152, 8_usize),
    ] {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        ppu.ly = 68;
        ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        ppu.blank_frame_active = false;
        ppu.bg_pipeline_state.mode3_started = true;
        ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 32;
        ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
        ppu.bg_pipeline_state.current_transfer_x = current_transfer_x;
        ppu.bg_pipeline_state.visible_pixels_output = current_transfer_x - 8;
        ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        ppu.bg_pipeline_state.startup_fifo_placeholders = 8 - min_x;
        ppu.bg_pipeline_state
            .fifo
            .extend(std::iter::repeat_n(0, fifo_len));
        ppu.line_dot = MODE0_START_DOT + 24;

        for sprite_slot in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
            ppu.mode2_scan_state.push(PpuSelectedSprite {
                oam_index: sprite_slot,
                y: 16,
                x: min_x + sprite_slot * 8,
                tile_index: sprite_slot,
                attributes: 0,
            });
        }

        assert!(matches!(
            ppu.current_transfer().map(|transfer| transfer.readiness),
            Some(Mode3TransferReadiness::Ready(_))
        ));
        assert_eq!(
            ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
            0x00,
            "step-8 preterminal tail with min_x={min_x} should publish HBlank early"
        );
    }
}

#[test]
fn cpu_stat_read_publishes_hblank_for_single_x12_terminal_tail_with_entry_delay() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 6;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 166;
    ppu.bg_pipeline_state.visible_pixels_output = 158;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 2));
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 4;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 12,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 6,
        "single-sprite x=12 tail still stretches two live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_single_x16_terminal_tail_with_entry_delay() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 10;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 166;
    ppu.bg_pipeline_state.visible_pixels_output = 158;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 2));
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 8;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 16,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 10,
        "single-sprite x=16 tail still stretches two live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_single_xa0_terminal_tail_without_entry_delay() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 6;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 166;
    ppu.bg_pipeline_state.visible_pixels_output = 158;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 2));
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 4;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0xA0,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 6,
        "single offscreen-right x=0xA0 tail still stretches two live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_single_xa7_terminal_tail() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 7;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 1));
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.line_dot = MODE0_START_DOT + 5;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0xA7,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 7,
        "single offscreen-right x=0xA7 tail still stretches two live dots internally"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_on_the_single_xa2_mode0_boundary() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 6;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = SCREEN_WIDTH as u8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.line_dot = MODE0_START_DOT + 6;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0xA2,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 6,
        "single offscreen-right x=0xA2 case reaches the mode0 boundary directly"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x163_visible_tail_on_saturated_sprite_lines_with_mode2_enable()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 163;
    ppu.bg_pipeline_state.visible_pixels_output = 155;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 5));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 2,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x162_placeholder_backed_tail_on_saturated_sprite_lines()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 162;
    ppu.bg_pipeline_state.visible_pixels_output = 154;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 6));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 2,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
#[ignore = "diagnostic terminal x162 placeholder-backed tail with blank_frame_active on saturated sprite lines"]
fn cpu_stat_read_logs_terminal_x162_placeholder_backed_tail_with_blank_frame_active_on_saturated_sprite_lines()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 162;
    ppu.bg_pipeline_state.visible_pixels_output = 154;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 6));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 2,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    println!(
        "blank_frame_active_case read={:#04X} mode0_start_dot={} current_transfer_x={} fifo_len={} placeholders={}",
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        ppu.current_mode0_start_dot(),
        ppu.bg_pipeline_state.current_transfer_x,
        ppu.bg_pipeline_state.fifo.len(),
        ppu.bg_pipeline_state.startup_fifo_placeholders
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x164_placeholder_only_visible_tail_on_saturated_sprite_lines()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 24;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 164;
    ppu.bg_pipeline_state.visible_pixels_output = 156;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 4));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 24;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 2,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 25,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x161_placeholder_backed_tail_on_saturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 161;
    ppu.bg_pipeline_state.visible_pixels_output = 153;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 7));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 2,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x165_placeholder_backed_tail_on_saturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 70;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 63;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 165;
    ppu.bg_pipeline_state.visible_pixels_output = 157;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 11));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.line_dot = MODE0_START_DOT + 64;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 0,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT + 65);
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_waiting_for_fifo_tail_on_saturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 152;
    ppu.bg_pipeline_state.visible_pixels_output = 144;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 19,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot while the FIFO is refilling"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::WaitingForFifo(_))
    ));
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_waiting_for_fifo_tail_on_saturated_sprite_lines_with_mode2_enable()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 152;
    ppu.bg_pipeline_state.visible_pixels_output = 144;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 19,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot while the FIFO is refilling"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::WaitingForFifo(_))
    ));
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x151_ready_tail_on_saturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 151;
    ppu.bg_pipeline_state.visible_pixels_output = 143;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 24,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot from the ready tail"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::Ready(_))
    ));
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x151_ready_tail_on_saturated_sprite_lines_with_mode2_enable()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = true;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 151;
    ppu.bg_pipeline_state.visible_pixels_output = 143;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 24,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot from the ready tail"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::Ready(_))
    ));
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x159_ready_tail_on_saturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 64;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 159;
    ppu.bg_pipeline_state.visible_pixels_output = 151;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 64;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 17,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 65,
        "live transfer still stretches one more dot from the ready tail"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::Ready(_))
    ));
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_publishes_hblank_for_terminal_x159_ready_tail_on_saturated_sprite_lines_with_mode2_enable()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 64;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 159;
    ppu.bg_pipeline_state.visible_pixels_output = 151;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 64;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 17,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 65,
        "live transfer still stretches one more dot from the ready tail"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::Ready(_))
    ));
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x159_ready_tail_on_shorter_saturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 159;
    ppu.bg_pipeline_state.visible_pixels_output = 151;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 17,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot from the shorter ready tail"
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x151_ready_tail_on_unsaturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 151;
    ppu.bg_pipeline_state.visible_pixels_output = 143;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..5 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 24,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot from the ready tail"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::Ready(_))
    ));
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x158_ready_tail_on_saturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 158;
    ppu.bg_pipeline_state.visible_pixels_output = 150;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..MAX_SELECTED_SPRITES_PER_LINE as u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 17,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot from the ready tail"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::Ready(_))
    ));
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_waiting_for_fifo_tail_on_unsaturated_sprite_lines() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 60;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 152;
    ppu.bg_pipeline_state.visible_pixels_output = 144;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 0;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.line_dot = MODE0_START_DOT + 60;

    for oam_index in 0..5 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 19,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot while the FIFO is refilling"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::WaitingForFifo(_))
    ));
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
#[ignore = "diagnostic case1 pre-read cpu-visible stat probe against the real mooneye ROM"]
fn cpu_stat_read_logs_case1_pre_read_state_against_real_rom() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..10_000_000 {
        let cpu_before = machine.cpu().snapshot();
        if machine.read_bus(0xFF80) == 1
            && cpu_before.registers.pc == 0x0B9C
            && matches!(
                cpu_before.execution_state,
                crate::CpuExecutionState::Execute {
                    opcode: 0xF0,
                    step: 2,
                    ..
                }
            )
        {
            let ppu_before = machine.ppu().snapshot();
            let stat_before = machine
                .ppu()
                .read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation);
            machine.step_t_cycle();
            let cpu_after = machine.cpu().snapshot();
            let ppu_after = machine.ppu().snapshot();
            let activity = cpu_after
                .last_bus_activity
                .expect("the next t-cycle should perform the FF41 read");
            println!(
                "case1_pre_read_probe stat_before={:#04X} before_pc={:#06X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} before_x={} before_vpo={} after_value={:#04X} after_pc={:#06X} after_ly={} after_line_dot={} after_mode={:?} after_mode0_start_dot={} after_x={} after_vpo={}",
                stat_before,
                cpu_before.registers.pc,
                ppu_before.ly,
                ppu_before.line_dot,
                ppu_before.mode,
                ppu_before.mode0_start_dot,
                ppu_before.bg_current_transfer_x,
                ppu_before.visible_pixels_output,
                activity.value,
                cpu_after.registers.pc,
                ppu_after.ly,
                ppu_after.line_dot,
                ppu_after.mode,
                ppu_after.mode0_start_dot,
                ppu_after.bg_current_transfer_x,
                ppu_after.visible_pixels_output,
            );
            assert_eq!(activity.address, 0xFF41);
            return;
        }

        machine.step_t_cycle();
    }

    panic!("probe did not reach the testcase 1 pre-read state");
}

#[test]
#[ignore = "diagnostic helper conditions at the real first FF41 read for testcase 1"]
fn cpu_stat_read_logs_case1_first_read_helper_conditions_against_real_rom() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut saw_irq_for_case1 = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        if machine.read_bus(0xFF80) != 1 {
            continue;
        }

        if !saw_irq_for_case1
            && matches!(
                machine.cpu().execution_state(),
                crate::CpuExecutionState::ServiceInterrupt {
                    source: crate::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_case1 = true;
        }

        let cpu_snapshot = machine.cpu().snapshot();
        if saw_irq_for_case1
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == crate::CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu();
            let published_mode = ppu.access_mode_for_line_dot(ppu.line_dot - 1);
            let current_mode = ppu.access_mode_for_line_dot(ppu.line_dot);
            let helper = ppu.terminal_visible_tail_should_publish_hblank_early();
            let current_transfer = ppu.current_transfer();
            let transfer_lane = current_transfer.map(|transfer| transfer.context.lane);
            let transfer_source_window =
                current_transfer.map(|transfer| transfer.context.source_window);
            println!(
                "case1_first_read_helper value={:#04X} pc={:#06X} line_dot={} ly={} published_mode={:?} current_mode={:?} current_mode0_start_dot={} helper={} blank_frame_active={} obj_stage={:?} pending_match_x={:?} pending_hit_len={} transfer_lane={:?} transfer_source_window={:?} current_transfer_x={} visible_pixels_output={} startup_fifo_placeholders={} fifo_len={} line_dot_plus_one_eq_mode0={} ly_visible={} obj_idle={} no_pending_match={} no_pending_hits={}",
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.line_dot,
                ppu.ly,
                published_mode,
                current_mode,
                ppu.current_mode0_start_dot(),
                helper,
                ppu.blank_frame_active,
                ppu.obj_pipeline_state.fetch.stage,
                ppu.obj_pipeline_state.pending_match_x,
                ppu.obj_pipeline_state.pending_sprite_slots.len(),
                transfer_lane,
                transfer_source_window,
                ppu.bg_pipeline_state.current_transfer_x,
                ppu.bg_pipeline_state.visible_pixels_output,
                ppu.bg_pipeline_state.startup_fifo_placeholders,
                ppu.bg_pipeline_state.fifo.len(),
                ppu.line_dot + 1 == ppu.current_mode0_start_dot(),
                ppu.ly < VISIBLE_SCANLINES,
                ppu.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle,
                ppu.obj_pipeline_state.pending_match_x.is_none(),
                ppu.obj_pipeline_state.pending_sprite_slots.is_empty(),
            );
            return;
        }
    }

    panic!("probe did not reach the testcase 1 first FF41 read");
}

#[test]
fn cpu_stat_read_suppresses_lyc_coincidence_on_the_first_dot_of_a_new_line() {
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

    ppu.ly = 1;
    ppu.lyc = 1;
    ppu.line_dot = 0;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x00
    );

    ppu.line_dot = 4;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x06
    );
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
fn lcd_reenable_first_line_skips_mode2_and_enters_mode3_late() {
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

    for t_cycle in 0..LCD_REENABLE_LINE0_MODE3_START_DOT - 1 {
        tick_ppu(&mut ppu, u64::from(t_cycle), &oam_bytes);
    }

    let line0_mode0_tail = ppu.snapshot();
    assert_eq!(
        line0_mode0_tail.line_dot,
        LCD_REENABLE_LINE0_MODE3_START_DOT - 1
    );
    assert_eq!(line0_mode0_tail.mode, PpuAccessMode::HBlank);
    assert_eq!(
        line0_mode0_tail.mode_dot,
        LCD_REENABLE_LINE0_MODE3_START_DOT - 1
    );
    assert_eq!(line0_mode0_tail.mode2_scanned_entries, 0);

    tick_ppu(
        &mut ppu,
        u64::from(LCD_REENABLE_LINE0_MODE3_START_DOT - 1),
        &oam_bytes,
    );

    let first_mode3_dot = ppu.snapshot();
    assert_eq!(first_mode3_dot.line_dot, LCD_REENABLE_LINE0_MODE3_START_DOT);
    assert_eq!(first_mode3_dot.mode, PpuAccessMode::Drawing);
    assert_eq!(first_mode3_dot.mode_dot, 0);
    assert_eq!(first_mode3_dot.mode2_scanned_entries, 0);

    let mut t_cycle = u64::from(LCD_REENABLE_LINE0_MODE3_START_DOT);
    while !(ppu.snapshot().ly == 1 && ppu.snapshot().line_dot == 2) {
        tick_ppu(&mut ppu, t_cycle, &oam_bytes);
        t_cycle += 1;
        assert!(t_cycle < 2 * DOTS_PER_SCANLINE as u64);
    }

    let first_normal_mode2_dot = ppu.snapshot();
    assert_eq!(first_normal_mode2_dot.mode, PpuAccessMode::OamScan);
    assert_eq!(first_normal_mode2_dot.mode_dot, 2);
    assert_eq!(first_normal_mode2_dot.mode2_scanned_entries, 1);
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
fn lcd_disable_preserves_pending_vblank_from_the_same_t_cycle() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let oam_bytes = [0; 160];

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x80,
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

    tick_ppu(&mut ppu, 0, &oam_bytes);
    assert_eq!(ppu.snapshot().ly, 144);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);

    ppu.write_register(0xFF40, 0x00);

    assert_eq!(ppu.snapshot().lcd_state, PpuLcdState::Disabled);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::VBlank]
    );
}

#[test]
fn lcd_disable_preserves_pending_lcd_stat_requests() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
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

    ppu.queue_interrupt_request(InterruptSource::LcdStat);
    ppu.write_register(0xFF40, 0x00);

    assert_eq!(ppu.snapshot().lcd_state, PpuLcdState::Disabled);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
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
    assert_eq!(first_blank_line.visible_pixels_output, 153);
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
fn dmg_mode2_oam_dma_reuses_the_last_latched_mode2_yx_word_for_selection() {
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
fn mode2_scanline_reset_preserves_the_latched_mode2_yx_word_for_dma_blocked_reads() {
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
    ppu.mode2_scan_state.latch_mode2_yx_word(16, 79);
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
fn late_obj_metadata_fetch_does_not_poison_the_mode2_dma_yx_latch() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];

    write_oam_entry_with_attributes(&mut oam_bytes, 0, 0, 0, 0xA5, 0x5A);

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
    ppu.mode2_scan_state.latch_mode2_yx_word(16, 79);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    oam.set_acquired(BusMaster::Ppu, true);
    let resolved = ppu.resolve_obj_fetch_sprite(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        PpuSelectedSprite {
            oam_index: 0,
            y: 0,
            x: 0,
            tile_index: 0,
            attributes: 0,
        },
        None,
    );

    assert_eq!(resolved.tile_index, 0xA5);
    assert_eq!(resolved.attributes, 0x5A);
    assert_eq!(ppu.mode2_scan_state.latched_mode2_yx_word(), Some((16, 79)));
    assert_eq!(
        ppu.obj_pipeline_state.late_metadata_word,
        Some((0xA5, 0x5A))
    );

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
fn resetting_the_obj_pipeline_clears_the_separate_late_metadata_word() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.obj_pipeline_state.late_metadata_word = Some((0x12, 0x34));

    ppu.obj_pipeline_state.reset();

    assert_eq!(ppu.obj_pipeline_state.late_metadata_word, None);
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
fn sprite_coupled_line10_tile_sel_replay_matches_trace_signature() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 26, 1, 0);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x00,
        stat: 0xA4,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF40, 0x83);

    let _ = tick_until_tile_sel_replay_position(&mut ppu, 0, &oam_bytes, &vram_bytes, 10, 85);

    let startup = ppu.snapshot();
    assert_eq!(startup.visible_lcdc, 0x83);
    assert_eq!(startup.pipeline_lcdc, 0x83);
    assert_eq!(startup.visible_pixels_output, 0);
    assert_eq!(startup.bg_current_transfer_x, 1);
    assert!(startup.bg_fill_pending);
    assert_eq!(startup.bg_fill_startup_dummy_pixels, 7);
    assert_eq!(startup.bg_startup_fifo_placeholders, 7);
    assert_eq!(startup.selected_sprites.len(), 1);
    assert_eq!(
        startup.bg_startup_fetch_seam,
        PpuBgStartupFetchSeamSnapshot::PostAlignment {
            first_real_push_skips_entry_delay: true,
            next_startup_continuation_slice: PpuBgStartupContinuationSliceSnapshot::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        }
    );
}

#[test]
fn sprite_coupled_line10_startup_tail_renders_correctly_once_panel_blank_is_lifted() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 26, 1, 0);
    for row in 0..BG_TILE_WIDTH {
        write_bg_tile_row(&mut vram_bytes, 0, row, 0x00, 0x00);
        let signed_tile_row = 0x1000 + row as usize * TILE_ROW_BYTES as usize;
        vram_bytes[signed_tile_row] = 0xFF;
        vram_bytes[signed_tile_row + 1] = 0xFF;
    }

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x00,
        stat: 0xA4,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF40, 0x83);

    let mut t_cycle =
        tick_until_tile_sel_replay_position(&mut ppu, 0, &oam_bytes, &vram_bytes, 10, 85);
    ppu.blank_frame_active = false;
    ppu.refresh_visible_output();
    assert_eq!(ppu.visible_output, PpuVisibleOutputState::Driving);
    t_cycle =
        tick_until_tile_sel_replay_position(&mut ppu, t_cycle, &oam_bytes, &vram_bytes, 10, 99);
    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0].expect(
        "the first visible startup-tail pixel should already be materialized before line_dot 100",
    );
    assert_eq!(
        front_cached.cached.origin,
        BgCachedSliceOrigin::StartupAlignmentFill
    );
    assert_eq!(ppu.bg_pipeline_state.fifo[0], 3);
    assert!(!front_cached.cached.needs_live_tilemap_refetch);
    assert!(!front_cached.cached.needs_live_tile_data_refetch);
    assert!(!front_cached.cached.needs_live_tile_data_current_row_refetch);
    assert!(!front_cached.cached.needs_live_tile_data_unsigned_reuse);
    assert_eq!(
        front_cached.cached.tile_low, 0xFF,
        "tile_high={:#04X} tile_data_address={:#06X}",
        front_cached.cached.tile_high, front_cached.cached.tile_data_address,
    );
    assert_eq!(front_cached.cached.tile_high, 0xFF);
    while !(ppu.snapshot().ly == 10 && ppu.snapshot().visible_pixels_output == 1) {
        apply_tile_sel_line_write_replay(&mut ppu);
        tick_ppu_with_vram(&mut ppu, t_cycle, &oam_bytes, &vram_bytes);
        t_cycle += 1;
        assert!(t_cycle < 11000);
    }

    let first_visible = ppu.snapshot();
    assert_eq!(
        first_visible.current_scanline_pixels[0],
        3,
        "line_dot={} visible_lcdc={:#04X} pipeline_lcdc={:#04X} visible_output={:?} current_transfer_x={}",
        first_visible.line_dot,
        first_visible.visible_lcdc,
        first_visible.pipeline_lcdc,
        first_visible.visible_output,
        first_visible.bg_current_transfer_x,
    );
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
    assert!(pipeline.take_startup_first_real_push_skip_entry_delay());
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
fn startup_post_alignment_seam_skips_the_first_real_push_entry_delay_once() {
    let mut pipeline = BgPipelineState::default();

    pipeline.begin_post_alignment_followup();

    assert_eq!(
        pipeline.startup_fetch_seam,
        BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: true,
            next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        }
    );
    assert!(pipeline.take_startup_first_real_push_skip_entry_delay());
    assert_eq!(
        pipeline.startup_fetch_seam,
        BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: false,
            next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        }
    );
    assert!(!pipeline.take_startup_first_real_push_skip_entry_delay());
}

#[test]
fn first_real_background_push_after_startup_alignment_skips_entry_delay() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; TEST_VRAM_BYTES]);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x91;
    ppu.bg_pipeline_state.begin_post_alignment_followup();
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x9801;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x8010;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x55;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x33;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert!(!ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileIndex
    );
    assert_eq!(ppu.bg_pipeline_state.fetcher.stage_dot, 0);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.fetch_x,
        BG_TILE_WIDTH as u16 * 2
    );
    assert_eq!(ppu.bg_pipeline_state.push.entry_delay_remaining, 0);
    assert_eq!(
        ppu.bg_pipeline_state.startup_fetch_seam,
        BgStartupFetchSeamState::PostAlignment {
            first_real_push_skips_entry_delay: false,
            next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
            startup_continuation_visible_tiles_remaining: 1,
            delayed_background_tileindex_read_tiles_remaining: 0,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 0,
        }
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
fn abstract_previsible_obj_start_keeps_startup_placeholders_non_fifo_backed() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 8;
    for _ in 0..16 {
        ppu.bg_pipeline_state.fifo.push_back(0);
    }
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    let arbitration = ppu.current_dot_arbitration();
    assert!(!arbitration.can_serve_bg_transfer());
    assert!(!arbitration.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));
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
fn late_second_startup_continuation_push_marks_live_tilemap_refetch_on_lcdc3_write() {
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
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.push.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.push.cached.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.push.cached.tile_map_address = 0x1801;
    ppu.bg_pipeline_state.push.cached.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.push.cached.tile_index = 0;
    ppu.bg_pipeline_state.push.cached.tile_low = 0x12;
    ppu.bg_pipeline_state.push.cached.tile_high = 0x34;
    ppu.bg_pipeline_state.push.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;

    ppu.write_register(0xFF40, 0x99);

    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
}

#[test]
fn third_startup_continuation_fetcher_carries_live_tilemap_refetch_on_lcdc3_write() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; TEST_VRAM_BYTES]);
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
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1802;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x0001;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x12;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x34;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };

    ppu.write_register(0xFF40, 0x99);
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));

    assert!(ppu.bg_pipeline_state.push.pending);
    assert_eq!(
        ppu.bg_pipeline_state.push.cached.origin,
        ppu.bg_pipeline_state.fetcher.cached_origin
    );
    assert!(ppu.bg_pipeline_state.push.cached.needs_live_tilemap_refetch);
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
fn flushing_bg_fill_tracks_cached_slice_in_fifo_sideband() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fill.cached.origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fill.cached.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fill.cached.tile_low = 0x55;
    ppu.bg_pipeline_state.fill.cached.tile_high = 0x33;

    ppu.flush_pending_bg_fifo_fill();

    assert_eq!(ppu.bg_pipeline_state.fifo.len(), BG_TILE_WIDTH as usize);
    assert_eq!(
        ppu.bg_pipeline_state.fifo_cached_pixels.len(),
        BG_TILE_WIDTH as usize
    );
    assert!(
        ppu.bg_pipeline_state
            .fifo_cached_pixels
            .iter()
            .enumerate()
            .all(|(pixel_index, cached)| {
                let Some(cached) = cached else {
                    return false;
                };
                cached.cached.origin
                    == BgCachedSliceOrigin::StartupContinuation(
                        BgStartupContinuationSlice::VisibleTile3,
                    )
                    && cached.cached.fetch_x == BG_TILE_WIDTH as u16 * 2
                    && cached.pixel_index == pixel_index as u8
            })
    );

    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    assert_eq!(ppu.bg_pipeline_state.fifo_cached_pixels.len(), 7);
}

#[test]
fn consuming_effective_fifo_pixel_keeps_the_visible_fifo_sideband_in_sync() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.push_back(2);
    ppu.bg_pipeline_state
        .fifo_cached_pixels
        .push_back(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                tile_low: 0xAA,
                tile_high: 0x00,
                ..BgCachedSlice::default()
            },
            0,
        )));

    assert_eq!(
        ppu.bg_pipeline_state.consume_effective_fifo_pixel(),
        Some(2)
    );
    assert_eq!(ppu.bg_pipeline_state.startup_fifo_placeholders, 0);
    assert!(ppu.bg_pipeline_state.fifo.is_empty());
    assert!(ppu.bg_pipeline_state.fifo_cached_pixels.is_empty());
}

#[test]
fn visible_fifo_sideband_keeps_full_cached_slice_metadata_for_future_closure_work() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupContinuation(
                BgStartupContinuationSlice::VisibleTile3,
            ),
            fetch_x: BG_TILE_WIDTH as u16 * 2,
            tile_map_address: 0x1802,
            tile_data_address: 0x0001,
            tile_index: 3,
            tile_low: 0x12,
            tile_high: 0x34,
            same_cycle_live_tilemap_refetch_window_open: true,
            ..BgCachedSlice::default()
        });

    let cached = ppu.bg_pipeline_state.fifo_cached_pixels[3]
        .expect("visible FIFO pixel should keep cached slice metadata");
    assert_eq!(
        cached.cached.origin,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3)
    );
    assert_eq!(cached.cached.fetch_x, BG_TILE_WIDTH as u16 * 2);
    assert_eq!(cached.cached.tile_map_address, 0x1802);
    assert_eq!(cached.cached.tile_data_address, 0x0001);
    assert_eq!(cached.cached.tile_index, 3);
    assert!(cached.cached.same_cycle_live_tilemap_refetch_window_open);
    assert_eq!(cached.pixel_index, 3);
}

#[test]
fn snapshot_exports_visible_fifo_cached_slice_metadata() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.fifo.push_back(2);
    ppu.bg_pipeline_state
        .fifo_cached_pixels
        .push_back(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                tile_map_address: 0x1802,
                tile_data_address: 0x0001,
                tile_index: 3,
                same_cycle_live_tilemap_refetch_window_open: true,
                needs_live_tilemap_refetch: true,
                tile_low: 0x12,
                tile_high: 0x34,
                ..BgCachedSlice::default()
            },
            5,
        )));

    let snapshot = ppu.snapshot();
    let cached = snapshot.bg_fifo_cached_pixels[0]
        .expect("snapshot should export visible FIFO sideband metadata");

    assert_eq!(snapshot.bg_fifo_pixels, vec![2]);
    assert_eq!(
        cached.origin,
        PpuBgCachedSliceOriginSnapshot::StartupContinuationVisibleTile3
    );
    assert_eq!(cached.fetch_x, BG_TILE_WIDTH as u16 * 2);
    assert_eq!(cached.pixel_index, 5);
    assert!(cached.same_cycle_live_tilemap_refetch_window_open);
    assert!(cached.needs_live_tilemap_refetch);
    assert_eq!(cached.tile_map_address, 0x1802);
    assert_eq!(cached.tile_data_address, 0x0001);
    assert_eq!(cached.tile_index, 3);
}

#[test]
fn snapshot_exports_mode3_startup_seam_observability() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 3 };
    ppu.bg_pipeline_state.begin_post_alignment_followup();
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 4;
    ppu.bg_pipeline_state
        .fetcher
        .post_alignment_fetch_restart_delay_dots = 1;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 12;

    let snapshot = ppu.snapshot();

    assert_eq!(
        snapshot.bg_startup_source_state,
        PpuMode3StartupSourceStateSnapshot::Abstract { remaining: 3 }
    );
    assert_eq!(
        snapshot.bg_startup_fetch_seam,
        PpuBgStartupFetchSeamSnapshot::PostAlignment {
            first_real_push_skips_entry_delay: true,
            next_startup_continuation_slice: PpuBgStartupContinuationSliceSnapshot::VisibleTile2,
            startup_continuation_visible_tiles_remaining: 2,
            delayed_background_tileindex_read_tiles_remaining: 1,
            delayed_background_tilemap_tiles_remaining: 0,
            delayed_background_tiledata_tiles_remaining: 1,
        }
    );
    assert_eq!(snapshot.bg_startup_fifo_placeholders, 2);
    assert_eq!(snapshot.bg_push_entry_delay_remaining, 1);
    assert_eq!(snapshot.bg_fill_startup_dummy_pixels, 4);
    assert_eq!(snapshot.bg_fetcher_post_alignment_restart_delay_dots, 1);
    assert_eq!(
        snapshot.bg_transfer_phase,
        PpuMode3TransferPhaseSnapshot::Output
    );
    assert_eq!(snapshot.bg_current_transfer_x, 12);
}

#[test]
fn scheduler_trace_reports_mode3_startup_and_cached_slice_observability() {
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
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state
        .fetcher
        .post_alignment_fetch_restart_delay_dots = 1;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;
    ppu.bg_pipeline_state.push.cached =
        BgCachedSlice::default().with_origin(BgCachedSliceOrigin::StartupAlignmentFill);
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.startup_dummy_pixels = 4;
    ppu.bg_pipeline_state.fill.cached = BgCachedSlice::default().with_origin(
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3),
    );
    ppu.bg_pipeline_state.fifo.push_back(2);
    ppu.bg_pipeline_state
        .fifo_cached_pixels
        .push_back(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                ..BgCachedSlice::default()
            },
            5,
        )));
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 3 };
    ppu.bg_pipeline_state.begin_post_alignment_followup();
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 12;
    ppu.bg_pipeline_state.visible_pixels_output = 9;

    let trace = ppu.scheduler_trace_message(&CycleContext::for_cycle(TCycle::new(123)));

    assert!(trace.contains("t_cycle=123"));
    assert!(trace.contains("bg_source=Background"));
    assert!(trace.contains("bg_stage=TileDataHigh"));
    assert!(trace.contains("bg_stage_dot=1"));
    assert!(trace.contains("bg_fetch_origin=StartupContinuation(VisibleTile2)"));
    assert!(trace.contains("bg_push_pending=true"));
    assert!(trace.contains("bg_push_entry_delay_remaining=1"));
    assert!(trace.contains("bg_push_origin=StartupAlignmentFill"));
    assert!(trace.contains("bg_fill_pending=true"));
    assert!(trace.contains("bg_fill_startup_dummy_pixels=4"));
    assert!(trace.contains("bg_fill_origin=StartupContinuation(VisibleTile3)"));
    assert!(trace.contains("bg_fifo_len=1"));
    assert!(trace.contains("bg_startup_fifo_placeholders=2"));
    assert!(trace.contains("bg_fifo_front_cached_origin=Some(StartupContinuation(VisibleTile3))"));
    assert!(trace.contains("bg_fifo_front_cached_fetch_x=Some(16)"));
    assert!(trace.contains("bg_fifo_front_cached_pixel_index=Some(5)"));
    assert!(trace.contains("bg_startup_source_state=Abstract { remaining: 3 }"));
    assert!(trace.contains("bg_startup_fetch_seam=PostAlignment"));
    assert!(trace.contains("bg_fetcher_post_alignment_restart_delay_dots=1"));
    assert!(trace.contains("bg_transfer_phase=Output"));
    assert!(trace.contains("bg_current_transfer_x=12"));
    assert!(trace.contains("bg_current_transfer_lane=Some(Visible)"));
    assert!(trace.contains("bg_current_transfer_source_window=Some(FifoBacked)"));
    assert!(trace.contains("bg_current_transfer_backing=Some(FifoBacked)"));
    assert!(trace.contains("bg_current_transfer_readiness=Some(Ready)"));
    assert!(trace.contains("bg_current_transfer_kind=Some(ServedVisiblePixel)"));
    assert!(trace.contains("visible_pixels_output=9"));
}

#[test]
fn snapshot_and_trace_export_current_transfer_context_observability() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);

    let snapshot = ppu.snapshot();

    assert_eq!(
        snapshot.bg_current_transfer_lane,
        Some(PpuMode3TransferLaneSnapshot::Visible)
    );
    assert_eq!(
        snapshot.bg_current_transfer_source_window,
        Some(PpuMode3TransferSourceWindowSnapshot::FifoBacked)
    );
    assert_eq!(
        snapshot.bg_current_transfer_backing,
        Some(PpuMode3TransferBackingSnapshot::FifoBacked)
    );
    assert_eq!(
        snapshot.bg_current_transfer_readiness,
        Some(PpuMode3TransferReadinessSnapshot::Ready)
    );
    assert_eq!(
        snapshot.bg_current_transfer_kind,
        Some(PpuMode3TransferDotKindSnapshot::ServedVisiblePixel)
    );

    let trace = ppu.scheduler_trace_message(&CycleContext::for_cycle(TCycle::new(123)));

    assert!(trace.contains("bg_current_transfer_lane=Some(Visible)"));
    assert!(trace.contains("bg_current_transfer_source_window=Some(FifoBacked)"));
    assert!(trace.contains("bg_current_transfer_backing=Some(FifoBacked)"));
    assert!(trace.contains("bg_current_transfer_readiness=Some(Ready)"));
    assert!(trace.contains("bg_current_transfer_kind=Some(ServedVisiblePixel)"));
}

#[test]
fn visible_fifo_second_startup_tile_marks_live_tilemap_refetch_on_lcdc3_write() {
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
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state
        .fifo_cached_pixels
        .push_back(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile2,
                ),
                fetch_x: BG_TILE_WIDTH as u16,
                tile_map_address: 0x1801,
                tile_data_address: 0x0001,
                tile_low: 0x12,
                tile_high: 0x34,
                ..BgCachedSlice::default()
            },
            2,
        )));

    ppu.write_register(0xFF40, 0x99);

    let cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("visible FIFO pixel should keep cached slice metadata");
    assert!(cached.cached.needs_live_tilemap_refetch);
}

#[test]
fn visible_fifo_third_startup_tile_marks_live_tilemap_refetch_on_lcdc3_write() {
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
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state
        .fifo_cached_pixels
        .push_back(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                tile_map_address: 0x1802,
                tile_data_address: 0x0001,
                tile_low: 0x12,
                tile_high: 0x34,
                ..BgCachedSlice::default()
            },
            0,
        )));

    ppu.write_register(0xFF40, 0x99);

    let cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("visible FIFO pixel should keep cached slice metadata");
    assert!(cached.cached.needs_live_tilemap_refetch);
}

#[test]
fn visible_fifo_visible_output_recomputes_marked_second_tilemap_pixel_on_demand() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tilemap_entry(&mut vram_bytes, 1, 0, 0);
    write_window_tilemap_entry(&mut vram_bytes, 1, 0, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x00, 0x00);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xFF, 0x00);
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
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state
        .fifo_cached_pixels
        .push_back(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile2,
                ),
                fetch_x: BG_TILE_WIDTH as u16,
                tile_map_address: 0x1801,
                tile_data_address: 0x0001,
                tile_index: 0,
                tile_low: 0x00,
                tile_high: 0x00,
                ..BgCachedSlice::default()
            },
            0,
        )));

    ppu.write_register(0xFF40, 0x99);

    let result =
        ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));

    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[0], 1);
}

#[test]
fn visible_fifo_visible_output_recomputes_marked_tilemap_pixel_on_demand() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tilemap_entry(&mut vram_bytes, 2, 0, 0);
    write_window_tilemap_entry(&mut vram_bytes, 2, 0, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x00, 0x00);
    write_bg_tile_row(&mut vram_bytes, 1, 0, 0xFF, 0x00);
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
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS - 1;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state
        .fifo_cached_pixels
        .push_back(Some(BgFifoPixelCached::new(
            BgCachedSlice {
                source: PpuBgFetcherSource::Background,
                origin: BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                ),
                fetch_x: BG_TILE_WIDTH as u16 * 2,
                tile_map_address: 0x1802,
                tile_data_address: 0x0001,
                tile_index: 0,
                tile_low: 0x00,
                tile_high: 0x00,
                ..BgCachedSlice::default()
            },
            0,
        )));

    ppu.write_register(0xFF40, 0x99);

    let result =
        ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));

    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[0], 1);
}

#[test]
fn traced_lcdc3_write_on_visible_tile2_tail_keeps_tail_pixels_live_and_retargets_visible_tile3() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_window_tilemap_entry(&mut vram_bytes, 1, 5, 0);
    write_window_tilemap_entry(&mut vram_bytes, 2, 5, 0);
    write_bg_tilemap_entry(&mut vram_bytes, 1, 5, 1);
    write_bg_tilemap_entry(&mut vram_bytes, 2, 5, 1);
    let old_tile_row = 0x1000 + TILE_ROW_BYTES as usize;
    vram_bytes[old_tile_row] = 0x00;
    vram_bytes[old_tile_row + 1] = 0x00;
    let new_tile_row = 0x1010 + TILE_ROW_BYTES as usize;
    vram_bytes[new_tile_row] = 0xFF;
    vram_bytes[new_tile_row + 1] = 0x00;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x8B,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 41,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 112;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 263;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 3;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 18;
    ppu.bg_pipeline_state.visible_pixels_output = 10;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupContinuation(
                BgStartupContinuationSlice::VisibleTile2,
            ),
            fetch_x: BG_TILE_WIDTH as u16,
            tile_map_address: 0x1CA1,
            tile_data_address: 0x1003,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 3;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1CA2;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x1003;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x00;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x00;

    ppu.write_register(0xFF40, 0x83);

    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("visible tail should keep cached slice metadata after the traced write");
    assert_eq!(front_cached.pixel_index, 2);
    assert!(front_cached.cached.needs_live_tilemap_refetch);
    assert!(
        ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_refetch_on_push
    );

    for expected_visible_x in 10..14 {
        ppu.maybe_recompute_pending_background_fill(&VramBusView::new(BusMaster::Ppu, &mut vram));
        ppu.flush_pending_bg_fifo_fill();
        let result =
            ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 1);
        let _ = ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram));
        ppu.line_dot += 1;
    }

    let remaining_visible_tile2 = ppu
        .bg_pipeline_state
        .fifo_cached_pixels
        .iter()
        .flatten()
        .filter(|cached| {
            cached.cached.origin
                == BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile2,
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(remaining_visible_tile2.len(), 2);
    assert!(
        remaining_visible_tile2
            .iter()
            .all(|cached| cached.cached.needs_live_tilemap_refetch)
    );

    let first_visible_tile3 = ppu
        .bg_pipeline_state
        .fifo_cached_pixels
        .iter()
        .flatten()
        .find(|cached| {
            cached.cached.origin
                == BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                )
        })
        .expect("traced write should still enqueue the retargeted VisibleTile3 slice");
    assert_eq!(first_visible_tile3.cached.tile_map_address, 0x18A2);
    assert_eq!(first_visible_tile3.cached.tile_index, 1);
    assert!(!first_visible_tile3.cached.needs_live_tilemap_refetch);
}

#[test]
fn traced_lcdc3_write_on_visible_tile2_earlier_tail_keeps_tail_pixels_live_and_retargets_visible_tile3()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_window_tilemap_entry(&mut vram_bytes, 1, 5, 0);
    write_window_tilemap_entry(&mut vram_bytes, 2, 5, 0);
    write_bg_tilemap_entry(&mut vram_bytes, 1, 5, 1);
    write_bg_tilemap_entry(&mut vram_bytes, 2, 5, 1);
    let old_tile_row = 0x1000 + TILE_ROW_BYTES as usize;
    vram_bytes[old_tile_row] = 0x00;
    vram_bytes[old_tile_row + 1] = 0x00;
    let new_tile_row = 0x1010 + TILE_ROW_BYTES as usize;
    vram_bytes[new_tile_row] = 0xFF;
    vram_bytes[new_tile_row + 1] = 0x00;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x8B,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 36,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 112;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 263;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 17;
    ppu.bg_pipeline_state.visible_pixels_output = 9;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupContinuation(
                BgStartupContinuationSlice::VisibleTile2,
            ),
            fetch_x: BG_TILE_WIDTH as u16,
            tile_map_address: 0x1CA1,
            tile_data_address: 0x1003,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile3);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 3;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x1CA2;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x1003;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x00;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x00;

    ppu.write_register(0xFF40, 0x83);

    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("visible earlier tail should keep cached slice metadata after the traced write");
    assert_eq!(front_cached.pixel_index, 1);
    assert!(front_cached.cached.needs_live_tilemap_refetch);
    assert!(
        ppu.bg_pipeline_state
            .fetcher
            .needs_live_tilemap_refetch_on_push
    );

    for expected_visible_x in 9..14 {
        ppu.maybe_recompute_pending_background_fill(&VramBusView::new(BusMaster::Ppu, &mut vram));
        ppu.flush_pending_bg_fifo_fill();
        let result =
            ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 1);
        let _ = ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram));
        ppu.line_dot += 1;
    }

    let remaining_visible_tile2 = ppu
        .bg_pipeline_state
        .fifo_cached_pixels
        .iter()
        .flatten()
        .filter(|cached| {
            cached.cached.origin
                == BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile2,
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(remaining_visible_tile2.len(), 2);
    assert!(
        remaining_visible_tile2
            .iter()
            .all(|cached| cached.cached.needs_live_tilemap_refetch)
    );

    let first_visible_tile3 = ppu
        .bg_pipeline_state
        .fifo_cached_pixels
        .iter()
        .flatten()
        .find(|cached| {
            cached.cached.origin
                == BgCachedSliceOrigin::StartupContinuation(
                    BgStartupContinuationSlice::VisibleTile3,
                )
        })
        .expect("traced earlier write should still enqueue the retargeted VisibleTile3 slice");
    assert_eq!(first_visible_tile3.cached.tile_map_address, 0x18A2);
    assert_eq!(first_visible_tile3.cached.tile_index, 1);
    assert!(!first_visible_tile3.cached.needs_live_tilemap_refetch);
}

#[test]
fn traced_lcdc4_write_behind_startup_alignment_fill_retains_visible_tile2_until_first_handoff() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 1, 0x00, 0x00);
    vram_bytes[0x0002] = 0xFF;
    vram_bytes[0x0003] = 0x00;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x83,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 41,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 104;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 263;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 3;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 10;
    ppu.bg_pipeline_state.visible_pixels_output = 2;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupAlignmentFill,
            tile_map_address: 0x18A0,
            tile_data_address: 0x1003,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x18A1;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x1003;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x00;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x00;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_map_address: 0x18A1,
        tile_data_address: 0x1003,
        tile_index: 0,
        tile_low: 0x00,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };

    ppu.write_register(0xFF40, 0x93);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_refetch
    );

    for expected_visible_x in 2..8 {
        ppu.maybe_recompute_pending_background_fill(&VramBusView::new(BusMaster::Ppu, &mut vram));
        ppu.flush_pending_bg_fifo_fill();
        let result =
            ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 0);
        let _ = ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram));
        ppu.line_dot += 1;
    }

    assert!(!ppu.bg_pipeline_state.push.pending);
    assert!(!ppu.bg_pipeline_state.fill.pending);
    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("VisibleTile2 should be at the FIFO front before the first visible handoff");
    assert_eq!(
        front_cached.cached.origin,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
    );
    assert_eq!(front_cached.pixel_index, 0);

    ppu.maybe_recompute_pending_background_fill(&VramBusView::new(BusMaster::Ppu, &mut vram));
    ppu.flush_pending_bg_fifo_fill();
    let result =
        ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));
    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[8], 1);
}

#[test]
fn traced_lcdc4_write_after_first_left_edge_pixel_still_retargets_visible_tile2_handoff() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_bg_tile_row(&mut vram_bytes, 0, 1, 0x00, 0x00);
    vram_bytes[0x0002] = 0xFF;
    vram_bytes[0x0003] = 0x00;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x83,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 36,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 103;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 263;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 8;
    ppu.bg_pipeline_state.visible_pixels_output = 0;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupAlignmentFill,
            tile_map_address: 0x18A0,
            tile_data_address: 0x1003,
            tile_index: 0,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.cached_origin =
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2);
    ppu.bg_pipeline_state.fetcher.fetch_x = BG_TILE_WIDTH as u16;
    ppu.bg_pipeline_state.fetcher.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.fetcher.tile_map_address = 0x18A1;
    ppu.bg_pipeline_state.fetcher.tile_data_address = 0x1003;
    ppu.bg_pipeline_state.fetcher.tile_index = 0;
    ppu.bg_pipeline_state.fetcher.tile_low = 0x00;
    ppu.bg_pipeline_state.fetcher.tile_high = 0x00;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.next_fetch_pixel = BG_TILE_WIDTH as u16 * 2;
    ppu.bg_pipeline_state.push.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_map_address: 0x18A1,
        tile_data_address: 0x1003,
        tile_index: 0,
        tile_low: 0x00,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };

    let first_dot =
        ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));
    assert_eq!(first_dot.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[0], 0);
    let _ = ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram));
    ppu.line_dot += 1;

    ppu.write_register(0xFF40, 0x93);
    assert!(
        ppu.bg_pipeline_state
            .push
            .cached
            .needs_live_tile_data_refetch
    );

    for expected_visible_x in 1..8 {
        ppu.maybe_recompute_pending_background_fill(&VramBusView::new(BusMaster::Ppu, &mut vram));
        ppu.flush_pending_bg_fifo_fill();
        let result =
            ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 0);
        let _ = ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram));
        ppu.line_dot += 1;
    }

    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("VisibleTile2 should reach the FIFO front after the alignment fill tail");
    assert_eq!(
        front_cached.cached.origin,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
    );
    assert_eq!(front_cached.pixel_index, 0);

    ppu.maybe_recompute_pending_background_fill(&VramBusView::new(BusMaster::Ppu, &mut vram));
    ppu.flush_pending_bg_fifo_fill();
    let handoff_dot =
        ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));
    assert_eq!(handoff_dot.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[8], 1);
}

#[test]
fn traced_startup_alignment_fill_keeps_front_visible_pixels_before_visible_tile2_handoff() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; TEST_VRAM_BYTES]);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x8B,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 36,
        lyc: 0x00,
        bgp: 0xE4,
        wy: 0x96,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 107;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 263;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = 0;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.startup_fetch_seam = BgStartupFetchSeamState::PostAlignment {
        first_real_push_skips_entry_delay: false,
        next_startup_continuation_slice: BgStartupContinuationSlice::VisibleTile3,
        startup_continuation_visible_tiles_remaining: 1,
        delayed_background_tileindex_read_tiles_remaining: 0,
        delayed_background_tilemap_tiles_remaining: 0,
        delayed_background_tiledata_tiles_remaining: 0,
    };
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.current_transfer_x = 12;
    ppu.bg_pipeline_state.visible_pixels_output = 4;
    ppu.bg_pipeline_state
        .push_cached_slice_fifo_pixels(BgCachedSlice {
            source: PpuBgFetcherSource::Background,
            origin: BgCachedSliceOrigin::StartupAlignmentFill,
            tile_low: 0x00,
            tile_high: 0x00,
            ..BgCachedSlice::default()
        });
    for _ in 0..4 {
        let _ = ppu.bg_pipeline_state.pop_real_fifo_pixel();
    }
    ppu.bg_pipeline_state.fill.pending = true;
    ppu.bg_pipeline_state.fill.includes_real_tile_pixels = true;
    ppu.bg_pipeline_state.fill.cached = BgCachedSlice {
        source: PpuBgFetcherSource::Background,
        origin: BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2),
        fetch_x: BG_TILE_WIDTH as u16,
        tile_map_address: 0x1CA1,
        tile_data_address: 0x1003,
        tile_index: 1,
        tile_low: 0xFF,
        tile_high: 0x00,
        ..BgCachedSlice::default()
    };

    ppu.flush_pending_bg_fifo_fill();

    let front_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("startup alignment fill should still be at the FIFO front after the flush");
    assert_eq!(
        front_cached.cached.origin,
        BgCachedSliceOrigin::StartupAlignmentFill
    );
    assert_eq!(front_cached.pixel_index, 4);

    for expected_visible_x in 4..8 {
        let result =
            ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));
        assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
        assert_eq!(ppu.current_scanline_pixels[expected_visible_x], 0);
        ppu.line_dot += 1;
    }

    let next_cached = ppu.bg_pipeline_state.fifo_cached_pixels[0]
        .expect("VisibleTile2 should take ownership once the alignment fill tail is gone");
    assert_eq!(
        next_cached.cached.origin,
        BgCachedSliceOrigin::StartupContinuation(BgStartupContinuationSlice::VisibleTile2)
    );
    assert_eq!(next_cached.pixel_index, 0);

    let result =
        ppu.advance_mode3_output_phase_with_vram(&VramBusView::new(BusMaster::Ppu, &mut vram));
    assert_eq!(result.kind, Mode3TransferDotKind::ServedVisiblePixel);
    assert_eq!(ppu.current_scanline_pixels[8], 1);
}

#[test]
fn queued_fill_from_real_push_preserves_the_same_tcycle_tilemap_refetch_window() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);

    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.disposition = BgPushDisposition::Ready;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state.push.cached.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state
        .push
        .cached
        .same_cycle_live_tilemap_refetch_window_open = true;

    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::QueuedFill);
    assert!(ppu.bg_pipeline_state.fill.pending);
    assert!(
        ppu.bg_pipeline_state
            .fill
            .cached
            .same_cycle_live_tilemap_refetch_window_open
    );
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
fn bg_fetcher_rereads_the_unsigned_tile_data_byte_when_tile_selector_flips_to_unsigned_on_low1() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    vram_bytes[0x1010] = 0x12;
    vram_bytes[0x0010] = 0x56;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x81;
    ppu.pipeline_registers.lcdc = 0x81;
    ppu.ly = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Background;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x12);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1010);

    ppu.pipeline_registers.lcdc = 0x81;
    ppu.visible_registers.lcdc = 0x91;
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_low, 0x56);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0010);
    assert_eq!(
        ppu.bg_pipeline_state.fetcher.stage,
        PpuBgFetcherStage::TileDataHigh
    );
}

#[test]
fn window_fetcher_rereads_the_unsigned_tile_data_byte_when_tile_selector_flips_to_unsigned_on_high1()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    vram_bytes[0x1011] = 0x34;
    vram_bytes[0x0011] = 0x78;
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x81;
    ppu.pipeline_registers.lcdc = 0x81;
    ppu.window_state.window_line_counter = 0;
    ppu.bg_pipeline_state.fetcher.source = PpuBgFetcherSource::Window;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.fetcher.tile_index = 1;

    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x34);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x1011);

    ppu.pipeline_registers.lcdc = 0x81;
    ppu.visible_registers.lcdc = 0x91;
    assert!(!ppu.advance_bg_fetcher(&VramBusView::new(BusMaster::Ppu, &mut vram)));
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_high, 0x78);
    assert_eq!(ppu.bg_pipeline_state.fetcher.tile_data_address, 0x0011);
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

    let (tile_index, attributes) =
        read_obj_fetch_sprite_metadata(&oam, sprite, Some(PpuDmaOamConflict::new(0xFE17, 0x77)));

    assert_eq!(tile_index, 0x99);
    assert_eq!(attributes, 0x77);
}

#[test]
fn object_fetch_uses_the_current_dma_byte_for_even_conflict_word_reads() {
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

    let (tile_index, attributes) =
        read_obj_fetch_sprite_metadata(&oam, sprite, Some(PpuDmaOamConflict::new(0xFE16, 0x66)));

    assert_eq!(tile_index, 0x66);
    assert_eq!(attributes, 0x10);
}

#[test]
#[ignore = "diagnostic probe for hacktix strikethrough line 68 DMA/OBJ overlap"]
fn sample_real_hacktix_strikethrough_line68_dma_obj_overlap() {
    for target_ly in 64..=72 {
        let (selected_sprites, events, segment, framebuffer_segment) =
            sample_hacktix_strikethrough_line(target_ly, 64);

        println!("ly={target_ly} selected_sprites={selected_sprites:#?}");
        println!("ly={target_ly} line_pixels_71_79={segment:?}");
        println!("ly={target_ly} framebuffer_71_79={framebuffer_segment:?}");
        for event in &events {
            println!("ly={target_ly} {event:?}");
        }
    }

    let (selected_sprites, events, _, _) = sample_hacktix_strikethrough_line(68, 64);
    assert!(!selected_sprites.is_empty() || !events.is_empty());
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
fn cpu_mmio_bgp_write_during_mode3_keeps_the_previous_palette_for_four_visible_pixels() {
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
    ppu.line_dot = 200;
    ppu.bg_pipeline_state.visible_pixels_output = 25;
    ppu.bg_pipeline_state.current_transfer_x = 33;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    for _ in 0..5 {
        ppu.bg_pipeline_state.fifo.push_back(1);
    }
    ppu.current_scanline_mixed_pixels[21..25].fill(MixedPixel::background(1));
    ppu.framebuffer[21..25].fill(1);

    ppu.write_register_with_source(0xFF47, 0xAA, PpuRegisterWriteSource::CpuMmioCommit);

    assert_eq!(&ppu.framebuffer()[21..25], &[1, 1, 1, 1]);

    for _ in 0..5 {
        ppu.sync_pipeline_registers();
        ppu.sync_visible_registers();
        let _ = ppu.advance_mode3_output_phase();
    }

    assert_eq!(&ppu.framebuffer()[25..30], &[1, 1, 1, 1, 2]);
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

        let mut t_cycle = 0;
        loop {
            tick_ppu_with_vram(&mut ppu, t_cycle, oam_bytes, vram_bytes);
            t_cycle += 1;

            let fetching = ppu.snapshot();
            if fetching.obj_fetcher_stage != PpuObjFetcherStage::Idle {
                assert!(
                    ppu.current_access_mode() == PpuAccessMode::Drawing,
                    "left-edge OBJ fetch must still begin during Mode 3"
                );
                assert!(
                    fetching.visible_pixels_output <= 1,
                    "left-edge OBJ fetch should still begin around the left edge"
                );
                break;
            }

            assert!(
                t_cycle < 160,
                "sprite fetch should begin during early Mode 3"
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
            if snapshot.obj_fetcher_stage != PpuObjFetcherStage::Idle {
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
fn first_hidden_same_x_cluster_fetch_can_skip_obj_tile_data_low_byte_when_bg_fetcher_is_on_tile_data_high_1()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 12, 0);
    write_oam_entry(&mut oam_bytes, 1, 16, 12, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    oam.set_acquired(BusMaster::Ppu, true);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 8;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 4;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 12));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;

    let current_sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 12,
        tile_index: 0,
        attributes: 0,
    };
    let next_sprite = PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 12,
        tile_index: 1,
        attributes: 0,
    };
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(4);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.start_fetch(0, current_sprite);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Startup;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
}

#[test]
fn first_hidden_same_x_cluster_fetch_at_x_six_keeps_the_low_byte_half_step() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 14, 0);
    write_oam_entry(&mut oam_bytes, 1, 16, 14, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    oam.set_acquired(BusMaster::Ppu, true);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 12;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 6;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 10));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;

    let current_sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 14,
        tile_index: 0,
        attributes: 0,
    };
    let next_sprite = PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 14,
        tile_index: 1,
        attributes: 0,
    };
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(6);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.start_fetch(0, current_sprite);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Startup;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite, Some(current_sprite));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.resolved_sprite,
        Some(current_sprite)
    );
}

#[test]
fn first_hidden_same_x_cluster_fetch_at_x_seven_keeps_the_full_low_byte() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 15, 0);
    write_oam_entry(&mut oam_bytes, 1, 16, 15, 1);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    oam.set_acquired(BusMaster::Ppu, true);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 14;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;

    let current_sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 15,
        tile_index: 0,
        attributes: 0,
    };
    let next_sprite = PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 15,
        tile_index: 1,
        attributes: 0,
    };
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(7);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.start_fetch(0, current_sprite);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Startup;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite, Some(current_sprite));
    assert_eq!(
        ppu.obj_pipeline_state.fetch.resolved_sprite,
        Some(current_sprite)
    );
}

#[test]
fn same_x_cluster_at_x_mod_8_eq_2_waits_until_the_next_dot_for_startup() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 2;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 6;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 14));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    let current_sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 2,
        tile_index: 0,
        attributes: 0,
    };
    let next_sprite = PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 2,
        tile_index: 1,
        attributes: 0,
    };
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(2);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(0);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);

    assert!(!ppu.try_start_object_fetch_from_current_dot(
        ObjFetchStartSource::FifoBackedTransfer,
        true,
    ));
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
}

#[test]
fn terminal_fifo_backed_obj_start_extends_mode3_immediately_to_keep_fetch_alive() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 167, 0);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    oam.set_acquired(BusMaster::Ppu, true);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = MODE0_START_DOT - 1;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 167,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    assert!(ppu.advance_mode3_object_phase(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
}

#[test]
fn late_visible_x160_obj_start_can_still_begin_from_fifo_backed_transfer() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut oam_bytes = [0; 160];
    let mut vram_bytes = [0; TEST_VRAM_BYTES];

    write_oam_entry(&mut oam_bytes, 0, 16, 160, 0);
    write_bg_tile_row(&mut vram_bytes, 0, 0, 0x55, 0x33);

    let mut oam = crate::bus::OamDomain::from_bytes(&oam_bytes);
    let mut vram = crate::bus::VramDomain::from_bytes(&vram_bytes);
    oam.set_acquired(BusMaster::Ppu, true);
    vram.set_acquired(BusMaster::Ppu, true);

    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 66;
    ppu.line_dot = MODE0_START_DOT - 1;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 160;
    ppu.bg_pipeline_state.visible_pixels_output = 152;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
    ppu.bg_pipeline_state.startup_fifo_placeholders = 2;
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 160,
        tile_index: 0,
        attributes: 0,
    });
    ppu.obj_pipeline_state
        .queue_fetch_hit(0, ppu.current_obj_hit_ownership());

    let transfer = ppu
        .current_transfer()
        .expect("late visible x160 should still have a transfer");
    assert_eq!(transfer.context.lane, Mode3TransferLane::Visible);
    assert!(transfer.can_start_obj_fetch_from_fifo_backed_transfer(
        ppu.bg_pipeline_state.fifo_contains_real_pixels()
    ));

    let arbitration = ppu.current_dot_arbitration();
    assert!(arbitration.can_start_obj_fetch(ObjFetchStartSource::FifoBackedTransfer));

    assert!(ppu.advance_mode3_object_phase(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
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
fn long_same_x_obj_chain_waits_one_dot_before_the_terminal_restart() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.current_transfer_x = 0;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 8;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 16));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    for sprite_slot in 0..6_u8 {
        let sprite = PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: 8,
            tile_index: sprite_slot,
            attributes: 0,
        };
        ppu.mode2_scan_state.push(sprite);
        if sprite_slot < 5 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = ppu
        .mode2_scan_state
        .selected_sprite(4)
        .expect("sprite slot 4 should exist");
    ppu.obj_pipeline_state.pending_match_x = Some(0);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(5);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = 4;
    ppu.obj_pipeline_state.fetch.sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;

    let mut oam = crate::bus::OamDomain::from_bytes(&[0; 160]);
    oam.set_acquired(BusMaster::Ppu, true);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
    vram.set_acquired(BusMaster::Ppu, true);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(
        ppu.bg_pipeline_state.mode0_start_dot,
        MODE0_START_DOT,
        "fetch={:?} pending={:?} match_x={:?}",
        ppu.obj_pipeline_state.fetch,
        ppu.obj_pipeline_state.pending_sprite_slots,
        ppu.obj_pipeline_state.pending_match_x
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(0));
    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![5]
    );
}

#[test]
fn visible_same_x_obj_chain_with_early_start_does_not_use_long_tail_restart() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 1;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    for sprite_slot in 0..10_u8 {
        let sprite = PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: 167,
            tile_index: sprite_slot,
            attributes: 0,
        };
        ppu.mode2_scan_state.push(sprite);
        if sprite_slot < 9 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = ppu
        .mode2_scan_state
        .selected_sprite(8)
        .expect("sprite slot 8 should exist");
    ppu.obj_pipeline_state.pending_match_x = Some(167);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(9);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = 8;
    ppu.obj_pipeline_state.fetch.sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;

    let mut oam = crate::bus::OamDomain::from_bytes(&[0; 160]);
    oam.set_acquired(BusMaster::Ppu, true);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
    vram.set_acquired(BusMaster::Ppu, true);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(167));
    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![9]
    );
    assert!(!ppu.obj_pipeline_state.fetch.count_terminal_push_dot);
}

#[test]
fn x_mod_8_eq_2_same_x_obj_chain_restart_reuses_the_current_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 16;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 10;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 6;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 14));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    let current_sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 2,
        tile_index: 0,
        attributes: 0,
    };
    let next_sprite = PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 2,
        tile_index: 1,
        attributes: 0,
    };
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(2);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = 0;
    ppu.obj_pipeline_state.fetch.sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;

    let mut oam = crate::bus::OamDomain::from_bytes(&[0; 160]);
    oam.set_acquired(BusMaster::Ppu, true);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
    vram.set_acquired(BusMaster::Ppu, true);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 10);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite_slot, 1);
}

#[test]
fn x_mod_8_eq_3_same_x_obj_chain_restart_reuses_the_current_dot() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 16;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 10;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 3;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 5;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 14));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    let current_sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 3,
        tile_index: 0,
        attributes: 0,
    };
    let next_sprite = PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 3,
        tile_index: 1,
        attributes: 0,
    };
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(3);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = 0;
    ppu.obj_pipeline_state.fetch.sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;

    let mut oam = crate::bus::OamDomain::from_bytes(&[0; 160]);
    oam.set_acquired(BusMaster::Ppu, true);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
    vram.set_acquired(BusMaster::Ppu, true);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 10);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite_slot, 1);
}

#[test]
fn terminal_previsible_x_mod_8_eq_2_same_x_chain_skips_startup_and_low_byte() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 20;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 20;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 6;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 14));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    for sprite_slot in 0..10_u8 {
        let sprite = PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: 2,
            tile_index: sprite_slot,
            attributes: 0,
        };
        ppu.mode2_scan_state.push(sprite);
        if sprite_slot < 8 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = ppu
        .mode2_scan_state
        .selected_sprite(8)
        .expect("sprite slot 8 should exist");
    ppu.obj_pipeline_state.pending_match_x = Some(2);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(9);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = 8;
    ppu.obj_pipeline_state.fetch.sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;

    let mut oam = crate::bus::OamDomain::from_bytes(&[0; 160]);
    oam.set_acquired(BusMaster::Ppu, true);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
    vram.set_acquired(BusMaster::Ppu, true);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 20);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataHigh
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 0);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite_slot, 9);
}

#[test]
fn hidden_x_mod_8_eq_4_late_same_x_chain_skips_first_low_half_step() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 24;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 20;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 4;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 12));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;

    for sprite_slot in 0..10_u8 {
        let sprite = PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: 4,
            tile_index: sprite_slot,
            attributes: 0,
        };
        ppu.mode2_scan_state.push(sprite);
        if sprite_slot < 5 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = ppu
        .mode2_scan_state
        .selected_sprite(5)
        .expect("sprite slot 5 should exist");
    ppu.obj_pipeline_state.pending_match_x = Some(4);
    for sprite_slot in 6..10_u8 {
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .push_back(sprite_slot);
    }
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = 5;
    ppu.obj_pipeline_state.fetch.sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;

    let mut oam = crate::bus::OamDomain::from_bytes(&[0; 160]);
    oam.set_acquired(BusMaster::Ppu, true);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
    vram.set_acquired(BusMaster::Ppu, true);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 21);
    assert_eq!(
        ppu.obj_pipeline_state.fetch.stage,
        PpuObjFetcherStage::TileDataLow
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage_dot, 1);
    assert_eq!(ppu.obj_pipeline_state.fetch.sprite_slot, 6);
}

#[test]
#[ignore = "diagnostic count=3 same-x push1 restart for x mod 8 == 2"]
fn x_mod_8_eq_2_count3_same_x_chain_logs_post_push1_restart_state() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 20;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 20;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 2;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 6;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, 14));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    for sprite_slot in 0..10_u8 {
        let sprite = PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: 2,
            tile_index: sprite_slot,
            attributes: 0,
        };
        ppu.mode2_scan_state.push(sprite);
        if sprite_slot < 2 {
            ppu.obj_pipeline_state.mark_fetched(sprite_slot);
        }
    }

    let current_sprite = ppu
        .mode2_scan_state
        .selected_sprite(2)
        .expect("sprite slot 2 should exist");
    ppu.obj_pipeline_state.pending_match_x = Some(2);
    for sprite_slot in 3..10_u8 {
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .push_back(sprite_slot);
    }
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = 2;
    ppu.obj_pipeline_state.fetch.sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;

    let transfer = ppu.current_transfer().expect("transfer should exist");
    println!("count3_before_transfer={transfer:?}");
    println!(
        "count3_before_previsible_can_start={} count3_before_arbitration={:?}",
        ppu.previsible_same_x_chain_can_start_obj_fetch(transfer),
        ppu.current_dot_arbitration()
    );

    let mut oam = crate::bus::OamDomain::from_bytes(&[0; 160]);
    oam.set_acquired(BusMaster::Ppu, true);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
    vram.set_acquired(BusMaster::Ppu, true);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));

    println!(
        "count3_after_fetch_stage={:?} stage_dot={} pending_match_x={:?} pending_len={} mode0_start_dot={}",
        ppu.obj_pipeline_state.fetch.stage,
        ppu.obj_pipeline_state.fetch.stage_dot,
        ppu.obj_pipeline_state.pending_match_x,
        ppu.obj_pipeline_state.pending_sprite_slots.len(),
        ppu.bg_pipeline_state.mode0_start_dot
    );
}

#[test]
fn x_mod_8_eq_7_same_x_obj_chain_restart_waits_before_reusing_the_full_low_byte() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE2_DOTS + MODE3_BG_FETCH_PRIMING_DOTS + 20;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 10;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::Abstract { remaining: 4 };
    ppu.bg_pipeline_state
        .startup_pre_visible_transfer_dots_remaining = MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS;
    ppu.bg_pipeline_state.current_transfer_x = 7;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    let current_sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 15,
        tile_index: 0,
        attributes: 0,
    };
    let next_sprite = PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 15,
        tile_index: 1,
        attributes: 0,
    };
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(next_sprite);
    ppu.obj_pipeline_state.pending_match_x = Some(7);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = 0;
    ppu.obj_pipeline_state.fetch.sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;

    let mut oam = crate::bus::OamDomain::from_bytes(&[0; 160]);
    oam.set_acquired(BusMaster::Ppu, true);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
    vram.set_acquired(BusMaster::Ppu, true);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 10);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(7));
    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );
}

#[test]
fn terminal_same_x_obj_chain_with_single_pending_slot_does_not_restart_but_still_counts_the_terminal_push_dot()
 {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.line_dot = MODE0_START_DOT;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 1;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.fifo.push_back(0);
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    let current_sprite = PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 167,
        tile_index: 0,
        attributes: 0,
    };
    ppu.mode2_scan_state.push(current_sprite);
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 167,
        tile_index: 1,
        attributes: 0,
    });
    ppu.obj_pipeline_state.pending_match_x = Some(167);
    ppu.obj_pipeline_state.pending_sprite_slots.push_back(1);
    ppu.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
    ppu.obj_pipeline_state.fetch.stage_dot = 1;
    ppu.obj_pipeline_state.fetch.sprite_slot = 0;
    ppu.obj_pipeline_state.fetch.sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.resolved_sprite = Some(current_sprite);
    ppu.obj_pipeline_state.fetch.tile_low = 0xFF;
    ppu.obj_pipeline_state.fetch.tile_high = 0x00;

    let mut oam = crate::bus::OamDomain::from_bytes(&[0; 160]);
    oam.set_acquired(BusMaster::Ppu, true);
    let mut vram = crate::bus::VramDomain::from_bytes(&[0; 0x2000]);
    vram.set_acquired(BusMaster::Ppu, true);

    assert!(ppu.advance_object_fetch(
        &OamBusView::new(BusMaster::Ppu, &mut oam),
        &VramBusView::new(BusMaster::Ppu, &mut vram),
        None,
    ));
    assert_eq!(ppu.bg_pipeline_state.mode0_start_dot, MODE0_START_DOT + 2);
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, Some(167));
    assert_eq!(
        ppu.obj_pipeline_state
            .pending_sprite_slots
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );
}

#[test]
#[ignore = "diagnostic case1 terminal x167 no-obj seam from intr_2_mode0_timing_sprites"]
fn terminal_visible_bg_transfer_without_obj_work_does_not_extend_mode3_past_x167() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = MODE0_START_DOT + 16;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 15;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 167;
    ppu.bg_pipeline_state.visible_pixels_output = 159;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 9));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
    ppu.bg_pipeline_state.fetcher.stage_dot = 1;

    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: 16,
        x: 0,
        tile_index: 0,
        attributes: 0,
    });
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 1,
        y: 16,
        x: 0,
        tile_index: 1,
        attributes: 0,
    });

    let transfer = ppu
        .current_transfer()
        .expect("terminal visible x167 should still expose the live transfer context");
    assert_eq!(transfer.context.lane, Mode3TransferLane::Visible);
    assert_eq!(
        transfer.context.source_window,
        Mode3TransferSourceWindow::FifoBacked
    );
    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, None);
    assert!(ppu.obj_pipeline_state.pending_sprite_slots.is_empty());
    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT + 16);
}

#[test]
fn saturated_placeholder_backed_terminal_bg_tail_stays_in_mode3_during_tile_data_high() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = 313;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 303;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = 160;
    ppu.bg_pipeline_state
        .saw_right_edge_visible_same_x_cluster_this_line = true;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;

    for sprite_slot in 0..10_u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: if sprite_slot < 5 { 0 } else { 160 },
            tile_index: sprite_slot,
            attributes: 0,
        });
    }

    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, None);
    assert!(ppu.obj_pipeline_state.pending_sprite_slots.is_empty());
    assert_eq!(ppu.current_mode0_start_dot(), 314);
}

#[test]
fn saturated_placeholder_backed_terminal_bg_tail_stays_in_mode3_during_push() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = 315;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 303;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = 160;
    ppu.bg_pipeline_state
        .saw_right_edge_visible_same_x_cluster_this_line = true;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;

    for sprite_slot in 0..10_u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: if sprite_slot < 5 { 0 } else { 160 },
            tile_index: sprite_slot,
            attributes: 0,
        });
    }

    assert_eq!(ppu.obj_pipeline_state.fetch.stage, PpuObjFetcherStage::Idle);
    assert_eq!(ppu.obj_pipeline_state.pending_match_x, None);
    assert!(ppu.obj_pipeline_state.pending_sprite_slots.is_empty());
    assert_eq!(ppu.current_mode0_start_dot(), 316);
}

#[test]
fn saturated_placeholder_backed_terminal_bg_tail_holds_one_extra_dot_after_push_entry_delay() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = 316;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 303;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = 160;
    ppu.bg_pipeline_state
        .saw_right_edge_visible_same_x_cluster_this_line = true;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 4;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 0;
    ppu.bg_pipeline_state
        .push
        .terminal_placeholder_tail_extra_hold_remaining = 1;

    for sprite_slot in 0..10_u8 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: if sprite_slot < 5 { 0 } else { 160 },
            tile_index: sprite_slot,
            attributes: 0,
        });
    }

    assert_eq!(ppu.current_mode0_start_dot(), 317);
    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::WaitingForEmptyFifo);
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .terminal_placeholder_tail_extra_hold_remaining,
        0
    );
}

#[test]
fn saturated_placeholder_backed_terminal_bg_tail_does_not_hold_without_right_edge_x160_cluster() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    ppu.visible_registers.lcdc = 0x82;
    ppu.ly = 68;
    ppu.line_dot = 315;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = 303;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = 168;
    ppu.bg_pipeline_state.visible_pixels_output = 160;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = 1;
    ppu.bg_pipeline_state.fifo.extend(std::iter::repeat_n(0, 8));
    ppu.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
    ppu.bg_pipeline_state.fetcher.stage_dot = 0;
    ppu.bg_pipeline_state.push.pending = true;
    ppu.bg_pipeline_state.push.entry_delay_remaining = 1;

    for sprite_slot in 0..10_u8 {
        let sprite = PpuSelectedSprite {
            oam_index: sprite_slot,
            y: 16,
            x: 7,
            tile_index: sprite_slot,
            attributes: 0,
        };
        ppu.mode2_scan_state.push(sprite);
        ppu.obj_pipeline_state.mark_fetched(sprite_slot);
    }

    assert_eq!(ppu.advance_bg_push(), BgPushDotResult::EntryDelay);
    assert_eq!(
        ppu.bg_pipeline_state
            .push
            .terminal_placeholder_tail_extra_hold_remaining,
        0
    );
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
