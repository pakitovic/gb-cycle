use super::*;
use crate::cpu::{CpuAddressEvent, CpuAddressEventKind, CpuAddressUpdateDirection};
use crate::ppu::{DmgObjPaletteReadPolicy, Ppu, PpuAccessMode, PpuBusState, PpuStartupState};
use crate::scheduler::{CycleContext, TCycle};

mod access;
mod corruption;
mod map;
mod view;

fn sync_test_video_ownership(ppu: &Ppu, oam: &mut OamDomain, vram: &mut VramDomain) {
    let bus_state = ppu.bus_state();
    let ppu_vram = bus_state.is_lcd_enabled() && bus_state.mode() == PpuAccessMode::Drawing;
    let ppu_oam = bus_state.is_lcd_enabled()
        && matches!(
            bus_state.mode(),
            PpuAccessMode::OamScan | PpuAccessMode::Drawing
        );

    oam.set_acquired(BusMaster::Ppu, ppu_oam);
    vram.set_acquired(BusMaster::Ppu, ppu_vram);
    oam.set_acquired(BusMaster::Dma, false);
    vram.set_acquired(BusMaster::Dma, false);
}

fn tick_ppu(ppu: &mut Ppu, t_cycle: u64) {
    let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
    let mut oam = OamDomain::new();
    let mut vram = VramDomain::new();
    sync_test_video_ownership(ppu, &mut oam, &mut vram);
    ppu.tick_t_cycle(
        &mut context,
        OamBusView::new(BusMaster::Ppu, &mut oam),
        VramBusView::new(BusMaster::Ppu, &mut vram),
        false,
        None,
    );
}

fn prepare_mode2_ppu_at_row(console_model: ConsoleModel, row: u8) -> Ppu {
    let mut ppu = Ppu::new(console_model);
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

    let ticks = if row == 0 { 0 } else { u64::from(row) * 4 + 1 };

    for t_cycle in 0..ticks {
        tick_ppu(&mut ppu, t_cycle);
    }

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(row));
    ppu
}

fn prepare_mode3_ppu(console_model: ConsoleModel) -> Ppu {
    let mut ppu = prepare_mode2_ppu_at_row(console_model, 0);
    for t_cycle in 0..80 {
        tick_ppu(&mut ppu, t_cycle);
    }
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
    ppu
}

fn write_oam_word_bytes(oam_bytes: &mut [u8], row: u8, word_index: usize, value: u16) {
    let word_start = row as usize * 8 + word_index * 2;
    let [low, high] = value.to_le_bytes();
    oam_bytes[word_start] = low;
    oam_bytes[word_start + 1] = high;
}

fn read_oam_word_bytes(oam_bytes: &[u8], row: u8, word_index: usize) -> u16 {
    let word_start = row as usize * 8 + word_index * 2;
    u16::from_le_bytes([oam_bytes[word_start], oam_bytes[word_start + 1]])
}

fn seed_oam_corruption_rows(oam_bytes: &mut [u8]) {
    write_oam_word_bytes(oam_bytes, 0, 0, 0x1357);
    write_oam_word_bytes(oam_bytes, 0, 1, 0x2468);
    write_oam_word_bytes(oam_bytes, 0, 2, 0xAAAA);
    write_oam_word_bytes(oam_bytes, 0, 3, 0xBBBB);
    write_oam_word_bytes(oam_bytes, 1, 0, 0x0F0F);
    write_oam_word_bytes(oam_bytes, 1, 1, 0x1111);
    write_oam_word_bytes(oam_bytes, 1, 2, 0x2222);
    write_oam_word_bytes(oam_bytes, 1, 3, 0x3333);
    write_oam_word_bytes(oam_bytes, 2, 0, 0x5555);
    write_oam_word_bytes(oam_bytes, 2, 1, 0x6666);
    write_oam_word_bytes(oam_bytes, 2, 2, 0x7777);
    write_oam_word_bytes(oam_bytes, 2, 3, 0x8888);
}
