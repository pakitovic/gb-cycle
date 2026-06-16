mod common;

use common::machine_driver::run_until_halted;
use common::synthetic_cartridge::{
    HEADER_MINIMUM_ROM_LEN, build_nom_bc_test_rom, build_nom_bc_test_rom_with_program_entry,
};
use gb_core::{
    CompatibilityPolicy, ConsoleModel, CpuAddressEventKind, CpuAddressUpdateDirection,
    CpuBusAccessKind, Machine, MachineConfig, OperatingMode, PpuAccessMode, PpuBgFetcherSource,
    PpuLcdState, PpuObjFetcherStage, PpuSnapshot, PpuVisibleOutputState, StartupMode,
};

include!("ppu/ppu_setup.rs");
include!("ppu/ppu_probe_lcd.rs");
include!("ppu/ppu_probe_intr20.rs");
include!("ppu/ppu_probe_stat_mode.rs");
include!("ppu/ppu_probe_hblank_scx.rs");

#[path = "ppu/ppu_lcd_restart.rs"]
mod ppu_lcd_restart;
#[path = "ppu/ppu_mode_edges.rs"]
mod ppu_mode_edges;
#[path = "ppu/ppu_oam_dma.rs"]
mod ppu_oam_dma;
#[path = "ppu/ppu_oracle_sweep.rs"]
mod ppu_oracle_sweep;

fn step_until_visible_pixels_output_on_line(
    machine: &mut Machine,
    target_ly: u8,
    min_visible_pixels_output: u8,
) {
    let mut stepped_t_cycles = 0u32;
    loop {
        let snapshot = machine.ppu().snapshot();
        if snapshot.ly == target_ly
            && snapshot.mode == PpuAccessMode::Drawing
            && snapshot.visible_pixels_output >= min_visible_pixels_output
        {
            return;
        }

        machine.step_t_cycle();
        stepped_t_cycles += 1;
        assert!(
            stepped_t_cycles < 2_000,
            "did not reach ly={} drawing with {} visible pixels in time; last snapshot={:?}",
            target_ly,
            min_visible_pixels_output,
            machine.ppu().snapshot()
        );
    }
}

fn cgb_native_machine() -> Machine {
    let mut rom = build_test_rom(&[0x18, 0xFE], 0x00);
    rom[0x0143] = 0x80;
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor)
            .with_operating_mode(OperatingMode::Cgb)
            .with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("CGB native test ROM should load");
    machine
}

fn cgb_dmg_ext_machine() -> Machine {
    let mut rom = build_test_rom(&[0x18, 0xFE], 0x00);
    rom[0x0143] = 0x88;
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor)
            .with_compatibility(CompatibilityPolicy::experimental())
            .with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("CGB DMG-ext test ROM should load under the experimental policy");
    assert_eq!(machine.config().operating_mode, OperatingMode::CgbDmgExt);
    machine
}

fn step_until_drawing_cpu_visible(machine: &mut Machine) {
    let mut stepped_t_cycles = 0u32;
    loop {
        let snapshot = machine.ppu().snapshot();
        if snapshot.mode == PpuAccessMode::Drawing && snapshot.line_dot >= 82 {
            return;
        }

        machine.step_t_cycle();
        stepped_t_cycles += 1;
        assert!(
            stepped_t_cycles < 2_000,
            "did not reach CPU-visible Mode 3 in time; last snapshot={:?}",
            machine.ppu().snapshot()
        );
    }
}

fn step_until_hblank_cpu_visible(machine: &mut Machine) {
    let mut stepped_t_cycles = 0u32;
    loop {
        let snapshot = machine.ppu().snapshot();
        if snapshot.mode == PpuAccessMode::HBlank && snapshot.line_dot > snapshot.mode0_start_dot {
            return;
        }

        machine.step_t_cycle();
        stepped_t_cycles += 1;
        assert!(
            stepped_t_cycles < 2_000,
            "did not reach CPU-visible HBlank in time; last snapshot={:?}",
            machine.ppu().snapshot()
        );
    }
}

fn write_cgb_vram_bank(machine: &mut Machine, bank: u8, address: u16, value: u8) {
    machine.write_bus(0xFF4F, bank);
    machine.write_bus(address, value);
}

fn run_live_bgp_write_prefix(config: MachineConfig) -> (Vec<u8>, Vec<u8>) {
    let mut machine = Machine::new(config);
    machine.write_bus(0xFF40, 0x91);
    machine.write_bus(0xFF42, 0x00);
    machine.write_bus(0xFF43, 0x00);
    machine.write_bus(0xFF47, 0x01);

    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0x00);
    for tile_x in 0..4 {
        seed_bg_tilemap_entry(&mut machine, tile_x, 0, 0);
    }

    step_until_visible_pixels_output_on_line(&mut machine, 0, 8);

    let before = machine.ppu().framebuffer()[..8].to_vec();
    machine.write_bus(0xFF47, 0x12);
    let after = machine.ppu().framebuffer()[..8].to_vec();
    (before, after)
}

#[test]
fn cgb_palette_data_ports_block_only_data_during_cpu_visible_mode3() {
    let mut machine = cgb_native_machine();

    machine.write_bus(0xFF68, 0x82);
    machine.write_bus(0xFF69, 0x56);
    machine.write_bus(0xFF68, 0x82);
    assert_eq!(machine.read_bus(0xFF69), 0x56);

    machine.write_bus(0xFF6A, 0x80);
    machine.write_bus(0xFF6B, 0x34);
    machine.write_bus(0xFF6A, 0x80);
    assert_eq!(machine.read_bus(0xFF6B), 0x34);

    step_until_drawing_cpu_visible(&mut machine);
    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);

    assert_eq!(machine.read_bus(0xFF69), 0xFF);
    assert_eq!(machine.read_bus(0xFF6B), 0xFF);

    machine.write_bus(0xFF69, 0xA5);
    assert_eq!(
        machine.read_bus(0xFF68),
        0xC3,
        "blocked BCPD writes must still honor BCPS auto-increment"
    );
    machine.write_bus(0xFF6B, 0xB6);
    assert_eq!(
        machine.read_bus(0xFF6A),
        0xC1,
        "blocked OCPD writes must still honor OCPS auto-increment"
    );

    machine.write_bus(0xFF68, 0x04);
    assert_eq!(
        machine.read_bus(0xFF68),
        0x44,
        "BCPS index writes must not be blocked by Mode 3 palette-data rules"
    );
    machine.write_bus(0xFF69, 0xC7);
    assert_eq!(
        machine.read_bus(0xFF68),
        0x44,
        "blocked data writes without auto-increment must not move the index"
    );

    step_until_hblank_cpu_visible(&mut machine);
    machine.write_bus(0xFF68, 0x82);
    assert_eq!(
        machine.read_bus(0xFF69),
        0x56,
        "Mode 3 BCPD write must not change palette RAM"
    );
    machine.write_bus(0xFF6A, 0x80);
    assert_eq!(
        machine.read_bus(0xFF6B),
        0x34,
        "Mode 3 OCPD write must not change palette RAM"
    );

    machine.write_bus(0xFF68, 0x04);
    machine.write_bus(0xFF69, 0x9A);
    machine.write_bus(0xFF68, 0x04);
    assert_eq!(
        machine.read_bus(0xFF69),
        0x9A,
        "palette data writes outside Mode 3 remain visible"
    );
}

#[test]
fn cgb_dmg_ext_palette_ports_expose_indexes_but_block_palette_ram_data() {
    let mut machine = cgb_dmg_ext_machine();

    machine.write_bus(0xFF68, 0x85);
    assert_eq!(machine.read_bus(0xFF68), 0xC5);
    assert_eq!(machine.read_bus(0xFF69), 0xFF);
    machine.write_bus(0xFF69, 0x56);
    assert_eq!(
        machine.read_bus(0xFF68),
        0xC5,
        "DMG-ext must not auto-increment BCPS through blocked BCPD writes"
    );
    assert_eq!(machine.read_bus(0xFF69), 0xFF);

    machine.write_bus(0xFF6A, 0x83);
    assert_eq!(machine.read_bus(0xFF6A), 0xC3);
    assert_eq!(machine.read_bus(0xFF6B), 0xFF);
    machine.write_bus(0xFF6B, 0x34);
    assert_eq!(
        machine.read_bus(0xFF6A),
        0xC3,
        "DMG-ext must not auto-increment OCPS through blocked OCPD writes"
    );
    assert_eq!(machine.read_bus(0xFF6B), 0xFF);

    assert_eq!(machine.read_bus(0xFF6C), 0xFF);
    machine.write_bus(0xFF6C, 0x00);
    assert_eq!(machine.read_bus(0xFF6C), 0xFE);
}

#[test]
fn cgb_cpu_vram_access_tracks_the_mode3_seam_and_retains_failed_writes() {
    let mut machine = cgb_native_machine();
    machine.write_bus(0x8000, 0x12);

    machine.step_t_cycle();
    assert_eq!(machine.ppu().snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(machine.read_bus(0x8000), 0x12);
    machine.write_bus(0x8000, 0x34);
    assert_eq!(machine.read_bus(0x8000), 0x34);

    step_until_drawing_cpu_visible(&mut machine);
    assert_eq!(
        machine.read_bus(0x8000),
        0xFF,
        "CPU VRAM reads must be blocked while Mode 3 is CPU-visible"
    );
    machine.write_bus(0x8000, 0x56);

    step_until_hblank(&mut machine);
    assert_eq!(
        machine.read_bus(0x8000),
        0x34,
        "CPU VRAM writes attempted during Mode 3 must be ignored"
    );
    machine.write_bus(0x8000, 0x78);
    assert_eq!(machine.read_bus(0x8000), 0x78);
}

#[test]
fn cgb_bg_fetch_latches_bank1_attributes_and_ignores_cpu_vbk_for_ppu_reads() {
    let mut machine = cgb_native_machine();
    machine.write_bus(0xFF40, 0x00);

    write_cgb_vram_bank(&mut machine, 0, 0x9800, 0x02);
    write_cgb_vram_bank(&mut machine, 0, 0x8020, 0x00);
    write_cgb_vram_bank(&mut machine, 0, 0x8021, 0x00);
    write_cgb_vram_bank(&mut machine, 1, 0x9800, 0x09);
    write_cgb_vram_bank(&mut machine, 1, 0x8020, 0xF0);
    write_cgb_vram_bank(&mut machine, 1, 0x8021, 0x00);
    assert_eq!(machine.debug_vram_bytes()[0x1800], 0x02);
    assert_eq!(machine.debug_vram_bytes()[0x2000 + 0x1800], 0x09);
    assert_eq!(machine.debug_vram_bytes()[0x2000 + 0x0020], 0xF0);

    machine.write_bus(0xFF4F, 0x01);
    machine.write_bus(0xFF42, 0x00);
    machine.write_bus(0xFF43, 0x00);
    machine.write_bus(0xFF40, 0x91);

    let mut stepped_t_cycles = 0u32;
    let latched = loop {
        let snapshot = machine.ppu().snapshot();
        if let Some(pixel) = snapshot
            .bg_fifo_cached_pixels
            .iter()
            .flatten()
            .copied()
            .find(|pixel| pixel.cgb_bg_attrs == Some(0x09))
        {
            break pixel;
        }

        machine.step_t_cycle();
        stepped_t_cycles += 1;
        assert!(
            stepped_t_cycles < 2_000,
            "did not observe first CGB BG cached slice; last snapshot={:?}",
            machine.ppu().snapshot()
        );
    };

    assert_eq!(
        latched.tile_map_address, 0x1800,
        "the first CGB BG tile-map fetch should use the 9800h map offset"
    );
    assert_eq!(
        latched.tile_index, 0x02,
        "PPU BG tile-number fetch must use VRAM bank 0 even while CPU VBK selects bank 1"
    );
    assert_eq!(
        latched.cgb_bg_attrs,
        Some(0x09),
        "PPU BG fetch must latch the raw attributes from VRAM bank 1"
    );
    assert_eq!(machine.read_bus(0xFF4F), 0xFF);
}

#[test]
fn live_machine_bus_access_uses_the_current_ppu_mode_from_the_raster_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
fn skip_boot_ppu_state_continues_from_the_published_snapshot_on_the_shared_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
fn bg_only_mode3_produces_visible_pixels_from_vram_on_the_machine_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
fn cgb_compatibility_machine_uses_cgb_bgp_conflicts_without_dmg_family_quirks() {
    let dmg_config =
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot);
    let cgb_compat_config = MachineConfig::new(ConsoleModel::GameBoyColor)
        .with_operating_mode(OperatingMode::GbCompatible)
        .with_startup_mode(StartupMode::SkipBoot);

    let (dmg_before, dmg_after) = run_live_bgp_write_prefix(dmg_config);
    let (cgb_before, cgb_after) = run_live_bgp_write_prefix(cgb_compat_config.clone());

    let capabilities = cgb_compat_config.capability_set();
    assert!(capabilities.dmg_software_contract());
    assert!(!capabilities.dmg_family_quirks_enabled());

    assert_eq!(dmg_before, vec![1; 8]);
    assert_ne!(dmg_after, dmg_before);
    assert_eq!(cgb_before, vec![1; 8]);
    assert_eq!(cgb_after, vec![1, 1, 1, 1, 2, 2, 2, 2]);
}

#[test]
fn scx_discard_keeps_vram_blocked_until_the_variable_mode3_end() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
fn live_machine_obj_fetch_stretches_mode3_and_keeps_vram_blocked_until_hblank() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 16, 8, 0, 0);
    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
    machine.write_bus(0xFF40, 0x82);

    step_until_line_dot(&mut machine, 252);

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert!(drawing.mode0_start_dot > 252);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    let mode0_start_dot = drawing.mode0_start_dot;
    step_until_line_dot(&mut machine, mode0_start_dot);

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode0_start_dot, mode0_start_dot);
    assert_eq!(&hblank.current_scanline_pixels[..8], &[2; 8]);
    assert_eq!(machine.read_bus(0x8000), 0x00);
}

#[test]
fn disabling_lcdc1_during_live_object_fetch_keeps_the_timing_cost_but_drops_pixels() {
    fn run_case(disable_obj_during_fetch: bool) -> PpuSnapshot {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
        );

        seed_oam_entry(&mut machine, 0, 16, 8, 0, 0);
        seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
        machine.write_bus(0xFF40, 0x82);

        let mut waited_t_cycles = 0;
        loop {
            let fetching = machine.ppu().snapshot();
            if fetching.obj_fetcher_stage != PpuObjFetcherStage::Idle {
                assert_eq!(fetching.mode, PpuAccessMode::Drawing);
                assert!(fetching.visible_pixels_output <= 1);
                break;
            }

            machine.step_t_cycle();
            waited_t_cycles += 1;
            assert!(
                waited_t_cycles < 160,
                "left-edge OBJ fetch should begin during the first visible scanline"
            );
        }

        let fetching = machine.ppu().snapshot();
        assert_eq!(fetching.mode, PpuAccessMode::Drawing);
        assert_ne!(fetching.obj_fetcher_stage, PpuObjFetcherStage::Idle);

        if disable_obj_during_fetch {
            machine.write_bus(0xFF40, 0x80);
        }

        step_until_hblank(&mut machine);

        let hblank = machine.ppu().snapshot();
        assert_eq!(hblank.mode, PpuAccessMode::HBlank);
        hblank
    }

    let enabled = run_case(false);
    let disabled = run_case(true);

    assert_eq!(disabled.mode0_start_dot, enabled.mode0_start_dot);
    assert_ne!(enabled.current_scanline_pixels[0], 0);
    assert_eq!(&disabled.current_scanline_pixels[..8], &[0; 8]);
}

#[test]
fn window_start_keeps_the_obj_fifo_alive_for_final_mixing_on_the_live_machine() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
