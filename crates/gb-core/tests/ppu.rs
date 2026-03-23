use gb_core::{
    ConsoleModel, CpuAddressEventKind, CpuAddressUpdateDirection, Machine, MachineConfig,
    PpuAccessMode, PpuBgFetcherSource, PpuLcdState, PpuObjFetcherStage, PpuVisibleOutputState,
    StartupMode,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn build_test_rom(program: &[u8], boot_opcode: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = boot_opcode;
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn seed_oam_entry(machine: &mut Machine, index: u8, y: u8, x: u8, tile_index: u8, attributes: u8) {
    let entry_start = 0xFE00 + index as u16 * 4;
    machine.write_bus(entry_start, y);
    machine.write_bus(entry_start + 1, x);
    machine.write_bus(entry_start + 2, tile_index);
    machine.write_bus(entry_start + 3, attributes);
}

fn seed_bg_tile_row(machine: &mut Machine, tile_index: u8, row: u8, low: u8, high: u8) {
    let tile_address = 0x8000 + tile_index as u16 * 16 + row as u16 * 2;
    machine.write_bus(tile_address, low);
    machine.write_bus(tile_address + 1, high);
}

fn seed_bg_tilemap_entry(machine: &mut Machine, x: u8, y: u8, tile_index: u8) {
    let tile_map_address = 0x9800 + y as u16 * 32 + x as u16;
    machine.write_bus(tile_map_address, tile_index);
}

fn seed_window_tilemap_entry(machine: &mut Machine, x: u8, y: u8, tile_index: u8) {
    let tile_map_address = 0x9C00 + y as u16 * 32 + x as u16;
    machine.write_bus(tile_map_address, tile_index);
}

fn build_oam_corruption_fixture() -> [[u16; 4]; 20] {
    let mut rows = [[0u16; 4]; 20];
    for (row_index, words) in rows.iter_mut().enumerate() {
        for (word_index, word) in words.iter_mut().enumerate() {
            *word = ((row_index as u16 + 1) << 8)
                | ((word_index as u16) * 0x11)
                | (row_index as u16 & 0x0F);
        }
    }
    rows
}

fn seed_oam_corruption_fixture(machine: &mut Machine, rows: &[[u16; 4]; 20]) {
    for (row_index, words) in rows.iter().enumerate() {
        for (word_index, value) in words.iter().copied().enumerate() {
            let address = 0xFE00 + row_index as u16 * 8 + word_index as u16 * 2;
            let [low, high] = value.to_le_bytes();
            machine.write_bus(address, low);
            machine.write_bus(address + 1, high);
        }
    }
}

fn read_oam_corruption_row(machine: &mut Machine, row: u8) -> [u16; 4] {
    let mut words = [0u16; 4];
    for (word_index, word) in words.iter_mut().enumerate() {
        let address = 0xFE00 + row as u16 * 8 + word_index as u16 * 2;
        let low = machine.read_bus(address);
        let high = machine.read_bus(address + 1);
        *word = u16::from_le_bytes([low, high]);
    }
    words
}

fn expected_write_corruption(rows: &[[u16; 4]; 20], row: u8) -> [u16; 4] {
    let current = rows[row as usize];
    let previous = rows[row as usize - 1];
    [
        ((current[0] ^ previous[2]) & (previous[0] ^ previous[2])) ^ previous[2],
        previous[1],
        previous[2],
        previous[3],
    ]
}

fn expected_read_corruption(rows: &[[u16; 4]; 20], row: u8) -> [u16; 4] {
    let current = rows[row as usize];
    let previous = rows[row as usize - 1];
    [
        previous[0] | (current[0] & previous[2]),
        previous[1],
        previous[2],
        previous[3],
    ]
}

fn step_until_line_dot(machine: &mut Machine, target_line_dot: u16) {
    while machine.ppu().snapshot().line_dot < target_line_dot {
        machine.step_t_cycle();
    }
}

fn step_until_hblank(machine: &mut Machine) {
    while machine.ppu().snapshot().mode != PpuAccessMode::HBlank {
        machine.step_t_cycle();
    }
}

fn step_until_position(machine: &mut Machine, target_ly: u8, target_line_dot: u16) {
    while !(machine.ppu().snapshot().ly == target_ly
        && machine.ppu().snapshot().line_dot == target_line_dot)
    {
        machine.step_t_cycle();
    }
}

fn step_until_next_frame_start(machine: &mut Machine) {
    let mut stepped = false;
    while !(stepped && machine.ppu().snapshot().ly == 0 && machine.ppu().snapshot().line_dot == 0) {
        machine.step_t_cycle();
        stepped = true;
    }
}

#[test]
fn skip_boot_ppu_state_continues_from_the_published_snapshot_on_the_shared_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    let startup = machine.ppu().snapshot();
    assert_eq!(startup.mode, PpuAccessMode::VBlank);
    assert_eq!(startup.ly, 0);
    assert_eq!(startup.line_dot, 0);
    assert_eq!(startup.lcd_state, PpuLcdState::Enabled);
    assert_eq!(startup.visible_output, PpuVisibleOutputState::Driving);

    machine.step_t_cycle();

    let after_first_dot = machine.ppu().snapshot();
    assert_eq!(after_first_dot.mode, PpuAccessMode::OamScan);
    assert_eq!(after_first_dot.ly, 0);
    assert_eq!(after_first_dot.line_dot, 1);
    assert_eq!(after_first_dot.mode_dot, 1);
}

#[test]
fn live_machine_bus_access_uses_the_current_ppu_mode_from_the_raster_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();
    assert_eq!(machine.ppu().snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);

    for _ in 1..80 {
        machine.step_t_cycle();
    }

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(drawing.line_dot, 80);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
}

#[test]
fn mode0_stat_request_appears_on_the_same_t_cycle_that_vram_unblocks() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0x8000, 0x12);
    machine.write_bus(0xFF45, 0x01);
    step_until_line_dot(&mut machine, 80);
    machine.write_bus(0xFF41, 0x08);
    machine.write_bus(0xFF0F, 0x00);

    step_until_line_dot(&mut machine, 251);

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x03);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    machine.step_t_cycle();

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.line_dot, 252);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x00);
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);
    assert_eq!(machine.read_bus(0x8000), 0x12);
}

#[test]
fn lcd_disabled_machine_state_keeps_the_ppu_raster_frozen() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0x00);

    for _ in 0..8 {
        machine.step_t_cycle();
    }

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.lcd_state, PpuLcdState::Disabled);
    assert_eq!(snapshot.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!snapshot.blank_frame_active);
    assert_eq!(snapshot.ly, 0);
    assert_eq!(snapshot.line_dot, 0);
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
}

#[test]
fn mid_scanline_lcdc7_disable_resets_the_raster_and_releases_ppu_bus_blocking() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0x8000, 0x12);
    step_until_line_dot(&mut machine, 100);

    assert_eq!(machine.ppu().snapshot().mode, PpuAccessMode::Drawing);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);

    machine.write_bus(0xFF40, 0x00);

    let disabled = machine.ppu().snapshot();
    assert_eq!(disabled.lcd_state, PpuLcdState::Disabled);
    assert_eq!(disabled.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!disabled.blank_frame_active);
    assert_eq!(disabled.ly, 0);
    assert_eq!(disabled.line_dot, 0);
    assert_eq!(disabled.mode, PpuAccessMode::HBlank);
    assert_eq!(machine.read_bus(0x8000), 0x12);
    assert_eq!(machine.read_bus(0xFE00), 0x00);
}

#[test]
fn mode2_selection_on_the_live_machine_preserves_oam_order_and_caps_at_ten_entries() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    for index in 0..12 {
        let x = match index {
            0 => 0,
            1 => 168,
            _ => 8 + index,
        };
        seed_oam_entry(&mut machine, index, 16, x, 0x40 + index, 0);
    }
    seed_oam_entry(&mut machine, 20, 8, 24, 0x99, 0);

    for _ in 0..80 {
        machine.step_t_cycle();
    }

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::Drawing);
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
fn mode2_selection_uses_live_lcdc2_on_the_machine_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 0, 24, 0x10, 0);
    seed_oam_entry(&mut machine, 1, 1, 32, 0x11, 0);

    machine.step_t_cycle();
    machine.step_t_cycle();
    assert!(machine.ppu().snapshot().selected_sprites.is_empty());

    machine.write_bus(0xFF40, 0x95);
    machine.step_t_cycle();
    machine.step_t_cycle();

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 2);
    assert_eq!(snapshot.selected_sprites.len(), 1);
    assert_eq!(snapshot.selected_sprites[0].oam_index, 1);
    assert_eq!(snapshot.selected_sprites[0].y, 1);
}

#[test]
fn bg_only_mode3_produces_visible_pixels_from_vram_on_the_machine_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xAA, 0xCC);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);
    seed_bg_tilemap_entry(&mut machine, 1, 0, 1);

    step_until_line_dot(&mut machine, 252);

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert_eq!(snapshot.mode0_start_dot, 252);
    assert_eq!(snapshot.visible_pixels_output, 160);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 3, 2, 1, 0, 3, 2, 1, 0]
    );
}

#[test]
fn scx_discard_keeps_vram_blocked_until_the_variable_mode3_end() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0x8000, 0x12);
    machine.write_bus(0xFF43, 0x07);

    step_until_line_dot(&mut machine, 252);

    let extended_drawing = machine.ppu().snapshot();
    assert_eq!(extended_drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(extended_drawing.mode0_start_dot, 259);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    step_until_line_dot(&mut machine, 259);

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode0_start_dot, 259);
    assert_eq!(machine.read_bus(0x8000), 0x12);
}

#[test]
fn window_starts_mid_scanline_on_the_live_machine_without_recomputing_the_bg_prefix() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0xF1);
    machine.write_bus(0xFF4A, 0x00);
    machine.write_bus(0xFF4B, 0x0F);

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xCC, 0xF0);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);
    seed_window_tilemap_entry(&mut machine, 0, 0, 1);

    step_until_line_dot(&mut machine, 270);

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert!(snapshot.window_started_this_line);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 3, 3, 2, 2, 1, 1, 0, 0]
    );
}

#[test]
fn window_status_bar_style_activation_uses_the_internal_line_counter_on_later_lines() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0xF1);
    machine.write_bus(0xFF4A, 0x01);
    machine.write_bus(0xFF4B, 0x07);

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xCC, 0xF0);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);
    seed_window_tilemap_entry(&mut machine, 0, 0, 1);

    while !(machine.ppu().snapshot().ly == 1
        && machine.ppu().snapshot().mode == PpuAccessMode::HBlank)
    {
        machine.step_t_cycle();
    }

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.window_line_counter, 0);
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert_eq!(
        &snapshot.current_scanline_pixels[..8],
        &[3, 3, 2, 2, 1, 1, 0, 0]
    );

    while !(machine.ppu().snapshot().ly == 2 && machine.ppu().snapshot().line_dot == 0) {
        machine.step_t_cycle();
    }

    assert_eq!(machine.ppu().snapshot().window_line_counter, 1);
}

#[test]
fn entering_vblank_can_raise_vblank_and_mode1_stat_together() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF45, 0xFF);
    step_until_line_dot(&mut machine, 80);
    machine.write_bus(0xFF41, 0x10);
    machine.write_bus(0xFF0F, 0x00);

    step_until_position(&mut machine, 143, 455);

    let before_vblank = machine.ppu().snapshot();
    assert_eq!(before_vblank.mode, PpuAccessMode::HBlank);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    machine.step_t_cycle();

    let vblank = machine.ppu().snapshot();
    assert_eq!(vblank.ly, 144);
    assert_eq!(vblank.line_dot, 0);
    assert_eq!(vblank.mode, PpuAccessMode::VBlank);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x01);
    assert_eq!(machine.read_bus(0xFF0F), 0xE3);
}

#[test]
fn lcd_reenable_restarts_immediately_but_keeps_the_first_frame_visibly_blank() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);

    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0xFF40, 0x91);

    let restart = machine.ppu().snapshot();
    assert_eq!(restart.lcd_state, PpuLcdState::Enabled);
    assert_eq!(restart.mode, PpuAccessMode::OamScan);
    assert_eq!(restart.ly, 0);
    assert_eq!(restart.line_dot, 4);
    assert_eq!(restart.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(restart.blank_frame_active);

    step_until_line_dot(&mut machine, 252);

    let blank_line = machine.ppu().snapshot();
    assert_eq!(blank_line.mode, PpuAccessMode::HBlank);
    assert_eq!(
        blank_line.visible_output,
        PpuVisibleOutputState::ForcedBlank
    );
    assert_eq!(blank_line.visible_pixels_output, 160);
    assert_eq!(&blank_line.current_scanline_pixels[..8], &[0; 8]);

    step_until_next_frame_start(&mut machine);

    let second_frame_start = machine.ppu().snapshot();
    assert_eq!(
        second_frame_start.visible_output,
        PpuVisibleOutputState::Driving
    );
    assert!(!second_frame_start.blank_frame_active);
    assert_eq!(second_frame_start.mode, PpuAccessMode::OamScan);

    step_until_line_dot(&mut machine, 252);

    let visible_line = machine.ppu().snapshot();
    assert_eq!(visible_line.mode, PpuAccessMode::HBlank);
    assert_eq!(visible_line.visible_output, PpuVisibleOutputState::Driving);
    assert_eq!(&visible_line.current_scanline_pixels[..8], &[2; 8]);
}

#[test]
fn lcd_off_releases_ppu_mode_restrictions_without_overriding_dma_hram_only_blocking() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0x8000, 0x12);
    machine.write_bus(0xFE00, 0x34);
    machine.write_bus(0xFF46, 0x80);
    machine.step_t_cycle();
    machine.step_t_cycle();

    let disabled = machine.ppu().snapshot();
    assert_eq!(disabled.lcd_state, PpuLcdState::Disabled);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);
    assert_eq!(machine.read_bus(0xFF80), 0x00);
}

#[test]
fn live_machine_obj_fetch_stretches_mode3_and_keeps_vram_blocked_until_hblank() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 16, 8, 0, 0);
    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
    machine.write_bus(0xFF40, 0x82);

    step_until_line_dot(&mut machine, 252);

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(drawing.mode0_start_dot, 260);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    step_until_line_dot(&mut machine, 260);

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode0_start_dot, 260);
    assert_eq!(&hblank.current_scanline_pixels[..8], &[2; 8]);
    assert_eq!(machine.read_bus(0x8000), 0x00);
}

#[test]
fn disabling_lcdc1_during_live_object_fetch_keeps_the_timing_cost_but_drops_pixels() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 16, 8, 0, 0);
    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
    machine.write_bus(0xFF40, 0x82);

    loop {
        let fetching = machine.ppu().snapshot();
        if fetching.obj_fetcher_stage == PpuObjFetcherStage::Startup {
            assert_eq!(fetching.mode, PpuAccessMode::Drawing);
            assert!(fetching.line_dot < 96);
            break;
        }
        machine.step_t_cycle();
        assert!(machine.ppu().snapshot().line_dot < 96);
    }

    let fetching = machine.ppu().snapshot();
    assert_eq!(fetching.mode, PpuAccessMode::Drawing);
    assert_eq!(fetching.obj_fetcher_stage, PpuObjFetcherStage::Startup);

    machine.write_bus(0xFF40, 0x80);
    step_until_line_dot(&mut machine, 260);

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode0_start_dot, 260);
    assert_eq!(&hblank.current_scanline_pixels[..8], &[0; 8]);
}

#[test]
fn window_start_keeps_the_obj_fifo_alive_for_final_mixing_on_the_live_machine() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 16, 28, 1, 0x80);
    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0x00);
    seed_bg_tile_row(&mut machine, 1, 0, 0x00, 0xFF);
    seed_bg_tile_row(&mut machine, 2, 0, 0xA0, 0x00);
    seed_window_tilemap_entry(&mut machine, 0, 0, 2);
    machine.write_bus(0xFF40, 0xF3);
    machine.write_bus(0xFF4A, 0x00);
    machine.write_bus(0xFF4B, 0x1F);

    step_until_hblank(&mut machine);

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert!(snapshot.window_started_this_line);
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert_eq!(
        &snapshot.current_scanline_pixels[20..28],
        &[2, 2, 2, 2, 1, 2, 1, 2]
    );
}

#[test]
fn direct_mode2_oam_write_corrupts_the_live_row_without_storing_the_cpu_byte() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let rows = build_oam_corruption_fixture();

    machine.write_bus(0xFF40, 0x00);
    seed_oam_corruption_fixture(&mut machine, &rows);
    machine.write_bus(0xFF40, 0x80);

    while machine.ppu().snapshot().current_oam_scan_row != Some(1) {
        machine.step_t_cycle();
    }

    let row = machine.ppu().snapshot().current_oam_scan_row.unwrap();
    machine.write_bus(0xFE20, 0x99);
    machine.write_bus(0xFF40, 0x00);

    assert_eq!(
        read_oam_corruption_row(&mut machine, row),
        expected_write_corruption(&rows, row)
    );
}

#[test]
fn direct_mode2_fea0_read_uses_blocked_readback_and_the_same_read_corruption_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let rows = build_oam_corruption_fixture();

    machine.write_bus(0xFF40, 0x00);
    seed_oam_corruption_fixture(&mut machine, &rows);
    machine.write_bus(0xFF40, 0x80);

    while machine.ppu().snapshot().current_oam_scan_row != Some(2) {
        machine.step_t_cycle();
    }

    let row = machine.ppu().snapshot().current_oam_scan_row.unwrap();
    assert_eq!(machine.read_bus(0xFEA0), 0xFF);
    machine.write_bus(0xFF40, 0x00);

    assert_eq!(
        read_oam_corruption_row(&mut machine, row),
        expected_read_corruption(&rows, row)
    );
}

#[test]
fn cpu_inc_hl_inside_fe_range_reaches_the_same_mode2_corruption_controller() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let rows = build_oam_corruption_fixture();

    machine
        .load_cartridge(build_test_rom(
            &[0x21, 0x08, 0xFE, 0x00, 0x00, 0x00, 0x00, 0x23, 0x00],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF40, 0x00);
    seed_oam_corruption_fixture(&mut machine, &rows);

    for _ in 0..12 {
        machine.step_t_cycle();
    }

    machine.write_bus(0xFF40, 0x80);

    let mut triggered_row = None;
    for _ in 0..80 {
        machine.step_t_cycle();
        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::IncDec
            && event.idu_address == Some(0xFE09)
            && event.update_direction == Some(CpuAddressUpdateDirection::Increment)
        {
            triggered_row = machine.ppu().snapshot().current_oam_scan_row;
            break;
        }
    }

    let row = triggered_row.expect("INC HL should trigger during Mode 2");
    assert!(row > 0);
    machine.write_bus(0xFF40, 0x00);

    assert_eq!(
        read_oam_corruption_row(&mut machine, row),
        expected_write_corruption(&rows, row)
    );
}
